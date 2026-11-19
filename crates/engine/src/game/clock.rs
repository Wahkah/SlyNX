//! The game and real clocks (clock-and-loop.md 1 to 3).
//!
//! Each frame the measured interval is clamped to [1/60 s, 1/30 s]: a fast frame still advances 1/60 s, a slow one
//! never more than 1/30 s, so the game slows down rather than skipping when it falls below 30 frames per second.
//! The game clock advances by that step times the time-scale factors (0 while paused); the real clock by the clamped
//! step, or by the game step when that is at least 1/60 s.

/// Shortest frame step, seconds.
pub const MIN_STEP: f32 = 1.0 / 60.0;
/// Longest frame step, seconds.
pub const MAX_STEP: f32 = 1.0 / 30.0;
/// The fixed step of the deterministic path.
pub const FIXED_STEP: f32 = 1.0 / 60.0;
/// The slow-motion power-up's factor.
pub const SLOW_MOTION: f32 = 0.25;

/// One frame's steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Steps {
    /// Game-clock step (0 while paused).
    pub game: f32,
    /// Real-clock step.
    pub real: f32,
}

/// The clocks.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    /// Game time: the sum of game steps since the world started (script operation 96).
    pub game_time: f32,
    /// Real time: the sum of real steps.
    pub real_time: f32,
    /// The pause screen's rate, 0 (paused) to 1.
    pub rate: f32,
    /// Power-up factor (slow motion 0.25, speed-up 1 + u × 0.5, 1 otherwise).
    pub power: f32,
    /// Development factor.
    pub debug: f32,
    /// The last frame's steps.
    pub last: Steps,
}

impl Default for Clock {
    fn default() -> Self {
        Clock::new()
    }
}

/// Clamps a measured frame interval to the frame step.
pub fn frame_step(measured: f32) -> f32 {
    if measured.is_nan() {
        return MIN_STEP;
    }
    measured.clamp(MIN_STEP, MAX_STEP)
}

impl Clock {
    /// Clocks at zero, running at rate 1 (the state when a world loads: power-up and rate factors reset to 1).
    pub fn new() -> Clock {
        Clock { game_time: 0.0, real_time: 0.0, rate: 1.0, power: 1.0, debug: 1.0, last: Steps::default() }
    }

    /// The game's time scale: enabled (0 when the rate is 0) × debug × power-up × rate.
    pub fn scale(&self) -> f32 {
        let enabled = if self.rate == 0.0 { 0.0 } else { 1.0 };
        enabled * self.debug * self.power * self.rate
    }

    /// Whether the game clock is stopped.
    pub fn paused(&self) -> bool {
        self.rate == 0.0
    }

    /// Advances both clocks for a measured frame interval and returns the steps.
    pub fn advance(&mut self, measured: f32) -> Steps {
        let step = frame_step(measured);
        let game = step * self.scale();
        let real = if game >= MIN_STEP { game } else { step };
        self.game_time += game;
        self.real_time += real;
        self.last = Steps { game, real };
        self.last
    }

    /// Music volume factor while the pause screen eases the rate (0.6 + 0.4 × rate).
    pub fn music_scale(&self) -> f32 {
        0.6 + 0.4 * self.rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    /// The vectors of clock-and-loop.md 5.
    #[test]
    fn spec_vectors() {
        let mut c = Clock::new();
        let s = c.advance(1.0 / 60.0);
        assert!(close(s.game, 0.0166667) && close(s.real, 0.0166667));
        let s = c.advance(1.0 / 120.0);
        assert!(close(s.game, 1.0 / 60.0) && close(s.real, 1.0 / 60.0));
        let s = c.advance(0.05);
        assert!(close(s.game, 1.0 / 30.0) && close(s.real, 1.0 / 30.0));
        c.rate = 0.0;
        let s = c.advance(0.02);
        assert_eq!(s.game, 0.0);
        assert!(close(s.real, 0.02));
        c.rate = 1.0;
        c.power = SLOW_MOTION;
        let s = c.advance(1.0 / 60.0);
        assert!(close(s.game, 0.0041667) && close(s.real, 1.0 / 60.0));
        assert!(close(c.music_scale(), 1.0));
        assert_eq!(frame_step(f32::NAN), MIN_STEP);
    }
}
