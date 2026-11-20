// SPDX-License-Identifier: GPL-3.0-or-later

//! Precision Instrument v2 visual language for the desktop settings UI, matching the
//! Doop `KineAnchor` design tokens and component library.
//!
//! Screen composition lives in [`crate::settings_ui`].

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, Response,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};
use std::sync::Arc;

pub const REPO: &str = "https://github.com/nandazahran/KineAnchor";
pub const RELEASES: &str = "https://github.com/nandazahran/KineAnchor/releases";
pub const ISSUES: &str = "https://github.com/nandazahran/KineAnchor/issues";
pub const PRIVACY_DOC: &str = "https://github.com/nandazahran/KineAnchor/blob/main/docs/PRIVACY.md";
pub const LICENSE_FILE: &str = "https://github.com/nandazahran/KineAnchor/blob/main/LICENSE";
pub const DENY_FILE: &str = "https://github.com/nandazahran/KineAnchor/blob/main/deny.toml";

const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const FONT_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
const FONT_MONO: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

const RADIUS_SM: u8 = 4;
const RADIUS_MD: u8 = 6;
const RADIUS_LG: u8 = 8;

const MEDIUM_FAMILY: &str = "plex_medium";
const SEMIBOLD_FAMILY: &str = "plex_semibold";

/// Medium (500) weight family from the design system.
pub fn medium() -> FontFamily {
    FontFamily::Name(MEDIUM_FAMILY.into())
}

/// SemiBold (600) weight family from the design system.
pub fn semibold() -> FontFamily {
    FontFamily::Name(SEMIBOLD_FAMILY.into())
}

/// Installs IBM Plex Sans (400/500/600) and Plex Mono as the egui font families.
///
/// `set_fonts` only takes effect at the start of the next pass, so the first call returns
/// `true` and the caller must skip laying out text for that pass; later calls return `false`.
pub fn install_fonts(ctx: &egui::Context) -> bool {
    let key = egui::Id::new("kineanchor.plex_fonts");
    if ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
        return false;
    }

    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        ("plex_regular", FONT_REGULAR),
        ("plex_medium", FONT_MEDIUM),
        ("plex_semibold", FONT_SEMIBOLD),
        ("plex_mono", FONT_MONO),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    fonts.families.insert(
        FontFamily::Name(MEDIUM_FAMILY.into()),
        vec![MEDIUM_FAMILY.to_owned(), "plex_regular".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name(SEMIBOLD_FAMILY.into()),
        vec![SEMIBOLD_FAMILY.to_owned(), MEDIUM_FAMILY.to_owned()],
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "plex_regular".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "plex_mono".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::new(13.0, medium()));
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::new(15.0, semibold()));
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(11.0, FontFamily::Monospace),
    );
    ctx.set_style(style);

    ctx.data_mut(|d| d.insert_temp(key, true));
    true
}

/// Colour roles resolved for one theme (`v2` Precision Instrument tokens).
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub sheet: Color32,
    pub sheet_stroke: Color32,
    pub card: Color32,
    pub well: Color32,
    pub subtle: Color32,
    pub chip: Color32,
    pub tile: Color32,
    pub tile_stroke: Color32,
    pub ink: Color32,
    pub body: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub success_fg: Color32,
    pub success_bg: Color32,
    pub warn_fg: Color32,
    pub warn_bg: Color32,
    pub warn_border: Color32,
    pub error_fg: Color32,
    pub error_bg: Color32,
    pub error_border: Color32,
    pub cta_bg: Color32,
    pub cta_fg: Color32,
}

/// Resolves the `v2` design tokens for the current theme.
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark,
            bg: Color32::from_rgb(0x11, 0x12, 0x14),
            sheet: Color32::from_rgb(0x17, 0x18, 0x1B),
            sheet_stroke: Color32::from_rgb(0x27, 0x29, 0x2E),
            card: Color32::from_rgb(0x17, 0x18, 0x1B),
            well: Color32::from_rgb(0x1D, 0x1F, 0x23),
            subtle: Color32::from_rgb(0x27, 0x29, 0x2E),
            chip: Color32::from_rgb(0x25, 0x28, 0x2E),
            tile: Color32::from_rgb(0x17, 0x18, 0x1B),
            tile_stroke: Color32::from_rgb(0x27, 0x29, 0x2E),
            ink: Color32::from_rgb(0xF2, 0xF3, 0xF5),
            body: Color32::from_rgb(0x9B, 0xA0, 0xA8),
            muted: Color32::from_rgb(0x68, 0x6D, 0x75),
            accent: Color32::from_rgb(0x38, 0xBD, 0xF8),
            success_fg: Color32::from_rgb(0x22, 0xC5, 0x5E),
            success_bg: Color32::from_rgb(0x14, 0x22, 0x19),
            warn_fg: Color32::from_rgb(0xF5, 0x9E, 0x0B),
            warn_bg: Color32::from_rgb(0x1C, 0x18, 0x10),
            warn_border: Color32::from_rgb(0x3E, 0x32, 0x19),
            error_fg: Color32::from_rgb(0xF8, 0x71, 0x71),
            error_bg: Color32::from_rgb(0x1F, 0x14, 0x16),
            error_border: Color32::from_rgb(0x45, 0x22, 0x26),
            cta_bg: Color32::from_rgb(0xF2, 0xF3, 0xF5),
            cta_fg: Color32::from_rgb(0x11, 0x12, 0x14),
        }
    } else {
        Palette {
            dark,
            bg: Color32::from_rgb(0xF7, 0xF7, 0xF5),
            sheet: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            sheet_stroke: Color32::from_rgb(0xE2, 0xE2, 0xDE),
            card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            well: Color32::from_rgb(0xEE, 0xEE, 0xEB),
            subtle: Color32::from_rgb(0xE2, 0xE2, 0xDE),
            chip: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            tile: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            tile_stroke: Color32::from_rgb(0xE2, 0xE2, 0xDE),
            ink: Color32::from_rgb(0x14, 0x15, 0x17),
            body: Color32::from_rgb(0x5C, 0x5E, 0x63),
            muted: Color32::from_rgb(0x8A, 0x8D, 0x93),
            accent: Color32::from_rgb(0x02, 0x84, 0xC7),
            success_fg: Color32::from_rgb(0x16, 0xA3, 0x4A),
            success_bg: Color32::from_rgb(0xEC, 0xFD, 0xF3),
            warn_fg: Color32::from_rgb(0xB4, 0x53, 0x09),
            warn_bg: Color32::from_rgb(0xFE, 0xF9, 0xEC),
            warn_border: Color32::from_rgb(0xF0, 0xD9, 0xA0),
            error_fg: Color32::from_rgb(0xB9, 0x1C, 0x1C),
            error_bg: Color32::from_rgb(0xFE, 0xF2, 0xF2),
            error_border: Color32::from_rgb(0xF5, 0xC2, 0xC2),
            cta_bg: Color32::from_rgb(0x14, 0x15, 0x17),
            cta_fg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        }
    }
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

pub fn wordmark(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(14.5)
            .family(semibold())
            .color(p.ink),
    );
}

pub fn section(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .family(semibold())
            .color(p.ink),
    );
}

pub fn title(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .family(semibold())
            .color(p.ink),
    );
}

pub fn label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .family(medium())
            .color(p.ink),
    );
}

pub fn meta(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(p.body));
}

pub fn tiny(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .family(FontFamily::Monospace)
            .color(p.body),
    );
}

pub fn mono(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .family(FontFamily::Monospace)
            .color(p.ink),
    );
}

// ---------------------------------------------------------------------------
// Surfaces & structural hairlines
// ---------------------------------------------------------------------------

/// Flat section container (`fill` + `1px` bottom hairline) replacing nested rounded cards.
pub fn band<R>(
    ui: &mut Ui,
    p: &Palette,
    fill: Color32,
    pad: Margin,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let inner = egui::Frame::new()
        .fill(fill)
        .inner_margin(pad)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            add(ui)
        })
        .inner;
    divider(ui, p);
    inner
}

/// Flush surface section (no bottom divider, used for the final panel region).
pub fn band_last<R>(ui: &mut Ui, fill: Color32, pad: Margin, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(pad)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            add(ui)
        })
        .inner
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, p.subtle);
}

/// Quiet 6 px status dot + label (`V2StatusDot`), replacing neon pill badges.
pub fn status_dot(ui: &mut Ui, p: &Palette, text: &str, dot_color: Color32, text_color: Color32) {
    let font = FontId::new(12.0, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, text_color);
    let size = Vec2::new(6.0 + 6.0 + galley.size().x, 20.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(
        Pos2::new(rect.left() + 3.0, rect.center().y),
        3.0,
        dot_color,
    );
    painter.galley(
        Pos2::new(rect.left() + 12.0, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );
    let _ = p;
}

/// Hairline-separated structural row (`V2Row`).
pub fn info_row(
    ui: &mut Ui,
    p: &Palette,
    title_text: &str,
    meta_text: &str,
    subtitle_text: &str,
    mono_title: bool,
    last: bool,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 3.0);
        ui.horizontal(|ui| {
            let font = if mono_title {
                FontId::new(12.0, FontFamily::Monospace)
            } else {
                FontId::new(13.0, medium())
            };
            ui.label(egui::RichText::new(title_text).font(font).color(p.ink));
            if !meta_text.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(meta_text)
                            .size(11.0)
                            .family(FontFamily::Monospace)
                            .color(p.muted),
                    );
                });
            }
        });
        if !subtitle_text.is_empty() {
            ui.label(egui::RichText::new(subtitle_text).size(12.0).color(p.body));
        }
    });
    if !last {
        ui.add_space(4.0);
        divider(ui, p);
        ui.add_space(4.0);
    }
}

// ---------------------------------------------------------------------------
// Brand mark
// ---------------------------------------------------------------------------

fn draw_anchor(painter: &egui::Painter, rect: Rect, p: &Palette) {
    let c = rect.center();
    let s = rect.width() / 464.0;
    let dot_r = (24.0 * s).max(1.5);
    for (dx, dy) in [
        (-112.0, -88.0),
        (112.0, -88.0),
        (-112.0, 88.0),
        (112.0, 88.0),
    ] {
        painter.circle_filled(Pos2::new(c.x + dx * s, c.y + dy * s), dot_r, p.ink);
    }

    let pill = Rect::from_center_size(c, Vec2::new(144.0 * s, 40.0 * s));
    let fill = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 51);
    let stroke = Stroke::new((14.0 * s).max(1.2), p.accent);
    painter.rect(
        pill,
        CornerRadius::same((20.0 * s).round().max(1.0) as u8),
        fill,
        stroke,
        StrokeKind::Middle,
    );
}

/// 24 px brand tile (`V2Mark`). Clickable: toggles About/Settings.
pub fn anchor_tile(ui: &mut Ui, p: &Palette, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_MD),
        if resp.hovered() { p.subtle } else { p.tile },
        Stroke::new(1.0_f32, p.tile_stroke),
        StrokeKind::Inside,
    );
    draw_anchor(painter, rect, p);
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.5),
            CornerRadius::same(RADIUS_MD + 1),
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 48 px brand tile for the About identity row.
pub fn anchor_tile_large(ui: &mut Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(10),
        p.bg,
        Stroke::new(1.0_f32, p.tile_stroke),
        StrokeKind::Inside,
    );
    draw_anchor(painter, rect, p);
}

// ---------------------------------------------------------------------------
// Unified Lucide-style geometric vector icons (2 px-equivalent stroke)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Settings,
    Back,
    Refresh,
    Play,
    Pause,
    Recenter,
    Reset,
    QrCode,
    Copy,
    Unlink,
    Smartphone,
    Usb,
    WifiOff,
    Terminal,
    Layers,
    Alert,
    Code,
    Shield,
    FileText,
    ExternalLink,
}

fn draw_icon(painter: &egui::Painter, center: Pos2, size: f32, icon: Icon, color: Color32) {
    let r = size * 0.5;
    let stroke = Stroke::new((size * 0.115).max(1.3), color);
    let pt = |nx: f32, ny: f32| Pos2::new(center.x + nx * r, center.y + ny * r);

    match icon {
        Icon::Settings => {
            painter.circle_stroke(center, r * 0.34, stroke);
            for i in 0..8 {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                painter.line_segment(
                    [
                        Pos2::new(center.x + c * r * 0.55, center.y + s * r * 0.55),
                        Pos2::new(center.x + c * r * 0.84, center.y + s * r * 0.84),
                    ],
                    stroke,
                );
            }
        }
        Icon::Back => {
            painter.line_segment([pt(0.65, 0.0), pt(-0.65, 0.0)], stroke);
            painter.line_segment([pt(-0.15, -0.5), pt(-0.65, 0.0)], stroke);
            painter.line_segment([pt(-0.15, 0.5), pt(-0.65, 0.0)], stroke);
        }
        Icon::Refresh => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -0.4 + (i as f32 / 9.0) * (std::f32::consts::PI * 1.45);
                    Pos2::new(center.x + a.cos() * r * 0.65, center.y + a.sin() * r * 0.65)
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            painter.line_segment([pt(0.65, -0.65), pt(0.65, -0.2)], stroke);
            painter.line_segment([pt(0.25, -0.2), pt(0.65, -0.2)], stroke);
        }
        Icon::Play => {
            let tri = vec![pt(-0.42, -0.58), pt(0.62, 0.0), pt(-0.42, 0.58)];
            painter.add(egui::Shape::convex_polygon(
                tri,
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        Icon::Pause => {
            painter.rect_stroke(
                Rect::from_min_max(pt(-0.5, -0.58), pt(-0.14, 0.58)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(pt(0.14, -0.58), pt(0.5, 0.58)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Middle,
            );
        }
        Icon::Recenter => {
            painter.circle_stroke(center, r * 0.58, stroke);
            painter.circle_filled(center, (r * 0.14).max(1.2), color);
            painter.line_segment([pt(0.0, -0.88), pt(0.0, -0.52)], stroke);
            painter.line_segment([pt(0.0, 0.52), pt(0.0, 0.88)], stroke);
            painter.line_segment([pt(-0.88, 0.0), pt(-0.52, 0.0)], stroke);
            painter.line_segment([pt(0.52, 0.0), pt(0.88, 0.0)], stroke);
        }
        Icon::Reset => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -2.6 + (i as f32 / 9.0) * (std::f32::consts::PI * 1.45);
                    Pos2::new(center.x + a.cos() * r * 0.62, center.y + a.sin() * r * 0.62)
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            painter.line_segment([pt(-0.68, -0.65), pt(-0.68, -0.18)], stroke);
            painter.line_segment([pt(-0.22, -0.18), pt(-0.68, -0.18)], stroke);
        }
        Icon::QrCode => {
            for (x0, y0, x1, y1) in [
                (-0.7, -0.7, -0.15, -0.15),
                (0.15, -0.7, 0.7, -0.15),
                (-0.7, 0.15, -0.15, 0.7),
            ] {
                painter.rect_stroke(
                    Rect::from_min_max(pt(x0, y0), pt(x1, y1)),
                    CornerRadius::same(1),
                    stroke,
                    StrokeKind::Middle,
                );
            }
            painter.rect_filled(Rect::from_min_max(pt(0.2, 0.2), pt(0.7, 0.7)), 0, color);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(pt(-0.15, -0.15), pt(0.72, 0.72)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment([pt(-0.72, 0.15), pt(-0.72, -0.72)], stroke);
            painter.line_segment([pt(-0.72, -0.72), pt(0.15, -0.72)], stroke);
        }
        Icon::Unlink => {
            painter.line_segment([pt(-0.7, -0.1), pt(-0.15, -0.65)], stroke);
            painter.line_segment([pt(0.15, 0.65), pt(0.7, 0.1)], stroke);
            painter.line_segment([pt(-0.65, -0.72), pt(-0.65, -0.45)], stroke);
            painter.line_segment([pt(0.65, 0.45), pt(0.65, 0.72)], stroke);
        }
        Icon::Smartphone => {
            painter.rect_stroke(
                Rect::from_min_max(pt(-0.46, -0.76), pt(0.46, 0.76)),
                CornerRadius::same(2),
                stroke,
                StrokeKind::Middle,
            );
            painter.circle_filled(pt(0.0, 0.48), (r * 0.12).max(1.0), color);
        }
        Icon::Usb => {
            painter.line_segment([pt(0.0, 0.75), pt(0.0, -0.65)], stroke);
            painter.line_segment([pt(-0.22, -0.4), pt(0.0, -0.68)], stroke);
            painter.line_segment([pt(0.22, -0.4), pt(0.0, -0.68)], stroke);
            painter.line_segment([pt(0.0, 0.2), pt(-0.45, -0.15)], stroke);
            painter.line_segment([pt(0.0, 0.05), pt(0.45, -0.28)], stroke);
            painter.circle_filled(pt(0.0, 0.75), (r * 0.16).max(1.2), color);
        }
        Icon::WifiOff => {
            painter.line_segment([pt(-0.72, -0.72), pt(0.72, 0.72)], stroke);
            painter.line_segment([pt(-0.6, -0.15), pt(0.0, -0.45)], stroke);
            painter.line_segment([pt(0.18, -0.35), pt(0.6, -0.15)], stroke);
            painter.circle_filled(pt(0.0, 0.55), (r * 0.15).max(1.2), color);
        }
        Icon::Terminal => {
            painter.line_segment([pt(-0.62, -0.4), pt(-0.18, 0.0)], stroke);
            painter.line_segment([pt(-0.62, 0.4), pt(-0.18, 0.0)], stroke);
            painter.line_segment([pt(0.05, 0.45), pt(0.65, 0.45)], stroke);
        }
        Icon::Layers => {
            painter.add(egui::Shape::closed_line(
                vec![
                    pt(0.0, -0.68),
                    pt(0.72, -0.25),
                    pt(0.0, 0.18),
                    pt(-0.72, -0.25),
                ],
                stroke,
            ));
            painter.line_segment([pt(-0.72, 0.2), pt(0.0, 0.62)], stroke);
            painter.line_segment([pt(0.72, 0.2), pt(0.0, 0.62)], stroke);
        }
        Icon::Alert => {
            painter.add(egui::Shape::closed_line(
                vec![pt(0.0, -0.72), pt(0.74, 0.62), pt(-0.74, 0.62)],
                stroke,
            ));
            painter.line_segment([pt(0.0, -0.22), pt(0.0, 0.16)], stroke);
            painter.circle_filled(pt(0.0, 0.4), (r * 0.11).max(1.1), color);
        }
        Icon::Code => {
            painter.line_segment([pt(-0.35, -0.42), pt(-0.72, 0.0)], stroke);
            painter.line_segment([pt(-0.35, 0.42), pt(-0.72, 0.0)], stroke);
            painter.line_segment([pt(0.35, -0.42), pt(0.72, 0.0)], stroke);
            painter.line_segment([pt(0.35, 0.42), pt(0.72, 0.0)], stroke);
        }
        Icon::Shield => {
            painter.add(egui::Shape::closed_line(
                vec![
                    pt(0.0, -0.74),
                    pt(0.64, -0.44),
                    pt(0.56, 0.24),
                    pt(0.0, 0.74),
                    pt(-0.56, 0.24),
                    pt(-0.64, -0.44),
                ],
                stroke,
            ));
        }
        Icon::FileText => {
            painter.rect_stroke(
                Rect::from_min_max(pt(-0.55, -0.72), pt(0.55, 0.72)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment([pt(-0.28, -0.22), pt(0.28, -0.22)], stroke);
            painter.line_segment([pt(-0.28, 0.14), pt(0.28, 0.14)], stroke);
        }
        Icon::ExternalLink => {
            painter.line_segment([pt(0.05, -0.68), pt(0.68, -0.68)], stroke);
            painter.line_segment([pt(0.68, -0.68), pt(0.68, -0.05)], stroke);
            painter.line_segment([pt(-0.1, 0.1), pt(0.68, -0.68)], stroke);
            painter.line_segment([pt(-0.1, -0.45), pt(-0.68, -0.45)], stroke);
            painter.line_segment([pt(-0.68, -0.45), pt(-0.68, 0.68)], stroke);
            painter.line_segment([pt(-0.68, 0.68), pt(0.45, 0.68)], stroke);
            painter.line_segment([pt(0.45, 0.68), pt(0.45, 0.1)], stroke);
        }
    }
}

pub fn glyph(ui: &mut Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    draw_icon(ui.painter(), rect.center(), size, icon, color);
}

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

fn blended(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color32::from_rgba_premultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), 255)
}

fn hover_tint(base: Color32, dark: bool) -> Color32 {
    let (target, t) = if dark {
        (Color32::WHITE, 0.08)
    } else {
        (Color32::BLACK, 0.05)
    };
    blended(base, target, t)
}

fn button_colors(p: &Palette, kind: Kind) -> (Color32, Color32, Stroke) {
    match kind {
        Kind::Primary => (p.cta_fg, p.cta_bg, Stroke::new(1.0_f32, p.cta_bg)),
        Kind::Secondary => (p.ink, p.card, Stroke::new(1.0_f32, p.subtle)),
        Kind::Danger => (p.error_fg, p.error_bg, Stroke::new(1.0_f32, p.error_border)),
        Kind::Ghost => (p.body, Color32::TRANSPARENT, Stroke::NONE),
    }
}

/// Architectural `6px`-radius button (`V2ActionButton` / `V2Button`).
pub fn btn_sized(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    kind: Kind,
    width: Option<f32>,
    height: f32,
    radius: Option<u8>,
) -> Response {
    action_btn(ui, p, None, text, kind, width, height, radius)
}

/// Button pairing an optional leading Lucide glyph with a terse label (`V2ActionButton`).
#[allow(clippy::too_many_arguments)]
pub fn action_btn(
    ui: &mut Ui,
    p: &Palette,
    icon: Option<Icon>,
    text: &str,
    kind: Kind,
    width: Option<f32>,
    height: f32,
    radius: Option<u8>,
) -> Response {
    let (fg, base, border) = button_colors(p, kind);
    let font = FontId::new(13.0, medium());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let icon_space = if icon.is_some() { 20.0 } else { 0.0 };
    let w = width.unwrap_or(galley.size().x + icon_space + 26.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click());
    let fill = if resp.hovered() && kind != Kind::Ghost {
        hover_tint(base, p.dark)
    } else {
        base
    };
    let cr = CornerRadius::same(radius.unwrap_or(RADIUS_MD));
    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect(rect, cr, fill, border, StrokeKind::Inside);
    }
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            cr,
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    let content_w = icon_space + galley.size().x;
    let start_x = rect.center().x - content_w * 0.5;
    if let Some(ic) = icon {
        draw_icon(
            painter,
            Pos2::new(start_x + 7.0, rect.center().y),
            13.5,
            ic,
            fg,
        );
    }
    painter.galley(
        Pos2::new(
            start_x + icon_space,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        fg,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn btn(ui: &mut Ui, p: &Palette, text: &str, kind: Kind) -> Response {
    btn_sized(ui, p, text, kind, None, 34.0, None)
}

/// Square `28px` or `34px` flat icon button (`V2IconButton`).
pub fn icon_btn(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    kind: Kind,
    size: f32,
    tooltip: &str,
) -> Response {
    let (fg, base, border) = button_colors(p, kind);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let fill = if resp.hovered() {
        if kind == Kind::Ghost {
            p.well
        } else {
            hover_tint(base, p.dark)
        }
    } else {
        base
    };
    let cr = CornerRadius::same(RADIUS_MD);
    let painter = ui.painter();
    if fill != Color32::TRANSPARENT || border != Stroke::NONE {
        painter.rect(rect, cr, fill, border, StrokeKind::Inside);
    }
    draw_icon(painter, rect.center(), (size * 0.5).min(15.0), icon, fg);
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            cr,
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

/// Text-only action (used inside notices). Returns its response.
pub fn link(ui: &mut Ui, p: &Palette, text: &str, color: Color32) -> Response {
    let font = FontId::new(12.5, semibold());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 8.0, 24.0), Sense::click());
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect(
            rect,
            CornerRadius::same(RADIUS_MD),
            if p.dark {
                Color32::from_white_alpha(14)
            } else {
                Color32::from_black_alpha(10)
            },
            Stroke::NONE,
            StrokeKind::Inside,
        );
    }
    if resp.has_focus() {
        painter.rect_stroke(
            rect,
            CornerRadius::same(RADIUS_MD),
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    painter.galley(rect.left_top() + Vec2::new(4.0, 4.0), galley, color);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Keyboard activation for custom-painted controls: Enter/Space while focused.
pub fn activated(resp: &Response) -> bool {
    resp.clicked()
        || (resp.has_focus()
            && resp
                .ctx
                .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)))
}

#[derive(Clone, Copy)]
pub struct SegStyle {
    pub height: f32,
    pub font_size: f32,
    pub chip_radius: Option<u8>,
    pub width: Option<f32>,
}

impl SegStyle {
    pub fn header() -> Self {
        Self {
            height: 28.0,
            font_size: 11.5,
            chip_radius: Some(RADIUS_SM),
            width: Some(138.0),
        }
    }
    pub fn card() -> Self {
        Self {
            height: 32.0,
            font_size: 12.0,
            chip_radius: Some(RADIUS_SM),
            width: None,
        }
    }
    pub fn tab() -> Self {
        Self {
            height: 34.0,
            font_size: 12.5,
            chip_radius: Some(RADIUS_SM),
            width: None,
        }
    }
}

fn chip_radius(ui_radius: Option<u8>, height: f32) -> CornerRadius {
    match ui_radius {
        None => CornerRadius::same((height * 0.5) as u8),
        Some(r) => CornerRadius::same(r),
    }
}

/// Single-choice segmented control (`V2SegmentTrack` / `V2ModeSwitch`).
pub fn segmented(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    options: &[&str],
    selected: usize,
    style: SegStyle,
) -> Option<usize> {
    segmented_enabled(ui, p, id_salt, options, selected, style, &[])
}

pub fn segmented_enabled(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    options: &[&str],
    selected: usize,
    style: SegStyle,
    disabled: &[bool],
) -> Option<usize> {
    let width = style
        .width
        .map(|w| w.min(ui.available_width()))
        .unwrap_or(ui.available_width());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, style.height), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_MD),
        p.well,
        Stroke::new(1.0_f32, p.subtle),
        StrokeKind::Inside,
    );

    let pad = 3.0;
    let gap = 3.0;
    let n = options.len().max(1) as f32;
    let chip_w = (width - pad * 2.0 - gap * (n - 1.0)) / n;
    let cr = chip_radius(style.chip_radius, style.height - pad * 2.0);
    let mut picked = None;

    for (i, option) in options.iter().enumerate() {
        let left = rect.left() + pad + i as f32 * (chip_w + gap);
        let chip = Rect::from_min_size(
            Pos2::new(left, rect.top() + pad),
            Vec2::new(chip_w, style.height - pad * 2.0),
        );
        let resp = ui.interact(chip, egui::Id::new((id_salt, i)), Sense::click());
        let active = i == selected;
        let off = disabled.get(i).copied().unwrap_or(false);
        if active {
            painter.rect(
                chip,
                cr,
                p.chip,
                if p.dark {
                    Stroke::NONE
                } else {
                    Stroke::new(1.0_f32, p.subtle)
                },
                StrokeKind::Inside,
            );
        } else if resp.hovered() && !off {
            painter.rect(
                chip,
                cr,
                hover_tint(p.well, p.dark),
                Stroke::NONE,
                StrokeKind::Inside,
            );
        }
        if resp.has_focus() {
            painter.rect_stroke(
                chip,
                cr,
                Stroke::new(1.5_f32, p.accent),
                StrokeKind::Outside,
            );
        }
        let color = if active {
            p.ink
        } else if off {
            p.muted.gamma_multiply(0.55)
        } else {
            p.body
        };
        let family = if active {
            medium()
        } else {
            FontFamily::Proportional
        };
        painter.text(
            chip.center(),
            Align2::CENTER_CENTER,
            *option,
            FontId::new(style.font_size, family),
            color,
        );
        if activated(&resp) && !off {
            picked = Some(i);
        }
        if resp.hovered() && !off {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    picked
}

/// Independent toggle chips (multi-select). Returns true when any state changed.
pub fn toggles(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    labels: &[&str],
    states: &mut [bool],
    enabled: &[bool],
) -> bool {
    let width = ui.available_width();
    let height = 32.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_MD),
        p.well,
        Stroke::new(1.0_f32, p.subtle),
        StrokeKind::Inside,
    );
    let pad = 3.0;
    let gap = 3.0;
    let n = labels.len().max(1) as f32;
    let chip_w = (width - pad * 2.0 - gap * (n - 1.0)) / n;
    let mut changed = false;

    for (i, text) in labels.iter().enumerate() {
        let chip = Rect::from_min_size(
            Pos2::new(
                rect.left() + pad + i as f32 * (chip_w + gap),
                rect.top() + pad,
            ),
            Vec2::new(chip_w, height - pad * 2.0),
        );
        let on = states.get(i).copied().unwrap_or(false);
        let active = on && enabled.get(i).copied().unwrap_or(true);
        let resp = ui.interact(chip, egui::Id::new((id_salt, i)), Sense::click());
        if active {
            painter.rect(
                chip,
                CornerRadius::same(RADIUS_SM),
                p.chip,
                Stroke::NONE,
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            painter.rect(
                chip,
                CornerRadius::same(RADIUS_SM),
                hover_tint(p.well, p.dark),
                Stroke::NONE,
                StrokeKind::Inside,
            );
        }
        if resp.has_focus() {
            painter.rect_stroke(
                chip,
                CornerRadius::same(RADIUS_SM),
                Stroke::new(1.5_f32, p.accent),
                StrokeKind::Outside,
            );
        }
        let color = if active {
            p.ink
        } else if enabled.get(i).copied().unwrap_or(true) {
            p.body
        } else {
            p.muted.gamma_multiply(0.6)
        };
        painter.text(
            chip.center(),
            Align2::CENTER_CENTER,
            *text,
            FontId::new(
                12.0,
                if active {
                    medium()
                } else {
                    FontFamily::Proportional
                },
            ),
            color,
        );
        if activated(&resp) && enabled.get(i).copied().unwrap_or(true) {
            if let Some(state) = states.get_mut(i) {
                *state = !*state;
                changed = true;
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    changed
}

#[derive(Clone, Copy)]
pub struct SliderRange {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Snaps an x coordinate to a slider value. Extracted for unit tests.
pub fn value_at_x(x: f32, rect: Rect, range: SliderRange) -> f32 {
    let t = ((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
    let raw = range.min + t * (range.max - range.min);
    let snapped = if range.step > 0.0 {
        range.min + ((raw - range.min) / range.step).round() * range.step
    } else {
        raw
    };
    let clamped = snapped.clamp(range.min, range.max);
    (clamped * 10_000.0).round() / 10_000.0
}

/// Linear fader slider (`V2Slider`): 3 px track + rectangular 3 px-radius thumb + mono readout.
pub fn slider(
    ui: &mut Ui,
    p: &Palette,
    label_text: &str,
    value: &mut f32,
    range: SliderRange,
    format: &dyn Fn(f32) -> String,
) -> bool {
    if !label_text.is_empty() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label_text).size(12.0).color(p.body));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format(*value))
                        .size(11.5)
                        .family(FontFamily::Monospace)
                        .color(p.body),
                );
            });
        });
        slider_bar(ui, p, value, range)
    } else {
        let mut changed = false;
        ui.horizontal(|ui| {
            let val_str = format(*value);
            let bar_w = (ui.available_width() - 62.0).max(80.0);
            ui.allocate_ui(Vec2::new(bar_w, 18.0), |ui| {
                changed = slider_bar(ui, p, value, range);
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(val_str)
                        .size(11.5)
                        .family(FontFamily::Monospace)
                        .color(p.body),
                );
            });
        });
        changed
    }
}

fn slider_bar(ui: &mut Ui, p: &Palette, value: &mut f32, range: SliderRange) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click_and_drag());
    let mut next = *value;
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            next = value_at_x(pos.x, rect, range);
        }
    }
    if resp.has_focus() {
        let delta = ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) {
                range.step
            } else if i.key_pressed(egui::Key::ArrowLeft) {
                -range.step
            } else {
                0.0
            }
        });
        next = (next + delta).clamp(range.min, range.max);
    }
    let changed = (next - *value).abs() > f32::EPSILON;
    *value = next;

    let painter = ui.painter();
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 3.0));
    painter.rect_filled(track, CornerRadius::same(2), p.subtle);
    let t = ((*value - range.min) / (range.max - range.min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let fill_w = track.width() * t;
    painter.rect_filled(
        Rect::from_min_size(track.min, Vec2::new(fill_w, track.height())),
        CornerRadius::same(2),
        p.ink,
    );
    let thumb_center = Pos2::new(
        (track.left() + fill_w).clamp(track.left() + 4.0, track.right() - 4.0),
        track.center().y,
    );
    let thumb_rect = Rect::from_center_size(thumb_center, Vec2::new(8.0, 14.0));
    painter.rect(
        thumb_rect,
        CornerRadius::same(3),
        p.card,
        Stroke::new(1.5_f32, p.ink),
        StrokeKind::Inside,
    );
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            CornerRadius::same(RADIUS_SM),
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    changed
}

// ---------------------------------------------------------------------------
// Composite blocks
// ---------------------------------------------------------------------------

/// Inline status/error banner (`6px` radius + `1px` border). Returns true when action clicked.
#[allow(clippy::too_many_arguments)]
pub fn notice_box(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    fg: Color32,
    bg: Color32,
    border: Color32,
    heading: &str,
    body: &str,
    action_icon: Option<Icon>,
) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(RADIUS_MD))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                glyph(ui, icon, 15.0, fg);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                    ui.label(
                        egui::RichText::new(heading)
                            .size(12.5)
                            .family(medium())
                            .color(fg),
                    );
                    if !body.is_empty() {
                        ui.label(egui::RichText::new(body).size(11.5).color(p.body));
                    }
                });
                if let Some(act) = action_icon {
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        clicked = activated(&icon_btn(ui, p, act, Kind::Secondary, 28.0, ""));
                    });
                }
            });
        });
    clicked
}

/// Read-only monospace value box (`V2CodeBox`, `6px` radius, `30px` height).
pub fn value_box(ui: &mut Ui, p: &Palette, height: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(p.well)
        .stroke(Stroke::new(1.0_f32, p.subtle))
        .corner_radius(CornerRadius::same(RADIUS_MD))
        .inner_margin(Margin::symmetric(10, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(height);
            ui.vertical_centered(|ui| {
                ui.add_space((height - 14.0) * 0.5);
                add(ui);
            });
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScopeState {
    Active,
    Stale,
    Idle,
}

/// Precision Instrument Horizon Scope (`V2Scope`): grid, reticle, 8 dots, horizon bar, readout.
#[allow(clippy::too_many_arguments)]
pub fn horizon_scope(
    ui: &mut Ui,
    p: &Palette,
    size: Vec2,
    dx: f32,
    dy: f32,
    roll: f32,
    vertical_reticule: bool,
    state: ScopeState,
    status_text: &str,
    readout: &str,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_LG),
        p.card,
        Stroke::new(1.0_f32, p.subtle),
        StrokeKind::Inside,
    );

    let grid_stroke = Stroke::new(1.0_f32, p.subtle.gamma_multiply(0.3));
    let mut gx = rect.left() + 40.0;
    while gx < rect.right() {
        painter.line_segment(
            [
                Pos2::new(gx, rect.top() + 1.0),
                Pos2::new(gx, rect.bottom() - 1.0),
            ],
            grid_stroke,
        );
        gx += 40.0;
    }
    let mut gy = rect.top() + 40.0;
    while gy < rect.bottom() {
        painter.line_segment(
            [
                Pos2::new(rect.left() + 1.0, gy),
                Pos2::new(rect.right() - 1.0, gy),
            ],
            grid_stroke,
        );
        gy += 40.0;
    }

    let center = rect.center();
    let inner = rect.shrink(16.0);

    painter.line_segment(
        [
            Pos2::new(inner.left(), center.y),
            Pos2::new(inner.right(), center.y),
        ],
        Stroke::new(1.0_f32, p.subtle),
    );
    if vertical_reticule {
        painter.line_segment(
            [
                Pos2::new(center.x, inner.top()),
                Pos2::new(center.x, inner.bottom() - 12.0),
            ],
            Stroke::new(1.0_f32, p.subtle),
        );
    }
    painter.circle_filled(center, 22.0, p.bg);
    painter.circle_stroke(center, 22.0, Stroke::new(1.0_f32, p.subtle));

    let signal = match state {
        ScopeState::Active => p.accent,
        ScopeState::Stale => p.warn_fg,
        ScopeState::Idle => p.muted,
    };
    let dot_alpha = match state {
        ScopeState::Active => 1.0,
        ScopeState::Stale => 0.7,
        ScopeState::Idle => 0.5,
    };

    let ring = [
        (-0.42, -0.28),
        (0.0, -0.34),
        (0.42, -0.28),
        (-0.46, 0.0),
        (0.46, 0.0),
        (-0.42, 0.24),
        (0.0, 0.26),
        (0.42, 0.24),
    ];
    let (sin_r, cos_r) = roll.sin_cos();
    for (ux, uy) in ring {
        let ox = ux * inner.width() * 0.5 + dx;
        let oy = uy * inner.height() * 0.5 + dy;
        let pos = Pos2::new(
            (center.x + ox * cos_r - oy * sin_r).clamp(rect.left() + 8.0, rect.right() - 8.0),
            (center.y + ox * sin_r + oy * cos_r).clamp(rect.top() + 8.0, rect.bottom() - 24.0),
        );
        painter.circle_filled(pos, 2.2, signal.gamma_multiply(dot_alpha));
    }

    let half = match state {
        ScopeState::Active => 30.0,
        ScopeState::Stale => 24.0,
        ScopeState::Idle => 18.0,
    };
    let bar_center = Pos2::new(
        center.x,
        center.y + dy.clamp(-inner.height() * 0.28, inner.height() * 0.28),
    );
    painter.line_segment(
        [
            Pos2::new(bar_center.x - half * cos_r, bar_center.y - half * sin_r),
            Pos2::new(bar_center.x + half * cos_r, bar_center.y + half * sin_r),
        ],
        Stroke::new(2.0_f32, signal),
    );
    let hub_fill = if state == ScopeState::Idle {
        p.card
    } else {
        signal
    };
    painter.circle_filled(center, 3.5, hub_fill);
    painter.circle_stroke(
        center,
        3.5,
        Stroke::new(
            1.5_f32,
            if state == ScopeState::Idle {
                p.muted
            } else {
                p.card
            },
        ),
    );

    if !status_text.is_empty() {
        painter.text(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 12.0),
            Align2::LEFT_CENTER,
            status_text,
            FontId::new(11.0, FontFamily::Proportional),
            p.body,
        );
    }
    if !readout.is_empty() {
        painter.text(
            Pos2::new(rect.right() - 12.0, rect.bottom() - 12.0),
            Align2::RIGHT_CENTER,
            readout,
            FontId::new(10.5, FontFamily::Monospace),
            p.muted,
        );
    }
}

/// Crisp hairline-bordered QR matrix tile (`V2QrTile`).
pub fn qr_tile(ui: &mut Ui, p: &Palette, matrix: &(usize, Vec<bool>), size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_LG),
        Color32::WHITE,
        Stroke::new(1.0_f32, p.subtle),
        StrokeKind::Inside,
    );
    let (modules, cells) = matrix;
    let quiet = 8.0;
    let inner = (size - quiet * 2.0).max(1.0);
    let module = inner / *modules as f32;
    for y in 0..*modules {
        for x in 0..*modules {
            if cells.get(y * modules + x).copied().unwrap_or(false) {
                let min = Pos2::new(
                    rect.left() + quiet + x as f32 * module,
                    rect.top() + quiet + y as f32 * module,
                );
                painter.rect_filled(
                    Rect::from_min_size(min, Vec2::splat(module.ceil().min(module + 0.5))),
                    0,
                    Color32::from_rgb(0x11, 0x12, 0x14),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton, RawInput};

    fn drive<R>(
        screen: Rect,
        mut ui_fn: impl FnMut(&mut Ui) -> R,
        scripts: &[Vec<Event>],
    ) -> Vec<R> {
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let raw = RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                install_fonts(ctx);
                egui::CentralPanel::default().show(ctx, |_ui| {});
            });
        }
        let mut out = Vec::new();
        for (frame, events) in scripts.iter().enumerate() {
            let raw = RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 * 0.016),
                events: events.clone(),
                ..Default::default()
            };
            let mut value = None;
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    value = Some(ui_fn(ui));
                });
            });
            out.push(value.expect("panel must run"));
        }
        out
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    fn click_frames(pos: Pos2) -> [Vec<Event>; 4] {
        [
            vec![],
            vec![Event::PointerMoved(pos)],
            vec![Event::PointerMoved(pos), press(pos, true)],
            vec![press(pos, false)],
        ]
    }

    #[test]
    fn segmented_chip_click_selects() {
        let p = palette(true);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let pos = Pos2::new(8.0 + 3.0 + 64.5 + 3.0 + 32.0, 8.0 + 3.0 + 11.0);
        let results = drive(
            screen,
            |ui| {
                let mut picked = None;
                ui.horizontal(|ui| {
                    picked = segmented(
                        ui,
                        &p,
                        "test-seg",
                        &["Simple", "Advanced"],
                        0,
                        SegStyle::header(),
                    );
                });
                picked
            },
            &click_frames(pos),
        );
        assert_eq!(results[3], Some(1), "clicking the Advanced chip selects it");
    }

    #[test]
    fn toggle_chip_click_switches_state() {
        let p = palette(true);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let width = 400.0 - 16.0;
        let chip_w = (width - 6.0 - 3.0) / 2.0;
        let pos = Pos2::new(8.0 + 3.0 + chip_w + 3.0 + chip_w * 0.5, 8.0 + 3.0 + 14.0);
        let results = drive(
            screen,
            |ui| {
                let mut states = [true, false];
                let changed = toggles(
                    ui,
                    &p,
                    "test-toggle",
                    &["A", "B"],
                    &mut states,
                    &[true, true],
                );
                (changed, states)
            },
            &click_frames(pos),
        );
        assert_eq!(results[3], (true, [true, true]), "second chip flips on");
    }

    #[test]
    fn disabled_toggle_ignores_click() {
        let p = palette(true);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let width = 400.0 - 16.0;
        let chip_w = (width - 6.0 - 3.0) / 2.0;
        let pos = Pos2::new(8.0 + 3.0 + chip_w + 3.0 + chip_w * 0.5, 8.0 + 3.0 + 14.0);
        let results = drive(
            screen,
            |ui| {
                let mut states = [false, false];
                let changed = toggles(
                    ui,
                    &p,
                    "test-toggle-off",
                    &["A", "B"],
                    &mut states,
                    &[true, false],
                );
                (changed, states)
            },
            &click_frames(pos),
        );
        assert_eq!(
            results[3],
            (false, [false, false]),
            "disabled chip stays off"
        );
    }

    #[test]
    fn focused_button_activates_with_enter() {
        let p = palette(true);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let enter = vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }];
        let tab = vec![Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }];
        let results = drive(
            screen,
            |ui| {
                let resp = btn(ui, &p, "Go", Kind::Primary);
                resp.has_focus() && activated(&resp)
            },
            &[vec![], tab, vec![], enter],
        );
        assert!(results[3], "Enter on a focused button must activate it");
    }

    #[test]
    fn slider_maps_and_snaps_to_range() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 18.0));
        let range = SliderRange {
            min: 0.0,
            max: 2.0,
            step: 0.05,
        };
        assert_eq!(value_at_x(-50.0, rect, range), 0.0);
        assert_eq!(value_at_x(50.0, rect, range), 0.5);
        assert_eq!(value_at_x(250.0, rect, range), 2.0);
        assert_eq!(value_at_x(100.0, rect, range), 1.0);
        let snapped = value_at_x(101.0, rect, range);
        assert!((snapped - 1.0).abs() < 1e-4, "snapped to {snapped}");
    }
}
