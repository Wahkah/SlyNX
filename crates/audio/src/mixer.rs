//! The voice pool and the mixing loop: SPU ADPCM decoded while playing, linear-interpolation resampling to 48 kHz,
//! the hardware envelope, per-voice gains ramped across each scheduler tick, a reverb send, and a simple reverb.
//!
//! Nothing here allocates after [`Mixer::new`].

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use crate::adpcm::Cursor;
use crate::envelope::Envelope;
use crate::fmath;

/// Output rate in Hz.
pub const OUTPUT_RATE: u32 = 48_000;
/// Scheduler ticks per second (step delays and the sequencer use this clock).
pub const TICK_RATE: u32 = 240;
/// Output frames per scheduler tick.
pub const TICK_FRAMES: usize = (OUTPUT_RATE / TICK_RATE) as usize;

/// Which sample data a voice plays from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The sample section of effect bank `n`.
    Bank(u8),
    /// The sample section of the current music file.
    Music,
    /// Channel `channel` of stream slot `slot`.
    Stream {
        /// Stream slot.
        slot: u8,
        /// Channel (0 for a VAG file).
        channel: u8,
    },
}

/// Who controls a voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// Nobody (free voice).
    None,
    /// A sound instance.
    Effect {
        /// Instance slot.
        instance: u8,
    },
    /// A note of the music sequencer.
    Music {
        /// Segment player.
        player: u8,
        /// MIDI channel.
        channel: u8,
        /// Note number.
        note: u8,
    },
    /// A streamed line or loop.
    Stream {
        /// Stream slot.
        slot: u8,
    },
}

/// The voice pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    /// Sound effects.
    Effects,
    /// Music notes.
    Music,
    /// Streams.
    Streams,
}

/// Gives the mixer the bytes a [`Source`] names.
pub trait SampleData {
    /// The sample bytes for `source` (empty when it is not loaded).
    fn data(&self, source: Source) -> &[u8];
}

/// Parameters of a voice that do not change while it plays.
#[derive(Clone, Copy, Debug)]
pub struct VoiceStart {
    /// Sample data.
    pub source: Source,
    /// Byte offset of the sample in that data.
    pub offset: usize,
    /// The envelope.
    pub envelope: Envelope,
    /// Owner.
    pub owner: Owner,
    /// Priority.
    pub priority: i8,
    /// Mix group 0..3.
    pub group: u8,
    /// Whether the voice feeds the reverb.
    pub reverb: bool,
    /// Product of the voice's fixed linear volumes (before the square law).
    pub volume: f32,
    /// Fixed pan in degrees.
    pub pan: f32,
    /// Fixed pitch in semitones relative to `rate_scale`.
    pub semitones: f32,
    /// Sample rate of the data relative to 48 kHz (1 for SPU samples).
    pub rate_scale: f32,
    /// Bend range down, semitones.
    pub bend_down: f32,
    /// Bend range up, semitones.
    pub bend_up: f32,
    /// Loop the sample even where its data does not (looped streams).
    pub wrap: bool,
}

/// One voice.
#[derive(Clone, Copy, Debug)]
pub struct Voice {
    /// Playing.
    pub active: bool,
    /// Key off has been sent.
    pub released: bool,
    /// Fixed parameters.
    pub p: VoiceStart,
    /// Start order (for "oldest first" when stealing).
    pub serial: u64,
    cursor: Cursor,
    s0: i32,
    s1: i32,
    frac: u32,
    primed: bool,
    tail: bool,
    step: u64,
    pub(crate) gain: [f32; 2],
    target: [f32; 2],
    delta: [f32; 2],
    send: f32,
    send_target: f32,
    send_delta: f32,
    fresh: bool,
}

impl Voice {
    const IDLE: Voice = Voice {
        active: false,
        released: false,
        p: VoiceStart {
            source: Source::Music,
            offset: 0,
            envelope: Envelope::constant(),
            owner: Owner::None,
            priority: 0,
            group: 0,
            reverb: false,
            volume: 0.0,
            pan: 0.0,
            semitones: 0.0,
            rate_scale: 1.0,
            bend_down: 2.0,
            bend_up: 2.0,
            wrap: false,
        },
        serial: 0,
        cursor: Cursor::new(0),
        s0: 0,
        s1: 0,
        frac: 0,
        primed: false,
        tail: false,
        step: 1 << 32,
        gain: [0.0; 2],
        target: [0.0; 2],
        delta: [0.0; 2],
        send: 0.0,
        send_target: 0.0,
        send_delta: 0.0,
        fresh: true,
    };

    /// Key off: the envelope goes to its release phase.
    pub fn release(&mut self) {
        if self.active && !self.released {
            self.released = true;
            self.p.envelope.release();
        }
    }

    /// Stops at once.
    pub fn kill(&mut self) {
        self.active = false;
        self.p.owner = Owner::None;
    }

    /// Bend in semitones for a bend fraction -1..1 (of the down or up range).
    pub fn bend_semitones(&self, fraction: f32) -> f32 {
        if fraction < 0.0 { fraction * self.p.bend_down } else { fraction * self.p.bend_up }
    }
}

/// Pan law (audio.md 3.5): left `(1 - sin p) / 2`, right `(1 + sin p) / 2`.
pub fn pan_gains(deg: f32) -> [f32; 2] {
    let s = fmath::sin_deg(deg as f64) as f32;
    [(1.0 - s) * 0.5, (1.0 + s) * 0.5]
}

/// A Schroeder-style reverb: four damped combs in parallel, two all-passes in series, mono in, added to both
/// output channels.
#[derive(Clone, Debug)]
pub struct Reverb {
    combs: [Delay; 4],
    allpasses: [Delay; 2],
    damp_state: [f32; 4],
    feedback: f32,
    damp: f32,
    wet: f32,
}

#[derive(Clone, Debug)]
struct Delay {
    buf: Vec<f32>,
    pos: usize,
}

impl Delay {
    fn new(len: usize) -> Delay {
        Delay { buf: vec![0.0; len.max(1)], pos: 0 }
    }
}

impl Reverb {
    fn new() -> Reverb {
        Reverb {
            combs: [Delay::new(1215), Delay::new(1293), Delay::new(1390), Delay::new(1476)],
            allpasses: [Delay::new(605), Delay::new(480)],
            damp_state: [0.0; 4],
            feedback: 0.0,
            damp: 0.3,
            wet: 0.0,
        }
    }

    /// Sets the reverb from the game's kind (0 none, 1..3 studio small/medium/large, 4 pipe, 5 room; the mapping to
    /// the original presets is unconfirmed) and depth 0..32767.
    pub fn set(&mut self, kind: u32, depth: u32) {
        let (feedback, damp) = match kind {
            1 => (0.70, 0.40),
            2 => (0.78, 0.35),
            3 => (0.84, 0.30),
            4 => (0.88, 0.10),
            5 => (0.75, 0.45),
            _ => (0.0, 0.3),
        };
        self.feedback = feedback;
        self.damp = damp;
        self.wet = if kind == 0 { 0.0 } else { depth.min(32767) as f32 / 32767.0 * 0.5 };
    }

    /// Whether the reverb produces anything.
    pub fn enabled(&self) -> bool {
        self.wet > 0.0
    }

    fn process(&mut self, input: &[f32], out: &mut [f32]) {
        for (i, &x) in input.iter().enumerate() {
            let x = x * 0.25;
            let mut acc = 0.0;
            for (c, st) in self.combs.iter_mut().zip(self.damp_state.iter_mut()) {
                let y = c.buf[c.pos];
                *st = y * (1.0 - self.damp) + *st * self.damp;
                c.buf[c.pos] = x + *st * self.feedback;
                c.pos = if c.pos + 1 == c.buf.len() { 0 } else { c.pos + 1 };
                acc += y;
            }
            for a in self.allpasses.iter_mut() {
                let b = a.buf[a.pos];
                a.buf[a.pos] = acc + b * 0.5;
                acc = b - acc;
                a.pos = if a.pos + 1 == a.buf.len() { 0 } else { a.pos + 1 };
            }
            let w = acc * self.wet;
            out[2 * i] += w;
            out[2 * i + 1] += w;
        }
    }
}

/// The voice pool and the output stage.
pub struct Mixer {
    /// All voices; [`Mixer::pool`] gives each pool's range.
    pub voices: Vec<Voice>,
    pools: [Range<usize>; 3],
    serial: u64,
    send: Vec<f32>,
    /// The reverb.
    pub reverb: Reverb,
}

/// Fixed-point scale of the resampling position.
const FRAC_ONE: f64 = 4_294_967_296.0;
/// Sample times envelope (both up to 2^15) to -1..1.
const SCALE: f32 = 1.0 / (32768.0 * 32768.0);

impl Mixer {
    /// A mixer with the given number of voices per pool.
    pub fn new(effects: usize, music: usize, streams: usize) -> Mixer {
        let total = effects + music + streams;
        Mixer {
            voices: vec![Voice::IDLE; total],
            pools: [0..effects, effects..effects + music, effects + music..total],
            serial: 0,
            send: vec![0.0; TICK_FRAMES],
            reverb: Reverb::new(),
        }
    }

    /// The index range of `pool`.
    pub fn pool(&self, pool: Pool) -> Range<usize> {
        self.pools[pool as usize].clone()
    }

    /// Finds a voice for a new sound: a free one, else the lowest-priority voice (oldest on ties) when the new
    /// priority is not lower. The chosen voice is stopped.
    pub fn alloc(&mut self, pool: Pool, priority: i8) -> Option<usize> {
        let range = self.pool(pool);
        if let Some(i) = range.clone().find(|&i| !self.voices[i].active) {
            return Some(i);
        }
        let victim = range.min_by_key(|&i| (self.voices[i].p.priority, self.voices[i].serial))?;
        if self.voices[victim].p.priority > priority {
            return None;
        }
        self.voices[victim].kill();
        Some(victim)
    }

    /// Starts voice `i`. Its gains are set by the first [`Mixer::set_target`] without a ramp.
    pub fn start(&mut self, i: usize, p: VoiceStart) {
        self.serial += 1;
        let v = &mut self.voices[i];
        *v = Voice::IDLE;
        v.active = true;
        v.serial = self.serial;
        v.cursor = if p.wrap { Cursor::looping(p.offset) } else { Cursor::new(p.offset) };
        v.p = p;
    }

    /// Sets voice `i`'s gains and pitch for the coming tick: `amplitude` (after the square law and group volumes),
    /// `pan` in degrees, `ratio` = playback rate / 48 kHz, `send` = reverb send level. Gains ramp to the new values
    /// across the next [`TICK_FRAMES`] frames.
    pub fn set_target(&mut self, i: usize, amplitude: f32, pan: f32, ratio: f64, send: f32) {
        let v = &mut self.voices[i];
        let g = pan_gains(pan);
        let t = [amplitude * g[0], amplitude * g[1]];
        let ratio = ratio.clamp(0.0, 64.0);
        v.step = (ratio * FRAC_ONE) as u64;
        if v.fresh {
            v.fresh = false;
            v.gain = t;
            v.delta = [0.0; 2];
            v.send = send;
            v.send_delta = 0.0;
        } else {
            v.gain = v.target;
            v.send = v.send_target;
            let n = TICK_FRAMES as f32;
            v.delta = [(t[0] - v.gain[0]) / n, (t[1] - v.gain[1]) / n];
            v.send_delta = (send - v.send) / n;
        }
        v.target = t;
        v.send_target = send;
    }

    /// Mixes `frames` frames of every active voice into `out` (interleaved stereo, added to what is there).
    /// `frames` must not exceed [`TICK_FRAMES`].
    pub fn mix<S: SampleData>(&mut self, data: &S, out: &mut [f32], frames: usize) {
        let frames = frames.min(TICK_FRAMES).min(out.len() / 2);
        let use_send = self.reverb.enabled();
        if use_send {
            self.send[..frames].fill(0.0);
        }
        for v in self.voices.iter_mut() {
            if !v.active || v.fresh {
                continue;
            }
            let bytes = data.data(v.p.source);
            mix_voice(v, bytes, &mut out[..frames * 2], if use_send { Some(&mut self.send[..frames]) } else { None });
        }
        if use_send {
            self.reverb.process(&self.send[..frames], &mut out[..frames * 2]);
        }
    }

    /// Stops every voice for which `f` is true.
    pub fn kill_where(&mut self, f: impl Fn(&Voice) -> bool) {
        for v in self.voices.iter_mut() {
            if v.active && f(v) {
                v.kill();
            }
        }
    }

    /// Releases every voice for which `f` is true.
    pub fn release_where(&mut self, f: impl Fn(&Voice) -> bool) {
        for v in self.voices.iter_mut() {
            if v.active && f(v) {
                v.release();
            }
        }
    }
}

#[inline]
fn mix_voice(v: &mut Voice, data: &[u8], out: &mut [f32], mut send: Option<&mut [f32]>) {
    if !v.primed {
        v.primed = true;
        match (v.cursor.next_sample(data), v.cursor.next_sample(data)) {
            (Some(a), Some(b)) => {
                v.s0 = a as i32;
                v.s1 = b as i32;
            }
            (Some(a), None) => {
                v.s0 = a as i32;
                v.s1 = 0;
                v.tail = true;
            }
            _ => {
                v.kill();
                return;
            }
        }
    }
    let mut gl = v.gain[0];
    let mut gr = v.gain[1];
    let mut sg = v.send;
    let feeds = v.p.reverb;
    for (i, frame) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let level = v.p.envelope.tick();
        if v.p.envelope.is_off() {
            v.kill();
            break;
        }
        let s = v.s0 + (((v.s1 - v.s0) as i64 * v.frac as i64) >> 32) as i32;
        let x = (s * level) as f32 * SCALE;
        frame[0] += x * gl;
        frame[1] += x * gr;
        if feeds && let Some(send) = send.as_deref_mut() {
            send[i] += x * sg;
        }
        gl += v.delta[0];
        gr += v.delta[1];
        sg += v.send_delta;
        let pos = v.frac as u64 + v.step;
        v.frac = pos as u32;
        let mut advance = pos >> 32;
        let mut ended = false;
        while advance > 0 {
            if v.tail {
                ended = true;
                break;
            }
            v.s0 = v.s1;
            match v.cursor.next_sample(data) {
                Some(n) => v.s1 = n as i32,
                // Past the last sample: interpolate toward silence for one more sample, then stop.
                None => {
                    v.s1 = 0;
                    v.tail = true;
                }
            }
            advance -= 1;
        }
        if ended {
            v.kill();
            break;
        }
    }
    v.gain = [gl, gr];
    v.send = sg;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adpcm::tests::block;
    use crate::adpcm::{FLAG_END, FLAG_LOOP_START, FLAG_REPEAT};
    use crate::envelope::AdsrWords;

    struct Data(Vec<u8>);
    impl SampleData for Data {
        fn data(&self, _: Source) -> &[u8] {
            &self.0
        }
    }

    fn start(m: &mut Mixer, pool: Pool, priority: i8, w2: u16) -> usize {
        let i = m.alloc(pool, priority).unwrap();
        m.start(
            i,
            VoiceStart {
                source: Source::Bank(0),
                offset: 0,
                envelope: Envelope::new(AdsrWords { w1: 0x80FF, w2 }),
                owner: Owner::Effect { instance: 0 },
                priority,
                group: 0,
                reverb: false,
                volume: 1.0,
                pan: 0.0,
                semitones: 0.0,
                rate_scale: 1.0,
                bend_down: 2.0,
                bend_up: 2.0,
                wrap: false,
            },
        );
        i
    }

    fn looping_square() -> Data {
        // Two blocks of a square-ish wave, looping from the start.
        let mut d = vec![];
        d.extend_from_slice(&block(4, 0, FLAG_LOOP_START, &[7; 28]));
        d.extend_from_slice(&block(4, 0, FLAG_END | FLAG_REPEAT, &[-7; 28]));
        Data(d)
    }

    #[test]
    fn pan_law() {
        assert_eq!(pan_gains(0.0), [0.5, 0.5]);
        let r = pan_gains(90.0);
        assert!(r[0].abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6);
        let l = pan_gains(-90.0);
        assert!((l[0] - 1.0).abs() < 1e-6 && l[1].abs() < 1e-6);
    }

    #[test]
    fn allocation_and_stealing() {
        let mut m = Mixer::new(2, 1, 1);
        let a = start(&mut m, Pool::Effects, 5, 0x9FC0);
        let b = start(&mut m, Pool::Effects, 3, 0x9FC0);
        assert_ne!(a, b);
        // Pool full: a lower priority than every voice gets nothing.
        assert_eq!(m.alloc(Pool::Effects, 2), None);
        // Equal priority steals the lowest (b).
        assert_eq!(m.alloc(Pool::Effects, 3), Some(b));
        assert!(!m.voices[b].active);
        assert_eq!(m.pool(Pool::Music), 2..3);
        assert_eq!(m.alloc(Pool::Streams, 0), Some(3));
    }

    #[test]
    fn plays_resamples_and_releases() {
        let data = looping_square();
        let mut m = Mixer::new(1, 0, 0);
        let i = start(&mut m, Pool::Effects, 0, 0x9FCB);
        m.set_target(i, 1.0, 90.0, 0.5, 0.0);
        let mut out = vec![0.0f32; TICK_FRAMES * 2];
        m.mix(&data, &mut out, TICK_FRAMES);
        // Panned right: nothing on the left.
        assert!(out.chunks(2).all(|f| f[0].abs() < 1e-6));
        let peak = out.chunks(2).map(|f| f[1].abs()).fold(0.0, f32::max);
        // Code 7 at shift 4 = 1792; times the envelope near 1.
        assert!((peak - 1792.0 / 32768.0).abs() < 0.002, "peak {peak}");
        // At half rate, 28 samples of +7 last 56 frames (after the attack).
        assert!(out[2 * 20 + 1] > 0.0 && out[2 * 60 + 1] < 0.0);
        // The loop keeps playing.
        for _ in 0..50 {
            m.set_target(i, 1.0, 90.0, 0.5, 0.0);
            out.fill(0.0);
            m.mix(&data, &mut out, TICK_FRAMES);
        }
        assert!(m.voices[i].active);
        m.voices[i].release();
        for _ in 0..40 {
            out.fill(0.0);
            m.mix(&data, &mut out, TICK_FRAMES);
        }
        assert!(!m.voices[i].active, "release of 4096 samples should have finished");
    }

    #[test]
    fn one_shot_ends() {
        let mut d = vec![];
        d.extend_from_slice(&block(4, 0, FLAG_END, &[3; 28]));
        let data = Data(d);
        let mut m = Mixer::new(1, 0, 0);
        let i = start(&mut m, Pool::Effects, 0, 0x9FC0);
        m.set_target(i, 1.0, 0.0, 1.0, 0.0);
        let mut out = vec![0.0f32; TICK_FRAMES * 2];
        m.mix(&data, &mut out, TICK_FRAMES);
        assert!(!m.voices[i].active);
        assert!(out[2 * 10] > 0.0);
        assert_eq!(out[2 * 40], 0.0);
    }

    #[test]
    fn gains_ramp_across_a_tick() {
        let data = looping_square();
        let mut m = Mixer::new(1, 0, 0);
        let i = start(&mut m, Pool::Effects, 0, 0x9FC0);
        m.set_target(i, 1.0, 0.0, 1.0, 0.0);
        let mut out = vec![0.0f32; TICK_FRAMES * 2];
        m.mix(&data, &mut out, TICK_FRAMES);
        m.set_target(i, 0.0, 0.0, 1.0, 0.0);
        out.fill(0.0);
        m.mix(&data, &mut out, TICK_FRAMES);
        let first = out[0].abs();
        let last = out[2 * (TICK_FRAMES - 1)].abs();
        assert!(first > 0.01 && last < first * 0.05, "first {first} last {last}");
    }

    #[test]
    fn reverb_adds_a_tail() {
        let mut d = vec![];
        d.extend_from_slice(&block(0, 0, FLAG_END, &[7; 28]));
        let data = Data(d);
        let mut m = Mixer::new(1, 0, 0);
        m.reverb.set(3, 20_000);
        let i = start(&mut m, Pool::Effects, 0, 0x9FC0);
        m.voices[i].p.reverb = true;
        m.set_target(i, 1.0, 0.0, 1.0, 1.0);
        let mut out = vec![0.0f32; TICK_FRAMES * 2];
        let mut tail = 0.0f32;
        for t in 0..20 {
            out.fill(0.0);
            m.mix(&data, &mut out, TICK_FRAMES);
            if t > 2 {
                tail = tail.max(out.iter().fold(0.0, |a, &x| a.max(x.abs())));
            }
        }
        assert!(tail > 1e-4, "no reverb tail");
    }
}
