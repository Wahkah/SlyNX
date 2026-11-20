//! Projectiles of the turrets, biplanes and combat cars, and the job hook that hands a job's machine to the player.
//!
//! **Projectiles.** A machine that fires adds a [`Projectile`] to `World::shared.vehicles`; [`step`] (once per game
//! step, a marked hook in the game loop) moves each under its gravity and tests it against what lies within its blast
//! radius ([`BLAST_RADIUS`], 200; specs/sly2/vehicles-and-minigames.md 10.4), in this order: guards (class 58), which
//! get `Notice::Hit` from the shooter; the gang's characters (not the shooter's driver), which take the contact
//! (kind `0x100`, 10) through `World::shared.gang_hits`; and other machines, which get `Notice::Hit`. A shot ends at
//! its first hit or when its life runs out. The projectiles' own objects (pool templates, option 1701's templates)
//! are not spawned; the sessions draw a streak for each (`game::shot_marks`). Speeds: turret 800 (the missiles'
//! default; the launchers' own settings are not read), combat car solved for its target, biplane (Sly 3) option
//! 1703.
//!
//! **Jobs** ([`follow_job`], called with the current job after the job system's step: Sly 2 in the game loop, Sly 3
//! in its session, each a marked hook): when a job begins, the machines already active are noted; while it runs, the
//! first machine of a driveable class that becomes active (a task's switches or script brought it in) is driven
//! (`World::shared.driving`, and the camera follows it); when the job ends the player gets the characters back. In
//! a Sly 2 hacking job, a current station task brings the hacking ship's placement into the world. In the game the
//! task's event-19 handler calls its job's property 1286 (focus, hack area) and starts sequence 2410, whose
//! handlers call property 1285 and open the dock (10.3); until those properties' effects run here, the hook stands
//! in [I].

use alloc::string::String;
use alloc::vec::Vec;

use crate::behavior::sly2_sensors::{GangHit, characters};
use crate::math::{Vec3, length, madd, sub};
use crate::object::Handle;
use crate::world::{Notice, World};

/// A flying shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Seconds left.
    pub life: f32,
    /// The machine that fired it.
    pub owner: Handle,
    /// Damage to a character, in health units.
    pub damage: f32,
    /// Downward acceleration (0 for straight shots).
    pub gravity: f32,
}

/// Machines and minigames' shared state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared {
    pub shots: Vec<Projectile>,
    /// The job being followed: its ID, the machines active when it began, and the machine it handed to the player.
    pub job: Option<(i32, Vec<Handle>, Option<Handle>)>,
    /// Hits counted (guards, characters, machines).
    pub hits: [u32; 3],
}

/// A projectile's blast radius (its owner's radius setting, both defaulting to 200) and its contact: kind 0x100,
/// value −10 (10.4; that −10 is the damage is the spec's inference).
pub const BLAST_RADIUS: f32 = 200.0;
pub const CONTACT_KIND: u32 = 0x100;
pub const DAMAGE: f32 = 10.0;

/// Classes the player can drive, per game.
pub fn driveable(world: &World, class: i16) -> bool {
    if world.sly2.is_some() {
        matches!(class, 69 | 72 | 77)
    } else {
        world.sly3.is_some() && matches!(class, 68 | 74 | 81..=85 | 87 | 88)
    }
}

/// Classes whose objects a shot hits as machines.
fn is_machine(world: &World, class: i16) -> bool {
    driveable(world, class) || (world.sly2.is_some() && class == 73) || (world.sly3.is_some() && class == 75)
}

/// Fires a projectile from `owner`.
pub fn fire(world: &mut World, p: Projectile) {
    world.shared.vehicles.shots.push(p);
}

/// One step of every projectile.
pub fn step(world: &mut World, dt: f32) {
    if world.shared.vehicles.shots.is_empty() {
        return;
    }
    let targets: Vec<(Handle, i16, Vec3)> = world
        .handles()
        .filter(|&h| world.is_active(h))
        .filter_map(|h| {
            let c = world.class(h)?;
            (c == 58 || is_machine(world, c)).then(|| world.position(h).map(|p| (h, c, p))).flatten()
        })
        .collect();
    let gang = characters(world);
    let driver_free = world.shared.driving;
    let mut shots = core::mem::take(&mut world.shared.vehicles.shots);
    shots.retain_mut(|s| {
        s.velocity[2] -= s.gravity * dt;
        s.position = madd(s.position, s.velocity, dt);
        s.life -= dt;
        for &(h, c, p) in &targets {
            if h == s.owner {
                continue;
            }
            if length(sub(p, s.position)) <= BLAST_RADIUS {
                world.notify(h, Notice::Hit(s.owner));
                world.shared.vehicles.hits[if c == 58 { 0 } else { 2 }] += 1;
                return false;
            }
        }
        // The characters, unless the shot is the player's own machine's.
        if Some(s.owner) != driver_free {
            for &(h, p) in &gang {
                if length(sub(p, s.position)) <= BLAST_RADIUS {
                    world.shared.gang_hits.push(GangHit {
                        target: h,
                        amount: s.damage,
                        kind: CONTACT_KIND,
                        source: s.position,
                    });
                    world.shared.vehicles.hits[1] += 1;
                    return false;
                }
            }
        }
        s.life > 0.0
    });
    world.shared.vehicles.shots = shots;
}

/// The active machines the player could drive.
fn active_machines(world: &World) -> Vec<Handle> {
    world.handles().filter(|&h| world.is_active(h) && world.class(h).is_some_and(|c| driveable(world, c))).collect()
}

/// Brings `h` and its ancestors into the world.
fn bring_in(world: &mut World, h: Handle) {
    let mut p = Some(h);
    while let Some(q) = p {
        if !world.in_world(q) {
            world.set_in_world(q, true);
        }
        p = world.parent(q);
    }
}

/// Follows the current job (`job` its object ID, `task` the current task's internal name): see the module.
pub fn follow_job(world: &mut World, job: Option<i32>, task: Option<&str>) {
    let st = world.shared.vehicles.job.clone();
    match (job, st) {
        (None, None) => {}
        (None, Some((_, _, handed))) => {
            if handed.is_some() && world.shared.driving == handed {
                world.shared.driving = None;
                world.shared.camera_follow = None;
            }
            world.shared.vehicles.job = None;
        }
        (Some(j), st) => {
            let (baseline, handed) = match st {
                Some((id, b, h)) if id == j => (b, h),
                old => {
                    if let Some((_, _, Some(h))) = old
                        && world.shared.driving == Some(h)
                    {
                        world.shared.driving = None;
                        world.shared.camera_follow = None;
                    }
                    (active_machines(world), None)
                }
            };
            if world.sly2.is_some() && task.is_some_and(|t| t.contains("hack_station")) {
                let ships: Vec<Handle> = world.handles().filter(|&h| world.class(h) == Some(72)).collect();
                if let Some(&ship) =
                    ships.iter().find(|&&h| !world.is_active(h)).filter(|_| !ships.iter().any(|&h| world.is_active(h)))
                {
                    bring_in(world, ship);
                    world.update_transforms();
                }
            }
            let mut handed = handed;
            if world.shared.driving.is_none() && handed.is_none() {
                let new = active_machines(world).into_iter().find(|h| !baseline.contains(h));
                if let Some(m) = new {
                    world.shared.driving = Some(m);
                    world.shared.camera_follow = Some(m);
                    handed = Some(m);
                }
            }
            world.shared.vehicles.job = Some((j, baseline, handed));
        }
    }
}

/// The current job's ID and task name of a job system, for [`follow_job`].
pub fn job_of(jobs: &crate::game::sly2_jobs::Jobs) -> (Option<i32>, Option<String>) {
    let job = jobs.job.map(|j| jobs.graph.jobs[j].id);
    let task = jobs.current.map(|t| jobs.graph.tasks[t].name.clone());
    (job, task)
}
