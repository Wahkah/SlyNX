//! The backend-neutral output: a list of textured or flat quads in a virtual screen space.
//!
//! **Space.** The UI works in a virtual frame 640 units wide (the original frame width) whose height follows the
//! display's aspect ratio ([`Canvas::for_display`]: 360 for 16:9, 480 for 4:3). x goes right, y **down**, origin top
//! left. The renderer maps a unit to `display_width / 640` pixels.
//!
//! **Order.** Items are drawn in list order with alpha blending (`src_alpha, one_minus_src_alpha`), no depth.
//!
//! **Colour.** `color` multiplies the texel (or is the colour of a flat quad), straight (not premultiplied) alpha,
//! 0..1.

use alloc::vec::Vec;

/// Width of the virtual frame.
pub const VIRTUAL_WIDTH: f32 = 640.0;

/// The virtual screen the UI lays out in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Canvas {
    /// Width (always 640).
    pub width: f32,
    /// Height (640 x display height / display width).
    pub height: f32,
}

impl Canvas {
    /// The canvas for a display of `width` x `height` pixels.
    pub fn for_display(width: u32, height: u32) -> Canvas {
        let h = if width == 0 { 480.0 } else { VIRTUAL_WIDTH * height as f32 / width as f32 };
        Canvas { width: VIRTUAL_WIDTH, height: h }
    }

    /// Display pixels per virtual unit.
    pub fn pixels_per_unit(&self, display_width: u32) -> f32 {
        display_width as f32 / self.width
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas { width: VIRTUAL_WIDTH, height: 480.0 }
    }
}

/// An axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A rectangle.
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Moved by (dx, dy).
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, ..*self }
    }

    /// Grown by `m` on every side.
    pub fn inflate(&self, m: f32) -> Rect {
        Rect { x: self.x - m, y: self.y - m, w: self.w + 2.0 * m, h: self.h + 2.0 * m }
    }
}

/// Which texture a quad samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    /// None: a flat colour.
    None,
    /// The glyph sheet ([`crate::font::FontSet::sheet`]).
    Glyphs,
}

/// One quad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Where, in canvas units.
    pub rect: Rect,
    /// Texture coordinates `[u0, v0, u1, v1]` (0..1), top-left and bottom-right; ignored for [`Texture::None`].
    pub uv: [f32; 4],
    /// Texture.
    pub texture: Texture,
    /// RGBA multiplier.
    pub color: [f32; 4],
}

/// A frame's UI: quads in draw order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    /// The canvas the coordinates refer to.
    pub canvas: Canvas,
    /// Quads, back to front.
    pub quads: Vec<Quad>,
}

impl DrawList {
    /// An empty list for `canvas`.
    pub fn new(canvas: Canvas) -> DrawList {
        DrawList { canvas, quads: Vec::new() }
    }

    /// Removes every quad (keeps the allocation).
    pub fn clear(&mut self) {
        self.quads.clear();
    }

    /// Adds a flat rectangle.
    pub fn rect(&mut self, rect: Rect, color: [f32; 4]) {
        if color[3] > 0.0 && rect.w > 0.0 && rect.h > 0.0 {
            self.quads.push(Quad { rect, uv: [0.0; 4], texture: Texture::None, color });
        }
    }

    /// Adds a rectangle outline of thickness `t`.
    pub fn frame(&mut self, r: Rect, t: f32, color: [f32; 4]) {
        self.rect(Rect::new(r.x, r.y, r.w, t), color);
        self.rect(Rect::new(r.x, r.bottom() - t, r.w, t), color);
        self.rect(Rect::new(r.x, r.y + t, t, r.h - 2.0 * t), color);
        self.rect(Rect::new(r.right() - t, r.y + t, t, r.h - 2.0 * t), color);
    }

    /// Adds a textured quad.
    pub fn textured(&mut self, rect: Rect, texture: Texture, uv: [f32; 4], color: [f32; 4]) {
        if color[3] > 0.0 {
            self.quads.push(Quad { rect, uv, texture, color });
        }
    }

    /// Multiplies the alpha of every quad added since index `from` by `a` (fades a group).
    pub fn fade_from(&mut self, from: usize, a: f32) {
        let from = from.min(self.quads.len());
        for q in &mut self.quads[from..] {
            q.color[3] *= a;
        }
    }

    /// Converts every quad to display pixels for a `width` x `height` display (x right, y down).
    pub fn to_pixels(&self, width: u32) -> Vec<Quad> {
        let s = self.canvas.pixels_per_unit(width);
        self.quads
            .iter()
            .map(|q| Quad { rect: Rect::new(q.rect.x * s, q.rect.y * s, q.rect.w * s, q.rect.h * s), ..*q })
            .collect()
    }
}

/// Colour from 8-bit RGBA.
pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
}

/// Opaque white.
pub const WHITE: [f32; 4] = [1.0; 4];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_and_list() {
        let c = Canvas::for_display(1920, 1080);
        assert_eq!(c.width, 640.0);
        assert_eq!(c.height, 360.0);
        assert_eq!(c.pixels_per_unit(1920), 3.0);
        let mut d = DrawList::new(c);
        d.rect(Rect::new(10.0, 10.0, 20.0, 5.0), WHITE);
        d.rect(Rect::new(10.0, 10.0, 20.0, 5.0), [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(d.quads.len(), 1);
        d.frame(Rect::new(0.0, 0.0, 10.0, 10.0), 1.0, WHITE);
        assert_eq!(d.quads.len(), 5);
        d.fade_from(1, 0.5);
        assert_eq!(d.quads[1].color[3], 0.5);
        assert_eq!(d.quads[0].color[3], 1.0);
        let px = d.to_pixels(1920);
        assert_eq!(px[0].rect, Rect::new(30.0, 30.0, 60.0, 15.0));
    }
}
