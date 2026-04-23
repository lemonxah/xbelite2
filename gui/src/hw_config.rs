use std::process::Command;

pub fn read_device_name() -> Result<String, String> {
    let output = Command::new("xbe2-rw")
        .arg("name")
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn set_device_name(name: &str) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("name")
        .arg(name)
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw name set failed".into());
    }

    Ok(())
}

pub fn set_profile_color(profile: u8, hex: &str) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("color")
        .arg(profile.to_string())
        .arg(hex.trim_start_matches('#'))
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw color set failed".into());
    }

    Ok(())
}

pub fn rumble(strong: u8, weak: u8, left_trigger: u8, right_trigger: u8) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("rumble")
        .arg(strong.to_string())
        .arg(weak.to_string())
        .arg(left_trigger.to_string())
        .arg(right_trigger.to_string())
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw rumble failed".into());
    }

    Ok(())
}

pub fn rumble_stop() -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("rumble-stop")
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw rumble-stop failed".into());
    }

    Ok(())
}

pub fn set_hw_remap(profile: u8, src: &str, dst: &str) -> Result<(), String> {
    let remap_arg = format!("{}={}", src, dst);
    let output = Command::new("xbe2-rw")
        .arg("remap")
        .arg(profile.to_string())
        .arg(remap_arg)
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw remap failed".into());
    }

    Ok(())
}

pub fn set_hw_remap_shift(profile: u8, src: &str, dst: &str) -> Result<(), String> {
    let remap_arg = format!("{}={}", src, dst);
    let output = Command::new("xbe2-rw")
        .arg("remap-shift")
        .arg(profile.to_string())
        .arg(remap_arg)
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw remap-shift failed".into());
    }

    Ok(())
}

pub fn set_deadzone(profile: u8, ls_inner: u8, ls_outer: u8, rs_inner: u8, rs_outer: u8) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("deadzone")
        .arg(profile.to_string())
        .arg(ls_inner.to_string())
        .arg(ls_outer.to_string())
        .arg(rs_inner.to_string())
        .arg(rs_outer.to_string())
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw deadzone failed".into());
    }

    Ok(())
}

pub fn set_stick_inversion(profile: u8, mask: u8) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("invert")
        .arg(profile.to_string())
        .arg(format!("0x{:02x}", mask))
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw invert failed".into());
    }

    Ok(())
}

pub fn set_vibration(profile: u8, main: u8, weak: u8, lt: u8, rt: u8) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("vibration")
        .arg(profile.to_string())
        .arg(main.to_string())
        .arg(weak.to_string())
        .arg(lt.to_string())
        .arg(rt.to_string())
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw vibration failed".into());
    }

    Ok(())
}

pub fn reset_curves(profile: u8) -> Result<(), String> {
    let output = Command::new("xbe2-rw")
        .arg("curves")
        .arg(profile.to_string())
        .arg("reset")
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw curves reset failed".into());
    }

    Ok(())
}

pub fn read_profiles() -> Result<String, String> {
    let output = Command::new("xbe2-rw")
        .arg("profiles")
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw profiles failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn read_profile(profile: u8) -> Result<String, String> {
    let output = Command::new("xbe2-rw")
        .arg("profile")
        .arg(profile.to_string())
        .output()
        .map_err(|e| format!("Failed to run xbe2-rw: {}", e))?;

    if !output.status.success() {
        return Err("xbe2-rw profile read failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
