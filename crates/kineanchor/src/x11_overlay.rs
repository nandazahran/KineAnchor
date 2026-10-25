// SPDX-License-Identifier: GPL-3.0-or-later

use eframe::egui;
use kineanchor_core::Engine;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct X11OverlayApp {
    engine: Arc<Mutex<Engine>>,
    running: Arc<AtomicBool>,
}

impl X11OverlayApp {
    pub fn new(engine: Arc<Mutex<Engine>>, running: Arc<AtomicBool>) -> Self {
        Self { engine, running }
    }
}

impl eframe::App for X11OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Fully transparent clear
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.running.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let screen_rect = ctx.screen_rect();
        let viewport = [screen_rect.width(), screen_rect.height()];
        let now = crate::monotonic_secs();

        let (dots, color) = {
            let mut engine = self.engine.lock().unwrap();
            let color = engine.config().color;
            let (dots_ref, _) = engine.tick(now, viewport);
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

        // Request continuous repaint while running
        ctx.request_repaint();
    }
}

pub fn spawn_x11_overlay(
    engine: Arc<Mutex<Engine>>,
    running: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, &'static str> {
    std::thread::Builder::new()
        .name("x11-overlay".into())
        .spawn(move || {
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_transparent(true)
                    .with_mouse_passthrough(true)
                    .with_always_on_top()
                    .with_decorations(false)
                    .with_taskbar(false)
                    .with_maximized(true),
                ..Default::default()
            };

            let _ = eframe::run_native(
                "kineanchor-overlay-x11",
                options,
                Box::new(|_cc| Ok(Box::new(X11OverlayApp::new(engine, running)))),
            );
        })
        .map_err(|_| "Failed to spawn X11 overlay thread")
}
