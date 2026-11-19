//! Unit tests of the Sly 3 option decoding (the disc tests are in `tests/sly3_worlds.rs`).

use alloc::vec;
use alloc::vec::Vec;

use formats_sly2::exe::OptionEntry;

use super::options::decode_value;
use crate::object::{Arg, OptionValue};

fn entry(value_type: u32, typed: bool, arg_count: i32) -> OptionEntry {
    OptionEntry { value_type, flags: if typed { 0x400 } else { 0 }, arg_count }
}

#[test]
fn fixed_values_by_size() {
    assert_eq!(decode_value(&entry(0, false, 0), &[1]), Some(OptionValue::Byte(1)));
    assert_eq!(decode_value(&entry(1, false, 0), &2.5f32.to_le_bytes()), Some(OptionValue::Float(2.5)));
    assert_eq!(decode_value(&entry(0x101E, false, 0), &3i32.to_le_bytes()), Some(OptionValue::Int(3)));
    assert_eq!(decode_value(&entry(13, false, 0), &[1, 2, 3, 0x80]), Some(OptionValue::Color([1, 2, 3, 0x80])));
    assert_eq!(decode_value(&entry(23, false, 0), &(-2i16).to_le_bytes()), Some(OptionValue::Short(-2)));
    let mut v = Vec::new();
    for x in [1.0f32, 2.0, 3.0, 4.0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    assert_eq!(decode_value(&entry(4, false, 0), &v[..12]), Some(OptionValue::Vector([1.0, 2.0, 3.0])));
    assert_eq!(decode_value(&entry(12, false, 0), &v), Some(OptionValue::Quad([1.0, 2.0, 3.0, 4.0])));
    assert_eq!(decode_value(&entry(10, false, 0), &v[..8]), Some(OptionValue::Pair([1.0, 2.0])));
    // Enumeration types without value bytes, and a wrong length.
    assert_eq!(decode_value(&entry(0x3019, false, 0), &[]), Some(OptionValue::Args(Vec::new().into_boxed_slice())));
    assert_eq!(decode_value(&entry(1, false, 0), &[0, 0]), None);
}

#[test]
fn wide_strings_and_typed_lists() {
    let v = decode_value(&entry(22, false, 0), &[2, 0, b'h', 0, b'i', 0]).unwrap();
    assert_eq!(v, OptionValue::Args(vec![Arg::Short(22, b'h' as i16), Arg::Short(22, b'i' as i16)].into_boxed_slice()));
    let mut b = Vec::new();
    b.extend_from_slice(&6u16.to_le_bytes());
    b.extend_from_slice(&149_054i32.to_le_bytes());
    b.extend_from_slice(&(0x8000u16 | 23).to_le_bytes());
    b.push(2);
    b.extend_from_slice(&[3, 0, 4, 0]);
    let v = decode_value(&entry(0, true, 2), &b).unwrap();
    assert_eq!(v, OptionValue::Args(vec![Arg::Int(149_054), Arg::Short(23, 3), Arg::Short(23, 4)].into_boxed_slice()));
    // Option 27: no arguments.
    assert_eq!(decode_value(&entry(18, true, 0), &[]), Some(OptionValue::Args(Vec::new().into_boxed_slice())));
}
