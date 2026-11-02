// SPDX-License-Identifier: GPL-3.0-or-later

use kineanchor::config_store::{load_config, load_pairing, save_pairing};
use kineanchor::controller_provider::ControllerProvider;
use kineanchor::iio_provider::IioProvider;
use kineanchor::monotonic_secs;
use kineanchor::net_provider::{AuthState, NetProvider};
use kineanchor::provider::{Provider, ProviderEvent};
#[cfg(target_os = "linux")]
use kineanchor::wayland_overlay;
use kineanchor::x11_overlay;
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
    initial_auth.mode = config.net_mode;
    let auth = Arc::new(Mutex::new(initial_auth));
    let connected_device: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let engine = Arc::new(Mutex::new(Engine::new(config)));

    // Single shutdown flag: false while the app runs, true once the UI closes or another thread
    // fails. Every provider loop treats it as "stop", so it must start false.
    let running = Arc::new(AtomicBool::new(true));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Spawn phone NetProvider + optional local IIO / Controller providers + event router thread
    let (tx, rx) = mpsc::channel::<ProviderEvent>();
    let iio = IioProvider::new();
    if iio.is_available() {
        let iio_tx = tx.clone();
        let iio_stop = shutdown.clone();
        let _ = std::thread::Builder::new()
            .name("iio-provider".into())
            .spawn(move || Box::new(iio).run(iio_tx, iio_stop));
    }
    let controller = ControllerProvider::new();
    if controller.is_available() {
        let ctl_tx = tx.clone();
        let ctl_stop = shutdown.clone();
        let _ = std::thread::Builder::new()
            .name("controller-provider".into())
            .spawn(move || Box::new(controller).run(ctl_tx, ctl_stop));
    }
    let net_provider = Box::new(NetProvider::new(auth.clone()));
    let net_stop = shutdown.clone();
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
            // Keep draining until the senders are gone, then exit: a dropped sender means every
            // provider has stopped.
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

    // Overlay backend: Wayland layer-shell owns its own thread and event queue; every other
    // target (X11, Windows) drives settings and overlay from one eframe event loop below.
    #[cfg(target_os = "linux")]
    {
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if has_wayland && wayland_overlay::WaylandOverlayApp::is_supported() {
            println!("Using Wayland wlr-layer-shell overlay backend.");
            let overlay_running = Arc::new(AtomicBool::new(true));
            let overlay_exited = Arc::new(AtomicBool::new(false));
            let handle = wayland_overlay::spawn_wayland_overlay(
                engine.clone(),
                overlay_running.clone(),
                overlay_exited.clone(),
            )
            .map_err(|e| format!("Failed to initialize Wayland overlay: {e}"))?;

            let result = if no_ui {
                // The layer-shell overlay owns its own event queue, so parking the main thread
                // here does not starve any UI. Exit when the overlay dies rather than idling.
                while running.load(Ordering::Relaxed) && !overlay_exited.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                if overlay_exited.load(Ordering::Relaxed) && running.load(Ordering::Relaxed) {
                    Err("Wayland overlay thread stopped unexpectedly".to_string())
                } else {
                    Ok(())
                }
            } else {
                run_settings(
                    engine,
                    auth,
                    connected_device,
                    running.clone(),
                    shutdown.clone(),
                )
            };

            running.store(false, Ordering::Relaxed);
            shutdown.store(true, Ordering::Relaxed);
            overlay_running.store(false, Ordering::Relaxed);
            let _ = handle.join();
            return result.map_err(Into::into);
        }
        println!("Wayland layer-shell not detected. Using X11/fallback overlay backend.");
    }

    let result = x11_overlay::run_desktop(
        engine,
        auth,
        connected_device,
        running.clone(),
        shutdown.clone(),
        no_ui,
    );
    running.store(false, Ordering::Relaxed);
    shutdown.store(true, Ordering::Relaxed);
    result.map_err(Into::into)
}

/// Settings-only eframe run, used on Wayland where the overlay has its own event queue.
#[cfg(target_os = "linux")]
fn run_settings(
    engine: Arc<Mutex<Engine>>,
    auth: Arc<Mutex<AuthState>>,
    connected_device: Arc<Mutex<Option<String>>>,
    running: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([480.0, 640.0])
            .with_min_inner_size([380.0, 460.0])
            .with_title("KineAnchor Settings"),
        ..Default::default()
    };
    eframe::run_native(
        "kineanchor-settings",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(x11_overlay::DesktopApp::new(
                engine,
                auth,
                connected_device,
                running,
                shutdown,
                false,
                // The Wayland layer-shell thread owns the overlay; do not paint a second one.
                false,
            )))
        }),
    )
    .map_err(|e| format!("{e:?}"))
}
