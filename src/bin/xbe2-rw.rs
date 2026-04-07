// GIP read/write tool for Xbox Elite Series 2.
// Phase 1: Deep reads of all sub-commands with parameter variations.
// Phase 2: Write device name (safe, reversible).
// Phase 3: Probe profile slots.

use std::time::Duration;

const VID: u16 = 0x045E;
const PID: u16 = 0x0B00;
const EP_OUT: u8 = 0x02;
const EP_IN: u8 = 0x82;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("read");

    println!("=== Xbox Elite 2 GIP Read/Write Tool ===");
    println!("Mode: {mode}\n");

    let handle = rusb::open_device_with_vid_pid(VID, PID)
        .expect("Elite 2 not found on USB");

    if handle.kernel_driver_active(0).unwrap_or(false) {
        handle.detach_kernel_driver(0).expect("Failed to detach kernel driver");
    }
    handle.claim_interface(0).expect("Failed to claim interface");
    drain(&handle);

    match mode {
        "read" => phase1_deep_reads(&handle),
        "name" => {
            let new_name = args.get(2).map(|s| s.as_str()).unwrap_or("xbelite2");
            phase2_write_name(&handle, new_name);
        }
        "profiles" => phase3_probe_profiles(&handle),
        "led" => phase4_probe_led(&handle),
        _ => {
            println!("Usage:");
            println!("  xbe2-rw read           Deep read all commands");
            println!("  xbe2-rw name <text>    Write device name (max 16 chars)");
            println!("  xbe2-rw profiles       Probe profile data slots");
            println!("  xbe2-rw led            Probe LED commands");
        }
    }

    let _ = handle.release_interface(0);
}

fn phase1_deep_reads(handle: &rusb::DeviceHandle<rusb::GlobalContext>) {
    println!("--- Phase 1: Deep Reads ---\n");

    // Read device name
    println!("[0x1E sub 0x05] Device Name:");
    let resp = gip_read(handle, 0x1E, &[0x05]);
    if let Some(data) = &resp {
        if data.len() >= 6 {
            let name_bytes = &data[5..];
            if let Ok(name) = std::str::from_utf8(name_bytes) {
                println!("  UTF-8: \"{name}\"");
            }
            // Try UTF-16LE
            let u16s: Vec<u16> = name_bytes.chunks(2)
                .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                .collect();
            let name16 = String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string();
            println!("  UTF-16LE: \"{name16}\"");
        }
    }

    // Read calibration
    println!("\n[0x1E sub 0x0F] Calibration/Config:");
    let resp = gip_read(handle, 0x1E, &[0x0F]);
    if let Some(data) = &resp {
        let payload = &data[4..];
        println!("  Sub: 0x{:02x}, Status: 0x{:02x}", payload[0], payload[1]);
        if payload.len() >= 22 {
            // Parse as u16 LE pairs
            let vals: Vec<u16> = payload[2..].chunks(2)
                .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                .collect();
            println!("  Values (u16 LE): {:?}", vals);
            if vals.len() >= 10 {
                println!("  Possible stick calibration:");
                println!("    LX center: {}", vals[0]);
                println!("    LY center: {}", vals[1]);
                println!("    RX center: {}", vals[2]);
                println!("    RY center: {}", vals[3]);
                println!("    Remaining: {:?}", &vals[4..]);
            }
        }
    }

    // 0x4D sub 0x03 — returned [0x80, 0x20] before
    println!("\n[0x4D sub 0x03] Capability/Size Info:");
    let resp = gip_4d_read(handle, &[0x03]);
    if let Some(data) = &resp {
        let payload = &data[4..];
        println!("  Values: {:02x?}", payload);
        if payload.len() >= 3 {
            println!("  Decoded: sub=0x{:02x} val1={} val2={}", payload[0], payload[1], payload[2]);
        }
    }

    // 0x4D sub 0x05 — 34 zero bytes before, try with profile parameter
    println!("\n[0x4D sub 0x05] Data Slot (no param):");
    let resp = gip_4d_read(handle, &[0x05]);
    print_response(&resp);

    for p in 0..4u8 {
        println!("\n[0x4D sub 0x05, param {p}] Data Slot:");
        let resp = gip_4d_read(handle, &[0x05, p]);
        print_response(&resp);
    }

    // Try 0x4D sub 0x00 with profile params
    for p in 0..4u8 {
        println!("\n[0x4D sub 0x00, param {p}]:");
        let resp = gip_4d_read(handle, &[0x00, p]);
        print_response(&resp);
    }

    // Try 0x4D sub 0x01 with profile params
    for p in 0..4u8 {
        println!("\n[0x4D sub 0x01, param {p}]:");
        let resp = gip_4d_read(handle, &[0x01, p]);
        print_response(&resp);
    }

    // Try 0x4D sub 0x02 with profile params
    for p in 0..4u8 {
        println!("\n[0x4D sub 0x02, param {p}]:");
        let resp = gip_4d_read(handle, &[0x02, p]);
        print_response(&resp);
    }

    // Try 0x4D sub 0x04 with various params
    println!("\n[0x4D sub 0x04] With params:");
    for p in 0..8u8 {
        let resp = gip_4d_read(handle, &[0x04, p]);
        if let Some(data) = &resp {
            let payload = &data[4..];
            print!("  param {p}: ");
            println!("{:02x?}", payload);
        } else {
            println!("  param {p}: no response");
        }
    }

    // Try 0x4D sub 0x06 with params (profile switch?)
    println!("\n[0x4D sub 0x06] With params:");
    for p in 0..4u8 {
        let resp = gip_4d_read(handle, &[0x06, p]);
        if let Some(data) = &resp {
            print!("  param {p}: ");
            println!("{:02x?}", &data[4..]);
        }
    }

    // Read 0x1E sub 0x02 (returned [02, 03] before)
    println!("\n[0x1E sub 0x02] Version?:");
    let resp = gip_read(handle, 0x1E, &[0x02]);
    print_response(&resp);

    // Read 0x1E sub 0x0C and 0x0D
    println!("\n[0x1E sub 0x0C]:");
    let resp = gip_read(handle, 0x1E, &[0x0C]);
    print_response(&resp);

    println!("\n[0x1E sub 0x0D]:");
    let resp = gip_read(handle, 0x1E, &[0x0D]);
    print_response(&resp);
}

fn phase2_write_name(handle: &rusb::DeviceHandle<rusb::GlobalContext>, new_name: &str) {
    println!("--- Phase 2: Write Device Name ---\n");

    // First read current name
    println!("Current name:");
    let resp = gip_read(handle, 0x1E, &[0x05]);
    if let Some(data) = &resp {
        let name_bytes = &data[9..]; // skip header + sub + status
        let u16s: Vec<u16> = name_bytes.chunks(2)
            .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
            .collect();
        let old_name = String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string();
        println!("  \"{old_name}\"\n");
    }

    // Encode new name as UTF-16LE, padded to 32 bytes (16 chars max)
    let truncated: String = new_name.chars().take(15).collect();
    let u16s: Vec<u16> = truncated.encode_utf16().collect();
    let mut name_buf = vec![0u8; 32]; // 16 UTF-16 chars = 32 bytes
    for (i, c) in u16s.iter().enumerate() {
        let bytes = c.to_le_bytes();
        name_buf[i * 2] = bytes[0];
        name_buf[i * 2 + 1] = bytes[1];
    }

    println!("Writing new name: \"{truncated}\"");
    println!("  UTF-16LE bytes: {:02x?}", &name_buf[..u16s.len() * 2 + 2]);

    // Build write command: 0x1E with sub 0x05 + name data
    // Format: [0x1E, flags, seq, payload_len, sub_cmd, name_data...]
    let mut cmd = vec![0x1E, 0x30, 0x40]; // cmd, flags (system+needack), seq
    let payload_len = 1 + name_buf.len(); // sub + name
    cmd.push(payload_len as u8);
    cmd.push(0x05); // sub-command
    cmd.extend_from_slice(&name_buf);

    println!("  Sending {} bytes: {:02x?}", cmd.len(), &cmd[..8]);

    match handle.write_interrupt(EP_OUT, &cmd, Duration::from_millis(1000)) {
        Ok(n) => println!("  Wrote {n} bytes"),
        Err(e) => {
            println!("  Write failed: {e}");
            return;
        }
    }

    // Read response
    for _ in 0..5 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(500)) {
            Ok(n) => {
                if buf[0] == 0x20 || buf[0] == 0x02 { continue; }
                println!("  Response: cmd=0x{:02x} {:02x?}", buf[0], &buf[..n]);
            }
            Err(rusb::Error::Timeout) => break,
            Err(_) => break,
        }
    }
    drain(handle);

    // Verify by reading back
    println!("\nVerifying:");
    let resp = gip_read(handle, 0x1E, &[0x05]);
    if let Some(data) = &resp {
        if data.len() > 9 {
            let name_bytes = &data[9..];
            let u16s: Vec<u16> = name_bytes.chunks(2)
                .filter_map(|c| if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None })
                .collect();
            let readback = String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string();
            println!("  Read back: \"{readback}\"");
            if readback == truncated {
                println!("  SUCCESS! Name changed.");
            } else {
                println!("  Name didn't change (write may need different format).");
            }
        }
    }
}

fn phase3_probe_profiles(handle: &rusb::DeviceHandle<rusb::GlobalContext>) {
    println!("--- Phase 3: Profile Probes ---\n");

    // Try reading profile data with different sub-command + param combos
    // The "access denied" commands (0x06, 0x08, 0x09, 0x0A in 0x1E) might be profile-related

    println!("[0x1E] Probing denied commands with params:");
    for sub in [0x06u8, 0x08, 0x09, 0x0A] {
        for param in 0..4u8 {
            let resp = gip_read(handle, 0x1E, &[sub, param]);
            if let Some(data) = &resp {
                let payload = &data[4..];
                if payload.len() >= 2 && payload[1] != 0x04 {
                    println!("  0x1E sub 0x{sub:02x} param {param}: NON-DENIED! {:02x?}", payload);
                }
            }
        }
    }
    println!("  (all returned access denied)\n");

    // Try 0x4D with larger payloads — maybe we need to specify an offset
    println!("[0x4D] Profile read attempts:");
    // Maybe format is: [sub, profile_id, offset_lo, offset_hi, length]
    for profile in 0..4u8 {
        for sub in [0x00u8, 0x01, 0x02, 0x05] {
            let resp = gip_4d_read(handle, &[sub, profile, 0x00, 0x00]);
            if let Some(data) = &resp {
                let payload = &data[4..];
                let is_interesting = payload.iter().any(|b| *b != 0 && *b != payload[0]);
                if is_interesting || payload.len() > 4 {
                    println!("  sub=0x{sub:02x} profile={profile} 4-byte param: {:02x?}", payload);
                }
            }
        }
    }

    // Try requesting with explicit read length
    println!("\n[0x4D sub 0x05] Extended reads:");
    for param in [0x00u8, 0x01, 0x02, 0x03, 0x10, 0x20, 0x40, 0x80] {
        let resp = gip_4d_read(handle, &[0x05, param]);
        if let Some(data) = &resp {
            let payload = &data[4..];
            let nonzero = payload.iter().filter(|b| **b != 0).count();
            if nonzero > 1 {
                println!("  param=0x{param:02x}: {:02x?}", payload);
            } else {
                println!("  param=0x{param:02x}: {} bytes, {} non-zero", payload.len(), nonzero);
            }
        }
    }
}

fn phase4_probe_led(handle: &rusb::DeviceHandle<rusb::GlobalContext>) {
    println!("--- Phase 4: LED Probes ---\n");

    // Standard GIP LED command (0x0A) — guide button brightness
    println!("[0x0A] Guide Button LED:");
    for pattern in [0x00u8, 0x01, 0x02, 0x0D] {
        for intensity in [0u8, 20, 47] {
            let cmd = [0x0A, 0x20, 0x50, 0x03, 0x00, pattern, intensity];
            println!("  pattern=0x{pattern:02x} intensity={intensity}");
            let _ = handle.write_interrupt(EP_OUT, &cmd, Duration::from_millis(500));
            std::thread::sleep(Duration::from_millis(800));
            drain(handle);
        }
    }

    // Reset to default
    let cmd = [0x0A, 0x20, 0x51, 0x03, 0x00, 0x01, 0x14];
    let _ = handle.write_interrupt(EP_OUT, &cmd, Duration::from_millis(500));
    println!("  Reset to default\n");
    drain(handle);

    // Try 0x4D for LED color — maybe sub 0x04 or 0x06?
    println!("[0x4D] Possible LED sub-commands:");
    // Sub 0x04 returned 0x00 — might be LED state
    // Try writing color values
    for sub in [0x04u8, 0x06, 0x08, 0x09, 0x0A] {
        println!("  sub=0x{sub:02x} current value:");
        let resp = gip_4d_read(handle, &[sub]);
        print_response(&resp);
    }
}

// Send a 0x1E extended command and read the response
fn gip_read(handle: &rusb::DeviceHandle<rusb::GlobalContext>, cmd: u8, payload: &[u8]) -> Option<Vec<u8>> {
    static SEQ: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0x60);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut pkt = vec![cmd, 0x30, seq, payload.len() as u8];
    pkt.extend_from_slice(payload);

    handle.write_interrupt(EP_OUT, &pkt, Duration::from_millis(500)).ok()?;

    for _ in 0..15 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
            Ok(n) => {
                if buf[0] == 0x20 || buf[0] == 0x02 { continue; } // skip input/hello
                if buf[0] == 0x01 { continue; } // skip ACK
                if buf[0] == cmd { return Some(buf[..n].to_vec()); }
            }
            Err(_) => break,
        }
    }
    drain(handle);
    None
}

// Send a 0x4D vendor command and read the response
fn gip_4d_read(handle: &rusb::DeviceHandle<rusb::GlobalContext>, payload: &[u8]) -> Option<Vec<u8>> {
    static SEQ: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0x80);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut pkt = vec![0x4D, 0x10, seq, payload.len() as u8];
    pkt.extend_from_slice(payload);

    handle.write_interrupt(EP_OUT, &pkt, Duration::from_millis(500)).ok()?;

    for _ in 0..15 {
        let mut buf = [0u8; 64];
        match handle.read_interrupt(EP_IN, &mut buf, Duration::from_millis(300)) {
            Ok(n) => {
                if buf[0] == 0x20 || buf[0] == 0x02 { continue; }
                if buf[0] == 0x01 { continue; } // ACK
                if buf[0] == 0x4D { return Some(buf[..n].to_vec()); }
            }
            Err(_) => break,
        }
    }
    drain(handle);
    None
}

fn print_response(resp: &Option<Vec<u8>>) {
    match resp {
        Some(data) => {
            println!("  {} bytes: {:02x?}", data.len(), data);
            if data.len() > 4 {
                print!("  payload: ");
                for b in &data[4..] {
                    if b.is_ascii_graphic() || *b == b' ' { print!("{}", *b as char); }
                    else { print!("."); }
                }
                println!();
            }
        }
        None => println!("  (no response)"),
    }
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
