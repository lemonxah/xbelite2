//! End-to-end test: run the daemon for a few seconds, verify virtual gamepad works.
//! Run with: sudo cargo run --bin xbe2-test
//!
//! This will:
//! 1. Find the Elite 2 controller
//! 2. Grab it exclusively
//! 3. Create a virtual gamepad
//! 4. Forward events with a simple identity profile (no remapping)
//! 5. Print both raw input and forwarded output for verification

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use xbelite2::evdev;
use xbelite2::transform;
use xbelite2::types::*;
use xbelite2::uinput::VirtualGamepad;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Xbox Elite 2 - End-to-End Test ===\n");

    // Discover
    let devices = evdev::discover_devices().expect("Failed to scan devices");
    if devices.is_empty() {
        eprintln!("No Elite 2 found. Is it plugged in?");
        std::process::exit(1);
    }

    let device = devices.into_iter().next().unwrap();
    println!("Found: {} at {}\n", device.name, device.path.display());

    // Open and grab
    let mut reader = evdev::EvdevReader::open(device).expect("Failed to open device");
    reader.grab().expect("Failed to grab device");
    println!("Grabbed device (other apps won't see raw events)\n");

    // Create virtual gamepad
    let mut gamepad = VirtualGamepad::new(0).expect("Failed to create virtual gamepad");
    println!("Created virtual gamepad\n");

    // Identity profile (passthrough)
    let profile = Profile::default();
    let mut prev_state = GamepadState::default();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed)).unwrap();

    println!("Forwarding events (Ctrl+C to stop)...");
    println!("Try pressing paddles, buttons, moving sticks.\n");
    println!("{:<12} {:<12} {:<15} {}", "DIRECTION", "TYPE", "CODE", "VALUE");
    println!("{}", "-".repeat(55));

    while running.load(Ordering::Relaxed) {
        let ev = match reader.read_event() {
            Ok(ev) => ev,
            Err(_) => break,
        };

        // Skip SYN events for display
        if ev.ev_type == 0x00 {
            // But we still need to handle SYN_REPORT to flush
            if ev.code == 0x00 {
                // Process accumulated state and emit
                // (For simplicity in this test, we emit per-event instead of batching)
            }
            continue;
        }

        // Print raw input
        let type_name = match ev.ev_type {
            0x01 => "EV_KEY",
            0x03 => "EV_ABS",
            _ => "OTHER",
        };
        let code_name = format_code(ev.ev_type, ev.code);
        println!(
            "{:<12} {:<12} {:<15} {}",
            "RAW IN", type_name, code_name, ev.value
        );

        // Update state
        apply_event(&mut prev_state, &ev);

        // For this test, just forward the event directly (identity transform)
        let output = transform::OutputEvent {
            ev_type: ev.ev_type,
            code: ev.code,
            value: ev.value,
        };
        if let Err(e) = gamepad.emit(&[output]) {
            eprintln!("Emit error: {e}");
        } else {
            println!(
                "{:<12} {:<12} {:<15} {}",
                "-> OUT", type_name, code_name, ev.value
            );
        }
    }

    println!("\nTest complete. Virtual gamepad destroyed.");
}

fn apply_event(state: &mut GamepadState, ev: &evdev::InputEvent) {
    match ev.ev_type {
        0x01 => {
            let pressed = ev.value != 0;
            match ev.code {
                BTN_A => state.btn_a = pressed,
                BTN_B => state.btn_b = pressed,
                BTN_X => state.btn_x = pressed,
                BTN_Y => state.btn_y = pressed,
                BTN_TL => state.btn_lb = pressed,
                BTN_TR => state.btn_rb = pressed,
                BTN_SELECT => state.btn_view = pressed,
                BTN_START => state.btn_menu = pressed,
                BTN_MODE => state.btn_xbox = pressed,
                BTN_THUMBL => state.btn_lstick = pressed,
                BTN_THUMBR => state.btn_rstick = pressed,
                BTN_GRIPL => state.paddle_ul = pressed,
                BTN_GRIPR => state.paddle_ur = pressed,
                BTN_GRIPL2 => state.paddle_ll = pressed,
                BTN_GRIPR2 => state.paddle_lr = pressed,
                _ => {}
            }
        }
        0x03 => match ev.code {
            ABS_X => state.left_stick_x = ev.value as i16,
            ABS_Y => state.left_stick_y = ev.value as i16,
            ABS_RX => state.right_stick_x = ev.value as i16,
            ABS_RY => state.right_stick_y = ev.value as i16,
            ABS_Z => state.left_trigger = ev.value as u16,
            ABS_RZ => state.right_trigger = ev.value as u16,
            _ => {}
        },
        _ => {}
    }
}

fn format_code(ev_type: u16, code: u16) -> String {
    if ev_type == 0x01 {
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
            _ => format!("KEY_{:#x}", code),
        }
    } else if ev_type == 0x03 {
        match code {
            0x00 => "ABS_X".into(),
            0x01 => "ABS_Y".into(),
            0x02 => "ABS_Z".into(),
            0x03 => "ABS_RX".into(),
            0x04 => "ABS_RY".into(),
            0x05 => "ABS_RZ".into(),
            0x10 => "ABS_HAT0X".into(),
            0x11 => "ABS_HAT0Y".into(),
            _ => format!("ABS_{:#x}", code),
        }
    } else {
        format!("{:#x}", code)
    }
}
