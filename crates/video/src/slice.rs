//! Slice and macroblock decoding (ISO/IEC 13818-2, 6.2.4 to 6.2.6 and clause 7): macroblock modes, motion
//! vectors, coefficient decoding, inverse quantisation, IDCT, prediction and reconstruction.

use crate::Error;
use crate::bits::BitReader;
use crate::frame::Frame;
use crate::headers::{PictureHeader, PictureStructure, PictureType};
use crate::idct::{idct, idct_dc};
use crate::mc::{Dst, PlaneView, predict};
use crate::tables::QUANTISER_SCALE;
use crate::vlc::{self, DCT_EOB, DCT_ESCAPE, MB_ESCAPE, Tables, mb};

/// Motion types (frame_motion_type and field_motion_type share the codes).
const MT_FIELD: u8 = 1;
/// Frame prediction in frame pictures, 16 x 8 prediction in field pictures.
const MT_FRAME_OR_16X8: u8 = 2;
const MT_DUAL_PRIME: u8 = 3;

/// Counters kept while decoding, for tests and tuning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MbStats {
    pub macroblocks: u64,
    pub skipped: u64,
    pub intra: u64,
    pub field_motion: u64,
    pub dual_prime: u64,
    pub sixteen_by_eight: u64,
    pub field_dct: u64,
    pub bidirectional: u64,
    pub coded_blocks: u64,
    pub dc_only_blocks: u64,
    pub missing_reference: u64,
}

impl MbStats {
    pub fn add(&mut self, o: &MbStats) {
        self.macroblocks += o.macroblocks;
        self.skipped += o.skipped;
        self.intra += o.intra;
        self.field_motion += o.field_motion;
        self.dual_prime += o.dual_prime;
        self.sixteen_by_eight += o.sixteen_by_eight;
        self.field_dct += o.field_dct;
        self.bidirectional += o.bidirectional;
        self.coded_blocks += o.coded_blocks;
        self.dc_only_blocks += o.dc_only_blocks;
        self.missing_reference += o.missing_reference;
    }
}

/// Everything about the picture that slices need, read-only.
pub(crate) struct PicCtx<'a> {
    pub tables: &'a Tables,
    pub ph: &'a PictureHeader,
    /// Quantiser matrices in raster order.
    pub intra_matrix: &'a [u8; 64],
    pub non_intra_matrix: &'a [u8; 64],
    pub mb_width: usize,
    /// Macroblock rows in this picture (half the frame's for a field picture).
    pub mb_rows: usize,
    /// Field parity (0 top, 1 bottom) for a field picture.
    pub parity: Option<u8>,
    /// Whether this is the second field of a frame.
    pub second_field: bool,
    pub stride: usize,
    pub chroma_stride: usize,
    pub coded_height: usize,
}

/// Reference frames for prediction.
pub(crate) struct Refs<'a> {
    pub forward: Option<&'a Frame>,
    pub backward: Option<&'a Frame>,
    /// The first field of the frame being decoded (for the second field of a P frame coded as fields).
    pub first_field: Option<&'a Frame>,
}

/// Where reconstructed samples go: whole frame planes, or a band of luma lines starting at `line0` (a multiple of
/// 16) for a frame picture.
pub(crate) struct Target<'a> {
    pub y: &'a mut [u8],
    pub cb: &'a mut [u8],
    pub cr: &'a mut [u8],
    pub line0: usize,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    /// mb::FORWARD and mb::BACKWARD; mb::INTRA for an intra macroblock.
    flags: i16,
    mtype: u8,
    /// mv[r][s] = (horizontal, vertical) in half samples.
    mv: [[[i32; 2]; 2]; 2],
    mvfs: [[u8; 2]; 2],
    dmv: [i32; 2],
}

struct Pred {
    y: [u8; 256],
    cb: [u8; 64],
    cr: [u8; 64],
}

struct SliceState {
    qscale_code: u32,
    dc_pred: [i32; 3],
    pmv: [[[i32; 2]; 2]; 2],
    prev: Motion,
    /// W * quantiser_scale by scan index, intra and non-intra.
    wq_intra: [i32; 64],
    wq_non: [i32; 64],
}

impl SliceState {
    fn set_qscale(&mut self, ctx: &PicCtx<'_>, code: u32) -> Result<(), Error> {
        if code == 0 {
            return Err(Error::Bitstream("quantiser_scale_code 0"));
        }
        self.qscale_code = code;
        let qs = i32::from(QUANTISER_SCALE[usize::from(ctx.ph.q_scale_type)][code as usize]);
        let scan = scan_of(ctx.ph);
        for n in 0..64 {
            let pos = scan[n] as usize;
            self.wq_intra[n] = i32::from(ctx.intra_matrix[pos]) * qs;
            self.wq_non[n] = i32::from(ctx.non_intra_matrix[pos]) * qs;
        }
        Ok(())
    }

    fn reset_dc(&mut self, ph: &PictureHeader) {
        self.dc_pred = [1 << (7 + ph.intra_dc_precision); 3];
    }
}

fn scan_of(ph: &PictureHeader) -> &'static [u8; 64] {
    &crate::tables::SCAN[usize::from(ph.alternate_scan)]
}

/// A decoded block: nothing, a constant (DC only), or a full residual.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Res {
    None,
    Dc(i16),
    Full,
}

/// Decodes one slice. `data` starts right after the slice start code; `code` is its last byte.
pub(crate) fn decode_slice(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    tgt: &mut Target<'_>,
    data: &[u8],
    code: u8,
    stats: &mut MbStats,
) -> Result<(), Error> {
    let row = code as usize - 1;
    if row >= ctx.mb_rows {
        return Err(Error::Bitstream("slice below the picture"));
    }
    let mut br = BitReader::new(data);
    let mut st = SliceState {
        qscale_code: 0,
        dc_pred: [0; 3],
        pmv: [[[0; 2]; 2]; 2],
        prev: Motion::default(),
        wq_intra: [0; 64],
        wq_non: [0; 64],
    };
    st.set_qscale(ctx, br.read(5))?;
    if br.peek(1) == 1 {
        br.skip(1 + 1 + 7); // intra_slice_flag, intra_slice, reserved_bits
        while br.bit() {
            br.skip(8);
            if br.overrun() {
                return Err(Error::Bitstream("truncated slice header"));
            }
        }
    } else {
        br.skip(1);
    }
    st.reset_dc(ctx.ph);
    let t = ctx.tables;
    let ptype = ctx.ph.picture_type;
    let mut blocks = [[0i16; 64]; 6];
    let mut pred = Pred { y: [0; 256], cb: [0; 64], cr: [0; 64] };
    let mut mbx: usize = 0;
    let mut first = true;
    loop {
        // macroblock_address_increment, with escapes
        let mut inc = 0usize;
        loop {
            match t.mba.decode(&mut br) {
                Some(MB_ESCAPE) => inc += 33,
                Some(v) => {
                    inc += v as usize;
                    break;
                }
                None => return Err(Error::Bitstream("macroblock_address_increment")),
            }
        }
        if first {
            mbx = inc - 1;
        } else {
            // Skipped macroblocks between the previous one and this one.
            for _ in 1..inc {
                mbx += 1;
                if mbx >= ctx.mb_width {
                    return Err(Error::Bitstream("skipped macroblocks past the end of the row"));
                }
                let m = match ptype {
                    PictureType::P => {
                        st.pmv = [[[0; 2]; 2]; 2];
                        zero_motion(ctx)
                    }
                    PictureType::B => {
                        if st.prev.flags & mb::INTRA != 0 {
                            return Err(Error::Bitstream("skipped macroblock after an intra macroblock"));
                        }
                        st.prev
                    }
                    PictureType::I => return Err(Error::Bitstream("skipped macroblock in an I picture")),
                };
                st.reset_dc(ctx.ph);
                form_prediction(ctx, refs, mbx, row, &m, &mut pred, stats);
                put_macroblock(ctx, tgt, mbx, row, &pred, &mut blocks, &[Res::None; 6], false, false)?;
                stats.skipped += 1;
                stats.macroblocks += 1;
            }
            mbx += 1;
        }
        if mbx >= ctx.mb_width {
            return Err(Error::Bitstream("macroblock past the end of the row"));
        }
        first = false;

        // macroblock_modes()
        let flags = t.mb_type[ptype as usize - 1].decode(&mut br).ok_or(Error::Bitstream("macroblock_type"))?;
        let intra = flags & mb::INTRA != 0;
        let frame_pic = ctx.ph.structure == PictureStructure::Frame;
        let mut mtype = if frame_pic { MT_FRAME_OR_16X8 } else { MT_FIELD };
        if flags & (mb::FORWARD | mb::BACKWARD) != 0 && (!frame_pic || !ctx.ph.frame_pred_frame_dct) {
            mtype = br.read(2) as u8;
            if mtype == 0 {
                return Err(Error::Bitstream("motion type 0"));
            }
        }
        let field_dct = frame_pic && !ctx.ph.frame_pred_frame_dct && flags & (mb::INTRA | mb::PATTERN) != 0 && br.bit();
        if flags & mb::QUANT != 0 {
            st.set_qscale(ctx, br.read(5))?;
        }

        let mut m = Motion { flags: flags & (mb::FORWARD | mb::BACKWARD), mtype, ..Motion::default() };
        if intra {
            if ctx.ph.concealment_motion_vectors {
                let mut cm = Motion { mtype: if frame_pic { MT_FRAME_OR_16X8 } else { MT_FIELD }, ..m };
                motion_vectors(ctx, &mut br, &mut st, &mut cm, 0)?;
                if !br.bit() {
                    return Err(Error::Bitstream("concealment marker bit"));
                }
            } else {
                st.pmv = [[[0; 2]; 2]; 2];
            }
            m.flags = mb::INTRA;
        } else {
            if flags & mb::FORWARD != 0 {
                motion_vectors(ctx, &mut br, &mut st, &mut m, 0)?;
            }
            if flags & mb::BACKWARD != 0 {
                motion_vectors(ctx, &mut br, &mut st, &mut m, 1)?;
            }
            if ptype == PictureType::P && flags & mb::FORWARD == 0 {
                // No motion compensation: zero vector, and the predictors reset.
                st.pmv = [[[0; 2]; 2]; 2];
                m = zero_motion(ctx);
            }
        }
        let cbp: u32 = if flags & mb::PATTERN != 0 {
            t.cbp.decode(&mut br).ok_or(Error::Bitstream("coded_block_pattern"))? as u32
        } else if intra {
            0b11_1111
        } else {
            0
        };

        // Blocks
        let mut res = [Res::None; 6];
        if intra {
            for (i, r) in res.iter_mut().enumerate() {
                let cc = if i < 4 { 0 } else { i - 3 };
                *r = intra_block(ctx, &mut br, &mut st, cc, &mut blocks[i])?;
            }
            st.prev = m;
            stats.intra += 1;
        } else {
            st.reset_dc(ctx.ph);
            for (i, r) in res.iter_mut().enumerate() {
                if cbp & (32 >> i) != 0 {
                    *r = non_intra_block(ctx, &mut br, &st, &mut blocks[i])?;
                }
            }
            form_prediction(ctx, refs, mbx, row, &m, &mut pred, stats);
            st.prev = m;
            match m.mtype {
                MT_DUAL_PRIME => stats.dual_prime += 1,
                MT_FIELD if frame_pic => stats.field_motion += 1,
                MT_FRAME_OR_16X8 if !frame_pic => stats.sixteen_by_eight += 1,
                _ => {}
            }
            if m.flags == mb::FORWARD | mb::BACKWARD {
                stats.bidirectional += 1;
            }
        }
        if br.overrun() {
            return Err(Error::Bitstream("slice data ends inside a macroblock"));
        }
        for r in &res {
            match r {
                Res::None => {}
                Res::Dc(_) => {
                    stats.coded_blocks += 1;
                    stats.dc_only_blocks += 1;
                }
                Res::Full => stats.coded_blocks += 1,
            }
        }
        stats.field_dct += u64::from(field_dct);
        put_macroblock(ctx, tgt, mbx, row, &pred, &mut blocks, &res, intra, field_dct)?;
        stats.macroblocks += 1;

        // The slice ends where the next start code (23 zero bits) begins.
        if br.peek(23) == 0 {
            return Ok(());
        }
    }
}

/// The prediction of a P macroblock without motion compensation (and of a skipped one).
fn zero_motion(ctx: &PicCtx<'_>) -> Motion {
    match ctx.parity {
        None => Motion { flags: mb::FORWARD, mtype: MT_FRAME_OR_16X8, ..Motion::default() },
        Some(p) => Motion { flags: mb::FORWARD, mtype: MT_FIELD, mvfs: [[p; 2]; 2], ..Motion::default() },
    }
}

/// `motion_vectors(s)` and the vector reconstruction of 7.6.3.
fn motion_vectors(
    ctx: &PicCtx<'_>,
    br: &mut BitReader<'_>,
    st: &mut SliceState,
    m: &mut Motion,
    s: usize,
) -> Result<(), Error> {
    let frame_pic = ctx.parity.is_none();
    let (count, field_format, dmv) = match (frame_pic, m.mtype) {
        (true, MT_FIELD) => (2, true, false),
        (true, MT_FRAME_OR_16X8) => (1, false, false),
        (false, MT_FIELD) => (1, true, false),
        (false, MT_FRAME_OR_16X8) => (2, true, false),
        (_, _) => (1, true, true),
    };
    let scale_vertical = field_format && frame_pic;
    let f_code = ctx.ph.f_code[s];
    if count == 1 {
        if field_format && !dmv {
            m.mvfs[0][s] = br.read(1) as u8;
        }
        m.mv[0][s] = motion_vector(ctx.tables, br, f_code, &mut st.pmv[0][s], scale_vertical, dmv, &mut m.dmv)?;
        st.pmv[1][s] = st.pmv[0][s];
    } else {
        for r in 0..2 {
            m.mvfs[r][s] = br.read(1) as u8;
            m.mv[r][s] = motion_vector(ctx.tables, br, f_code, &mut st.pmv[r][s], scale_vertical, false, &mut m.dmv)?;
        }
    }
    Ok(())
}

fn motion_vector(
    t: &Tables,
    br: &mut BitReader<'_>,
    f_code: [u8; 2],
    pmv: &mut [i32; 2],
    scale_vertical: bool,
    dmv: bool,
    dmvector: &mut [i32; 2],
) -> Result<[i32; 2], Error> {
    let mut out = [0i32; 2];
    for k in 0..2 {
        let mut code = i32::from(t.motion.decode(br).ok_or(Error::Bitstream("motion_code"))?);
        if code != 0 && br.bit() {
            code = -code;
        }
        let fc = u32::from(f_code[k]);
        if !(1..=9).contains(&fc) {
            return Err(Error::Bitstream("f_code out of range"));
        }
        let r_size = fc - 1;
        let f = 1i32 << r_size;
        let delta = if f == 1 || code == 0 {
            code
        } else {
            let residual = br.read(r_size) as i32;
            let d = (code.abs() - 1) * f + residual + 1;
            if code < 0 { -d } else { d }
        };
        if dmv {
            dmvector[k] = i32::from(t.dmv.decode(br).ok_or(Error::Bitstream("dmvector"))?);
        }
        let (low, high, range) = (-16 * f, 16 * f - 1, 32 * f);
        let scaled = scale_vertical && k == 1;
        let prediction = if scaled { pmv[k] >> 1 } else { pmv[k] };
        let mut v = prediction + delta;
        if v < low {
            v += range;
        }
        if v > high {
            v -= range;
        }
        pmv[k] = if scaled { v * 2 } else { v };
        out[k] = v;
    }
    Ok(out)
}

/// One DCT coefficient: `None` at end of block, else (run, signed level). The code and its sign bit come from one
/// peek of the bit reader.
#[inline(always)]
fn dct_coefficient(br: &mut BitReader<'_>, table: &vlc::Vlc) -> Result<Option<(usize, i32)>, Error> {
    let p = br.peek32();
    let (sym, len) = table.lookup(p).ok_or(Error::Bitstream("dct coefficient"))?;
    if sym >= 0 {
        // Codes are at most 16 bits, so the sign bit is in the same word.
        let level = i32::from(vlc::level_of(sym));
        let negative = (p << len) & 0x8000_0000 != 0;
        br.skip(len + 1);
        return Ok(Some((vlc::run_of(sym) as usize, if negative { -level } else { level })));
    }
    br.skip(len);
    if sym == DCT_EOB {
        return Ok(None);
    }
    debug_assert_eq!(sym, DCT_ESCAPE);
    let w = br.read(18);
    let run = (w >> 12) as usize;
    let mut level = (w & 0xFFF) as i32;
    if level >= 2048 {
        level -= 4096;
    }
    if level == 0 || level == -2048 {
        return Err(Error::Bitstream("escape level"));
    }
    Ok(Some((run, level)))
}

/// Decodes an intra block into `blk` (raster order, dequantised, mismatch controlled).
#[inline(always)]
fn intra_block(
    ctx: &PicCtx<'_>,
    br: &mut BitReader<'_>,
    st: &mut SliceState,
    cc: usize,
    blk: &mut [i16; 64],
) -> Result<Res, Error> {
    let t = ctx.tables;
    let size_table = if cc == 0 { &t.dc_luma } else { &t.dc_chroma };
    let size = size_table.decode(br).ok_or(Error::Bitstream("dct_dc_size"))? as u32;
    let diff = if size == 0 {
        0
    } else {
        let v = br.read(size) as i32;
        if v < 1 << (size - 1) { v - (1 << size) + 1 } else { v }
    };
    st.dc_pred[cc] += diff;
    let dc = (st.dc_pred[cc] << (3 - ctx.ph.intra_dc_precision)).clamp(-2048, 2047);
    blk[0] = dc as i16;
    let mut parity = dc & 1;
    let mut any_ac = false;
    let table = if ctx.ph.intra_vlc_format { &t.dct_one } else { &t.dct_zero };
    let scan = scan_of(ctx.ph);
    let mut n = 0usize;
    while let Some((run, level)) = dct_coefficient(br, table)? {
        n += run + 1;
        if n > 63 {
            return Err(Error::Bitstream("more than 64 coefficients"));
        }
        let v = ((level * st.wq_intra[n]) / 16).clamp(-2048, 2047);
        blk[scan[n] as usize] = v as i16;
        parity ^= v & 1;
        any_ac = true;
    }
    Ok(finish_block(blk, parity, any_ac))
}

/// Decodes a non-intra block.
#[inline(always)]
fn non_intra_block(
    ctx: &PicCtx<'_>,
    br: &mut BitReader<'_>,
    st: &SliceState,
    blk: &mut [i16; 64],
) -> Result<Res, Error> {
    let table = &ctx.tables.dct_zero;
    let scan = scan_of(ctx.ph);
    let mut parity = 0;
    let mut any_ac = false;
    let mut n: isize = -1;
    let mut first = true;
    loop {
        let (run, level) = if first && br.peek(1) == 1 {
            // "1s": the first coefficient's short code for (0, +-1).
            let negative = br.peek(2) & 1 != 0;
            br.skip(2);
            (0, if negative { -1 } else { 1 })
        } else {
            match dct_coefficient(br, table)? {
                Some(c) => c,
                None if first => return Err(Error::Bitstream("end of block before any coefficient")),
                None => break,
            }
        };
        first = false;
        n += run as isize + 1;
        if n > 63 {
            return Err(Error::Bitstream("more than 64 coefficients"));
        }
        let k = n as usize;
        let v = (((2 * level + level.signum()) * st.wq_non[k]) / 32).clamp(-2048, 2047);
        blk[scan[k] as usize] = v as i16;
        parity ^= v & 1;
        any_ac |= k != 0;
    }
    Ok(finish_block(blk, parity, any_ac))
}

/// Mismatch control (7.4.4), then the IDCT; returns how the block is to be added.
#[inline(always)]
fn finish_block(blk: &mut [i16; 64], parity: i32, any_ac: bool) -> Res {
    if parity == 0 {
        blk[63] ^= 1;
    } else if !any_ac {
        let dc = idct_dc(blk[0]);
        blk[0] = 0;
        return Res::Dc(dc);
    }
    idct(blk);
    Res::Full
}

/// Chooses the reference frame for direction `s` and field `sel` (field pictures).
fn reference<'a>(ctx: &PicCtx<'_>, refs: &Refs<'a>, s: usize, sel: Option<u8>) -> Option<&'a Frame> {
    if s == 1 {
        return refs.backward;
    }
    if let (Some(p), Some(sel)) = (ctx.parity, sel)
        && ctx.second_field
        && ctx.ph.picture_type == PictureType::P
        && sel != p
    {
        return refs.first_field.or(refs.forward);
    }
    refs.forward
}

struct Views<'a> {
    y: PlaneView<'a>,
    cb: PlaneView<'a>,
    cr: PlaneView<'a>,
}

fn views<'a>(ctx: &PicCtx<'_>, f: &'a Frame, field: Option<u8>) -> Views<'a> {
    let ch = ctx.coded_height / 2;
    Views {
        y: PlaneView::new(&f.y, ctx.stride, ctx.coded_height, field),
        cb: PlaneView::new(&f.cb, ctx.chroma_stride, ch, field),
        cr: PlaneView::new(&f.cr, ctx.chroma_stride, ch, field),
    }
}

/// Predicts one region of the macroblock: luma `16 x lh` at luma half-sample position (`hx`, `hy`) into `pred`
/// at line `line` with line step `step`; chroma likewise at half size.
#[allow(clippy::too_many_arguments)]
fn region(
    v: &Views<'_>,
    pred: &mut Pred,
    hx: i32,
    hy: i32,
    mv: [i32; 2],
    line: usize,
    step: usize,
    lh: usize,
    avg: bool,
) {
    let mut d = Dst { buf: &mut pred.y, offset: line * 16, pitch: 16 * step };
    predict::<16>(&mut d, &v.y, hx + mv[0], hy + mv[1], lh, avg);
    let (cx, cy) = (mv[0] / 2, mv[1] / 2);
    // Interleaved fields (step 2) start on the same chroma line; stacked halves at half the luma line.
    let cline = if step == 2 { line } else { line / 2 };
    let mut d = Dst { buf: &mut pred.cb, offset: cline * 8, pitch: 8 * step };
    predict::<8>(&mut d, &v.cb, hx / 2 + cx, hy / 2 + cy, lh / 2, avg);
    let mut d = Dst { buf: &mut pred.cr, offset: cline * 8, pitch: 8 * step };
    predict::<8>(&mut d, &v.cr, hx / 2 + cx, hy / 2 + cy, lh / 2, avg);
}

/// The dual-prime vector for predicting a field from the field of the other parity (7.6.3.6).
fn dual_prime_vector(mv: [i32; 2], dmv: [i32; 2], m: i32, e: i32) -> [i32; 2] {
    let scale = |v: i32| (v * m + i32::from(v > 0)) >> 1;
    [scale(mv[0]) + dmv[0], scale(mv[1]) + e + dmv[1]]
}

fn form_prediction(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    mbx: usize,
    mby: usize,
    m: &Motion,
    pred: &mut Pred,
    stats: &mut MbStats,
) {
    let mut avg = false;
    for s in 0..2 {
        let dir = if s == 0 { mb::FORWARD } else { mb::BACKWARD };
        if m.flags & dir == 0 {
            continue;
        }
        let ok = match ctx.parity {
            None => predict_frame_picture(ctx, refs, mbx, mby, m, s, pred, avg),
            Some(p) => predict_field_picture(ctx, refs, mbx, mby, m, s, p, pred, avg),
        };
        if !ok {
            stats.missing_reference += 1;
            if !avg {
                pred.y.fill(128);
                pred.cb.fill(128);
                pred.cr.fill(128);
            }
        }
        avg = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn predict_frame_picture(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    mbx: usize,
    mby: usize,
    m: &Motion,
    s: usize,
    pred: &mut Pred,
    avg: bool,
) -> bool {
    let Some(f) = reference(ctx, refs, s, None) else { return false };
    let hx = (mbx * 32) as i32;
    match m.mtype {
        MT_FRAME_OR_16X8 => {
            let v = views(ctx, f, None);
            region(&v, pred, hx, (mby * 32) as i32, m.mv[0][s], 0, 1, 16, avg);
        }
        MT_FIELD => {
            for r in 0..2 {
                let v = views(ctx, f, Some(m.mvfs[r][s]));
                region(&v, pred, hx, (mby * 16) as i32, m.mv[r][s], r, 2, 8, avg);
            }
        }
        _ => {
            let tff = ctx.ph.top_field_first;
            let mv = m.mv[0][s];
            for p in 0..2u8 {
                // Same parity with the transmitted vector, opposite parity with the derived one, averaged.
                let (mm, e) = if p == 0 { (if tff { 1 } else { 3 }, -1) } else { (if tff { 3 } else { 1 }, 1) };
                let same = views(ctx, f, Some(p));
                let other = views(ctx, f, Some(1 - p));
                let hy = (mby * 16) as i32;
                region(&same, pred, hx, hy, mv, p as usize, 2, 8, avg);
                region(&other, pred, hx, hy, dual_prime_vector(mv, m.dmv, mm, e), p as usize, 2, 8, true);
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn predict_field_picture(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    mbx: usize,
    mby: usize,
    m: &Motion,
    s: usize,
    parity: u8,
    pred: &mut Pred,
    avg: bool,
) -> bool {
    let hx = (mbx * 32) as i32;
    let hy = (mby * 32) as i32;
    match m.mtype {
        MT_FIELD => {
            let sel = m.mvfs[0][s];
            let Some(f) = reference(ctx, refs, s, Some(sel)) else { return false };
            region(&views(ctx, f, Some(sel)), pred, hx, hy, m.mv[0][s], 0, 1, 16, avg);
        }
        MT_FRAME_OR_16X8 => {
            for r in 0..2 {
                let sel = m.mvfs[r][s];
                let Some(f) = reference(ctx, refs, s, Some(sel)) else { return false };
                region(&views(ctx, f, Some(sel)), pred, hx, hy + 16 * r as i32, m.mv[r][s], 8 * r, 1, 8, avg);
            }
        }
        _ => {
            let mv = m.mv[0][s];
            let (Some(same), Some(other)) =
                (reference(ctx, refs, s, Some(parity)), reference(ctx, refs, s, Some(1 - parity)))
            else {
                return false;
            };
            let e = if parity == 0 { -1 } else { 1 };
            region(&views(ctx, same, Some(parity)), pred, hx, hy, mv, 0, 1, 16, avg);
            let dv = dual_prime_vector(mv, m.dmv, 1, e);
            region(&views(ctx, other, Some(1 - parity)), pred, hx, hy, dv, 0, 1, 16, true);
        }
    }
    true
}

/// Writes one 8 x 8 block: prediction plus residual, saturated.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn put_block(
    dst: &mut [u8],
    base: usize,
    pitch: usize,
    pred: &[u8],
    pbase: usize,
    ppitch: usize,
    res: Res,
    blk: &[i16; 64],
) {
    for r in 0..8 {
        let d = &mut dst[base + r * pitch..][..8];
        let p = &pred[pbase + r * ppitch..][..8];
        match res {
            Res::None => d.copy_from_slice(p),
            Res::Dc(v) => {
                for (d, &p) in d.iter_mut().zip(p) {
                    *d = (i16::from(p) + v).clamp(0, 255) as u8;
                }
            }
            Res::Full => {
                for ((d, &p), &x) in d.iter_mut().zip(p).zip(&blk[r * 8..r * 8 + 8]) {
                    *d = (i16::from(p) + x).clamp(0, 255) as u8;
                }
            }
        }
    }
}

const ZERO_PRED: [u8; 256] = [0; 256];

#[allow(clippy::too_many_arguments)]
fn put_macroblock(
    ctx: &PicCtx<'_>,
    tgt: &mut Target<'_>,
    mbx: usize,
    mby: usize,
    pred: &Pred,
    blocks: &mut [[i16; 64]; 6],
    res: &[Res; 6],
    intra: bool,
    field_dct: bool,
) -> Result<(), Error> {
    let (s, cs) = (ctx.stride, ctx.chroma_stride);
    // First luma line of the macroblock in the target, and the distance between its lines.
    let (line, pitch_lines) = match ctx.parity {
        None => ((mby * 16) as isize - tgt.line0 as isize, 1),
        Some(p) => ((mby * 32 + p as usize) as isize - tgt.line0 as isize, 2),
    };
    let (cline, _) = match ctx.parity {
        None => ((mby * 8) as isize - (tgt.line0 / 2) as isize, 1),
        Some(p) => ((mby * 16 + p as usize) as isize - (tgt.line0 / 2) as isize, 2),
    };
    if line < 0
        || cline < 0
        || (line as usize + 15 * pitch_lines + 1) * s > tgt.y.len()
        || (cline as usize + 7 * pitch_lines + 1) * cs > tgt.cb.len()
        || (cline as usize + 7 * pitch_lines + 1) * cs > tgt.cr.len()
    {
        return Err(Error::Bitstream("macroblock outside the picture"));
    }
    let ybase = line as usize * s + mbx * 16;
    let cbase = cline as usize * cs + mbx * 8;
    let (py, pcb, pcr): (&[u8], &[u8], &[u8]) =
        if intra { (&ZERO_PRED, &ZERO_PRED, &ZERO_PRED) } else { (&pred.y, &pred.cb, &pred.cr) };
    for k in 0..4 {
        let (first_line, step) = if field_dct { (k >> 1, 2) } else { ((k >> 1) * 8, 1) };
        let col = (k & 1) * 8;
        put_block(
            tgt.y,
            ybase + first_line * pitch_lines * s + col,
            step * pitch_lines * s,
            py,
            first_line * 16 + col,
            16 * step,
            res[k],
            &blocks[k],
        );
    }
    put_block(tgt.cb, cbase, pitch_lines * cs, pcb, 0, 8, res[4], &blocks[4]);
    put_block(tgt.cr, cbase, pitch_lines * cs, pcr, 0, 8, res[5], &blocks[5]);
    for (b, r) in blocks.iter_mut().zip(res) {
        if *r == Res::Full {
            *b = [0; 64];
        }
    }
    Ok(())
}
