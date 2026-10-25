# KineAnchor

FOSS cross-platform motion-sickness overlay (`GPL-3.0-or-later`) for **Linux (Wayland `wlr-layer-shell` + X11)**, **Windows**, and **Android**.

KineAnchor renders subtle, click-through peripheral dots that drift opposite to vehicle acceleration, braking, turning, heave, and roll so visual cues agree with vestibular signals (Apple Vehicle Motion Cues convention: accelerate = dots down, brake = dots up, turn left = dots right, turn right = dots left).

## Features

- **100% Offline-First**: Zero cloud dependencies, zero telemetry, zero Google Play Services (F-Droid clean). Works in cars and ships with mobile data and Wi-Fi completely off.
- **Linux Wayland Native Overlay**: Uses `wlr-layer-shell` (`smithay-client-toolkit`) on `Layer::Overlay` with an empty `WlRegion` input mask for true click-through on KDE Plasma 6, Sway, Hyprland, Wayfire, river, niri, and labwc. Falls back to X11/winit or the companion GNOME Shell extension (`packaging/gnome-extension/`).
- **Phone as USB/LAN Sensor Puck**: Laptops rarely have built-in IMUs. Plug your Android phone in via USB tethering (or local hotspot/LAN), scan the offline QR code (`motion://<ip>:47474?pair=<secret>&fp=<id>`) once, and stream 50 Hz `MotionSample` frames over TCP NDJSON v1.
- **Cable-Drop Safety**: If the USB cable disconnects mid-drive, the watchdog transitions to `Stale` after 500 ms and smoothly crossfades to static dots over 1.0 s — never freezing or jumping.
- **Car & Ship Modes + Synthetic Sway**:
  - **Car**: Responsive 2.5 Hz low-pass filter + yaw rate coupling for road acceleration, braking, and turns.
  - **Ship**: 0.1–0.3 Hz wave band filter + roll rotation for ferries and vessels.
  - **Synthetic Sway**: ~0.2 Hz experimental synthetic wave generator (Diels & Howarth 2013 band) requiring no sensors.

## Workspace Layout

- [`crates/kineanchor-core/`](crates/kineanchor-core): Pure Rust core (no I/O, no UI) — types, NDJSON v1 protocol codec, 3D gravity calibration (`basis_from_gravity`), 2D PCA forward-axis auto-detection (`align_forward_pca`), spring-damper physics, edge-band dot field, and stream recorder/player.
- [`crates/kineanchor/`](crates/kineanchor): Desktop binary and library — Wayland layer-shell overlay, X11/Windows `eframe` overlay, `NetProvider`, `IioProvider`, `ControllerProvider`, sysfs USB tether / charge-only cable hint, and `egui` settings + QR pairing UI.
- [`crates/kineanchor-android-ffi/`](crates/kineanchor-android-ffi): Zero-dependency C-ABI JNI bridge exposing `Engine` to the Android Kotlin shell.
- [`android/`](android): Kotlin Android app supporting both on-phone click-through overlay (`OverlayService`, capped at `alpha <= 0.8` for Android 12+ untrusted-touch pass-through) and 50 Hz PC sensor puck streaming (`StreamService`).
- [`packaging/`](packaging):
  - [`packaging/linux/`](packaging/linux): `.desktop`, NetworkManager USB tether profile (`route-metric=2000`), Polkit helper (`kineanchor-netsetup`), `ufw`/`firewalld` service definitions, and Arch/CachyOS `PKGBUILD`.
  - [`packaging/windows/`](packaging/windows): WiX MSI (`main.wxs`) and `winget` (`io.github.nandazahran.KineAnchor.yaml`) manifests.
  - [`packaging/fdroid/`](packaging/fdroid) & [`fastlane/`](fastlane): F-Droid build recipe and store metadata.
  - [`packaging/gnome-extension/`](packaging/gnome-extension): GNOME Shell 45–48 top-chrome overlay extension.

## Quick Start (Desktop)

```bash
# Build and run with settings + QR pairing UI
cargo run --release -p kineanchor

# Run headless overlay in synthetic mode (works inside isolated network namespace)
cargo run --release -p kineanchor -- --no-ui --synthetic

# Simulate a 50 Hz phone sensor stream over loopback
cargo run -p kineanchor --example fake_phone -- --listen 127.0.0.1:47474
```

## Documentation

- [Architecture Specification](docs/ARCHITECTURE.md)
- [Transport Protocol v1 (`NDJSON-TCP`)](docs/PROTOCOL.md)
- [Roadmap & Risk Matrix](docs/ROADMAP.md)

## Prior Art & References

KineAnchor is an independent clean-room Rust/Kotlin implementation under `GPL-3.0-or-later`. Prior art and scientific literature consulted:

- [`dev.davidv.motionsickness`](https://github.com/DavidVentura/motion-sickness-app) — Android motion sickness overlay on F-Droid (`GPL-3.0-only`, referenced for Android overlay concepts; no code copied).
- [`monperrus/motion-cues`](https://github.com/monperrus/motion-cues) — Linux X11 + PyQt6 implementation.
- [`smithynb/motion-cues`](https://github.com/smithynb/motion-cues) & [`Exudius/motionsick`](https://github.com/Exudius/motionsick) — Desktop motion cue prototypes.
- Commercial / proprietary references: Apple Vehicle Motion Cues (iOS 18+), KineStop, EasyQueasy, MotionEase.
- Diels, C., & Howarth, P. A. (2013). *Frequency characteristics of visually induced motion sickness (VIMS)*. Human Factors, 55(3), 595–604.

## License

Licensed under the [GNU General Public License v3.0 or later (`GPL-3.0-or-later`)](LICENSE).
