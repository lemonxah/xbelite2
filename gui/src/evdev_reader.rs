use evdev::{Device, InputEventKind, AbsoluteAxisType, Key};
use std::os::fd::AsRawFd;
use std::path::Path;

pub struct ControllerInput {
    pub buttons: u16,
    pub paddles: u8,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u16,
    pub right_trigger: u16,
}

impl Default for ControllerInput {
    fn default() -> Self {
        Self {
            buttons: 0,
            paddles: 0,
            left_stick_x: 0,
            left_stick_y: 0,
            right_stick_x: 0,
            right_stick_y: 0,
            left_trigger: 0,
            right_trigger: 0,
        }
    }
}

pub struct EvdevReader {
    pub device: Option<Device>,
    state: ControllerInput,
}

impl EvdevReader {
    pub fn new() -> Self {
        Self {
            device: None,
            state: ControllerInput::default(),
        }
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        match Device::open(path) {
            Ok(dev) => {
                // Set O_NONBLOCK so fetch_events() returns immediately when the
                // queue is empty instead of blocking the Qt main thread.
                let fd = dev.as_raw_fd();
                unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                    if flags >= 0 {
                        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                    }
                }
                self.device = Some(dev);
                Ok(())
            }
            Err(e) => Err(format!("Failed to open {}: {}", path.display(), e)),
        }
    }

    pub fn poll(&mut self) -> ControllerInput {
        if let Some(dev) = &mut self.device {
            if let Ok(events) = dev.fetch_events() {
                for ev in events {
                    let value = ev.value();
                    match ev.kind() {
                        InputEventKind::Key(key) => {
                            let pressed = value == 1;
                            let released = value == 0;
                            if pressed || released {
                                Self::update_button(&mut self.state, key, pressed);
                            }
                        }
                        InputEventKind::AbsAxis(axis) => {
                            Self::update_axis(&mut self.state, axis, value);
                        }
                        _ => {}
                    }
                }
            }
        }
        self.state.clone()
    }

    fn update_button(state: &mut ControllerInput, key: Key, pressed: bool) {
        let bit = match key {
            Key::BTN_SOUTH => Some(0),
            Key::BTN_EAST => Some(1),
            Key::BTN_NORTH => Some(2),
            Key::BTN_WEST => Some(3),
            Key::BTN_TL => Some(4),
            Key::BTN_TR => Some(5),
            Key::BTN_SELECT => Some(6),
            Key::BTN_START => Some(7),
            Key::BTN_MODE => Some(8),
            Key::BTN_THUMBL => Some(9),
            Key::BTN_THUMBR => Some(10),
            Key::BTN_TRIGGER_HAPPY1 => Some(16),
            Key::BTN_TRIGGER_HAPPY2 => Some(17),
            Key::BTN_TRIGGER_HAPPY3 => Some(18),
            Key::BTN_TRIGGER_HAPPY4 => Some(19),
            _ => None,
        };

        if let Some(b) = bit {
            if pressed {
                if b >= 16 {
                    let paddle_bit = b - 16;
                    state.paddles |= 1 << paddle_bit;
                } else {
                    state.buttons |= 1 << b;
                }
            } else {
                if b >= 16 {
                    let paddle_bit = b - 16;
                    state.paddles &= !(1 << paddle_bit);
                } else {
                    state.buttons &= !(1 << b);
                }
            }
        }
    }

    fn update_axis(state: &mut ControllerInput, axis: AbsoluteAxisType, value: i32) {
        match axis {
            AbsoluteAxisType::ABS_X => {
                state.left_stick_x = value as i16;
            }
            AbsoluteAxisType::ABS_Y => {
                state.left_stick_y = value as i16;
            }
            AbsoluteAxisType::ABS_RX => {
                state.right_stick_x = value as i16;
            }
            AbsoluteAxisType::ABS_RY => {
                state.right_stick_y = value as i16;
            }
            AbsoluteAxisType::ABS_Z => {
                state.left_trigger = value as u16;
            }
            AbsoluteAxisType::ABS_RZ => {
                state.right_trigger = value as u16;
            }
            _ => {}
        }
    }
}

impl Clone for ControllerInput {
    fn clone(&self) -> Self {
        Self {
            buttons: self.buttons,
            paddles: self.paddles,
            left_stick_x: self.left_stick_x,
            left_stick_y: self.left_stick_y,
            right_stick_x: self.right_stick_x,
            right_stick_y: self.right_stick_y,
            left_trigger: self.left_trigger,
            right_trigger: self.right_trigger,
        }
    }
}
