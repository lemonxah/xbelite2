//! Profile configuration persistence and IPC.
//!
//! Profiles are stored as JSON files in ~/.config/xbelite2/profiles/.
//! A Unix domain socket at /run/xbelite2.sock provides IPC for
//! a GUI configuration app to communicate with the running daemon.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::types::{DeviceConfig, Profile};

/// Get the configuration directory path.
pub fn config_dir() -> PathBuf {
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
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("xbelite2.sock")
    } else {
        PathBuf::from("/run/xbelite2.sock")
    }
}

/// Load device configuration from disk.
pub fn load_config(device_id: &str) -> Result<DeviceConfig> {
    let path = config_dir().join(format!("{device_id}.json"));
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

/// Save device configuration to disk.
pub fn save_config(device_id: &str, config: &DeviceConfig) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir).context("create config dir")?;
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
    /// Get the full config for a device
    GetConfig { device_id: String },
    /// Set the full config for a device
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
}
