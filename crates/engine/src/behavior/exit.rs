//! Level exits (class 125, level-flow.md 2; classes/markers-and-volumes.md 6).
//!
//! The player entering the exit's convex volume triggers it: its own animations play, then after option 173's delay
//! (or once the animations end) the game is asked to load the destination at the named warp (option 163) with the
//! wipe of option 166. An exit with option 171 leaves through the world's default exit instead. A key exit (option
//! 169) is present only once the level's key is collected. During a sprint, a key, default or "follows default"
//! exit ends the sprint instead of leaving. After a warp arrival, exits are blocked until the player has left their
//! volume.

use formats::collision::ConvexVolume;
use formats::disc::Descriptor;

use super::{ClassBehavior, Cx};
use crate::game::progress::level_flag;
use crate::object::Handle;
use crate::world::volume::PlacedVolume;
use crate::world::{AnimCommand, Request, World};

/// Exit options (level-flow.md 2).
pub mod opt {
    /// Warp (OID or alias) to arrive at.
    pub const WARP: u16 = 163;
    /// Wipe kind: 0 fade, 1 iris, 2 world map, 3 freeze.
    pub const WIPE: u16 = 166;
    /// The world's default exit.
    pub const DEFAULT: u16 = 168;
    /// Key exit: present only once the key is collected.
    pub const KEY_EXIT: u16 = 169;
    /// Follows the default exit.
    pub const FOLLOWS_DEFAULT: u16 = 170;
    /// Trigger the world's default exit instead of its own destination.
    pub const USE_DEFAULT: u16 = 171;
    /// Show the collected-totals panel while the player is near.
    pub const TOTALS: u16 = 172;
    /// Delay between triggering and the wipe, seconds.
    pub const DELAY: u16 = 173;
}

/// An exit's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExitState {
    /// Waiting for the player.
    Idle,
    /// Triggered: waiting for the delay (`Some(seconds left)`) or the animations (`None`).
    Triggered(Option<f32>),
    /// The request was made.
    Left,
}

/// A level exit.
#[derive(Clone, Debug, PartialEq)]
pub struct Exit {
    /// Destination identity (the level descriptor's search key), if any.
    pub destination: Option<u32>,
    /// The trigger volume.
    pub volume: PlacedVolume,
    /// State.
    pub state: ExitState,
    /// Blocked until the player leaves the volume (after a warp arrival).
    pub blocked: bool,
    /// Owned animations.
    pub animations: u32,
    /// Owned animations still playing.
    pub playing: u32,
}

impl Exit {
    /// Reads the exit's destination, volume and owned animations from its record.
    pub fn new(world: &World, h: Handle) -> Option<Exit> {
        let rec = world.record(h)?;
        let r = world.level.record(rec);
        let destination = world
            .level
            .bytes
            .get(r.offset + 6..r.offset + 6 + Descriptor::SIZE)
            .and_then(|b| Descriptor::decode(b).ok().flatten())
            .map(|d| d.identity);
        let volume = world
            .level
            .stream
            .volumes
            .iter()
            .find(|v| v.record == rec)
            .and_then(|v| ConvexVolume::parse(&world.level.bytes, v.offset).ok())
            .unwrap_or_default();
        let animations = world.level.stream.owned_animations.iter().filter(|a| a.owner == rec).count() as u32;
        let w = world.world_transform(h).unwrap_or(formats::geometry::Affine::IDENTITY);
        Some(Exit {
            destination,
            volume: PlacedVolume::new(volume, &w),
            state: ExitState::Idle,
            blocked: false,
            animations,
            playing: 0,
        })
    }

    fn leave(&mut self, world: &mut World, me: Handle) {
        self.state = ExitState::Left;
        let o = world.options(me).cloned().unwrap_or_default();
        if o.bool_or(opt::USE_DEFAULT, false) {
            world.request(Request::DefaultExit);
            return;
        }
        if let Some(identity) = self.destination {
            world.request(Request::LoadWorld {
                identity,
                warp: o.i16(opt::WARP),
                wipe: o.i32(opt::WIPE).unwrap_or(0).clamp(0, 3) as u8,
            });
        }
    }
}

impl ClassBehavior for Exit {
    fn ticks(&self) -> bool {
        self.state != ExitState::Left
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let key_exit = cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::KEY_EXIT, false));
        if key_exit && !cx.progress.has_flag(cx.world.info.id, level_flag::KEY) {
            cx.world.remove_quietly(cx.me);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if let Some(w) = cx.world.world_transform(cx.me) {
            self.volume.set_world(&w);
        }
        let inside = cx.player_position().is_some_and(|p| self.volume.contains(p));
        if self.blocked {
            self.blocked = inside;
            return;
        }
        match self.state {
            ExitState::Left => {}
            ExitState::Idle => {
                if !inside {
                    return;
                }
                let o = cx.world.options(cx.me).cloned().unwrap_or_default();
                let ends_sprint = o.bool_or(opt::KEY_EXIT, false)
                    || o.bool_or(opt::DEFAULT, false)
                    || o.bool_or(opt::FOLLOWS_DEFAULT, false);
                if ends_sprint && cx.progress.sprint.is_some_and(|s| s.world == cx.world.info.id) {
                    if cx.progress.finish_sprint() {
                        cx.world.request(Request::Autosave);
                    }
                    self.blocked = true;
                    return;
                }
                for i in 0..self.animations {
                    cx.world.anim.push(AnimCommand::PlayOwned {
                        owner: cx.me,
                        index: i,
                        reverse: false,
                        speed: 1.0,
                        tag: i,
                    });
                }
                self.playing = self.animations;
                self.state = ExitState::Triggered(o.f32(opt::DELAY));
                if o.f32(opt::DELAY).is_none() && self.animations == 0 {
                    self.leave(cx.world, cx.me);
                }
            }
            ExitState::Triggered(Some(t)) => {
                let t = t - cx.dt;
                if t <= 0.0 {
                    self.leave(cx.world, cx.me);
                } else {
                    self.state = ExitState::Triggered(Some(t));
                }
            }
            ExitState::Triggered(None) => {
                if self.playing == 0 {
                    self.leave(cx.world, cx.me);
                }
            }
        }
    }

    fn animation_finished(&mut self, _world: &mut World, _me: Handle, _tag: u32) {
        self.playing = self.playing.saturating_sub(1);
    }
}
