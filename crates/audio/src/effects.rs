//! Playing an effect sound through its steps (audio.md 3.2): one [`Instance`] per playing sound, stepped once per
//! scheduler tick on the audio thread. Nothing here allocates.

use core::sync::atomic::Ordering;

use crate::bank::{EffectBank, StepKind, ValueSource};
use crate::envelope::Envelope;
use crate::mixer::{Mixer, Owner, Pool, Source, VoiceStart};
use crate::rng::Rng;
use crate::spatial::Mix;

/// Most sound instances playing at once.
pub const MAX_INSTANCES: usize = 64;
/// Number of global registers that voice volumes and pans can read.
pub const GLOBAL_REGISTERS: usize = 16;
const LOOP_DEPTH: usize = 4;
const CHOICE_DEPTH: usize = 4;
/// Most steps one instance runs in one tick (guards against loops without delays). The caller gives each instance
/// one budget per tick, shared by every [`run`] call for it in that tick.
pub const STEPS_PER_TICK: usize = 64;

/// What an instance is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Slot unused.
    Free,
    /// Running its steps.
    Running,
    /// Steps finished or stopped; waiting for its voices to fall silent.
    Draining,
}

/// One playing sound.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// State.
    pub state: State,
    /// The game's handle (shared with child sounds).
    pub handle: u32,
    /// Bank slot.
    pub bank: u8,
    /// Sound index in the bank.
    pub sound: u16,
    next: usize,
    end: usize,
    arrived: bool,
    wait: i32,
    extra: i32,
    waiting_voices: bool,
    loops: [usize; LOOP_DEPTH],
    loop_depth: usize,
    choices: [(usize, usize); CHOICE_DEPTH],
    choice_depth: usize,
    /// Pitch bend as a fraction (-1..1) of the voices' bend ranges.
    pub bend: f32,
    /// The four sound registers.
    pub registers: [i32; 4],
    /// Game-side volume, pan and pitch.
    pub mix: Mix,
    /// Sound volume / 127.
    pub volume: f32,
    /// Sound pan in degrees.
    pub pan: f32,
    /// Mix group.
    pub group: u8,
    /// Active voices (counted at the start of each tick, plus the ones started since).
    pub voices: u16,
    /// The scheduler tick this instance last ran in (so it runs at most once per tick).
    pub ran_tick: u64,
    /// Index of the sound's stop marker (kind 43), if it has one.
    pub stop_marker: Option<usize>,
    /// A stop request is running the steps after the stop marker.
    pub stopping: bool,
}

impl Instance {
    /// An unused slot.
    pub const FREE: Instance = Instance {
        state: State::Free,
        handle: 0,
        bank: 0,
        sound: 0,
        next: 0,
        end: 0,
        arrived: false,
        wait: 0,
        extra: 0,
        waiting_voices: false,
        loops: [0; LOOP_DEPTH],
        loop_depth: 0,
        choices: [(0, 0); CHOICE_DEPTH],
        choice_depth: 0,
        bend: 0.0,
        registers: [0; 4],
        mix: Mix::UNITY,
        volume: 1.0,
        pan: 0.0,
        group: 0,
        voices: 0,
        ran_tick: u64::MAX,
        stop_marker: None,
        stopping: false,
    };

    /// A new instance of `sound` in `bank` (slot `bank_slot`), or `None` when the sound does not exist.
    pub fn new(bank: &EffectBank, bank_slot: u8, sound: u16, handle: u32, mix: Mix) -> Option<Instance> {
        let rec = bank.sounds.get(sound as usize)?;
        let steps = rec.steps();
        let stop_marker =
            bank.steps[steps.clone()].iter().position(|s| s.kind == StepKind::StopMarker).map(|i| steps.start + i);
        Some(Instance {
            stop_marker,
            state: State::Running,
            handle,
            bank: bank_slot,
            sound,
            next: steps.start,
            end: steps.end,
            mix,
            volume: rec.volume.max(0) as f32 / 127.0,
            pan: rec.pan as f32,
            group: rec.group.clamp(0, 3) as u8,
            ..Instance::FREE
        })
    }
}

impl Instance {
    /// A stop request (sly2/audio.md 2, kind 43): a sound with a stop marker not yet reached runs the steps after
    /// it and returns `true`; otherwise returns `false` and the caller ends the sound as Sly 1 does.
    pub fn request_stop(&mut self) -> bool {
        let Some(m) = self.stop_marker else { return false };
        if self.stopping || self.state == State::Free || m + 1 >= self.end {
            return false;
        }
        self.stopping = true;
        self.state = State::Running;
        self.next = m + 1;
        self.arrived = false;
        self.wait = 0;
        self.extra = 0;
        self.waiting_voices = false;
        self.loop_depth = 0;
        self.choice_depth = 0;
        true
    }
}

/// Pan in degrees for a 0..127 register value (the controller-10 rule of audio.md 5.3).
pub fn register_pan(v: i32) -> f32 {
    let v = v.clamp(0, 127);
    if v >= 64 { (v - 64) as f32 * 90.0 / 63.0 } else { 270.0 + v as f32 * 89.0 / 63.0 }
}

/// Everything a step needs besides the instance itself.
pub struct StepContext<'a> {
    /// The instance's bank.
    pub bank: &'a EffectBank,
    /// The voice pool.
    pub mixer: &'a mut Mixer,
    /// Random numbers.
    pub rng: &'a mut Rng,
    /// Global registers (steps 30 and 31 with a negative register write them).
    pub globals: &'a mut [i32; GLOBAL_REGISTERS],
}

/// Writes a sound register, or a global one for a negative register number (`-1` is global register 0).
fn set_register(inst: &mut Instance, ctx: &mut StepContext<'_>, register: i16, value: i32) {
    if register < 0 {
        if let Some(g) = ctx.globals.get_mut((-(register as i32) - 1) as usize) {
            *g = value;
        }
    } else {
        inst.registers[register as usize & 3] = value;
    }
}

fn resolve(src: ValueSource, inst: &Instance, ctx: &mut StepContext<'_>) -> i32 {
    match src {
        ValueSource::Value(v) => v,
        ValueSource::SoundRegister(r) => inst.registers[r as usize & 3],
        ValueSource::Random => ctx.rng.range_i32(0, 127),
        ValueSource::GlobalRegister(g) => ctx.globals.get(g as usize).copied().unwrap_or(0),
    }
}

/// A child sound to start (step kind 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChildStart {
    /// Sound index in the same bank.
    pub sound: u16,
    /// Volume factor 0..1 for the child.
    pub volume: f32,
    /// Pan added to the child's own, degrees.
    pub pan: f32,
}

/// Runs instance `k` for one tick, spending at most `budget` steps (decremented). Returns a child sound to start,
/// if a child step ran; the caller starts it (it needs a free slot) and calls again with the same budget until
/// `None`.
pub fn run(instances: &mut [Instance], k: usize, ctx: &mut StepContext<'_>, budget: &mut usize) -> Option<ChildStart> {
    while *budget > 0 {
        *budget -= 1;
        let inst = &mut instances[k];
        if inst.state != State::Running {
            return None;
        }
        if inst.waiting_voices {
            if inst.voices > 0 {
                return None;
            }
            inst.waiting_voices = false;
        }
        while inst.choice_depth > 0 && inst.next >= inst.choices[inst.choice_depth - 1].0 {
            inst.choice_depth -= 1;
            inst.next = inst.choices[inst.choice_depth].1;
            inst.arrived = false;
        }
        if inst.next >= inst.end {
            inst.state = State::Draining;
            return None;
        }
        let index = inst.next;
        let step = ctx.bank.steps[index];
        if !inst.arrived {
            inst.arrived = true;
            inst.wait = step.delay.max(0).saturating_add(inst.extra);
            inst.extra = 0;
        }
        if inst.wait > 0 {
            inst.wait -= 1;
            return None;
        }
        inst.next += 1;
        inst.arrived = false;
        match step.kind {
            StepKind::Voice(vp) | StepKind::VoiceLegacy(vp) => {
                let volume = resolve(ValueSource::volume(vp.volume), inst, ctx).clamp(0, 127);
                let pan = match ValueSource::pan(vp.pan) {
                    ValueSource::Value(v) => v as f32,
                    other => register_pan(resolve(other, inst, ctx)),
                };
                let inst = &mut instances[k];
                if let Some(vi) = ctx.mixer.alloc(Pool::Effects, vp.priority) {
                    ctx.mixer.start(
                        vi,
                        VoiceStart {
                            source: Source::Bank(inst.bank),
                            offset: vp.sample_offset as usize,
                            envelope: Envelope::new(vp.adsr),
                            owner: Owner::Effect { instance: k as u8 },
                            priority: vp.priority,
                            group: inst.group,
                            reverb: inst.group == 0 || inst.group == 3,
                            volume: volume as f32 / 127.0,
                            pan,
                            semitones: vp.semitones_for(60),
                            rate_scale: 1.0,
                            bend_down: vp.bend_range_down(),
                            bend_up: vp.bend_range_up(),
                            wrap: false,
                        },
                    );
                    inst.voices += 1;
                }
            }
            StepKind::Child { sound, volume, pan } => {
                if sound >= 0 {
                    let v = resolve(ValueSource::volume(volume.clamp(-128, 127) as i8), inst, ctx).clamp(0, 127);
                    let pan = match ValueSource::pan(pan) {
                        ValueSource::Value(p) => p as f32,
                        ValueSource::Random => ctx.rng.range_i32(-90, 90) as f32,
                        other => register_pan(resolve(other, inst, ctx)),
                    };
                    return Some(ChildStart { sound: sound as u16, volume: v as f32 / 127.0, pan });
                }
            }
            StepKind::LoopStart => {
                if inst.loop_depth == LOOP_DEPTH {
                    inst.loop_depth -= 1;
                }
                inst.loops[inst.loop_depth] = inst.next;
                inst.loop_depth += 1;
            }
            StepKind::LoopEnd => {
                if inst.loop_depth > 0 {
                    inst.next = inst.loops[inst.loop_depth - 1];
                }
            }
            StepKind::Stop => {
                ctx.mixer.release_where(|v| v.p.owner == Owner::Effect { instance: k as u8 });
                inst.state = State::Draining;
                return None;
            }
            StepKind::RandomChoice { choices, per_choice, .. } => {
                let n = choices.max(1) as usize;
                let per = per_choice.max(0) as usize;
                let first = inst.next;
                let after = (first + n * per).min(inst.end);
                let last = ctx.bank.choice_last[index].load(Ordering::Relaxed);
                let mut c = ctx.rng.range_i32(0, n as i32 - 1);
                if n > 1 && c as i16 == last {
                    // Pick among the others instead.
                    c = (c + 1 + ctx.rng.range_i32(0, n as i32 - 2)) % n as i32;
                }
                ctx.bank.choice_last[index].store(c as i16, Ordering::Relaxed);
                let start = first + c as usize * per;
                if per == 0 || start >= after {
                    inst.next = after;
                } else {
                    if inst.choice_depth == CHOICE_DEPTH {
                        inst.choice_depth -= 1;
                    }
                    inst.choices[inst.choice_depth] = ((start + per).min(after), after);
                    inst.choice_depth += 1;
                    inst.next = start;
                }
            }
            StepKind::RandomDelay(max) => {
                inst.extra = inst.extra.saturating_add(ctx.rng.range_i32(0, max.max(0)));
            }
            StepKind::RandomBend(amount) => {
                let a = (amount as i32).abs();
                inst.bend = (ctx.rng.range_i32(-a, a) as f32 / 100.0).clamp(-1.0, 1.0);
            }
            StepKind::AddBend(amount) => {
                inst.bend = (inst.bend + amount as f32 / 100.0).clamp(-1.0, 1.0);
            }
            StepKind::RandomRegister { register, low, high } => {
                let v = ctx.rng.range_i32(low as i32, high as i32);
                set_register(inst, ctx, register, v);
            }
            StepKind::SetRegister { register, value } => set_register(inst, ctx, register, value as i32),
            StepKind::AddBendScaled(amount) => {
                inst.bend = (inst.bend + amount as f32 / 127.0).clamp(-1.0, 1.0);
            }
            StepKind::GroupCycle { groups, per_group, .. } => {
                // The groups play in turn: this pass plays group `counter`, which then advances.
                let n = groups.max(1) as i32;
                let per = per_group.max(0) as usize;
                let first = inst.next;
                let after = (first + n as usize * per).min(inst.end);
                let c = (ctx.bank.choice_last[index].load(Ordering::Relaxed) as i32).rem_euclid(n);
                ctx.bank.choice_last[index].store(((c + 1) % n) as i16, Ordering::Relaxed);
                let start = first + c as usize * per;
                if per == 0 || start >= after {
                    inst.next = after;
                } else {
                    if inst.choice_depth == CHOICE_DEPTH {
                        inst.choice_depth -= 1;
                    }
                    inst.choices[inst.choice_depth] = ((start + per).min(after), after);
                    inst.choice_depth += 1;
                    inst.next = start;
                }
            }
            StepKind::LoopBreak => {
                // Jump past the next loop end of the sound (no effect without one).
                let end = inst.end;
                if let Some(off) = ctx.bank.steps[inst.next..end].iter().position(|s| s.kind == StepKind::LoopEnd) {
                    inst.next += off + 1;
                    inst.loop_depth = inst.loop_depth.saturating_sub(1);
                }
            }
            StepKind::StopMarker => {
                // Normal playback ends here; the steps after it run only on a stop request.
                inst.state = State::Draining;
                return None;
            }
            StepKind::WaitVoices => {
                if inst.voices > 0 {
                    inst.waiting_voices = true;
                    return None;
                }
            }
            StepKind::Lfo(_) | StepKind::Nop | StepKind::Unknown(_) | StepKind::Opaque { .. } => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adpcm::FLAG_END;
    use crate::adpcm::tests::block;
    use crate::bank::tests::{build_bank, voice_data};
    use alloc::vec;
    use alloc::vec::Vec;

    fn bank(steps: Vec<(u32, i32, Vec<u8>)>, flags: u16) -> EffectBank {
        let samples = block(12, 0, FLAG_END, &[1; 28]);
        EffectBank::parse(build_bank(&[(127, 0, flags, steps)], &samples)).unwrap()
    }

    fn voice() -> (u32, i32, Vec<u8>) {
        (1, 0, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0))
    }

    /// Ticks the instance `n` times, recounting voices like the renderer does; returns the tick of each voice start.
    fn ticks(b: &EffectBank, n: usize) -> (Vec<usize>, Instance) {
        let mut inst = [Instance::new(b, 0, 0, 1, Mix::UNITY).unwrap()];
        let mut mixer = Mixer::new(8, 0, 0);
        let mut rng = Rng::new(7);
        let mut globals = [0; GLOBAL_REGISTERS];
        let mut starts = vec![];
        let mut seen = 0u64;
        for t in 0..n {
            for v in mixer.voices.iter_mut() {
                v.kill();
            }
            inst[0].voices = 0;
            let mut ctx = StepContext { bank: b, mixer: &mut mixer, rng: &mut rng, globals: &mut globals };
            let mut budget = STEPS_PER_TICK;
            while run(&mut inst, 0, &mut ctx, &mut budget).is_some() {}
            for v in &mixer.voices {
                if v.active && v.serial > seen {
                    seen = v.serial;
                    starts.push(t);
                }
            }
        }
        (starts, inst[0])
    }

    #[test]
    fn delays_count_ticks() {
        let b = bank(vec![voice(), (1, 3, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0)), (20, 2, vec![])], 0);
        let (starts, inst) = ticks(&b, 10);
        assert_eq!(starts, [0, 3]);
        assert_eq!(inst.state, State::Draining);
    }

    #[test]
    fn loop_repeats_forever() {
        let b = bank(vec![(21, 0, vec![]), (1, 4, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0)), (22, 0, vec![])], 1);
        let (starts, inst) = ticks(&b, 21);
        assert_eq!(starts, [4, 8, 12, 16, 20]);
        assert_eq!(inst.state, State::Running);
    }

    #[test]
    fn random_choice_plays_one_group() {
        let mut steps = vec![(25, 0, [3i16, 2, -1].iter().flat_map(|v| v.to_le_bytes()).collect())];
        for _ in 0..3 {
            steps.push(voice());
            steps.push((20, 0, vec![]));
        }
        steps.push((1, 1, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0)));
        let b = bank(steps, 0);
        let (starts, _) = ticks(&b, 5);
        // One voice from the chosen group at tick 0, then the final step one tick later.
        assert_eq!(starts, [0, 1]);
        // Consecutive choices differ.
        let mut last = -1;
        for _ in 0..20 {
            ticks(&b, 1);
            let c = b.choice_last[0].load(Ordering::Relaxed);
            assert!((0..3).contains(&c));
            assert_ne!(c, last);
            last = c;
        }
    }

    #[test]
    fn stop_and_wait() {
        let b = bank(vec![voice(), (38, 0, vec![]), voice(), (24, 0, vec![]), voice()], 0);
        let mut inst = [Instance::new(&b, 0, 0, 1, Mix::UNITY).unwrap()];
        let mut mixer = Mixer::new(8, 0, 0);
        let mut rng = Rng::new(1);
        let mut globals = [0; GLOBAL_REGISTERS];
        let mut ctx = StepContext { bank: &b, mixer: &mut mixer, rng: &mut rng, globals: &mut globals };
        run(&mut inst, 0, &mut ctx, &mut STEPS_PER_TICK.clone());
        // The first voice plays; the wait step holds the rest.
        assert_eq!(inst[0].voices, 1);
        assert_eq!(inst[0].state, State::Running);
        inst[0].voices = 0; // its voice finished
        run(&mut inst, 0, &mut ctx, &mut STEPS_PER_TICK.clone());
        assert_eq!(inst[0].voices, 1);
        assert_eq!(inst[0].state, State::Draining);
        assert_eq!(mixer.voices.iter().filter(|v| v.active && v.released).count(), 2);
    }

    #[test]
    fn registers_feed_voice_volume() {
        let mut regstep = vec![0u8; 6];
        regstep[0..2].copy_from_slice(&1i16.to_le_bytes());
        regstep[2..4].copy_from_slice(&50i16.to_le_bytes());
        regstep[4..6].copy_from_slice(&50i16.to_le_bytes());
        let b = bank(vec![(31, 0, regstep), (1, 0, voice_data(-2, -60, 0, 0x80FF, 0x9FC0, 0))], 0);
        let mut inst = [Instance::new(&b, 0, 0, 1, Mix::UNITY).unwrap()];
        let mut mixer = Mixer::new(8, 0, 0);
        let mut rng = Rng::new(1);
        let mut globals = [0; GLOBAL_REGISTERS];
        let mut ctx = StepContext { bank: &b, mixer: &mut mixer, rng: &mut rng, globals: &mut globals };
        run(&mut inst, 0, &mut ctx, &mut STEPS_PER_TICK.clone());
        let v = mixer.voices.iter().find(|v| v.active).unwrap();
        assert!((v.p.volume - 50.0 / 127.0).abs() < 1e-6);
        assert_eq!(register_pan(127), 90.0);
        assert_eq!(register_pan(64), 0.0);
        assert_eq!(register_pan(0), 270.0);
    }
}
