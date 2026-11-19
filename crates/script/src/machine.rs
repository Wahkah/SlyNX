//! State machines (class 136): data that picks animations to reach a goal state (scripts.md section 9,
//! classes/animation-and-state.md section 2).
//!
//! [`StateMachine`] is the parsed record; [`MachineRun`] is one running instance. The engine plays and stops the
//! transitions' animations and reports when one ends, through [`MachineHost`].

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use formats::options::OptionTable;
use formats::stream::Record;

use crate::tree::ParseError;

/// Option: start by itself after the world has loaded.
pub const OPTION_AUTO_START: i16 = 513;
/// Option: the goal state (OID).
pub const OPTION_GOAL: i16 = 516;
/// Option: playback speed of the machine's animations.
pub const OPTION_SPEED: i16 = 519;
/// Most instantaneous steps taken in one call before the rest waits for the next update.
pub const MAX_INSTANT_STEPS: usize = 64;

/// A state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// The state's OID.
    pub oid: i16,
    /// The state the goal moves on to when this state is reached as the goal.
    pub next: Option<i16>,
}

/// A transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    /// Whether it has an animation (owned records follow the machine, one per animated transition, in order).
    pub animated: bool,
    /// From-state OID.
    pub from: i16,
    /// To-state OID.
    pub to: i16,
    /// Flags: bit 0 two-way, bit 1 interruptible.
    pub flags: i32,
    /// Weight when choosing among equally short routes.
    pub probability: f32,
}

impl Transition {
    /// Usable in both directions (backwards, the animation plays in reverse from its end).
    pub fn two_way(&self) -> bool {
        self.flags & 1 != 0
    }

    /// May be interrupted when the goal changes.
    pub fn interruptible(&self) -> bool {
        self.flags & 2 != 0
    }
}

/// A parsed state machine record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateMachine {
    /// States in table order.
    pub states: Vec<State>,
    /// Transitions in table order.
    pub transitions: Vec<Transition>,
    /// Option 513 is set (non-zero) in the machine's option list.
    pub auto_start: bool,
}

fn rd<const N: usize>(data: &[u8], pos: &mut usize) -> Result<[u8; N], ParseError> {
    let b = data
        .get(*pos..*pos + N)
        .ok_or(ParseError { offset: *pos, what: "state machine truncated" })?
        .try_into()
        .expect("length");
    *pos += N;
    Ok(b)
}

impl StateMachine {
    /// Parses the body of a class-136 record that the `formats` stream walker found.
    pub fn from_record(data: &[u8], record: &Record, table: &OptionTable) -> Result<StateMachine, ParseError> {
        if record.class != 136 {
            return Err(ParseError { offset: record.offset, what: "not a state machine record" });
        }
        let mut m = Self::parse_body(data, record.offset + 6)?;
        let opts = record.options.ok_or(ParseError { offset: record.offset, what: "state machine without options" })?;
        for e in opts.iter(data, table) {
            let (id, value) = e.map_err(|_| ParseError { offset: opts.start, what: "bad option list" })?;
            if id == OPTION_AUTO_START {
                m.auto_start = value.iter().any(|&b| b != 0);
            }
        }
        Ok(m)
    }

    /// Parses the state and transition tables at `pos` (the start of the body). Options are not read.
    pub fn parse_body(data: &[u8], mut pos: usize) -> Result<StateMachine, ParseError> {
        let p = &mut pos;
        let s = rd::<1>(data, p)?[0] as usize;
        let mut states = Vec::with_capacity(s);
        for _ in 0..s {
            let oid = i16::from_le_bytes(rd(data, p)?);
            let next = i16::from_le_bytes(rd(data, p)?);
            states.push(State { oid, next: (next != -1).then_some(next) });
        }
        let t = rd::<1>(data, p)?[0] as usize;
        let mut transitions = Vec::with_capacity(t);
        for _ in 0..t {
            let animated = rd::<1>(data, p)?[0] as i8 != 0;
            let from = i16::from_le_bytes(rd(data, p)?);
            let to = i16::from_le_bytes(rd(data, p)?);
            let flags = i32::from_le_bytes(rd(data, p)?);
            let probability = f32::from_le_bytes(rd(data, p)?);
            transitions.push(Transition { animated, from, to, flags, probability });
        }
        Ok(StateMachine { states, transitions, auto_start: false })
    }

    /// The state with an OID.
    pub fn state(&self, oid: i16) -> Option<&State> {
        self.states.iter().find(|s| s.oid == oid)
    }

    /// Index among the animated transitions (the owned animation record to play) of transition `t`.
    pub fn animation_index(&self, t: usize) -> Option<usize> {
        self.transitions.get(t).filter(|x| x.animated)?;
        Some(self.transitions[..t].iter().filter(|x| x.animated).count())
    }

    /// The steps (transition, direction) along which `from` can be left: forward along any transition from it,
    /// backwards along two-way transitions into it. Self-loops are listed once.
    fn edges(&self) -> impl Iterator<Item = Step> + '_ {
        self.transitions.iter().enumerate().flat_map(|(i, t)| {
            let fwd = Step { transition: i, reverse: false, from: t.from, to: t.to };
            let back = (t.two_way() && t.from != t.to).then_some(Step {
                transition: i,
                reverse: true,
                from: t.to,
                to: t.from,
            });
            core::iter::once(fwd).chain(back)
        })
    }

    /// The first steps of every shortest route (at least one step) from `from` to `goal`, in table order.
    /// Empty when the goal cannot be reached.
    pub fn first_steps(&self, from: i16, goal: i16) -> Vec<Step> {
        // Distance of each state to the goal, by a breadth-first search over the steps walked backwards.
        let mut dist: Vec<(i16, u32)> = alloc::vec![(goal, 0)];
        let mut queue = VecDeque::from([goal]);
        while let Some(s) = queue.pop_front() {
            let d = dist.iter().find(|e| e.0 == s).map(|e| e.1).unwrap_or(0);
            for e in self.edges().filter(|e| e.to == s) {
                if !dist.iter().any(|x| x.0 == e.from) {
                    dist.push((e.from, d + 1));
                    queue.push_back(e.from);
                }
            }
        }
        let d = |s: i16| dist.iter().find(|e| e.0 == s).map(|e| e.1);
        let best = self.edges().filter(|e| e.from == from).filter_map(|e| d(e.to)).min();
        match best {
            Some(b) => self.edges().filter(|e| e.from == from && d(e.to) == Some(b)).collect(),
            None => Vec::new(),
        }
    }
}

/// One step: a transition walked forwards or backwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Transition index.
    pub transition: usize,
    /// Walked backwards (animation in reverse from its end).
    pub reverse: bool,
    /// State left.
    pub from: i16,
    /// State entered at the end.
    pub to: i16,
}

/// What a running machine needs from the engine.
pub trait MachineHost {
    /// A random number in [0, 1).
    fn random(&mut self) -> f32;
    /// Start playing an animated transition's animation (animation.md 5) on the machine's root object; call
    /// [`MachineRun::animation_finished`] when it reaches its end.
    fn play(&mut self, step: Step);
    /// Stop an animation started by [`MachineHost::play`] (an interrupted step, or the machine stopped).
    fn stop(&mut self, step: Step);
    /// The machine entered a state; the engine raises event 14 for the machine object.
    fn entered(&mut self, from: i16, to: i16);
}

/// A running (or stopped) instance of a state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRun {
    /// Current state OID.
    pub state: i16,
    /// Goal state OID.
    pub goal: Option<i16>,
    /// The animated step being played.
    pub step: Option<Step>,
    /// Whether the machine is running.
    pub running: bool,
}

impl MachineRun {
    /// A stopped run in the machine's first state (scripts.md 11, item 12), with no goal.
    pub fn new(m: &StateMachine) -> MachineRun {
        MachineRun { state: m.states.first().map_or(-1, |s| s.oid), goal: None, step: None, running: false }
    }

    /// Starts the machine (the engine raises event 17) and takes the first step toward any goal.
    pub fn start(&mut self, m: &StateMachine, host: &mut dyn MachineHost) {
        self.running = true;
        self.advance(m, host);
    }

    /// Stops the machine (the engine raises event 18); a step being played is stopped.
    pub fn stop(&mut self, host: &mut dyn MachineHost) {
        if let Some(s) = self.step.take() {
            host.stop(s);
        }
        self.running = false;
    }

    /// Sets the goal (option 516). While an interruptible step plays, its animation stops and the step ends in its
    /// destination state (with the enter-state call), and the machine chooses again from there; during any other
    /// step the new goal is followed when the step ends (scripts.md 11, item 12).
    pub fn set_goal(&mut self, m: &StateMachine, host: &mut dyn MachineHost, goal: Option<i16>) {
        self.goal = goal;
        match self.step {
            Some(s) if m.transitions[s.transition].interruptible() => {
                host.stop(s);
                self.step = None;
                self.enter(m, host, s);
                self.advance(m, host);
                return;
            }
            Some(_) => return,
            None => {}
        }
        if self.goal == Some(self.state) {
            self.arrived(m);
        }
        self.advance(m, host);
    }

    /// The animation of the current step reached its end: enter its state and choose again.
    pub fn animation_finished(&mut self, m: &StateMachine, host: &mut dyn MachineHost) {
        let Some(s) = self.step.take() else { return };
        self.enter(m, host, s);
        self.advance(m, host);
    }

    /// Takes steps while idle and a goal is set (call once per frame; harmless when nothing is to do).
    pub fn update(&mut self, m: &StateMachine, host: &mut dyn MachineHost) {
        self.advance(m, host);
    }

    fn arrived(&mut self, m: &StateMachine) {
        self.goal = m.state(self.state).and_then(|s| s.next);
    }

    fn enter(&mut self, m: &StateMachine, host: &mut dyn MachineHost, s: Step) {
        self.state = s.to;
        host.entered(s.from, s.to);
        if self.goal == Some(self.state) {
            self.arrived(m);
        }
    }

    fn advance(&mut self, m: &StateMachine, host: &mut dyn MachineHost) {
        for _ in 0..MAX_INSTANT_STEPS {
            if !self.running || self.step.is_some() {
                return;
            }
            let Some(goal) = self.goal else { return };
            let candidates = m.first_steps(self.state, goal);
            let Some(step) = choose(m, &candidates, host) else {
                self.goal = None;
                return;
            };
            if m.transitions[step.transition].animated {
                self.step = Some(step);
                host.play(step);
                return;
            }
            self.enter(m, host, step);
        }
    }
}

/// Picks among candidate steps at random, weighted by their transitions' probabilities; the last candidate when
/// every weight is zero.
pub fn choose(m: &StateMachine, candidates: &[Step], host: &mut dyn MachineHost) -> Option<Step> {
    let last = *candidates.last()?;
    let w = |s: &Step| m.transitions[s.transition].probability.max(0.0);
    let total: f32 = candidates.iter().map(w).sum();
    if total.is_nan() || total <= 0.0 {
        return Some(last);
    }
    let r = host.random() * total;
    let mut acc = 0.0;
    for s in candidates {
        acc += w(s);
        if r < acc {
            return Some(*s);
        }
    }
    Some(last)
}
