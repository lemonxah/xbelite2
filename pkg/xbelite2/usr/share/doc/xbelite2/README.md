# xbelite2

Linux driver and configuration tools for the Xbox Elite Wireless Controller Series 2.

## What this does

The Xbox Elite 2 has four back paddles and a hardware profile switch. The stock
Linux drivers (`xpad`, `hid-generic`, `xpadneo`) don't expose the paddles
reliably, and there's no way to configure button remapping, stick response
curves, or trigger dead zones without the Windows Xbox Accessories app.

This project fixes that. It consists of three parts:

- **xbelite2.ko** — a kernel module that binds the Elite 2 over USB and
  Bluetooth, registers a native Linux input device (just like `xpad`), exposes
  the current hardware profile via sysfs, and provides a misc character device
  (`/dev/xbelite2`) for configuration access.
- **xbe2-rw** — a CLI tool that reads and writes the controller's hardware
  profiles (LED color, button remaps, dead zones, stick curves, stick
  inversion, vibration, device name, rumble test).
- **xbelite2-gui** — a Qt6/QML configuration app that shows live controller
  state and lets you edit profiles. It drives the same GIP protocol
  directly — no background service required.

### How profiles work

The Elite 2 has a physical profile switch with 4 positions:

- **Profile 0 (Default)** — pure passthrough. All inputs go straight through.
- **Profiles 1, 2, 3** — each stores a hardware configuration on the
  controller itself: button remaps, stick curves, dead zones, LED color,
  vibration. The controller applies them before reporting input.

Because the profiles live on the controller's own flash, your settings travel
with the controller between machines.

### What you can configure per profile

- Button and paddle remapping (normal + shift mode)
- Stick response curves
- Stick dead zones (LS, RS)
- Trigger dead zones (LT, RT)
- Stick axis inversion (LY, RY, LX, RX)
- LED color and brightness
- Vibration intensity (main + weak motor, trigger rumbles)
- Device name

## Installing

After the package installs the files:

```
sudo modprobe xbelite2
sudo usermod -aG input $USER    # log out/in to apply
```

The kernel module registers both a USB and a Bluetooth HID driver. The udev
rule installs `/dev/xbelite2` with `GROUP=input, MODE=0660` so users in the
`input` group can configure the controller without sudo.

## Running

### CLI

```
xbe2-rw help                            # list all commands
xbe2-rw read                            # device info + all profiles
xbe2-rw color 1 00aaff                  # profile 1 LED → #00AAFF
xbe2-rw remap 2 A=B Y=X                 # profile 2: swap A/B, remap Y → X
xbe2-rw invert 1 0x0f                   # profile 1: invert all 4 stick axes
xbe2-rw rumble 50 50                    # 500ms rumble at 50% left/right
```

### GUI

```
xbelite2-gui
```

Shows live controller state, the active hardware profile, and lets you edit
remaps / colors / inversion for profiles 1–3.

## Architecture

```
Controller (USB or BT)
    |
    v
xbelite2.ko (kernel module)
    |-- registers /dev/input/eventN (native input device)
    |-- registers /dev/xbelite2     (misc char device for GIP config)
    |-- exposes hw_profile via sysfs (auto-synced from controller)
    |
    +---> games see a normal Xbox gamepad
    |
    +---> xbe2-rw / xbelite2-gui   (open /dev/xbelite2, speak GIP)
                    |
                    v
            writes profile pages to the controller's flash
```

There is no userspace service or IPC layer — the kernel module handles input,
and configuration tools speak directly to the controller.

## License

GPL-2.0-only
