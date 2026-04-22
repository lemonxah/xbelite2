// SPDX-License-Identifier: GPL-2.0
// Xbox Elite Series 2 controller driver logic.

// BT HID: called after probe succeeds (HID parse + start done in C)
void xbelite2_on_bt_connect(void)
{
	// Future: initialize per-device state here
}

// BT HID: called before remove
void xbelite2_on_bt_disconnect(void)
{
	// Future: cleanup per-device state here
}

// BT HID: process a raw HID report. Return 0 to pass through to hidraw.
int xbelite2_on_bt_report(const unsigned char *data, int size)
{
	// All reports pass through to hidraw for the daemon.
	// Future: could filter or transform reports in-kernel.
	(void)data;
	(void)size;
	return 0;
}

// USB GIP: called after USB probe succeeds
void xbelite2_on_usb_connect(void)
{
	// Future: initialize USB GIP state
}

// USB GIP: called before disconnect
void xbelite2_on_usb_disconnect(void)
{
	// Future: cleanup USB GIP state
}

// USB GIP: process a GIP message from the controller.
// Returns true (1) if the message should be forwarded to userspace.
int xbelite2_on_gip_message(const unsigned char *data, int size)
{
	unsigned char cmd;

	if (!data || size < 1) {
		return 0;
	}

	cmd = data[0];

	// Forward gamepad input, elite extended reports, and vendor messages
	switch (cmd) {
		case 0x20: // INPUT
		case 0x07: // GUIDE
		case 0x0C: // ELITE
		case 0x4D: // VENDOR
		case 0x1E: // SYSTEM
		case 0x01: // ACK
		case 0x02: // HELLO
		case 0x03: // STATUS
			return 1;
		default:
			return 0;
	}
}
