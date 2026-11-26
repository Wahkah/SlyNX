//! The canal chase's boat (specs/sly3/missions.md 4.1a), around its own script (module 330 on rig 139806, which
//! reads the pad, steers, jumps, fires and heats up).
//!
//! - **Movement**: the script writes its velocity as option 194 (stream 77), (0, sideways, vertical) in the boat's
//!   own frame, which its owner turns (the rig 139840 that the looping segment 139836 carries round the canal,
//!   `clips::Clips::play_segment`; the boat sits on it turned a quarter, its x axis along the track). Each frame the
//!   object moves by it, relative to where its owner carries it.
//!   Nothing stops it at the canal's banks: the boat has no collision here.
//! - **Jump**: the script sets its drive machine's run (machine 150013) to the jump state 15663; the machine's clips
//!   are not played, so, stand-in [I], the run goes back to the water state 152160 after [`JUMP_TIME`], with 15663
//!   as its state before for one frame (what the script reads as a landing). The jump itself does not lift the
//!   boat.

use alloc::vec::Vec;

use engine::object::{Handle, OptionValue};
use engine::world::World;

/// The velocity option (script ID 194).
const VELOCITY: u16 = 77;
/// The rig class that carries the option.
const RIG_CLASS: i16 = 47;
/// The machine-run class and its state options (script IDs 529, 532).
const RUN_CLASS: i16 = 173;
const RUN_STATE: u16 = 416;
const RUN_PREVIOUS: u16 = 419;
/// The drive machine's water and jump states (machine 150013).
pub const WATER: i32 = 152160;
pub const JUMP: i32 = 15663;
/// How long a jump lasts, seconds (stand-in [I]).
pub const JUMP_TIME: f32 = 0.8;

/// The enemy boats (missions.md 4.1a): stand-in [I] for their movement (their placements' paths are open). Each
/// rig of script 2337's list (139843, 139846, 139849) keeps the distance ahead of the boat's owner that 2337's
/// module aims for (1800, 2600 and 3300 units), on the owner's own track, at the track's steady speed.
pub const ENEMY_RIGS: [(i32, f32); 3] = [(139843, 1800.0), (139846, 2600.0), (139849, 3300.0)];
/// The owner whose track the enemies keep to, and its speed (â‰ˆ3000 units/s, missions.md 4.1a).
pub const OWNER_RIG: i32 = 139840;
pub const OWNER_SPEED: f32 = 3000.0;
/// The enemy boats' health (script variable 2416 of their module) and the defeat event raised once it reaches 0:
/// stand-in [I] (what raises it in the game is open).
pub const HEALTH_SYMBOL: u32 = 2416;
pub const DEFEAT_EVENT: u32 = 5;

/// The movers found and the runs in their jump.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boats {
    movers: Vec<Handle>,
    scan: f32,
    jumps: Vec<(Handle, f32)>,
    landed: Vec<Handle>,
    /// Enemy boats whose defeat was raised.
    pub defeated: Vec<Handle>,
}

impl Boats {
    /// The enemy rigs ahead of the owner (stand-in, [`ENEMY_RIGS`]) while their placement is in the world.
    pub fn place_enemies(world: &mut World, clips: &super::clips::Clips) {
        for (rig, ahead) in ENEMY_RIGS {
            let Some(h) = world.find_sly3_id(rig, None) else { continue };
            let live = world.children(h).any(|c| world.in_world(c));
            let Some(at) = clips.segment_point(world, OWNER_RIG, ahead / OWNER_SPEED) else { continue };
            let Some(next) = clips.segment_point(world, OWNER_RIG, (ahead + 100.0) / OWNER_SPEED) else { continue };
            if !live {
                continue;
            }
            let mut m = world.world_transform(h).unwrap_or(formats::geometry::Affine::IDENTITY);
            let (dx, dy) = (next[0] - at[0], next[1] - at[1]);
            if dx * dx + dy * dy > 1e-6 {
                let yaw = engine::math::atan2(dy, dx);
                let (s, c) = (engine::math::sin(yaw), engine::math::cos(yaw));
                m.cols = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]];
            }
            m.t = at;
            world.set_world_transform(h, m);
        }
    }
}

/// How far a velocity `v` in a frame with axes `cols` (x, y, z columns) carries an object in `dt` seconds, in the
/// world.
pub fn displacement(cols: &[[f32; 3]; 3], v: [f32; 3], dt: f32) -> [f32; 3] {
    core::array::from_fn(|i| (cols[0][i] * v[0] + cols[1][i] * v[1] + cols[2][i] * v[2]) * dt)
}

fn vel(world: &World, h: Handle) -> Option<[f32; 3]> {
    world.options(h)?.vec3(VELOCITY)
}

impl Boats {
    /// One frame of `dt` seconds.
    pub fn step(&mut self, world: &mut World, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        // The rigs a script gave a velocity, looked for once a second.
        self.scan -= dt;
        if self.scan <= 0.0 {
            self.scan = 1.0;
            self.movers = world
                .slots_of_class(RIG_CLASS)
                .iter()
                .filter_map(|&s| world.handle(s))
                .filter(|&h| vel(world, h).is_some())
                .collect();
        }
        for &h in &self.movers.clone() {
            let (Some(v), Some(m)) = (vel(world, h), world.world_transform(h)) else { continue };
            if !world.in_world(h) || v == [0.0; 3] {
                continue;
            }
            // Its own frame (x forward, y sideways, z up), which turns with its owner.
            let d = displacement(&m.cols, v, dt);
            let mut to = m;
            to.t = [m.t[0] + d[0], m.t[1] + d[1], m.t[2] + d[2]];
            world.set_world_transform(h, to);
        }
        // A landing reads for one frame, then the state before is the water's again.
        for h in core::mem::take(&mut self.landed) {
            if let Some(o) = world.options_mut(h) {
                o.set(RUN_PREVIOUS, OptionValue::Int(WATER));
            }
        }
        // Jumps land after `JUMP_TIME`.
        let runs: Vec<Handle> = world.slots_of_class(RUN_CLASS).iter().filter_map(|&s| world.handle(s)).collect();
        for h in runs {
            let state = world.options(h).and_then(|o| o.get(RUN_STATE)).and_then(OptionValue::as_i32);
            if state == Some(JUMP) && !self.jumps.iter().any(|j| j.0 == h) {
                self.jumps.push((h, 0.0));
            }
        }
        for j in &mut self.jumps {
            j.1 += dt;
            if j.1 >= JUMP_TIME
                && let Some(o) = world.options_mut(j.0)
            {
                o.set(RUN_PREVIOUS, OptionValue::Int(JUMP));
                o.set(RUN_STATE, OptionValue::Int(WATER));
                self.landed.push(j.0);
            }
        }
        self.jumps.retain(|j| j.1 < JUMP_TIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_moves_in_the_boats_own_frame() {
        // A boat turned a quarter (its x along the world's −y): sideways (its y) is the world's +x.
        let cols = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let d = displacement(&cols, [0.0, 600.0, -60.0], 0.5);
        assert_eq!(d, [300.0, 0.0, -30.0]);
        assert_eq!(displacement(&cols, [0.0; 3], 1.0), [0.0; 3]);
    }
}
