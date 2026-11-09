// SPDX-License-Identifier: GPL-3.0-or-later

use crate::provider::{Provider, ProviderEvent};
use crate::udev_hint::{CableHint, probe_cable_hint};
use kineanchor_core::{
    DEFAULT_PORT, MAX_LINE_BYTES, Message, MotionSample, NetMode, PROTOCOL_VERSION, decode_message,
    encode_message,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PAIR_SECRET_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedPairing {
    pub paired_token: Option<String>,
    pub paired_device: Option<String>,
    pub desktop_name: String,
    pub target_addr: Option<String>,
    pub port: u16,
}

#[derive(Debug)]
pub struct AuthState {
    pub paired_token: Option<String>,
    pub paired_device: Option<String>,
    pub active_pair_secret: Option<(String, Instant)>,
    pub desktop_name: String,
    pub target_addr: Option<String>,
    pub port: u16,
    /// Session direction, kept live so a settings change applies without a restart.
    pub mode: NetMode,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new(DEFAULT_PORT)
    }
}

impl AuthState {
    pub fn new(port: u16) -> Self {
        let hostname = std::fs::read_to_string("/etc/hostname")
            .ok()
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .or_else(|| std::env::var("HOSTNAME").ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "kineanchor-desktop".to_string());
        Self {
            paired_token: None,
            paired_device: None,
            active_pair_secret: None,
            desktop_name: hostname,
            target_addr: None,
            port,
            mode: NetMode::default(),
        }
    }

    pub fn generate_pair_secret(&mut self) -> String {
        let secret = random_hex(12);
        self.active_pair_secret = Some((secret.clone(), Instant::now()));
        secret
    }

    pub fn current_pair_secret(&mut self) -> String {
        if let Some((ref sec, created)) = self.active_pair_secret {
            if created.elapsed() < PAIR_SECRET_TTL {
                return sec.clone();
            }
        }
        self.generate_pair_secret()
    }

    pub fn to_persisted(&self) -> PersistedPairing {
        PersistedPairing {
            paired_token: self.paired_token.clone(),
            paired_device: self.paired_device.clone(),
            desktop_name: self.desktop_name.clone(),
            target_addr: self.target_addr.clone(),
            port: self.port,
        }
    }

    pub fn apply_persisted(&mut self, p: PersistedPairing) {
        self.paired_token = p.paired_token;
        self.paired_device = p.paired_device;
        if !p.desktop_name.is_empty() {
            self.desktop_name = p.desktop_name;
        }
        self.target_addr = p.target_addr;
        if p.port != 0 {
            self.port = p.port;
        }
    }
}

pub fn random_hex(bytes_len: usize) -> String {
    let mut buf = vec![0u8; bytes_len];
    let mut filled = false;
    if let Ok(mut f) = File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            filled = true;
        }
    }
    if !filled {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id() as u128;
        let mut seed = nanos ^ (pid << 32) ^ 0x9e37_79b9_7f4a_7c15_u128;
        for b in &mut buf {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *b = (seed >> 24) as u8;
        }
    }
    let mut out = String::with_capacity(bytes_len * 2);
    for b in buf {
        use core::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn read_bounded_line<R: BufRead>(reader: &mut R, buf: &mut String) -> io::Result<usize> {
    buf.clear();
    let mut limited = reader.take((MAX_LINE_BYTES + 2) as u64);
    limited.read_line(buf)
}

pub struct NetProvider {
    auth: Arc<Mutex<AuthState>>,
    bind_addr: String,
    bound_addr_out: Option<Arc<Mutex<Option<SocketAddr>>>>,
}

impl NetProvider {
    pub fn new(auth: Arc<Mutex<AuthState>>) -> Self {
        let port = auth.lock().unwrap().port;
        Self {
            auth,
            bind_addr: format!("0.0.0.0:{port}"),
            bound_addr_out: None,
        }
    }

    /// Selects the session direction. `DialPhone` never binds, `ListenForPhone` never dials.
    ///
    /// The choice is stored in the shared [`AuthState`] so a settings change applies to the
    /// running provider without a restart.
    pub fn with_mode(self, mode: NetMode) -> Self {
        self.auth.lock().unwrap().mode = mode;
        self
    }

    /// Binds a caller-chosen listener for integration tests. Implies `ListenForPhone`.
    pub fn with_bind_addr(
        auth: Arc<Mutex<AuthState>>,
        bind_addr: impl Into<String>,
        bound_addr_out: Option<Arc<Mutex<Option<SocketAddr>>>>,
    ) -> Self {
        auth.lock().unwrap().mode = NetMode::ListenForPhone;
        Self {
            auth,
            bind_addr: bind_addr.into(),
            bound_addr_out,
        }
    }
}

pub fn handle_session(
    mut stream: TcpStream,
    auth: &Arc<Mutex<AuthState>>,
    tx: &Sender<ProviderEvent>,
    stop: &Arc<AtomicBool>,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));

    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();

    // 1. Read Hello line
    match read_bounded_line(&mut reader, &mut line) {
        Ok(0) | Err(_) => return,
        Ok(_) => {}
    }

    let Ok(Message::Hello {
        v: _,
        device,
        token,
        pair,
    }) = decode_message(&line)
    else {
        let reject = Message::Reject {
            v: PROTOCOL_VERSION,
            reason: "expected_hello".to_string(),
        };
        if let Ok(enc) = encode_message(&reject) {
            let _ = stream.write_all(enc.as_bytes());
        }
        return;
    };

    // 2. Verify token or pairing secret
    let auth_result = {
        let mut guard = auth.lock().unwrap();
        let token_ok = matches!(
            (&token, &guard.paired_token),
            (Some(t), Some(saved)) if !saved.is_empty() && t == saved
        );

        if token_ok {
            let tok = guard.paired_token.clone().unwrap();
            guard.paired_device = Some(device.clone());
            Ok((tok, guard.desktop_name.clone(), false))
        } else {
            let pair_ok = matches!(
                (&pair, &guard.active_pair_secret),
                (Some(p), Some((sec, created)))
                    if !sec.is_empty() && p == sec && created.elapsed() < PAIR_SECRET_TTL
            );

            if pair_ok {
                // Single-use pairing secret: consume and issue long-lived token
                guard.active_pair_secret = None;
                let new_token = random_hex(32);
                guard.paired_token = Some(new_token.clone());
                guard.paired_device = Some(device.clone());
                Ok((new_token, guard.desktop_name.clone(), true))
            } else {
                Err("bad_token")
            }
        }
    };

    let (issued_token, desktop_name, newly_paired) = match auth_result {
        Ok(v) => v,
        Err(reason) => {
            let reject = Message::Reject {
                v: PROTOCOL_VERSION,
                reason: reason.to_string(),
            };
            if let Ok(enc) = encode_message(&reject) {
                let _ = stream.write_all(enc.as_bytes());
            }
            return;
        }
    };

    let welcome = Message::Welcome {
        v: PROTOCOL_VERSION,
        token: issued_token.clone(),
        name: desktop_name,
    };
    let Ok(welcome_line) = encode_message(&welcome) else {
        return;
    };
    if stream.write_all(welcome_line.as_bytes()).is_err() {
        return;
    }

    if newly_paired {
        let _ = tx.send(ProviderEvent::Paired {
            device: device.clone(),
            token: issued_token,
        });
    }
    let _ = tx.send(ProviderEvent::Connected(device.clone()));

    let _ = stream.set_read_timeout(Some(Duration::from_millis(600)));

    // 3. Stream samples until EOF, error, or stop.
    while !stop.load(Ordering::Relaxed) {
        match read_bounded_line(&mut reader, &mut line) {
            Ok(0) => break, // EOF
            Ok(_) => match decode_message(&line) {
                Ok(msg @ Message::Sample { .. }) => {
                    if let Ok(sample) = MotionSample::try_from(msg) {
                        let _ = tx.send(ProviderEvent::Sample(sample));
                    }
                }
                Ok(Message::Ping { .. }) => {
                    if let Ok(pong) = encode_message(&Message::Ping {
                        v: PROTOCOL_VERSION,
                    }) {
                        if stream.write_all(pong.as_bytes()).is_err() {
                            break;
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break, // Invalid JSON, bad version, or >1024B line closes connection
            },
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }

    let _ = tx.send(ProviderEvent::Disconnected(device));
}

impl Provider for NetProvider {
    fn name(&self) -> &'static str {
        "phone-net"
    }

    fn run(self: Box<Self>, tx: Sender<ProviderEvent>, stop: Arc<AtomicBool>) {
        loop {
            let mode = self.auth.lock().unwrap().mode;
            match mode {
                NetMode::ListenForPhone => self.run_listener(&tx, &stop),
                NetMode::DialPhone => self.run_dialer(&tx, &stop),
            }
            if stop.load(Ordering::Relaxed) {
                return;
            }
        }
    }
}

impl NetProvider {
    /// Binds the configured address and serves inbound phone sessions. Never dials.
    fn run_listener(&self, tx: &Sender<ProviderEvent>, stop: &Arc<AtomicBool>) {
        let listener = match TcpListener::bind(&self.bind_addr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "Net: cannot listen on {} ({e}); no phone sessions will be accepted",
                    self.bind_addr
                );
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        if let Some(ref out) = self.bound_addr_out {
            *out.lock().unwrap() = listener.local_addr().ok();
        }

        while !stop.load(Ordering::Relaxed) {
            if self.auth.lock().unwrap().mode != NetMode::ListenForPhone {
                return;
            }
            match listener.accept() {
                Ok((stream, _peer)) => handle_session(stream, &self.auth, tx, stop),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    /// Dials the phone (persisted target or USB tether gateway). Never binds a listener.
    fn run_dialer(&self, tx: &Sender<ProviderEvent>, stop: &Arc<AtomicBool>) {
        let mut last_dial = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(Instant::now);

        while !stop.load(Ordering::Relaxed) {
            if self.auth.lock().unwrap().mode != NetMode::DialPhone {
                return;
            }
            if last_dial.elapsed() >= Duration::from_millis(500) {
                last_dial = Instant::now();
                let dial_target = {
                    let g = self.auth.lock().unwrap();
                    let port = g.port;
                    g.target_addr
                        .clone()
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| match probe_cable_hint() {
                            CableHint::TetherActive {
                                gateway: Some(gw), ..
                            } => Some(format!("{gw}:{port}")),
                            _ => None,
                        })
                };

                if let Some(target_str) = dial_target {
                    if let Ok(mut addrs) = target_str.to_socket_addrs() {
                        if let Some(addr) = addrs.next() {
                            if let Ok(stream) =
                                TcpStream::connect_timeout(&addr, Duration::from_millis(250))
                            {
                                handle_session(stream, &self.auth, tx, stop);
                                continue;
                            }
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
