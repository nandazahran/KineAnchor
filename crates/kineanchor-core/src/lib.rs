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
pub use protocol::{Message, PROTOCOL_VERSION, decode_message, encode_message};
pub use record::{Player, Recorder};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_frequency_zero_crossings() {
        // Sample at 100 Hz over 10 seconds with 0.2 Hz target frequency
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

        // Over 10 seconds at 0.2 Hz, there are 2 complete cycles = 4 zero-crossings
        assert!(
            (3..=5).contains(&zero_crossings),
            "Expected ~4 zero crossings for 0.2 Hz over 10s, got {zero_crossings}"
        );
    }

    #[test]
    fn test_spring_displacement_bounded() {
        let mut engine = Engine::new(Config::default());
        let viewport = [1920.0, 1080.0];

        // Sustained extreme 10G acceleration for 5 seconds
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

            // Verify all dots stay within screen bounds + allowable margin
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

        // With zero motion offset
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
    fn test_protocol_line_too_long() {
        let long_line = "a".repeat(protocol::MAX_LINE_BYTES + 1);
        let res = decode_message(&long_line);
        assert_eq!(res, Err(protocol::DecodeError::LineTooLong));
    }

    #[test]
    fn test_cable_drop_stale_fallback() {
        let mut engine = Engine::new(Config::default());
        let viewport = [1920.0, 1080.0];

        // Receive sample at t = 1.0
        engine.push(
            MotionSample {
                t: 1.0,
                ax: 1.0,
                ay: 1.0,
                az: 0.0,
                gx: 0.0,
                gy: 0.0,
                gz: 0.0,
                gravity: None,
            },
            1.0,
        );
        let (_, status) = engine.tick(1.0, viewport);
        assert!(matches!(status, Status::Live { .. }));

        // At t = 1.6 (600ms without new sample), must transition to Stale
        let (dots, status) = engine.tick(1.6, viewport);
        assert!(matches!(status, Status::Stale { since } if (since - 1.0).abs() < 1e-4));
        assert!(
            !dots.is_empty(),
            "Must never freeze / drop dots on cable pull"
        );
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
