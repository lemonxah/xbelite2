// Probe all unknown GIP commands on the Xbox Elite 2.
// Systematically queries extended sub-commands and 0x4D variants
// to discover profile/LED/config commands.

use std::time::Duration;

const VID: u16 = 0x045E;
const PID: u16 = 0x0B00;
const EP_OUT: u8 = 0x02;
const EP_IN: u8 = 0x82;
const TIMEOUT: Duration = Duration::from_millis(500);

fn main() {
    println!("=== Xbox Elite 2 GIP Command Probe ===\n");

    let mut handle = rusb::open_device_with_vid_pid(VID, PID)
        .expect("Elite 2 not found on USB");

    if handle.kernel_driver_active(0).unwrap_or(false) {
        handle.detach_kernel_driver(0).expect("Failed to detach kernel driver");
    }
    handle.claim_interface(0).expect("Failed to claim interface");

    // Drain initial messages
    drain(&handle);

    // 1. Probe all supported extended sub-commands (0x1E)
    let supported_subs: Vec<u8> = vec![
        0x00, 0x01, 0x02, 0x04, 0x05, 0x06,
        0x08, 0x09, 0x0A, 0x0C, 0x0D, 0x0F, 0x10,
    ];

    println!("--- Probing Extended Sub-Commands (0x1E) ---\n");
    for (i, sub) in supported_subs.iter().enumerate() {
        let seq = (i + 10) as u8;
        let req = [0x1E, 0x30, seq, 0x01, *sub];
        print!("  Sub 0x{sub:02x}: ");
        if handle.write_interrupt(EP_OUT, &req, TIMEOUT).is_err() {
            println!("WRITE FAILED");
            continue;
        }

        // Read responses (skip ACKs and input reports)
        let mut got_response = false;
        for _ in 0..10 {
            let mut buf = [0u8; 64];
            match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
                Ok(n) => {
                    if buf[0] == 0x20 { continue; } // skip gamepad input
                    if buf[0] == 0x02 { continue; } // skip hello
                    if buf[0] == 0x01 { continue; } // skip ACK
                    println!("RESPONSE len={n}");
                    print_hex(&buf[..n]);
                    got_response = true;
                    break;
                }
                Err(rusb::Error::Timeout) => break,
                Err(e) => { println!("ERR: {e}"); break; }
            }
        }
        if !got_response {
            println!("(no data response, ACK only)");
        }
        drain(&handle);
    }

    // 2. Probe 0x4D with different sub-command bytes
    println!("\n--- Probing Command 0x4D (Vendor) ---\n");
    // From xpad.c, the elite2 init sends: 0x4d 0x10 0x01 0x02 0x07 0x00
    // Try different first payload bytes to discover sub-commands
    let payloads: Vec<Vec<u8>> = vec![
        vec![0x4D, 0x10, 0x20, 0x01, 0x00],           // sub 0x00
        vec![0x4D, 0x10, 0x21, 0x01, 0x01],           // sub 0x01
        vec![0x4D, 0x10, 0x22, 0x01, 0x02],           // sub 0x02
        vec![0x4D, 0x10, 0x23, 0x02, 0x07, 0x00],     // the known init cmd
        vec![0x4D, 0x10, 0x24, 0x02, 0x07, 0x01],     // variant
        vec![0x4D, 0x10, 0x25, 0x02, 0x07, 0x02],     // variant
        vec![0x4D, 0x10, 0x26, 0x01, 0x03],           // sub 0x03
        vec![0x4D, 0x10, 0x27, 0x01, 0x04],           // sub 0x04
        vec![0x4D, 0x10, 0x28, 0x01, 0x05],           // sub 0x05
        vec![0x4D, 0x10, 0x29, 0x01, 0x06],           // sub 0x06
        vec![0x4D, 0x10, 0x2A, 0x01, 0x08],           // sub 0x08
        vec![0x4D, 0x10, 0x2B, 0x01, 0x09],           // sub 0x09
        vec![0x4D, 0x10, 0x2C, 0x01, 0x0A],           // sub 0x0A
        vec![0x4D, 0x00, 0x2D, 0x00],                 // no flags, empty
        vec![0x4D, 0x20, 0x2E, 0x00],                 // system flag, empty
    ];

    for payload in &payloads {
        print!("  0x4D payload {:02x?}: ", &payload[2..]);
        if handle.write_interrupt(EP_OUT, payload, TIMEOUT).is_err() {
            println!("WRITE FAILED");
            continue;
        }

        let mut got_data = false;
        for _ in 0..10 {
            let mut buf = [0u8; 64];
            match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
                Ok(n) => {
                    if buf[0] == 0x20 || buf[0] == 0x02 { continue; }
                    if buf[0] == 0x01 {
                        // ACK — check if it's for our 0x4D
                        if n >= 6 && buf[5] == 0x4D {
                            continue; // just an ACK, keep reading
                        }
                        continue;
                    }
                    if buf[0] == 0x4D {
                        println!("GOT 0x4D RESPONSE! len={n}");
                        print_hex(&buf[..n]);
                        got_data = true;
                        break;
                    }
                    println!("cmd=0x{:02x} len={n}", buf[0]);
                    print_hex(&buf[..n]);
                    got_data = true;
                    break;
                }
                Err(rusb::Error::Timeout) => break,
                Err(_) => break,
            }
        }
        if !got_data {
            println!("(ACK only, no data)");
        }
        drain(&handle);
    }

    // 3. Try reading with 0x4D as a "get" — send request, see if controller
    // responds with profile data
    println!("\n--- Probing 0x4D Read Requests ---\n");
    // The format might be: [0x4D, flags, seq, len, sub_cmd, profile_num]
    for profile in 0..4u8 {
        let req = vec![0x4D, 0x10, 0x30 + profile, 0x02, 0x01, profile];
        print!("  Read profile {profile}: ");
        if handle.write_interrupt(EP_OUT, &req, TIMEOUT).is_err() {
            println!("WRITE FAILED");
            continue;
        }
        let mut got = false;
        for _ in 0..10 {
            let mut buf = [0u8; 64];
            match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
                Ok(n) => {
                    if buf[0] == 0x20 || buf[0] == 0x02 || buf[0] == 0x01 { continue; }
                    println!("RESPONSE cmd=0x{:02x} len={n}", buf[0]);
                    print_hex(&buf[..n]);
                    got = true;
                    break;
                }
                Err(_) => break,
            }
        }
        if !got { println!("(no data)"); }
        drain(&handle);
    }

    let _ = handle.release_interface(0);
    println!("\nDone.");
}

fn drain(handle: &rusb::DeviceHandle<rusb::GlobalContext>) {
    loop {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(50)) {
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

fn print_hex(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("    {:04x}: ", i * 16);
        for b in chunk { print!("{b:02x} "); }
        print!("  ");
        for b in chunk {
            if b.is_ascii_graphic() || *b == b' ' { print!("{}", *b as char); }
            else { print!("."); }
        }
        println!();
    }
}
