//! The default icon: the SlyNX wordmark and the game number, drawn in code. Original project art; no game imagery.
//!
//! The Homebrew Menu shows a 256x256 JPEG.

use crate::jpeg::{Rgb, encode};

pub const ICON_SIZE: usize = 256;

/// 5x7 glyphs; bit 4 of each row is the leftmost pixel.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'l' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'y' => [0x00, 0x11, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        _ => [0; 7],
    }
}

fn text_width(text: &str, scale: usize) -> usize {
    let n = text.chars().count();
    if n == 0 { 0 } else { (n * 6 - 1) * scale }
}

struct Canvas {
    px: Vec<[f32; 3]>,
}

impl Canvas {
    fn fill_rect(&mut self, x0: isize, y0: isize, w: usize, h: usize, color: [f32; 3]) {
        for y in y0.max(0)..(y0 + h as isize).min(ICON_SIZE as isize) {
            for x in x0.max(0)..(x0 + w as isize).min(ICON_SIZE as isize) {
                self.px[y as usize * ICON_SIZE + x as usize] = color;
            }
        }
    }

    fn text(&mut self, text: &str, x: isize, y: isize, scale: usize, color: impl Fn(usize) -> [f32; 3]) {
        for (i, c) in text.chars().enumerate() {
            let gx = x + (i * 6 * scale) as isize;
            for (r, bits) in glyph(c).iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        let px = gx + (col * scale) as isize;
                        let py = y + (r * scale) as isize;
                        self.fill_rect(px, py, scale, scale, color(i));
                    }
                }
            }
        }
    }
}

/// Fully saturated colour for hue `h` in 0..1.
fn hue(h: f32) -> [f32; 3] {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let f = h.fract();
    match h as u32 {
        0 => [1.0, f, 0.0],
        1 => [1.0 - f, 1.0, 0.0],
        2 => [0.0, 1.0, f],
        3 => [0.0, 1.0 - f, 1.0],
        4 => [f, 0.0, 1.0],
        _ => [1.0, 0.0, 1.0 - f],
    }
}

/// The icon as pixels. `number` (1 to 3) is drawn under the wordmark; 0 leaves it out.
pub fn render(number: u8) -> Rgb {
    let n = ICON_SIZE as f32;
    let mut c = Canvas { px: Vec::with_capacity(ICON_SIZE * ICON_SIZE) };
    // Background: deep blue to violet, top to bottom, with faint diagonal bands.
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let t = y as f32 / n;
            let band = if ((x + y) / 24) % 2 == 0 { 0.015 } else { 0.0 };
            c.px.push([0.05 + 0.13 * t + band, 0.06 + 0.02 * t + band, 0.20 + 0.12 * t + band]);
        }
    }
    // Floor stripe.
    c.fill_rect(0, 214, ICON_SIZE, 42, [0.03, 0.03, 0.09]);

    let word = "SlyNX";
    let scale = 8;
    let x = ((ICON_SIZE - text_width(word, scale)) / 2) as isize;
    let y = 46;
    c.text(word, x + 5, y + 5, scale, |_| [0.0, 0.0, 0.02]);
    c.text(word, x, y, scale, |i| hue(0.55 + i as f32 * 0.09).map(|v| 0.55 + 0.45 * v));

    if (1..=3).contains(&number) {
        let digit = char::from(b'0' + number).to_string();
        let s = 12;
        let w = text_width(&digit, s);
        let dx = ((ICON_SIZE - w) / 2) as isize;
        c.text(&digit, dx + 4, 124 + 4, s, |_| [0.0, 0.0, 0.02]);
        c.text(&digit, dx, 124, s, |_| [0.95, 0.85, 0.35]);
    }
    let pixels = c.px.iter().map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
    Rgb { width: ICON_SIZE, height: ICON_SIZE, pixels }
}

/// The icon as a JPEG.
pub fn default_icon(number: u8) -> Vec<u8> {
    encode(&render(number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jpeg::{dimensions, tests::decode, tests::psnr};

    #[test]
    fn icon_is_a_256_jpeg() {
        for n in 0..=3 {
            let img = render(n);
            let jpg = default_icon(n);
            assert_eq!(dimensions(&jpg), Some((256, 256)));
            assert!(jpg.len() < 64 * 1024, "{} bytes", jpg.len());
            assert!(psnr(&img, &decode(&jpg)) > 30.0);
        }
        assert_ne!(render(1), render(2));
    }
}
