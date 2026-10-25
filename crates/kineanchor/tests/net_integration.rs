// SPDX-License-Identifier: GPL-3.0-or-later

use kineanchor::net_provider::{AuthState, NetProvider};
use kineanchor::provider::{Provider, ProviderEvent};
use kineanchor_core::{Message, MotionSample, PROTOCOL_VERSION, decode_message, encode_message};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

fn wait_for_bound_addr(slot: &Arc<Mutex<Option<SocketAddr>>>) -> SocketAddr {
    for _ in 0..100 {
        if let Some(addr) = *slot.lock().unwrap() {
            return addr;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("NetProvider did not bind within timeout");
}

#[test]
fn test_net_provider_pair_disconnect_and_reconnect_with_token() {
    let mut auth_state = AuthState::new(0);
    let pair_secret = auth_state.generate_pair_secret();
    let auth = Arc::new(Mutex::new(auth_state));

    let bound_slot = Arc::new(Mutex::new(None));
    let provider = Box::new(NetProvider::with_bind_addr(
        auth.clone(),
        "127.0.0.1:0",
        Some(bound_slot.clone()),
    ));

    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    let handle = std::thread::spawn(move || {
        provider.run(tx, stop_clone);
    });

    let server_addr = wait_for_bound_addr(&bound_slot);

    // 1. Attempt connection with bad token -> must be rejected
    {
        let mut bad_client = TcpStream::connect(server_addr).expect("connect bad client");
        let hello = Message::Hello {
            v: PROTOCOL_VERSION,
            device: "AttackerPhone".to_string(),
            token: Some("wrong_token".to_string()),
            pair: None,
        };
        bad_client
            .write_all(encode_message(&hello).unwrap().as_bytes())
            .unwrap();
        let mut reader = BufReader::new(bad_client);
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        let msg = decode_message(&resp).expect("decode reject");
        assert_eq!(
            msg,
            Message::Reject {
                v: PROTOCOL_VERSION,
                reason: "bad_token".to_string()
            }
        );
    }

    // 2. First connection with QR pair_secret -> issues token, streams sample, then drops cable
    let issued_token = {
        let mut client = TcpStream::connect(server_addr).expect("connect first pair");
        let hello = Message::Hello {
            v: PROTOCOL_VERSION,
            device: "Pixel 7".to_string(),
            token: None,
            pair: Some(pair_secret.clone()),
        };
        client
            .write_all(encode_message(&hello).unwrap().as_bytes())
            .unwrap();

        let mut reader = BufReader::new(client.try_clone().unwrap());
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        let Message::Welcome { token, .. } = decode_message(&resp).expect("decode welcome") else {
            panic!("Expected Welcome message, got {resp}");
        };

        // Send a motion sample
        let sample = MotionSample {
            t: 10.0,
            ax: 1.25,
            ay: -0.5,
            az: 0.0,
            gx: 0.0,
            gy: 0.0,
            gz: 0.1,
            gravity: Some([0.0, 0.0, 9.81]),
        };
        client
            .write_all(encode_message(&Message::from(sample)).unwrap().as_bytes())
            .unwrap();

        // Drop client (simulating cable disconnect / killed fake_phone)
        drop(reader);
        drop(client);
        token
    };

    // Verify events: Paired -> Connected -> Sample -> Disconnected
    let ev1 = rx.recv_timeout(Duration::from_secs(2)).expect("ev1");
    assert!(matches!(ev1, ProviderEvent::Paired { ref device, .. } if device == "Pixel 7"));

    let ev2 = rx.recv_timeout(Duration::from_secs(2)).expect("ev2");
    assert_eq!(ev2, ProviderEvent::Connected("Pixel 7".to_string()));

    let ev3 = rx.recv_timeout(Duration::from_secs(2)).expect("ev3");
    assert!(matches!(ev3, ProviderEvent::Sample(s) if (s.ax - 1.25).abs() < 1e-5));

    let ev4 = rx.recv_timeout(Duration::from_secs(2)).expect("ev4");
    assert_eq!(ev4, ProviderEvent::Disconnected("Pixel 7".to_string()));

    // Verify single-use pair_secret is consumed (reusing it must fail)
    {
        let mut replay_client = TcpStream::connect(server_addr).expect("connect replay");
        let hello = Message::Hello {
            v: PROTOCOL_VERSION,
            device: "Pixel 7".to_string(),
            token: None,
            pair: Some(pair_secret),
        };
        replay_client
            .write_all(encode_message(&hello).unwrap().as_bytes())
            .unwrap();
        let mut reader = BufReader::new(replay_client);
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        assert!(matches!(decode_message(&resp), Ok(Message::Reject { .. })));
    }

    // 3. Reconnect using saved issued_token without re-pairing
    {
        let mut client2 = TcpStream::connect(server_addr).expect("reconnect with token");
        let hello = Message::Hello {
            v: PROTOCOL_VERSION,
            device: "Pixel 7".to_string(),
            token: Some(issued_token.clone()),
            pair: None,
        };
        client2
            .write_all(encode_message(&hello).unwrap().as_bytes())
            .unwrap();

        let mut reader = BufReader::new(client2.try_clone().unwrap());
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        let Message::Welcome { token, .. } = decode_message(&resp).expect("welcome 2") else {
            panic!("Expected Welcome on reconnect");
        };
        assert_eq!(token, issued_token);

        let sample2 = MotionSample {
            t: 20.0,
            ax: -2.0,
            ay: 1.0,
            az: 0.0,
            gx: 0.0,
            gy: 0.0,
            gz: -0.2,
            gravity: Some([0.0, 0.0, 9.81]),
        };
        client2
            .write_all(encode_message(&Message::from(sample2)).unwrap().as_bytes())
            .unwrap();
    }

    let ev5 = rx.recv_timeout(Duration::from_secs(2)).expect("ev5");
    assert_eq!(ev5, ProviderEvent::Connected("Pixel 7".to_string()));

    let ev6 = rx.recv_timeout(Duration::from_secs(2)).expect("ev6");
    assert!(matches!(ev6, ProviderEvent::Sample(s) if (s.ax + 2.0).abs() < 1e-5));

    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
}

#[test]
fn test_net_provider_dials_phone_listener() {
    // Phone listens (Q2 primary direction), desktop NetProvider dials phone
    let phone_listener = TcpListener::bind("127.0.0.1:0").expect("bind phone listener");
    let phone_addr = phone_listener.local_addr().unwrap();

    let mut auth_state = AuthState::new(phone_addr.port());
    auth_state.paired_token = Some("pre_shared_tok_123".to_string());
    auth_state.target_addr = Some(phone_addr.to_string());
    let auth = Arc::new(Mutex::new(auth_state));

    let provider = Box::new(NetProvider::with_bind_addr(auth, "127.0.0.1:0", None));
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    let handle = std::thread::spawn(move || {
        provider.run(tx, stop_clone);
    });

    let (mut phone_stream, _) = phone_listener.accept().expect("phone accept desktop dial");
    let hello = Message::Hello {
        v: PROTOCOL_VERSION,
        device: "TetherPhone".to_string(),
        token: Some("pre_shared_tok_123".to_string()),
        pair: None,
    };
    phone_stream
        .write_all(encode_message(&hello).unwrap().as_bytes())
        .unwrap();

    let mut reader = BufReader::new(phone_stream.try_clone().unwrap());
    let mut welcome_line = String::new();
    reader.read_line(&mut welcome_line).unwrap();
    assert!(matches!(
        decode_message(&welcome_line),
        Ok(Message::Welcome { .. })
    ));

    let sample = MotionSample {
        t: 5.0,
        ax: 0.75,
        ay: 1.5,
        az: 0.0,
        gx: 0.0,
        gy: 0.0,
        gz: 0.0,
        gravity: Some([0.0, 0.0, 9.81]),
    };
    phone_stream
        .write_all(encode_message(&Message::from(sample)).unwrap().as_bytes())
        .unwrap();
    drop(reader);
    drop(phone_stream);

    let ev_conn = rx.recv_timeout(Duration::from_secs(2)).expect("connected");
    assert_eq!(ev_conn, ProviderEvent::Connected("TetherPhone".to_string()));

    let ev_sample = rx.recv_timeout(Duration::from_secs(2)).expect("sample");
    assert!(matches!(ev_sample, ProviderEvent::Sample(s) if (s.ax - 0.75).abs() < 1e-5));

    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
}
