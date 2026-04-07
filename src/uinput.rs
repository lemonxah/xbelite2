//! Virtual gamepad creation via Linux uinput.
//!
//! Creates a virtual input device that applications see as a standard
//! Xbox gamepad. We emit transformed events to this device.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem;
use std::os::unix::io::AsRawFd;

use anyhow::{Context, Result};

use crate::transform::OutputEvent;
use crate::types::*;

// uinput ioctl numbers
const UINPUT_IOCTL_BASE: u8 = b'U';

// These are _IOW type ioctls
nix::ioctl_write_int!(ui_set_evbit, UINPUT_IOCTL_BASE, 100);
nix::ioctl_write_int!(ui_set_keybit, UINPUT_IOCTL_BASE, 101);
nix::ioctl_write_int!(ui_set_absbit, UINPUT_IOCTL_BASE, 103);
nix::ioctl_none!(ui_dev_create, UINPUT_IOCTL_BASE, 1);
nix::ioctl_none!(ui_dev_destroy, UINPUT_IOCTL_BASE, 2);

/// Matches the kernel's struct input_event layout
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InputEvent {
    time_sec: u64,  // struct timeval.tv_sec
    time_usec: u64, // struct timeval.tv_usec
    ev_type: u16,
    code: u16,
    value: i32,
}

/// Matches the kernel's struct uinput_user_dev
#[repr(C)]
#[derive(Clone, Copy)]
struct UinputUserDev {
    name: [u8; 80],
    id_bustype: u16,
    id_vendor: u16,
    id_product: u16,
    id_version: u16,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

impl Default for UinputUserDev {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

const EV_SYN: u16 = 0x00;
const SYN_REPORT: u16 = 0x00;

/// A virtual gamepad device created via uinput.
pub struct VirtualGamepad {
    file: File,
}

impl VirtualGamepad {
    /// Create a new virtual gamepad device.
    pub fn new(dev_index: usize) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .context("Failed to open /dev/uinput (need root or uinput group)")?;

        let fd = file.as_raw_fd();

        // Enable event types
        unsafe {
            ui_set_evbit(fd, EV_KEY as _).context("set EV_KEY")?;
            ui_set_evbit(fd, EV_ABS as _).context("set EV_ABS")?;
        }

        // Enable buttons
        let buttons = [
            BTN_A, BTN_B, BTN_X, BTN_Y, BTN_TL, BTN_TR, BTN_SELECT, BTN_START, BTN_MODE,
            BTN_THUMBL, BTN_THUMBR, BTN_GRIPL, BTN_GRIPR, BTN_GRIPL2, BTN_GRIPR2,
        ];
        for btn in buttons {
            unsafe { ui_set_keybit(fd, btn as _).context("set key bit")? };
        }

        // Enable axes
        let axes = [ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT0X, ABS_HAT0Y];
        for axis in axes {
            unsafe { ui_set_absbit(fd, axis as _).context("set abs bit")? };
        }

        // Set up device info
        let mut dev = UinputUserDev::default();
        let name = format!("Xbox Elite 2 (xbelite2 #{dev_index})");
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(79);
        dev.name[..len].copy_from_slice(&name_bytes[..len]);

        // Bus type 0x05 = BUS_BLUETOOTH
        dev.id_bustype = 0x05;
        dev.id_vendor = VENDOR_MICROSOFT;
        dev.id_product = 0xBE12;
        dev.id_version = 1;

        // Stick axes: -32768 to 32767
        for axis in [ABS_X, ABS_Y, ABS_RX, ABS_RY] {
            dev.absmin[axis as usize] = -32768;
            dev.absmax[axis as usize] = 32767;
            dev.absfuzz[axis as usize] = 16;
            dev.absflat[axis as usize] = 128;
        }

        // Trigger axes: 0 to 1023
        for axis in [ABS_Z, ABS_RZ] {
            dev.absmin[axis as usize] = 0;
            dev.absmax[axis as usize] = 1023;
            dev.absfuzz[axis as usize] = 0;
            dev.absflat[axis as usize] = 0;
        }

        // Hat (D-pad): -1 to 1
        for axis in [ABS_HAT0X, ABS_HAT0Y] {
            dev.absmin[axis as usize] = -1;
            dev.absmax[axis as usize] = 1;
        }

        // Write device setup
        let dev_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                &dev as *const UinputUserDev as *const u8,
                mem::size_of::<UinputUserDev>(),
            )
        };
        (&file).write_all(dev_bytes).context("write uinput_user_dev")?;

        // Create the device
        unsafe { ui_dev_create(fd).context("UI_DEV_CREATE")? };

        log::info!("Created virtual gamepad: {name}");
        Ok(Self { file })
    }

    /// Emit a batch of events followed by SYN_REPORT.
    pub fn emit(&mut self, events: &[OutputEvent]) -> Result<()> {
        for ev in events {
            self.write_event(ev.ev_type, ev.code, ev.value)?;
        }
        // SYN_REPORT to flush
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        Ok(())
    }

    fn write_event(&mut self, ev_type: u16, code: u16, value: i32) -> Result<()> {
        let event = InputEvent {
            ev_type,
            code,
            value,
            ..Default::default()
        };
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                &event as *const InputEvent as *const u8,
                mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes).context("write input event")?;
        Ok(())
    }
}

impl Drop for VirtualGamepad {
    fn drop(&mut self) {
        unsafe {
            let _ = ui_dev_destroy(self.file.as_raw_fd());
        }
        log::info!("Destroyed virtual gamepad");
    }
}
