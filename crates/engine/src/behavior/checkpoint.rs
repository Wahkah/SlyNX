//! Checkpoints (class 56, level-flow.md 4).
//!
//! A checkpoint triggers when the player enters one of its volumes (its class-123 children) or hits it. Triggering
//! records its bit and the respawn warp (its child warp) in the progress, sends its state machine to state 1352 (or
//! 1353 when it was reached before) and rumbles. It re-arms once the player is more than 1000 units from the volume
//! that triggered it. Checkpoints are hidden during a sprint; an already-reached checkpoint starts in state 1351.

use alloc::vec::Vec;

use formats::collision::ConvexVolume;

use super::{ClassBehavior, Cx, dist_sq};
use crate::object::class::id;
use crate::object::{Handle, OptionValue};
use crate::world::volume::PlacedVolume;
use crate::world::{Notice, Request, World};

/// State machine state OIDs.
pub mod state {
    /// Already triggered when the world loaded.
    pub const RESTORED: i16 = 1351;
    /// Just triggered.
    pub const TRIGGERED: i16 = 1352;
    /// Triggered again.
    pub const RETRIGGERED: i16 = 1353;
}

/// Distance from the triggering volume that re-arms a checkpoint.
pub const REARM_DISTANCE: f32 = 1000.0;
/// Rumble when hit: strength and duration in seconds ("medium, 0.2 s").
pub const RUMBLE: (f32, f32) = (0.5, 0.2);
/// The state machine's goal option.
const OPT_GOAL: u16 = script::machine::OPTION_GOAL as u16;

/// A checkpoint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checkpoint {
    /// Its number among the world's checkpoints (load order).
    pub index: u32,
    /// Its volumes, with the volume objects.
    pub volumes: Vec<(Handle, PlacedVolume)>,
    /// The volume that triggered it while it is not re-armed.
    pub armed_by: Option<usize>,
}

impl Checkpoint {
    /// A checkpoint (volumes are found after load).
    pub fn new() -> Checkpoint {
        Checkpoint::default()
    }

    fn machine(world: &World, me: Handle) -> Option<Handle> {
        world.children_of_class(me, id::STATE_MACHINE).first().copied()
    }

    fn trigger(&mut self, cx: &mut Cx<'_>, volume: usize) {
        self.armed_by = Some(volume);
        let warp = cx.world.children_of_class(cx.me, id::WARP).first().and_then(|&w| cx.world.oid(w));
        let before = cx.progress.reach_checkpoint(cx.world.info.id, self.index, warp);
        if let Some(m) = Self::machine(cx.world, cx.me) {
            // Checkpoints start their own machine (scripts.md 9), then send it to the triggered state.
            let goal = if before { state::RETRIGGERED } else { state::TRIGGERED };
            if let Some(s) = cx.world.slot(m) {
                let mut b = cx.world.take_behavior(s);
                if let super::Behavior::Machine(mc) = &mut b
                    && !mc.run.running
                {
                    mc.start(cx.world, m);
                }
                cx.world.put_behavior(s, b);
            }
            cx.world.set_option(m, OPT_GOAL, OptionValue::Short(goal));
        }
        cx.world.request(Request::Rumble { kind: -1, strength: RUMBLE.0 });
    }
}

impl ClassBehavior for Checkpoint {
    fn ticks(&self) -> bool {
        true
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        self.index =
            cx.world.slots_of_class(id::CHECKPOINT).iter().position(|&s| s == cx.me.index()).unwrap_or(0) as u32;
        for v in cx.world.children_of_class(cx.me, id::VOLUME) {
            let Some(rec) = cx.world.record(v) else { continue };
            let Some(vr) = cx.world.level.stream.volumes.iter().find(|x| x.record == rec) else { continue };
            let Ok(cv) = ConvexVolume::parse(&cx.world.level.bytes, vr.offset) else { continue };
            let w = cx.world.world_transform(v).unwrap_or(formats::geometry::Affine::IDENTITY);
            self.volumes.push((v, PlacedVolume::new(cv, &w)));
        }
        let id = cx.world.info.id;
        let reached = cx.progress.level_ref(id).is_some_and(|l| l.checkpoints & (1 << self.index.min(7)) != 0);
        if reached
            && let Some(m) = Self::machine(cx.world, cx.me)
            && let Some(mb) = cx.world.behavior_mut(m).and_then(super::Behavior::as_machine_mut)
        {
            mb.run.state = state::RESTORED;
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        cx.world.set_hidden(cx.me, cx.progress.sprint.is_some());
        let hit = cx.world.take_inbox(cx.me.index()).iter().any(|n| matches!(n, Notice::Hit(_)));
        let Some(p) = cx.player_position() else { return };
        for (h, v) in &mut self.volumes {
            if let Some(w) = cx.world.world_transform(*h) {
                v.set_world(&w);
            }
        }
        if let Some(k) = self.armed_by {
            let centre = self.volumes.get(k).and_then(|(h, _)| cx.world.position(*h)).unwrap_or(p);
            if dist_sq(p, centre) > REARM_DISTANCE * REARM_DISTANCE {
                self.armed_by = None;
            }
            return;
        }
        if let Some(k) = self.volumes.iter().position(|(_, v)| v.contains(p)) {
            self.trigger(cx, k);
        } else if hit {
            self.trigger(cx, 0);
        }
    }
}
