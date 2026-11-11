// SPDX-License-Identifier: GPL-3.0-or-later

//! Desktop settings window, laid out from the Penpot boards:
//! `Desktop Simple · Dark Effort / Light Frame / Wireless QR`,
//! `Desktop Advanced · Dark Wi-Fi / Light USB / Stale Signal / Linux udev`,
//! `Desktop About · Offline Privacy` and `Desktop Legal · FOSS Licenses`.
//!
//! Visual tokens live in [`crate::design`]; this module composes screens and maps every
//! control onto real engine/provider state.

use crate::config_store::{config_file_path, save_config, save_pairing};
use crate::controller_provider::ControllerProvider;
use crate::design::{
    self, Kind, Palette, SegStyle, SliderRange, btn, btn_sized, card, medium, palette, pill,
    segmented, semibold,
};
use crate::iio_provider::IioProvider;
use crate::net_provider::AuthState;
use crate::udev_hint::{CableHint, detect_local_ip, probe_cable_hint};
use eframe::egui::{self, Align, Color32, Margin, RichText, Stroke, Ui, Vec2};
use kineanchor_core::{Config, Engine, NetMode, PairingUri, Source, Status, Vehicle};
use qrcode::{Color as QrColor, QrCode};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Comfort presets on the Simple screen: gain, opacity, density.
const PRESETS: [(&str, f32, f32, f32); 3] = [
    ("Gentle", 0.7, 0.5, 0.8),
    ("Balanced", 1.0, 0.65, 1.0),
    ("Winding road", 1.6, 0.7, 1.3),
];

const SOURCES: [&str; 4] = ["Wi-Fi UDP", "USB Tether", "Local IIO / HID", "Demo wave"];
const MAPPINGS: [&str; 2] = ["Force and tilt", "Full 6-DoF"];

const DEPENDENCIES: [(&str, &str); 6] = [
    ("eframe / egui 0.31.1", "MIT OR Apache-2.0"),
    ("smithay-client-toolkit 0.21.1", "MIT"),
    ("wayland-client 0.31.15", "MIT"),
    ("qrcode 0.14.1", "MIT OR Apache-2.0"),
    ("serde & serde_json 1.0", "MIT OR Apache-2.0"),
    ("IBM Plex Sans / Mono", "OFL-1.1"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Settings,
    About,
}

pub struct SettingsApp {
    engine: Arc<Mutex<Engine>>,
    auth: Arc<Mutex<AuthState>>,
    connected_device: Arc<Mutex<Option<String>>>,
    config: Config,
    /// 0 = Simple (design default), 1 = Advanced.
    mode: usize,
    screen: Screen,
    /// 0 = About and privacy, 1 = Licenses and credits.
    about_tab: usize,
    show_qr: bool,
    qr_uri: String,
    qr_matrix: Option<(usize, Vec<bool>)>,
    cable_hint: CableHint,
    last_probe: Instant,
    netsetup_status: Option<String>,
    /// Runtime-only cue pause, mirrored into the engine. Never persisted.
    paused: bool,
    /// Whether a local IIO or controller IMU exists. Local sensors always feed the engine
    /// when present, so this is a status fact, not a mode.
    local_imu: bool,
    dirty: bool,
}

impl SettingsApp {
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        auth: Arc<Mutex<AuthState>>,
        connected_device: Arc<Mutex<Option<String>>>,
    ) -> Self {
        let (initial_config, paused) = {
            let eng = engine.lock().unwrap();
            (*eng.config(), eng.is_paused())
        };

        let mut app = Self {
            engine,
            auth,
            connected_device,
            config: initial_config,
            mode: 0,
            screen: Screen::Settings,
            about_tab: 0,
            show_qr: false,
            qr_uri: String::new(),
            qr_matrix: None,
            cable_hint: probe_cable_hint(),
            last_probe: Instant::now(),
            netsetup_status: None,
            paused,
            local_imu: IioProvider::new().is_available()
                || ControllerProvider::new().is_available(),
            dirty: false,
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

    /// The design's four-source well, derived from the two real fields it can express.
    ///
    /// `Local IIO / HID` is not an exclusive mode: local provider threads always feed the
    /// engine whenever hardware is present. It maps to the same `Sensor + DialPhone` pair as
    /// USB tether and is only selectable when such hardware was detected.
    /// Pause is runtime state: it flips the engine flag and is never written to disk.
    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.engine.lock().unwrap().set_paused(paused);
    }

    fn source_index(&self) -> usize {
        if self.config.source == Source::Synthetic {
            3
        } else if self.config.net_mode == NetMode::ListenForPhone {
            0
        } else if self.local_imu && self.connected_device.lock().unwrap().is_none() {
            2
        } else {
            1
        }
    }

    fn select_source(&mut self, index: usize) {
        match index {
            0 => {
                self.config.source = Source::Sensor;
                self.config.net_mode = NetMode::ListenForPhone;
            }
            2 | 1 => {
                self.config.source = Source::Sensor;
                self.config.net_mode = NetMode::DialPhone;
            }
            _ => {
                self.config.source = Source::Synthetic;
            }
        }
        self.dirty = true;
    }

    fn preset_index(&self) -> Option<usize> {
        PRESETS.iter().position(|(_, gain, opacity, density)| {
            (self.config.gain - gain).abs() < 1e-3
                && (self.config.opacity - opacity).abs() < 1e-3
                && (self.config.density - density).abs() < 1e-3
        })
    }

    fn apply_preset(&mut self, index: usize) {
        if let Some((_, gain, opacity, density)) = PRESETS.get(index).copied() {
            self.config.gain = gain;
            self.config.opacity = opacity;
            self.config.density = density;
            self.dirty = true;
        }
    }

    fn reset_defaults(&mut self) {
        let net = self.config.net_mode;
        self.config = Config::default();
        // Keep the chosen transport direction: resetting visual tuning must not silently
        // stop the phone from reconnecting.
        self.config.net_mode = net;
        self.dirty = true;
    }

    fn apply_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        {
            let mut eng = self.engine.lock().unwrap();
            eng.set_config(self.config);
            eng.set_paused(self.paused);
        }
        self.auth.lock().unwrap().mode = self.config.net_mode;
        save_config(&self.config);
        self.dirty = false;
    }

    /// Builds the settings panel. Called by the settings root viewport and reused by the
    /// combined desktop app so both surfaces share one event loop.
    pub fn ui(&mut self, ctx: &egui::Context) {
        if design::install_fonts(ctx) {
            // New font definitions are only live from the next pass.
            ctx.request_repaint();
            return;
        }
        // `ctx.theme()` resolves the OS preference (and any explicit user override) for us.
        let dark = ctx.theme() == egui::Theme::Dark;
        let p = palette(dark);
        install_visuals(ctx, &p, dark);

        if self.last_probe.elapsed() >= Duration::from_secs(2) {
            self.last_probe = Instant::now();
            self.cable_hint = probe_cable_hint();
        }

        let now = crate::monotonic_secs();
        let (status, offset) = {
            let eng = self.engine.lock().unwrap();
            (eng.status(now), eng.current_offset())
        };
        let dot_count = {
            let eng = self.engine.lock().unwrap();
            eng.current_dots().len()
        };
        let dev_opt = self.connected_device.lock().unwrap().clone();
        let paired_dev = self.auth.lock().unwrap().paired_device.clone();
        let age = match status {
            Status::Stale { since } if since > 0.0 => Some((now - since).max(0.0)),
            _ => None,
        };

        // Dark boards are a rounded ground; light boards sit on a white sheet inset 8 px.
        let (ground, sheet_fill, sheet_radius) = if dark {
            (p.bg, p.bg, 0)
        } else {
            (p.bg, p.sheet, 16)
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ground))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(sheet_fill)
                    .corner_radius(egui::CornerRadius::same(sheet_radius))
                    .inner_margin(Margin::symmetric(12, 0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Frame::new()
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .show(ui, |ui| {
                                        self.screen_body(
                                            ui,
                                            &p,
                                            ctx,
                                            &status,
                                            age,
                                            offset,
                                            dot_count,
                                            dev_opt.as_deref(),
                                            paired_dev.as_deref(),
                                        );
                                        ui.add_space(16.0);
                                    });
                            });
                    });
            });

        self.apply_if_dirty();
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    #[allow(clippy::too_many_arguments)]
    fn screen_body(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        status: &Status,
        age: Option<f64>,
        offset: (f32, f32, f32),
        dot_count: usize,
        device: Option<&str>,
        paired: Option<&str>,
    ) {
        match self.screen {
            Screen::Settings => {
                self.header(ui, p, ctx, status, age);
                ui.add_space(10.0);
                if self.mode == 0 {
                    self.simple(ui, p, ctx, device, paired, offset);
                } else {
                    self.advanced(ui, p, ctx, status, age, offset, dot_count, device, paired);
                }
            }
            Screen::About => {
                self.about_header(ui, p);
                ui.add_space(8.0);
                let picked = segmented(
                    ui,
                    p,
                    "about-tabs",
                    &["About and privacy", "Licenses and credits"],
                    self.about_tab,
                    SegStyle::tab(),
                );
                if let Some(i) = picked {
                    self.about_tab = i;
                }
                ui.add_space(6.0);
                if self.about_tab == 0 {
                    self.about(ui, p, paired);
                } else {
                    self.legal(ui, p);
                }
                ui.add_space(12.0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Headers
    // -----------------------------------------------------------------------

    fn status_pill(
        &self,
        p: &Palette,
        status: &Status,
        age: Option<f64>,
    ) -> (String, Color32, Color32) {
        if self.paused {
            return ("Paused".to_string(), p.body, p.well);
        }
        match status {
            Status::Live { rate_hz } => {
                if self.mode == 0 {
                    ("Active".to_string(), p.success_fg, p.success_bg)
                } else {
                    (
                        format!("Streaming · {rate_hz:.0} Hz"),
                        p.success_fg,
                        p.success_bg,
                    )
                }
            }
            Status::Stale { .. } => match age {
                Some(seconds) => (format!("Stale · {seconds:.1} s ago"), p.warn_fg, p.warn_bg),
                None => ("No sensor".to_string(), p.error_fg, p.error_bg),
            },
            Status::Synthetic => (
                format!("Demo wave · {:.2} Hz", self.config.synthetic.freq_hz),
                p.accent,
                p.well,
            ),
            Status::Calibrating => ("Calibrating".to_string(), p.warn_fg, p.warn_bg),
        }
    }

    fn header(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        status: &Status,
        age: Option<f64>,
    ) {
        let (text, fg, bg) = self.status_pill(p, status, age);
        ui.horizontal(|ui| {
            let tile = design::anchor_tile(ui, p, 24.0).on_hover_text("About KineAnchor");
            if design::activated(&tile) {
                self.screen = Screen::About;
            }
            design::wordmark(ui, p, "KineAnchor");
            ui.add_space(4.0);
            pill(ui, &text, fg, bg);

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let picked = segmented(
                    ui,
                    p,
                    "settings-mode",
                    &["Simple", "Advanced"],
                    self.mode,
                    SegStyle::header(),
                );
                if let Some(new_mode) = picked {
                    if new_mode != self.mode {
                        self.mode = new_mode;
                        let size = if new_mode == 0 {
                            [480.0, 580.0]
                        } else {
                            [520.0, 780.0]
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size.into()));
                    }
                }
            });
        });
    }

    fn about_header(&mut self, ui: &mut Ui, p: &Palette) {
        ui.horizontal(|ui| {
            let tile = design::anchor_tile(ui, p, 24.0);
            if design::activated(&tile) {
                self.screen = Screen::Settings;
            }
            design::wordmark(ui, p, "KineAnchor");
            ui.add_space(4.0);
            pill(
                ui,
                &format!("v{} · GPL-3.0+", env!("CARGO_PKG_VERSION")),
                p.success_fg,
                p.success_bg,
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Back to settings",
                    Kind::Secondary,
                    Some(136.0),
                    30.0,
                    None,
                )) {
                    self.screen = Screen::Settings;
                }
            });
        });
    }

    // -----------------------------------------------------------------------
    // Simple mode
    // -----------------------------------------------------------------------

    fn simple(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        device: Option<&str>,
        paired: Option<&str>,
        offset: (f32, f32, f32),
    ) {
        if self.show_qr {
            self.wireless_pair_card(ui, p, ctx);
            ui.add_space(2.0);
            self.usb_tether_card(ui, p);
            ui.add_space(2.0);
            self.comfort_card(ui, p);
            return;
        }

        card(ui, p, |ui| {
            let paused = self.paused;
            design::horizon_scope(
                ui,
                p,
                Vec2::new(ui.available_width(), 136.0),
                if paused {
                    0.0
                } else {
                    offset.0.clamp(-20.0, 20.0)
                },
                if paused {
                    0.0
                } else {
                    offset.1.clamp(-20.0, 20.0)
                },
                if paused { 0.0 } else { offset.2 },
                false,
            );
            let caption = if paused {
                "Motion cues paused · dots hold a static reference".to_string()
            } else if device.is_some() {
                format!(
                    "Horizon tracking · dx {:+.1} px · dy {:+.1} px",
                    offset.0, offset.1
                )
            } else {
                "No phone connected · showing the static reference".to_string()
            };
            design::tiny(ui, p, &caption);

            ui.horizontal(|ui| {
                // Design: 272 px primary + 126 px secondary inside a 440 px card.
                let width = (ui.available_width() - 10.0) * 0.62;
                let label = if paused {
                    "Resume motion cues"
                } else {
                    "Pause motion cues"
                };
                if design::activated(&btn_sized(
                    ui,
                    p,
                    label,
                    Kind::Primary,
                    Some(width),
                    42.0,
                    None,
                )) {
                    self.set_paused(!paused);
                }
                let rest = ui.available_width();
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Recenter",
                    Kind::Secondary,
                    Some(rest),
                    42.0,
                    None,
                )) {
                    self.engine.lock().unwrap().zero();
                }
            });
        });

        ui.add_space(2.0);
        self.comfort_card(ui, p);
        ui.add_space(2.0);

        card(ui, p, |ui| {
            design::meta(ui, p, "Connected phone");
            let name = device
                .map(ToOwned::to_owned)
                .or_else(|| paired.map(ToOwned::to_owned))
                .unwrap_or_else(|| "No phone paired".to_string());
            let transport = match (
                self.config.source,
                self.config.net_mode,
                self.cable_hint.clone(),
            ) {
                (Source::Synthetic, _, _) => "Demo wave (no sensor)".to_string(),
                (_, _, CableHint::TetherActive { iface, .. }) => {
                    format!("USB tether · {iface}")
                }
                (_, NetMode::ListenForPhone, _) => "Wi-Fi LAN (desktop listening)".to_string(),
                _ => "Dialing phone over USB tether".to_string(),
            };
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(name).size(13.0).family(medium()).color(p.ink));
                    design::tiny(ui, p, &transport);
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if design::activated(&btn_sized(
                        ui,
                        p,
                        "Pair new",
                        Kind::Secondary,
                        Some(88.0),
                        34.0,
                        None,
                    )) {
                        self.config.source = Source::Sensor;
                        self.config.net_mode = NetMode::ListenForPhone;
                        self.set_paused(false);
                        self.show_qr = true;
                        self.dirty = true;
                        self.refresh_qr(false);
                    }
                });
            });
            design::tiny(ui, p, self.cable_hint.label().as_str());
        });
    }

    fn comfort_card(&mut self, ui: &mut Ui, p: &Palette) {
        card(ui, p, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
            design::label(ui, p, "Motion comfort");
            let labels: Vec<&str> = PRESETS.iter().map(|(name, ..)| *name).collect();
            let selected = self.preset_index().unwrap_or(usize::MAX);
            if let Some(i) = segmented(ui, p, "comfort", &labels, selected, SegStyle::card()) {
                self.apply_preset(i);
            }

            ui.add_space(2.0);
            let mut opacity = self.config.opacity;
            if design::slider(
                ui,
                p,
                "Cue visibility",
                &mut opacity,
                SliderRange {
                    min: 0.1,
                    max: 0.8,
                    step: 0.01,
                },
                &|value| visibility_label(value).to_string(),
            ) {
                self.config.opacity = opacity;
                self.dirty = true;
            }
        });
    }

    fn wireless_pair_card(&mut self, ui: &mut Ui, p: &Palette, ctx: &egui::Context) {
        card(ui, p, |ui| {
            design::section(ui, p, "Pair your Android phone (Wireless)");
            ui.horizontal(|ui| {
                if let Some(ref matrix) = self.qr_matrix {
                    design::qr_tile(ui, matrix, 116.0);
                }
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Scan with KineAnchor camera")
                            .size(14.0)
                            .family(medium())
                            .color(p.ink),
                    );
                    design::meta(
                        ui,
                        p,
                        "Open KineAnchor on your phone and tap Scan laptop QR.",
                    );
                    design::tiny(ui, p, "Or pair with a link");
                    ui.horizontal(|ui| {
                        if design::activated(&btn_sized(
                            ui,
                            p,
                            "Copy laptop link",
                            Kind::Primary,
                            Some(132.0),
                            34.0,
                            None,
                        )) {
                            ctx.copy_text(self.qr_uri.clone());
                        }
                        if design::activated(&btn_sized(
                            ui,
                            p,
                            "Hide QR",
                            Kind::Secondary,
                            Some(96.0),
                            34.0,
                            None,
                        )) {
                            self.show_qr = false;
                        }
                    });
                });
            });
            design::mono(ui, p, &self.qr_uri);
            ui.horizontal(|ui| {
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Regenerate secret",
                    Kind::Ghost,
                    None,
                    28.0,
                    None,
                )) {
                    self.refresh_qr(true);
                }
            });
        });
    }

    fn usb_tether_card(&mut self, ui: &mut Ui, p: &Palette) {
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Wired USB Tethering (No Wi-Fi needed)");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let auto = matches!(self.cable_hint, CableHint::TetherActive { .. });
                    let (text, fg, bg) = if auto {
                        ("Auto-detect", p.success_fg, p.success_bg)
                    } else {
                        ("Waiting", p.body, p.ink_bg())
                    };
                    pill(ui, text, fg, bg);
                });
            });
            design::meta(
                ui,
                p,
                "1. Plug your phone into this computer with a USB cable.",
            );
            design::meta(ui, p, "2. Turn on USB Tethering in Android Settings.");
            design::tiny(ui, p, self.cable_hint.label().as_str());
            ui.horizontal(|ui| {
                if design::activated(&btn(
                    ui,
                    p,
                    "Configure USB tether & firewall",
                    Kind::Secondary,
                )) {
                    self.launch_netsetup();
                }
                if let Some(ref msg) = self.netsetup_status {
                    design::tiny(ui, p, msg);
                }
            });
        });
    }

    fn launch_netsetup(&mut self) {
        let helper = std::env::var_os("APPDIR")
            .map(std::path::PathBuf::from)
            .map(|d| d.join("usr/lib/kineanchor/kineanchor-netsetup"))
            .filter(|p| p.is_file())
            .unwrap_or_else(|| std::path::PathBuf::from("/usr/lib/kineanchor/kineanchor-netsetup"));
        match Command::new("pkexec").arg(helper).spawn() {
            Ok(_) => self.netsetup_status = Some("Launched polkit helper".to_string()),
            Err(e) => self.netsetup_status = Some(format!("Helper unavailable: {e}")),
        }
    }

    // -----------------------------------------------------------------------
    // Advanced mode
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn advanced(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        status: &Status,
        age: Option<f64>,
        offset: (f32, f32, f32),
        dot_count: usize,
        device: Option<&str>,
        paired: Option<&str>,
    ) {
        let paused = self.paused;
        let rate_hz = match status {
            Status::Live { rate_hz } => *rate_hz,
            _ => 0.0,
        };

        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                let total = ui.available_width();
                let scope_w = (total - 220.0).clamp(160.0, total * 0.62);
                ui.vertical(|ui| {
                    design::horizon_scope(
                        ui,
                        p,
                        Vec2::new(scope_w, 168.0),
                        if paused {
                            0.0
                        } else {
                            offset.0.clamp(-24.0, 24.0)
                        },
                        if paused {
                            0.0
                        } else {
                            offset.1.clamp(-24.0, 24.0)
                        },
                        if paused { 0.0 } else { offset.2 },
                        true,
                    );
                    let caption = match age {
                        Some(seconds) => {
                            format!("Last sample {seconds:.1} s ago · damping to rest")
                        }
                        None if paused => "Motion cues paused".to_string(),
                        None => format!(
                            "dx {:+.1} px · dy {:+.1} px · roll {:+.1}°",
                            offset.0,
                            offset.1,
                            offset.2.to_degrees()
                        ),
                    };
                    design::tiny(ui, p, &caption);
                });

                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    design::meta(ui, p, "Sample rate");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{rate_hz:.0} Hz"))
                                .size(28.0)
                                .color(p.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let note = if rate_hz > 0.0 {
                                device.unwrap_or("phone stream")
                            } else {
                                "No active packets"
                            };
                            design::tiny(ui, p, note);
                        });
                    });
                    design::divider(ui, p);
                    design::meta(ui, p, "Horizon displacement");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:+.1} px", offset.0))
                                .size(22.0)
                                .color(p.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            design::tiny(ui, p, &format!("{dot_count} dots"));
                        });
                    });
                    if design::activated(&btn_sized(
                        ui,
                        p,
                        "Zero orientation",
                        Kind::Primary,
                        Some(ui.available_width()),
                        36.0,
                        None,
                    )) {
                        self.engine.lock().unwrap().zero();
                    }
                });
            });
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            design::section(ui, p, "Sensor source and transport");
            let current = self.source_index();
            if let Some(i) = design::segmented_enabled(
                ui,
                p,
                "source",
                &SOURCES,
                current,
                SegStyle::card(),
                &[false, false, !self.local_imu, false],
            ) {
                if i != current {
                    self.select_source(i);
                }
            }
            design::tiny(ui, p, self.cable_hint.label().as_str());
            if self.local_imu {
                design::tiny(
                    ui,
                    p,
                    "Local IIO / controller IMU detected — it always feeds the engine when present.",
                );
            }

            let (fg, bg, heading, body, action) = match status {
                Status::Stale { .. } if age.is_some() => (
                    p.warn_fg,
                    p.warn_bg,
                    format!("Stream interrupted {:.1} s ago.", age.unwrap_or(0.0)),
                    "Peripheral dots are damping to rest. Reconnect the phone or switch source."
                        .to_string(),
                    Some("Show QR"),
                ),
                Status::Stale { .. } if self.config.source == Source::Sensor => (
                    p.error_fg,
                    p.error_bg,
                    "No sensor data received.".to_string(),
                    if self.local_imu {
                        "Local IMU is present but silent; pair a phone or check the cable."
                            .to_string()
                    } else {
                        "No local IIO or controller IMU on this machine. Pair a phone or plug in a USB tether."
                            .to_string()
                    },
                    Some("Pair phone"),
                ),
                _ => (
                    p.success_fg,
                    p.success_bg,
                    String::new(),
                    String::new(),
                    None,
                ),
            };
            if !heading.is_empty() {
                let clicked = design::notice(ui, p, fg, bg, &heading, &body, action);
                if clicked {
                    self.config.source = Source::Sensor;
                    self.config.net_mode = NetMode::ListenForPhone;
                    self.set_paused(false);
                    self.show_qr = true;
                    self.dirty = true;
                    self.refresh_qr(false);
                }
            }

            if self.config.source == Source::Sensor {
                ui.horizontal(|ui| {
                    design::value_box(ui, p, 36.0, |ui| design::mono(ui, p, &self.qr_uri));
                    if design::activated(&btn_sized(
                        ui,
                        p,
                        "Show QR",
                        Kind::Secondary,
                        Some(78.0),
                        36.0,
                        Some(8),
                    )) {
                        self.show_qr = !self.show_qr;
                        if self.show_qr {
                            self.refresh_qr(false);
                        }
                    }
                    if design::activated(&btn_sized(
                        ui,
                        p,
                        "Copy URI",
                        Kind::Secondary,
                        Some(82.0),
                        36.0,
                        Some(8),
                    )) {
                        ctx.copy_text(self.qr_uri.clone());
                    }
                });
            }

            if self.show_qr && self.config.source == Source::Sensor {
                ui.add_space(2.0);
                egui::Frame::new()
                    .fill(p.well)
                    .corner_radius(12)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(ref matrix) = self.qr_matrix {
                                design::qr_tile(ui, matrix, 116.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Scan with KineAnchor camera")
                                        .size(14.0)
                                        .family(medium())
                                        .color(p.ink),
                                );
                                design::meta(
                                    ui,
                                    p,
                                    "The desktop must listen (Wi-Fi UDP) for the phone to dial.",
                                );
                                if design::activated(&btn(
                                    ui,
                                    p,
                                    "Regenerate secret",
                                    Kind::Secondary,
                                )) {
                                    self.refresh_qr(true);
                                }
                            });
                        });
                    });
            }

            design::tiny(
                ui,
                p,
                match (self.config.source, self.config.net_mode) {
                    (Source::Synthetic, _) => "Demo wave runs locally; no phone or sensor is used.",
                    (_, NetMode::DialPhone) => {
                        "Desktop dials the phone over USB tether. No QR scan needed."
                    }
                    (_, NetMode::ListenForPhone) => {
                        "Desktop listens on port 47474; the phone dials this address."
                    }
                },
            );
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            design::section(ui, p, "Motion response and spring physics");

            design::label(ui, p, "Motion mapping");
            let mapping = if self.config.vehicle == Vehicle::Ship {
                1
            } else {
                0
            };
            if let Some(i) = segmented(ui, p, "mapping", &MAPPINGS, mapping, SegStyle::card()) {
                self.config.vehicle = if i == 1 { Vehicle::Ship } else { Vehicle::Car };
                self.dirty = true;
            }

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let half = (ui.available_width() - 12.0) * 0.5;
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let mut gain = self.config.gain;
                    if design::slider(
                        ui,
                        p,
                        "Sensitivity gain",
                        &mut gain,
                        SliderRange {
                            min: 0.2,
                            max: 3.0,
                            step: 0.05,
                        },
                        &|value| format!("{value:.2}x"),
                    ) {
                        self.config.gain = gain;
                        self.dirty = true;
                    }
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    let mut band = self.config.edge_band;
                    if design::slider(
                        ui,
                        p,
                        "Edge margin",
                        &mut band,
                        SliderRange {
                            min: 0.05,
                            max: 0.25,
                            step: 0.01,
                        },
                        &|value| format!("{:.0} %", value * 100.0),
                    ) {
                        self.config.edge_band = band;
                        self.dirty = true;
                    }
                });
            });

            ui.horizontal(|ui| {
                let half = (ui.available_width() - 12.0) * 0.5;
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let mut density = self.config.density;
                    if design::slider(
                        ui,
                        p,
                        "Cue dot density",
                        &mut density,
                        SliderRange {
                            min: 0.3,
                            max: 2.5,
                            step: 0.05,
                        },
                        &|value| {
                            let approx = (12.0 * value) as i32;
                            if dot_count > 0 {
                                format!("{dot_count}")
                            } else {
                                format!("~{approx}")
                            }
                        },
                    ) {
                        self.config.density = density;
                        self.dirty = true;
                    }
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    let mut radius = self.config.dot_radius_px;
                    if design::slider(
                        ui,
                        p,
                        "Dot size",
                        &mut radius,
                        SliderRange {
                            min: 3.0,
                            max: 18.0,
                            step: 0.5,
                        },
                        &|value| format!("{value:.0} px"),
                    ) {
                        self.config.dot_radius_px = radius;
                        self.dirty = true;
                    }
                });
            });

            let mut opacity = self.config.opacity;
            if design::slider(
                ui,
                p,
                "Dot opacity",
                &mut opacity,
                SliderRange {
                    min: 0.1,
                    max: 0.8,
                    step: 0.01,
                },
                &|value| format!("{:.0} %", value * 100.0),
            ) {
                self.config.opacity = opacity;
                self.dirty = true;
            }

            design::label(ui, p, "Invert axis");
            let mut invert = self.config.invert;
            let ship = self.config.vehicle == Vehicle::Ship;
            let labels: [&str; 3] = ["Horizontal", "Vertical", "Roll"];
            let enabled = [true, true, ship];
            if design::toggles(ui, p, "invert", &labels, &mut invert, &enabled) {
                self.config.invert = invert;
                self.dirty = true;
            }

            if self.config.source == Source::Synthetic {
                ui.add_space(2.0);
                design::label(ui, p, "Demo wave");
                let mut freq = self.config.synthetic.freq_hz;
                if design::slider(
                    ui,
                    p,
                    "Frequency",
                    &mut freq,
                    SliderRange {
                        min: 0.05,
                        max: 0.5,
                        step: 0.01,
                    },
                    &|value| format!("{value:.2} Hz"),
                ) {
                    self.config.synthetic.freq_hz = freq;
                    self.dirty = true;
                }
                let mut amp = self.config.synthetic.amplitude;
                if design::slider(
                    ui,
                    p,
                    "Amplitude",
                    &mut amp,
                    SliderRange {
                        min: 0.0,
                        max: 0.5,
                        step: 0.01,
                    },
                    &|value| format!("{value:.2}"),
                ) {
                    self.config.synthetic.amplitude = amp;
                    self.dirty = true;
                }
            }

            design::divider(ui, p);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Spring constants")
                            .size(13.0)
                            .family(medium())
                            .color(p.ink),
                    );
                    design::meta(ui, p, "0.5 s stale watchdog · 1.0 s static crossfade");
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if design::activated(&btn_sized(
                        ui,
                        p,
                        "Reset defaults",
                        Kind::Secondary,
                        Some(114.0),
                        32.0,
                        Some(8),
                    )) {
                        self.reset_defaults();
                    }
                });
            });
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            design::meta(ui, p, "Connected phone");
            let name = device
                .map(ToOwned::to_owned)
                .or_else(|| paired.map(ToOwned::to_owned))
                .unwrap_or_else(|| "No phone paired".to_string());
            ui.label(RichText::new(name).size(13.0).family(medium()).color(p.ink));
            design::tiny(ui, p, self.cable_hint.label().as_str());

            let mut dev_toggles = [!self.paused];
            let labels = [if self.paused {
                "Resume cues"
            } else {
                "Pause cues"
            }];
            if design::toggles(ui, p, "overlay-power", &labels, &mut dev_toggles, &[true]) {
                self.set_paused(!dev_toggles[0]);
            }
            ui.horizontal(|ui| {
                if design::activated(&btn(
                    ui,
                    p,
                    "Configure USB tether & firewall",
                    Kind::Secondary,
                )) {
                    self.launch_netsetup();
                }
                if self.netsetup_status.is_some() {
                    design::tiny(ui, p, self.netsetup_status.as_deref().unwrap_or(""));
                }
            });
        });
    }

    // -----------------------------------------------------------------------
    // About / Legal screens
    // -----------------------------------------------------------------------

    fn about(&mut self, ui: &mut Ui, p: &Palette, paired: Option<&str>) {
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                design::anchor_tile_large(ui, p, 64.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("KineAnchor Desktop")
                            .size(18.0)
                            .family(semibold())
                            .color(p.ink),
                    );
                    design::meta(ui, p, "Cross-platform peripheral motion-sickness overlay");
                    design::tiny(
                        ui,
                        p,
                        &format!(
                            "v{} · Linux Wayland (wlr-layer-shell) + X11 + Windows",
                            env!("CARGO_PKG_VERSION")
                        ),
                    );
                    ui.label(
                        RichText::new("Free software under GNU GPL v3.0 or later")
                            .size(12.0)
                            .family(medium())
                            .color(p.ink),
                    );
                });
            });
            ui.horizontal(|ui| {
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Source on GitHub",
                    Kind::Primary,
                    Some(144.0),
                    38.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::REPO));
                }
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Verify SHA256SUMS",
                    Kind::Secondary,
                    Some(152.0),
                    38.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::RELEASES));
                }
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Report issue",
                    Kind::Secondary,
                    Some(132.0),
                    38.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::ISSUES));
                }
            });
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Privacy policy: zero data collection")
                        .size(15.0)
                        .family(semibold())
                        .color(p.ink),
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    pill(ui, "100% Offline", p.success_fg, p.success_bg);
                });
            });
            design::meta(
                ui,
                p,
                "KineAnchor never connects to the internet. No servers, analytics, \
                 telemetry, or crash reports exist in this binary.",
            );
            design::divider(ui, p);
            privacy_item(
                ui,
                p,
                "Local peer-to-peer network only",
                "TCP port 47474 exchanges 50 Hz NDJSON motion samples strictly between \
                 your phone and desktop over USB tether or LAN.",
            );
            design::divider(ui, p);
            privacy_item(
                ui,
                p,
                "No accounts or third-party SDKs",
                "Zero cloud dependencies or tracking identifiers. Built from auditable \
                 Rust crates verified by cargo-deny.",
            );
            design::divider(ui, p);
            privacy_item(
                ui,
                p,
                "Local-only configuration storage",
                "Saves only your comfort preset and local pairing token on disk. Removing \
                 KineAnchor deletes all local state.",
            );
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            design::section(ui, p, "Stored on this computer");
            let path = config_file_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "config directory unavailable".to_string());
            let token_state = match paired {
                Some(name) => format!("paired token · {name}"),
                None => "no paired phone token".to_string(),
            };
            design::value_box(ui, p, 34.0, |ui| {
                design::mono(ui, p, &format!("{path} · {token_state}"));
            });
            design::tiny(
                ui,
                p,
                "Watchdog safety: 500 ms stale threshold + 1.0 s static crossfade",
            );
            ui.horizontal(|ui| {
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Revoke paired token",
                    Kind::Danger,
                    Some(214.0),
                    38.0,
                    None,
                )) {
                    {
                        let mut g = self.auth.lock().unwrap();
                        g.paired_token = None;
                        g.paired_device = None;
                        save_pairing(&g.to_persisted());
                    }
                }
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "Read docs/PRIVACY.md",
                    Kind::Secondary,
                    Some(218.0),
                    38.0,
                    None,
                )) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(design::PRIVACY_DOC));
                }
            });
        });
    }

    fn legal(&mut self, ui: &mut Ui, p: &Palette) {
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "GNU General Public License v3.0 or later");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    pill(ui, "GPL-3.0-or-later", p.dark_tint(), p.subtle);
                });
            });
            design::meta(
                ui,
                p,
                "You are free to run, study, share, and modify KineAnchor under the GNU \
                 GPL v3.0 or any later version. All dependencies pass cargo-deny GPLv3 checks.",
            );
            ui.horizontal(|ui| {
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "View full LICENSE text",
                    Kind::Primary,
                    Some(208.0),
                    36.0,
                    None,
                )) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(design::LICENSE_FILE));
                }
                if design::activated(&btn_sized(
                    ui,
                    p,
                    "View deny.toml policy",
                    Kind::Secondary,
                    Some(218.0),
                    36.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::DENY_FILE));
                }
            });
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Runtime dependencies (Cargo.lock)");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    design::tiny(ui, p, "SPDX license gate enforced in CI");
                });
            });
            for (index, (name, license)) in DEPENDENCIES.iter().enumerate() {
                if index > 0 {
                    design::divider(ui, p);
                }
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(*name)
                            .size(12.0)
                            .family(medium())
                            .color(p.ink),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        design::meta(ui, p, license);
                    });
                });
            }
        });

        ui.add_space(2.0);
        card(ui, p, |ui| {
            design::section(ui, p, "Scientific research and prior art");
            ui.label(
                RichText::new("Diels & Howarth (2013) · VIMS Frequency Band")
                    .size(13.0)
                    .family(medium())
                    .color(p.ink),
            );
            design::meta(
                ui,
                p,
                "Human Factors, 55(3), 595-604. Characterizes the 0.2 Hz nauseogenic \
                 visual band used in the Ship filter and Demo wave.",
            );
            design::divider(ui, p);
            ui.label(
                RichText::new("Clean-room FOSS & platform prior art")
                    .size(13.0)
                    .family(medium())
                    .color(p.ink),
            );
            design::meta(
                ui,
                p,
                "Inspired by dev.davidv.motionsickness (F-Droid, GPL-3.0), \
                 monperrus/motion-cues, and the Apple Vehicle Motion Cues convention. \
                 Zero code copied.",
            );
        });
    }
}

fn privacy_item(ui: &mut Ui, p: &Palette, heading: &str, body: &str) {
    ui.label(
        RichText::new(heading)
            .size(13.0)
            .family(medium())
            .color(p.ink),
    );
    design::meta(ui, p, body);
}

fn visibility_label(opacity: f32) -> &'static str {
    if opacity < 0.35 {
        "Gentle"
    } else if opacity < 0.55 {
        "Light"
    } else if opacity < 0.72 {
        "Medium"
    } else {
        "Strong"
    }
}

/// Applies the design palette to egui's own widget style so stock widgets (text edits,
/// scrollbars, tooltips) match the painted controls.
fn install_visuals(ctx: &egui::Context, p: &Palette, dark: bool) {
    let key = egui::Id::new("kineanchor.design_visuals");
    if ctx.data(|d| d.get_temp::<bool>(key)) == Some(dark) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, dark));
    let mut visuals = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.card;
    visuals.extreme_bg_color = p.well;
    visuals.faint_bg_color = p.well;
    visuals.code_bg_color = p.well;
    visuals.override_text_color = Some(p.ink);
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0_f32, p.ink);
    visuals.window_corner_radius = egui::CornerRadius::same(16);
    visuals.slider_trailing_fill = true;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = p.chip;
        widget.weak_bg_fill = p.chip;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
        widget.corner_radius = egui::CornerRadius::same(8);
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.body);
    ctx.set_visuals(visuals);
}

impl Palette {
    /// Foreground for neutral pills on the current theme.
    fn dark_tint(&self) -> Color32 {
        if self.dark {
            self.ink
        } else {
            Color32::from_rgb(0x35, 0x37, 0x36)
        }
    }

    /// Default pill background for neutral states.
    fn ink_bg(&self) -> Color32 {
        self.well
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(config: Config) -> SettingsApp {
        let engine = Arc::new(Mutex::new(Engine::new(config)));
        let auth = Arc::new(Mutex::new(AuthState::default()));
        let device = Arc::new(Mutex::new(None));
        SettingsApp::new(engine, auth, device)
    }

    #[test]
    fn pause_is_runtime_only_and_reaches_the_engine() {
        let mut app = app_with(Config::default());
        assert!(!app.engine.lock().unwrap().is_paused());

        app.set_paused(true);
        assert!(app.paused);
        assert!(app.engine.lock().unwrap().is_paused());
        assert!(!app.dirty, "pausing must not mark settings dirty");

        app.set_paused(false);
        assert!(!app.engine.lock().unwrap().is_paused());
    }

    #[test]
    fn source_chips_map_onto_source_and_net_mode() {
        let mut app = app_with(Config::default());

        app.select_source(0);
        assert_eq!(app.config.source, Source::Sensor);
        assert_eq!(app.config.net_mode, NetMode::ListenForPhone);
        assert_eq!(app.source_index(), 0);

        app.select_source(1);
        assert_eq!(app.config.net_mode, NetMode::DialPhone);
        assert_eq!(app.source_index(), 1);

        app.select_source(3);
        assert_eq!(app.config.source, Source::Synthetic);
        assert_eq!(app.source_index(), 3);

        // Local IIO is not an exclusive mode; without detected hardware it is unavailable.
        app.select_source(0);
        app.local_imu = false;
        assert_ne!(app.source_index(), 2);
    }

    #[test]
    fn visibility_labels_follow_opacity() {
        assert_eq!(visibility_label(0.2), "Gentle");
        assert_eq!(visibility_label(0.5), "Light");
        assert_eq!(visibility_label(0.65), "Medium");
        assert_eq!(visibility_label(0.8), "Strong");
    }

    #[test]
    fn presets_are_distinct() {
        let mut seen: Vec<(u32, u32, u32)> = Vec::new();
        for (name, gain, opacity, density) in PRESETS {
            let key = (
                (gain * 1000.0) as u32,
                (opacity * 1000.0) as u32,
                (density * 1000.0) as u32,
            );
            assert!(!seen.contains(&key), "duplicate preset values for {name}");
            seen.push(key);
        }
    }
}
