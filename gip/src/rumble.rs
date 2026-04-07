use crate::transport::GipDevice;

/// Send a USB GIP rumble command.
/// Each motor value is 0–100.
///
/// Motor layout:
/// - `left_motor`: big body rumble (left grip)
/// - `right_motor`: small body rumble (right grip)
/// - `left_trigger`: left trigger impulse motor
/// - `right_trigger`: right trigger impulse motor
pub fn set(
    dev: &mut GipDevice,
    left_motor: u8,
    right_motor: u8,
    left_trigger: u8,
    right_trigger: u8,
) {
    // GIP rumble: [cmd, flags, seq, len, sub, mask, LT, RT, LMotor, RMotor, 0xFF, 0x00, 0xEB]
    let pkt = [
        0x09u8, 0x00, 0x00, 0x09,
        0x00, 0x0F,
        left_trigger.min(100),
        right_trigger.min(100),
        left_motor.min(100),
        right_motor.min(100),
        0xFF, 0x00, 0xEB,
    ];
    if let Err(e) = dev.send(&pkt) {
        eprintln!("rumble send failed: {e}");
    }
}

/// Stop all USB GIP rumble motors.
pub fn stop(dev: &mut GipDevice) {
    set(dev, 0, 0, 0, 0);
    set(dev, 0, 0, 0, 0);
}

/// Send a BT HID rumble command.
/// Each motor value is 0–100.
pub fn set_bt(
    dev: &mut GipDevice,
    left_motor: u8,
    right_motor: u8,
    left_trigger: u8,
    right_trigger: u8,
) {
    // BT HID output report: [report_id=0x03, motor_mask=0x0F, LT, RT, LMotor, RMotor, duration, delay, repeat]
    let pkt = [
        0x03u8, 0x0F,
        left_trigger.min(100),
        right_trigger.min(100),
        left_motor.min(100),
        right_motor.min(100),
        0xFF, 0x00, 0x00,
    ];
    if let Err(e) = dev.send(&pkt) {
        eprintln!("bt rumble send failed: {e}");
    }
}

/// Stop all BT rumble motors.
pub fn stop_bt(dev: &mut GipDevice) {
    set_bt(dev, 0, 0, 0, 0);
    set_bt(dev, 0, 0, 0, 0);
}
