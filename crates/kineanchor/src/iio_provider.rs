// SPDX-License-Identifier: GPL-3.0-or-later

use crate::monotonic_secs;
use crate::provider::{Provider, ProviderEvent};
use kineanchor_core::MotionSample;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::time::Duration;

pub struct IioProvider {
    device_dir: Option<PathBuf>,
}

impl Default for IioProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IioProvider {
    pub fn new() -> Self {
        Self {
            device_dir: Self::discover_iio_accel(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.device_dir.is_some()
    }

    fn discover_iio_accel() -> Option<PathBuf> {
        let entries = fs::read_dir("/sys/bus/iio/devices").ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("in_accel_x_raw").exists()
                && path.join("in_accel_y_raw").exists()
                && path.join("in_accel_z_raw").exists()
            {
                return Some(path);
            }
        }
        None
    }

    fn read_f32(path: &Path) -> Option<f32> {
        fs::read_to_string(path).ok()?.trim().parse::<f32>().ok()
    }
}

impl Provider for IioProvider {
    fn name(&self) -> &'static str {
        "linux-iio"
    }

    fn run(self: Box<Self>, tx: Sender<ProviderEvent>, stop: Arc<AtomicBool>) {
        let Some(dir) = self.device_dir else {
            return;
        };

        let accel_scale = Self::read_f32(&dir.join("in_accel_scale")).unwrap_or(0.00981);
        let gyro_scale = Self::read_f32(&dir.join("in_anglvel_scale")).unwrap_or(0.001);

        while !stop.load(Ordering::Relaxed) {
            let ax_raw = Self::read_f32(&dir.join("in_accel_x_raw"));
            let ay_raw = Self::read_f32(&dir.join("in_accel_y_raw"));
            let az_raw = Self::read_f32(&dir.join("in_accel_z_raw"));

            if let (Some(rx), Some(ry), Some(rz)) = (ax_raw, ay_raw, az_raw) {
                let gx = Self::read_f32(&dir.join("in_anglvel_x_raw")).unwrap_or(0.0) * gyro_scale;
                let gy = Self::read_f32(&dir.join("in_anglvel_y_raw")).unwrap_or(0.0) * gyro_scale;
                let gz = Self::read_f32(&dir.join("in_anglvel_z_raw")).unwrap_or(0.0) * gyro_scale;

                let ax = rx * accel_scale;
                let ay = ry * accel_scale;
                let az = rz * accel_scale;
                let t = monotonic_secs();

                let _ = tx.send(ProviderEvent::Sample(MotionSample {
                    t,
                    ax,
                    ay,
                    az,
                    gx,
                    gy,
                    gz,
                    gravity: Some([0.0, 0.0, 9.81]),
                }));
            }

            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
