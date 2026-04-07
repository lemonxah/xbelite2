// SPDX-License-Identifier: GPL-2.0

//! Xbox Elite Series 2 controller driver — Rust logic.

#![allow(missing_docs)]

use core::ffi::c_int;

/// BT HID: called after probe succeeds (HID parse + start done in C)
#[no_mangle]
pub extern "C" fn xbelite2_on_bt_connect() {
    // Future: initialize per-device state here
}

/// BT HID: called before remove
#[no_mangle]
pub extern "C" fn xbelite2_on_bt_disconnect() {
    // Future: cleanup per-device state here
}

/// BT HID: process a raw HID report. Return 0 to pass through to hidraw.
#[no_mangle]
pub extern "C" fn xbelite2_on_bt_report(_data: *const u8, _size: c_int) -> c_int {
    // All reports pass through to hidraw for the daemon.
    // Future: could filter or transform reports in-kernel.
    0
}

/// USB GIP: called after USB probe succeeds
#[no_mangle]
pub extern "C" fn xbelite2_on_usb_connect() {
    // Future: initialize USB GIP state
}

/// USB GIP: called before disconnect
#[no_mangle]
pub extern "C" fn xbelite2_on_usb_disconnect() {
    // Future: cleanup USB GIP state
}

/// USB GIP: process a GIP message from the controller.
/// Returns true (1) if the message should be forwarded to userspace.
#[no_mangle]
pub extern "C" fn xbelite2_on_gip_message(data: *const u8, size: c_int) -> c_int {
    if data.is_null() || size < 1 {
        return 0;
    }

    let cmd = unsafe { *data };

    // Forward gamepad input, elite extended reports, and vendor messages
    match cmd {
        0x20 | 0x07 | 0x0C | 0x4D | 0x1E | 0x01 | 0x02 | 0x03 => 1, // INPUT, GUIDE, ELITE, VENDOR, SYSTEM, ACK, HELLO, STATUS
        _ => 0,
    }
}
