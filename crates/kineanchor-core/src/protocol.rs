// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::MotionSample;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 1024;
pub const DEFAULT_PORT: u16 = 47474;

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

#[derive(Deserialize)]
struct VersionEnvelope {
    v: u32,
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

    let env: VersionEnvelope =
        serde_json::from_str(trimmed).map_err(|e| DecodeError::InvalidJson(e.to_string()))?;
    if env.v != PROTOCOL_VERSION {
        return Err(DecodeError::UnsupportedVersion(env.v));
    }

    let msg: Message =
        serde_json::from_str(trimmed).map_err(|e| DecodeError::InvalidJson(e.to_string()))?;

    Ok(msg)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingUri {
    pub host: String,
    pub port: u16,
    pub pair_secret: String,
    pub fingerprint: String,
}

impl PairingUri {
    pub fn to_uri(&self) -> String {
        format!(
            "motion://{}:{}?pair={}&fp={}",
            self.host, self.port, self.pair_secret, self.fingerprint
        )
    }

    pub fn parse(uri: &str) -> Result<Self, &'static str> {
        let rest = uri
            .trim()
            .strip_prefix("motion://")
            .ok_or("Missing motion:// scheme")?;
        let (addr_part, query_part) = rest.split_once('?').ok_or("Missing query parameters")?;
        let (host, port_str) = addr_part.rsplit_once(':').ok_or("Missing port")?;
        if host.is_empty() {
            return Err("Empty host");
        }
        let port: u16 = port_str.parse().map_err(|_| "Invalid port")?;

        let mut pair_secret = None;
        let mut fingerprint = String::new();
        for kv in query_part.split('&') {
            if let Some((k, v)) = kv.split_once('=') {
                match k {
                    "pair" | "pin" => pair_secret = Some(v.to_string()),
                    "fp" => fingerprint = v.to_string(),
                    _ => {}
                }
            }
        }
        let pair_secret = pair_secret
            .filter(|s| !s.is_empty())
            .ok_or("Missing pair secret")?;
        Ok(Self {
            host: host.to_string(),
            port,
            pair_secret,
            fingerprint,
        })
    }
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
