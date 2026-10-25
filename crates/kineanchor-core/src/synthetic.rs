// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{MotionSample, SyntheticParams, Vehicle};
use core::f32::consts::PI;

pub struct SyntheticGenerator;

impl SyntheticGenerator {
    pub fn sample(t: f64, vehicle: Vehicle, params: &SyntheticParams) -> MotionSample {
        let time = t as f32;
        let base_f = params.freq_hz.max(0.01);
        let amp = params.amplitude;

        match vehicle {
            Vehicle::Car => {
                // Subtle road sway and turning:
                // ax: lateral acceleration (turning left/right)
                // ay: longitudinal acceleration (accel/brake)
                let omega = 2.0 * PI * base_f;
                let ax = amp * (omega * time).sin();
                let ay = amp * 0.6 * (omega * 0.7 * time + PI / 3.0).cos();
                let gz = (ax / 9.81) * 0.3; // yaw rate correlated with lateral accel

                MotionSample {
                    t,
                    ax,
                    ay,
                    az: 0.0,
                    gx: 0.0,
                    gy: 0.0,
                    gz,
                    gravity: Some([0.0, 0.0, 9.81]),
                }
            }
            Vehicle::Ship => {
                // Multi-frequency wave motion (heave, pitch, roll in 0.08 - 0.2 Hz band)
                let f_heave = base_f * 0.8;
                let f_roll = base_f * 0.6;
                let f_pitch = base_f * 0.5;

                let heave_accel = amp * (2.0 * PI * f_heave * time).sin();
                let roll_rate = params.roll_amplitude * (2.0 * PI * f_roll * time).cos();
                let pitch_rate =
                    params.roll_amplitude * 0.5 * (2.0 * PI * f_pitch * time + 0.4).sin();

                MotionSample {
                    t,
                    ax: amp * 0.3 * (2.0 * PI * f_roll * time).sin(),
                    ay: amp * 0.2 * (2.0 * PI * f_pitch * time).sin(),
                    az: heave_accel,
                    gx: roll_rate,
                    gy: pitch_rate,
                    gz: 0.0,
                    gravity: Some([0.0, 0.0, 9.81]),
                }
            }
        }
    }
}
