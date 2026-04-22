use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

const SOCKET_PATH: &str = "/run/xbelite2.sock";

pub fn is_daemon_running() -> bool {
    Path::new(SOCKET_PATH).exists()
}

fn send_request(req: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;
    
    writeln!(stream, "{}", serde_json::to_string(&req).unwrap())
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    serde_json::from_str(&response).map_err(|e| format!("Invalid JSON response: {}", e))
}

fn get_device_id() -> Result<String, String> {
    let resp = send_request(serde_json::json!({ "type": "GetStatus" }))?;
    
    resp.get("devices")
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|dev| dev.get("device_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No devices connected".to_string())
}

fn check_error(resp: &serde_json::Value) -> Result<(), String> {
    if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
        return Err(msg.to_string());
    }
    Ok(())
}

pub fn set_profile_color(_profile_idx: usize, r: u8, g: u8, b: u8) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "SetProfileColor",
        "device_id": device_id,
        "r": r,
        "g": g,
        "b": b
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn set_device_name(name: &str) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "SetDeviceName",
        "device_id": device_id,
        "name": name
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn set_deadzones(_profile_idx: usize, lstick: u8, rstick: u8, ltrig: u8, rtrig: u8) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "SetDeadzones",
        "device_id": device_id,
        "lstick": lstick,
        "rstick": rstick,
        "ltrig": ltrig,
        "rtrig": rtrig
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn set_vibration(_profile_idx: usize, left: u8, right: u8) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "SetVibration",
        "device_id": device_id,
        "left": left,
        "right": right
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn set_curves(_profile_idx: usize, lx: u8, ly: u8, rx: u8, ry: u8) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "SetCurves",
        "device_id": device_id,
        "lx": lx,
        "ly": ly,
        "rx": rx,
        "ry": ry
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn reset_remaps(_profile_idx: usize) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "ResetRemaps",
        "device_id": device_id
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn reset_profile(_profile_idx: usize) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "ResetProfile",
        "device_id": device_id
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn remap_buttons(_profile_idx: usize, remaps: &[(xbelite2_gip::types::GipButton, xbelite2_gip::types::GipButton)]) -> Result<(), String> {
    let device_id = get_device_id()?;
    
    let mappings: Vec<_> = remaps.iter()
        .map(|(from, to)| serde_json::json!({
            "from": from.name(),
            "to": to.name()
        }))
        .collect();
    
    let req = serde_json::json!({
        "type": "SetHwRemap",
        "device_id": device_id,
        "mappings": mappings
    });
    let resp = send_request(req)?;
    check_error(&resp)
}

pub fn remap_paddles(_profile_idx: usize, p1: &str, p2: &str, p3: &str, p4: &str) -> Result<(), String> {
    let device_id = get_device_id()?;
    let req = serde_json::json!({
        "type": "RemapPaddles",
        "device_id": device_id,
        "p1": p1,
        "p2": p2,
        "p3": p3,
        "p4": p4
    });
    let resp = send_request(req)?;
    check_error(&resp)
}
