use std::fs;
use std::path::{Path, PathBuf};

pub struct ControllerInfo {
    pub connected: bool,
    pub is_usb: bool,
    pub name: String,
    pub hw_profile: u8,
    pub event_path: Option<PathBuf>,
    pub sysfs_path: Option<PathBuf>,
}

/// Cheap poll: read just the hw_profile sysfs file from a previously-detected path.
/// Returns `None` if the device is no longer bound (caller should re-detect).
pub fn poll_hw_profile(sysfs_path: &Path) -> Option<u8> {
    let file = sysfs_path.join("hw_profile");
    let data = fs::read_to_string(&file).ok()?;
    data.trim().parse::<u8>().ok()
}

pub fn detect_controller() -> ControllerInfo {
    let hid_driver = Path::new("/sys/bus/hid/drivers/xbelite2");
    let usb_driver = Path::new("/sys/bus/usb/drivers/xbelite2");

    if !hid_driver.exists() && !usb_driver.exists() {
        return ControllerInfo {
            connected: false,
            is_usb: false,
            name: "xbelite2 module not loaded".into(),
            hw_profile: 0,
            event_path: None,
            sysfs_path: None,
        };
    }

    if let Ok(entries) = fs::read_dir(usb_driver) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.contains(':') {
                continue;
            }
            let iface_path = entry.path();
            if !iface_path.is_dir() {
                continue;
            }
            let event_path = find_event_device(&iface_path);
            let hw_profile = read_hw_profile(&iface_path);
            return ControllerInfo {
                connected: true,
                is_usb: true,
                name: "Xbox Elite Wireless Controller Series 2".into(),
                hw_profile,
                event_path,
                sysfs_path: Some(iface_path),
            };
        }
    }

    if let Ok(entries) = fs::read_dir(hid_driver) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("0005:045E:") {
                let device_path = entry.path();
                let event_path = find_event_device(&device_path);
                let hw_profile = read_hw_profile(&device_path);
                return ControllerInfo {
                    connected: true,
                    is_usb: false,
                    name: "Xbox Elite Wireless Controller Series 2".into(),
                    hw_profile,
                    event_path,
                    sysfs_path: Some(device_path),
                };
            }
        }
    }

    disconnected()
}

fn disconnected() -> ControllerInfo {
    ControllerInfo {
        connected: false,
        is_usb: false,
        name: "No controller found".into(),
        hw_profile: 0,
        event_path: None,
        sysfs_path: None,
    }
}

fn find_event_device(device_path: &Path) -> Option<PathBuf> {
    let input_dir = device_path.join("input");
    if !input_dir.exists() {
        return None;
    }

    for entry in fs::read_dir(&input_dir).ok()?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        
        if name_str.starts_with("input") {
            let input_path = entry.path();
            
            for event_entry in fs::read_dir(&input_path).ok()?.flatten() {
                let event_name = event_entry.file_name();
                if event_name.to_string_lossy().starts_with("event") {
                    return Some(PathBuf::from("/dev/input").join(event_name));
                }
            }
        }
    }

    None
}

fn read_hw_profile(device_path: &Path) -> u8 {
    fs::read_to_string(device_path.join("hw_profile"))
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .unwrap_or(0)
}
