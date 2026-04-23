use serde::{Deserialize, Serialize};

use crate::transport::GipDevice;
use crate::types::*;
use crate::profile;

/// In-memory snapshot of the 3 hardware profiles read from the controller.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwProfileCache {
    pub profiles: [HwProfile; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwProfile {
    /// Paddle outputs (bytes 1-4): [P1, P2, P3, P4]
    pub paddles: [u8; 4],
    /// Face button outputs (bytes 5-8): [A, B, X, Y]
    pub face: [u8; 4],
    /// Extended outputs (bytes 9-16): [DUp, DDown, DLeft, DRight, LB, RB, LStick, RStick]
    pub ext: [u8; 8],
    /// SlotB paddle outputs (shift mode page)
    #[serde(default = "default_paddles")]
    pub shift_paddles: [u8; 4],
    /// SlotB face remap (shift mode page)
    pub shift_face: [u8; 4],
    /// SlotB extended remap (shift mode page)
    pub shift_ext: [u8; 8],
    /// Reserved region (bytes 17-27)
    pub reserved: [u8; 11],
    /// Dead zones: [LStick, RStick, LTrigger, RTrigger]
    pub deadzones: [u8; 4],
    /// Color (None = default white)
    pub color: Option<(u8, u8, u8)>,
    /// LED brightness (0-100, default 100)
    pub brightness: u8,
    /// Stick inversion bitmask (bit0=LY, bit1=RY) from curves page byte 27
    pub stick_inversion: u8,
    /// Vibration: (left, right)
    pub vibration: (u8, u8),
}

fn default_paddles() -> [u8; 4] { DEFAULT_FACE }

impl HwProfile {
    pub fn is_ext_remapped(&self, index: usize) -> bool {
        if index >= 8 { return false; }
        self.ext[index] != DEFAULT_EXT[index]
    }

    pub fn is_face_remapped(&self, index: usize) -> bool {
        if index >= 4 { return false; }
        self.face[index] != DEFAULT_FACE[index]
    }

    pub fn has_any_remap(&self) -> bool {
        self.face != DEFAULT_FACE
            || self.paddles != DEFAULT_FACE
            || self.ext != DEFAULT_EXT
    }

    pub fn has_paddle_remaps(&self) -> bool {
        self.paddles != DEFAULT_FACE
    }

    /// Reconstruct the raw 56-byte SlotA mapping page from cached data.
    pub fn to_slot_a_bytes(&self) -> Vec<u8> {
        let mut data = vec![0u8; 56];
        data[OFF_FLAGS] = FLAGS_DEFAULT;
        data[OFF_PADDLES..OFF_PADDLES + 4].copy_from_slice(&self.paddles);
        data[OFF_FACE..OFF_FACE + 4].copy_from_slice(&self.face);
        data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&self.ext);
        data[17..28].copy_from_slice(&self.reserved);
        data[OFF_DEADZONES..OFF_DEADZONES + 4].copy_from_slice(&self.deadzones);
        data[OFF_BRIGHTNESS] = self.brightness;
        if let Some((r, g, b)) = self.color {
            data[OFF_COLOR_FLAG] = 0x00;
            data[OFF_COLOR_R] = r;
            data[OFF_COLOR_G] = g;
            data[OFF_COLOR_B] = b;
        } else {
            data[OFF_COLOR_FLAG] = 0xFF;
        }
        data[OFF_VIBRATION] = self.vibration.0;
        data[OFF_VIBRATION + 1] = self.vibration.1;
        data
    }

    /// Reconstruct the raw 56-byte SlotB mapping page from cached data.
    pub fn to_slot_b_bytes(&self) -> Vec<u8> {
        let mut data = self.to_slot_a_bytes();
        // SlotB uses shift variants instead of normal
        data[OFF_PADDLES..OFF_PADDLES + 4].copy_from_slice(&self.shift_paddles);
        data[OFF_FACE..OFF_FACE + 4].copy_from_slice(&self.shift_face);
        data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&self.shift_ext);
        data
    }
}

/// Read all 3 hardware profiles from the controller (USB only) and return the cache.
pub fn read_from_controller(dev: &mut GipDevice) -> HwProfileCache {
    let mut cache = HwProfileCache::default();
    for i in 0..3 {
        if let Some(mapping) = profile::read_mapping(dev, i, 0) {
            let mut paddle_region = [0u8; 11];
            if mapping.raw.len() >= 28 {
                let src = &mapping.raw[17..28];
                paddle_region[..src.len()].copy_from_slice(src);
            }
            // Read curves page for stick inversion
            let stick_inversion = if let Some(curves) = profile::read_curves(dev, i, 0) {
                if curves.raw.len() > 27 { curves.raw[27] } else { 0 }
            } else { 0 };

            // Also read SlotB (shift page) for shift remaps
            let (shift_p, shift_f, shift_e) = if let Some(shift) = profile::read_mapping(dev, i, 1) {
                (shift.paddles, shift.face, shift.ext)
            } else {
                (DEFAULT_FACE, DEFAULT_FACE, DEFAULT_EXT)
            };
            cache.profiles[i] = HwProfile {
                paddles: mapping.paddles,
                face: mapping.face,
                ext: mapping.ext,
                shift_paddles: shift_p,
                shift_face: shift_f,
                shift_ext: shift_e,
                reserved: paddle_region,
                deadzones: mapping.deadzones,
                color: mapping.color,
                brightness: mapping.brightness,
                stick_inversion,
                vibration: mapping.vibration,
            };
        }
    }
    cache
}

