//! Plain data types shared by the platform layer and its users. No FFI here, so they build on any target.

/// One vertex of the built-in colour pipeline. Must match `SlynxVertex` in `c/slynx_shim.h`.
///
/// `pos` is in normalised device coordinates: x from -1 (left) to 1 (right), y from -1 (top) to 1 (bottom).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

const _: () = assert!(core::mem::size_of::<Vertex>() == 24);
const _: () = assert!(core::mem::align_of::<Vertex>() == 4);

/// Controller button bits, as reported by libnx's `HidNpadButton` (player 1 plus handheld).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons(pub u64);

impl Buttons {
    pub const A: Buttons = Buttons(1 << 0);
    pub const B: Buttons = Buttons(1 << 1);
    pub const X: Buttons = Buttons(1 << 2);
    pub const Y: Buttons = Buttons(1 << 3);
    pub const L: Buttons = Buttons(1 << 6);
    pub const R: Buttons = Buttons(1 << 7);
    pub const PLUS: Buttons = Buttons(1 << 10);
    pub const MINUS: Buttons = Buttons(1 << 11);
    pub const LEFT: Buttons = Buttons(1 << 12);
    pub const UP: Buttons = Buttons(1 << 13);
    pub const RIGHT: Buttons = Buttons(1 << 14);
    pub const DOWN: Buttons = Buttons(1 << 15);

    /// True if any bit of `other` is set in `self`.
    pub const fn contains(self, other: Buttons) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_contains() {
        let b = Buttons(Buttons::PLUS.0 | Buttons::A.0);
        assert!(b.contains(Buttons::PLUS));
        assert!(b.contains(Buttons::A));
        assert!(!b.contains(Buttons::B));
    }

    #[test]
    fn vertex_layout_matches_c() {
        // x, y at 0 and 4; r, g, b, a from 8.
        let v = Vertex::default();
        let base = &v as *const Vertex as usize;
        assert_eq!(&v.pos as *const _ as usize - base, 0);
        assert_eq!(&v.color as *const _ as usize - base, 8);
    }
}
