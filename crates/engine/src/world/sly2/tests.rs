//! Unit tests of the Sly 2 option decoding and loading context (the disc tests are in `tests/sly2_worlds.rs`).

use alloc::vec;
use alloc::vec::Vec;

use formats_sly2::exe::OptionEntry;

use super::options::decode_value;
use crate::object::{Arg, OptionValue};

fn entry(value_type: u32, typed: bool, arg_count: i32) -> OptionEntry {
    OptionEntry { value_type, flags: if typed { 0x400 } else { 0 }, arg_count }
}

#[test]
fn fixed_values() {
    assert_eq!(decode_value(&entry(0, false, 0), &[1]), Some(OptionValue::Byte(1)));
    assert_eq!(decode_value(&entry(1, false, 0), &2.5f32.to_le_bytes()), Some(OptionValue::Float(2.5)));
    assert_eq!(decode_value(&entry(6, false, 0), &(-3i32).to_le_bytes()), Some(OptionValue::Int(-3)));
    assert_eq!(decode_value(&entry(0x1004, false, 0), &7i32.to_le_bytes()), Some(OptionValue::Int(7)));
    assert_eq!(decode_value(&entry(15, false, 0), &(-2i16).to_le_bytes()), Some(OptionValue::Short(-2)));
    assert_eq!(decode_value(&entry(13, false, 0), &[1, 2, 3, 0x80]), Some(OptionValue::Color([1, 2, 3, 0x80])));
    let mut v = Vec::new();
    for x in [1.0f32, 2.0, 3.0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    assert_eq!(decode_value(&entry(2, false, 0), &v), Some(OptionValue::Vector([1.0, 2.0, 3.0])));
    assert_eq!(decode_value(&entry(10, false, 0), &v[..8]), Some(OptionValue::Pair([1.0, 2.0])));
    assert_eq!(decode_value(&entry(3, false, 0), &v), None);
    assert_eq!(decode_value(&entry(1, false, 0), &[0, 0]), None);
}

#[test]
fn u16_lists() {
    let bytes = [2, 0, 5, 0, 0xFF, 0xFF];
    let v = decode_value(&entry(0x16, false, 0), &bytes).unwrap();
    assert_eq!(v, OptionValue::Args(vec![Arg::Short(0x16, 5), Arg::Short(0x16, -1)].into_boxed_slice()));
}

#[test]
fn typed_arguments_with_lists() {
    // Three arguments: a bool, a list of two enumeration values (tag 0x1010), a short of tag 21.
    let mut b = vec![0, 0, 1];
    b.extend_from_slice(&(0x8000u16 | 0x1010).to_le_bytes());
    b.push(2);
    b.extend_from_slice(&9i32.to_le_bytes());
    b.extend_from_slice(&10i32.to_le_bytes());
    b.extend_from_slice(&21u16.to_le_bytes());
    b.extend_from_slice(&4i16.to_le_bytes());
    let v = decode_value(&entry(0, true, 3), &b).unwrap();
    assert_eq!(
        v,
        OptionValue::Args(vec![Arg::Bool(true), Arg::Int(9), Arg::Int(10), Arg::Short(21, 4)].into_boxed_slice())
    );
    // No arguments (option 26).
    assert_eq!(decode_value(&entry(0, true, 0), &[]), Some(OptionValue::Args(Vec::new().into_boxed_slice())));
    // An unknown tag fails.
    assert_eq!(decode_value(&entry(0, true, 1), &[3, 0, 0, 0]), None);
}
