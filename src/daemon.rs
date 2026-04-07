//! Main daemon loop.
//!
//! Unified driver for Xbox Elite Series 2 via hidraw (BT and USB).
//! Falls back to evdev when xpad is still bound (USB only).
//!
//! Reads raw HID reports, parses paddles + profile from confirmed
//! byte offsets, applies profile transforms, emits via uinput.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
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

/// Input source — either hidraw (preferred) or evdev (fallback).
enum InputSource {
    Hidraw {
        file: std::fs::File,
        _grab: Option<std::fs::File>, // Grabbed evdev to prevent duplicates
    },
    Evdev {
        reader: evdev::EvdevReader,
    },
}

/// State for a single connected controller.
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

        let ipc_fd_idx = pollfds.len();
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

        if pollfds[ipc_fd_idx].revents & libc::POLLIN != 0 {
            handle_ipc(&listener, &mut controllers);
        }

        if last_scan.elapsed() >= scan_interval {
            discover_and_attach(&mut controllers)?;
            last_scan = std::time::Instant::now();
        }
    }

    log::info!("xbelite2 daemon shutting down");
    let _ = std::fs::remove_file(&sock_path);
    Ok(())
}

fn discover_and_attach(controllers: &mut HashMap<String, ControllerState>) -> Result<()> {
    // Try hidraw first (works for both BT and USB-via-usbhid)
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

    // Fallback: try evdev (for USB when xpad is still bound)
    let evdev_devices = evdev::discover_devices().unwrap_or_default();
    for (idx, device) in evdev_devices.into_iter().enumerate() {
        let device_id = format!("evdev:{}", device.path.display());
        // Don't add evdev if we already have a hidraw for this controller
        if controllers.contains_key(&device_id) {
            continue;
        }
        // Skip if we already have a hidraw source (same physical device)
        let already_have = controllers.values().any(|c| {
            matches!(&c.source, InputSource::Hidraw { .. })
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

    // Grab the corresponding evdev to prevent duplicate events
    let grab = grab_evdev_for_hidraw(&device.path);

    let gamepad = VirtualGamepad::new(idx)?;
    let config = config::load_config("elite2").unwrap_or_default();

    Ok(ControllerState {
        source: InputSource::Hidraw { file, _grab: grab },
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

    let gamepad = VirtualGamepad::new(idx + 10)?; // Offset to avoid collision
    let config = config::load_config("elite2").unwrap_or_default();

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
            let active_idx = ctrl.config.active_override
                .unwrap_or_else(|| ctrl.config.hw_profile_map[current.hw_profile as usize]);
            let profile = ctrl.config.profiles.get(active_idx)
                .or_else(|| ctrl.config.profiles.first());
            if let Some(profile) = profile {
                let events = transform::transform(&current, &ctrl.prev_state, profile);
                if !events.is_empty() {
                    ctrl.gamepad.emit(&events)?;
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

/// Parse raw HID report (confirmed from hardware capture).
fn parse_hidraw_report(data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();

    let b0 = data[1];
    let b1 = data[2];

    state.btn_a = b0 & 0x01 != 0;
    state.btn_b = b0 & 0x02 != 0;
    state.btn_x = b0 & 0x08 != 0;
    state.btn_y = b0 & 0x10 != 0;
    state.btn_lb = b0 & 0x40 != 0;
    state.btn_rb = b0 & 0x80 != 0;
    state.btn_view = b1 & 0x04 != 0;
    state.btn_menu = b1 & 0x08 != 0;
    state.btn_lstick = b1 & 0x20 != 0;
    state.btn_rstick = b1 & 0x40 != 0;

    // TODO: confirm d-pad encoding from hardware capture
    // For now, d-pad bits might be in byte 2 lower nibble or separate

    if data.len() >= 7 {
        state.left_trigger = u16::from_le_bytes([data[3], data[4]]) & 0x03FF;
        state.right_trigger = u16::from_le_bytes([data[5], data[6]]) & 0x03FF;
    }
    if data.len() >= 15 {
        state.left_stick_x = i16::from_le_bytes([data[7], data[8]]);
        state.left_stick_y = i16::from_le_bytes([data[9], data[10]]);
        state.right_stick_x = i16::from_le_bytes([data[11], data[12]]);
        state.right_stick_y = i16::from_le_bytes([data[13], data[14]]);
    }

    // Byte 17: profile, Byte 19: paddles (confirmed from hardware)
    if data.len() > 17 {
        state.hw_profile = data[17] & 0x03;
    }
    if data.len() > 19 {
        let paddles = data[19];
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

fn handle_ipc(listener: &UnixListener, controllers: &mut HashMap<String, ControllerState>) {
    let (stream, _) = match listener.accept() {
        Ok(s) => s,
        Err(_) => return,
    };

    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let request: IpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = IpcResponse::Error {
                    message: format!("Invalid request: {e}"),
                };
                let _ = writeln!(&stream, "{}", serde_json::to_string(&resp).unwrap());
                continue;
            }
        };

        let response = handle_ipc_request(request, controllers);
        let _ = writeln!(&stream, "{}", serde_json::to_string(&response).unwrap());
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
                .map(|ctrl| DeviceStatus {
                    device_id: ctrl.device_id.clone(),
                    name: match &ctrl.source {
                        InputSource::Hidraw { .. } => "Elite 2 (hidraw)".to_string(),
                        InputSource::Evdev { reader } => reader.info.name.clone(),
                    },
                    hw_profile: ctrl.prev_state.hw_profile,
                    active_profile: ctrl.config.active_override.unwrap_or(0),
                    connected: true,
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
                ctrl.config = config.clone();
                if let Err(e) = config::save_config("elite2", &config) {
                    return IpcResponse::Error { message: format!("Failed to save: {e}") };
                }
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
            let dir = config::config_dir();
            let profiles = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries.flatten()
                        .filter_map(|e| {
                            let name = e.file_name().to_str()?.to_string();
                            name.ends_with(".json").then_some(name)
                        })
                        .collect()
                })
                .unwrap_or_default();
            IpcResponse::ProfileList { profiles }
        }
    }
}
