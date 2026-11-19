//! The game's fonts (textures.md 2.6, ui-and-menus.md 1.1) and one glyph-sheet texture holding them all.
//!
//! A font record names a bitmap and a palette, then holds 4 metric bytes (cell width, space width, line height and
//! the font's own number), a scale, and 8-byte glyph records: `u16` character code, then `s16` x, `s16` y and `s16`
//! width. (x, y) is the top-left corner of the glyph's cell, with y counted from the top of the upright picture;
//! every cell is `width` x line height. Font bitmaps are stored bottom row first (storage row 0 is the bottom row of
//! the upright picture); [`read_font`] turns the bitmap upright so the records apply directly. Fonts 0 to 3 are in
//! every world, a fifth in 13 worlds; font 3 has six pictures (one per part of the game), so it is always taken from
//! the world being played (ui-and-menus.md 1.1).

use alloc::vec;
use alloc::vec::Vec;

use formats::textures::RgbaImage;
use formats::world::World;

use crate::Error;

/// One glyph: where it is in its font's bitmap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Character code.
    pub code: u16,
    /// Left texel column of the cell in the font bitmap.
    pub x: i16,
    /// Top texel row of the cell, counted from the top of the upright picture.
    pub y: i16,
    /// Width in texels.
    pub w: i16,
}

/// The four metric bytes of a font record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontMetrics {
    /// Cell width (used for characters the font lacks).
    pub cell_width: u8,
    /// Advance of a space.
    pub space_width: u8,
    /// Line height, and the height of every glyph.
    pub line_height: u8,
    /// The font's number (0..4, its index in the table).
    pub id: u8,
}

/// One font, placed in the glyph sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// Metrics.
    pub metrics: FontMetrics,
    /// The record's scale (1.0, or 2.0 for the button pictures).
    pub scale: f32,
    /// Glyphs, sorted by code.
    pub glyphs: Vec<Glyph>,
    /// Size of the font bitmap.
    pub bitmap_size: (u32, u32),
    /// Where the bitmap's top-left texel is in the glyph sheet.
    pub sheet_origin: (u32, u32),
}

impl Font {
    /// The glyph for `code`.
    pub fn glyph(&self, code: u16) -> Option<&Glyph> {
        self.glyphs.binary_search_by_key(&code, |g| g.code).ok().map(|i| &self.glyphs[i])
    }

    /// Height of a glyph in texels.
    pub fn line_height(&self) -> f32 {
        self.metrics.line_height as f32
    }
}

/// The sheet texture: RGBA8, rows top to bottom (row 0 is v = 0), straight alpha.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphSheet {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// A font's source data before packing.
#[derive(Clone, Debug)]
pub struct FontSource {
    /// Metrics.
    pub metrics: FontMetrics,
    /// Scale.
    pub scale: f32,
    /// Glyphs (any order).
    pub glyphs: Vec<Glyph>,
    /// The decoded bitmap.
    pub image: RgbaImage,
}

/// All fonts and their sheet. Index `i` is font `i` of the world's table (ui-and-menus.md 1.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSet {
    /// Fonts by number.
    pub fonts: Vec<Font>,
    /// The sheet.
    pub sheet: GlyphSheet,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn les16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

/// Parses one 8-byte glyph record: `u16` code, `s16` x, y, width (textures.md 2.6).
pub fn parse_glyph(g: &[u8; 8]) -> Glyph {
    Glyph { code: le16(g, 0), x: les16(g, 2), y: les16(g, 4), w: les16(g, 6) }
}

/// Reads one font's glyph records and decodes its bitmap.
pub fn read_font(world: &World<'_>, index: usize) -> Result<FontSource, Error> {
    let t = &world.layout.materials;
    let f = t.fonts.get(index).ok_or(Error::Invalid("font index"))?;
    let bitmap = t.bitmaps.get(f.bitmap as usize).ok_or(Error::Invalid("font bitmap index"))?;
    let palette = if f.palette >= 0 { t.palettes.get(f.palette as usize) } else { None };
    let stored = world.payload().decode(bitmap, palette)?;
    let m = f.metrics;
    // Font bitmaps are stored bottom row first (textures.md 2.6): turn the picture upright, so sheet row 0 is the
    // top.
    let (w, h) = (stored.width as usize, stored.height as usize);
    let mut pixels = vec![0u8; w * h * 4];
    for row in 0..h {
        let src = (h - 1 - row) * w * 4;
        pixels[row * w * 4..(row + 1) * w * 4].copy_from_slice(&stored.pixels[src..src + w * 4]);
    }
    let image = RgbaImage { width: stored.width, height: stored.height, pixels };
    let n = f.glyph_count as usize;
    let raw = world.data.get(f.glyphs..f.glyphs + n * 8).ok_or(Error::Invalid("glyph records"))?;
    // The glyph records' rows count from the top of the upright picture.
    let glyphs = raw.as_chunks::<8>().0.iter().map(parse_glyph).collect();
    Ok(FontSource {
        metrics: FontMetrics { cell_width: m[0], space_width: m[1], line_height: m[2], id: m[3] },
        scale: f.scale,
        glyphs,
        image,
    })
}

impl FontSet {
    /// Reads every font of a world and packs them into one sheet.
    pub fn from_world(world: &World<'_>) -> Result<FontSet, Error> {
        let n = world.layout.materials.fonts.len();
        let sources = (0..n).map(|i| read_font(world, i)).collect::<Result<Vec<_>, _>>()?;
        Ok(FontSet::pack(sources))
    }

    /// Packs fonts into a glyph sheet (shelves of the tallest bitmaps first, power-of-two size, at least 1024 wide).
    pub fn pack(sources: Vec<FontSource>) -> FontSet {
        let max_w = sources.iter().map(|s| s.image.width).max().unwrap_or(1);
        let width = max_w.max(1024).next_power_of_two();
        let mut order: Vec<usize> = (0..sources.len()).collect();
        order.sort_by_key(|&i| (core::cmp::Reverse(sources[i].image.height), i));
        let mut origins = vec![(0u32, 0u32); sources.len()];
        // Shelf packing; inside a shelf, a bitmap may also stack under the previous one in the same column.
        let (mut shelf_y, mut shelf_h, mut x) = (0u32, 0u32, 0u32);
        let mut column: Option<(u32, u32, u32)> = None; // (x, next y, column width)
        for &i in &order {
            let (w, h) = (sources[i].image.width, sources[i].image.height);
            if let Some((cx, cy, cw)) = column
                && w <= cw
                && cy + h <= shelf_y + shelf_h
            {
                origins[i] = (cx, cy);
                column = Some((cx, cy + h, cw));
                continue;
            }
            if x + w > width {
                shelf_y += shelf_h;
                shelf_h = 0;
                x = 0;
            }
            if shelf_h == 0 {
                shelf_h = h;
            }
            origins[i] = (x, shelf_y);
            column = Some((x, shelf_y + h, w));
            x += w;
        }
        let used = order.iter().map(|&i| origins[i].1 + sources[i].image.height).max().unwrap_or(1);
        let height = used.max(1).next_power_of_two();
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut fonts = Vec::with_capacity(sources.len());
        for (s, &(ox, oy)) in sources.into_iter().zip(&origins) {
            let (w, h) = (s.image.width, s.image.height);
            for row in 0..h {
                let src = (row * w * 4) as usize;
                let dst = (((oy + row) * width + ox) * 4) as usize;
                pixels[dst..dst + (w * 4) as usize].copy_from_slice(&s.image.pixels[src..src + (w * 4) as usize]);
            }
            let mut glyphs = s.glyphs;
            glyphs.sort_by_key(|g| g.code);
            fonts.push(Font {
                metrics: s.metrics,
                scale: s.scale,
                glyphs,
                bitmap_size: (w, h),
                sheet_origin: (ox, oy),
            });
        }
        FontSet { fonts, sheet: GlyphSheet { width, height, pixels } }
    }

    /// Adds the fonts of another set that this one lacks (for example the hideout's font 4 to a world without a fifth
    /// font) and repacks. This set's own fonts are kept, font 3 (the part of the game's icons) in particular.
    pub fn merged(&self, other: &FontSet) -> FontSet {
        let mut sources = Vec::new();
        for (i, f) in self.fonts.iter().enumerate().chain(other.fonts.iter().enumerate().skip(self.fonts.len())) {
            let set = if i < self.fonts.len() { self } else { other };
            sources.push(set.source_of(f));
        }
        FontSet::pack(sources)
    }

    fn source_of(&self, f: &Font) -> FontSource {
        let (w, h) = f.bitmap_size;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            let src = (((f.sheet_origin.1 + row) * self.sheet.width + f.sheet_origin.0) * 4) as usize;
            pixels[(row * w * 4) as usize..((row + 1) * w * 4) as usize]
                .copy_from_slice(&self.sheet.pixels[src..src + (w * 4) as usize]);
        }
        FontSource {
            metrics: f.metrics,
            scale: f.scale,
            glyphs: f.glyphs.clone(),
            image: RgbaImage { width: w, height: h, pixels },
        }
    }

    /// Texture coordinates `[u0, v0, u1, v1]` of a glyph of font `font`.
    pub fn glyph_uv(&self, font: usize, g: &Glyph) -> [f32; 4] {
        let f = &self.fonts[font];
        let (aw, ah) = (self.sheet.width as f32, self.sheet.height as f32);
        let x0 = f.sheet_origin.0 as f32 + g.x as f32;
        let y0 = f.sheet_origin.1 as f32 + g.y as f32;
        [x0 / aw, y0 / ah, (x0 + g.w.max(0) as f32) / aw, (y0 + f.line_height()) / ah]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic font: glyphs for `codes`, each `w` wide, solid white boxes in a bitmap.
    pub(crate) fn synthetic(id: u8, line: u8, codes: &[u16], w: i16) -> FontSource {
        let bw = 256u32;
        let bh = 64u32;
        let mut pixels = vec![0u8; (bw * bh * 4) as usize];
        let mut glyphs = Vec::new();
        for (i, &c) in codes.iter().enumerate() {
            let x = 1 + (i as i16 % 8) * (w + 2);
            let y = 1 + (i as i16 / 8) * (line as i16 + 1);
            glyphs.push(Glyph { code: c, x, y, w });
            for yy in y as u32..(y as u32 + line as u32).min(bh) {
                for xx in x as u32..(x + w) as u32 {
                    let o = ((yy * bw + xx) * 4) as usize;
                    pixels[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        FontSource {
            metrics: FontMetrics { cell_width: w as u8 + 2, space_width: w as u8 / 2, line_height: line, id },
            scale: 1.0,
            glyphs,
            image: RgbaImage { width: bw, height: bh, pixels },
        }
    }

    /// Five fonts shaped like the game's (font 0 text, 1 buttons, 2 numerals, 3 icons, 4 pictures).
    pub(crate) fn test_set() -> FontSet {
        let text: Vec<u16> = (0x21..0x7F).collect();
        FontSet::pack(vec![
            synthetic(0, 20, &text, 10),
            synthetic(1, 16, &[b'X' as u16, b'T' as u16, b'a' as u16, b'+' as u16], 16),
            synthetic(2, 30, &(0x25..0x7Fu16).collect::<Vec<_>>(), 12),
            synthetic(3, 30, &[b'C' as u16, b'K' as u16], 30),
            synthetic(4, 30, &[b'a' as u16], 30),
        ])
    }

    #[test]
    fn glyph_records_are_signed() {
        // Code 0x2122, x 10, y 300, width 18.
        let g = parse_glyph(&[0x22, 0x21, 10, 0, 0x2C, 0x01, 18, 0]);
        assert_eq!(g, Glyph { code: 0x2122, x: 10, y: 300, w: 18 });
        assert_eq!(parse_glyph(&[0x41, 0, 0xFE, 0xFF, 0, 0, 1, 0]).x, -2);
    }

    #[test]
    fn packs_and_looks_up() {
        let s = test_set();
        assert_eq!(s.fonts.len(), 5);
        assert_eq!(s.sheet.width, 1024);
        assert!(s.sheet.height.is_power_of_two());
        // No two fonts overlap.
        for (i, a) in s.fonts.iter().enumerate() {
            for b in &s.fonts[i + 1..] {
                let sep = a.sheet_origin.0 + a.bitmap_size.0 <= b.sheet_origin.0
                    || b.sheet_origin.0 + b.bitmap_size.0 <= a.sheet_origin.0
                    || a.sheet_origin.1 + a.bitmap_size.1 <= b.sheet_origin.1
                    || b.sheet_origin.1 + b.bitmap_size.1 <= a.sheet_origin.1;
                assert!(sep);
            }
        }
        let g = *s.fonts[0].glyph(b'A' as u16).unwrap();
        let uv = s.glyph_uv(0, &g);
        assert!(uv[2] > uv[0] && uv[3] > uv[1]);
        // The glyph's texels are white in the glyph sheet.
        let f = &s.fonts[0];
        let px = ((f.sheet_origin.1 + g.y as u32) * s.sheet.width + f.sheet_origin.0 + g.x as u32) as usize * 4;
        assert_eq!(&s.sheet.pixels[px..px + 4], &[255, 255, 255, 255]);
        assert!(s.fonts[1].glyph(b'Z' as u16).is_none());
        // Merging keeps fonts and adds missing ones.
        let four = FontSet::pack(vec![synthetic(0, 20, &[65], 10)]);
        let m = four.merged(&s);
        assert_eq!(m.fonts.len(), 5);
        assert_eq!(m.fonts[0].glyphs.len(), 1);
        assert_eq!(m.fonts[4].glyphs.len(), 1);
    }
}
