// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform-independent motion cue engine.
//!
//! Pipeline: motion samples -> calibration -> filters -> spring-damper -> dot field.
//! This crate performs no I/O and has no UI; see `docs/ARCHITECTURE.md`.

pub mod dots;
pub mod engine;
pub mod physics;
pub mod protocol;
pub mod record;
pub mod synthetic;
pub mod types;

pub use engine::Engine;
pub use protocol::{
    DEFAULT_PORT, DecodeError, MAX_LINE_BYTES, Message, PROTOCOL_VERSION, PairingUri,
    decode_message, encode_message,
};
pub use record::{Player, Recorder};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_frequency_zero_crossings() {
        let freq = 0.2;
        let params = SyntheticParams {
            freq_hz: freq,
            amplitude: 1.0,
            roll_amplitude: 0.1,
        };
        let mut zero_crossings = 0;
        let mut prev_ax = 0.0;

        for step in 0..1000 {
            let t = step as f64 * 0.01;
            let sample = synthetic::SyntheticGenerator::sample(t, Vehicle::Car, &params);
            if step > 0
                && ((prev_ax <= 0.0 && sample.ax > 0.0) || (prev_ax >= 0.0 && sample.ax < 0.0))
            {
                zero_crossings += 1;
            }
            prev_ax = sample.ax;
        }

        assert!(
            (3..=5).contains(&zero_crossings),
            "Expected ~4 zero crossings for 0.2 Hz over 10s, got {zero_crossings}"
        );
    }

    #[test]
    fn test_spring_displacement_bounded() {
        let mut engine = Engine::new(Config::default());
        let viewport = [1920.0, 1080.0];

        for step in 0..300 {
            let now = step as f64 * (1.0 / 60.0);
            let sample = MotionSample {
                t: now,
                ax: 50.0,
                ay: 98.1,
                az: 0.0,
                gx: 0.0,
                gy: 0.0,
                gz: 2.0,
                gravity: None,
            };
            engine.push(sample, now);
            let (dots, status) = engine.tick(now, viewport);
            assert!(matches!(status, Status::Live { .. }));

            for dot in dots {
                assert!(
                    dot.x >= -20.0 && dot.x <= viewport[0] + 20.0,
                    "Dot x out of bounds: {}",
                    dot.x
                );
                assert!(
                    dot.y >= -20.0 && dot.y <= viewport[1] + 20.0,
                    "Dot y out of bounds: {}",
                    dot.y
                );
            }
        }
    }

    #[test]
    fn test_dots_stay_in_edge_band() {
        let config = Config {
            edge_band: 0.15,
            ..Default::default()
        };
        let mut engine = Engine::new(config);
        let viewport = [1000.0, 1000.0];

        let (dots, _) = engine.tick(0.0, viewport);
        assert!(!dots.is_empty(), "Dot field must not be empty");

        let band_w = 1000.0 * 0.15;
        let band_h = 1000.0 * 0.15;

        for dot in dots {
            let in_edge = dot.x <= band_w * 1.5
                || dot.x >= 1000.0 - band_w * 1.5
                || dot.y <= band_h * 1.5
                || dot.y >= 1000.0 - band_h * 1.5;
            assert!(
                in_edge,
                "Dot placed in center screen: ({}, {})",
                dot.x, dot.y
            );
        }
    }

    #[test]
    fn test_protocol_roundtrip() {
        let sample = MotionSample {
            t: 1234.56,
            ax: 0.12,
            ay: -0.34,
            az: 9.81,
            gx: 0.01,
            gy: -0.02,
            gz: 0.03,
            gravity: Some([0.1, 0.2, 9.7]),
        };
        let msg = Message::from(sample);
        let encoded = encode_message(&msg).expect("Encoding failed");
        assert!(encoded.ends_with('\n'));
        assert!(encoded.len() <= protocol::MAX_LINE_BYTES);

        let decoded = decode_message(&encoded).expect("Decoding failed");
        assert_eq!(msg, decoded);

        let sample_back = MotionSample::try_from(decoded).expect("Conversion back failed");
        assert_eq!(sample.t, sample_back.t);
        assert_eq!(sample.ax, sample_back.ax);
        assert_eq!(sample.gravity, sample_back.gravity);
    }

    #[test]
    fn test_protocol_line_too_long_and_bad_version() {
        let long_line = "a".repeat(protocol::MAX_LINE_BYTES + 1);
        assert_eq!(decode_message(&long_line), Err(DecodeError::LineTooLong));

        let bad_v = r#"{"v":2,"type":"future_unknown_type","foo":123}"#;
        assert_eq!(
            decode_message(bad_v),
            Err(DecodeError::UnsupportedVersion(2))
        );

        let forward_compat = r#"{"v":1,"type":"ping","extra_field":"ignored"}"#;
        assert_eq!(decode_message(forward_compat), Ok(Message::Ping { v: 1 }));
    }

    #[test]
    fn test_golden_vectors_ndjson() {
        let raw = include_str!("../../../tests/vectors/sample_stream.ndjson");
        let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 6);

        let m0 = decode_message(lines[0]).expect("line 0");
        assert!(matches!(m0, Message::Hello { v: 1, ref device, .. } if device == "Pixel 7"));

        let m1 = decode_message(lines[1]).expect("line 1");
        assert!(matches!(m1, Message::Welcome { v: 1, .. }));

        for line in &lines[2..5] {
            let msg = decode_message(line).expect("sample line");
            let s = MotionSample::try_from(msg).expect("sample conversion");
            assert_eq!(s.gravity, Some([0.0, 0.0, 9.81]));
        }

        let m5 = decode_message(lines[5]).expect("line 5");
        assert_eq!(m5, Message::Ping { v: 1 });
    }

    #[test]
    fn test_pairing_uri_roundtrip() {
        let p = PairingUri {
            host: "192.168.42.129".to_string(),
            port: DEFAULT_PORT,
            pair_secret: "sec_987654".to_string(),
            fingerprint: "cachyos-laptop".to_string(),
        };
        let uri = p.to_uri();
        assert_eq!(
            uri,
            "motion://192.168.42.129:47474?pair=sec_987654&fp=cachyos-laptop"
        );
        let parsed = PairingUri::parse(&uri).expect("parse URI");
        assert_eq!(parsed, p);
    }

    #[test]
    fn test_cable_drop_stale_fallback() {
        let mut engine = Engine::new(Config::default());
        let viewport = [1920.0, 1080.0];

        // Feed active motion up to t = 1.0
        for i in 0..=50 {
            let t = i as f64 * 0.02;
            engine.push(
                MotionSample {
                    t,
                    ax: 2.0,
                    ay: 3.0,
                    az: 0.0,
                    gx: 0.0,
                    gy: 0.0,
                    gz: 0.2,
                    gravity: Some([0.0, 0.0, 9.81]),
                },
                t,
            );
            let (dots, status) = engine.tick(t, viewport);
            assert!(matches!(status, Status::Live { .. }));
            assert!(!dots.is_empty());
        }

        // At t = 1.50 (500ms after last sample at 1.0): still Live
        let (_, status_500ms) = engine.tick(1.50, viewport);
        assert!(matches!(status_500ms, Status::Live { .. }));

        let (live_x, live_y, _) = engine.current_offset();
        assert!(live_x.abs() > 1.0 && live_y.abs() > 1.0);

        // Just past 500ms (t = 1.51): transitions to Stale
        let (dots, status_stale) = engine.tick(1.51, viewport);
        assert!(matches!(status_stale, Status::Stale { since } if (since - 1.0).abs() < 1e-4));
        assert!(!dots.is_empty());

        // Mid-crossfade at t = 2.0 (1.0s after last sample, 0.5s into 1.0s fade): blend == 0.5
        let (dots_mid, _) = engine.tick(2.0, viewport);
        assert!(!dots_mid.is_empty());
        assert!((engine.stale_blend() - 0.5).abs() < 1e-3);
        let (mid_x, mid_y, _) = engine.current_offset();
        assert!((mid_x - live_x * 0.5).abs() < 1e-2);
        assert!((mid_y - live_y * 0.5).abs() < 1e-2);

        // At t = 2.5 (1.5s after last sample): crossfade reaches 100% static fallback
        let (dots_end, status_end) = engine.tick(2.5, viewport);
        assert!(matches!(status_end, Status::Stale { .. }));
        assert!(!dots_end.is_empty(), "tick() must never return empty frame");
        assert!((engine.stale_blend() - 1.0).abs() < 1e-5);
        let (end_x, end_y, end_r) = engine.current_offset();
        assert!(end_x.abs() < 1e-4 && end_y.abs() < 1e-4 && end_r.abs() < 1e-4);
    }

    #[test]
    fn test_calibration_rotated_stream_matches_unrotated() {
        let viewport = [1920.0, 1080.0];
        let mut engine_flat = Engine::new(Config::default());
        let mut engine_tilted = Engine::new(Config::default());

        // 35 degree pitch around device +X (e.g. phone in angled dash mount)
        // Rotation around +X by theta maps:
        // y' =  y * cos(theta) + z * sin(theta)
        // z' = -y * sin(theta) + z * cos(theta)
        let theta = 35.0_f32.to_radians();
        let (sin_t, cos_t) = theta.sin_cos();
        let rotate_x = |v: [f32; 3]| -> [f32; 3] {
            [
                v[0],
                v[1] * cos_t + v[2] * sin_t,
                -v[1] * sin_t + v[2] * cos_t,
            ]
        };

        let g_flat = [0.0, 0.0, 9.81];
        let g_tilted = rotate_x(g_flat);

        // Initial rest sample + zero() calibration
        engine_flat.push(
            MotionSample {
                t: 0.0,
                ax: 0.0,
                ay: 0.0,
                az: 0.0,
                gx: 0.0,
                gy: 0.0,
                gz: 0.0,
                gravity: Some(g_flat),
            },
            0.0,
        );
        engine_flat.zero();

        engine_tilted.push(
            MotionSample {
                t: 0.0,
                ax: 0.0,
                ay: 0.0,
                az: 0.0,
                gx: 0.05, // also inject constant gyro bias on tilted phone
                gy: -0.03,
                gz: 0.02,
                gravity: Some(g_tilted),
            },
            0.0,
        );
        engine_tilted.zero();

        for step in 1..=100 {
            let t = step as f64 * 0.02;
            let a_veh = [
                1.5 * (t as f32 * 1.3).sin(),
                2.2 * (t as f32 * 0.9).cos(),
                0.3 * (t as f32 * 2.0).sin(),
            ];
            let w_veh = [0.0, 0.0, 0.25 * (t as f32 * 1.1).sin()];

            let s_flat = MotionSample {
                t,
                ax: a_veh[0],
                ay: a_veh[1],
                az: a_veh[2],
                gx: w_veh[0],
                gy: w_veh[1],
                gz: w_veh[2],
                gravity: Some(g_flat),
            };

            let a_rot = rotate_x(a_veh);
            let w_rot = rotate_x(w_veh);
            let s_tilted = MotionSample {
                t,
                ax: a_rot[0],
                ay: a_rot[1],
                az: a_rot[2],
                gx: w_rot[0] + 0.05,
                gy: w_rot[1] - 0.03,
                gz: w_rot[2] + 0.02,
                gravity: Some(g_tilted),
            };

            engine_flat.push(s_flat, t);
            engine_tilted.push(s_tilted, t);

            let (dots_flat, _) = engine_flat.tick(t, viewport);
            let (dots_tilted, _) = engine_tilted.tick(t, viewport);

            assert_eq!(dots_flat.len(), dots_tilted.len());
            for (df, dt_dot) in dots_flat.iter().zip(dots_tilted.iter()) {
                assert!(
                    (df.x - dt_dot.x).abs() < 1e-3 && (df.y - dt_dot.y).abs() < 1e-3,
                    "Mismatch at step {step}: flat=({}, {}) vs tilted=({}, {})",
                    df.x,
                    df.y,
                    dt_dot.x,
                    dt_dot.y
                );
            }
        }
    }

    #[test]
    fn test_config_serde_json() {
        let config = Config::default();
        let serialized = serde_json::to_string(&config).expect("Serialize config failed");
        let deserialized: Config =
            serde_json::from_str(&serialized).expect("Deserialize config failed");
        assert_eq!(config.vehicle, deserialized.vehicle);
        assert_eq!(config.density, deserialized.density);
        assert_eq!(config.color, deserialized.color);
    }

    #[test]
    fn test_record_and_replay() {
        let mut buffer = Vec::new();
        let mut recorder = Recorder::new(&mut buffer);

        let samples = vec![
            MotionSample {
                t: 100.0,
                ax: 0.1,
                ay: 0.2,
                az: 9.8,
                gx: 0.0,
                gy: 0.0,
                gz: 0.05,
                gravity: Some([0.0, 0.0, 9.8]),
            },
            MotionSample {
                t: 100.02,
                ax: 0.15,
                ay: 0.22,
                az: 9.81,
                gx: 0.01,
                gy: -0.01,
                gz: 0.06,
                gravity: Some([0.0, 0.0, 9.8]),
            },
        ];

        for s in &samples {
            recorder.record(s).expect("Record failed");
        }
        recorder.flush().expect("Flush failed");

        let mut player = Player::new(buffer.as_slice());
        let mut replayed = Vec::new();
        while let Some(res) = player.next_sample() {
            replayed.push(res.expect("Replay failed"));
        }

        assert_eq!(samples.len(), replayed.len());
        assert_eq!(samples[0].t, replayed[0].t);
        assert_eq!(samples[1].gz, replayed[1].gz);
    }
}
