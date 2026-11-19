//! The SPU volume envelope (audio.md 3.4): attack, decay, sustain and release, from the two hardware envelope words,
//! stepped once per output sample at 48 kHz.
//!
//! Rate rule (public SPU documentation): a phase has a mode (linear or exponential), a direction, a shift `s` and a
//! step `k`. The level changes by `k << max(0, 11 - s)` every `1 << max(0, s - 11)` samples. Exponential increase
//! waits four times as long once the level is above `0x6000`; exponential decrease scales the change by
//! `level / 0x8000`.

/// Highest envelope level.
pub const MAX_LEVEL: i32 = 0x7FFF;

/// Envelope phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Rising from 0 to the top.
    Attack,
    /// Falling (exponentially) to the sustain level.
    Decay,
    /// Held, rising or falling, until the voice is released.
    Sustain,
    /// Falling to 0 after the voice is released.
    Release,
    /// Finished: the voice is silent.
    Off,
}

/// The two envelope words as stored in a voice step or key region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdsrWords {
    /// Attack, decay and sustain level.
    pub w1: u16,
    /// Sustain rate and release.
    pub w2: u16,
}

#[derive(Clone, Copy, Debug)]
struct Rate {
    exponential: bool,
    decrease: bool,
    shift: u32,
    step: i32,
}

/// A running envelope.
#[derive(Clone, Copy, Debug)]
pub struct Envelope {
    words: AdsrWords,
    phase: Phase,
    level: i32,
    counter: u32,
}

impl Envelope {
    /// A new envelope at the start of its attack.
    pub const fn new(words: AdsrWords) -> Envelope {
        Envelope { words, phase: Phase::Attack, level: 0, counter: 0 }
    }

    /// An envelope that stays at full level until released, then stops at once (for streamed lines).
    pub const fn constant() -> Envelope {
        // Attack: linear, shift 0, step 7 (reaches the top in 3 samples); no decay; sustain held; release instant.
        Envelope::new(AdsrWords { w1: 0x00FF, w2: 0x1FC0 })
    }

    /// The current level, 0..=0x7FFF.
    pub fn level(&self) -> i32 {
        self.level
    }

    /// The current phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether the envelope has finished its release.
    pub fn is_off(&self) -> bool {
        self.phase == Phase::Off
    }

    /// Starts the release phase (key off).
    pub fn release(&mut self) {
        if self.phase != Phase::Off {
            self.phase = Phase::Release;
            self.counter = 0;
        }
    }

    /// Stops the envelope immediately.
    pub fn kill(&mut self) {
        self.phase = Phase::Off;
        self.level = 0;
    }

    fn sustain_level(&self) -> i32 {
        ((self.words.w1 & 0xF) as i32 + 1) * 0x800
    }

    fn rate(&self) -> Rate {
        let w1 = self.words.w1;
        let w2 = self.words.w2;
        match self.phase {
            Phase::Attack => Rate {
                exponential: w1 & 0x8000 != 0,
                decrease: false,
                shift: ((w1 >> 10) & 0x1F) as u32,
                step: 7 - ((w1 >> 8) & 3) as i32,
            },
            Phase::Decay => Rate { exponential: true, decrease: true, shift: ((w1 >> 4) & 0xF) as u32, step: -8 },
            Phase::Sustain => {
                let decrease = w2 & 0x4000 != 0;
                let v = ((w2 >> 6) & 3) as i32;
                Rate {
                    exponential: w2 & 0x8000 != 0,
                    decrease,
                    shift: ((w2 >> 8) & 0x1F) as u32,
                    step: if decrease { -8 + v } else { 7 - v },
                }
            }
            Phase::Release | Phase::Off => {
                Rate { exponential: w2 & 0x20 != 0, decrease: true, shift: (w2 & 0x1F) as u32, step: -8 }
            }
        }
    }

    /// Advances one sample and returns the new level.
    #[inline]
    pub fn tick(&mut self) -> i32 {
        match self.phase {
            Phase::Off => return 0,
            // Sustain with the slowest rates barely moves; handled by the general path.
            _ => self.step(),
        }
        self.level
    }

    fn step(&mut self) {
        let r = self.rate();
        let mut cycles = 1u32 << r.shift.saturating_sub(11);
        let mut delta = r.step << 11u32.saturating_sub(r.shift);
        if r.exponential && !r.decrease && self.level > 0x6000 {
            cycles *= 4;
        }
        if r.exponential && r.decrease {
            delta = (delta * self.level) >> 15;
        }
        self.counter += 1;
        if self.counter >= cycles {
            self.counter = 0;
            self.level = (self.level + delta).clamp(0, MAX_LEVEL);
        }
        match self.phase {
            Phase::Attack if self.level >= MAX_LEVEL => {
                self.phase = Phase::Decay;
                self.counter = 0;
            }
            Phase::Decay if self.level <= self.sustain_level() => {
                self.phase = Phase::Sustain;
                self.counter = 0;
            }
            Phase::Release if self.level <= 0 => {
                self.phase = Phase::Off;
                self.level = 0;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_until(e: &mut Envelope, phase: Phase, limit: usize) -> usize {
        for n in 0..limit {
            if e.phase() == phase {
                return n;
            }
            e.tick();
        }
        panic!("phase {phase:?} not reached in {limit} samples (at {:?}, level {})", e.phase(), e.level());
    }

    #[test]
    fn common_effect_setting() {
        // 0x80FF / 0x9FC0: exponential attack shift 0 step 7, decay ends at once, sustain held, release shift 0.
        let mut e = Envelope::new(AdsrWords { w1: 0x80FF, w2: 0x9FC0 });
        let n = run_until(&mut e, Phase::Sustain, 100);
        assert!(n < 10, "attack + decay took {n} samples");
        assert_eq!(e.level(), MAX_LEVEL);
        for _ in 0..48_000 {
            e.tick();
        }
        // Sustain increase at shift 31 changes once every 2^20 samples: still at the top.
        assert_eq!(e.level(), MAX_LEVEL);
        e.release();
        let n = run_until(&mut e, Phase::Off, 100);
        assert!(n <= 3, "release took {n}");
    }

    #[test]
    fn linear_release_rate() {
        // Release linear, shift 11: -8 per sample, 0x7FFF / 8 = 4096 samples.
        let mut e = Envelope::new(AdsrWords { w1: 0x80FF, w2: 0x9FCB });
        run_until(&mut e, Phase::Sustain, 100);
        e.release();
        let n = run_until(&mut e, Phase::Off, 10_000);
        assert_eq!(n, 4096);
    }

    #[test]
    fn linear_attack_timing_and_decay() {
        // Attack linear shift 13 step 7 (value 0): +7 every 4 samples -> 0x7FFF / 7 * 4.
        let w1 = (13 << 10) | (8 << 4) | 7;
        let mut e = Envelope::new(AdsrWords { w1, w2: 0x1FC0 });
        let n = run_until(&mut e, Phase::Decay, 100_000);
        assert_eq!(n, 4681 * 4);
        // Decay shift 8, exponential down to (7 + 1) * 0x800 = 0x4000.
        let n = run_until(&mut e, Phase::Sustain, 100_000);
        assert!(n > 0);
        assert!(e.level() <= 0x4000 && e.level() > 0x3F00, "level {}", e.level());
    }

    #[test]
    fn exponential_release_reaches_zero() {
        let mut e = Envelope::new(AdsrWords { w1: 0x00FF, w2: 0x1FC0 | 0x20 | 16 });
        run_until(&mut e, Phase::Sustain, 100);
        e.release();
        run_until(&mut e, Phase::Off, 1_000_000);
        assert_eq!(e.level(), 0);
        assert_eq!(e.tick(), 0);
    }
}
