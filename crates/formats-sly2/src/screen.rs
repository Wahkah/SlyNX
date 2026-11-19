//! Screen movies (classes 126 to 131; classes/effects-logic-ui.md 4, user-interface.md 1): the game's 2-D screens,
//! read in full. The walk in [`crate::stream`] only steps over them.
//!
//! A body is: options, a streamed flag, the frame time, the canvas size and an unknown word, then either a file
//! record (streamed, [`Screen::file`]) or the dictionary and the sprites. A streamed file, once decompressed
//! ([`Screen::parse_streamed`]), is `ZRB.`, a `u16` version (6), then the dictionary and the sprites to its end.
//!
//! - **Dictionary items** ([`Item`]): shapes (primitives of coloured vertices in 12.4 fixed point), images, texts
//!   (with a string ID when their text kind is 0 or 1), 16-bit values, rectangles naming an object, and fonts (a
//!   nested dictionary of glyph shapes, the sorted character codes and each character's advance).
//! - **Sprites** ([`Sprite`]): labels (a duration and an ID: `0x30F` show, `0x310` idle, `0x311` hide by
//!   convention) of keys; each key places items by a 3 × 3 matrix, sets colour pairs, 16-bit values, bytes and
//!   actions. What each placement field means is open (the spec gives the sizes), so they are kept raw.

use alloc::vec::Vec;

use formats::error::Result;
use formats::reader::Reader;

use crate::record::FileRecord;

/// Shared sprite labels (user-interface.md 2).
pub mod label {
    pub const SHOW: i32 = 0x30F;
    pub const IDLE: i32 = 0x310;
    pub const HIDE: i32 = 0x311;
    pub const SWAP: i32 = 0x312;
}

/// One vertex of a shape primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeVertex {
    /// RGBA, alpha 0x80 = opaque.
    pub rgba: [u8; 4],
    /// The byte after the colour (meaning open).
    pub extra: u8,
    /// Position in sixteenths of a pixel (12.4 fixed point).
    pub x: i16,
    pub y: i16,
}

/// How a primitive's vertices form triangles (its first setting byte, user-interface.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topology {
    Fan,
    Strip,
    LineStrip,
    Other(u8),
}

/// A shape primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primitive {
    /// The three setting bytes (the first selects the topology).
    pub settings: [u8; 3],
    /// The "has extra bytes" flag.
    pub flag: u8,
    pub vertices: Vec<ShapeVertex>,
}

impl Primitive {
    /// The primitive's topology.
    pub fn topology(&self) -> Topology {
        match self.settings[0] {
            1 => Topology::Fan,
            2 => Topology::Strip,
            3 => Topology::LineStrip,
            k => Topology::Other(k),
        }
    }

    /// Its triangles as vertex index triples (none for line strips).
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        let n = self.vertices.len();
        match self.topology() {
            Topology::Fan => (1..n.saturating_sub(1)).map(|i| [0, i, i + 1]).collect(),
            Topology::Strip => (0..n.saturating_sub(2)).map(|i| [i, i + 1, i + 2]).collect(),
            _ => Vec::new(),
        }
    }
}

/// A text item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Text {
    /// The two leading words (meaning open).
    pub words: [u32; 2],
    /// Three floats (meaning open; the text size is among them [R]).
    pub floats: [f32; 3],
    /// The byte before the text kind.
    pub flag: i8,
    /// Text kind.
    pub kind: i8,
    /// The string ID (text kinds 0 and 1).
    pub string: Option<i32>,
}

/// A font (item kind 8).
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// The glyphs, one nested item per character, in character order.
    pub glyphs: Vec<Item>,
    /// The character codes (sorted).
    pub chars: Vec<u32>,
    /// Each character's advance.
    pub advances: Vec<f32>,
}

/// A dictionary item's payload.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemBody {
    /// Kinds 0, 5 and 9: nothing.
    Empty(u32),
    /// Kind 1.
    Shape(Vec<Primitive>),
    /// Kind 2: a matrix and 24-byte entries.
    Image { matrix: [f32; 9], entries: Vec<[u8; 24]> },
    /// Kind 3.
    Text(Text),
    /// Kind 4.
    Value(u16),
    /// Kinds 6 and 7: left, top, right, bottom and an object ID.
    Rect { kind: u32, ltrb: [i16; 4], object: i32 },
    /// Kind 8.
    Font(Font),
}

/// A dictionary item.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub number: u16,
    pub body: ItemBody,
}

/// A placement of a key (50 bytes; the fields' meanings are open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub words: [u16; 3],
    pub depth: i16,
    pub bytes: [u8; 2],
    pub value: i32,
    pub matrix: [f32; 9],
}

/// A key of a label.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub time: f32,
    pub placements: Vec<Placement>,
    pub colours: Vec<[u32; 2]>,
    pub values: Vec<u16>,
    pub bytes: Vec<u8>,
    pub actions: Vec<(u8, i32)>,
}

/// A label of a sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub duration: f32,
    pub id: i32,
    pub keys: Vec<Key>,
}

/// A sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub instances: u16,
    pub slots: u16,
    pub labels: Vec<Label>,
}

/// A screen movie.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Screen {
    /// Seconds per frame.
    pub frame_time: f32,
    /// Canvas size (640 × 480 on the disc).
    pub size: [u32; 2],
    /// The word after the size.
    pub word: u32,
    /// The file the movie streams from.
    pub file: Option<FileRecord>,
    pub items: Vec<Item>,
    pub sprites: Vec<Sprite>,
}

fn matrix(r: &mut Reader<'_>) -> Result<[f32; 9]> {
    let mut m = [0.0; 9];
    for v in &mut m {
        *v = r.f32()?;
    }
    Ok(m)
}

/// Reads a dictionary.
pub fn read_dictionary(r: &mut Reader<'_>) -> Result<Vec<Item>> {
    let n = r.u16()? as usize;
    let mut items = Vec::with_capacity(n.min(r.remaining() / 6));
    for _ in 0..n {
        let number = r.u16()?;
        let kind = r.u32()?;
        let body = match kind {
            0 | 5 | 9 => ItemBody::Empty(kind),
            1 => {
                let count = r.u16()? as usize;
                let mut prims = Vec::with_capacity(count.min(r.remaining() / 5));
                for _ in 0..count {
                    let k = r.u8()? as usize;
                    let settings = [r.u8()?, r.u8()?, r.u8()?];
                    let flag = r.u8()?;
                    let mut vertices = Vec::with_capacity(k);
                    for _ in 0..k {
                        let rgba = r.array::<4>()?;
                        let extra = r.u8()?;
                        let x = r.i16()?;
                        let y = r.i16()?;
                        vertices.push(ShapeVertex { rgba, extra, x, y });
                    }
                    prims.push(Primitive { settings, flag, vertices });
                }
                ItemBody::Shape(prims)
            }
            2 => {
                let m = matrix(r)?;
                let count = r.u16()? as usize;
                let mut entries = Vec::with_capacity(count.min(r.remaining() / 24));
                for _ in 0..count {
                    entries.push(r.array::<24>()?);
                }
                ItemBody::Image { matrix: m, entries }
            }
            3 => {
                let words = [r.u32()?, r.u32()?];
                let floats = [r.f32()?, r.f32()?, r.f32()?];
                let flag = r.i8()?;
                let kind = r.i8()?;
                let string = if kind == 0 || kind == 1 { Some(r.i32()?) } else { None };
                ItemBody::Text(Text { words, floats, flag, kind, string })
            }
            4 => ItemBody::Value(r.u16()?),
            6 | 7 => ItemBody::Rect { kind, ltrb: [r.i16()?, r.i16()?, r.i16()?, r.i16()?], object: r.i32()? },
            8 => {
                let glyphs = read_dictionary(r)?;
                let c = r.u32()? as usize;
                let chars = (0..c.min(r.remaining() / 4)).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
                let c = r.u32()? as usize;
                let advances = (0..c.min(r.remaining() / 4)).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
                ItemBody::Font(Font { glyphs, chars, advances })
            }
            _ => return Err(r.corrupt("unknown screen-movie item kind")),
        };
        items.push(Item { number, body });
    }
    Ok(items)
}

/// Reads the sprites.
pub fn read_sprites(r: &mut Reader<'_>) -> Result<Vec<Sprite>> {
    let n = r.u16()? as usize;
    let mut sprites = Vec::with_capacity(n.min(r.remaining() / 6));
    for _ in 0..n {
        let instances = r.u16()?;
        let slots = r.u16()?;
        let nl = r.u16()? as usize;
        let mut labels = Vec::with_capacity(nl.min(r.remaining() / 10));
        for _ in 0..nl {
            let duration = r.f32()?;
            let id = r.i32()?;
            let nk = r.u16()? as usize;
            let mut keys = Vec::with_capacity(nk.min(r.remaining() / 9));
            for _ in 0..nk {
                let time = r.f32()?;
                let np = r.u8()? as usize;
                let mut placements = Vec::with_capacity(np);
                for _ in 0..np {
                    placements.push(Placement {
                        words: [r.u16()?, r.u16()?, r.u16()?],
                        depth: r.i16()?,
                        bytes: [r.u8()?, r.u8()?],
                        value: r.i32()?,
                        matrix: matrix(r)?,
                    });
                }
                let nc = r.u8()? as usize;
                let colours = (0..nc).map(|_| Ok([r.u32()?, r.u32()?])).collect::<Result<Vec<_>>>()?;
                let nv = r.u8()? as usize;
                let values = (0..nv).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
                let nb = r.u8()? as usize;
                let bytes = r.bytes(nb)?.to_vec();
                let na = r.u8()? as usize;
                let actions = (0..na).map(|_| Ok((r.u8()?, r.i32()?))).collect::<Result<Vec<_>>>()?;
                keys.push(Key { time, placements, colours, values, bytes, actions });
            }
            labels.push(Label { duration, id, keys });
        }
        sprites.push(Sprite { instances, slots, labels });
    }
    Ok(sprites)
}

impl Screen {
    /// Reads a screen movie's body from `body` (the byte after the option list) to `end`.
    pub fn parse(data: &[u8], body: usize, end: usize) -> Result<Screen> {
        let mut r = Reader::at(data.get(..end).unwrap_or(data), body);
        let streamed = r.i8()? != 0;
        let frame_time = r.f32()?;
        let size = [r.u32()?, r.u32()?];
        let word = r.u32()?;
        let mut s = Screen { frame_time, size, word, ..Screen::default() };
        if streamed {
            s.file = Some(FileRecord::decode(r.bytes(FileRecord::SIZE)?));
        } else {
            s.items = read_dictionary(&mut r)?;
            s.sprites = read_sprites(&mut r)?;
        }
        if r.remaining() != 0 {
            return Err(r.corrupt("screen movie body longer than its parts"));
        }
        Ok(s)
    }

    /// Fills a streamed screen from its decompressed file (`ZRB.`, version, dictionary, sprites).
    pub fn parse_streamed(&mut self, file: &[u8]) -> Result<()> {
        let mut r = Reader::new(file);
        if r.bytes(4)? != b"ZRB." {
            return Err(r.corrupt("streamed screen without ZRB."));
        }
        let _version = r.u16()?;
        self.items = read_dictionary(&mut r)?;
        self.sprites = read_sprites(&mut r)?;
        if r.remaining() != 0 {
            return Err(r.corrupt("streamed screen longer than its parts"));
        }
        Ok(())
    }

    /// The fonts of the dictionary, in order.
    pub fn fonts(&self) -> impl Iterator<Item = &Font> + '_ {
        self.items.iter().filter_map(|i| match &i.body {
            ItemBody::Font(f) => Some(f),
            _ => None,
        })
    }

    /// The string IDs its text items show (text kinds 0 and 1).
    pub fn strings(&self) -> impl Iterator<Item = i32> + '_ {
        self.items.iter().filter_map(|i| match &i.body {
            ItemBody::Text(t) => t.string,
            _ => None,
        })
    }

    /// A sprite label by ID: (sprite index, label).
    pub fn label(&self, id: i32) -> Option<(usize, &Label)> {
        self.sprites.iter().enumerate().find_map(|(i, s)| s.labels.iter().find(|l| l.id == id).map(|l| (i, l)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fans_and_strips() {
        let v = ShapeVertex { rgba: [0; 4], extra: 0, x: 0, y: 0 };
        let p = |s| Primitive { settings: [s, 0, 0], flag: 0, vertices: alloc::vec![v; 5] };
        assert_eq!(p(1).triangles(), alloc::vec![[0, 1, 2], [0, 2, 3], [0, 3, 4]]);
        assert_eq!(p(2).triangles(), alloc::vec![[0, 1, 2], [1, 2, 3], [2, 3, 4]]);
        assert!(p(3).triangles().is_empty());
    }
}
