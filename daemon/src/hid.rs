//! HID report parsing for the Xbox Elite Series 2 controller.
//!
//! Parses raw bytes from hidraw into GamepadState structs.
//! Handles BT BLE (firmware 5.x) confirmed from real hardware captures.
//!
//! Confirmed BT BLE report layout (20 bytes, report ID 0x01):
//!   Byte  0: report ID (0x01)
//!   Byte  1: buttons byte 0
//!   Byte  2: buttons byte 1 (upper nibble) + hat switch (lower nibble)
//!   Bytes 3-4: left trigger (16-bit LE, 10-bit effective 0-1023)
//!   Bytes 5-6: right trigger (16-bit LE, 10-bit effective 0-1023)
//!   Bytes 7-8: left stick X (int16 LE)
//!   Bytes 9-10: left stick Y (int16 LE)
//!   Bytes 11-12: right stick X (int16 LE)
//!   Bytes 13-14: right stick Y (int16 LE)
//!   Bytes 15-17: padding/unknown (zeros)
//!   Byte 18: profile (lower 2 bits) + trigger mode (upper bits)
//!            0x0a = profile 2, 0x08 = profile 0 with trigger mode, etc.
//!   Byte 19: paddle bitmask
//!            bit 0 = upper right (P1)
//!            bit 1 = lower right (P2)
//!            bit 2 = upper left (P3)
//!            bit 3 = lower left (P4)

use crate::types::{GamepadState, ReportFormat};

/// Detect firmware report format from the HID report size.
pub fn detect_format(report: &[u8]) -> ReportFormat {
    match report.len() {
        55.. => ReportFormat::V4,
        20.. => ReportFormat::V5Early,
        _ => ReportFormat::V5Early,
    }
}

/// Parse a BT HID gamepad report (report ID 0x01) into GamepadState.
pub fn parse_bt_report(data: &[u8], _format: ReportFormat) -> Option<GamepadState> {
    if data.len() < 15 {
        return None;
    }

    let mut state = GamepadState::default();

    // Byte 1: buttons low
    let btns0 = data[1];
    // Byte 2: buttons high (upper nibble) + hat (lower nibble)
    let btns1 = data[2];

    // Button mapping confirmed from hardware:
    // Byte 1 bits: A=0, B=1, ?=2, X=3, Y=4, ?=5, LB=6, RB=7
    state.btn_a = btns0 & 0x01 != 0;
    state.btn_b = btns0 & 0x02 != 0;
    state.btn_x = btns0 & 0x08 != 0;
    state.btn_y = btns0 & 0x10 != 0;
    state.btn_lb = btns0 & 0x40 != 0;
    state.btn_rb = btns0 & 0x80 != 0;

    // Byte 2 upper nibble: View=2, Menu=3, LStick=5, RStick=6
    state.btn_view = btns1 & 0x04 != 0;
    state.btn_menu = btns1 & 0x08 != 0;
    state.btn_lstick = btns1 & 0x20 != 0;
    state.btn_rstick = btns1 & 0x40 != 0;

    // D-pad from hat switch (byte 2 lower nibble... but byte 2 is mixed)
    // Actually hat is in its own field. From the capture:
    // byte 2 = 0x81 at rest -> upper bits = buttons, lower = hat value 1?
    // Let's use the standard hat decoding on the lower nibble of a dedicated byte
    // Looking at capture: btns1 = 0x81 -> hat = 1 (N+E?) or this is just button state
    // The hat might be encoded differently. For now, handle via separate byte if present.
    // TODO: confirm hat encoding with d-pad presses
    let hat = btns1 & 0x0F;
    // Standard hat: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8+=centered
    // But the resting value shows 0x01 in lower nibble which would be NE...
    // This suggests the lower nibble has button bits, not hat.
    // The hat may be elsewhere. We'll handle d-pad via evdev for now.

    // Triggers (16-bit LE at bytes 3-6, effective range 0-1023)
    if data.len() >= 7 {
        state.left_trigger = u16::from_le_bytes([data[3], data[4]]) & 0x03FF;
        state.right_trigger = u16::from_le_bytes([data[5], data[6]]) & 0x03FF;
    }

    // Thumbsticks (signed 16-bit LE at bytes 7-14)
    if data.len() >= 15 {
        state.left_stick_x = i16::from_le_bytes([data[7], data[8]]);
        state.left_stick_y = i16::from_le_bytes([data[9], data[10]]);
        state.right_stick_x = i16::from_le_bytes([data[11], data[12]]);
        state.right_stick_y = i16::from_le_bytes([data[13], data[14]]);
    }

    // Byte 17: profile number (0-3) — confirmed from hardware capture
    //   0x00 = profile 0 (default, no LED)
    //   0x01 = profile 1
    //   0x02 = profile 2
    //   0x03 = profile 3
    if data.len() > 17 {
        state.hw_profile = data[17] & 0x03;
    }

    // Byte 18: trigger mode — constant 0x0a in testing, not profile

    // Byte 19: paddle bitmask — confirmed working in ALL profiles
    //   bit 0 = P1 upper right
    //   bit 1 = P2 lower right
    //   bit 2 = P3 upper left
    //   bit 3 = P4 lower left
    //
    // NOTE: In profiles 1-3, byte 15 also contains the firmware's
    // remapped button. We ignore byte 15 and use byte 19 exclusively
    // for paddles so our PC-side profiles override the controller's.
    if data.len() > 19 {
        let paddles = data[19];
        state.paddle_ur = paddles & 0x01 != 0;
        state.paddle_lr = paddles & 0x02 != 0;
        state.paddle_ul = paddles & 0x04 != 0;
        state.paddle_ll = paddles & 0x08 != 0;
    }

    Some(state)
}

/// Parse an extended report (firmware 5.11+) containing paddle data.
pub fn parse_extended_report(data: &[u8], state: &mut GamepadState) -> bool {
    if data.len() < 20 {
        return false;
    }
    let paddles = data[19];
    state.paddle_ur = paddles & 0x01 != 0;
    state.paddle_lr = paddles & 0x02 != 0;
    state.paddle_ul = paddles & 0x04 != 0;
    state.paddle_ll = paddles & 0x08 != 0;
    true
}

/// Init command to send to firmware 5.11+ to enable extended paddle reports.
pub const ELITE2_EXTENDED_REPORT_INIT: &[u8] = &[0x4d, 0x10, 0x01, 0x02, 0x07, 0x00];

/// Check if a HID device is an Xbox Elite Series 2 based on vendor/product ID.
pub fn is_elite2(vendor: u16, product: u16) -> bool {
    vendor == crate::types::VENDOR_MICROSOFT
        && matches!(
            product,
            crate::types::PID_ELITE2_BT_CLASSIC
                | crate::types::PID_ELITE2_BLE
                | crate::types::PID_XBOX360_SPOOFED
        )
}
