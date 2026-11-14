//! Builds the smoke-test frame as a triangle list. Pure Rust, no platform calls.

use crate::font::{GLYPH_W, glyph, row_runs};
use alloc::vec::Vec;
use platform_switch::Vertex;

const PI: f32 = core::f32::consts::PI;
const TAU: f32 = core::f32::consts::TAU;

/// `(sin x, cos x)` without libm: reduce to [-pi, pi], fold to [-pi/2, pi/2], then a Taylor series.
/// Absolute error is below 1e-6 over that range, far more than a demo needs.
pub fn sin_cos(x: f32) -> (f32, f32) {
    fn sin_small(x: f32) -> f32 {
        // x in [-pi/2, pi/2]
        let x2 = x * x;
        x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0)))))
    }
    fn sin_any(x: f32) -> f32 {
        let mut r = x - TAU * ((x / TAU) as i64 as f32);
        if r > PI {
            r -= TAU;
        } else if r < -PI {
            r += TAU;
        }
        if r > PI / 2.0 {
            r = PI - r;
        } else if r < -PI / 2.0 {
            r = -PI - r;
        }
        sin_small(r)
    }
    (sin_any(x), sin_any(x + PI / 2.0))
}

/// HSV (all 0..1) to linear RGB.
fn hue(h: f32) -> [f32; 3] {
    let h = (h - (h as i64 as f32)).abs() * 6.0;
    let i = h as u32;
    let f = h - i as f32;
    match i % 6 {
        0 => [1.0, f, 0.0],
        1 => [1.0 - f, 1.0, 0.0],
        2 => [0.0, 1.0, f],
        3 => [0.0, 1.0 - f, 1.0],
        4 => [f, 0.0, 1.0],
        _ => [1.0, 0.0, 1.0 - f],
    }
}

pub struct Scene {
    width: f32,
    height: f32,
}

impl Scene {
    /// Upper bound on what [`Scene::build`] emits; the renderer is created with this capacity.
    pub const MAX_VERTICES: usize = 8192;

    pub fn new(width: f32, height: f32) -> Scene {
        Scene { width, height }
    }

    fn to_ndc(&self, x: f32, y: f32) -> [f32; 2] {
        [x / self.width * 2.0 - 1.0, y / self.height * 2.0 - 1.0]
    }

    fn rect(&self, out: &mut Vec<Vertex>, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) {
        let (a, b, c, d) = (self.to_ndc(x0, y0), self.to_ndc(x1, y0), self.to_ndc(x1, y1), self.to_ndc(x0, y1));
        for pos in [a, b, c, a, c, d] {
            out.push(Vertex { pos, color });
        }
    }

    /// Width in pixels of `text` drawn at `scale` pixels per font pixel (one blank column between glyphs).
    pub fn text_width(text: &str, scale: f32) -> f32 {
        let n = text.chars().count() as f32;
        if n == 0.0 { 0.0 } else { (n * (GLYPH_W as f32 + 1.0) - 1.0) * scale }
    }

    /// Draws `text` with its top-left at (x, y). `style(index)` gives each glyph a y offset and a colour.
    fn text(
        &self,
        out: &mut Vec<Vertex>,
        text: &str,
        x: f32,
        y: f32,
        scale: f32,
        mut style: impl FnMut(usize) -> (f32, [f32; 4]),
    ) {
        for (i, c) in text.chars().enumerate() {
            let Some(rows) = glyph(c) else { continue };
            let (dy, color) = style(i);
            let gx = x + i as f32 * (GLYPH_W as f32 + 1.0) * scale;
            for (r, &bits) in rows.iter().enumerate() {
                for (col, len) in row_runs(bits) {
                    let x0 = gx + col as f32 * scale;
                    let y0 = y + dy + r as f32 * scale;
                    self.rect(out, x0, y0, x0 + len as f32 * scale, y0 + scale, color);
                }
            }
        }
    }

    /// Appends the frame at time `t` (seconds) to `out` and returns the clear colour.
    pub fn build(&self, t: f32, out: &mut Vec<Vertex>) -> [f32; 4] {
        let (w, h) = (self.width, self.height);

        // Background: a deep blue that breathes slowly.
        let (s, _) = sin_cos(t * 0.6);
        let clear = [0.04, 0.07 + 0.03 * s, 0.18 + 0.05 * s, 1.0];

        // Floor band, so the frame is clearly not just a clear colour.
        self.rect(out, 0.0, h * 0.86, w, h, [0.02, 0.03, 0.08, 1.0]);

        // Title: "SlyNX" in big letters, each bobbing on its own phase, with a drop shadow.
        let title = "SlyNX";
        let scale = 18.0;
        let tx = (w - Self::text_width(title, scale)) * 0.5;
        let ty = h * 0.08;
        let bob = |i: usize| sin_cos(t * 3.0 + i as f32 * 0.7).0 * 10.0;
        self.text(out, title, tx + 8.0, ty + 8.0, scale, |i| (bob(i), [0.0, 0.0, 0.0, 1.0]));
        self.text(out, title, tx, ty, scale, |i| {
            let [r, g, b] = hue(t * 0.1 + i as f32 * 0.12);
            (bob(i), [0.55 + 0.45 * r, 0.55 + 0.45 * g, 0.55 + 0.45 * b, 1.0])
        });

        // Spinning triangle with rainbow corners.
        let (cx, cy, radius) = (w * 0.5, h * 0.58, h * 0.2);
        let angle = t * 1.2;
        let mut corners = [Vertex::default(); 3];
        for (k, v) in corners.iter_mut().enumerate() {
            let a = angle + k as f32 * TAU / 3.0;
            let (sa, ca) = sin_cos(a);
            let [r, g, b] = hue(t * 0.2 + k as f32 / 3.0);
            *v = Vertex { pos: self.to_ndc(cx + ca * radius, cy + sa * radius), color: [r, g, b, 1.0] };
        }
        out.extend_from_slice(&corners);

        // Status lines.
        let white = [0.92, 0.94, 1.0, 1.0];
        let grey = [0.6, 0.65, 0.8, 1.0];
        let line1 = "DEKO3D + RUST  V0.0.1";
        let line2 = "PRESS + TO EXIT";
        let small = 4.0;
        self.text(out, line1, (w - Self::text_width(line1, small)) * 0.5, h * 0.885, small, |_| (0.0, white));
        self.text(out, line2, (w - Self::text_width(line2, small)) * 0.5, h * 0.94, small, |_| (0.0, grey));

        debug_assert!(out.len() <= Self::MAX_VERTICES);
        clear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_is_accurate() {
        let mut x = -20.0f32;
        while x < 20.0 {
            let (s, c) = sin_cos(x);
            assert!((s - x.sin()).abs() < 1e-4, "sin {x}: {s} vs {}", x.sin());
            assert!((c - x.cos()).abs() < 1e-4, "cos {x}: {c} vs {}", x.cos());
            x += 0.013;
        }
    }

    #[test]
    fn frame_fits_budget_and_stays_on_screen() {
        let scene = Scene::new(1280.0, 720.0);
        let mut out = Vec::new();
        for i in 0..600 {
            out.clear();
            let clear = scene.build(i as f32 / 60.0, &mut out);
            assert!(out.len() <= Scene::MAX_VERTICES);
            assert_eq!(out.len() % 3, 0, "whole triangles only");
            assert!(clear.iter().all(|c| (0.0..=1.0).contains(c)));
            for v in &out {
                assert!(v.pos.iter().all(|p| p.is_finite() && (-1.0..=1.0).contains(p)), "{v:?}");
                assert!(v.color.iter().all(|c| (0.0..=1.0).contains(c)), "{v:?}");
            }
        }
    }
}
