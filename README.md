# xbelite2

Linux driver and configuration tool for the Xbox Elite Wireless Controller Series 2.

![Xbox Elite Series 2](docs/elite2.png)

## What this does

The Xbox Elite 2 has four back paddles, a hardware profile switch, and per-profile LED colors, button remapping, stick curves, and trigger dead zones stored on the controller. On Linux, the stock drivers (`xpad`, `hid-generic`) don't expose the paddles properly, and there's no way to configure the controller without the Windows Xbox Accessories app.

This project fixes that with a custom kernel module, a userspace daemon, a CLI config tool, and a GUI.

## Components

| Component | Description |
|-----------|-------------|
| **xbelite2** (kmod) | DKMS kernel module. Claims the controller on USB and BT, provides `/dev/xbelite2` (USB) and `/dev/xbelite2_bt` (BT) character devices for userspace |
| **xbelite2d** (daemon) | Reads input from the kernel module, applies profile transforms (remapping, curves, dead zones), emits a virtual gamepad via uinput. Handles force-feedback/rumble forwarding |
| **xbe2-rw** (CLI) | Reads and writes controller config over USB via the GIP protocol: profiles, button remaps, LED colors, dead zones, stick curves, device name, rumble testing |
| **xbelite2-gui** (GUI) | Qt6/QML configuration app. Connects to the daemon over a Unix socket for live input display and profile editing |

### What you can configure

**On the controller hardware** (via `xbe2-rw` or GUI, USB only):
- Profile LED colors (RGB)
- Button remapping (normal and shift/alternate mode)
- Stick dead zones and response curves
- Vibration motor intensity
- Device name

**In software profiles** (via GUI or config file):
- Button and paddle remapping
- Stick response curves (16-point per axis)
- Stick and trigger dead zones
- Vibration intensity scaling

### How profiles work

The Elite 2 has a physical profile switch with 4 positions (0-3):

- **Profile 0 (Default)** — pure passthrough, no modifications. Use this for Steam Input or games that handle their own bindings.
- **Profiles 1, 2, 3** — each maps to a software profile. Button remapping, stick curves, dead zones, and vibration scaling are applied before the virtual gamepad sees the input.

Software profiles are stored in `~/.config/xbelite2/elite2.json`.

## Installation

### Arch Linux (AUR)

```
yay -S xbelite2-dkms
```

This installs the kernel module (via DKMS), daemon, GUI, udev rules, and systemd service.

### From source

Requirements:
- Rust toolchain (stable)
- Linux kernel headers (for DKMS module build)
- Qt 6 with QtQuick/QML (`qt6-base`, `qt6-declarative`)

```bash
# Build everything
cargo build --workspace --release

# Build and load kernel module
just kmod

# Install daemon + service
just install
```

### Manual setup

```bash
# Install udev rules and modprobe config
sudo cp 99-xbelite2.rules /etc/udev/rules.d/
sudo cp pkg/modprobe.d/xbelite2-blacklist.conf /etc/modprobe.d/
sudo udevadm control --reload-rules

# Start daemon
sudo systemctl enable --now xbelite2d
```

## Usage

### CLI tool

```bash
# Read controller info
sudo xbe2-rw read

# Read/write device name
sudo xbe2-rw name
sudo xbe2-rw name "my controller"

# Profile summary
sudo xbe2-rw profiles

# Profile detail (normal + shift mappings, curves, colors)
sudo xbe2-rw profile 1

# Set LED color
sudo xbe2-rw color 1 ff0000

# Button remapping
sudo xbe2-rw remap 1 A=B B=A          # swap A and B
sudo xbe2-rw remap-shift 1 A=LB       # A becomes LB in shift mode
sudo xbe2-rw remap-reset 1             # reset to default

# Dead zones and vibration
sudo xbe2-rw deadzone 1 10 10 5 5
sudo xbe2-rw vibration 1 48 48

# Rumble test
sudo xbe2-rw rumble 50 50 0 0
sudo xbe2-rw rumble-stop

# Live LED preview
sudo xbe2-rw led 0000ff
sudo xbe2-rw led-off
```

### GUI

```bash
xbelite2-gui
```

Connects to the daemon at `/run/xbelite2.sock`.

## Architecture

```
Controller (BT/USB)
    |
    v
xbelite2.ko (kernel module)
    |
    +--> /dev/xbelite2      (USB GIP ring buffer)
    +--> /dev/xbelite2_bt   (BT HID ring buffer)
    |
    v
xbelite2d (daemon)
    |
    +--> parse input reports
    +--> apply profile transforms
    +--> emit to /dev/uinput virtual gamepad
    +--> forward force-feedback/rumble to controller
    |
    +--> /run/xbelite2.sock (IPC)
            |
            v
      xbelite2-gui / xbe2-rw
            |
            v
  ~/.config/xbelite2/elite2.json
```

### Workspace layout

```
xboxelite2/
  daemon/       xbelite2d daemon + library
  gip/          shared GIP protocol library
  xbe2-rw/      CLI config tool
  gui/          Qt6/QML GUI
  kmod/         kernel module (C + Rust)
  pkg/          Arch Linux packaging
  docs/         protocol documentation
```

## Protocol

See [docs/protocol.md](docs/protocol.md) for the reverse-engineered GIP protocol reference.

## License

GPL-2.0-only
