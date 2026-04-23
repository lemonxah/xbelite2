use crate::transport::GipDevice;
use crate::types::*;

/// Read a raw profile page from the controller.
pub fn read_page(dev: &mut GipDevice, page: u8, size: u8) -> Option<Vec<u8>> {
    let resp = dev.vendor_cmd(&[0x02, page, size])?;
    let payload = &resp[4..];
    if payload.len() < 4 || payload[0] != 0x02 {
        return None;
    }
    Some(payload[4..].to_vec())
}

/// Write a raw profile page to the controller.
/// Uses fire-and-forget to avoid ring buffer response conflicts.
pub fn write_page(dev: &mut GipDevice, page: u8, data: &[u8]) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static WRITE_SEQ: AtomicU8 = AtomicU8::new(0xD0);
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut pkt = vec![0x4D, 0x10, seq, (3 + data.len()) as u8, 0x01, page, data.len() as u8];
    pkt.extend_from_slice(data);
    let _ = dev.send(&pkt);
    std::thread::sleep(std::time::Duration::from_millis(100));
}

/// Full unlock dance before writing profile pages.
pub fn begin_write(dev: &mut GipDevice) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static SEQ: AtomicU8 = AtomicU8::new(0xA0);
    for _ in 0..2 {
        let s = SEQ.fetch_add(1, Ordering::Relaxed);
        let _ = dev.send(&[0x4D, 0x10, s, 0x02, 0x07, 0x00]);
    }
    let s = SEQ.fetch_add(1, Ordering::Relaxed);
    let _ = dev.send(&[0x4D, 0x10, s, 0x01, 0x03]);
    let s = SEQ.fetch_add(1, Ordering::Relaxed);
    let _ = dev.send(&[0x4D, 0x10, s, 0x02, 0x07, 0x00]);
    for _ in 0..2 {
        let s = SEQ.fetch_add(1, Ordering::Relaxed);
        let _ = dev.send(&[0x4D, 0x10, s, 0x01, 0x03]);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    dev.drain();
}

/// Commit written profile data — re-inits extended reports and sends persist command.
/// Uses fire-and-forget sends to avoid ring buffer response conflicts with any
/// concurrent reader on `/dev/xbelite2` (vendor_cmd would block waiting for a
/// response another reader might consume).
pub fn commit(dev: &mut GipDevice) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static COMMIT_SEQ: AtomicU8 = AtomicU8::new(0xE0);
    // Re-init extended reports (equivalent to old init_extended / vendor_cmd [0x07, 0x00])
    let seq = COMMIT_SEQ.fetch_add(1, Ordering::Relaxed);
    let _ = dev.send(&[0x4D, 0x10, seq, 0x02, 0x07, 0x00]);
    std::thread::sleep(std::time::Duration::from_millis(150));
    // Persist/unlock command (equivalent to old vendor_cmd [0x03])
    let seq = COMMIT_SEQ.fetch_add(1, Ordering::Relaxed);
    let _ = dev.send(&[0x4D, 0x10, seq, 0x01, 0x03]);
    std::thread::sleep(std::time::Duration::from_millis(150));
    dev.drain();
}

/// Read the mapping page for a profile (0-indexed) and slot (0=A/normal, 1=B/shift).
pub fn read_mapping(dev: &mut GipDevice, profile: usize, slot: usize) -> Option<ProfileMapping> {
    let page = PROFILE_MAPPING_PAGES[profile][slot];
    let raw = read_page(dev, page, MAPPING_SIZE)?;
    ProfileMapping::from_raw(&raw)
}

/// Read the curves page for a profile and slot.
pub fn read_curves(dev: &mut GipDevice, profile: usize, slot: usize) -> Option<ProfileCurves> {
    let page = PROFILE_CURVES_PAGES[profile][slot];
    let raw = read_page(dev, page, CURVES_SIZE)?;
    ProfileCurves::from_raw(&raw)
}

/// Write a mapping page.
pub fn write_mapping(dev: &mut GipDevice, profile: usize, slot: usize, data: &[u8]) {
    let page = PROFILE_MAPPING_PAGES[profile][slot];
    write_page(dev, page, data);
}

/// Write a curves page.
pub fn write_curves(dev: &mut GipDevice, profile: usize, slot: usize, data: &[u8]) {
    let page = PROFILE_CURVES_PAGES[profile][slot];
    write_page(dev, page, data);
}

// --- Helper: apply a remap to a data buffer ---

fn apply_remap_to_data(data: &mut Vec<u8>, from: GipButton, to: GipButton) {
    let fc = from.code();
    let tc = to.code();
    if fc >= 0x04 && fc <= 0x07 {
        // Face button
        data[OFF_FACE + (fc - 0x04) as usize] = tc;
    } else if let Some(slot) = ext_slot_for_button(fc) {
        // Extended: DPad, LB, RB, LStick, RStick
        data[OFF_REMAP_EXT + slot] = tc;
    }
}

// --- Public remap functions ---

/// Set or clear the shift modifier button using cached data (avoids read conflicts).
/// When set, removes the button from ext[] (shifted up, 0x00 at end) and sets flags bit 0.
/// `cached_data` should be the raw 56 bytes from the hw_profile cache.
pub fn set_shift_button_from_cache(dev: &mut GipDevice, profile: usize, button: Option<GipButton>, cached_slot_a: &[u8], cached_slot_b: &[u8]) {
    for (slot, cached) in [(0, cached_slot_a), (1, cached_slot_b)] {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        let mut data = cached.to_vec();
        if data.len() < 56 { continue; }

        match button {
            Some(btn) => {
                let btn_code = btn.code();
                if let Some(slot_idx) = ext_slot_for_button(btn_code) {
                    let ext_start = OFF_REMAP_EXT;
                    for i in slot_idx..7 {
                        data[ext_start + i] = data[ext_start + i + 1];
                    }
                    data[ext_start + 7] = 0x00;
                }
                data[OFF_FLAGS] = (data[OFF_FLAGS] & !0x01) | 0x01;
            }
            None => {
                data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&DEFAULT_EXT);
                data[OFF_FLAGS] &= !0x01;
            }
        }

        write_page(dev, page, &data);
    }
    commit(dev);
}

/// Write a remap using cached data (avoids read conflicts with other readers).
pub fn write_remap_from_cache(dev: &mut GipDevice, page: u8, mut data: Vec<u8>, from: GipButton, to: GipButton) {
    let fc = from.code();
    let tc = to.code();
    if fc >= 0x04 && fc <= 0x07 {
        data[OFF_FACE + (fc - 0x04) as usize] = tc;
    } else if let Some(slot) = ext_slot_for_button(fc) {
        data[OFF_REMAP_EXT + slot] = tc;
    }
    write_page(dev, page, &data);
}

/// Write a paddle remap using cached data.
pub fn write_paddle_from_cache(dev: &mut GipDevice, page: u8, mut data: Vec<u8>, paddle_idx: usize, to: GipButton) {
    if paddle_idx < 4 {
        data[OFF_PADDLES + paddle_idx] = to.code();
    }
    write_page(dev, page, &data);
}

/// Remap buttons in normal mode (SlotA).
pub fn remap_buttons(dev: &mut GipDevice, profile: usize, remaps: &[(GipButton, GipButton)]) {
    let page = PROFILE_MAPPING_PAGES[profile][0];
    if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
        for &(from, to) in remaps {
            apply_remap_to_data(&mut data, from, to);
        }
        write_page(dev, page, &data);
    }
    commit(dev);
}

/// Remap paddles. paddle_idx: 0=P1, 1=P2, 2=P3, 3=P4.
pub fn remap_paddles(dev: &mut GipDevice, profile: usize, remaps: &[(usize, GipButton)]) {
    let page = PROFILE_MAPPING_PAGES[profile][0];
    if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
        for &(paddle_idx, to) in remaps {
            if paddle_idx < 4 {
                data[OFF_PADDLES + paddle_idx] = to.code();
            }
        }
        write_page(dev, page, &data);
    }
    commit(dev);
}

/// Remap buttons in shift mode (SlotB).
pub fn remap_shift(dev: &mut GipDevice, profile: usize, remaps: &[(GipButton, GipButton)]) {
    let page = PROFILE_MAPPING_PAGES[profile][1];
    if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
        for &(from, to) in remaps {
            apply_remap_to_data(&mut data, from, to);
        }
        write_page(dev, page, &data);
    }
    commit(dev);
}

/// Set the LED color for a profile. Updates both slots and persists.
pub fn set_color(dev: &mut GipDevice, profile: usize, r: u8, g: u8, b: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
            data[OFF_COLOR_FLAG] = 0x00;
            data[OFF_COLOR_R] = r;
            data[OFF_COLOR_G] = g;
            data[OFF_COLOR_B] = b;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Reset color to default.
pub fn reset_color(dev: &mut GipDevice, profile: usize) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
            data[OFF_COLOR_FLAG] = 0xFF;
            data[OFF_COLOR_R] = 0;
            data[OFF_COLOR_G] = 0;
            data[OFF_COLOR_B] = 0;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Set dead zones for a profile. Updates both slots.
pub fn set_deadzones(dev: &mut GipDevice, profile: usize, lstick: u8, rstick: u8, ltrig: u8, rtrig: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
            data[OFF_DEADZONES] = lstick;
            data[OFF_DEADZONES + 1] = rstick;
            data[OFF_DEADZONES + 2] = ltrig;
            data[OFF_DEADZONES + 3] = rtrig;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Set vibration intensity for a profile. Updates both slots.
pub fn set_vibration(dev: &mut GipDevice, profile: usize, left: u8, right: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
            data[OFF_VIBRATION] = left;
            data[OFF_VIBRATION + 1] = right;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Reset all button remaps to default for a profile.
pub fn reset_remaps(dev: &mut GipDevice, profile: usize) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, MAPPING_SIZE) {
            data[OFF_PADDLES..OFF_PADDLES + 4].copy_from_slice(&DEFAULT_FACE);
            data[OFF_FACE..OFF_FACE + 4].copy_from_slice(&DEFAULT_FACE);
            data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&DEFAULT_EXT);
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Fully reset a profile to factory defaults (all 4 pages).
pub fn reset_profile(dev: &mut GipDevice, profile: usize) {
    let mut mapping = vec![0u8; 56];
    mapping[OFF_FLAGS] = FLAGS_DEFAULT;
    mapping[OFF_PADDLES..OFF_PADDLES + 4].copy_from_slice(&DEFAULT_FACE);
    mapping[OFF_FACE..OFF_FACE + 4].copy_from_slice(&DEFAULT_FACE);
    mapping[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&DEFAULT_EXT);
    mapping[OFF_DEADZONES] = 100;
    mapping[OFF_DEADZONES + 1] = 100;
    mapping[OFF_DEADZONES + 2] = 100;
    mapping[OFF_DEADZONES + 3] = 100;
    let default_ranges: [u8; 12] = [0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00];
    mapping[32..44].copy_from_slice(&default_ranges);
    mapping[OFF_BRIGHTNESS] = 100;
    mapping[OFF_COLOR_FLAG] = 0xFF;
    mapping[OFF_VIBRATION] = 48;
    mapping[OFF_VIBRATION + 1] = 48;

    let mut curves = vec![0u8; 43];
    curves[0] = FLAGS_DEFAULT;
    for i in 0..4 {
        curves[1 + i * 6..7 + i * 6].copy_from_slice(&DEFAULT_CURVE);
    }

    for slot in 0..2 {
        write_page(dev, PROFILE_MAPPING_PAGES[profile][slot], &mapping);
        write_page(dev, PROFILE_CURVES_PAGES[profile][slot], &curves);
    }
    commit(dev);
}

/// Set stick curves for a profile. Updates both slots.
pub fn set_curves(dev: &mut GipDevice, profile: usize, lx: [u8; 6], ly: [u8; 6], rx: [u8; 6], ry: [u8; 6]) {
    for slot in 0..2 {
        let page = PROFILE_CURVES_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, CURVES_SIZE) {
            data[1..7].copy_from_slice(&lx);
            data[7..13].copy_from_slice(&ly);
            data[13..19].copy_from_slice(&rx);
            data[19..25].copy_from_slice(&ry);
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Reset stick curves to default linear.
pub fn reset_curves(dev: &mut GipDevice, profile: usize) {
    set_curves(dev, profile, DEFAULT_CURVE, DEFAULT_CURVE, DEFAULT_CURVE, DEFAULT_CURVE);
}

/// Stick inversion mask location in the curves page.
/// bit0=LY, bit1=RY, bit2=LX, bit3=RX.
const OFF_STICK_INVERSION: usize = 27;

/// Read the stick inversion bitmask from the curves page (SlotA).
pub fn get_stick_inversion(dev: &mut GipDevice, profile: usize) -> Option<u8> {
    let page = PROFILE_CURVES_PAGES[profile][0];
    let raw = read_page(dev, page, CURVES_SIZE)?;
    raw.get(OFF_STICK_INVERSION).copied()
}

/// Set the stick inversion bitmask on both slots of a profile's curves page.
pub fn set_stick_inversion(dev: &mut GipDevice, profile: usize, mask: u8) {
    for slot in 0..2 {
        let page = PROFILE_CURVES_PAGES[profile][slot];
        if let Some(mut data) = read_page(dev, page, CURVES_SIZE) {
            if data.len() > OFF_STICK_INVERSION {
                data[OFF_STICK_INVERSION] = mask;
                write_page(dev, page, &data);
            }
        }
    }
    commit(dev);
}

/// Read all profile data (both slots, mapping + curves).
pub struct FullProfile {
    pub mapping_a: Option<ProfileMapping>,
    pub mapping_b: Option<ProfileMapping>,
    pub curves_a: Option<ProfileCurves>,
    pub curves_b: Option<ProfileCurves>,
}

pub fn read_full(dev: &mut GipDevice, profile: usize) -> FullProfile {
    FullProfile {
        mapping_a: read_mapping(dev, profile, 0),
        mapping_b: read_mapping(dev, profile, 1),
        curves_a: read_curves(dev, profile, 0),
        curves_b: read_curves(dev, profile, 1),
    }
}
