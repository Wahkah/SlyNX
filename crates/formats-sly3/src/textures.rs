//! Material tables of a group (textures.md 2): palettes and bitmaps twice (copy 1, then copy 0), animation records
//! and shaders; decoding with the colour grading (textures.md 3, 5).

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::reader::Reader;
use formats::textures::{RgbaImage, csm1_index};

use crate::filter::Filter;

/// Storage kinds (textures.md 2.1).
pub const STORAGE_TEXTURE_DATA: u8 = 1;
/// Pixel formats.
pub const FORMAT_8BIT: u8 = 0x13;
pub const FORMAT_4BIT: u8 = 0x14;

/// Default colour-grading numbers (textures.md 5): lightness, contrast, saturation.
pub const GRADING: Grading = Grading { lightness: 1.5, contrast: 1.2, saturation: 1.4 };

/// Where an item's bytes are: inline (absolute offset in the world) or in the group's texture-data block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    Inline(u32),
    TextureData(u32),
}

/// A palette header and where its colours are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub id: i32,
    pub storage: u8,
    pub filter: Filter,
    pub flags: u32,
    pub data: Data,
    /// Offset of the 16-byte header.
    pub offset: u32,
}

impl Palette {
    /// Number of colours (flags bits 7..15).
    pub fn colors(&self) -> u16 {
        ((self.flags >> 7) & 0x1FF) as u16
    }

    /// Whether the colour grading applies (flags bit 0).
    pub fn graded(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// A bitmap header and where its texels are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub id: i32,
    pub storage: u8,
    pub filter: Filter,
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub size: u32,
    pub data: Data,
    /// Offset of the 24-byte header.
    pub offset: u32,
}

/// A shader: its header, animation index and texture record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shader {
    pub id: i32,
    pub kind: u8,
    pub flags: u8,
    pub color: [u8; 4],
    pub params: [u8; 2],
    pub pass: u8,
    /// Index into the animation records, −1 none.
    pub animation: i8,
    /// Bitmap index per frame (into copy 1).
    pub frames: Vec<u16>,
    /// Palette indices (`0xFFFF` = none), `palettes_per_frame` per frame (into copy 1).
    pub palettes: Vec<u16>,
    /// Offset of the 24-byte header.
    pub offset: u32,
}

impl Shader {
    /// Palettes per frame by kind (textures.md 4): 2 for kind 1; kind 7: 2 with flag 0x10, else 3; 1 otherwise.
    pub fn palettes_per_frame(&self) -> usize {
        match self.kind {
            1 => 2,
            7 => {
                if self.flags & 0x10 != 0 {
                    2
                } else {
                    3
                }
            }
            _ => 1,
        }
    }

    /// Bitmap and palette slot `slot` of frame `frame`.
    pub fn frame(&self, frame: usize, slot: usize) -> Option<(u16, Option<u16>)> {
        let b = *self.frames.get(frame)?;
        let k = self.palettes_per_frame();
        let p = self.palettes.get(k * frame + slot).copied().filter(|&p| p != 0xFFFF);
        Some((b, p))
    }
}

/// One copy of the palette and bitmap tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableCopy {
    pub palettes: Vec<Palette>,
    pub bitmaps: Vec<Bitmap>,
}

/// The material tables of one group.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterialTables {
    /// Copy 1 (the one the game uses) and copy 0.
    pub copies: [TableCopy; 2],
    /// Animation records: (class index, offset of the 36 settings bytes); empty slots are `(0xFFFF, 0)`.
    pub animations: Vec<(u16, u32)>,
    pub shaders: Vec<Shader>,
}

impl MaterialTables {
    /// Copy 1's palettes.
    pub fn palettes(&self) -> &[Palette] {
        &self.copies[0].palettes
    }

    /// Copy 1's bitmaps.
    pub fn bitmaps(&self) -> &[Bitmap] {
        &self.copies[0].bitmaps
    }

    pub fn read(r: &mut Reader<'_>) -> Result<MaterialTables> {
        let mut t = MaterialTables::default();
        for copy in &mut t.copies {
            copy.palettes = sized_table(r, palette)?;
            copy.bitmaps = sized_table(r, bitmap)?;
        }
        let n = r.u8()?;
        for _ in 0..n {
            let class = r.u16()?;
            if class == 0xFFFF {
                t.animations.push((class, 0));
                continue;
            }
            let at = r.pos() as u32;
            r.skip(36)?;
            t.animations.push((class, at));
        }
        let n = r.u16()?;
        for _ in 0..n {
            let offset = r.pos() as u32;
            let id = r.i32()?;
            let kind = r.u8()?;
            let flags = r.u8()?;
            r.skip(2)?;
            let color = r.array::<4>()?;
            r.skip(8)?;
            let params = r.array::<2>()?;
            let pass = r.u8()?;
            let records = r.u8()?;
            let animation = r.i8()?;
            let mut frames = Vec::new();
            let mut palettes = Vec::new();
            for _ in 0..records {
                r.skip(4 + 2)?;
                let f = r.u8()?;
                let p = r.u8()?;
                for _ in 0..f {
                    frames.push(r.u16()?);
                }
                for _ in 0..p {
                    palettes.push(r.u16()?);
                }
            }
            t.shaders.push(Shader { id, kind, flags, color, params, pass, animation, frames, palettes, offset });
        }
        Ok(t)
    }
}

/// A table with an `s32` byte size and a `u16` count; it must end exactly at its size.
fn sized_table<T>(r: &mut Reader<'_>, item: fn(&mut Reader<'_>) -> Result<T>) -> Result<Vec<T>> {
    let size = r.i32()?;
    if size < 2 {
        return Err(r.corrupt("material table size"));
    }
    let start = r.pos();
    let n = r.u16()?;
    let items = (0..n).map(|_| item(r)).collect::<Result<Vec<_>>>()?;
    if r.pos() - start != size as usize {
        return Err(Error::Corrupt { offset: r.pos(), what: "material table does not end at its size" });
    }
    Ok(items)
}

fn palette(r: &mut Reader<'_>) -> Result<Palette> {
    let offset = r.pos() as u32;
    let id = r.i32()?;
    let storage = r.u8()?;
    r.skip(1)?;
    let f = r.i8()?;
    let c = r.i8()?;
    let flags = r.u32()?;
    let at = r.u32()?;
    if f < 0 || c < 0 {
        return Err(r.corrupt("negative filter or condition count"));
    }
    let filter = Filter::read(r, f as usize, c as usize)?;
    let colors = ((flags >> 7) & 0x1FF) as usize;
    let data = if storage == STORAGE_TEXTURE_DATA {
        Data::TextureData(at)
    } else {
        let here = r.pos() as u32;
        r.skip(colors * 4)?;
        Data::Inline(here)
    };
    Ok(Palette { id, storage, filter, flags, data, offset })
}

fn bitmap(r: &mut Reader<'_>) -> Result<Bitmap> {
    let offset = r.pos() as u32;
    let id = r.i32()?;
    let storage = r.u8()?;
    r.skip(1)?;
    let f = r.i8()?;
    let c = r.i8()?;
    let width = r.u16()?;
    let height = r.u16()?;
    let format = r.u8()?;
    r.skip(3)?;
    let size = r.u32()?;
    let at = r.u32()?;
    if f < 0 || c < 0 {
        return Err(r.corrupt("negative filter or condition count"));
    }
    let filter = Filter::read(r, f as usize, c as usize)?;
    let data = if storage == STORAGE_TEXTURE_DATA {
        Data::TextureData(at)
    } else {
        let here = r.pos() as u32;
        r.skip(size as usize)?;
        Data::Inline(here)
    };
    Ok(Bitmap { id, storage, filter, width, height, format, size, data, offset })
}

/// Resolves an item's bytes: `world` is the whole decompressed world, `texture_data` the group's block.
pub fn item_bytes<'a>(data: Data, len: usize, world: &'a [u8], texture_data: &'a [u8]) -> Result<&'a [u8]> {
    let (src, off) = match data {
        Data::Inline(o) => (world, o as usize),
        Data::TextureData(o) => (texture_data, o as usize),
    };
    src.get(off..off + len).ok_or(Error::Corrupt { offset: off, what: "texture item outside its data" })
}

/// The colour-grading numbers (textures.md 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grading {
    pub lightness: f32,
    pub contrast: f32,
    pub saturation: f32,
}

fn hue_to_rgb(m1: f32, m2: f32, h: f32) -> f32 {
    let h = if h < 0.0 {
        h + 1.0
    } else if h > 1.0 {
        h - 1.0
    } else {
        h
    };
    if h < 1.0 / 6.0 {
        m1 + (m2 - m1) * h * 6.0
    } else if h < 0.5 {
        m2
    } else if h < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - h) * 6.0
    } else {
        m1
    }
}

impl Grading {
    /// Grades one colour (R, G, B 0..255; alpha kept): to HLS, `L' = clamp((L·a − a/4)·b + a/4)`,
    /// `S' = clamp(S·c)`, back to RGB, `× 255 + 0.5`.
    pub fn apply(&self, c: [u8; 4]) -> [u8; 4] {
        let (r, g, b) = (c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let (h, s) = if max == min {
            (0.0, 0.0)
        } else {
            let d = max - min;
            let s = if l <= 0.5 { d / (max + min) } else { d / (2.0 - max - min) };
            let h = if r == max {
                (g - b) / d
            } else if g == max {
                2.0 + (b - r) / d
            } else {
                4.0 + (r - g) / d
            };
            let h = h / 6.0;
            (if h < 0.0 { h + 1.0 } else { h }, s)
        };
        let a = self.lightness;
        let l = ((l * a - a / 4.0) * self.contrast + a / 4.0).clamp(0.0, 1.0);
        let s = (s * self.saturation).clamp(0.0, 1.0);
        let (r, g, b) = if s == 0.0 {
            (l, l, l)
        } else {
            let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s };
            let m1 = 2.0 * l - m2;
            (hue_to_rgb(m1, m2, h + 1.0 / 3.0), hue_to_rgb(m1, m2, h), hue_to_rgb(m1, m2, h - 1.0 / 3.0))
        };
        let q = |v: f32| (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        [q(r), q(g), q(b), c[3]]
    }
}

/// Palette alpha (0..0x80) to 0..255: `min(255, a × 255 / 128)`, rounded down (textures.md 3).
pub fn alpha(a: u8) -> u8 {
    (a as u32 * 255 / 128).min(255) as u8
}

/// Decodes a bitmap with a palette to RGBA8, rows in storage order. With `grading`, a palette whose flag bit 0 is
/// set has its colours graded first (textures.md 5).
pub fn decode(
    bitmap: &Bitmap,
    palette: &Palette,
    world: &[u8],
    texture_data: &[u8],
    grading: Option<&Grading>,
) -> Result<RgbaImage> {
    let (w, h) = (bitmap.width as usize, bitmap.height as usize);
    let bits = match bitmap.format {
        FORMAT_4BIT => 4,
        FORMAT_8BIT => 8,
        _ => return Err(Error::Corrupt { offset: bitmap.offset as usize, what: "unknown pixel format" }),
    };
    let texels = item_bytes(bitmap.data, w * h * bits / 8, world, texture_data)?;
    let n = palette.colors() as usize;
    let raw = item_bytes(palette.data, n * 4, world, texture_data)?;
    let mut lut = Vec::with_capacity(n);
    for c in raw.as_chunks::<4>().0 {
        let c = match grading {
            Some(g) if palette.graded() => g.apply(*c),
            _ => *c,
        };
        lut.push([c[0], c[1], c[2], alpha(c[3])]);
    }
    let mut pixels = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let index = if bits == 4 {
            let b = texels[i / 2];
            if i % 2 == 0 { b & 0x0F } else { b >> 4 }
        } else {
            csm1_index(texels[i])
        } as usize;
        let c = lut.get(index).ok_or(Error::Corrupt { offset: 0, what: "texel index past the palette" })?;
        pixels.extend_from_slice(c);
    }
    Ok(RgbaImage { width: w as u32, height: h as u32, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grading_of_known_colours() {
        // textures.md 6, shader 2: raw 6a5331 grades to b6843a; shader 24: 807f76 to cdcdc7.
        assert_eq!(GRADING.apply([0x6a, 0x53, 0x31, 0x80]), [0xb6, 0x84, 0x3a, 0x80]);
        assert_eq!(GRADING.apply([0x80, 0x7f, 0x76, 0x80]), [0xcd, 0xcd, 0xc7, 0x80]);
        assert_eq!(GRADING.apply([0, 0, 0, 0]), [0, 0, 0, 0]);
    }

    #[test]
    fn palettes_per_frame() {
        let mut s = Shader {
            id: 0,
            kind: 7,
            flags: 0x10,
            color: [0; 4],
            params: [0; 2],
            pass: 4,
            animation: -1,
            frames: alloc::vec![1, 2],
            palettes: alloc::vec![1, 2, 3, 4],
            offset: 0,
        };
        assert_eq!(s.palettes_per_frame(), 2);
        assert_eq!(s.frame(1, 1), Some((2, Some(4))));
        s.flags = 0x20;
        assert_eq!(s.palettes_per_frame(), 3);
        s.kind = 1;
        assert_eq!(s.palettes_per_frame(), 2);
        s.kind = 0;
        assert_eq!(s.palettes_per_frame(), 1);
    }
}
