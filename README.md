# xbelite2

Linux driver and configuration tool for the Xbox Elite Wireless Controller Series 2.

![Xbox Elite Series 2](docs/elite2.png)

## What this does

The Xbox Elite 2 stores its configuration (button remaps, stick curves, dead zones, LED colors, vibration) directly on the controller hardware across 3 profiles. On Linux, the stock drivers (`xpad`, `hid-generic`) don't expose paddles properly, and there's no way to read or write the controller's hardware config without the Windows Xbox Accessories app.

This project gives you full control over the Elite 2 on Linux:

- A **kernel module** that claims the controller on USB and BT, providing clean character devices for userspace
- A **daemon** that translates raw controller input into a standard virtual gamepad, forwards rumble, and handles paddle suppression for hardware-remapped profiles
- A **CLI tool** that reads and writes the controller's hardware profiles directly over the GIP protocol (USB)
- A **BT test tool** for testing rumble and dumping raw BT HID reports
- A **GUI** for live input display and profile management

## Components

| Component | Description |
|-----------|-------------|
| **xbelite2** (kmod) | DKMS kernel module. Claims the controller on USB and BT, provides `/dev/xbelite2` (USB) and `/dev/xbelite2_bt` (BT) character devices |
| **xbelite2d** (daemon) | Reads input from the kernel module, emits a virtual gamepad via uinput, forwards force-feedback/rumble to the controller. Caches hardware profiles from USB to suppress duplicate paddle events in BT mode |
| **xbe2-rw** (CLI) | Reads and writes controller hardware config over USB: profiles, button remaps (normal + shift), LED colors, dead zones, stick curves, vibration, device name |
| **xbe2-bt** (CLI) | BT testing tool: rumble, raw report dumping |
| **xbelite2-gui** (GUI) | Qt6/QML app for live input display and profile management |

### What you can configure on the controller hardware

All configuration is stored on the controller itself and persists across reboots. Changes are made over USB via the GIP protocol using `xbe2-rw` or the GUI:

- Button remapping (normal and shift/alternate mode per profile)
- Profile LED colors (RGB)
- Stick dead zones and response curves
- Vibration motor intensity
- Device name (the Bluetooth advertised name)

### How profiles work

The Elite 2 has a physical profile switch with 4 positions:

- **Profile 0 (Default)** — no hardware remaps, paddles report as paddles
- **Profiles 1, 2, 3** — each has its own button remaps, curves, dead zones, and LED color stored on the controller

The controller handles all remapping in its firmware. The daemon's job is to:
1. Read raw input reports and emit them as a standard Linux gamepad
2. Forward rumble/force-feedback from games to the controller
3. Suppress paddle events when paddles are hardware-remapped (to avoid duplicate inputs)

When connected via USB, the daemon reads and caches the hardware profile config. When the controller later connects via BT, the daemon uses this cache to know which paddles are remapped.

## Installation

### Arch Linux (AUR)

```
yay -S xbelite2-dkms
```

Installs the kernel module (DKMS), daemon, CLI tools, GUI, udev rules, and systemd service.

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

### CLI tool (USB)

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

# Button remapping (stored on controller hardware)
sudo xbe2-rw remap 1 A=B B=A          # swap A and B
sudo xbe2-rw remap-shift 1 A=LB       # A becomes LB in shift mode
sudo xbe2-rw remap-reset 1             # reset to default

# Dead zones and vibration
sudo xbe2-rw deadzone 1 10 10 5 5
sudo xbe2-rw vibration 1 48 48

# Rumble test
sudo xbe2-rw rumble 50 50 0 0
sudo xbe2-rw rumble-stop

# Stick curves
sudo xbe2-rw curves 1 reset

# Live LED preview
sudo xbe2-rw led 0000ff
sudo xbe2-rw led-off
```

### BT test tool

```bash
# Test rumble over Bluetooth
sudo xbe2-bt rumble 50 50 0 0
sudo xbe2-bt rumble-stop

# Dump raw BT HID reports
sudo xbe2-bt dump
```

### GUI

```bash
xbelite2-gui
```

## Architecture

```
Controller (BT/USB)
    |
    v
xbelite2.ko (kernel module)
    |
    +--> /dev/xbelite2      (USB GIP ring buffer, read + write)
    +--> /dev/xbelite2_bt   (BT HID ring buffer, read + write)
    |
    +---------------------------+
    |                           |
    v                           v
xbelite2d (daemon)         xbe2-rw / xbe2-bt (CLI tools)
    |                           |
    +--> parse input            +--> read/write hardware profiles
    +--> suppress remapped      +--> LED color, rumble, name
         paddles                +--> button remaps, curves, deadzones
    +--> emit virtual gamepad
    +--> forward rumble
    |
    +--> /run/xbelite2.sock
            |
            v
      xbelite2-gui
```

### Workspace layout

```
xboxelite2/
  daemon/       xbelite2d daemon
  gip/          shared GIP protocol library
  xbe2-rw/      USB CLI config tool
  xbe2-bt/      BT CLI test tool
  gui/          Qt6/QML GUI
  kmod/         kernel module (C + Rust)
  pkg/          Arch Linux packaging
  docs/         protocol documentation
```

## Protocol

See [docs/protocol.md](docs/protocol.md) for the reverse-engineered GIP protocol reference.

## License

GPL-2.0-only
