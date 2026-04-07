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

/// Write a raw profile page to the controller. Requires unlock() first.
pub fn write_page(dev: &mut GipDevice, page: u8, data: &[u8]) {
    let mut payload = vec![0x01, page, data.len() as u8];
    payload.extend_from_slice(data);
    dev.vendor_cmd(&payload);
}

/// Commit/persist written profile data to controller flash.
/// Must be called after writing pages, otherwise changes are lost on reboot.
/// Sequence: re-init extended reports, then send persist command.
pub fn commit(dev: &mut GipDevice) {
    dev.init_extended();
    dev.vendor_cmd(&[0x03]);
}

/// Read the mapping page for a profile (1-3) and slot (0=A/normal, 1=B/shift).
pub fn read_mapping(dev: &mut GipDevice, profile: usize, slot: usize) -> Option<ProfileMapping> {
    let page = PROFILE_MAPPING_PAGES[profile][slot];
    let raw = read_page(dev, page, MAPPING_SIZE)?;
    ProfileMapping::from_raw(&raw)
}

/// Read the curves page for a profile (1-3) and slot (0=A, 1=B).
pub fn read_curves(dev: &mut GipDevice, profile: usize, slot: usize) -> Option<ProfileCurves> {
    let page = PROFILE_CURVES_PAGES[profile][slot];
    let raw = read_page(dev, page, CURVES_SIZE)?;
    ProfileCurves::from_raw(&raw)
}

/// Write a mapping page. Modifies raw bytes and writes back.
pub fn write_mapping(dev: &mut GipDevice, profile: usize, slot: usize, data: &[u8]) {
    let page = PROFILE_MAPPING_PAGES[profile][slot];
    write_page(dev, page, data);
}

/// Write a curves page.
pub fn write_curves(dev: &mut GipDevice, profile: usize, slot: usize, data: &[u8]) {
    let page = PROFILE_CURVES_PAGES[profile][slot];
    write_page(dev, page, data);
}

/// Set the LED color for a profile. Updates both slots and persists.
pub fn set_color(dev: &mut GipDevice, profile: usize, r: u8, g: u8, b: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            data[OFF_COLOR_FLAG] = 0x00; // custom
            data[OFF_COLOR_R] = r;
            data[OFF_COLOR_G] = g;
            data[OFF_COLOR_B] = b;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Reset color to default (white) for a profile.
pub fn reset_color(dev: &mut GipDevice, profile: usize) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            data[OFF_COLOR_FLAG] = 0xFF;
            data[OFF_COLOR_R] = 0;
            data[OFF_COLOR_G] = 0;
            data[OFF_COLOR_B] = 0;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Set dead zones for a profile. Updates both slots and persists.
pub fn set_deadzones(dev: &mut GipDevice, profile: usize, lstick: u8, rstick: u8, ltrig: u8, rtrig: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            data[OFF_DEADZONES] = lstick;
            data[OFF_DEADZONES + 1] = rstick;
            data[OFF_DEADZONES + 2] = ltrig;
            data[OFF_DEADZONES + 3] = rtrig;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Set vibration intensity for a profile. Updates both slots and persists.
pub fn set_vibration(dev: &mut GipDevice, profile: usize, left: u8, right: u8) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            data[OFF_VIBRATION] = left;
            data[OFF_VIBRATION + 1] = right;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Remap buttons for a profile.
/// `remaps` is a list of (from, to) pairs.
/// Normal mode remaps go to SlotA, shift remaps go to SlotB.
pub fn remap_buttons(dev: &mut GipDevice, profile: usize, remaps: &[(GipButton, GipButton)]) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            for &(from, to) in remaps {
                let from_code = from.code();
                let to_code = to.code();
                if from_code >= 0x04 && from_code <= 0x07 {
                    // Face button
                    let idx = (from_code - 0x04) as usize;
                    let off = if slot == 0 { OFF_REMAP_A } else { OFF_REMAP_B };
                    data[off + idx] = to_code;
                } else if from_code >= 0x08 && from_code <= 0x0F {
                    // Extended button (shared across slots)
                    let idx = (from_code - 0x08) as usize;
                    data[OFF_REMAP_EXT + idx] = to_code;
                }
            }
            data[OFF_FLAGS] = FLAGS_CUSTOM;
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Remap buttons only in shift mode (SlotB).
pub fn remap_shift(dev: &mut GipDevice, profile: usize, remaps: &[(GipButton, GipButton)]) {
    let page = PROFILE_MAPPING_PAGES[profile][1]; // SlotB
    if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
        let mut data = raw;
        for &(from, to) in remaps {
            let from_code = from.code();
            let to_code = to.code();
            if from_code >= 0x04 && from_code <= 0x07 {
                let idx = (from_code - 0x04) as usize;
                data[OFF_REMAP_B + idx] = to_code;
            } else if from_code >= 0x08 && from_code <= 0x0F {
                let idx = (from_code - 0x08) as usize;
                data[OFF_REMAP_EXT + idx] = to_code;
            }
        }
        data[OFF_FLAGS] = FLAGS_CUSTOM;
        write_page(dev, page, &data);
    }
    commit(dev);
}

/// Reset all button remaps to default for a profile.
pub fn reset_remaps(dev: &mut GipDevice, profile: usize) {
    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, MAPPING_SIZE) {
            let mut data = raw;
            data[OFF_REMAP_A..OFF_REMAP_A + 4].copy_from_slice(&DEFAULT_FACE);
            data[OFF_REMAP_B..OFF_REMAP_B + 4].copy_from_slice(&DEFAULT_FACE);
            data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&DEFAULT_EXT);
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Set stick curves for a profile. Updates both slots.
/// Each curve is [x1, y1, x2, y2, x3, y3] — 3 control points.
pub fn set_curves(
    dev: &mut GipDevice,
    profile: usize,
    lx: [u8; 6],
    ly: [u8; 6],
    rx: [u8; 6],
    ry: [u8; 6],
) {
    for slot in 0..2 {
        let page = PROFILE_CURVES_PAGES[profile][slot];
        if let Some(raw) = read_page(dev, page, CURVES_SIZE) {
            let mut data = raw;
            data[1..7].copy_from_slice(&lx);
            data[7..13].copy_from_slice(&ly);
            data[13..19].copy_from_slice(&rx);
            data[19..25].copy_from_slice(&ry);
            write_page(dev, page, &data);
        }
    }
    commit(dev);
}

/// Reset stick curves to default linear for a profile.
pub fn reset_curves(dev: &mut GipDevice, profile: usize) {
    set_curves(dev, profile, DEFAULT_CURVE, DEFAULT_CURVE, DEFAULT_CURVE, DEFAULT_CURVE);
}

/// Read all profile data (both slots, mapping + curves) for a profile (0-indexed).
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
