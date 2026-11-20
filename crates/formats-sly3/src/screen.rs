//! A Sly 3 screen object's element set and group tree (classes/effects-logic-ui.md 6), read into Sly 2's screen form
//! (`formats_sly2::screen::Screen`: dictionary items and sprites), so the Sly 2 code that draws screen sprites serves
//! both games.
//!
//! How the fields map (6.1 and 6.2 give the sizes; the meanings below were read off the gadget-grid screens, whose
//! keys repeat Sly 2's screen 885 field for field) [I]:
//!
//! - **Elements** become items numbered by their index: kind 1 shapes (an `s32`, then Sly 2's primitives), kind 3
//!   texts (colour word, three floats, three bytes, the string ID for text kinds 0 and 1), kind 4 group references
//!   (an `s32`, then the group number: Sly 2's 16-bit value item, "show that sprite"), others empty.
//! - **Groups** become sprites: the two leading `u16`s are the instance and slot counts, each part a label (duration,
//!   ID, keys).
//! - **A key's items** become placements: the element (0 keeps the slot's), four bytes (the slot; unknown; Sly 2's
//!   flag byte; the index of the key's colour pair, 127 for none), the `s32` value and the 3 × 3 matrix. The key's
//!   trailing bytes are the slots it empties (Sly 2's 16-bit values). The two bytes and the `s32` before the items
//!   are kept nowhere (their meaning is open).
//!
//! An external screen's file is `ZRB.`, a `u16` version (7), then the element set and the group tree
//! ([`read_file`]).

use alloc::vec::Vec;

use formats::error::Result;
use formats::reader::Reader;
use formats_sly2::screen::{Item, ItemBody, Key, Label, Placement, Primitive, Screen, ShapeVertex, Sprite, Text};

/// Deepest nesting of elements (a font's glyphs).
const MAX_DEPTH: u32 = 4;

fn shape(r: &mut Reader<'_>) -> Result<Vec<Primitive>> {
    let n = r.u16()? as usize;
    let mut prims = Vec::with_capacity(n.min(r.remaining() / 5));
    for _ in 0..n {
        let k = r.u8()? as usize;
        let settings = [r.u8()?, r.u8()?, r.u8()?];
        let flag = r.u8()?;
        let mut vertices = Vec::with_capacity(k);
        for _ in 0..k {
            let rgba = r.array::<4>()?;
            let extra = r.u8()?;
            let (x, y) = (r.i16()?, r.i16()?);
            vertices.push(ShapeVertex { rgba, extra, x, y });
        }
        prims.push(Primitive { settings, flag, vertices });
    }
    Ok(prims)
}

fn element(r: &mut Reader<'_>, depth: u32) -> Result<ItemBody> {
    if depth > MAX_DEPTH {
        return Err(r.corrupt("screen elements nested too deeply"));
    }
    let kind = r.u32()?;
    Ok(match kind {
        1 => {
            r.i32()?;
            ItemBody::Shape(shape(r)?)
        }
        3 => {
            let colour = r.u32()?;
            let floats = [r.f32()?, r.f32()?, r.f32()?];
            let flag = r.i8()?;
            let text_kind = r.i8()?;
            r.i8()?;
            let string = if matches!(text_kind, 0 | 1) { Some(r.i32()?) } else { None };
            ItemBody::Text(Text { words: [0, colour], floats, flag, kind: text_kind, string })
        }
        4 => {
            r.i32()?;
            ItemBody::Value(r.u16()?)
        }
        6 | 7 => {
            let ltrb = [r.i16()?, r.i16()?, r.i16()?, r.i16()?];
            ItemBody::Rect { kind, ltrb, object: r.i32()? }
        }
        8 => {
            // A font: kept as a count of glyphs read past (the screens' fonts are read elsewhere).
            r.u8()?;
            for _ in 0..r.u16()? {
                r.skip(2 + 4)?;
                element(r, depth + 1)?;
            }
            ItemBody::Empty(8)
        }
        0 | 2 | 5 | 9 => ItemBody::Empty(kind),
        _ => return Err(r.corrupt("unknown screen element kind")),
    })
}

/// Reads an element set and a group tree.
pub fn read_elements_and_groups(r: &mut Reader<'_>) -> Result<Screen> {
    let n = r.u16()? as usize;
    let mut items = Vec::with_capacity(n.min(r.remaining() / 6));
    for _ in 0..n {
        let number = r.u16()?;
        items.push(Item { number, body: element(r, 0)? });
    }
    let groups = r.u16()? as usize;
    let mut sprites = Vec::with_capacity(groups.min(r.remaining() / 6));
    for _ in 0..groups {
        let instances = r.u16()?;
        let slots = r.u16()?;
        let parts = r.u16()? as usize;
        let mut labels = Vec::with_capacity(parts.min(r.remaining() / 10));
        for _ in 0..parts {
            let duration = r.f32()?;
            let id = r.i32()?;
            let nk = r.u16()? as usize;
            let mut keys = Vec::with_capacity(nk.min(r.remaining() / 13));
            for _ in 0..nk {
                let time = r.f32()?;
                let ni = r.u8()? as usize;
                let nc = r.u8()? as usize;
                let nw = r.u8()? as usize;
                r.skip(2 + 4)?;
                let mut placements = Vec::with_capacity(ni);
                for _ in 0..ni {
                    let item = r.u16()?;
                    let b = r.array::<4>()?;
                    let value = r.i32()?;
                    let mut matrix = [0.0f32; 9];
                    for m in &mut matrix {
                        *m = r.f32()?;
                    }
                    let depth = if b[3] == 127 { -1 } else { i16::from(b[3]) };
                    placements.push(Placement {
                        words: [item, u16::from(b[0]), 0],
                        depth,
                        bytes: [b[2], b[1]],
                        value,
                        matrix,
                    });
                }
                let colours = (0..nc).map(|_| Ok([r.u32()?, r.u32()?])).collect::<Result<Vec<_>>>()?;
                let values = (0..nw).map(|_| r.u8().map(u16::from)).collect::<Result<Vec<_>>>()?;
                keys.push(Key { time, placements, colours, values, bytes: Vec::new(), actions: Vec::new() });
            }
            labels.push(Label { duration, id, keys });
        }
        sprites.push(Sprite { instances, slots, labels });
    }
    Ok(Screen { items, sprites, ..Screen::default() })
}

/// Reads an external screen file (decompressed): `ZRB.`, the version, the element set and the group tree, to its
/// end.
pub fn read_file(file: &[u8]) -> Result<Screen> {
    let mut r = Reader::new(file);
    if r.bytes(4)? != b"ZRB." {
        return Err(r.corrupt("screen file without ZRB."));
    }
    r.u16()?;
    let s = read_elements_and_groups(&mut r)?;
    if r.remaining() != 0 {
        return Err(r.corrupt("screen file longer than its parts"));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_reference_and_a_key() {
        let mut d = Vec::new();
        d.extend_from_slice(b"ZRB.");
        d.extend_from_slice(&7u16.to_le_bytes());
        // Two elements: 1 an empty kind 9, 2 a reference to group 0.
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&9u32.to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&4u32.to_le_bytes());
        d.extend_from_slice(&(-1i32).to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        // One group (1 instance, 1 slot), one part (label 7), one key placing element 2 in slot 3 with colour pair 0,
        // emptying slot 5.
        for _ in 0..4 {
            d.extend_from_slice(&1u16.to_le_bytes());
        }
        d.extend_from_slice(&0.5f32.to_le_bytes());
        d.extend_from_slice(&7i32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&0.0f32.to_le_bytes());
        d.extend_from_slice(&[1, 1, 1, 0, 0]);
        d.extend_from_slice(&(-1i32).to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&[3, 0, 1, 0]);
        d.extend_from_slice(&(-1i32).to_le_bytes());
        for v in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 1.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&0x7F7F_7F7Fu32.to_le_bytes());
        d.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        d.push(5);
        let s = read_file(&d).unwrap();
        assert_eq!(s.items[1], Item { number: 2, body: ItemBody::Value(0) });
        let l = s.label(7).unwrap().1;
        let p = l.keys[0].placements[0];
        assert_eq!((p.words, p.depth, p.bytes), ([2, 3, 0], 0, [1, 0]));
        assert_eq!((l.keys[0].colours.len(), l.keys[0].values.clone()), (1, alloc::vec![5]));
    }
}
