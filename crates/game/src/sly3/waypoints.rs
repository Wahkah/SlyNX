//! Mission waypoints (class 120; specs/sly3/missions.md 4.2): the job start markers that task option 1407 names.
//! Walking in is what starts a job; the waypoint reads no button.
//!
//! Each frame, for every waypoint in the world: the player (player 0's controlled object) counts only when its class
//! derives from option 1483's (default 49). It is in when nearer than the enter radius, or than the leave radius
//! once in (option 1484; defaults 300 and 500). Kinds 0 and 2 (option 1486) do not count while a job runs. When the
//! state changes the waypoint gets script event 5 (enter) or 6 (leave), then task event 29 or 30 is broadcast with
//! the waypoint as its object, to the available tasks and then the current job; the task's script starts the job.
//!
//! Not modelled: the extra test on a class-51 player (missions.md 4.2 step 4, "not busy").

use alloc::vec::Vec;

use engine::game::sly2_jobs::Jobs;
use engine::object::Handle;
use engine::world::World;

use super::flow::actions;

/// The waypoint class.
pub const WAYPOINT_CLASS: i16 = 120;
/// Script events raised on the waypoint.
pub const EVENT_ENTER: u32 = 5;
/// See [`EVENT_ENTER`].
pub const EVENT_LEAVE: u32 = 6;
/// Task events broadcast.
pub const TASK_ENTER: u32 = 29;
/// See [`TASK_ENTER`].
pub const TASK_LEAVE: u32 = 30;

/// Option IDs.
pub mod opt {
    /// The class the player must derive from.
    pub const PLAYER_CLASS: u16 = 1483;
    /// Enter and leave radii.
    pub const RADII: u16 = 1484;
    /// Kind.
    pub const KIND: u16 = 1486;
}

/// One waypoint and its state.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    /// The object.
    pub handle: Handle,
    /// Option 1483: the player's class must derive from it.
    pub player_class: i16,
    /// Option 1484.
    pub enter: f32,
    /// See [`Waypoint::enter`].
    pub leave: f32,
    /// Option 1486: 0 a job's start, 1 used during a job, 2 counts only with no job, −1 a fall-back.
    pub kind: i32,
    /// Whether the player is in.
    pub inside: bool,
}

/// What changed this frame: (waypoint, entered).
pub type Change = (Handle, bool);

fn values(world: &World, h: Handle, id: u16) -> Vec<f32> {
    let mut v: Vec<f32> = actions(world, h, id).into_iter().flatten().map(|x| x as f32).collect();
    if v.is_empty()
        && let Some(o) = world.options(h)
    {
        v.extend(o.f32(id));
    }
    v
}

/// The world's waypoints.
pub fn waypoints(world: &World) -> Vec<Waypoint> {
    world
        .handles()
        .filter(|&h| world.class(h) == Some(WAYPOINT_CLASS))
        .map(|h| {
            let radii = values(world, h, opt::RADII);
            Waypoint {
                handle: h,
                player_class: values(world, h, opt::PLAYER_CLASS).first().map_or(49, |&c| c as i16),
                enter: radii.first().copied().unwrap_or(300.0),
                leave: radii.get(1).copied().unwrap_or(500.0),
                kind: values(world, h, opt::KIND).first().map_or(0, |&k| k as i32),
                inside: false,
            }
        })
        .collect()
}

/// One frame: each waypoint's in/out state for the player, and the events on a change. `derives(class, base)`
/// says whether `class` derives from `base`.
pub fn update(
    list: &mut [Waypoint],
    world: &mut World,
    mut jobs: Option<&mut Jobs>,
    derives: &dyn Fn(i16, i16) -> bool,
) -> Vec<Change> {
    let player = world.player;
    let at = player.and_then(|p| world.position(p));
    let class = player.and_then(|p| world.class(p));
    let job_running = jobs.as_ref().is_some_and(|j| j.job.is_some());
    let mut changes = Vec::new();
    for w in list.iter_mut() {
        if !world.is_active(w.handle) {
            continue;
        }
        let Some(me) = world.position(w.handle) else { continue };
        let counts = class.is_some_and(|c| derives(c, w.player_class)) && !(job_running && matches!(w.kind, 0 | 2));
        let inside = counts
            && at.is_some_and(|p| {
                let (dx, dy, dz) = (p[0] - me[0], p[1] - me[1], p[2] - me[2]);
                let d = engine::math::sqrt(dx * dx + dy * dy + dz * dz);
                d < if w.inside { w.leave } else { w.enter }
            });
        if inside != w.inside {
            w.inside = inside;
            world.raise(w.handle, if inside { EVENT_ENTER } else { EVENT_LEAVE }, Vec::new());
            if let Some(j) = jobs.as_deref_mut() {
                j.task_event(if inside { TASK_ENTER } else { TASK_LEAVE }, Some(w.handle));
                j.apply(world);
            }
            changes.push((w.handle, inside));
        }
    }
    changes
}
