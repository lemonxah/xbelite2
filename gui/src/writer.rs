//! Background writer thread.
//!
//! Holds a persistent GipDevice and serializes hardware writes so the Qt
//! main thread never blocks on USB I/O. Callers optimistically update the
//! in-memory model state, push a command here, and return immediately.

use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

use xbelite2_gip::transport::GipDevice;
use xbelite2_gip::types::GipButton;
use xbelite2_gip::{led, name, profile, rumble};

pub enum WriteOp {
    SetColor { profile_idx: usize, r: u8, g: u8, b: u8 },
    SetRemapNormal { profile_idx: usize, from: GipButton, to: GipButton },
    SetRemapShift { profile_idx: usize, from: GipButton, to: GipButton },
    SetStickInversion { profile_idx: usize, mask: u8 },
    SetDeviceName { name: String },
    /// Start rumble; worker schedules an auto-stop after `duration_ms`.
    Rumble { lm: u8, rm: u8, lt: u8, rt: u8, duration_ms: u64 },
    RumbleStop,
    ResetCurves { profile_idx: usize },
}

pub struct Writer {
    tx: Sender<WriteOp>,
}

impl Writer {
    pub fn spawn() -> Self {
        let (tx, rx) = channel::<WriteOp>();
        let tx_self = tx.clone();
        thread::spawn(move || {
            let mut dev: Option<GipDevice> = None;
            while let Ok(op) = rx.recv() {
                // Open /dev/xbelite2 lazily; reopen on error.
                if dev.is_none() {
                    match GipDevice::open_usb() {
                        Ok(mut d) => {
                            d.unlock();
                            dev = Some(d);
                        }
                        Err(e) => {
                            eprintln!("writer: cannot open /dev/xbelite2: {e}");
                            continue;
                        }
                    }
                }
                let d = match dev.as_mut() {
                    Some(d) => d,
                    None => continue,
                };
                exec(d, op, &tx_self);
            }
        });
        Self { tx }
    }

    pub fn send(&self, op: WriteOp) {
        let _ = self.tx.send(op);
    }
}

fn exec(dev: &mut GipDevice, op: WriteOp, tx: &Sender<WriteOp>) {
    match op {
        WriteOp::SetColor { profile_idx, r, g, b } => {
            profile::set_color(dev, profile_idx, r, g, b);
            led::set_color(dev, r, g, b);
        }
        WriteOp::SetRemapNormal { profile_idx, from, to } => {
            profile::remap_buttons(dev, profile_idx, &[(from, to)]);
        }
        WriteOp::SetRemapShift { profile_idx, from, to } => {
            profile::remap_shift(dev, profile_idx, &[(from, to)]);
        }
        WriteOp::SetStickInversion { profile_idx, mask } => {
            profile::set_stick_inversion(dev, profile_idx, mask);
        }
        WriteOp::SetDeviceName { name } => {
            let _ = name::write(dev, &name);
        }
        WriteOp::Rumble { lm, rm, lt, rt, duration_ms } => {
            rumble::set(dev, lm, rm, lt, rt);
            // Schedule auto-stop off-thread so the worker stays free to
            // process other writes during the rumble.
            let tx = tx.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(duration_ms));
                let _ = tx.send(WriteOp::RumbleStop);
            });
        }
        WriteOp::RumbleStop => {
            rumble::stop(dev);
        }
        WriteOp::ResetCurves { profile_idx } => {
            profile::reset_curves(dev, profile_idx);
        }
    }
}
