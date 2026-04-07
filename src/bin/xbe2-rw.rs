// GIP read/write tool for Xbox Elite Series 2.
// Communicates through /dev/xbelite2 (kernel misc device).
// Supports: device name, profile config, button remapping, dead zones, LED color.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

const DEV_PATH: &str = "/dev/xbelite2";

// Profile page layout:
//   Profile 1: mapping 0x20/0x26, curves 0x21/0x27
//   Profile 2: mapping 0x22/0x28, curves 0x23/0x29
//   Profile 3: mapping 0x24/0x2a, curves 0x25/0x2b
const PROFILE_MAPPING_PAGES: [[u8; 2]; 3] = [[0x20, 0x26], [0x22, 0x28], [0x24, 0x2a]];
const PROFILE_CURVES_PAGES: [[u8; 2]; 3] = [[0x21, 0x27], [0x23, 0x29], [0x25, 0x2b]];
const MAPPING_SIZE: u8 = 0x38; // 56 bytes
const CURVES_SIZE: u8 = 0x2b;  // 43 bytes

// Button remap codes (from protocol RE)
const BTN_A: u8 = 0x04;
const BTN_B: u8 = 0x05;
const BTN_X: u8 = 0x06;
const BTN_Y: u8 = 0x07;
const BTN_LB: u8 = 0x08;
const BTN_RB: u8 = 0x09;
const BTN_LT: u8 = 0x0A;
const BTN_RT: u8 = 0x0B;
const BTN_DUP: u8 = 0x0C;
const BTN_DDOWN: u8 = 0x0D;
const BTN_DLEFT: u8 = 0x0E;
const BTN_DRIGHT: u8 = 0x0F;

fn btn_name(code: u8) -> &'static str {
    match code {
        0x04 => "A",
        0x05 => "B",
        0x06 => "X",
        0x07 => "Y",
        0x08 => "LB",
        0x09 => "RB",
        0x0A => "LT",
        0x0B => "RT",
        0x0C => "DUp",
        0x0D => "DDown",
        0x0E => "DLeft",
        0x0F => "DRight",
        _ => "?",
    }
}

fn btn_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "a" => Some(BTN_A),
        "b" => Some(BTN_B),
        "x" => Some(BTN_X),
        "y" => Some(BTN_Y),
        "lb" => Some(BTN_LB),
        "rb" => Some(BTN_RB),
        "lt" => Some(BTN_LT),
        "rt" => Some(BTN_RT),
        "dup" | "up" => Some(BTN_DUP),
        "ddown" | "down" => Some(BTN_DDOWN),
        "dleft" | "left" => Some(BTN_DLEFT),
        "dright" | "right" => Some(BTN_DRIGHT),
        _ => None,
    }
}

// Mapping page layout (56 bytes, 0-indexed after stripping sub/idx/page/size header):
// [0]     flags (0x11=default, 0x10=customized)
// [1..5]  remap slot A: face buttons [A, B, X, Y]
// [5..9]  remap slot B: face buttons [A, B, X, Y]
// [9..17] extended remap: [LB, RB, LT, RT, DUp, DDown, DLeft, DRight]
// [17..29] padding/reserved
// [29..33] dead zones: [LStick, RStick, LTrigger, RTrigger] (0-100)
// [33..44] trigger/stick ranges
// [44]    color flag: 0xFF=default, 0x00=custom
// [45..48] RGB color + padding
// [48]    unknown
// [49..51] vibration motor levels
// [51..56] padding
const OFF_FLAGS: usize = 0;
const OFF_REMAP_A: usize = 1;
const OFF_REMAP_B: usize = 5;
const OFF_REMAP_EXT: usize = 9;
const OFF_DEADZONES: usize = 29;
const OFF_COLOR_FLAG: usize = 44;
const OFF_COLOR_R: usize = 45;
const OFF_COLOR_G: usize = 46;
const OFF_COLOR_B: usize = 47;
const OFF_VIBRATION: usize = 49;

/// Handle to /dev/xbelite2 for bidirectional GIP communication.
struct GipDev {
    file: File,
}

impl GipDev {
    fn open() -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEV_PATH)
            .unwrap_or_else(|e| {
                eprintln!("Failed to open {DEV_PATH}: {e}");
                eprintln!("Is the controller connected and xbelite2 module loaded?");
                std::process::exit(1);
            });
        // Set non-blocking for reads
        unsafe {
            let fd = file.as_raw_fd();
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let mut dev = GipDev { file };
        dev.drain();
        dev
    }

    /// Write a raw GIP packet to the controller.
    fn send(&mut self, pkt: &[u8]) {
        self.file.write_all(pkt).unwrap_or_else(|e| {
            eprintln!("Write failed: {e}");
        });
    }

    /// Read one frame from the ring buffer (2-byte LE length + payload).
    /// Returns None on timeout.
    fn recv(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        loop {
            // Read 2-byte length prefix
            let mut len_buf = [0u8; 2];
            match self.file.read(&mut len_buf) {
                Ok(2) => {
                    let frame_len = u16::from_le_bytes(len_buf) as usize;
                    if frame_len == 0 || frame_len > 512 {
                        continue;
                    }
                    let mut buf = vec![0u8; frame_len];
                    // Read the full payload — may need multiple reads
                    let mut read_so_far = 0;
                    while read_so_far < frame_len {
                        match self.file.read(&mut buf[read_so_far..]) {
                            Ok(n) if n > 0 => read_so_far += n,
                            Ok(_) => break,
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                if Instant::now() > deadline {
                                    return None;
                                }
                                std::thread::sleep(Duration::from_millis(1));
                            }
                            Err(_) => return None,
                        }
                    }
                    if read_so_far == frame_len {
                        return Some(buf);
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return None,
            }
        }
    }

    /// Read a response matching a specific command byte, skipping input reports.
    fn recv_cmd(&mut self, want_cmd: u8, timeout: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.recv(remaining) {
                Some(frame) => {
                    if frame.is_empty() {
                        continue;
                    }
                    let cmd = frame[0];
                    // Skip input reports and ACKs
                    if cmd == 0x20 || cmd == 0x02 || cmd == 0x01 || cmd == 0x0C || cmd == 0x03 || cmd == 0x07 {
                        continue;
                    }
                    if cmd == want_cmd {
                        return Some(frame);
                    }
                }
                None => return None,
            }
        }
    }

    /// Drain all pending data from the ring buffer.
    fn drain(&mut self) {
        let mut buf = [0u8; 512];
        loop {
            match self.file.read(&mut buf) {
                Ok(n) if n > 0 => continue,
                _ => break,
            }
        }
    }

    /// Send a 0x4D vendor command and read the 0x4D response.
    fn gip_4d(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let seq = SEQ_4D.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![0x4D, 0x10, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt);
        self.recv_cmd(0x4D, Duration::from_millis(500))
    }

    /// Send a 0x1E system command and read the 0x1E response.
    fn gip_1e(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let seq = SEQ_1E.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![0x1E, 0x30, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt);
        self.recv_cmd(0x1E, Duration::from_millis(500))
    }

    /// Send a raw command (no response expected).
    fn gip_send(&mut self, cmd: u8, flags: u8, payload: &[u8]) {
        let seq = SEQ_1E.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![cmd, flags, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt);
    }
}

static SEQ_1E: AtomicU8 = AtomicU8::new(0x60);
static SEQ_4D: AtomicU8 = AtomicU8::new(0x80);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let mut dev = GipDev::open();

    match mode {
        "read" => cmd_read(&mut dev),
        "name" => {
            match args.get(2) {
                Some(new_name) => cmd_write_name(&mut dev, new_name),
                None => cmd_read_name(&mut dev),
            }
        }
        "profiles" => cmd_read_profiles(&mut dev),
        "profile" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            if idx == 0 || idx > 3 {
                eprintln!("Profile must be 1-3");
                return;
            }
            cmd_read_profile(&mut dev, idx);
        }
        "color" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let color = args.get(3).map(|s| s.as_str());
            if idx == 0 || idx > 3 || color.is_none() {
                eprintln!("Usage: xbe2-rw color <1-3> <RRGGBB>");
                return;
            }
            cmd_set_color(&mut dev, idx, color.unwrap());
        }
        "remap" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            if idx == 0 || idx > 3 || args.len() < 4 {
                eprintln!("Usage: xbe2-rw remap <1-3> <FROM=TO> [FROM=TO] ...");
                eprintln!("  Buttons: A B X Y LB RB LT RT DUp DDown DLeft DRight");
                eprintln!("  Example: xbe2-rw remap 1 A=B B=A    (swap A and B)");
                return;
            }
            let mappings: Vec<&str> = args[3..].iter().map(|s| s.as_str()).collect();
            cmd_remap(&mut dev, idx, &mappings);
        }
        "remap-reset" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            if idx == 0 || idx > 3 {
                eprintln!("Usage: xbe2-rw remap-reset <1-3>");
                return;
            }
            cmd_remap_reset(&mut dev, idx);
        }
        "deadzone" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            if idx == 0 || idx > 3 || args.len() < 7 {
                eprintln!("Usage: xbe2-rw deadzone <1-3> <lstick> <rstick> <ltrigger> <rtrigger>");
                eprintln!("  Values: 0-100");
                return;
            }
            let vals: Vec<u8> = args[3..7]
                .iter()
                .filter_map(|s| s.parse::<u8>().ok())
                .collect();
            if vals.len() != 4 {
                eprintln!("Need 4 values (0-100)");
                return;
            }
            cmd_set_deadzone(&mut dev, idx, vals[0], vals[1], vals[2], vals[3]);
        }
        "vibration" => {
            let idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let left: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(255);
            let right: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(255);
            if idx == 0 || idx > 3 {
                eprintln!("Usage: xbe2-rw vibration <1-3> <left 0-100> <right 0-100>");
                return;
            }
            cmd_set_vibration(&mut dev, idx, left, right);
        }
        "led" => {
            let color = args.get(2).map(|s| s.as_str());
            if color.is_none() {
                eprintln!("Usage: xbe2-rw led <RRGGBB>  (live preview, not saved)");
                return;
            }
            cmd_led_preview(&mut dev, color.unwrap());
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("Xbox Elite 2 GIP Read/Write Tool");
    println!();
    println!("Usage:");
    println!("  xbe2-rw read                              Deep read all info");
    println!("  xbe2-rw name                              Read device name");
    println!("  xbe2-rw name <text>                       Write device name (max 15 chars)");
    println!("  xbe2-rw profiles                          Read all 3 profiles (summary)");
    println!("  xbe2-rw profile <1-3>                     Read profile detail");
    println!("  xbe2-rw color <1-3> <RRGGBB>              Set profile LED color");
    println!("  xbe2-rw remap <1-3> <FROM=TO> ...         Remap buttons");
    println!("  xbe2-rw remap-reset <1-3>                 Reset button mapping to default");
    println!("  xbe2-rw deadzone <1-3> <LS> <RS> <LT> <RT>  Set dead zones (0-100)");
    println!("  xbe2-rw vibration <1-3> <left> <right>    Set vibration (0-100)");
    println!("  xbe2-rw led <RRGGBB>                      Live LED preview (not saved)");
    println!();
    println!("Buttons: A B X Y LB RB LT RT DUp DDown DLeft DRight");
}

// --- Commands ---

fn cmd_read_name(dev: &mut GipDev) {
    match read_device_name(dev) {
        Some(name) => println!("{name}"),
        None => println!("(failed to read name)"),
    }
}

fn cmd_write_name(dev: &mut GipDev, new_name: &str) {
    let old = read_device_name(dev).unwrap_or_default();
    println!("Current: \"{old}\"");

    let truncated: String = new_name.chars().take(15).collect();
    let u16s: Vec<u16> = truncated.encode_utf16().collect();
    let mut name_buf = vec![0u8; 32];
    for (i, c) in u16s.iter().enumerate() {
        let bytes = c.to_le_bytes();
        name_buf[i * 2] = bytes[0];
        name_buf[i * 2 + 1] = bytes[1];
    }

    let mut payload = vec![0x05];
    payload.extend_from_slice(&name_buf);
    let seq = SEQ_1E.fetch_add(1, Ordering::Relaxed);
    let mut pkt = vec![0x1E, 0x30, seq, payload.len() as u8];
    pkt.extend_from_slice(&payload);
    dev.send(&pkt);
    dev.drain();

    match read_device_name(dev) {
        Some(readback) => {
            println!("New:     \"{readback}\"");
            if readback == truncated {
                println!("OK");
            } else {
                println!("WARN: name didn't change");
            }
        }
        None => println!("Failed to verify"),
    }
}

fn cmd_read_profiles(dev: &mut GipDev) {
    for i in 0..3 {
        let profile_num = i + 1;
        let page = PROFILE_MAPPING_PAGES[i][0];
        if let Some(data) = read_profile_page(dev, page, MAPPING_SIZE) {
            print_profile_summary(profile_num, &data);
        } else {
            println!("Profile {profile_num}: (read failed)");
        }
    }
}

fn cmd_read_profile(dev: &mut GipDev, idx: usize) {
    let i = idx - 1;
    println!("Profile {idx}:");

    for (slot, label) in [(0, "SlotA"), (1, "SlotB")] {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        println!("\n  {label} Mapping (page 0x{page:02x}):");
        if let Some(data) = read_profile_page(dev, page, MAPPING_SIZE) {
            print_profile_mapping(&data);
        } else {
            println!("    (read failed)");
        }

        let page = PROFILE_CURVES_PAGES[i][slot];
        println!("  {label} Curves (page 0x{page:02x}):");
        if let Some(data) = read_profile_page(dev, page, CURVES_SIZE) {
            print_profile_curves(&data);
        } else {
            println!("    (read failed)");
        }
    }
}

fn cmd_remap(dev: &mut GipDev, idx: usize, mappings: &[&str]) {
    let i = idx - 1;

    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        let mut data = match read_profile_page(dev, page, MAPPING_SIZE) {
            Some(d) => d,
            None => {
                println!("Failed to read profile {idx} slot {slot}");
                return;
            }
        };

        for mapping in mappings {
            let parts: Vec<&str> = mapping.split('=').collect();
            if parts.len() != 2 {
                eprintln!("Invalid mapping: {mapping} (use FROM=TO)");
                return;
            }
            let from = match btn_code(parts[0]) {
                Some(c) => c,
                None => {
                    eprintln!("Unknown button: {}", parts[0]);
                    return;
                }
            };
            let to = match btn_code(parts[1]) {
                Some(c) => c,
                None => {
                    eprintln!("Unknown button: {}", parts[1]);
                    return;
                }
            };

            if from >= BTN_A && from <= BTN_Y {
                let offset = if slot == 0 { OFF_REMAP_A } else { OFF_REMAP_B };
                let btn_idx = (from - BTN_A) as usize;
                if btn_idx < 4 {
                    data[offset + btn_idx] = to;
                }
            } else if from >= BTN_LB && from <= BTN_DRIGHT {
                let btn_idx = (from - BTN_LB) as usize;
                if btn_idx < 8 {
                    data[OFF_REMAP_EXT + btn_idx] = to;
                }
            }
        }

        data[OFF_FLAGS] = 0x10;
        write_profile_page(dev, page, &data);
    }

    println!("Profile {idx} remapped:");
    for mapping in mappings {
        println!("  {mapping}");
    }
}

fn cmd_remap_reset(dev: &mut GipDev, idx: usize) {
    let i = idx - 1;
    let default_face = [BTN_A, BTN_B, BTN_X, BTN_Y];
    let default_ext = [BTN_LB, BTN_RB, BTN_LT, BTN_RT, BTN_DUP, BTN_DDOWN, BTN_DLEFT, BTN_DRIGHT];

    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        let mut data = match read_profile_page(dev, page, MAPPING_SIZE) {
            Some(d) => d,
            None => {
                println!("Failed to read profile {idx} slot {slot}");
                return;
            }
        };

        data[OFF_REMAP_A..OFF_REMAP_A + 4].copy_from_slice(&default_face);
        data[OFF_REMAP_B..OFF_REMAP_B + 4].copy_from_slice(&default_face);
        data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8].copy_from_slice(&default_ext);

        write_profile_page(dev, page, &data);
    }
    println!("Profile {idx} remapping reset to default");
}

fn cmd_set_color(dev: &mut GipDev, idx: usize, color_hex: &str) {
    let (r, g, b) = parse_color(color_hex);
    let i = idx - 1;

    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        let mut data = match read_profile_page(dev, page, MAPPING_SIZE) {
            Some(d) => d,
            None => {
                println!("Failed to read profile {idx} slot {slot}");
                return;
            }
        };

        data[OFF_COLOR_FLAG] = 0x00;
        data[OFF_COLOR_R] = r;
        data[OFF_COLOR_G] = g;
        data[OFF_COLOR_B] = b;

        write_profile_page(dev, page, &data);
    }

    send_led_color(dev, r, g, b);
    println!("Profile {idx} color set to #{r:02x}{g:02x}{b:02x}");
}

fn cmd_set_deadzone(dev: &mut GipDev, idx: usize, lstick: u8, rstick: u8, ltrigger: u8, rtrigger: u8) {
    let i = idx - 1;

    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        let mut data = match read_profile_page(dev, page, MAPPING_SIZE) {
            Some(d) => d,
            None => {
                println!("Failed to read profile {idx} slot {slot}");
                return;
            }
        };

        data[OFF_DEADZONES] = lstick.min(100);
        data[OFF_DEADZONES + 1] = rstick.min(100);
        data[OFF_DEADZONES + 2] = ltrigger.min(100);
        data[OFF_DEADZONES + 3] = rtrigger.min(100);

        write_profile_page(dev, page, &data);
    }
    println!("Profile {idx} dead zones: LS={lstick} RS={rstick} LT={ltrigger} RT={rtrigger}");
}

fn cmd_set_vibration(dev: &mut GipDev, idx: usize, left: u8, right: u8) {
    let i = idx - 1;

    for slot in 0..2 {
        let page = PROFILE_MAPPING_PAGES[i][slot];
        let mut data = match read_profile_page(dev, page, MAPPING_SIZE) {
            Some(d) => d,
            None => {
                println!("Failed to read profile {idx} slot {slot}");
                return;
            }
        };

        data[OFF_VIBRATION] = left.min(100);
        data[OFF_VIBRATION + 1] = right.min(100);

        write_profile_page(dev, page, &data);
    }
    println!("Profile {idx} vibration: left={left} right={right}");
}

fn cmd_led_preview(dev: &mut GipDev, color_hex: &str) {
    let (r, g, b) = parse_color(color_hex);
    send_led_color(dev, r, g, b);
    println!("LED set to #{r:02x}{g:02x}{b:02x} (not saved to profile)");
}

fn cmd_read(dev: &mut GipDev) {
    println!("--- Device Info ---\n");

    print!("Name: ");
    match read_device_name(dev) {
        Some(name) => println!("\"{name}\""),
        None => println!("(failed)"),
    }

    println!("\nCalibration:");
    let resp = dev.gip_1e(&[0x0F]);
    if let Some(data) = &resp {
        let payload = &data[4..];
        if payload.len() >= 22 {
            let vals: Vec<u16> = payload[2..]
                .chunks(2)
                .filter_map(|c| {
                    if c.len() == 2 {
                        Some(u16::from_le_bytes([c[0], c[1]]))
                    } else {
                        None
                    }
                })
                .collect();
            if vals.len() >= 4 {
                println!("  LX center: {} LY center: {}", vals[0], vals[1]);
                println!("  RX center: {} RY center: {}", vals[2], vals[3]);
            }
        }
    }

    println!("\nCapability:");
    let resp = dev.gip_4d(&[0x03]);
    if let Some(data) = &resp {
        println!("  {:02x?}", &data[4..]);
    }

    println!("\nProfiles:");
    cmd_read_profiles(dev);
}

// --- Profile page read/write ---

fn read_profile_page(dev: &mut GipDev, page: u8, size: u8) -> Option<Vec<u8>> {
    let resp = dev.gip_4d(&[0x02, page, size])?;
    let payload = &resp[4..];
    if payload.len() < 4 || payload[0] != 0x02 {
        return None;
    }
    Some(payload[4..].to_vec())
}

fn write_profile_page(dev: &mut GipDev, page: u8, data: &[u8]) {
    let size = data.len() as u8;
    let mut payload = vec![0x01, page, size];
    payload.extend_from_slice(data);
    dev.gip_4d(&payload);
}

// --- LED ---

fn send_led_color(dev: &mut GipDev, r: u8, g: u8, b: u8) {
    dev.gip_send(0x0E, 0x00, &[0x00, 0x00, r, g, b]);
    dev.drain();
}

// --- Device name ---

fn read_device_name(dev: &mut GipDev) -> Option<String> {
    let resp = dev.gip_4d(&[0x05])?;
    let payload = &resp[4..];
    if payload.len() < 4 {
        return None;
    }
    let name_bytes = &payload[2..];
    let u16s: Vec<u16> = name_bytes
        .chunks(2)
        .filter_map(|c| {
            if c.len() == 2 {
                Some(u16::from_le_bytes([c[0], c[1]]))
            } else {
                None
            }
        })
        .collect();
    Some(
        String::from_utf16_lossy(&u16s)
            .trim_end_matches('\0')
            .to_string(),
    )
}

// --- Helpers ---

fn parse_color(hex: &str) -> (u8, u8, u8) {
    let clean: String = hex
        .trim_start_matches('#')
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if clean.len() != 6 {
        eprintln!("Color must be 6 hex digits (RRGGBB), got: {hex}");
        std::process::exit(1);
    }
    let r = u8::from_str_radix(&clean[0..2], 16).unwrap();
    let g = u8::from_str_radix(&clean[2..4], 16).unwrap();
    let b = u8::from_str_radix(&clean[4..6], 16).unwrap();
    (r, g, b)
}

// --- Display helpers ---

fn print_profile_summary(num: usize, data: &[u8]) {
    if data.len() < 56 {
        println!("Profile {num}: (data too short)");
        return;
    }
    let flags = data[OFF_FLAGS];
    let remap = &data[OFF_REMAP_A..OFF_REMAP_A + 4];
    let dz = &data[OFF_DEADZONES..OFF_DEADZONES + 4];
    let color_default = data[OFF_COLOR_FLAG] == 0xFF;
    let (r, g, b) = (data[OFF_COLOR_R], data[OFF_COLOR_G], data[OFF_COLOR_B]);
    let vib = (data[OFF_VIBRATION], data[OFF_VIBRATION + 1]);
    let customized = flags != 0x11;

    print!("Profile {num}:");
    if customized {
        print!(" [custom]");
    } else {
        print!(" [default]");
    }
    print!(
        " face=[{} {} {} {}]",
        btn_name(remap[0]),
        btn_name(remap[1]),
        btn_name(remap[2]),
        btn_name(remap[3])
    );
    print!(" dz=[{} {} {} {}]", dz[0], dz[1], dz[2], dz[3]);
    if color_default {
        print!(" color=default");
    } else {
        print!(" color=#{r:02x}{g:02x}{b:02x}");
    }
    println!(" vib={},{}", vib.0, vib.1);
}

fn print_profile_mapping(data: &[u8]) {
    if data.len() < 56 {
        println!("    (too short: {} bytes)", data.len());
        return;
    }
    let flags = data[OFF_FLAGS];
    let remap_a = &data[OFF_REMAP_A..OFF_REMAP_A + 4];
    let remap_b = &data[OFF_REMAP_B..OFF_REMAP_B + 4];
    let ext = &data[OFF_REMAP_EXT..OFF_REMAP_EXT + 8];
    let dz = &data[OFF_DEADZONES..OFF_DEADZONES + 4];
    let color_default = data[OFF_COLOR_FLAG] == 0xFF;
    let (r, g, b) = (data[OFF_COLOR_R], data[OFF_COLOR_G], data[OFF_COLOR_B]);
    let vib = (data[OFF_VIBRATION], data[OFF_VIBRATION + 1]);

    println!("    flags: 0x{flags:02x}");
    println!(
        "    face remap A: {} {} {} {}",
        btn_name(remap_a[0]),
        btn_name(remap_a[1]),
        btn_name(remap_a[2]),
        btn_name(remap_a[3])
    );
    println!(
        "    face remap B: {} {} {} {}",
        btn_name(remap_b[0]),
        btn_name(remap_b[1]),
        btn_name(remap_b[2]),
        btn_name(remap_b[3])
    );
    println!(
        "    extended:     {} {} {} {} {} {} {} {}",
        btn_name(ext[0]),
        btn_name(ext[1]),
        btn_name(ext[2]),
        btn_name(ext[3]),
        btn_name(ext[4]),
        btn_name(ext[5]),
        btn_name(ext[6]),
        btn_name(ext[7])
    );
    println!("    dead zones:   LS={} RS={} LT={} RT={}", dz[0], dz[1], dz[2], dz[3]);
    if color_default {
        println!("    color:        default (white)");
    } else {
        println!("    color:        #{r:02x}{g:02x}{b:02x}");
    }
    println!("    vibration:    left={} right={}", vib.0, vib.1);
    println!("    raw: {:02x?}", data);
}

fn print_profile_curves(data: &[u8]) {
    if data.len() < 43 {
        println!("    (too short: {} bytes)", data.len());
        return;
    }
    let flags = data[0];
    println!("    flags: 0x{flags:02x}");
    let curve_data = &data[1..];
    for (i, name) in ["LStick X", "LStick Y", "RStick X", "RStick Y"]
        .iter()
        .enumerate()
    {
        let off = i * 6;
        if off + 6 <= curve_data.len() {
            let pts = &curve_data[off..off + 6];
            println!(
                "    {name}: [{:02x} {:02x} {:02x} {:02x} {:02x} {:02x}]",
                pts[0], pts[1], pts[2], pts[3], pts[4], pts[5]
            );
        }
    }
}
