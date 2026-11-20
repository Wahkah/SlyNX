//! The original Sly 1 memory-card record's frame (`specs/sly1/save-format.md` 6): the header, the check word, the
//! load-time refusals and a new record's starting values. SlyNX saves in its own format ([`crate::slotfile`]); this
//! is the groundwork for importing an original card's `save1` to `save3`, whose field layout past the header is
//! not confirmed yet.
//!
//! | Offset | Value |
//! |---:|---|
//! | 0 | version, 18 |
//! | 4 | record size, 6,656 |
//! | 8 | check word: the sum of the bytes from offset 12 to the end, each unsigned, in a 32-bit integer |
//!
//! Loading refuses a slot whose file size is wrong (error 8), whose version or size is wrong (error 28), or whose
//! check word does not match the recomputed sum (error 27). A new record is zero-filled, then gets the header and
//! the starting values: offset 6,620 = 3, 6,624 = 5 (the starting lives), 6,652 = −1 ("no power-up"), and 6,616
//! = 6,640 = 0.

use alloc::vec;
use alloc::vec::Vec;

/// The record's version.
pub const VERSION: u32 = 18;
/// The record's size, which is also the slot file's size.
pub const RECORD_SIZE: usize = 6656;
/// Where the check word is, and where the summed bytes start.
pub const CHECK_AT: usize = 8;
pub const SUM_FROM: usize = 12;
/// A new record's starting values: (offset, value).
pub const NEW_RECORD: [(usize, i32); 5] = [(6620, 3), (6624, 5), (6652, -1), (6616, 0), (6640, 0)];
/// Offset of the starting lives and of the "no power-up" word.
pub const LIVES_AT: usize = 6624;
pub const POWER_UP_AT: usize = 6652;

/// Why a slot is refused, with the game's error numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardError {
    /// The file is not 6,656 bytes (error 8).
    Size,
    /// The version is not 18 or the record size not 6,656 (error 28).
    Header,
    /// The check word differs from the recomputed sum (error 27).
    Check,
}

impl CardError {
    /// The game's error number.
    pub fn code(self) -> u32 {
        match self {
            CardError::Size => 8,
            CardError::Header => 28,
            CardError::Check => 27,
        }
    }
}

fn word(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

/// The check word of `record`: the unsigned bytes from offset 12 to the end summed into 32 bits.
pub fn check_word(record: &[u8]) -> u32 {
    record.get(SUM_FROM..).unwrap_or(&[]).iter().fold(0u32, |s, &b| s.wrapping_add(b as u32))
}

/// Checks a slot file as the game does on loading.
pub fn validate(record: &[u8]) -> Result<(), CardError> {
    if record.len() != RECORD_SIZE {
        return Err(CardError::Size);
    }
    if word(record, 0) != VERSION || word(record, 4) as usize != RECORD_SIZE {
        return Err(CardError::Header);
    }
    if word(record, CHECK_AT) != check_word(record) {
        return Err(CardError::Check);
    }
    Ok(())
}

/// Writes the header and the check word into `record` (6,656 bytes), as the game does before saving.
pub fn seal(record: &mut [u8]) {
    record[0..4].copy_from_slice(&VERSION.to_le_bytes());
    record[4..8].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes());
    let sum = check_word(record);
    record[CHECK_AT..CHECK_AT + 4].copy_from_slice(&sum.to_le_bytes());
}

/// A new record: zero-filled, the starting values, the header and its check word.
pub fn new_record() -> Vec<u8> {
    let mut r = vec![0u8; RECORD_SIZE];
    for (at, v) in NEW_RECORD {
        r[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    seal(&mut r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_record_passes_and_holds_its_starting_values() {
        let r = new_record();
        assert_eq!(r.len(), 6656);
        assert_eq!(validate(&r), Ok(()));
        assert_eq!((word(&r, 0), word(&r, 4)), (18, 6656));
        assert_eq!(word(&r, LIVES_AT), 5);
        assert_eq!(word(&r, POWER_UP_AT) as i32, -1);
        assert_eq!(word(&r, 6620), 3);
        // The check word is the byte sum from offset 12: 3 + 5 + 4 × 0xFF.
        assert_eq!(word(&r, CHECK_AT), 3 + 5 + 4 * 255);
    }

    #[test]
    fn loading_refuses_size_header_and_check_errors() {
        let r = new_record();
        assert_eq!(validate(&r[..6655]).map_err(CardError::code), Err(8));
        let mut v = r.clone();
        v[0] = 17;
        assert_eq!(validate(&v).map_err(CardError::code), Err(28));
        let mut s = r.clone();
        s[4..8].copy_from_slice(&6655u32.to_le_bytes());
        assert_eq!(validate(&s).map_err(CardError::code), Err(28));
        let mut c = r.clone();
        c[100] ^= 1;
        assert_eq!(validate(&c).map_err(CardError::code), Err(27));
        // Bytes before offset 12 are not summed; resealing fixes the check word.
        seal(&mut c);
        assert_eq!(validate(&c), Ok(()));
    }

    #[test]
    fn the_sum_is_unsigned_and_32_bit() {
        let mut r = vec![0xFFu8; RECORD_SIZE];
        seal(&mut r);
        assert_eq!(word(&r, CHECK_AT), 255 * (RECORD_SIZE as u32 - 12));
        assert_eq!(validate(&r), Ok(()));
    }
}
