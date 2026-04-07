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
nix::ioctl_write_int!(ui_set_ffbit, UINPUT_IOCTL_BASE, 107);
nix::ioctl_none!(ui_dev_create, UINPUT_IOCTL_BASE, 1);
nix::ioctl_none!(ui_dev_destroy, UINPUT_IOCTL_BASE, 2);

// FF event types
const EV_FF: u16 = 0x15;
const FF_RUMBLE: u16 = 0x50;

// uinput_ff_upload / uinput_ff_erase ioctls
const UI_BEGIN_FF_UPLOAD: u8 = 200;
const UI_END_FF_UPLOAD: u8 = 201;
const UI_BEGIN_FF_ERASE: u8 = 202;
const UI_END_FF_ERASE: u8 = 203;

/// Matches kernel's struct ff_rumble_effect
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct FfRumble {
    pub strong_magnitude: u16,
    pub weak_magnitude: u16,
}

/// Matches kernel's struct ff_effect (simplified for rumble)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfEffect {
    pub effect_type: u16,
    pub id: i16,
    pub direction: u16,
    pub trigger_button: u16,
    pub trigger_interval: u16,
    pub replay_length: u16,
    pub replay_delay: u16,
    // Union: we only care about rumble
    pub u: [u8; 52], // padded union (largest variant)
}

impl Default for FfEffect {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl FfEffect {
    pub fn rumble(&self) -> FfRumble {
        unsafe { std::ptr::read_unaligned(self.u.as_ptr() as *const FfRumble) }
    }
}

/// Matches kernel's struct uinput_ff_upload
#[repr(C)]
#[derive(Clone, Copy)]
struct UinputFfUpload {
    request_id: u32,
    retval: i32,
    effect: FfEffect,
    old: FfEffect,
}

impl Default for UinputFfUpload {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

/// Matches kernel's struct uinput_ff_erase
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct UinputFfErase {
    request_id: u32,
    retval: i32,
    effect_id: u32,
}

// _IOWR('U', 200, struct uinput_ff_upload)
nix::ioctl_readwrite!(ui_begin_ff_upload, UINPUT_IOCTL_BASE, UI_BEGIN_FF_UPLOAD, UinputFfUpload);
nix::ioctl_readwrite!(ui_end_ff_upload, UINPUT_IOCTL_BASE, UI_END_FF_UPLOAD, UinputFfUpload);
nix::ioctl_readwrite!(ui_begin_ff_erase, UINPUT_IOCTL_BASE, UI_BEGIN_FF_ERASE, UinputFfErase);
nix::ioctl_readwrite!(ui_end_ff_erase, UINPUT_IOCTL_BASE, UI_END_FF_ERASE, UinputFfErase);

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
    effects: [Option<FfEffect>; 16],
}

impl VirtualGamepad {
    /// Create a new virtual gamepad device.
    pub fn new(dev_index: usize) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .context("Failed to open /dev/uinput (need root or uinput group)")?;

        let fd = file.as_raw_fd();

        // Enable event types
        unsafe {
            ui_set_evbit(fd, EV_KEY as _).context("set EV_KEY")?;
            ui_set_evbit(fd, EV_ABS as _).context("set EV_ABS")?;
            ui_set_evbit(fd, EV_FF as _).context("set EV_FF")?;
            ui_set_ffbit(fd, FF_RUMBLE as _).context("set FF_RUMBLE")?;
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
        dev.ff_effects_max = 16;

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
        Ok(Self { file, effects: [None; 16] })
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

    /// Get the fd for polling (needed to receive FF upload/erase/play events).
    pub fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    /// Handle a pending FF upload request from the kernel.
    /// Returns Ok(()) even if there's nothing to do.
    pub fn handle_ff_upload(&mut self) -> Result<()> {
        let fd = self.file.as_raw_fd();
        let mut upload = UinputFfUpload::default();
        match unsafe { ui_begin_ff_upload(fd, &mut upload) } {
            Ok(_) => {
                let id = upload.effect.id;
                if id >= 0 && (id as usize) < self.effects.len() {
                    self.effects[id as usize] = Some(upload.effect);
                }
                upload.retval = 0;
                unsafe { ui_end_ff_upload(fd, &mut upload).context("end ff upload")? };
            }
            Err(_) => {}
        }
        Ok(())
    }

    /// Handle a pending FF erase request from the kernel.
    pub fn handle_ff_erase(&mut self) -> Result<()> {
        let fd = self.file.as_raw_fd();
        let mut erase = UinputFfErase::default();
        match unsafe { ui_begin_ff_erase(fd, &mut erase) } {
            Ok(_) => {
                let id = erase.effect_id;
                if (id as usize) < self.effects.len() {
                    self.effects[id as usize] = None;
                }
                erase.retval = 0;
                unsafe { ui_end_ff_erase(fd, &mut erase).context("end ff erase")? };
            }
            Err(_) => {}
        }
        Ok(())
    }

    /// Look up a stored FF effect's rumble magnitudes.
    /// Returns (strong, weak) scaled to 0-100.
    pub fn get_ff_rumble(&self, effect_id: u16) -> Option<(u8, u8)> {
        let effect = self.effects.get(effect_id as usize)?.as_ref()?;
        let rumble = effect.rumble();
        // Scale from u16 (0-65535) to 0-100
        let strong = (rumble.strong_magnitude as u32 * 100 / 65535) as u8;
        let weak = (rumble.weak_magnitude as u32 * 100 / 65535) as u8;
        Some((strong, weak))
    }

    /// Read an input_event from uinput (for EV_FF play/stop and EV_UINPUT upload/erase).
    /// Returns None if no event is available (non-blocking).
    pub fn read_event(&self) -> Option<(u16, u16, i32)> {
        use std::io::Read;
        let mut buf = [0u8; mem::size_of::<InputEvent>()];
        let fd = self.file.as_raw_fd();
        // Peek if data available
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let ret = unsafe { libc::poll(&mut pfd, 1, 0) };
        if ret <= 0 || (pfd.revents & libc::POLLIN) == 0 {
            return None;
        }
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if n != buf.len() as isize {
            return None;
        }
        let event: InputEvent = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const _) };
        Some((event.ev_type, event.code, event.value))
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
