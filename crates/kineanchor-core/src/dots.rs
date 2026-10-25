// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{Config, Dot};

#[derive(Debug, Clone)]
struct HomeDot {
    base_x: f32,
    base_y: f32,
}

pub struct DotField {
    home_dots: Vec<HomeDot>,
    active_dots: Vec<Dot>,
    cached_viewport: [f32; 2],
    cached_density: f32,
    cached_edge_band: f32,
}

impl Default for DotField {
    fn default() -> Self {
        Self::new()
    }
}

impl DotField {
    pub fn new() -> Self {
        Self {
            home_dots: Vec::new(),
            active_dots: Vec::new(),
            cached_viewport: [0.0, 0.0],
            cached_density: 0.0,
            cached_edge_band: 0.0,
        }
    }

    /// Recomputes home positions if viewport size, density, or edge band changed.
    fn ensure_home_dots(&mut self, width: f32, height: f32, config: &Config) {
        let density = config.density.clamp(0.2, 3.0);
        let edge_band = config.edge_band.clamp(0.05, 0.35);

        if (self.cached_viewport[0] - width).abs() < 1.0
            && (self.cached_viewport[1] - height).abs() < 1.0
            && (self.cached_density - density).abs() < 0.01
            && (self.cached_edge_band - edge_band).abs() < 0.01
            && !self.home_dots.is_empty()
        {
            return;
        }

        self.cached_viewport = [width, height];
        self.cached_density = density;
        self.cached_edge_band = edge_band;
        self.home_dots.clear();

        if width <= 10.0 || height <= 10.0 {
            return;
        }

        let spacing = (65.0 / density).max(30.0);
        let band_w = width * edge_band;
        let band_h = height * edge_band;

        // Left band
        let mut x = band_w * 0.3;
        while x <= band_w * 0.85 {
            let mut y = spacing * 0.5;
            while y < height {
                self.home_dots.push(HomeDot {
                    base_x: x,
                    base_y: y,
                });
                y += spacing;
            }
            x += spacing;
        }

        // Right band
        let mut x = width - band_w * 0.85;
        while x <= width - band_w * 0.3 {
            let mut y = spacing * 0.5;
            while y < height {
                self.home_dots.push(HomeDot {
                    base_x: x,
                    base_y: y,
                });
                y += spacing;
            }
            x += spacing;
        }

        // Top band (between left and right bands)
        let mut y = band_h * 0.3;
        while y <= band_h * 0.85 {
            let mut x = band_w + spacing;
            while x < width - band_w {
                self.home_dots.push(HomeDot {
                    base_x: x,
                    base_y: y,
                });
                x += spacing;
            }
            y += spacing;
        }

        // Bottom band (between left and right bands)
        let mut y = height - band_h * 0.85;
        while y <= height - band_h * 0.3 {
            let mut x = band_w + spacing;
            while x < width - band_w {
                self.home_dots.push(HomeDot {
                    base_x: x,
                    base_y: y,
                });
                x += spacing;
            }
            y += spacing;
        }
    }

    /// Computes current dot positions given displacement and roll angle.
    pub fn update(
        &mut self,
        viewport: [f32; 2],
        dx: f32,
        dy: f32,
        roll: f32,
        config: &Config,
    ) -> &[Dot] {
        let (width, height) = (viewport[0], viewport[1]);
        self.ensure_home_dots(width, height, config);

        self.active_dots.clear();
        self.active_dots.reserve(self.home_dots.len());

        let cx = width * 0.5;
        let cy = height * 0.5;
        let (sin_r, cos_r) = roll.sin_cos();

        let base_alpha = config.opacity.clamp(0.05, 0.8); // Capped at 0.8 for Android 12+ touch rule & ergonomics
        let radius = config.dot_radius_px.clamp(2.0, 30.0);

        for home in &self.home_dots {
            // Apply displacement
            let px = home.base_x + dx;
            let py = home.base_y + dy;

            // Apply roll rotation around screen center
            let rx = px - cx;
            let ry = py - cy;
            let final_x = cx + rx * cos_r - ry * sin_r;
            let final_y = cy + rx * sin_r + ry * cos_r;

            // Edge boundary fade-out
            let margin = 10.0;
            if final_x < -margin
                || final_x > width + margin
                || final_y < -margin
                || final_y > height + margin
            {
                continue;
            }

            // Smooth fade near edges
            let dist_x = (final_x).min(width - final_x).max(0.0);
            let dist_y = (final_y).min(height - final_y).max(0.0);
            let edge_dist = dist_x.min(dist_y);
            let fade = (edge_dist / 20.0).clamp(0.0, 1.0);

            let alpha = base_alpha * fade;
            if alpha > 0.01 {
                self.active_dots.push(Dot {
                    x: final_x,
                    y: final_y,
                    r: radius,
                    alpha,
                });
            }
        }

        &self.active_dots
    }

    pub fn dots(&self) -> &[Dot] {
        &self.active_dots
    }
}
