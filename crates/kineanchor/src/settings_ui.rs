// SPDX-License-Identifier: GPL-3.0-or-later

use crate::config_store::{save_config, save_pairing};
use crate::net_provider::AuthState;
use crate::udev_hint::{CableHint, detect_local_ip, probe_cable_hint};
use eframe::egui;
use kineanchor_core::{Config, Engine, PairingUri, Source, Status, Vehicle};
use qrcode::{Color as QrColor, QrCode};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct SettingsApp {
    engine: Arc<Mutex<Engine>>,
    auth: Arc<Mutex<AuthState>>,
    connected_device: Arc<Mutex<Option<String>>>,
    config: Config,
    status_label: String,
    status_color: egui::Color32,
    show_qr: bool,
    qr_uri: String,
    qr_matrix: Option<(usize, Vec<bool>)>,
    cable_hint: CableHint,
    last_probe: Instant,
    netsetup_status: Option<String>,
}

impl SettingsApp {
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        auth: Arc<Mutex<AuthState>>,
        connected_device: Arc<Mutex<Option<String>>>,
    ) -> Self {
        let initial_config = {
            let eng = engine.lock().unwrap();
            *eng.config()
        };

        let mut app = Self {
            engine,
            auth,
            connected_device,
            config: initial_config,
            status_label: "Initializing...".to_string(),
            status_color: egui::Color32::GRAY,
            show_qr: false,
            qr_uri: String::new(),
            qr_matrix: None,
            cable_hint: probe_cable_hint(),
            last_probe: Instant::now(),
            netsetup_status: None,
        };
        app.refresh_qr(false);
        app
    }

    fn refresh_qr(&mut self, force_new_secret: bool) {
        let ip = detect_local_ip();
        let (port, secret, fp) = {
            let mut g = self.auth.lock().unwrap();
            let sec = if force_new_secret {
                g.generate_pair_secret()
            } else {
                g.current_pair_secret()
            };
            (g.port, sec, g.desktop_name.clone())
        };

        let uri = PairingUri {
            host: ip,
            port,
            pair_secret: secret,
            fingerprint: fp,
        }
        .to_uri();

        if let Ok(code) = QrCode::new(uri.as_bytes()) {
            let width = code.width();
            let colors: Vec<bool> = code
                .to_colors()
                .into_iter()
                .map(|c| c == QrColor::Dark)
                .collect();
            self.qr_matrix = Some((width, colors));
        }
        self.qr_uri = uri;
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut config_changed = false;

        if self.last_probe.elapsed() >= Duration::from_secs(2) {
            self.last_probe = Instant::now();
            self.cable_hint = probe_cable_hint();
        }

        let dev_opt = self.connected_device.lock().unwrap().clone();
        let paired_dev = self.auth.lock().unwrap().paired_device.clone();

        {
            let eng = self.engine.lock().unwrap();
            let now = crate::monotonic_secs();
            let status = eng.status(now);
            match status {
                Status::Live { rate_hz } => {
                    let dev = dev_opt.as_deref().unwrap_or("Phone");
                    self.status_label = format!("● Live: {dev} ({rate_hz:.0} Hz)");
                    self.status_color = egui::Color32::from_rgb(46, 204, 113); // Green
                }
                Status::Stale { since } => {
                    if since > 0.0 || dev_opt.is_some() {
                        self.status_label = "● Stale (Static Fallback)".to_string();
                        self.status_color = egui::Color32::from_rgb(241, 196, 15); // Amber
                    } else {
                        self.status_label = "● No Sensor Connected".to_string();
                        self.status_color = egui::Color32::from_rgb(231, 76, 60); // Red
                    }
                }
                Status::Synthetic => {
                    self.status_label =
                        format!("● Synthetic ({:.2} Hz)", self.config.synthetic.freq_hz);
                    self.status_color = egui::Color32::from_rgb(52, 152, 219);
                }
                Status::Calibrating => {
                    self.status_label = "● Calibrating...".to_string();
                    self.status_color = egui::Color32::from_rgb(155, 89, 182);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
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
                                    Source::Sensor,
                                    "Phone Sensor",
                                )
                                .clicked()
                            {
                                config_changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.config.source,
                                    Source::Synthetic,
                                    "Synthetic (Experimental)",
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

                if self.config.source == Source::Sensor {
                    ui.separator();
                    ui.label(egui::RichText::new("Phone Sensor Connection (USB / LAN)").strong());

                    let hint_color = match self.cable_hint {
                        CableHint::TetherActive { .. } => egui::Color32::from_rgb(46, 204, 113),
                        CableHint::UsbPluggedNoTether { .. } => {
                            egui::Color32::from_rgb(241, 196, 15)
                        }
                        CableHint::NoUsbPhone => egui::Color32::GRAY,
                    };
                    ui.colored_label(hint_color, self.cable_hint.label());

                    ui.horizontal(|ui| {
                        match &paired_dev {
                            Some(name) => ui.label(format!("Paired phone: {name}")),
                            None => ui.label("Paired phone: None"),
                        };
                        if ui
                            .button(if self.show_qr {
                                "Hide Pairing QR"
                            } else {
                                "Pair Phone (QR)"
                            })
                            .clicked()
                        {
                            self.show_qr = !self.show_qr;
                            if self.show_qr {
                                self.refresh_qr(false);
                            }
                        }
                        if paired_dev.is_some() && ui.button("Unpair").clicked() {
                            let persisted = {
                                let mut g = self.auth.lock().unwrap();
                                g.paired_token = None;
                                g.paired_device = None;
                                g.to_persisted()
                            };
                            save_pairing(&persisted);
                        }
                    });

                    if self.show_qr {
                        ui.group(|ui| {
                            ui.label("1. Plug USB cable & enable USB Tethering on phone");
                            ui.label("2. Scan this QR code in KineAnchor Android app:");
                            if let Some((w, ref matrix)) = self.qr_matrix {
                                let module_px = 4.0_f32;
                                let quiet = 2usize;
                                let total_modules = (w + quiet * 2) as f32;
                                let size = egui::vec2(
                                    total_modules * module_px,
                                    total_modules * module_px,
                                );
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                let painter = ui.painter_at(rect);
                                painter.rect_filled(rect, 4.0, egui::Color32::WHITE);
                                for y in 0..w {
                                    for x in 0..w {
                                        if matrix[y * w + x] {
                                            let min = egui::pos2(
                                                rect.min.x + (x + quiet) as f32 * module_px,
                                                rect.min.y + (y + quiet) as f32 * module_px,
                                            );
                                            let r = egui::Rect::from_min_size(
                                                min,
                                                egui::vec2(module_px, module_px),
                                            );
                                            painter.rect_filled(r, 0.0, egui::Color32::BLACK);
                                        }
                                    }
                                }
                            }
                            ui.monospace(&self.qr_uri);
                            if ui.button("Regenerate QR Secret").clicked() {
                                self.refresh_qr(true);
                            }
                        });
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Configure USB Tether & Firewall").clicked() {
                            match Command::new("pkexec")
                                .arg("/usr/lib/kineanchor/kineanchor-netsetup")
                                .spawn()
                            {
                                Ok(_) => {
                                    self.netsetup_status =
                                        Some("Launched polkit helper".to_string());
                                }
                                Err(e) => {
                                    self.netsetup_status = Some(format!("Helper unavailable: {e}"));
                                }
                            }
                        }
                        if let Some(ref msg) = self.netsetup_status {
                            ui.label(msg);
                        }
                    });
                }

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
                        .add(
                            egui::Slider::new(&mut self.config.dot_radius_px, 3.0..=18.0)
                                .text("px"),
                        )
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
                    ui.label(
                        egui::RichText::new(
                            "Synthetic Motion (Experimental — 0.0 amplitude = static reference)",
                        )
                        .strong(),
                    );
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
                                0.0..=0.5,
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
        });

        if config_changed {
            let mut eng = self.engine.lock().unwrap();
            eng.set_config(self.config);
            save_config(&self.config);
        }

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
