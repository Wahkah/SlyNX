//! Decoder tests on small streams written here, covering what the disc's movies do not use: field DCT, field
//! prediction in frame pictures, dual prime, skipped macroblocks, and field pictures (including a second field
//! predicted from the first). Expected pictures are computed from the field geometry alone.

use video::decoder::{Decoded, Decoder};
use video::vlc::{CODED_BLOCK_PATTERN, DC_SIZE_CHROMA, DC_SIZE_LUMA, MACROBLOCK_ADDRESS_INCREMENT, MOTION_CODE};

const W: usize = 48;
const H: usize = 64;

#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.acc = (self.acc << 1) | u64::from((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn code(&mut self, s: &str) {
        for c in s.bytes().filter(|&c| c != b' ') {
            self.put(u32::from(c - b'0'), 1);
        }
    }
    fn align(&mut self) {
        while self.n != 0 {
            self.put(0, 1);
        }
    }
    fn start(&mut self, code: u8) {
        self.align();
        self.out.extend_from_slice(&[0, 0, 1, code]);
    }
}

fn lookup(table: &[(&'static str, i16)], v: i16) -> &'static str {
    table.iter().find(|e| e.1 == v).map(|e| e.0).expect("symbol in table")
}

fn sequence(b: &mut Bits) {
    b.start(0xB3);
    b.put(W as u32, 12);
    b.put(H as u32, 12);
    b.put(1, 4); // aspect
    b.put(4, 4); // 29.97 Hz
    b.put(1000, 18);
    b.put(1, 1);
    b.put(10, 10);
    b.put(0, 3); // constrained, no matrices
    b.start(0xB5);
    b.put(1, 4);
    b.put(0x48, 8);
    b.put(0, 1); // interlaced sequence
    b.put(1, 2); // 4:2:0
    b.put(0, 2 + 2 + 12);
    b.put(1, 1);
    b.put(0, 8 + 1 + 2 + 5);
    b.start(0xB8);
    b.put(0, 25);
    b.put(0b10, 2); // closed GOP
}

/// Picture header and coding extension. `structure` 1 top, 2 bottom, 3 frame.
fn picture(b: &mut Bits, ptype: u32, structure: u32, fpfd: bool) {
    b.start(0x00);
    b.put(0, 10);
    b.put(ptype, 3);
    b.put(0xFFFF, 16);
    if ptype != 1 {
        b.put(0b0111, 4);
    }
    b.put(0, 1);
    b.start(0xB5);
    b.put(8, 4);
    let fc = if ptype == 1 { 15 } else { 1 };
    b.put(fc, 4);
    b.put(fc, 4);
    b.put(15, 4);
    b.put(15, 4);
    b.put(0, 2); // intra_dc_precision 8 bits
    b.put(structure, 2);
    b.put(1, 1); // top field first
    b.put(u32::from(fpfd), 1);
    b.put(0, 5); // concealment, q_scale_type, intra_vlc_format, alternate_scan, repeat_first_field
    b.put(1, 1); // chroma_420_type
    b.put(0, 2); // progressive_frame, composite_display_flag
}

fn slice(b: &mut Bits, row: usize) {
    b.start(row as u8 + 1);
    b.put(8, 5);
    b.put(0, 1);
}

fn dc(b: &mut Bits, luma: bool, diff: i32) {
    let size = if diff == 0 { 0 } else { 32 - diff.unsigned_abs().leading_zeros() };
    b.code(lookup(if luma { DC_SIZE_LUMA } else { DC_SIZE_CHROMA }, size as i16));
    if size > 0 {
        let v = if diff > 0 { diff } else { diff + (1 << size) - 1 };
        b.put(v as u32, size);
    }
    b.code("10"); // end of block (table zero)
}

fn motion(b: &mut Bits, delta: i32) {
    b.code(lookup(MOTION_CODE, delta.abs() as i16));
    if delta != 0 {
        b.put(u32::from(delta < 0), 1);
    }
}

fn inc(b: &mut Bits, n: usize) {
    b.code(lookup(MACROBLOCK_ADDRESS_INCREMENT, n as i16));
}

/// The intra value of luma block `k` of macroblock (`mx`, `my`).
fn value(mx: usize, my: usize, k: usize) -> i32 {
    30 + 7 * k as i32 + 19 * mx as i32 + 41 * my as i32
}

/// Writes an intra picture body: every macroblock DC-only, luma blocks from [`value`], chroma 128. With `field_dct`
/// (frame pictures) blocks 0/1 hold the top field and 2/3 the bottom field.
fn intra_rows(b: &mut Bits, rows: usize, field_dct: Option<bool>) {
    for my in 0..rows {
        slice(b, my);
        let mut pred = 128;
        for mx in 0..W / 16 {
            inc(b, 1);
            b.code("1"); // intra
            if let Some(f) = field_dct {
                b.put(u32::from(f), 1);
            }
            for k in 0..4 {
                let v = value(mx, my, k);
                dc(b, true, v - pred);
                pred = v;
            }
            dc(b, false, 0);
            dc(b, false, 0);
        }
    }
}

fn decode_all(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut d = Decoder::new();
    d.push(stream);
    d.finish();
    let mut out = Vec::new();
    loop {
        match d.decode().expect("decode") {
            Decoded::Frame(f) => {
                assert_eq!((f.width, f.height), (W, H));
                out.push(f.y[..W * H].to_vec());
            }
            Decoded::End => break,
            Decoded::NeedData => panic!("need data after finish"),
        }
    }
    assert_eq!(d.stats().slice_errors, 0, "{:?}", d.stats().first_error);
    out
}

/// Sample of a field at half-sample position (hx, hy), clamped to the field, rounded as the standard does.
fn field_sample(f: &dyn Fn(i32, i32) -> i32, hx: i32, hy: i32) -> i32 {
    let (ix, iy, fx, fy) = (hx >> 1, hy >> 1, hx & 1, hy & 1);
    let a = f(ix, iy);
    let bx = f(ix + 1, iy);
    let cy = f(ix, iy + 1);
    let d = f(ix + 1, iy + 1);
    match (fx, fy) {
        (0, 0) => a,
        (1, 0) => (a + bx + 1) >> 1,
        (0, 1) => (a + cy + 1) >> 1,
        _ => (a + bx + cy + d + 2) >> 2,
    }
}

#[test]
fn field_dct_field_motion_dual_prime_and_skips() {
    let mut b = Bits::default();
    sequence(&mut b);
    picture(&mut b, 1, 3, false);
    intra_rows(&mut b, H / 16, Some(true));
    picture(&mut b, 2, 3, false);
    // Row 0, each macroblock its own slice (predictors start at zero): field prediction with these vectors
    // (top field from field sel0 with v0, bottom from sel1 with v1), in half samples, vertical in field lines.
    let field_cases = [((1u32, [4, 3]), (0u32, [-3, 2])), ((0, [-2, 5]), (1, [1, -1])), ((1, [0, 0]), (1, [0, 0]))];
    for (mx, ((s0, v0), (s1, v1))) in field_cases.iter().enumerate() {
        slice(&mut b, 0);
        inc(&mut b, mx + 1);
        b.code("001"); // forward, not coded
        b.put(0b01, 2); // field prediction
        b.put(*s0, 1);
        motion(&mut b, v0[0]);
        motion(&mut b, v0[1]);
        b.put(*s1, 1);
        motion(&mut b, v1[0]);
        motion(&mut b, v1[1]);
    }
    // Row 1: dual prime with zero vector and zero differential, every macroblock.
    slice(&mut b, 1);
    for _ in 0..W / 16 {
        inc(&mut b, 1);
        b.code("001");
        b.put(0b11, 2);
        for _ in 0..2 {
            motion(&mut b, 0);
            b.code("0"); // dmvector 0
        }
    }
    // Row 2: frame prediction (+2, +2) half samples at column 0, column 1 skipped, column 2 coded with no motion
    // compensation but a residual of +16 in block 0 (frame DCT).
    slice(&mut b, 2);
    inc(&mut b, 1);
    b.code("001");
    b.put(0b10, 2);
    motion(&mut b, 2);
    motion(&mut b, 2);
    inc(&mut b, 2);
    b.code("01"); // no MC, coded
    b.put(0, 1); // frame DCT
    b.code(lookup(CODED_BLOCK_PATTERN, 32));
    // One coefficient: DC level 1 in a non-intra block: (2*1+1)*16*16/32 = 24, so +3 after the IDCT.
    b.code("1");
    b.put(0, 1);
    b.code("10");
    b.start(0xB7);
    b.align();

    let frames = decode_all(&b.out);
    assert_eq!(frames.len(), 2);
    let i = &frames[0];
    // The intra frame: field DCT puts blocks 0/1 on even lines and 2/3 on odd lines.
    for y in 0..H {
        for x in 0..W {
            let (mx, my) = (x / 16, y / 16);
            let k = (x % 16) / 8 + 2 * (y % 2);
            assert_eq!(i32::from(i[y * W + x]), value(mx, my, k), "intra ({x}, {y})");
        }
    }
    let field = |s: u32| {
        move |x: i32, y: i32| -> i32 {
            let x = x.clamp(0, W as i32 - 1) as usize;
            let y = y.clamp(0, H as i32 / 2 - 1) as usize;
            i32::from(i[(2 * y + s as usize) * W + x])
        }
    };
    let frame_s = |x: i32, y: i32| -> i32 {
        i32::from(i[y.clamp(0, H as i32 - 1) as usize * W + x.clamp(0, W as i32 - 1) as usize])
    };
    let p = &frames[1];
    for y in 0..16 {
        for x in 0..W {
            let mx = x / 16;
            let ((s0, v0), (s1, v1)) = field_cases[mx];
            let (sel, v) = if y % 2 == 0 { (s0, v0) } else { (s1, v1) };
            let fy = (y / 2) as i32;
            let want = field_sample(&field(sel), 2 * x as i32 + v[0], 2 * fy + v[1]);
            assert_eq!(i32::from(p[y * W + x]), want, "field prediction ({x}, {y})");
        }
    }
    for y in 16..32 {
        for x in 0..W {
            let fy = (y / 2) as i32;
            let par = (y % 2) as u32;
            let x2 = 2 * x as i32;
            // Same-parity field, and the other field half a field line towards this one.
            let same = field_sample(&field(par), x2, 2 * fy);
            let other = if par == 0 {
                field_sample(&field(1), x2, 2 * fy - 1)
            } else {
                field_sample(&field(0), x2, 2 * fy + 1)
            };
            assert_eq!(i32::from(p[y * W + x]), (same + other + 1) >> 1, "dual prime ({x}, {y})");
        }
    }
    for y in 32..48 {
        for x in 0..W {
            let want = match x / 16 {
                0 => field_sample(&frame_s, 2 * x as i32 + 2, 2 * y as i32 + 2),
                1 => i32::from(i[y * W + x]),
                _ => i32::from(i[y * W + x]) + if x % 16 < 8 && y % 16 < 8 { 3 } else { 0 },
            };
            assert_eq!(i32::from(p[y * W + x]), want, "row 2 ({x}, {y})");
        }
    }
    // Row 3 had no slice: the P frame keeps whatever the buffer held; not checked.
}

#[test]
fn field_pictures_second_field_from_first() {
    let mut b = Bits::default();
    sequence(&mut b);
    // Top field: intra. Field macroblock rows are 16 field lines; 2 rows for a 64-line frame.
    picture(&mut b, 1, 1, false);
    intra_rows(&mut b, H / 32, None);
    // Bottom field: P, predicted from the top field of the same frame (motion_vertical_field_select 0).
    picture(&mut b, 2, 2, false);
    let cases = [[0, 1], [3, -2], [-4, 4]];
    for (mx, v) in cases.iter().enumerate() {
        slice(&mut b, 0);
        inc(&mut b, mx + 1);
        b.code("001");
        b.put(0b01, 2); // field prediction
        b.put(0, 1); // from the top field
        motion(&mut b, v[0]);
        motion(&mut b, v[1]);
    }
    // Row 1: 16 x 8 prediction, upper half vector (0, 2), lower half (2, -2), both from the top field.
    // Each half has its own predictor: the first macroblock codes the vectors, the later ones code zero deltas.
    slice(&mut b, 1);
    for mx in 0..W / 16 {
        let (upper, lower) = if mx == 0 { ([0, 2], [2, -2]) } else { ([0, 0], [0, 0]) };
        inc(&mut b, 1);
        b.code("001");
        b.put(0b10, 2);
        b.put(0, 1);
        motion(&mut b, upper[0]);
        motion(&mut b, upper[1]);
        b.put(0, 1);
        motion(&mut b, lower[0]);
        motion(&mut b, lower[1]);
    }
    b.start(0xB7);
    b.align();

    let frames = decode_all(&b.out);
    assert_eq!(frames.len(), 1);
    let f = &frames[0];
    let top = |x: i32, y: i32| -> i32 {
        let x = x.clamp(0, W as i32 - 1) as usize;
        let y = y.clamp(0, H as i32 / 2 - 1) as usize;
        let (mx, my) = (x / 16, y / 16);
        // Frame-DCT blocks within the field: 0/1 above 2/3.
        value(mx, my, (x % 16) / 8 + 2 * ((y % 16) / 8))
    };
    for fy in 0..H / 2 {
        for x in 0..W {
            assert_eq!(i32::from(f[2 * fy * W + x]), top(x as i32, fy as i32), "top field ({x}, {fy})");
            let v = if fy < 16 {
                cases[x / 16]
            } else if fy % 16 < 8 {
                [0, 2]
            } else {
                [2, -2]
            };
            let want = field_sample(&top, 2 * x as i32 + v[0], 2 * fy as i32 + v[1]);
            assert_eq!(i32::from(f[(2 * fy + 1) * W + x]), want, "bottom field ({x}, {fy})");
        }
    }
}

/// Wraps an elementary stream and an audio stream into a program stream: packs of video PES packets (2 KiB of
/// payload each) with an audio packet (sub-stream 0xFF, 4-byte header) after every second video packet.
fn program_stream(video: &[u8], audio: &[u8]) -> Vec<u8> {
    fn pes(out: &mut Vec<u8>, id: u8, payload: &[u8]) {
        let len = 3 + payload.len();
        out.extend_from_slice(&[0, 0, 1, id, (len >> 8) as u8, len as u8, 0x81, 0, 0]);
        out.extend_from_slice(payload);
    }
    let mut out = Vec::new();
    let mut v = video.chunks(2048);
    let mut a = audio.chunks(1500);
    loop {
        let (vc1, vc2, ac) = (v.next(), v.next(), a.next());
        if vc1.is_none() && ac.is_none() {
            break;
        }
        out.extend_from_slice(&[0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0, 0xFF, 0x53, 0xF8]);
        for c in [vc1, vc2].into_iter().flatten() {
            pes(&mut out, 0xE0, c);
        }
        if let Some(c) = ac {
            let mut p = vec![0xFF, 0xA0, 0, 0];
            p.extend_from_slice(c);
            pes(&mut out, 0xBD, &p);
        }
    }
    out.extend_from_slice(&[0, 0, 1, 0xB9]);
    out
}

#[test]
fn player_on_a_synthetic_movie() {
    use video::movies::Region;
    use video::{MoviePlayer, PlayerStatus};
    // 15 intra frames (half a second at 29.97 Hz).
    let mut b = Bits::default();
    sequence(&mut b);
    for _ in 0..15 {
        picture(&mut b, 1, 3, false);
        intra_rows(&mut b, H / 16, Some(false));
    }
    b.start(0xB7);
    b.align();
    // Half a second of audio: blocks of 256 left samples (value = frame index) and 256 right samples (negated).
    let frames = 24_000usize;
    let mut pcm = Vec::new();
    for block in 0..frames.div_ceil(256) {
        for ch in 0..2 {
            for k in 0..256 {
                let i = (block * 256 + k) as i16;
                pcm.extend_from_slice(&(if ch == 0 { i } else { -i }).to_le_bytes());
            }
        }
    }
    let mut audio = Vec::new();
    audio.extend_from_slice(b"SShd");
    for v in [0x18u32, 1, 48_000, 2, 0x200] {
        audio.extend_from_slice(&v.to_le_bytes());
    }
    audio.extend_from_slice(&[0xFF; 8]);
    audio.extend_from_slice(b"SSbd");
    audio.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    audio.extend_from_slice(&pcm);
    let file = program_stream(&b.out, &audio);

    let mut p = MoviePlayer::new(Region::new(&file[..], 0, file.len() as u64));
    let h = p.audio_header().unwrap().unwrap();
    assert_eq!(h.frames(), (pcm.len() / 4) as u64);
    let mut played = 0u64;
    let mut buf = vec![0i16; 1600];
    let mut shown = Vec::new();
    let mut status = PlayerStatus::Playing;
    let mut expect_sample = 0i32;
    for _ in 0..120 {
        let n = p.read_audio(&mut buf).unwrap();
        for pair in buf[..n].as_chunks::<2>().0 {
            assert_eq!((i32::from(pair[0]), i32::from(pair[1])), (expect_sample, -expect_sample));
            expect_sample = (expect_sample + 1) & 0xFFFF;
            if expect_sample == 0x8000 {
                expect_sample = -0x8000;
            }
        }
        played += n as u64 / 2;
        status = p.update(played * 1_000_000_000 / 48_000).unwrap();
        if let Some(f) = p.frame()
            && shown.last() != Some(&f.info.display_index)
        {
            assert_eq!(i32::from(f.y[0]), value(0, 0, 0));
            shown.push(f.info.display_index);
        }
        if status == PlayerStatus::Ended {
            break;
        }
    }
    assert_eq!(status, PlayerStatus::Ended);
    assert_eq!(shown, (0..15).collect::<Vec<u64>>());
    assert_eq!(played, frames.div_ceil(256) as u64 * 256);
    assert_eq!(p.decoder_stats().slice_errors, 0);
}
