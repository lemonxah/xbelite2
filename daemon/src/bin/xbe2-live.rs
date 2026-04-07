// Interactive GIP session — stays connected so you can switch profiles
// on the controller and read data per-profile.

use std::io::Write;
use std::time::Duration;

const VID: u16 = 0x045E;
const PID: u16 = 0x0B00;
const EP_OUT: u8 = 0x02;
const EP_IN: u8 = 0x82;

static SEQ: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0x10);

fn next_seq() -> u8 {
    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn main() {
    println!("=== Xbox Elite 2 Live GIP Session ===");
    println!("Stay connected — switch profiles on the controller, then type commands.\n");

    let handle = rusb::open_device_with_vid_pid(VID, PID)
        .expect("Elite 2 not found on USB");

    if handle.kernel_driver_active(0).unwrap_or(false) {
        handle.detach_kernel_driver(0).expect("Failed to detach kernel driver");
    }
    handle.claim_interface(0).expect("Failed to claim interface");

    // Do GIP handshake — read HELLO, send power-on
    println!("Performing GIP handshake...");
    drain_print(&handle, Duration::from_millis(1000));

    // Send power-on (Set Device State = Start)
    let power_on = [0x05, 0x20, next_seq(), 0x01, 0x00];
    let _ = handle.write_interrupt(EP_OUT, &power_on, Duration::from_millis(500));
    println!("Sent power-on");

    // Send Elite 2 init (enable extended reports)
    let elite_init = [0x4D, 0x10, next_seq(), 0x02, 0x07, 0x00];
    let _ = handle.write_interrupt(EP_OUT, &elite_init, Duration::from_millis(500));
    println!("Sent Elite 2 init (0x4D sub 0x07)");

    drain_print(&handle, Duration::from_millis(500));
    println!("\nReady. Type commands:");
    println!("  name         Read device/profile name");
    println!("  cal          Read calibration data");
    println!("  info         Read 0x4D sub 0x03 (size/capability)");
    println!("  slot         Read 0x4D sub 0x05 (data slot)");
    println!("  flags        Read 0x4D sub 0x06 (profile flags)");
    println!("  all4d        Read all 0x4D sub-commands");
    println!("  all1e        Read interesting 0x1E sub-commands");
    println!("  input        Read 5 seconds of input (shows profile switches)");
    println!("  raw XX YY..  Send raw bytes as 0x4D command");
    println!("  quit         Exit\n");

    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() { break; }
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "quit" | "q" | "exit" => break,

            "name" => {
                let resp = gip_1e(&handle, &[0x05]);
                if let Some(data) = resp {
                    if data.len() > 9 {
                        let name_bytes = &data[9..];
                        let u16s: Vec<u16> = name_bytes.chunks(2)
                            .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                            .collect();
                        let name = String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string();
                        if name.is_empty() {
                            println!("  Name: (empty)");
                        } else {
                            println!("  Name: \"{name}\"");
                        }
                    }
                    println!("  Raw: {:02x?}", &data[4..]);
                }
            }

            "cal" => {
                let resp = gip_1e(&handle, &[0x0F]);
                if let Some(data) = resp {
                    let payload = &data[4..];
                    println!("  Sub: 0x{:02x}, Status: 0x{:02x}", payload[0], payload[1]);
                    if payload.len() >= 4 {
                        let vals: Vec<u16> = payload[2..].chunks(2)
                            .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                            .collect();
                        println!("  Values: {:?}", vals);
                    }
                    println!("  Raw: {:02x?}", payload);
                }
            }

            "info" => {
                let resp = gip_4d(&handle, &[0x03]);
                if let Some(data) = resp {
                    println!("  {:02x?}", &data[4..]);
                }
            }

            "slot" => {
                let resp = gip_4d(&handle, &[0x05]);
                if let Some(data) = resp {
                    let payload = &data[4..];
                    let nonzero = payload.iter().filter(|b| **b != 0).count();
                    println!("  {} bytes, {} non-zero", payload.len(), nonzero);
                    if nonzero > 1 {
                        println!("  {:02x?}", payload);
                    }
                }
            }

            "flags" => {
                for p in 0..4u8 {
                    let resp = gip_4d(&handle, &[0x06, p]);
                    if let Some(data) = resp {
                        println!("  profile {p}: {:02x?}", &data[4..]);
                    }
                }
            }

            "all4d" => {
                for sub in 0x00..=0x0Au8 {
                    let resp = gip_4d(&handle, &[sub]);
                    if let Some(data) = resp {
                        let payload = &data[4..];
                        println!("  0x4D sub 0x{sub:02x}: {:02x?}", payload);
                    } else {
                        println!("  0x4D sub 0x{sub:02x}: (no response)");
                    }
                }
            }

            "all1e" => {
                for sub in [0x02u8, 0x04, 0x05, 0x06, 0x08, 0x09, 0x0A, 0x0C, 0x0D, 0x0F, 0x10] {
                    let resp = gip_1e(&handle, &[sub]);
                    if let Some(data) = resp {
                        let payload = &data[4..];
                        // Decode name for sub 0x05
                        if sub == 0x05 && payload.len() > 5 {
                            let u16s: Vec<u16> = payload[5..].chunks(2)
                                .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                                .collect();
                            let name = String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string();
                            println!("  0x1E sub 0x{sub:02x}: name=\"{name}\" raw={:02x?}", payload);
                        } else {
                            println!("  0x1E sub 0x{sub:02x}: {:02x?}", payload);
                        }
                    } else {
                        println!("  0x1E sub 0x{sub:02x}: (no response)");
                    }
                }
            }

            "input" => {
                println!("  Reading input for 5 seconds. Switch profiles now!");
                let start = std::time::Instant::now();
                let mut last_profile: Option<u8> = None;
                while start.elapsed() < Duration::from_secs(5) {
                    let mut buf = [0u8; 64];
                    match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(100)) {
                        Ok(n) => {
                            if buf[0] == 0x20 && n >= 18 {
                                // GIP input report — extract profile from extended data
                                // Standard GIP input is 18 bytes (header 4 + payload 14)
                                // Elite extended data follows
                                if n > 18 {
                                    let profile = buf[18]; // or wherever the profile byte is
                                    if last_profile != Some(profile) {
                                        println!("  Profile byte at [18]: {} (raw bytes [18..]: {:02x?})", profile, &buf[18..n]);
                                        last_profile = Some(profile);
                                    }
                                }
                            } else if buf[0] == 0x4D {
                                println!("  0x4D message: {:02x?}", &buf[..n]);
                            } else if buf[0] != 0x01 && buf[0] != 0x02 {
                                println!("  cmd=0x{:02x}: {:02x?}", buf[0], &buf[..n.min(20)]);
                            }
                        }
                        Err(_) => {}
                    }
                }
                println!("  Done.");
            }

            "raw" => {
                if parts.len() < 2 {
                    println!("  Usage: raw XX YY ZZ (hex bytes as 0x4D payload)");
                    continue;
                }
                let bytes: Vec<u8> = parts[1..].iter()
                    .filter_map(|s| u8::from_str_radix(s, 16).ok())
                    .collect();
                println!("  Sending 0x4D with payload: {:02x?}", bytes);
                let resp = gip_4d(&handle, &bytes);
                if let Some(data) = resp {
                    println!("  Response: {:02x?}", &data[4..]);
                } else {
                    println!("  (no data response)");
                }
            }

            "raw1e" => {
                if parts.len() < 2 {
                    println!("  Usage: raw1e XX YY ZZ (hex bytes as 0x1E payload)");
                    continue;
                }
                let bytes: Vec<u8> = parts[1..].iter()
                    .filter_map(|s| u8::from_str_radix(s, 16).ok())
                    .collect();
                println!("  Sending 0x1E with payload: {:02x?}", bytes);
                let resp = gip_1e(&handle, &bytes);
                if let Some(data) = resp {
                    println!("  Response: {:02x?}", &data[4..]);
                } else {
                    println!("  (no data response)");
                }
            }

            "writename" => {
                let text = if parts.len() > 1 { parts[1..].join(" ") } else { "Test".to_string() };
                let truncated: String = text.chars().take(15).collect();
                let u16s: Vec<u16> = truncated.encode_utf16().collect();
                let mut name_buf = vec![0u8; 32];
                for (i, c) in u16s.iter().enumerate() {
                    let b = c.to_le_bytes();
                    name_buf[i * 2] = b[0];
                    name_buf[i * 2 + 1] = b[1];
                }
                println!("  Writing name: \"{truncated}\" via 0x1E sub 0x05");
                let mut payload = vec![0x05];
                payload.extend_from_slice(&name_buf);
                let resp = gip_1e(&handle, &payload);
                if let Some(data) = resp {
                    println!("  Response: {:02x?}", &data[4..]);
                } else {
                    println!("  (no response — might be write-only, checking readback...)");
                }
                let resp2 = gip_1e(&handle, &[0x05]);
                if let Some(data) = resp2 {
                    if data.len() > 9 {
                        let nb = &data[9..];
                        let u16r: Vec<u16> = nb.chunks(2)
                            .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                            .collect();
                        let readback = String::from_utf16_lossy(&u16r).trim_end_matches('\0').to_string();
                        println!("  Readback: \"{readback}\"");
                    }
                }
            }

            "led" => {
                let intensity: u8 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
                let pattern: u8 = parts.get(2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0x01);
                println!("  LED: intensity={intensity} pattern=0x{pattern:02x}");
                let cmd = [0x0A, 0x20, next_seq(), 0x03, 0x00, pattern, intensity];
                let _ = handle.write_interrupt(EP_OUT, &cmd, Duration::from_millis(500));
                drain(&handle);
            }

            _ => println!("  Unknown command. Type 'quit' to exit."),
        }
    }

    let _ = handle.release_interface(0);
    println!("Disconnected.");
}

fn gip_1e(handle: &rusb::DeviceHandle<rusb::GlobalContext>, payload: &[u8]) -> Option<Vec<u8>> {
    let mut pkt = vec![0x1E, 0x30, next_seq(), payload.len() as u8];
    pkt.extend_from_slice(payload);
    handle.write_interrupt(EP_OUT, &pkt, Duration::from_millis(500)).ok()?;

    for _ in 0..15 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
            Ok(n) => {
                if buf[0] == 0x20 || buf[0] == 0x02 || buf[0] == 0x01 { continue; }
                if buf[0] == 0x1E { return Some(buf[..n].to_vec()); }
            }
            Err(_) => break,
        }
    }
    drain(&handle);
    None
}

fn gip_4d(handle: &rusb::DeviceHandle<rusb::GlobalContext>, payload: &[u8]) -> Option<Vec<u8>> {
    let mut pkt = vec![0x4D, 0x10, next_seq(), payload.len() as u8];
    pkt.extend_from_slice(payload);
    handle.write_interrupt(EP_OUT, &pkt, Duration::from_millis(500)).ok()?;

    for _ in 0..15 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
            Ok(n) => {
                if buf[0] == 0x20 || buf[0] == 0x02 || buf[0] == 0x01 { continue; }
                if buf[0] == 0x4D { return Some(buf[..n].to_vec()); }
            }
            Err(_) => break,
        }
    }
    drain(&handle);
    None
}

fn drain(handle: &rusb::DeviceHandle<rusb::GlobalContext>) {
    loop {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(30)) {
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

fn drain_print(handle: &rusb::DeviceHandle<rusb::GlobalContext>, duration: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < duration {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(100)) {
            Ok(n) => {
                if buf[0] != 0x20 { // skip gamepad input
                    let name = match buf[0] {
                        0x01 => "ACK", 0x02 => "HELLO", 0x03 => "STATUS",
                        0x04 => "METADATA", 0x07 => "GUIDE", 0x4D => "VENDOR",
                        _ => "?",
                    };
                    println!("  [{name}] {:02x?}", &buf[..n.min(20)]);
                }
            }
            Err(_) => {}
        }
    }
}
