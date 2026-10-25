// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{Config, MotionSample, Vehicle};
use core::f32::consts::PI;

#[derive(Debug, Clone, Default)]
pub struct SpringState {
    pub pos: [f32; 2], // [x, y] in screen pixels
    pub vel: [f32; 2], // [vx, vy]
    pub roll: f32,     // roll angle in radians
    pub roll_vel: f32,
}

#[derive(Debug, Clone)]
struct FilterState {
    lpf: [f32; 3],  // [lat, long, vert] low-pass filtered
    bias: [f32; 3], // slow high-pass bias tracker
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            lpf: [0.0; 3],
            bias: [0.0; 3],
        }
    }
}

pub struct PhysicsEngine {
    state: SpringState,
    stale_anchor: Option<SpringState>,
    filter: FilterState,
    right_axis: [f32; 3],
    forward_axis: [f32; 3],
    up_axis: [f32; 3],
    gyro_bias: [f32; 3],
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

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = dot3(v, v).sqrt();
    if len > 1e-4 {
        Some([v[0] / len, v[1] / len, v[2] / len])
    } else {
        None
    }
}

/// Computes orthonormal vehicle basis (right, forward, up) in device coordinates from gravity.
/// Device convention: +x right, +y top edge, +z out of screen.
/// Uses the horizontal projection of (0, 1, -1) so flat (+y forward) and upright (-z forward)
/// and any intermediate tilt angle map continuously to the same vehicle forward axis.
pub fn basis_from_gravity(gravity: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let up = match norm3(gravity) {
        Some(u) => u,
        None => return ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    };

    let ref_dir = [0.0, 1.0, -1.0];
    let proj_dot = dot3(ref_dir, up);
    let f_raw = [
        ref_dir[0] - proj_dot * up[0],
        ref_dir[1] - proj_dot * up[1],
        ref_dir[2] - proj_dot * up[2],
    ];

    let forward = norm3(f_raw).unwrap_or_else(|| {
        // Fallback if gravity is colinear with (0, 1, -1)
        let alt = [0.0, 1.0, 1.0];
        let d = dot3(alt, up);
        norm3([alt[0] - d * up[0], alt[1] - d * up[1], alt[2] - d * up[2]])
            .unwrap_or([0.0, 1.0, 0.0])
    });

    let right = norm3(cross3(forward, up)).unwrap_or([1.0, 0.0, 0.0]);
    (right, forward, up)
}

impl PhysicsEngine {
    pub fn new() -> Self {
        Self {
            state: SpringState::default(),
            stale_anchor: None,
            filter: FilterState::default(),
            right_axis: [1.0, 0.0, 0.0],
            forward_axis: [0.0, 1.0, 0.0],
            up_axis: [0.0, 0.0, 1.0],
            gyro_bias: [0.0, 0.0, 0.0],
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
                let (r, f, u) = basis_from_gravity(g);
                self.right_axis = r;
                self.forward_axis = f;
                self.up_axis = u;
                self.is_calibrated = true;
            }
            self.gyro_bias = [s.gx, s.gy, s.gz];
        }
        self.state = SpringState::default();
        self.stale_anchor = None;
        self.filter = FilterState::default();
    }

    pub fn record_sample(&mut self, arrival: f64) {
        self.last_sample_time = Some(arrival);
        self.stale_anchor = None;
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

    /// Projects a device-frame sample into vehicle-frame (lat, long, vert, yaw, roll, pitch).
    pub fn project_sample(&mut self, sample: &MotionSample) -> (f32, f32, f32, f32, f32, f32) {
        if !self.is_calibrated {
            if let Some(g) = sample.gravity {
                let (r, f, u) = basis_from_gravity(g);
                self.right_axis = r;
                self.forward_axis = f;
                self.up_axis = u;
                self.is_calibrated = true;
            }
        }

        let a = [sample.ax, sample.ay, sample.az];
        let w = [
            sample.gx - self.gyro_bias[0],
            sample.gy - self.gyro_bias[1],
            sample.gz - self.gyro_bias[2],
        ];

        let a_lat = dot3(a, self.right_axis);
        let a_long = dot3(a, self.forward_axis);
        let a_vert = dot3(a, self.up_axis);

        let w_yaw = dot3(w, self.up_axis);
        let w_roll = dot3(w, self.forward_axis);
        let w_pitch = dot3(w, self.right_axis);

        (a_lat, a_long, a_vert, w_yaw, w_roll, w_pitch)
    }

    /// Updates spring-damper state given target vehicle forces.
    /// dt is bounded to [0.001, 0.1] to prevent numerical explosion.
    pub fn update(&mut self, sample: &MotionSample, dt: f32, config: &Config) -> (f32, f32, f32) {
        let dt = dt.clamp(0.001, 0.1);
        let (a_lat, a_long, a_vert, w_yaw, w_roll, _w_pitch) = self.project_sample(sample);

        // Low-pass filter + slow high-pass bias removal
        let cutoff_hz = match config.vehicle {
            Vehicle::Car => 2.5,
            Vehicle::Ship => 0.5,
        };
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let alpha_lpf = dt / (rc + dt);

        let hpf_rc = 1.0 / (2.0 * PI * 0.03);
        let alpha_bias = dt / (hpf_rc + dt);

        let raw = [a_lat, a_long, a_vert];
        for (i, &r_val) in raw.iter().enumerate() {
            self.filter.bias[i] += alpha_bias * (r_val - self.filter.bias[i]);
            let unbiased = r_val - self.filter.bias[i];
            self.filter.lpf[i] += alpha_lpf * (unbiased - self.filter.lpf[i]);
        }

        let (f_lat, f_long, f_vert) = (self.filter.lpf[0], self.filter.lpf[1], self.filter.lpf[2]);

        let (raw_lat, raw_long, raw_roll) = match config.vehicle {
            Vehicle::Car => {
                let inv_x = if config.invert[0] { -1.0 } else { 1.0 };
                let inv_y = if config.invert[1] { -1.0 } else { 1.0 };

                // Apple convention:
                // Accelerate forward (positive a_long) -> dots move DOWN (+dy)
                // Brake (negative a_long) -> dots move UP (-dy)
                // Turn left (positive centripetal a_lat / positive yaw) -> dots move RIGHT (+dx)
                // Turn right (negative a_lat / negative yaw) -> dots move LEFT (-dx)
                let lat = (f_lat + w_yaw * 3.0) * inv_x;
                let long = f_long * inv_y;
                (lat, long, 0.0)
            }
            Vehicle::Ship => {
                let inv_x = if config.invert[0] { -1.0 } else { 1.0 };
                let inv_y = if config.invert[1] { -1.0 } else { 1.0 };
                let inv_r = if config.invert[2] { -1.0 } else { 1.0 };

                let lat = f_lat * inv_x;
                let long = (f_vert * 0.8 + f_long * 0.4) * inv_y;
                let roll = w_roll * inv_r;
                (lat, long, roll)
            }
        };

        let force_scale = 18.0 * config.gain;
        let target_x = raw_lat * force_scale;
        let target_y = raw_long * force_scale;
        let target_roll = raw_roll * 0.5 * config.gain;

        let omega = match config.vehicle {
            Vehicle::Car => 2.0 * PI * 1.2,
            Vehicle::Ship => 2.0 * PI * 0.4,
        };
        let zeta = 0.95;

        let k = omega * omega;
        let c = 2.0 * zeta * omega;

        let accel_x = k * (target_x - self.state.pos[0]) - c * self.state.vel[0];
        self.state.vel[0] += accel_x * dt;
        self.state.pos[0] += self.state.vel[0] * dt;

        let accel_y = k * (target_y - self.state.pos[1]) - c * self.state.vel[1];
        self.state.vel[1] += accel_y * dt;
        self.state.pos[1] += self.state.vel[1] * dt;

        let omega_r = 2.0 * PI * 0.5;
        let accel_r = (omega_r * omega_r) * (target_roll - self.state.roll)
            - (2.0 * 0.9 * omega_r) * self.state.roll_vel;
        self.state.roll_vel += accel_r * dt;
        self.state.roll += self.state.roll_vel * dt;

        let max_disp = 150.0;
        self.state.pos[0] = self.state.pos[0].clamp(-max_disp, max_disp);
        self.state.pos[1] = self.state.pos[1].clamp(-max_disp, max_disp);

        (self.state.pos[0], self.state.pos[1], self.state.roll)
    }

    /// Crossfades from the last live state to static (0, 0, 0) over 1.0s starting at 0.5s stale.
    /// `stale_duration` is `now - last_sample_time` (> 0.5s).
    /// Returns `((dx, dy, roll), blend_progress)` where `blend_progress` reaches 1.0 at 1.5s.
    pub fn crossfade_stale(&mut self, stale_duration: f64) -> ((f32, f32, f32), f32) {
        let anchor = self
            .stale_anchor
            .get_or_insert_with(|| self.state.clone())
            .clone();

        let blend = ((stale_duration - 0.5) as f32 / 1.0).clamp(0.0, 1.0);
        let keep = 1.0 - blend;

        self.state.pos[0] = anchor.pos[0] * keep;
        self.state.pos[1] = anchor.pos[1] * keep;
        self.state.vel = [0.0, 0.0];
        self.state.roll = anchor.roll * keep;
        self.state.roll_vel = 0.0;

        (
            (self.state.pos[0], self.state.pos[1], self.state.roll),
            blend,
        )
    }

    pub fn current_offset(&self) -> (f32, f32, f32) {
        (self.state.pos[0], self.state.pos[1], self.state.roll)
    }
}
