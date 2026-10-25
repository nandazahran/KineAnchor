// SPDX-License-Identifier: GPL-3.0-or-later

//! Optional controller IMU provider (DualSense / DualShock 4 / Switch Pro) via Linux kernel
//! `INPUT_PROP_ACCELEROMETER` evdev nodes (`/sys/class/input/event*/device/properties`).

use crate::monotonic_secs;
use crate::provider::{Provider, ProviderEvent};
use kineanchor_core::MotionSample;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::time::Duration;

const EV_SYN: u16 = 0x00;
const EV_ABS: u16 = 0x03;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;

pub struct ControllerProvider {
    event_dev: Option<PathBuf>,
}

impl Default for ControllerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerProvider {
    pub fn new() -> Self {
        Self {
            event_dev: Self::discover_imu_evdev(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.event_dev.is_some()
    }

    /// Finds `/dev/input/eventN` where `/sys/class/input/eventN/device/properties` has
    /// `INPUT_PROP_ACCELEROMETER` (bit 6 = 0x40).
    pub fn discover_imu_evdev() -> Option<PathBuf> {
        let entries = fs::read_dir("/sys/class/input").ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("event") {
                continue;
            }
            let props_path = entry.path().join("device/properties");
            if let Ok(raw) = fs::read_to_string(props_path) {
                let first_word = raw.split_whitespace().next().unwrap_or("0");
                if let Ok(mask) = u64::from_str_radix(first_word, 16) {
                    if (mask & (1 << 6)) != 0 {
                        let dev_path = PathBuf::from("/dev/input").join(name);
                        if dev_path.exists() {
                            return Some(dev_path);
                        }
                    }
                }
            }
        }
        None
    }
}

impl Provider for ControllerProvider {
    fn name(&self) -> &'static str {
        "controller-imu"
    }

    fn run(self: Box<Self>, tx: Sender<ProviderEvent>, stop: Arc<AtomicBool>) {
        let Some(dev_path) = self.event_dev else {
            return;
        };
        let Ok(mut file) = File::open(dev_path) else {
            return;
        };

        // 64-bit Linux input_event is 24 bytes: tv_sec(8), tv_usec(8), type(2), code(2), value(4)
        let mut buf = [0u8; 24];
        let mut raw_accel = [0.0_f32; 3];
        let mut raw_gyro = [0.0_f32; 3];

        // DualSense / hid-playstation default scale: ~8192 LSB/g, ~1024 LSB/(rad/s)
        let accel_scale = 9.81 / 8192.0;
        let gyro_scale = 1.0 / 1024.0;

        while !stop.load(Ordering::Relaxed) {
            if file.read_exact(&mut buf).is_err() {
                std::thread::sleep(Duration::from_millis(100));
                break;
            }
            let ev_type = u16::from_ne_bytes([buf[16], buf[17]]);
            let ev_code = u16::from_ne_bytes([buf[18], buf[19]]);
            let ev_val = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]) as f32;

            if ev_type == EV_ABS {
                match ev_code {
                    ABS_X => raw_accel[0] = ev_val * accel_scale,
                    ABS_Y => raw_accel[1] = ev_val * accel_scale,
                    ABS_Z => raw_accel[2] = ev_val * accel_scale,
                    ABS_RX => raw_gyro[0] = ev_val * gyro_scale,
                    ABS_RY => raw_gyro[1] = ev_val * gyro_scale,
                    ABS_RZ => raw_gyro[2] = ev_val * gyro_scale,
                    _ => {}
                }
            } else if ev_type == EV_SYN {
                let t = monotonic_secs();
                let _ = tx.send(ProviderEvent::Sample(MotionSample {
                    t,
                    ax: raw_accel[0],
                    ay: raw_accel[1],
                    az: raw_accel[2],
                    gx: raw_gyro[0],
                    gy: raw_gyro[1],
                    gz: raw_gyro[2],
                    gravity: Some([0.0, 0.0, 9.81]),
                }));
            }
        }
    }
}
