//! A small CPU rasterizer for draw lists: the reference the renderer's overlay pass should match, used by the tests
//! and the capture example. Texels are sampled bilinearly at texel centres; blending is straight alpha.

use crate::draw::{DrawList, Texture};
use crate::font::GlyphSheet;

fn sample(sheet: &GlyphSheet, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = (sheet.width as i64, sheet.height as i64);
    let x = u * sheet.width as f32 - 0.5;
    let y = v * sheet.height as f32 - 0.5;
    let (x0, y0) = (crate::floor(x), crate::floor(y));
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: i64, yi: i64| -> [f32; 4] {
        let xi = xi.clamp(0, w - 1);
        let yi = yi.clamp(0, h - 1);
        let o = ((yi * w + xi) * 4) as usize;
        let p = &sheet.pixels[o..o + 4];
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]
    };
    let (xi, yi) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (px(xi, yi), px(xi + 1, yi), px(xi, yi + 1), px(xi + 1, yi + 1));
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bot = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bot - top) * fy;
    }
    out
}

/// Draws `list` over `pixels` (RGBA8, `width` x `height`, rows top to bottom).
pub fn rasterize(list: &DrawList, sheet: &GlyphSheet, width: u32, height: u32, pixels: &mut [u8]) {
    let s = list.canvas.pixels_per_unit(width);
    for q in &list.quads {
        let (x0, y0) = (q.rect.x * s, q.rect.y * s);
        let (x1, y1) = (q.rect.right() * s, q.rect.bottom() * s);
        let px0 = crate::floor(x0).max(0.0) as u32;
        let py0 = crate::floor(y0).max(0.0) as u32;
        let px1 = (crate::floor(x1 + 0.999) as i64).clamp(0, width as i64) as u32;
        let py1 = (crate::floor(y1 + 0.999) as i64).clamp(0, height as i64) as u32;
        for py in py0..py1 {
            let cy = py as f32 + 0.5;
            if cy < y0 || cy >= y1 {
                continue;
            }
            for px in px0..px1 {
                let cx = px as f32 + 0.5;
                if cx < x0 || cx >= x1 {
                    continue;
                }
                let mut c = q.color;
                if q.texture == Texture::Glyphs && !sheet.pixels.is_empty() {
                    let u = q.uv[0] + (q.uv[2] - q.uv[0]) * (cx - x0) / (x1 - x0);
                    let v = q.uv[1] + (q.uv[3] - q.uv[1]) * (cy - y0) / (y1 - y0);
                    let t = sample(sheet, u, v);
                    for k in 0..4 {
                        c[k] *= t[k];
                    }
                }
                let a = c[3].clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let o = ((py * width + px) * 4) as usize;
                for k in 0..3 {
                    let dst = pixels[o + k] as f32 / 255.0;
                    let v = c[k].clamp(0.0, 1.0) * a + dst * (1.0 - a);
                    pixels[o + k] = (v * 255.0 + 0.5) as u8;
                }
                let da = pixels[o + 3] as f32 / 255.0;
                pixels[o + 3] = ((a + da * (1.0 - a)) * 255.0 + 0.5) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Canvas, Rect, WHITE};
    use alloc::vec;

    #[test]
    fn flat_and_blended() {
        let mut d = DrawList::new(Canvas::for_display(64, 48));
        d.rect(Rect::new(0.0, 0.0, 320.0, 240.0), WHITE);
        d.rect(Rect::new(0.0, 0.0, 640.0, 480.0), [1.0, 0.0, 0.0, 0.5]);
        let mut px = vec![0u8; 64 * 48 * 4];
        rasterize(&d, &GlyphSheet::default(), 64, 48, &mut px);
        assert_eq!(&px[0..4], &[255, 128, 128, 255]);
        let o = (47 * 64 + 63) * 4;
        assert_eq!(&px[o..o + 4], &[128, 0, 0, 128]);
    }
}
