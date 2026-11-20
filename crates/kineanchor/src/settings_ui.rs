// SPDX-License-Identifier: GPL-3.0-or-later

//! Desktop settings window implementing the Precision Instrument v2 Doop design:
//! Simple (Dark/Light/Paused/Unpaired/QR), Advanced (Wi-Fi/USB/Stale/udev/Auth/Demo),
//! About (Privacy), and Legal (FOSS Licenses).
//!
//! Visual tokens and primitives live in [`crate::design`].

use crate::config_store::{config_file_path, save_config, save_pairing};
use crate::controller_provider::ControllerProvider;
use crate::design::{
    self, Icon, Kind, Palette, ScopeState, SegStyle, SliderRange, action_btn, activated, band,
    band_last, divider, icon_btn, info_row, medium, palette, segmented, status_dot,
};
use crate::iio_provider::IioProvider;
use crate::net_provider::AuthState;
use crate::udev_hint::{CableHint, detect_local_ip, probe_cable_hint};
use eframe::egui::{self, Align, Margin, RichText, Stroke, Ui, Vec2};
use kineanchor_core::{Config, Engine, NetMode, PairingUri, Source, Status, Vehicle};
use qrcode::{Color as QrColor, QrCode};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Comfort presets on the Simple screen: gain, opacity, density.
const PRESETS: [(&str, f32, f32, f32); 3] = [
    ("Gentle", 0.7, 0.5, 0.8),
    ("Balanced", 1.0, 0.65, 1.0),
    ("Winding", 1.6, 0.7, 1.3),
];

const SOURCES: [&str; 4] = ["Wi-Fi", "USB", "Local IIO", "Demo"];
const MAPPINGS: [&str; 2] = ["Force & tilt", "Full 6-DoF"];

const DEPENDENCIES: [(&str, &str); 6] = [
    ("eframe / egui 0.31.1", "MIT OR Apache-2.0"),
    ("smithay-client-toolkit 0.21.1", "MIT"),
    ("wayland-client 0.31.15", "MIT"),
    ("qrcode 0.14.1, serde 1.0", "MIT OR Apache-2.0"),
    ("lucide-icons (vector baseline)", "ISC"),
    ("IBM Plex Sans & Mono", "OFL-1.1"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Settings,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StatusTone {
    Live,
    Warn,
    Err,
    Muted,
}

pub struct SettingsApp {
    engine: Arc<Mutex<Engine>>,
    auth: Arc<Mutex<AuthState>>,
    connected_device: Arc<Mutex<Option<String>>>,
    config: Config,
    /// 0 = Simple (default), 1 = Advanced.
    mode: usize,
    screen: Screen,
    /// 0 = Privacy, 1 = Licenses.
    about_tab: usize,
    show_qr: bool,
    qr_uri: String,
    qr_matrix: Option<(usize, Vec<bool>)>,
    cable_hint: CableHint,
    last_probe: Instant,
    netsetup_error: Option<String>,
    overlay_fallback: Option<String>,
    udev_denied_dev: Option<String>,
    confirm_revoke: bool,
    /// Runtime-only cue pause, mirrored into the engine. Never persisted.
    paused: bool,
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
            netsetup_error: None,
            overlay_fallback: None,
            udev_denied_dev: probe_udev_denied(),
            confirm_revoke: false,
            paused,
            local_imu: IioProvider::new().is_available()
                || ControllerProvider::new().is_available(),
            dirty: false,
        };
        app.refresh_qr(false);
        app
    }

    pub fn set_overlay_fallback(&mut self, msg: Option<String>) {
        self.overlay_fallback = msg;
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

    fn status_indicator(
        &self,
        status: &Status,
        age: Option<f64>,
        auth_rejected: Option<&str>,
    ) -> (String, StatusTone) {
        if self.paused {
            return ("Paused".to_string(), StatusTone::Muted);
        }
        if auth_rejected.is_some() && !matches!(status, Status::Live { .. } | Status::Synthetic) {
            return ("Rejected".to_string(), StatusTone::Err);
        }
        if self.udev_denied_dev.is_some()
            && self.source_index() == 2
            && !matches!(status, Status::Live { .. })
        {
            return ("Denied".to_string(), StatusTone::Err);
        }
        match status {
            Status::Live { rate_hz } => {
                if self.mode == 0 {
                    ("Active".to_string(), StatusTone::Live)
                } else if matches!(self.cable_hint, CableHint::TetherActive { .. })
                    && self.config.net_mode == NetMode::DialPhone
                {
                    (format!("USB · {rate_hz:.0} Hz"), StatusTone::Live)
                } else {
                    (format!("{rate_hz:.0} Hz"), StatusTone::Live)
                }
            }
            Status::Stale { .. } => match age {
                Some(seconds) => (format!("Stale · {seconds:.1} s"), StatusTone::Warn),
                None => ("No sensor".to_string(), StatusTone::Err),
            },
            Status::Synthetic => (
                format!("Demo · {:.2} Hz", self.config.synthetic.freq_hz),
                StatusTone::Warn,
            ),
            Status::Calibrating => ("Calibrating".to_string(), StatusTone::Warn),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        if design::install_fonts(ctx) {
            ctx.request_repaint();
            return;
        }
        let dark = ctx.theme() == egui::Theme::Dark;
        let p = palette(dark);
        install_visuals(ctx, &p, dark);

        if self.last_probe.elapsed() >= Duration::from_secs(2) {
            self.last_probe = Instant::now();
            self.cable_hint = probe_cable_hint();
            self.udev_denied_dev = probe_udev_denied();
            self.local_imu =
                IioProvider::new().is_available() || ControllerProvider::new().is_available();
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
        let (paired_dev, auth_rejected, listen_error) = {
            let g = self.auth.lock().unwrap();
            (
                g.paired_device.clone(),
                g.last_reject_reason.clone(),
                g.listen_error.clone(),
            )
        };
        let age = match status {
            Status::Stale { since } if since > 0.0 => Some((now - since).max(0.0)),
            _ => None,
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
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
                            auth_rejected.as_deref(),
                            listen_error.as_deref(),
                        );
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
        auth_rejected: Option<&str>,
        listen_error: Option<&str>,
    ) {
        match self.screen {
            Screen::Settings => {
                self.header(ui, p, ctx, status, age, auth_rejected);
                if self.mode == 0 {
                    self.simple(ui, p, ctx, status, age, device, paired, offset);
                } else {
                    self.advanced(
                        ui,
                        p,
                        ctx,
                        status,
                        age,
                        offset,
                        dot_count,
                        device,
                        paired,
                        auth_rejected,
                        listen_error,
                    );
                }
            }
            Screen::About => {
                self.about_header(ui, p);
                band(ui, p, p.bg, Margin::symmetric(18, 10), |ui| {
                    if let Some(i) = segmented(
                        ui,
                        p,
                        "about-tabs",
                        &["Privacy", "Licenses"],
                        self.about_tab,
                        SegStyle::tab(),
                    ) {
                        self.about_tab = i;
                    }
                });
                if self.about_tab == 0 {
                    self.about(ui, p, paired);
                } else {
                    self.legal(ui, p);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Headers (`V2Header`)
    // -----------------------------------------------------------------------

    fn header(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        status: &Status,
        age: Option<f64>,
        auth_rejected: Option<&str>,
    ) {
        let (label, tone) = self.status_indicator(status, age, auth_rejected);
        let (dot_color, text_color) = match tone {
            StatusTone::Live => (p.success_fg, p.body),
            StatusTone::Warn => (p.warn_fg, p.warn_fg),
            StatusTone::Err => (p.error_fg, p.error_fg),
            StatusTone::Muted => (p.muted, p.body),
        };
        band(ui, p, p.card, Margin::symmetric(18, 11), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let tile = design::anchor_tile(ui, p, 24.0).on_hover_text("About KineAnchor");
                if activated(&tile) {
                    self.screen = Screen::About;
                }
                design::wordmark(ui, p, "KineAnchor");
                ui.add_space(4.0);
                status_dot(ui, p, &label, dot_color, text_color);

                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
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
                                [440.0, 520.0]
                            } else {
                                [500.0, 680.0]
                            };
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size.into()));
                        }
                    }
                    if activated(&icon_btn(
                        ui,
                        p,
                        Icon::Settings,
                        Kind::Secondary,
                        28.0,
                        "About and privacy",
                    )) {
                        self.screen = Screen::About;
                    }
                });
            });
        });
    }

    fn about_header(&mut self, ui: &mut Ui, p: &Palette) {
        band(ui, p, p.card, Margin::symmetric(18, 11), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let tile = design::anchor_tile(ui, p, 24.0);
                if activated(&tile) {
                    self.screen = Screen::Settings;
                }
                design::wordmark(ui, p, "KineAnchor");
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("v{} · GPL-3.0+", env!("CARGO_PKG_VERSION")))
                        .size(11.0)
                        .family(egui::FontFamily::Monospace)
                        .color(p.body),
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if activated(&icon_btn(ui, p, Icon::Back, Kind::Secondary, 28.0, "Back")) {
                        self.confirm_revoke = false;
                        self.screen = Screen::Settings;
                    }
                });
            });
        });
    }

    // -----------------------------------------------------------------------
    // Simple mode
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn simple(
        &mut self,
        ui: &mut Ui,
        p: &Palette,
        ctx: &egui::Context,
        status: &Status,
        age: Option<f64>,
        device: Option<&str>,
        paired: Option<&str>,
        offset: (f32, f32, f32),
    ) {
        if self.show_qr {
            self.wireless_pair_band(ui, p, ctx);
            self.usb_tether_band(ui, p);
            self.comfort_band(ui, p, true);
            return;
        }

        let paused = self.paused;
        let no_sensor =
            !paused && matches!(status, Status::Stale { .. }) && age.is_none() && !self.local_imu;
        let scope_state = if paused || no_sensor {
            ScopeState::Idle
        } else if matches!(status, Status::Stale { .. }) {
            ScopeState::Stale
        } else {
            ScopeState::Active
        };
        let (status_text, readout) = if paused {
            ("Cues held static", "0.0° · Paused".to_string())
        } else if no_sensor {
            ("No phone or local IMU", "0.0 px".to_string())
        } else if let Some(s) = age {
            ("Damping to rest", format!("{s:.1} s"))
        } else if device.is_some() || self.local_imu || self.config.source == Source::Synthetic {
            (
                "Horizon steady",
                format!("{:+.1}° · dx {:+.1}", offset.2.to_degrees(), offset.0),
            )
        } else {
            ("Static reference", "0.0 px".to_string())
        };

        band(ui, p, p.bg, Margin::symmetric(18, 16), |ui| {
            design::horizon_scope(
                ui,
                p,
                Vec2::new(ui.available_width(), 156.0),
                if paused || no_sensor {
                    0.0
                } else {
                    offset.0.clamp(-20.0, 20.0)
                },
                if paused || no_sensor {
                    0.0
                } else {
                    offset.1.clamp(-20.0, 20.0)
                },
                if paused || no_sensor { 0.0 } else { offset.2 },
                false,
                scope_state,
                status_text,
                &readout,
            );

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let total = ui.available_width();
                let zero_w = 88.0;
                let primary_w = (total - zero_w - 8.0).max(120.0);

                if no_sensor {
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::QrCode),
                        "Pair phone",
                        Kind::Primary,
                        Some(primary_w),
                        36.0,
                        None,
                    )) {
                        self.config.source = Source::Sensor;
                        self.config.net_mode = NetMode::ListenForPhone;
                        self.show_qr = true;
                        self.dirty = true;
                        self.refresh_qr(false);
                    }
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::Pause),
                        "Pause",
                        Kind::Secondary,
                        Some(zero_w),
                        36.0,
                        None,
                    )) {
                        self.set_paused(true);
                    }
                } else {
                    let (icon, text) = if paused {
                        (Icon::Play, "Resume")
                    } else {
                        (Icon::Pause, "Pause")
                    };
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(icon),
                        text,
                        Kind::Primary,
                        Some(primary_w),
                        36.0,
                        None,
                    )) {
                        self.set_paused(!paused);
                    }
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::Recenter),
                        "Zero",
                        Kind::Secondary,
                        Some(zero_w),
                        36.0,
                        None,
                    )) {
                        self.engine.lock().unwrap().zero();
                    }
                }
            });
        });

        if no_sensor {
            band(ui, p, p.card, Margin::symmetric(18, 12), |ui| {
                let (title, body) = match &self.cable_hint {
                    CableHint::UsbPluggedNoTether { product } => (
                        format!("{product} plugged in"),
                        "Turn on USB tethering in phone settings.".to_string(),
                    ),
                    _ => (
                        "No USB phone detected".to_string(),
                        "Cable may be charge-only, or enable USB tethering on phone.".to_string(),
                    ),
                };
                if design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.error_fg,
                    p.error_bg,
                    p.error_border,
                    &title,
                    &body,
                    Some(Icon::Refresh),
                ) {
                    self.cable_hint = probe_cable_hint();
                }
            });
        }

        self.comfort_band(ui, p, no_sensor);

        if !no_sensor {
            band_last(ui, p.card, Margin::symmetric(18, 13), |ui| {
                let name = device
                    .map(ToOwned::to_owned)
                    .or_else(|| paired.map(ToOwned::to_owned))
                    .unwrap_or_else(|| "No phone paired".to_string());
                let transport = match (
                    self.config.source,
                    self.config.net_mode,
                    self.cable_hint.clone(),
                ) {
                    (Source::Synthetic, _, _) => "Demo wave (local)".to_string(),
                    (_, _, CableHint::TetherActive { iface, .. }) => {
                        format!("{iface} · 0.4 ms")
                    }
                    (_, NetMode::ListenForPhone, _) => "Wi-Fi · TCP :47474".to_string(),
                    _ => "USB tether · auto".to_string(),
                };
                ui.horizontal(|ui| {
                    design::glyph(ui, Icon::Smartphone, 15.0, p.body);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                        design::label(ui, p, &name);
                        design::tiny(ui, p, &transport);
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        if activated(&icon_btn(
                            ui,
                            p,
                            Icon::Unlink,
                            Kind::Ghost,
                            28.0,
                            "Revoke pairing",
                        )) {
                            let mut g = self.auth.lock().unwrap();
                            g.paired_token = None;
                            g.paired_device = None;
                            save_pairing(&g.to_persisted());
                        }
                        if activated(&icon_btn(
                            ui,
                            p,
                            Icon::QrCode,
                            Kind::Secondary,
                            28.0,
                            "Show pairing QR",
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
            });
        }
    }

    fn comfort_band(&mut self, ui: &mut Ui, p: &Palette, is_last: bool) {
        let draw_inner = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                design::label(ui, p, "Comfort");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let labels: Vec<&str> = PRESETS.iter().map(|(name, ..)| *name).collect();
                    let selected = self.preset_index().unwrap_or(usize::MAX);
                    let mut style = SegStyle::card();
                    style.width = Some(236.0);
                    if let Some(i) = segmented(ui, p, "comfort", &labels, selected, style) {
                        self.apply_preset(i);
                    }
                });
            });
            divider(ui, p);
            ui.horizontal(|ui| {
                design::label(ui, p, "Visibility");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.allocate_ui(Vec2::new(236.0, 20.0), |ui| {
                        let mut opacity = self.config.opacity;
                        if design::slider(
                            ui,
                            p,
                            "",
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
                });
            });
        };
        if is_last {
            band_last(ui, p.card, Margin::symmetric(18, 13), draw_inner);
        } else {
            band(ui, p, p.card, Margin::symmetric(18, 13), draw_inner);
        }
    }

    fn wireless_pair_band(&mut self, ui: &mut Ui, p: &Palette, ctx: &egui::Context) {
        band(ui, p, p.bg, Margin::symmetric(18, 16), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);
                if let Some(ref matrix) = self.qr_matrix {
                    design::qr_tile(ui, p, matrix, 118.0);
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    design::section(ui, p, "Scan with phone");
                    design::meta(ui, p, "Android app · TCP :47474");
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        if activated(&action_btn(
                            ui,
                            p,
                            Some(Icon::Copy),
                            "Copy",
                            Kind::Primary,
                            None,
                            28.0,
                            None,
                        )) {
                            ctx.copy_text(self.qr_uri.clone());
                        }
                        if activated(&icon_btn(
                            ui,
                            p,
                            Icon::Refresh,
                            Kind::Secondary,
                            28.0,
                            "New pairing secret",
                        )) {
                            self.refresh_qr(true);
                        }
                        if activated(&icon_btn(ui, p, Icon::Back, Kind::Secondary, 28.0, "Done")) {
                            self.show_qr = false;
                        }
                    });
                });
            });
            design::value_box(ui, p, 30.0, |ui| design::mono(ui, p, &self.qr_uri));
        });
    }

    fn usb_tether_band(&mut self, ui: &mut Ui, p: &Palette) {
        band(ui, p, p.card, Margin::symmetric(18, 14), |ui| {
            ui.horizontal(|ui| {
                design::glyph(ui, Icon::Usb, 14.0, p.body);
                design::label(ui, p, "USB tether");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| match &self
                    .cable_hint
                {
                    CableHint::TetherActive { .. } => {
                        status_dot(ui, p, "Active", p.success_fg, p.body);
                    }
                    CableHint::UsbPluggedNoTether { .. } => {
                        status_dot(ui, p, "Tether off", p.warn_fg, p.warn_fg);
                    }
                    CableHint::NoUsbPhone => {
                        status_dot(ui, p, "Waiting", p.muted, p.body);
                    }
                });
            });
            design::tiny(ui, p, &self.cable_hint.label());
            ui.horizontal(|ui| {
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::Terminal),
                    "USB & firewall",
                    Kind::Secondary,
                    None,
                    28.0,
                    None,
                )) {
                    self.launch_netsetup();
                }
                if let Some(ref msg) = self.netsetup_error {
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
            Ok(_) => self.netsetup_error = None,
            Err(e) => self.netsetup_error = Some(format!("Helper unavailable: {e}")),
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
        auth_rejected: Option<&str>,
        listen_error: Option<&str>,
    ) {
        let paused = self.paused;
        let rate_hz = match status {
            Status::Live { rate_hz } => *rate_hz,
            Status::Synthetic => 60.0,
            _ => 0.0,
        };
        let scope_state = if paused
            || (matches!(status, Status::Stale { .. }) && age.is_none())
            || auth_rejected.is_some() && !matches!(status, Status::Live { .. } | Status::Synthetic)
        {
            ScopeState::Idle
        } else if matches!(status, Status::Stale { .. }) {
            ScopeState::Stale
        } else {
            ScopeState::Active
        };

        // 1. Hero Scope + KPIs
        band(ui, p, p.bg, Margin::symmetric(18, 16), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);
                let total = ui.available_width();
                let scope_w = 244.0_f32.min(total * 0.54);
                ui.allocate_ui(Vec2::new(scope_w, 158.0), |ui| {
                    let (st_text, readout) = match age {
                        Some(s) => ("Damping to rest".to_string(), format!("{s:.1} s")),
                        None if paused => ("Cues held static".to_string(), "Paused".to_string()),
                        None if auth_rejected.is_some()
                            && !matches!(status, Status::Live { .. } | Status::Synthetic) =>
                        {
                            ("Auth rejected".to_string(), "0.0 px".to_string())
                        }
                        None if self.config.source == Source::Synthetic => (
                            "Synthetic wave".to_string(),
                            format!("{:+.1}°", offset.2.to_degrees()),
                        ),
                        None if matches!(status, Status::Stale { .. }) => {
                            ("Awaiting IMU".to_string(), "0.0 px".to_string())
                        }
                        None => (
                            format!("dx {:+.1} · dy {:+.1}", offset.0, offset.1),
                            format!("{:+.1}°", offset.2.to_degrees()),
                        ),
                    };
                    design::horizon_scope(
                        ui,
                        p,
                        Vec2::new(scope_w, 158.0),
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
                        scope_state,
                        &st_text,
                        &readout,
                    );
                });

                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);
                    design::meta(ui, p, "Rate");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{rate_hz:.0} Hz"))
                                .size(24.0)
                                .family(medium())
                                .color(p.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let note = if let Some(reason) = auth_rejected {
                                reason
                            } else if self.config.source == Source::Synthetic {
                                "Synthetic"
                            } else if rate_hz > 0.0 {
                                device.or(paired).unwrap_or("Stream")
                            } else {
                                "No packets"
                            };
                            design::tiny(ui, p, note);
                        });
                    });
                    divider(ui, p);
                    design::meta(ui, p, "Shift");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:+.1} px", offset.0))
                                .size(20.0)
                                .family(medium())
                                .color(p.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            design::tiny(ui, p, &format!("{dot_count} dots"));
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        let btn_w = (ui.available_width() - 34.0).max(80.0);
                        if activated(&action_btn(
                            ui,
                            p,
                            Some(Icon::Recenter),
                            "Zero",
                            Kind::Primary,
                            Some(btn_w),
                            28.0,
                            None,
                        )) {
                            self.engine.lock().unwrap().zero();
                        }
                        let pause_icon = if paused { Icon::Play } else { Icon::Pause };
                        if activated(&icon_btn(
                            ui,
                            p,
                            pause_icon,
                            Kind::Secondary,
                            28.0,
                            if paused { "Resume" } else { "Pause" },
                        )) {
                            self.set_paused(!paused);
                        }
                    });
                });
            });
        });

        // 2. Transport & system error banners
        band(ui, p, p.card, Margin::symmetric(18, 14), |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Transport");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let kicker = if age.is_some() {
                        "Watchdog 500 ms"
                    } else if self.udev_denied_dev.is_some() {
                        "udev rule"
                    } else if self.config.source == Source::Synthetic {
                        "Local test"
                    } else {
                        "TCP :47474"
                    };
                    design::tiny(ui, p, kicker);
                });
            });

            let current = self.source_index();
            if let Some(i) = design::segmented_enabled(
                ui,
                p,
                "source",
                &SOURCES,
                current,
                SegStyle::card(),
                &[
                    false,
                    false,
                    !self.local_imu && self.udev_denied_dev.is_none(),
                    false,
                ],
            ) {
                if i != current {
                    self.select_source(i);
                }
            }

            if let Some(reason) = auth_rejected {
                if design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.error_fg,
                    p.error_bg,
                    p.error_border,
                    &format!("Pairing rejected ({reason})"),
                    "Refresh the QR code and scan again on your phone.",
                    Some(Icon::Refresh),
                ) {
                    self.refresh_qr(true);
                    self.show_qr = true;
                    self.mode = 0;
                }
            }

            if let Some(ref dev_path) = self.udev_denied_dev {
                if design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.error_fg,
                    p.error_bg,
                    p.error_border,
                    &format!("{dev_path} permission denied"),
                    "Copy 99-kineanchor.rules to /etc/udev/rules.d/.",
                    Some(Icon::Copy),
                ) {
                    ctx.copy_text("sudo cp packaging/linux/99-kineanchor.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules".to_string());
                }
            }

            if let Some(err) = listen_error {
                let _ = design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.error_fg,
                    p.error_bg,
                    p.error_border,
                    err,
                    "Close other KineAnchor instance or check firewall rules.",
                    None,
                );
            }

            if let Some(s) = age {
                if design::notice_box(
                    ui,
                    p,
                    Icon::WifiOff,
                    p.warn_fg,
                    p.warn_bg,
                    p.warn_border,
                    &format!("Stream stalled ({s:.1} s)"),
                    "Dots hold static. Re-scan QR or plug USB.",
                    Some(Icon::QrCode),
                ) {
                    self.config.source = Source::Sensor;
                    self.config.net_mode = NetMode::ListenForPhone;
                    self.show_qr = true;
                    self.mode = 0;
                    self.dirty = true;
                    self.refresh_qr(false);
                }
            } else if matches!(status, Status::Stale { .. })
                && self.config.source == Source::Sensor
                && design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.error_fg,
                    p.error_bg,
                    p.error_border,
                    "No sensor data received",
                    &self.cable_hint.label(),
                    Some(Icon::QrCode),
                )
            {
                self.config.net_mode = NetMode::ListenForPhone;
                self.show_qr = true;
                self.mode = 0;
                self.dirty = true;
                self.refresh_qr(false);
            }

            if let Some(ref msg) = self.overlay_fallback {
                let _ = design::notice_box(
                    ui,
                    p,
                    Icon::Layers,
                    p.warn_fg,
                    p.warn_bg,
                    p.warn_border,
                    "wlr-layer-shell unavailable",
                    msg,
                    None,
                );
            }

            if let Some(ref err) = self.netsetup_error {
                if design::notice_box(
                    ui,
                    p,
                    Icon::Terminal,
                    p.warn_fg,
                    p.warn_bg,
                    p.warn_border,
                    "kineanchor-netsetup unavailable",
                    err,
                    Some(Icon::Copy),
                ) {
                    ctx.copy_text("sudo ufw allow 47474/tcp".to_string());
                }
            }

            if self.config.source == Source::Sensor {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    let box_w = (ui.available_width() - 68.0).max(140.0);
                    ui.allocate_ui(Vec2::new(box_w, 30.0), |ui| {
                        design::value_box(ui, p, 30.0, |ui| design::mono(ui, p, &self.qr_uri));
                    });
                    if activated(&icon_btn(
                        ui,
                        p,
                        Icon::QrCode,
                        Kind::Secondary,
                        28.0,
                        "QR code",
                    )) {
                        self.show_qr = !self.show_qr;
                        if self.show_qr {
                            self.refresh_qr(false);
                            self.mode = 0;
                        }
                    }
                    if activated(&icon_btn(
                        ui,
                        p,
                        Icon::Copy,
                        Kind::Secondary,
                        28.0,
                        "Copy URI",
                    )) {
                        ctx.copy_text(self.qr_uri.clone());
                    }
                });
                let iface_line = match (self.config.net_mode, &self.cable_hint) {
                    (_, CableHint::TetherActive { iface, .. }) => format!("{iface} · RNDIS"),
                    (NetMode::ListenForPhone, _) => "wlan0 · TCP :47474".to_string(),
                    (NetMode::DialPhone, _) => self.cable_hint.label(),
                };
                ui.horizontal(|ui| {
                    design::tiny(ui, p, &iface_line);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if activated(&action_btn(
                            ui,
                            p,
                            Some(Icon::Terminal),
                            "Netsetup",
                            Kind::Ghost,
                            None,
                            24.0,
                            None,
                        )) {
                            self.launch_netsetup();
                        }
                    });
                });
            }
        });

        // 3. Filter & cues
        band_last(ui, p.card, Margin::symmetric(18, 14), |ui| {
            design::section(ui, p, "Filter & cues");
            let mapping = if self.config.vehicle == Vehicle::Ship {
                1
            } else {
                0
            };
            if let Some(i) = segmented(ui, p, "mapping", &MAPPINGS, mapping, SegStyle::card()) {
                self.config.vehicle = if i == 1 { Vehicle::Ship } else { Vehicle::Car };
                self.dirty = true;
            }

            ui.horizontal(|ui| {
                let half = (ui.available_width() - 18.0) * 0.5;
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let mut gain = self.config.gain;
                    if design::slider(
                        ui,
                        p,
                        "Gain",
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
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    let mut band_val = self.config.edge_band;
                    if design::slider(
                        ui,
                        p,
                        "Margin",
                        &mut band_val,
                        SliderRange {
                            min: 0.05,
                            max: 0.25,
                            step: 0.01,
                        },
                        &|value| format!("{:.0}%", value * 100.0),
                    ) {
                        self.config.edge_band = band_val;
                        self.dirty = true;
                    }
                });
            });

            ui.horizontal(|ui| {
                let half = (ui.available_width() - 18.0) * 0.5;
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let mut density = self.config.density;
                    if design::slider(
                        ui,
                        p,
                        "Dots",
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
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    let mut opacity = self.config.opacity;
                    if design::slider(
                        ui,
                        p,
                        "Opacity",
                        &mut opacity,
                        SliderRange {
                            min: 0.1,
                            max: 0.8,
                            step: 0.01,
                        },
                        &|value| format!("{:.0}%", value * 100.0),
                    ) {
                        self.config.opacity = opacity;
                        self.dirty = true;
                    }
                });
            });

            ui.horizontal(|ui| {
                design::meta(ui, p, "Invert axis");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.allocate_ui(Vec2::new(236.0, 32.0), |ui| {
                        let mut invert = self.config.invert;
                        let ship = self.config.vehicle == Vehicle::Ship;
                        if design::toggles(
                            ui,
                            p,
                            "invert",
                            &["Horizontal", "Vertical", "Roll"],
                            &mut invert,
                            &[true, true, ship],
                        ) {
                            self.config.invert = invert;
                            self.dirty = true;
                        }
                    });
                });
            });

            if self.config.source == Source::Synthetic {
                ui.horizontal(|ui| {
                    let half = (ui.available_width() - 18.0) * 0.5;
                    ui.vertical(|ui| {
                        ui.set_width(half);
                        let mut freq = self.config.synthetic.freq_hz;
                        if design::slider(
                            ui,
                            p,
                            "Demo frequency",
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
                    });
                    ui.add_space(18.0);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        let mut amp = self.config.synthetic.amplitude;
                        if design::slider(
                            ui,
                            p,
                            "Demo amplitude",
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
                    });
                });
                let _ = design::notice_box(
                    ui,
                    p,
                    Icon::Alert,
                    p.warn_fg,
                    p.warn_bg,
                    p.warn_border,
                    "0.2 Hz is an experimental motion band",
                    "Pause cues or switch to Sensor if uncomfortable.",
                    None,
                );
            }

            divider(ui, p);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                    design::label(ui, p, "Spring ODE");
                    design::tiny(ui, p, "omega 6.0 rad/s, zeta 0.70, tau 0.50 s");
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if activated(&icon_btn(
                        ui,
                        p,
                        Icon::Reset,
                        Kind::Secondary,
                        28.0,
                        "Reset defaults",
                    )) {
                        self.reset_defaults();
                    }
                });
            });
        });
    }

    // -----------------------------------------------------------------------
    // About / Legal screens
    // -----------------------------------------------------------------------

    fn about(&mut self, ui: &mut Ui, p: &Palette, paired: Option<&str>) {
        band(ui, p, p.card, Margin::symmetric(18, 16), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(14.0, 0.0);
                design::anchor_tile_large(ui, p, 48.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                    design::title(ui, p, "KineAnchor Desktop");
                    design::meta(ui, p, "Peripheral motion-sickness overlay");
                    design::tiny(ui, p, "Wayland, X11, Windows");
                });
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let w = ((ui.available_width() - 16.0) / 3.0).max(80.0);
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::Code),
                    "GitHub",
                    Kind::Primary,
                    Some(w),
                    28.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::REPO));
                }
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::Shield),
                    "SHA256",
                    Kind::Secondary,
                    Some(w),
                    28.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::RELEASES));
                }
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::Alert),
                    "Issue",
                    Kind::Secondary,
                    Some(w),
                    28.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::ISSUES));
                }
            });
        });

        band(ui, p, p.card, Margin::symmetric(18, 16), |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Zero telemetry");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    design::tiny(ui, p, "Offline");
                });
            });
            info_row(
                ui,
                p,
                "Peer-to-peer local",
                "TCP :47474",
                "50 Hz NDJSON strictly over USB tether or LAN.",
                false,
                false,
            );
            info_row(
                ui,
                p,
                "No accounts or SDKs",
                "cargo-deny",
                "Zero cloud SDKs or tracking IDs; auditable crates.",
                false,
                false,
            );
            info_row(
                ui,
                p,
                "Local config",
                "JSON",
                "Preset and pairing token only.",
                false,
                true,
            );
        });

        band_last(ui, p.card, Margin::symmetric(18, 16), |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Local storage");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    design::tiny(ui, p, "500 ms watchdog · 1.0 s fade");
                });
            });
            let path = config_file_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "config directory unavailable".to_string());
            let token_state = match paired {
                Some(name) => format!("paired · {name}"),
                None => "0 tokens".to_string(),
            };
            design::value_box(ui, p, 30.0, |ui| {
                design::mono(ui, p, &format!("{path} · {token_state}"));
            });

            if self.confirm_revoke {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                    let w = ((ui.available_width() - 8.0) * 0.5).max(100.0);
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::Unlink),
                        "Confirm revoke",
                        Kind::Danger,
                        Some(w),
                        28.0,
                        None,
                    )) {
                        let mut g = self.auth.lock().unwrap();
                        g.paired_token = None;
                        g.paired_device = None;
                        save_pairing(&g.to_persisted());
                        self.confirm_revoke = false;
                    }
                    if activated(&action_btn(
                        ui,
                        p,
                        None,
                        "Cancel",
                        Kind::Secondary,
                        Some(w),
                        28.0,
                        None,
                    )) {
                        self.confirm_revoke = false;
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                    let w = ((ui.available_width() - 8.0) * 0.5).max(100.0);
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::Unlink),
                        "Revoke token",
                        Kind::Danger,
                        Some(w),
                        28.0,
                        None,
                    )) {
                        self.confirm_revoke = true;
                    }
                    if activated(&action_btn(
                        ui,
                        p,
                        Some(Icon::FileText),
                        "PRIVACY.md",
                        Kind::Secondary,
                        Some(w),
                        28.0,
                        None,
                    )) {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(design::PRIVACY_DOC));
                    }
                });
            }
        });
    }

    fn legal(&mut self, ui: &mut Ui, p: &Palette) {
        band(ui, p, p.card, Margin::symmetric(18, 15), |ui| {
            design::section(ui, p, "GNU GPL v3.0 or later");
            design::meta(
                ui,
                p,
                "Copyleft FOSS. Runtime crates pass cargo-deny SPDX checks.",
            );
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let w = ((ui.available_width() - 8.0) * 0.5).max(100.0);
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::FileText),
                    "LICENSE",
                    Kind::Primary,
                    Some(w),
                    28.0,
                    None,
                )) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(design::LICENSE_FILE));
                }
                if activated(&action_btn(
                    ui,
                    p,
                    Some(Icon::Shield),
                    "deny.toml",
                    Kind::Secondary,
                    Some(w),
                    28.0,
                    None,
                )) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(design::DENY_FILE));
                }
            });
        });

        band(ui, p, p.card, Margin::symmetric(18, 15), |ui| {
            ui.horizontal(|ui| {
                design::section(ui, p, "Dependencies");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    design::tiny(ui, p, "Cargo.lock");
                });
            });
            for (index, (name, license)) in DEPENDENCIES.iter().enumerate() {
                info_row(
                    ui,
                    p,
                    name,
                    license,
                    "",
                    true,
                    index + 1 == DEPENDENCIES.len(),
                );
            }
        });

        band_last(ui, p.card, Margin::symmetric(18, 15), |ui| {
            design::section(ui, p, "Research & prior art");
            info_row(
                ui,
                p,
                "Diels & Howarth (2013)",
                "Human Factors",
                "55(3), 595-604 · 0.2 Hz nauseogenic band (Ship & Sway).",
                false,
                false,
            );
            info_row(
                ui,
                p,
                "Clean-room prior art",
                "0 lines copied",
                "dev.davidv.motionsickness (GPL-3.0), monperrus/motion-cues, Apple VMC.",
                false,
                true,
            );
        });
    }
}

fn probe_udev_denied() -> Option<String> {
    ControllerProvider::permission_denied_device().or_else(IioProvider::permission_denied_device)
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
    visuals.window_corner_radius = egui::CornerRadius::same(8);
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
        widget.bg_stroke = Stroke::new(1.0_f32, p.subtle);
        widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
        widget.corner_radius = egui::CornerRadius::same(6);
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.body);
    ctx.set_visuals(visuals);
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

        app.select_source(0);
        app.local_imu = false;
        assert_ne!(app.source_index(), 2);
    }

    #[test]
    fn status_indicator_covers_system_states() {
        let mut app = app_with(Config::default());
        app.udev_denied_dev = None;

        assert_eq!(
            app.status_indicator(&Status::Stale { since: 0.0 }, None, None),
            ("No sensor".to_string(), StatusTone::Err)
        );
        assert_eq!(
            app.status_indicator(&Status::Stale { since: 1.0 }, Some(4.2), None),
            ("Stale · 4.2 s".to_string(), StatusTone::Warn)
        );
        assert_eq!(
            app.status_indicator(&Status::Stale { since: 0.0 }, None, Some("bad_token")),
            ("Rejected".to_string(), StatusTone::Err)
        );
        assert_eq!(
            app.status_indicator(&Status::Synthetic, None, None),
            ("Demo · 0.20 Hz".to_string(), StatusTone::Warn)
        );
        assert_eq!(
            app.status_indicator(&Status::Live { rate_hz: 50.0 }, None, None),
            ("Active".to_string(), StatusTone::Live)
        );

        app.set_paused(true);
        assert_eq!(
            app.status_indicator(&Status::Live { rate_hz: 50.0 }, None, None),
            ("Paused".to_string(), StatusTone::Muted)
        );
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
