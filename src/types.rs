//! Core data types for the Xbox Elite 2 driver.

use serde::{Deserialize, Serialize};

/// Microsoft vendor ID
pub const VENDOR_MICROSOFT: u16 = 0x045E;
/// Elite 2 Bluetooth Classic (firmware 4.x)
pub const PID_ELITE2_BT_CLASSIC: u16 = 0x0B05;
/// Elite 2 BLE (firmware 5.x+)
pub const PID_ELITE2_BLE: u16 = 0x0B22;
/// Elite 2 USB
pub const PID_ELITE2_USB: u16 = 0x0B00;
/// Xbox 360 spoofed PID (used by BT HID layer and xpadneo)
pub const PID_XBOX360_SPOOFED: u16 = 0x028E;

/// Maximum number of profiles
pub const MAX_PROFILES: usize = 8;
/// Maximum remap entries per profile
pub const MAX_REMAPS: usize = 32;
/// Number of points in a piecewise-linear curve
pub const CURVE_POINTS: usize = 16;

/// Linux input event types
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;

/// Standard gamepad button codes
pub const BTN_A: u16 = 0x130;
pub const BTN_B: u16 = 0x131;
pub const BTN_X: u16 = 0x133;
pub const BTN_Y: u16 = 0x134;
pub const BTN_TL: u16 = 0x136; // LB
pub const BTN_TR: u16 = 0x137; // RB
pub const BTN_SELECT: u16 = 0x13A; // View/Back
pub const BTN_START: u16 = 0x13B; // Menu
pub const BTN_MODE: u16 = 0x13C; // Xbox button
pub const BTN_THUMBL: u16 = 0x13D; // Left stick click
pub const BTN_THUMBR: u16 = 0x13E; // Right stick click

/// Paddle button codes (kernel 6.17+ standard)
pub const BTN_GRIPL: u16 = 0x224;
pub const BTN_GRIPR: u16 = 0x225;
pub const BTN_GRIPL2: u16 = 0x226;
pub const BTN_GRIPR2: u16 = 0x227;

/// Fallback paddle codes for older kernels (trigger happy)
pub const BTN_TRIGGER_HAPPY5: u16 = 0x2C4;
pub const BTN_TRIGGER_HAPPY6: u16 = 0x2C5;
pub const BTN_TRIGGER_HAPPY7: u16 = 0x2C6;
pub const BTN_TRIGGER_HAPPY8: u16 = 0x2C7;

/// D-pad as hat axis values
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

/// Axis codes
pub const ABS_X: u16 = 0x00; // Left stick X
pub const ABS_Y: u16 = 0x01; // Left stick Y
pub const ABS_Z: u16 = 0x02; // Left trigger
pub const ABS_RX: u16 = 0x03; // Right stick X
pub const ABS_RY: u16 = 0x04; // Right stick Y
pub const ABS_RZ: u16 = 0x05; // Right trigger

/// Firmware report format variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Firmware 4.x: BT Classic, specific byte offsets
    V4,
    /// Firmware 5.0-5.10: BLE, different offsets
    V5Early,
    /// Firmware 5.11+: BLE, paddles in extended report
    V5Late,
}

/// Raw parsed gamepad state from a single HID report
#[derive(Debug, Clone, Default)]
pub struct GamepadState {
    // Buttons (true = pressed)
    pub btn_a: bool,
    pub btn_b: bool,
    pub btn_x: bool,
    pub btn_y: bool,
    pub btn_lb: bool,
    pub btn_rb: bool,
    pub btn_view: bool,
    pub btn_menu: bool,
    pub btn_xbox: bool,
    pub btn_lstick: bool,
    pub btn_rstick: bool,

    // D-pad
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,

    // Paddles
    pub paddle_ur: bool, // Upper right (P1)
    pub paddle_lr: bool, // Lower right (P2)
    pub paddle_ul: bool, // Upper left (P3)
    pub paddle_ll: bool, // Lower left (P4)

    // Axes (raw values)
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u16,  // 0-1023
    pub right_trigger: u16, // 0-1023

    // Profile reported by controller hardware (0-3)
    pub hw_profile: u8,
}

/// A single input event (button press or axis movement)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputId {
    pub ev_type: u16,
    pub code: u16,
}

impl InputId {
    pub const fn key(code: u16) -> Self {
        Self {
            ev_type: EV_KEY,
            code,
        }
    }
    pub const fn abs(code: u16) -> Self {
        Self {
            ev_type: EV_ABS,
            code,
        }
    }
}

/// A remap rule: source input -> destination input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemapEntry {
    pub src: InputId,
    pub dst: InputId,
    /// For key->abs: the axis value to emit on press. For abs->abs: scale factor (0.0-2.0).
    pub scale: f32,
}

/// Piecewise-linear response curve for stick axes.
/// Points are evenly spaced across input range [0, 32767].
/// Applied to abs(value), sign is preserved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickCurve {
    pub points: [i16; CURVE_POINTS],
}

impl Default for StickCurve {
    fn default() -> Self {
        // Linear identity curve
        let mut points = [0i16; CURVE_POINTS];
        for i in 0..CURVE_POINTS {
            points[i] = ((i as i32 * 32767) / (CURVE_POINTS as i32 - 1)) as i16;
        }
        Self { points }
    }
}

impl StickCurve {
    /// Evaluate the curve for a raw stick value.
    pub fn evaluate(&self, raw: i16) -> i16 {
        if raw == 0 {
            return 0;
        }
        let sign = if raw < 0 { -1i32 } else { 1 };
        let abs_val = (raw as i32).unsigned_abs().min(32767) as u32;

        // Map abs_val [0..32767] to fractional index in [0..CURVE_POINTS-1]
        let scaled = abs_val as u64 * (CURVE_POINTS as u64 - 1);
        let idx = (scaled / 32767) as usize;
        let frac = (scaled % 32767) as i32;

        let result = if idx >= CURVE_POINTS - 1 {
            self.points[CURVE_POINTS - 1] as i32
        } else {
            let a = self.points[idx] as i32;
            let b = self.points[idx + 1] as i32;
            a + ((b - a) * frac) / 32767
        };

        (result * sign).clamp(-32767, 32767) as i16
    }
}

/// Trigger dead zone configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerZone {
    pub dead_min: u16, // Below this -> 0
    pub dead_max: u16, // Above this -> 1023
}

impl Default for TriggerZone {
    fn default() -> Self {
        Self {
            dead_min: 0,
            dead_max: 1023,
        }
    }
}

impl TriggerZone {
    /// Apply dead zone to a raw trigger value (0-1023).
    pub fn apply(&self, raw: u16) -> u16 {
        if raw <= self.dead_min {
            return 0;
        }
        if raw >= self.dead_max {
            return 1023;
        }
        let range = self.dead_max - self.dead_min;
        if range == 0 {
            return 0;
        }
        ((raw - self.dead_min) as u32 * 1023 / range as u32) as u16
    }
}

/// Per-motor vibration intensity (0-100).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationConfig {
    pub main_motor: u8,
    pub weak_motor: u8,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

impl Default for VibrationConfig {
    fn default() -> Self {
        Self {
            main_motor: 100,
            weak_motor: 100,
            left_trigger: 100,
            right_trigger: 100,
        }
    }
}

/// A complete profile configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub remaps: Vec<RemapEntry>,
    pub stick_curves: [Option<StickCurve>; 4], // LX, LY, RX, RY
    pub trigger_zones: [Option<TriggerZone>; 2], // Left, Right
    pub vibration: VibrationConfig,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::from("Default"),
            remaps: Vec::new(),
            stick_curves: [None, None, None, None],
            trigger_zones: [None, None],
            vibration: VibrationConfig::default(),
        }
    }
}

/// Configuration mapping hardware profiles to software profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Profiles indexed by slot number
    pub profiles: Vec<Profile>,
    /// Map hardware profile (0-3) to software profile index
    pub hw_profile_map: [usize; 4],
    /// Currently active software profile (overrides hw_profile_map if set)
    pub active_override: Option<usize>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            profiles: vec![Profile::default()],
            hw_profile_map: [0, 0, 0, 0],
            active_override: None,
        }
    }
}
