//! The dance (Sly 2 classes 121 director and 68 dancers, specs/sly2/vehicles-and-minigames.md 5 and 10.1).
//!
//! The director's states: 0 off, 1 prompts shown (game paused), 2 playing. The dance's sequence (class 114) gives it
//! its commands as sequence options: 699 sets the director's state, 700 adds a beat (music time, button, move,
//! dancer; dancer −1 is the sequence's subject), 701 sets a dancer's default move ([`sequence_command`], forwarded
//! by a marked hook in the sequence's behaviour).
//!
//! While playing, each beat is scored once: a press of its button within the early or late window (options 1344
//! and 1345, default 0.4 s; 0.2 s on the disc's row 11) scores `1 − u` (u the time from the beat over the window),
//! a wrong button 1351 (−0.8), a second press 1352 (−0.2), a press before any window 1350 (−1.5), a beat passed
//! without a press 1353 (−1.5). Negative scores are scaled by `clamp(1.2 − 0.6·h, 0, 1)`, h the difficulty help.
//! The mean of the last 1348 (8) scores, each kept within −1.5..1, is read by the jobs as option 1355; the director
//! raises task event 52 when a beat's time passes, 53 on each press and 54 on each score, and the jobs count a
//! failure themselves when the mean is below −0.75 (10.1). Option 1356 clears the scores. The crowd's reaction level
//! ([`Score::reaction`]: below −0.5, below 0.5, else) is ours [I].

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::{Pad, task_event};
use crate::behavior::{ClassBehavior, Cx};
use crate::object::{Arg, Handle, OptionValue};
use crate::world::World;

/// Default timing window, early and late (section 5).
pub const WINDOW: [f32; 2] = [0.4, 0.4];
/// Default scores of the mistakes.
pub const WRONG: f32 = -0.8;
pub const TWICE: f32 = -0.2;
pub const EARLY: f32 = -1.5;
pub const MISSED: f32 = -1.5;

/// Director options (10.1).
pub mod opt {
    pub const EARLY_WINDOW: u16 = 1344;
    pub const LATE_WINDOW: u16 = 1345;
    pub const MEAN_COUNT: u16 = 1348;
    pub const EARLY: u16 = 1350;
    pub const WRONG: u16 = 1351;
    pub const TWICE: u16 = 1352;
    pub const MISSED: u16 = 1353;
    pub const STATE: u16 = 1354;
    pub const MEAN: u16 = 1355;
    pub const CLEAR: u16 = 1356;
    /// The sequence's commands.
    pub const SET_STATE: u16 = 699;
    pub const ADD_BEAT: u16 = 700;
    pub const DEFAULT_MOVE: u16 = 701;
}

/// Task events the director raises.
pub mod event {
    pub const BEAT_PASSED: u32 = 52;
    pub const PRESS: u32 = 53;
    pub const SCORE: u32 = 54;
}

/// One beat to hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub dancer: Option<Handle>,
    /// The button, in `vehicles::button` bits (the disc's 16, 32, 64 and 128).
    pub button: u32,
    pub time: f32,
    /// Scored already.
    pub done: bool,
    /// Its time has passed (task event 52 raised).
    pub passed: bool,
}

/// The negative-score scale for difficulty help `h` [D (1.2, −0.6, 0), specs/sly2/vehicles-and-minigames.md 5].
pub fn negative_scale(h: f32) -> f32 {
    (1.2 - 0.6 * h).clamp(0.0, 1.0)
}

/// The scoring of a dance.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub beats: Vec<Beat>,
    pub recent: VecDeque<f32>,
    pub all: Vec<f32>,
    /// Early and late windows; the mistakes' scores (early, wrong, twice, missed); scores in the mean.
    pub window: [f32; 2],
    pub penalties: [f32; 4],
    pub mean_count: usize,
    /// The last beat that took a press (for "pressed twice").
    last_hit: Option<usize>,
}

impl Default for Score {
    fn default() -> Self {
        Score {
            beats: Vec::new(),
            recent: VecDeque::new(),
            all: Vec::new(),
            window: WINDOW,
            penalties: [EARLY, WRONG, TWICE, MISSED],
            mean_count: 8,
            last_hit: None,
        }
    }
}

/// What a step of the scoring produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepResult {
    pub scores: Vec<f32>,
    pub pressed: bool,
    pub passed: u32,
}

impl Score {
    fn push(&mut self, s: f32, help: f32) -> f32 {
        let s = if s < 0.0 { s * negative_scale(help) } else { s };
        let s = s.clamp(-1.5, 1.0);
        self.recent.push_back(s);
        while self.recent.len() > self.mean_count.max(1) {
            self.recent.pop_front();
        }
        self.all.push(s);
        s
    }

    /// The mean of the last scores (option 1355).
    pub fn mean(&self) -> f32 {
        if self.recent.is_empty() { 0.0 } else { self.recent.iter().sum::<f32>() / self.recent.len() as f32 }
    }

    /// The crowd's reaction level (0 to 2) [I].
    pub fn reaction(&self) -> u8 {
        let m = self.mean();
        if m < -0.5 {
            0
        } else if m < 0.5 {
            1
        } else {
            2
        }
    }

    /// Clears the scores (option 1356).
    pub fn clear(&mut self) {
        self.recent.clear();
        self.all.clear();
        self.last_hit = None;
    }

    /// Scores the presses of this frame at music time `t` and the beats that passed; returns the new scores.
    pub fn step(&mut self, pressed: u32, t: f32, help: f32) -> Vec<f32> {
        self.step_full(pressed, t, help).scores
    }

    /// [`Score::step`] with the presses and passed beats counted (for the director's task events).
    pub fn step_full(&mut self, pressed: u32, t: f32, help: f32) -> StepResult {
        let [early_w, late_w] = self.window;
        let [early, wrong, twice, missed] = self.penalties;
        let mut out = StepResult { pressed: pressed != 0, ..Default::default() };
        for b in self.beats.iter_mut().filter(|b| !b.passed && t >= b.time) {
            b.passed = true;
            out.passed += 1;
        }
        if pressed != 0 {
            let open = self
                .beats
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.done && t >= b.time - early_w && t <= b.time + late_w)
                .map(|(i, _)| i)
                .next();
            let s = match open {
                Some(i) => {
                    let b = &mut self.beats[i];
                    b.done = true;
                    self.last_hit = Some(i);
                    if pressed & b.button == 0 {
                        wrong
                    } else {
                        let d = t - b.time;
                        let u = if d < 0.0 { -d / early_w.max(1e-6) } else { d / late_w.max(1e-6) };
                        1.0 - u
                    }
                }
                None => match self.last_hit {
                    Some(i) if t <= self.beats[i].time + late_w => twice,
                    _ => early,
                },
            };
            let s = self.push(s, help);
            out.scores.push(s);
        }
        let gone: Vec<usize> =
            self.beats.iter().enumerate().filter(|(_, b)| !b.done && t > b.time + late_w).map(|(i, _)| i).collect();
        for i in gone {
            self.beats[i].done = true;
            let s = self.push(missed, help);
            out.scores.push(s);
        }
        out
    }
}

/// The dance director (class 121).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Director {
    pub state: u8,
    /// Music time since playing started.
    pub time: f32,
    pub score: Score,
    /// Dancers' default moves (dancer, animation object), from command 701.
    pub default_moves: Vec<(Option<Handle>, i32)>,
}

fn num(a: &Arg) -> Option<f32> {
    match *a {
        Arg::Int(i) => Some(i as f32),
        Arg::Float(f) => Some(f),
        Arg::Short(_, s) => Some(f32::from(s)),
        Arg::Bool(b) => Some(f32::from(u8::from(b))),
        _ => None,
    }
}

impl Director {
    pub fn new(world: &World, me: Handle) -> Director {
        let o = world.options(me);
        let f = |id: u16, d: f32| o.map_or(d, |o| o.f32_or(id, d));
        let score = Score {
            window: [f(opt::EARLY_WINDOW, WINDOW[0]), f(opt::LATE_WINDOW, WINDOW[1])],
            penalties: [f(opt::EARLY, EARLY), f(opt::WRONG, WRONG), f(opt::TWICE, TWICE), f(opt::MISSED, MISSED)],
            mean_count: o.and_then(|o| o.i32(opt::MEAN_COUNT)).map_or(8, |n| n.clamp(1, 64) as usize),
            ..Score::default()
        };
        Director { score, ..Director::default() }
    }

    /// Sets the director's state (0 off, 1 prompts, 2 playing; entering 2 starts the music time).
    pub fn set_state(&mut self, s: u8) {
        if s == 2 && self.state != 2 {
            self.time = 0.0;
        }
        self.state = s;
    }

    /// Adds a beat (dancer, button mask, music time).
    pub fn add_beat(&mut self, dancer: Option<Handle>, button: u32, time: f32) {
        self.score.beats.push(Beat { dancer, button, time, done: false, passed: false });
    }

    /// One of the sequence's commands (699, 700, 701), with the sequence's arguments.
    pub fn command(&mut self, world: &World, id: u16, args: &[Arg]) -> bool {
        let find =
            |a: Option<&Arg>| a.and_then(num).filter(|&d| d >= 0.0).and_then(|d| world.find_sly2_id(d as i32, None));
        match id {
            opt::SET_STATE => self.set_state(args.first().and_then(num).unwrap_or(0.0) as u8),
            opt::ADD_BEAT => {
                let time = args.first().and_then(num).unwrap_or(0.0);
                let button = args.get(1).and_then(num).unwrap_or(0.0) as u32;
                self.add_beat(find(args.get(3)), button, time);
            }
            opt::DEFAULT_MOVE => {
                let anim = args.first().and_then(num).unwrap_or(-1.0) as i32;
                self.default_moves.push((find(args.get(1)), anim));
            }
            _ => return false,
        }
        true
    }
}

/// The dance director of a Sly 2 world (the first class-121 object).
pub fn director(world: &World) -> Option<Handle> {
    world.handles().find(|&h| world.class(h) == Some(121))
}

/// The sequence options 699 to 701, forwarded from a sequence (class 114) to the director. Returns whether `id` is
/// one of them.
pub fn sequence_command(world: &mut World, id: u16, args: &[Arg]) -> bool {
    if !(opt::SET_STATE..=opt::DEFAULT_MOVE).contains(&id) {
        return false;
    }
    if let Some(d) = director(world) {
        world.run_action(d, id, args);
    }
    true
}

impl ClassBehavior for Director {
    fn ticks(&self) -> bool {
        self.state == 2
    }

    fn action(&mut self, world: &mut World, _me: Handle, id: u16, args: &[Arg]) -> bool {
        if id == opt::CLEAR {
            self.score.clear();
            return true;
        }
        let done = self.command(world, id, args);
        if done {
            world.mark_ticking_dirty();
        }
        done
    }

    fn option_changed(&mut self, world: &mut World, _me: Handle, id: u16, value: &OptionValue) {
        if id == opt::STATE {
            let v = match *value {
                OptionValue::Int(i) => i as f32,
                OptionValue::Short(s) => f32::from(s),
                OptionValue::Byte(b) => f32::from(b),
                OptionValue::Float(f) => f,
                _ => return,
            };
            self.set_state(v as u8);
            world.mark_ticking_dirty();
        }
    }

    fn get_option(&self, _world: &World, _me: Handle, id: u16) -> Option<OptionValue> {
        match id {
            opt::MEAN => Some(OptionValue::Float(self.score.mean())),
            opt::STATE => Some(OptionValue::Int(i32::from(self.state))),
            _ => None,
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.state != 2 {
            return;
        }
        self.time += cx.dt;
        // The dancers are the gang: the dance reads the frame's controller.
        let pad = Pad::from_frame(&cx.world.shared.input);
        let help = cx.world.shared.difficulty;
        let r = self.score.step_full(pad.pressed, self.time, help);
        for _ in 0..r.passed {
            task_event(cx.world, event::BEAT_PASSED);
        }
        if r.pressed {
            task_event(cx.world, event::PRESS);
        }
        for _ in &r.scores {
            task_event(cx.world, event::SCORE);
        }
    }
}
