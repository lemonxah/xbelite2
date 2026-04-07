//! Input device grabbing.
//!
//! When we create a virtual gamepad via uinput, we need to prevent
//! applications from also seeing the raw hidraw events through the
//! kernel's standard input device. We do this by finding the evdev
//! device for the controller and grabbing it with EVIOCGRAB.

use std::fs::{self, File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use anyhow::{Context, Result};

nix::ioctl_write_int!(eviocgrab, b'E', 0x90);

/// An evdev device that has been exclusively grabbed.
pub struct GrabbedDevice {
    _file: File,
    path: PathBuf,
}

impl GrabbedDevice {
    /// Find and grab the evdev device corresponding to a hidraw device.
    ///
    /// We match by looking at the sysfs tree to find the evdev sibling
    /// of the hidraw device.
    pub fn grab_for_hidraw(hidraw_path: &std::path::Path) -> Result<Self> {
        let evdev_path = find_evdev_for_hidraw(hidraw_path)?;
        let file = OpenOptions::new()
            .read(true)
            .open(&evdev_path)
            .with_context(|| format!("open {}", evdev_path.display()))?;

        unsafe {
            eviocgrab(file.as_raw_fd(), 1).with_context(|| {
                format!("EVIOCGRAB {}", evdev_path.display())
            })?;
        }

        log::info!("Grabbed {}", evdev_path.display());
        Ok(Self {
            _file: file,
            path: evdev_path,
        })
    }
}

impl Drop for GrabbedDevice {
    fn drop(&mut self) {
        log::info!("Released grab on {}", self.path.display());
        // EVIOCGRAB(0) is automatic when fd is closed
    }
}

/// Find the /dev/input/eventN device that corresponds to a /dev/hidrawN device.
fn find_evdev_for_hidraw(hidraw_path: &std::path::Path) -> Result<PathBuf> {
    let hidraw_name = hidraw_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("invalid hidraw path")?;

    // Walk sysfs: /sys/class/hidraw/hidrawN/device/input/inputN/eventN
    let sysfs_base = PathBuf::from(format!("/sys/class/hidraw/{hidraw_name}/device"));

    // The input subsystem creates /sys/class/hidraw/hidrawN/device/input/inputN/
    let input_dir = sysfs_base.join("input");
    if !input_dir.exists() {
        anyhow::bail!(
            "No input directory found at {} - device may not have an evdev node",
            input_dir.display()
        );
    }

    for entry in fs::read_dir(&input_dir).context("read input dir")? {
        let entry = entry?;
        let input_name = entry.file_name();
        let input_str = input_name.to_str().unwrap_or("");
        if !input_str.starts_with("input") {
            continue;
        }

        // Look for eventN inside this inputN directory
        let input_path = entry.path();
        for sub_entry in fs::read_dir(&input_path).context("read inputN dir")? {
            let sub_entry = sub_entry?;
            let sub_name = sub_entry.file_name();
            let sub_str = sub_name.to_str().unwrap_or("");
            if sub_str.starts_with("event") {
                let evdev_path = PathBuf::from(format!("/dev/input/{sub_str}"));
                if evdev_path.exists() {
                    return Ok(evdev_path);
                }
            }
        }
    }

    anyhow::bail!("No evdev device found for {}", hidraw_path.display())
}
