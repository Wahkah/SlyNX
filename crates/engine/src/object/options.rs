//! Option values and the per-object option store (object-stream.md section 4, options-by-class.md section 1).
//!
//! Options are applied in order when an object loads, after its class defaults. The store keeps every occurrence in
//! order: a plain read takes the last one ("later options of the same ID overwrite earlier ones"), and "add" options
//! read all of them. Action options (typed argument lists) are not stored: they run once per occurrence.

use alloc::boxed::Box;
use alloc::vec::Vec;

use formats::options::{OptionEncoding, OptionList, OptionTable};
use formats::reader::Reader;

/// One option value, decoded by the option's encoding (object-stream.md 4.2).
#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    /// `b`: a byte (booleans and small enumerations).
    Byte(u8),
    /// `h`: an `s16` (OIDs, class IDs, sound IDs, enumerations).
    Short(i16),
    /// `f`.
    Float(f32),
    /// `i`.
    Int(i32),
    /// `c`: packed colour R, G, B, A (A = 0x80 is opaque).
    Color([u8; 4]),
    /// `p`: two floats (a range or two parameters).
    Pair([f32; 2]),
    /// `v`: three floats.
    Vector([f32; 3]),
    /// `q`: four floats.
    Quad([f32; 4]),
    /// `0` to `6`: a typed argument list (an action).
    Args(Box<[Arg]>),
}

/// One argument of a typed argument list (object-stream.md 4.3).
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// Tag 0.
    Bool(bool),
    /// Tag 1.
    Float(f32),
    /// Tags 2, 4, 9, 11.
    Vector(i16, [f32; 3]),
    /// Tags 3, 12.
    Quad(i16, [f32; 4]),
    /// Tag 5.
    Int(i32),
    /// Tags 6, 7, 14 to 17, 20, and enumerations 4096 to 8191: the tag and the `s16`.
    Short(i16, i16),
    /// Tag 10.
    Range([f32; 2]),
    /// Tag 13.
    Color(u32),
}

impl OptionValue {
    /// Decodes one value of encoding `enc` from `bytes` (the value bytes an option list yields).
    pub fn decode(enc: OptionEncoding, bytes: &[u8]) -> Option<OptionValue> {
        let mut r = Reader::new(bytes);
        let f = |r: &mut Reader<'_>| r.f32().ok();
        Some(match enc {
            OptionEncoding::Byte => OptionValue::Byte(r.u8().ok()?),
            OptionEncoding::Short => OptionValue::Short(r.i16().ok()?),
            OptionEncoding::Float => OptionValue::Float(f(&mut r)?),
            OptionEncoding::Int => OptionValue::Int(r.i32().ok()?),
            OptionEncoding::Color => OptionValue::Color(r.array::<4>().ok()?),
            OptionEncoding::Pair => OptionValue::Pair([f(&mut r)?, f(&mut r)?]),
            OptionEncoding::Vector => OptionValue::Vector(r.vec3().ok()?),
            OptionEncoding::Quad => OptionValue::Quad([f(&mut r)?, f(&mut r)?, f(&mut r)?, f(&mut r)?]),
            OptionEncoding::Args(n) => {
                let mut args = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    args.push(Arg::read(&mut r)?);
                }
                OptionValue::Args(args.into_boxed_slice())
            }
        })
    }

    /// The value as a float (bytes, shorts and ints convert).
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            OptionValue::Float(v) => Some(v),
            OptionValue::Int(v) => Some(v as f32),
            OptionValue::Short(v) => Some(v as f32),
            OptionValue::Byte(v) => Some(v as f32),
            _ => None,
        }
    }

    /// The value as an integer (floats truncate).
    pub fn as_i32(&self) -> Option<i32> {
        match *self {
            OptionValue::Int(v) => Some(v),
            OptionValue::Short(v) => Some(v as i32),
            OptionValue::Byte(v) => Some(v as i32),
            OptionValue::Float(v) => Some(v as i32),
            _ => None,
        }
    }

    /// The value as a boolean (non-zero numbers are true).
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            OptionValue::Byte(v) => Some(v != 0),
            OptionValue::Short(v) => Some(v != 0),
            OptionValue::Int(v) => Some(v != 0),
            OptionValue::Float(v) => Some(v != 0.0),
            _ => None,
        }
    }

    /// The value as a vector.
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match *self {
            OptionValue::Vector(v) => Some(v),
            _ => None,
        }
    }

    /// The value as a pair.
    pub fn as_pair(&self) -> Option<[f32; 2]> {
        match *self {
            OptionValue::Pair(v) => Some(v),
            _ => None,
        }
    }

    /// Whether the value is an action (typed argument list).
    pub fn is_action(&self) -> bool {
        matches!(self, OptionValue::Args(_))
    }
}

impl Arg {
    fn read(r: &mut Reader<'_>) -> Option<Arg> {
        let tag = r.i16().ok()?;
        let f = |r: &mut Reader<'_>| r.f32().ok();
        Some(match tag {
            0 => Arg::Bool(r.u8().ok()? != 0),
            1 => Arg::Float(f(r)?),
            2 | 4 | 9 | 11 => Arg::Vector(tag, r.vec3().ok()?),
            3 | 12 => Arg::Quad(tag, [f(r)?, f(r)?, f(r)?, f(r)?]),
            5 => Arg::Int(r.i32().ok()?),
            6 | 7 | 14..=17 | 20 | 4096..=8191 => Arg::Short(tag, r.i16().ok()?),
            10 => Arg::Range([f(r)?, f(r)?]),
            13 => Arg::Color(r.u32().ok()?),
            _ => return None,
        })
    }
}

/// "Add" options: each occurrence appends to a list instead of replacing the previous value (options-by-class.md 1.1,
/// 1.2, 2.6).
pub const ADD_OPTIONS: &[u16] =
    &[205, 212, 251, 252, 253, 254, 255, 256, 264, 267, 268, 275, 276, 289, 290, 299, 300, 708];

/// The options of one object, in the order they were applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptionStore {
    entries: Vec<(u16, OptionValue)>,
}

impl OptionStore {
    /// An empty store.
    pub const fn new() -> OptionStore {
        OptionStore { entries: Vec::new() }
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry in order.
    pub fn entries(&self) -> &[(u16, OptionValue)] {
        &self.entries
    }

    /// The last value of option `id`.
    pub fn get(&self, id: u16) -> Option<&OptionValue> {
        self.entries.iter().rev().find(|(i, _)| *i == id).map(|(_, v)| v)
    }

    /// Every value of option `id`, in order (for "add" options).
    pub fn all(&self, id: u16) -> impl Iterator<Item = &OptionValue> + '_ {
        self.entries.iter().filter(move |(i, _)| *i == id).map(|(_, v)| v)
    }

    /// Whether option `id` is set.
    pub fn has(&self, id: u16) -> bool {
        self.entries.iter().any(|(i, _)| *i == id)
    }

    /// Appends one occurrence (the order of application is kept).
    pub fn push(&mut self, id: u16, value: OptionValue) {
        self.entries.push((id, value));
    }

    /// Sets option `id` the way a script does: replaces every earlier value, or appends for "add" options.
    pub fn set(&mut self, id: u16, value: OptionValue) {
        if !ADD_OPTIONS.contains(&id) {
            self.entries.retain(|(i, _)| *i != id);
        }
        self.entries.push((id, value));
    }

    /// Removes every value of option `id`.
    pub fn clear(&mut self, id: u16) {
        self.entries.retain(|(i, _)| *i != id);
    }

    /// Float value of `id`.
    pub fn f32(&self, id: u16) -> Option<f32> {
        self.get(id).and_then(OptionValue::as_f32)
    }

    /// Float value of `id`, or `default`.
    pub fn f32_or(&self, id: u16, default: f32) -> f32 {
        self.f32(id).unwrap_or(default)
    }

    /// Integer value of `id`.
    pub fn i32(&self, id: u16) -> Option<i32> {
        self.get(id).and_then(OptionValue::as_i32)
    }

    /// `s16` value of `id` (an OID, a class ID, an enumeration).
    pub fn i16(&self, id: u16) -> Option<i16> {
        self.i32(id).map(|v| v as i16)
    }

    /// Boolean value of `id`, or `default`.
    pub fn bool_or(&self, id: u16, default: bool) -> bool {
        self.get(id).and_then(OptionValue::as_bool).unwrap_or(default)
    }

    /// Vector value of `id`.
    pub fn vec3(&self, id: u16) -> Option<[f32; 3]> {
        self.get(id).and_then(OptionValue::as_vec3)
    }

    /// Pair value of `id`.
    pub fn pair(&self, id: u16) -> Option<[f32; 2]> {
        self.get(id).and_then(OptionValue::as_pair)
    }

    /// Every `s16` value of an "add" option (OID lists).
    pub fn shorts(&self, id: u16) -> impl Iterator<Item = i16> + '_ {
        self.all(id).filter_map(|v| v.as_i32().map(|x| x as i16))
    }
}

/// Decodes every entry of an option list stored in `data`. Entries that fail to decode (never on the disc: the walk
/// already checked every list) are skipped.
pub fn decode_list(data: &[u8], list: &OptionList, table: &OptionTable) -> Vec<(u16, OptionValue)> {
    let mut out = Vec::with_capacity(list.count as usize);
    for item in list.iter(data, table) {
        let Ok((id, bytes)) = item else { break };
        let Some(enc) = table.get(id) else { continue };
        if let Some(v) = OptionValue::decode(enc, bytes) {
            out.push((id as u16, v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_values() {
        assert_eq!(OptionValue::decode(OptionEncoding::Byte, &[1]), Some(OptionValue::Byte(1)));
        assert_eq!(OptionValue::decode(OptionEncoding::Short, &[0xFE, 0xFF]), Some(OptionValue::Short(-2)));
        assert_eq!(OptionValue::decode(OptionEncoding::Float, &1.5f32.to_le_bytes()), Some(OptionValue::Float(1.5)));
        let mut v = alloc::vec::Vec::new();
        for x in [1.0f32, 2.0, 3.0] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        assert_eq!(OptionValue::decode(OptionEncoding::Vector, &v), Some(OptionValue::Vector([1.0, 2.0, 3.0])));
        // A one-argument action: tag 6 (an OID) = 42.
        let args = [6u8, 0, 42, 0];
        assert_eq!(
            OptionValue::decode(OptionEncoding::Args(1), &args),
            Some(OptionValue::Args(alloc::vec![Arg::Short(6, 42)].into_boxed_slice()))
        );
        assert_eq!(OptionValue::decode(OptionEncoding::Args(0), &[]), Some(OptionValue::Args(Box::new([]))));
        assert_eq!(OptionValue::decode(OptionEncoding::Int, &[1, 2]), None);
    }

    #[test]
    fn store_last_wins_and_add_accumulates() {
        let mut s = OptionStore::new();
        s.push(257, OptionValue::Float(1.0));
        s.push(257, OptionValue::Float(2.5));
        assert_eq!(s.f32(257), Some(2.5));
        assert_eq!(s.all(257).count(), 2);
        s.set(257, OptionValue::Float(4.0));
        assert_eq!(s.all(257).count(), 1);
        s.set(212, OptionValue::Short(5));
        s.set(212, OptionValue::Short(6));
        assert_eq!(s.shorts(212).collect::<alloc::vec::Vec<_>>(), [5, 6]);
        assert!(s.bool_or(1, true));
        s.push(1, OptionValue::Byte(0));
        assert!(!s.bool_or(1, true));
        s.clear(1);
        assert!(!s.has(1));
    }
}
