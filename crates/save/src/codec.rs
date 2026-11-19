//! SlyNX's save file format: a checked, versioned container of tagged sections.
//!
//! ```text
//! offset size
//!  0      8  magic: "SLYNXSV\0" (a save slot) or "SLYNXCF\0" (the port's settings)
//!  8      2  format version of the writer
//! 10      2  oldest reader version that can read this file
//! 12      1  game (1 = Sly 1)
//! 13      3  zero
//! 16      4  payload length
//! 20      4  CRC-32 (IEEE) of bytes 0..20 followed by the payload
//! 24      n  payload: sections, each u16 tag, u32 length, then that many bytes
//! ```
//!
//! All integers are little-endian, floats IEEE 754.
//!
//! Forward compatibility: a reader skips sections whose tag it does not know, reads only the fields it knows from
//! the start of each section or record (records carry their own length), and ignores the rest. A newer writer that
//! adds fields or sections leaves the "oldest reader" field alone, so older SlyNX builds still load the file; it raises
//! that field only for a change older readers would get wrong. Fields missing from an older file keep their
//! new-game defaults and the file is upgraded on its next save.

use alloc::vec::Vec;

pub const SLOT_MAGIC: &[u8; 8] = b"SLYNXSV\0";
pub const SETTINGS_MAGIC: &[u8; 8] = b"SLYNXCF\0";
pub const HEADER_SIZE: usize = 24;
pub const GAME_SLY1: u8 = 1;
/// Largest payload accepted (a slot is about 4 KiB; this guards against absurd lengths).
pub const MAX_PAYLOAD: usize = 1 << 20;

/// Why a file could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Not a SlyNX file of the expected kind.
    BadMagic,
    /// Shorter than its header says, or a section runs past the end.
    Truncated,
    /// The checksum does not match: the file is damaged.
    Checksum,
    /// For another game.
    WrongGame(u8),
    /// Written by a newer SlyNX whose format this build cannot read.
    TooNew { version: u16, needs_reader: u16 },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::BadMagic => f.write_str("not a SlyNX save file"),
            DecodeError::Truncated => f.write_str("the file is cut short"),
            DecodeError::Checksum => f.write_str("the file is damaged (checksum mismatch)"),
            DecodeError::WrongGame(g) => write!(f, "the file is for another game ({g})"),
            DecodeError::TooNew { version, needs_reader } => {
                write!(f, "the file was written by a newer SlyNX (format {version}, needs reader {needs_reader})")
            }
        }
    }
}

/// CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320), bitwise: save files are small.
pub fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = !0u32;
    for part in parts {
        for &b in *part {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
    }
    !crc
}

/// Builds a payload.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Writes a section: `body` fills it; the length is patched in afterwards.
    pub fn section(&mut self, tag: u16, body: impl FnOnce(&mut Writer)) {
        self.u16(tag);
        let at = self.buf.len();
        self.u32(0);
        body(self);
        let len = (self.buf.len() - at - 4) as u32;
        self.buf[at..at + 4].copy_from_slice(&len.to_le_bytes());
    }

    /// Writes a record prefixed with its u16 length.
    pub fn record(&mut self, body: impl FnOnce(&mut Writer)) {
        let at = self.buf.len();
        self.u16(0);
        body(self);
        let len = (self.buf.len() - at - 2) as u16;
        self.buf[at..at + 2].copy_from_slice(&len.to_le_bytes());
    }
}

/// Reads fields from a section or record. Reading past its end yields `None`, so a field an older writer did not
/// write falls back to its default.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

macro_rules! read_le {
    ($name:ident, $t:ty) => {
        pub fn $name(&mut self) -> Option<$t> {
            const N: usize = core::mem::size_of::<$t>();
            let b = self.data.get(self.pos..self.pos + N)?;
            self.pos += N;
            Some(<$t>::from_le_bytes(b.try_into().ok()?))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    read_le!(u8, u8);
    read_le!(i8, i8);
    read_le!(u16, u16);
    read_le!(u32, u32);
    read_le!(i32, i32);
    read_le!(u64, u64);
    read_le!(f32, f32);
    read_le!(f64, f64);

    /// A record written by [`Writer::record`]: its bytes (whatever length the writer gave it).
    pub fn record(&mut self) -> Option<&'a [u8]> {
        let len = self.u16()? as usize;
        let r = self.data.get(self.pos..self.pos + len)?;
        self.pos += len;
        Some(r)
    }
}

/// Wraps a payload in the container.
pub fn seal(magic: &[u8; 8], version: u16, oldest_reader: u16, game: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
    out.extend_from_slice(magic);
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&oldest_reader.to_le_bytes());
    out.extend_from_slice(&[game, 0, 0, 0]);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    let crc = crc32(&[&out[..20], payload]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// A checked container.
pub struct Opened<'a> {
    pub version: u16,
    pub oldest_reader: u16,
    pub game: u8,
    pub payload: &'a [u8],
}

/// Checks magic, length, checksum and reader version, and returns the payload.
pub fn open<'a>(magic: &[u8; 8], reader_version: u16, game: u8, data: &'a [u8]) -> Result<Opened<'a>, DecodeError> {
    if data.len() < HEADER_SIZE || &data[..8] != magic {
        return Err(if data.len() >= 8 && &data[..8] != magic {
            DecodeError::BadMagic
        } else {
            DecodeError::Truncated
        });
    }
    let u16_at = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    let len = u32_at(16) as usize;
    if len > MAX_PAYLOAD || data.len() < HEADER_SIZE + len {
        return Err(DecodeError::Truncated);
    }
    let payload = &data[HEADER_SIZE..HEADER_SIZE + len];
    if crc32(&[&data[..20], payload]) != u32_at(20) {
        return Err(DecodeError::Checksum);
    }
    let (version, oldest_reader) = (u16_at(8), u16_at(10));
    if oldest_reader > reader_version {
        return Err(DecodeError::TooNew { version, needs_reader: oldest_reader });
    }
    if data[12] != game {
        return Err(DecodeError::WrongGame(data[12]));
    }
    Ok(Opened { version, oldest_reader, game: data[12], payload })
}

/// Iterates the payload's sections as `(tag, body)`.
pub fn sections(payload: &[u8]) -> impl Iterator<Item = Result<(u16, &[u8]), DecodeError>> {
    let mut pos = 0usize;
    core::iter::from_fn(move || {
        if pos >= payload.len() {
            return None;
        }
        let head = payload.get(pos..pos + 6);
        let Some(head) = head else {
            pos = payload.len();
            return Some(Err(DecodeError::Truncated));
        };
        let tag = u16::from_le_bytes([head[0], head[1]]);
        let len = u32::from_le_bytes(head[2..6].try_into().unwrap()) as usize;
        let body = payload.get(pos + 6..pos + 6 + len);
        pos = match body {
            Some(_) => pos + 6 + len,
            None => payload.len(),
        };
        Some(body.map(|b| (tag, b)).ok_or(DecodeError::Truncated))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_value() {
        // The standard check value of CRC-32/IEEE.
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
    }

    #[test]
    fn container() {
        let mut w = Writer::default();
        w.section(1, |w| w.u32(7));
        w.section(9, |w| w.record(|w| w.f32(1.5)));
        let file = seal(SLOT_MAGIC, 1, 1, GAME_SLY1, &w.buf);
        let o = open(SLOT_MAGIC, 1, GAME_SLY1, &file).unwrap();
        assert_eq!((o.version, o.oldest_reader, o.game), (1, 1, 1));
        let s: Vec<_> = sections(o.payload).collect::<Result<_, _>>().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(Reader::new(s[0].1).u32(), Some(7));
        let mut r = Reader::new(s[1].1);
        let rec = r.record().unwrap();
        assert_eq!(Reader::new(rec).f32(), Some(1.5));
        assert_eq!(Reader::new(rec).f64(), None, "past the record's end");

        assert_eq!(open(SETTINGS_MAGIC, 1, 1, &file).err(), Some(DecodeError::BadMagic));
        assert_eq!(open(SLOT_MAGIC, 1, 2, &file).err(), Some(DecodeError::WrongGame(1)));
        assert_eq!(open(SLOT_MAGIC, 1, 1, &file[..file.len() - 1]).err(), Some(DecodeError::Truncated));
        assert_eq!(open(SLOT_MAGIC, 1, 1, &file[..10]).err(), Some(DecodeError::Truncated));
        let newer = seal(SLOT_MAGIC, 5, 3, GAME_SLY1, &w.buf);
        assert_eq!(open(SLOT_MAGIC, 2, 1, &newer).err(), Some(DecodeError::TooNew { version: 5, needs_reader: 3 }));
        assert!(open(SLOT_MAGIC, 3, 1, &newer).is_ok());
    }

    #[test]
    fn every_single_bit_flip_is_detected() {
        let mut w = Writer::default();
        w.section(1, |w| {
            for i in 0..40u32 {
                w.u32(i * 0x0101_0101);
            }
        });
        let file = seal(SLOT_MAGIC, 1, 1, GAME_SLY1, &w.buf);
        for byte in 0..file.len() {
            for bit in 0..8 {
                let mut f = file.clone();
                f[byte] ^= 1 << bit;
                assert!(open(SLOT_MAGIC, 1, GAME_SLY1, &f).is_err(), "flip at byte {byte} bit {bit}");
            }
        }
    }
}
