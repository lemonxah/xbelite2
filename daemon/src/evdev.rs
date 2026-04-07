//! evdev input device reading.
//!
//! For USB connections, the kernel xpad driver handles GIP and creates
//! an evdev device. We read from that directly instead of hidraw.
//! This also works for BT when xpadneo creates the evdev device.

use std::fs::{self, File, OpenOptions};
use std::mem;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Linux input_event struct (matches kernel ABI)
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct InputEvent {
    pub tv_sec: i64,
    pub tv_usec: i64,
    pub ev_type: u16,
    pub code: u16,
    pub value: i32,
}

const INPUT_EVENT_SIZE: usize = mem::size_of::<InputEvent>();

/// EVIOCGRAB ioctl
nix::ioctl_write_int!(eviocgrab, b'E', 0x90);

/// EVIOCGNAME ioctl - get device name
nix::ioctl_read_buf!(eviocgname, b'E', 0x06, u8);

/// EVIOCGID ioctl - get device ID
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}
nix::ioctl_read!(eviocgid, b'E', 0x02, InputId);

/// EVIOCGBIT ioctl - get event type bits
nix::ioctl_read_buf!(eviocgbit_ev, b'E', 0x20, u8); // EV bits (type 0)
nix::ioctl_read_buf!(eviocgbit_key, b'E', 0x21, u8); // KEY bits (type 1)
nix::ioctl_read_buf!(eviocgbit_abs, b'E', 0x23, u8); // ABS bits (type 3)

/// EVIOCGABS ioctl - get axis info
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

/// A discovered Xbox Elite 2 evdev device.
#[derive(Debug, Clone)]
pub struct EvdevDevice {
    pub path: PathBuf,
    pub name: String,
    pub id: InputId,
}

/// Discover Xbox Elite 2 controllers as evdev devices.
pub fn discover_devices() -> Result<Vec<EvdevDevice>> {
    let mut devices = Vec::new();

    let input_dir = Path::new("/dev/input");
    let entries = fs::read_dir(input_dir).context("read /dev/input")?;

    for entry in entries.flatten() {
        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !fname.starts_with("event") {
            continue;
        }

        match probe_evdev(&path) {
            Ok(Some(dev)) => {
                log::debug!("Found Elite 2 evdev: {} at {}", dev.name, path.display());
                devices.push(dev);
            }
            Ok(None) => {} // Not an Elite 2
            Err(e) => {
                log::debug!("Could not probe {}: {e}", path.display());
            }
        }
    }

    Ok(devices)
}

/// Check if an evdev device is an Xbox Elite Series 2 **gamepad** (not sub-devices).
fn probe_evdev(path: &Path) -> Result<Option<EvdevDevice>> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    let fd = file.as_raw_fd();

    let mut id = InputId::default();
    unsafe { eviocgid(fd, &mut id).context("EVIOCGID")? };

    // Must be Microsoft vendor
    if id.vendor != 0x045E {
        return Ok(None);
    }

    // Get the device name
    let mut name_buf = [0u8; 256];
    let name = match unsafe { eviocgname(fd, &mut name_buf) } {
        Ok(len) => String::from_utf8_lossy(&name_buf[..(len as usize).min(255)])
            .trim_end_matches('\0')
            .to_string(),
        Err(_) => return Ok(None),
    };

    let name_lower = name.to_lowercase();

    // Skip sub-devices: Mouse, Keyboard, Consumer Control
    if name_lower.contains("mouse")
        || name_lower.contains("keyboard")
        || name_lower.contains("consumer")
    {
        return Ok(None);
    }

    // Skip our own virtual gamepad
    if name_lower.contains("xbelite2") {
        return Ok(None);
    }

    // Match by PID: 0x0B00 (USB), 0x0B05 (BT Classic), 0x0B22 (BLE)
    // For 0x028E (spoofed BT PID), also require "elite" in name to avoid
    // matching regular Xbox controllers
    let is_elite2 = matches!(id.product, 0x0B00 | 0x0B05 | 0x0B22)
        || (id.product == 0x028E && name_lower.contains("elite"));

    // Also match by name if PID didn't match (xpad names it "Elite 2 pad")
    if !is_elite2 && !name_lower.contains("elite") {
        return Ok(None);
    }

    // Verify it's actually a gamepad by checking for ABS_X capability
    let mut abs_bits = [0u8; 8];
    if unsafe { eviocgbit_abs(fd, &mut abs_bits).is_ok() } {
        // ABS_X = bit 0 in byte 0
        if abs_bits[0] & 0x01 == 0 {
            return Ok(None); // No ABS_X = not a gamepad
        }
    } else {
        return Ok(None);
    }

    Ok(Some(EvdevDevice {
        path: path.to_path_buf(),
        name,
        id,
    }))
}

/// An opened evdev device for reading input events.
pub struct EvdevReader {
    file: File,
    pub info: EvdevDevice,
    grabbed: bool,
}

impl EvdevReader {
    /// Open an evdev device for reading.
    pub fn open(device: EvdevDevice) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .open(&device.path)
            .with_context(|| format!("open {}", device.path.display()))?;

        Ok(Self {
            file,
            info: device,
            grabbed: false,
        })
    }

    /// Exclusively grab the device so no other process receives its events.
    pub fn grab(&mut self) -> Result<()> {
        unsafe {
            eviocgrab(self.file.as_raw_fd(), 1)
                .with_context(|| format!("EVIOCGRAB {}", self.info.path.display()))?;
        }
        self.grabbed = true;
        log::info!("Grabbed {}", self.info.path.display());
        Ok(())
    }

    /// Read a single input event. Blocks until available.
    pub fn read_event(&mut self) -> Result<InputEvent> {
        let mut event = InputEvent::default();
        let buf = unsafe {
            std::slice::from_raw_parts_mut(
                &mut event as *mut InputEvent as *mut u8,
                INPUT_EVENT_SIZE,
            )
        };
        use std::io::Read;
        self.file.read_exact(buf).context("read evdev event")?;
        Ok(event)
    }

    /// Get the raw fd for poll().
    pub fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    /// Get axis info for a specific axis.
    pub fn get_abs_info(&self, axis: u16) -> Result<AbsInfo> {
        let mut info = AbsInfo::default();
        // EVIOCGABS(axis) = _IOR('E', 0x40 + axis, struct input_absinfo)
        let request = nix::request_code_read!(b'E', 0x40 + axis as u16, mem::size_of::<AbsInfo>());
        let ret = unsafe { libc::ioctl(self.file.as_raw_fd(), request as _, &mut info as *mut _) };
        if ret < 0 {
            anyhow::bail!("EVIOCGABS({axis}) failed: {}", std::io::Error::last_os_error());
        }
        Ok(info)
    }

    /// Dump device capabilities for debugging.
    pub fn dump_capabilities(&self) -> Result<()> {
        println!("Device: {} ({})", self.info.name, self.info.path.display());
        println!(
            "  ID: bus={:#06x} vendor={:#06x} product={:#06x} version={:#06x}",
            self.info.id.bustype, self.info.id.vendor, self.info.id.product, self.info.id.version
        );

        // Get supported event types
        let mut ev_bits = [0u8; 4];
        unsafe { eviocgbit_ev(self.file.as_raw_fd(), &mut ev_bits).ok() };
        println!("  Event types: {ev_bits:02x?}");

        // Get supported keys
        let mut key_bits = [0u8; 96]; // Up to 768 bits
        if unsafe { eviocgbit_key(self.file.as_raw_fd(), &mut key_bits).is_ok() } {
            print!("  Buttons:");
            for byte_idx in 0..key_bits.len() {
                for bit_idx in 0..8 {
                    if key_bits[byte_idx] & (1 << bit_idx) != 0 {
                        let code = byte_idx * 8 + bit_idx;
                        print!(" {:#06x}", code);
                    }
                }
            }
            println!();
        }

        // Get supported axes
        let mut abs_bits = [0u8; 8]; // Up to 64 bits
        if unsafe { eviocgbit_abs(self.file.as_raw_fd(), &mut abs_bits).is_ok() } {
            println!("  Axes:");
            for byte_idx in 0..abs_bits.len() {
                for bit_idx in 0..8 {
                    if abs_bits[byte_idx] & (1 << bit_idx) != 0 {
                        let axis = (byte_idx * 8 + bit_idx) as u16;
                        if let Ok(info) = self.get_abs_info(axis) {
                            println!(
                                "    ABS {:#04x}: min={} max={} fuzz={} flat={}",
                                axis, info.minimum, info.maximum, info.fuzz, info.flat
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for EvdevReader {
    fn drop(&mut self) {
        if self.grabbed {
            unsafe {
                let _ = eviocgrab(self.file.as_raw_fd(), 0);
            }
            log::info!("Released grab on {}", self.info.path.display());
        }
    }
}
