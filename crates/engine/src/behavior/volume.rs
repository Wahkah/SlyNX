//! Trigger volumes (classes 41 and 42, interactables.md 2).
//!
//! A volume reacts to the player being inside it: inside its collision mesh (asked of the collision queries), or
//! within its sphere radius when it collides as a sphere (option 133). Anything inside presses it like a button
//! (event 2, sound unless silent, its animations, its dialogue); after the repush delay (option 269, default 1 s,
//! negative = never) it returns to untouched, or with "must exit" (option 274) waits until everything has left.
//! With "report changes" (option 277) it raises event 12 whenever the set inside changes. A remembered volume
//! (option 273) stays pressed across checkpoint restarts.

use alloc::vec::Vec;

use super::{ClassBehavior, Cx, dist_sq, play_child_segments};
use crate::object::Handle;
use crate::world::{AnimCommand, Request, World, opt as wopt};

/// Trigger-volume options (interactables.md 2).
pub mod opt {
    /// Repush delay, seconds.
    pub const REPUSH: u16 = 269;
    /// Silent.
    pub const SILENT: u16 = 271;
    /// Restart animations on every press.
    pub const RESTART: u16 = 272;
    /// Remembered across checkpoint restarts.
    pub const REMEMBERED: u16 = 273;
    /// Must exit before it can fire again.
    pub const MUST_EXIT: u16 = 274;
    /// Raise event 12 when the set inside changes.
    pub const REPORT: u16 = 277;
    /// A dialogue (OID) to play when pressed.
    pub const DIALOGUE: u16 = 279;
    /// "On" animations (add).
    pub const ON_ANIMS: u16 = 267;
    /// "Off" animations (add).
    pub const OFF_ANIMS: u16 = 268;
    /// Objects that count (add; up to 8).
    pub const OBJECTS: u16 = 275;
    /// Goal records (add).
    pub const GOALS: u16 = 276;
}

/// Animation tag of the volume's animations.
pub const TAG_ON: u32 = 1;

/// A trigger volume's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VolumeState {
    /// Waiting.
    Untouched,
    /// Pressed, for this long.
    Pressed(f32),
    /// Waiting for everything to leave ("must exit").
    WaitExit,
}

/// A trigger volume.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerVolume {
    /// State.
    pub state: VolumeState,
    /// The player was inside last frame.
    pub inside: bool,
}

impl TriggerVolume {
    /// A volume.
    pub fn new(_world: &World, _h: Handle) -> TriggerVolume {
        TriggerVolume { state: VolumeState::Untouched, inside: false }
    }

    /// Whether a world point is inside the volume (collision mesh, else the sphere radius).
    pub fn contains(cx: &Cx<'_>, p: [f32; 3]) -> bool {
        if let Some(b) = cx.collision.point_in_solid(cx.world, cx.me, p) {
            return b;
        }
        match cx.world.options(cx.me).and_then(|o| o.f32(wopt::SPHERE_RADIUS)) {
            Some(r) => dist_sq(p, cx.my_position()) <= r * r,
            None => false,
        }
    }

    fn press(&mut self, world: &mut World, me: Handle, restored: bool) {
        let o = world.options(me).cloned().unwrap_or_default();
        self.state = VolumeState::Pressed(0.0);
        if restored {
            world.raise(me, script::event::SAVED_TRIGGER, Vec::new());
            play_child_segments(world, me, TAG_ON, true);
            world.anim.push(AnimCommand::Finish { owner: me, tag: TAG_ON });
            return;
        }
        world.raise(me, script::event::TRIGGERED, Vec::new());
        if !o.bool_or(opt::SILENT, false) {
            world.play_sound(super::button::PRESS_SOUND, Some(me));
        }
        play_child_segments(world, me, TAG_ON, o.bool_or(opt::RESTART, false));
        super::play_listed(world, me, opt::ON_ANIMS, TAG_ON, o.bool_or(opt::RESTART, false));
        super::apply_goal_records(world, me, opt::GOALS, true);
        if let Some(d) = o.i16(opt::DIALOGUE).filter(|&d| d > 0) {
            world.request(Request::Dialogue(d));
        }
    }
}

impl ClassBehavior for TriggerVolume {
    fn ticks(&self) -> bool {
        true
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let remembered = cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::REMEMBERED, false));
        let world_id = cx.world.info.id;
        if remembered
            && cx
                .world
                .persist_key(cx.me)
                .is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.pressed.contains(&k)))
        {
            self.press(cx.world, cx.me, true);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        // What counts: the objects the volume lists (up to 8), else the player (interactables.md 2).
        let listed: Vec<i16> =
            cx.world.options(cx.me).map(|o| o.shorts(opt::OBJECTS).take(8).collect()).unwrap_or_default();
        let inside = if listed.is_empty() {
            cx.player_position().is_some_and(|p| TriggerVolume::contains(cx, p))
        } else {
            listed.iter().any(|&oid| {
                let p = if oid == crate::game::player_bridge::PLAYER_OID {
                    cx.player_position()
                } else {
                    cx.world.find_nearest(oid, Some(cx.me)).and_then(|h| cx.world.position(h))
                };
                p.is_some_and(|p| TriggerVolume::contains(cx, p))
            })
        };
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        if inside != self.inside && o.bool_or(opt::REPORT, false) {
            cx.raise(script::event::VOLUME_UPDATE, Vec::new());
        }
        let was_inside = self.inside;
        self.inside = inside;
        match self.state {
            VolumeState::Untouched => {
                if inside {
                    self.press(cx.world, cx.me, false);
                    if o.bool_or(opt::REMEMBERED, false)
                        && let Some(k) = cx.world.persist_key(cx.me)
                    {
                        let world_id = cx.world.info.id;
                        cx.progress.level(world_id).pressed.insert(k);
                    }
                }
            }
            VolumeState::Pressed(t) => {
                if was_inside && !inside {
                    cx.raise(script::event::UNTRIGGERED, Vec::new());
                    super::play_listed(cx.world, cx.me, opt::OFF_ANIMS, 2, true);
                    super::apply_goal_records(cx.world, cx.me, opt::GOALS, false);
                }
                let t = t + cx.dt;
                let delay = o.f32_or(opt::REPUSH, super::button::DEFAULT_REPUSH);
                if delay >= 0.0 && t >= delay {
                    self.state = if o.bool_or(opt::MUST_EXIT, false) && inside {
                        VolumeState::WaitExit
                    } else {
                        VolumeState::Untouched
                    };
                } else {
                    self.state = VolumeState::Pressed(t);
                }
            }
            VolumeState::WaitExit => {
                if !inside {
                    cx.raise(script::event::UNTRIGGERED, Vec::new());
                    self.state = VolumeState::Untouched;
                }
            }
        }
    }
}
