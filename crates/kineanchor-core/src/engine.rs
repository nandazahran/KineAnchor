// SPDX-License-Identifier: GPL-3.0-or-later

use crate::dots::DotField;
use crate::physics::PhysicsEngine;
use crate::synthetic::SyntheticGenerator;
use crate::types::{Config, Dot, MotionSample, Source, Status};

pub struct Engine {
    config: Config,
    physics: PhysicsEngine,
    dot_field: DotField,
    last_sample: Option<MotionSample>,
    last_tick_time: Option<f64>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            physics: PhysicsEngine::new(),
            dot_field: DotField::new(),
            last_sample: None,
            last_tick_time: None,
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn push(&mut self, sample: MotionSample, arrival: f64) {
        self.physics.record_sample(arrival);
        self.last_sample = Some(sample);
    }

    pub fn zero(&mut self) {
        self.physics.zero(self.last_sample.as_ref());
    }

    pub fn tick(&mut self, now: f64, viewport: [f32; 2]) -> (&[Dot], Status) {
        let dt = match self.last_tick_time {
            Some(last) => (now - last) as f32,
            None => 1.0 / 60.0,
        };
        self.last_tick_time = Some(now);

        let (dx, dy, roll, status) = match self.config.source {
            Source::Synthetic => {
                let sample =
                    SyntheticGenerator::sample(now, self.config.vehicle, &self.config.synthetic);
                let (x, y, r) = self.physics.update(&sample, dt, &self.config);
                (x, y, r, Status::Synthetic)
            }
            Source::Sensor => {
                let last_time = self.physics.last_sample_time();
                let is_stale = match last_time {
                    Some(t) => (now - t) > 0.5,
                    None => true,
                };

                if is_stale {
                    let since = last_time.unwrap_or(0.0);
                    let (x, y, r) = self.physics.decay_to_zero(dt);
                    (x, y, r, Status::Stale { since })
                } else {
                    let sample = self.last_sample.unwrap_or(MotionSample {
                        t: now,
                        ax: 0.0,
                        ay: 0.0,
                        az: 0.0,
                        gx: 0.0,
                        gy: 0.0,
                        gz: 0.0,
                        gravity: None,
                    });
                    let (x, y, r) = self.physics.update(&sample, dt, &self.config);
                    let rate_hz = self.physics.sample_rate_hz();
                    (x, y, r, Status::Live { rate_hz })
                }
            }
        };

        let dots = self.dot_field.update(viewport, dx, dy, roll, &self.config);
        (dots, status)
    }

    pub fn current_dots(&self) -> &[Dot] {
        self.dot_field.dots()
    }
}
