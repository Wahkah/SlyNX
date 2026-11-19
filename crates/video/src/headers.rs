//! Sequence, GOP and picture headers and their extensions (ISO/IEC 13818-2, 6.2 and 6.3).

use crate::Error;
use crate::bits::BitReader;
use crate::tables::{DEFAULT_INTRA_MATRIX, DEFAULT_NON_INTRA_MATRIX, SCAN};

/// Start code values (the byte after `00 00 01`).
pub mod code {
    pub const PICTURE: u8 = 0x00;
    pub const SLICE_FIRST: u8 = 0x01;
    pub const SLICE_LAST: u8 = 0xAF;
    pub const USER_DATA: u8 = 0xB2;
    pub const SEQUENCE_HEADER: u8 = 0xB3;
    pub const SEQUENCE_ERROR: u8 = 0xB4;
    pub const EXTENSION: u8 = 0xB5;
    pub const SEQUENCE_END: u8 = 0xB7;
    pub const GROUP: u8 = 0xB8;
}

/// Extension identifiers (Table 6-2).
mod ext {
    pub const SEQUENCE: u32 = 1;
    pub const SEQUENCE_DISPLAY: u32 = 2;
    pub const QUANT_MATRIX: u32 = 3;
    pub const PICTURE_CODING: u32 = 8;
}

/// `sequence_header()` together with `sequence_extension()` and `sequence_display_extension()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceHeader {
    /// Display width and height in samples (horizontal_size and vertical_size with their extensions).
    pub width: u32,
    pub height: u32,
    pub aspect_ratio_information: u8,
    pub frame_rate_code: u8,
    pub frame_rate_extension_n: u8,
    pub frame_rate_extension_d: u8,
    /// bit_rate in units of 400 bit/s (value and extension combined).
    pub bit_rate: u32,
    /// vbv_buffer_size in units of 16 kbit (value and extension combined).
    pub vbv_buffer_size: u32,
    pub constrained_parameters_flag: bool,
    /// Whether a sequence_extension was present (MPEG-2). MPEG-1 streams are not supported.
    pub mpeg2: bool,
    pub profile_and_level: u8,
    pub progressive_sequence: bool,
    /// chroma_format: 1 = 4:2:0 (the only one supported), 2 = 4:2:2, 3 = 4:4:4.
    pub chroma_format: u8,
    pub low_delay: bool,
    /// matrix_coefficients from the sequence display extension (5 when absent, i.e. ITU-R BT.601).
    pub matrix_coefficients: u8,
    /// Quantiser matrices in raster order, as loaded by the sequence header (defaults otherwise).
    pub intra_matrix: [u8; 64],
    pub non_intra_matrix: [u8; 64],
}

impl SequenceHeader {
    /// Parses `sequence_header()`; `br` starts after the start code.
    pub(crate) fn parse(br: &mut BitReader<'_>) -> Result<SequenceHeader, Error> {
        let width = br.read(12);
        let height = br.read(12);
        let aspect = br.read(4) as u8;
        let frame_rate_code = br.read(4) as u8;
        let bit_rate = br.read(18);
        if !br.bit() {
            return Err(Error::Bitstream("sequence header marker bit"));
        }
        let vbv = br.read(10);
        let constrained = br.bit();
        let intra_matrix = if br.bit() { read_matrix(br) } else { DEFAULT_INTRA_MATRIX };
        let non_intra_matrix = if br.bit() { read_matrix(br) } else { DEFAULT_NON_INTRA_MATRIX };
        if br.overrun() {
            return Err(Error::Bitstream("truncated sequence header"));
        }
        if width == 0 || height == 0 || frame_rate_code == 0 || frame_rate_code > 8 {
            return Err(Error::Bitstream("sequence header values out of range"));
        }
        Ok(SequenceHeader {
            width,
            height,
            aspect_ratio_information: aspect,
            frame_rate_code,
            frame_rate_extension_n: 0,
            frame_rate_extension_d: 0,
            bit_rate,
            vbv_buffer_size: vbv,
            constrained_parameters_flag: constrained,
            mpeg2: false,
            profile_and_level: 0,
            progressive_sequence: true,
            chroma_format: 1,
            low_delay: false,
            matrix_coefficients: 5,
            intra_matrix,
            non_intra_matrix,
        })
    }

    /// Frame period as a fraction of a second: (numerator, denominator).
    pub fn frame_period(&self) -> (u64, u64) {
        let (num, den): (u64, u64) = match self.frame_rate_code {
            1 => (24000, 1001),
            2 => (24, 1),
            3 => (25, 1),
            4 => (30000, 1001),
            5 => (30, 1),
            6 => (50, 1),
            7 => (60000, 1001),
            _ => (60, 1),
        };
        // frame_rate = frame_rate_value * (n + 1) / (d + 1)
        let n = u64::from(self.frame_rate_extension_n) + 1;
        let d = u64::from(self.frame_rate_extension_d) + 1;
        (den * d, num * n)
    }

    /// Width and height in macroblocks of a frame.
    pub fn mb_size(&self) -> (usize, usize) {
        let w = (self.width as usize).div_ceil(16);
        // Interlaced sequences round the height up to a whole number of field macroblock rows.
        let h = if self.progressive_sequence {
            (self.height as usize).div_ceil(16)
        } else {
            2 * (self.height as usize).div_ceil(32)
        };
        (w, h)
    }

    fn parse_extension(&mut self, br: &mut BitReader<'_>) -> Result<(), Error> {
        self.profile_and_level = br.read(8) as u8;
        self.progressive_sequence = br.bit();
        self.chroma_format = br.read(2) as u8;
        self.width |= br.read(2) << 12;
        self.height |= br.read(2) << 12;
        self.bit_rate |= br.read(12) << 18;
        if !br.bit() {
            return Err(Error::Bitstream("sequence extension marker bit"));
        }
        self.vbv_buffer_size |= br.read(8) << 10;
        self.low_delay = br.bit();
        self.frame_rate_extension_n = br.read(2) as u8;
        self.frame_rate_extension_d = br.read(5) as u8;
        self.mpeg2 = true;
        Ok(())
    }

    fn parse_display_extension(&mut self, br: &mut BitReader<'_>) {
        let _video_format = br.read(3);
        if br.bit() {
            let _primaries = br.read(8);
            let _transfer = br.read(8);
            self.matrix_coefficients = br.read(8) as u8;
        }
        // display_horizontal_size, marker, display_vertical_size: not needed.
    }
}

/// Reads a quantiser matrix (64 bytes in zigzag order) into raster order.
fn read_matrix(br: &mut BitReader<'_>) -> [u8; 64] {
    let mut m = [0u8; 64];
    for &pos in &SCAN[0] {
        m[pos as usize] = br.read(8) as u8;
    }
    m
}

/// `group_of_pictures_header()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GopHeader {
    pub time_code: u32,
    pub closed_gop: bool,
    pub broken_link: bool,
}

impl GopHeader {
    pub(crate) fn parse(br: &mut BitReader<'_>) -> GopHeader {
        let time_code = br.read(25);
        let closed_gop = br.bit();
        let broken_link = br.bit();
        GopHeader { time_code, closed_gop, broken_link }
    }
}

/// picture_coding_type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PictureType {
    #[default]
    I = 1,
    P = 2,
    B = 3,
}

/// picture_structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PictureStructure {
    TopField = 1,
    BottomField = 2,
    #[default]
    Frame = 3,
}

/// `picture_header()` with `picture_coding_extension()` and the quantiser matrices in force for the picture.
#[derive(Clone, Debug)]
pub struct PictureHeader {
    pub temporal_reference: u16,
    pub picture_type: PictureType,
    /// f_code[s][t]: s = 0 forward, 1 backward; t = 0 horizontal, 1 vertical.
    pub f_code: [[u8; 2]; 2],
    pub intra_dc_precision: u8,
    pub structure: PictureStructure,
    pub top_field_first: bool,
    pub frame_pred_frame_dct: bool,
    pub concealment_motion_vectors: bool,
    pub q_scale_type: bool,
    pub intra_vlc_format: bool,
    pub alternate_scan: bool,
    pub repeat_first_field: bool,
    pub progressive_frame: bool,
    /// Whether a picture_coding_extension was present.
    pub has_coding_extension: bool,
}

impl PictureHeader {
    /// Parses `picture_header()`; `br` starts after the start code.
    pub(crate) fn parse(br: &mut BitReader<'_>) -> Result<PictureHeader, Error> {
        let temporal_reference = br.read(10) as u16;
        let picture_type = match br.read(3) {
            1 => PictureType::I,
            2 => PictureType::P,
            3 => PictureType::B,
            4 => return Err(Error::Unsupported("D pictures")),
            _ => return Err(Error::Bitstream("picture_coding_type")),
        };
        let _vbv_delay = br.read(16);
        if picture_type != PictureType::I {
            let _full_pel_forward = br.read(1);
            let _forward_f_code = br.read(3);
        }
        if picture_type == PictureType::B {
            let _full_pel_backward = br.read(1);
            let _backward_f_code = br.read(3);
        }
        while br.bit() {
            br.skip(8);
            if br.overrun() {
                return Err(Error::Bitstream("truncated picture header"));
            }
        }
        Ok(PictureHeader {
            temporal_reference,
            picture_type,
            f_code: [[15; 2]; 2],
            intra_dc_precision: 0,
            structure: PictureStructure::Frame,
            top_field_first: false,
            frame_pred_frame_dct: true,
            concealment_motion_vectors: false,
            q_scale_type: false,
            intra_vlc_format: false,
            alternate_scan: false,
            repeat_first_field: false,
            progressive_frame: true,
            has_coding_extension: false,
        })
    }

    fn parse_coding_extension(&mut self, br: &mut BitReader<'_>) -> Result<(), Error> {
        for code in self.f_code.as_flattened_mut() {
            *code = br.read(4) as u8;
        }
        self.intra_dc_precision = br.read(2) as u8;
        self.structure = match br.read(2) {
            1 => PictureStructure::TopField,
            2 => PictureStructure::BottomField,
            3 => PictureStructure::Frame,
            _ => return Err(Error::Bitstream("picture_structure")),
        };
        self.top_field_first = br.bit();
        self.frame_pred_frame_dct = br.bit();
        self.concealment_motion_vectors = br.bit();
        self.q_scale_type = br.bit();
        self.intra_vlc_format = br.bit();
        self.alternate_scan = br.bit();
        self.repeat_first_field = br.bit();
        let _chroma_420_type = br.bit();
        self.progressive_frame = br.bit();
        // composite display fields are not needed.
        self.has_coding_extension = true;
        Ok(())
    }

    /// Whether this is one field of a frame.
    pub fn is_field(&self) -> bool {
        self.structure != PictureStructure::Frame
    }
}

/// Quantiser matrices in force (sequence header, updated by quant_matrix_extension()).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantMatrices {
    pub intra: [u8; 64],
    pub non_intra: [u8; 64],
}

/// What an `extension_start_code` unit carried.
pub(crate) enum Extension {
    Sequence,
    SequenceDisplay,
    QuantMatrix,
    PictureCoding,
    Other,
}

/// Parses an extension unit (after the start code) into whichever header it extends.
pub(crate) fn parse_extension(
    br: &mut BitReader<'_>,
    seq: Option<&mut SequenceHeader>,
    pic: Option<&mut PictureHeader>,
    quant: Option<&mut QuantMatrices>,
) -> Result<Extension, Error> {
    let id = br.read(4);
    let kind = match id {
        ext::SEQUENCE => {
            let seq = seq.ok_or(Error::Bitstream("sequence extension without a sequence header"))?;
            seq.parse_extension(br)?;
            Extension::Sequence
        }
        ext::SEQUENCE_DISPLAY => {
            if let Some(seq) = seq {
                seq.parse_display_extension(br);
            }
            Extension::SequenceDisplay
        }
        ext::QUANT_MATRIX => {
            if let Some(q) = quant {
                if br.bit() {
                    q.intra = read_matrix(br);
                }
                if br.bit() {
                    q.non_intra = read_matrix(br);
                }
                // Chroma matrices only apply to 4:2:2 and 4:4:4.
            }
            Extension::QuantMatrix
        }
        ext::PICTURE_CODING => {
            let pic = pic.ok_or(Error::Bitstream("picture coding extension without a picture header"))?;
            pic.parse_coding_extension(br)?;
            Extension::PictureCoding
        }
        _ => Extension::Other,
    };
    if br.overrun() {
        return Err(Error::Bitstream("truncated extension"));
    }
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_periods() {
        let mut s = SequenceHeader {
            width: 640,
            height: 480,
            aspect_ratio_information: 2,
            frame_rate_code: 4,
            frame_rate_extension_n: 0,
            frame_rate_extension_d: 0,
            bit_rate: 12500,
            vbv_buffer_size: 112,
            constrained_parameters_flag: false,
            mpeg2: true,
            profile_and_level: 0x48,
            progressive_sequence: false,
            chroma_format: 1,
            low_delay: false,
            matrix_coefficients: 5,
            intra_matrix: DEFAULT_INTRA_MATRIX,
            non_intra_matrix: DEFAULT_NON_INTRA_MATRIX,
        };
        assert_eq!(s.frame_period(), (1001, 30000));
        assert_eq!(s.mb_size(), (40, 30));
        s.frame_rate_code = 3;
        assert_eq!(s.frame_period(), (1, 25));
        s.height = 470;
        assert_eq!(s.mb_size(), (40, 30));
    }
}
