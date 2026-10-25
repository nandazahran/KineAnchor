// SPDX-License-Identifier: GPL-3.0-or-later

pub mod config_store;
pub mod net_provider;
pub mod provider;
pub mod settings_ui;
pub mod udev_hint;
pub mod wayland_overlay;
pub mod x11_overlay;

use std::sync::OnceLock;
use std::time::Instant;

/// Returns monotonic elapsed seconds from a single process-wide epoch.
pub fn monotonic_secs() -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64()
}
