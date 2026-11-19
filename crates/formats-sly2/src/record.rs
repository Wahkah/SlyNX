//! The 64-byte file record (disc.md 4): start sector, size and identity, each recovered by XOR of two words.

use formats::iso::SECTOR;

/// Sector added to every decoded start sector (disc.md 4).
pub const SECTOR_BASE: u32 = 0xCA0;

/// A decoded file record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRecord {
    /// First sector of the file.
    pub sector: u32,
    /// Size in bytes (compressed size for worlds); 0 means "no file".
    pub size: u32,
    /// The file's identity (a world's world id).
    pub identity: u32,
}

impl FileRecord {
    pub const SIZE: usize = 64;

    /// Decodes 64 bytes: sector `(w0 ^ w13) + 0xCA0`, size `w1 ^ w15`, identity `w2 ^ w14`.
    pub fn decode(raw: &[u8]) -> FileRecord {
        let w = |i: usize| u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap());
        FileRecord { sector: (w(0) ^ w(13)).wrapping_add(SECTOR_BASE), size: w(1) ^ w(15), identity: w(2) ^ w(14) }
    }

    /// No file (size 0).
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Byte offset of the file in the disc image.
    pub fn offset(&self) -> u64 {
        self.sector as u64 * SECTOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding() {
        let mut raw = [0u8; 64];
        let put = |raw: &mut [u8; 64], i: usize, v: u32| raw[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut raw, 0, 0x1234_5678);
        put(&mut raw, 13, 0x1234_5678 ^ (335_488 - SECTOR_BASE));
        put(&mut raw, 1, 0xAAAA_AAAA);
        put(&mut raw, 15, 0xAAAA_AAAA ^ 9_969_830);
        put(&mut raw, 2, 0x5555_0000);
        put(&mut raw, 14, 0x5555_0000 ^ 0x5D);
        put(&mut raw, 7, 0xFFFF_FFFF); // filler words carry nothing
        let r = FileRecord::decode(&raw);
        assert_eq!(r, FileRecord { sector: 335_488, size: 9_969_830, identity: 0x5D });
        assert_eq!(r.offset(), 335_488 * 2048);
        assert!(!r.is_empty());
        let empty = FileRecord::decode(&[0u8; 64]);
        assert!(empty.is_empty());
        assert_eq!(empty.sector, SECTOR_BASE);
    }
}
