//! The MPEG-2 video decoder: elementary-stream units, reference frames and display order.

use alloc::vec::Vec;
use core::ops::Range;

use crate::Error;
use crate::bits::{BitReader, find_start_code};
use crate::frame::{Frame, FrameInfo};
use crate::headers::{
    Extension, GopHeader, PictureHeader, PictureStructure, PictureType, QuantMatrices, SequenceHeader, code,
    parse_extension,
};
use crate::slice::{MbStats, PicCtx, Refs, Target, decode_slice};
use crate::vlc::Tables;

/// Largest frame width and height decoded (the movies are 640 x 448; sizes up to 16,383 are codable).
pub const MAX_FRAME_SIDE: usize = 2048;

/// Counters since the decoder was made.
#[derive(Clone, Debug, Default)]
pub struct DecoderStats {
    /// Coded pictures decoded (each field picture counts once).
    pub pictures: u64,
    pub i_pictures: u64,
    pub p_pictures: u64,
    pub b_pictures: u64,
    /// B pictures passed over by [`Decoder::set_skip_b`].
    pub skipped_b: u64,
    /// Frames returned in display order.
    pub frames_out: u64,
    pub sequence_headers: u64,
    pub gops: u64,
    /// GOP headers with closed_gop set.
    pub closed_gops: u64,
    /// GOP headers with broken_link set.
    pub broken_links: u64,
    /// Slices that failed to decode; the first error is kept.
    pub slice_errors: u64,
    pub first_error: Option<Error>,
    pub mb: MbStats,
}

/// The result of [`Decoder::decode`].
#[derive(Debug)]
pub enum Decoded<'a> {
    /// The next frame in display order. It stays valid until the next call that decodes.
    Frame(&'a Frame),
    /// The buffered data ends inside a unit: push more with [`Decoder::push`], or call [`Decoder::finish`].
    NeedData,
    /// The input is finished and every frame has been returned.
    End,
}

/// A frame coded as two fields, between its first and second field.
#[derive(Clone, Copy)]
struct PendingField {
    slot: usize,
    parity: u8,
    picture_type: PictureType,
}

/// Decodes an MPEG-2 video elementary stream (Main Profile, 4:2:0) pushed in pieces of any size.
pub struct Decoder {
    tables: Tables,
    seq: Option<SequenceHeader>,
    quant: QuantMatrices,
    gop: GopHeader,
    pool: [Frame; 3],
    snapshot: Frame,
    forward: Option<usize>,
    backward: Option<usize>,
    backward_shown: bool,
    pending_field: Option<PendingField>,
    input: Vec<u8>,
    pos: usize,
    /// Where to resume looking for the end of the unit at `pos`.
    scan_from: usize,
    eof: bool,
    display_index: u64,
    skip_b: bool,
    parallel: Option<alloc::boxed::Box<dyn Parallel>>,
    stats: DecoderStats,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            tables: Tables::new(),
            seq: None,
            quant: QuantMatrices {
                intra: crate::tables::DEFAULT_INTRA_MATRIX,
                non_intra: crate::tables::DEFAULT_NON_INTRA_MATRIX,
            },
            gop: GopHeader::default(),
            pool: Default::default(),
            snapshot: Frame::default(),
            forward: None,
            backward: None,
            backward_shown: true,
            pending_field: None,
            input: Vec::new(),
            pos: 0,
            scan_from: 0,
            eof: false,
            display_index: 0,
            skip_b: false,
            parallel: None,
            stats: DecoderStats::default(),
        }
    }

    /// Appends elementary-stream bytes.
    pub fn push(&mut self, data: &[u8]) {
        if self.pos > 0 && (self.pos >= self.input.len() / 2 || self.input.len() + data.len() > self.input.capacity()) {
            self.input.drain(..self.pos);
            self.scan_from -= self.pos.min(self.scan_from);
            self.pos = 0;
        }
        self.input.extend_from_slice(data);
    }

    /// Marks the end of the input: the last unit is complete, and the last reference frame will be returned.
    pub fn finish(&mut self) {
        self.eof = true;
    }

    /// Bytes buffered and not yet decoded.
    pub fn buffered(&self) -> usize {
        self.input.len() - self.pos
    }

    /// The sequence header in force, once one has been read.
    pub fn sequence(&self) -> Option<&SequenceHeader> {
        self.seq.as_ref()
    }

    /// The last group-of-pictures header read.
    pub fn gop(&self) -> &GopHeader {
        &self.gop
    }

    pub fn stats(&self) -> &DecoderStats {
        &self.stats
    }

    /// When set, B pictures are not decoded (they are never references); their display slots still count. A player
    /// that falls behind uses this to catch up.
    pub fn set_skip_b(&mut self, skip: bool) {
        self.skip_b = skip;
    }

    /// Decodes the slices of frame pictures as parallel jobs on `parallel` (none: on the calling thread).
    pub fn set_parallel(&mut self, parallel: Option<alloc::boxed::Box<dyn Parallel>>) {
        self.parallel = parallel;
    }

    /// Slice decoding on `n` scoped `std` threads (feature `threads`); 1 or less turns it off. Without the feature
    /// this does nothing.
    pub fn set_threads(&mut self, n: usize) {
        #[cfg(feature = "threads")]
        {
            self.parallel = (n > 1).then(|| alloc::boxed::Box::new(StdThreads(n)) as alloc::boxed::Box<dyn Parallel>);
        }
        #[cfg(not(feature = "threads"))]
        let _ = n;
    }

    /// Decodes until the next frame in display order is ready, more data is needed, or the stream ends.
    pub fn decode(&mut self) -> Result<Decoded<'_>, Error> {
        match self.step()? {
            Step::Frame(slot) => Ok(Decoded::Frame(&self.pool[slot])),
            Step::NeedData => Ok(Decoded::NeedData),
            Step::End => Ok(Decoded::End),
        }
    }

    fn step(&mut self) -> Result<Step, Error> {
        loop {
            let Some(start) = find_start_code(&self.input, self.pos) else {
                // Nothing before the last two bytes can start a code: skip it, so data without start codes is
                // scanned once, not again on every push.
                self.pos = self.pos.max(self.input.len().saturating_sub(2));
                return self.out_of_data();
            };
            self.pos = start;
            if start + 4 > self.input.len() {
                return self.out_of_data();
            }
            let unit_code = self.input[start + 3];
            // Find where the unit ends: a picture runs to the next picture, sequence, GOP or end code; anything
            // else to the next start code.
            let mut search = self.scan_from.max(start + 4);
            let end = loop {
                match find_start_code(&self.input, search) {
                    Some(n) if n + 3 < self.input.len() => {
                        let c = self.input[n + 3];
                        let ends = unit_code != code::PICTURE
                            || matches!(c, code::PICTURE | code::SEQUENCE_HEADER | code::GROUP | code::SEQUENCE_END);
                        if ends {
                            break Some(n);
                        }
                        search = n + 3;
                    }
                    Some(n) => {
                        search = n;
                        break None;
                    }
                    None => {
                        search = self.input.len().saturating_sub(2).max(start + 4);
                        break None;
                    }
                }
            };
            let end = match end {
                Some(e) => e,
                None if self.eof => self.input.len(),
                None => {
                    self.scan_from = search;
                    return Ok(Step::NeedData);
                }
            };
            self.scan_from = 0;
            let input = core::mem::take(&mut self.input);
            let result = self.unit(unit_code, &input[start + 4..end]);
            self.input = input;
            self.pos = end;
            if let Some(slot) = result? {
                return Ok(Step::Frame(slot));
            }
        }
    }

    fn out_of_data(&mut self) -> Result<Step, Error> {
        if !self.eof {
            return Ok(Step::NeedData);
        }
        self.pos = self.input.len();
        Ok(match self.flush() {
            Some(slot) => Step::Frame(slot),
            None => Step::End,
        })
    }

    /// Returns the last reference frame if it has not been shown.
    fn flush(&mut self) -> Option<usize> {
        let slot = self.backward?;
        if self.backward_shown {
            return None;
        }
        self.backward_shown = true;
        Some(self.emit(slot))
    }

    fn emit(&mut self, slot: usize) -> usize {
        self.pool[slot].info.display_index = self.display_index;
        self.display_index += 1;
        self.stats.frames_out += 1;
        slot
    }

    fn unit(&mut self, unit_code: u8, data: &[u8]) -> Result<Option<usize>, Error> {
        match unit_code {
            code::PICTURE => self.picture(data),
            code::SEQUENCE_HEADER => {
                let mut br = BitReader::new(data);
                let seq = SequenceHeader::parse(&mut br)?;
                self.quant = QuantMatrices { intra: seq.intra_matrix, non_intra: seq.non_intra_matrix };
                self.seq = Some(seq);
                self.stats.sequence_headers += 1;
                Ok(None)
            }
            code::EXTENSION => {
                let mut br = BitReader::new(data);
                parse_extension(&mut br, self.seq.as_mut(), None, Some(&mut self.quant))?;
                Ok(None)
            }
            code::GROUP => {
                self.gop = GopHeader::parse(&mut BitReader::new(data));
                self.stats.gops += 1;
                self.stats.closed_gops += u64::from(self.gop.closed_gop);
                self.stats.broken_links += u64::from(self.gop.broken_link);
                Ok(None)
            }
            code::SEQUENCE_END => Ok(self.flush()),
            _ => Ok(None),
        }
    }

    fn ensure_pool(&mut self, seq: &SequenceHeader) {
        let (mw, mh) = seq.mb_size();
        let (cw, ch) = (mw * 16, mh * 16);
        for f in self.pool.iter_mut() {
            if f.stride != cw || f.coded_height != ch || f.y.len() != cw * ch {
                *f = Frame::new(seq.width as usize, seq.height as usize, cw, ch);
            }
            f.width = seq.width as usize;
            f.height = seq.height as usize;
            f.matrix_coefficients = seq.matrix_coefficients;
        }
    }

    fn picture(&mut self, data: &[u8]) -> Result<Option<usize>, Error> {
        let seq = self.seq.clone().ok_or(Error::Bitstream("picture before a sequence header"))?;
        // Frames are allocated at the coded size: refuse sizes no movie has.
        if seq.width as usize > MAX_FRAME_SIDE || seq.height as usize > MAX_FRAME_SIDE {
            return Err(Error::Unsupported("frame larger than 2048 x 2048"));
        }
        if !seq.mpeg2 {
            return Err(Error::Unsupported("MPEG-1 video"));
        }
        if seq.chroma_format != 1 {
            return Err(Error::Unsupported("chroma formats other than 4:2:0"));
        }
        let mut br = BitReader::new(data);
        let mut ph = PictureHeader::parse(&mut br)?;
        let mut quant = self.quant.clone();
        let mut slices: Vec<(u8, Range<usize>)> = Vec::new();
        let mut at = find_start_code(data, br.position() / 8);
        while let Some(s) = at {
            if s + 4 > data.len() {
                break;
            }
            let next = find_start_code(data, s + 4);
            let body = s + 4..next.unwrap_or(data.len());
            match data[s + 3] {
                code::EXTENSION => {
                    let mut ebr = BitReader::new(&data[body.clone()]);
                    if let Extension::QuantMatrix = parse_extension(&mut ebr, None, Some(&mut ph), Some(&mut quant))? {
                        self.quant = quant.clone();
                    }
                }
                c @ code::SLICE_FIRST..=code::SLICE_LAST => slices.push((c, body)),
                _ => {}
            }
            at = next;
        }
        if !ph.has_coding_extension {
            return Err(Error::Unsupported("MPEG-1 picture (no picture coding extension)"));
        }
        self.ensure_pool(&seq);

        let parity = match ph.structure {
            PictureStructure::Frame => None,
            PictureStructure::TopField => Some(0u8),
            PictureStructure::BottomField => Some(1u8),
        };
        // A second field completes the frame begun by the first.
        let second = match (self.pending_field, parity) {
            (Some(pf), Some(p)) if pf.parity != p => Some(pf),
            _ => None,
        };
        if parity.is_none() || second.is_none() {
            self.pending_field = None;
        }
        let frame_type = second.map_or(ph.picture_type, |pf| pf.picture_type);

        if frame_type == PictureType::B && self.skip_b {
            // Skip a whole frame: both fields of a field pair.
            self.stats.skipped_b += 1;
            if parity.is_some() && second.is_none() {
                self.pending_field =
                    Some(PendingField { slot: usize::MAX, parity: parity.unwrap_or(0), picture_type: frame_type });
            } else {
                self.display_index += 1;
            }
            return Ok(None);
        }
        if second.is_some_and(|pf| pf.slot == usize::MAX) {
            // The second field of a skipped B frame (skipping was switched off in between).
            self.display_index += 1;
            return Ok(None);
        }

        let slot = match second {
            Some(pf) => pf.slot,
            None => {
                let is_ref = |i: usize| Some(i) == self.backward || Some(i) == self.forward;
                if frame_type == PictureType::B {
                    (0..3).find(|&i| !is_ref(i)).unwrap_or(0)
                } else {
                    (0..3).find(|&i| !is_ref(i)).or_else(|| (0..3).find(|&i| Some(i) != self.backward)).unwrap_or(0)
                }
            }
        };
        let (fwd_slot, bwd_slot) = match frame_type {
            PictureType::B => (self.forward, self.backward),
            _ => (self.backward, None),
        };
        let mut cur = core::mem::take(&mut self.pool[slot]);
        if second.is_some() && ph.picture_type == PictureType::P {
            self.snapshot.copy_from(&cur);
        }
        let (mw, mh) = seq.mb_size();
        let ctx = PicCtx {
            tables: &self.tables,
            ph: &ph,
            intra_matrix: &self.quant.intra,
            non_intra_matrix: &self.quant.non_intra,
            mb_width: mw,
            mb_rows: if parity.is_some() { mh / 2 } else { mh },
            parity,
            second_field: second.is_some(),
            stride: cur.stride,
            chroma_stride: cur.chroma_stride,
            coded_height: cur.coded_height,
        };
        let refs = Refs {
            forward: fwd_slot.map(|i| &self.pool[i]),
            backward: bwd_slot.map(|i| &self.pool[i]),
            first_field: if second.is_some() && ph.picture_type == PictureType::P {
                Some(&self.snapshot)
            } else {
                None
            },
        };
        let mut errors = (0u64, None);
        let mut mbs = MbStats::default();
        decode_slices(&ctx, &refs, &mut cur, data, &slices, self.parallel.as_deref(), &mut mbs, &mut errors);
        self.stats.mb.add(&mbs);
        self.stats.slice_errors += errors.0;
        if self.stats.first_error.is_none() {
            self.stats.first_error = errors.1;
        }
        self.stats.pictures += 1;
        match ph.picture_type {
            PictureType::I => self.stats.i_pictures += 1,
            PictureType::P => self.stats.p_pictures += 1,
            PictureType::B => self.stats.b_pictures += 1,
        }
        if second.is_none() {
            cur.info = FrameInfo {
                display_index: 0,
                picture_type: ph.picture_type,
                temporal_reference: ph.temporal_reference,
                top_field_first: ph.top_field_first || parity == Some(0),
                progressive_frame: ph.progressive_frame,
                repeat_first_field: ph.repeat_first_field,
                field_pictures: parity.is_some(),
            };
        }
        self.pool[slot] = cur;

        if let (Some(p), None) = (parity, second) {
            self.pending_field = Some(PendingField { slot, parity: p, picture_type: ph.picture_type });
            return Ok(None);
        }
        // The frame is complete.
        if frame_type == PictureType::B {
            return Ok(Some(self.emit(slot)));
        }
        let show = if self.backward_shown { None } else { self.backward };
        self.forward = self.backward;
        self.backward = Some(slot);
        self.backward_shown = false;
        if let Some(s) = show {
            if let Some(f) = self.forward {
                debug_assert_eq!(f, s);
            }
            return Ok(Some(self.emit(s)));
        }
        Ok(None)
    }
}

enum Step {
    Frame(usize),
    NeedData,
    End,
}

/// Runs a batch of jobs, possibly on several threads, and returns only when every job has finished. The decoder
/// uses it to decode bands of macroblock rows of a frame picture at the same time.
///
/// `std` builds can use [`StdThreads`] (feature `threads`); the Switch build implements it on the platform layer's
/// threads (cores 1 and 2 are mostly idle while a movie plays).
pub trait Parallel: Send + Sync {
    /// Calls every job once, in any order and on any threads, and returns after all of them have returned.
    fn run(&self, jobs: &mut [&mut (dyn FnMut() + Send)]);
    /// How many jobs to split a picture into (1 disables splitting).
    fn width(&self) -> usize;
}

/// [`Parallel`] on scoped `std` threads: the first job runs on the calling thread, the others on new threads.
#[cfg(feature = "threads")]
#[derive(Clone, Copy, Debug)]
pub struct StdThreads(pub usize);

#[cfg(feature = "threads")]
impl Parallel for StdThreads {
    fn run(&self, jobs: &mut [&mut (dyn FnMut() + Send)]) {
        let Some((first, rest)) = jobs.split_first_mut() else { return };
        std::thread::scope(|scope| {
            for job in rest.iter_mut() {
                scope.spawn(job);
            }
            first();
        });
    }

    fn width(&self) -> usize {
        self.0.max(1)
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_slices(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    cur: &mut Frame,
    data: &[u8],
    slices: &[(u8, Range<usize>)],
    parallel: Option<&dyn Parallel>,
    mbs: &mut MbStats,
    errors: &mut (u64, Option<Error>),
) {
    // Field pictures read the frame they write (the second field predicts from the first), so they stay serial.
    if let Some(p) = parallel
        && p.width() > 1
        && ctx.parity.is_none()
        && ctx.mb_rows >= p.width()
    {
        decode_bands(ctx, refs, cur, data, slices, p, mbs, errors);
        return;
    }
    let mut tgt = Target { y: &mut cur.y, cb: &mut cur.cb, cr: &mut cur.cr, line0: 0 };
    for (c, r) in slices {
        if let Err(e) = decode_slice(ctx, refs, &mut tgt, &data[r.clone()], *c, mbs) {
            errors.0 += 1;
            errors.1.get_or_insert(e);
        }
    }
}

/// One band of macroblock rows and what decoding it produced.
struct Band<'a> {
    rows: Range<usize>,
    target: Target<'a>,
    stats: MbStats,
    errors: u64,
    first_error: Option<Error>,
}

/// Splits a frame picture into bands of macroblock rows (each slice lies within one row) and decodes them as
/// parallel jobs; each band writes only its own lines.
#[allow(clippy::too_many_arguments)]
fn decode_bands(
    ctx: &PicCtx<'_>,
    refs: &Refs<'_>,
    cur: &mut Frame,
    data: &[u8],
    slices: &[(u8, Range<usize>)],
    parallel: &dyn Parallel,
    mbs: &mut MbStats,
    errors: &mut (u64, Option<Error>),
) {
    let n = parallel.width();
    let rows = ctx.mb_rows;
    let (s, cs) = (cur.stride, cur.chroma_stride);
    let mut y: &mut [u8] = &mut cur.y;
    let mut cb: &mut [u8] = &mut cur.cb;
    let mut cr: &mut [u8] = &mut cur.cr;
    let mut bands = Vec::with_capacity(n);
    let mut row0 = 0;
    for k in 0..n {
        let row1 = rows * (k + 1) / n;
        let lines = row1 - row0;
        // The split points are computed before the planes are taken apart.
        let (ly, lc) = ((lines * 16 * s).min(y.len()), (lines * 8 * cs).min(cb.len().min(cr.len())));
        let (a, rest) = core::mem::take(&mut y).split_at_mut(ly);
        y = rest;
        let (ca, rest) = core::mem::take(&mut cb).split_at_mut(lc);
        cb = rest;
        let (ra, rest) = core::mem::take(&mut cr).split_at_mut(lc);
        cr = rest;
        let target = Target { y: a, cb: ca, cr: ra, line0: row0 * 16 };
        bands.push(Band { rows: row0..row1, target, stats: MbStats::default(), errors: 0, first_error: None });
        row0 = row1;
    }
    {
        let mut jobs: Vec<alloc::boxed::Box<dyn FnMut() + Send + '_>> = bands
            .iter_mut()
            .map(|b| {
                alloc::boxed::Box::new(move || {
                    for (c, r) in slices.iter().filter(|(c, _)| b.rows.contains(&(*c as usize - 1))) {
                        if let Err(e) = decode_slice(ctx, refs, &mut b.target, &data[r.clone()], *c, &mut b.stats) {
                            b.errors += 1;
                            b.first_error.get_or_insert(e);
                        }
                    }
                }) as alloc::boxed::Box<dyn FnMut() + Send + '_>
            })
            .collect();
        let mut refs_to_jobs: Vec<&mut (dyn FnMut() + Send)> = jobs.iter_mut().map(|j| &mut **j as _).collect();
        parallel.run(&mut refs_to_jobs);
    }
    // Slices below the last band (a corrupt stream) are counted as errors by `decode_slice` in no band; count them.
    for (c, _) in slices {
        if *c as usize > rows {
            errors.0 += 1;
            errors.1.get_or_insert(Error::Bitstream("slice below the picture"));
        }
    }
    for b in bands {
        mbs.add(&b.stats);
        errors.0 += b.errors;
        if errors.1.is_none() {
            errors.1 = b.first_error;
        }
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;

    /// A sequence header of `w` x `h`, 30 fps, no matrices, then a picture start code and the sequence end.
    fn stream(w: u32, h: u32) -> Vec<u8> {
        let mut d = alloc::vec![0, 0, 1, 0xB3];
        let bits: u64 = ((w as u64) << 52) | ((h as u64) << 40) | (1 << 36) | (5 << 32) | (0x3FFFF << 14) | (1 << 13);
        d.extend_from_slice(&bits.to_be_bytes());
        d.extend_from_slice(&[0, 0, 1, 0x00, 0, 0x0F, 0xFF, 0xF8]);
        d.extend_from_slice(&[0, 0, 1, 0xB7]);
        d
    }

    #[test]
    fn huge_frames_are_refused() {
        let mut dec = Decoder::new();
        dec.push(&stream(4095, 4095));
        dec.finish();
        assert!(matches!(dec.decode(), Err(Error::Unsupported(_))));
    }

    #[test]
    fn data_without_start_codes_is_scanned_once() {
        let mut dec = Decoder::new();
        let junk = alloc::vec![0xFFu8; 4096];
        for _ in 0..64 {
            dec.push(&junk);
            assert!(matches!(dec.decode(), Ok(Decoded::NeedData)));
            // Everything but the last two bytes is behind the read position.
            assert!(dec.buffered() <= 2);
        }
    }
}
