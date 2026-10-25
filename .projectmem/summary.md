# projectmem - KineAnchor

_Last updated: 2026-10-25_

## Project purpose
Cross-platform FOSS (GPL-3.0-or-later) motion-sickness overlay app for Linux (Wayland `wlr-layer-shell` + X11), Windows, and Android.

## Recent issues
- [DONE] #0001 Clock mismatch: settings_ui used SystemTime::UNIX_EPOCH while overlays used Instant::now() and also mutated Engine by calling tick(100x100) from UI thread [crates/kineanchor/src/settings_ui.rs:41] -> Unified process-wide monotonic_secs() clock across overlays, provider router, and UI; added non-mutating Engine::status(now) so UI thread no longer calls tick(100x100) [crates/kineanchor/src/lib.rs:14] (fixed)

## Decisions
- UI: egui for desktop settings and dot painting; Wayland overlay via smithay-client-toolkit layer-shell + egui-wgpu (no winit); Android uses Kotlin shell + Rust core over JNI, not eframe. GPUI rejected. [docs/ARCHITECTURE.md]
- Transport v1: TCP NDJSON, port 47474, token pairing via QR; phone listens and desktop dials the tether/hotspot gateway, desktop-listens is the LAN fallback. [docs/PROTOCOL.md]
- Synthetic sway is experimental with low default amplitude (0.2 Hz is the most nauseogenic visual band, Diels & Howarth 2013); sensor loss falls back to decaying hold then static dots, not sway. [docs/ROADMAP.md]
- Tether NM profile matches by driver (rndis_host;cdc_ncm;cdc_ether) and uses route-metric=2000 instead of ipv4.never-default. [docs/ARCHITECTURE.md]
- Spike outcome: wayapp rejected (pins egui 0.35, forces tokio/hotpath, lacks multi-output and empty input region API). Use direct smithay-client-toolkit for Wayland layer-shell. [crates/kineanchor/src/]
- Use zero-dependency C-ABI JNI functions with packed (f32, f32) jlong returns in kineanchor-android-ffi to avoid jni crate dependencies and JVM heap allocations at 60 fps [crates/kineanchor-android-ffi/src/lib.rs:1]

## Notes
- Initial commit
- gotcha: dev laptop (CachyOS, Plasma 6.8.1 Wayland) has no IIO sensors and runs ufw, not firewalld; Android overlays must keep window alpha <= 0.8 or Android 12+ blocks touches underneath.
- gotcha: smithay-client-toolkit and wayland-client must be gated under [target.'cfg(target_os = "linux")'.dependencies] so windows-latest CI builds crates/kineanchor cleanly [crates/kineanchor/Cargo.toml:10]

## Key files
- `.gitattributes`
- `.gitignore`
- `LICENSE`
- `README.md`
- `0.2`
- `ipv4.never`
- `6.8.1`
- `0.8`
- `0.35`

## Open questions
- None logged yet.
