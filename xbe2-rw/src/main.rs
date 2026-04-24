use xbelite2_gip::transport::GipDevice;
use xbelite2_gip::types::*;
use xbelite2_gip::{led, name, profile, rumble};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    if mode == "help" || mode == "--help" || mode == "-h" {
        print_usage();
        return;
    }

    let mut dev = GipDevice::open_usb().unwrap_or_else(|e| {
        eprintln!("Failed to open /dev/xbelite2: {e}");
        eprintln!("Is the controller connected and xbelite2 module loaded?");
        std::process::exit(1);
    });
    dev.unlock();
    run(mode, &args, &mut dev);
}

fn run(mode: &str, args: &[String], dev: &mut GipDevice) {
    match mode {
        "read" => cmd_read(dev),
        "name" => match args.get(2) {
            Some(n) => cmd_write_name(dev, n),
            None => cmd_read_name(dev),
        },
        "profiles" => cmd_read_profiles(dev),
        "profile" => {
            let idx = parse_profile_idx(args, 2);
            cmd_read_profile(dev, idx);
        }
        "color" => {
            let idx = parse_profile_idx(args, 2);
            let (r, g, b) = parse_color(args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: xbe2-rw color <1-3> <RRGGBB>");
                std::process::exit(1);
            }));
            profile::set_color(dev, idx, r, g, b);
            led::set_color(dev, r, g, b);
            println!("Profile {} color set to #{r:02x}{g:02x}{b:02x}", idx + 1);
        }
        "remap" => {
            let idx = parse_profile_idx(args, 2);
            if args.len() < 4 {
                eprintln!("Usage: xbe2-rw remap <1-3> <FROM=TO> ...");
                eprintln!("  Buttons: A B X Y LB RB LT RT DUp DDown DLeft DRight");
                std::process::exit(1);
            }
            let remaps = parse_remaps(&args[3..]);
            profile::remap_buttons(dev, idx, &remaps);
            println!("Profile {} remapped:", idx + 1);
            for (from, to) in &remaps {
                println!("  {}={}", from.name(), to.name());
            }
        }
        "remap-shift" => {
            let idx = parse_profile_idx(args, 2);
            if args.len() < 4 {
                eprintln!("Usage: xbe2-rw remap-shift <1-3> <FROM=TO> ...");
                std::process::exit(1);
            }
            let remaps = parse_remaps(&args[3..]);
            profile::remap_shift(dev, idx, &remaps);
            println!("Profile {} shift remapped:", idx + 1);
            for (from, to) in &remaps {
                println!("  {}={}", from.name(), to.name());
            }
        }
        "remap-reset" => {
            let idx = parse_profile_idx(args, 2);
            profile::reset_remaps(dev, idx);
            println!("Profile {} remapping reset to default", idx + 1);
        }
        "reset" => {
            let idx = parse_profile_idx(args, 2);
            profile::reset_profile(dev, idx);
            println!("Profile {} fully reset to factory default", idx + 1);
        }
        "rumble-intensity" | "deadzone" => {
            let idx = parse_profile_idx(args, 2);
            if args.len() < 7 {
                eprintln!("Usage: xbe2-rw rumble-intensity <1-3> <weak> <strong> <rt> <lt>");
                eprintln!("  Values are 0-100 per motor. Default 100.");
                eprintln!("  (This field was previously misnamed 'deadzone'.)");
                std::process::exit(1);
            }
            let vals: Vec<u8> = args[3..7].iter().filter_map(|s| s.parse().ok()).collect();
            if vals.len() != 4 {
                eprintln!("Need 4 numeric values (weak, strong, rt, lt)");
                std::process::exit(1);
            }
            profile::set_rumble_intensity(dev, idx, [vals[0], vals[1], vals[2], vals[3]]);
            println!(
                "Profile {} rumble intensity: weak={} strong={} rt={} lt={}",
                idx + 1, vals[0], vals[1], vals[2], vals[3]
            );
        }
        "saturation" => {
            let idx = parse_profile_idx(args, 2);
            if args.len() < 4 {
                match profile::get_saturation(dev, idx) {
                    Some(s) => println!(
                        "Profile {} saturation: LT={} LS={} RT={} RS={}",
                        idx + 1, s[0], s[1], s[2], s[3]
                    ),
                    None => eprintln!("Failed to read profile {}", idx + 1),
                }
            } else if args.len() >= 7 {
                let vals: Vec<u8> = args[3..7].iter().filter_map(|s| s.parse().ok()).collect();
                if vals.len() != 4 {
                    eprintln!("Need 4 u8 values: LT LS RT RS (each 0-255, 255 = full range)");
                    std::process::exit(1);
                }
                profile::set_saturation(dev, idx, [vals[0], vals[1], vals[2], vals[3]]);
                println!(
                    "Profile {} saturation: LT={} LS={} RT={} RS={}",
                    idx + 1, vals[0], vals[1], vals[2], vals[3]
                );
            } else {
                eprintln!("Usage: xbe2-rw saturation <1-3> [LT LS RT RS]");
                eprintln!("       (no values = read current; each 0-255, 255 = full analog, 0 = binary)");
                std::process::exit(1);
            }
        }
        "vibration" => {
            let idx = parse_profile_idx(args, 2);
            let left: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(48);
            let right: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(48);
            profile::set_vibration(dev, idx, left, right);
            println!("Profile {} vibration: left={left} right={right}", idx + 1);
        }
        "curves" => {
            let idx = parse_profile_idx(args, 2);
            if args.get(3).map(|s| s.as_str()) == Some("reset") {
                profile::reset_curves(dev, idx);
                println!("Profile {} curves reset to linear", idx + 1);
            } else {
                eprintln!("Usage: xbe2-rw curves <1-3> reset");
                std::process::exit(1);
            }
        }
        "invert" => {
            let idx = parse_profile_idx(args, 2);
            match args.get(3) {
                Some(m) => {
                    let mask: u8 = parse_invert_mask(m);
                    profile::set_stick_inversion(dev, idx, mask);
                    println!("Profile {} stick inversion mask set to 0x{:02x}", idx + 1, mask);
                }
                None => {
                    let mask = profile::get_stick_inversion(dev, idx).unwrap_or(0);
                    println!("{:#04x}", mask);
                }
            }
        }
        "led" => {
            let (r, g, b) = parse_color(args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: xbe2-rw led <RRGGBB>");
                std::process::exit(1);
            }));
            led::set_color(dev, r, g, b);
            println!("LED set to #{r:02x}{g:02x}{b:02x} (not saved)");
        }
        "led-off" => {
            led::off(dev);
            println!("LED returned to profile color");
        }
        "rumble" => {
            if args.len() < 3 {
                eprintln!("Usage: xbe2-rw rumble <left> <right> [ltrigger] [rtrigger] [duration_ms]");
                std::process::exit(1);
            }
            let lm: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let rm: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let lt: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let rt: u8 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
            let ms: u64 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(250);
            rumble::set(dev, lm, rm, lt, rt);
            println!("Rumble: LM={lm} RM={rm} LT={lt} RT={rt} ({ms}ms)");
            std::thread::sleep(std::time::Duration::from_millis(ms));
            rumble::stop(dev);
        }
        "rumble-stop" => {
            rumble::stop(dev);
            println!("Rumble stopped");
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("Xbox Elite 2 GIP Read/Write Tool");
    println!();
    println!("Usage:");
    println!("  xbe2-rw read                                  Device info + all profiles");
    println!("  xbe2-rw name                                  Read device name");
    println!("  xbe2-rw name <text>                           Write device name (max 15 chars)");
    println!("  xbe2-rw profiles                              Read all 3 profiles (summary)");
    println!("  xbe2-rw profile <1-3>                         Read profile detail");
    println!("  xbe2-rw color <1-3> <RRGGBB>                  Set profile LED color");
    println!("  xbe2-rw remap <1-3> <FROM=TO> ...             Remap buttons (normal mode)");
    println!("  xbe2-rw remap-shift <1-3> <FROM=TO> ...       Remap buttons (shift mode)");
    println!("  xbe2-rw remap-reset <1-3>                     Reset remaps to default");
    println!("  xbe2-rw rumble-intensity <1-3> <weak> <strong> <rt> <lt>   Per-motor rumble scale (0-100)");
    println!("  xbe2-rw saturation <1-3> [LT LS RT RS]       Read/set per-axis saturation (0-255, 255=full analog, 0=binary)");
    println!("  xbe2-rw vibration <1-3> <left> <right>        Set vibration (0-100)");
    println!("  xbe2-rw curves <1-3> reset                    Reset stick curves to linear");
    println!("  xbe2-rw invert <1-3> [mask]                   Get/set stick inversion mask");
    println!("                                                 bit0=LY bit1=RY bit2=LX bit3=RX");
    println!("  xbe2-rw led <RRGGBB>                          Live LED preview (not saved)");
    println!("  xbe2-rw led-off                               Return LED to profile color");
    println!("  xbe2-rw rumble <LM> <RM> [LT] [RT] [MS]       Test rumble motors (0-100, default 250ms)");
    println!("  xbe2-rw rumble-stop                           Stop rumble");
    println!();
    println!("Buttons: A B X Y LB RB LT RT DUp DDown DLeft DRight");
}

fn cmd_read_name(dev: &mut GipDevice) {
    match name::read(dev) {
        Some(n) => println!("{n}"),
        None => println!("(failed)"),
    }
}

fn cmd_write_name(dev: &mut GipDevice, new_name: &str) {
    let old = name::read(dev).unwrap_or_default();
    println!("Current: \"{old}\"");
    match name::write(dev, new_name) {
        Some(readback) => {
            println!("New:     \"{readback}\"");
            if readback.starts_with(&new_name.chars().take(15).collect::<String>()) {
                println!("OK");
            }
        }
        None => println!("Failed to verify"),
    }
}

fn cmd_read_profiles(dev: &mut GipDevice) {
    for i in 0..3 {
        if let Some(m) = profile::read_mapping(dev, i, 0) {
            print_profile_summary(i + 1, &m);
        } else {
            println!("Profile {}: (read failed)", i + 1);
        }
    }
}

fn cmd_read_profile(dev: &mut GipDevice, idx: usize) {
    let p = profile::read_full(dev, idx);
    println!("Profile {}:", idx + 1);

    for (slot, label, mapping, curves) in [
        (0, "Normal (SlotA)", &p.mapping_a, &p.curves_a),
        (1, "Shift  (SlotB)", &p.mapping_b, &p.curves_b),
    ] {
        let page_m = PROFILE_MAPPING_PAGES[idx][slot];
        let page_c = PROFILE_CURVES_PAGES[idx][slot];
        println!("\n  {label}:");
        if let Some(m) = mapping {
            print_mapping(m, page_m);
        } else {
            println!("    mapping: (read failed)");
        }
        if let Some(c) = curves {
            print_curves(c, page_c);
        } else {
            println!("    curves: (read failed)");
        }
    }
}

fn cmd_read(dev: &mut GipDevice) {
    print!("Name: ");
    match name::read(dev) {
        Some(n) => println!("\"{n}\""),
        None => println!("(failed)"),
    }

    println!("\nCalibration:");
    if let Some(resp) = dev.system_cmd(&[0x0F]) {
        let payload = &resp[4..];
        if payload.len() >= 22 {
            let vals: Vec<u16> = payload[2..]
                .chunks(2)
                .filter_map(|c| {
                    if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None }
                })
                .collect();
            if vals.len() >= 4 {
                println!("  LX={} LY={} RX={} RY={}", vals[0], vals[1], vals[2], vals[3]);
            }
        }
    }

    println!("\nProfiles:");
    cmd_read_profiles(dev);
}

// --- Display ---

fn print_profile_summary(num: usize, m: &ProfileMapping) {
    let btn = |c: u8| GipButton::from_code(c).map(|b| b.name()).unwrap_or("?");
    print!("Profile {num}:");
    print!(" [{}]", if m.is_custom() { "custom" } else { "default" });
    print!(" face=[{},{},{},{}]", btn(m.face[0]), btn(m.face[1]), btn(m.face[2]), btn(m.face[3]));
    print!(" rumble=[w{},s{},rt{},lt{}]",
        m.rumble_intensity[0], m.rumble_intensity[1],
        m.rumble_intensity[2], m.rumble_intensity[3]);
    match m.color {
        Some((r, g, b)) => print!(" color=#{r:02x}{g:02x}{b:02x}"),
        None => print!(" color=default"),
    }
    println!(" vib={},{}", m.vibration.0, m.vibration.1);
}

fn print_mapping(m: &ProfileMapping, page: u8) {
    let btn = |c: u8| GipButton::from_code(c).map(|b| b.name()).unwrap_or("?");
    println!("    page 0x{page:02x}, flags=0x{:02x}", m.flags);
    println!("    face:     {} {} {} {}", btn(m.face[0]), btn(m.face[1]), btn(m.face[2]), btn(m.face[3]));
    println!("    paddles:  {} {} {} {}", btn(m.paddles[0]), btn(m.paddles[1]), btn(m.paddles[2]), btn(m.paddles[3]));
    println!(
        "    extended: {} {} {} {} {} {} {} {}",
        btn(m.ext[0]), btn(m.ext[1]), btn(m.ext[2]), btn(m.ext[3]),
        btn(m.ext[4]), btn(m.ext[5]), btn(m.ext[6]), btn(m.ext[7])
    );
    println!("    rumble intensity: weak={} strong={} RT={} LT={} (0-100)",
        m.rumble_intensity[0], m.rumble_intensity[1],
        m.rumble_intensity[2], m.rumble_intensity[3]);
    println!("    saturation: LT={} LS={} RT={} RS={} (255=full analog, 0=binary)",
        m.saturation[0], m.saturation[1], m.saturation[2], m.saturation[3]);
    match m.color {
        Some((r, g, b)) => println!("    color: #{r:02x}{g:02x}{b:02x}"),
        None => println!("    color: default"),
    }
    println!("    vibration: left={} right={}", m.vibration.0, m.vibration.1);
    print!("    raw (56 bytes):");
    for (i, b) in m.raw.iter().take(56).enumerate() {
        if i % 16 == 0 { print!("\n      [{i:2}]"); }
        print!(" {b:02x}");
    }
    println!();
}

fn print_curves(c: &ProfileCurves, page: u8) {
    println!("    page 0x{page:02x}, flags=0x{:02x}", c.flags);
    for (i, label) in ["LX", "LY", "RX", "RY"].iter().enumerate() {
        let pts = &c.curves[i];
        println!("    {label}: [{:02x} {:02x} {:02x} {:02x} {:02x} {:02x}]",
            pts[0], pts[1], pts[2], pts[3], pts[4], pts[5]);
    }
}

// --- Parsing helpers ---

fn parse_profile_idx(args: &[String], pos: usize) -> usize {
    let idx: usize = args.get(pos).and_then(|s| s.parse().ok()).unwrap_or(0);
    if idx == 0 || idx > 3 {
        eprintln!("Profile must be 1-3");
        std::process::exit(1);
    }
    idx - 1 // 0-indexed
}

fn parse_color(hex: &str) -> (u8, u8, u8) {
    let clean: String = hex.trim_start_matches('#').chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 6 {
        eprintln!("Color must be 6 hex digits (RRGGBB)");
        std::process::exit(1);
    }
    (
        u8::from_str_radix(&clean[0..2], 16).unwrap(),
        u8::from_str_radix(&clean[2..4], 16).unwrap(),
        u8::from_str_radix(&clean[4..6], 16).unwrap(),
    )
}

fn parse_invert_mask(s: &str) -> u8 {
    let clean = s.trim_start_matches("0x");
    u8::from_str_radix(clean, 16)
        .or_else(|_| s.parse::<u8>())
        .unwrap_or_else(|_| {
            eprintln!("Inversion mask must be 0-15 (decimal) or 0x00-0x0F (hex)");
            std::process::exit(1);
        })
}

fn parse_remaps(args: &[String]) -> Vec<(GipButton, GipButton)> {
    args.iter()
        .map(|s| {
            let parts: Vec<&str> = s.split('=').collect();
            if parts.len() != 2 {
                eprintln!("Invalid remap: {s} (use FROM=TO)");
                std::process::exit(1);
            }
            let from = GipButton::from_name(parts[0]).unwrap_or_else(|| {
                eprintln!("Unknown button: {}", parts[0]);
                std::process::exit(1);
            });
            let to = GipButton::from_name(parts[1]).unwrap_or_else(|| {
                eprintln!("Unknown button: {}", parts[1]);
                std::process::exit(1);
            });
            (from, to)
        })
        .collect()
}
