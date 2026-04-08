use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::transport::GipDevice;
use crate::types::*;
use crate::profile;

/// Cached hardware profile data read from the controller.
/// Stores which buttons/paddles are remapped so the daemon knows
/// not to emit duplicate events when a paddle is hardware-remapped.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwProfileCache {
    pub profiles: [HwProfile; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwProfile {
    /// Face button remap for normal mode (SlotA page): [A, B, X, Y]
    pub remap_a: [u8; 4],
    /// Face shift layer within SlotA page (bytes 5-8)
    pub remap_b: [u8; 4],
    /// Extended remap from SlotA: [LB, RB, LT, RT, DUp, DDown, DLeft, DRight]
    pub remap_ext: [u8; 8],
    /// SlotB face remap (shift mode page): [A, B, X, Y]
    pub shift_remap_a: [u8; 4],
    /// SlotB extended remap (shift mode page)
    pub shift_remap_ext: [u8; 8],
    /// Paddle/reserved region (bytes 17-27, may contain paddle remap assignments)
    pub paddle_region: [u8; 11],
    /// Dead zones: [LStick, RStick, LTrigger, RTrigger]
    pub deadzones: [u8; 4],
    /// Color (None = default white)
    pub color: Option<(u8, u8, u8)>,
    /// LED brightness (0-100, default 100)
    pub brightness: u8,
    /// Vibration: (left, right)
    pub vibration: (u8, u8),
}

impl HwProfile {
    /// Check if a specific extended button slot is remapped from its default.
    /// Extended buttons: index 0=LB, 1=RB, 2=LT, 3=RT, 4=DUp, 5=DDown, 6=DLeft, 7=DRight
    pub fn is_ext_remapped(&self, index: usize) -> bool {
        if index >= 8 { return false; }
        self.remap_ext[index] != DEFAULT_EXT[index]
    }

    /// Check if a face button slot is remapped from its default (in normal mode).
    pub fn is_face_remapped(&self, index: usize) -> bool {
        if index >= 4 { return false; }
        self.remap_a[index] != DEFAULT_FACE[index]
    }

    /// Check if any button has been remapped at all.
    pub fn has_any_remap(&self) -> bool {
        self.remap_a != DEFAULT_FACE
            || self.remap_b != DEFAULT_FACE
            || self.remap_ext != DEFAULT_EXT
    }

    /// Check if paddles have remap data (bytes 17-27 non-zero).
    pub fn has_paddle_remaps(&self) -> bool {
        self.paddle_region.iter().any(|&b| b != 0)
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
            // Also read SlotB (shift page) for shift remaps
            let (shift_a, shift_ext) = if let Some(shift) = profile::read_mapping(dev, i, 1) {
                (shift.remap_a, shift.remap_ext)
            } else {
                ([0x04, 0x05, 0x06, 0x07], [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F])
            };
            cache.profiles[i] = HwProfile {
                remap_a: mapping.remap_a,
                remap_b: mapping.remap_b,
                remap_ext: mapping.remap_ext,
                shift_remap_a: shift_a,
                shift_remap_ext: shift_ext,
                paddle_region,
                deadzones: mapping.deadzones,
                color: mapping.color,
                brightness: mapping.brightness,
                vibration: mapping.vibration,
            };
        }
    }
    cache
}

/// Cache file path for hardware profiles.
fn cache_path() -> PathBuf {
    let dir = if let Ok(d) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(d).join("xbelite2")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache/xbelite2")
    } else {
        PathBuf::from("/var/cache/xbelite2")
    };
    dir.join("hw_profiles.json")
}

/// Save the hardware profile cache to disk.
pub fn save(cache: &HwProfileCache) -> std::io::Result<()> {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}

/// Load the hardware profile cache from disk.
pub fn load() -> Option<HwProfileCache> {
    let path = cache_path();
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Load cache from a specific path (for daemon running as root).
pub fn load_from(dir: &Path) -> Option<HwProfileCache> {
    let path = dir.join("hw_profiles.json");
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Save cache to a specific path.
pub fn save_to(cache: &HwProfileCache, dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join("hw_profiles.json");
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}
