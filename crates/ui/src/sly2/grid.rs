//! The gadget assignment screen (Sly 2 objects 885 to 887; safehouse-and-shop.md 6): the current character's grid
//! of item cells, the selected cell, the three slots (L1, L2, R2) and a message. The game keeps the selection and
//! the rules (`engine::gang::Assignment`); this draws a view of it. It pauses the game.
//!
//! Each cell shows its item's icon from the screen's own art: the cell's sprite in one of its three labels (open,
//! owned, selected: the cell record's three screen parts), baked into the glyph sheet as the icon font
//! ([`ICON_FONT`]) and placed where the screen's idle label puts it. The screen's 4:3 canvas (640 × 480) is scaled to
//! fit and centred (user-interface.md 3). Without the art (no disc data), cells are plain squares at the
//! executable's positions.

use alloc::string::String;
use alloc::vec::Vec;

use super::font::FontSheet;
use super::jobs::Screen;
use crate::draw::{DrawList, Rect};

/// Text size [working choice].
pub const SIZE: f32 = 16.0;
/// Seconds a message stays [working choice].
pub const MESSAGE_TIME: f32 = 2.0;
/// The glyph sheet's font holding the screens' baked art.
pub const ICON_FONT: usize = 2;
/// Font units per screen unit in the baked art.
pub const ICON_SCALE: f32 = 1.0;

/// A cell's icon in the baked art.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Icon {
    /// The glyph codes of its three states: open (not owned), owned, selected.
    pub codes: [u32; 3],
    /// Where the screen places it (640 × 480).
    pub at: [f32; 2],
    /// The placement's scale.
    pub scale: f32,
}

/// A cell as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Position on the 640 × 480 screen (the executable's).
    pub position: [f32; 2],
    pub owned: bool,
    /// May be put on a button.
    pub assignable: bool,
    /// The item's name.
    pub name: String,
    /// The slot it is on (0 L1, 1 L2, 2 R2).
    pub slot: Option<usize>,
    /// Its icon, when the screen's art is loaded.
    pub icon: Option<Icon>,
}

/// What the screen shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridView {
    /// The screen's title (string 3151, "Gadget Grid").
    pub title: String,
    /// Where the screen places the title, and its size (640 × 480).
    pub title_at: Option<([f32; 2], f32)>,
    pub cells: Vec<Cell>,
    pub selected: usize,
    /// The slots' item names.
    pub slots: [Option<String>; 3],
}

/// The screen: its phases and message.
#[derive(Clone, Debug, PartialEq)]
pub struct GridScreen {
    pub screen: Screen,
    pub message: Option<(String, f32)>,
}

impl GridScreen {
    /// Opens the screen of grid object `object`.
    pub fn open(object: i32) -> GridScreen {
        let mut screen = Screen::new(object);
        screen.show();
        GridScreen { screen, message: None }
    }

    /// Shows a message (not owned, not assignable).
    pub fn say(&mut self, text: String) {
        self.message = Some((text, MESSAGE_TIME));
    }

    /// Advances on the real clock.
    pub fn update(&mut self, dt: f32) {
        self.screen.update(dt);
        if let Some((_, t)) = self.message.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
    }

    /// Draws `view` over the canvas.
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, view: &GridView) {
        if !self.screen.visible() {
            return;
        }
        let a = self.screen.coverage();
        let (cw, ch) = (list.canvas.width, list.canvas.height);
        list.rect(Rect::new(0.0, 0.0, cw, ch), [0.0, 0.02, 0.08, 0.85 * a]);
        // The screen's 4:3 canvas, scaled to fit and centred.
        let s = (cw / 640.0).min(ch / 480.0);
        let (ox, oy) = ((cw - 640.0 * s) / 2.0, (ch - 480.0 * s) / 2.0);
        let to = |p: [f32; 2]| [ox + p[0] * s, oy + p[1] * s];
        let text = |s: &str| -> Vec<u8> { s.chars().map(|c| c as u32 as u8).collect() };
        let white = [[1.0, 1.0, 1.0, a], [1.0, 0.85, 0.3, a]];
        for (i, c) in view.cells.iter().enumerate() {
            let selected = i == view.selected;
            let drawn = match (c.icon, font) {
                (Some(icon), Some(f)) => {
                    let state = if selected { 2 } else { usize::from(c.owned) };
                    let k = icon.scale * s / ICON_SCALE;
                    let p = to(icon.at);
                    f.draw_glyph(list, ICON_FONT, icon.codes[state], p, k, [1.0, 1.0, 1.0, a])
                        .then(|| Rect::new(p[0] - 30.0 * k, p[1] - 30.0 * k, 60.0 * k, 60.0 * k))
                }
                _ => None,
            };
            let r = drawn.unwrap_or_else(|| {
                let p = to(c.position);
                let r = Rect::new(p[0] - 22.0 * s, p[1] - 22.0 * s, 44.0 * s, 44.0 * s);
                list.rect(r, if c.owned { [0.25, 0.45, 0.9, 0.9 * a] } else { [0.2, 0.2, 0.25, 0.6 * a] });
                if selected {
                    list.frame(r.inflate(3.0), 3.0, [1.0, 0.85, 0.2, a]);
                }
                r
            });
            if let (Some(k), Some(f)) = (c.slot, font) {
                let label: &[u8] = [b"L1", b"L2", b"R2"][k];
                f.draw(list, label, r.x, r.y, SIZE * 0.8, white, None);
            }
        }
        let Some(f) = font else { return };
        let (at, size) = view.title_at.map_or(([24.0, 20.0], SIZE + 6.0), |(p, size)| (to(p), size * s));
        f.draw(list, &text(&view.title), at[0], at[1], size, white, None);
        // Under the grid: the selected item's name and the message on the left, the three slots on the right.
        let left = to([60.0, 360.0]);
        if let Some(c) = view.cells.get(view.selected) {
            f.draw(list, &text(&c.name), left[0], left[1], SIZE + 2.0, white, Some(300.0 * s));
        }
        if let Some((m, _)) = &self.message {
            let y = left[1] + f.line_height(SIZE + 2.0) + 4.0;
            f.draw(list, &text(m), left[0], y, SIZE, [[1.0, 0.5, 0.4, a]; 2], Some(300.0 * s));
        }
        let right = to([400.0, 360.0]);
        for (k, slot) in view.slots.iter().enumerate() {
            let line = [[b"L1", b"L2", b"R2"][k] as &[u8], b": ", &text(slot.as_deref().unwrap_or("-"))].concat();
            let y = right[1] + f.line_height(SIZE) * k as f32;
            f.draw(list, &line, right[0], y, SIZE, white, Some(220.0 * s));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Texture;
    use alloc::vec;

    #[test]
    fn draws_cells_and_selection() {
        let mut g = GridScreen::open(885);
        for _ in 0..60 {
            g.update(1.0 / 60.0);
        }
        let cell = |x, owned| Cell {
            position: [x, 100.0],
            owned,
            assignable: true,
            name: "Smoke Bomb".into(),
            slot: None,
            icon: None,
        };
        let view = GridView {
            title: "Gadget Grid".into(),
            title_at: None,
            cells: vec![cell(145.0, true), cell(245.0, false)],
            selected: 0,
            slots: [None, None, None],
        };
        let mut list = DrawList::default();
        g.draw(&mut list, None, &view);
        // Background, two cells, the selection frame (4 edges).
        assert_eq!(list.quads.len(), 1 + 2 + 4);
        g.say("Not owned".into());
        for _ in 0..130 {
            g.update(1.0 / 60.0);
        }
        assert!(g.message.is_none());
    }

    #[test]
    fn icons_come_from_the_icon_font() {
        use super::super::font::GlyphShape;
        let mut g = GridScreen::open(885);
        for _ in 0..60 {
            g.update(1.0 / 60.0);
        }
        // A sheet with an empty text font, an empty icon font and three baked icons (codes 0, 1, 2).
        let square = |code| GlyphShape {
            code,
            advance: 0.0,
            triangles: vec![
                ([[-20.0, -20.0], [20.0, -20.0], [20.0, 20.0]], [1.0; 4]),
                ([[-20.0, -20.0], [20.0, 20.0], [-20.0, 20.0]], [1.0; 4]),
            ],
        };
        let sheet = FontSheet::build(&[Vec::new(), Vec::new(), vec![square(0), square(1), square(2)]]);
        let icon = Icon { codes: [0, 1, 2], at: [183.0, 141.0], scale: 0.8 };
        let cell = Cell {
            position: [145.0, 155.0],
            owned: true,
            assignable: true,
            name: String::new(),
            slot: None,
            icon: Some(icon),
        };
        let view = GridView {
            title: String::new(),
            title_at: None,
            cells: vec![cell],
            selected: 0,
            slots: [None, None, None],
        };
        let mut list = DrawList::default();
        g.draw(&mut list, Some(&sheet), &view);
        // The background and the icon: no fallback square or frame.
        let icons: Vec<_> = list.quads.iter().filter(|q| q.texture == Texture::Glyphs).collect();
        assert_eq!(icons.len(), 1);
        assert_eq!(list.quads.len(), 2);
        // Centred on the placement (canvas 640 × 480: scale 1).
        let r = icons[0].rect;
        assert!((r.x + r.w / 2.0 - 183.0).abs() < 2.0 && (r.y + r.h / 2.0 - 141.0).abs() < 2.0, "{r:?}");
    }
}
