// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::MotionSample;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    #[serde(rename = "hello")]
    Hello {
        v: u32,
        device: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pair: Option<String>,
    },
    #[serde(rename = "welcome")]
    Welcome { v: u32, token: String, name: String },
    #[serde(rename = "reject")]
    Reject { v: u32, reason: String },
    #[serde(rename = "s")]
    Sample {
        v: u32,
        t: f64,
        ax: f32,
        ay: f32,
        az: f32,
        gx: f32,
        gy: f32,
        gz: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gvx: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gvy: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gvz: Option<f32>,
    },
    #[serde(rename = "ping")]
    Ping { v: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    LineTooLong,
    InvalidJson(String),
    UnsupportedVersion(u32),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LineTooLong => write!(f, "Line exceeds max bytes limit ({MAX_LINE_BYTES})"),
            Self::InvalidJson(err) => write!(f, "Invalid JSON: {err}"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported protocol version {v}"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn encode_message(msg: &Message) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(msg)?;
    s.push('\n');
    Ok(s)
}

pub fn decode_message(line: &str) -> Result<Message, DecodeError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DecodeError::LineTooLong);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(DecodeError::InvalidJson("Empty line".to_string()));
    }
    let msg: Message =
        serde_json::from_str(trimmed).map_err(|e| DecodeError::InvalidJson(e.to_string()))?;

    let version = match &msg {
        Message::Hello { v, .. } => *v,
        Message::Welcome { v, .. } => *v,
        Message::Reject { v, .. } => *v,
        Message::Sample { v, .. } => *v,
        Message::Ping { v } => *v,
    };

    if version != PROTOCOL_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    Ok(msg)
}

impl From<MotionSample> for Message {
    fn from(s: MotionSample) -> Self {
        let (gvx, gvy, gvz) = match s.gravity {
            Some([x, y, z]) => (Some(x), Some(y), Some(z)),
            None => (None, None, None),
        };
        Message::Sample {
            v: PROTOCOL_VERSION,
            t: s.t,
            ax: s.ax,
            ay: s.ay,
            az: s.az,
            gx: s.gx,
            gy: s.gy,
            gz: s.gz,
            gvx,
            gvy,
            gvz,
        }
    }
}

impl TryFrom<Message> for MotionSample {
    type Error = &'static str;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        match msg {
            Message::Sample {
                t,
                ax,
                ay,
                az,
                gx,
                gy,
                gz,
                gvx,
                gvy,
                gvz,
                ..
            } => {
                let gravity = match (gvx, gvy, gvz) {
                    (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                };
                Ok(MotionSample {
                    t,
                    ax,
                    ay,
                    az,
                    gx,
                    gy,
                    gz,
                    gravity,
                })
            }
            _ => Err("Message is not a MotionSample"),
        }
    }
}
