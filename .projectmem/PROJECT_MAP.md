# Project Map - KineAnchor

Status: P0–P5 complete. Read `docs/ARCHITECTURE.md` first.

## Project purpose
Cross-platform FOSS (GPL-3.0-or-later) motion-sickness overlay app for Linux (Wayland `wlr-layer-shell` + X11), Windows, and Android.

## Structure
- `crates/kineanchor-core/` — pure engine: types, protocol v1 codec + `PairingUri`, 3D gravity calibration (`basis_from_gravity`) + 2D horizontal PCA forward-axis auto-alignment (`align_forward_pca`), Car/Ship LPF/HPF filters, 0.5s watchdog + 1.0s static crossfade, synthetic generator, dot field, NDJSON record/replay. No I/O, no UI.
- `crates/kineanchor/` — desktop lib + binary: `Provider` trait, `NetProvider` (TCP NDJSON v1, QR pairing secret + persisted token auth, dial-phone-gateway + listen-LAN modes), `IioProvider` (`/sys/bus/iio/devices`), `ControllerProvider` (`INPUT_PROP_ACCELEROMETER` evdev IMU), `udev_hint` (sysfs USB tether & charge-only cable detection), overlay backends (Linux Wayland `wlr-layer-shell` + X11/Windows `eframe` fallback), eframe settings + QR UI, `examples/fake_phone.rs`, `tests/net_integration.rs`.
- `crates/kineanchor-android-ffi/` — zero-dependency C-ABI JNI bridge (`cdylib` + `rlib`) exposing `Engine` to `io.github.nandazahran.kineanchor.NativeEngine`.
- `android/` — Kotlin Android app (`Protocol.kt`, `NativeEngine.kt`, `OverlayService.kt` with Android 12+ `alpha <= 0.8` untrusted-touch compliance, `StreamService.kt` `specialUse` FGS, `MainActivity.kt` dual Overlay/Stream UI, `ProtocolTest.kt`).
- `packaging/linux/` — `.desktop`, NetworkManager keyfile (`90-kineanchor-tether.nmconnection`), `kineanchor-netsetup` polkit helper + `.policy`, ufw & firewalld service definitions, `PKGBUILD`.
- `packaging/windows/` — WiX installer manifest (`main.wxs`) and winget manifest (`io.github.nandazahran.KineAnchor.yaml`).
- `packaging/fdroid/` & `fastlane/metadata/android/en-US/` — F-Droid build recipe (`cargo-ndk` + Gradle) and store metadata.
- `packaging/gnome-extension/` — GNOME Shell 45–48 companion extension for Wayland without `wlr-layer-shell`.
- `docs/` — ARCHITECTURE, ROADMAP, PROTOCOL.
- `deny.toml` — dependency license gate (GPLv3-compatible only).

## Relationships
- Desktop and android-ffi depend on core; core depends on nothing platform-specific.
- Providers feed `MotionSample`s through a channel to the engine; the engine owns the synthetic and stale-crossfade fallback.
