//! Screen-space text: a stats overlay and on-screen messages drawn by the renderer itself.
//!
//! [`TextOverlay::new`] adds a quad mesh, one small texture per glyph (one pool, a layer per glyph) and a fixed
//! number of hidden glyph instances in [`Pass::Overlay`]. [`TextOverlay::set_text`] then places and shows as many
//! of them as the text needs. Everything stays in the renderer's normal path: the glyphs are one draw command with
//! one instance per character, drawn last, with no extra pipeline.
//!
//! The font is a 5 x 7 pixel font drawn here (upper case, digits and common punctuation; lower case letters are
//! shown as upper case). Each glyph texture has a one-pixel black outline so the text reads on any background.

use crate::backend::Backend;
use crate::math::{Mat4, Vec3};
use crate::renderer::Renderer;
use crate::types::{InstanceId, Material, MaterialId, MeshData, MeshId, Pass, RenderError, TextureData, Vertex};
use alloc::vec;
use alloc::vec::Vec;

/// Glyph texture size: the 5 x 7 glyph, a one-pixel outline, and one spare column and row.
pub const CELL_W: u32 = 8;
pub const CELL_H: u32 = 10;
/// Horizontal advance per character and line height, in font pixels (before scaling).
pub const ADVANCE: u32 = 6;
pub const LINE_HEIGHT: u32 = 10;

/// The characters with glyphs, in texture-layer order. Anything else is drawn as `?`.
const CHARS: &str = " !\"#%'()*+,-./0123456789:;<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ[]_|";

/// 5 x 7 glyphs, one string per character of [`CHARS`]: seven rows of five columns, `#` set.
const GLYPHS: [&str; 60] = [
    "..... ..... ..... ..... ..... ..... .....", // space
    "..#.. ..#.. ..#.. ..#.. ..#.. ..... ..#..", // !
    ".#.#. .#.#. ..... ..... ..... ..... .....", // "
    ".#.#. ##### .#.#. .#.#. .#.#. ##### .#.#.", // #
    "##..# ##.#. ...#. ..#.. .#... .#.## #..##", // %
    "..#.. ..#.. ..... ..... ..... ..... .....", // '
    "...#. ..#.. .#... .#... .#... ..#.. ...#.", // (
    ".#... ..#.. ...#. ...#. ...#. ..#.. .#...", // )
    "..... #.#.# .###. ##### .###. #.#.# .....", // *
    "..... ..#.. ..#.. ##### ..#.. ..#.. .....", // +
    "..... ..... ..... ..... .##.. ..#.. .#...", // ,
    "..... ..... ..... ##### ..... ..... .....", // -
    "..... ..... ..... ..... ..... .##.. .##..", // .
    "....# ...#. ...#. ..#.. .#... .#... #....", // /
    ".###. #...# #..## #.#.# ##..# #...# .###.", // 0
    "..#.. .##.. ..#.. ..#.. ..#.. ..#.. .###.", // 1
    ".###. #...# ....# ...#. ..#.. .#... #####", // 2
    "##### ...#. ..#.. ...#. ....# #...# .###.", // 3
    "...#. ..##. .#.#. #..#. ##### ...#. ...#.", // 4
    "##### #.... ####. ....# ....# #...# .###.", // 5
    "..##. .#... #.... ####. #...# #...# .###.", // 6
    "##### ....# ...#. ..#.. .#... .#... .#...", // 7
    ".###. #...# #...# .###. #...# #...# .###.", // 8
    ".###. #...# #...# .#### ....# ...#. .##..", // 9
    "..... .##.. .##.. ..... .##.. .##.. .....", // :
    "..... .##.. .##.. ..... .##.. ..#.. .#...", // ;
    "...#. ..#.. .#... #.... .#... ..#.. ...#.", // <
    "..... ..... ##### ..... ##### ..... .....", // =
    ".#... ..#.. ...#. ....# ...#. ..#.. .#...", // >
    ".###. #...# ....# ...#. ..#.. ..... ..#..", // ?
    ".###. #...# #...# ##### #...# #...# #...#", // A
    "####. #...# #...# ####. #...# #...# ####.", // B
    ".###. #...# #.... #.... #.... #...# .###.", // C
    "###.. #..#. #...# #...# #...# #..#. ###..", // D
    "##### #.... #.... ####. #.... #.... #####", // E
    "##### #.... #.... ####. #.... #.... #....", // F
    ".###. #...# #.... #.### #...# #...# .####", // G
    "#...# #...# #...# ##### #...# #...# #...#", // H
    ".###. ..#.. ..#.. ..#.. ..#.. ..#.. .###.", // I
    "..### ...#. ...#. ...#. ...#. #..#. .##..", // J
    "#...# #..#. #.#.. ##... #.#.. #..#. #...#", // K
    "#.... #.... #.... #.... #.... #.... #####", // L
    "#...# ##.## #.#.# #.#.# #...# #...# #...#", // M
    "#...# #...# ##..# #.#.# #..## #...# #...#", // N
    ".###. #...# #...# #...# #...# #...# .###.", // O
    "####. #...# #...# ####. #.... #.... #....", // P
    ".###. #...# #...# #...# #.#.# #..#. .##.#", // Q
    "####. #...# #...# ####. #.#.. #..#. #...#", // R
    ".#### #.... #.... .###. ....# ....# ####.", // S
    "##### ..#.. ..#.. ..#.. ..#.. ..#.. ..#..", // T
    "#...# #...# #...# #...# #...# #...# .###.", // U
    "#...# #...# #...# #...# #...# .#.#. ..#..", // V
    "#...# #...# #...# #.#.# #.#.# #.#.# .#.#.", // W
    "#...# #...# .#.#. ..#.. .#.#. #...# #...#", // X
    "#...# #...# .#.#. ..#.. ..#.. ..#.. ..#..", // Y
    "##### ....# ...#. ..#.. .#... #.... #####", // Z
    ".###. .#... .#... .#... .#... .#... .###.", // [
    ".###. ...#. ...#. ...#. ...#. ...#. .###.", // ]
    "..... ..... ..... ..... ..... ..... #####", // _
    "..#.. ..#.. ..#.. ..#.. ..#.. ..#.. ..#..", // |
];

const _: () = assert!(CHARS.len() == GLYPHS.len());

/// Layer (glyph index) for a character: lower case maps to upper case, unknown characters to `?`.
pub fn glyph_index(c: char) -> usize {
    let c = c.to_ascii_uppercase();
    CHARS.find(c).unwrap_or_else(|| CHARS.find('?').unwrap_or(0))
}

/// The RGBA8 texture of glyph `index`: white glyph pixels, a black outline around them, transparent elsewhere.
pub fn glyph_texture(index: usize) -> Vec<u8> {
    let (w, h) = (CELL_W as usize, CELL_H as usize);
    let mut set = vec![false; w * h];
    for (row, bits) in GLYPHS[index].split(' ').enumerate() {
        for (col, b) in bits.bytes().enumerate() {
            if b == b'#' {
                set[(row + 1) * w + col + 1] = true;
            }
        }
    }
    let mut px = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let near = |dx: isize, dy: isize| {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && set[ny as usize * w + nx as usize]
            };
            let rgba = if set[i] {
                [255, 255, 255, 255]
            } else if (-1..=1).any(|dy| (-1..=1).any(|dx| near(dx, dy))) {
                [0, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            };
            px[i * 4..i * 4 + 4].copy_from_slice(&rgba);
        }
    }
    px
}

/// Screen-space text drawn with the renderer.
pub struct TextOverlay {
    quad: MeshId,
    glyphs: Vec<MaterialId>,
    instances: Vec<InstanceId>,
    shown: usize,
}

impl TextOverlay {
    /// Adds the overlay's mesh, glyph textures, materials and `max_chars` hidden instances. Call it while the
    /// level's resources are being added (before [`Renderer::commit`]), once per level.
    pub fn new<B: Backend>(r: &mut Renderer<B>, max_chars: usize) -> Result<TextOverlay, RenderError> {
        // Unit quad, (0, 0) top left to (1, 1) bottom right; texture row 0 is the glyph's top row.
        let n = [0.0, 0.0, 1.0];
        let white = [255; 4];
        let v = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], n, white),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], n, white),
            Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0], n, white),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], n, white),
        ];
        let quad = r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2, 0, 2, 3] })?;
        let mut glyphs = Vec::with_capacity(GLYPHS.len());
        for g in 0..GLYPHS.len() {
            let t = r.add_texture(TextureData::rgba8(CELL_W, CELL_H, glyph_texture(g))?)?;
            glyphs.push(r.add_material(Material {
                lighting: crate::Lighting::Unlit,
                ..Material::opaque(t).with_pass(Pass::Overlay)
            })?);
        }
        let mut instances = Vec::with_capacity(max_chars);
        for _ in 0..max_chars {
            let id = r.add_instance(quad, glyphs[0], Mat4::IDENTITY)?;
            r.set_visible(id, false)?;
            r.set_nearest(id, true)?;
            instances.push(id);
        }
        Ok(TextOverlay { quad, glyphs, instances, shown: 0 })
    }

    /// The overlay's quad mesh (for tests and diagnostics).
    pub fn mesh(&self) -> MeshId {
        self.quad
    }

    /// Most characters that can be shown at once.
    pub fn capacity(&self) -> usize {
        self.instances.len()
    }

    /// Characters shown by the last [`TextOverlay::set_text`] (spaces are not drawn).
    pub fn shown(&self) -> usize {
        self.shown
    }

    /// Shows `lines` with the top-left corner at pixel (`x`, `y`) of the render target, each font pixel `scale`
    /// screen pixels, in `color`. Characters past the capacity are dropped. Call it every frame the text should
    /// change (the screen size is read from the backend, so docking is followed).
    pub fn set_text<B: Backend>(
        &mut self,
        r: &mut Renderer<B>,
        lines: &[&str],
        x: u32,
        y: u32,
        scale: u32,
        color: [u8; 4],
    ) -> Result<(), RenderError> {
        let (w, h) = r.backend().target_size();
        let clip = r.backend().clip_space();
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        // Pixel (px, py), y down, to clip space.
        let sx = 2.0 / w;
        let sy = if clip.y_down { 2.0 / h } else { -2.0 / h };
        let y0 = if clip.y_down { -1.0 } else { 1.0 };
        let (cw, ch) = ((CELL_W * scale) as f32 * sx, (CELL_H * scale) as f32 * sy);

        let mut n = 0;
        for (row, line) in lines.iter().enumerate() {
            let py = (y + row as u32 * LINE_HEIGHT * scale) as f32;
            for (col, c) in line.chars().enumerate() {
                if c == ' ' {
                    continue;
                }
                let Some(&id) = self.instances.get(n) else { break };
                // The glyph sits one font pixel in from the cell's corner (the outline).
                let px = (x + col as u32 * ADVANCE * scale) as f32 - scale as f32;
                let py = py - scale as f32;
                let mut m = Mat4::scale(Vec3::new(cw, ch, 1.0));
                m.m[12] = -1.0 + px * sx;
                m.m[13] = y0 + py * sy;
                r.set_transform(id, m)?;
                r.set_material(id, self.glyphs[glyph_index(c)])?;
                r.set_tint(id, color)?;
                r.set_visible(id, true)?;
                n += 1;
            }
        }
        for &id in &self.instances[n..self.shown.max(n)] {
            r.set_visible(id, false)?;
        }
        self.shown = n;
        Ok(())
    }

    /// Hides all text.
    pub fn clear<B: Backend>(&mut self, r: &mut Renderer<B>) -> Result<(), RenderError> {
        for &id in &self.instances[..self.shown] {
            r.set_visible(id, false)?;
        }
        self.shown = 0;
        Ok(())
    }
}

/// Width in screen pixels of `chars` characters at `scale`.
pub fn text_width(chars: usize, scale: u32) -> u32 {
    chars as u32 * ADVANCE * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::frame::INSTANCE_SCREEN;
    use crate::renderer::Camera;

    #[test]
    fn glyphs_are_well_formed() {
        for (i, g) in GLYPHS.iter().enumerate() {
            let rows: Vec<&str> = g.split(' ').collect();
            assert_eq!(rows.len(), 7, "glyph {i}");
            assert!(rows.iter().all(|r| r.len() == 5 && r.bytes().all(|b| b == b'#' || b == b'.')), "glyph {i}");
        }
        assert_eq!(glyph_index('a'), glyph_index('A'));
        assert_eq!(glyph_index('~'), glyph_index('?'));
        // 'I': top bar row 1 (texture row 2 with the outline), columns 1..=3 of the glyph.
        let t = glyph_texture(glyph_index('I'));
        let at = |x: usize, y: usize| &t[(y * CELL_W as usize + x) * 4..][..4];
        assert_eq!(at(2, 1), [255, 255, 255, 255]);
        assert_eq!(at(1, 1), [0, 0, 0, 255]); // outline left of the bar
        assert_eq!(at(7, 9), [0, 0, 0, 0]);
    }

    #[test]
    fn text_places_one_instance_per_character() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let mut o = TextOverlay::new(&mut r, 16).unwrap();
        r.commit().unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        o.set_text(&mut r, &["FPS 60", "OK"], 8, 8, 2, [255; 4]).unwrap();
        assert_eq!(o.shown(), 7);
        let s = r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!(s.visible, 7);
        assert_eq!(s.draw_commands, 1);
        let nb = r.backend();
        assert!(nb.last_instances.iter().all(|i| i.flags() & INSTANCE_SCREEN != 0));
        // First glyph: one font pixel up and left of (8, 8) -> pixel (6, 6) -> clip (-1 + 6/320, 1 - 6/240).
        let rows = nb.last_instances[0].rows;
        assert!((rows[0][3] - (-1.0 + 6.0 / 320.0)).abs() < 1e-6);
        assert!((rows[1][3] - (1.0 - 6.0 / 240.0)).abs() < 1e-6);
        assert!((rows[0][0] - 16.0 / 320.0).abs() < 1e-6);
        assert!((rows[1][1] + 20.0 / 240.0).abs() < 1e-6);

        // Shorter text hides the rest; overflow is dropped.
        o.set_text(&mut r, &["A"], 0, 0, 1, [255; 4]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 1);
        o.set_text(&mut r, &["ABCDEFGHIJKLMNOPQRSTUVWXYZ"], 0, 0, 1, [255; 4]).unwrap();
        assert_eq!(o.shown(), 16);
        o.clear(&mut r).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 0);
    }
}
