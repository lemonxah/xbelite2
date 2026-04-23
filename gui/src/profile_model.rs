//! QObject bridge for profile configuration.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, device_name)]
        #[qproperty(i32, hw_profile)]
        #[qproperty(i32, active_profile)]
        #[qproperty(bool, connected)]
        #[qproperty(i32, profile_count)]
        #[qproperty(QString, profile_name)]
        #[qproperty(i32, left_trigger_min)]
        #[qproperty(i32, left_trigger_max)]
        #[qproperty(i32, right_trigger_min)]
        #[qproperty(i32, right_trigger_max)]
        #[qproperty(i32, vibration_main)]
        #[qproperty(i32, vibration_weak)]
        #[qproperty(i32, vibration_lt)]
        #[qproperty(i32, vibration_rt)]
        #[qproperty(QString, left_stick_x_curve)]
        #[qproperty(QString, left_stick_y_curve)]
        #[qproperty(QString, right_stick_x_curve)]
        #[qproperty(QString, right_stick_y_curve)]
        #[qproperty(i32, left_stick_deadzone)]
        #[qproperty(i32, right_stick_deadzone)]
        // Connection mode
        #[qproperty(bool, is_usb)]
        #[qproperty(QString, profile_color)] // "#rrggbb" or "default"
        #[qproperty(i32, profile_brightness)] // 0-100
        // Live input state from controller
        #[qproperty(i32, live_buttons)]
        #[qproperty(i32, live_paddles)]
        #[qproperty(i32, live_lx)]
        #[qproperty(i32, live_ly)]
        #[qproperty(i32, live_rx)]
        #[qproperty(i32, live_ry)]
        #[qproperty(i32, live_lt)]
        #[qproperty(i32, live_rt)]
        type ProfileModel = super::ProfileModelRust;

        #[qinvokable]
        fn init_device(self: Pin<&mut ProfileModel>);

        #[qinvokable]
        fn select_profile(self: Pin<&mut ProfileModel>, index: i32);

        #[qinvokable]
        fn create_profile(self: Pin<&mut ProfileModel>, name: QString);

        #[qinvokable]
        fn delete_profile(self: Pin<&mut ProfileModel>);

        #[qinvokable]
        fn set_remap(self: Pin<&mut ProfileModel>, src_code: i32, dst_code: i32);

        #[qinvokable]
        fn remove_remap(self: Pin<&mut ProfileModel>, src_code: i32);

        #[qinvokable]
        fn get_remaps_json(self: &ProfileModel) -> QString;

        #[qinvokable]
        fn set_stick_curve(self: Pin<&mut ProfileModel>, axis: i32, points_json: QString);

        #[qinvokable]
        fn set_trigger_zone(self: Pin<&mut ProfileModel>, trigger: i32, min_val: i32, max_val: i32);

        #[qinvokable]
        fn set_vibration(self: Pin<&mut ProfileModel>, motor: i32, intensity: i32);

        #[qinvokable]
        fn save_profile(self: Pin<&mut ProfileModel>);

        #[qinvokable]
        fn get_profile_names(self: &ProfileModel) -> QString;

        #[qinvokable]
        fn set_hw_profile_mapping(self: Pin<&mut ProfileModel>, hw_profile: i32, sw_profile: i32);

        #[qinvokable]
        fn test_vibration(self: Pin<&mut ProfileModel>, motor: i32, intensity: i32);

        #[qinvokable]
        fn refresh_status(self: Pin<&mut ProfileModel>);

        #[qinvokable]
        fn set_stick_deadzone(self: Pin<&mut ProfileModel>, stick: i32, value: i32);

        #[qinvokable]
        fn test_all_vibration(self: Pin<&mut ProfileModel>);

        #[qinvokable]
        fn set_device_name_text(self: Pin<&mut ProfileModel>, name: QString);

        #[qinvokable]
        fn set_profile_color_hex(self: Pin<&mut ProfileModel>, hex: QString);

        #[qinvokable]
        fn read_hw_profile_color(self: Pin<&mut ProfileModel>);

        /// Remap a GIP button for both normal and shift mode on the current hw profile.
        /// Button names: A, B, X, Y, LB, RB, LT, RT, DUp, DDown, DLeft, DRight
        #[qinvokable]
        fn set_hw_remap(self: Pin<&mut ProfileModel>, src: QString, normal_dst: QString, shift_dst: QString);

        /// Set a button as the shift modifier for the current hw profile.
        #[qinvokable]
        fn set_shift_button(self: Pin<&mut ProfileModel>, btn: QString);

        #[qinvokable]
        fn set_profile_brightness_value(self: Pin<&mut ProfileModel>, brightness: i32);

        /// Set stick axis inversion. stick: 0=left, 1=right. axis: 0=X, 1=Y.
        #[qinvokable]
        fn set_stick_invert(self: Pin<&mut ProfileModel>, stick: i32, axis: i32, inverted: bool);

        /// Get stick inversion state as JSON: {"lx":false,"ly":false,"rx":false,"ry":true}
        #[qinvokable]
        fn get_stick_inversion(self: &ProfileModel) -> QString;

        /// Get the current profile's remap data as JSON for the GUI to display.
        /// Returns: {"normal": {"A":"B","X":"Y",...}, "shift": {"A":"LB",...}, "color": "#rrggbb"}
        #[qinvokable]
        fn get_hw_profile_info(self: &ProfileModel) -> QString;
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};

use crate::device_monitor;
use crate::evdev_reader::EvdevReader;
use crate::hw_config;
use crate::writer::{Writer, WriteOp};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    profiles: Vec<Profile>,
    hw_profile_map: [usize; 4],
    active_override: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            profiles: vec![
                Profile { name: "Profile 1".into(), ..Default::default() },
                Profile { name: "Profile 2".into(), ..Default::default() },
                Profile { name: "Profile 3".into(), ..Default::default() },
            ],
            hw_profile_map: [0, 0, 1, 2],
            active_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Profile {
    name: String,
    remaps: Vec<Remap>,
    stick_curves: [Option<Curve>; 4],
    trigger_zones: [Option<TZone>; 2],
    #[serde(default)]
    stick_deadzones: [u16; 2],
    vibration: Vib,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Remap {
    src: IId,
    dst: IId,
    scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IId {
    ev_type: u16,
    code: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Curve {
    points: [i16; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TZone {
    dead_min: u16,
    dead_max: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vib {
    main_motor: u8,
    weak_motor: u8,
    left_trigger: u8,
    right_trigger: u8,
}

impl Default for Vib {
    fn default() -> Self {
        Self { main_motor: 100, weak_motor: 100, left_trigger: 100, right_trigger: 100 }
    }
}

pub struct ProfileModelRust {
    device_name: QString,
    hw_profile: i32,
    active_profile: i32,
    connected: bool,
    profile_count: i32,
    profile_name: QString,
    left_trigger_min: i32,
    left_trigger_max: i32,
    right_trigger_min: i32,
    right_trigger_max: i32,
    vibration_main: i32,
    vibration_weak: i32,
    vibration_lt: i32,
    vibration_rt: i32,
    left_stick_x_curve: QString,
    left_stick_y_curve: QString,
    right_stick_x_curve: QString,
    right_stick_y_curve: QString,
    left_stick_deadzone: i32,
    right_stick_deadzone: i32,
    is_usb: bool,
    profile_color: QString,
    profile_brightness: i32,
    live_buttons: i32,
    live_paddles: i32,
    live_lx: i32,
    live_ly: i32,
    live_rx: i32,
    live_ry: i32,
    live_lt: i32,
    live_rt: i32,

    config: Config,
    sel_idx: usize,
    evdev: EvdevReader,
    /// Cached sysfs interface path (e.g. /sys/bus/usb/drivers/xbelite2/1-1:1.0).
    /// When set, fast-path polls read hw_profile from <path>/hw_profile without
    /// re-walking sysfs. Cleared when the path disappears to trigger re-detect.
    sysfs_path: Option<std::path::PathBuf>,
    /// In-memory snapshot of the 3 hardware profiles, read once on connect and
    /// updated optimistically on every successful write.
    hw_cache: xbelite2_gip::hw_profile::HwProfileCache,
    /// True once `hw_cache` has been populated for the current device.
    hw_cache_loaded: bool,
    /// Background writer thread. All hardware writes are posted here so the
    /// Qt main thread never blocks on USB I/O.
    writer: Writer,
}

impl Default for ProfileModelRust {
    fn default() -> Self {
        Self {
            device_name: QString::default(),
            hw_profile: 0,
            active_profile: 0,
            connected: false,
            profile_count: 3,
            profile_name: QString::default(),
            left_trigger_min: 0,
            left_trigger_max: 1023,
            right_trigger_min: 0,
            right_trigger_max: 1023,
            vibration_main: 100,
            vibration_weak: 100,
            vibration_lt: 100,
            vibration_rt: 100,
            left_stick_x_curve: QString::default(),
            left_stick_y_curve: QString::default(),
            right_stick_x_curve: QString::default(),
            right_stick_y_curve: QString::default(),
            left_stick_deadzone: 0,
            right_stick_deadzone: 0,
            is_usb: false,
            profile_color: QString::from("default"),
            profile_brightness: 100,
            live_buttons: 0,
            live_paddles: 0,
            live_lx: 0,
            live_ly: 0,
            live_rx: 0,
            live_ry: 0,
            live_lt: 0,
            live_rt: 0,
            config: Config::default(),
            sel_idx: 0,
            evdev: EvdevReader::new(),
            sysfs_path: None,
            hw_cache: xbelite2_gip::hw_profile::HwProfileCache::default(),
            hw_cache_loaded: false,
            writer: Writer::spawn(),
        }
    }
}

impl qobject::ProfileModel {
    fn init_device(mut self: Pin<&mut Self>) {
        let config = load_user_config();
        let cnt = config.profiles.len() as i32;
        self.as_mut().set_profile_count(cnt);
        self.as_mut().rust_mut().config = config.clone();

        let info = device_monitor::detect_controller();
        
        if info.connected {
            self.as_mut().set_connected(true);
            self.as_mut().set_is_usb(info.is_usb);
            self.as_mut().set_device_name(QString::from(&info.name));
            self.as_mut().set_hw_profile(info.hw_profile as i32);
            
            if let Some(event_path) = info.event_path {
                let _ = self.as_mut().rust_mut().evdev.open(&event_path);
            }
            
            if info.is_usb {
                if let Ok(name) = hw_config::read_device_name() {
                    self.as_mut().set_device_name(QString::from(&name));
                }
            }
            self.as_mut().rust_mut().sysfs_path = info.sysfs_path;
            self.as_mut().refresh_hw_cache();
            self.as_mut().update_profile_color_from_cache();
        } else {
            self.as_mut().set_connected(false);
            self.as_mut().set_device_name(QString::from(&info.name));
        }

        let idx = self.as_ref().rust().sel_idx;
        self.load_profile(idx);
    }

    fn select_profile(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().set_active_profile(index);
        self.as_mut().rust_mut().sel_idx = index as usize;
        self.load_profile(index as usize);
    }

    fn create_profile(mut self: Pin<&mut Self>, name: QString) {
        self.as_mut().rust_mut().config.profiles.push(Profile {
            name: name.to_string(),
            ..Default::default()
        });
        let cnt = self.rust().config.profiles.len() as i32;
        self.as_mut().set_profile_count(cnt);
        self.select_profile(cnt - 1);
    }

    fn delete_profile(mut self: Pin<&mut Self>) {
        let idx = self.rust().sel_idx;
        if self.rust().config.profiles.len() > 1 {
            self.as_mut().rust_mut().config.profiles.remove(idx);
            let cnt = self.rust().config.profiles.len() as i32;
            self.as_mut().set_profile_count(cnt);
            let ni = idx.min(self.rust().config.profiles.len() - 1);
            self.select_profile(ni as i32);
        }
    }

    fn set_remap(mut self: Pin<&mut Self>, src_code: i32, dst_code: i32) {
        let idx = self.rust().sel_idx;
        if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
            p.remaps.retain(|r| r.src.code != src_code as u16);
            p.remaps.push(Remap {
                src: IId { ev_type: 1, code: src_code as u16 },
                dst: IId { ev_type: 1, code: dst_code as u16 },
                scale: 1.0,
            });
        }
    }

    fn remove_remap(mut self: Pin<&mut Self>, src_code: i32) {
        let idx = self.rust().sel_idx;
        if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
            p.remaps.retain(|r| r.src.code != src_code as u16);
        }
    }

    fn get_remaps_json(&self) -> QString {
        let idx = self.rust().sel_idx;
        let json = self.rust().config.profiles.get(idx)
            .map(|p| serde_json::to_string(&p.remaps).unwrap_or_default())
            .unwrap_or_else(|| "[]".into());
        QString::from(&json)
    }

    fn set_stick_curve(mut self: Pin<&mut Self>, axis: i32, points_json: QString) {
        let idx = self.rust().sel_idx;
        let ai = axis as usize;
        if let Ok(pts) = serde_json::from_str::<Vec<i16>>(&points_json.to_string()) {
            if pts.len() == 16 && ai < 4 {
                let mut arr = [0i16; 16];
                arr.copy_from_slice(&pts);
                if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
                    p.stick_curves[ai] = Some(Curve { points: arr });
                }
            }
        }
    }

    fn set_trigger_zone(mut self: Pin<&mut Self>, trigger: i32, min_val: i32, max_val: i32) {
        let idx = self.rust().sel_idx;
        let t = trigger as usize;
        if t < 2 {
            if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
                p.trigger_zones[t] = Some(TZone { dead_min: min_val as u16, dead_max: max_val as u16 });
            }
            if t == 0 {
                self.as_mut().set_left_trigger_min(min_val);
                self.as_mut().set_left_trigger_max(max_val);
            } else {
                self.as_mut().set_right_trigger_min(min_val);
                self.as_mut().set_right_trigger_max(max_val);
            }
        }
    }

    fn set_vibration(mut self: Pin<&mut Self>, motor: i32, intensity: i32) {
        let idx = self.rust().sel_idx;
        let v = intensity.clamp(0, 100) as u8;
        if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
            match motor {
                0 => p.vibration.main_motor = v,
                1 => p.vibration.weak_motor = v,
                2 => p.vibration.left_trigger = v,
                3 => p.vibration.right_trigger = v,
                _ => {}
            }
        }
    }

    fn save_profile(self: Pin<&mut Self>) {
        let cfg = self.rust().config.clone();
        save_user_config(&cfg);
    }

    fn get_profile_names(&self) -> QString {
        let names: Vec<&str> = self.rust().config.profiles.iter().map(|p| p.name.as_str()).collect();
        QString::from(&serde_json::to_string(&names).unwrap_or_default())
    }

    fn set_hw_profile_mapping(mut self: Pin<&mut Self>, hw_profile: i32, sw_profile: i32) {
        let hw = hw_profile as usize;
        if hw < 4 {
            self.as_mut().rust_mut().config.hw_profile_map[hw] = sw_profile as usize;
        }
    }

    fn set_device_name_text(mut self: Pin<&mut Self>, name: QString) {
        if !self.rust().is_usb { return; }
        let name_str = name.to_string();
        // Optimistic UI update; hardware write happens off-thread.
        self.as_mut().set_device_name(QString::from(&name_str));
        self.rust().writer.send(WriteOp::SetDeviceName { name: name_str });
    }

    fn set_profile_color_hex(mut self: Pin<&mut Self>, hex: QString) {
        if !self.rust().is_usb { return; }
        let hw = self.rust().hw_profile as u8;
        if hw < 1 || hw > 3 { return; }

        let hex_str = hex.to_string();
        let clean = hex_str.trim_start_matches('#');
        if clean.len() != 6 { return; }
        let (r, g, b) = match (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            (Ok(r), Ok(g), Ok(b)) => (r, g, b),
            _ => return,
        };

        // Optimistic UI + cache update.
        let idx = (hw - 1) as usize;
        self.as_mut().rust_mut().hw_cache.profiles[idx].color = Some((r, g, b));
        self.as_mut().set_profile_color(hex);
        self.rust().writer.send(WriteOp::SetColor { profile_idx: idx, r, g, b });
    }

    fn read_hw_profile_color(mut self: Pin<&mut Self>) {
        self.as_mut().update_profile_color_from_cache();
    }

    fn get_hw_profile_info(&self) -> QString {
        // Returns the current hw profile's remap data as JSON for QML.
        // Backed by the in-memory cache populated on connect and updated
        // optimistically after writes.
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 || !self.rust().hw_cache_loaded {
            return QString::from("{}");
        }
        let idx = (hw - 1) as usize;
        let profile = &self.rust().hw_cache.profiles[idx];

        let btn_name = |code: u8| -> &str {
            xbelite2_gip::types::GipButton::from_code(code)
                .map(|b| b.name())
                .unwrap_or("?")
        };

        let default_face: [u8; 4] = [0x04, 0x05, 0x06, 0x07];
        let default_ext: [u8; 8] = [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F];
        let face_labels = ["A", "B", "X", "Y"];
        let ext_labels = ["DUp", "DDown", "DLeft", "DRight", "LB", "RB", "L Stick", "R Stick"];
        let paddle_labels = ["P1", "P2", "P3", "P4"];

        let mut normal_map = serde_json::Map::new();
        let mut shift_map = serde_json::Map::new();

        // Face buttons (bytes 5-8)
        for (i, &code) in profile.face.iter().enumerate() {
            if code != default_face[i] {
                normal_map.insert(face_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }
        // Extended (bytes 9-16)
        for (i, &code) in profile.ext.iter().enumerate() {
            if code != default_ext[i] {
                normal_map.insert(ext_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }
        // Paddles (bytes 1-4)
        for (i, &code) in profile.paddles.iter().enumerate() {
            if code != default_face[i] {
                normal_map.insert(paddle_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }

        // Shift mode (from SlotB page)
        for (i, &code) in profile.shift_face.iter().enumerate() {
            if code != default_face[i] {
                shift_map.insert(face_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }
        for (i, &code) in profile.shift_ext.iter().enumerate() {
            if code != default_ext[i] {
                shift_map.insert(ext_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }
        // Shift paddles (SlotB bytes 1-4)
        for (i, &code) in profile.shift_paddles.iter().enumerate() {
            if code != default_face[i] {
                shift_map.insert(paddle_labels[i].into(), serde_json::Value::String(btn_name(code).into()));
            }
        }

        let color_str = match profile.color {
            Some((r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
            None => "default".to_string(),
        };

        // Detect shift button: the button missing from ext[] (shifted out, 0x00 at end)
        let default_ext_set: std::collections::HashSet<u8> = default_ext.iter().copied().collect();
        let current_ext_set: std::collections::HashSet<u8> = profile.ext.iter().copied().filter(|&b| b != 0).collect();
        let shift_button = if profile.ext.iter().any(|&b| b == 0) {
            // Find which default button is missing
            default_ext_set.difference(&current_ext_set)
                .next()
                .and_then(|&code| xbelite2_gip::types::GipButton::from_code(code))
                .map(|b| b.name().to_string())
        } else {
            None
        };

        let result = serde_json::json!({
            "normal": normal_map,
            "shift": shift_map,
            "color": color_str,
            "shift_button": shift_button,
        });
        QString::from(&result.to_string())
    }

    fn set_profile_brightness_value(mut self: Pin<&mut Self>, brightness: i32) {
        self.as_mut().set_profile_brightness(brightness);
    }

    fn set_stick_invert(mut self: Pin<&mut Self>, stick: i32, axis: i32, inverted: bool) {
        if !self.rust().is_usb { return; }
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 { return; }
        let idx = (hw - 1) as usize;

        // bit0=LY, bit1=RY, bit2=LX, bit3=RX — matches gip::profile::set_stick_inversion
        let bit = match (stick, axis) {
            (0, 1) => 0, // left Y
            (1, 1) => 1, // right Y
            (0, 0) => 2, // left X
            (1, 0) => 3, // right X
            _ => return,
        };

        let mut mask = self.rust().hw_cache.profiles[idx].stick_inversion;
        if inverted {
            mask |= 1 << bit;
        } else {
            mask &= !(1 << bit);
        }

        // Optimistic cache update; hardware write off-thread.
        self.as_mut().rust_mut().hw_cache.profiles[idx].stick_inversion = mask;
        self.rust().writer.send(WriteOp::SetStickInversion { profile_idx: idx, mask });
    }

    fn get_stick_inversion(&self) -> QString {
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 || !self.rust().hw_cache_loaded {
            return QString::from("{\"ly\":false,\"ry\":false,\"lx\":false,\"rx\":false}");
        }
        let mask = self.rust().hw_cache.profiles[(hw - 1) as usize].stick_inversion;
        let result = serde_json::json!({
            "ly": mask & 0x01 != 0,
            "ry": mask & 0x02 != 0,
            "lx": mask & 0x04 != 0,
            "rx": mask & 0x08 != 0,
        });
        QString::from(&result.to_string())
    }

    fn set_shift_button(self: Pin<&mut Self>, btn: QString) {
        if !self.rust().is_usb { return; }
        let hw = self.rust().hw_profile as u8;
        if hw < 1 || hw > 3 { return; }
        
        // Shift button is configured via hardware profile settings
        // This would need a dedicated xbe2-rw command - for now, stub
        let _btn_str = btn.to_string();
        // TODO: Implement shift button configuration when xbe2-rw supports it
    }

    fn set_hw_remap(self: Pin<&mut Self>, src: QString, normal_dst: QString, shift_dst: QString) {
        if !self.rust().is_usb { return; }
        let hw = self.rust().hw_profile as u8;
        if hw < 1 || hw > 3 { return; }
        let idx = (hw - 1) as usize;

        let src_str = src.to_string();
        let normal_str = normal_dst.to_string();
        let shift_str = shift_dst.to_string();

        let src_btn = match xbelite2_gip::types::GipButton::from_name(&src_str) {
            Some(b) => b,
            None => return,
        };

        if !normal_str.is_empty() {
            if let Some(to) = xbelite2_gip::types::GipButton::from_name(&normal_str) {
                self.rust().writer.send(WriteOp::SetRemapNormal {
                    profile_idx: idx,
                    from: src_btn,
                    to,
                });
            }
        }

        if !shift_str.is_empty() {
            if let Some(to) = xbelite2_gip::types::GipButton::from_name(&shift_str) {
                self.rust().writer.send(WriteOp::SetRemapShift {
                    profile_idx: idx,
                    from: src_btn,
                    to,
                });
            }
        }
    }

    fn load_profile(mut self: Pin<&mut Self>, idx: usize) {
        let p = match self.rust().config.profiles.get(idx) {
            Some(p) => p.clone(),
            None => return,
        };
        self.as_mut().set_profile_name(QString::from(&p.name));

        let curves = [&p.stick_curves[0], &p.stick_curves[1], &p.stick_curves[2], &p.stick_curves[3]];
        let jsons: Vec<String> = curves.iter().map(|c| {
            match c {
                Some(c) => serde_json::to_string(&c.points).unwrap_or_default(),
                None => "null".into(),
            }
        }).collect();
        self.as_mut().set_left_stick_x_curve(QString::from(&jsons[0]));
        self.as_mut().set_left_stick_y_curve(QString::from(&jsons[1]));
        self.as_mut().set_right_stick_x_curve(QString::from(&jsons[2]));
        self.as_mut().set_right_stick_y_curve(QString::from(&jsons[3]));

        let (ltmn, ltmx) = p.trigger_zones[0].as_ref().map(|z| (z.dead_min as i32, z.dead_max as i32)).unwrap_or((0, 1023));
        let (rtmn, rtmx) = p.trigger_zones[1].as_ref().map(|z| (z.dead_min as i32, z.dead_max as i32)).unwrap_or((0, 1023));
        self.as_mut().set_left_trigger_min(ltmn);
        self.as_mut().set_left_trigger_max(ltmx);
        self.as_mut().set_right_trigger_min(rtmn);
        self.as_mut().set_right_trigger_max(rtmx);

        self.as_mut().set_vibration_main(p.vibration.main_motor as i32);
        self.as_mut().set_vibration_weak(p.vibration.weak_motor as i32);
        self.as_mut().set_vibration_lt(p.vibration.left_trigger as i32);
        self.as_mut().set_vibration_rt(p.vibration.right_trigger as i32);
        self.as_mut().set_left_stick_deadzone(p.stick_deadzones[0] as i32);
        self.as_mut().set_right_stick_deadzone(p.stick_deadzones[1] as i32);
    }

    fn test_vibration(self: Pin<&mut Self>, motor: i32, intensity: i32) {
        let i = intensity.clamp(0, 100) as u8;
        let (lm, rm, lt, rt) = match motor {
            0 => (i, 0, 0, 0),
            1 => (0, i, 0, 0),
            2 => (0, 0, i, 0),
            3 => (0, 0, 0, i),
            _ => return,
        };
        self.rust().writer.send(WriteOp::Rumble { lm, rm, lt, rt, duration_ms: 50 });
    }

    fn test_all_vibration(self: Pin<&mut Self>) {
        let lm = self.rust().vibration_main.clamp(0, 100) as u8;
        let rm = self.rust().vibration_weak.clamp(0, 100) as u8;
        let lt = self.rust().vibration_lt.clamp(0, 100) as u8;
        let rt = self.rust().vibration_rt.clamp(0, 100) as u8;
        self.rust().writer.send(WriteOp::Rumble { lm, rm, lt, rt, duration_ms: 50 });
    }

    fn refresh_status(mut self: Pin<&mut Self>) {
        // Fast path: we already have a bound device. Just read hw_profile
        // from the cached sysfs path (one file read) and poll evdev.
        let cached = self.rust().sysfs_path.clone();
        if let Some(path) = cached {
            match device_monitor::poll_hw_profile(&path) {
                Some(hw) => {
                    let hw_i = hw as i32;
                    if hw_i != self.rust().hw_profile {
                        self.as_mut().set_hw_profile(hw_i);
                        self.as_mut().update_profile_color_from_cache();
                    }
                    let input = self.as_mut().rust_mut().evdev.poll();
                    self.as_mut().set_live_buttons(input.buttons as i32);
                    self.as_mut().set_live_paddles(input.paddles as i32);
                    self.as_mut().set_live_lx(input.left_stick_x as i32);
                    self.as_mut().set_live_ly(input.left_stick_y as i32);
                    self.as_mut().set_live_rx(input.right_stick_x as i32);
                    self.as_mut().set_live_ry(input.right_stick_y as i32);
                    self.as_mut().set_live_lt(input.left_trigger as i32);
                    self.as_mut().set_live_rt(input.right_trigger as i32);
                    return;
                }
                None => {
                    // Device unbound — drop caches and fall through to re-detect.
                    self.as_mut().rust_mut().sysfs_path = None;
                    self.as_mut().rust_mut().evdev.device = None;
                }
            }
        }

        // Slow path: walk sysfs to find a bound device. Only runs when we
        // don't have a cached path (disconnected or just started).
        let info = device_monitor::detect_controller();
        if self.rust().connected != info.connected {
            self.as_mut().set_connected(info.connected);
        }
        if self.rust().is_usb != info.is_usb {
            self.as_mut().set_is_usb(info.is_usb);
        }
        let name_qs = QString::from(&info.name);
        if self.rust().device_name != name_qs {
            self.as_mut().set_device_name(name_qs);
        }
        if self.rust().hw_profile != info.hw_profile as i32 {
            self.as_mut().set_hw_profile(info.hw_profile as i32);
        }

        if info.connected {
            self.as_mut().rust_mut().sysfs_path = info.sysfs_path;
            if let Some(event_path) = info.event_path {
                if self.rust().evdev.device.is_none() {
                    let _ = self.as_mut().rust_mut().evdev.open(&event_path);
                }
            }
            if !self.rust().hw_cache_loaded {
                self.as_mut().refresh_hw_cache();
            }
            self.as_mut().update_profile_color_from_cache();
        } else {
            // Device went away — invalidate the cache so a fresh read happens on reconnect.
            self.as_mut().rust_mut().hw_cache_loaded = false;
        }
    }

    /// Update the QML-facing `profile_color` property from the in-memory cache.
    fn update_profile_color_from_cache(mut self: Pin<&mut Self>) {
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 || !self.rust().hw_cache_loaded {
            let def = QString::from("default");
            if self.rust().profile_color != def {
                self.as_mut().set_profile_color(def);
            }
            return;
        }
        let idx = (hw - 1) as usize;
        let color = self.rust().hw_cache.profiles[idx]
            .color
            .map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or_else(|| "default".to_string());
        let color_qs = QString::from(&color);
        if self.rust().profile_color != color_qs {
            self.as_mut().set_profile_color(color_qs);
        }
    }

    /// Open /dev/xbelite2 and read all 3 hardware profiles into the in-memory
    /// cache. Called once per connect; subsequent writes update the cache
    /// optimistically. USB only.
    fn refresh_hw_cache(mut self: Pin<&mut Self>) {
        if !self.rust().is_usb {
            self.as_mut().rust_mut().hw_cache_loaded = false;
            return;
        }
        match xbelite2_gip::transport::GipDevice::open_usb() {
            Ok(mut dev) => {
                dev.unlock();
                let cache = xbelite2_gip::hw_profile::read_from_controller(&mut dev);
                self.as_mut().rust_mut().hw_cache = cache;
                self.as_mut().rust_mut().hw_cache_loaded = true;
            }
            Err(e) => {
                eprintln!("refresh_hw_cache: failed to open /dev/xbelite2: {e}");
                self.as_mut().rust_mut().hw_cache_loaded = false;
            }
        }
    }

    fn set_stick_deadzone(mut self: Pin<&mut Self>, stick: i32, value: i32) {
        let idx = self.rust().sel_idx;
        let s = stick as usize;
        if s < 2 {
            if let Some(p) = self.as_mut().rust_mut().config.profiles.get_mut(idx) {
                p.stick_deadzones[s] = value.clamp(0, 50) as u16;
            }
            if s == 0 {
                self.as_mut().set_left_stick_deadzone(value);
            } else {
                self.as_mut().set_right_stick_deadzone(value);
            }
        }
    }
}

fn user_config_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(dir).join("xbelite2")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config/xbelite2")
    } else {
        std::path::PathBuf::from("/tmp/xbelite2")
    }
}

fn load_user_config() -> Config {
    let path = user_config_dir().join("elite2.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str::<Config>(&data) {
            return config;
        }
    }
    Config::default()
}

fn save_user_config(config: &Config) {
    let dir = user_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("elite2.json");
    if let Ok(data) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(&path, data);
    }
}
