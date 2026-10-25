// SPDX-License-Identifier: GPL-3.0-or-later

mod config_store;
mod settings_ui;
mod wayland_overlay;
mod x11_overlay;

use kineanchor_core::{Engine, Source};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = config_store::load_config();

    let args: Vec<String> = std::env::args().collect();
    let no_ui = args.iter().any(|a| a == "--no-ui");
    let force_synthetic = args.iter().any(|a| a == "--synthetic");

    if force_synthetic {
        config.source = Source::Synthetic;
    }

    let engine = Arc::new(Mutex::new(Engine::new(config)));
    let running = Arc::new(AtomicBool::new(true));

    // Determine overlay backend
    let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let wayland_layer_shell = has_wayland && wayland_overlay::WaylandOverlayApp::is_supported();

    let _overlay_handle = if wayland_layer_shell {
        println!("Using Wayland wlr-layer-shell overlay backend.");
        match wayland_overlay::spawn_wayland_overlay(engine.clone(), running.clone()) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!(
                    "Failed to initialize Wayland overlay: {e}. Falling back to X11/generic overlay."
                );
                x11_overlay::spawn_x11_overlay(engine.clone(), running.clone()).ok()
            }
        }
    } else {
        println!("Wayland layer-shell not detected. Using X11/fallback overlay backend.");
        x11_overlay::spawn_x11_overlay(engine.clone(), running.clone()).ok()
    };

    if no_ui {
        println!("KineAnchor running in headless overlay mode. Press Ctrl+C to exit.");
        let r = running.clone();
        ctrlc_handler(move || {
            r.store(false, Ordering::Relaxed);
        });
        while running.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        return Ok(());
    }

    // Launch eframe settings app on the main thread
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([460.0, 520.0])
            .with_min_inner_size([360.0, 420.0])
            .with_title("KineAnchor Settings"),
        ..Default::default()
    };

    let app_engine = engine.clone();
    let result = eframe::run_native(
        "kineanchor-settings",
        native_options,
        Box::new(|_cc| Ok(Box::new(settings_ui::SettingsApp::new(app_engine)))),
    );

    // Stop overlay when settings window closes
    running.store(false, Ordering::Relaxed);

    result.map_err(|e| format!("{e:?}").into())
}

fn ctrlc_handler<F: FnOnce() + Send + 'static>(f: F) {
    let mut f_opt = Some(f);
    let _ = std::thread::spawn(move || {
        if let Some(f) = f_opt.take() {
            let (_tx, rx) = std::sync::mpsc::channel::<()>();
            let _ = rx.recv();
            f();
        }
    });
}
