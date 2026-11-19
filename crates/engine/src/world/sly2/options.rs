//! Sly 2 option values (specs/sly2/object-stream.md 1): each option's encoding comes from the executable's option
//! table, read by `formats-sly2`. Values are decoded into the engine's [`OptionValue`] by the working meanings of
//! object-stream.md 1.2: 0 a byte (boolean), 1 a float, 2 and 9 vectors, 5 and 6 integers, 7 and 14 to 16 16-bit
//! identifiers, 10 a pair, 13 a packed colour, the enumeration types `0x1000` to `0x2FFF` 32-bit integers.
//!
//! Typed argument lists (flag `0x400`, 1.3) become [`OptionValue::Args`]; a list argument (tag bit 15) is flattened
//! into its values in order. The `u16` lists of value type `0x16` are kept as `Args` of [`Arg::Short`] with tag
//! `0x16`; they are values, not actions.

use alloc::boxed::Box;
use alloc::vec::Vec;

use formats::reader::Reader;
use formats_sly2::exe::{OptionEntry, OptionTable};
use formats_sly2::options::{Opt, OptionList, VALUE_TYPE_U16_LIST};

use crate::object::{Arg, OptionValue};

/// Option 26: remove the object when the level starts (object-stream.md 1.4).
pub const REMOVE_AT_START: u16 = 26;

/// One decoded option of a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// Option ID.
    pub id: u16,
    /// Value.
    pub value: OptionValue,
    /// A typed argument list (an action in the option table's sense).
    pub action: bool,
}

/// Decodes every option of `list` in order. An option whose bytes do not decode is left out (and counted in the
/// second value).
pub fn decode_list(data: &[u8], list: &OptionList, table: &OptionTable) -> (Vec<Decoded>, u32) {
    let mut out = Vec::with_capacity(list.options.len());
    let mut failed = 0;
    for o in &list.options {
        match decode(data, o, table) {
            Some(d) => out.push(d),
            None => failed += 1,
        }
    }
    (out, failed)
}

/// Decodes one option.
pub fn decode(data: &[u8], o: &Opt, table: &OptionTable) -> Option<Decoded> {
    let entry = table.get(o.id)?;
    let (at, len) = (o.value.0 as usize, o.value.1 as usize);
    let bytes = data.get(at..at.checked_add(len)?)?;
    let action = entry.is_typed();
    let value = decode_value(entry, bytes)?;
    Some(Decoded { id: o.id as u16, value, action })
}

/// Decodes the value bytes of one option of `entry`.
pub fn decode_value(entry: &OptionEntry, bytes: &[u8]) -> Option<OptionValue> {
    let mut r = Reader::new(bytes);
    if entry.is_typed() {
        let mut args = Vec::new();
        for _ in 0..entry.arg_count.max(0) {
            let tag = r.u16().ok()?;
            let count = if tag & 0x8000 != 0 { r.u8().ok()? as usize } else { 1 };
            for _ in 0..count {
                args.push(arg(&mut r, tag & 0x7FFF)?);
            }
        }
        return Some(OptionValue::Args(args.into_boxed_slice()));
    }
    let f = |r: &mut Reader<'_>| r.f32().ok();
    Some(match entry.value_type {
        0 => OptionValue::Byte(r.u8().ok()?),
        1 => OptionValue::Float(f(&mut r)?),
        5 | 6 | 0x1000..=0x2FFF => OptionValue::Int(r.i32().ok()?),
        13 => OptionValue::Color(r.array::<4>().ok()?),
        2 | 9 => OptionValue::Vector(r.vec3().ok()?),
        7 | 14 | 15 | 16 => OptionValue::Short(r.i16().ok()?),
        10 => OptionValue::Pair([f(&mut r)?, f(&mut r)?]),
        VALUE_TYPE_U16_LIST => {
            let n = r.u16().ok()? as usize;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(Arg::Short(VALUE_TYPE_U16_LIST as i16, r.i16().ok()?));
            }
            OptionValue::Args(v.into_boxed_slice())
        }
        _ => return None,
    })
}

/// One typed-argument value of tag `tag` (object-stream.md 1.3).
fn arg(r: &mut Reader<'_>, tag: u16) -> Option<Arg> {
    let f = |r: &mut Reader<'_>| r.f32().ok();
    Some(match tag {
        0 => Arg::Bool(r.u8().ok()? != 0),
        1 => Arg::Float(f(r)?),
        5 | 6 | 0x1000..=0x1FFF => Arg::Int(r.i32().ok()?),
        2 => Arg::Vector(2, r.vec3().ok()?),
        7 | 14 | 15 | 16 | 19 | 20 | 21 => Arg::Short(tag as i16, r.i16().ok()?),
        10 => Arg::Range([f(r)?, f(r)?]),
        _ => return None,
    })
}

/// The arguments of an action value (empty for any other value).
pub fn args_of(v: &OptionValue) -> Box<[Arg]> {
    match v {
        OptionValue::Args(a) => a.clone(),
        _ => Box::new([]),
    }
}
