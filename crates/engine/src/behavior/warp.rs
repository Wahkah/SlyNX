//! Warp points (class 116, level-flow.md 3; classes/markers-and-volumes.md 5) and arriving in a world.
//!
//! Choosing the warp on arrival: the warp with the requested OID; else a warp whose alias (option 159) matches;
//! else, during a world-map wipe, one with option 158; else the default warp (option 157). Arriving places the player
//! at the warp, plays the warp's animations, hides the objects it lists, raises event 2 on it, and blocks every
//! enabled exit until the player has left its volume. The camera turns by option 160.

use alloc::vec::Vec;

use formats::anim::Segment;
use formats::geometry::Affine;
use formats::reader::Reader;

use super::ClassBehavior;
use crate::object::Handle;
use crate::object::class::id;
use crate::world::{AnimCommand, World};

/// Warp options (level-flow.md 3).
pub mod opt {
    /// Default warp of the world.
    pub const DEFAULT: u16 = 157;
    /// Default warp after a world-map wipe.
    pub const DEFAULT_AFTER_MAP: u16 = 158;
    /// Alias (an exit's option 163 names it).
    pub const ALIAS: u16 = 159;
    /// Initial camera angle, radians.
    pub const CAMERA_ANGLE: u16 = 160;
}

/// A warp's data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Warp {
    /// Name (a game string, for example `check_point1`).
    pub name: Vec<u8>,
    /// Objects hidden while the warp plays (OIDs).
    pub hidden: Vec<i16>,
    /// Owned animations.
    pub animations: u32,
}

impl Warp {
    /// Reads the warp's name, hidden objects and animation count from its record.
    pub fn new(world: &World, h: Handle) -> Warp {
        let Some(rec) = world.record(h) else { return Warp::default() };
        let r = world.level.record(rec);
        let Some(options) = r.options else { return Warp::default() };
        let data = &world.level.bytes;
        let parsed = (|| -> formats::Result<Warp> {
            let mut rd = Reader::at(data, options.end);
            let a = rd.i16()?.max(0) as u32;
            for _ in 0..a {
                rd.skip(6)?;
                let seg = Segment::parse(data, rd.pos(), &world.level.table)?;
                rd.set_pos(seg.end);
            }
            let name = rd.string()?.to_vec();
            let n = rd.i16()?.max(0);
            let mut hidden = Vec::with_capacity(n as usize);
            for _ in 0..n {
                hidden.push(rd.i16()?);
            }
            Ok(Warp { name, hidden, animations: a })
        })();
        parsed.unwrap_or_default()
    }

    /// The name as text (bytes outside ASCII are replaced).
    pub fn name_str(&self) -> alloc::string::String {
        self.name.iter().map(|&b| if b.is_ascii() { b as char } else { '?' }).collect()
    }
}

impl ClassBehavior for Warp {}

/// The result of arriving at a warp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    /// The warp.
    pub warp: Handle,
    /// Its world transform.
    pub transform: Affine,
    /// Camera angle offset (option 160), radians.
    pub camera_angle: f32,
}

impl World {
    /// The warp an arrival uses (see the module documentation).
    pub fn choose_warp(&self, requested: Option<i16>, map_wipe: bool) -> Option<Handle> {
        let warps: Vec<Handle> = self.slots_of_class(id::WARP).iter().filter_map(|&s| self.handle(s)).collect();
        let opt_set = |h: Handle, o: u16| self.options(h).is_some_and(|x| x.bool_or(o, false));
        if let Some(r) = requested {
            if let Some(&w) = warps.iter().find(|&&w| self.oid(w) == Some(r)) {
                return Some(w);
            }
            if let Some(&w) = warps.iter().find(|&&w| self.options(w).and_then(|o| o.i16(opt::ALIAS)) == Some(r)) {
                return Some(w);
            }
        }
        if map_wipe && let Some(&w) = warps.iter().find(|&&w| opt_set(w, opt::DEFAULT_AFTER_MAP)) {
            return Some(w);
        }
        warps.iter().copied().find(|&w| opt_set(w, opt::DEFAULT))
    }

    /// Arrives at a warp: places the player there, plays its animations, hides its objects, raises event 2 on it,
    /// and blocks the exits. Returns `None` when no warp fits.
    pub fn arrive(&mut self, requested: Option<i16>, map_wipe: bool) -> Option<Arrival> {
        self.update_transforms();
        let warp = self.choose_warp(requested, map_wipe)?;
        let transform = self.world_transform(warp)?;
        if let Some(p) = self.player {
            self.set_world_transform(p, transform);
        }
        let (hidden, animations) = match self.behavior(warp) {
            Some(super::Behavior::Warp(w)) => (w.hidden.clone(), w.animations),
            _ => (Vec::new(), 0),
        };
        for oid in hidden {
            let mut v = Vec::new();
            self.find_all_oid(oid, None, &mut v);
            for h in v {
                self.set_hidden(h, true);
            }
        }
        for i in 0..animations {
            self.anim.push(AnimCommand::PlayOwned { owner: warp, index: i, reverse: false, speed: 1.0, tag: i });
        }
        self.raise(warp, script::event::TRIGGERED, Vec::new());
        let exits: Vec<u32> = self.slots_of_class(id::EXIT).to_vec();
        for s in exits {
            if let Some(h) = self.handle(s)
                && let Some(e) = self.behavior_mut(h).and_then(super::Behavior::as_exit_mut)
            {
                e.blocked = true;
            }
        }
        let camera_angle = self.options(warp).map_or(0.0, |o| o.f32_or(opt::CAMERA_ANGLE, 0.0));
        Some(Arrival { warp, transform, camera_angle })
    }
}
