//! RGBA8 images, the common currency of captures, PNG files and comparisons.

use std::fmt;

/// An 8-bit-per-channel RGBA image, rows top to bottom, no padding.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

impl Image {
    /// A new image filled with one colour.
    pub fn new(width: u32, height: u32, fill: [u8; 4]) -> Self {
        let n = width as usize * height as usize;
        let mut pixels = Vec::with_capacity(n * 4);
        for _ in 0..n {
            pixels.extend_from_slice(&fill);
        }
        Image { width, height, pixels }
    }

    /// Wraps existing RGBA8 data. Returns `None` if the length is not `width * height * 4`.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        (pixels.len() == width as usize * height as usize * 4).then_some(Image { width, height, pixels })
    }

    /// Converts tightly packed RGB8 data. Returns `None` if the length is not `width * height * 3`.
    pub fn from_rgb(width: u32, height: u32, rgb: &[u8]) -> Option<Self> {
        if rgb.len() != width as usize * height as usize * 3 {
            return None;
        }
        let pixels = rgb.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        Some(Image { width, height, pixels })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGBA8 bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The raw RGBA8 bytes, mutable.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Consumes the image and returns its bytes.
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    fn index(&self, x: u32, y: u32) -> usize {
        assert!(x < self.width && y < self.height, "pixel ({x}, {y}) outside {}x{}", self.width, self.height);
        (y as usize * self.width as usize + x as usize) * 4
    }

    /// The pixel at `(x, y)`. Panics when out of bounds.
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.index(x, y);
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]]
    }

    /// Sets the pixel at `(x, y)`. Panics when out of bounds.
    pub fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = self.index(x, y);
        self.pixels[i..i + 4].copy_from_slice(&c);
    }

    /// Fills a rectangle, clipped to the image.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, c: [u8; 4]) {
        let x0 = x.clamp(0, self.width as i32) as u32;
        let y0 = y.clamp(0, self.height as i32) as u32;
        let x1 = (x as i64 + w as i64).clamp(0, self.width as i64) as u32;
        let y1 = (y as i64 + h as i64).clamp(0, self.height as i64) as u32;
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.set(xx, yy, c);
            }
        }
    }

    /// Flips the rows (for framebuffers read back bottom-up, as OpenGL does).
    pub fn flip_vertical(&mut self) {
        let row = self.width as usize * 4;
        let h = self.height as usize;
        for y in 0..h / 2 {
            let (top, bottom) = self.pixels.split_at_mut((h - 1 - y) * row);
            top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
        }
    }

    /// Whether every pixel is fully opaque.
    pub fn is_opaque(&self) -> bool {
        self.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_access_and_fill() {
        let mut img = Image::new(4, 3, [1, 2, 3, 4]);
        assert_eq!(img.get(3, 2), [1, 2, 3, 4]);
        img.fill_rect(-1, 1, 3, 10, [9, 9, 9, 255]);
        assert_eq!(img.get(0, 1), [9, 9, 9, 255]);
        assert_eq!(img.get(1, 2), [9, 9, 9, 255]);
        assert_eq!(img.get(2, 2), [1, 2, 3, 4]);
        assert_eq!(img.get(0, 0), [1, 2, 3, 4]);
        assert!(!img.is_opaque());
        assert!(Image::from_rgba(2, 2, vec![0; 15]).is_none());
        let rgb = Image::from_rgb(1, 1, &[5, 6, 7]).unwrap();
        assert_eq!(rgb.get(0, 0), [5, 6, 7, 255]);
    }

    #[test]
    fn flip() {
        let mut img = Image::new(2, 3, [0; 4]);
        img.set(0, 0, [1; 4]);
        img.set(1, 2, [3; 4]);
        img.set(0, 1, [2; 4]);
        img.flip_vertical();
        assert_eq!(img.get(0, 2), [1; 4]);
        assert_eq!(img.get(1, 0), [3; 4]);
        assert_eq!(img.get(0, 1), [2; 4]);
    }
}
