//! VAG files (audio.md 2.2): a 48-byte big-endian header, then mono SPU ADPCM. Used for dialogue lines, the ambient
//! loops, the prompt sound and the developer commentary.

use alloc::vec::Vec;

use crate::Error;

/// Size of the header.
pub const HEADER_SIZE: usize = 48;

/// The header fields the game uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VagHeader {
    /// Version (`0x20` on every file).
    pub version: u32,
    /// Size of the ADPCM data in bytes.
    pub data_size: u32,
    /// Sample rate in Hz (22,050 or 44,100 on the disc).
    pub sample_rate: u32,
    /// The name field, NUL-padded.
    pub name: [u8; 16],
}

fn be_u32(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

impl VagHeader {
    /// Parses the header at the start of `bytes`.
    pub fn parse(bytes: &[u8]) -> Result<VagHeader, Error> {
        if bytes.len() < HEADER_SIZE {
            return Err(Error::Truncated { what: "VAG header", needed: HEADER_SIZE, len: bytes.len() });
        }
        if &bytes[0..4] != b"VAGp" {
            return Err(Error::BadMagic("VAGp"));
        }
        let mut name = [0u8; 16];
        name.copy_from_slice(&bytes[32..48]);
        let h =
            VagHeader { version: be_u32(bytes, 4), data_size: be_u32(bytes, 12), sample_rate: be_u32(bytes, 16), name };
        if h.sample_rate == 0 || h.sample_rate > 192_000 {
            return Err(Error::Invalid("VAG sample rate"));
        }
        Ok(h)
    }

    /// The name up to the first NUL.
    pub fn name(&self) -> &[u8] {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(16);
        &self.name[..end]
    }

    /// Total file size implied by the header (header plus data).
    pub fn file_size(&self) -> usize {
        HEADER_SIZE + self.data_size as usize
    }
}

/// A whole VAG file held in memory, ready to be played by the mixer (it decodes while playing).
#[derive(Clone, Debug)]
pub struct VagSound {
    /// The header.
    pub header: VagHeader,
    /// The whole file (header included); the ADPCM data starts at [`HEADER_SIZE`].
    pub bytes: Vec<u8>,
}

impl VagSound {
    /// Takes ownership of a VAG file's bytes. Data past the header's size is ignored; a file shorter than its header
    /// claims is accepted (the voice stops where the data does).
    pub fn new(bytes: Vec<u8>) -> Result<VagSound, Error> {
        let header = VagHeader::parse(&bytes)?;
        Ok(VagSound { header, bytes })
    }

    /// The ADPCM data.
    pub fn adpcm(&self) -> &[u8] {
        let end = self.header.file_size().min(self.bytes.len());
        &self.bytes[HEADER_SIZE..end]
    }

    /// Length in samples of one pass to the end flag.
    pub fn sample_count(&self) -> usize {
        let data = self.adpcm();
        crate::adpcm::sample_len(data, 0).unwrap_or(data.len() / 16 * 16) / 16 * 28
    }

    /// Duration in seconds of one pass.
    pub fn duration(&self) -> f32 {
        self.sample_count() as f32 / self.header.sample_rate as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    pub(crate) fn vag_bytes(rate: u32, data: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; HEADER_SIZE];
        v[0..4].copy_from_slice(b"VAGp");
        v[4..8].copy_from_slice(&0x20u32.to_be_bytes());
        v[12..16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        v[16..20].copy_from_slice(&rate.to_be_bytes());
        v[32..35].copy_from_slice(b"dap");
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn parses_header() {
        let mut data = vec![0u8; 32];
        data[17] = 1;
        let s = VagSound::new(vag_bytes(22_050, &data)).unwrap();
        assert_eq!(s.header.version, 0x20);
        assert_eq!(s.header.sample_rate, 22_050);
        assert_eq!(s.header.data_size, 32);
        assert_eq!(s.header.name(), b"dap");
        assert_eq!(s.adpcm().len(), 32);
        assert_eq!(s.sample_count(), 56);
        assert!(VagHeader::parse(&[0; 47]).is_err());
        let mut bad = vag_bytes(22_050, &data);
        bad[0] = b'X';
        assert!(VagSound::new(bad).is_err());
    }
}
