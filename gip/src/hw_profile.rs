use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
    /// Per-motor rumble intensity at mapping bytes 28-31: [weak, strong, RT, LT]
    /// (0-100). Setting any to 0 silences that motor while the profile is active.
    #[serde(alias = "deadzones", default = "default_rumble_intensity")]
    pub rumble_intensity: [u8; 4],
    /// Per-axis max-output saturation at mapping bytes [32, 34, 38, 40]:
    /// [LT, LS, RT, RS]. 0xFF = full analog range; lower = output saturates
    /// earlier on the physical travel; 0 = binary output. The Elite 2 firmware
    /// uses these to drive hair-trigger behavior. Serde default is all-0xFF
    /// so cached profiles written before this field existed still deserialize
    /// (and load as "full analog" rather than "binary").
    #[serde(default = "default_saturation")]
    pub saturation: [u8; 4],
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
fn default_saturation() -> [u8; 4] { [0xFF; 4] }
fn default_rumble_intensity() -> [u8; 4] { [100; 4] }

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
        data[OFF_RUMBLE_INTENSITY..OFF_RUMBLE_INTENSITY + 4].copy_from_slice(&self.rumble_intensity);
        // Axis saturation bytes — must be written, otherwise they default to 0
        // and the controller interprets that as "binary output" on triggers.
        // The high byte of each u16 LE field is always 0 in firmware captures.
        data[OFF_SAT_LT] = self.saturation[0];
        data[OFF_SAT_LT + 1] = 0;
        data[OFF_SAT_LS] = self.saturation[1];
        data[OFF_SAT_LS + 1] = 0;
        data[OFF_SAT_RT] = self.saturation[2];
        data[OFF_SAT_RT + 1] = 0;
        data[OFF_SAT_RS] = self.saturation[3];
        data[OFF_SAT_RS + 1] = 0;
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

/// Default on-disk cache path: `$XDG_CACHE_HOME/xbelite2/hw_profiles.json`,
/// falling back to `~/.cache/xbelite2/hw_profiles.json`.
pub fn default_cache_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return Some(PathBuf::from(dir).join("xbelite2").join("hw_profiles.json"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".cache/xbelite2/hw_profiles.json"));
    }
    None
}

/// Save the in-memory cache to disk so it can be shown while the controller
/// is connected over BT (where we can't read profile pages).
pub fn save(cache: &HwProfileCache) -> std::io::Result<()> {
    let path = default_cache_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no HOME/XDG_CACHE_HOME"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}

/// Load the cached profile state from disk. Returns `None` if the file is
/// missing or unparseable.
pub fn load() -> Option<HwProfileCache> {
    let path = default_cache_path()?;
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
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
                rumble_intensity: mapping.rumble_intensity,
                saturation: mapping.saturation,
                color: mapping.color,
                brightness: mapping.brightness,
                stick_inversion,
                vibration: mapping.vibration,
            };
        }
    }
    cache
}

