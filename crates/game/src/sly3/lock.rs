//! The cell lock (class 79; specs/sly3/interactables.md 5, "cell lock"): the tumble-lock minigame a use point starts
//! (method 1600, script ID 247, with the user).
//!
//! - **States 0 to 5.** Entering a state stops the previous state's script and starts the new one's with the user:
//!   1 the intro (1868), 2 quitting (1869), 3 the solving view (1870), 4 solved (1871); 0 and 5 have none.
//!   - 1 goes to 3 when the intro script ends.
//!   - In 3, Triangle quits (2), and the stick turns the dial. The lock is solved (4) when all three tumblers are at
//!     5 or 6 and none is part-way.
//!   - 2 and 4 hand control back to the user when their script ends.
//!   - Losing control sends the lock to 0 below state 4, else to 5.
//! - **The dial** turns with the stick's rotation. Each frame, the change of the stick's angle (atan2) has a dead
//!   zone of 0.1 rad. Its rate per second is smoothed into the dial's speed, and the angle advances by speed × dt.
//!   The dial has 100 marks. The combination, 85, 20, 75, is the game's constant, shared by every class-79 lock.
//! - **Tumblers** run −1 to 6, one step per frame toward what the dial sets.
//!   - The dial works on one tumbler at a time, turning up the marks for tumblers 0 and 2 and down for tumbler 1.
//!   - Within 8 marks of the tumbler's gate the tumbler goes to 3, and within 4 to 4. Crossing the gate sets it (5),
//!     and the dial moves on to the next tumbler.
//!   - A pass the wrong way while a tumbler is part-way (3 or 4) resets it to −1.
//!   - Working choices: the per-value gate thresholds (the 8- and 4-mark tolerances as the bands for 3 and 4), the
//!     direction by index parity alone, 5 for every set tumbler (6 is not produced), and the smoothing rate.
//!
//! - **Stand-in:** what raises event 5 on the lock's part 113853 (which forwards it to the lock, whose task event 29
//!   completes 2115) is not traced. Until it is, the lock raises event 5 on itself when it enters state 4.
//!
//! Starting the lock gives it player 0's control (`World::shared.driving`, as command 0x13 hands control over), so
//! the crew gets an idle pad while it runs.

use engine::game::systems::FrameInput;
use engine::math::atan2;
use engine::object::Handle;
use engine::world::World;
use platform_api::Buttons;

use super::commands::Scripts;

/// Each state's script (missions.md 4.1a; interactables.md 5).
pub const STATE_SCRIPTS: [Option<i32>; 6] = [None, Some(1868), Some(1869), Some(1870), Some(1871), None];
/// The tumblers' range.
pub const TUMBLER_MIN: i32 = -1;
/// See [`TUMBLER_MIN`].
pub const TUMBLER_MAX: i32 = 6;
/// The values of a set tumbler.
pub const OPEN: [i32; 2] = [5, 6];
/// The dial's marks.
pub const MARKS: f32 = 100.0;
/// The combination, in marks: the game's constant, the same for every class-79 lock.
pub const COMBINATION: [f32; 3] = [85.0, 20.0, 75.0];
/// The bands before a gate: within the first the tumbler goes to 3, within the second to 4.
pub const TOLERANCES: [f32; 2] = [8.0, 4.0];
/// The stick's dead zone per frame, radians.
pub const DEAD_ZONE: f32 = 0.1;
/// How fast the dial's speed follows the stick's rate, per second (working choice).
pub const SMOOTHING: f32 = 10.0;
/// How far the stick must be pushed to have an angle.
const PUSH: f32 = 0.5;

/// `a` modulo `m`, in 0..m (`f32::rem_euclid` is not in `core`).
fn modulo(a: f32, m: f32) -> f32 {
    let r = a % m;
    if r < 0.0 { r + m } else { r }
}

/// The angle `a` wrapped to −π..π.
fn wrap(a: f32) -> f32 {
    let t = core::f32::consts::TAU;
    let a = modulo(a, t);
    if a > t / 2.0 { a - t } else { a }
}

/// The dial and its three tumblers.
#[derive(Clone, Debug, PartialEq)]
pub struct Dial {
    /// The dial's angle, radians.
    pub angle: f32,
    /// Its angular speed, radians per second.
    pub speed: f32,
    /// The tumblers' values.
    pub values: [i32; 3],
    /// The values the tumblers step toward.
    pub targets: [i32; 3],
    /// The tumbler the dial works on (3 once all are set).
    pub active: usize,
    /// The stick's angle the last counted change ended at.
    last: Option<f32>,
}

impl Default for Dial {
    fn default() -> Dial {
        Dial { angle: 0.0, speed: 0.0, values: [TUMBLER_MIN; 3], targets: [TUMBLER_MIN; 3], active: 0, last: None }
    }
}

impl Dial {
    /// The dial's position in marks, 0 to 100.
    pub fn marks(&self) -> f32 {
        modulo(self.angle / core::f32::consts::TAU * MARKS, MARKS)
    }

    /// Whether every tumbler is set and none is part-way.
    pub fn solved(&self) -> bool {
        self.values == self.targets && self.values.iter().all(|v| OPEN.contains(v))
    }

    /// The way the dial turns for tumbler `i`: up the marks (1) or down (−1).
    pub fn direction(i: usize) -> f32 {
        if i % 2 == 0 { 1.0 } else { -1.0 }
    }

    /// One frame of the stick.
    pub fn steer(&mut self, input: &FrameInput, dt: f32) {
        let [x, y] = input.left;
        let mut rate = 0.0;
        if x * x + y * y > PUSH * PUSH {
            let a = atan2(y, x);
            if let Some(l) = self.last {
                let d = wrap(a - l);
                if d.abs() >= DEAD_ZONE {
                    rate = d / dt.max(1e-4);
                }
            }
            self.last = Some(a);
        } else {
            self.last = None;
        }
        self.speed += (rate - self.speed) * (dt * SMOOTHING).min(1.0);
        let before = self.marks();
        self.angle += self.speed * dt;
        let mut moved = self.marks() - before;
        if moved > MARKS / 2.0 {
            moved -= MARKS;
        } else if moved < -MARKS / 2.0 {
            moved += MARKS;
        }
        self.gate(before, moved);
        for (v, t) in self.values.iter_mut().zip(&self.targets) {
            *v += (*t - *v).signum();
        }
    }

    /// The active tumbler, for a move of `moved` marks from `from`.
    fn gate(&mut self, from: f32, moved: f32) {
        let i = self.active;
        if i >= 3 || moved == 0.0 {
            return;
        }
        let want = Dial::direction(i);
        if moved.signum() != want {
            if matches!(self.targets[i], 3 | 4) {
                self.targets[i] = TUMBLER_MIN;
            }
            return;
        }
        // The marks left to the gate the way the dial turns, after the move.
        let left = modulo((COMBINATION[i] - from) * want, MARKS) - moved.abs();
        if left <= 0.0 {
            self.targets[i] = OPEN[0];
            self.active += 1;
        } else if left <= TOLERANCES[1] {
            self.targets[i] = 4;
        } else if left <= TOLERANCES[0] {
            self.targets[i] = 3;
        }
    }
}

/// A running cell lock.
#[derive(Clone, Debug, PartialEq)]
pub struct Lock {
    /// The lock object.
    pub handle: Handle,
    /// Who started it.
    pub user: Option<Handle>,
    /// 0 to 5 (module documentation).
    pub state: i32,
    /// The dial and tumblers.
    pub dial: Dial,
    /// The script of the current state, if it started.
    pub script: Option<i32>,
}

impl Lock {
    /// Starts lock `handle` for `user`: control goes to the lock and it enters state 1.
    pub fn start(handle: Handle, user: Option<Handle>, world: &mut World, scripts: &mut Scripts) -> Lock {
        let mut l = Lock { handle, user, state: 0, dial: Dial::default(), script: None };
        world.shared.driving = Some(handle);
        l.enter(1, world, scripts);
        l
    }

    /// Enters state `s`: the previous state's script stops and the new one's starts with the user.
    pub fn enter(&mut self, s: i32, world: &mut World, scripts: &mut Scripts) {
        if let Some(old) = self.script.take() {
            scripts.stop(old);
        }
        self.state = s;
        // Stand-in for an untraced link (interactables.md 5, open question: what raises event 5 on the lock's part
        // 113853): the solved lock receives event 5 itself, which broadcasts task event 29 and completes
        // `t1_recon_tumble_lock` (2115).
        if s == 4 {
            world.raise(self.handle, 5, alloc::vec::Vec::new());
        }
        let user = self.user.and_then(|u| world.sly3_id(u)).unwrap_or(-1);
        if let Some(id) = STATE_SCRIPTS.get(s as usize).copied().flatten()
            && scripts.start(world, id, user)
        {
            self.script = Some(id);
        }
    }

    /// Whether the current state's script has ended (or never started).
    fn script_done(&self, scripts: &Scripts) -> bool {
        self.script.is_none_or(|id| !scripts.is_running(id))
    }

    /// One frame with the pad `input`.
    pub fn update(&mut self, input: &FrameInput, dt: f32, world: &mut World, scripts: &mut Scripts) {
        if (1..=4).contains(&self.state) && world.shared.driving != Some(self.handle) {
            let s = if self.state < 4 { 0 } else { 5 };
            self.enter(s, world, scripts);
            return;
        }
        match self.state {
            1 if self.script_done(scripts) => self.enter(3, world, scripts),
            3 => {
                if input.pressed & Buttons::X.0 != 0 {
                    self.enter(2, world, scripts);
                    return;
                }
                self.dial.steer(input, dt);
                if self.dial.solved() {
                    self.enter(4, world, scripts);
                }
            }
            2 | 4 if self.script_done(scripts) => {
                // Control goes back to the user; the next frame sees it lost.
                if world.shared.driving == Some(self.handle) {
                    world.shared.driving = None;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::math::{cos, sin};

    /// The stick turned by `step` radians a frame, from angle `at`.
    fn turn(d: &mut Dial, at: &mut f32, step: f32) {
        *at += step;
        d.steer(&FrameInput { left: [cos(*at), sin(*at)], ..Default::default() }, 1.0 / 60.0);
    }

    #[test]
    fn small_stick_changes_do_not_turn_the_dial() {
        let mut d = Dial::default();
        let mut at = 0.0;
        for _ in 0..120 {
            turn(&mut d, &mut at, 0.05);
        }
        assert!(d.angle.abs() < 1e-3, "{}", d.angle);
    }

    #[test]
    fn turning_through_the_combination_sets_the_tumblers() {
        let mut d = Dial::default();
        let mut at = 0.0;
        for i in 0..3 {
            for _ in 0..2000 {
                if d.active != i {
                    break;
                }
                turn(&mut d, &mut at, 0.15 * Dial::direction(i));
            }
            assert_eq!(d.active, i + 1, "tumbler {i} set");
        }
        for _ in 0..10 {
            turn(&mut d, &mut at, 0.0);
        }
        assert!(d.solved(), "{d:?}");
    }

    #[test]
    fn a_wrong_way_pass_resets_a_part_way_tumbler() {
        let mut d = Dial { values: [3, -1, -1], targets: [3, -1, -1], ..Default::default() };
        d.gate(80.0, -1.0);
        assert_eq!(d.targets[0], TUMBLER_MIN);
        assert!(!d.solved());
    }
}
