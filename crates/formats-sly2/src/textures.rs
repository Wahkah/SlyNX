//! Material tables of a package (textures.md 2): palettes, bitmaps, the node list and shaders; and decoding a
//! bitmap with a palette to RGBA8 (textures.md 3, as Sly 1).

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::reader::Reader;
use formats::textures::{RgbaImage, convert_alpha, csm1_index};

/// Storage kinds.
pub const STORAGE_TEXTURE_DATA: u8 = 1;
pub const STORAGE_INLINE: u8 = 3;
/// Pixel formats.
pub const FORMAT_8BIT: u8 = 0x13;
pub const FORMAT_4BIT: u8 = 0x14;

/// Where an item's bytes are: inline (absolute offset in the world) or in the package's texture-data block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    Inline(u32),
    TextureData(u32),
}

/// A palette header (16 bytes) and where its colours are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub id: i32,
    pub storage: u8,
    pub conditions: Vec<i16>,
    pub colors: u16,
    pub data: Data,
}

/// A bitmap header (24 bytes) and where its texels are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub id: i32,
    pub storage: u8,
    pub conditions: Vec<i16>,
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub size: u32,
    pub data: Data,
}

/// A shader (24 bytes, node reference, one texture record).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shader {
    pub id: i32,
    pub kind: u8,
    pub flags: u8,
    pub color: [u8; 4],
    pub pass: u8,
    pub node: i16,
    /// Self-illumination (bytes 20 and 21; rendering.md 2.4: / 4096).
    pub self_illum: u16,
    /// Second colour (bytes 12 to 15; kind 3's constant colour, rendering.md 3.5).
    pub color2: [u8; 4],
    /// Bitmap index per frame.
    pub frames: Vec<u16>,
    /// Palette indices (`0xFFFF` = none), `palettes_per_frame` per frame.
    pub palettes: Vec<u16>,
}

impl Shader {
    /// Palettes per frame by kind (textures.md 4).
    pub fn palettes_per_frame(&self) -> usize {
        match self.kind {
            1 => 2,
            7 => {
                if self.flags & 0x20 != 0 {
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

/// The material tables of one package.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterialTables {
    pub palettes: Vec<Palette>,
    pub bitmaps: Vec<Bitmap>,
    /// Node list: (class index, offset of its 32 settings bytes).
    pub nodes: Vec<(u16, u32)>,
    pub shaders: Vec<Shader>,
}

fn conditions(r: &mut Reader<'_>, c: i8) -> Result<Vec<i16>> {
    if c < 0 {
        return Err(r.corrupt("negative condition count"));
    }
    (0..c).map(|_| r.i16()).collect()
}

impl MaterialTables {
    pub fn read(r: &mut Reader<'_>) -> Result<MaterialTables> {
        let mut t = MaterialTables::default();
        let n = r.u16()?;
        for _ in 0..n {
            let id = r.i32()?;
            let storage = r.u8()?;
            let c = r.i8()?;
            r.skip(2)?;
            let colors = r.u16()?;
            let _class = r.u16()?;
            let offset = r.u32()?;
            let conditions = conditions(r, c)?;
            let data = match storage {
                STORAGE_TEXTURE_DATA => Data::TextureData(offset),
                STORAGE_INLINE => {
                    let at = r.pos() as u32;
                    r.skip(colors as usize * 4)?;
                    Data::Inline(at)
                }
                _ => return Err(r.corrupt("unknown palette storage")),
            };
            t.palettes.push(Palette { id, storage, conditions, colors, data });
        }
        let n = r.u16()?;
        for _ in 0..n {
            let id = r.i32()?;
            let storage = r.u8()?;
            let c = r.i8()?;
            r.skip(2)?;
            let width = r.u16()?;
            let height = r.u16()?;
            let format = r.u8()?;
            let _buffer_width = r.u8()?;
            let _blocks = r.u16()?;
            let size = r.u32()?;
            let offset = r.u32()?;
            let conditions = conditions(r, c)?;
            let data = match storage {
                STORAGE_TEXTURE_DATA => Data::TextureData(offset),
                STORAGE_INLINE => {
                    let at = r.pos() as u32;
                    r.skip(size as usize)?;
                    Data::Inline(at)
                }
                _ => return Err(r.corrupt("unknown bitmap storage")),
            };
            t.bitmaps.push(Bitmap { id, storage, conditions, width, height, format, size, data });
        }
        let n = r.u16()?;
        for _ in 0..n {
            let class = r.u16()?;
            let at = r.pos() as u32;
            r.skip(32)?;
            t.nodes.push((class, at));
        }
        let n = r.u16()?;
        for _ in 0..n {
            let id = r.i32()?;
            let kind = r.u8()?;
            let flags = r.u8()?;
            r.skip(2)?;
            let color = r.array::<4>()?;
            let color2 = r.array::<4>()?;
            r.skip(4)?; // third colour
            let self_illum = r.u16()?;
            let pass = r.u8()?;
            let records = r.u8()?;
            let node = r.i16()?;
            let mut frames = Vec::new();
            let mut palettes = Vec::new();
            for _ in 0..records {
                let _id = r.i32()?;
                r.skip(2)?;
                let f = r.u8()?;
                let p = r.u8()?;
                for _ in 0..f {
                    frames.push(r.u16()?);
                }
                for _ in 0..p {
                    palettes.push(r.u16()?);
                }
            }
            t.shaders.push(Shader { id, kind, flags, color, pass, node, self_illum, color2, frames, palettes });
        }
        Ok(t)
    }

    /// Whether an item with these conditions is loaded for level row `row` (`None`: no level is current).
    pub fn loaded(conditions: &[i16], row: Option<i16>) -> bool {
        conditions.is_empty() || row.is_none_or(|r| conditions.contains(&r))
    }
}

/// Resolves an item's bytes: `world` is the whole decompressed world, `texture_data` the package's block.
pub fn item_bytes<'a>(data: Data, len: usize, world: &'a [u8], texture_data: &'a [u8]) -> Result<&'a [u8]> {
    let (src, off) = match data {
        Data::Inline(o) => (world, o as usize),
        Data::TextureData(o) => (texture_data, o as usize),
    };
    off.checked_add(len)
        .and_then(|end| src.get(off..end))
        .ok_or(Error::Corrupt { offset: off, what: "texture item outside its data" })
}

/// Decodes a bitmap with a palette to RGBA8, rows in storage order (textures.md 3).
pub fn decode(bitmap: &Bitmap, palette: &Palette, world: &[u8], texture_data: &[u8]) -> Result<RgbaImage> {
    let (w, h) = (bitmap.width as usize, bitmap.height as usize);
    let bits = match bitmap.format {
        FORMAT_4BIT => 4,
        FORMAT_8BIT => 8,
        _ => return Err(Error::Corrupt { offset: 0, what: "unknown pixel format" }),
    };
    // Rounded up: a 4-bit image of odd size ends in half a byte.
    let texels = item_bytes(bitmap.data, (w * h * bits).div_ceil(8), world, texture_data)?;
    let colors = item_bytes(palette.data, palette.colors as usize * 4, world, texture_data)?;
    let mut pixels = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let index = if bits == 4 {
            let b = texels[i / 2];
            if i % 2 == 0 { b & 0x0F } else { b >> 4 }
        } else {
            csm1_index(texels[i])
        } as usize;
        match colors.get(index * 4..index * 4 + 4) {
            Some(c) => pixels.extend_from_slice(&[c[0], c[1], c[2], convert_alpha(c[3])]),
            None => return Err(Error::Corrupt { offset: 0, what: "texel index past the palette" }),
        }
    }
    Ok(RgbaImage { width: w as u32, height: h as u32, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_per_frame() {
        let mut s = Shader {
            id: 0,
            kind: 1,
            flags: 0,
            color: [0; 4],
            pass: 3,
            node: -1,
            self_illum: 0,
            color2: [0; 4],
            frames: alloc::vec![4, 5],
            palettes: alloc::vec![1, 2, 3, 0xFFFF],
        };
        assert_eq!(s.palettes_per_frame(), 2);
        assert_eq!(s.frame(1, 0), Some((5, Some(3))));
        assert_eq!(s.frame(1, 1), Some((5, None)));
        assert_eq!(s.frame(2, 0), None);
        s.kind = 7;
        assert_eq!(s.palettes_per_frame(), 3);
        s.flags = 0x20;
        assert_eq!(s.palettes_per_frame(), 2);
        s.kind = 0;
        assert_eq!(s.palettes_per_frame(), 1);
    }

    #[test]
    fn conditions_rule() {
        assert!(MaterialTables::loaded(&[], Some(3)));
        assert!(MaterialTables::loaded(&[3, 5], Some(3)));
        assert!(!MaterialTables::loaded(&[4], Some(3)));
        assert!(MaterialTables::loaded(&[4], None));
    }

    #[test]
    fn decode_4_and_8_bit() {
        let pal = Palette { id: 0, storage: 1, conditions: alloc::vec![], colors: 2, data: Data::TextureData(0) };
        let mut td = alloc::vec![10, 20, 30, 0x80, 40, 50, 60, 0x40];
        td.extend_from_slice(&[0x10]); // 4-bit: index 0 then 1
        let bmp = Bitmap {
            id: 0,
            storage: 1,
            conditions: alloc::vec![],
            width: 2,
            height: 1,
            format: FORMAT_4BIT,
            size: 1,
            data: Data::TextureData(8),
        };
        let img = decode(&bmp, &pal, &[], &td).unwrap();
        assert_eq!(img.pixels, [10, 20, 30, 255, 40, 50, 60, 128]);
        let bmp8 = Bitmap { format: FORMAT_8BIT, size: 2, ..bmp };
        let td8 = [&td[..8], &[1, 0][..]].concat();
        assert_eq!(decode(&bmp8, &pal, &[], &td8).unwrap().pixels, [40, 50, 60, 128, 10, 20, 30, 255]);
    }

    #[test]
    fn odd_4_bit_images_end_in_half_a_byte() {
        // 3 x 1 at 4 bits is a byte and a half: the last texel is the low half of the second byte, and nothing is
        // read past it.
        let pal = Palette { id: 0, storage: 1, conditions: alloc::vec![], colors: 2, data: Data::TextureData(0) };
        let td = [10, 20, 30, 0x80, 40, 50, 60, 0x40, 0x10, 0x01];
        let bmp = Bitmap {
            id: 0,
            storage: 1,
            conditions: alloc::vec![],
            width: 3,
            height: 1,
            format: FORMAT_4BIT,
            size: 2,
            data: Data::TextureData(8),
        };
        let img = decode(&bmp, &pal, &[], &td).unwrap();
        assert_eq!(&img.pixels[8..12], &[40, 50, 60, 128]);
        // Cut short (the half byte missing): refused, not a panic.
        assert!(decode(&bmp, &pal, &[], &td[..9]).is_err());
        // An offset near the top of the address space does not overflow.
        assert!(item_bytes(Data::TextureData(u32::MAX), usize::MAX, &[], &td).is_err());
    }
}
