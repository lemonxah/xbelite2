use serde::{Deserialize, Serialize};

/// GIP button remap codes (hardware-level, from protocol RE)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GipButton {
    A = 0x04,
    B = 0x05,
    X = 0x06,
    Y = 0x07,
    LB = 0x08,
    RB = 0x09,
    LT = 0x0A,
    RT = 0x0B,
    DUp = 0x0C,
    DDown = 0x0D,
    DLeft = 0x0E,
    DRight = 0x0F,
}

impl GipButton {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0x04 => Some(Self::A),
            0x05 => Some(Self::B),
            0x06 => Some(Self::X),
            0x07 => Some(Self::Y),
            0x08 => Some(Self::LB),
            0x09 => Some(Self::RB),
            0x0A => Some(Self::LT),
            0x0B => Some(Self::RT),
            0x0C => Some(Self::DUp),
            0x0D => Some(Self::DDown),
            0x0E => Some(Self::DLeft),
            0x0F => Some(Self::DRight),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "a" => Some(Self::A),
            "b" => Some(Self::B),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "lb" => Some(Self::LB),
            "rb" => Some(Self::RB),
            "lt" => Some(Self::LT),
            "rt" => Some(Self::RT),
            "dup" | "up" => Some(Self::DUp),
            "ddown" | "down" => Some(Self::DDown),
            "dleft" | "left" => Some(Self::DLeft),
            "dright" | "right" => Some(Self::DRight),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::X => "X",
            Self::Y => "Y",
            Self::LB => "LB",
            Self::RB => "RB",
            Self::LT => "LT",
            Self::RT => "RT",
            Self::DUp => "DUp",
            Self::DDown => "DDown",
            Self::DLeft => "DLeft",
            Self::DRight => "DRight",
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }
}

/// Profile page layout offsets (56-byte mapping page, 0-indexed after header strip)
pub const MAPPING_SIZE: u8 = 0x38; // 56 bytes
pub const CURVES_SIZE: u8 = 0x2B;  // 43 bytes

pub const OFF_FLAGS: usize = 0;
pub const OFF_REMAP_A: usize = 1;
pub const OFF_REMAP_B: usize = 5;
pub const OFF_REMAP_EXT: usize = 9;
pub const OFF_DEADZONES: usize = 28;
pub const OFF_COLOR_FLAG: usize = 45;
pub const OFF_COLOR_R: usize = 46;
pub const OFF_COLOR_G: usize = 47;
pub const OFF_COLOR_B: usize = 48;
pub const OFF_VIBRATION: usize = 49;

pub const FLAGS_DEFAULT: u8 = 0x11;
pub const FLAGS_CUSTOM: u8 = 0x10;

/// Profile page addresses
/// Each profile has 2 slots (A=normal, B=shift) x 2 types (mapping, curves)
pub const PROFILE_MAPPING_PAGES: [[u8; 2]; 3] = [
    [0x20, 0x26], // Profile 1: SlotA, SlotB
    [0x22, 0x28], // Profile 2
    [0x24, 0x2A], // Profile 3
];

pub const PROFILE_CURVES_PAGES: [[u8; 2]; 3] = [
    [0x21, 0x27],
    [0x23, 0x29],
    [0x25, 0x2B],
];

/// Default stick curve: linear (3 control points)
pub const DEFAULT_CURVE: [u8; 6] = [0x2B, 0x2B, 0x7F, 0x7F, 0xBF, 0xBF];

/// Default face button remap
pub const DEFAULT_FACE: [u8; 4] = [0x04, 0x05, 0x06, 0x07]; // A B X Y
pub const DEFAULT_EXT: [u8; 8] = [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F];

/// Decoded profile mapping data
#[derive(Debug, Clone)]
pub struct ProfileMapping {
    pub flags: u8,
    pub remap_a: [u8; 4],
    pub remap_b: [u8; 4],
    pub remap_ext: [u8; 8],
    pub deadzones: [u8; 4], // [LStick, RStick, LTrigger, RTrigger]
    pub color: Option<(u8, u8, u8)>, // None = default white
    pub vibration: (u8, u8),
    pub raw: Vec<u8>,
}

impl ProfileMapping {
    pub fn from_raw(data: &[u8]) -> Option<Self> {
        if data.len() < 56 {
            return None;
        }
        let color = if data[OFF_COLOR_FLAG] == 0xFF {
            None
        } else {
            Some((data[OFF_COLOR_R], data[OFF_COLOR_G], data[OFF_COLOR_B]))
        };
        Some(Self {
            flags: data[OFF_FLAGS],
            remap_a: [data[1], data[2], data[3], data[4]],
            remap_b: [data[5], data[6], data[7], data[8]],
            remap_ext: [
                data[9], data[10], data[11], data[12],
                data[13], data[14], data[15], data[16],
            ],
            deadzones: [data[28], data[29], data[30], data[31]],
            color,
            vibration: (data[OFF_VIBRATION], data[OFF_VIBRATION + 1]),
            raw: data.to_vec(),
        })
    }

    pub fn is_custom(&self) -> bool {
        self.flags != FLAGS_DEFAULT
    }
}

/// Decoded profile curves data
#[derive(Debug, Clone)]
pub struct ProfileCurves {
    pub flags: u8,
    pub curves: [[u8; 6]; 4], // LX, LY, RX, RY
    pub raw: Vec<u8>,
}

impl ProfileCurves {
    pub fn from_raw(data: &[u8]) -> Option<Self> {
        if data.len() < 43 {
            return None;
        }
        let mut curves = [[0u8; 6]; 4];
        for i in 0..4 {
            let off = 1 + i * 6;
            curves[i].copy_from_slice(&data[off..off + 6]);
        }
        Some(Self {
            flags: data[0],
            curves,
            raw: data.to_vec(),
        })
    }
}
