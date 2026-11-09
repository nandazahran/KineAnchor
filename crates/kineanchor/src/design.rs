// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual language for the desktop settings UI, transcribed from the Penpot file
//! "KineAnchor UI/UX: Desktop, Android & Overlay" (Desktop Simple, Desktop Advanced,
//! Desktop About, Desktop Legal boards).
//!
//! The palette, radii, type scale and control geometry here are the design source of truth.
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

const CARD_RADIUS: u8 = 20;
const WELL_RADIUS: u8 = 14;

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
        FontId::new(14.0, FontFamily::Proportional),
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
        .insert(egui::TextStyle::Heading, FontId::new(18.0, semibold()));
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(11.0, FontFamily::Monospace),
    );
    ctx.set_style(style);

    ctx.data_mut(|d| d.insert_temp(key, true));
    true
}

/// Colour roles resolved for one theme. Dark is the design's primary theme; light mirrors it.
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
    pub error_fg: Color32,
    pub error_bg: Color32,
    pub cta_bg: Color32,
    pub cta_fg: Color32,
}

/// Resolves the design tokens for the current theme.
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark,
            bg: Color32::from_rgb(0x0C, 0x0D, 0x0F),
            sheet: Color32::from_rgb(0x0C, 0x0D, 0x0F),
            sheet_stroke: Color32::from_rgb(0x15, 0x17, 0x1A),
            card: Color32::from_rgb(0x15, 0x17, 0x1A),
            well: Color32::from_rgb(0x0C, 0x0D, 0x0F),
            subtle: Color32::from_rgb(0x22, 0x25, 0x2A),
            chip: Color32::from_rgb(0x26, 0x2A, 0x30),
            tile: Color32::from_rgb(0x0C, 0x0D, 0x0F),
            tile_stroke: Color32::from_rgb(0x22, 0x25, 0x2A),
            ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            body: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            muted: Color32::from_rgb(0x94, 0x94, 0x94),
            accent: Color32::from_rgb(0x4C, 0xB3, 0xF0),
            success_fg: Color32::from_rgb(0xC6, 0xF2, 0x4E),
            success_bg: Color32::from_rgb(0x22, 0x2A, 0x10),
            warn_fg: Color32::from_rgb(0xE5, 0xA9, 0x3B),
            warn_bg: Color32::from_rgb(0x2B, 0x24, 0x14),
            error_fg: Color32::from_rgb(0xE5, 0x56, 0x6F),
            error_bg: Color32::from_rgb(0x2C, 0x1A, 0x20),
            cta_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            cta_fg: Color32::from_rgb(0x0C, 0x0D, 0x0F),
        }
    } else {
        Palette {
            dark,
            bg: Color32::from_rgb(0xF2, 0xF5, 0xF7),
            sheet: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            sheet_stroke: Color32::from_rgb(0xE8, 0xEA, 0xEC),
            card: Color32::from_rgb(0xF4, 0xF6, 0xF8),
            well: Color32::from_rgb(0xEB, 0xED, 0xEE),
            subtle: Color32::from_rgb(0xE8, 0xEA, 0xEC),
            chip: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            tile: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            tile_stroke: Color32::from_rgb(0xDC, 0xE1, 0xE6),
            ink: Color32::from_rgb(0x13, 0x14, 0x16),
            body: Color32::from_rgb(0x5D, 0x5F, 0x60),
            muted: Color32::from_rgb(0x5D, 0x5F, 0x60),
            accent: Color32::from_rgb(0x1B, 0x6C, 0x80),
            success_fg: Color32::from_rgb(0x04, 0x25, 0x0E),
            success_bg: Color32::from_rgb(0xDC, 0xFA, 0xE5),
            warn_fg: Color32::from_rgb(0xE5, 0xA9, 0x3B),
            warn_bg: Color32::from_rgb(0x2B, 0x24, 0x14),
            error_fg: Color32::from_rgb(0xE5, 0x56, 0x6F),
            error_bg: Color32::from_rgb(0x2C, 0x1A, 0x20),
            cta_bg: Color32::from_rgb(0x13, 0x14, 0x16),
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
            .size(18.0)
            .family(semibold())
            .color(p.ink),
    );
}

pub fn section(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .family(medium())
            .color(p.ink),
    );
}

pub fn title(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(18.0)
            .family(semibold())
            .color(p.ink),
    );
}

pub fn label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(14.0).color(p.ink));
}

pub fn meta(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(p.body));
}

pub fn tiny(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(11.0).color(p.muted));
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
// Surfaces
// ---------------------------------------------------------------------------

/// Rounded card (radius 20, fill `card`) spanning the available width.
pub fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(p.card)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            add(ui)
        })
        .inner
}

/// Inset well used behind segmented controls, the scope and read-only values.
pub fn well<R>(ui: &mut Ui, p: &Palette, radius: u8, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(p.well)
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, p.subtle);
}

/// Status pill: 24 px tall, fully rounded, text 12 px medium.
pub fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    let font = FontId::new(12.0, medium());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let size = Vec2::new(galley.size().x + 24.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(12),
        bg,
        Stroke::NONE,
        StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() * 0.5, galley, fg);
}

// ---------------------------------------------------------------------------
// Brand mark
// ---------------------------------------------------------------------------

fn draw_anchor(painter: &egui::Painter, rect: Rect, p: &Palette) {
    let c = rect.center();
    let s = rect.width() / 24.0;
    let white = Stroke::new((2.0 * s).max(1.2), p.ink);
    let accent = Stroke::new((2.0 * s).max(1.2), p.accent);

    painter.line_segment(
        [Pos2::new(c.x, c.y - 8.0 * s), Pos2::new(c.x, c.y + 6.0 * s)],
        white,
    );
    painter.circle_stroke(Pos2::new(c.x, c.y - 9.0 * s), 2.0 * s, white);
    painter.line_segment(
        [Pos2::new(c.x - 6.0 * s, c.y), Pos2::new(c.x - 3.0 * s, c.y)],
        accent,
    );
    painter.line_segment(
        [Pos2::new(c.x + 3.0 * s, c.y), Pos2::new(c.x + 6.0 * s, c.y)],
        accent,
    );
    painter.circle_filled(Pos2::new(c.x - 5.0 * s, c.y), 1.6 * s, p.accent);
    painter.circle_filled(Pos2::new(c.x + 5.0 * s, c.y), 1.6 * s, p.accent);

    // Fluke arc (smile) plus the two ears.
    let mut points = Vec::with_capacity(18);
    for i in 0..=16 {
        let t = i as f32 / 16.0;
        let angle = std::f32::consts::PI * (0.15 + 0.70 * t);
        points.push(Pos2::new(
            c.x - (8.0 * s) * angle.cos(),
            c.y + 6.0 * s - (4.0 * s) * angle.sin(),
        ));
    }
    painter.add(egui::Shape::line(points, white));
    painter.line_segment(
        [
            Pos2::new(c.x - 7.4 * s, c.y + 2.0 * s),
            Pos2::new(c.x - 6.4 * s, c.y - 0.5 * s),
        ],
        white,
    );
    painter.line_segment(
        [
            Pos2::new(c.x + 7.4 * s, c.y + 2.0 * s),
            Pos2::new(c.x + 6.4 * s, c.y - 0.5 * s),
        ],
        white,
    );
}

/// 24 px brand tile. Clickable: opens About & privacy.
pub fn anchor_tile(ui: &mut Ui, p: &Palette, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same((size * 0.25) as u8),
        if resp.hovered() { p.subtle } else { p.tile },
        Stroke::new(1.0_f32, p.tile_stroke),
        StrokeKind::Inside,
    );
    draw_anchor(painter, rect, p);
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.5),
            CornerRadius::same((size * 0.25) as u8 + 1),
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Large 64 px brand tile for the About identity card.
pub fn anchor_tile_large(ui: &mut Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(14),
        p.tile,
        Stroke::new(1.0_f32, p.tile_stroke),
        StrokeKind::Inside,
    );
    draw_anchor(painter, rect, p);
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

/// Pill/rounded button. `width` forces a fixed width; `radius` None = fully rounded.
pub fn btn_sized(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    kind: Kind,
    width: Option<f32>,
    height: f32,
    radius: Option<u8>,
) -> Response {
    let fg = match kind {
        Kind::Primary => p.cta_fg,
        Kind::Secondary => p.ink,
        Kind::Danger => p.error_fg,
        Kind::Ghost => p.body,
    };
    let font = FontId::new(13.0, medium());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let w = width.unwrap_or(galley.size().x + 32.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click());
    let base = match kind {
        Kind::Primary => p.cta_bg,
        Kind::Secondary => p.chip,
        Kind::Danger => p.error_bg,
        Kind::Ghost => Color32::TRANSPARENT,
    };
    let fill = if resp.hovered() && kind != Kind::Ghost {
        hover_tint(base, p.dark)
    } else {
        base
    };
    let cr = CornerRadius::same(radius.unwrap_or((height * 0.5) as u8));
    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect(rect, cr, fill, Stroke::NONE, StrokeKind::Inside);
    }
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            cr,
            Stroke::new(1.5_f32, p.accent),
            StrokeKind::Outside,
        );
    }
    painter.galley(rect.center() - galley.size() * 0.5, galley, fg);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn btn(ui: &mut Ui, p: &Palette, text: &str, kind: Kind) -> Response {
    btn_sized(ui, p, text, kind, None, 34.0, None)
}

/// Text-only action (used inside notices). Returns its response.
pub fn link(ui: &mut Ui, p: &Palette, text: &str, color: Color32) -> Response {
    let font = FontId::new(13.0, semibold());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 8.0, 24.0), Sense::click());
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect(
            rect,
            CornerRadius::same(6),
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
            CornerRadius::same(6),
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
            font_size: 11.0,
            chip_radius: None,
            width: Some(138.0),
        }
    }
    pub fn card() -> Self {
        Self {
            height: 34.0,
            font_size: 12.0,
            chip_radius: Some(7),
            width: None,
        }
    }
    pub fn tab() -> Self {
        Self {
            height: 36.0,
            font_size: 12.0,
            chip_radius: Some(9),
            width: None,
        }
    }
}

/// `None` = fully rounded (pill), `Some(r)` = fixed radius in points.
fn chip_radius(ui_radius: Option<u8>, height: f32) -> CornerRadius {
    match ui_radius {
        None => CornerRadius::same((height * 0.5) as u8),
        Some(r) => CornerRadius::same(r),
    }
}

/// Single-choice segmented control. Returns the newly selected index.
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

/// Same control with per-option availability. A disabled option cannot be chosen; it is
/// still painted so the design's four-way well keeps its shape.
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
        CornerRadius::same((style.height * 0.5) as u8),
        p.well,
        Stroke::NONE,
        StrokeKind::Inside,
    );

    let pad = 3.0;
    let gap = if options.len() > 2 { 4.0 } else { 3.0 };
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
            painter.rect(chip, cr, p.chip, Stroke::NONE, StrokeKind::Inside);
        } else if resp.hovered() {
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
        if resp.hovered() {
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
    let height = 34.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(10),
        p.well,
        Stroke::NONE,
        StrokeKind::Inside,
    );
    let pad = 3.0;
    let gap = 4.0;
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
                CornerRadius::same(7),
                p.chip,
                Stroke::NONE,
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            painter.rect(
                chip,
                CornerRadius::same(7),
                hover_tint(p.well, p.dark),
                Stroke::NONE,
                StrokeKind::Inside,
            );
        }
        if resp.has_focus() {
            painter.rect_stroke(
                chip,
                CornerRadius::same(7),
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

/// Label + value row above a 6 px track with a 14 px thumb.
pub fn slider(
    ui: &mut Ui,
    p: &Palette,
    label_text: &str,
    value: &mut f32,
    range: SliderRange,
    format: &dyn Fn(f32) -> String,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label_text).size(14.0).color(p.ink));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format(*value))
                    .size(13.0)
                    .family(medium())
                    .color(p.body),
            );
        });
    });

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
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 6.0));
    painter.rect_filled(track, CornerRadius::same(3), p.subtle);
    let t = ((*value - range.min) / (range.max - range.min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let fill_w = track.width() * t;
    painter.rect_filled(
        Rect::from_min_size(track.min, Vec2::new(fill_w, track.height())),
        CornerRadius::same(3),
        p.ink,
    );
    let thumb = Pos2::new(track.left() + fill_w, track.center().y);
    painter.circle_filled(thumb, 7.0, p.ink);
    painter.circle_stroke(thumb, 7.0, Stroke::new(2.0_f32, p.card));
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            CornerRadius::same(4),
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

/// Inline status/error well. Returns true when its action was activated.
pub fn notice(
    ui: &mut Ui,
    p: &Palette,
    fg: Color32,
    bg: Color32,
    heading: &str,
    body: &str,
    action: Option<&str>,
) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
            ui.label(
                egui::RichText::new(heading)
                    .size(13.0)
                    .family(medium())
                    .color(fg),
            );
            ui.label(egui::RichText::new(body).size(12.0).color(fg));
            if let Some(text) = action {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    clicked = activated(&link(ui, p, text, fg));
                });
            }
        });
    clicked
}

/// Read-only value box (design's URI / config-path wells).
pub fn value_box(ui: &mut Ui, p: &Palette, height: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(p.well)
        .stroke(Stroke::new(1.0_f32, p.subtle))
        .corner_radius(CornerRadius::same(8))
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

/// Preview of the overlay's inertial horizon: well, reticule, horizon bar, cue dots.
pub fn horizon_scope(
    ui: &mut Ui,
    p: &Palette,
    size: Vec2,
    dx: f32,
    dy: f32,
    roll: f32,
    vertical_reticule: bool,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(WELL_RADIUS),
        p.well,
        Stroke::NONE,
        StrokeKind::Inside,
    );
    let center = rect.center();
    let inner = rect.shrink(16.0);

    painter.line_segment(
        [
            Pos2::new(inner.left(), center.y),
            Pos2::new(inner.right(), center.y),
        ],
        Stroke::new(2.0_f32, p.subtle),
    );
    if vertical_reticule {
        painter.line_segment(
            [
                Pos2::new(center.x, inner.top()),
                Pos2::new(center.x, inner.bottom()),
            ],
            Stroke::new(2.0_f32, p.subtle),
        );
    }
    painter.circle_stroke(center, 20.0, Stroke::new(1.0_f32, p.subtle));

    // Cue dots: same ring layout as the design, displaced by the live offset/roll.
    let ring = [
        (-0.42, -0.30),
        (0.0, -0.36),
        (0.42, -0.30),
        (-0.46, 0.0),
        (0.46, 0.0),
        (-0.42, 0.30),
        (0.0, 0.36),
        (0.42, 0.30),
    ];
    let (sin_r, cos_r) = roll.sin_cos();
    let scale = 1.0;
    for (ux, uy) in ring {
        let ox = ux * inner.width() * 0.5 + dx * scale;
        let oy = uy * inner.height() * 0.5 + dy * scale;
        let pos = Pos2::new(
            center.x + ox * cos_r - oy * sin_r,
            center.y + ox * sin_r + oy * cos_r,
        );
        painter.circle_filled(pos, 3.0, p.accent);
        painter.circle_stroke(pos, 3.0, Stroke::new(1.0_f32, p.well));
    }

    // Artificial horizon bar: rolls with the live roll angle.
    let bar_center = Pos2::new(
        center.x,
        center.y + dy.clamp(-inner.height() * 0.3, inner.height() * 0.3),
    );
    let half = 28.0;
    painter.line_segment(
        [
            Pos2::new(bar_center.x - half * cos_r, bar_center.y - half * sin_r),
            Pos2::new(bar_center.x + half * cos_r, bar_center.y + half * sin_r),
        ],
        Stroke::new(2.0_f32, p.accent),
    );
    painter.circle_filled(center, 6.0, p.accent);
    painter.circle_stroke(center, 6.0, Stroke::new(2.0_f32, p.ink));
}

/// QR matrix inside a white rounded tile, as in the Wireless QR board.
pub fn qr_tile(ui: &mut Ui, matrix: &(usize, Vec<bool>), size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(12),
        Color32::WHITE,
        Stroke::NONE,
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
                    Color32::from_rgb(0x0C, 0x0D, 0x0F),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton, RawInput};

    /// Runs `ui_fn` for a few passes with the given scripted pointer events.
    /// Passes are separate frames because hover state settles one frame before clicks.
    fn drive<R>(
        screen: Rect,
        mut ui_fn: impl FnMut(&mut Ui) -> R,
        scripts: &[Vec<Event>],
    ) -> Vec<R> {
        let ctx = egui::Context::default();
        // Font definitions only apply on the pass after `set_fonts`; warm up twice.
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

    /// egui hit-tests against the previous frame's widget rects, so interaction is spread
    /// over frames: layout, hover, press, release.
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
        // Chip 1 of a 138 px header switch: x = left + 3 + 64.5 + 3, y = top + 3 + 11.
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
        // Two equal chips across the full width: second chip starts at (w-6-4)/2 + 7.
        let width = 400.0 - 16.0;
        let chip_w = (width - 6.0 - 4.0) / 2.0;
        let pos = Pos2::new(8.0 + 3.0 + chip_w + 4.0 + chip_w * 0.5, 8.0 + 3.0 + 14.0);
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
        let chip_w = (width - 6.0 - 4.0) / 2.0;
        let pos = Pos2::new(8.0 + 3.0 + chip_w + 4.0 + chip_w * 0.5, 8.0 + 3.0 + 14.0);
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
        // egui grants keyboard focus on Tab, not on click.
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
