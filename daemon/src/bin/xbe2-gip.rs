// GIP protocol tool for Xbox Elite Series 2.
// Reads device metadata (command 0x04) to discover vendor-specific commands,
// capabilities, and supported interfaces.
//
// Must be run with the controller connected via USB and xpad unbound:
//   sudo ./target/debug/xbe2-gip

use std::time::Duration;

const VID: u16 = 0x045E;
const PID: u16 = 0x0B00;

// GIP command IDs
const GIP_ACK: u8 = 0x01;
const GIP_HELLO: u8 = 0x02;
const GIP_STATUS: u8 = 0x03;
const GIP_METADATA: u8 = 0x04;
const GIP_SET_STATE: u8 = 0x05;
const GIP_GUIDE: u8 = 0x07;
const GIP_RUMBLE: u8 = 0x09;
const GIP_LED: u8 = 0x0A;
const GIP_EXTENDED: u8 = 0x1E;
const GIP_INPUT: u8 = 0x20;

// GIP flags
const GIP_FLAG_SYSTEM: u8 = 0x20;
const GIP_FLAG_NEED_ACK: u8 = 0x10;
const GIP_FLAG_CHUNK_START: u8 = 0x40;
const GIP_FLAG_CHUNKED: u8 = 0x80;

fn main() {
    println!("=== Xbox Elite 2 GIP Protocol Tool ===\n");

    let device = rusb::open_device_with_vid_pid(VID, PID);
    let mut handle = match device {
        Some(h) => h,
        None => {
            eprintln!("Elite 2 not found on USB (VID:{VID:04x} PID:{PID:04x}).");
            eprintln!("Make sure it's plugged in via USB cable.");
            std::process::exit(1);
        }
    };

    println!("Found Elite 2 on USB");

    // Detach kernel driver if attached
    if handle.kernel_driver_active(0).unwrap_or(false) {
        println!("Detaching kernel driver from interface 0...");
        handle.detach_kernel_driver(0).expect("Failed to detach kernel driver");
    }

    handle.claim_interface(0).expect("Failed to claim interface 0");
    println!("Claimed interface 0\n");

    // First, read any pending data (the controller sends HELLO messages)
    println!("--- Reading initial messages from controller ---\n");
    drain_messages(&handle, Duration::from_millis(500));

    // Send metadata request (command 0x04)
    // GIP header: [type, flags, sequence, length]
    // Metadata request has zero-length payload
    println!("--- Sending Metadata Request (0x04) ---\n");
    let metadata_req = [GIP_METADATA, GIP_FLAG_SYSTEM | GIP_FLAG_NEED_ACK, 0x01, 0x00];
    match handle.write_interrupt(0x02, &metadata_req, Duration::from_millis(1000)) {
        Ok(n) => println!("Sent {n} bytes: {:02x?}", metadata_req),
        Err(e) => {
            eprintln!("Failed to send metadata request: {e}");
            eprintln!("Trying alternative format...");
            // Some controllers need the request in a slightly different format
            let alt_req = [GIP_METADATA, GIP_FLAG_SYSTEM, 0x01, 0x00];
            handle.write_interrupt(0x02, &alt_req, Duration::from_millis(1000))
                .expect("Alternative format also failed");
        }
    }

    // Read the response — metadata is typically fragmented (chunked)
    println!("\n--- Reading Metadata Response ---\n");
    let mut metadata_buf: Vec<u8> = Vec::new();
    let mut reading = true;
    let mut msg_count = 0;

    while reading && msg_count < 100 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(0x82, &mut buf, Duration::from_millis(2000)) {
            Ok(n) => {
                msg_count += 1;
                let data = &buf[..n];
                let _cmd = data[0] & 0x1F;
                let _flags = if n > 1 { data[1] } else { 0 };

                print!("  [{msg_count:3}] cmd=0x{:02x} flags=0x{:02x} len={n}: ", data[0], _flags);

                match data[0] {
                    GIP_ACK => {
                        println!("ACK");
                    }
                    GIP_HELLO => {
                        println!("HELLO (device announcement)");
                        print_hex(data);
                    }
                    GIP_STATUS => {
                        println!("STATUS (battery)");
                        if n >= 8 {
                            println!("    battery_level={} battery_type={}", data[4], data[5]);
                        }
                        print_hex(data);
                    }
                    GIP_METADATA => {
                        println!("METADATA RESPONSE");

                        // Check if chunked
                        if n > 1 && (data[1] & GIP_FLAG_CHUNKED) != 0 {
                            // Chunked response — extract payload after header
                            let header_len = gip_header_len(data);
                            if header_len < n {
                                metadata_buf.extend_from_slice(&data[header_len..n]);
                            }
                            println!("    (chunked, accumulated {} bytes so far)", metadata_buf.len());

                            // Send ACK for chunked transfer
                            let ack = [GIP_ACK, GIP_FLAG_SYSTEM, data[2], 0x01, data[0]];
                            let _ = handle.write_interrupt(0x02, &ack, Duration::from_millis(100));
                        } else {
                            // Single response
                            let header_len = gip_header_len(data);
                            if header_len < n {
                                metadata_buf.extend_from_slice(&data[header_len..n]);
                            }
                            reading = false;
                        }
                        print_hex(data);
                    }
                    GIP_GUIDE => {
                        println!("GUIDE BUTTON");
                    }
                    GIP_INPUT => {
                        println!("INPUT (gamepad)");
                        // Don't print these, too noisy
                    }
                    _ => {
                        println!("UNKNOWN cmd=0x{:02x}", data[0]);
                        print_hex(data);
                    }
                }
            }
            Err(rusb::Error::Timeout) => {
                println!("  (timeout — no more messages)");
                reading = false;
            }
            Err(e) => {
                eprintln!("  Read error: {e}");
                reading = false;
            }
        }
    }

    // Try to parse metadata as JSON
    println!("\n--- Metadata ({} bytes) ---\n", metadata_buf.len());
    if !metadata_buf.is_empty() {
        // Try as UTF-8 JSON
        if let Ok(text) = std::str::from_utf8(&metadata_buf) {
            println!("JSON metadata:\n{text}");
            // Pretty-print if valid JSON
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                println!("\nParsed:\n{}", serde_json::to_string_pretty(&parsed).unwrap());
            }
        } else {
            // Binary metadata (older format)
            println!("Binary metadata (not JSON):");
            print_hex(&metadata_buf);
            parse_binary_metadata(&metadata_buf);
        }
    } else {
        println!("No metadata received. Trying to read raw responses...\n");
        drain_messages(&handle, Duration::from_millis(3000));
    }

    // Also try extended commands (0x1E) to get serial number and capabilities
    println!("\n--- Extended: Get Capabilities (0x1E sub 0x00) ---\n");
    let caps_req = [GIP_EXTENDED, GIP_FLAG_SYSTEM | GIP_FLAG_NEED_ACK, 0x02, 0x01, 0x00];
    if handle.write_interrupt(0x02, &caps_req, Duration::from_millis(1000)).is_ok() {
        read_and_print(&handle, 5);
    }

    println!("\n--- Extended: Get Serial Number (0x1E sub 0x04) ---\n");
    let serial_req = [GIP_EXTENDED, GIP_FLAG_SYSTEM | GIP_FLAG_NEED_ACK, 0x03, 0x01, 0x04];
    if handle.write_interrupt(0x02, &serial_req, Duration::from_millis(1000)).is_ok() {
        read_and_print(&handle, 5);
    }

    // Try command 0x4D (Elite 2 vendor command) with empty payload
    println!("\n--- Vendor Command 0x4D (Elite 2 specific) ---\n");
    let vendor_req = [0x4D, 0x10, 0x04, 0x00];
    match handle.write_interrupt(0x02, &vendor_req, Duration::from_millis(1000)) {
        Ok(_) => {
            println!("Sent 0x4D probe");
            read_and_print(&handle, 10);
        }
        Err(e) => println!("Failed to send 0x4D: {e}"),
    }

    // Release
    let _ = handle.release_interface(0);
    println!("\nDone.");
}

fn gip_header_len(data: &[u8]) -> usize {
    if data.len() < 4 { return data.len(); }
    // GIP header: [type, flags, sequence, length...]
    // Length is LEB128 encoded starting at byte 3
    // For simple cases, byte 3 is the length and header is 4 bytes
    // For chunked start, there's extra chunk header bytes
    if data[1] & GIP_FLAG_CHUNK_START != 0 {
        // Chunk start has: [type, flags, seq, total_len(2 bytes), chunk_offset(2 bytes)]
        // Actually the format varies. Let's just find where the payload starts
        // by looking for the JSON/binary start
        for i in 4..data.len().min(16) {
            if data[i] == b'{' || data[i] == 0x00 { return i; }
        }
        8 // reasonable default for chunked header
    } else {
        4
    }
}

fn drain_messages(handle: &rusb::DeviceHandle<rusb::GlobalContext>, duration: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < duration {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(0x82, &mut buf, Duration::from_millis(200)) {
            Ok(n) => {
                let name = match buf[0] {
                    GIP_ACK => "ACK",
                    GIP_HELLO => "HELLO",
                    GIP_STATUS => "STATUS",
                    GIP_METADATA => "METADATA",
                    GIP_GUIDE => "GUIDE",
                    GIP_INPUT => "INPUT",
                    0x4D => "VENDOR_4D",
                    _ => "UNKNOWN",
                };
                if buf[0] != GIP_INPUT { // skip noisy input reports
                    println!("  cmd=0x{:02x} ({name}) len={n}", buf[0]);
                    print_hex(&buf[..n]);
                }
            }
            Err(rusb::Error::Timeout) => break,
            Err(_) => break,
        }
    }
}

fn read_and_print(handle: &rusb::DeviceHandle<rusb::GlobalContext>, max: usize) {
    for _ in 0..max {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(0x82, &mut buf, Duration::from_millis(1000)) {
            Ok(n) => {
                if buf[0] == GIP_INPUT { continue; } // skip gamepad input
                println!("  cmd=0x{:02x} len={n}", buf[0]);
                print_hex(&buf[..n]);
            }
            Err(rusb::Error::Timeout) => { println!("  (timeout)"); break; }
            Err(e) => { println!("  Error: {e}"); break; }
        }
    }
}

fn print_hex(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("    {:04x}: ", i * 16);
        for b in chunk {
            print!("{b:02x} ");
        }
        // ASCII
        print!("  ");
        for b in chunk {
            if b.is_ascii_graphic() || *b == b' ' {
                print!("{}", *b as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
}

fn parse_binary_metadata(data: &[u8]) {
    println!("\n  Attempting binary metadata parse:");
    if data.len() < 16 {
        println!("  Too short for binary metadata");
        return;
    }

    // The older binary descriptor format has offset/count pairs
    // pointing to sections within the data. Try to identify structure.
    println!("  First 32 bytes:");
    for (i, b) in data.iter().take(32).enumerate() {
        if i % 4 == 0 && i > 0 { print!(" "); }
        print!("{b:02x}");
    }
    println!();

    // Look for GUIDs (16 bytes) — they often appear in the descriptor
    for i in 0..data.len().saturating_sub(16) {
        let slice = &data[i..i+16];
        // Check if it looks like a GUID (has some variation, not all zeros)
        let nonzero = slice.iter().filter(|b| **b != 0).count();
        if nonzero >= 8 && nonzero <= 16 {
            let guid = format!(
                "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]),
                u16::from_le_bytes([slice[4], slice[5]]),
                u16::from_le_bytes([slice[6], slice[7]]),
                slice[8], slice[9], slice[10], slice[11],
                slice[12], slice[13], slice[14], slice[15]
            );
            // Known GUIDs
            let known = match guid.as_str() {
                "31c1034d-b5b7-4551-9813-8769d4a0e4f9" => " (IProgrammableGamepad)",
                _ => "",
            };
            if !known.is_empty() || (nonzero >= 12) {
                println!("  Possible GUID at offset {i}: {guid}{known}");
            }
        }
    }
}
