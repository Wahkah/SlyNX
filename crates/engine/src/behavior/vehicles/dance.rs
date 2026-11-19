//! The dance (Sly 2 classes 121 director and 68 dancers, specs/sly2/vehicles-and-minigames.md 5).
//!
//! The director's states: 0 off, 1 prompts shown (game paused), 2 playing. The dance script gives it beats (dancer,
//! button, music time); while playing, each beat is scored once: a press of its button within 0.4 s early or late
//! scores `1 − u` (u the time from the beat over the window), a wrong button −0.8, a second press −0.2, a press
//! before any window −1.5, a beat passed without a press −1.5. Negative scores are scaled by
//! `clamp(1.2 − 0.6·h, 0, 1)`, h the difficulty help. The mean of the last 8 scores (each kept within −1.5..1)
//! gives the crowd's reaction level 0 to 2 (below −0.5, below 0.5, else [I]); each score queues task event 54.
//!
//! The script commands that set the state, add beats and set a dancer's default move are not numbered in the spec,
//! so the director takes them through [`Director::set_state`] and [`Director::add_beat`] until they are.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::{Pad, task_event};
use crate::behavior::{ClassBehavior, Cx};
use crate::object::Handle;
use crate::world::World;

/// Timing window, early and late (director defaults).
pub const WINDOW: [f32; 2] = [0.4, 0.4];
/// Scores of the mistakes.
pub const WRONG: f32 = -0.8;
pub const TWICE: f32 = -0.2;
pub const EARLY: f32 = -1.5;
pub const MISSED: f32 = -1.5;

/// One beat to hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub dancer: Option<Handle>,
    /// The button, in `vehicles::button` bits.
    pub button: u32,
    pub time: f32,
    /// Scored already.
    pub done: bool,
}

/// The negative-score scale for difficulty help `h` [D (1.2, −0.6, 0) at 0x002BDBE0].
pub fn negative_scale(h: f32) -> f32 {
    (1.2 - 0.6 * h).clamp(0.0, 1.0)
}

/// The scoring of a dance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub beats: Vec<Beat>,
    pub recent: VecDeque<f32>,
    pub all: Vec<f32>,
    /// The last beat that took a press (for "pressed twice").
    last_hit: Option<usize>,
}

impl Score {
    fn push(&mut self, s: f32, help: f32) -> f32 {
        let s = if s < 0.0 { s * negative_scale(help) } else { s };
        let s = s.clamp(-1.5, 1.0);
        self.recent.push_back(s);
        if self.recent.len() > 8 {
            self.recent.pop_front();
        }
        self.all.push(s);
        s
    }

    /// The mean of the last 8 scores.
    pub fn mean(&self) -> f32 {
        if self.recent.is_empty() { 0.0 } else { self.recent.iter().sum::<f32>() / self.recent.len() as f32 }
    }

    /// The crowd's reaction level (0 to 2).
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

    /// Scores the presses of this frame at music time `t` and the beats that passed; returns the new scores.
    pub fn step(&mut self, pressed: u32, t: f32, help: f32) -> Vec<f32> {
        let mut out = Vec::new();
        if pressed != 0 {
            let open = self
                .beats
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.done && t >= b.time - WINDOW[0] && t <= b.time + WINDOW[1])
                .map(|(i, _)| i)
                .next();
            let s = match open {
                Some(i) => {
                    let b = &mut self.beats[i];
                    b.done = true;
                    self.last_hit = Some(i);
                    if pressed & b.button == 0 {
                        WRONG
                    } else {
                        let d = t - b.time;
                        let u = if d < 0.0 { -d / WINDOW[0] } else { d / WINDOW[1] };
                        1.0 - u
                    }
                }
                None => match self.last_hit {
                    Some(i) if t <= self.beats[i].time + WINDOW[1] => TWICE,
                    _ => EARLY,
                },
            };
            out.push(self.push(s, help));
        }
        let missed: Vec<usize> =
            self.beats.iter().enumerate().filter(|(_, b)| !b.done && t > b.time + WINDOW[1]).map(|(i, _)| i).collect();
        for i in missed {
            self.beats[i].done = true;
            out.push(self.push(MISSED, help));
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
}

impl Director {
    pub fn new(_world: &World, _me: Handle) -> Director {
        Director::default()
    }

    /// Sets the director's state (0 off, 1 prompts, 2 playing; entering 2 starts the music time).
    pub fn set_state(&mut self, s: u8) {
        if s == 2 && self.state != 2 {
            self.time = 0.0;
        }
        self.state = s;
    }

    /// Adds a beat (dancer, button, music time).
    pub fn add_beat(&mut self, dancer: Option<Handle>, button: u32, time: f32) {
        self.score.beats.push(Beat { dancer, button, time, done: false });
    }
}

impl ClassBehavior for Director {
    fn ticks(&self) -> bool {
        self.state == 2
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.state != 2 {
            return;
        }
        self.time += cx.dt;
        // The dancers are the gang: the dance reads the frame's controller.
        let pad = Pad::from_frame(&cx.world.shared.input);
        let help = cx.world.shared.difficulty;
        for _ in self.score.step(pad.pressed, self.time, help) {
            task_event(cx.world, 54);
        }
    }
}
