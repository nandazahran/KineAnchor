# Project Map - KineAnchor

Status: P2 complete. Read `docs/ARCHITECTURE.md` first.

## Project purpose
The cross-platform app to help prevent motion sickness
## Structure
- `crates/kineanchor-core/` — pure engine: types, protocol v1 codec + `PairingUri`, 3D gravity calibration + LPF/HPF filters, 0.5s watchdog + 1.0s static crossfade, synthetic generator, dot field, NDJSON record/replay. No I/O, no UI.
- `crates/kineanchor/` — desktop lib + binary: `Provider` trait, `NetProvider` (TCP NDJSON v1, QR pairing secret + persisted token auth, both dial-phone-gateway and listen-LAN modes), `udev_hint` (sysfs USB tether & charge-only cable detection), overlay backends (Wayland `wlr-layer-shell` + X11/winit fallback), eframe settings + QR UI, `examples/fake_phone.rs`, `tests/net_integration.rs`.
- `crates/kineanchor-android-ffi/` — JNI cdylib for the Android Kotlin shell (P4).
- `android/` — Kotlin sender app (`Protocol.kt`, `StreamService.kt` `specialUse` FGS, `MainActivity.kt` 3-tap tether + QR flow, `ProtocolTest.kt` golden vector JUnit test).
- `packaging/linux/` — `.desktop`, NetworkManager keyfile (`90-kineanchor-tether.nmconnection`), `kineanchor-netsetup` polkit helper + `.policy`, ufw & firewalld service definitions, `PKGBUILD`.
- `docs/` — ARCHITECTURE, ROADMAP, PROTOCOL.
- `deny.toml` — dependency license gate (GPLv3-compatible only).

## Relationships
- Desktop and android-ffi depend on core; core depends on nothing platform-specific.
- Providers feed `MotionSample`s through a channel to the engine; the engine owns the synthetic fallback.
