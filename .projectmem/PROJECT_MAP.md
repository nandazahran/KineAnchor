# Project Map - KineAnchor

Status: P0 scaffold. Read `docs/ARCHITECTURE.md` first.

## Project purpose
The cross-platform app to help prevent motion sickness
## Structure
- `crates/kineanchor-core/` — pure engine: types, protocol v1 codec, calibration, filters, synthetic, dot field, config. No I/O, no UI.
- `crates/kineanchor/` — desktop binary: providers (phone stream, IIO), overlay backends (Wayland layer-shell, X11/Windows winit), eframe settings.
- `crates/kineanchor-android-ffi/` — JNI cdylib for the Android Kotlin shell.
- `android/` (P2/P4) — Kotlin app: stream sender + overlay service.
- `docs/` — ARCHITECTURE, ROADMAP, PROTOCOL.
- `deny.toml` — dependency license gate (GPLv3-compatible only).

## Relationships
- Desktop and android-ffi depend on core; core depends on nothing platform-specific.
- Providers feed `MotionSample`s through a channel to the engine; the engine owns the synthetic fallback.
