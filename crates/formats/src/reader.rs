//! A bounds-checked little-endian cursor over a byte slice.
//!
//! Positions are absolute offsets into the slice the reader was made from. World-file readers always work on the
//! whole decompressed file so that offsets and the alignment rules (which count from the file start) are direct.

use crate::error::{Error, Result};

/// Three `f32`: x, y, z.
pub type Vec3 = [f32; 3];

/// A cursor over `data`, reading little-endian values with no alignment requirement.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// A reader at offset 0.
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    /// A reader at offset `pos`.
    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }

    /// The whole slice.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The current offset.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Moves to `pos` (checked on the next read).
    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Bytes left after the current offset.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn eof(&self, needed: usize) -> Error {
        Error::Eof { offset: self.pos, needed, len: self.data.len() }
    }

    /// A [`Error::Corrupt`] at the current offset.
    pub fn corrupt(&self, what: &'static str) -> Error {
        Error::Corrupt { offset: self.pos, what }
    }

    /// Takes the next `n` bytes.
    #[inline]
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(|| self.eof(n))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    /// Takes the next `N` bytes as an array.
    #[inline]
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let s = self.bytes(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    /// Skips `n` bytes.
    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    /// Skips `count * size` bytes, checking the multiplication.
    #[inline]
    pub fn skip_n(&mut self, count: usize, size: usize) -> Result<()> {
        let n = count.checked_mul(size).ok_or_else(|| self.eof(usize::MAX))?;
        self.skip(n)
    }

    /// Reads a `u8`.
    #[inline]
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    /// Reads an `s8`.
    #[inline]
    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    /// Reads a `u16`.
    #[inline]
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    /// Reads an `s16`.
    #[inline]
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    /// Reads a `u32`.
    #[inline]
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    /// Reads an `s32`.
    #[inline]
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    /// Reads an `f32`.
    #[inline]
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// Three `f32`.
    #[inline]
    pub fn vec3(&mut self) -> Result<Vec3> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// Three `vec3` (the columns of a 3 x 3 matrix).
    pub fn mat3(&mut self) -> Result<[Vec3; 3]> {
        Ok([self.vec3()?, self.vec3()?, self.vec3()?])
    }

    /// A length-prefixed string: `u16 n`, then `n` bytes (returned without the prefix).
    pub fn string(&mut self) -> Result<&'a [u8]> {
        let n = self.u16()? as usize;
        self.bytes(n)
    }

    /// Skips zero bytes up to the next multiple of `align` (counted from offset 0 of the slice) and returns how many
    /// were skipped. Fails if a padding byte is not zero.
    pub fn align_zero(&mut self, align: usize) -> Result<usize> {
        let pad = (align - self.pos % align) % align;
        let at = self.pos;
        let s = self.bytes(pad)?;
        if s.iter().any(|&b| b != 0) {
            return Err(Error::Corrupt { offset: at, what: "non-zero alignment padding" });
        }
        Ok(pad)
    }
}

/// Reads a little-endian `u32` at `off` of `data`.
pub fn le_u32(data: &[u8], off: usize) -> Result<u32> {
    Reader::at(data, off).u32()
}

/// Reads a little-endian `u16` at `off` of `data`.
pub fn le_u16(data: &[u8], off: usize) -> Result<u16> {
    Reader::at(data, off).u16()
}

/// Reads a little-endian `f32` at `off` of `data`.
pub fn le_f32(data: &[u8], off: usize) -> Result<f32> {
    Reader::at(data, off).f32()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_and_checks_bounds() {
        let d = [1u8, 2, 3, 4, 0, 0, 0x80, 0x3f, 0xff];
        let mut r = Reader::new(&d);
        assert_eq!(r.u16().unwrap(), 0x0201);
        assert_eq!(r.i16().unwrap(), 0x0403);
        assert_eq!(r.f32().unwrap(), 1.0);
        assert_eq!(r.i8().unwrap(), -1);
        assert!(matches!(r.u8(), Err(Error::Eof { offset: 9, needed: 1, .. })));
    }

    #[test]
    fn alignment_counts_from_slice_start() {
        let d = [7u8, 0, 0, 0, 9];
        let mut r = Reader::at(&d, 1);
        assert_eq!(r.align_zero(4).unwrap(), 3);
        assert_eq!(r.pos(), 4);
        assert_eq!(r.align_zero(4).unwrap(), 0);
        let bad = [0u8, 1, 0, 0];
        assert!(Reader::at(&bad, 1).align_zero(4).is_err());
    }

    #[test]
    fn strings_are_length_prefixed() {
        let d = [3u8, 0, b'a', b'b', b'c', 0, 0];
        let mut r = Reader::new(&d);
        assert_eq!(r.string().unwrap(), b"abc");
        assert_eq!(r.string().unwrap(), b"");
    }
}
