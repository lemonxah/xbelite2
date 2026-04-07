//! Input transformation engine.
//!
//! Takes a raw GamepadState and applies the active profile's
//! remapping, stick curves, and trigger dead zones.

use crate::types::*;

/// An output event ready to be sent via uinput.
#[derive(Debug, Clone)]
pub struct OutputEvent {
    pub ev_type: u16,
    pub code: u16,
    pub value: i32,
}

/// Transform a raw GamepadState through the given Profile, producing output events.
/// Compares against previous state to only emit changed events (delta).
pub fn transform(
    current: &GamepadState,
    previous: &GamepadState,
    profile: &Profile,
) -> Vec<OutputEvent> {
    let mut events = Vec::with_capacity(32);

    // Build raw event list from current state
    let raw_buttons = gather_buttons(current);
    let prev_buttons = gather_buttons(previous);
    let raw_axes = gather_axes(current, profile);
    let prev_axes = gather_axes(previous, profile);

    // Emit button deltas
    for (id, pressed) in &raw_buttons {
        let prev_pressed = prev_buttons
            .iter()
            .find(|(pid, _)| pid == id)
            .map(|(_, v)| *v)
            .unwrap_or(false);

        if *pressed != prev_pressed {
            let value = if *pressed { 1 } else { 0 };
            let remapped = apply_remap_key(profile, *id, value);
            events.push(remapped);
        }
    }

    // Emit axis deltas
    for (id, value) in &raw_axes {
        let prev_value = prev_axes
            .iter()
            .find(|(pid, _)| pid == id)
            .map(|(_, v)| *v)
            .unwrap_or(0);

        if *value != prev_value {
            let remapped = apply_remap_abs(profile, *id, *value);
            events.push(remapped);
        }
    }

    events
}

/// Gather all button states as (InputId, pressed) pairs.
fn gather_buttons(state: &GamepadState) -> Vec<(InputId, bool)> {
    vec![
        (InputId::key(BTN_A), state.btn_a),
        (InputId::key(BTN_B), state.btn_b),
        (InputId::key(BTN_X), state.btn_x),
        (InputId::key(BTN_Y), state.btn_y),
        (InputId::key(BTN_TL), state.btn_lb),
        (InputId::key(BTN_TR), state.btn_rb),
        (InputId::key(BTN_SELECT), state.btn_view),
        (InputId::key(BTN_START), state.btn_menu),
        (InputId::key(BTN_MODE), state.btn_xbox),
        (InputId::key(BTN_THUMBL), state.btn_lstick),
        (InputId::key(BTN_THUMBR), state.btn_rstick),
        // Paddles - always exposed regardless of hardware profile
        (InputId::key(BTN_GRIPL), state.paddle_ul),
        (InputId::key(BTN_GRIPR), state.paddle_ur),
        (InputId::key(BTN_GRIPL2), state.paddle_ll),
        (InputId::key(BTN_GRIPR2), state.paddle_lr),
    ]
}

/// Gather all axis values, applying stick curves and trigger dead zones.
fn gather_axes(state: &GamepadState, profile: &Profile) -> Vec<(InputId, i32)> {
    let lx = apply_stick_curve(profile, 0, state.left_stick_x);
    let ly = apply_stick_curve(profile, 1, state.left_stick_y);
    let rx = apply_stick_curve(profile, 2, state.right_stick_x);
    let ry = apply_stick_curve(profile, 3, state.right_stick_y);
    let lt = apply_trigger_zone(profile, 0, state.left_trigger);
    let rt = apply_trigger_zone(profile, 1, state.right_trigger);

    // D-pad as hat axes
    let hat_x: i32 = match (state.dpad_left, state.dpad_right) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    let hat_y: i32 = match (state.dpad_up, state.dpad_down) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };

    vec![
        (InputId::abs(ABS_X), lx as i32),
        (InputId::abs(ABS_Y), ly as i32),
        (InputId::abs(ABS_RX), rx as i32),
        (InputId::abs(ABS_RY), ry as i32),
        (InputId::abs(ABS_Z), lt as i32),
        (InputId::abs(ABS_RZ), rt as i32),
        (InputId::abs(ABS_HAT0X), hat_x),
        (InputId::abs(ABS_HAT0Y), hat_y),
    ]
}

/// Apply stick curve if configured for the given axis index.
fn apply_stick_curve(profile: &Profile, axis_idx: usize, raw: i16) -> i16 {
    match &profile.stick_curves[axis_idx] {
        Some(curve) => curve.evaluate(raw),
        None => raw,
    }
}

/// Apply trigger dead zone if configured.
fn apply_trigger_zone(profile: &Profile, trigger_idx: usize, raw: u16) -> u16 {
    match &profile.trigger_zones[trigger_idx] {
        Some(zone) => zone.apply(raw),
        None => raw,
    }
}

/// Apply key remap: look up the source button in the profile's remap table.
fn apply_remap_key(profile: &Profile, src: InputId, value: i32) -> OutputEvent {
    for remap in &profile.remaps {
        if remap.src == src {
            return OutputEvent {
                ev_type: remap.dst.ev_type,
                code: remap.dst.code,
                value,
            };
        }
    }
    // No remap: pass through
    OutputEvent {
        ev_type: src.ev_type,
        code: src.code,
        value,
    }
}

/// Apply axis remap.
fn apply_remap_abs(profile: &Profile, src: InputId, value: i32) -> OutputEvent {
    for remap in &profile.remaps {
        if remap.src == src {
            let scaled = (value as f32 * remap.scale) as i32;
            return OutputEvent {
                ev_type: remap.dst.ev_type,
                code: remap.dst.code,
                value: scaled,
            };
        }
    }
    OutputEvent {
        ev_type: src.ev_type,
        code: src.code,
        value,
    }
}
