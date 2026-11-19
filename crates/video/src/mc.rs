//! Motion-compensated prediction with half-sample interpolation (ISO/IEC 13818-2, 7.6.4 and 7.6.7).

/// A view of one plane of a reference frame: the whole frame, or one field of it.
#[derive(Clone, Copy)]
pub(crate) struct PlaneView<'a> {
    pub data: &'a [u8],
    /// Index of the view's first sample.
    pub offset: usize,
    /// Distance between the view's lines.
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
}

impl<'a> PlaneView<'a> {
    /// The frame view of a plane, or the field of parity `field` (0 top, 1 bottom).
    pub fn new(data: &'a [u8], stride: usize, height: usize, field: Option<u8>) -> Self {
        match field {
            None => PlaneView { data, offset: 0, pitch: stride, width: stride, height },
            Some(p) => {
                PlaneView { data, offset: p as usize * stride, pitch: stride * 2, width: stride, height: height / 2 }
            }
        }
    }
}

/// Destination of a prediction: a block in a macroblock-sized buffer.
pub(crate) struct Dst<'a> {
    pub buf: &'a mut [u8],
    pub offset: usize,
    pub pitch: usize,
}

/// Predicts a `W` x `h` block whose top-left sample, in half-sample units of `src`, is (`hx`, `hy`). With `avg`, the
/// prediction is averaged into what `dst` already holds (bidirectional and dual-prime prediction).
#[inline(always)]
pub(crate) fn predict<const W: usize>(dst: &mut Dst<'_>, src: &PlaneView<'_>, hx: i32, hy: i32, h: usize, avg: bool) {
    let ix = hx >> 1;
    let iy = hy >> 1;
    let fx = (hx & 1) as usize;
    let fy = (hy & 1) as usize;
    let inside = ix >= 0
        && iy >= 0
        && ix as usize + W + fx <= src.width
        && iy as usize + h + fy <= src.height
        && src.offset + (iy as usize + h - 1 + fy) * src.pitch + ix as usize + W + fx <= src.data.len();
    if inside {
        let start = src.offset + iy as usize * src.pitch + ix as usize;
        interpolate::<W>(dst, &src.data[start..], src.pitch, h, fx, fy, avg);
    } else {
        // Clamp to the edges (a conforming stream never points outside, but a corrupt one must not crash).
        let mut tmp = [0u8; 17 * 17];
        let tp = W + 1;
        for r in 0..=h {
            let sy = (iy + r as i32).clamp(0, src.height as i32 - 1) as usize;
            for c in 0..tp {
                let sx = (ix + c as i32).clamp(0, src.width as i32 - 1) as usize;
                tmp[r * tp + c] = src.data.get(src.offset + sy * src.pitch + sx).copied().unwrap_or(128);
            }
        }
        interpolate::<W>(dst, &tmp, tp, h, fx, fy, avg);
    }
}

#[inline(always)]
fn store<const W: usize>(d: &mut [u8], p: &[u8; W], avg: bool) {
    let d: &mut [u8; W] = (&mut d[..W]).try_into().unwrap_or_else(|_| unreachable!());
    if avg {
        for (d, &p) in d.iter_mut().zip(p) {
            *d = ((u16::from(*d) + u16::from(p) + 1) >> 1) as u8;
        }
    } else {
        *d = *p;
    }
}

#[inline(always)]
fn interpolate<const W: usize>(dst: &mut Dst<'_>, src: &[u8], pitch: usize, h: usize, fx: usize, fy: usize, avg: bool) {
    let mut row = [0u8; W];
    for r in 0..h {
        let s0 = &src[r * pitch..];
        match (fx, fy) {
            (0, 0) => row.copy_from_slice(&s0[..W]),
            (1, 0) => {
                for (o, (a, b)) in row.iter_mut().zip(s0[..W].iter().zip(&s0[1..=W])) {
                    *o = ((u16::from(*a) + u16::from(*b) + 1) >> 1) as u8;
                }
            }
            (0, _) => {
                let s1 = &src[(r + 1) * pitch..];
                for (o, (a, b)) in row.iter_mut().zip(s0[..W].iter().zip(&s1[..W])) {
                    *o = ((u16::from(*a) + u16::from(*b) + 1) >> 1) as u8;
                }
            }
            _ => {
                let s1 = &src[(r + 1) * pitch..];
                for c in 0..W {
                    let v = u16::from(s0[c]) + u16::from(s0[c + 1]) + u16::from(s1[c]) + u16::from(s1[c + 1]);
                    row[c] = ((v + 2) >> 2) as u8;
                }
            }
        }
        let o = dst.offset + r * dst.pitch;
        store::<W>(&mut dst.buf[o..], &row, avg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane() -> ([u8; 64 * 32], usize) {
        let mut p = [0u8; 64 * 32];
        for (i, v) in p.iter_mut().enumerate() {
            *v = ((i % 64) * 3 + (i / 64) * 5) as u8;
        }
        (p, 64)
    }

    #[test]
    fn full_and_half_sample_positions() {
        let (p, s) = plane();
        let view = PlaneView::new(&p, s, 32, None);
        let mut out = [0u8; 256];
        let mut d = Dst { buf: &mut out, offset: 0, pitch: 16 };
        predict::<16>(&mut d, &view, 4, 6, 16, false);
        assert_eq!(out[0], p[3 * 64 + 2]);
        let mut d = Dst { buf: &mut out, offset: 0, pitch: 16 };
        predict::<16>(&mut d, &view, 5, 7, 16, false);
        let want = (u16::from(p[3 * 64 + 2])
            + u16::from(p[3 * 64 + 3])
            + u16::from(p[4 * 64 + 2])
            + u16::from(p[4 * 64 + 3])
            + 2)
            >> 2;
        assert_eq!(u16::from(out[0]), want);
    }

    #[test]
    fn field_views_and_edges() {
        let (p, s) = plane();
        let bottom = PlaneView::new(&p, s, 32, Some(1));
        let mut out = [0u8; 64];
        let mut d = Dst { buf: &mut out, offset: 0, pitch: 8 };
        predict::<8>(&mut d, &bottom, 0, 2, 8, false);
        assert_eq!(out[0], p[3 * 64]);
        // Far outside: clamped to the corner, no panic.
        let mut d = Dst { buf: &mut out, offset: 0, pitch: 8 };
        predict::<8>(&mut d, &bottom, -1000, 1000, 8, false);
        assert_eq!(out[0], p[31 * 64]);
        let mut d = Dst { buf: &mut out, offset: 0, pitch: 8 };
        predict::<8>(&mut d, &bottom, 0, 0, 8, true);
        assert_eq!(out[0], ((u16::from(p[31 * 64]) + u16::from(p[64]) + 1) >> 1) as u8);
    }
}
