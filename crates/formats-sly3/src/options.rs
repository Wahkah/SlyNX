//! Option lists (object-stream.md 1): Sly 2's shape with Sly 3's value sizes. The parsed list is Sly 2's
//! [`OptionList`].

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::reader::Reader;
use formats_sly2::exe::OptionTable;
pub use formats_sly2::options::{Opt, OptionList};

/// Remove the object when the level starts (object-stream.md 1.3).
pub const REMOVE_AT_START: i16 = 27;
/// A group's copy count in an option set (world-file.md 3.7).
pub const SET_COPY_COUNT: i16 = 1032;
/// A further condition an option set adds (conditions.md 1).
pub const SET_EXTRA_CONDITION: i16 = 1029;
/// Emitter position kind (classes 27).
pub const EMITTER_POSITION_27: i16 = 617;
/// Emitter position kind (classes 30, 31).
pub const EMITTER_POSITION_30: i16 = 670;
/// Effect component position kind.
pub const COMPONENT_POSITION: i16 = 558;

/// How many bytes a fixed value of `value_type` takes (object-stream.md 1.2); `None` for a wide string.
pub fn value_size(value_type: u32) -> Option<usize> {
    Some(match value_type {
        0 => 1,
        1 | 5 | 6 | 13 => 4,
        10 => 8,
        2 | 4 | 9 | 11 => 12,
        3 | 12 => 16,
        22 => return None,
        8 | 18 | 24.. => {
            if matches!((value_type >> 12) & 0xF, 1 | 2) {
                4
            } else {
                0
            }
        }
        _ => 2,
    })
}

/// Bytes of one typed-argument value with tag `tag` (low 15 bits; object-stream.md 1.2).
pub fn arg_size(tag: u16) -> Option<usize> {
    Some(match tag {
        0 => 1,
        1 | 5 | 6 => 4,
        2 | 9 => 12,
        10 => 8,
        7 | 14..=17 | 19..=21 | 23 => 2,
        0x1000..=0x1FFF => 4,
        _ => return None,
    })
}

/// Reads one option list.
pub fn read(r: &mut Reader<'_>, table: &OptionTable) -> Result<OptionList> {
    let start = r.pos();
    let mut options = Vec::new();
    loop {
        let at = r.pos();
        let id = r.i16()?;
        if id < 0 {
            break;
        }
        let entry = table.get(id).ok_or(Error::UnknownOption { offset: at, id })?;
        let vstart = r.pos();
        if entry.is_typed() {
            let n = entry.arg_count;
            if !(0..=64).contains(&n) {
                return Err(Error::Corrupt { offset: at, what: "option argument count out of range" });
            }
            for _ in 0..n {
                let tag = r.u16()?;
                let size = arg_size(tag & 0x7FFF).ok_or_else(|| r.corrupt("unknown typed-argument tag"))?;
                let count = if tag & 0x8000 != 0 { r.u8()? as usize } else { 1 };
                r.skip_n(count, size)?;
            }
        } else {
            match value_size(entry.value_type) {
                Some(n) => r.skip(n)?,
                None => {
                    let n = r.u16()? as usize;
                    r.skip_n(n, 2)?;
                }
            }
        }
        options.push(Opt { id, value: (vstart as u32, (r.pos() - vstart) as u32) });
    }
    Ok(OptionList { options, start, end: r.pos() })
}

/// The first value of option `id` as an `i32`: the first typed argument of a typed list, else the plain value
/// (1-, 2- or 4-byte values only).
pub fn first_argument(list: &OptionList, data: &[u8], table: &OptionTable, id: i16) -> Option<i32> {
    let v = list.value(data, id)?;
    let (size, at) = if table.get(id)?.is_typed() {
        let tag = u16::from_le_bytes([*v.first()?, *v.get(1)?]);
        if tag & 0x8000 != 0 { (arg_size(tag & 0x7FFF)?, 3) } else { (arg_size(tag)?, 2) }
    } else {
        (v.len(), 0)
    };
    let b = v.get(at..at + size)?;
    Some(match size {
        1 => b[0] as i8 as i32,
        2 => i16::from_le_bytes([b[0], b[1]]) as i32,
        4 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats_sly2::exe::OPTION_ENTRY_SIZE;

    #[test]
    fn sizes() {
        assert_eq!(value_size(3), Some(16));
        assert_eq!(value_size(12), Some(16));
        assert_eq!(value_size(4), Some(12));
        assert_eq!(value_size(22), None);
        assert_eq!(value_size(18), Some(0));
        assert_eq!(value_size(0x1018), Some(4));
        assert_eq!(value_size(0x2019), Some(4));
        assert_eq!(value_size(0x3019), Some(0));
        assert_eq!(value_size(23), Some(2));
        assert_eq!(arg_size(9), Some(12));
        assert_eq!(arg_size(17), Some(2));
        assert_eq!(arg_size(3), None);
    }

    #[test]
    fn wide_strings_and_typed_lists() {
        let mut raw = alloc::vec![0u8; 3 * OPTION_ENTRY_SIZE];
        raw[0..4].copy_from_slice(&22u32.to_le_bytes());
        raw[40 + 4..40 + 8].copy_from_slice(&0x400u32.to_le_bytes());
        raw[40 + 24..40 + 28].copy_from_slice(&1i32.to_le_bytes());
        raw[80..84].copy_from_slice(&3u32.to_le_bytes());
        let t = OptionTable::from_raw(&raw);
        let mut d = alloc::vec![];
        d.extend_from_slice(&0i16.to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&[b'h', 0, b'i', 0]);
        d.extend_from_slice(&1i16.to_le_bytes());
        d.extend_from_slice(&6u16.to_le_bytes());
        d.extend_from_slice(&3i32.to_le_bytes());
        d.extend_from_slice(&2i16.to_le_bytes());
        d.extend_from_slice(&[0u8; 16]);
        d.extend_from_slice(&(-1i16).to_le_bytes());
        let l = read(&mut Reader::new(&d), &t).unwrap();
        assert_eq!(l.end, d.len());
        assert_eq!(l.options.iter().map(|o| o.id).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(first_argument(&l, &d, &t, 1), Some(3));
    }
}
