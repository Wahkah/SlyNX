//! Sly 3 devices with rules of their own: bouncers (class 94, alarms-sensors.md 5), the vault tumbler (class 121,
//! pickpocket-and-pickups.md 6) and the noise templates (class 139, guards.md 4).
//!
//! - **Bouncers** derive from the contact trigger (93). A contact puts the bouncer in state 1; more than 0.05 s
//!   later it goes to state 2 and launches what touched it, then back to 0 on the next update. The target point is
//!   option 496's object, else the bouncer's local point (0, 0, 300). The launch is the ballistic arc of
//!   [`ballistic`] with the arc factor 499 (0.75) under the character's gravity; it goes to
//!   `World::shared.launches` with the factor 498 (1.0) for the game to apply (a playable character's jump kind 3; a
//!   guard's own launch). Then the bouncer plays its clip (497).
//! - **Vault tumblers**: a ten-position dial ([`Tumbler`]). Entering state 1 turns it one position up, state 0 one
//!   down, state 2 to a random position; it then rests (state 3) once its state machine reaches the position's state
//!   (1853 + position). Reset, snap and "is solved" are methods.
//! - **Safe controllers** (class 80, pickpocket-and-pickups.md 6.1): hold the tumblers (option 1610, at most 6; each
//!   names a placement of a tumbler) and drive them ([`Safe`]); the class-121 objects themselves get no behaviour.
//!   The combination (1606) fills the wanted positions from the last tumbler backwards on entering state 0. States:
//!   0 idle, 1 scrambling, 2 in use (method 1605), 3 leaving (then 1 with 1607, else 0). In use, the stick moves
//!   the selected dial left and right (sound 54 past either end) and turns it down (stick down) or up (stick up),
//!   waiting until it rests; Circle confirms (sound 62 when wrong, unless 1608); Triangle leaves. With 1608 the safe
//!   is solved as soon as every dial rests on its wanted position.
//! - **Noise templates** (class 139): options 1184 to 1188 are Sly 2's 1090 to 1094 (copied by
//!   `behavior::sly3::aliases`), and option 1189 is Sly 2's "make a noise here" method (1095).

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::behavior::sly2_sensors::{TouchPad, characters};
use crate::behavior::{Behavior, ClassBehavior, Cx};
use crate::math::{Vec3, atan2, cos, sin, sqrt, sub};
use crate::object::{Arg, Handle};
use crate::world::{AnimCommand, Notice, World};

/// Sly 3 classes handled here.
pub mod class {
    /// Safe controllers.
    pub const SAFE: i16 = 80;
    /// Bouncers.
    pub const BOUNCER: i16 = 94;
    /// The vault tumbler.
    pub const TUMBLER: i16 = 121;
    /// Noise templates.
    pub const NOISE_TEMPLATE: i16 = 139;
}

/// Option IDs.
pub mod opt {
    /// Bouncer: the target object.
    pub const BOUNCE_TARGET: u16 = 496;
    /// Bouncer: the bounce clip.
    pub const BOUNCE_CLIP: u16 = 497;
    /// Bouncer: the launch factor (default 1.0).
    pub const BOUNCE_FACTOR: u16 = 498;
    /// Bouncer: the arc factor (default 0.75).
    pub const BOUNCE_ARC: u16 = 499;
    /// Noise template: "make a noise here" (Sly 2's 1095).
    pub const NEW_NOISE: u16 = 1189;
    /// Safe: start using it (one argument, the user).
    pub const SAFE_USE: u16 = 1605;
    /// Safe: the combination, a decimal number.
    pub const SAFE_COMBINATION: u16 = 1606;
    /// Safe: after leaving, scramble again.
    pub const SAFE_RESCRAMBLE: u16 = 1607;
    /// Safe: open as soon as the dials match.
    pub const SAFE_AUTO: u16 = 1608;
    /// Safe: adds a tumbler.
    pub const SAFE_TUMBLER: u16 = 1610;
    /// Safe: reset every tumbler.
    pub const SAFE_RESET: u16 = 1611;
    /// Safe: snap every tumbler to its wanted position.
    pub const SAFE_SNAP: u16 = 1612;
}

/// The bouncer's target without option 496: this point in its own frame.
pub const BOUNCE_LOCAL_TARGET: Vec3 = [0.0, 0.0, 300.0];
/// State 1 lasts more than this before the launch.
pub const BOUNCE_DELAY: f32 = 0.05;
/// The default arc factor.
pub const BOUNCE_ARC: f32 = 0.75;

/// A launch for the game to apply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// Who is launched.
    pub who: Handle,
    /// The launch velocity.
    pub velocity: Vec3,
    /// The launch factor (bouncer option 498).
    pub factor: f32,
    /// The launcher.
    pub by: Handle,
}

/// The velocity from `from` that carries a projectile through `to` under `gravity` (negative, along z), leaving at
/// α = θ + arc·(π/2 − θ), θ the elevation of `to` above `from` (at least 0): v = (d / cos α)·√(|g| / (2(d·tan α −
/// h))). Straight above (d = 0) it is the vertical speed that just reaches `to`. `None` when there is no solution
/// (the game then uses a fixed vector, not given).
pub fn ballistic(from: Vec3, to: Vec3, arc: f32, gravity: f32) -> Option<Vec3> {
    let g = gravity.abs();
    let delta = sub(to, from);
    let d = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
    let h = delta[2];
    if d < 1e-3 {
        return (h > 0.0).then(|| [0.0, 0.0, sqrt(2.0 * g * h)]);
    }
    let theta = atan2(h, d).max(0.0);
    let alpha = theta + arc * (core::f32::consts::FRAC_PI_2 - theta);
    let (sa, ca) = (sin(alpha), cos(alpha));
    if ca <= 1e-6 {
        return None;
    }
    let denom = d * sa / ca - h;
    if denom <= 0.0 {
        return None;
    }
    let v = d / ca * sqrt(g / (2.0 * denom));
    let (ux, uy) = (delta[0] / d, delta[1] / d);
    Some([ux * v * ca, uy * v * ca, v * sa])
}

/// A bouncer (class 94).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bouncer {
    /// State: 0 waiting, 1 touched, 2 launching.
    pub state: u8,
    /// Time in the state.
    pub since: f32,
    /// What touched it.
    pub toucher: Option<Handle>,
    /// Launches so far.
    pub launches: u32,
}

impl Bouncer {
    /// The target point.
    pub fn target(world: &World, me: Handle) -> Vec3 {
        let named = world
            .options(me)
            .and_then(|o| o.i32(opt::BOUNCE_TARGET))
            .and_then(|id| world.find_sly3_id(id, Some(me)))
            .and_then(|h| world.position(h));
        named.unwrap_or_else(|| {
            world.world_transform(me).map_or(BOUNCE_LOCAL_TARGET, |t| t.transform_point(BOUNCE_LOCAL_TARGET))
        })
    }

    fn toucher(cx: &Cx<'_>) -> Option<Handle> {
        let me = cx.my_position();
        let (reach, above) = TouchPad::REACH;
        let character = characters(cx.world).into_iter().find(|&(_, c)| {
            let d = sub(c, me);
            let feet = d[2] - crate::behavior::sly2_sensors::CHARACTER_CENTRE;
            sqrt(d[0] * d[0] + d[1] * d[1]) <= reach && (-20.0..=above).contains(&feet)
        });
        character.map(|c| c.0).or_else(|| {
            cx.world.notices(cx.me).iter().find_map(|n| match *n {
                Notice::Touched(h) => Some(h),
                _ => None,
            })
        })
    }

    fn launch(&mut self, cx: &mut Cx<'_>) {
        let Some(who) = self.toucher else { return };
        let Some(from) = cx.world.position(who) else { return };
        let o = cx.world.options(cx.me);
        let arc = o.and_then(|o| o.f32(opt::BOUNCE_ARC)).unwrap_or(BOUNCE_ARC);
        let factor = o.and_then(|o| o.f32(opt::BOUNCE_FACTOR)).unwrap_or(1.0);
        let clip = o.and_then(|o| o.i32(opt::BOUNCE_CLIP));
        let to = Self::target(cx.world, cx.me);
        if let Some(velocity) = ballistic(from, to, arc, crate::gang::consts::spec::GRAVITY) {
            cx.world.shared.launches.push(Launch { who, velocity, factor, by: cx.me });
            self.launches += 1;
        }
        if let Some(segment) = clip.and_then(|id| cx.world.find_sly3_id(id, Some(cx.me))) {
            let owner = cx.me;
            cx.world.anim.push(AnimCommand::Play { segment, owner, time: 0.0, speed: 1.0, tag: 497 });
        }
    }
}

impl ClassBehavior for Bouncer {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.since += cx.dt;
        match self.state {
            0 => {
                if let Some(h) = Self::toucher(cx) {
                    *self = Bouncer { state: 1, since: 0.0, toucher: Some(h), ..*self };
                }
            }
            1 if self.since > BOUNCE_DELAY => {
                self.state = 2;
                self.since = 0.0;
                self.launch(cx);
            }
            2 => {
                self.state = 0;
                self.since = 0.0;
                self.toucher = None;
            }
            _ => {}
        }
    }
}

/// The vault tumbler's first position state (position 0; position p is 1853 + p).
pub const TUMBLER_FIRST_STATE: i32 = 1853;

/// A vault tumbler (class 121): a dial with ten positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tumbler {
    /// Current position, 0 to 9.
    pub position: u8,
    /// Wanted position.
    pub wanted: u8,
    /// State: −1 at start, 0 turning down, 1 turning up, 2 going random, 3 resting.
    pub state: i8,
    /// Time in the state.
    pub since: f32,
    /// The state-machine state reached (1853 + a position).
    pub machine: i32,
}

impl Default for Tumbler {
    fn default() -> Self {
        Tumbler { position: 0, wanted: 0, state: -1, since: 0.0, machine: TUMBLER_FIRST_STATE }
    }
}

impl Tumbler {
    /// The state-machine state of a position.
    pub fn state_of(position: u8) -> i32 {
        TUMBLER_FIRST_STATE + i32::from(position % 10)
    }

    /// Enters state `state` (0 down, 1 up, 2 random with `random` in 0 to 9).
    pub fn enter(&mut self, state: i8, random: u8) {
        self.position = match state {
            0 => (self.position + 9) % 10,
            1 => (self.position + 1) % 10,
            2 => random % 10,
            _ => self.position,
        };
        self.state = state;
        self.since = 0.0;
    }

    /// Reset: position 0, snapped, resting.
    pub fn reset(&mut self) {
        self.position = 0;
        self.machine = Self::state_of(0);
        self.state = 3;
    }

    /// Snap to the wanted position, resting.
    pub fn snap(&mut self) {
        self.position = self.wanted;
        self.machine = Self::state_of(self.position);
        self.state = 3;
    }

    /// Resting at the wanted position.
    pub fn solved(&self) -> bool {
        self.state == 3 && self.position == self.wanted
    }

    /// One step: while turning, the state machine moves to the position's state (taken as reached at once; its
    /// transitions' timing is the state machine's), and then the tumbler rests.
    pub fn step(&mut self, dt: f32) {
        self.since += dt;
        match self.state {
            -1 => self.state = 3,
            0..=2 => {
                self.machine = Self::state_of(self.position);
                self.state = 3;
            }
            _ => {}
        }
    }
}

impl ClassBehavior for Tumbler {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.step(cx.dt);
    }
}

/// The most tumblers a safe holds.
pub const SAFE_MAX_TUMBLERS: usize = 6;
/// The safe's input repeat.
pub const SAFE_REPEAT: f32 = 0.15;
/// Sound: the selection moved past either end.
pub const SAFE_SOUND_END: i32 = 54;
/// Sound: a wrong combination confirmed.
pub const SAFE_SOUND_WRONG: i32 = 62;

/// The safe's input for one frame, in the PlayStation's terms (the game maps its buttons by position: Triangle the
/// top face button, Circle the right one).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeInput {
    /// Left stick, x right and y up.
    pub stick: [f32; 2],
    /// Triangle went down.
    pub triangle: bool,
    /// Circle went down.
    pub circle: bool,
}

impl SafeInput {
    /// From the world's controller this frame.
    pub fn from_frame(i: &crate::game::systems::FrameInput) -> SafeInput {
        use platform_api::Buttons;
        SafeInput { stick: i.left, triangle: i.just_pressed(Buttons::X), circle: i.just_pressed(Buttons::A) }
    }
}

/// A safe controller (class 80) and its tumblers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Safe {
    /// State: 0 idle, 1 scrambling, 2 in use, 3 leaving.
    pub state: u8,
    /// Time in the state.
    pub since: f32,
    /// The combination (1606).
    pub combination: i32,
    /// Scramble again after leaving (1607).
    pub rescramble: bool,
    /// Open as soon as the dials match (1608).
    pub auto: bool,
    /// The tumblers: the class-121 object (if found) and its dial.
    pub tumblers: Vec<(Option<Handle>, Tumbler)>,
    /// The user while in use.
    pub user: Option<Handle>,
    /// The selected dial.
    pub selected: usize,
    /// Waiting for the selected tumbler to rest before reading the stick again.
    pub waiting: bool,
    /// Time until the stick is read again.
    pub repeat: f32,
    /// Solved (scripts see it through the state [I]).
    pub solved: bool,
    /// Set on leaving (field +0x1D0).
    pub left: bool,
}

impl Safe {
    /// From the controller's options.
    pub fn new(world: &World, h: Handle) -> Safe {
        let o = world.options(h);
        let tumblers = o
            .map(|o| o.all(opt::SAFE_TUMBLER).filter_map(|v| v.as_i32()).collect::<Vec<i32>>())
            .unwrap_or_default()
            .into_iter()
            .take(SAFE_MAX_TUMBLERS)
            .map(|id| {
                // The ID names a placement of a tumbler, or the tumbler.
                let found = world.find_sly3_id(id, Some(h)).and_then(|p| {
                    if world.class(p) == Some(class::TUMBLER) {
                        Some(p)
                    } else {
                        world.children(p).find(|&c| world.class(c) == Some(class::TUMBLER))
                    }
                });
                (found, Tumbler::default())
            })
            .collect();
        let mut s = Safe {
            combination: o.and_then(|o| o.i32(opt::SAFE_COMBINATION)).unwrap_or(0),
            rescramble: o.and_then(|o| o.i32(opt::SAFE_RESCRAMBLE)).is_some_and(|v| v != 0),
            auto: o.and_then(|o| o.i32(opt::SAFE_AUTO)).is_some_and(|v| v != 0),
            tumblers,
            ..Safe::default()
        };
        s.enter(0);
        s
    }

    /// The combination's decimal digits to the wanted positions, from the last tumbler backwards; with fewer digits
    /// than tumblers the first keep their wanted positions.
    pub fn set_wanted(&mut self) {
        let mut n = self.combination.max(0);
        for (_, t) in self.tumblers.iter_mut().rev() {
            if n == 0 {
                break;
            }
            t.wanted = (n % 10) as u8;
            n /= 10;
        }
    }

    /// Enters controller state `state`.
    pub fn enter(&mut self, state: u8) {
        self.state = state;
        self.since = 0.0;
        match state {
            0 => self.set_wanted(),
            1 => {}
            2 => {
                self.selected = 0;
                self.waiting = false;
                self.repeat = 0.0;
            }
            _ => self.left = true,
        }
    }

    /// Every dial rests on its wanted position.
    pub fn dials_match(&self) -> bool {
        !self.tumblers.is_empty() && self.tumblers.iter().all(|(_, t)| t.solved())
    }

    /// Reset every tumbler (method 1611).
    pub fn reset(&mut self) {
        self.tumblers.iter_mut().for_each(|(_, t)| t.reset());
    }

    /// Snap every tumbler (method 1612).
    pub fn snap(&mut self) {
        self.tumblers.iter_mut().for_each(|(_, t)| t.snap());
    }

    /// One step with this frame's input; `random` gives 0 to 9 for scrambling; returns the sounds to play.
    pub fn step(&mut self, dt: f32, input: &SafeInput, mut random: impl FnMut() -> u8) -> Vec<i32> {
        let mut sounds = Vec::new();
        self.since += dt;
        for (_, t) in &mut self.tumblers {
            t.step(dt);
        }
        match self.state {
            1 => {
                if self.since <= dt {
                    for (_, t) in &mut self.tumblers {
                        t.enter(2, random());
                    }
                }
                if self.tumblers.iter().all(|(_, t)| t.state != 2) {
                    self.enter(0);
                }
            }
            2 => {
                self.repeat = (self.repeat - dt).max(0.0);
                if input.triangle {
                    self.enter(3);
                    return sounds;
                }
                if self.auto && self.dials_match() {
                    self.solved = true;
                    self.enter(0);
                    return sounds;
                }
                if input.circle {
                    if self.dials_match() {
                        self.solved = true;
                        self.enter(0);
                        return sounds;
                    } else if !self.auto {
                        sounds.push(SAFE_SOUND_WRONG);
                    }
                }
                if self.waiting {
                    self.waiting = self.tumblers.get(self.selected).is_some_and(|(_, t)| t.state != 3);
                    return sounds;
                }
                if self.repeat > 0.0 {
                    return sounds;
                }
                let [x, y] = input.stick;
                if x.abs() >= 0.5 && x.abs() >= y.abs() {
                    let next = self.selected as i32 + if x > 0.0 { 1 } else { -1 };
                    if next < 0 || next as usize >= self.tumblers.len() {
                        sounds.push(SAFE_SOUND_END);
                    } else {
                        self.selected = next as usize;
                    }
                    self.repeat = SAFE_REPEAT;
                } else if y.abs() >= 0.5
                    && let Some((_, t)) = self.tumblers.get_mut(self.selected)
                {
                    // A positive vertical step on the PlayStation stick (down) turns the dial down.
                    t.enter(if y < 0.0 { 0 } else { 1 }, 0);
                    self.waiting = true;
                    self.repeat = SAFE_REPEAT;
                }
            }
            3 => {
                self.user = None;
                let next = if self.rescramble { 1 } else { 0 };
                self.enter(next);
            }
            _ => {}
        }
        sounds
    }
}

impl ClassBehavior for Safe {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let input = if self.state == 2 { SafeInput::from_frame(&cx.world.shared.input) } else { SafeInput::default() };
        let rng = &mut *cx.rng;
        let sounds = self.step(cx.dt, &input, || (rng.next_u32() % 10) as u8);
        for s in sounds {
            cx.world.play_sound(s, Some(cx.me));
        }
    }

    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[Arg]) -> bool {
        match id {
            opt::SAFE_USE => {
                self.user = args.iter().find_map(|a| match *a {
                    Arg::Int(id) => world.find_sly3_id(id, Some(me)),
                    _ => None,
                });
                self.enter(2);
            }
            opt::SAFE_RESET => self.reset(),
            opt::SAFE_SNAP => self.snap(),
            _ => return false,
        }
        true
    }
}

/// A Sly 3 noise template (class 139): the Sly 2 template's rules with Sly 3's IDs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoiseTemplate {
    /// Noises made from it.
    pub made: u32,
}

impl ClassBehavior for NoiseTemplate {
    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[Arg]) -> bool {
        if id != opt::NEW_NOISE {
            return false;
        }
        let at = args.iter().find_map(|a| match *a {
            Arg::Vector(_, v) => Some(v),
            _ => None,
        });
        let owner = args.iter().find_map(|a| match *a {
            Arg::Int(id) => world.find_sly3_id(id, Some(me)),
            Arg::Short(6, oid) if oid >= 0 => world.find_nearest(oid, Some(me)),
            _ => None,
        });
        let at = at.or_else(|| world.position(me)).unwrap_or([0.0; 3]);
        let template = world.sly3_id(me).unwrap_or(-1);
        let now = world.shared.time;
        crate::behavior::sly2::make_noise(world, template, at, owner, None, now);
        self.made += 1;
        world.action_result = Some(me);
        true
    }
}

/// The behaviour of a Sly 3 device, from its class.
pub fn for_class(world: &World, h: Handle) -> Behavior {
    match world.class(h) {
        Some(class::BOUNCER) => Behavior::Custom(Box::<Bouncer>::default()),
        Some(class::SAFE) => Behavior::Custom(Box::new(Safe::new(world, h))),
        Some(class::NOISE_TEMPLATE) => Behavior::Custom(Box::<NoiseTemplate>::default()),
        _ => Behavior::None,
    }
}

/// The lists of launches pending, cleared (for the game).
pub fn take_launches(world: &mut World) -> Vec<Launch> {
    core::mem::take(&mut world.shared.launches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ballistic_arcs_reach_the_target() {
        let g = -2275.0;
        // Straight up: the vertical speed that just reaches 300.
        let v = ballistic([0.0; 3], [0.0, 0.0, 300.0], 0.75, g).unwrap();
        assert!((v[2] - sqrt(2.0 * 2275.0 * 300.0)).abs() < 1e-2 && v[0] == 0.0);
        // A target 1,000 away and 200 up: the arc passes through it.
        let to = [1000.0, 0.0, 200.0];
        let v = ballistic([0.0; 3], to, 0.75, g).unwrap();
        let t = to[0] / v[0];
        let z = v[2] * t + 0.5 * g * t * t;
        assert!((z - 200.0).abs() < 0.5, "{z}");
        // α = θ + 0.75 (π/2 − θ).
        let theta = atan2(200.0, 1000.0);
        let alpha = theta + 0.75 * (core::f32::consts::FRAC_PI_2 - theta);
        assert!((atan2(v[2], v[0]) - alpha).abs() < 1e-4);
        assert!(ballistic([0.0; 3], [0.0, 0.0, -10.0], 0.75, g).is_none());
    }

    #[test]
    fn safe_combination_and_input() {
        let mut s = Safe {
            combination: 113,
            auto: true,
            tumblers: alloc::vec![(None, Tumbler::default()); 3],
            ..Safe::default()
        };
        s.enter(0);
        assert_eq!(s.tumblers.iter().map(|t| t.1.wanted).collect::<Vec<_>>(), [1, 1, 3]);
        // Fewer digits than tumblers: the first keeps its wanted position.
        s.combination = 7;
        s.tumblers[0].1.wanted = 5;
        s.set_wanted();
        assert_eq!(s.tumblers.iter().map(|t| t.1.wanted).collect::<Vec<_>>(), [5, 1, 7]);
        s.combination = 113;
        s.enter(0);
        let dt = 1.0 / 60.0;
        let idle = SafeInput::default();
        s.step(dt, &idle, || 0);
        s.enter(2);
        let stick = |x: f32, y: f32| SafeInput { stick: [x, y], ..SafeInput::default() };
        // Dial 0 up once (0 → 1); then right; dial 1 up once; right; dial 2 down 7 times (0 → 3).
        let run = |s: &mut Safe, i: SafeInput| {
            let out = s.step(dt, &i, || 0);
            for _ in 0..12 {
                s.step(dt, &idle, || 0);
            }
            out
        };
        assert_eq!(run(&mut s, stick(-1.0, 0.0)), [SAFE_SOUND_END]);
        run(&mut s, stick(0.0, 1.0));
        assert_eq!(s.tumblers[0].1.position, 1);
        run(&mut s, stick(1.0, 0.0));
        run(&mut s, stick(0.0, 1.0));
        run(&mut s, stick(1.0, 0.0));
        for _ in 0..7 {
            run(&mut s, stick(0.0, -1.0));
        }
        // With 1608 the safe opens as soon as the dials match.
        assert!(s.solved && s.state == 0, "{:?}", s.tumblers);
        // Without it a wrong confirm plays sound 62, and Triangle leaves.
        let mut s = Safe { combination: 9, tumblers: alloc::vec![(None, Tumbler::default())], ..Safe::default() };
        s.enter(0);
        s.step(dt, &idle, || 0);
        s.enter(2);
        assert_eq!(s.step(dt, &SafeInput { circle: true, ..idle }, || 0), [SAFE_SOUND_WRONG]);
        s.step(dt, &SafeInput { triangle: true, ..idle }, || 0);
        assert_eq!(s.state, 3);
        s.step(dt, &idle, || 0);
        assert_eq!(s.state, 0);
    }

    #[test]
    fn tumbler_turns_and_rests() {
        let mut t = Tumbler::default();
        t.step(0.1);
        assert_eq!(t.state, 3);
        t.enter(0, 0);
        assert_eq!(t.position, 9);
        t.step(0.1);
        assert_eq!((t.state, t.machine), (3, 1862));
        t.enter(1, 0);
        t.enter(1, 0);
        assert_eq!(t.position, 1);
        t.enter(2, 7);
        t.step(0.1);
        assert_eq!((t.position, t.machine), (7, 1860));
        t.wanted = 4;
        assert!(!t.solved());
        t.snap();
        assert!(t.solved());
        t.reset();
        assert_eq!((t.position, t.state), (0, 3));
    }
}
