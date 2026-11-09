// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionSample {
    pub t: f64,
    pub ax: f32,
    pub ay: f32,
    pub az: f32,
    pub gx: f32,
    pub gy: f32,
    pub gz: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Vehicle {
    #[default]
    Car,
    Ship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Sensor,
    Synthetic,
}

/// Direction of the desktop/phone TCP session.
///
/// Only one side ever dials: `DialPhone` makes the desktop a client, `ListenForPhone`
/// makes it a server. The phone applies the mirrored rule from the same pairing data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetMode {
    /// Desktop dials the phone over a USB tether gateway (or a persisted target address).
    #[default]
    DialPhone,
    /// Desktop listens and the phone dials over LAN. Never dials.
    ListenForPhone,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SyntheticParams {
    pub freq_hz: f32,
    pub amplitude: f32,
    pub roll_amplitude: f32,
}

impl Default for SyntheticParams {
    fn default() -> Self {
        Self {
            freq_hz: 0.2,
            amplitude: 0.15,
            roll_amplitude: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub vehicle: Vehicle,
    pub source: Source,
    pub density: f32,
    pub dot_radius_px: f32,
    pub opacity: f32,
    pub color: [u8; 4],
    pub edge_band: f32,
    pub gain: f32,
    pub invert: [bool; 3], // [x, y, roll]
    pub synthetic: SyntheticParams,
    /// Absent in config files written before 0.1.0; defaults to `DialPhone`.
    #[serde(default)]
    pub net_mode: NetMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vehicle: Vehicle::Car,
            source: Source::Sensor,
            density: 1.0,
            dot_radius_px: 6.0,
            opacity: 0.65,
            // Design cue-dot colour: "Horizon Data" #4CB3F0.
            color: [76, 179, 240, 255],
            edge_band: 0.12,
            gain: 1.0,
            invert: [false, false, false],
            synthetic: SyntheticParams::default(),
            net_mode: NetMode::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Dot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Live { rate_hz: f32 },
    Stale { since: f64 },
    Synthetic,
    Calibrating,
}
