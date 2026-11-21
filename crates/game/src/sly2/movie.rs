//! A still of a screen movie's sprite (classes/effects-logic-ui.md 4, user-interface.md 2 and 3): the shapes a
//! sprite label shows once all its keys have played, flattened into triangles on the 640 × 480 canvas, with the
//! sprites it places and its texts. The menus use it for the art the game draws itself (the gadget grids' icons and
//! the placement of their texts).
//!
//! How a label's keys are read (from the disc's grid screens, [I] where the spec leaves the fields open):
//!
//! - a placement's first word is the dictionary item it shows (0: keep the one already in that slot), its second
//!   word the slot, its matrix the slot's transform (row vectors: x' = a x + c y + tx, y' = b x + d y + ty);
//! - its signed 16-bit field, when not −1, is the index of the key's colour pair it takes;
//! - a key's 16-bit values are slots it empties;
//! - a dictionary value item (kind 4) shows the sprite of that number, playing its first label;
//! - a colour pair is an offset word (per byte R, G, B, A: byte − 127) and a scale word ((byte − 127) / 127,
//!   clamped to 0..1), as the spec writer confirmed.

use alloc::vec::Vec;

use formats_sly2::screen::{ItemBody, Label, Screen};

/// The marker labels before a sprite's idle label (seen on the menus' sprites: show, marker, idle, ..., hide, ...):
/// Sly 2's 5572 and Sly 3's 4869.
pub const BEFORE_IDLE: [i32; 2] = [5572, 4869];

/// A 2-D affine transform: `[a, b, c, d, tx, ty]`.
pub type Affine = [f32; 6];

/// The identity.
pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `outer` after `inner`.
pub fn compose(outer: &Affine, inner: &Affine) -> Affine {
    let [a, b, c, d, x, y] = *outer;
    let [e, f, g, h, u, v] = *inner;
    [a * e + c * f, b * e + d * f, a * g + c * h, b * g + d * h, a * u + c * v + x, b * u + d * v + y]
}

/// Applies `m` to a point.
pub fn apply(m: &Affine, p: [f32; 2]) -> [f32; 2] {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

/// The transform's scale (square root of the determinant's size).
pub fn scale_of(m: &Affine) -> f32 {
    let det = (m[0] * m[3] - m[1] * m[2]).abs();
    // Newton's square root: no `std` on the Switch.
    let mut s = if det > 1.0 { det } else { 1.0 };
    for _ in 0..20 {
        s = 0.5 * (s + det / s);
    }
    s
}

/// A colour transform: c × mul + add (0..1 per channel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub mul: [f32; 4],
    pub add: [f32; 4],
}

impl Tint {
    pub const NONE: Tint = Tint { mul: [1.0; 4], add: [0.0; 4] };

    /// From a key's colour pair (offset word, scale word).
    pub fn from_pair(pair: [u32; 2]) -> Tint {
        let o = pair[0].to_le_bytes();
        let s = pair[1].to_le_bytes();
        Tint {
            mul: s.map(|b| ((b as f32 - 127.0) / 127.0).clamp(0.0, 1.0)),
            add: o.map(|b| (b as f32 - 127.0) / 255.0),
        }
    }

    /// `self` applied after `inner`.
    pub fn then(&self, inner: &Tint) -> Tint {
        let mut t = Tint::NONE;
        for k in 0..4 {
            t.mul[k] = inner.mul[k] * self.mul[k];
            t.add[k] = inner.add[k] * self.mul[k] + self.add[k];
        }
        t
    }

    /// A colour through the tint, clamped.
    pub fn colour(&self, c: [f32; 4]) -> [f32; 4] {
        let mut out = [0.0; 4];
        for k in 0..4 {
            out[k] = (c[k] * self.mul[k] + self.add[k]).clamp(0.0, 1.0);
        }
        out
    }
}

/// A text item as placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedText {
    pub string: Option<i32>,
    /// Its origin on the canvas.
    pub at: [f32; 2],
    /// The placement's scale.
    pub scale: f32,
    /// The item's three floats (the first is its size [I]).
    pub floats: [f32; 3],
    /// The item's colour word (user-interface.md 3).
    pub colour: u32,
}

/// What a still shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Still {
    /// Triangles in drawing order (later ones on top), with their colours (0..1).
    pub triangles: Vec<([[f32; 2]; 3], [f32; 4])>,
    pub texts: Vec<PlacedText>,
    /// Each sprite placed (sprite number, its transform).
    pub sprites: Vec<(usize, Affine)>,
}

/// Deepest sprite nesting followed.
pub const MAX_DEPTH: u32 = 8;
/// Most slots drawn for one still, across all nesting: bounds the work a screen whose sprites place each other many
/// times over can cause (the depth cap alone allows slots^8).
pub const MAX_SLOTS: u32 = 4096;

/// A sprite's label by ID.
pub fn label(screen: &Screen, sprite: usize, id: i32) -> Option<&Label> {
    screen.sprites.get(sprite)?.labels.iter().find(|l| l.id == id)
}

/// A sprite's idle label: the one after a [`BEFORE_IDLE`] marker, else its first label with keys.
pub fn idle_label(screen: &Screen, sprite: usize) -> Option<&Label> {
    let labels = &screen.sprites.get(sprite)?.labels;
    let after = labels.windows(2).find(|w| BEFORE_IDLE.contains(&w[0].id)).map(|w| &w[1]);
    after.or_else(|| labels.iter().find(|l| !l.keys.is_empty()))
}

struct Slot {
    item: u16,
    m: Affine,
    tint: Tint,
}

/// The slots a label shows once its keys up to `time` have played, in slot order.
fn slots(l: &Label, time: f32) -> Vec<(u16, Slot)> {
    let mut out: Vec<(u16, Slot)> = Vec::new();
    for k in l.keys.iter().filter(|k| k.time <= time) {
        for p in &k.placements {
            let slot = p.words[1];
            let m = p.matrix;
            let m = [m[0], m[1], m[3], m[4], m[6], m[7]];
            let tint = usize::try_from(p.depth).ok().and_then(|i| k.colours.get(i)).map(|c| Tint::from_pair(*c));
            let at = out.iter().position(|s| s.0 == slot);
            match (p.words[0], at) {
                (0, Some(i)) => {
                    out[i].1.m = m;
                    if let Some(t) = tint {
                        out[i].1.tint = t;
                    }
                }
                (0, None) => {}
                (item, Some(i)) => out[i].1 = Slot { item, m, tint: tint.unwrap_or(Tint::NONE) },
                (item, None) => out.push((slot, Slot { item, m, tint: tint.unwrap_or(Tint::NONE) })),
            }
        }
        out.retain(|s| !k.values.contains(&s.0));
    }
    out.sort_by_key(|s| s.0);
    out
}

/// The still of `label` (after `time` seconds; `f32::MAX` for its end) placed by `m` and `tint`. Sprites placed by
/// it play their first label to its end.
pub fn still(screen: &Screen, l: &Label, time: f32, m: &Affine, tint: &Tint) -> Still {
    let mut out = Still::default();
    let mut budget = MAX_SLOTS;
    draw_label(screen, l, time, m, tint, 0, &mut budget, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn draw_label(
    screen: &Screen,
    l: &Label,
    time: f32,
    m: &Affine,
    tint: &Tint,
    depth: u32,
    budget: &mut u32,
    out: &mut Still,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for (_, s) in slots(l, time) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let Some(item) = screen.items.iter().find(|i| i.number == s.item) else { continue };
        let m = compose(m, &s.m);
        let tint = tint.then(&s.tint);
        match &item.body {
            ItemBody::Shape(prims) => {
                // Lower draw layers first (user-interface.md 3).
                let mut order: Vec<usize> = (0..prims.len()).collect();
                order.sort_by_key(|&i| prims[i].settings[0]);
                for i in order {
                    let p = &prims[i];
                    let c = p.colour();
                    let col = tint.colour([
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        (c[3] as f32 / 128.0).min(1.0),
                    ]);
                    for t in p.triangles() {
                        let v = t.map(|k| apply(&m, [p.vertices[k].x as f32 / 16.0, p.vertices[k].y as f32 / 16.0]));
                        out.triangles.push((v, col));
                    }
                }
            }
            ItemBody::Value(n) => {
                let n = *n as usize;
                out.sprites.push((n, m));
                if let Some(first) = screen.sprites.get(n).and_then(|s| s.labels.first()) {
                    draw_label(screen, first, f32::MAX, &m, &tint, depth + 1, budget, out);
                }
            }
            ItemBody::Text(t) => out.texts.push(PlacedText {
                string: t.string,
                at: apply(&m, [0.0, 0.0]),
                scale: scale_of(&m),
                floats: t.floats,
                colour: t.words[1],
            }),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats_sly2::screen::{Item, Key, Placement, Primitive, ShapeVertex, Sprite};

    fn place(item: u16, slot: u16, depth: i16, x: f32) -> Placement {
        Placement {
            words: [item, slot, 0],
            depth,
            bytes: [0, 0],
            value: -1,
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0, 1.0],
        }
    }

    fn key(time: f32, placements: Vec<Placement>, colours: Vec<[u32; 2]>, values: Vec<u16>) -> Key {
        Key { time, placements, colours, values, bytes: Vec::new(), actions: Vec::new() }
    }

    #[test]
    fn keys_place_move_tint_and_empty_slots() {
        let v = |x: i16, y: i16| ShapeVertex { rgba: [255, 255, 255, 0x80], extra: 0, x: x * 16, y: y * 16 };
        let tri = Primitive { settings: [0, 1, 1], flag: 0, vertices: alloc::vec![v(0, 0), v(1, 0), v(0, 1)] };
        let screen = Screen {
            items: alloc::vec![
                Item { number: 1, body: ItemBody::Shape(alloc::vec![tri]) },
                Item { number: 2, body: ItemBody::Value(1) },
            ],
            sprites: alloc::vec![
                Sprite {
                    instances: 1,
                    slots: 2,
                    labels: alloc::vec![Label {
                        duration: 1.0,
                        id: 7,
                        keys: alloc::vec![
                            key(
                                0.0,
                                alloc::vec![place(1, 0, -1, 10.0), place(2, 1, 0, 20.0)],
                                alloc::vec![[0x7F7F_7F7F, 0x80FF_FFFF]],
                                alloc::vec![]
                            ),
                            key(0.5, alloc::vec![place(0, 0, -1, 30.0)], alloc::vec![], alloc::vec![]),
                            key(0.9, alloc::vec![], alloc::vec![], alloc::vec![1]),
                        ],
                    }],
                },
                Sprite {
                    instances: 1,
                    slots: 1,
                    labels: alloc::vec![Label {
                        duration: 0.0,
                        id: -1,
                        keys: alloc::vec![key(0.0, alloc::vec![place(1, 0, -1, 1.0)], alloc::vec![], alloc::vec![])]
                    }],
                },
            ],
            ..Screen::default()
        };
        let l = label(&screen, 0, 7).unwrap();
        // At 0.6: the shape moved to 30, the sprite at 20 (its shape at 21) with its alpha halved.
        let s = still(&screen, l, 0.6, &IDENTITY, &Tint::NONE);
        assert_eq!(s.triangles.len(), 2);
        assert_eq!(s.triangles[0].0[0], [30.0, 0.0]);
        assert_eq!(s.triangles[1].0[0], [21.0, 0.0]);
        assert!((s.triangles[1].1[3] - 1.0 / 127.0).abs() < 1e-4);
        assert_eq!(s.sprites, alloc::vec![(1, [1.0, 0.0, 0.0, 1.0, 20.0, 0.0])]);
        // At the end slot 1 is emptied.
        let s = still(&screen, l, f32::MAX, &IDENTITY, &Tint::NONE);
        assert_eq!(s.triangles.len(), 1);
    }

    #[test]
    fn nested_sprites_stop_at_the_work_budget() {
        // A sprite whose label places itself in 16 slots: 16^8 slots without the budget.
        let screen = Screen {
            items: alloc::vec![Item { number: 1, body: ItemBody::Value(0) }],
            sprites: alloc::vec![Sprite {
                instances: 1,
                slots: 16,
                labels: alloc::vec![Label {
                    duration: 0.0,
                    id: 1,
                    keys: alloc::vec![key(
                        0.0,
                        (0..16).map(|i| place(1, i, -1, 0.0)).collect(),
                        alloc::vec![],
                        alloc::vec![]
                    )],
                }],
            }],
            ..Screen::default()
        };
        let s = still(&screen, label(&screen, 0, 1).unwrap(), f32::MAX, &IDENTITY, &Tint::NONE);
        assert_eq!(s.sprites.len(), MAX_SLOTS as usize);
    }

    #[test]
    fn transforms_compose() {
        let a = [2.0, 0.0, 0.0, 2.0, 5.0, 0.0];
        let b = [1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        assert_eq!(apply(&compose(&a, &b), [0.0, 0.0]), [7.0, 2.0]);
        assert!((scale_of(&a) - 2.0).abs() < 1e-4);
    }
}
