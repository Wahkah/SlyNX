//! Option lists (object-stream.md 1): `s16` option ID and value, repeated, ended by a negative ID. Each ID's value
//! encoding comes from the executable's option table.

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::reader::Reader;

use crate::exe::{OptionTable, arg_size, value_size};

/// Value type 0x16 (object-stream.md 1.2), only on the mission options 1262 and 1295 (classes 132 and 133): a `u16`
/// count followed by that many `u16` values.
pub const VALUE_TYPE_U16_LIST: u32 = 0x16;

/// One option: its ID and where its value bytes are (absolute offsets into the parsed data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opt {
    pub id: i16,
    pub value: (u32, u32),
}

/// A parsed option list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionList {
    pub options: Vec<Opt>,
    /// Offset of the list and of the byte after its end marker.
    pub start: usize,
    pub end: usize,
}

impl OptionList {
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
                if !(0..=32).contains(&n) {
                    return Err(Error::Corrupt { offset: at, what: "option argument count out of range" });
                }
                for _ in 0..n {
                    let tag = r.u16()?;
                    let size = arg_size(tag & 0x7FFF).ok_or_else(|| r.corrupt("unknown typed-argument tag"))?;
                    let count = if tag & 0x8000 != 0 { r.u8()? as usize } else { 1 };
                    r.skip_n(count, size)?;
                }
            } else if entry.value_type == VALUE_TYPE_U16_LIST {
                // A `u16` count, then that many `u16` (object-stream.md 1.2).
                let n = r.u16()? as usize;
                r.skip(n * 2)?;
            } else {
                let size = value_size(entry.value_type).ok_or(Error::UnknownOption { offset: at, id })?;
                r.skip(size)?;
            }
            options.push(Opt { id, value: (vstart as u32, (r.pos() - vstart) as u32) });
        }
        Ok(OptionList { options, start, end: r.pos() })
    }

    pub fn has(&self, id: i16) -> bool {
        self.options.iter().any(|o| o.id == id)
    }

    /// The value bytes of the first option `id`.
    pub fn value<'a>(&self, data: &'a [u8], id: i16) -> Option<&'a [u8]> {
        self.options
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| data.get(o.value.0 as usize..(o.value.0 + o.value.1) as usize))
    }

    /// A 4-byte value of option `id` as `u32`.
    pub fn u32_value(&self, data: &[u8], id: i16) -> Option<u32> {
        self.value(data, id).and_then(|v| v.try_into().ok()).map(u32::from_le_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exe::{OPTION_ENTRY_SIZE, OptionTable};

    /// Option 0: a float (type 1); option 1: a vector (type 2); option 2: typed list of 2 arguments; option 3: an
    /// unknown value type.
    fn table() -> OptionTable {
        let mut raw = alloc::vec![0u8; 4 * OPTION_ENTRY_SIZE];
        raw[0..4].copy_from_slice(&1u32.to_le_bytes());
        raw[40..44].copy_from_slice(&2u32.to_le_bytes());
        raw[80 + 4..80 + 8].copy_from_slice(&0x400u32.to_le_bytes());
        raw[80 + 24..80 + 28].copy_from_slice(&2i32.to_le_bytes());
        raw[120..124].copy_from_slice(&3u32.to_le_bytes());
        OptionTable::from_raw(&raw)
    }

    #[test]
    fn parse() {
        let mut d = alloc::vec![];
        d.extend_from_slice(&1i16.to_le_bytes());
        d.extend_from_slice(&[0u8; 12]);
        d.extend_from_slice(&0i16.to_le_bytes());
        d.extend_from_slice(&5u32.to_le_bytes());
        d.extend_from_slice(&2i16.to_le_bytes());
        d.extend_from_slice(&6u16.to_le_bytes()); // one int
        d.extend_from_slice(&7u32.to_le_bytes());
        d.extend_from_slice(&(0x8000u16 | 0x1010).to_le_bytes()); // a list of 2 enum values
        d.push(2);
        d.extend_from_slice(&[1, 0, 0, 0, 2, 0, 0, 0]);
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.push(0xAB);
        let t = table();
        let mut r = Reader::new(&d);
        let l = OptionList::read(&mut r, &t).unwrap();
        assert_eq!(l.options.iter().map(|o| o.id).collect::<Vec<_>>(), [1, 0, 2]);
        assert_eq!(l.end, d.len() - 1);
        assert_eq!(l.u32_value(&d, 0), Some(5));
        assert!(l.has(2) && !l.has(3));

        let bad = [3u8, 0, 0, 0];
        assert!(OptionList::read(&mut Reader::new(&bad), &t).is_err());
        let unknown = [9u8, 0];
        assert!(matches!(OptionList::read(&mut Reader::new(&unknown), &t), Err(Error::UnknownOption { .. })));
    }
}
