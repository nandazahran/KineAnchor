// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{Config, MotionSample, Vehicle};
use core::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct SpringState {
    pub pos: [f32; 2], // [x, y] in screen pixels
    pub vel: [f32; 2], // [vx, vy]
    pub roll: f32,     // roll angle in radians
    pub roll_vel: f32,
}

impl Default for SpringState {
    fn default() -> Self {
        Self {
            pos: [0.0, 0.0],
            vel: [0.0, 0.0],
            roll: 0.0,
            roll_vel: 0.0,
        }
    }
}

pub struct PhysicsEngine {
    state: SpringState,
    calibrated_gravity: [f32; 3],
    is_calibrated: bool,
    last_sample_time: Option<f64>,
    sample_count: u32,
    rate_timer: f64,
    current_rate_hz: f32,
}

impl Default for PhysicsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsEngine {
    pub fn new() -> Self {
        Self {
            state: SpringState::default(),
            calibrated_gravity: [0.0, 0.0, 9.81],
            is_calibrated: false,
            last_sample_time: None,
            sample_count: 0,
            rate_timer: 0.0,
            current_rate_hz: 0.0,
        }
    }

    pub fn zero(&mut self, sample: Option<&MotionSample>) {
        if let Some(s) = sample {
            if let Some(g) = s.gravity {
                self.calibrated_gravity = g;
                self.is_calibrated = true;
            }
        }
        self.state = SpringState::default();
    }

    pub fn record_sample(&mut self, arrival: f64) {
        self.last_sample_time = Some(arrival);
        self.sample_count += 1;
        if self.rate_timer == 0.0 {
            self.rate_timer = arrival;
        } else if arrival - self.rate_timer >= 1.0 {
            let dt = (arrival - self.rate_timer) as f32;
            self.current_rate_hz = (self.sample_count as f32 / dt).max(0.0);
            self.sample_count = 0;
            self.rate_timer = arrival;
        }
    }

    pub fn sample_rate_hz(&self) -> f32 {
        self.current_rate_hz
    }

    pub fn last_sample_time(&self) -> Option<f64> {
        self.last_sample_time
    }

    /// Updates spring-damper state given target vehicle forces.
    /// dt is bounded to [0.001, 0.1] to prevent numerical explosion.
    pub fn update(&mut self, sample: &MotionSample, dt: f32, config: &Config) -> (f32, f32, f32) {
        let dt = dt.clamp(0.001, 0.1);

        // Project motion onto vehicle coordinates:
        // By default, assuming phone upright or flat:
        // ax: left/right lateral accel
        // ay: forward/backward accel (or az if upright)
        let (raw_lat, raw_long, raw_roll) = match config.vehicle {
            Vehicle::Car => {
                // Invert flags: [invert_x, invert_y, invert_roll]
                let inv_x = if config.invert[0] { -1.0 } else { 1.0 };
                let inv_y = if config.invert[1] { -1.0 } else { 1.0 };

                // Apple convention:
                // Accelerate forward (positive ay) -> dots move DOWN (+dy)
                // Brake (negative ay) -> dots move UP (-dy)
                // Turn left (positive centripetal ax) -> dots move RIGHT (+dx)
                // Turn right (negative ax) -> dots move LEFT (-dx)
                // Gyro gz assists turn response:
                let lat = (sample.ax + sample.gz * 3.0) * inv_x;
                let long = sample.ay * inv_y;
                (lat, long, 0.0)
            }
            Vehicle::Ship => {
                let inv_x = if config.invert[0] { -1.0 } else { 1.0 };
                let inv_y = if config.invert[1] { -1.0 } else { 1.0 };
                let inv_r = if config.invert[2] { -1.0 } else { 1.0 };

                let lat = sample.ax * inv_x;
                // Heave (az) + pitch (ay)
                let long = (sample.az * 0.8 + sample.ay * 0.4) * inv_y;
                let roll = sample.gx * inv_r;
                (lat, long, roll)
            }
        };

        // Scale by config gain and screen factor
        let force_scale = 18.0 * config.gain;
        let target_x = raw_lat * force_scale;
        let target_y = raw_long * force_scale;
        let target_roll = raw_roll * 0.5 * config.gain;

        // 2nd order critically damped spring-damper ODE:
        // x'' + 2*zeta*omega*x' + omega^2 * x = omega^2 * target
        let omega = match config.vehicle {
            Vehicle::Car => 2.0 * PI * 1.2, // ~1.2 Hz natural frequency (responsive)
            Vehicle::Ship => 2.0 * PI * 0.4, // ~0.4 Hz natural frequency (gentle sway)
        };
        let zeta = 0.95; // near critical damping to avoid overshoot nausea

        let k = omega * omega;
        let c = 2.0 * zeta * omega;

        // Integrate X
        let accel_x = k * (target_x - self.state.pos[0]) - c * self.state.vel[0];
        self.state.vel[0] += accel_x * dt;
        self.state.pos[0] += self.state.vel[0] * dt;

        // Integrate Y
        let accel_y = k * (target_y - self.state.pos[1]) - c * self.state.vel[1];
        self.state.vel[1] += accel_y * dt;
        self.state.pos[1] += self.state.vel[1] * dt;

        // Integrate Roll
        let omega_r = 2.0 * PI * 0.5;
        let accel_r = (omega_r * omega_r) * (target_roll - self.state.roll)
            - (2.0 * 0.9 * omega_r) * self.state.roll_vel;
        self.state.roll_vel += accel_r * dt;
        self.state.roll += self.state.roll_vel * dt;

        // Max displacement clamp to 150px so dots stay inside peripheral view
        let max_disp = 150.0;
        self.state.pos[0] = self.state.pos[0].clamp(-max_disp, max_disp);
        self.state.pos[1] = self.state.pos[1].clamp(-max_disp, max_disp);

        (self.state.pos[0], self.state.pos[1], self.state.roll)
    }

    /// Decay offset smoothly toward zero (used when input becomes stale).
    pub fn decay_to_zero(&mut self, dt: f32) -> (f32, f32, f32) {
        let dt = dt.clamp(0.001, 0.1);
        let decay = (-3.0 * dt).exp(); // decay factor
        self.state.pos[0] *= decay;
        self.state.pos[1] *= decay;
        self.state.vel[0] *= decay;
        self.state.vel[1] *= decay;
        self.state.roll *= decay;
        self.state.roll_vel *= decay;

        (self.state.pos[0], self.state.pos[1], self.state.roll)
    }
}
