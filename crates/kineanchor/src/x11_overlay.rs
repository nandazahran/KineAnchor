// SPDX-License-Identifier: GPL-3.0-or-later

//! Desktop fallback overlay for X11 and Windows.
//!
//! eframe/winit requires every window to be driven from the thread that owns the event loop,
//! so the settings window and the transparent overlay share one `eframe::App`: the settings
//! panel is the root viewport and the overlay is an immediate child viewport. Creating a second
//! `eframe::run_native` on a worker thread panics inside winit and never paints.

use crate::net_provider::AuthState;
use crate::settings_ui::SettingsApp;
use eframe::egui;
use kineanchor_core::Engine;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Stable id for the overlay child viewport, kept across frames so the window is reused.
const OVERLAY_VIEWPORT: &str = "kineanchor-overlay";

/// Fullscreen, transparent, click-through, always-on-top overlay window.
fn overlay_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_transparent(true)
        .with_mouse_passthrough(true)
        .with_always_on_top()
        .with_decorations(false)
        .with_taskbar(false)
        .with_maximized(true)
        .with_title("kineanchor-overlay")
}

/// Paints the current dot field across the whole viewport.
fn paint_dots(ctx: &egui::Context, engine: &Arc<Mutex<Engine>>) {
    let screen_rect = ctx.screen_rect();
    let viewport = [screen_rect.width(), screen_rect.height()];
    let now = crate::monotonic_secs();

    let (dots, color) = {
        let mut eng = engine.lock().unwrap();
        let color = eng.config().color;
        let (dots_ref, _) = eng.tick(now, viewport);
        (dots_ref.to_vec(), color)
    };

    let painter = ctx.layer_painter(egui::LayerId::background());
    for dot in dots {
        let alpha = (dot.alpha * 255.0) as u8;
        let dot_color = egui::Color32::from_rgba_premultiplied(
            ((color[0] as f32) * dot.alpha) as u8,
            ((color[1] as f32) * dot.alpha) as u8,
            ((color[2] as f32) * dot.alpha) as u8,
            alpha,
        );
        painter.circle_filled(egui::pos2(dot.x, dot.y), dot.r, dot_color);
    }
}

pub struct DesktopApp {
    engine: Arc<Mutex<Engine>>,
    /// False once the user closes the UI; mirrored into `shutdown` for the provider threads.
    running: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    settings: Option<SettingsApp>,
    /// False on Wayland, where a dedicated layer-shell thread already paints the overlay.
    own_overlay: bool,
}

impl DesktopApp {
    /// `no_ui` runs the overlay as the root viewport instead of opening settings.
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        auth: Arc<Mutex<AuthState>>,
        connected_device: Arc<Mutex<Option<String>>>,
        running: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
        no_ui: bool,
        own_overlay: bool,
    ) -> Self {
        let settings = if no_ui {
            None
        } else {
            Some(SettingsApp::new(
                engine.clone(),
                auth.clone(),
                connected_device,
            ))
        };
        Self {
            engine,
            running,
            shutdown,
            settings,
            own_overlay,
        }
    }
}

impl eframe::App for DesktopApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.settings.is_some() {
            visuals.panel_fill.to_normalized_gamma_f32()
        } else {
            [0.0, 0.0, 0.0, 0.0] // transparent overlay-only root
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.running.load(Ordering::Relaxed) {
            self.shutdown.store(true, Ordering::Relaxed);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let engine = self.engine.clone();
        let running = self.running.clone();

        match &mut self.settings {
            Some(settings) => {
                if ctx.input(|i| i.viewport().close_requested()) {
                    // Closing settings must stop providers and the overlay with it.
                    running.store(false, Ordering::Relaxed);
                    self.shutdown.store(true, Ordering::Relaxed);
                }
                settings.ui(ctx);

                if self.own_overlay {
                    ctx.show_viewport_immediate(
                        egui::ViewportId::from_hash_of(OVERLAY_VIEWPORT),
                        overlay_viewport(),
                        |ctx, _class| {
                            if !running.load(Ordering::Relaxed) {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                return;
                            }
                            paint_dots(ctx, &engine);
                            ctx.request_repaint();
                        },
                    );
                }
            }
            None => {
                // Overlay-only mode: this viewport is the overlay.
                paint_dots(ctx, &engine);
                ctx.request_repaint();
            }
        }
    }
}

/// Runs the desktop settings + overlay pair on the calling (main) thread.
///
/// Errors from window creation are returned instead of being discarded, so a failed overlay
/// cannot look like a healthy run.
pub fn run_desktop(
    engine: Arc<Mutex<Engine>>,
    auth: Arc<Mutex<AuthState>>,
    connected_device: Arc<Mutex<Option<String>>>,
    running: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    no_ui: bool,
) -> Result<(), String> {
    let viewport = if no_ui {
        overlay_viewport()
    } else {
        egui::ViewportBuilder::default()
            .with_inner_size([480.0, 580.0])
            .with_min_inner_size([420.0, 520.0])
            .with_title("KineAnchor Settings")
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        if no_ui {
            "kineanchor-overlay"
        } else {
            "kineanchor-settings"
        },
        options,
        Box::new(move |_cc| {
            Ok(Box::new(DesktopApp::new(
                engine,
                auth,
                connected_device,
                running,
                shutdown,
                no_ui,
                true,
            )))
        }),
    );

    result.map_err(|e| format!("{e:?}"))
}
