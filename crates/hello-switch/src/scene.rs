//! Builds the demo frame as a triangle list. Pure Rust, no platform calls.
//!
//! Everything is laid out in a fixed 1280x720 design space and converted to normalised device coordinates, so the
//! same frame fills the screen at 1280x720 (handheld) and 1920x1080 (docked).

use crate::font::{GLYPH_W, glyph, row_runs};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use platform_api::{Buttons, DisplayInfo, InputState, PadState, Stick, Vertex};

const PI: f32 = core::f32::consts::PI;
const TAU: f32 = core::f32::consts::TAU;

/// Design-space size.
pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;

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

/// A fully saturated, full-value colour for hue `h` (0..1, wrapping), as RGB components in 0..1. The components
/// are written to the framebuffer as they are (no gamma conversion).
pub fn hue(h: f32) -> [f32; 3] {
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

/// What one frame shows.
pub struct View<'a> {
    /// Seconds since start, for the animations.
    pub t: f32,
    pub display: DisplayInfo,
    pub input: &'a InputState,
    /// Status lines under the title (platform name, files, audio, boost, frame rate).
    pub status: &'a [String],
}

const WHITE: [f32; 4] = [0.92, 0.94, 1.0, 1.0];
const GREY: [f32; 4] = [0.55, 0.6, 0.75, 1.0];
const DIM: [f32; 4] = [0.3, 0.33, 0.45, 1.0];
const PANEL: [f32; 4] = [0.06, 0.08, 0.16, 1.0];
const KEY_OFF: [f32; 4] = [0.12, 0.15, 0.27, 1.0];
const KEY_ON: [f32; 4] = [0.95, 0.7, 0.15, 1.0];
const KEY_PRESSED: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub struct Scene;

impl Scene {
    /// Upper bound on what [`Scene::build`] emits; the debug draw is created with this capacity.
    pub const MAX_VERTICES: usize = 49_152;

    fn to_ndc(x: f32, y: f32) -> [f32; 2] {
        [x / W * 2.0 - 1.0, y / H * 2.0 - 1.0]
    }

    fn rect(out: &mut Vec<Vertex>, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) {
        let (a, b, c, d) = (Self::to_ndc(x0, y0), Self::to_ndc(x1, y0), Self::to_ndc(x1, y1), Self::to_ndc(x0, y1));
        for pos in [a, b, c, a, c, d] {
            out.push(Vertex { pos, color });
        }
    }

    /// Width in pixels of `text` drawn at `scale` pixels per font pixel (one blank column between glyphs).
    pub fn text_width(text: &str, scale: f32) -> f32 {
        let n = text.chars().count() as f32;
        if n == 0.0 { 0.0 } else { (n * (GLYPH_W as f32 + 1.0) - 1.0) * scale }
    }

    /// Draws `text` with its top-left at (x, y). `style(index)` gives each glyph a y offset and a colour. Characters
    /// the font lacks are skipped (they still take up space).
    fn text_styled(
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
                    Self::rect(out, x0, y0, x0 + len as f32 * scale, y0 + scale, color);
                }
            }
        }
    }

    fn text(out: &mut Vec<Vertex>, text: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
        Self::text_styled(out, text, x, y, scale, |_| (0.0, color));
    }

    fn stick(out: &mut Vec<Vertex>, x: f32, y: f32, size: f32, label: &str, s: Stick, live: bool) {
        Self::rect(out, x, y, x + size, y + size, KEY_OFF);
        let c = size * 0.5;
        Self::rect(out, x + c - 1.0, y + 4.0, x + c + 1.0, y + size - 4.0, PANEL);
        Self::rect(out, x + 4.0, y + c - 1.0, x + size - 4.0, y + c + 1.0, PANEL);
        let travel = c - 8.0;
        let (dx, dy) = (x + c + s.x * travel, y + c - s.y * travel);
        Self::rect(out, dx - 6.0, dy - 6.0, dx + 6.0, dy + 6.0, if live { KEY_ON } else { DIM });
        Self::text(out, label, x + 4.0, y + 4.0, 2.0, GREY);
    }

    fn panel(out: &mut Vec<Vertex>, x: f32, y: f32, player: usize, pad: &PadState) {
        let (pw, ph) = (600.0, 262.0);
        Self::rect(out, x, y, x + pw, y + ph, PANEL);
        let live = pad.connected();
        let title = if live {
            format!("P{} {}", player + 1, pad.kind.name())
        } else {
            format!("P{} NOT CONNECTED", player + 1)
        };
        Self::text(out, &title, x + 12.0, y + 12.0, 3.0, if live { WHITE } else { DIM });

        // Buttons: two rows of nine, lit while held, white on the frame they go down.
        for (i, name) in Buttons::NAMES.iter().enumerate() {
            let bit = Buttons(1 << i);
            let (bx, by) = (x + 12.0 + (i % 9) as f32 * 64.0, y + 46.0 + (i / 9) as f32 * 36.0);
            let fill = if pad.pressed.any(bit) {
                KEY_PRESSED
            } else if pad.held.any(bit) {
                KEY_ON
            } else {
                KEY_OFF
            };
            Self::rect(out, bx, by, bx + 60.0, by + 32.0, fill);
            let lw = Self::text_width(name, 2.0);
            let ink = if fill == KEY_OFF { if live { GREY } else { DIM } } else { PANEL };
            Self::text(out, name, bx + (60.0 - lw) * 0.5, by + 9.0, 2.0, ink);
        }

        // Sticks after the deadzone, with their values.
        let sy = y + 126.0;
        Self::stick(out, x + 12.0, sy, 124.0, "L", pad.left, live);
        Self::stick(out, x + 148.0, sy, 124.0, "R", pad.right, live);
        let ink = if live { GREY } else { DIM };
        let lines = [
            format!("LX {:+.2} LY {:+.2}", pad.left.x, pad.left.y),
            format!("RX {:+.2} RY {:+.2}", pad.right.x, pad.right.y),
            format!("HELD {:05X}", pad.held.0),
        ];
        for (i, line) in lines.iter().enumerate() {
            Self::text(out, line, x + 290.0, sy + 8.0 + i as f32 * 26.0, 2.0, ink);
        }
    }

    /// Appends the frame to `out` and returns the clear colour.
    pub fn build(view: &View<'_>, out: &mut Vec<Vertex>) -> [f32; 4] {
        let t = view.t;
        let (s, _) = sin_cos(t * 0.6);
        let clear = [0.03, 0.05 + 0.02 * s, 0.12 + 0.03 * s, 1.0];

        // Title: "SlyNX", each letter bobbing on its own phase, with a drop shadow.
        let title = "SlyNX";
        let scale = 9.0;
        let (tx, ty) = (40.0, 26.0);
        let bob = |i: usize| sin_cos(t * 3.0 + i as f32 * 0.7).0 * 5.0;
        Self::text_styled(out, title, tx + 5.0, ty + 5.0, scale, |i| (bob(i), [0.0, 0.0, 0.0, 1.0]));
        Self::text_styled(out, title, tx, ty, scale, |i| {
            let [r, g, b] = hue(t * 0.1 + i as f32 * 0.12);
            (bob(i), [0.55 + 0.45 * r, 0.55 + 0.45 * g, 0.55 + 0.45 * b, 1.0])
        });

        // A spinning triangle with rainbow corners: shows at a glance that frames keep coming.
        let (cx, cy, radius) = (1200.0, 72.0, 44.0);
        for k in 0..3 {
            let a = t * 1.2 + k as f32 * TAU / 3.0;
            let (sa, ca) = sin_cos(a);
            let [r, g, b] = hue(t * 0.2 + k as f32 / 3.0);
            out.push(Vertex { pos: Self::to_ndc(cx + ca * radius, cy + sa * radius), color: [r, g, b, 1.0] });
        }

        // Status lines.
        let d = view.display;
        let mode = format!("{} {}X{}", if d.docked { "DOCKED" } else { "HANDHELD" }, d.width, d.height);
        Self::text(out, &mode, 320.0, 24.0, 3.0, WHITE);
        for (i, line) in view.status.iter().take(3).enumerate() {
            Self::text(out, line, 320.0, 52.0 + i as f32 * 24.0, 2.0, GREY);
        }

        // One panel per player.
        for (p, pad) in view.input.players.iter().enumerate() {
            let x = 27.0 + (p % 2) as f32 * 626.0;
            let y = 140.0 + (p / 2) as f32 * 278.0;
            Self::panel(out, x, y, p, pad);
        }

        let footer = "A B X Y PLAY A TONE   ZR CPU BOOST   + EXIT";
        Self::text(out, footer, (W - Self::text_width(footer, 2.0)) * 0.5, 700.0, 2.0, GREY);

        debug_assert!(out.len() <= Self::MAX_VERTICES);
        clear
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use platform_api::{ControllerKind, Deadzone, MAX_PLAYERS, RawPad};

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
    fn hue_wraps() {
        assert_eq!(hue(0.0), [1.0, 0.0, 0.0]);
        assert_eq!(hue(1.0), hue(0.0));
        let g = hue(1.0 / 3.0);
        assert!((g[1] - 1.0).abs() < 1e-5 && g[0] < 1e-5);
    }

    fn worst_case_input() -> InputState {
        // Every player connected with every button held and sticks pushed: the most vertices.
        let raw = [RawPad {
            kind: ControllerKind::JoyConPair,
            buttons: Buttons::ALL,
            left: Stick { x: -1.0, y: 1.0 },
            right: Stick { x: 0.7, y: -0.7 },
        }; MAX_PLAYERS];
        let mut input = InputState::default();
        input.update(&raw, Deadzone::default());
        input
    }

    #[test]
    fn frame_fits_budget_and_stays_on_screen() {
        let status = vec![
            "PC  AUDIO 10MS  BOOST OFF  60 FPS".to_string(),
            "ROMFS HELLO FROM ROMFS 0123456789".to_string(),
            "SAVE RUN 12345  IO CORE 1".to_string(),
        ];
        let mut out = Vec::new();
        for input in [InputState::default(), worst_case_input()] {
            for i in 0..240 {
                out.clear();
                let view = View {
                    t: i as f32 / 60.0,
                    display: DisplayInfo { width: 1920, height: 1080, docked: true },
                    input: &input,
                    status: &status,
                };
                let clear = Scene::build(&view, &mut out);
                assert!(out.len() <= Scene::MAX_VERTICES, "{} vertices", out.len());
                assert_eq!(out.len() % 3, 0, "whole triangles only");
                assert!(clear.iter().all(|c| (0.0..=1.0).contains(c)));
                for v in &out {
                    assert!(v.pos.iter().all(|p| p.is_finite() && (-1.0..=1.0).contains(p)), "{v:?}");
                    assert!(v.color.iter().all(|c| (0.0..=1.0).contains(c)), "{v:?}");
                }
            }
        }
    }
}
