// SPDX-License-Identifier: GPL-3.0-or-later

//! Replays a recorded NDJSON sample capture through the real engine.
//!
//! `cargo run -p kineanchor --example replay_stream -- capture.ndjson`
//!
//! Each line is decoded with the shared protocol codec and pushed through `Engine::push` using
//! the same monotonic arrival clock the live router uses, so dot offsets from a replay match the
//! live session at corresponding timestamps. Protocol v1 only; nothing here writes to the network.

use kineanchor::monotonic_secs;
use kineanchor_core::{Engine, MAX_LINE_BYTES, Status, decode_message};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: replay_stream <capture.ndjson>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: replay_stream <capture.ndjson>");
        return ExitCode::from(2);
    }

    match replay(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn replay(path: &str) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut reader = BufReader::new(file);

    let mut engine = Engine::new(Default::default());
    let mut line = String::new();
    let mut line_no = 0usize;
    let mut samples = 0usize;
    let mut first_t: Option<f64> = None;
    let mut last_t = 0.0_f64;

    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| format!("{path}:{}: read error: {e}", line_no + 1))?;
        if read == 0 {
            break;
        }
        line_no += 1;

        if read > MAX_LINE_BYTES + 1 {
            return Err(format!(
                "{path}:{line_no}: line exceeds {MAX_LINE_BYTES} bytes ({read} bytes); refusing to decode"
            ));
        }

        let message = decode_message(&line).map_err(|e| format!("{path}:{line_no}: {e}"))?;
        let sample = kineanchor_core::MotionSample::try_from(message)
            .map_err(|e| format!("{path}:{line_no}: {e}"))?;

        // Same arrival clock the live provider router uses, so the engine cannot tell a replay
        // from a session that just delivered these samples.
        first_t.get_or_insert(sample.t);
        engine.push(sample, monotonic_secs());
        samples += 1;
        last_t = sample.t;
    }

    if samples == 0 {
        return Err(format!("{path}: no samples decoded"));
    }

    let now = monotonic_secs();
    let status = match engine.status(now) {
        Status::Live { rate_hz } => format!("live ({rate_hz:.1} Hz)"),
        Status::Stale { since } => format!("stale (since {since:.2}s)"),
        Status::Synthetic => "synthetic".to_string(),
        Status::Calibrating => "calibrating".to_string(),
    };
    let (dots, _) = engine.tick(now, [1920.0, 1080.0]);
    let first = dots.first().copied();

    match first {
        Some(dot) => println!(
            "replayed {samples} samples from {path} (t={:.3}..{last_t:.3}s): status={status}, first_dot=({:.1}, {:.1}, r={:.1}, a={:.2})",
            first_t.unwrap_or(0.0),
            dot.x,
            dot.y,
            dot.r,
            dot.alpha
        ),
        None => println!(
            "replayed {samples} samples from {path} (t={:.3}..{last_t:.3}s): status={status}, no dots",
            first_t.unwrap_or(0.0)
        ),
    }
    Ok(())
}
