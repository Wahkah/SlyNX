//! Sly 2's two fonts (user-interface.md 4.2): the text font and the button-icon font, item-kind-8 sets of screen
//! object 884. Each character is a shape in sixteenths of a pixel with the baseline at 0 and y down; glyphs are
//! drawn at (text size) / 51.2 [R], and the advances are in the same units as the coordinates divided by 16.
//!
//! The glyph shapes are rasterised once into one RGBA sheet (four-by-four supersampled coverage, the vertex colour
//! of each triangle), which the 2D layer samples as its glyph texture. Text is laid out with the game's escapes:
//! `&x` switches to the second colour and the icon font, `&.` back, `&2<c>&.` shows the button icon `c`, `\n`
//! breaks the line; long lines wrap at spaces inside a width.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use crate::draw::{DrawList, Rect, Texture};

/// The glyph size of text size 1 is 1/51.2 of the font's units [R].
pub const SIZE_UNIT: f32 = 51.2;
/// Sheet pixels per font unit.
pub const SHEET_SCALE: f32 = 0.75;

/// One glyph as the disc gives it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphShape {
    /// The character code (Latin-1).
    pub code: u32,
    /// The advance, in font units.
    pub advance: f32,
    /// Its triangles, in font units (pixels at text size 51.2), y down, with each triangle's RGBA (0..1).
    pub triangles: Vec<([[f32; 2]; 3], [f32; 4])>,
}

/// A glyph in the sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// The sheet area `[u0, v0, u1, v1]`.
    pub uv: [f32; 4],
    /// The area it covers relative to the pen at the baseline, in font units: x0, y0, x1, y1.
    pub bounds: [f32; 4],
    pub advance: f32,
}

/// A font in the sheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Font {
    pub glyphs: BTreeMap<u32, Glyph>,
    /// Highest point above the baseline and lowest below it (font units, positive).
    pub ascent: f32,
    pub descent: f32,
}

/// The glyph sheet with both fonts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSheet {
    pub width: u32,
    pub height: u32,
    /// RGBA8, row 0 at the top.
    pub pixels: Vec<u8>,
    /// The text font and the button-icon font.
    pub fonts: Vec<Font>,
}

/// `floor` and `ceil` without `std` (values well inside `i32`).
fn floor(v: f32) -> f32 {
    let t = v as i32 as f32;
    if t > v { t - 1.0 } else { t }
}

fn ceil(v: f32) -> f32 {
    let t = v as i32 as f32;
    if t < v { t + 1.0 } else { t }
}

fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

impl FontSheet {
    /// Rasterises the fonts' glyphs into one sheet.
    pub fn build(fonts: &[Vec<GlyphShape>]) -> FontSheet {
        // Cell size: the largest glyph box over every font, at the sheet scale, plus a pixel of margin each side.
        let mut cells: Vec<(usize, usize, [f32; 4])> = Vec::new();
        let (mut cw, mut ch) = (1usize, 1usize);
        for (f, glyphs) in fonts.iter().enumerate() {
            for (g, gl) in glyphs.iter().enumerate() {
                let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for (t, _) in &gl.triangles {
                    for p in t {
                        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
                    }
                }
                if b[0] > b[2] {
                    b = [0.0; 4];
                }
                cw = cw.max(ceil((b[2] - b[0]) * SHEET_SCALE) as usize + 2);
                ch = ch.max(ceil((b[3] - b[1]) * SHEET_SCALE) as usize + 2);
                cells.push((f, g, b));
            }
        }
        let cols = 16usize;
        let rows = cells.len().div_ceil(cols).max(1);
        let (width, height) = ((cols * cw) as u32, (rows * ch) as u32);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut out: Vec<Font> = fonts.iter().map(|_| Font::default()).collect();
        for (i, &(f, g, b)) in cells.iter().enumerate() {
            let (cx, cy) = ((i % cols) * cw, (i / cols) * ch);
            let gl = &fonts[f][g];
            // Sample (sx, sy) of the cell (4 × 4 per pixel) lies at font units b0 + (sx / 4 - 1 + 1/8) / scale. Each
            // triangle marks the samples inside it (the first to cover a sample gives its colour).
            let (sw, sh) = (cw * 4, ch * 4);
            let mut sample: Vec<Option<[f32; 4]>> = vec![None; sw * sh];
            let to_s = |u: f32, o: f32| ((u - o) * SHEET_SCALE + 1.0) * 4.0 - 0.5;
            for (t, c) in &gl.triangles {
                let xs = t.map(|p| to_s(p[0], b[0]));
                let ys = t.map(|p| to_s(p[1], b[1]));
                let x0 = floor(xs.iter().fold(f32::MAX, |a, &v| a.min(v))).max(0.0) as usize;
                let x1 = (ceil(xs.iter().fold(f32::MIN, |a, &v| a.max(v))).max(0.0) as usize).min(sw - 1);
                let y0 = floor(ys.iter().fold(f32::MAX, |a, &v| a.min(v))).max(0.0) as usize;
                let y1 = (ceil(ys.iter().fold(f32::MIN, |a, &v| a.max(v))).max(0.0) as usize).min(sh - 1);
                let q = [[xs[0], ys[0]], [xs[1], ys[1]], [xs[2], ys[2]]];
                for sy in y0..=y1 {
                    for sx in x0..=x1 {
                        let p = [sx as f32, sy as f32];
                        let (e0, e1, e2) = (edge(q[0], q[1], p), edge(q[1], q[2], p), edge(q[2], q[0], p));
                        let inside = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
                        if inside && sample[sy * sw + sx].is_none() {
                            sample[sy * sw + sx] = Some(*c);
                        }
                    }
                }
            }
            for py in 0..ch {
                for px in 0..cw {
                    let (mut n, mut col) = (0.0f32, [0.0f32; 4]);
                    for sy in 0..4 {
                        for sx in 0..4 {
                            if let Some(c) = sample[(py * 4 + sy) * sw + px * 4 + sx] {
                                n += 1.0;
                                for k in 0..4 {
                                    col[k] += c[k];
                                }
                            }
                        }
                    }
                    if n > 0.0 {
                        let at = (((cy + py) as u32 * width + (cx + px) as u32) * 4) as usize;
                        for k in 0..3 {
                            pixels[at + k] = ((col[k] / n).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        }
                        pixels[at + 3] = ((col[3] / 16.0).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
            let (w, h) = (width as f32, height as f32);
            let font = &mut out[f];
            font.ascent = font.ascent.max(-b[1]);
            font.descent = font.descent.max(b[3]);
            let pad = 1.0 / SHEET_SCALE;
            font.glyphs.insert(
                gl.code,
                Glyph {
                    uv: [cx as f32 / w, cy as f32 / h, (cx + cw) as f32 / w, (cy + ch) as f32 / h],
                    bounds: [
                        b[0] - pad,
                        b[1] - pad,
                        b[0] - pad + cw as f32 / SHEET_SCALE,
                        b[1] - pad + ch as f32 / SHEET_SCALE,
                    ],
                    advance: gl.advance,
                },
            );
        }
        FontSheet { width, height, pixels, fonts: out }
    }

    /// The height of a line at text `size` in canvas units.
    pub fn line_height(&self, size: f32) -> f32 {
        let f = self.fonts.first();
        f.map_or(size, |f| (f.ascent + f.descent) * size / SIZE_UNIT * 1.1)
    }

    /// Draws `text` (Latin-1 bytes with the game's escapes) with its baseline-top at `(x, y)` at text `size`:
    /// `colours` are the first and second colour. Lines wrap at spaces past `width` (none: no wrapping). Returns
    /// the size drawn (widest line, total height).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        list: &mut DrawList,
        text: &[u8],
        x: f32,
        y: f32,
        size: f32,
        colours: [[f32; 4]; 2],
        width: Option<f32>,
    ) -> (f32, f32) {
        let k = size / SIZE_UNIT;
        let line = self.line_height(size);
        let ascent = self.fonts.first().map_or(0.0, |f| f.ascent) * k;
        let (mut pen, mut base) = (x, y + ascent);
        let mut widest = 0.0f32;
        let mut second = false;
        let mut i = 0;
        let advance_of =
            |font: usize, c: u32| self.fonts.get(font).and_then(|f| f.glyphs.get(&c)).map_or(0.0, |g| g.advance);
        while i < text.len() {
            let c = text[i];
            if c == b'&' && i + 1 < text.len() {
                match text[i + 1] {
                    b'x' => {
                        second = true;
                        i += 2;
                        continue;
                    }
                    b'.' => {
                        second = false;
                        i += 2;
                        continue;
                    }
                    b'2' if i + 2 < text.len() => {
                        // A button icon: the next character in the icon font.
                        let icon = text[i + 2] as u32;
                        pen += self.glyph(list, 1, icon, [pen, base, k], colours[1]);
                        i += 3;
                        continue;
                    }
                    _ => {}
                }
            }
            if c == b'\n' {
                widest = widest.max(pen - x);
                pen = x;
                base += line;
                i += 1;
                continue;
            }
            // Wrap before a word that would pass the width.
            if c == b' '
                && let Some(w) = width
            {
                let word: f32 = text[i + 1..]
                    .iter()
                    .take_while(|&&b| b != b' ' && b != b'\n' && b != b'&')
                    .map(|&b| advance_of(0, b as u32) * k)
                    .sum();
                if pen + advance_of(0, b' ' as u32) * k + word - x > w {
                    widest = widest.max(pen - x);
                    pen = x;
                    base += line;
                    i += 1;
                    continue;
                }
            }
            let col = if second { colours[1] } else { colours[0] };
            pen += self.glyph(list, 0, c as u32, [pen, base, k], col);
            i += 1;
        }
        widest = widest.max(pen - x);
        (widest, base - y + self.fonts.first().map_or(0.0, |f| f.descent) * k)
    }

    /// The width `text` takes at `size` (one line, escapes skipped).
    pub fn measure(&self, text: &[u8], size: f32) -> f32 {
        let mut list = DrawList::default();
        self.draw(&mut list, text, 0.0, 0.0, size, [[1.0; 4]; 2], None).0
    }

    fn glyph(&self, list: &mut DrawList, font: usize, c: u32, at: [f32; 3], colour: [f32; 4]) -> f32 {
        let [pen, base, k] = at;
        let Some(g) = self.fonts.get(font).and_then(|f| f.glyphs.get(&c)) else { return 0.0 };
        let b = g.bounds;
        let r = Rect::new(pen + b[0] * k, base + b[1] * k, (b[2] - b[0]) * k, (b[3] - b[1]) * k);
        list.textured(r, Texture::Glyphs, g.uv, colour);
        g.advance * k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(code: u32) -> GlyphShape {
        // A 20 × 40 box standing on the baseline.
        let a = [0.0, -40.0];
        let b = [20.0, -40.0];
        let c = [20.0, 0.0];
        let d = [0.0, 0.0];
        GlyphShape { code, advance: 24.0, triangles: vec![([a, b, c], [1.0; 4]), ([a, c, d], [1.0; 4])] }
    }

    #[test]
    fn sheet_and_layout() {
        let s = FontSheet::build(&[vec![square(b'A' as u32), square(b' ' as u32)], vec![square(b'X' as u32)]]);
        assert_eq!(s.fonts.len(), 2);
        assert!(s.pixels.chunks(4).any(|p| p[3] == 255), "covered pixels");
        let g = s.fonts[0].glyphs[&(b'A' as u32)];
        assert!(g.uv[2] > g.uv[0]);
        // Two letters at size 51.2 advance 48 units; the icon escape draws from the second font.
        let mut list = DrawList::default();
        let (w, _) = s.draw(&mut list, b"AA&2X&.", 0.0, 0.0, SIZE_UNIT, [[1.0; 4]; 2], None);
        assert_eq!(list.quads.len(), 3);
        assert!((w - 72.0).abs() < 1e-3);
        // Wrapping at a width of 30: "A A" breaks after the first A.
        let mut list = DrawList::default();
        let (_, h) = s.draw(&mut list, b"A A", 0.0, 0.0, SIZE_UNIT, [[1.0; 4]; 2], Some(30.0));
        assert!(h > s.line_height(SIZE_UNIT));
    }
}
