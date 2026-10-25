# #0001 Clock mismatch: settings_ui used SystemTime::UNIX_EPOCH while overlays used Instant::now() and also mutated Engine by calling tick(100x100) from UI thread

- 2026-10-25T04:07:48Z `issue`: Clock mismatch: settings_ui used SystemTime::UNIX_EPOCH while overlays used Instant::now() and also mutated Engine by calling tick(100x100) from UI thread [crates/kineanchor/src/settings_ui.rs:41]
- 2026-10-25T04:12:59Z `fix`: Unified process-wide monotonic_secs() clock across overlays, provider router, and UI; added non-mutating Engine::status(now) so UI thread no longer calls tick(100x100) [crates/kineanchor/src/lib.rs:14]
