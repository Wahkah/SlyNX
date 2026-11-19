//! Effect banks (audio.md 3): the bank file, sound records, steps and the voice parameters.

use alloc::vec::Vec;
use core::ops::Range;
use core::sync::atomic::AtomicI16;

use crate::Error;
use crate::bytes::{i8_at, i16_at, i32_at, magic, u16_at, u32_at};
use crate::envelope::AdsrWords;

/// Size of a sound record.
pub const SOUND_RECORD_SIZE: usize = 12;
/// Size of a step (version 1, Sly 1).
pub const STEP_SIZE: usize = 40;
/// Size of a step in version 3 (Sly 2).
pub const STEP_SIZE_V3: usize = 8;
/// Size of the voice parameters (voice step data and music key regions).
pub const VOICE_PARAMS_SIZE: usize = 24;

/// Sound flag: keeps playing until stopped.
pub const SOUND_CONTINUOUS: u16 = 1;

/// The voice parameters shared by effect voice steps and music key regions (audio.md 3.2 and 5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoiceParams {
    /// Priority (higher keeps its voice).
    pub priority: i8,
    /// Volume 0..127; negative values name a register (see [`ValueSource`]).
    pub volume: i8,
    /// Centre note as stored (negative: tuned for the 48 kHz chip).
    pub centre: i8,
    /// Fine tuning in 1/128 semitone.
    pub fine: u8,
    /// Pan in degrees; small negative values name a register.
    pub pan: i16,
    /// Lowest note of the range (music).
    pub low_note: i8,
    /// Highest note of the range (music).
    pub high_note: i8,
    /// Pitch-bend range down in semitones, as stored (0 means 2).
    pub bend_down: i8,
    /// Pitch-bend range up in semitones, as stored (0 means 2).
    pub bend_up: i8,
    /// Envelope words.
    pub adsr: AdsrWords,
    /// Flags (music: bit 7 = no sample).
    pub flags: u16,
    /// Sample offset in bytes from the start of the sample section.
    pub sample_offset: u32,
}

impl VoiceParams {
    /// Parses the 24 bytes at `off`.
    pub fn parse(b: &[u8], off: usize) -> Result<VoiceParams, Error> {
        if b.len() < off + VOICE_PARAMS_SIZE {
            return Err(Error::Truncated { what: "voice parameters", needed: off + VOICE_PARAMS_SIZE, len: b.len() });
        }
        Ok(VoiceParams {
            priority: i8_at(b, off)?,
            volume: i8_at(b, off + 1)?,
            centre: i8_at(b, off + 2)?,
            fine: b[off + 3],
            pan: i16_at(b, off + 4)?,
            low_note: i8_at(b, off + 6)?,
            high_note: i8_at(b, off + 7)?,
            bend_down: i8_at(b, off + 8)?,
            bend_up: i8_at(b, off + 9)?,
            adsr: AdsrWords { w1: u16_at(b, off + 10)?, w2: u16_at(b, off + 12)? },
            flags: u16_at(b, off + 14)?,
            sample_offset: u32_at(b, off + 16)?,
        })
    }

    /// Bend range down in semitones (stored 0 means 2).
    pub fn bend_range_down(&self) -> f32 {
        if self.bend_down == 0 { 2.0 } else { self.bend_down as f32 }
    }

    /// Bend range up in semitones (stored 0 means 2).
    pub fn bend_range_up(&self) -> f32 {
        if self.bend_up == 0 { 2.0 } else { self.bend_up as f32 }
    }

    /// Semitones from the chip's 48 kHz rate to play `note` (audio.md 3.3 and 5.2): `note - |centre| + fine / 128`.
    pub fn semitones_for(&self, note: i32) -> f32 {
        (note - (self.centre as i32).abs()) as f32 + self.fine as f32 / 128.0
    }
}

/// Where a voice's volume or pan comes from (audio.md 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueSource {
    /// A constant.
    Value(i32),
    /// Sound register 0..3 (stored -1..-4).
    SoundRegister(u8),
    /// A random value (stored -5).
    Random,
    /// Global register 0.. (stored -6 and below).
    GlobalRegister(u8),
}

impl ValueSource {
    /// Interprets a stored volume byte.
    pub fn volume(v: i8) -> ValueSource {
        Self::from_stored(v as i32)
    }

    /// Interprets a stored pan value: -1..-16 name registers like volumes do; other values are degrees.
    pub fn pan(v: i16) -> ValueSource {
        if (-16..0).contains(&v) { Self::from_stored(v as i32) } else { ValueSource::Value(v as i32) }
    }

    fn from_stored(v: i32) -> ValueSource {
        match v {
            0.. => ValueSource::Value(v),
            -4..=-1 => ValueSource::SoundRegister((-v - 1) as u8),
            -5 => ValueSource::Random,
            _ => ValueSource::GlobalRegister((-v - 6).min(255) as u8),
        }
    }
}

/// One sound record (audio.md 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRecord {
    /// Volume 0..127.
    pub volume: i8,
    /// Mix group 0..3.
    pub group: i8,
    /// Pan in degrees.
    pub pan: i16,
    /// Number of steps.
    pub step_count: u8,
    /// Instance limit (0 in every bank).
    pub instance_limit: i8,
    /// Flags ([`SOUND_CONTINUOUS`]).
    pub flags: u16,
    /// Index of the first step in [`EffectBank::steps`].
    pub first_step: usize,
}

impl SoundRecord {
    /// Whether the sound keeps playing until stopped.
    pub fn continuous(&self) -> bool {
        self.flags & SOUND_CONTINUOUS != 0
    }

    /// The range of the sound's steps in [`EffectBank::steps`].
    pub fn steps(&self) -> Range<usize> {
        self.first_step..self.first_step + self.step_count as usize
    }
}

/// What a step does (audio.md 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    /// 1: start one sample.
    Voice(VoiceParams),
    /// 4: low-frequency oscillator set-up (layout unconfirmed; kept raw, not played).
    Lfo([u8; 32]),
    /// 5: start another sound of the same bank.
    Child {
        /// Sound index in the bank (`s16` at data offset 12).
        sound: i16,
        /// Volume for the child, 0..127, or a register reference like a voice volume (`s16` at data offset 0).
        volume: i16,
        /// Pan for the child in degrees, or a register reference (`s16` at data offset 4; 0 or -5 = random).
        pan: i16,
    },
    /// 20: nothing.
    Nop,
    /// 21: loop start.
    LoopStart,
    /// 22: loop end (back to the matching loop start).
    LoopEnd,
    /// 24: release all voices and end the sound.
    Stop,
    /// 25: pick one of the following groups of steps.
    RandomChoice {
        /// Number of groups.
        choices: i16,
        /// Steps per group.
        per_choice: i16,
        /// Last choice made (initial value as stored).
        last: i16,
    },
    /// 26: random extra delay before the next step, 0..=max ticks.
    RandomDelay(i32),
    /// 27: random pitch bend, in percent of the bend range.
    RandomBend(i16),
    /// 29: add to the pitch bend (percent of the bend range).
    AddBend(i16),
    /// 31: set a sound register to a random value in a range.
    RandomRegister {
        /// Register number.
        register: i16,
        /// Lowest value.
        low: i16,
        /// Highest value.
        high: i16,
    },
    /// 38: wait until every voice of the sound has finished.
    WaitVoices,
    /// Any other kind (none on the Sly 1 disc): skipped.
    Unknown(u32),
    /// 9 (Sly 2): a voice step whose data uses a non-negative centre note (the older 44.1 kHz tuning convention).
    VoiceLegacy(VoiceParams),
    /// 29 in version 3: add `amount / 127` of the bend range to the pitch bend.
    AddBendScaled(i8),
    /// 30 (Sly 2): set a register (negative: a global register) to a value.
    SetRegister {
        /// Register.
        register: i16,
        /// Value.
        value: i16,
    },
    /// 39 (Sly 2): play the following groups of steps in turn, one per pass.
    GroupCycle {
        /// Number of groups.
        groups: i16,
        /// Steps per group.
        per_group: i16,
        /// The pass counter as stored (run-time state; 0 on the disc).
        pass: i16,
    },
    /// 23 (Sly 2): leave the current loop (jump past the next loop-end step).
    LoopBreak,
    /// 43 (Sly 2): normal playback ends here; a stop request runs the steps after it.
    StopMarker,
    /// A version-3 kind with no known action: kept with its 24-bit argument, skipped.
    Opaque {
        /// Kind number.
        kind: u32,
        /// The step word's low 24 bits (an inline value or an offset into the step-data area).
        arg: u32,
    },
}

/// A step: what it does and the delay before it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Delay before the step, in scheduler ticks (1/240 s).
    pub delay: i32,
    /// The kind and its data.
    pub kind: StepKind,
}

impl Step {
    /// Parses the 40-byte step at `off`.
    pub fn parse(b: &[u8], off: usize) -> Result<Step, Error> {
        if b.len() < off + STEP_SIZE {
            return Err(Error::Truncated { what: "step", needed: off + STEP_SIZE, len: b.len() });
        }
        let kind = u32_at(b, off)?;
        let delay = i32_at(b, off + 4)?;
        let d = off + 8;
        let kind = match kind {
            1 => StepKind::Voice(VoiceParams::parse(b, d)?),
            4 => {
                let mut raw = [0u8; 32];
                raw.copy_from_slice(&b[d..d + 32]);
                StepKind::Lfo(raw)
            }
            5 => StepKind::Child { sound: i16_at(b, d + 12)?, volume: i16_at(b, d)?, pan: i16_at(b, d + 4)? },
            20 => StepKind::Nop,
            21 => StepKind::LoopStart,
            22 => StepKind::LoopEnd,
            24 => StepKind::Stop,
            25 => StepKind::RandomChoice {
                choices: i16_at(b, d)?,
                per_choice: i16_at(b, d + 2)?,
                last: i16_at(b, d + 4)?,
            },
            26 => StepKind::RandomDelay(i32_at(b, d)?),
            27 => StepKind::RandomBend(i16_at(b, d)?),
            29 => StepKind::AddBend(i16_at(b, d)?),
            31 => StepKind::RandomRegister { register: i16_at(b, d)?, low: i16_at(b, d + 2)?, high: i16_at(b, d + 4)? },
            38 => StepKind::WaitVoices,
            k => StepKind::Unknown(k),
        };
        Ok(Step { delay, kind })
    }

    /// Parses an 8-byte version-3 step at `off` (sly2/audio.md 2). The step word's top byte is the kind; its low
    /// 24 bits are an offset into the step-data area at `area` (kinds 1, 4, 5 and 9) or an inline argument, read
    /// as three signed bytes `a0`, `a1`, `a2` (kind 26 uses all 24 bits).
    pub fn parse_v3(b: &[u8], off: usize, area: usize) -> Result<Step, Error> {
        let w0 = u32_at(b, off)?;
        let delay = i32_at(b, off + 4)?;
        let kind = w0 >> 24;
        let arg = w0 & 0x00FF_FFFF;
        let at = area.checked_add(arg as usize).ok_or(Error::Invalid("step data offset"))?;
        let sb = |shift: u32| ((arg >> shift) & 0xFF) as u8 as i8 as i16;
        let kind = match kind {
            1 => StepKind::Voice(VoiceParams::parse(b, at)?),
            // LFO data near the end of the area may be shorter than 32 bytes; the rest reads as zero.
            4 => match b.get(at..) {
                Some(rest) if !rest.is_empty() => {
                    let mut r = [0u8; 32];
                    let n = rest.len().min(32);
                    r[..n].copy_from_slice(&rest[..n]);
                    StepKind::Lfo(r)
                }
                _ => StepKind::Opaque { kind: 4, arg },
            },
            5 => StepKind::Child { sound: i16_at(b, at + 12)?, volume: i16_at(b, at)?, pan: i16_at(b, at + 4)? },
            20 => StepKind::Nop,
            21 => StepKind::LoopStart,
            22 => StepKind::LoopEnd,
            24 => StepKind::Stop,
            9 => StepKind::VoiceLegacy(VoiceParams::parse(b, at)?),
            23 => StepKind::LoopBreak,
            25 => StepKind::RandomChoice { choices: sb(0), per_choice: sb(8), last: sb(16) },
            26 => StepKind::RandomDelay(arg as i32),
            27 => StepKind::RandomBend(sb(0)),
            29 => StepKind::AddBendScaled(sb(0) as i8),
            30 => StepKind::SetRegister { register: sb(0), value: sb(8) },
            31 => StepKind::RandomRegister { register: sb(0), low: sb(8), high: sb(16) },
            38 => StepKind::WaitVoices,
            39 => StepKind::GroupCycle { groups: sb(0), per_group: sb(8), pass: sb(16) },
            43 => StepKind::StopMarker,
            k => StepKind::Opaque { kind: k, arg },
        };
        Ok(Step { delay, kind })
    }

    /// The kind number as stored.
    pub fn kind_number(&self) -> u32 {
        match self.kind {
            StepKind::Voice(_) => 1,
            StepKind::Lfo(_) => 4,
            StepKind::Child { .. } => 5,
            StepKind::Nop => 20,
            StepKind::LoopStart => 21,
            StepKind::LoopEnd => 22,
            StepKind::Stop => 24,
            StepKind::RandomChoice { .. } => 25,
            StepKind::RandomDelay(_) => 26,
            StepKind::RandomBend(_) => 27,
            StepKind::AddBend(_) => 29,
            StepKind::RandomRegister { .. } => 31,
            StepKind::WaitVoices => 38,
            StepKind::Unknown(k) => k,
            StepKind::Opaque { kind, .. } => kind,
            StepKind::VoiceLegacy(_) => 9,
            StepKind::AddBendScaled(_) => 29,
            StepKind::SetRegister { .. } => 30,
            StepKind::GroupCycle { .. } => 39,
            StepKind::LoopBreak => 23,
            StepKind::StopMarker => 43,
        }
    }
}

/// A parsed effect bank. Holds the whole file; samples are decoded while they play.
#[derive(Debug)]
pub struct EffectBank {
    /// The bank file.
    pub bytes: Vec<u8>,
    /// Byte range of the description section.
    pub description: Range<usize>,
    /// Byte range of the sample section.
    pub samples: Range<usize>,
    /// Sound records.
    pub sounds: Vec<SoundRecord>,
    /// All steps, in file order.
    pub steps: Vec<Step>,
    /// The `SBlk` version: 1 (Sly 1) or 3 (Sly 2).
    pub version: u32,
    /// Per step: the last choice a random-choice step made (shared by every instance, so a sound does not repeat
    /// the same choice twice in a row), or a group cycle's pass counter. Updated by the audio thread.
    pub(crate) choice_last: Vec<AtomicI16>,
}

impl EffectBank {
    /// Parses a bank file (audio.md 3.1 and 3.2).
    pub fn parse(bytes: Vec<u8>) -> Result<EffectBank, Error> {
        let b = &bytes;
        if u32_at(b, 0)? != 3 || u32_at(b, 4)? != 2 {
            return Err(Error::Invalid("effect bank: file kind 3 with 2 sections expected"));
        }
        let section = |i: usize| -> Result<Range<usize>, Error> {
            let off = u32_at(b, 8 + i * 8)? as usize;
            let size = u32_at(b, 12 + i * 8)? as usize;
            let end = off.checked_add(size).filter(|&e| e <= b.len()).ok_or(Error::Invalid("bank section"))?;
            Ok(off..end)
        };
        let description = section(0)?;
        let samples = section(1)?;
        let d = &b[description.clone()];
        magic(d, 0, b"SBlk")?;
        let version = u32_at(d, 4)?;
        let (step_size, data_area) = match version {
            1 => (STEP_SIZE, 0),
            3 => (STEP_SIZE_V3, u32_at(d, 52)? as usize),
            _ => return Err(Error::Invalid("effect bank: SBlk version 1 or 3 expected")),
        };
        let sound_count = u16_at(d, 22)? as usize;
        let step_count = u16_at(d, 24)? as usize;
        let sound_table = u32_at(d, 28)? as usize;
        let step_table = u32_at(d, 32)? as usize;
        if step_table.saturating_add(step_count.saturating_mul(step_size)) > d.len()
            || sound_table.saturating_add(sound_count.saturating_mul(SOUND_RECORD_SIZE)) > d.len()
        {
            return Err(Error::Invalid("effect bank tables run past the description section"));
        }
        let mut steps = Vec::with_capacity(step_count);
        for i in 0..step_count {
            steps.push(if version == 1 {
                Step::parse(d, step_table + i * STEP_SIZE)?
            } else {
                Step::parse_v3(d, step_table + i * STEP_SIZE_V3, data_area)?
            });
        }
        let mut sounds = Vec::with_capacity(sound_count);
        for i in 0..sound_count {
            let o = sound_table + i * SOUND_RECORD_SIZE;
            let step_count = i8_at(d, o + 4)?.max(0) as u8;
            // Sounds without steps store a first-step offset of minus one step; ignore it.
            let first = if step_count == 0 { 0 } else { u32_at(d, o + 8)? as usize };
            if first % step_size != 0 {
                return Err(Error::Invalid("sound's first step is not on a step boundary"));
            }
            let rec = SoundRecord {
                volume: i8_at(d, o)?,
                group: i8_at(d, o + 1)?,
                pan: i16_at(d, o + 2)?,
                step_count,
                instance_limit: i8_at(d, o + 5)?,
                flags: u16_at(d, o + 6)?,
                first_step: first / step_size,
            };
            if rec.steps().end > steps.len() {
                return Err(Error::Invalid("sound's steps run past the step table"));
            }
            sounds.push(rec);
        }
        let choice_last = steps
            .iter()
            .map(|s| {
                AtomicI16::new(match s.kind {
                    StepKind::RandomChoice { last, .. } => last,
                    StepKind::GroupCycle { pass, .. } => pass,
                    _ => -1,
                })
            })
            .collect();
        Ok(EffectBank { bytes, description, samples, sounds, steps, version, choice_last })
    }

    /// The sample section.
    pub fn sample_data(&self) -> &[u8] {
        &self.bytes[self.samples.clone()]
    }

    /// The steps of sound `index`.
    pub fn sound_steps(&self, index: usize) -> &[Step] {
        self.sounds.get(index).map_or(&[], |s| &self.steps[s.steps()])
    }
}

/// Reads only the sound count of a bank file from its first bytes (enough to number sound IDs, audio.md 3.6).
pub fn bank_sound_count(head: &[u8]) -> Result<u16, Error> {
    let desc = u32_at(head, 8)? as usize;
    magic(head, desc, b"SBlk")?;
    u16_at(head, desc + 22)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// A test step: (kind, delay, data).
    pub(crate) type TestStep = (u32, i32, Vec<u8>);
    /// A test sound: (volume, group, flags, steps).
    pub(crate) type TestSound = (i8, i8, u16, Vec<TestStep>);

    /// Builds a bank file from test sounds.
    pub(crate) fn build_bank(sounds: &[TestSound], samples: &[u8]) -> Vec<u8> {
        let step_total: usize = sounds.iter().map(|s| s.3.len()).sum();
        let sound_table = 0x34;
        let step_table = sound_table + sounds.len() * SOUND_RECORD_SIZE;
        let desc_size = step_table + step_total * STEP_SIZE;
        let mut d = vec![0u8; desc_size];
        d[0..4].copy_from_slice(b"SBlk");
        d[4..8].copy_from_slice(&1u32.to_le_bytes());
        d[8..12].copy_from_slice(&4u32.to_le_bytes());
        d[22..24].copy_from_slice(&(sounds.len() as u16).to_le_bytes());
        d[24..26].copy_from_slice(&(step_total as u16).to_le_bytes());
        d[28..32].copy_from_slice(&(sound_table as u32).to_le_bytes());
        d[32..36].copy_from_slice(&(step_table as u32).to_le_bytes());
        let mut si = 0;
        for (i, (vol, group, flags, steps)) in sounds.iter().enumerate() {
            let o = sound_table + i * SOUND_RECORD_SIZE;
            d[o] = *vol as u8;
            d[o + 1] = *group as u8;
            d[o + 4] = steps.len() as u8;
            d[o + 6..o + 8].copy_from_slice(&flags.to_le_bytes());
            d[o + 8..o + 12].copy_from_slice(&((si * STEP_SIZE) as u32).to_le_bytes());
            for (kind, delay, data) in steps {
                let so = step_table + si * STEP_SIZE;
                d[so..so + 4].copy_from_slice(&kind.to_le_bytes());
                d[so + 4..so + 8].copy_from_slice(&delay.to_le_bytes());
                d[so + 8..so + 8 + data.len()].copy_from_slice(data);
                si += 1;
            }
        }
        let mut f = vec![0u8; 0x18];
        f[0..4].copy_from_slice(&3u32.to_le_bytes());
        f[4..8].copy_from_slice(&2u32.to_le_bytes());
        f[8..12].copy_from_slice(&0x18u32.to_le_bytes());
        f[12..16].copy_from_slice(&(desc_size as u32).to_le_bytes());
        f[16..20].copy_from_slice(&((0x18 + desc_size) as u32).to_le_bytes());
        f[20..24].copy_from_slice(&(samples.len() as u32).to_le_bytes());
        f.extend_from_slice(&d);
        f.extend_from_slice(samples);
        f
    }

    /// Voice step data.
    pub(crate) fn voice_data(volume: i8, centre: i8, fine: u8, w1: u16, w2: u16, offset: u32) -> Vec<u8> {
        let mut v = vec![0u8; 24];
        v[1] = volume as u8;
        v[2] = centre as u8;
        v[3] = fine;
        v[10..12].copy_from_slice(&w1.to_le_bytes());
        v[12..14].copy_from_slice(&w2.to_le_bytes());
        v[16..20].copy_from_slice(&offset.to_le_bytes());
        v
    }

    #[test]
    fn parses_synthetic_bank() {
        let samples = vec![0u8; 64];
        let bank = build_bank(
            &[
                (100, 3, 0, vec![(1, 0, voice_data(110, -86, 66, 0x80FF, 0x9FC0, 16))]),
                (
                    90,
                    0,
                    1,
                    vec![
                        (21, 0, vec![]),
                        (26, 5, 30i32.to_le_bytes().to_vec()),
                        (22, 0, vec![]),
                        (5, 2, vec![127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                    ],
                ),
            ],
            &samples,
        );
        assert_eq!(bank_sound_count(&bank).unwrap(), 2);
        let b = EffectBank::parse(bank).unwrap();
        assert_eq!(b.sounds.len(), 2);
        assert_eq!(b.steps.len(), 5);
        assert_eq!(b.sample_data().len(), 64);
        let s0 = b.sounds[0];
        assert_eq!((s0.volume, s0.group, s0.continuous()), (100, 3, false));
        let StepKind::Voice(v) = b.sound_steps(0)[0].kind else { panic!() };
        assert_eq!((v.volume, v.centre, v.fine, v.sample_offset), (110, -86, 66, 16));
        assert_eq!(v.adsr, AdsrWords { w1: 0x80FF, w2: 0x9FC0 });
        assert!((v.semitones_for(60) - (-26.0 + 66.0 / 128.0)).abs() < 1e-6);
        let s1 = b.sound_steps(1);
        assert!(b.sounds[1].continuous());
        assert_eq!(s1[1], Step { delay: 5, kind: StepKind::RandomDelay(30) });
        assert_eq!(s1[3].kind, StepKind::Child { sound: 0, volume: 127, pan: 0 });
        assert_eq!(s1.iter().map(|s| s.kind_number()).collect::<Vec<_>>(), [21, 26, 22, 5]);
    }

    /// A version-3 test sound: (volume, group, steps as (step word, delay)).
    pub(crate) type TestSoundV3 = (i8, i8, Vec<(u32, i32)>);

    /// Builds a version-3 bank: `sounds` = (volume, group, steps as (step word, delay)), with `area` as the
    /// step-data area.
    pub(crate) fn build_bank_v3(sounds: &[TestSoundV3], area: &[u8], samples: &[u8]) -> Vec<u8> {
        let steps: usize = sounds.iter().map(|s| s.2.len()).sum();
        let sound_table = 0x40;
        let step_table = sound_table + sounds.len() * SOUND_RECORD_SIZE;
        let data_area = step_table + steps * STEP_SIZE_V3;
        let mut d = vec![0u8; data_area];
        d[0..4].copy_from_slice(b"SBlk");
        d[4..8].copy_from_slice(&3u32.to_le_bytes());
        d[22..24].copy_from_slice(&(sounds.len() as u16).to_le_bytes());
        d[24..26].copy_from_slice(&(steps as u16).to_le_bytes());
        d[28..32].copy_from_slice(&(sound_table as u32).to_le_bytes());
        d[32..36].copy_from_slice(&(step_table as u32).to_le_bytes());
        d[52..56].copy_from_slice(&(data_area as u32).to_le_bytes());
        let mut si = 0;
        for (i, (vol, group, st)) in sounds.iter().enumerate() {
            let o = sound_table + i * SOUND_RECORD_SIZE;
            d[o] = *vol as u8;
            d[o + 1] = *group as u8;
            d[o + 4] = st.len() as u8;
            let first: i32 = if st.is_empty() { -8 } else { (si * STEP_SIZE_V3) as i32 };
            d[o + 8..o + 12].copy_from_slice(&first.to_le_bytes());
            for (w0, delay) in st {
                let so = step_table + si * STEP_SIZE_V3;
                d[so..so + 4].copy_from_slice(&w0.to_le_bytes());
                d[so + 4..so + 8].copy_from_slice(&delay.to_le_bytes());
                si += 1;
            }
        }
        d.extend_from_slice(area);
        let mut f = vec![0u8; 0x18];
        f[0..4].copy_from_slice(&3u32.to_le_bytes());
        f[4..8].copy_from_slice(&2u32.to_le_bytes());
        f[8..12].copy_from_slice(&0x18u32.to_le_bytes());
        f[12..16].copy_from_slice(&(d.len() as u32).to_le_bytes());
        f[16..20].copy_from_slice(&((0x18 + d.len()) as u32).to_le_bytes());
        f[20..24].copy_from_slice(&(samples.len() as u32).to_le_bytes());
        f.extend_from_slice(&d);
        f.extend_from_slice(samples);
        f
    }

    #[test]
    fn parses_version_3_steps() {
        let mut area = voice_data(110, -86, 66, 0x80FF, 0x9FC0, 32);
        area.resize(32, 0);
        let mut child = vec![0u8; 16];
        child[0] = 100;
        child[12] = 1;
        area.extend_from_slice(&child);
        let steps = vec![
            ((1 << 24), 0),
            ((25 << 24) | (2 << 8) | 3, 5),
            ((26 << 24) | 30, 0),
            ((27 << 24) | 0x4B, 0),
            ((29 << 24) | 0xBA, 0),
            ((31 << 24) | (0x78 << 16) | (0x5A << 8) | 0xFF, 0),
            ((5 << 24) | 32, 0),
            ((39 << 24) | (1 << 8) | 4, 0),
            ((9 << 24), 0),
            ((30 << 24) | (100 << 8), 0),
            (23 << 24, 0),
            (43 << 24, 0),
            ((50 << 24) | 7, 0),
        ];
        let bank = build_bank_v3(&[(75, 3, steps), (100, 0, vec![])], &area, &[0u8; 64]);
        let b = EffectBank::parse(bank).unwrap();
        assert_eq!(b.version, 3);
        assert_eq!(b.sounds.len(), 2);
        assert_eq!(b.sound_steps(1).len(), 0);
        let s = b.sound_steps(0);
        let StepKind::Voice(v) = s[0].kind else { panic!() };
        assert_eq!((v.volume, v.centre, v.fine, v.sample_offset), (110, -86, 66, 32));
        assert_eq!(s[1], Step { delay: 5, kind: StepKind::RandomChoice { choices: 3, per_choice: 2, last: 0 } });
        assert_eq!(s[2].kind, StepKind::RandomDelay(30));
        assert_eq!(s[3].kind, StepKind::RandomBend(75));
        assert_eq!(s[4].kind, StepKind::AddBendScaled(-70));
        assert_eq!(s[5].kind, StepKind::RandomRegister { register: -1, low: 90, high: 120 });
        assert_eq!(s[6].kind, StepKind::Child { sound: 1, volume: 100, pan: 0 });
        assert_eq!(s[7].kind, StepKind::GroupCycle { groups: 4, per_group: 1, pass: 0 });
        assert!(matches!(s[8].kind, StepKind::VoiceLegacy(v) if v.volume == 110));
        assert_eq!(s[9].kind, StepKind::SetRegister { register: 0, value: 100 });
        assert_eq!((s[10].kind, s[11].kind), (StepKind::LoopBreak, StepKind::StopMarker));
        assert_eq!(s[12].kind, StepKind::Opaque { kind: 50, arg: 7 });
        let numbers: Vec<u32> = s.iter().map(|x| x.kind_number()).collect();
        assert_eq!(numbers, [1, 25, 26, 27, 29, 31, 5, 39, 9, 30, 23, 43, 50]);
        // A version other than 1 or 3 is refused.
        let mut bad = build_bank_v3(&[(75, 3, vec![])], &[], &[]);
        bad[0x18 + 4] = 2;
        assert!(EffectBank::parse(bad).is_err());
    }

    #[test]
    fn value_sources() {
        assert_eq!(ValueSource::volume(100), ValueSource::Value(100));
        assert_eq!(ValueSource::volume(-1), ValueSource::SoundRegister(0));
        assert_eq!(ValueSource::volume(-4), ValueSource::SoundRegister(3));
        assert_eq!(ValueSource::volume(-5), ValueSource::Random);
        assert_eq!(ValueSource::volume(-6), ValueSource::GlobalRegister(0));
        assert_eq!(ValueSource::pan(-90), ValueSource::Value(-90));
        assert_eq!(ValueSource::pan(-2), ValueSource::SoundRegister(1));
    }

    #[test]
    fn rejects_bad_banks() {
        assert!(EffectBank::parse(vec![0u8; 8]).is_err());
        let mut bank = build_bank(&[(100, 0, 0, vec![(20, 0, vec![])])], &[]);
        bank[0x18] = b'X';
        assert!(EffectBank::parse(bank).is_err());
    }
}
