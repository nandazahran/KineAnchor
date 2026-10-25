# projectmem - KineAnchor

_Last updated: 2026-10-25_

## Project purpose
The cross-platform app to help prevent motion sickness

## Recent issues
- No issues logged yet.

## Decisions
- UI: egui for desktop settings and dot painting; Wayland overlay via smithay-client-toolkit layer-shell + egui-wgpu (no winit); Android uses Kotlin shell + Rust core over JNI, not eframe. GPUI rejected. [docs/ARCHITECTURE.md]
- Transport v1: TCP NDJSON, port 47474, token pairing via QR; phone listens and desktop dials the tether/hotspot gateway, desktop-listens is the LAN fallback. [docs/PROTOCOL.md]
- Synthetic sway is experimental with low default amplitude (0.2 Hz is the most nauseogenic visual band, Diels & Howarth 2013); sensor loss falls back to decaying hold then static dots, not sway. [docs/ROADMAP.md]
- Tether NM profile matches by driver (rndis_host;cdc_ncm;cdc_ether) and uses route-metric=2000 instead of ipv4.never-default. [docs/ARCHITECTURE.md]
- Spike outcome: wayapp rejected (pins egui 0.35, forces tokio/hotpath, lacks multi-output and empty input region API). Use direct smithay-client-toolkit for Wayland layer-shell. [crates/kineanchor/src/]

## Notes
- Initial commit
- gotcha: dev laptop (CachyOS, Plasma 6.8.1 Wayland) has no IIO sensors and runs ufw, not firewalld; Android overlays must keep window alpha <= 0.8 or Android 12+ blocks touches underneath.

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
