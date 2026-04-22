# Xbox Elite Series 2 — GIP Protocol Reference

Reverse-engineered from USB packet captures (firmware 5.17).
All multi-byte values are little-endian unless noted.

## USB Transport

- Vendor: `0x045E` (Microsoft), Product: `0x0B00`
- Interface 0, Protocol `0xD0` (GIP)
- EP 0x02 OUT (Interrupt, 64 bytes max, 4ms interval) — host to controller
- EP 0x82 IN (Interrupt) — controller to host

### GIP Frame Header (4 bytes)

```
[0] command
[1] flags
[2] sequence number
[3] payload length
```

Common flags:
- `0x00` — no flags
- `0x10` — vendor command (used with 0x4D)
- `0x20` — needs-ack / system command
- `0x30` — system + needs-ack (used with 0x1E)

### Write Quirk

`usb_interrupt_msg()` fails for packets > ~7 bytes on Linux.
Use async URB submission (`usb_fill_int_urb` + `usb_submit_urb` + completion callback) instead.
This matches how the xpad driver handles similar endpoints.

---

## Commands

### 0x01 — ACK

Sent by controller to acknowledge a command.

```
01 20 <seq> 09 00 <cmd> 00 <payload_len> 00 00 00 00 00
```

### 0x02 — HELLO

Sent by controller on connect. The host replies with power-on and init.

### 0x03 — STATUS

Controller reports status periodically.

```
03 20 <seq> 04 <status bytes...>
```

### 0x05 — POWER

```
05 20 <seq> 01 05
```

Sent by host after connect and before disconnect. Payload `05` = power on.

### 0x07 — GUIDE BUTTON

```
07 20 <seq> 01 <pressed>
```

`pressed`: `0x01` = down, `0x00` = up. Sent as a separate command, not part of the input report.

### 0x09 — RUMBLE

```
09 00 <seq> 09 00 0f <LMotor> <RMotor> <LTrigger> <RTrigger> ff 00 eb
```

4 motors, each 0–100:
- `LTrigger` — left trigger impulse motor
- `RTrigger` — right trigger impulse motor
- `LMotor` — left body rumble (big motor)
- `RMotor` — right body rumble (small motor)

The trailing `ff 00 eb` appears constant.

### 0x0E — LED COLOR (live preview)

```
0e 00 <seq> 05 <mode> 00 <R> <G> <B>
```

- `mode=0` — set color
- `mode=1, RGB=000000` — turn off / return to profile color

Not saved to the controller. Used by Xbox Accessories for the color picker animation.
Requires an UNLOCK (0x4D sub 0x03) to have been sent first.

### 0x0C — ELITE INPUT REPORT

Compact input report, 17-byte payload:

```
[0-1]   buttons (same layout as 0x20 input)
[2-3]   left trigger (u16 LE, 10-bit)
[4-5]   right trigger (u16 LE, 10-bit)
[6-7]   left stick X (i16 LE)
[8-9]   left stick Y (i16 LE)
[10-11] right stick X (i16 LE)
[12-13] right stick Y (i16 LE)
[14]    paddles bitmask (bit0=UR, bit1=LR, bit2=UL, bit3=LL)
[15]    hardware profile number (0-3)
[16]    trigger mode (constant 0x0a)
```

Sent alongside 0x20 input reports after extended mode is enabled.

### 0x1E — SYSTEM

Used for calibration and device info reads.

Sub-commands (payload byte 0):
- `0x02` — version info
- `0x03` — profile button press notification (controller to host, unsolicited)
- `0x05` — device name (read, via 0x1E)
- `0x0F` — stick calibration data

### 0x20 — INPUT REPORT

Standard input report. After extended mode init (0x4D sub 0x07), this becomes a 47-byte payload (51 total):

```
[0]     buttons byte 1: Menu(bit2) View(bit3) A(bit4) B(bit5) X(bit6) Y(bit7)
[1]     buttons byte 2: DUp(bit0) DDown(bit1) DLeft(bit2) DRight(bit3) LB(bit4) RB(bit5) LS(bit6) RS(bit7)
[2-3]   left trigger (u16 LE, 0–1023)
[4-5]   right trigger (u16 LE, 0–1023)
[6-7]   left stick X (i16 LE)
[8-9]   left stick Y (i16 LE)
[10-11] right stick X (i16 LE)
[12-13] right stick Y (i16 LE)
[14]    paddles bitmask (bit0=UR, bit1=LR, bit2=UL, bit3=LL)
[15-34] reserved (zeros)
[35-46] timestamps
```

Note: byte offsets above are relative to the payload (after the 4-byte GIP header).

---

## 0x4D — VENDOR COMMANDS

The main command for profile configuration. All profile reads/writes go through 0x4D.

### Sub 0x03 — UNLOCK / COMMIT

Dual-purpose command:
- **Before writes**: unlocks the controller for profile/LED/name writes
- **After writes**: persists changes to controller flash (must follow sub 0x07 re-init)

Without the post-write commit, changes are lost on controller reboot.

```
OUT: 4d 10 <seq> 01 03
IN:  4d 00 <seq> 02 03 00
```

### Sub 0x04 — WRITE DEVICE NAME

```
OUT: 4d 10 <seq> 21 04 <UTF-16LE name, 32 bytes, zero-padded>
IN:  4d 00 <seq> 02 04 00
```

Max 15 characters. Name is stored on the controller.

### Sub 0x05 — READ DEVICE NAME

```
OUT: 4d 10 <seq> 01 05
IN:  4d 00 <seq> 22 05 00 <UTF-16LE name, 32 bytes>
```

### Sub 0x07 — INIT EXTENDED REPORTS

Enables extended input reports (paddles, profile data in 0x20, and 0x0C reports).

```
OUT: 4d 10 <seq> 02 07 00
IN:  4d 00 <seq> 02 07 00
```

Must be sent during init. Without this, controller sends standard 18-byte input reports only.

### Sub 0x02 — READ PROFILE PAGE

```
OUT: 4d 10 <seq> 03 02 <page> <size>
IN:  4d <flags> <seq> <len> 02 00 <page> <size> <data...>
```

### Sub 0x01 — WRITE PROFILE PAGE

```
OUT: 4d 10 <seq> <3+size> 01 <page> <size> <data...>
IN:  4d 00 <seq> 03 01 00 <page>
```

Requires UNLOCK (sub 0x03) first. Xbox Accessories always writes all 4 pages (both slots, mapping + curves) when saving a profile.

---

## Profile Pages

Each profile has 4 pages across 2 slots:

| Profile | SlotA Mapping | SlotA Curves | SlotB Mapping | SlotB Curves |
|---------|--------------|-------------|--------------|-------------|
| 1       | 0x20         | 0x21        | 0x26         | 0x27        |
| 2       | 0x22         | 0x23        | 0x28         | 0x29        |
| 3       | 0x24         | 0x25        | 0x2A         | 0x2B        |

SlotA = normal mode. SlotB = shift/alternate mapping (activated by holding a shift button).

### Mapping Page (56 bytes)

```
[0]     flags (bitmask):
          bit 0 (0x01): shift modifier assigned
          bit 2 (0x04): has button/paddle remaps
          bit 4 (0x10): unmodified (default state)
          Common values: 0x11=default, 0x00=modified, 0x01=shift, 0x04=remapped, 0x05=remap+keyboard
[1-4]   paddle outputs: [P1, P2, P3, P4] — what each paddle sends (default: [A, B, X, Y])
[5-8]   face button outputs: [A, B, X, Y] — what each face button sends (default: [A, B, X, Y])
[9-16]  extended remap: [DUp, DDown, DLeft, DRight, LB, RB, LStick, RStick]
[17-27] keyboard/special remap data:
          [17]    keyboard remap source button GIP code (0 if none)
          [18-27] additional remap metadata (zeros when unused)
[28-31] dead zones: [LStick, RStick, LTrigger, RTrigger] (0-255)
[32-44] trigger/stick ranges:
          [32-33] LT max range (u16 LE, 0xFF=100%, 0xAB=67%)
          [34-35] LT min range (u16 LE)
          [36-37] padding
          [38-39] RT max range (u16 LE)
          [40-41] RT min range (u16 LE)
          [42-43] padding
          [44]    LED brightness (0-100, default 0x64=100)
[45]    color flag: 0xFF = default (white), 0x00 = custom
[46]    color R
[47]    color G
[48]    color B
[49]    vibration left (0-100, default 0x30=48)
[50]    vibration right (0-100, default 0x30=48)
[51-54] reserved
[55]    keyboard mode flag (0x80 = keyboard key mapped, 0x00 = normal)
```

#### Button Remap Codes

| Code | Button |
|------|--------|
| 0x00 | None/Disabled |
| 0x04 | A |
| 0x05 | B |
| 0x06 | X |
| 0x07 | Y |
| 0x08 | D-pad Up |
| 0x09 | D-pad Down |
| 0x0A | D-pad Left |
| 0x0B | D-pad Right |
| 0x0C | LB |
| 0x0D | RB |
| 0x0E | Left Stick Click |
| 0x0F | Right Stick Click |

Note: these remap codes differ from the GIP input report bit positions!
LT and RT are NOT remappable through profile data.

Default paddle mapping: `[0x04, 0x05, 0x06, 0x07]` (P1→A, P2→B, P3→X, P4→Y).
Default face mapping: `[0x04, 0x05, 0x06, 0x07]` (A→A, B→B, X→X, Y→Y).
Default ext mapping: `[0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F]` (DUp, DDown, DLeft, DRight, LB, RB, LStick, RStick).

#### Extended Remap Slots (bytes 9-16)

The 8 ext slots are indexed positions, not fixed to specific buttons. The default mapping assigns each slot to its corresponding button, but any slot can hold any button code (or 0x00 for disabled).

When a button is assigned as the **shift modifier**, it is removed from ext[] and everything shifts up to fill the gap. Slot 7 becomes 0x00 (NONE). The "missing" button is the shift button.

Example: If LB is the shift modifier:
- Default ext: `[LB, RB, LT, RT, DUp, DDown, DLeft, DRight]`
- After shift: `[RB, LT, RT, DUp, DDown, DLeft, DRight, NONE]`
- LB is removed, everything shifts up, flags gets bit 0 set (0x01)

#### Paddle Remapping

Paddles are remapped through the face[] array, not through separate entries. When a paddle is bound to a face button action, it appears as the GIP code in the corresponding face slot. The flags byte bit 2 (0x04) indicates paddle remaps are present.

### Curves Page (43 bytes)

```
[0]    flags (same as mapping page)
[1-6]  left stick X curve (3 control points)
[7-12] left stick Y curve
[13-18] right stick X curve
[19-24] right stick Y curve
[25-26] reserved
[27]   stick inversion bitmask:
         bit 0: left stick Y inverted
         bit 1: right stick Y inverted
[28-42] reserved (zeros)
```

Each curve is 6 bytes: 3 control points as `[x1, y1, x2, y2, x3, y3]`.
Default linear curve: `[2B, 2B, 7F, 7F, BF, BF]` — points at (43,43), (127,127), (191,191).

---

## Init Sequence

On USB connect, the controller sends a HELLO (0x02). The host should respond with the full init sequence (matches Linux kernel xpad.c):

1. Power on: `05 20 00 01 00`
2. BT→USB transition: `05 20 00 0F 06` (required for Elite 2 / Xbox One S)
3. Extended reports: `4D 10 01 02 07 00` (enables paddles + profile in reports)
4. LED on: `0A 20 00 03 00 01 14`
5. Auth done: `06 20 00 02 01 00`

After this the controller sends extended 0x20 (51-byte) and 0x0C (21-byte) input reports.
The keepalive (rumble stop command `09 00 00 09 00 0F 00 00 00 00 FF 00 EB`) should be sent every ~2 seconds to prevent HELLO re-sends.

## Disconnect Sequence

On USB disconnect, to allow automatic Bluetooth reconnection:

1. USB→BT transition: `05 20 00 0F 00` (reverse of BT→USB, allows BT reconnect without re-pairing)

Without this command, the controller firmware remains in USB mode after cable unplug and requires full Bluetooth re-pairing.

## Write Sequence

Before any profile, LED, or name writes:

1. Send UNLOCK: `4D 10 <seq> 01 03` (can send 1–3 times)
2. Perform writes (all 4 pages for a profile: mapping A, mapping B, curves A, curves B)
3. Re-init extended reports: `4D 10 <seq> 02 07 00`
4. Send COMMIT: `4D 10 <seq> 01 03` (same as unlock — persists to flash)
5. Send POWER reload: `05 20 <seq> 01 05` (makes controller reload profile from flash)
6. Optionally read back to verify

---

## BT HID Input Report (report ID 0x01, 20 bytes)

```
[0]    report ID (0x01)
[1]    buttons 0: A(bit0) B(bit1) X(bit3) Y(bit4) LB(bit6) RB(bit7)
[2]    buttons 1: hat(bits0-3) View(bit2) Menu(bit3) LS(bit5) RS(bit6)
[3-4]  left trigger (u16 LE, masked 0x03FF)
[5-6]  right trigger (u16 LE, masked 0x03FF)
[7-8]  left stick X (i16 LE)
[9-10] left stick Y (i16 LE)
[11-12] right stick X (i16 LE)
[13-14] right stick Y (i16 LE)
[15-16] unknown
[17]   profile (bits 0-1)
[18]   trigger mode (0x0a)
[19]   paddles bitmask (bit0=UR, bit1=LR, bit2=UL, bit3=LL)
```
