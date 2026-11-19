//! A minimal immediate-mode drawing interface for platform demos and bring-up tests. The game renders through the
//! renderer crate, not this; both backends implement it with a single colour pipeline so a demo or a harness smoke
//! test can put pixels on screen before the renderer exists.

/// One vertex of the debug pipeline: position in normalised device coordinates and an RGBA colour.
///
/// `pos`: x from -1 (left) to 1 (right), y from -1 (top) to 1 (bottom) on every backend. `color`: components in
/// 0..=1, written to the 8-bit framebuffer as they are (no gamma conversion), so 0.5 stores 128.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

const _: () = assert!(core::mem::size_of::<Vertex>() == 24);
const _: () = assert!(core::mem::align_of::<Vertex>() == 4);

/// Clears the screen and draws one triangle list per frame.
pub trait DebugDraw {
    type Error: core::fmt::Debug + core::fmt::Display;

    /// Most vertices one frame can draw; extra vertices are dropped, as is a trailing partial triangle.
    fn max_vertices(&self) -> usize;

    /// Clears to `clear`, draws `vertices` as a triangle list over the current render size and presents.
    fn frame(&mut self, clear: [f32; 4], vertices: &[Vertex]) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_layout_matches_c() {
        let v = Vertex::default();
        let base = &v as *const Vertex as usize;
        assert_eq!(&v.pos as *const _ as usize - base, 0);
        assert_eq!(&v.color as *const _ as usize - base, 8);
    }
}
