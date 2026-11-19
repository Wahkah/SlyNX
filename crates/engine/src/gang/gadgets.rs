//! Gadgets and special moves (specs/sly2/gadgets.md): what each item does, when it is allowed and when it pays,
//! the carryable objects (pocket items and Bentley's thrown bombs), blasts, the Health Extractor's capture,
//! medicine, and the Select assignment screen (safehouse-and-shop.md 6).
//!
//! The character side (pressing, refusing, paying, draining, the moves themselves) is in [`super::character`]; the
//! objects the items make (smoke, bombs, the extractor) are the object model's, driven with the types here.

use alloc::vec::Vec;

use formats_sly2::exe::Executable;

use super::Who;
use super::items::{ItemTable, Owned, Slots};
use crate::math::{Vec3, cos, length, normalize, scale, sin};

/// The executable values of gadgets.md 7 and 8 (and pickpocket-and-pickups.md 4 for medicine).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GadgetConstants {
    /// Contacts are ignored this long after a carryable object changes to state 0 (0.25 s) [D 0x2C42E0].
    pub contact_ignore: f32,
    /// Blast: the push scale (100) [D 0x2C4330] and the impulse and speed limit along a ray (1000) [D 0x2C4334].
    pub blast: [f32; 2],
    /// Health Extractor: capture time (3 s) [D 0x2C43B0].
    pub extract_time: f32,
    /// Health Extractor: the captured guard's velocity factor and upward speed (0.5, 400) [D 0x2C43F0, 0x2C43F4].
    pub extract_launch: [f32; 2],
    /// Health Extractor: position and velocity spring records ((0, 15, 0), (0, 2.5, 0)) [D 0x2C43D0, 0x2C43E0].
    pub extract_springs: [Vec3; 2],
}

impl GadgetConstants {
    /// The spec's values.
    pub const SPEC: GadgetConstants = GadgetConstants {
        contact_ignore: 0.25,
        blast: [100.0, 1000.0],
        extract_time: 3.0,
        extract_launch: [0.5, 400.0],
        extract_springs: [[0.0, 15.0, 0.0], [0.0, 2.5, 0.0]],
    };

    /// Reads them from the player's executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<GadgetConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let v = |va: u32| -> formats::Result<Vec3> { Ok([f(va)?, f(va + 4)?, f(va + 8)?]) };
        Ok(GadgetConstants {
            contact_ignore: f(0x002C_42E0)?,
            blast: [f(0x002C_4330)?, f(0x002C_4334)?],
            extract_time: f(0x002C_43B0)?,
            extract_launch: [f(0x002C_43F0)?, f(0x002C_43F4)?],
            extract_springs: [v(0x002C_43D0)?, v(0x002C_43E0)?],
        })
    }
}

/// When an item's cost is paid (gadgets.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayAt {
    /// When it starts.
    Start,
    /// At its animation's activation event.
    Event,
    /// When it is thrown (pocket items).
    Throw,
}

/// What an item does, in the terms the character and the object model act on (gadgets.md 4 and 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Runs while the button is held, draining power (25, 29, 40, 42; 12 the Hover Pack).
    Held,
    /// A pocket item: taken from a pool, aimed and thrown, paying on the throw (26, 37, 38, 41).
    Pocket,
    /// Taken from a pool, aimed and thrown; stays out until the next press triggers it (7, 11).
    ThrownTrigger,
    /// Summoned and held, then placed or thrown (9, 13).
    Summoned,
    /// An animation whose activation event applies an overlay; using it again removes the overlay (15, 34, 35).
    Overlay,
    /// An animation whose activation event pays and acts on the world (8, 14, 21, 23).
    Activation,
    /// A jump of the given kind (30: 14, 31: 9, 12: 17).
    Jump(u8),
    /// A move: a state of the character (10, 16, 19, 20, 22, 24, 33, 39).
    Move,
    /// Works whenever owned, without a button (17, 18, 27, 28, 32, 36).
    Passive,
    /// Not an item (the empty records).
    None,
}

/// What an activated item does to the world, for the object model (gadgets.md 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldEffect {
    /// Smoke Bomb: smoke placed at the character; every alarm the player set off is cleared and guards cannot raise
    /// a new one for `resume` seconds (2).
    Smoke {
        /// Seconds.
        resume: f32,
    },
    /// Temporal Lock: every guard in the world frozen for `duration` seconds (6).
    Freeze {
        /// Seconds.
        duration: f32,
    },
    /// Guttural Roar: every guard in the world switches to a frightened mode.
    Frighten,
    /// Size Destabilizer: a guard hit by the crossbow shot is shrunk (its hits do half damage).
    Shrink,
}

/// Item effects and rules.
pub mod effect {
    use super::{Effect, PayAt, WorldEffect};

    /// The effect of item `n`.
    pub fn of(n: u8) -> Effect {
        match n {
            12 | 25 | 29 | 40 | 42 => Effect::Held,
            26 | 37 | 38 | 41 => Effect::Pocket,
            7 | 11 => Effect::ThrownTrigger,
            9 | 13 => Effect::Summoned,
            15 | 34 | 35 => Effect::Overlay,
            8 | 14 | 21 | 23 => Effect::Activation,
            30 => Effect::Jump(14),
            31 => Effect::Jump(9),
            10 | 16 | 19 | 20 | 22 | 24 | 33 | 39 => Effect::Move,
            17 | 18 | 27 | 28 | 32 | 36 => Effect::Passive,
            _ => Effect::None,
        }
    }

    /// When item `n` pays (gadgets.md 3): at the activation event for the items that play an animation first, on
    /// the throw for the pocket items, else at the start.
    pub fn pay_at(n: u8) -> PayAt {
        match n {
            23 | 14 | 21 | 15 | 34 | 35 | 8 => PayAt::Event,
            26 | 37 | 38 | 41 => PayAt::Throw,
            _ => PayAt::Start,
        }
    }

    /// The world effect an activated item has, with the executable's times (`smoke_resume`, `freeze`).
    pub fn world(n: u8, smoke_resume: f32, freeze: f32) -> Option<WorldEffect> {
        match n {
            23 => Some(WorldEffect::Smoke { resume: smoke_resume }),
            14 => Some(WorldEffect::Freeze { duration: freeze }),
            21 => Some(WorldEffect::Frighten),
            8 => Some(WorldEffect::Shrink),
            _ => None,
        }
    }

    /// Whether item `n` is taken from a pool of carryable copies.
    pub fn pooled(n: u8) -> bool {
        matches!(n, 7 | 11 | 26 | 37 | 38 | 41)
    }
}

/// A use is refused when its pool has fewer than this many free copies (gadgets.md 6).
pub const POOL_MIN_FREE: u32 = 2;

/// Paraglide is not allowed within this long of landing (gadgets.md 5).
pub const PARAGLIDE_LANDING: f32 = 0.3;

/// Why an item was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The character cannot use it in its current state (refusal sound 46).
    State,
    /// Not enough power (refusal sound 46 and the power meter flashes).
    Power,
    /// Its pool has fewer than 2 free copies.
    Pool,
}

/// A carryable object's state (gadgets.md 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryState {
    /// 0: held by its carrier.
    Held,
    /// 1: flying.
    Flying,
    /// 2: landed and fixed in place.
    Landed,
    /// 3: used.
    Used,
    /// 4: fading out, then back to the pool.
    Fading,
    /// 5: the Health Extractor at work.
    Extracting,
}

impl CarryState {
    /// The state's number.
    pub const fn code(self) -> u8 {
        match self {
            CarryState::Held => 0,
            CarryState::Flying => 1,
            CarryState::Landed => 2,
            CarryState::Used => 3,
            CarryState::Fading => 4,
            CarryState::Extracting => 5,
        }
    }

    /// The goal sent to the object's own state machine on entering the state (109 + state).
    pub const fn goal(self) -> i32 {
        109 + self.code() as i32
    }
}

/// A carryable gadget object: a pocket item or one of Bentley's thrown items (gadgets.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carryable {
    /// The item number it was given as its kind.
    pub item: u8,
    /// State.
    pub state: CarryState,
    /// Time in the state.
    pub time: f32,
    /// Kept out (a planted Trigger Bomb): the next use reuses it instead of taking a new copy.
    pub kept: bool,
}

/// How long a used carryable fades before it returns to its pool (gadgets.md 6).
pub const FADE_TIME: f32 = 0.25;

impl Carryable {
    /// A copy taken from its pool, held by its carrier.
    pub fn take(item: u8) -> Carryable {
        Carryable { item, state: CarryState::Held, time: 0.0, kept: false }
    }

    fn set(&mut self, s: CarryState, goals: &mut Vec<i32>) {
        self.state = s;
        self.time = 0.0;
        goals.push(s.goal());
    }

    /// Thrown.
    pub fn throw(&mut self, goals: &mut Vec<i32>) {
        self.set(CarryState::Flying, goals);
    }

    /// Landed. Alarm Clock (26) and TOM (41) are used at once; the others stay fixed in place (the Trigger Bomb
    /// and the Health Extractor are kept out until triggered).
    pub fn land(&mut self, goals: &mut Vec<i32>) {
        if matches!(self.item, 26 | 41) {
            self.set(CarryState::Used, goals);
        } else {
            self.kept = matches!(self.item, 7 | 11);
            self.set(CarryState::Landed, goals);
        }
    }

    /// The trigger press: a Trigger Bomb is used (the blast is the object model's), a Health Extractor starts
    /// extracting.
    pub fn trigger(&mut self, goals: &mut Vec<i32>) {
        self.kept = false;
        match self.item {
            11 => self.set(CarryState::Extracting, goals),
            _ => self.set(CarryState::Used, goals),
        }
    }

    /// The extraction ended: with no guard found or once done, used; a capture that ended early lands it again.
    pub fn end_extraction(&mut self, early: bool, goals: &mut Vec<i32>) {
        self.set(if early { CarryState::Landed } else { CarryState::Used }, goals);
    }

    /// Whether contacts are ignored now (the first 0.25 s after changing to state 0).
    pub fn ignores_contacts(&self, c: &GadgetConstants) -> bool {
        self.state == CarryState::Held && self.time < c.contact_ignore
    }

    /// Ages the object: used objects fade, faded ones return to the pool (returns true then).
    pub fn step(&mut self, dt: f32, goals: &mut Vec<i32>) -> bool {
        self.time += dt;
        match self.state {
            CarryState::Used => {
                self.set(CarryState::Fading, goals);
                false
            }
            CarryState::Fading => self.time >= FADE_TIME,
            _ => false,
        }
    }
}

/// A bomb's blast as `bentley_mine`'s kind option selects it (gadgets.md 7.1) [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlastSpec {
    /// Radius r.
    pub radius: u32,
    /// The filter passed to what it hits (option 1107).
    pub filter: u32,
    /// The state code passed to what it hits (option 1108).
    pub state_code: i32,
    /// Effect object.
    pub effect: i32,
    /// Sound, if any.
    pub sound: Option<i32>,
}

/// The blast of a bomb item: Trigger Bomb (7) and Snooze Bomb (9) radius 300, Reduction Bomb (13) and Rage Bomb
/// (37) radius 600.
pub fn blast_of(item: u8) -> Option<BlastSpec> {
    Some(match item {
        5 | 7 => BlastSpec { radius: 300, filter: 256, state_code: -20, effect: 3995, sound: Some(149) },
        9 => BlastSpec { radius: 300, filter: 524_288, state_code: -60, effect: 3978, sound: None },
        13 => BlastSpec { radius: 600, filter: 16_777_216, state_code: -5, effect: 3977, sound: None },
        37 => BlastSpec { radius: 600, filter: 33_554_432, state_code: -5, effect: 3970, sound: None },
        _ => return None,
    })
}

/// The blast's ring elevations (radians) and ray counts (gadgets.md 7.1).
pub const BLAST_RINGS: [(f32, u32); 11] = [
    (-1.5, 3),
    (-1.2, 7),
    (-0.9, 9),
    (-0.6, 10),
    (-0.3, 11),
    (0.0, 12),
    (0.3, 11),
    (0.6, 10),
    (0.9, 9),
    (1.2, 7),
    (1.5, 3),
];
/// At most this many solids are blast candidates, and at most this many hits are recorded.
pub const BLAST_LIMITS: [usize; 2] = [256, 32];

/// The blast's fixed ray directions (unit vectors, 92 of them): 11 rings, evenly spaced in heading.
pub fn blast_rays() -> Vec<Vec3> {
    let mut out = Vec::new();
    for (e, n) in BLAST_RINGS {
        let (se, ce) = (sin(e), cos(e));
        for k in 0..n {
            let h = core::f32::consts::TAU * k as f32 / n as f32;
            out.push([ce * cos(h), ce * sin(h), se]);
        }
    }
    out
}

/// The push a blast gives a body along a ray (gadgets.md 7.1): with u = the hit point's speed along the ray / 1000,
/// a velocity change of `(1 − u²) × 100`, limited to 0..1 before scaling, and never past a speed of 1000 along the
/// ray. Returns the change of speed along the ray.
pub fn blast_push(c: &GadgetConstants, speed_along: f32) -> f32 {
    let [push, limit] = c.blast;
    let u = speed_along / limit;
    let dv = (1.0 - u * u).clamp(0.0, 1.0) * push;
    dv.min((limit - speed_along).max(0.0))
}

/// The Health Extractor's capture of a guard (gadgets.md 7.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capture {
    /// Time since the capture.
    pub time: f32,
}

/// The extractor looks for a guard within this distance of its source.
pub const EXTRACT_RADIUS: f32 = 400.0;
/// Within this distance of the source the captured guard takes its held pose.
pub const EXTRACT_HOLD: f32 = 100.0;

impl Capture {
    /// The guard's velocity at the capture: half its velocity plus 400 up.
    pub fn launch(c: &GadgetConstants, v: Vec3) -> Vec3 {
        let [k, up] = c.extract_launch;
        [v[0] * k, v[1] * k, v[2] * k + up]
    }

    /// The pull toward the source (gravity cancelled): a spring of 15 on the position and 2.5 on the velocity.
    pub fn pull(c: &GadgetConstants, position: Vec3, velocity: Vec3, source: Vec3) -> Vec3 {
        let (k, d) = (c.extract_springs[0][1], c.extract_springs[1][1]);
        let e = crate::math::sub(source, position);
        crate::math::madd(scale(e, k), velocity, -d)
    }

    /// The guard's scale: `1.2 − u²` limited to 0..1, u = time / 3 s.
    pub fn scale(&self, c: &GadgetConstants) -> f32 {
        let u = self.time / c.extract_time;
        (1.2 - u * u).clamp(0.0, 1.0)
    }

    /// Ages the capture; true once it is done (a health pickup appears at the source, the guard is removed and the
    /// mission system receives event 24).
    pub fn step(&mut self, c: &GadgetConstants, dt: f32) -> bool {
        self.time += dt;
        self.time >= c.extract_time
    }
}

/// TOM's target: the nearest of up to 32 guards, with the height difference counted twice (gadgets.md 4.1).
pub fn tom_target(from: Vec3, guards: &[Vec3]) -> Option<usize> {
    guards
        .iter()
        .take(32)
        .enumerate()
        .map(|(i, g)| {
            let d = crate::math::sub(*g, from);
            (i, length([d[0], d[1], 2.0 * d[2]]))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Medicine (pickpocket-and-pickups.md 4): 10 health and 20 power, each up to its maximum; refused when both are
/// full. Returns whether it was taken.
pub fn take_medicine(health: &mut super::health::Health, power: &mut super::items::Power) -> bool {
    if health.value >= health.capacity && power.value >= power.max {
        return false;
    }
    health.value = (health.value + MEDICINE[0]).min(health.capacity);
    power.add(MEDICINE[1]);
    true
}

/// Medicine's health and power amounts.
pub const MEDICINE: [f32; 2] = [10.0, 20.0];

/// One cell of a character's gadget grid (safehouse-and-shop.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridCell {
    /// Item record.
    pub item: i32,
    /// Screen parts: open, owned, selected.
    pub parts: [i32; 3],
    /// Position on the screen.
    pub position: [f32; 2],
    /// Flags (bit 0: may be put on a button).
    pub flags: i32,
}

impl GridCell {
    /// Whether the item may be put on a button.
    pub fn assignable(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// The grid addresses and cell counts: Sly, Bentley, Murray.
pub const GRIDS: [(Who, u32, usize); 3] =
    [(Who::Sly, 0x002C_2DE8, 18), (Who::Bentley, 0x002C_3028, 9), (Who::Murray, 0x002C_3148, 8)];

/// Reads a character's grid from the executable after its SHA-256 check.
pub fn grid_from_executable(exe: &Executable, who: Who) -> formats::Result<Vec<GridCell>> {
    if !exe.is_supported() {
        return Err(formats::Error::UnknownExecutable);
    }
    let (_, va, n) = GRIDS.iter().copied().find(|g| g.0 == who).unwrap_or((who, 0, 0));
    let raw = exe.bytes_at(va, n * 32)?;
    Ok(raw
        .as_chunks::<32>()
        .0
        .iter()
        .map(|c| {
            let i = |k: usize| i32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]]);
            let f = |k: usize| f32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]]);
            GridCell { item: i(0), parts: [i(1), i(2), i(3)], position: [f(5), f(6)], flags: i(7) }
        })
        .collect())
}

/// What the assignment screen said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignMessage {
    /// The item was put on the slot.
    Assigned,
    /// Not owned.
    NotOwned,
    /// Not assignable (button use −1).
    NotAssignable,
}

/// The Select assignment screen (safehouse-and-shop.md 6): the current character's grid and the selected cell. It
/// pauses the game while open.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    /// The character.
    pub who: Who,
    /// The grid.
    pub cells: Vec<GridCell>,
    /// The selected cell.
    pub selected: usize,
}

impl Assignment {
    /// Opens the screen on `who`'s grid, with the first owned cell selected.
    pub fn open(who: Who, cells: Vec<GridCell>, owned: &Owned) -> Assignment {
        let selected = cells.iter().position(|c| owned.has(c.item as u8)).unwrap_or(0);
        Assignment { who, cells, selected }
    }

    /// Moves the selection in direction `dir` (screen units, any length): to the nearest owned cell within 750 along
    /// the direction and 40 across it, else the nearest within 200 across.
    pub fn step(&mut self, dir: [f32; 2], owned: &Owned) {
        let l = crate::math::sqrt(dir[0] * dir[0] + dir[1] * dir[1]);
        if l <= 0.0 {
            return;
        }
        let d = [dir[0] / l, dir[1] / l];
        let here = self.cells[self.selected].position;
        let pick = |across_max: f32| {
            let mut best: Option<(f32, usize)> = None;
            for (i, c) in self.cells.iter().enumerate() {
                if i == self.selected || !owned.has(c.item as u8) {
                    continue;
                }
                let o = [c.position[0] - here[0], c.position[1] - here[1]];
                let along = o[0] * d[0] + o[1] * d[1];
                let across = (o[0] * d[1] - o[1] * d[0]).abs();
                if along <= 0.0 || along > 750.0 || across > across_max {
                    continue;
                }
                let dist = along + across;
                if best.is_none_or(|(b, _)| dist < b) {
                    best = Some((dist, i));
                }
            }
            best.map(|b| b.1)
        };
        if let Some(i) = pick(40.0).or_else(|| pick(200.0)) {
            self.selected = i;
        }
    }

    /// L1, L2 or R2 (`slot` 0, 1, 2): puts the selected item on that slot.
    pub fn assign(&self, slot: usize, slots: &mut Slots, table: &ItemTable, owned: &Owned) -> AssignMessage {
        let cell = self.cells[self.selected];
        let item = cell.item as u8;
        if !owned.has(item) {
            return AssignMessage::NotOwned;
        }
        if !cell.assignable() || !slots.assign(slot, item, self.who, table, owned) {
            return AssignMessage::NotAssignable;
        }
        AssignMessage::Assigned
    }
}

/// The direction of a thrown carryable's flight at launch (unit), for callers that need it.
pub fn launch_direction(velocity: Vec3) -> Vec3 {
    normalize(velocity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_pay_times_and_carryables() {
        assert_eq!(effect::of(23), Effect::Activation);
        assert_eq!(effect::pay_at(23), PayAt::Event);
        assert_eq!(effect::pay_at(41), PayAt::Throw);
        assert_eq!(effect::pay_at(29), PayAt::Start);
        assert_eq!(effect::of(31), Effect::Jump(9));
        assert_eq!(effect::of(36), Effect::Passive);
        assert_eq!(effect::world(14, 2.0, 6.0), Some(WorldEffect::Freeze { duration: 6.0 }));
        let mut goals = Vec::new();
        let c = GadgetConstants::SPEC;
        let mut t = Carryable::take(7);
        assert!(t.ignores_contacts(&c));
        t.step(0.3, &mut goals);
        assert!(!t.ignores_contacts(&c));
        t.throw(&mut goals);
        t.land(&mut goals);
        assert_eq!((t.state, t.kept), (CarryState::Landed, true));
        t.trigger(&mut goals);
        assert_eq!(t.state, CarryState::Used);
        assert!(!t.step(0.01, &mut goals));
        assert_eq!(t.state, CarryState::Fading);
        assert!(t.step(0.25, &mut goals));
        assert_eq!(goals, [110, 111, 112, 113]);
        let mut clock = Carryable::take(26);
        clock.throw(&mut goals);
        clock.land(&mut goals);
        assert_eq!(clock.state, CarryState::Used, "the Alarm Clock is used on landing");
    }

    #[test]
    fn blasts_extractor_tom_and_medicine() {
        // gadgets.md 7.1: the bombs' radii and codes.
        assert_eq!(blast_of(7).map(|b| (b.radius, b.state_code)), Some((300, -20)));
        assert_eq!(blast_of(9).map(|b| (b.radius, b.state_code)), Some((300, -60)));
        assert_eq!(blast_of(13).map(|b| b.radius), Some(600));
        assert_eq!(blast_of(37).map(|b| (b.radius, b.filter)), Some((600, 33_554_432)));
        assert_eq!(blast_of(11), None);
        let rays = blast_rays();
        assert_eq!(rays.len(), 92);
        assert!(rays.iter().all(|r| (length(*r) - 1.0).abs() < 1e-5));
        let c = GadgetConstants::SPEC;
        assert_eq!(blast_push(&c, 0.0), 100.0);
        assert_eq!(blast_push(&c, 500.0), 75.0);
        assert!((blast_push(&c, 950.0) - 9.75).abs() < 1e-3);
        assert_eq!(blast_push(&c, 1200.0), 0.0);
        assert_eq!(Capture::launch(&c, [100.0, 0.0, -50.0]), [50.0, 0.0, 375.0]);
        let mut cap = Capture { time: 0.0 };
        assert_eq!(cap.scale(&c), 1.0);
        assert!(!cap.step(&c, 2.9));
        assert!((cap.scale(&c) - (1.2 - (2.9f32 / 3.0).powi(2))).abs() < 1e-6);
        assert!(cap.step(&c, 0.2));
        let p = Capture::pull(&c, [0.0; 3], [10.0, 0.0, 0.0], [100.0, 0.0, 0.0]);
        assert_eq!(p, [1475.0, 0.0, 0.0]);
        // TOM counts the height difference twice.
        assert_eq!(tom_target([0.0; 3], &[[0.0, 0.0, 100.0], [150.0, 0.0, 0.0]]), Some(1));
        let mut h = super::super::health::Health { value: 35.0, ..Default::default() };
        let mut p = super::super::items::Power::new(100.0, 100.0);
        assert!(take_medicine(&mut h, &mut p));
        assert_eq!((h.value, p.value), (40.0, 100.0));
        assert!(!take_medicine(&mut h, &mut p), "both full");
    }

    #[test]
    fn assignment_screen() {
        let cell = |item, x, y, flags| GridCell { item, parts: [0; 3], position: [x, y], flags };
        let cells = alloc::vec![cell(23, 145.0, 155.0, 1), cell(29, 245.0, 160.0, 1), cell(36, 145.0, 255.0, 0)];
        let mut owned = Owned::default();
        for n in [23, 29, 36] {
            owned.add(n);
        }
        let mut raw = alloc::vec![0u8; 43 * 32];
        for (n, rec) in
            [(23usize, [2i32, 7, 0, 0, 0, 0, 0, 0]), (29, [1, 7, 0, 2, 0, 0, 0, 0]), (36, [-1, 7, 0, 0, 0, 0, 0, 0])]
        {
            for (i, v) in rec.iter().enumerate() {
                raw[n * 32 + i * 4..n * 32 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        let table = ItemTable::from_bytes(&raw);
        let mut a = Assignment::open(Who::Sly, cells, &owned);
        let mut slots = Slots::default();
        assert_eq!(a.assign(0, &mut slots, &table, &owned), AssignMessage::Assigned);
        a.step([1.0, 0.0], &owned);
        assert_eq!(a.selected, 1);
        assert_eq!(a.assign(0, &mut slots, &table, &owned), AssignMessage::Assigned);
        assert_eq!(slots.0, [Some(29), None, None]);
        a.step([-1.0, 1.0], &owned);
        assert_eq!(a.selected, 2);
        assert_eq!(a.assign(1, &mut slots, &table, &owned), AssignMessage::NotAssignable);
        owned.remove(29);
        a.selected = 1;
        assert_eq!(a.assign(2, &mut slots, &table, &owned), AssignMessage::NotOwned);
    }
}
