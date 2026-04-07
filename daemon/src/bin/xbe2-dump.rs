//! Diagnostic tool: discover the Elite 2 and dump its capabilities + live events.
//! Run with: cargo run --bin xbe2-dump
//! Needs read access to /dev/input/event* (run with sudo or add user to input group)

// We need the library modules
use xbelite2::evdev;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Xbox Elite Series 2 Diagnostic Tool ===\n");

    println!("Scanning for Elite 2 controllers...\n");
    let devices = match evdev::discover_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error scanning devices: {e}");
            eprintln!("Try running with sudo or add yourself to the 'input' group.");
            std::process::exit(1);
        }
    };

    if devices.is_empty() {
        eprintln!("No Xbox Elite Series 2 controllers found.");
        eprintln!("Check that the controller is connected (USB or Bluetooth).");
        std::process::exit(1);
    }

    for dev in &devices {
        println!("Found: {} at {}", dev.name, dev.path.display());
        println!(
            "  VID:PID = {:04x}:{:04x}",
            dev.id.vendor, dev.id.product
        );
    }

    // Open the first one
    let device = devices.into_iter().next().unwrap();
    let mut reader = match evdev::EvdevReader::open(device) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open device: {e}");
            std::process::exit(1);
        }
    };

    println!("\n=== Device Capabilities ===\n");
    if let Err(e) = reader.dump_capabilities() {
        eprintln!("Failed to dump capabilities: {e}");
    }

    println!("\n=== Live Events (press Ctrl+C to stop) ===");
    println!("Press buttons, move sticks, pull triggers, press paddles...\n");
    println!("{:<20} {:<10} {:<10} {}", "TIME", "TYPE", "CODE", "VALUE");
    println!("{}", "-".repeat(60));

    loop {
        match reader.read_event() {
            Ok(ev) => {
                // Skip SYN events for cleaner output
                if ev.ev_type == 0x00 {
                    continue;
                }

                let type_name = match ev.ev_type {
                    0x01 => "EV_KEY",
                    0x03 => "EV_ABS",
                    0x15 => "EV_FF",
                    _ => "OTHER",
                };

                let code_name = format_code(ev.ev_type, ev.code);
                let value_str = if ev.ev_type == 0x01 {
                    match ev.value {
                        0 => "RELEASED".to_string(),
                        1 => "PRESSED".to_string(),
                        2 => "REPEAT".to_string(),
                        _ => format!("{}", ev.value),
                    }
                } else {
                    format!("{}", ev.value)
                };

                println!(
                    "{:<20} {:<10} {:<10} {}",
                    format!("{}.{:06}", ev.tv_sec, ev.tv_usec),
                    type_name,
                    code_name,
                    value_str
                );
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }
    }
}

fn format_code(ev_type: u16, code: u16) -> String {
    if ev_type == 0x01 {
        // Key/button codes
        match code {
            0x130 => "BTN_A".into(),
            0x131 => "BTN_B".into(),
            0x133 => "BTN_X".into(),
            0x134 => "BTN_Y".into(),
            0x136 => "BTN_TL".into(),
            0x137 => "BTN_TR".into(),
            0x13A => "BTN_SELECT".into(),
            0x13B => "BTN_START".into(),
            0x13C => "BTN_MODE".into(),
            0x13D => "BTN_THUMBL".into(),
            0x13E => "BTN_THUMBR".into(),
            0x224 => "BTN_GRIPL".into(),
            0x225 => "BTN_GRIPR".into(),
            0x226 => "BTN_GRIPL2".into(),
            0x227 => "BTN_GRIPR2".into(),
            0x2C0 => "BTN_TH5".into(),
            0x2C1 => "BTN_TH6".into(),
            0x2C2 => "BTN_TH7".into(),
            0x2C3 => "BTN_TH8".into(),
            0x2C4 => "BTN_TH9".into(),
            0x2C5 => "BTN_TH10".into(),
            0x2C6 => "BTN_TH11".into(),
            0x2C7 => "BTN_TH12".into(),
            _ => format!("KEY_{:#06x}", code),
        }
    } else if ev_type == 0x03 {
        // Axis codes
        match code {
            0x00 => "ABS_X".into(),
            0x01 => "ABS_Y".into(),
            0x02 => "ABS_Z".into(),
            0x03 => "ABS_RX".into(),
            0x04 => "ABS_RY".into(),
            0x05 => "ABS_RZ".into(),
            0x10 => "ABS_HAT0X".into(),
            0x11 => "ABS_HAT0Y".into(),
            0x28 => "ABS_PROFILE".into(),
            _ => format!("ABS_{:#04x}", code),
        }
    } else {
        format!("{:#06x}", code)
    }
}
