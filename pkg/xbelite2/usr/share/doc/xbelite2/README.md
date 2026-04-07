# xbelite2

Linux userspace driver and configuration tool for the Xbox Elite Wireless Controller Series 2.

![Xbox Elite Series 2](docs/elite2.png)

## What this does

The Xbox Elite 2 has four back paddles and a hardware profile switch. On Linux, the stock drivers (`xpad`, `hid-generic`) don't expose the paddles properly over Bluetooth, and there's no way to configure button remapping, stick response curves, or trigger dead zones without the Windows Xbox Accessories app.

This project fixes that. It consists of two parts:

- **xbelite2d** — a daemon that reads raw HID reports from the controller (Bluetooth or USB), parses all inputs including paddles and the hardware profile switch, and emits a virtual gamepad via uinput with configurable input transformations
- **xbelite2-gui** — a Qt6/QML configuration app that talks to the daemon over a Unix socket, showing live controller state and letting you edit profiles

### How profiles work

The Elite 2 has a physical profile switch with 4 positions (0-3):

- **Profile 0 (Default)** — pure passthrough. All inputs go straight through to the virtual gamepad with no modifications. Use this for Steam Input or any game that handles its own bindings.
- **Profiles 1, 2, 3** — each maps to a software profile you configure in the GUI. Button remapping, stick response curves, stick dead zones, trigger dead zones, and vibration intensity are all applied before the virtual gamepad sees the input.

Profiles are stored in `~/.config/xbelite2/elite2.json` (owned by your user, not root).

### What you can configure per profile

- Button and paddle remapping (any button to any other button)
- Stick response curves (16-point piecewise-linear, per axis)
- Stick dead zones (per stick, 0-50%)
- Trigger dead zones (min/max per trigger)
- Vibration intensity (per motor: main, weak, left trigger, right trigger)

## Building

### Requirements

- Rust toolchain (stable)
- Qt 6 with QtQuick/QML (`qt6-base`, `qt6-declarative`)
- `libc` headers

On Arch Linux:

```
sudo pacman -S qt6-base qt6-declarative
```

### Daemon

```
cargo build --release
```

### GUI

```
cd gui
cargo build --release
```

## Running

### Start the daemon

The daemon needs root for access to `/dev/hidraw*` and `/dev/uinput`:

```
sudo ./target/release/xbelite2d
```

Or install the systemd service:

```
sudo cp xbelite2d.service /etc/systemd/system/
sudo cp 99-xbelite2.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo systemctl enable --now xbelite2d
```

### Run the GUI

```
./gui/target/release/xbelite2-gui
```

The GUI connects to the daemon at `/run/xbelite2.sock`. It loads and saves profiles from `~/.config/xbelite2/`.

## Technical details

### Bluetooth HID report format

The Elite 2 connects over BLE (PID `0x0B22`, appears as `0x028E` after kernel spoofing). The 20-byte HID report (report ID `0x01`) layout, confirmed from the actual HID descriptor and hardware testing:

| Bytes | Field | Format |
|-------|-------|--------|
| 1-2 | Left Stick X | unsigned u16, center=32768 |
| 3-4 | Left Stick Y | unsigned u16, center=32768 |
| 5-6 | Right Stick X | unsigned u16, center=32768 |
| 7-8 | Right Stick Y | unsigned u16, center=32768 |
| 9-10 | Left Trigger | 10-bit + 6 padding, 0-1023 |
| 11-12 | Right Trigger | 10-bit + 6 padding, 0-1023 |
| 13 | D-pad hat | 4-bit (1-8 directions, 0=center) |
| 14-15 | Buttons | 12 bits (A,B,X,Y,LB,RB,View,Menu,LS,RS,Xbox,Share) |
| 16 | Share button | 1 bit + 7 padding |
| 17 | Profile number | 0-3 |
| 18 | Trigger mode | 4-bit + 4 padding |
| 19 | Paddles | 4-bit (UR, LR, UL, LL) |

The paddles at byte 19 report in all four hardware profiles over Bluetooth. This is the key finding that makes the whole project possible — `xpadneo` and `xpad` both suppress paddle data in profiles 1-3, but the raw HID report always contains it.

### Architecture

```
Controller (BT/USB)
    |
    v
/dev/hidraw* -----> xbelite2d (reads raw HID, grabs evdev)
                        |
                        |--> parse HID report
                        |--> apply profile transforms (if profile 1-3)
                        |--> emit to /dev/uinput virtual gamepad
                        |
                        |--> /run/xbelite2.sock (IPC)
                                |
                                v
                          xbelite2-gui (Qt6/QML)
                                |
                                v
                     ~/.config/xbelite2/elite2.json
```

## License

GPL-2.0-only
