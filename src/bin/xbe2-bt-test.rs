//! BT end-to-end test: read from hidraw, parse paddles, emit to virtual gamepad.
//!
//! Run: sudo cargo run --bin xbe2-bt-test
//!
//! Connect the Elite 2 via Bluetooth first.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use xbelite2::transform;
use xbelite2::types::*;
use xbelite2::uinput::VirtualGamepad;

#[repr(C)]
#[derive(Default)]
struct HidrawDevinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

nix::ioctl_read!(hidiocgrawinfo, b'H', 0x03, HidrawDevinfo);
nix::ioctl_read_buf!(hidiocgrawname, b'H', 0x04, u8);

const BUS_BLUETOOTH: u32 = 5;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Xbox Elite 2 Bluetooth Driver Test ===\n");

    // Find the BT hidraw device
    let hidraw_path = find_elite2_bt_hidraw();

    println!("Opening {}...", hidraw_path);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&hidraw_path)
        .expect("Failed to open hidraw");

    // Create virtual gamepad with paddles
    let mut gamepad = VirtualGamepad::new(0).expect("Failed to create virtual gamepad");
    println!("Created virtual gamepad: Xbox Elite 2 (xbelite2 #0)\n");

    // Also grab the evdev device to prevent duplicate events from hid-generic
    let _grab = grab_evdev_for_hidraw(&hidraw_path);

    let profile = Profile::default();
    let mut prev_state = GamepadState::default();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed)).unwrap();

    println!("Forwarding BT HID events (Ctrl+C to stop)...");
    println!("Paddles work in ALL profiles via raw HID parsing.\n");
    println!("{:<15} {:<15} {}", "INPUT", "CODE", "VALUE");
    println!("{}", "-".repeat(50));

    let mut buf = [0u8; 128];

    while running.load(Ordering::Relaxed) {
        let n = match file.read(&mut buf) {
            Ok(n) if n > 0 => n,
            Ok(_) => break,
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        };

        // Only process report ID 0x01 (gamepad reports)
        if buf[0] != 0x01 {
            continue;
        }

        let current = parse_report(&buf[..n]);

        // Generate output events via transform engine
        let events = transform::transform(&current, &prev_state, &profile);

        if !events.is_empty() {
            // Print what changed
            for ev in &events {
                let name = format_event(ev);
                println!("{:<15} {:<15} {}", "BT->UINPUT", name, ev.value);
            }

            if let Err(e) = gamepad.emit(&events) {
                eprintln!("Emit error: {e}");
            }
        }

        prev_state = current;
    }

    println!("\nDone.");
}

/// Parse the raw 20-byte BT HID report into GamepadState.
/// Uses confirmed byte offsets from real hardware capture.
fn parse_report(data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();

    if data.len() < 15 {
        return state;
    }

    // Byte 1: buttons low
    let b0 = data[1];
    state.btn_a = b0 & 0x01 != 0;
    state.btn_b = b0 & 0x02 != 0;
    state.btn_x = b0 & 0x08 != 0;
    state.btn_y = b0 & 0x10 != 0;
    state.btn_lb = b0 & 0x40 != 0;
    state.btn_rb = b0 & 0x80 != 0;

    // Byte 2: more buttons + hat
    let b1 = data[2];
    state.btn_view = b1 & 0x04 != 0;
    state.btn_menu = b1 & 0x08 != 0;
    state.btn_lstick = b1 & 0x20 != 0;
    state.btn_rstick = b1 & 0x40 != 0;

    // Hat switch - need to confirm encoding from d-pad presses
    // For now, parse from byte 2 or a separate mechanism
    // The resting state showed 0x81 which is buttons, not hat
    // D-pad might be in the buttons byte or a separate hat field

    // Triggers (bytes 3-6, 16-bit LE, 10-bit range)
    if data.len() >= 7 {
        state.left_trigger = u16::from_le_bytes([data[3], data[4]]) & 0x03FF;
        state.right_trigger = u16::from_le_bytes([data[5], data[6]]) & 0x03FF;
    }

    // Sticks (bytes 7-14, int16 LE)
    if data.len() >= 15 {
        state.left_stick_x = i16::from_le_bytes([data[7], data[8]]);
        state.left_stick_y = i16::from_le_bytes([data[9], data[10]]);
        state.right_stick_x = i16::from_le_bytes([data[11], data[12]]);
        state.right_stick_y = i16::from_le_bytes([data[13], data[14]]);
    }

    // Byte 17: profile number (0-3) — confirmed from hardware
    if data.len() > 17 {
        state.hw_profile = data[17] & 0x03;
    }

    // Byte 19: paddles - THIS IS THE KEY PART
    // Works in ALL profiles because we read raw HID, not evdev
    if data.len() > 19 {
        let paddles = data[19];
        state.paddle_ur = paddles & 0x01 != 0; // P1
        state.paddle_lr = paddles & 0x02 != 0; // P2
        state.paddle_ul = paddles & 0x04 != 0; // P3
        state.paddle_ll = paddles & 0x08 != 0; // P4
    }

    state
}

fn find_elite2_bt_hidraw() -> String {
    let entries = std::fs::read_dir("/dev").expect("read /dev");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("hidraw") {
            continue;
        }

        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut info = HidrawDevinfo::default();
        if unsafe { hidiocgrawinfo(file.as_raw_fd(), &mut info).is_err() } {
            continue;
        }

        // Microsoft vendor, Bluetooth bus
        if info.vendor as u16 == VENDOR_MICROSOFT && info.bustype == BUS_BLUETOOTH {
            let mut name_buf = [0u8; 256];
            let dev_name = match unsafe { hidiocgrawname(file.as_raw_fd(), &mut name_buf) } {
                Ok(len) => String::from_utf8_lossy(&name_buf[..(len as usize).min(255)])
                    .trim_end_matches('\0')
                    .to_string(),
                Err(_) => String::from("Unknown"),
            };
            println!(
                "Found BT controller: {} (PID:{:04x}) at {}",
                dev_name,
                info.product as u16,
                path.display()
            );
            return path.display().to_string();
        }
    }

    eprintln!("No Xbox Elite 2 found on Bluetooth.");
    eprintln!("Make sure the controller is connected via BT (not USB).");
    std::process::exit(1);
}

/// Try to grab the corresponding evdev device to prevent hid-generic from
/// emitting duplicate (potentially incomplete) events.
fn grab_evdev_for_hidraw(hidraw_path: &str) -> Option<std::fs::File> {
    let hidraw_name = std::path::Path::new(hidraw_path)
        .file_name()?
        .to_str()?;

    let sysfs_input = format!("/sys/class/hidraw/{hidraw_name}/device/input");
    let input_dir = std::fs::read_dir(&sysfs_input).ok()?;

    for entry in input_dir.flatten() {
        let input_name = entry.file_name();
        if !input_name.to_str()?.starts_with("input") {
            continue;
        }
        for sub in std::fs::read_dir(entry.path()).ok()?.flatten() {
            let sub_name = sub.file_name();
            let sub_str = sub_name.to_str()?;
            if sub_str.starts_with("event") {
                let evdev_path = format!("/dev/input/{sub_str}");
                let file = OpenOptions::new().read(true).open(&evdev_path).ok()?;
                nix::ioctl_write_int!(eviocgrab, b'E', 0x90);
                match unsafe { eviocgrab(file.as_raw_fd(), 1) } {
                    Ok(_) => {
                        println!("Grabbed evdev: {evdev_path}");
                        return Some(file);
                    }
                    Err(e) => {
                        eprintln!("Could not grab {evdev_path}: {e}");
                    }
                }
            }
        }
    }
    None
}

fn format_event(ev: &transform::OutputEvent) -> String {
    if ev.ev_type == EV_KEY {
        match ev.code {
            BTN_A => "BTN_A".into(),
            BTN_B => "BTN_B".into(),
            BTN_X => "BTN_X".into(),
            BTN_Y => "BTN_Y".into(),
            BTN_TL => "BTN_TL".into(),
            BTN_TR => "BTN_TR".into(),
            BTN_SELECT => "BTN_SELECT".into(),
            BTN_START => "BTN_START".into(),
            BTN_MODE => "BTN_MODE".into(),
            BTN_THUMBL => "BTN_THUMBL".into(),
            BTN_THUMBR => "BTN_THUMBR".into(),
            BTN_GRIPL => "BTN_GRIPL".into(),
            BTN_GRIPR => "BTN_GRIPR".into(),
            BTN_GRIPL2 => "BTN_GRIPL2".into(),
            BTN_GRIPR2 => "BTN_GRIPR2".into(),
            _ => format!("KEY_{:#x}", ev.code),
        }
    } else if ev.ev_type == EV_ABS {
        match ev.code {
            ABS_X => "ABS_X".into(),
            ABS_Y => "ABS_Y".into(),
            ABS_Z => "ABS_Z".into(),
            ABS_RX => "ABS_RX".into(),
            ABS_RY => "ABS_RY".into(),
            ABS_RZ => "ABS_RZ".into(),
            ABS_HAT0X => "ABS_HAT0X".into(),
            ABS_HAT0Y => "ABS_HAT0Y".into(),
            _ => format!("ABS_{:#x}", ev.code),
        }
    } else {
        format!("{:#x}:{:#x}", ev.ev_type, ev.code)
    }
}
