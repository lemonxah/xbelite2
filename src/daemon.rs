use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};

use xbelite2::config::{self, DeviceStatus, IpcRequest, IpcResponse};
use xbelite2::evdev;
use xbelite2::hidraw;
use xbelite2::transform;
use xbelite2::types::*;
use xbelite2::uinput::VirtualGamepad;

enum InputSource {
    Hidraw {
        file: std::fs::File,
        _grab: Option<std::fs::File>,
    },
    MiscDev {
        file: std::fs::File,
    },
    UsbMiscDev {
        file: std::fs::File,
    },
    Evdev {
        reader: evdev::EvdevReader,
    },
}

struct ControllerState {
    source: InputSource,
    gamepad: VirtualGamepad,
    config: DeviceConfig,
    prev_state: GamepadState,
    device_id: String,
}

impl ControllerState {
    fn fd(&self) -> i32 {
        match &self.source {
            InputSource::Hidraw { file, .. } => file.as_raw_fd(),
            InputSource::MiscDev { file } => file.as_raw_fd(),
            InputSource::UsbMiscDev { file } => file.as_raw_fd(),
            InputSource::Evdev { reader } => reader.fd(),
        }
    }
}

pub fn run(running: Arc<AtomicBool>) -> Result<()> {
    log::info!("xbelite2 daemon starting");

    let mut controllers: HashMap<String, ControllerState> = HashMap::new();

    let sock_path = config::socket_path();
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).context("bind IPC socket")?;
    listener.set_nonblocking(true).context("set socket nonblocking")?;
    // Make socket world-accessible so non-root GUI can connect
    let _ = std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o666));
    log::info!("IPC socket at {}", sock_path.display());

    discover_and_attach(&mut controllers)?;

    if controllers.is_empty() {
        log::warn!("No Xbox Elite 2 controllers found. Waiting...");
    }

    let mut last_scan = std::time::Instant::now();
    let scan_interval = std::time::Duration::from_secs(5);

    while running.load(Ordering::Relaxed) {
        let mut pollfds: Vec<libc::pollfd> = Vec::new();
        let device_ids: Vec<String> = controllers.keys().cloned().collect();

        for id in &device_ids {
            if let Some(ctrl) = controllers.get(id) {
                pollfds.push(libc::pollfd {
                    fd: ctrl.fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
        }

        let _ipc_fd_idx = pollfds.len();
        pollfds.push(libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });

        let ret = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 100) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err).context("poll");
        }

        let mut disconnected = Vec::new();
        for (i, id) in device_ids.iter().enumerate() {
            if pollfds[i].revents & libc::POLLIN != 0 {
                if let Some(ctrl) = controllers.get_mut(id) {
                    if let Err(e) = process_events(ctrl) {
                        log::warn!("Controller {id} error: {e}");
                        disconnected.push(id.clone());
                    }
                }
            }
            if pollfds[i].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                log::info!("Controller {id} disconnected");
                disconnected.push(id.clone());
            }
        }
        for id in disconnected {
            controllers.remove(&id);
        }

        // Handle force-feedback events from uinput (Steam ping, rumble, etc.)
        for ctrl in controllers.values_mut() {
            process_ff_events(ctrl);
        }

        handle_ipc(&listener, &mut controllers);

        if controllers.is_empty() && last_scan.elapsed() >= scan_interval {
            discover_and_attach(&mut controllers)?;
            last_scan = std::time::Instant::now();
        }
    }

    log::info!("xbelite2 daemon shutting down");
    let _ = std::fs::remove_file(&sock_path);
    Ok(())
}

fn discover_and_attach(controllers: &mut HashMap<String, ControllerState>) -> Result<()> {
    // Prefer the kernel module's misc devices (not visible to Steam as gamepads)
    let bt_misc = std::path::Path::new("/dev/xbelite2_bt");
    let bt_key = "misc:/dev/xbelite2_bt".to_string();
    if bt_misc.exists() && !controllers.contains_key(&bt_key) {
        match setup_misc_controller(bt_misc, 0) {
            Ok(state) => {
                log::info!("Attached controller (BT misc): {}", state.device_id);
                controllers.insert(state.device_id.clone(), state);
            }
            Err(e) => log::error!("Failed to set up BT misc controller: {e}"),
        }
    }

    let usb_misc = std::path::Path::new("/dev/xbelite2");
    let usb_key = "usb:/dev/xbelite2".to_string();
    if usb_misc.exists() && !controllers.contains_key(&usb_key) {
        match setup_usb_misc_controller(usb_misc, 1) {
            Ok(state) => {
                log::info!("Attached controller (USB misc): {}", state.device_id);
                controllers.insert(state.device_id.clone(), state);
            }
            Err(e) => log::error!("Failed to set up USB misc controller: {e}"),
        }
    }

    let hidraw_devices = hidraw::discover_devices().unwrap_or_default();
    for (idx, device) in hidraw_devices.into_iter().enumerate() {
        let device_id = format!("hidraw:{}", device.path.display());
        if controllers.contains_key(&device_id) {
            continue;
        }
        match setup_hidraw_controller(device, idx) {
            Ok(state) => {
                log::info!("Attached controller (hidraw): {}", state.device_id);
                controllers.insert(state.device_id.clone(), state);
            }
            Err(e) => log::error!("Failed to set up hidraw controller: {e}"),
        }
    }

    let evdev_devices = evdev::discover_devices().unwrap_or_default();
    for (idx, device) in evdev_devices.into_iter().enumerate() {
        let device_id = format!("evdev:{}", device.path.display());
        if controllers.contains_key(&device_id) {
            continue;
        }
        let already_have = controllers.values().any(|c| {
            matches!(&c.source, InputSource::Hidraw { .. } | InputSource::MiscDev { .. } | InputSource::UsbMiscDev { .. })
        });
        if already_have {
            continue;
        }

        match setup_evdev_controller(device, idx) {
            Ok(state) => {
                log::info!("Attached controller (evdev fallback): {}", state.device_id);
                controllers.insert(state.device_id.clone(), state);
            }
            Err(e) => log::error!("Failed to set up evdev controller: {e}"),
        }
    }

    Ok(())
}

fn setup_hidraw_controller(device: hidraw::HidrawDevice, idx: usize) -> Result<ControllerState> {
    let device_id = format!("hidraw:{}", device.path.display());

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&device.path)
        .with_context(|| format!("open {}", device.path.display()))?;

    let grab = grab_evdev_for_hidraw(&device.path);

    let gamepad = VirtualGamepad::new(idx)?;
    let config = DeviceConfig::default();

    Ok(ControllerState {
        source: InputSource::Hidraw { file, _grab: grab },
        gamepad,
        config,
        prev_state: GamepadState::default(),
        device_id,
    })
}

fn setup_misc_controller(path: &std::path::Path, idx: usize) -> Result<ControllerState> {
    let device_id = format!("misc:{}", path.display());

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    let gamepad = VirtualGamepad::new(idx)?;
    let config = DeviceConfig::default();

    Ok(ControllerState {
        source: InputSource::MiscDev { file },
        gamepad,
        config,
        prev_state: GamepadState::default(),
        device_id,
    })
}

fn setup_usb_misc_controller(path: &std::path::Path, idx: usize) -> Result<ControllerState> {
    let device_id = format!("usb:{}", path.display());

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    let gamepad = VirtualGamepad::new(idx)?;
    let config = DeviceConfig::default();

    Ok(ControllerState {
        source: InputSource::UsbMiscDev { file },
        gamepad,
        config,
        prev_state: GamepadState::default(),
        device_id,
    })
}

fn setup_evdev_controller(device: evdev::EvdevDevice, idx: usize) -> Result<ControllerState> {
    let device_id = format!("evdev:{}", device.path.display());
    let mut reader = evdev::EvdevReader::open(device)?;
    reader.grab()?;

    let gamepad = VirtualGamepad::new(idx + 10)?;
    let config = DeviceConfig::default();

    Ok(ControllerState {
        source: InputSource::Evdev { reader },
        gamepad,
        config,
        prev_state: GamepadState::default(),
        device_id,
    })
}

fn process_events(ctrl: &mut ControllerState) -> Result<()> {
    let mut buf = [0u8; 128];

    match &mut ctrl.source {
        InputSource::Hidraw { file, .. } => {
            let n = file.read(&mut buf).context("read hidraw")?;
            if n == 0 {
                anyhow::bail!("EOF on hidraw");
            }
            if buf[0] != 0x01 || n < 15 {
                return Ok(());
            }
            let current = parse_hidraw_report(&buf[..n]);

            if current.hw_profile == 0 {
                let identity = Profile::default();
                let events = transform::transform(&current, &ctrl.prev_state, &identity);
                if !events.is_empty() {
                    ctrl.gamepad.emit(&events)?;
                }
            } else {
                let sw_idx = (current.hw_profile as usize).saturating_sub(1);
                let profile = ctrl.config.profiles.get(sw_idx)
                    .or_else(|| ctrl.config.profiles.first());
                if let Some(profile) = profile {
                    let events = transform::transform(&current, &ctrl.prev_state, profile);
                    if !events.is_empty() {
                        ctrl.gamepad.emit(&events)?;
                    }
                }
            }
            ctrl.prev_state = current;
        }
        InputSource::MiscDev { file } => {
            // Misc device sends length-prefixed frames: 2 bytes LE length + payload
            let mut len_buf = [0u8; 2];
            file.read_exact(&mut len_buf).context("read misc len")?;
            let frame_len = u16::from_le_bytes(len_buf) as usize;
            if frame_len == 0 || frame_len > buf.len() {
                return Ok(());
            }
            file.read_exact(&mut buf[..frame_len]).context("read misc payload")?;
            if buf[0] != 0x01 || frame_len < 15 {
                return Ok(());
            }
            let current = parse_hidraw_report(&buf[..frame_len]);

            if current.hw_profile == 0 {
                let identity = Profile::default();
                let events = transform::transform(&current, &ctrl.prev_state, &identity);
                if !events.is_empty() {
                    ctrl.gamepad.emit(&events)?;
                }
            } else {
                let sw_idx = (current.hw_profile as usize).saturating_sub(1);
                let profile = ctrl.config.profiles.get(sw_idx)
                    .or_else(|| ctrl.config.profiles.first());
                if let Some(profile) = profile {
                    let events = transform::transform(&current, &ctrl.prev_state, profile);
                    if !events.is_empty() {
                        ctrl.gamepad.emit(&events)?;
                    }
                }
            }
            ctrl.prev_state = current;
        }
        InputSource::UsbMiscDev { file } => {
            // USB misc device sends length-prefixed GIP frames: 2 bytes LE length + payload
            let mut len_buf = [0u8; 2];
            file.read_exact(&mut len_buf).context("read usb misc len")?;
            let frame_len = u16::from_le_bytes(len_buf) as usize;
            if frame_len == 0 || frame_len > buf.len() {
                return Ok(());
            }
            file.read_exact(&mut buf[..frame_len]).context("read usb misc payload")?;

            // Only process GIP input reports (command 0x20)
            if buf[0] != 0x20 || frame_len < 18 {
                return Ok(());
            }
            let current = parse_gip_input(&buf[..frame_len]);

            if current.hw_profile == 0 {
                let identity = Profile::default();
                let events = transform::transform(&current, &ctrl.prev_state, &identity);
                if !events.is_empty() {
                    ctrl.gamepad.emit(&events)?;
                }
            } else {
                let sw_idx = (current.hw_profile as usize).saturating_sub(1);
                let profile = ctrl.config.profiles.get(sw_idx)
                    .or_else(|| ctrl.config.profiles.first());
                if let Some(profile) = profile {
                    let events = transform::transform(&current, &ctrl.prev_state, profile);
                    if !events.is_empty() {
                        ctrl.gamepad.emit(&events)?;
                    }
                }
            }
            ctrl.prev_state = current;
        }
        InputSource::Evdev { reader } => {
            let ev = reader.read_event()?;
            if ev.ev_type == 0x00 && ev.code == 0x00 {
                let active_idx = ctrl.config.active_override.unwrap_or(0);
                let profile = ctrl.config.profiles.get(active_idx)
                    .or_else(|| ctrl.config.profiles.first());
                if let Some(profile) = profile {
                    let events = transform::transform(&ctrl.prev_state, &GamepadState::default(), profile);
                    if !events.is_empty() {
                        ctrl.gamepad.emit(&events)?;
                    }
                }
            } else if ev.ev_type != 0x00 {
                apply_evdev_event(&mut ctrl.prev_state, &ev);
            }
        }
    }

    Ok(())
}

/// Handle force-feedback events from uinput (e.g. Steam "Ping" button).
/// Reads EV_FF play events and EV_UINPUT upload/erase requests,
/// then sends rumble commands to the physical controller.
fn process_ff_events(ctrl: &mut ControllerState) {
    const EV_UINPUT: u16 = 0x0101;
    const UI_FF_UPLOAD: u16 = 1;
    const UI_FF_ERASE: u16 = 2;
    const EV_FF: u16 = 0x15;

    // Process all pending events from the uinput fd
    while let Some((ev_type, code, value)) = ctrl.gamepad.read_event() {
        log::debug!("uinput event: type=0x{ev_type:04x} code={code} value={value}");
        match ev_type {
            EV_UINPUT => {
                match code {
                    UI_FF_UPLOAD => {
                        log::info!("FF upload request");
                        let _ = ctrl.gamepad.handle_ff_upload();
                    }
                    UI_FF_ERASE => {
                        log::info!("FF erase request");
                        let _ = ctrl.gamepad.handle_ff_erase();
                    }
                    _ => {}
                }
            }
            EV_FF => {
                log::info!("FF play: code={code} value={value}");
                if value > 0 {
                    match send_rumble_command(ctrl, 0x20, 0x20, 0x40, 0x40, 0x20) {
                        Ok(_) => log::info!("Rumble sent"),
                        Err(e) => log::error!("Rumble failed: {e}"),
                    }
                } else {
                    match send_rumble_command(ctrl, 0, 0, 0, 0, 0) {
                        Ok(_) => log::info!("Rumble stop sent"),
                        Err(e) => log::error!("Rumble stop failed: {e}"),
                    }
                }
            }
            _ => {}
        }
    }
}

/// Send a rumble command using the correct format for the transport.
/// Motor values are 0-100. Duration is in 10ms units (0x20 = ~320ms).
fn send_rumble_command(ctrl: &mut ControllerState, lt: u8, rt: u8, strong: u8, weak: u8, duration: u8) -> Result<()> {
    match &mut ctrl.source {
        InputSource::Hidraw { file, .. } | InputSource::MiscDev { file } => {
            // BT HID output report: [report_id=0x03, motor_mask, LT, RT, strong, weak, duration, delay, repeat]
            let report = [0x03u8, 0x0F, lt, rt, strong, weak, duration, 0x00, 0x00];
            use std::io::Write;
            file.write_all(&report).context("write BT rumble")?;
        }
        InputSource::UsbMiscDev { file } => {
            // GIP rumble: [cmd=0x09, device=0, seq=0, len=9, sub=0, mask=0x0F, LT, RT, strong, weak, duration, delay, repeat]
            let report = [0x09u8, 0x00, 0x00, 0x09, 0x00, 0x0F, lt, rt, strong, weak, duration, 0x00, 0x00];
            use std::io::Write;
            file.write_all(&report).context("write GIP rumble")?;
        }
        InputSource::Evdev { .. } => {}
    }
    Ok(())
}

/// Parse raw HID report from BLE HID descriptor analysis.
///
/// Report ID 0x01, 20 bytes total:
///   Bytes 1-2:   Left Stick X  (UNSIGNED u16, 0-65535, center=32768)
///   Bytes 3-4:   Left Stick Y  (UNSIGNED u16, 0-65535, center=32768)
///   Bytes 5-6:   Right Stick X (UNSIGNED u16, 0-65535, center=32768)
///   Bytes 7-8:   Right Stick Y (UNSIGNED u16, 0-65535, center=32768)
///   Bytes 9-10:  Left Trigger  (10-bit + 6 padding, 0-1023)
///   Bytes 11-12: Right Trigger (10-bit + 6 padding, 0-1023)
///   Byte  13:    Hat switch (4 bits, 1-8=directions, 0=centered) + 4 padding
///   Bytes 14-15: 15 buttons (Usage 1-15, some unused) + 1 padding
///   Byte  16:    Share button (Consumer 0x00B2, 1 bit) + 7 padding
///   Byte  17:    Profile number (consumer usage 0x0085)
///   Byte  18:    Trigger mode (consumer usage 0x0099, 4 bits + 4 padding)
///   Byte  19:    Paddles (consumer usage 0x0081, 4 bits + 4 padding)
fn parse_hidraw_report(data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();

    if data.len() < 16 {
        return state;
    }

    // Sticks: UNSIGNED u16 (0-65535), convert to signed (-32768..32767)
    let lsx = u16::from_le_bytes([data[1], data[2]]);
    let lsy = u16::from_le_bytes([data[3], data[4]]);
    let rsx = u16::from_le_bytes([data[5], data[6]]);
    let rsy = u16::from_le_bytes([data[7], data[8]]);
    state.left_stick_x = (lsx as i32 - 32768) as i16;
    state.left_stick_y = (lsy as i32 - 32768) as i16;
    state.right_stick_x = (rsx as i32 - 32768) as i16;
    state.right_stick_y = (rsy as i32 - 32768) as i16;

    // Triggers: 10-bit (0-1023) packed in 16 bits with 6 padding
    state.left_trigger = u16::from_le_bytes([data[9], data[10]]) & 0x03FF;
    state.right_trigger = u16::from_le_bytes([data[11], data[12]]) & 0x03FF;

    // Hat switch (d-pad): byte 13, lower 4 bits
    // 0=centered, 1=N, 2=NE, 3=E, 4=SE, 5=S, 6=SW, 7=W, 8=NW
    let hat = data[13] & 0x0F;
    match hat {
        1 => state.dpad_up = true,
        2 => { state.dpad_up = true; state.dpad_right = true; }
        3 => state.dpad_right = true,
        4 => { state.dpad_down = true; state.dpad_right = true; }
        5 => state.dpad_down = true,
        6 => { state.dpad_down = true; state.dpad_left = true; }
        7 => state.dpad_left = true,
        8 => { state.dpad_up = true; state.dpad_left = true; }
        _ => {} // 0 = centered
    }

    // Buttons: bytes 14-15, 15 bits (HID Usage 1-15, some unused)
    // Decoded from BLE HID report descriptor (PID 0x0B22):
    //   Bit 0:  A          Bit 8:  (unused)
    //   Bit 1:  B          Bit 9:  (unused)
    //   Bit 2:  (unused)   Bit 10: View/Back
    //   Bit 3:  X          Bit 11: Menu/Start
    //   Bit 4:  Y          Bit 12: Xbox/Guide
    //   Bit 5:  (unused)   Bit 13: L Stick click
    //   Bit 6:  LB         Bit 14: R Stick click
    //   Bit 7:  RB         Bit 15: (padding)
    let btns = u16::from_le_bytes([data[14], data[15]]);
    state.btn_a      = btns & (1 << 0) != 0;
    state.btn_b      = btns & (1 << 1) != 0;
    state.btn_x      = btns & (1 << 3) != 0;
    state.btn_y      = btns & (1 << 4) != 0;
    state.btn_lb     = btns & (1 << 6) != 0;
    state.btn_rb     = btns & (1 << 7) != 0;
    state.btn_view   = btns & (1 << 10) != 0;
    state.btn_menu   = btns & (1 << 11) != 0;
    state.btn_xbox   = btns & (1 << 12) != 0;
    state.btn_lstick = btns & (1 << 13) != 0;
    state.btn_rstick = btns & (1 << 14) != 0;

    // Profile: byte 17
    if data.len() > 17 {
        state.hw_profile = data[17] & 0x03;
    }

    // Paddles: byte 19
    if data.len() > 19 {
        let paddles = data[19] & 0x0F;
        state.paddle_ur = paddles & 0x01 != 0;
        state.paddle_lr = paddles & 0x02 != 0;
        state.paddle_ul = paddles & 0x04 != 0;
        state.paddle_ll = paddles & 0x08 != 0;
    }

    state
}

/// Parse GIP (Game Input Protocol) input report from USB.
///
/// GIP header (4 bytes): [command=0x20, flags, sequence, length]
/// Payload (14+ bytes):
///   Byte 4:      Buttons 1 (Menu bit2, View bit3, A bit4, B bit5, X bit6, Y bit7)
///   Byte 5:      Buttons 2 (LB bit0, RB bit1, DUp bit2, DDown bit3, DLeft bit4, DRight bit5, LS bit6, RS bit7)
///   Bytes 6-7:   Left Trigger  (u16 LE, 0-1023)
///   Bytes 8-9:   Right Trigger (u16 LE, 0-1023)
///   Bytes 10-11: Left Stick X  (i16 LE, -32768..32767)
///   Bytes 12-13: Left Stick Y  (i16 LE, -32768..32767)
///   Bytes 14-15: Right Stick X (i16 LE, -32768..32767)
///   Bytes 16-17: Right Stick Y (i16 LE, -32768..32767)
///   Byte 18+:    Elite extended data (profile, paddles, etc.)
fn parse_gip_input(data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();

    if data.len() < 18 {
        return state;
    }

    // Buttons byte 1 (data[4])
    let b1 = data[4];
    state.btn_menu   = b1 & (1 << 2) != 0;
    state.btn_view   = b1 & (1 << 3) != 0;
    state.btn_a      = b1 & (1 << 4) != 0;
    state.btn_b      = b1 & (1 << 5) != 0;
    state.btn_x      = b1 & (1 << 6) != 0;
    state.btn_y      = b1 & (1 << 7) != 0;

    // Buttons byte 2 (data[5])
    let b2 = data[5];
    state.btn_lb     = b2 & (1 << 0) != 0;
    state.btn_rb     = b2 & (1 << 1) != 0;
    state.dpad_up    = b2 & (1 << 2) != 0;
    state.dpad_down  = b2 & (1 << 3) != 0;
    state.dpad_left  = b2 & (1 << 4) != 0;
    state.dpad_right = b2 & (1 << 5) != 0;
    state.btn_lstick = b2 & (1 << 6) != 0;
    state.btn_rstick = b2 & (1 << 7) != 0;

    // Triggers (u16 LE, 0-1023)
    state.left_trigger  = u16::from_le_bytes([data[6], data[7]]);
    state.right_trigger = u16::from_le_bytes([data[8], data[9]]);

    // Sticks (i16 LE)
    state.left_stick_x  = i16::from_le_bytes([data[10], data[11]]);
    state.left_stick_y  = i16::from_le_bytes([data[12], data[13]]);
    state.right_stick_x = i16::from_le_bytes([data[14], data[15]]);
    state.right_stick_y = i16::from_le_bytes([data[16], data[17]]);

    // Xbox/Guide button — GIP sends this as a separate command (0x07),
    // but some firmware versions include it in data[4] bit 0
    state.btn_xbox = b1 & (1 << 0) != 0;

    // Elite 2 extended data (if present after the standard 18 bytes)
    if data.len() > 18 {
        // Profile byte at offset 18 (from 0x4D init enabling extended reports)
        state.hw_profile = data[18] & 0x03;
    }

    // Paddles — Elite 2 reports these in the extended data
    if data.len() > 22 {
        let paddles = data[22] & 0x0F;
        state.paddle_ur = paddles & 0x01 != 0;
        state.paddle_lr = paddles & 0x02 != 0;
        state.paddle_ul = paddles & 0x04 != 0;
        state.paddle_ll = paddles & 0x08 != 0;
    }

    state
}

fn apply_evdev_event(state: &mut GamepadState, ev: &evdev::InputEvent) {
    match ev.ev_type {
        0x01 => {
            let pressed = ev.value != 0;
            match ev.code {
                BTN_A => state.btn_a = pressed,
                BTN_B => state.btn_b = pressed,
                BTN_X => state.btn_x = pressed,
                BTN_Y => state.btn_y = pressed,
                BTN_TL => state.btn_lb = pressed,
                BTN_TR => state.btn_rb = pressed,
                BTN_SELECT => state.btn_view = pressed,
                BTN_START => state.btn_menu = pressed,
                BTN_MODE => state.btn_xbox = pressed,
                BTN_THUMBL => state.btn_lstick = pressed,
                BTN_THUMBR => state.btn_rstick = pressed,
                BTN_GRIPL => state.paddle_ul = pressed,
                BTN_GRIPR => state.paddle_ur = pressed,
                BTN_GRIPL2 => state.paddle_ll = pressed,
                BTN_GRIPR2 => state.paddle_lr = pressed,
                _ => {}
            }
        }
        0x03 => match ev.code {
            ABS_X => state.left_stick_x = ev.value as i16,
            ABS_Y => state.left_stick_y = ev.value as i16,
            ABS_RX => state.right_stick_x = ev.value as i16,
            ABS_RY => state.right_stick_y = ev.value as i16,
            ABS_Z => state.left_trigger = ev.value as u16,
            ABS_RZ => state.right_trigger = ev.value as u16,
            ABS_HAT0X => {
                state.dpad_left = ev.value < 0;
                state.dpad_right = ev.value > 0;
            }
            ABS_HAT0Y => {
                state.dpad_up = ev.value < 0;
                state.dpad_down = ev.value > 0;
            }
            _ => {}
        },
        _ => {}
    }
}

fn grab_evdev_for_hidraw(hidraw_path: &std::path::Path) -> Option<std::fs::File> {
    let hidraw_name = hidraw_path.file_name()?.to_str()?;
    let sysfs_input = format!("/sys/class/hidraw/{hidraw_name}/device/input");
    let input_dir = std::fs::read_dir(&sysfs_input).ok()?;

    for entry in input_dir.flatten() {
        let input_name = entry.file_name();
        if !input_name.to_str()?.starts_with("input") {
            continue;
        }
        for sub in std::fs::read_dir(entry.path()).ok()?.flatten() {
            let sub_str = sub.file_name().to_str()?.to_string();
            if sub_str.starts_with("event") {
                let evdev_path = format!("/dev/input/{sub_str}");
                let file = OpenOptions::new().read(true).open(&evdev_path).ok()?;
                nix::ioctl_write_int!(eviocgrab, b'E', 0x90);
                if unsafe { eviocgrab(file.as_raw_fd(), 1) }.is_ok() {
                    log::info!("Grabbed evdev: {evdev_path}");
                    return Some(file);
                }
            }
        }
    }
    None
}

static mut IPC_CLIENTS: Vec<std::os::unix::net::UnixStream> = Vec::new();

fn handle_ipc(listener: &UnixListener, controllers: &mut HashMap<String, ControllerState>) {
    while let Ok((stream, _)) = listener.accept() {
        stream.set_nonblocking(true).ok();
        unsafe { IPC_CLIENTS.push(stream); }
    }

    let clients = unsafe { &mut IPC_CLIENTS };
    let mut to_remove = Vec::new();

    for (i, stream) in clients.iter_mut().enumerate() {
        let mut buf = [0u8; 4096];
        let mut pos = 0;
        loop {
            match std::io::Read::read(stream, &mut buf[pos..pos+1]) {
                Ok(0) => { to_remove.push(i); break; }
                Ok(_) => {
                    if buf[pos] == b'\n' {
                        if let Ok(line) = std::str::from_utf8(&buf[..pos]) {
                            let response = match serde_json::from_str::<IpcRequest>(line) {
                                Ok(req) => handle_ipc_request(req, controllers),
                                Err(e) => IpcResponse::Error { message: format!("Invalid: {e}") },
                            };
                            let _ = writeln!(stream, "{}", serde_json::to_string(&response).unwrap());
                        }
                        pos = 0;
                        continue;
                    }
                    pos += 1;
                    if pos >= 4095 { pos = 0; }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => { to_remove.push(i); break; }
            }
        }
    }

    to_remove.sort();
    to_remove.dedup();
    for i in to_remove.into_iter().rev() {
        clients.remove(i);
    }
}

fn handle_ipc_request(
    request: IpcRequest,
    controllers: &mut HashMap<String, ControllerState>,
) -> IpcResponse {
    match request {
        IpcRequest::GetStatus => {
            let devices: Vec<DeviceStatus> = controllers
                .values()
                .map(|ctrl| {
                    let s = &ctrl.prev_state;
                    let mut buttons: u16 = 0;
                    if s.btn_a { buttons |= 1 << 0; }
                    if s.btn_b { buttons |= 1 << 1; }
                    if s.btn_x { buttons |= 1 << 2; }
                    if s.btn_y { buttons |= 1 << 3; }
                    if s.btn_lb { buttons |= 1 << 4; }
                    if s.btn_rb { buttons |= 1 << 5; }
                    if s.btn_view { buttons |= 1 << 6; }
                    if s.btn_menu { buttons |= 1 << 7; }
                    if s.btn_xbox { buttons |= 1 << 8; }
                    if s.btn_lstick { buttons |= 1 << 9; }
                    if s.btn_rstick { buttons |= 1 << 10; }
                    if s.dpad_up { buttons |= 1 << 11; }
                    if s.dpad_down { buttons |= 1 << 12; }
                    if s.dpad_left { buttons |= 1 << 13; }
                    if s.dpad_right { buttons |= 1 << 14; }
                    let mut paddles: u8 = 0;
                    if s.paddle_ur { paddles |= 0x01; }
                    if s.paddle_lr { paddles |= 0x02; }
                    if s.paddle_ul { paddles |= 0x04; }
                    if s.paddle_ll { paddles |= 0x08; }
                    DeviceStatus {
                        device_id: ctrl.device_id.clone(),
                        name: match &ctrl.source {
                            InputSource::Hidraw { .. } => "Elite 2 (hidraw)".to_string(),
                            InputSource::MiscDev { .. } => "Elite 2 (BT)".to_string(),
                            InputSource::UsbMiscDev { .. } => "Elite 2 (USB)".to_string(),
                            InputSource::Evdev { reader } => reader.info.name.clone(),
                        },
                        hw_profile: s.hw_profile,
                        active_profile: ctrl.config.active_override.unwrap_or(0),
                        connected: true,
                        buttons,
                        paddles,
                        left_stick_x: s.left_stick_x,
                        left_stick_y: s.left_stick_y,
                        right_stick_x: s.right_stick_x,
                        right_stick_y: s.right_stick_y,
                        left_trigger: s.left_trigger,
                        right_trigger: s.right_trigger,
                    }
                })
                .collect();
            IpcResponse::Status { devices }
        }
        IpcRequest::GetConfig { device_id } => {
            if let Some(ctrl) = controllers.get(&device_id) {
                IpcResponse::Config { config: ctrl.config.clone() }
            } else {
                IpcResponse::Error { message: format!("Device {device_id} not found") }
            }
        }
        IpcRequest::SetConfig { device_id, config } => {
            if let Some(ctrl) = controllers.get_mut(&device_id) {
                ctrl.config = config;
                log::info!("Config updated via IPC for {device_id}");
                IpcResponse::Ok
            } else {
                IpcResponse::Error { message: format!("Device {device_id} not found") }
            }
        }
        IpcRequest::SetActiveProfile { device_id, profile_index } => {
            if let Some(ctrl) = controllers.get_mut(&device_id) {
                ctrl.config.active_override = profile_index;
                IpcResponse::Ok
            } else {
                IpcResponse::Error { message: format!("Device {device_id} not found") }
            }
        }
        IpcRequest::ListProfiles => {
            let mut profiles = Vec::new();
            for ctrl in controllers.values() {
                for p in &ctrl.config.profiles {
                    if !profiles.contains(&p.name) {
                        profiles.push(p.name.clone());
                    }
                }
            }
            IpcResponse::ProfileList { profiles }
        }
        IpcRequest::TestVibration { device_id, motor, intensity } => {
            if let Some(ctrl) = controllers.get_mut(&device_id) {
                match send_rumble(ctrl, motor, intensity) {
                    Ok(()) => IpcResponse::Ok,
                    Err(e) => IpcResponse::Error { message: format!("Rumble failed: {e}") },
                }
            } else {
                IpcResponse::Error { message: format!("Device {device_id} not found") }
            }
        }
        IpcRequest::TestAllVibration { device_id, intensities } => {
            if let Some(ctrl) = controllers.get_mut(&device_id) {
                let is_gip = matches!(&ctrl.source, InputSource::UsbMiscDev { .. });
                let fd = match &ctrl.source {
                    InputSource::Hidraw { file, .. } => file.as_raw_fd(),
                    InputSource::MiscDev { file } => file.as_raw_fd(),
                    InputSource::UsbMiscDev { file } => file.as_raw_fd(),
                    _ => return IpcResponse::Error { message: "Not supported over evdev".into() },
                };
                std::thread::spawn(move || {
                    for motor in 0..4u8 {
                        let v = intensities[motor as usize].min(100);
                        let (report, stop): (Vec<u8>, Vec<u8>) = if is_gip {
                            let mut r = vec![0x09u8, 0x00, 0x00, 0x09, 0x00, 0x0F, 0, 0, 0, 0, 0x20, 0x00, 0x00];
                            match motor {
                                0 => r[8] = v,   // strong
                                1 => r[9] = v,   // weak
                                2 => r[6] = v,   // LT
                                3 => r[7] = v,   // RT
                                _ => {}
                            }
                            let s = vec![0x09u8, 0x00, 0x00, 0x09, 0x00, 0x0F, 0, 0, 0, 0, 0x00, 0x00, 0x00];
                            (r, s)
                        } else {
                            let mut r = vec![0x03u8, 0x0F, 0, 0, 0, 0, 0x20, 0x00, 0x00];
                            match motor {
                                0 => r[4] = v,   // strong
                                1 => r[5] = v,   // weak
                                2 => r[2] = v,   // LT
                                3 => r[3] = v,   // RT
                                _ => {}
                            }
                            let s = vec![0x03u8, 0x0F, 0, 0, 0, 0, 0x00, 0x00, 0x00];
                            (r, s)
                        };
                        unsafe { libc::write(fd, report.as_ptr() as *const _, report.len()); }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        unsafe { libc::write(fd, stop.as_ptr() as *const _, stop.len()); }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                });
                IpcResponse::Ok
            } else {
                IpcResponse::Error { message: format!("Device {device_id} not found") }
            }
        }
    }
}

fn send_rumble(ctrl: &mut ControllerState, motor: u8, intensity: u8) -> anyhow::Result<()> {
    let v = intensity.min(100);
    let (mut lt, mut rt, mut strong, mut weak) = (0u8, 0u8, 0u8, 0u8);
    match motor {
        0 => strong = v,
        1 => weak = v,
        2 => lt = v,
        3 => rt = v,
        _ => {}
    }
    send_rumble_command(ctrl, lt, rt, strong, weak, 0x20)?;
    // Auto-stop after 500ms
    let fd = match &ctrl.source {
        InputSource::Hidraw { file, .. } => file.as_raw_fd(),
        InputSource::MiscDev { file } => file.as_raw_fd(),
        InputSource::UsbMiscDev { file } => file.as_raw_fd(),
        InputSource::Evdev { .. } => return Ok(()),
    };
    let is_gip = matches!(&ctrl.source, InputSource::UsbMiscDev { .. });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let stop: Vec<u8> = if is_gip {
            vec![0x09u8, 0x00, 0x00, 0x09, 0x00, 0x0F, 0, 0, 0, 0, 0x00, 0x00, 0x00]
        } else {
            vec![0x03u8, 0x0F, 0, 0, 0, 0, 0x00, 0x00, 0x00]
        };
        unsafe { libc::write(fd, stop.as_ptr() as *const _, stop.len()); }
    });
    Ok(())
}
