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
    fn test_paused_freezes_cues_without_changing_status() {
        let config = Config {
            source: Source::Synthetic,
            ..Default::default()
        };
        let mut engine = Engine::new(config);
        engine.set_paused(true);
        let viewport = [800.0, 600.0];
        let (dx, dy, roll) = engine.current_offset();
        assert_eq!((dx, dy, roll), (0.0, 0.0, 0.0));

        let static_positions: Vec<(f32, f32)> = engine
            .tick(0.0, viewport)
            .0
            .iter()
            .map(|d| (d.x, d.y))
            .collect();
        assert!(
            !static_positions.is_empty(),
            "paused field must still paint"
        );

        let (dots, status) = engine.tick(1.0, viewport);
        assert!(matches!(status, Status::Synthetic));
        let later: Vec<(f32, f32)> = dots.iter().map(|d| (d.x, d.y)).collect();
        assert_eq!(static_positions, later, "paused dots must not move");
        assert_eq!(engine.current_offset(), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_config_ignores_removed_transport_keys() {
        // Config files written by the interim build carried `net_enabled`/`paused`.
        let legacy = r#"{"vehicle":"car","source":"sensor","density":1.0,
            "dot_radius_px":6.0,"opacity":0.65,"color":[76,179,240,255],
            "edge_band":0.12,"gain":1.0,"invert":[false,false,false],
            "synthetic":{"freq_hz":0.2,"amplitude":0.15,"roll_amplitude":0.05},
            "net_mode":"dial_phone","net_enabled":false,"paused":true}"#;
        let cfg: Config = serde_json::from_str(legacy).expect("legacy config must load");
        assert_eq!(cfg.net_mode, NetMode::DialPhone);
        assert_eq!(cfg.source, Source::Sensor);
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

    #[test]
    fn test_ship_mode_wave_band_energy_concentration() {
        let ship_cfg = Config {
            vehicle: Vehicle::Ship,
            ..Default::default()
        };

        let mut wave_phys = physics::PhysicsEngine::new();
        let mut vib_phys = physics::PhysicsEngine::new();

        let dt = 0.02_f32; // 50 Hz
        let mut wave_energy = 0.0_f32;
        let mut vib_energy = 0.0_f32;

        // Run 30 seconds at 50 Hz (1500 samples); measure steady-state energy over last 20s
        for i in 0..1500 {
            let t = i as f64 * dt as f64;
            let wave_heave = 2.0 * (2.0 * core::f32::consts::PI * 0.2 * t as f32).sin();
            let vib_heave = 2.0 * (2.0 * core::f32::consts::PI * 3.0 * t as f32).sin();

            let (_, wy, _) = wave_phys.update(
                &MotionSample {
                    t,
                    ax: 0.0,
                    ay: 0.0,
                    az: wave_heave,
                    gx: 0.0,
                    gy: 0.0,
                    gz: 0.0,
                    gravity: Some([0.0, 0.0, 9.81]),
                },
                dt,
                &ship_cfg,
            );
            let (_, vy, _) = vib_phys.update(
                &MotionSample {
                    t,
                    ax: 0.0,
                    ay: 0.0,
                    az: vib_heave,
                    gx: 0.0,
                    gy: 0.0,
                    gz: 0.0,
                    gravity: Some([0.0, 0.0, 9.81]),
                },
                dt,
                &ship_cfg,
            );

            if i >= 500 {
                wave_energy += wy * wy;
                vib_energy += vy * vy;
            }
        }

        let wave_rms = (wave_energy / 1000.0).sqrt();
        let vib_rms = (vib_energy / 1000.0).sqrt();
        assert!(
            wave_rms > vib_rms * 8.0,
            "Ship mode must pass 0.2Hz wave band (rms={wave_rms}) and attenuate 3Hz vibration (rms={vib_rms})"
        );
    }

    #[test]
    fn test_forward_axis_pca_auto_detection() {
        let mut phys = physics::PhysicsEngine::new();
        let yaw_true = 30.0_f32.to_radians();
        let (sin_y, cos_y) = yaw_true.sin_cos();

        let mut samples = Vec::new();
        for i in 0..50 {
            let t = i as f64 * 0.02;
            let a_long = 2.5 * (t as f32 * 2.0).sin();
            samples.push(MotionSample {
                t,
                ax: a_long * sin_y,
                ay: a_long * cos_y,
                az: 0.0,
                gx: 0.0,
                gy: 0.0,
                gz: 0.0,
                gravity: Some([0.0, 0.0, 9.81]),
            });
        }

        let estimated_yaw = phys
            .align_forward_pca(&samples)
            .expect("PCA should converge");
        assert!(
            (estimated_yaw - yaw_true).abs() < 1e-3,
            "Expected yaw {yaw_true}, got {estimated_yaw}"
        );

        // After PCA alignment, pure acceleration along rotated axis maps entirely to a_long (a_lat ≈ 0)
        let (a_lat, a_long, _, _, _, _) = phys.project_sample(&samples[10]);
        assert!(
            a_lat.abs() < 1e-4,
            "Expected zero lateral leak, got {a_lat}"
        );
        assert!(a_long.abs() > 0.5);
    }

    fn yaw_sample(t: f64, yaw: f32, a_long: f32, gravity: [f32; 3]) -> MotionSample {
        let (sin_y, cos_y) = yaw.sin_cos();
        MotionSample {
            t,
            ax: a_long * sin_y,
            ay: a_long * cos_y,
            az: 0.0,
            gx: 0.0,
            gy: 0.0,
            gz: 0.0,
            gravity: Some(gravity),
        }
    }

    /// Feeds a straight-line acceleration stream for `yaw` through the production intake path
    /// (`push_sample`, called by `Engine::push`) and returns the engine.
    fn converged_engine(yaw: f32) -> physics::PhysicsEngine {
        let mut phys = physics::PhysicsEngine::new();
        for i in 0..80 {
            let t = i as f64 * 0.02;
            let a_long = 2.5 * (t as f32 * 2.0).sin();
            let sample = yaw_sample(t, yaw, a_long, [0.0, 0.0, 9.81]);
            phys.push_sample(&sample, t);
        }
        phys
    }

    #[test]
    fn test_pca_activates_through_sample_intake_for_perpendicular_sequences() {
        // Two headings 90 degrees apart, each starting from the same gravity calibration.
        for yaw_deg in [45.0_f32, -45.0] {
            let yaw = yaw_deg.to_radians();
            let mut phys = converged_engine(yaw);

            // Motion along the true forward axis must land entirely in a_long after auto-detection.
            let probe = yaw_sample(2.0, yaw, 1.5, [0.0, 0.0, 9.81]);
            let (a_lat, a_long, _, _, _, _) = phys.project_sample(&probe);
            assert!(
                a_lat.abs() < 1e-3,
                "yaw {yaw_deg} deg: expected no lateral leak, got {a_lat}"
            );
            assert!(
                a_long > 1.4,
                "yaw {yaw_deg} deg: lost forward energy: {a_long}"
            );
        }
    }

    #[test]
    fn test_pca_ignores_degenerate_and_non_finite_input() {
        let mut phys = physics::PhysicsEngine::new();
        let gravity = [0.0, 0.0, 9.81];
        let rest = yaw_sample(0.0, 0.0, 0.0, gravity);
        phys.zero(Some(&rest));

        let probe = yaw_sample(0.0, 0.0, 1.0, gravity);
        let before = phys.project_sample(&probe);

        // Zero motion carries no directional information; NaN/inf frames must be dropped.
        for i in 0..80 {
            let t = i as f64 * 0.02;
            let sample = MotionSample {
                t,
                ax: if i % 2 == 0 { 0.0 } else { f32::NAN },
                ay: 0.0,
                az: 0.0,
                gx: 0.0,
                gy: f32::INFINITY,
                gz: 0.0,
                gravity: Some(gravity),
            };
            phys.push_sample(&sample, t);
        }

        let after = phys.project_sample(&probe);
        assert_eq!(
            before, after,
            "degenerate input must not rotate the calibrated axes"
        );
    }

    #[test]
    fn test_pca_window_resets_on_recalibration() {
        let yaw = 30.0_f32.to_radians();
        let mut phys = converged_engine(yaw);
        let gravity = [0.0, 0.0, 9.81];

        // Recalibration restores the gravity-derived forward axis (device +y).
        phys.zero(Some(&yaw_sample(2.0, yaw, 0.0, gravity)));
        let probed = yaw_sample(2.0, yaw, 1.5, gravity);
        let (a_lat, _, _, _, _, _) = phys.project_sample(&probed);
        assert!(
            a_lat.abs() > 0.1,
            "expected the PCA yaw to be discarded on zero(), got {a_lat}"
        );

        // Zoomed-out view of the same requirement: the window is empty, so one frame of motion
        // cannot immediately re-apply the old estimate.
        let mut fresh = physics::PhysicsEngine::new();
        fresh.zero(Some(&yaw_sample(0.0, 0.0, 0.0, gravity)));
        fresh.push_sample(&probed, 0.0);
        let (fresh_lat, _, _, _, _, _) = fresh.project_sample(&probed);
        assert_eq!(a_lat, fresh_lat);
    }
}
