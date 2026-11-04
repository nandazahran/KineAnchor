# Contributing to KineAnchor

KineAnchor is a Rust workspace plus a Kotlin Android shell. This file covers building from source,
running the developer tools, and the checks CI enforces. For installing a release instead, see the
[releases page](https://github.com/nandazahran/KineAnchor/releases).

## Prerequisites

- Rust 1.85 or newer (`edition = "2024"`).
- **Linux desktop**: `pkg-config`, `libwayland-dev`, `libxkbcommon-dev`, `libx11-dev`,
  `libxcursor-dev`, `libxrandr-dev`, `libxi-dev`, `libgl1-mesa-dev`.
- **Windows desktop**: MSVC Rust toolchain (Visual Studio Build Tools).
- **AppImage packaging**: `appimagetool` (run with `APPIMAGE_EXTRACT_AND_RUN=1`),
  `desktop-file-utils`, `file`.
- **Android**: JDK 17, Android SDK platform 35, build-tools 35.0.0, NDK `27.2.12479018`,
  `cargo-ndk` 3.5.4, and the Rust targets `aarch64-linux-android`,
  `armv7-linux-androideabi`, `x86_64-linux-android`.

## Build and run

```bash
# Build and run with settings + the click-through overlay
cargo run --release -p kineanchor

# Overlay only, synthetic motion (no sensors, no UI)
cargo run --release -p kineanchor -- --no-ui --synthetic
```

## Workspace Layout

- [`crates/kineanchor-core/`](crates/kineanchor-core): Pure Rust core (no I/O, no UI) — types, NDJSON v1 protocol codec, 3D gravity calibration (`basis_from_gravity`), 2D PCA forward-axis auto-detection (`align_forward_pca`), spring-damper physics, edge-band dot field, and stream recorder/player.
- [`crates/kineanchor/`](crates/kineanchor): Desktop binary and library — Wayland layer-shell overlay, X11/Windows `eframe` overlay, `NetProvider`, `IioProvider`, `ControllerProvider`, sysfs USB tether / charge-only cable hint, and `egui` settings + QR pairing UI.
- [`crates/kineanchor-android-ffi/`](crates/kineanchor-android-ffi): Zero-dependency C-ABI JNI bridge exposing `Engine` to the Android Kotlin shell.
- [`android/`](android): Kotlin Android app supporting both on-phone click-through overlay (`OverlayService`, capped at `alpha <= 0.8` for Android 12+ untrusted-touch pass-through) and 50 Hz PC sensor puck streaming (`StreamService`).
- [`packaging/`](packaging):
  - [`packaging/linux/`](packaging/linux): `.desktop`, AppImage builder (`build-appimage.sh`), NetworkManager USB tether profile (`route-metric=2000`), Polkit helper (`kineanchor-netsetup`), `ufw`/`firewalld` service definitions, and Arch/CachyOS `PKGBUILD`.
  - [`packaging/windows/`](packaging/windows): WiX MSI (`main.wxs`) and `winget` (`io.github.nandazahran.KineAnchor.yaml`) manifests.
  - [`packaging/fdroid/`](packaging/fdroid) & [`fastlane/`](fastlane): F-Droid build recipe and store metadata.
  - [`packaging/gnome-extension/`](packaging/gnome-extension): GNOME Shell 45–48 top-chrome overlay extension.

## Developer tools

```bash
# Simulate a 50 Hz phone sensor stream over loopback
cargo run -p kineanchor --example fake_phone -- --listen 127.0.0.1:47474

# Record a live session to NDJSON (desktop dials the phone; uses the stored pairing token)
cargo run -p kineanchor --example record_stream -- <phone-host>:47474 capture.ndjson

# Replay a capture through the real engine and print the resulting status and dot offset
cargo run -p kineanchor --example replay_stream -- capture.ndjson
```

`replay_stream` rejects malformed JSON, unsupported protocol versions, and lines over 1024 bytes
with a line-numbered error, so a corrupt capture cannot be silently half-applied.

## Checks

CI runs these on Linux and Windows; run them before opening a pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## Android build

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o android/app/src/main/jniLibs build --release -p kineanchor-android-ffi

cd android
./gradlew --no-daemon testDebugUnitTest assembleDebug assembleRelease
```

## Packaging

```bash
# Linux AppImage (after cargo build --release -p kineanchor)
./packaging/linux/build-appimage.sh x86_64 target/release/kineanchor \
  kineanchor-<version>-x86_64.AppImage
```

- Windows installer: [`packaging/windows/main.wxs`](packaging/windows/main.wxs) (WiX) and the
  [`winget` manifest](packaging/windows/io.github.nandazahran.KineAnchor.yaml).
- Android/F-Droid: [`packaging/fdroid/`](packaging/fdroid).
- Arch/CachyOS: [`packaging/linux/PKGBUILD`](packaging/linux/PKGBUILD).

## Documentation

- [Architecture Specification](docs/ARCHITECTURE.md)
- [Transport Protocol v1 (`NDJSON-TCP`)](docs/PROTOCOL.md)
- [Roadmap & Risk Matrix](docs/ROADMAP.md)
