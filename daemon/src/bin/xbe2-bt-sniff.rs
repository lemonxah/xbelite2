//! Bluetooth HID report sniffer for the Xbox Elite 2.
//! Shows raw bytes from hidraw so we can see exactly what the controller sends over BT.
//!
//! Run: sudo cargo run --bin xbe2-bt-sniff
//!
//! Steps:
//! 1. Unplug USB cable
//! 2. Press Xbox button to connect via BT
//! 3. Run this tool
//! 4. Press buttons/paddles and observe raw bytes

use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::Path;

#[repr(C)]
#[derive(Default)]
struct HidrawDevinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

nix::ioctl_read!(hidiocgrawinfo, b'H', 0x03, HidrawDevinfo);
nix::ioctl_read_buf!(hidiocgrawname, b'H', 0x04, u8);

fn main() {
    println!("=== Xbox Elite 2 Bluetooth HID Report Sniffer ===\n");

    // Scan hidraw devices
    let mut found = None;
    let entries = fs::read_dir("/dev").expect("read /dev");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("hidraw") {
            continue;
        }

        if let Some(info) = probe(&path) {
            println!("Found: {} at {} (VID:{:04x} PID:{:04x} bus:{})",
                info.0, path.display(), info.1, info.2, info.3);
            // Microsoft vendor, Elite 2 PIDs (including 0x028E spoofed by BT HID)
            // bus:5 = BUS_BLUETOOTH
            if info.1 == 0x045E && (info.2 == 0x0B05 || info.2 == 0x0B22 || info.2 == 0x0B00 || info.2 == 0x028E) {
                found = Some(path.clone());
            }
        }
    }

    // Also scan evdev if no hidraw found (USB via xpad won't have hidraw)
    if found.is_none() {
        println!("\nNo Elite 2 hidraw device found.");
        println!("Over USB, the controller uses GIP protocol (no hidraw).");
        println!("For BT sniffing:");
        println!("  1. Unplug USB cable");
        println!("  2. Press Xbox button on controller to connect via BT");
        println!("  3. Run this tool again");
        println!("\nListing all hidraw devices:");
        list_all_hidraw();
        std::process::exit(1);
    }

    let path = found.unwrap();
    println!("\nOpening {}...\n", path.display());

    let mut file = OpenOptions::new()
        .read(true)
        .open(&path)
        .expect("Failed to open hidraw device");

    println!("Reading raw HID reports. Press buttons, paddles, sticks...");
    println!("Format: [report_id] byte0 byte1 byte2 ...\n");
    println!("{:<8} {:<6} {}", "REPORT#", "SIZE", "HEX BYTES");
    println!("{}", "-".repeat(80));

    let mut buf = [0u8; 128];
    let mut report_num = 0u64;
    let mut prev_report = [0u8; 128];
    let mut prev_len = 0;

    loop {
        let n = match file.read(&mut buf) {
            Ok(n) if n > 0 => n,
            Ok(_) => break,
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        };

        report_num += 1;

        // Only show if different from previous (skip duplicate axis noise)
        if n == prev_len && buf[..n] == prev_report[..n] {
            continue;
        }

        // Show the full report
        let hex: String = buf[..n]
            .iter()
            .enumerate()
            .map(|(i, b)| {
                // Highlight bytes that changed
                if i < prev_len && prev_report[i] != *b {
                    format!("[{:02x}]", b)
                } else {
                    format!(" {:02x} ", b)
                }
            })
            .collect();

        println!("{:<8} {:<6} {}", report_num, n, hex);

        // Show decoded info for known fields
        if n >= 16 {
            let report_id = buf[0];
            if report_id == 0x01 {
                // Likely gamepad report
                decode_gamepad_report(&buf[..n]);
            }
        }

        prev_report[..n].copy_from_slice(&buf[..n]);
        prev_len = n;
    }
}

fn decode_gamepad_report(data: &[u8]) {
    println!("  decoded: report_id={:#04x} buttons=[{:08b} {:08b}]",
        data[0],
        data.get(1).copied().unwrap_or(0),
        data.get(2).copied().unwrap_or(0));

    // Try to find paddle bytes by scanning for non-zero bytes beyond standard gamepad data
    if data.len() > 16 {
        print!("  extra bytes (offset 16+):");
        for (i, b) in data[16..].iter().enumerate() {
            if *b != 0 {
                print!(" [{}]={:#04x}", 16 + i, b);
            }
        }
        println!();
    }
}

fn probe(path: &Path) -> Option<(String, u16, u16, u32)> {
    let file = OpenOptions::new().read(true).open(path).ok()?;
    let fd = file.as_raw_fd();

    let mut info = HidrawDevinfo::default();
    unsafe { hidiocgrawinfo(fd, &mut info).ok()? };

    let mut name_buf = [0u8; 256];
    let name = match unsafe { hidiocgrawname(fd, &mut name_buf) } {
        Ok(len) => String::from_utf8_lossy(&name_buf[..(len as usize).min(255)])
            .trim_end_matches('\0')
            .to_string(),
        Err(_) => String::from("Unknown"),
    };

    Some((name, info.vendor as u16, info.product as u16, info.bustype))
}

fn list_all_hidraw() {
    let entries = fs::read_dir("/dev").expect("read /dev");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("hidraw") {
            continue;
        }
        if let Some(info) = probe(&path) {
            println!("  {}: {} (VID:{:04x} PID:{:04x} bus:{})",
                path.display(), info.0, info.1, info.2, info.3);
        }
    }
}
