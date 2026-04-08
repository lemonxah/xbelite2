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
        fn connect_daemon(self: Pin<&mut ProfileModel>);

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
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Req {
    GetStatus,
    GetConfig { device_id: String },
    SetConfig { device_id: String, config: Config },
    SetActiveProfile { device_id: String, profile_index: Option<usize> },
    TestVibration { device_id: String, motor: u8, intensity: u8 },
    TestAllVibration { device_id: String, intensities: [u8; 4] },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Resp {
    Status { devices: Vec<DevSt> },
    Config { config: Config },
    Ok,
    Error { message: String },
    ProfileList { profiles: Vec<String> },
}

#[derive(Debug, Serialize, Deserialize)]
struct DevSt {
    device_id: String,
    name: String,
    hw_profile: u8,
    active_profile: usize,
    connected: bool,
    #[serde(default)]
    is_usb: bool,
    #[serde(default)]
    buttons: u16,
    #[serde(default)]
    paddles: u8,
    #[serde(default)]
    left_stick_x: i16,
    #[serde(default)]
    left_stick_y: i16,
    #[serde(default)]
    right_stick_x: i16,
    #[serde(default)]
    right_stick_y: i16,
    #[serde(default)]
    left_trigger: u16,
    #[serde(default)]
    right_trigger: u16,
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
    live_buttons: i32,
    live_paddles: i32,
    live_lx: i32,
    live_ly: i32,
    live_rx: i32,
    live_ry: i32,
    live_lt: i32,
    live_rt: i32,

    config: Config,
    device_id: String,
    sel_idx: usize,
    // Persistent IPC connection for fast polling
    poll_conn: Option<UnixStream>,
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
            live_buttons: 0,
            live_paddles: 0,
            live_lx: 0,
            live_ly: 0,
            live_rx: 0,
            live_ry: 0,
            live_lt: 0,
            live_rt: 0,
            config: Config::default(),
            device_id: String::new(),
            sel_idx: 0,
            poll_conn: None,
        }
    }
}

impl qobject::ProfileModel {
    fn connect_daemon(mut self: Pin<&mut Self>) {
        // Load config from user's home directory
        let config = load_user_config();
        let cnt = config.profiles.len() as i32;
        self.as_mut().set_profile_count(cnt);
        self.as_mut().rust_mut().config = config.clone();

        // Try connecting to daemon
        let sp = sock_path();
        match ipc(&sp, &Req::GetStatus) {
            Ok(Resp::Status { devices }) => {
                if let Some(dev) = devices.first() {
                    let hw = dev.hw_profile as i32;
                    self.as_mut().set_device_name(QString::from(&dev.name));
                    self.as_mut().set_hw_profile(hw);
                    self.as_mut().set_connected(dev.connected);
                    self.as_mut().set_is_usb(dev.is_usb);
                    let did = dev.device_id.clone();
                    self.as_mut().rust_mut().device_id = did.clone();
                    let _ = ipc(&sp, &Req::SetConfig { device_id: did, config });

                    // Select the correct software profile based on HW profile
                    if hw >= 1 && hw <= 3 {
                        let sw_idx = (hw - 1) as usize;
                        self.as_mut().rust_mut().sel_idx = sw_idx;
                        self.as_mut().set_active_profile(hw);
                    } else {
                        self.as_mut().set_active_profile(0);
                    }
                } else {
                    self.as_mut().set_device_name(QString::from("No controller found"));
                    self.as_mut().set_connected(false);
                }
            }
            _ => {
                self.as_mut().set_device_name(QString::from("Daemon not running"));
                self.as_mut().set_connected(false);
            }
        }

        // Load the active profile's data
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
        // Save to user's home directory
        save_user_config(&cfg);
        // Send to daemon for immediate application
        let sp = sock_path();
        let did = self.rust().device_id.clone();
        let _ = ipc(&sp, &Req::SetConfig { device_id: did, config: cfg });
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
        // Write via GIP (runs in-process, needs USB)
        if let Ok(mut gip) = xbelite2_gip::transport::GipDevice::open_usb() {
            gip.unlock();
            if let Some(readback) = xbelite2_gip::name::write(&mut gip, &name_str) {
                self.as_mut().set_device_name(QString::from(&readback));
            }
        }
    }

    fn set_profile_color_hex(mut self: Pin<&mut Self>, hex: QString) {
        if !self.rust().is_usb { return; }
        let hex_str = hex.to_string();
        let clean: String = hex_str.trim_start_matches('#').chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if clean.len() != 6 { return; }
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0);

        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 { return; }
        let profile_idx = (hw - 1) as usize;

        if let Ok(mut gip) = xbelite2_gip::transport::GipDevice::open_usb() {
            gip.unlock();
            xbelite2_gip::profile::set_color(&mut gip, profile_idx, r, g, b);
            xbelite2_gip::led::set_color(&mut gip, r, g, b);
        }
        self.as_mut().set_profile_color(QString::from(&format!("#{:02x}{:02x}{:02x}", r, g, b)));
    }

    fn read_hw_profile_color(mut self: Pin<&mut Self>) {
        if !self.rust().is_usb {
            self.as_mut().set_profile_color(QString::from("default"));
            return;
        }
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 {
            self.as_mut().set_profile_color(QString::from("default"));
            return;
        }
        let profile_idx = (hw - 1) as usize;
        if let Ok(mut gip) = xbelite2_gip::transport::GipDevice::open_usb() {
            if let Some(mapping) = xbelite2_gip::profile::read_mapping(&mut gip, profile_idx, 0) {
                match mapping.color {
                    Some((r, g, b)) => {
                        self.as_mut().set_profile_color(QString::from(&format!("#{:02x}{:02x}{:02x}", r, g, b)));
                    }
                    None => {
                        self.as_mut().set_profile_color(QString::from("default"));
                    }
                }
            }
        }
    }

    fn set_hw_remap(self: Pin<&mut Self>, src: QString, normal_dst: QString, shift_dst: QString) {
        if !self.rust().is_usb { return; }
        let hw = self.rust().hw_profile;
        if hw < 1 || hw > 3 { return; }
        let profile_idx = (hw - 1) as usize;

        let src_btn = match xbelite2_gip::types::GipButton::from_name(&src.to_string()) {
            Some(b) => b,
            None => return,
        };
        let normal_btn = match xbelite2_gip::types::GipButton::from_name(&normal_dst.to_string()) {
            Some(b) => b,
            None => return,
        };
        let shift_btn = match xbelite2_gip::types::GipButton::from_name(&shift_dst.to_string()) {
            Some(b) => b,
            None => return,
        };

        if let Ok(mut gip) = xbelite2_gip::transport::GipDevice::open_usb() {
            gip.unlock();
            // Write normal mode remap (SlotA)
            xbelite2_gip::profile::remap_buttons(&mut gip, profile_idx, &[(src_btn, normal_btn)]);
            // Write shift mode remap (SlotB)
            xbelite2_gip::profile::remap_shift(&mut gip, profile_idx, &[(src_btn, shift_btn)]);
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
        let sp = sock_path();
        let did = self.rust().device_id.clone();
        if !did.is_empty() {
            let _ = ipc(&sp, &Req::TestVibration {
                device_id: did,
                motor: motor as u8,
                intensity: intensity as u8,
            });
        }
    }

    fn test_all_vibration(self: Pin<&mut Self>) {
        let sp = sock_path();
        let did = self.rust().device_id.clone();
        if !did.is_empty() {
            let intensities = [
                self.rust().vibration_main as u8,
                self.rust().vibration_weak as u8,
                self.rust().vibration_lt as u8,
                self.rust().vibration_rt as u8,
            ];
            let _ = ipc(&sp, &Req::TestAllVibration { device_id: did, intensities });
        }
    }

    fn refresh_status(mut self: Pin<&mut Self>) {
        // Use persistent connection for fast polling
        let resp = fast_poll(self.as_mut().rust_mut());
        if let Some(dev) = resp {
            let old_hw = self.rust().hw_profile;
            let new_hw = dev.hw_profile as i32;
            self.as_mut().set_hw_profile(new_hw);
            self.as_mut().set_connected(dev.connected);
            self.as_mut().set_is_usb(dev.is_usb);
            self.as_mut().set_device_name(QString::from(&dev.name));

            if old_hw != new_hw && new_hw >= 1 && new_hw <= 3 {
                let sw_idx = (new_hw - 1) as usize;
                self.as_mut().rust_mut().sel_idx = sw_idx;
                self.as_mut().set_active_profile(new_hw);
                self.as_mut().load_profile(sw_idx);
                self.as_mut().read_hw_profile_color();
            }

            self.as_mut().set_live_buttons(dev.buttons as i32);
            self.as_mut().set_live_paddles(dev.paddles as i32);
            self.as_mut().set_live_lx(dev.left_stick_x as i32);
            self.as_mut().set_live_ly(dev.left_stick_y as i32);
            self.as_mut().set_live_rx(dev.right_stick_x as i32);
            self.as_mut().set_live_ry(dev.right_stick_y as i32);
            self.as_mut().set_live_lt(dev.left_trigger as i32);
            self.as_mut().set_live_rt(dev.right_trigger as i32);
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

fn sock_path() -> String {
    "/run/xbelite2.sock".into()
}

/// Fast status poll using a persistent connection.
/// Reconnects if the connection is lost.
fn fast_poll(model: Pin<&mut ProfileModelRust>) -> Option<DevSt> {
    let model = unsafe { model.get_unchecked_mut() };
    let timeout = Some(std::time::Duration::from_millis(10));

    // Ensure we have a connection
    if model.poll_conn.is_none() {
        if let Ok(s) = UnixStream::connect(sock_path()) {
            s.set_read_timeout(timeout).ok();
            s.set_write_timeout(timeout).ok();
            s.set_nonblocking(false).ok();
            model.poll_conn = Some(s);
        } else {
            return None;
        }
    }

    let conn = model.poll_conn.as_mut()?;
    let req = "{\"type\":\"GetStatus\"}\n";

    // Write request
    if conn.write_all(req.as_bytes()).is_err() {
        model.poll_conn = None;
        return None;
    }

    // Read response line
    let mut buf = [0u8; 4096];
    let mut pos = 0;
    loop {
        match conn.read(&mut buf[pos..pos + 1]) {
            Ok(1) => {
                if buf[pos] == b'\n' || pos >= 4094 {
                    break;
                }
                pos += 1;
            }
            _ => {
                model.poll_conn = None;
                return None;
            }
        }
    }

    let line = std::str::from_utf8(&buf[..pos]).ok()?;
    let resp: Resp = serde_json::from_str(line).ok()?;
    match resp {
        Resp::Status { devices } => devices.into_iter().next(),
        _ => None,
    }
}

fn ipc(path: &str, req: &Req) -> Result<Resp, String> {
    let s = UnixStream::connect(path).map_err(|e| e.to_string())?;
    // Set short timeouts so we don't block the Qt event loop
    let timeout = Some(std::time::Duration::from_millis(30));
    s.set_read_timeout(timeout).ok();
    s.set_write_timeout(timeout).ok();
    let mut sw = &s;
    let j = serde_json::to_string(req).map_err(|e| e.to_string())?;
    writeln!(sw, "{j}").map_err(|e| e.to_string())?;
    let mut r = BufReader::new(&s);
    let mut l = String::new();
    r.read_line(&mut l).map_err(|e| e.to_string())?;
    serde_json::from_str(&l).map_err(|e| e.to_string())
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
