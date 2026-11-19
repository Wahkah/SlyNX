//! Sly 3 option values (specs/sly3/object-stream.md 1): each option's encoding comes from the executable's option
//! table; the sizes are Sly 3's (1.2). Values decode by size into the engine's [`OptionValue`] as the Sly 2 decoder
//! does ([`crate::world::sly2::options`]): 1 byte a byte, 2 bytes an `s16`, 4 bytes a float (type 1), a colour
//! (type 13) or an integer, 8 a pair, 12 a vector, 16 four floats; a wide string (type 22) becomes `Args` of
//! [`Arg::Short`] with tag 22; an enumeration type with no value bytes becomes an empty `Args`.
//!
//! Typed argument lists become [`OptionValue::Args`], list arguments flattened, as for Sly 2. The [`Decoded`] type
//! and [`args_of`] are Sly 2's.

use alloc::vec::Vec;

use formats::reader::Reader;
use formats_sly2::exe::{OptionEntry, OptionTable};
use formats_sly2::options::{Opt, OptionList};
use formats_sly3::options::value_size;

use crate::object::{Arg, OptionValue};
pub use crate::world::sly2::options::{Decoded, args_of};

/// Option 27: remove the object when the level starts (object-stream.md 1.3).
pub const REMOVE_AT_START: u16 = formats_sly3::options::REMOVE_AT_START as u16;
/// The wide-string value type.
pub const VALUE_TYPE_WIDE_STRING: u32 = 22;

/// Decodes every option of `list` in order; the second value counts options that did not decode.
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
    Some(Decoded { id: o.id as u16, value: decode_value(entry, bytes)?, action: entry.is_typed() })
}

/// Decodes the value bytes of one option of `entry`.
pub fn decode_value(entry: &OptionEntry, bytes: &[u8]) -> Option<OptionValue> {
    let mut r = Reader::new(bytes);
    let f = |r: &mut Reader<'_>| r.f32().ok();
    if entry.is_typed() {
        let mut args = Vec::new();
        for _ in 0..entry.arg_count.max(0) {
            let tag = r.u16().ok()?;
            let count = if tag & 0x8000 != 0 { r.u8().ok()? as usize } else { 1 };
            for _ in 0..count {
                args.push(arg(&mut r, tag & 0x7FFF)?);
            }
        }
        return (r.remaining() == 0).then(|| OptionValue::Args(args.into_boxed_slice()));
    }
    let value = match value_size(entry.value_type) {
        None => {
            let n = r.u16().ok()? as usize;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(Arg::Short(VALUE_TYPE_WIDE_STRING as i16, r.i16().ok()?));
            }
            OptionValue::Args(v.into_boxed_slice())
        }
        Some(0) => OptionValue::Args(Vec::new().into_boxed_slice()),
        Some(1) => OptionValue::Byte(r.u8().ok()?),
        Some(2) => OptionValue::Short(r.i16().ok()?),
        Some(4) => match entry.value_type {
            1 => OptionValue::Float(f(&mut r)?),
            13 => OptionValue::Color(r.array::<4>().ok()?),
            _ => OptionValue::Int(r.i32().ok()?),
        },
        Some(8) => OptionValue::Pair([f(&mut r)?, f(&mut r)?]),
        Some(12) => OptionValue::Vector(r.vec3().ok()?),
        Some(16) => OptionValue::Quad([f(&mut r)?, f(&mut r)?, f(&mut r)?, f(&mut r)?]),
        Some(_) => return None,
    };
    (r.remaining() == 0).then_some(value)
}

/// One typed-argument value of tag `tag` (object-stream.md 1.2, Sly 2's tag format).
fn arg(r: &mut Reader<'_>, tag: u16) -> Option<Arg> {
    let f = |r: &mut Reader<'_>| r.f32().ok();
    Some(match tag {
        0 => Arg::Bool(r.u8().ok()? != 0),
        1 => Arg::Float(f(r)?),
        5 | 6 | 0x1000..=0x1FFF => Arg::Int(r.i32().ok()?),
        2 | 9 => Arg::Vector(tag as i16, r.vec3().ok()?),
        7 | 14..=17 | 19..=21 | 23 => Arg::Short(tag as i16, r.i16().ok()?),
        10 => Arg::Range([f(r)?, f(r)?]),
        _ => return None,
    })
}
