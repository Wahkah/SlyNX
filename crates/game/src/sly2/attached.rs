//! Sly 2: the objects attached under the characters in the object tree (gadgets, their blast volumes, switch rings
//! and bursts, held items; characters.md 9.3, gadgets.md 6), drawn every frame at their world transform as the
//! engine places them.
//!
//! Their scene draws stay instances (`stage::prep::SceneInput::dynamic`); each keeps its placement relative to its
//! object at load, and follows the object (and so the character or bone it hangs from) from then on. A part shows
//! while its object is in the world, active and visible, with two exceptions the engine does not signal yet:
//!
//! - a gadget object (classes 100 to 102) in a group directly under a character is its pool's template
//!   (gadgets.md 6: copies are taken from a pool): not drawn;
//! - a bomb's blast volumes (kinds 2 and 3 under a class-101 object) show only while the bomb is out of the
//!   characters (placed), and only the volume whose size is the bomb's blast radius (option 1104, gadgets.md 7.1).

use alloc::vec::Vec;

use engine::object::Handle;
use engine::world::World;
use render::math::Mat4;
use render::{Backend, InstanceId, RenderError, Renderer};

use super::skin::CHARACTERS;
use crate::level::mat4;

/// The gadget objects' classes (gadgets.md 6): carried objects, Bentley's bombs, the trap.
pub const GADGET_CLASSES: core::ops::RangeInclusive<i16> = 100..=102;
/// The bomb class (`bentley_mine`).
pub const BOMB_CLASS: i16 = 101;
/// The grouping class the pools' templates hang in.
const GROUP_CLASS: i16 = 6;
/// The bomb's kind option (stream 1104): the item it was used as.
const BOMB_KIND: u16 = 1104;

/// One drawn part of an attached object.
#[derive(Clone, Copy, Debug)]
struct Part {
    id: InstanceId,
    object: Handle,
    /// The draw's transform relative to its object's world transform at load.
    local: Mat4,
    /// A stencil volume (kind 2 or 3).
    volume: bool,
    /// The scene mesh's bounding radius.
    radius: f32,
    shown: bool,
}

/// The attached objects' parts.
#[derive(Clone, Debug, Default)]
pub struct AttachedParts {
    parts: Vec<Part>,
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

/// A pool's template: a gadget object in a group directly under a character.
pub fn pool_template(world: &World, h: Handle) -> bool {
    if !world.class(h).is_some_and(|c| GADGET_CLASSES.contains(&c)) {
        return false;
    }
    let Some(group) = world.parent(h).filter(|&p| world.class(p) == Some(GROUP_CLASS)) else { return false };
    world.parent(group).and_then(|c| world.class(c)).is_some_and(|c| CHARACTERS.contains(&c))
}

/// The bomb `h` belongs to (itself or an ancestor of class 101).
fn bomb_of(world: &World, h: Handle) -> Option<Handle> {
    let mut at = Some(h);
    while let Some(x) = at {
        if world.class(x) == Some(BOMB_CLASS) {
            return Some(x);
        }
        at = world.parent(x);
    }
    None
}

/// Whether a blast volume of mesh radius `radius` is the one of `bomb`'s blast: the bomb's radius (300 or 600 by
/// its kind option; the Trigger Bomb's without one), the volume a little larger (325 and 620 on the disc).
pub fn blast_volume_matches(world: &World, bomb: Handle, radius: f32) -> bool {
    let kind = world.options(bomb).and_then(|o| o.i32(BOMB_KIND)).unwrap_or(7);
    let blast = engine::gang::gadgets::blast_of(kind.clamp(0, 255) as u8).map_or(300.0, |b| b.radius as f32);
    radius >= blast && radius <= blast * 1.15
}

/// Whether a part of `object` shows now.
pub fn part_shown(world: &World, object: Handle, volume: bool, radius: f32) -> bool {
    if !(world.in_world(object) && world.is_active(object) && world.is_visible(object)) {
        return false;
    }
    let Some(bomb) = bomb_of(world, object) else { return !pool_template(world, object) };
    if pool_template(world, bomb) {
        return false;
    }
    // A bomb's volumes: only while it is placed (out of the characters), and only its blast's.
    !volume || (!under_character(world, bomb) && blast_volume_matches(world, bomb, radius))
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
                parts.push(Part { id, object, local, volume: d.volume, radius: d.radius, shown: true });
            }
        }
        Ok(AttachedParts { parts })
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

    /// Moves the shown parts to their objects and shows or hides each (call once per rendered frame).
    pub fn update<B: Backend>(&mut self, r: &mut Renderer<B>, world: &World) -> Result<(), RenderError> {
        for p in &mut self.parts {
            let shown = part_shown(world, p.object, p.volume, p.radius);
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
