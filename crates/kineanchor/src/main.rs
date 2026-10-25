// SPDX-License-Identifier: GPL-3.0-or-later

use kineanchor::config_store::{load_config, load_pairing, save_pairing};
use kineanchor::monotonic_secs;
use kineanchor::net_provider::{AuthState, NetProvider};
use kineanchor::provider::{Provider, ProviderEvent};
use kineanchor::{settings_ui, wayland_overlay, x11_overlay};
use kineanchor_core::{Engine, Source};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = monotonic_secs(); // Initialize monotonic clock epoch

    let mut config = load_config();
    let persisted_pairing = load_pairing();

    let args: Vec<String> = std::env::args().collect();
    let no_ui = args.iter().any(|a| a == "--no-ui");
    let force_synthetic = args.iter().any(|a| a == "--synthetic");

    if force_synthetic {
        config.source = Source::Synthetic;
    }

    let mut initial_auth = AuthState::default();
    initial_auth.apply_persisted(persisted_pairing);
    let auth = Arc::new(Mutex::new(initial_auth));
    let connected_device: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let engine = Arc::new(Mutex::new(Engine::new(config)));
    let running = Arc::new(AtomicBool::new(true));

    // Spawn phone NetProvider + event router thread
    let (tx, rx) = mpsc::channel::<ProviderEvent>();
    let net_provider = Box::new(NetProvider::new(auth.clone()));
    let net_stop = running.clone();
    std::thread::Builder::new()
        .name("net-provider".into())
        .spawn(move || {
            net_provider.run(tx, net_stop);
        })?;

    let router_engine = engine.clone();
    let router_auth = auth.clone();
    let router_dev = connected_device.clone();
    let router_running = running.clone();
    std::thread::Builder::new()
        .name("provider-router".into())
        .spawn(move || {
            while router_running.load(Ordering::Relaxed) {
                match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(ProviderEvent::Sample(sample)) => {
                        let now = monotonic_secs();
                        if let Ok(mut eng) = router_engine.lock() {
                            eng.push(sample, now);
                        }
                    }
                    Ok(ProviderEvent::Connected(dev)) => {
                        *router_dev.lock().unwrap() = Some(dev);
                    }
                    Ok(ProviderEvent::Disconnected(_)) => {
                        *router_dev.lock().unwrap() = None;
                    }
                    Ok(ProviderEvent::Paired { .. }) => {
                        let persisted = router_auth.lock().unwrap().to_persisted();
                        save_pairing(&persisted);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;

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
            .with_inner_size([480.0, 580.0])
            .with_min_inner_size([380.0, 460.0])
            .with_title("KineAnchor Settings"),
        ..Default::default()
    };

    let app_engine = engine.clone();
    let app_auth = auth.clone();
    let app_dev = connected_device.clone();
    let result = eframe::run_native(
        "kineanchor-settings",
        native_options,
        Box::new(|_cc| {
            Ok(Box::new(settings_ui::SettingsApp::new(
                app_engine, app_auth, app_dev,
            )))
        }),
    );

    // Stop overlay & providers when settings window closes
    running.store(false, Ordering::Relaxed);

    result.map_err(|e| format!("{e:?}").into())
}

fn ctrlc_handler<F: FnOnce() + Send + 'static>(f: F) {
    let mut f_opt = Some(f);
    let _ = std::thread::spawn(move || {
        if let Some(f) = f_opt.take() {
            let (_tx, rx) = mpsc::channel::<()>();
            let _ = rx.recv();
            f();
        }
    });
}
