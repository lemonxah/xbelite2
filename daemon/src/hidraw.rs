//! hidraw device discovery, opening, and exclusive grab.
//!
//! Scans /dev/hidraw* for Xbox Elite 2 controllers and provides
//! raw HID report reading.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

// ioctl for HIDIOCGRAWINFO
#[repr(C)]
#[derive(Default)]
struct HidrawDevinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

// HIDIOCGRAWINFO = _IOR('H', 0x03, struct hidraw_devinfo)
nix::ioctl_read!(hidiocgrawinfo, b'H', 0x03, HidrawDevinfo);

// HIDIOCGRAWNAME(len) = _IOC(_IOC_READ, 'H', 0x04, len)
nix::ioctl_read_buf!(hidiocgrawname, b'H', 0x04, u8);

// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
// Used to send feature reports / init commands
nix::ioctl_read_buf!(hidiocsfeature, b'H', 0x06, u8);

/// Grab exclusive access to the hidraw device.
/// EVIOCGRAB equivalent for hidraw doesn't exist natively,
/// but we can grab the underlying evdev device if needed.
/// For now, we rely on blacklisting xpadneo for these PIDs.

/// Information about a discovered Elite 2 controller.
#[derive(Debug, Clone)]
pub struct HidrawDevice {
    pub path: PathBuf,
    pub vendor: u16,
    pub product: u16,
    pub name: String,
}

/// Scan /dev/hidraw* for Xbox Elite Series 2 controllers.
pub fn discover_devices() -> Result<Vec<HidrawDevice>> {
    let mut devices = Vec::new();

    let entries = fs::read_dir("/dev").context("read /dev")?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("hidraw") {
            continue;
        }

        if let Ok(dev) = probe_hidraw(&path) {
            // Match by specific Elite 2 PIDs, or by 0x028E (BT spoofed) + BT bus
            let is_ms = dev.vendor == crate::types::VENDOR_MICROSOFT;
            let is_specific_pid = matches!(
                dev.product,
                crate::types::PID_ELITE2_BT_CLASSIC
                    | crate::types::PID_ELITE2_BLE
                    | crate::types::PID_ELITE2_USB
            );
            // 0x028E over BT could be any Xbox controller — we accept it
            // since the daemon will detect Elite 2 from the report format.
            // TODO: distinguish Elite 2 from regular Xbox controllers.
            let is_bt_spoofed = dev.product == crate::types::PID_XBOX360_SPOOFED;
            let name_lower = dev.name.to_lowercase();
            let is_our_device = !name_lower.contains("xbelite2");

            if is_ms && (is_specific_pid || is_bt_spoofed) && is_our_device {
                log::debug!("Found Elite 2 hidraw: {} ({})", dev.name, path.display());
                devices.push(dev);
            }
        }
    }

    Ok(devices)
}

/// Probe a single hidraw device for its vendor/product info.
fn probe_hidraw(path: &Path) -> Result<HidrawDevice> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    let fd = file.as_raw_fd();

    let mut info = HidrawDevinfo::default();
    unsafe { hidiocgrawinfo(fd, &mut info).context("HIDIOCGRAWINFO")? };

    let mut name_buf = [0u8; 256];
    let name = match unsafe { hidiocgrawname(fd, &mut name_buf) } {
        Ok(len) => {
            let len = (len as usize).min(255);
            String::from_utf8_lossy(&name_buf[..len])
                .trim_end_matches('\0')
                .to_string()
        }
        Err(_) => String::from("Unknown"),
    };

    Ok(HidrawDevice {
        path: path.to_path_buf(),
        vendor: info.vendor as u16,
        product: info.product as u16,
        name,
    })
}

/// An opened hidraw device for reading reports.
pub struct HidrawReader {
    file: File,
    pub info: HidrawDevice,
}

impl HidrawReader {
    /// Open a hidraw device for reading.
    pub fn open(device: HidrawDevice) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true) // Needed for sending output/feature reports
            .open(&device.path)
            .with_context(|| format!("open {}", device.path.display()))?;

        Ok(Self {
            file,
            info: device,
        })
    }

    /// Read a single HID report. Blocks until data is available.
    pub fn read_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.file.read(buf).context("read hidraw")?;
        Ok(n)
    }

    /// Send a raw output report to the controller.
    pub fn write_report(&mut self, data: &[u8]) -> Result<()> {
        use std::io::Write;
        self.file.write_all(data).context("write hidraw")?;
        Ok(())
    }

    /// Get the raw file descriptor for poll/select.
    pub fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }
}
