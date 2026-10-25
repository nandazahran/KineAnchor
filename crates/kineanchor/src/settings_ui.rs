// SPDX-License-Identifier: GPL-3.0-or-later

use crate::config_store::save_config;
use eframe::egui;
use kineanchor_core::{Config, Engine, Source, Status, Vehicle};
use std::sync::{Arc, Mutex};

pub struct SettingsApp {
    engine: Arc<Mutex<Engine>>,
    config: Config,
    status_label: String,
    status_color: egui::Color32,
}

impl SettingsApp {
    pub fn new(engine: Arc<Mutex<Engine>>) -> Self {
        let initial_config = {
            let eng = engine.lock().unwrap();
            *eng.config()
        };

        Self {
            engine,
            config: initial_config,
            status_label: "Initializing...".to_string(),
            status_color: egui::Color32::GRAY,
        }
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut config_changed = false;

        // Query status from engine
        {
            let mut eng = self.engine.lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let (_, status) = eng.tick(now, [100.0, 100.0]);
            match status {
                Status::Live { rate_hz } => {
                    self.status_label = format!("● Live Stream ({rate_hz:.1} Hz)");
                    self.status_color = egui::Color32::from_rgb(46, 204, 113);
                }
                Status::Stale { .. } => {
                    self.status_label = "● Waiting for sensor...".to_string();
                    self.status_color = egui::Color32::from_rgb(241, 196, 15);
                }
                Status::Synthetic => {
                    self.status_label =
                        format!("● Synthetic Mode ({:.2} Hz)", self.config.synthetic.freq_hz);
                    self.status_color = egui::Color32::from_rgb(52, 152, 219);
                }
                Status::Calibrating => {
                    self.status_label = "● Calibrating...".to_string();
                    self.status_color = egui::Color32::from_rgb(155, 89, 182);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("KineAnchor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(self.status_color, &self.status_label);
                });
            });
            ui.separator();

            egui::Grid::new("mode_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Motion Source:");
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_value(
                                &mut self.config.source,
                                Source::Synthetic,
                                "Synthetic (0.2 Hz)",
                            )
                            .clicked()
                        {
                            config_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut self.config.source,
                                Source::Sensor,
                                "Sensor (Phone Stream)",
                            )
                            .clicked()
                        {
                            config_changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("Vehicle Type:");
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_value(
                                &mut self.config.vehicle,
                                Vehicle::Car,
                                "Car (Turn / Accel)",
                            )
                            .clicked()
                        {
                            config_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut self.config.vehicle,
                                Vehicle::Ship,
                                "Ship (Heave / Roll)",
                            )
                            .clicked()
                        {
                            config_changed = true;
                        }
                    });
                    ui.end_row();
                });

            ui.separator();
            ui.label(egui::RichText::new("Overlay Appearance").strong());

            ui.horizontal(|ui| {
                ui.label("Dot Density:");
                if ui
                    .add(egui::Slider::new(&mut self.config.density, 0.3..=2.5).text("x"))
                    .changed()
                {
                    config_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Dot Size:");
                if ui
                    .add(egui::Slider::new(&mut self.config.dot_radius_px, 3.0..=18.0).text("px"))
                    .changed()
                {
                    config_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui
                    .add(egui::Slider::new(&mut self.config.opacity, 0.1..=0.8).text("alpha"))
                    .changed()
                {
                    config_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Edge Band:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.config.edge_band, 0.05..=0.25)
                            .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                    )
                    .changed()
                {
                    config_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Motion Gain:");
                if ui
                    .add(egui::Slider::new(&mut self.config.gain, 0.2..=3.0).text("x"))
                    .changed()
                {
                    config_changed = true;
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Invert Axis").strong());
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.config.invert[0], "Invert Horizontal (X)")
                    .changed()
                {
                    config_changed = true;
                }
                if ui
                    .checkbox(&mut self.config.invert[1], "Invert Vertical (Y)")
                    .changed()
                {
                    config_changed = true;
                }
                if self.config.vehicle == Vehicle::Ship
                    && ui
                        .checkbox(&mut self.config.invert[2], "Invert Roll")
                        .changed()
                {
                    config_changed = true;
                }
            });

            if self.config.source == Source::Synthetic {
                ui.separator();
                ui.label(egui::RichText::new("Synthetic Motion Tuning").strong());
                ui.horizontal(|ui| {
                    ui.label("Frequency:");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.config.synthetic.freq_hz, 0.05..=0.5)
                                .text("Hz"),
                        )
                        .changed()
                    {
                        config_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Amplitude:");
                    if ui
                        .add(egui::Slider::new(
                            &mut self.config.synthetic.amplitude,
                            0.02..=0.5,
                        ))
                        .changed()
                    {
                        config_changed = true;
                    }
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Zero Sensor Calibration").clicked() {
                    let mut eng = self.engine.lock().unwrap();
                    eng.zero();
                }

                if ui.button("Save Settings").clicked() {
                    save_config(&self.config);
                }
            });
        });

        if config_changed {
            let mut eng = self.engine.lock().unwrap();
            eng.set_config(self.config);
            save_config(&self.config);
        }

        // Repaint periodically to keep status indicator fresh
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}
