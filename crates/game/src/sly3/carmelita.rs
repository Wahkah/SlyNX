//! Carmelita played (specs/sly3/missions.md 4.1a, the turf-war battles of m1_turf_war): each battle's script gives
//! player 0 control (`0x13`) of a Carmelita copy (pool placement 139252: copy root 28, class 60, option 937 = 600 her
//! maximum health), and the battle's enemies take damage only from her shots (class-89-derived projectiles, 2659).
//!
//! What the spec gives is used as given; her controls, fire button, fire rate, shot speed, damage and aim are **not
//! traced**, so they are stand-ins here, each labelled [I]:
//!
//! - **Movement**: the crew controller (`crew`, Carmelita stand-in): the played member takes the pad and its
//!   position and heading are written into her object.
//! - **Health**: her current health (script ID 940, stream 938) is her maximum (939, stream 937: 600) once her copy
//!   is taken from its pool (1207, `script_host::sly3`: a taken instance is refilled, an inference of the spec). What
//!   happens when it reaches 0 is not traced; nothing here lowers it.
//! - **The shock pistol** [I]: Square ([`FIRE`], the boat's fire button in the canal chase) fires her playable shot
//!   straight ahead, at most every [`FIRE_INTERVAL`] seconds. The shot is a pooled instance of her playable spawn
//!   point (the placement under her copy whose ID her option 1545 names, 6676 in the spec; `World::sly3_acquire`),
//!   placed [`MUZZLE_UP`] above and [`MUZZLE_AHEAD`] ahead of her, flying at [`SHOT_SPEED`] along her heading with
//!   [`SHOT_DAMAGE`] as its damage (549). Aim assist [I] (whether the game has any is not traced): a guard in the
//!   world within [`AIM_CONE`] of her heading and within the shot's reach is aimed at (the nearest to her heading).
//!   The flight and the hit are
//!   `engine::behavior::sly3::projectile` (a hit on a guard is script event 12 with a hit record, whose amount 124 is
//!   the damage).

use engine::behavior::sly3::projectile;
use engine::object::{Handle, OptionValue};
use engine::world::World;
use formats::geometry::Affine;

/// Carmelita's class.
pub const CLASS: i16 = 60;
/// Her current and maximum health (script IDs 940 and 939; streams 938 and 937).
pub const HEALTH: u16 = 938;
pub const MAX_HEALTH: u16 = 937;
/// The option naming her playable shot's spawn point (missions.md 4.1a), and the spec's value for it.
pub const SHOT_POINT_OPTION: u16 = 1545;
pub const SHOT_POINT: i32 = 6676;
/// The fire button [I]: Square (PS2), `Buttons::Y` on the Switch layout.
pub const FIRE: u32 = platform_api::Buttons::Y.0;
/// Working values [I] (not traced): seconds between shots, shot speed (units per second), damage per shot, and
/// where the shot starts (units above her feet, ahead of her).
pub const FIRE_INTERVAL: f32 = 0.25;
pub const SHOT_SPEED: f32 = 4000.0;
pub const SHOT_DAMAGE: f32 = 20.0;
pub const MUZZLE_UP: f32 = 100.0;
pub const MUZZLE_AHEAD: f32 = 60.0;
/// The aim assist's half-angle, radians [I].
pub const AIM_CONE: f32 = 0.35;

/// The Carmelita copy player 0 drives, if any.
pub fn driven(world: &World) -> Option<Handle> {
    world.shared.driving.filter(|&d| world.class(d) == Some(CLASS))
}

/// Her playable shot's spawn point: the placement (class 8) under her copy named by her option 1545, else the
/// spec's 6676.
fn shot_point(world: &World, me: Handle) -> Option<Handle> {
    let id = world.options(me).and_then(|o| o.i32(SHOT_POINT_OPTION)).unwrap_or(SHOT_POINT);
    world.find_sly3_id(id, Some(me)).filter(|&p| world.class(p) == Some(formats_sly3::stream::PLACEMENT))
}

/// The aim assist (module notes): the unit direction from `from` to the guard nearest to heading `dir` within
/// [`AIM_CONE`] and the shot's reach, if any.
fn aim(world: &World, from: [f32; 3], dir: [f32; 3]) -> Option<[f32; 3]> {
    let reach = SHOT_SPEED * projectile::LIFETIME;
    let cos_cone = engine::math::cos(AIM_CONE);
    let mut best: Option<(f32, [f32; 3])> = None;
    for &k in world.slots_of_class(projectile::GUARD_CLASS) {
        let Some(h) = world.handle(k).filter(|&h| world.in_world(h) && world.is_active(h)) else { continue };
        let Some(p) = world.position(h) else { continue };
        let d = [p[0] - from[0], p[1] - from[1], p[2] - from[2]];
        let flat = engine::math::sqrt(d[0] * d[0] + d[1] * d[1]);
        let len = engine::math::sqrt(flat * flat + d[2] * d[2]);
        if len > reach || flat < 1.0 {
            continue;
        }
        let c = (d[0] * dir[0] + d[1] * dir[1]) / flat;
        if c >= cos_cone && best.is_none_or(|b| c > b.0) {
            best = Some((c, [d[0] / len, d[1] / len, d[2] / len]));
        }
    }
    best.map(|b| b.1)
}

/// The shot stand-in, once a frame: `cooldown` counts down by `dt`; Square fires when it has run out. Returns the
/// shot fired.
pub fn update(world: &mut World, pressed: u32, dt: f32, cooldown: &mut f32) -> Option<Handle> {
    *cooldown = (*cooldown - dt).max(0.0);
    let me = driven(world)?;
    if pressed & FIRE == 0 || *cooldown > 0.0 {
        return None;
    }
    let at = world.world_transform(me)?;
    let point = shot_point(world, me)?;
    let shot = world.sly3_acquire(point)?;
    *cooldown = FIRE_INTERVAL;
    let ahead = [at.cols[0][0], at.cols[0][1], 0.0];
    let len = engine::math::sqrt(ahead[0] * ahead[0] + ahead[1] * ahead[1]).max(1e-6);
    let mut dir = [ahead[0] / len, ahead[1] / len, 0.0];
    let from = [at.t[0] + dir[0] * MUZZLE_AHEAD, at.t[1] + dir[1] * MUZZLE_AHEAD, at.t[2] + MUZZLE_UP];
    if let Some(d) = aim(world, from, dir) {
        dir = d;
    }
    world.set_world_transform(shot, Affine { t: from, ..at });
    // Its shooter (script ID 552, missions.md 4.1a): Carmelita.
    if let Some(s) = world.slot(shot) {
        world.shared.sly3_shooters.insert(s, me);
    }
    if let Some(o) = world.options_mut(shot) {
        let v = [dir[0] * SHOT_SPEED, dir[1] * SHOT_SPEED, dir[2] * SHOT_SPEED];
        o.set(projectile::VELOCITY, OptionValue::Vector(v));
        o.set(projectile::DAMAGE, OptionValue::Float(SHOT_DAMAGE));
    }
    Some(shot)
}
