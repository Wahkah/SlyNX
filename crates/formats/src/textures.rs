//! Palettes, bitmaps, shaders and fonts, and decoding bitmaps to RGBA8 (textures.md).
//!
//! **Row order of decoded images:** rows are output in storage order, row 0 first. Storage row 0 is texture
//! coordinate v = 0 (as on the PS2), so upload the rows as they are and use the mesh UVs unchanged. Shown in an
//! ordinary image viewer (top row first), many images appear upside down; that is expected.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::reader::Reader;

/// Pixel format of a bitmap (GS format code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// `0x00` PSMCT32: R, G, B, A bytes per texel.
    Rgba32,
    /// `0x13` PSMT8: 8-bit palette index (CSM1 palette order).
    Indexed8,
    /// `0x14` PSMT4: 4-bit palette index, low nibble first.
    Indexed4,
}

impl PixelFormat {
    /// From the stored code.
    pub fn from_code(code: u8) -> Option<PixelFormat> {
        match code {
            0x00 => Some(PixelFormat::Rgba32),
            0x13 => Some(PixelFormat::Indexed8),
            0x14 => Some(PixelFormat::Indexed4),
            _ => None,
        }
    }

    /// Bits per texel.
    pub fn bits(self) -> usize {
        match self {
            PixelFormat::Rgba32 => 32,
            PixelFormat::Indexed8 => 8,
            PixelFormat::Indexed4 => 4,
        }
    }
}

/// A palette entry of the palette table (textures.md 2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Zone mask.
    pub zone_mask: u32,
    /// Colour count (1..256).
    pub count: u16,
    /// GS upload size class (not needed to decode).
    pub size_class: u16,
    /// Offset of the colours, relative to the texture base T.
    pub offset: u32,
}

/// A bitmap entry of the bitmap table (textures.md 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bitmap {
    /// Width in texels.
    pub width: u16,
    /// Height in texels.
    pub height: u16,
    /// Zone mask.
    pub zone_mask: u32,
    /// The stored format code.
    pub format_code: u8,
    /// GS buffer width in 64-texel units (not needed to decode).
    pub gs_width: u8,
    /// Size in 256-byte GS blocks (not needed to decode).
    pub gs_blocks: u16,
    /// Size of the texel data.
    pub size: u32,
    /// Offset of the texel data, relative to T.
    pub offset: u32,
}

impl Bitmap {
    /// The pixel format, if the code is known.
    pub fn format(&self) -> Option<PixelFormat> {
        PixelFormat::from_code(self.format_code)
    }
}

/// A texture-animation record (textures.md 2.4); also used inside meshes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimRecord {
    /// Kind: 0 or 0xFFFF none; 1..8 an animation.
    pub kind: u16,
    /// Object id (0 when there is no animation).
    pub oid: i16,
    /// Instancing flag.
    pub instancing: u16,
    /// Kind-specific parameters (zero when there is no animation).
    pub params: [u8; 24],
}

impl AnimRecord {
    /// Reads a record; kinds other than 0, 0xFFFF and 1..8 are corrupt.
    pub fn read(r: &mut Reader<'_>) -> Result<AnimRecord> {
        let at = r.pos();
        let kind = r.u16()?;
        match kind {
            0 | 0xFFFF => Ok(AnimRecord { kind, oid: 0, instancing: 0, params: [0; 24] }),
            1..=8 => Ok(AnimRecord { kind, oid: r.i16()?, instancing: r.u16()?, params: r.array()? }),
            _ => Err(Error::Corrupt { offset: at, what: "unknown texture-animation kind" }),
        }
    }

    /// Whether this record describes an animation.
    pub fn is_animated(&self) -> bool {
        (1..=8).contains(&self.kind)
    }
}

/// A texture record of a shader (textures.md 2.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureRef {
    /// Object id.
    pub oid: i16,
    /// Texture flags (not fully understood).
    pub flags: i16,
    /// Bitmap index per animation frame.
    pub bitmaps: Vec<u16>,
    /// Palette indices (three per frame for kind-0 shaders, one otherwise; 0xFFFF for none).
    pub palettes: Vec<u16>,
}

impl TextureRef {
    fn read(r: &mut Reader<'_>) -> Result<TextureRef> {
        let oid = r.i16()?;
        let flags = r.i16()?;
        let f = r.u8()?;
        let p = r.u8()?;
        let bitmaps = (0..f).map(|_| r.u16()).collect::<Result<_>>()?;
        let palettes = (0..p).map(|_| r.u16()).collect::<Result<_>>()?;
        Ok(TextureRef { oid, flags, bitmaps, palettes })
    }
}

/// Shader kinds (textures.md 4), named by their kind value.
pub mod shader_kind {
    /// Kind 0: three palettes per frame over the same bitmap; most world geometry.
    pub const KIND_0: u8 = 0;
    /// Kind 1: one palette per frame; vertex colours common.
    pub const KIND_1: u8 = 1;
    /// Kind 2: uses the 64 x 64 32-bit bitmaps.
    pub const KIND_2: u8 = 2;
    /// Kind 5: bitmap content filled at run time (the shaders point at placeholder bitmaps).
    pub const KIND_5: u8 = 5;
}

/// Palette slot used as the base image of a kind-0 shader for an unlit render (textures.md 4).
pub const KIND_0_BASE_SLOT: usize = 1;

/// A shader record (textures.md 2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shader {
    /// Kind (section 4).
    pub kind: u8,
    /// Blend flags.
    pub blend: u8,
    /// Object id.
    pub oid: i16,
    /// Material colour R, G, B, A.
    pub color: [u8; 4],
    /// Second colour.
    pub color2: [u8; 4],
    /// Zone mask.
    pub zone_mask: u32,
    /// Alternate object id.
    pub alt_oid: u16,
    /// Render pass.
    pub pass: u8,
    /// Texture animation.
    pub animation: AnimRecord,
    /// Texture records (always one on the disc).
    pub textures: Vec<TextureRef>,
}

impl Shader {
    fn read(r: &mut Reader<'_>) -> Result<Shader> {
        let kind = r.u8()?;
        let blend = r.u8()?;
        let oid = r.i16()?;
        let color = r.array()?;
        let color2 = r.array()?;
        let zone_mask = r.u32()?;
        let alt_oid = r.u16()?;
        let pass = r.u8()?;
        let count = r.u8()?;
        let animation = AnimRecord::read(r)?;
        let textures = (0..count).map(|_| TextureRef::read(r)).collect::<Result<_>>()?;
        Ok(Shader { kind, blend, oid, color, color2, zone_mask, alt_oid, pass, animation, textures })
    }

    /// Palettes per frame: 3 for kind 0, 1 otherwise.
    pub fn palettes_per_frame(&self) -> usize {
        if self.kind == shader_kind::KIND_0 { 3 } else { 1 }
    }

    /// The (bitmap index, palette index) pair of animation frame `frame`, palette slot `slot` (0..3 for kind 0,
    /// 0 otherwise), from the first texture record. The palette is `None` when stored as 0xFFFF.
    pub fn frame_texture(&self, frame: usize, slot: usize) -> Option<(u16, Option<u16>)> {
        let t = self.textures.first()?;
        let bitmap = *t.bitmaps.get(frame)?;
        let ppf = self.palettes_per_frame();
        if slot >= ppf {
            return None;
        }
        let palette = match t.palettes.get(frame * ppf + slot).copied() {
            Some(0xFFFF) | None => None,
            Some(p) => Some(p),
        };
        Some((bitmap, palette))
    }

    /// The (bitmap, palette) of frame 0 for an unlit render: slot 1 for kind-0 shaders, the only slot otherwise.
    pub fn base_texture(&self) -> Option<(u16, Option<u16>)> {
        let slot = if self.kind == shader_kind::KIND_0 { KIND_0_BASE_SLOT } else { 0 };
        self.frame_texture(0, slot)
    }
}

/// A font record (textures.md 2.6); glyphs are kept as raw offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    /// Bitmap index.
    pub bitmap: u16,
    /// Palette index.
    pub palette: i16,
    /// Metrics bytes.
    pub metrics: [u8; 4],
    /// Scale.
    pub scale: f32,
    /// Glyph count.
    pub glyph_count: u32,
    /// File offset of the glyph records (8 bytes each).
    pub glyphs: usize,
}

/// The four material tables of a world (world-file.md 4.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialTables {
    /// Palette table.
    pub palettes: Vec<Palette>,
    /// Bitmap table.
    pub bitmaps: Vec<Bitmap>,
    /// The second shader-table count (shaders that carry an animation record).
    pub animated_shader_count: u16,
    /// Shader table.
    pub shaders: Vec<Shader>,
    /// Font table.
    pub fonts: Vec<Font>,
}

impl MaterialTables {
    /// Reads the tables at the reader's position.
    pub fn read(r: &mut Reader<'_>) -> Result<MaterialTables> {
        let n = r.u16()?;
        let mut palettes = Vec::with_capacity(n as usize);
        for _ in 0..n {
            palettes.push(Palette { zone_mask: r.u32()?, count: r.u16()?, size_class: r.u16()?, offset: r.u32()? });
        }
        let n = r.u16()?;
        let mut bitmaps = Vec::with_capacity(n as usize);
        for _ in 0..n {
            bitmaps.push(Bitmap {
                width: r.u16()?,
                height: r.u16()?,
                zone_mask: r.u32()?,
                format_code: r.u8()?,
                gs_width: r.u8()?,
                gs_blocks: r.u16()?,
                size: r.u32()?,
                offset: r.u32()?,
            });
        }
        let n = r.u16()?;
        let animated_shader_count = r.u16()?;
        let shaders = (0..n).map(|_| Shader::read(r)).collect::<Result<_>>()?;
        let n = r.u16()?;
        let mut fonts = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let bitmap = r.u16()?;
            let palette = r.i16()?;
            let metrics = r.array()?;
            let scale = r.f32()?;
            let glyph_count = r.u32()?;
            let glyphs = r.pos();
            r.skip_n(glyph_count as usize, 8)?;
            fonts.push(Font { bitmap, palette, metrics, scale, glyph_count, glyphs });
        }
        Ok(MaterialTables { palettes, bitmaps, animated_shader_count, shaders, fonts })
    }

    /// The largest `offset + size` over the bitmaps (equals header word 0 on every world).
    pub fn texel_extent(&self) -> u64 {
        self.bitmaps.iter().map(|b| b.offset as u64 + b.size as u64).max().unwrap_or(0)
    }

    /// The bitmap and palette a shader uses for an unlit render (frame 0, base slot). `None` for a shader without a
    /// texture or with dangling indices (textures.md 2.5).
    pub fn shader_texture(&self, shader: usize) -> Option<(usize, Option<usize>)> {
        let (b, p) = self.shaders.get(shader)?.base_texture()?;
        let b = b as usize;
        if b >= self.bitmaps.len() {
            return None;
        }
        match p {
            Some(p) if (p as usize) < self.palettes.len() => Some((b, Some(p as usize))),
            Some(_) => None,
            None => Some((b, None)),
        }
    }
}

/// An RGBA8 image, rows in storage order (row 0 = v 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, R, G, B, A.
    pub pixels: Vec<u8>,
}

/// Converts a PS2 alpha (0x80 = opaque) to 8-bit alpha: `min(255, (a * 255 + 64) / 128)`.
#[inline]
pub fn convert_alpha(a: u8) -> u8 {
    ((a as u32 * 255 + 64) / 128).min(255) as u8
}

/// Maps an 8-bit index to its palette entry (CSM1 order): swaps bits 3 and 4.
#[inline]
pub fn csm1_index(i: u8) -> u8 {
    (i & 0xE7) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)
}

/// The texture payload: the bytes from the texture base T to the end of the file.
#[derive(Clone, Copy, Debug)]
pub struct TexturePayload<'a> {
    /// The bytes from T on.
    pub bytes: &'a [u8],
}

impl<'a> TexturePayload<'a> {
    /// The payload of `world` with texture base `t`.
    pub fn new(world: &'a [u8], t: usize) -> Result<Self> {
        let bytes = world.get(t..).ok_or(Error::Eof { offset: t, needed: 0, len: world.len() })?;
        Ok(TexturePayload { bytes })
    }

    fn slice(&self, offset: u32, len: usize) -> Result<&'a [u8]> {
        let o = offset as usize;
        o.checked_add(len).and_then(|e| self.bytes.get(o..e)).ok_or(Error::Eof {
            offset: o,
            needed: len,
            len: self.bytes.len(),
        })
    }

    /// The raw texel bytes of a bitmap.
    pub fn texels(&self, b: &Bitmap) -> Result<&'a [u8]> {
        self.slice(b.offset, b.size as usize)
    }

    /// The raw colours of a palette (`count * 4` bytes, R, G, B, A).
    pub fn colors(&self, p: &Palette) -> Result<&'a [u8]> {
        self.slice(p.offset, p.count as usize * 4)
    }

    /// Decodes a bitmap to RGBA8 (textures.md 3). Indexed bitmaps need a palette; 32-bit ones ignore it.
    pub fn decode(&self, b: &Bitmap, palette: Option<&Palette>) -> Result<RgbaImage> {
        let fmt = b.format().ok_or(Error::Corrupt { offset: b.offset as usize, what: "unknown pixel format" })?;
        let (w, h) = (b.width as usize, b.height as usize);
        let texels = w * h;
        if b.size as usize != (texels * fmt.bits()).div_ceil(8) {
            return Err(Error::Corrupt {
                offset: b.offset as usize,
                what: "bitmap size does not match its dimensions",
            });
        }
        let data = self.texels(b)?;
        let mut pixels = vec![0u8; texels * 4];
        match fmt {
            PixelFormat::Rgba32 => {
                for (o, i) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(data.as_chunks::<4>().0.iter()) {
                    o.copy_from_slice(&[i[0], i[1], i[2], convert_alpha(i[3])]);
                }
            }
            PixelFormat::Indexed8 | PixelFormat::Indexed4 => {
                let p = palette
                    .ok_or(Error::Corrupt { offset: b.offset as usize, what: "indexed bitmap without palette" })?;
                let raw = self.colors(p)?;
                // Converted palette, padded to 256 entries; out-of-range indices are detected below.
                let mut lut = [[0u8; 4]; 256];
                for (e, c) in lut.iter_mut().zip(raw.as_chunks::<4>().0.iter()) {
                    *e = [c[0], c[1], c[2], convert_alpha(c[3])];
                }
                let n = p.count as usize;
                let mut max = 0u8;
                if fmt == PixelFormat::Indexed8 {
                    let mut swapped = [[0u8; 4]; 256];
                    for (i, s) in swapped.iter_mut().enumerate() {
                        *s = lut[csm1_index(i as u8) as usize];
                    }
                    for (o, &i) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(data) {
                        max = max.max(csm1_index(i));
                        o.copy_from_slice(&swapped[i as usize]);
                    }
                } else {
                    let (pairs, last) = pixels.as_chunks_mut::<8>();
                    for (o, &byte) in pairs.iter_mut().zip(data) {
                        let (lo, hi) = (byte & 15, byte >> 4);
                        max = max.max(lo).max(hi);
                        o[..4].copy_from_slice(&lut[lo as usize]);
                        o[4..].copy_from_slice(&lut[hi as usize]);
                    }
                    // An odd texel count leaves one pixel in the low nibble of the last byte.
                    if last.len() == 4
                        && let Some(&byte) = data.get(texels / 2)
                    {
                        let lo = byte & 15;
                        max = max.max(lo);
                        last.copy_from_slice(&lut[lo as usize]);
                    }
                }
                if max as usize >= n {
                    return Err(Error::OutOfRange { what: "palette entry", index: max as usize, len: n });
                }
            }
        }
        Ok(RgbaImage { width: w as u32, height: h as u32, pixels })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::tests::W;

    #[test]
    fn alpha_and_index_rules() {
        assert_eq!(convert_alpha(0x80), 0xFF);
        assert_eq!(convert_alpha(0x40), 0x80);
        assert_eq!(convert_alpha(0), 0);
        assert_eq!(convert_alpha(0xFF), 0xFF);
        assert_eq!(convert_alpha(0x18), 48);
        assert_eq!(csm1_index(0x08), 0x10);
        assert_eq!(csm1_index(0x10), 0x08);
        assert_eq!(csm1_index(0x18), 0x18);
        assert_eq!(csm1_index(0x07), 0x07);
        assert_eq!(csm1_index(0xE9), 0xF1);
    }

    fn tables() -> Vec<u8> {
        let mut w = W::default();
        // Palettes: a 16-colour one at 0, a 256-colour one at 64.
        w.u16(2).u32(1).u16(16).u16(1).u32(0).u32(1).u16(256).u16(4).u32(64);
        // Bitmaps: 4x2 4-bit at 1088, 4x2 8-bit at 1092, 2x1 32-bit at 1100.
        w.u16(3);
        w.u16(4).u16(2).u32(1).u8(0x14).u8(2).u16(1).u32(4).u32(1088);
        w.u16(4).u16(2).u32(1).u8(0x13).u8(2).u16(1).u32(8).u32(1092);
        w.u16(2).u16(1).u32(1).u8(0x00).u8(1).u16(1).u32(8).u32(1100);
        // Shaders: kind 0 (three palettes), kind 2 with a 32-bit bitmap, kind 1 with dangling bitmap 9.
        w.u16(3).u16(1);
        w.u8(0).u8(2).i16(100).bytes(&[255; 8]).u32(1).u16(0xFFFF).u8(3).u8(1).u16(0);
        w.i16(1).i16(0).u8(1).u8(3).u16(0).u16(0).u16(1).u16(0);
        w.u8(2).u8(2).i16(101).bytes(&[255; 8]).u32(1).u16(0xFFFF).u8(3).u8(1);
        w.u16(6).i16(5).u16(0).zeros(24);
        w.i16(2).i16(0).u8(1).u8(1).u16(2).u16(0xFFFF);
        w.u8(1).u8(2).i16(102).bytes(&[255; 8]).u32(1).u16(0xFFFF).u8(3).u8(1).u16(0xFFFF);
        w.i16(3).i16(0).u8(1).u8(1).u16(9).u16(0);
        // One font with 2 glyphs.
        w.u16(1).u16(0).i16(0).bytes(&[1, 2, 3, 4]).f32(1.0).u32(2).zeros(16);
        w.0
    }

    #[test]
    fn reads_tables_and_binds_shaders() {
        let data = tables();
        let mut r = Reader::new(&data);
        let t = MaterialTables::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!((t.palettes.len(), t.bitmaps.len(), t.shaders.len(), t.fonts.len()), (2, 3, 3, 1));
        assert_eq!(t.texel_extent(), 1108);
        assert_eq!(t.shaders[0].frame_texture(0, 2), Some((0, Some(0))));
        assert_eq!(t.shaders[0].base_texture(), Some((0, Some(1))));
        assert_eq!(t.shader_texture(0), Some((0, Some(1))));
        assert_eq!(t.shaders[1].animation.kind, 6);
        assert_eq!(t.shader_texture(1), Some((2, None)));
        assert_eq!(t.shader_texture(2), None); // dangling bitmap
        assert_eq!(t.fonts[0].glyph_count, 2);
    }

    #[test]
    fn decodes_all_three_formats() {
        let t = MaterialTables::read(&mut Reader::new(&tables())).unwrap();
        let mut payload = vec![0u8; 1108];
        for i in 0..16 {
            payload[i * 4..i * 4 + 4].copy_from_slice(&[i as u8, 0, 0, 0x80]);
        }
        for i in 0..256 {
            payload[64 + i * 4..68 + i * 4].copy_from_slice(&[0, i as u8, 0, 0x40]);
        }
        payload[1088..1092].copy_from_slice(&[0x10, 0x32, 0x54, 0xF6]);
        payload[1092..1100].copy_from_slice(&[0x00, 0x08, 0x10, 0x18, 1, 2, 3, 0xFF]);
        payload[1100..1108].copy_from_slice(&[1, 2, 3, 0x80, 4, 5, 6, 0x00]);
        let p = TexturePayload { bytes: &payload };
        let img = p.decode(&t.bitmaps[0], Some(&t.palettes[0])).unwrap();
        assert_eq!((img.width, img.height), (4, 2));
        let reds: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|c| c[0]).collect();
        assert_eq!(reds, [0, 1, 2, 3, 4, 5, 6, 15]);
        assert_eq!(&img.pixels[..4], &[0, 0, 0, 255]);
        let img = p.decode(&t.bitmaps[1], Some(&t.palettes[1])).unwrap();
        let greens: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|c| c[1]).collect();
        assert_eq!(greens, [0, 0x10, 0x08, 0x18, 1, 2, 3, 0xFF]);
        assert_eq!(img.pixels[3], 0x80);
        let img = p.decode(&t.bitmaps[2], None).unwrap();
        assert_eq!(img.pixels, [1, 2, 3, 255, 4, 5, 6, 0]);
        // A 16-colour palette cannot serve an 8-bit bitmap with index 0xFF.
        assert!(p.decode(&t.bitmaps[1], Some(&t.palettes[0])).is_err());
        assert!(p.decode(&t.bitmaps[0], None).is_err());
    }

    #[test]
    fn odd_4bit_texel_count_keeps_the_last_pixel() {
        // Three texels in two bytes: indices 1, 2, then 3 in the low nibble of the second byte.
        let mut payload = vec![0u8; 64 + 2];
        for i in 0..16 {
            payload[i * 4..i * 4 + 4].copy_from_slice(&[i as u8, 0, 0, 0x80]);
        }
        payload[64..66].copy_from_slice(&[0x21, 0x03]);
        let palette = Palette { zone_mask: 0, count: 16, size_class: 0, offset: 0 };
        let bitmap = Bitmap {
            width: 3,
            height: 1,
            zone_mask: 0,
            format_code: 0x14,
            gs_width: 0,
            gs_blocks: 0,
            size: 2,
            offset: 64,
        };
        let img = TexturePayload { bytes: &payload }.decode(&bitmap, Some(&palette)).unwrap();
        let reds: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|c| c[0]).collect();
        assert_eq!(reds, [1, 2, 3]);
    }
}
