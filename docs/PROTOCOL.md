# KineAnchor — Transport protocol v1

Plain **TCP, newline-delimited JSON (NDJSON), UTF-8**. No WebSocket: it adds a dependency on both sides and buys nothing on a LAN. Port default **47474** (configurable).

Message types (one JSON object per line, max 1024 bytes; longer lines close the connection):

```jsonc
// 1. phone -> desktop, first line
{"v":1,"type":"hello","device":"Pixel 7","token":"<base64 32B>"}      // remembered pairing
{"v":1,"type":"hello","device":"Pixel 7","pair":"<secret from QR>"}    // first pairing
// 2. desktop -> phone
{"v":1,"type":"welcome","token":"<base64 32B>","name":"cachyos-laptop"}  // token issued on first pair
{"v":1,"type":"reject","reason":"bad_token"}
// 3. phone -> desktop, ~50 Hz
{"v":1,"type":"s","t":12345.678,"ax":0.12,"ay":-0.03,"az":0.01,"gx":0.001,"gy":0.0,"gz":0.02,"gvx":0.1,"gvy":9.7,"gvz":0.4}
// 4. either side, every 1 s if idle
{"v":1,"type":"ping"}
```

- `t`: sender monotonic seconds (`SystemClock.elapsedRealtimeNanos()/1e9`), used for filter `dt`. Staleness uses receiver arrival time, so the two clocks never need to agree.
- `ax..az`: `TYPE_LINEAR_ACCELERATION`. `gx..gz`: `TYPE_GYROSCOPE`. `gvx..gvz`: `TYPE_GRAVITY`. The gravity fields are optional and may be sent at a lower rate. They are needed because linear acceleration has gravity removed, so the receiver cannot otherwise know phone orientation.
- Unknown fields are ignored and an unknown `v` is rejected, which gives forward compatibility.
- Phone stays a dumb sensor puck. Calibration and "zero" happen on the receiver, so desktop → phone commands are not needed.
- Pairing QR: `motion://<ip>:<port>?pair=<secret>&fp=<desktop-id>`. The secret is single-use with a 5-minute TTL. The issued token is stored on both sides, which removes the need for a PIN on later connections.
- Security posture: plaintext on a LAN. Worst case of an injected stream = wrong dot motion. Mitigations: token required before samples are accepted, one client at a time, bounded line length, the listener runs only while phone mode is active, and the firewall rule is scoped to the tether interface where possible. TLS is not planned (it adds no meaningful value for this threat model). Revisit if scope changes.
- mDNS `_motionsync._tcp` (crate `mdns-sd`): nice-to-have, P5.

> Connection direction (ROADMAP Q2, default applied): the phone listens and the desktop dials the phone at the tether/hotspot gateway address. Desktop-listens remains the fallback for a shared Wi-Fi LAN. The protocol is symmetric after TCP connect, so either side can dial.

