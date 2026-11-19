//! A small baseline JPEG encoder (for the generated icon) and a header reader (to check a player's own icon).
//!
//! Baseline sequential DCT, 8-bit, three components (Y, Cb, Cr) without subsampling, one scan. The Huffman tables
//! are built for each image from its own symbol counts (ITU-T T.81 Annex K.2, code lengths limited to 16 bits), so
//! no stock tables are involved; the decoder reads them from the file like any others.

/// Zigzag scan order (T.81 figure A.6): coefficient `k` of the scan is at `ZIGZAG[k]` in row-major order. Walks
/// the anti-diagonals `row + col = d`, going up and to the right on even diagonals and down and to the left on odd
/// ones.
const ZIGZAG: [usize; 64] = zigzag();

const fn zigzag() -> [usize; 64] {
    let mut order = [0usize; 64];
    let mut k = 0;
    let mut d = 0usize;
    while d < 15 {
        let lo = d.saturating_sub(7);
        let hi = if d < 7 { d } else { 7 };
        let mut i = 0;
        while i <= hi - lo {
            let row = if d % 2 == 0 { hi - i } else { lo + i };
            order[k] = row * 8 + (d - row);
            k += 1;
            i += 1;
        }
        d += 1;
    }
    order
}

/// Quantisation step for frequency (u, v): fine at low frequencies, coarser towards the high ones. Chroma is
/// quantised a little more than luma.
fn quant_tables() -> [[u16; 64]; 2] {
    let mut t = [[0u16; 64]; 2];
    for (k, table) in t.iter_mut().enumerate() {
        for v in 0..8 {
            for u in 0..8 {
                table[v * 8 + u] = (2 + (u + v) as u16 * (2 + k as u16)).min(255);
            }
        }
    }
    t
}

/// An RGB8 image, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 3]>,
}

fn dct_basis() -> [[f32; 8]; 8] {
    // c(x, u) = alpha(u) * cos((2x + 1) u pi / 16)
    let mut b = [[0f32; 8]; 8];
    for (x, row) in b.iter_mut().enumerate() {
        for (u, v) in row.iter_mut().enumerate() {
            let a = if u == 0 { (1.0f32 / 8.0).sqrt() } else { (2.0f32 / 8.0).sqrt() };
            *v = a * (((2 * x + 1) as f32 * u as f32 * core::f32::consts::PI) / 16.0).cos();
        }
    }
    b
}

/// Forward DCT of one 8x8 block (level-shifted samples), separable.
fn fdct(block: &[f32; 64], basis: &[[f32; 8]; 8]) -> [f32; 64] {
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            tmp[y * 8 + u] = (0..8).map(|x| block[y * 8 + x] * basis[x][u]).sum();
        }
    }
    let mut out = [0f32; 64];
    for v in 0..8 {
        for u in 0..8 {
            out[v * 8 + u] = (0..8).map(|y| tmp[y * 8 + u] * basis[y][v]).sum();
        }
    }
    out
}

/// Number of bits needed for |v| (the JPEG "category").
fn category(v: i32) -> u8 {
    (32 - v.unsigned_abs().leading_zeros()) as u8
}

/// Low `size` bits of the amplitude code for `v` (one's complement for negatives).
fn amplitude_bits(v: i32, size: u8) -> u32 {
    let raw = if v < 0 { v - 1 } else { v };
    (raw as u32) & ((1u32 << size) - 1)
}

/// A Huffman table: code counts per length 1..=16 and the symbols in code order, plus the code of every symbol.
#[derive(Clone, Debug)]
pub struct Huffman {
    pub bits: [u8; 16],
    pub values: Vec<u8>,
    codes: [(u16, u8); 256],
}

impl Huffman {
    /// Builds a table from symbol counts (T.81 Annex K.2): Huffman code lengths with a reserved symbol so no code
    /// is all ones, then lengths over 16 folded back.
    pub fn from_counts(counts: &[u32; 256]) -> Huffman {
        let mut freq = [0u64; 257];
        for (f, &c) in freq.iter_mut().zip(counts) {
            *f = c as u64;
        }
        freq[256] = 1;
        let mut size = [0usize; 257];
        let mut others = [usize::MAX; 257];
        loop {
            // Least frequency, the larger symbol on ties; then the next least.
            let mut v1 = usize::MAX;
            for i in 0..257 {
                if freq[i] > 0 && (v1 == usize::MAX || freq[i] <= freq[v1]) {
                    v1 = i;
                }
            }
            let mut v2 = usize::MAX;
            for i in 0..257 {
                if freq[i] > 0 && i != v1 && (v2 == usize::MAX || freq[i] <= freq[v2]) {
                    v2 = i;
                }
            }
            if v2 == usize::MAX {
                break;
            }
            freq[v1] += freq[v2];
            freq[v2] = 0;
            let mut a = v1;
            size[a] += 1;
            while others[a] != usize::MAX {
                a = others[a];
                size[a] += 1;
            }
            others[a] = v2;
            let mut b = v2;
            size[b] += 1;
            while others[b] != usize::MAX {
                b = others[b];
                size[b] += 1;
            }
        }
        let mut bits = [0u32; 33];
        for &s in &size {
            if s > 0 {
                bits[s.min(32)] += 1;
            }
        }
        for i in (17..=32).rev() {
            while bits[i] > 0 {
                let mut j = i - 2;
                while bits[j] == 0 {
                    j -= 1;
                }
                bits[i] -= 2;
                bits[i - 1] += 1;
                bits[j + 1] += 2;
                bits[j] -= 1;
            }
        }
        // Drop the reserved symbol (one of the longest codes).
        let mut i = 16;
        while bits[i] == 0 {
            i -= 1;
        }
        bits[i] -= 1;

        let mut order: Vec<usize> = (0..256).filter(|&s| size[s] > 0).collect();
        order.sort_by_key(|&s| (size[s], s));
        let mut out_bits = [0u8; 16];
        for l in 1..=16 {
            out_bits[l - 1] = bits[l] as u8;
        }
        let values: Vec<u8> = order.iter().map(|&s| s as u8).collect();
        Huffman::from_spec(out_bits, values)
    }

    /// Canonical codes from counts per length and symbols (T.81 Annex C).
    pub fn from_spec(bits: [u8; 16], values: Vec<u8>) -> Huffman {
        let mut codes = [(0u16, 0u8); 256];
        let mut code = 0u32;
        let mut k = 0;
        for (l, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                if let Some(&sym) = values.get(k) {
                    codes[sym as usize] = (code as u16, l as u8 + 1);
                }
                code += 1;
                k += 1;
            }
            code <<= 1;
        }
        Huffman { bits, values, codes }
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, len: u8) {
        for i in (0..len).rev() {
            self.acc = (self.acc << 1) | ((code >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                let b = self.acc as u8;
                self.out.push(b);
                if b == 0xFF {
                    self.out.push(0); // byte stuffing
                }
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        while self.n != 0 {
            self.put(1, 1); // pad with ones
        }
        self.out
    }
}

/// One component's quantised blocks, in zigzag order.
type Blocks = Vec<[i32; 64]>;

fn quantised_blocks(img: &Rgb, quant: &[[u16; 64]; 2]) -> [Blocks; 3] {
    let basis = dct_basis();
    let bw = img.width.div_ceil(8);
    let bh = img.height.div_ceil(8);
    let mut out: [Blocks; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for by in 0..bh {
        for bx in 0..bw {
            let mut planes = [[0f32; 64]; 3];
            for y in 0..8 {
                for x in 0..8 {
                    // Edge pixels repeat past the image.
                    let px = (bx * 8 + x).min(img.width - 1);
                    let py = (by * 8 + y).min(img.height - 1);
                    let [r, g, b] = img.pixels[py * img.width + px].map(|c| c as f32);
                    planes[0][y * 8 + x] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    planes[1][y * 8 + x] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    planes[2][y * 8 + x] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                }
            }
            for c in 0..3 {
                let coef = fdct(&planes[c], &basis);
                let q = &quant[(c > 0) as usize];
                let mut z = [0i32; 64];
                for (k, &nat) in ZIGZAG.iter().enumerate() {
                    z[k] = (coef[nat] / q[nat] as f32).round() as i32;
                }
                out[c].push(z);
            }
        }
    }
    out
}

/// Visits the Huffman symbols (and amplitude bits) of one block: `f(is_ac, symbol, amplitude, amplitude_len)`.
fn block_symbols(block: &[i32; 64], pred: i32, mut f: impl FnMut(bool, u8, u32, u8)) {
    let diff = block[0] - pred;
    let s = category(diff);
    f(false, s, amplitude_bits(diff, s), s);
    let mut run = 0u8;
    for &v in &block[1..] {
        if v == 0 {
            run += 1;
            continue;
        }
        while run >= 16 {
            f(true, 0xF0, 0, 0);
            run -= 16;
        }
        let s = category(v);
        f(true, (run << 4) | s, amplitude_bits(v, s), s);
        run = 0;
    }
    if run > 0 {
        f(true, 0x00, 0, 0);
    }
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// Encodes `img` as a baseline JPEG.
pub fn encode(img: &Rgb) -> Vec<u8> {
    assert!(img.width > 0 && img.height > 0 && img.width < 65536 && img.height < 65536);
    assert_eq!(img.pixels.len(), img.width * img.height);
    let quant = quant_tables();
    let blocks = quantised_blocks(img, &quant);

    // Pass 1: count symbols. Tables: 0 = luma DC, 1 = luma AC, 2 = chroma DC, 3 = chroma AC.
    let mut counts = [[0u32; 256]; 4];
    let mcus = || blocks[0].iter().zip(&blocks[1]).zip(&blocks[2]).map(|((y, cb), cr)| [y, cb, cr]);
    let mut pred = [0i32; 3];
    for mcu in mcus() {
        for (c, block) in mcu.into_iter().enumerate() {
            let t = if c == 0 { 0 } else { 2 };
            block_symbols(block, pred[c], |ac, sym, _, _| counts[t + ac as usize][sym as usize] += 1);
            pred[c] = block[0];
        }
    }
    let tables: Vec<Huffman> = counts.iter().map(Huffman::from_counts).collect();

    // Pass 2: entropy-code.
    let mut bw = BitWriter { out: Vec::new(), acc: 0, n: 0 };
    let mut pred = [0i32; 3];
    for mcu in mcus() {
        for (c, block) in mcu.into_iter().enumerate() {
            let t = if c == 0 { 0 } else { 2 };
            block_symbols(block, pred[c], |ac, sym, amp, len| {
                let (code, clen) = tables[t + ac as usize].codes[sym as usize];
                bw.put(code as u32, clen);
                if len > 0 {
                    bw.put(amp, len);
                }
            });
            pred[c] = block[0];
        }
    }
    let scan = bw.finish();

    let mut out = vec![0xFF, 0xD8];
    segment(&mut out, 0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    for (id, q) in quant.iter().enumerate() {
        let mut body = vec![id as u8];
        body.extend(ZIGZAG.iter().map(|&n| q[n] as u8));
        segment(&mut out, 0xDB, &body);
    }
    let (w, h) = (img.width as u16, img.height as u16);
    let mut sof = vec![8];
    sof.extend_from_slice(&h.to_be_bytes());
    sof.extend_from_slice(&w.to_be_bytes());
    sof.extend_from_slice(&[3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    for (i, t) in tables.iter().enumerate() {
        let class_id = [0x00, 0x10, 0x01, 0x11][i];
        let mut body = vec![class_id];
        body.extend_from_slice(&t.bits);
        body.extend_from_slice(&t.values);
        segment(&mut out, 0xC4, &body);
    }
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    out.extend_from_slice(&scan);
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// Width and height from a JPEG's frame header, or `None` if `data` is not a JPEG this reader understands.
pub fn dimensions(data: &[u8]) -> Option<(u16, u16)> {
    if data.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut p = 2;
    loop {
        while *data.get(p)? == 0xFF && *data.get(p + 1)? == 0xFF {
            p += 1;
        }
        if *data.get(p)? != 0xFF {
            return None;
        }
        let marker = *data.get(p + 1)?;
        let len = u16::from_be_bytes([*data.get(p + 2)?, *data.get(p + 3)?]) as usize;
        // SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let h = u16::from_be_bytes([*data.get(p + 5)?, *data.get(p + 6)?]);
            let w = u16::from_be_bytes([*data.get(p + 7)?, *data.get(p + 8)?]);
            return Some((w, h));
        }
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        p += 2 + len;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal baseline decoder (independent of the encoder's code paths), for checking the encoder: parses the
    /// markers, decodes Huffman symbols bit by bit from the file's tables, dequantises, inverse DCT, converts back.
    pub(crate) fn decode(data: &[u8]) -> Rgb {
        let mut q = [[0u16; 64]; 4];
        let mut huff: Vec<Option<Vec<(u16, u8, u8)>>> = vec![None; 8]; // (code, len, symbol)
        let (mut w, mut h) = (0usize, 0usize);
        let mut comp_q = [0usize; 3];
        let mut comp_tables = [(0usize, 0usize); 3];
        let mut p = 2;
        let scan_start;
        loop {
            assert_eq!(data[p], 0xFF);
            let m = data[p + 1];
            let len = u16::from_be_bytes([data[p + 2], data[p + 3]]) as usize;
            let body = &data[p + 4..p + 2 + len];
            match m {
                0xDB => {
                    let id = (body[0] & 15) as usize;
                    for k in 0..64 {
                        q[id][ZIGZAG[k]] = body[1 + k] as u16;
                    }
                }
                0xC0 => {
                    h = u16::from_be_bytes([body[1], body[2]]) as usize;
                    w = u16::from_be_bytes([body[3], body[4]]) as usize;
                    assert_eq!(body[5], 3);
                    for c in 0..3 {
                        assert_eq!(body[6 + c * 3 + 1], 0x11, "no subsampling");
                        comp_q[c] = body[6 + c * 3 + 2] as usize;
                    }
                }
                0xC4 => {
                    let class = (body[0] >> 4) as usize;
                    let id = (body[0] & 15) as usize;
                    let counts = &body[1..17];
                    let syms = &body[17..];
                    let mut code = 0u16;
                    let mut k = 0;
                    let mut list = Vec::new();
                    for (l, &n) in counts.iter().enumerate() {
                        for _ in 0..n {
                            list.push((code, l as u8 + 1, syms[k]));
                            code += 1;
                            k += 1;
                        }
                        code <<= 1;
                    }
                    huff[class * 4 + id] = Some(list);
                }
                0xDA => {
                    for c in 0..3 {
                        let t = body[2 + c * 2];
                        comp_tables[c] = ((t >> 4) as usize, (t & 15) as usize);
                    }
                    scan_start = p + 2 + len;
                    break;
                }
                _ => {}
            }
            p += 2 + len;
        }
        // Unstuff the scan.
        let mut bytes = Vec::new();
        let mut i = scan_start;
        while !(data[i] == 0xFF && data[i + 1] == 0xD9) {
            bytes.push(data[i]);
            i += if data[i] == 0xFF { 2 } else { 1 };
        }
        let mut bitpos = 0usize;
        let mut bit = |bytes: &[u8]| {
            let b = (bytes[bitpos / 8] >> (7 - bitpos % 8)) & 1;
            bitpos += 1;
            b as u16
        };
        let sym = |bytes: &[u8], t: &Vec<(u16, u8, u8)>, bit: &mut dyn FnMut(&[u8]) -> u16| -> u8 {
            let (mut code, mut len) = (0u16, 0u8);
            loop {
                code = (code << 1) | bit(bytes);
                len += 1;
                if let Some(e) = t.iter().find(|e| e.0 == code && e.1 == len) {
                    return e.2;
                }
                assert!(len < 16, "bad code");
            }
        };
        let extend = |v: u16, s: u8| -> i32 {
            if s == 0 {
                0
            } else if v < (1 << (s - 1)) {
                v as i32 - (1 << s) + 1
            } else {
                v as i32
            }
        };
        let basis = dct_basis();
        let (bw, bh) = (w.div_ceil(8), h.div_ceil(8));
        let mut planes = vec![vec![0f32; bw * 8 * bh * 8]; 3];
        let mut pred = [0i32; 3];
        for by in 0..bh {
            for bx in 0..bw {
                for c in 0..3 {
                    let dc_t = huff[comp_tables[c].0].as_ref().unwrap();
                    let ac_t = huff[4 + comp_tables[c].1].as_ref().unwrap();
                    let mut z = [0i32; 64];
                    let s = sym(&bytes, dc_t, &mut bit);
                    let mut v = 0u16;
                    for _ in 0..s {
                        v = (v << 1) | bit(&bytes);
                    }
                    pred[c] += extend(v, s);
                    z[0] = pred[c];
                    let mut k = 1;
                    while k < 64 {
                        let rs = sym(&bytes, ac_t, &mut bit);
                        if rs == 0 {
                            break;
                        }
                        if rs == 0xF0 {
                            k += 16;
                            continue;
                        }
                        k += (rs >> 4) as usize;
                        let s = rs & 15;
                        let mut v = 0u16;
                        for _ in 0..s {
                            v = (v << 1) | bit(&bytes);
                        }
                        z[k] = extend(v, s);
                        k += 1;
                    }
                    let qt = &q[comp_q[c]];
                    let mut coef = [0f32; 64];
                    for k in 0..64 {
                        coef[ZIGZAG[k]] = (z[k] * qt[ZIGZAG[k]] as i32) as f32;
                    }
                    for y in 0..8 {
                        for x in 0..8 {
                            let mut s = 0f32;
                            for v in 0..8 {
                                for u in 0..8 {
                                    s += coef[v * 8 + u] * basis[x][u] * basis[y][v];
                                }
                            }
                            planes[c][(by * 8 + y) * bw * 8 + bx * 8 + x] = s;
                        }
                    }
                }
            }
        }
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let i = y * bw * 8 + x;
                let (yy, cb, cr) = (planes[0][i] + 128.0, planes[1][i], planes[2][i]);
                let r = yy + 1.402 * cr;
                let g = yy - 0.344_136 * cb - 0.714_136 * cr;
                let b = yy + 1.772 * cb;
                pixels.push([r, g, b].map(|v| v.round().clamp(0.0, 255.0) as u8));
            }
        }
        Rgb { width: w, height: h, pixels }
    }

    pub(crate) fn psnr(a: &Rgb, b: &Rgb) -> f64 {
        let mse: f64 = a
            .pixels
            .iter()
            .zip(&b.pixels)
            .flat_map(|(p, q)| (0..3).map(move |c| (p[c] as f64 - q[c] as f64).powi(2)))
            .sum::<f64>()
            / (a.pixels.len() * 3) as f64;
        10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
    }

    fn test_image(w: usize, h: usize) -> Rgb {
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let edge = if (x / 9 + y / 7) % 2 == 0 { 40 } else { 0 };
                pixels.push([(x * 255 / w) as u8, (y * 255 / h) as u8, (128 + edge) as u8]);
            }
        }
        Rgb { width: w, height: h, pixels }
    }

    #[test]
    fn round_trip() {
        for (w, h) in [(256, 256), (21, 13), (8, 8), (1, 1)] {
            let img = test_image(w, h);
            let jpg = encode(&img);
            assert_eq!(dimensions(&jpg), Some((w as u16, h as u16)));
            assert_eq!(&jpg[jpg.len() - 2..], &[0xFF, 0xD9]);
            let back = decode(&jpg);
            assert_eq!((back.width, back.height), (w, h));
            let p = psnr(&img, &back);
            assert!(p > 34.0, "{w}x{h}: PSNR {p:.1} dB");
        }
    }

    #[test]
    fn flat_image_uses_tiny_tables() {
        let img = Rgb { width: 64, height: 64, pixels: vec![[10, 200, 30]; 64 * 64] };
        let back = decode(&encode(&img));
        assert!(psnr(&img, &back) > 40.0);
    }

    #[test]
    fn huffman_lengths_are_limited() {
        // Fibonacci-like counts make a maximally skewed tree (lengths far over 16 before folding).
        let mut counts = [0u32; 256];
        let (mut a, mut b) = (1u32, 1u32);
        for c in counts.iter_mut().take(40) {
            *c = a;
            (a, b) = (b, a.saturating_add(b));
        }
        let t = Huffman::from_counts(&counts);
        let total: u32 = t.bits.iter().map(|&n| n as u32).sum();
        assert_eq!(total as usize, t.values.len());
        assert_eq!(t.values.len(), 40);
        // Kraft: the codes fit, with the all-ones code unused.
        let kraft: f64 = t.bits.iter().enumerate().map(|(l, &n)| n as f64 / 2f64.powi(l as i32 + 1)).sum();
        assert!(kraft < 1.0, "{kraft}");
    }

    #[test]
    fn zigzag_order() {
        // Starts (0,0) (0,1) (1,0) (2,0) (1,1) (0,2); ends (7,6) (7,7); visits every position once.
        assert_eq!(&ZIGZAG[..6], &[0, 1, 8, 16, 9, 2]);
        assert_eq!(&ZIGZAG[62..], &[62, 63]);
        let mut seen = [false; 64];
        for &z in &ZIGZAG {
            assert!(!seen[z]);
            seen[z] = true;
        }
    }

    #[test]
    fn dimension_reader() {
        assert_eq!(dimensions(b"not a jpeg"), None);
        assert_eq!(dimensions(&[0xFF, 0xD8, 0xFF, 0xD9]), None);
    }
}
