// SPDX-License-Identifier: GPL-3.0-or-later

//! Records a live phone stream to an NDJSON capture usable by `replay_stream`.
//!
//! `cargo run -p kineanchor --example record_stream -- <host:port> <output.ndjson>`
//!
//! The desktop dials the phone, runs the production `handle_session` authentication against the
//! pairing data cached under the app config directory, and writes every sample that reaches the
//! provider channel through the shared `kineanchor_core::Recorder`. The resulting file is exactly
//! the protocol v1 wire format, so it can be replayed without translation.

use kineanchor::config_store::load_pairing;
use kineanchor::net_provider::{AuthState, handle_session};
use kineanchor::provider::ProviderEvent;
use kineanchor_core::Recorder;
use std::fs::File;
use std::io::BufWriter;
use std::net::TcpStream;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, atomic::AtomicBool, mpsc};
use std::time::Duration;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(target), Some(out_path)) = (args.next(), args.next()) else {
        eprintln!("usage: record_stream <host:port> <output.ndjson>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: record_stream <host:port> <output.ndjson>");
        return ExitCode::from(2);
    }

    match record(&target, &out_path) {
        Ok(count) => {
            println!("recorded {count} samples to {out_path}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn record(target: &str, out_path: &str) -> Result<usize, String> {
    let mut auth_state = AuthState::new(0);
    auth_state.apply_persisted(load_pairing());
    if auth_state.paired_token.as_deref().is_none_or(str::is_empty) {
        return Err(format!(
            "{target}: no paired phone token in the KineAnchor config dir; pair once in the app first"
        ));
    }
    let auth = Arc::new(Mutex::new(auth_state));

    let stream = TcpStream::connect(target).map_err(|e| format!("connect {target}: {e}"))?;
    stream
        .set_nodelay(true)
        .map_err(|e| format!("nodelay {target}: {e}"))?;

    let file = File::create(out_path).map_err(|e| format!("{out_path}: {e}"))?;
    let mut recorder = Recorder::new(BufWriter::new(file));

    let (tx, rx) = mpsc::channel::<ProviderEvent>();
    let stop = Arc::new(AtomicBool::new(false));
    let session_stop = stop.clone();

    // Reuse the production handshake: same token check, same rejection paths, same framing.
    let session = std::thread::Builder::new()
        .name("record-session".into())
        .spawn(move || handle_session(stream, &auth, &tx, &session_stop))
        .map_err(|e| format!("spawn session thread: {e}"))?;

    let mut count = 0usize;
    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(ProviderEvent::Sample(sample)) => {
                recorder
                    .record(&sample)
                    .map_err(|e| format!("write {out_path}: {e}"))?;
                count += 1;
            }
            Ok(ProviderEvent::Connected(name)) => {
                println!(
                    "authenticated with {}",
                    if name.is_empty() { target } else { &name }
                );
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if session.is_finished() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = session.join();
    recorder
        .flush()
        .map_err(|e| format!("flush {out_path}: {e}"))?;
    Ok(count)
}
