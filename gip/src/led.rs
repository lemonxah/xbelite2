use crate::transport::GipDevice;

/// Set the LED color (live preview, not saved to profile).
/// Requires unlock() to have been called.
pub fn set_color(dev: &mut GipDevice, r: u8, g: u8, b: u8) {
    let _ = dev.send_cmd(0x0E, 0x00, &[0x00, 0x00, r, g, b]);
    dev.drain();
}

/// Turn off the LED / return to profile color.
pub fn off(dev: &mut GipDevice) {
    let _ = dev.send_cmd(0x0E, 0x00, &[0x01, 0x00, 0x00, 0x00, 0x00]);
    dev.drain();
}
