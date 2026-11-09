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
    last_stale_blend: f32,
    /// Runtime-only pause: holds the dot field at the zero reference while providers keep
    /// running. Never persisted, so a restart always resumes with live cues.
    paused: bool,
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
            last_stale_blend: 0.0,
            paused: false,
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn push(&mut self, sample: MotionSample, arrival: f64) {
        self.physics.push_sample(&sample, arrival);
        self.last_sample = Some(sample);
        self.last_stale_blend = 0.0;
    }

    pub fn zero(&mut self) {
        self.physics.zero(self.last_sample.as_ref());
    }

    pub fn status(&self, now: f64) -> Status {
        match self.config.source {
            Source::Synthetic => Status::Synthetic,
            Source::Sensor => {
                let last_time = self.physics.last_sample_time();
                let is_stale = last_time.is_none_or(|t| (now - t) > 0.5);
                if is_stale {
                    Status::Stale {
                        since: last_time.unwrap_or(0.0),
                    }
                } else {
                    Status::Live {
                        rate_hz: self.physics.sample_rate_hz(),
                    }
                }
            }
        }
    }

    pub fn tick(&mut self, now: f64, viewport: [f32; 2]) -> (&[Dot], Status) {
        let dt = match self.last_tick_time {
            Some(last) => (now - last) as f32,
            None => 1.0 / 60.0,
        };
        self.last_tick_time = Some(now);

        if self.paused {
            // Paused means a static reference: no displacement, no roll, no synthetic wave.
            self.last_stale_blend = 0.0;
            let status = self.status(now);
            let dots = self.dot_field.update(viewport, 0.0, 0.0, 0.0, &self.config);
            return (dots, status);
        }

        let (dx, dy, roll, status) = match self.config.source {
            Source::Synthetic => {
                self.last_stale_blend = 0.0;
                let sample =
                    SyntheticGenerator::sample(now, self.config.vehicle, &self.config.synthetic);
                let (x, y, r) = self.physics.update(&sample, dt, &self.config);
                (x, y, r, Status::Synthetic)
            }
            Source::Sensor => {
                let last_time = self.physics.last_sample_time();
                let stale_duration = last_time.map(|t| now - t);
                let is_stale = stale_duration.is_none_or(|d| d > 0.5);

                if is_stale {
                    let since = last_time.unwrap_or(0.0);
                    let dur = stale_duration.unwrap_or(1.5);
                    let ((x, y, r), blend) = self.physics.crossfade_stale(dur);
                    self.last_stale_blend = blend;
                    (x, y, r, Status::Stale { since })
                } else {
                    self.last_stale_blend = 0.0;
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

    pub fn current_offset(&self) -> (f32, f32, f32) {
        self.physics.current_offset()
    }

    pub fn stale_blend(&self) -> f32 {
        self.last_stale_blend
    }
}
