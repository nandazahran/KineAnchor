// SPDX-License-Identifier: GPL-3.0-or-later

use kineanchor_core::synthetic::SyntheticGenerator;
use kineanchor_core::{
    DEFAULT_PORT, Message, PROTOCOL_VERSION, PairingUri, SyntheticParams, Vehicle, decode_message,
    encode_message,
};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut addr = format!("127.0.0.1:{DEFAULT_PORT}");
    let mut pair_secret: Option<String> = None;
    let mut token: Option<String> = None;
    let mut listen_mode = false;
    let mut max_samples: Option<usize> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--uri" => {
                if let Some(u) = args.next() {
                    let parsed = PairingUri::parse(&u)?;
                    addr = format!("{}:{}", parsed.host, parsed.port);
                    pair_secret = Some(parsed.pair_secret);
                }
            }
            "--addr" => {
                if let Some(a) = args.next() {
                    addr = a;
                }
            }
            "--pair" => pair_secret = args.next(),
            "--token" => token = args.next(),
            "--listen" => listen_mode = true,
            "--count" => {
                if let Some(c) = args.next() {
                    max_samples = c.parse().ok();
                }
            }
            _ => {}
        }
    }

    let mut stream = if listen_mode {
        println!("fake_phone listening on {addr}...");
        let listener = TcpListener::bind(&addr)?;
        let (s, peer) = listener.accept()?;
        println!("Accepted desktop connection from {peer}");
        s
    } else {
        println!("fake_phone dialing {addr}...");
        TcpStream::connect(&addr)?
    };
    stream.set_nodelay(true)?;

    let hello = Message::Hello {
        v: PROTOCOL_VERSION,
        device: "FakePhone-Rust".to_string(),
        token,
        pair: pair_secret,
    };
    stream.write_all(encode_message(&hello)?.as_bytes())?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    match decode_message(&line)? {
        Message::Welcome {
            token: issued_tok,
            name,
            ..
        } => {
            println!("Connected to desktop '{name}', token={issued_tok}");
        }
        Message::Reject { reason, .. } => {
            return Err(format!("Rejected by desktop: {reason}").into());
        }
        other => {
            return Err(format!("Unexpected handshake response: {other:?}").into());
        }
    }

    let start = Instant::now();
    let params = SyntheticParams {
        freq_hz: 0.25,
        amplitude: 2.0,
        roll_amplitude: 0.1,
    };

    let mut sent = 0usize;
    loop {
        if let Some(limit) = max_samples {
            if sent >= limit {
                break;
            }
        }
        let t = start.elapsed().as_secs_f64();
        let sample = SyntheticGenerator::sample(t, Vehicle::Car, &params);
        let msg = Message::from(sample);
        stream.write_all(encode_message(&msg)?.as_bytes())?;
        sent += 1;
        std::thread::sleep(Duration::from_millis(20)); // 50 Hz
    }

    Ok(())
}
