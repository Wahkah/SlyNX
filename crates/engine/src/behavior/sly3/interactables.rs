//! Sly 3 interactables with no Sly 2 behaviour to reuse yet, as far as the Sly 3 specs describe them on their own
//! (specs/sly3/interactables.md 3 and 5, pickpocket-and-pickups.md, answers to the spec questions):
//!
//! - **Barriers** (class 97, Sly 2 class 86): solids whose collision applies only to some objects. Option 512 lists
//!   the classes, 510 the objects (Sly 3 IDs), 513 is the mode: 0, the listed objects collide and everything else
//!   passes; 1, the listed objects pass and everything else collides (Sly 2's rule, interactables.md 6) [R]. Each
//!   object touching a barrier is recorded (up to 16).
//! - **Use points** (class 33, Sly 2 class 35): the use radius (1163), the command script started on use (1164),
//!   up to 4 (task, state) pairs (1170, a load-time method) and 1168, 1175, 1178 (meanings open). The behaviour
//!   tracks which playable character is within the radius; [`use_point_used`] is what using the point does.
//! - **Treasure maps** (class 153) and the **compass** (class 125): data only ([`TreasureMap`], [`Compass`]).
//!
//! List options (512, 510, 1170, 1905) are typed lists, which the Sly 3 loader keeps as the level's actions rather
//! than as stored values; [`lists`] reads them.

use alloc::vec::Vec;

use crate::behavior::sly2_sensors::characters;
use crate::behavior::{ClassBehavior, Cx};
use crate::object::{Arg, Handle};
use crate::world::{Notice, World};

/// Sly 3 classes handled here.
pub mod class {
    /// Barriers.
    pub const BARRIER: i16 = 97;
    /// Use points.
    pub const USE_POINT: i16 = 33;
    /// The compass.
    pub const COMPASS: i16 = 125;
    /// Treasure maps.
    pub const TREASURE_MAP: i16 = 153;
}

/// Option IDs.
pub mod opt {
    /// Barrier: listed objects (Sly 3 IDs).
    pub const BARRIER_OBJECTS: u16 = 510;
    /// Barrier: listed classes (Sly 2 456).
    pub const BARRIER_CLASSES: u16 = 512;
    /// Barrier: mode (Sly 2 458).
    pub const BARRIER_MODE: u16 = 513;
    /// Use point: the use radius (Sly 2 1069).
    pub const USE_RADIUS: u16 = 1163;
    /// Use point: the command script started when the point is used (Sly 2 1070).
    pub const USE_SCRIPT: u16 = 1164;
    /// Use point: 3,300 to 3,500 or a short (Sly 2 1074; meaning open).
    pub const USE_1168: u16 = 1168;
    /// Use point: up to 4 (task, state) pairs applied when the point is used, −1 meaning "make current" (Sly 2 1076).
    pub const USE_TASKS: u16 = 1170;
    /// Treasure map: the overhead view's height (5,000; 4,000 in row 31).
    pub const MAP_VIEW_HEIGHT: u16 = 1901;
    /// Treasure map: its screen.
    pub const MAP_SCREEN: u16 = 1902;
    /// Treasure map: the current marker's arrival radius (400 on the disc; default 500).
    pub const MAP_ARRIVAL: u16 = 1903;
    /// Treasure map: the trail markers (class-3 placed markers), in order.
    pub const MAP_MARKER: u16 = 1904;
    /// Treasure map: the clues, (riddle, plain hint) string IDs, −1 none.
    pub const MAP_STRINGS: u16 = 1905;
    /// Treasure map: its command script.
    pub const MAP_SCRIPT: u16 = 1906;
    /// Compass: the needle.
    pub const COMPASS_NEEDLE: u16 = 1894;
}

/// The most contacts a barrier records.
pub const MAX_CONTACTS: usize = 16;

/// The typed lists of option `id` on `h` (each list's arguments), in stream order.
pub fn lists<'a>(world: &'a World, h: Handle, id: u16) -> impl Iterator<Item = &'a [Arg]> + 'a {
    let slot = world.slot(h);
    world.sly3.as_deref().into_iter().flat_map(move |l| {
        l.actions.iter().filter(move |(s, i, _)| Some(*s) == slot && *i == id).map(|(_, _, a)| &a[..])
    })
}

/// An argument as an integer (an `Int`, the value of a `Short`, a `Bool`).
pub fn arg_i32(a: &Arg) -> Option<i32> {
    match *a {
        Arg::Int(n) => Some(n),
        Arg::Short(_, n) => Some(i32::from(n)),
        Arg::Bool(b) => Some(i32::from(b)),
        _ => None,
    }
}

/// The box of a Sly 3 solid's collision points, in the solid's frame (Sly 3 water's sensor parts; the Sly 2 water
/// behaviour's `water_box` for Sly 3 worlds).
pub fn solid_box(world: &World, h: Handle) -> Option<(formats::geometry::Affine, [f32; 3], [f32; 3])> {
    let r = world.sly3_record(h)?;
    let formats_sly3::stream::Body::Solid(s) = &r.body else { return None };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for part in &s.collision {
        for p in &part.mesh.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    Some((world.world_transform(h)?, lo, hi))
}

/// A Sly 3 barrier (class 97).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Barrier {
    /// Listed classes (512).
    pub classes: Vec<i16>,
    /// Listed objects, Sly 3 IDs (510).
    pub objects: Vec<i32>,
    /// Mode (513): 0 the listed objects collide, 1 they pass.
    pub mode: u8,
    /// Objects touching it, latest last (at most [`MAX_CONTACTS`]).
    pub contacts: Vec<Handle>,
}

impl Barrier {
    /// From the barrier's options.
    pub fn new(world: &World, h: Handle) -> Barrier {
        let ints = |id| lists(world, h, id).flat_map(|a| a.iter().filter_map(arg_i32)).collect::<Vec<i32>>();
        Barrier {
            classes: ints(opt::BARRIER_CLASSES).into_iter().map(|c| c as i16).collect(),
            objects: ints(opt::BARRIER_OBJECTS),
            mode: world.options(h).and_then(|o| o.i32(opt::BARRIER_MODE)).unwrap_or(0) as u8,
            contacts: Vec::new(),
        }
    }

    /// Whether the barrier collides with `h`.
    pub fn blocks(&self, world: &World, h: Handle) -> bool {
        let class = world.class(h).unwrap_or(-1);
        let listed = self.classes.contains(&class) || world.sly3_id(h).is_some_and(|id| self.objects.contains(&id));
        if self.mode == 0 { listed } else { !listed }
    }
}

/// Whether barrier `me` collides with `h` (for collision queries; reads the options each call).
pub fn barrier_blocks(world: &World, me: Handle, h: Handle) -> bool {
    Barrier::new(world, me).blocks(world, h)
}

impl ClassBehavior for Barrier {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        for n in cx.world.take_inbox(cx.me.index()) {
            let Notice::Touched(h) = n else { continue };
            self.contacts.retain(|&c| c != h);
            self.contacts.push(h);
            if self.contacts.len() > MAX_CONTACTS {
                self.contacts.remove(0);
            }
        }
    }
}

/// The use radius of a point without option 1163 (138235, `t1_recon_go_to_key`'s point, has none). Working choice:
/// the specs give no default; 100 is the middle of the disc's values (50, 100, 150).
pub const USE_RADIUS_DEFAULT: f32 = 100.0;

/// A Sly 3 use point (class 33).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsePoint {
    /// Use radius (1163).
    pub radius: f32,
    /// The command script started on use (1164).
    pub script: Option<i32>,
    /// The (task, state) pairs applied on use (1170), at most 4.
    pub tasks: Vec<(i32, i32)>,
    /// The playable character within the radius, if any (the nearest).
    pub user: Option<Handle>,
}

impl UsePoint {
    /// From the use point's options.
    pub fn new(world: &World, h: Handle) -> UsePoint {
        let o = world.options(h);
        UsePoint {
            radius: o.and_then(|o| o.f32(opt::USE_RADIUS)).unwrap_or(USE_RADIUS_DEFAULT),
            script: o.and_then(|o| o.i32(opt::USE_SCRIPT)).filter(|&s| s >= 0),
            tasks: lists(world, h, opt::USE_TASKS)
                .map(|a| {
                    let n = |k: usize| a.get(k).and_then(arg_i32).unwrap_or(-1);
                    (n(0), n(1))
                })
                .take(4)
                .collect(),
            user: None,
        }
    }

    /// The nearest playable character within the radius of the use point's position.
    pub fn user_of(&self, world: &World, me: Handle) -> Option<Handle> {
        let p = world.position(me)?;
        let r2 = self.radius * self.radius;
        characters(world)
            .into_iter()
            .filter_map(|(h, _)| {
                let q = world.position(h)?;
                let d = crate::math::sub(q, p);
                Some((d[0] * d[0] + d[1] * d[1] + d[2] * d[2], h))
            })
            .filter(|&(d2, _)| d2 <= r2)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }
}

impl ClassBehavior for UsePoint {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.user = self.user_of(cx.world, cx.me);
    }
}

/// Sly 3 hook (use button): player 0's character uses point `me` if it is within the radius. In order: the user (the
/// one the behaviour tracks), each (task, state) pair goes to the job system as a checked request (−1 making the task current), event 5
/// is raised on the point, and its command script (1164) is returned for the caller to start. None when the point is
/// out of reach.
pub fn use_point_used(world: &mut World, me: Handle) -> Option<Option<i32>> {
    let p = UsePoint::new(world, me);
    let player = world.player?;
    p.user_of(world, me).filter(|&u| u == player)?;
    world.shared.task_requests.extend(p.tasks.iter().copied());
    world.raise(me, 5, Vec::new());
    Some(p.script)
}

/// One pace of a treasure-map clue, in units (gadgets.md 6.10).
pub const MAP_PACE: f32 = 200.0;
/// The arrival radius when option 1903 is not set.
pub const MAP_ARRIVAL_DEFAULT: f32 = 500.0;
/// The overhead view's height when option 1901 is not set.
pub const MAP_VIEW_DEFAULT: f32 = 5000.0;
/// The overhead view's pan easing (fast, slow, fast time) (gadgets.md 6.10.1).
pub const MAP_PAN_EASE: [f32; 3] = [1500.0, 700.0, 0.5];

/// A treasure map (class 153; gadgets.md 6.10, 6.10.1) [D for the values].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreasureMap {
    /// The overhead view's height (1901; 5,000, 4,000 in row 31).
    pub view_height: f32,
    /// Its screen (1902, a class-158 screen object).
    pub screen: i32,
    /// The current marker's arrival radius (1903; 400 on the disc).
    pub arrival_radius: f32,
    /// The trail markers in order (1904), Sly 3 IDs; the last ends the trail.
    pub markers: Vec<i32>,
    /// The clues (1905): (riddle, plain hint) string IDs, −1 none.
    pub strings: Vec<(i32, i32)>,
    /// Its command script (1906).
    pub script: i32,
}

impl TreasureMap {
    /// From the map's options.
    pub fn new(world: &World, h: Handle) -> TreasureMap {
        let o = world.options(h);
        TreasureMap {
            view_height: o.and_then(|o| o.f32(opt::MAP_VIEW_HEIGHT)).unwrap_or(MAP_VIEW_DEFAULT),
            screen: o.and_then(|o| o.i32(opt::MAP_SCREEN)).unwrap_or(-1),
            arrival_radius: o.and_then(|o| o.f32(opt::MAP_ARRIVAL)).unwrap_or(MAP_ARRIVAL_DEFAULT),
            script: o.and_then(|o| o.i32(opt::MAP_SCRIPT)).unwrap_or(-1),
            markers: o.map(|o| o.all(opt::MAP_MARKER).filter_map(|v| v.as_i32()).collect()).unwrap_or_default(),
            strings: lists(world, h, opt::MAP_STRINGS)
                .map(|a| {
                    let n = |k: usize| a.get(k).and_then(arg_i32).unwrap_or(-1);
                    (n(0), n(1))
                })
                .collect(),
        }
    }
}

impl TreasureMap {
    /// Whether the player at `player` has reached the current marker at `marker`: the full 3-D distance is within
    /// the arrival radius (6.10.1).
    pub fn reached(&self, marker: [f32; 3], player: [f32; 3]) -> bool {
        crate::math::length(crate::math::sub(marker, player)) <= self.arrival_radius
    }

    /// One step of the overhead map view (6.10.1): the pan eases with (1,500, 700, 0.5) toward the stick's
    /// direction × half its magnitude × the view height; the camera sits the view height above the player plus
    /// the pan, looking straight down. `stick` is world-relative, magnitude 0 to 1. Returns the eye.
    pub fn view(&self, pan: &mut [f32; 2], player: [f32; 3], stick: [f32; 2], dt: f32) -> [f32; 3] {
        let goal = [stick[0] * 0.5 * self.view_height, stick[1] * 0.5 * self.view_height];
        let [fast, slow, fast_time] = MAP_PAN_EASE;
        let p = crate::physics::SmoothParams { fast, slow, fast_time, max_speed_change: None };
        let d = [goal[0] - pan[0], goal[1] - pan[1]];
        let l = crate::math::sqrt(d[0] * d[0] + d[1] * d[1]);
        if l > 0.0 {
            let step = (p.speed_at(l) * dt).min(l);
            pan[0] += d[0] / l * step;
            pan[1] += d[1] / l * step;
        }
        [player[0] + pan[0], player[1] + pan[1], player[2] + self.view_height]
    }
}

/// The compass (class 125).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Compass {
    /// The needle (1894), a Sly 3 ID.
    pub needle: i32,
}

impl Compass {
    /// From the compass's options.
    pub fn new(world: &World, h: Handle) -> Compass {
        Compass { needle: world.options(h).and_then(|o| o.i32(opt::COMPASS_NEEDLE)).unwrap_or(-1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treasure_map_arrival_and_view() {
        let m = TreasureMap { view_height: 5000.0, arrival_radius: 400.0, ..TreasureMap::default() };
        // The full 3-D distance counts: 300 across and 300 up is 424, not reached.
        assert!(m.reached([0.0; 3], [300.0, 0.0, 0.0]));
        assert!(!m.reached([0.0; 3], [300.0, 0.0, 300.0]));
        // The view pans toward stick × ½ × 5,000 and sits 5,000 above the player plus the pan.
        let mut pan = [0.0; 2];
        let mut eye = [0.0; 3];
        for _ in 0..600 {
            eye = m.view(&mut pan, [100.0, 0.0, 50.0], [1.0, 0.0], 1.0 / 60.0);
        }
        assert!((pan[0] - 2500.0).abs() < 1.0 && pan[1] == 0.0, "{pan:?}");
        assert_eq!(eye, [100.0 + pan[0], 0.0, 5050.0]);
        assert_eq!((MAP_PACE, crate::gang::character::MAP_DIRECTIONS), (200.0, 8));
    }
}
