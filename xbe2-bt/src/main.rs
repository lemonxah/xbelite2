use xbelite2_gip::transport::GipDevice;
use xbelite2_gip::rumble;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let mut dev = GipDevice::open_bt().unwrap_or_else(|e| {
        eprintln!("Failed to open /dev/xbelite2_bt: {e}");
        eprintln!("Is the controller connected via Bluetooth and xbelite2 module loaded?");
        std::process::exit(1);
    });

    match mode {
        "rumble" => {
            let lm: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let rm: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let lt: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let rt: u8 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
            if args.len() < 3 {
                eprintln!("Usage: xbe2-bt rumble <left> <right> [ltrigger] [rtrigger]");
                std::process::exit(1);
            }
            rumble::set_bt(&mut dev, lm, rm, lt, rt);
            println!("BT Rumble: LM={lm} RM={rm} LT={lt} RT={rt}");
        }
        "rumble-stop" => {
            rumble::stop_bt(&mut dev);
            println!("BT Rumble stopped");
        }
        "dump" => {
            println!("Reading BT HID reports from /dev/xbelite2_bt...");
            println!("Press Ctrl+C to stop.\n");
            loop {
                if let Some(frame) = dev.recv(Duration::from_millis(100)) {
                    if frame.len() >= 2 {
                        let len = u16::from_le_bytes([frame[0], frame[1]]) as usize;
                        let payload = &frame[2..];
                        if payload.len() >= len && len >= 20 {
                            let report = &payload[..len];
                            print_bt_report(report);
                        }
                    }
                    // Raw frame from ring buffer (length-prefixed)
                    if frame.len() >= 20 && frame[0] == 0x01 {
                        print_bt_report(&frame);
                    }
                }
            }
        }
        "read" => {
            println!("Reading single BT HID report...");
            // Read a few frames to get a clean report
            for _ in 0..50 {
                if let Some(frame) = dev.recv(Duration::from_millis(200)) {
                    if frame.len() >= 20 {
                        print_bt_report(&frame);
                        return;
                    }
                }
            }
            println!("No report received");
        }
        _ => {
            println!("Xbox Elite 2 Bluetooth Test Tool");
            println!();
            println!("Usage:");
            println!("  xbe2-bt rumble <LM> <RM> [LT] [RT]   Test rumble motors (0-100)");
            println!("  xbe2-bt rumble-stop                   Stop rumble");
            println!("  xbe2-bt dump                          Dump raw BT HID reports");
            println!("  xbe2-bt read                          Read single BT report");
        }
    }
}

fn print_bt_report(data: &[u8]) {
    if data.len() < 20 {
        println!("short report: {} bytes {:02x?}", data.len(), data);
        return;
    }

    // BT HID Report ID 0x01, 20 bytes
    let btns0 = data[1];
    let btns1 = data[2];

    let a = btns0 & 0x01 != 0;
    let b = btns0 & 0x02 != 0;
    let x = btns0 & 0x08 != 0;
    let y = btns0 & 0x10 != 0;
    let lb = btns0 & 0x40 != 0;
    let rb = btns0 & 0x80 != 0;

    let hat = btns1 & 0x0F;
    let view = btns1 & 0x04 != 0;
    let menu = btns1 & 0x08 != 0;
    let lstick = btns1 & 0x20 != 0;
    let rstick = btns1 & 0x40 != 0;

    let lt = u16::from_le_bytes([data[3], data[4]]) & 0x03FF;
    let rt = u16::from_le_bytes([data[5], data[6]]) & 0x03FF;
    let lx = i16::from_le_bytes([data[7], data[8]]);
    let ly = i16::from_le_bytes([data[9], data[10]]);
    let rx = i16::from_le_bytes([data[11], data[12]]);
    let ry = i16::from_le_bytes([data[13], data[14]]);

    let profile = data[17] & 0x03;
    let paddles = data[19];
    let p_ur = paddles & 0x01 != 0;
    let p_lr = paddles & 0x02 != 0;
    let p_ul = paddles & 0x04 != 0;
    let p_ll = paddles & 0x08 != 0;

    let mut btns = String::new();
    if a { btns.push_str("A "); }
    if b { btns.push_str("B "); }
    if x { btns.push_str("X "); }
    if y { btns.push_str("Y "); }
    if lb { btns.push_str("LB "); }
    if rb { btns.push_str("RB "); }
    if view { btns.push_str("View "); }
    if menu { btns.push_str("Menu "); }
    if lstick { btns.push_str("LS "); }
    if rstick { btns.push_str("RS "); }
    if p_ur { btns.push_str("P1 "); }
    if p_lr { btns.push_str("P2 "); }
    if p_ul { btns.push_str("P3 "); }
    if p_ll { btns.push_str("P4 "); }

    let hat_str = match hat {
        1 => "Up", 2 => "UpRight", 3 => "Right", 4 => "DownRight",
        5 => "Down", 6 => "DownLeft", 7 => "Left", 8 => "UpLeft",
        _ => "",
    };

    print!("P{profile} LX={lx:6} LY={ly:6} RX={rx:6} RY={ry:6} LT={lt:4} RT={rt:4}");
    if !btns.is_empty() {
        print!(" [{btns}]");
    }
    if !hat_str.is_empty() {
        print!(" D={hat_str}");
    }
    println!();
}
