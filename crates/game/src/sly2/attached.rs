//! Sly 2: the objects attached under the characters in the object tree (gadgets, their blast volumes, switch rings
//! and bursts, held items; characters.md 9.3, gadgets.md 6), drawn every frame at their world transform as the
//! engine places them.
//!
//! Their scene draws stay instances (`stage::prep::SceneInput::dynamic`); each keeps its placement relative to its
//! object at load, and follows the object (and so the character or bone it hangs from) from then on. A part shows
//! while its object is in the world, active and visible, and:
//!
//! - a gadget object (classes 100 to 102) only while a copy of it is out of its pool (`World::shared.gadget_copies`,
//!   `engine::gang::pool`); the templates left in the pools' groups are not drawn;
//! - a bomb's mesh groups (gadgets.md 7.1.1: a sphere and four light cards per bomb kind, all hidden at load) only
//!   for the copy's own kind: shown while it is landed (armed, state 2), blinking over [`BLINK_TIME`] once
//!   triggered (state 3), hidden otherwise. The other kinds' groups never show.

use alloc::vec::Vec;

use engine::gang::pool::{BOMB_CLASS, GADGET_CLASSES, GadgetCopy};
use engine::object::Handle;
use engine::world::World;
use render::math::Mat4;
use render::{Backend, InstanceId, RenderError, Renderer};

use super::skin::CHARACTERS;
use crate::level::mat4;

/// The countdown blink of a triggered bomb (gadgets.md 7.1.1: 112 frames at 60 per second).
pub const BLINK_TIME: f32 = 112.0 / 60.0;
/// The blink's toggles (gadgets.md 7.1.1, frames at 60 per second): hidden from each even entry to the next.
const BLINK: [u16; 12] = [20, 30, 50, 59, 68, 75, 84, 90, 98, 103, 108, 112];

/// One drawn part of an attached object.
#[derive(Clone, Copy, Debug)]
struct Part {
    id: InstanceId,
    object: Handle,
    /// The draw's transform relative to its object's world transform at load.
    local: Mat4,
    /// For a bomb's mesh group: the bomb kind (item) it belongs to (gadgets.md 7.1.1).
    group: Option<u8>,
    shown: bool,
}

/// The attached objects' parts.
#[derive(Clone, Debug, Default)]
pub struct AttachedParts {
    parts: Vec<Part>,
    /// Triggered bombs and the time since (their blink).
    triggered: Vec<(Handle, f32)>,
}

/// Whether `h` has a character (class 53 to 55) among its ancestors.
fn under_character(world: &World, h: Handle) -> bool {
    let mut up = world.parent(h);
    while let Some(p) = up {
        if world.class(p).is_some_and(|c| CHARACTERS.contains(&c)) {
            return true;
        }
        up = world.parent(p);
    }
    false
}

/// The objects attached under a character, other than the characters, with their records' byte ranges.
pub fn attached_objects(world: &World) -> Vec<(Handle, (u32, u32))> {
    world
        .handles()
        .filter(|&h| !world.class(h).is_some_and(|c| CHARACTERS.contains(&c)) && under_character(world, h))
        .filter_map(|h| world.sly2_record(h).map(|r| (h, (r.header_offset, r.body_offset + r.body_size))))
        .collect()
}

/// The gadget object `h` belongs to (itself or its nearest ancestor of classes 100 to 102).
fn gadget_of(world: &World, h: Handle) -> Option<Handle> {
    let mut at = Some(h);
    while let Some(x) = at {
        if world.class(x).is_some_and(|c| GADGET_CLASSES.contains(&c)) {
            return Some(x);
        }
        at = world.parent(x);
    }
    None
}

/// The copy `gadget` is out of its pool as, if it is.
fn copy_of(world: &World, gadget: Handle) -> Option<GadgetCopy> {
    world.shared.gadget_copies.iter().find(|g| g.object == gadget).copied()
}

/// The bomb kind a mesh group of `bentley_mine` belongs to (gadgets.md 7.1.1), from the mesh's transform (the
/// light cards: 15–18 Trigger, 11–14 Snooze, 7–10 Reduction, 3–6 Rage) or, for a sphere (transform 2, a kind-3
/// volume), its radius and colour (325 red Trigger, 325 green Snooze, 620 green Reduction, 620 purple Rage).
pub fn bomb_group(transform: i16, volume: bool, radius: f32, tint: [u8; 4]) -> Option<u8> {
    match transform {
        15..=18 => Some(7),
        11..=14 => Some(9),
        7..=10 => Some(13),
        3..=6 => Some(37),
        2 if volume && radius > 300.0 => {
            let green = tint[1] > tint[0] && tint[1] > tint[2];
            Some(match (radius < 470.0, green) {
                (true, false) => 7,
                (true, true) => 9,
                (false, true) => 13,
                (false, false) => 37,
            })
        }
        _ => None,
    }
}

/// Whether a triggered bomb's groups show `t` seconds into the blink.
pub fn blink_shown(t: f32) -> bool {
    let frame = t * 60.0;
    let toggles = BLINK.iter().take_while(|&&f| f32::from(f) <= frame).count();
    toggles % 2 == 0 && t < BLINK_TIME
}

/// The kind group an item's copy shows (the Trigger Bomb is item 7, its kind 5 or 7).
fn group_of_item(item: u8) -> u8 {
    if item == 5 { 7 } else { item }
}

/// Whether a part shows now: its object in the world, active and visible; a gadget's only while its copy is out;
/// a bomb group's only for the copy's kind, while armed, or blinking `blink` seconds into its countdown.
pub fn part_shown(world: &World, object: Handle, group: Option<u8>, blink: Option<f32>) -> bool {
    if !(world.in_world(object) && world.is_active(object) && world.is_visible(object)) {
        return false;
    }
    let Some(gadget) = gadget_of(world, object) else { return true };
    let Some(copy) = copy_of(world, gadget) else { return false };
    let Some(group) = group else { return true };
    if world.class(gadget) != Some(BOMB_CLASS) || group != group_of_item(copy.item) {
        return false;
    }
    match copy.state {
        2 => true,
        3 => blink.is_some_and(blink_shown),
        _ => false,
    }
}

/// A dynamic draw as placed on the renderer.
#[derive(Clone, Copy, Debug)]
pub struct PlacedDraw {
    /// Its instance.
    pub id: InstanceId,
    pub mesh: render::MeshId,
    pub material: render::MaterialId,
    /// Its weights' base, when it has two colours per vertex.
    pub weights: Option<u32>,
    /// Its transform at load.
    pub transform: Mat4,
    /// From [`stage::prep::DynamicDraw`].
    pub record: u32,
    pub volume: bool,
    pub radius: f32,
    /// The scene mesh's index in its set.
    pub mesh_index: u32,
    /// Its material's constant colour (volumes).
    pub tint: [u8; 4],
}

impl AttachedParts {
    /// The parts of the dynamic draws, matched to their objects by record: the innermost objects whose record holds
    /// the draw's (records nest). Several objects can share one record (a pool's copies are proxies of it), and the
    /// scene places the record's meshes once: the draw goes to the object it was placed nearest to, and each other
    /// object sharing the record gets an instance of its own.
    pub fn new<B: Backend>(
        r: &mut Renderer<B>,
        world: &World,
        draws: &[PlacedDraw],
    ) -> Result<AttachedParts, RenderError> {
        let objects = attached_objects(world);
        let mut parts = Vec::with_capacity(draws.len());
        for d in draws {
            let holding = || objects.iter().filter(|(_, (a, b))| d.record >= *a && d.record < *b);
            let Some(inner) = holding().map(|(_, (a, b))| b - a).min() else { continue };
            let mut sharing: Vec<(Handle, Mat4, f32)> = holding()
                .filter(|(_, (a, b))| b - a == inner)
                .filter_map(|&(h, _)| {
                    let inv = world.world_transform(h).and_then(|w| engine::math::affine_inverse(&w))?;
                    let local = mat4(&inv) * d.transform;
                    let t = [local.m[12], local.m[13], local.m[14]];
                    Some((h, local, t[0] * t[0] + t[1] * t[1] + t[2] * t[2]))
                })
                .collect();
            sharing.sort_by(|a, b| a.2.total_cmp(&b.2));
            let Some(&(_, local, _)) = sharing.first() else { continue };
            for (k, &(object, _, _)) in sharing.iter().enumerate() {
                let id = if k == 0 {
                    d.id
                } else {
                    let id = r.add_instance(d.mesh, d.material, d.transform)?;
                    if d.weights.is_some() && r.material(d.material).is_some_and(|m| m.palettes.is_some()) {
                        r.set_weights(id, d.weights)?;
                    }
                    id
                };
                // A bomb's mesh groups, by the mesh's transform in its set.
                let group = gadget_of(world, object)
                    .filter(|&g| world.class(g) == Some(BOMB_CLASS))
                    .and_then(|_| world.sly2_record(object))
                    .and_then(|r| match &r.body {
                        formats_sly2::stream::Body::Solid(s) => s.meshes.meshes.get(d.mesh_index as usize),
                        _ => None,
                    })
                    .and_then(|m| bomb_group(m.transform, d.volume, d.radius, d.tint));
                parts.push(Part { id, object, local, group, shown: true });
            }
        }
        Ok(AttachedParts { parts, triggered: Vec::new() })
    }

    /// Parts.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Parts shown after the last update.
    pub fn shown(&self) -> usize {
        self.parts.iter().filter(|p| p.shown).count()
    }

    /// Moves the shown parts to their objects and shows or hides each (call once per rendered frame, `dt` the time
    /// played since the last).
    pub fn update<B: Backend>(&mut self, r: &mut Renderer<B>, world: &World, dt: f32) -> Result<(), RenderError> {
        // The triggered bombs' blink clocks: started on state 3, dropped once the copy leaves it.
        let copies = &world.shared.gadget_copies;
        self.triggered.retain(|(h, _)| copies.iter().any(|g| g.object == *h && g.state == 3));
        for (_, t) in &mut self.triggered {
            *t += dt;
        }
        for g in copies.iter().filter(|g| g.state == 3) {
            if !self.triggered.iter().any(|(h, _)| *h == g.object) {
                self.triggered.push((g.object, 0.0));
            }
        }
        for p in &mut self.parts {
            let blink =
                gadget_of(world, p.object).and_then(|g| self.triggered.iter().find(|(h, _)| *h == g)).map(|(_, t)| *t);
            let shown = part_shown(world, p.object, p.group, blink);
            if shown && let Some(w) = world.world_transform(p.object) {
                r.set_transform(p.id, mat4(&w) * p.local)?;
            }
            if shown != p.shown {
                r.set_visible(p.id, shown)?;
                p.shown = shown;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bomb_groups_by_transform_and_sphere() {
        assert_eq!(bomb_group(16, false, 14.0, [255; 4]), Some(7));
        assert_eq!(bomb_group(12, false, 14.0, [255; 4]), Some(9));
        assert_eq!(bomb_group(8, false, 14.0, [255; 4]), Some(13));
        assert_eq!(bomb_group(4, false, 14.0, [255; 4]), Some(37));
        assert_eq!(bomb_group(2, true, 325.0, [255, 0, 2, 76]), Some(7));
        assert_eq!(bomb_group(2, true, 325.0, [4, 220, 1, 76]), Some(9));
        assert_eq!(bomb_group(2, true, 620.0, [4, 220, 1, 76]), Some(13));
        assert_eq!(bomb_group(2, true, 620.0, [179, 0, 220, 76]), Some(37));
        // The body and its shadow belong to no group.
        assert_eq!(bomb_group(2, false, 15.0, [255; 4]), None);
        assert_eq!(bomb_group(1, true, 201.0, [0, 0, 0, 52]), None);
    }

    #[test]
    fn the_blink_quickens_and_ends() {
        // Shown until frame 20, hidden 20..30, shown 30..50, ... hidden from 108 on; over after 112 frames.
        let at = |f: f32| blink_shown(f / 60.0);
        assert!(at(0.0) && at(19.0) && !at(20.0) && !at(29.0) && at(30.0) && at(49.0) && !at(50.0));
        assert!(at(103.0) && at(107.0) && !at(108.0) && !at(111.0) && !at(112.0) && !at(200.0));
    }
}
