//! Sly 2 state machines (class 111; specs/sly2/classes/animation.md 2, animation.md 8, with the run rules of
//! ../sly1/scripts.md 9 that Sly 2 keeps [R]): the record, and one run of it.
//!
//! - **Record** ([`MachineDef`]): member objects (the clips and helpers it plays), states (state ID, object: the
//!   clip held while in the state; −1 or 32 none) and transitions (from, to, flags, an `s32` not used here, a
//!   probability and the clip played on the way; −1 none). A transition naming an object that is not a member is
//!   dropped at load [R]. Flag bit 0 makes a transition usable both ways (backwards, its clip plays in reverse from
//!   its end). The groups of flag bits 2 to 12 are not used yet.
//! - **Run** ([`MachineRun`]): a current state and a goal. With a goal different from the state and no step under
//!   way, the run takes the first step of a shortest route (fewest transitions) from the state to the goal; among
//!   the first steps of all shortest routes it picks one at random weighted by probability, or the last in table
//!   order when all weights are 0. A step with a clip lasts the clip's duration (divided by the run's speed); one
//!   without is taken at once. Reaching the end enters the step's far state and chooses again. With no route the
//!   goal is cleared. Setting a goal "instantly" jumps to it.
//!
//! What the steps and states play is reported as [`MachineRun::playing`]: the clip object and its local time,
//! for the animation system to apply to the run's target (`game::sly2_anim`).

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use formats::reader::Reader;
use formats_sly2::stream::Record;

/// A state's object ID meaning "no object of its own" [R].
pub const NO_OBJECT: i32 = 32;
/// Most instantaneous steps taken in one update.
pub const MAX_INSTANT_STEPS: usize = 64;

/// A transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: i32,
    pub to: i32,
    pub flags: i32,
    /// The fourth field (meaning not established).
    pub extra: i32,
    /// Weight when choosing among equally short routes.
    pub probability: f32,
    /// The clip played on the way (object ID, −1 none).
    pub object: i32,
}

impl Transition {
    /// Usable both ways.
    pub fn two_way(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// A state machine record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineDef {
    /// Object IDs of the member objects.
    pub members: Vec<i32>,
    /// (state ID, object ID).
    pub states: Vec<(i32, i32)>,
    pub transitions: Vec<Transition>,
}

impl MachineDef {
    /// Reads a class-111 record's body from the world bytes `data`.
    pub fn read(data: &[u8], r: &Record) -> Option<MachineDef> {
        MachineDef::read_with_end(data, r).map(|x| x.0)
    }

    /// [`MachineDef::read`], with the offset where the transitions end (the option list starts there).
    pub fn read_with_end(data: &[u8], r: &Record) -> Option<(MachineDef, usize)> {
        if r.class != 111 {
            return None;
        }
        let members: Vec<i32> = r.nested().iter().map(|c| c.id).collect();
        // The member list ends where its last record's body ends.
        let start = match r.nested().last() {
            Some(c) => (c.body_offset + c.body_size) as usize,
            None => r.body_offset as usize + 2,
        };
        let mut rd = Reader::at(data, start);
        let n = rd.u8().ok()? as usize;
        let mut states = Vec::with_capacity(n);
        for _ in 0..n {
            states.push((rd.i32().ok()?, rd.i32().ok()?));
        }
        let m = rd.u8().ok()? as usize;
        let mut transitions = Vec::with_capacity(m);
        for _ in 0..m {
            let t = Transition {
                from: rd.i32().ok()?,
                to: rd.i32().ok()?,
                flags: rd.i32().ok()?,
                extra: rd.i32().ok()?,
                probability: rd.f32().ok()?,
                object: rd.i32().ok()?,
            };
            if t.object == -1 || members.contains(&t.object) {
                transitions.push(t);
            }
        }
        Some((MachineDef { members, states, transitions }, rd.pos()))
    }

    /// The object a state holds (none for −1 and 32).
    pub fn state_object(&self, state: i32) -> Option<i32> {
        self.states.iter().find(|s| s.0 == state).map(|s| s.1).filter(|&o| o != -1 && o != NO_OBJECT)
    }

    /// The first state (where a run starts).
    pub fn first_state(&self) -> Option<i32> {
        self.states.first().map(|s| s.0).or_else(|| self.transitions.first().map(|t| t.from))
    }

    /// Steps usable from `state`: (transition, reversed, far state).
    fn steps_from(&self, state: i32) -> impl Iterator<Item = (usize, bool, i32)> + '_ {
        self.transitions.iter().enumerate().filter_map(move |(i, t)| {
            if t.from == state {
                Some((i, false, t.to))
            } else if t.two_way() && t.to == state {
                Some((i, true, t.from))
            } else {
                None
            }
        })
    }

    /// Fewest steps from `from` to `goal`, or `None` without a route.
    pub fn distance(&self, from: i32, goal: i32) -> Option<usize> {
        if from == goal {
            return Some(0);
        }
        let mut seen = vec![from];
        let mut queue = VecDeque::from([(from, 0usize)]);
        while let Some((s, d)) = queue.pop_front() {
            for (_, _, far) in self.steps_from(s) {
                if far == goal {
                    return Some(d + 1);
                }
                if !seen.contains(&far) {
                    seen.push(far);
                    queue.push_back((far, d + 1));
                }
            }
        }
        None
    }

    /// The first step toward `goal` from `from` (missing: no route). `pick` gets a number in [0, 1).
    pub fn choose(&self, from: i32, goal: i32, pick: f32) -> Option<(usize, bool, i32)> {
        let d = self.distance(from, goal)?;
        if d == 0 {
            return None;
        }
        let candidates: Vec<(usize, bool, i32)> =
            self.steps_from(from).filter(|&(_, _, far)| self.distance(far, goal) == Some(d - 1)).collect();
        let total: f32 = candidates.iter().map(|c| self.transitions[c.0].probability.max(0.0)).sum();
        if total <= 0.0 {
            return candidates.last().copied();
        }
        let mut x = pick * total;
        for &c in &candidates {
            x -= self.transitions[c.0].probability.max(0.0);
            if x < 0.0 {
                return Some(c);
            }
        }
        candidates.last().copied()
    }
}

/// A step under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The transition.
    pub transition: usize,
    /// Walked backwards.
    pub reverse: bool,
    /// The far state.
    pub to: i32,
    /// Seconds into the step.
    pub time: f32,
    /// Its length in seconds (the clip's duration).
    pub length: f32,
}

/// What a run reported during an update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunEvent {
    /// A step began.
    Began { transition: usize, reverse: bool },
    /// A state was entered.
    Entered { from: i32, to: i32 },
    /// The goal could not be reached and was cleared.
    NoRoute { goal: i32 },
}

/// One run of a machine.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineRun {
    /// The current state.
    pub state: i32,
    /// The goal.
    pub goal: Option<i32>,
    /// The step under way.
    pub step: Option<Step>,
    /// Playback speed.
    pub speed: f32,
    /// Seconds in the current state (its clip's local time, looping).
    pub held: f32,
}

impl MachineRun {
    /// A run at the machine's first state.
    pub fn new(def: &MachineDef) -> MachineRun {
        MachineRun { state: def.first_state().unwrap_or(-1), goal: None, step: None, speed: 1.0, held: 0.0 }
    }

    /// Sets the goal; `instant` jumps there (ending a step under way).
    pub fn set_goal(&mut self, goal: i32, instant: bool, out: &mut Vec<RunEvent>) {
        if instant {
            let from = self.state;
            self.step = None;
            self.goal = None;
            if from != goal {
                self.state = goal;
                self.held = 0.0;
                out.push(RunEvent::Entered { from, to: goal });
            }
        } else {
            self.goal = Some(goal);
        }
    }

    /// Advances by `dt` seconds. `length(object)` gives a clip's duration; `random()` a number in [0, 1).
    pub fn update(
        &mut self,
        def: &MachineDef,
        dt: f32,
        length: &mut dyn FnMut(i32) -> Option<f32>,
        random: &mut dyn FnMut() -> f32,
        out: &mut Vec<RunEvent>,
    ) {
        let mut dt = dt * self.speed.max(0.0);
        for _ in 0..MAX_INSTANT_STEPS {
            if let Some(mut s) = self.step {
                s.time += dt;
                if s.time < s.length {
                    self.step = Some(s);
                    return;
                }
                dt = s.time - s.length;
                let from = self.state;
                self.state = s.to;
                self.step = None;
                self.held = 0.0;
                out.push(RunEvent::Entered { from, to: s.to });
            }
            let Some(goal) = self.goal else { break };
            if goal == self.state {
                self.goal = None;
                break;
            }
            match def.choose(self.state, goal, random()) {
                Some((t, reverse, to)) => {
                    let obj = def.transitions[t].object;
                    let len = if obj == -1 { 0.0 } else { length(obj).unwrap_or(0.0) };
                    out.push(RunEvent::Began { transition: t, reverse });
                    self.step = Some(Step { transition: t, reverse, to, time: 0.0, length: len });
                }
                None => {
                    out.push(RunEvent::NoRoute { goal });
                    self.goal = None;
                    break;
                }
            }
        }
        if self.step.is_none() {
            self.held += dt;
        }
    }

    /// What plays now: (clip object, local time). A step's clip, reversed from its end when walked backwards; else
    /// the state's clip, at the time held (the caller wraps it).
    pub fn playing(&self, def: &MachineDef) -> Option<(i32, f32)> {
        match self.step {
            Some(s) => {
                let obj = def.transitions[s.transition].object;
                (obj != -1).then(|| (obj, if s.reverse { (s.length - s.time).max(0.0) } else { s.time }))
            }
            None => def.state_object(self.state).map(|o| (o, self.held)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> MachineDef {
        let t = |from, to, flags, probability, object| Transition { from, to, flags, extra: 0, probability, object };
        MachineDef {
            members: vec![100, 101, 102],
            states: vec![(1, 100), (2, -1), (3, NO_OBJECT), (4, 102)],
            transitions: vec![t(1, 2, 0, 0.0, 101), t(2, 3, 1, 0.0, -1), t(1, 4, 0, 1.0, -1), t(4, 3, 0, 0.0, 102)],
        }
    }

    #[test]
    fn routes_and_choices() {
        let d = def();
        assert_eq!(d.distance(1, 3), Some(2));
        // Two shortest routes from 1 to 3: via 2 (weight 0) and via 4 (weight 1): the weighted one wins.
        assert_eq!(d.choose(1, 3, 0.5), Some((2, false, 4)));
        // 3 → 2 only backwards over the two-way transition; 2 → 1 has no route.
        assert_eq!(d.choose(3, 2, 0.0), Some((1, true, 2)));
        assert_eq!(d.distance(2, 1), None);
        assert_eq!(d.state_object(3), None);
        assert_eq!(d.state_object(1), Some(100));
    }

    #[test]
    fn a_run_walks_to_its_goal() {
        let d = def();
        let mut r = MachineRun::new(&d);
        assert_eq!(r.state, 1);
        let mut ev = Vec::new();
        r.goal = Some(2);
        let mut len = |o: i32| (o == 101).then_some(0.5);
        let mut rnd = || 0.0;
        r.update(&d, 0.25, &mut len, &mut rnd, &mut ev);
        assert_eq!(r.state, 1);
        assert_eq!(r.playing(&d), Some((101, 0.25)));
        r.update(&d, 0.3, &mut len, &mut rnd, &mut ev);
        assert_eq!((r.state, r.goal), (2, None));
        assert!(ev.contains(&RunEvent::Entered { from: 1, to: 2 }));
        // 2 → 3 is instantaneous; 3 → 1 has no route: the goal clears.
        r.goal = Some(3);
        r.update(&d, 0.0, &mut len, &mut rnd, &mut ev);
        assert_eq!(r.state, 3);
        r.goal = Some(1);
        r.update(&d, 0.0, &mut len, &mut rnd, &mut ev);
        assert_eq!((r.state, r.goal), (3, None));
        assert!(ev.contains(&RunEvent::NoRoute { goal: 1 }));
        r.set_goal(4, true, &mut ev);
        assert_eq!(r.state, 4);
    }
}
