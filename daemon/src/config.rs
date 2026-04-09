use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::types::{DeviceConfig, Profile};

/// Get the user-space configuration directory.
/// Used by the GUI (which runs as the user) — NOT the daemon.
pub fn user_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("xbelite2")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/xbelite2")
    } else {
        PathBuf::from("/etc/xbelite2")
    }
}

/// Get the socket path for IPC.
pub fn socket_path() -> PathBuf {
    PathBuf::from("/run/xbelite2.sock")
}

/// Load device configuration from a given directory.
pub fn load_config_from(dir: &Path, device_id: &str) -> Result<DeviceConfig> {
    let path = dir.join(format!("{device_id}.json"));
    if !path.exists() {
        log::info!("No config found at {}, using defaults", path.display());
        return Ok(DeviceConfig::default());
    }
    let data = fs::read_to_string(&path)
        .with_context(|| format!("read config {}", path.display()))?;
    let config: DeviceConfig =
        serde_json::from_str(&data).with_context(|| format!("parse config {}", path.display()))?;
    log::info!("Loaded config from {}", path.display());
    Ok(config)
}

/// Save device configuration to a given directory.
pub fn save_config_to(dir: &Path, device_id: &str, config: &DeviceConfig) -> Result<()> {
    fs::create_dir_all(dir).context("create config dir")?;
    let path = dir.join(format!("{device_id}.json"));
    let data = serde_json::to_string_pretty(config).context("serialize config")?;
    fs::write(&path, data).with_context(|| format!("write config {}", path.display()))?;
    log::info!("Saved config to {}", path.display());
    Ok(())
}

/// Save a single profile to a standalone file (for import/export).
pub fn save_profile(path: &Path, profile: &Profile) -> Result<()> {
    let data = serde_json::to_string_pretty(profile).context("serialize profile")?;
    fs::write(path, data).with_context(|| format!("write profile {}", path.display()))?;
    Ok(())
}

/// Load a single profile from a standalone file.
pub fn load_profile(path: &Path) -> Result<Profile> {
    let data =
        fs::read_to_string(path).with_context(|| format!("read profile {}", path.display()))?;
    let profile: Profile =
        serde_json::from_str(&data).with_context(|| format!("parse profile {}", path.display()))?;
    Ok(profile)
}

/// IPC message types for communication between daemon and GUI app.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    /// Get current status (connected devices, active profiles)
    GetStatus,
    /// Get the full config for a device (from daemon's in-memory state)
    GetConfig { device_id: String },
    /// Set the full config for a device (GUI sends this after loading/editing)
    SetConfig {
        device_id: String,
        config: DeviceConfig,
    },
    /// Set the active profile override for a device
    SetActiveProfile {
        device_id: String,
        profile_index: Option<usize>,
    },
    /// List all profile files
    ListProfiles,
    /// Test vibration on a specific motor (0=main, 1=weak, 2=lt, 3=rt), intensity 0-100
    TestVibration {
        device_id: String,
        motor: u8,
        intensity: u8,
    },
    /// Test all 4 motors sequentially, 500ms each
    TestAllVibration {
        device_id: String,
        intensities: [u8; 4],
    },
    /// Set hardware profile LED color (USB only)
    SetProfileColor {
        device_id: String,
        r: u8,
        g: u8,
        b: u8,
    },
    /// Set controller device name (USB only)
    SetDeviceName {
        device_id: String,
        name: String,
    },
    /// Remap a button on the current HW profile (USB only)
    SetHwRemap {
        device_id: String,
        src: String,
        normal_dst: String,
        shift_dst: String,
    },
    /// Set stick axis inversion on current hw profile (USB only)
    SetStickInversion {
        device_id: String,
        /// Bitmask: bit0=LY, bit1=RY (from protocol)
        inversion_mask: u8,
    },
    /// Set hardware profile LED brightness (USB only, 0-100)
    SetProfileBrightness {
        device_id: String,
        brightness: u8,
    },
    /// Set a button as the shift modifier (USB only)
    SetShiftButton {
        device_id: String,
        button: String, // button name or "none" to clear
    },
    /// Persist all pending hardware changes to controller flash
    PersistHwChanges {
        device_id: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    Status {
        devices: Vec<DeviceStatus>,
    },
    Config {
        config: DeviceConfig,
    },
    Ok,
    Error {
        message: String,
    },
    ProfileList {
        profiles: Vec<String>,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceStatus {
    pub device_id: String,
    pub name: String,
    pub hw_profile: u8,
    pub active_profile: usize,
    pub connected: bool,
    pub is_usb: bool,
    #[serde(default)]
    pub profile_color: String,  // "#rrggbb" or "default"
    #[serde(default = "default_brightness")]
    pub profile_brightness: u8, // 0-100
    // Live input state
    pub buttons: u16,
    pub paddles: u8,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u16,
    pub right_trigger: u16,
}

fn default_brightness() -> u8 { 100 }
