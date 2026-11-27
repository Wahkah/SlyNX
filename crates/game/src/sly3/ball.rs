//! m1_ball (specs/sly3/missions.md 4.1a, job 2168 in v_ext): Murray launched as a ball at six tanks.
//!
//! What the spec gives is used as given:
//!
//! - **The launcher**: group `murray_ball_launcher` has a clip (163095, class 130) whose module answers script event
//!   2 (the clip playing). The handler sets Murray's velocity to (0, 0, 5,000), which launches him (a crew member's
//!   velocity, `script_host::sly3`), and, when the launcher's custom property 2760 names the current job, calls the
//!   job's method 2677: the launch state, and 2173 (`t1_ball_first_launch`) made current.
//! - **2175** is made current by its own module on task event 94 when the job's 2679 ("launched recently") is true.
//! - **The tanks**: the job's table pairs tank placements 139371 to 139376 with tasks 2178 to 2183 ([`TANKS`]); the
//!   job's method 2678 (tank) destroys one and asks for its task's state 2.
//!
//! Stand-ins, each labelled [I] (the spec does not trace them):
//!
//! - **What plays the launcher's clip**: Murray, played and not in flight, coming within [`PAD_REACH`] of the
//!   launcher (the clip's class-47 owner, a copy of `murray_ball_launcher`) plays it: script event 2 on the clip, with
//!   the launcher as the run's animated object (script ID 265, which the handler reads; class 130 does not list it,
//!   so it is given as a custom property of the clip). He must step off the pad before it plays again.
//! - **The ball's flight** (its controls and physics were not traced): from the launch until he lands, the stick
//!   sets Murray's horizontal velocity, at [`BALL_SPEED`] for a full deflection (relative to the camera, as the crew
//!   reads the stick).
//! - **The impact** (task event 94, "probably a ball impact"): broadcast when he lands after a launch.
//! - **What calls 2678**: Murray in flight, or within [`HIT_AFTER`] seconds of landing, within [`TANK_REACH`] of a
//!   tank whose task is available (and within twice that height) hits it: the job's 2678 is called with the tank (its
//!   copy root, as a search for the tank's ID finds it), once per tank.
//! - The failure cases (the job's 1260 on codes 15, 16, 24 and 29) need hits on Murray, which nothing here makes.

use alloc::vec::Vec;

use engine::game::Game;
use engine::game::sly2_jobs::Jobs;
use engine::gang::Who;
use engine::object::Handle;
use engine::world::World;

/// The job.
pub const JOB: i32 = 2168;
/// The launcher's clip and its class.
pub const LAUNCH_CLIP: i32 = 163095;
pub const CLIP_CLASS: i16 = 130;
/// The class of the launcher (the clip's owner).
pub const LAUNCHER_CLASS: i16 = 47;
/// The event the clip's module answers (the clip playing), and the run's animated object (script ID 265).
pub const PLAY_EVENT: u32 = 2;
pub const ANIMATED: u32 = 265;
/// The task event 2175's module answers (the impact, an interpretation of the spec).
pub const IMPACT_EVENT: u32 = 94;
/// The job's method that destroys a tank.
pub const DESTROY: u32 = 2678;
/// The tanks' placements and their tasks.
pub const TANKS: [(i32, i32); 6] =
    [(139371, 2178), (139372, 2179), (139373, 2180), (139374, 2181), (139375, 2182), (139376, 2183)];
/// Murray's class.
pub const MURRAY_CLASS: i16 = 53;
/// Working values [I]: how near the launcher plays it (horizontally, units), how fast a full stick moves the ball in
/// flight (units per second), how near a tank a hit lands (units), and how long after landing a hit still counts
/// (seconds).
pub const PAD_REACH: f32 = 120.0;
pub const BALL_SPEED: f32 = 2500.0;
pub const TANK_REACH: f32 = 400.0;
pub const HIT_AFTER: f32 = 0.5;

/// What the ball stand-in keeps between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ball {
    /// In flight since a launch (not yet landed).
    pub flying: bool,
    /// Seconds since the last landing (counts up; infinite before the first).
    pub since_landed: f32,
    /// Seconds in flight.
    pub flight: f32,
    /// The launcher may play (Murray stepped off it since the last launch).
    pub armed: bool,
    /// Launches and tanks hit so far.
    pub launches: u32,
    pub hit: Vec<i32>,
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])
}

/// The launchers: (the clip, its class-47 owner).
pub fn launchers(world: &World) -> Vec<(Handle, Handle)> {
    world
        .slots_with_sly3_id(LAUNCH_CLIP)
        .into_iter()
        .filter_map(|k| world.handle(k))
        .filter(|&h| world.class(h) == Some(CLIP_CLASS))
        .filter_map(|c| {
            let owner = core::iter::successors(world.parent(c), |&q| world.parent(q))
                .find(|&q| world.class(q) == Some(LAUNCHER_CLASS))?;
            Some((c, owner))
        })
        .collect()
}

/// A tank's object as the scripts find it: the copy root under the tank's placement.
pub fn tank(world: &World, id: i32) -> Option<Handle> {
    let p = world.find_sly3_id(id, None)?;
    world.children(p).next().or(Some(p))
}

impl Ball {
    /// A ball before any launch.
    pub fn new() -> Ball {
        Ball { since_landed: f32::INFINITY, armed: true, ..Default::default() }
    }

    /// One frame: the launcher, the flight, the landing and the hits (module notes). `stick` is the pad's left stick
    /// and `camera_yaw` the camera's heading.
    pub fn update(&mut self, g: &mut Game, jobs: &mut Jobs, stick: [f32; 2], camera_yaw: f32, dt: f32) {
        let Some(k) = jobs.job.filter(|&k| jobs.graph.jobs[k].id == JOB) else { return };
        let Some(me) = g.world.player.filter(|&p| g.world.class(p) == Some(MURRAY_CLASS)) else { return };
        let Some(at) = g.world.position(me) else { return };
        self.since_landed += dt;
        // The launcher.
        let near = launchers(&g.world).into_iter().find(|&(_, o)| {
            g.world.position(o).is_some_and(|p| dist2(p, at) <= PAD_REACH * PAD_REACH && (p[2] - at[2]).abs() < 300.0)
        });
        match near {
            Some((clip, owner)) if self.armed && !self.flying => {
                self.armed = false;
                self.flying = true;
                self.flight = 0.0;
                self.launches += 1;
                g.set_custom(clip, ANIMATED, script::Value::Object(Some(owner.to_script())));
                g.raise_now(clip, PLAY_EVENT, &[]);
            }
            None => self.armed = true,
            _ => {}
        }
        // The flight: the stick steers; landing ends it.
        if self.flying {
            self.flight += dt;
            let (s, c) = (engine::math::sin(camera_yaw), engine::math::cos(camera_yaw));
            // Up is the camera's heading, right its right (as the crew reads the stick).
            let dir = [c * stick[1] + s * stick[0], s * stick[1] - c * stick[0]];
            let murray = g.systems.player.gang_mut().and_then(|gang| gang.get_mut(Who::Murray));
            if let Some(m) = murray {
                if self.flight > 0.2 && m.grounded {
                    self.flying = false;
                    self.since_landed = 0.0;
                    jobs.task_event(IMPACT_EVENT, Some(me));
                } else {
                    m.velocity[0] = dir[0] * BALL_SPEED;
                    m.velocity[1] = dir[1] * BALL_SPEED;
                }
            }
        }
        // The hits.
        if !(self.flying || self.since_landed <= HIT_AFTER) {
            return;
        }
        let Some(job) = g.world.handle(jobs.graph.jobs[k].slot) else { return };
        for (id, task) in TANKS {
            if self.hit.contains(&id) || jobs.task_state(task) != Some(1) {
                continue;
            }
            let Some(t) = tank(&g.world, id) else { continue };
            let reach =
                |p: [f32; 3]| dist2(p, at) <= TANK_REACH * TANK_REACH && (p[2] - at[2]).abs() <= 2.0 * TANK_REACH;
            if g.world.position(t).is_some_and(reach) {
                self.hit.push(id);
                g.call_custom(job, DESTROY, alloc::vec![script::Value::Object(Some(t.to_script()))]);
            }
        }
    }
}
