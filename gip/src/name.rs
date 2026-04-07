use crate::transport::GipDevice;

/// Read the device name (controller name, not profile name).
pub fn read(dev: &mut GipDevice) -> Option<String> {
    let resp = dev.vendor_cmd(&[0x05])?;
    let payload = &resp[4..];
    if payload.len() < 4 {
        return None;
    }
    let name_bytes = &payload[2..];
    let u16s: Vec<u16> = name_bytes
        .chunks(2)
        .filter_map(|c| {
            if c.len() == 2 {
                Some(u16::from_le_bytes([c[0], c[1]]))
            } else {
                None
            }
        })
        .collect();
    Some(
        String::from_utf16_lossy(&u16s)
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Write the device name (max 15 characters). Requires unlock() first.
/// Returns the name that was actually written (truncated if needed).
pub fn write(dev: &mut GipDevice, new_name: &str) -> Option<String> {
    let truncated: String = new_name.chars().take(15).collect();
    let u16s: Vec<u16> = truncated.encode_utf16().collect();
    let mut name_buf = vec![0u8; 32]; // 16 UTF-16 chars = 32 bytes
    for (i, c) in u16s.iter().enumerate() {
        let bytes = c.to_le_bytes();
        name_buf[i * 2] = bytes[0];
        name_buf[i * 2 + 1] = bytes[1];
    }

    // Write via 0x4D sub 0x04
    let mut payload = vec![0x04];
    payload.extend_from_slice(&name_buf);
    dev.vendor_cmd(&payload);
    dev.drain();

    // Read back to verify
    read(dev)
}
