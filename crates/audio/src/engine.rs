//! The two halves of the sound system: [`SoundEngine`] on the game thread and [`AudioRenderer`] on the audio
//! thread, joined by two lock-free queues (commands one way, events and released data the other).
//!
//! The game thread owns all loading and parsing and every allocation. The renderer receives parsed data as `Arc`s,
//! never frees them itself (it sends them back to be dropped on the game thread), and never allocates.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use platform_api::AudioRender;

use crate::bank::EffectBank;
use crate::effects::{self, GLOBAL_REGISTERS, Instance, MAX_INSTANCES, State, StepContext};
use crate::envelope::Envelope;
use crate::fmath;
use crate::mixer::{Mixer, Owner, Pool, SampleData, Source, TICK_FRAMES, VoiceStart};
use crate::music::MusicFile;
use crate::pcm::PcmReader;
use crate::queue::{Consumer, Producer, channel};
use crate::rng::Rng;
use crate::sequencer::Sequencer;
use crate::sly2::music::{Falloff, MusicMood};
use crate::sly2::vpk::VpkStream;
use crate::spatial::Vec3;
use crate::spatial::{Emitter, Listener, Mix, mix_for};
use crate::vag::VagSound;

/// Number of effect bank slots (the executable's bank table).
pub const BANK_SLOTS: usize = 7;
/// Most streams (dialogue, ambient loops, music streams) at once.
pub const MAX_STREAMS: usize = 8;
/// Most channels of one stream (`VPK ` files have 2 to 5).
pub const MAX_STREAM_CHANNELS: usize = crate::sly2::vpk::MAX_CHANNELS;
/// Number of music layers a stream can have (channel group numbers 0..7).
pub const LAYERS: usize = crate::sly2::music::LAYERS;
/// Intensity level requested by an alarm or an attacking guard.
pub const INTENSITY_ALARM: i32 = 107;
/// Intensity level requested while hiding or sneaking.
pub const INTENSITY_HIDING: i32 = -20;

/// Mix groups (audio.md 3.5).
pub mod group {
    /// Effects.
    pub const EFFECTS: u8 = 0;
    /// Music.
    pub const MUSIC: u8 = 1;
    /// Dialogue.
    pub const DIALOGUE: u8 = 2;
    /// The second effects group.
    pub const EFFECTS2: u8 = 3;
}

/// Sizes of the sound system.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Effect voices (one SPU core: 24).
    pub effect_voices: usize,
    /// Music voices (one SPU core: 24).
    pub music_voices: usize,
    /// Stream voices.
    pub stream_voices: usize,
    /// Command queue length.
    pub queue: usize,
    /// Random seed.
    pub seed: u64,
    /// Scale music regions with a non-negative centre note by 44,100 / 48,000 (unconfirmed rule, off by default).
    pub legacy_music_tuning: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            effect_voices: 24,
            music_voices: 24,
            stream_voices: 16,
            queue: 1024,
            seed: 0x5EED,
            legacy_music_tuning: false,
        }
    }
}

/// Data the renderer gives back to be dropped on the game thread.
pub enum Garbage {
    /// An effect bank.
    Bank(Arc<EffectBank>),
    /// A music file.
    Music(Arc<MusicFile>),
    /// A streamed sound.
    Stream(Arc<VagSound>),
    /// A multi-channel stream.
    Vpk(Arc<VpkStream>),
    /// A detached PCM stream input.
    Pcm(PcmReader),
}

/// Game thread to audio thread.
pub enum Command {
    /// Attach a PCM stream input (or detach the current one); the old one goes back as garbage.
    AttachPcm(Option<PcmReader>),
    /// Put a bank in slot `index` (or empty it).
    LoadBank {
        /// Slot 0..7.
        index: u8,
        /// The bank.
        bank: Option<Arc<EffectBank>>,
    },
    /// Start a sound.
    Play {
        /// Handle chosen by the game side.
        handle: u32,
        /// Bank slot.
        bank: u8,
        /// Sound index in the bank.
        sound: u16,
        /// Initial mix.
        mix: Mix,
    },
    /// Start a stream.
    PlayStream {
        /// Handle.
        handle: u32,
        /// The sound.
        sound: Arc<VagSound>,
        /// Mix group.
        group: u8,
        /// Loop it.
        looped: bool,
        /// Initial mix.
        mix: Mix,
    },
    /// Start a multi-channel `VPK ` stream (Sly 2 music and long sounds).
    PlayVpk {
        /// Handle.
        handle: u32,
        /// The stream.
        stream: Arc<VpkStream>,
        /// Mix group.
        group: u8,
        /// Loop it (music does).
        looped: bool,
        /// Initial mix.
        mix: Mix,
    },
    /// Set a stream's layer gains (by channel group number), fading over `fade` seconds.
    SetLayers {
        /// Handle.
        handle: u32,
        /// Gains 0..1 per layer.
        gains: [f32; LAYERS],
        /// Fade time in seconds.
        fade: f32,
    },
    /// Set a stream's per-channel gains and pans (the resident stream's distance layers).
    SetChannels {
        /// Handle.
        handle: u32,
        /// Gains 0..1 per channel.
        gains: [f32; MAX_STREAM_CHANNELS],
        /// Pans in degrees per channel, added to the channel's own.
        pans: [f32; MAX_STREAM_CHANNELS],
        /// Fade time in seconds for the gains.
        fade: f32,
    },
    /// Fade a stream out over `seconds`, then stop it.
    FadeOut {
        /// Handle.
        handle: u32,
        /// Fade time.
        seconds: f32,
    },
    /// Change a playing sound's or stream's mix.
    SetMix {
        /// Handle.
        handle: u32,
        /// New mix.
        mix: Mix,
    },
    /// Stop a sound or stream (voices release).
    Stop {
        /// Handle.
        handle: u32,
    },
    /// Stop every sound and stream.
    StopAll,
    /// Load music and start it at `segment`.
    PlayMusic {
        /// The music.
        music: Arc<MusicFile>,
        /// First segment (0 when a world loads).
        segment: u8,
    },
    /// Stop the music; with `unload`, also drop it.
    StopMusic {
        /// Drop the music data too.
        unload: bool,
    },
    /// Set a music register.
    SetMusicRegister {
        /// Register.
        register: u8,
        /// Value.
        value: u8,
    },
    /// Set the music intensity value (level + 20, 0..127).
    SetIntensity(u8),
    /// Set a mix group's master volume (0..1).
    SetGroupVolume {
        /// Group 0..3.
        group: u8,
        /// Volume.
        volume: f32,
    },
    /// Set the world's music volume factor (option 525).
    SetMusicVolume(f32),
    /// Set the reverb (option 552: kind and depth).
    SetReverb {
        /// Kind 0..5.
        kind: u32,
        /// Depth 0..32767.
        depth: u32,
    },
    /// Set a global register read by voice volumes and pans.
    SetGlobalRegister {
        /// Register.
        register: u8,
        /// Value.
        value: i32,
    },
    /// Pause (silence) or resume all output.
    Pause(bool),
}

/// Audio thread to game thread.
pub enum Event {
    /// A sound or stream finished (all its instances).
    Ended(u32),
    /// Data to drop.
    Garbage(Garbage),
}

/// What a stream slot plays.
pub enum StreamData {
    /// A mono VAG file.
    Vag(Arc<VagSound>),
    /// A multi-channel stream.
    Vpk(Arc<VpkStream>),
}

impl StreamData {
    fn channel(&self, c: usize) -> &[u8] {
        match self {
            StreamData::Vag(v) => {
                if c == 0 {
                    v.adpcm()
                } else {
                    &[]
                }
            }
            StreamData::Vpk(s) => s.channels.get(c).map_or(&[], |d| d.as_slice()),
        }
    }

    fn into_garbage(self) -> Garbage {
        match self {
            StreamData::Vag(v) => Garbage::Stream(v),
            StreamData::Vpk(s) => Garbage::Vpk(s),
        }
    }
}

/// A playing stream.
pub struct StreamSlot {
    data: Option<StreamData>,
    handle: u32,
    group: u8,
    voices: [usize; MAX_STREAM_CHANNELS],
    channels: usize,
    mix: Mix,
    layers: [f32; LAYERS],
    layer_target: [f32; LAYERS],
    layer_step: f32,
    chan_gain: [f32; MAX_STREAM_CHANNELS],
    chan_target: [f32; MAX_STREAM_CHANNELS],
    chan_step: f32,
    chan_pan: [f32; MAX_STREAM_CHANNELS],
    fading_out: bool,
}

impl StreamSlot {
    const EMPTY: StreamSlot = StreamSlot {
        data: None,
        handle: 0,
        group: 0,
        voices: [usize::MAX; MAX_STREAM_CHANNELS],
        channels: 0,
        mix: Mix::UNITY,
        layers: [1.0; LAYERS],
        layer_target: [1.0; LAYERS],
        layer_step: 1.0,
        chan_gain: [1.0; MAX_STREAM_CHANNELS],
        chan_target: [1.0; MAX_STREAM_CHANNELS],
        chan_step: 1.0,
        chan_pan: [0.0; MAX_STREAM_CHANNELS],
        fading_out: false,
    };
}

/// The `Arc`s the renderer plays from.
pub struct Slots {
    banks: [Option<Arc<EffectBank>>; BANK_SLOTS],
    music: Option<Arc<MusicFile>>,
    streams: [StreamSlot; MAX_STREAMS],
}

impl SampleData for Slots {
    fn data(&self, source: Source) -> &[u8] {
        match source {
            Source::Bank(b) => self.banks.get(b as usize).and_then(|b| b.as_deref()).map_or(&[], |b| b.sample_data()),
            Source::Music => self.music.as_deref().map_or(&[], |m| m.sample_data()),
            Source::Stream { slot, channel } => self
                .streams
                .get(slot as usize)
                .and_then(|s| s.data.as_ref())
                .map_or(&[], |d| d.channel(channel as usize)),
        }
    }
}

const DEFERRED: usize = 32;

/// Gain change per scheduler tick for a fade of `seconds` (instant for 0).
fn fade_step(seconds: f32) -> f32 {
    if seconds <= 0.0 { 1.0 } else { 1.0 / (seconds * crate::mixer::TICK_RATE as f32) }
}

/// Leaves -0.8..0.8 untouched and bends larger values smoothly toward +-1 instead of clipping them (a dense mix,
/// such as music at alarm intensity, can sum past full scale).
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE {
        return x;
    }
    let t = (a - KNEE) / (1.0 - KNEE);
    let y = KNEE + (1.0 - KNEE) * t / (1.0 + t);
    if x < 0.0 { -y } else { y }
}

/// The audio-thread half: implements [`AudioRender`]. Created by [`create`].
pub struct AudioRenderer {
    rx: Consumer<Command>,
    tx: Producer<Event>,
    mixer: Mixer,
    data: Slots,
    instances: [Instance; MAX_INSTANCES],
    seq: Sequencer,
    groups: [f32; 4],
    duck: f32,
    music_volume: f32,
    globals: [i32; GLOBAL_REGISTERS],
    rng: Rng,
    frames_to_tick: usize,
    deferred: [Option<Event>; DEFERRED],
    paused: bool,
    ticks: u64,
    /// The attached PCM stream input, mixed after the groups.
    pcm: Option<PcmReader>,
}

/// Creates the two halves. Give the renderer to the platform (`start_audio(Box::new(renderer))`) and keep the
/// engine on the game thread.
pub fn create(config: Config) -> (SoundEngine, AudioRenderer) {
    let (ctx, crx) = channel(config.queue);
    let (etx, erx) = channel(config.queue * 2);
    let mut seq = Sequencer::new();
    seq.legacy_tuning = config.legacy_music_tuning;
    let renderer = AudioRenderer {
        rx: crx,
        tx: etx,
        mixer: Mixer::new(config.effect_voices, config.music_voices, config.stream_voices),
        data: Slots { banks: Default::default(), music: None, streams: core::array::from_fn(|_| StreamSlot::EMPTY) },
        instances: [Instance::FREE; MAX_INSTANCES],
        seq,
        groups: [1.0; 4],
        duck: 1.0,
        music_volume: 1.0,
        globals: [127; GLOBAL_REGISTERS],
        rng: Rng::new(config.seed),
        frames_to_tick: 0,
        deferred: Default::default(),
        paused: false,
        ticks: 0,
        pcm: None,
    };
    let engine = SoundEngine {
        tx: ctx,
        rx: erx,
        overflow: VecDeque::new(),
        next_handle: 1,
        active: Vec::new(),
        banks: Default::default(),
        bank_counts: [0; BANK_SLOTS],
        sound_map: None,
        sound_flags: None,
        music_stack: Vec::new(),
        mood: MusicMood::default(),
        characters: [None; 3],
        last_layers: None,
        resident: None,
        falloffs: Vec::new(),
        next_falloff: 1,
        listener: Listener::default(),
        requests: Vec::new(),
        next_request: 1,
        intensity_level: 0,
        intensity_sent: 20,
        fall_counter: 0,
        intermittent: Vec::new(),
        rng: Rng::new(config.seed ^ 0xA5A5_5A5A),
    };
    (engine, renderer)
}

impl AudioRender for AudioRenderer {
    fn render(&mut self, out: &mut [f32]) {
        AudioRenderer::render(self, out);
    }
}

impl AudioRenderer {
    /// Fills `out` (interleaved stereo, 48 kHz) with the next frames.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        if self.paused {
            self.drain_commands();
            return;
        }
        let frames = out.len() / 2;
        let mut done = 0;
        while done < frames {
            if self.frames_to_tick == 0 {
                self.tick();
                self.frames_to_tick = TICK_FRAMES;
                if self.paused {
                    return;
                }
            }
            let n = (frames - done).min(self.frames_to_tick);
            self.mixer.mix(&self.data, &mut out[done * 2..(done + n) * 2], n);
            done += n;
            self.frames_to_tick -= n;
        }
        if let Some(p) = &mut self.pcm {
            p.mix_into(out);
        }
        for s in out.iter_mut() {
            *s = soft_clip(*s);
        }
    }

    /// Scheduler ticks run so far.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Voices playing now.
    pub fn active_voices(&self) -> usize {
        self.mixer.voices.iter().filter(|v| v.active).count()
    }

    /// The sequencer (for inspection).
    pub fn sequencer(&self) -> &Sequencer {
        &self.seq
    }

    fn send(&mut self, ev: Event) {
        if let Err(ev) = self.tx.push(ev) {
            match self.deferred.iter_mut().find(|d| d.is_none()) {
                Some(slot) => *slot = Some(ev),
                // Both full: never free memory on this thread; leak instead (only if the game stops calling
                // `update` for a long time).
                None => core::mem::forget(ev),
            }
        }
    }

    fn retry_deferred(&mut self) {
        for i in 0..DEFERRED {
            if let Some(ev) = self.deferred[i].take()
                && let Err(ev) = self.tx.push(ev)
            {
                self.deferred[i] = Some(ev);
                return;
            }
        }
    }

    fn drain_commands(&mut self) {
        while let Some(cmd) = self.rx.pop() {
            self.command(cmd);
        }
    }

    fn end_handle_if_done(&mut self, handle: u32) {
        let busy = self.instances.iter().any(|i| i.state != State::Free && i.handle == handle)
            || self.data.streams.iter().any(|s| s.data.is_some() && s.handle == handle);
        if !busy {
            self.send(Event::Ended(handle));
        }
    }

    fn command(&mut self, cmd: Command) {
        match cmd {
            Command::AttachPcm(reader) => {
                if let Some(old) = core::mem::replace(&mut self.pcm, reader) {
                    self.send(Event::Garbage(Garbage::Pcm(old)));
                }
            }
            Command::LoadBank { index, bank } => {
                let index = index as usize;
                if index >= BANK_SLOTS {
                    if let Some(b) = bank {
                        self.send(Event::Garbage(Garbage::Bank(b)));
                    }
                    return;
                }
                self.mixer.kill_where(|v| v.p.source == Source::Bank(index as u8));
                for k in 0..MAX_INSTANCES {
                    if self.instances[k].state != State::Free && self.instances[k].bank as usize == index {
                        let h = self.instances[k].handle;
                        self.instances[k].state = State::Free;
                        self.end_handle_if_done(h);
                    }
                }
                if let Some(old) = core::mem::replace(&mut self.data.banks[index], bank) {
                    self.send(Event::Garbage(Garbage::Bank(old)));
                }
            }
            Command::Play { handle, bank, sound, mix } => {
                let inst = self
                    .data
                    .banks
                    .get(bank as usize)
                    .and_then(|b| b.as_deref())
                    .and_then(|b| Instance::new(b, bank, sound, handle, mix));
                match (inst, self.instances.iter().position(|i| i.state == State::Free)) {
                    // It runs its first steps in this tick's instance pass.
                    (Some(inst), Some(k)) => self.instances[k] = inst,
                    _ => self.send(Event::Ended(handle)),
                }
            }
            Command::PlayStream { handle, sound, group, looped, mix } => {
                self.start_stream(handle, StreamData::Vag(sound), group, looped, mix)
            }
            Command::PlayVpk { handle, stream, group, looped, mix } => {
                self.start_stream(handle, StreamData::Vpk(stream), group, looped, mix)
            }
            Command::SetLayers { handle, gains, fade } => {
                let step = fade_step(fade);
                for s in self.data.streams.iter_mut().filter(|s| s.data.is_some() && s.handle == handle) {
                    s.layer_target = gains.map(|x| x.clamp(0.0, 1.0));
                    s.layer_step = step;
                }
            }
            Command::SetChannels { handle, gains, pans, fade } => {
                let step = fade_step(fade);
                for s in self.data.streams.iter_mut().filter(|s| s.data.is_some() && s.handle == handle) {
                    if !s.fading_out {
                        s.chan_target = gains.map(|x| x.clamp(0.0, 4.0));
                        s.chan_step = step;
                    }
                    s.chan_pan = pans;
                }
            }
            Command::FadeOut { handle, seconds } => {
                let step = fade_step(seconds);
                for s in self.data.streams.iter_mut().filter(|s| s.data.is_some() && s.handle == handle) {
                    s.chan_target = [0.0; MAX_STREAM_CHANNELS];
                    s.chan_step = step;
                    s.fading_out = true;
                }
            }
            Command::SetMix { handle, mix } => {
                for i in self.instances.iter_mut().filter(|i| i.state != State::Free && i.handle == handle) {
                    i.mix = mix;
                }
                for s in self.data.streams.iter_mut().filter(|s| s.data.is_some() && s.handle == handle) {
                    s.mix = mix;
                }
            }
            Command::Stop { handle } => self.stop_where(|h| h == handle),
            Command::StopAll => self.stop_where(|_| true),
            Command::PlayMusic { music, segment } => {
                self.mixer.kill_where(|v| v.p.source == Source::Music);
                self.seq.reset();
                self.seq.start(&music, segment);
                if let Some(old) = self.data.music.replace(music) {
                    self.send(Event::Garbage(Garbage::Music(old)));
                }
            }
            Command::StopMusic { unload } => {
                self.seq.stop_all();
                if unload {
                    self.mixer.kill_where(|v| v.p.source == Source::Music);
                    if let Some(old) = self.data.music.take() {
                        self.send(Event::Garbage(Garbage::Music(old)));
                    }
                } else {
                    self.mixer.release_where(|v| v.p.source == Source::Music);
                }
            }
            Command::SetMusicRegister { register, value } => {
                self.seq.registers[register as usize % crate::sequencer::REGISTERS] = value.min(127)
            }
            Command::SetIntensity(v) => self.seq.intensity = v.min(127),
            Command::SetGroupVolume { group, volume } => {
                if let Some(g) = self.groups.get_mut(group as usize) {
                    *g = volume.clamp(0.0, 4.0);
                }
            }
            Command::SetMusicVolume(v) => self.music_volume = v.clamp(0.0, 4.0),
            Command::SetReverb { kind, depth } => self.mixer.reverb.set(kind, depth),
            Command::SetGlobalRegister { register, value } => {
                if let Some(g) = self.globals.get_mut(register as usize) {
                    *g = value;
                }
            }
            Command::Pause(p) => self.paused = p,
        }
    }

    fn stop_where(&mut self, f: impl Fn(u32) -> bool) {
        for k in 0..MAX_INSTANCES {
            let i = &mut self.instances[k];
            if i.state == State::Free || !f(i.handle) {
                continue;
            }
            // A sound with a stop marker (Sly 2 kind 43) runs its stop steps; the others end.
            if !i.request_stop() && i.state == State::Running {
                i.state = State::Draining;
            }
            self.mixer.release_where(|v| v.p.owner == Owner::Effect { instance: k as u8 });
        }
        for s in 0..MAX_STREAMS {
            if self.data.streams[s].data.is_some() && f(self.data.streams[s].handle) {
                self.mixer.release_where(|v| v.p.owner == Owner::Stream { slot: s as u8 });
            }
        }
    }

    /// Runs instance `k` for this tick, and any children it starts in slots before it (children in later slots run
    /// when the tick's pass reaches them). Each instance runs at most once per tick with its own step budget, so
    /// the work per tick is bounded by `MAX_INSTANCES x STEPS_PER_TICK` steps whatever the data does.
    fn run_instance(&mut self, k: usize) {
        let tick = self.ticks;
        let mut pending = [0u8; MAX_INSTANCES];
        let mut n = 1;
        pending[0] = k as u8;
        while n > 0 {
            n -= 1;
            let k = pending[n] as usize;
            if self.instances[k].state != State::Running || self.instances[k].ran_tick == tick {
                continue;
            }
            self.instances[k].ran_tick = tick;
            let b = self.instances[k].bank as usize;
            let Some(bank) = self.data.banks[b].as_deref() else {
                self.instances[k].state = State::Draining;
                continue;
            };
            let mut ctx = StepContext { bank, mixer: &mut self.mixer, rng: &mut self.rng, globals: &mut self.globals };
            let mut budget = effects::STEPS_PER_TICK;
            while let Some(child) = effects::run(&mut self.instances, k, &mut ctx, &mut budget) {
                let parent = self.instances[k];
                if let Some(mut c) = Instance::new(bank, parent.bank, child.sound, parent.handle, parent.mix)
                    && let Some(j) = self.instances.iter().position(|i| i.state == State::Free)
                {
                    c.volume *= child.volume;
                    c.pan += child.pan;
                    self.instances[j] = c;
                    if j < k && n < MAX_INSTANCES {
                        pending[n] = j as u8;
                        n += 1;
                    }
                }
            }
        }
    }

    fn start_stream(&mut self, handle: u32, data: StreamData, group: u8, looped: bool, mix: Mix) {
        let (rate, channels): (u32, usize) = match &data {
            StreamData::Vag(v) => (v.header.sample_rate, 1),
            StreamData::Vpk(s) => (s.header.sample_rate, s.channel_count().min(MAX_STREAM_CHANNELS)),
        };
        let free = self.data.streams.iter().position(|s| s.data.is_none());
        let free_voices = self.mixer.pool(Pool::Streams).filter(|&i| !self.mixer.voices[i].active).count();
        let (Some(slot), true) = (free, free_voices >= channels && channels > 0) else {
            self.send(Event::Garbage(data.into_garbage()));
            self.send(Event::Ended(handle));
            return;
        };
        let group = group.min(3);
        let mut voices = [usize::MAX; MAX_STREAM_CHANNELS];
        for (c, v) in voices.iter_mut().enumerate().take(channels) {
            let Some(i) = self.mixer.alloc(Pool::Streams, i8::MAX) else { break };
            *v = i;
            self.mixer.start(
                i,
                VoiceStart {
                    source: Source::Stream { slot: slot as u8, channel: c as u8 },
                    offset: 0,
                    envelope: Envelope::constant(),
                    owner: Owner::Stream { slot: slot as u8 },
                    priority: i8::MAX,
                    group,
                    reverb: false,
                    volume: 1.0,
                    pan: 0.0,
                    semitones: 0.0,
                    rate_scale: rate as f32 / 48_000.0,
                    bend_down: 0.0,
                    bend_up: 0.0,
                    wrap: looped,
                },
            );
        }
        self.data.streams[slot] =
            StreamSlot { data: Some(data), handle, group, voices, channels, mix, ..StreamSlot::EMPTY };
        self.update_stream_voice(slot);
    }

    /// Amplitude scale of a mix group: its master volume, the dialogue duck (not on dialogue), and for music the
    /// world's music volume and the hiding scale.
    fn group_scale(&self, g: u8) -> f32 {
        let base = self.groups[g.min(3) as usize];
        match g {
            group::DIALOGUE => base,
            group::MUSIC => {
                let level = self.seq.intensity as i32 - 20;
                let calm = if level < 0 { 1.0 + level as f32 / 60.0 } else { 1.0 };
                base * self.music_volume * calm * self.duck
            }
            _ => base * self.duck,
        }
    }

    fn update_stream_voice(&mut self, s: usize) {
        let scale = self.group_scale(self.data.streams[s].group);
        let slot = &mut self.data.streams[s];
        for (cur, t) in slot.layers.iter_mut().zip(slot.layer_target) {
            *cur += (t - *cur).clamp(-slot.layer_step, slot.layer_step);
        }
        for (cur, t) in slot.chan_gain.iter_mut().zip(slot.chan_target) {
            *cur += (t - *cur).clamp(-slot.chan_step, slot.chan_step);
        }
        if slot.fading_out && slot.chan_gain.iter().all(|&g| g <= 0.0) {
            // Faded out: the stream ends (its slot is retired on the next tick).
            let owner = Owner::Stream { slot: s as u8 };
            self.mixer.kill_where(|v| v.p.owner == owner);
            return;
        }
        let slot = &self.data.streams[s];
        for c in 0..slot.channels {
            let v = slot.voices[c];
            if v >= self.mixer.voices.len() {
                continue;
            }
            let (vol, pan, layer) = match &slot.data {
                Some(StreamData::Vpk(st)) => {
                    let ch = st.header.channels[c];
                    (ch.volume.min(127) as f32 / 127.0, ch.pan as f32, slot.layers[ch.group as usize % LAYERS])
                }
                _ => (1.0, 0.0, 1.0),
            };
            let lin = slot.mix.gain * vol;
            let amp = lin * lin * layer * slot.chan_gain[c] * scale;
            let ratio = fmath::semitone_ratio(slot.mix.pitch as f64) * self.mixer.voices[v].p.rate_scale as f64;
            self.mixer.set_target(v, amp, slot.mix.pan + pan + slot.chan_pan[c], ratio, 0.0);
        }
    }

    fn tick(&mut self) {
        self.ticks += 1;
        self.retry_deferred();
        self.drain_commands();
        if self.paused {
            return;
        }

        // Count each instance's voices.
        for i in self.instances.iter_mut() {
            i.voices = 0;
        }
        for v in &self.mixer.voices {
            if v.active
                && let Owner::Effect { instance } = v.p.owner
            {
                self.instances[instance as usize].voices += 1;
            }
        }
        for k in 0..MAX_INSTANCES {
            if self.instances[k].state == State::Running && self.instances[k].ran_tick != self.ticks {
                self.run_instance(k);
            }
        }
        for k in 0..MAX_INSTANCES {
            let i = self.instances[k];
            if i.state == State::Draining && i.voices == 0 {
                self.instances[k].state = State::Free;
                self.end_handle_if_done(i.handle);
            }
        }

        if let Some(m) = self.data.music.as_deref() {
            self.seq.tick(m, &mut self.mixer);
        }

        // Streams that finished.
        let mut dialogue = false;
        for s in 0..MAX_STREAMS {
            let slot = &self.data.streams[s];
            if slot.data.is_none() {
                continue;
            }
            let owner = Owner::Stream { slot: s as u8 };
            let playing = slot.voices[..slot.channels]
                .iter()
                .any(|&v| self.mixer.voices.get(v).is_some_and(|v| v.active && v.p.owner == owner));
            if playing {
                dialogue |= slot.group == group::DIALOGUE;
                continue;
            }
            let handle = slot.handle;
            // Voices of this slot that are still around (a partial stream) stop with it.
            self.mixer.kill_where(|v| v.p.owner == owner);
            if let Some(data) = self.data.streams[s].data.take() {
                self.send(Event::Garbage(data.into_garbage()));
            }
            self.end_handle_if_done(handle);
        }

        // Dialogue ducks the other groups to half (smoothly).
        let target = if dialogue { 0.5 } else { 1.0 };
        self.duck += (target - self.duck).clamp(-0.05, 0.05);

        self.update_voices();
    }

    fn update_voices(&mut self) {
        for i in self.mixer.pool(Pool::Effects) {
            let v = self.mixer.voices[i];
            if !v.active {
                continue;
            }
            let Owner::Effect { instance } = v.p.owner else { continue };
            let inst = &self.instances[instance as usize];
            let lin = inst.volume * v.p.volume * inst.mix.gain;
            let amp = lin * lin * self.groups[v.p.group as usize] * self.duck;
            let pan = inst.pan + v.p.pan + inst.mix.pan;
            let semis = v.p.semitones + v.bend_semitones(inst.bend) + inst.mix.pitch;
            let ratio = fmath::semitone_ratio(semis as f64);
            self.mixer.set_target(i, amp, pan, ratio, if v.p.reverb { amp } else { 0.0 });
        }
        let master = self.group_scale(group::MUSIC);
        self.seq.update_voices(&mut self.mixer, master);
        for s in 0..MAX_STREAMS {
            if self.data.streams[s].data.is_some() {
                self.update_stream_voice(s);
            }
        }
    }
}

/// A resident stream's per-channel gains and pans.
type ChannelMix = ([f32; MAX_STREAM_CHANNELS], [f32; MAX_STREAM_CHANNELS]);

/// Fade time when a music stream is pushed over or popped, seconds (not given by the spec).
pub const MUSIC_FADE: f32 = 1.0;
/// Most music streams on the stack.
pub const MUSIC_STACK: usize = 8;

/// A fall-off added with [`SoundEngine::add_falloff`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FalloffId(u32);

/// A playing sound, as the game sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SoundHandle(pub u32);

impl SoundHandle {
    /// "No sound" (a request that was ignored).
    pub const NONE: SoundHandle = SoundHandle(0);

    /// Whether this names a sound that was started.
    pub fn is_some(self) -> bool {
        self.0 != 0
    }
}

/// A request for an intensity level (see [`SoundEngine::request_intensity`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntensityRequest(u32);

/// An intermittent world sound (options 549, 550, 551).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intermittent {
    /// Sound ID.
    pub sound_id: u16,
    /// Volume range (min, max), 0..1.
    pub volume: (f32, f32),
    /// Repeat interval range in seconds (min, max).
    pub interval: (f32, f32),
}

struct IntermittentState {
    spec: Intermittent,
    timer: f32,
}

#[derive(Clone, Copy, Debug)]
struct Active {
    handle: u32,
    emitter: Emitter,
    last: Mix,
}

/// The game-thread half: start and stop sounds, move emitters and the listener, run the music and intensity, load
/// banks. Call [`SoundEngine::update`] once per frame.
pub struct SoundEngine {
    tx: Producer<Command>,
    rx: Consumer<Event>,
    overflow: VecDeque<Command>,
    next_handle: u32,
    active: Vec<Active>,
    banks: [Option<Arc<EffectBank>>; BANK_SLOTS],
    bank_counts: [u16; BANK_SLOTS],
    sound_map: Option<Vec<Option<u16>>>,
    sound_flags: Option<Vec<u8>>,
    music_stack: Vec<(Arc<VpkStream>, SoundHandle)>,
    mood: MusicMood,
    characters: [Option<Vec3>; 3],
    last_layers: Option<[f32; LAYERS]>,
    /// Resident stream: handle, channel numbers, last channel gains and pans sent.
    resident: Option<(SoundHandle, Vec<i16>, Option<ChannelMix>)>,
    falloffs: Vec<(u32, Falloff)>,
    next_falloff: u32,
    listener: Listener,
    requests: Vec<(u32, i32)>,
    next_request: u32,
    intensity_level: i32,
    intensity_sent: u8,
    fall_counter: u32,
    intermittent: Vec<IntermittentState>,
    rng: Rng,
}

impl SoundEngine {
    fn send(&mut self, cmd: Command) {
        if !self.overflow.is_empty() {
            self.flush();
        }
        if self.overflow.is_empty() {
            if let Err(cmd) = self.tx.push(cmd) {
                self.overflow.push_back(cmd);
            }
        } else {
            self.overflow.push_back(cmd);
        }
    }

    fn flush(&mut self) {
        while let Some(cmd) = self.overflow.pop_front() {
            if let Err(cmd) = self.tx.push(cmd) {
                self.overflow.push_front(cmd);
                break;
            }
        }
    }

    fn new_handle(&mut self) -> u32 {
        let h = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        h
    }

    /// Sets the sound count of each of the 7 banks, which numbers the sound IDs (audio.md 3.6). Loading a bank
    /// also records its count.
    pub fn set_bank_counts(&mut self, counts: [u16; BANK_SLOTS]) {
        self.bank_counts = counts;
    }

    /// Uses a per-world map from game sound IDs to the sounds of the bank in slot 0 (Sly 2,
    /// [`crate::sly2::WorldSound::sound_map`]) instead of the Sly 1 numbering; `None` goes back to it.
    pub fn set_sound_map(&mut self, map: Option<Vec<Option<u16>>>) {
        self.sound_map = map;
    }

    /// Sets the Sly 2 per-sound flag bytes of the bank in slot 0 (flag 2 = continuous, in addition to the sound
    /// record's own continuous bit); `None` clears them.
    pub fn set_sound_flags(&mut self, flags: Option<Vec<u8>>) {
        self.sound_flags = flags;
    }

    /// The bank and sound index of a sound ID.
    pub fn resolve(&self, sound_id: u16) -> Option<(usize, u16)> {
        if let Some(map) = &self.sound_map {
            return map.get(sound_id as usize).copied().flatten().map(|i| (0, i));
        }
        let mut base = 0u32;
        for (b, &n) in self.bank_counts.iter().enumerate() {
            if (sound_id as u32) < base + n as u32 {
                return Some((b, (sound_id as u32 - base) as u16));
            }
            base += n as u32;
        }
        None
    }

    /// Puts `bank` in slot `index` (0..7); `None` unloads the slot. Sounds playing from the old bank stop.
    pub fn load_bank(&mut self, index: usize, bank: Option<Arc<EffectBank>>) {
        if index >= BANK_SLOTS {
            return;
        }
        if let Some(b) = &bank {
            self.bank_counts[index] = b.sounds.len() as u16;
        }
        self.banks[index] = bank.clone();
        self.send(Command::LoadBank { index: index as u8, bank });
    }

    /// The bank in slot `index`.
    pub fn bank(&self, index: usize) -> Option<&Arc<EffectBank>> {
        self.banks.get(index)?.as_ref()
    }

    /// Plays sound `sound_id` from `emitter`. Returns [`SoundHandle::NONE`] when the sound's bank is not loaded, or
    /// when a one-shot sound starts out of range (a continuous sound starts silently instead).
    pub fn play(&mut self, sound_id: u16, emitter: Emitter) -> SoundHandle {
        let Some((b, index)) = self.resolve(sound_id) else { return SoundHandle::NONE };
        let Some(bank) = self.banks[b].as_ref() else { return SoundHandle::NONE };
        let Some(rec) = bank.sounds.get(index as usize) else { return SoundHandle::NONE };
        let flagged = b == 0 && self.sound_flags.as_ref().is_some_and(|f| f.get(index as usize) == Some(&2));
        let continuous = rec.continuous() || flagged;
        let mix = match mix_for(&self.listener, &emitter) {
            Some(m) => m,
            None if continuous => Mix { gain: 0.0, pan: 0.0, pitch: emitter.pitch },
            None => return SoundHandle::NONE,
        };
        let handle = self.new_handle();
        self.active.push(Active { handle, emitter, last: mix });
        self.send(Command::Play { handle, bank: b as u8, sound: index, mix });
        SoundHandle(handle)
    }

    /// Plays a dialogue line (mix group 2; the other groups drop to half volume while it plays).
    pub fn play_dialogue(&mut self, line: Arc<VagSound>, emitter: Emitter) -> SoundHandle {
        self.play_stream(line, group::DIALOGUE, false, emitter)
    }

    /// Plays a VAG sound in `group`, once or looped (ambient loops).
    pub fn play_stream(&mut self, sound: Arc<VagSound>, group: u8, looped: bool, emitter: Emitter) -> SoundHandle {
        let mix = mix_for(&self.listener, &emitter).unwrap_or(Mix { gain: 0.0, pan: 0.0, pitch: emitter.pitch });
        let handle = self.new_handle();
        self.active.push(Active { handle, emitter, last: mix });
        self.send(Command::PlayStream { handle, sound, group, looped, mix });
        SoundHandle(handle)
    }

    /// Plays a multi-channel `VPK ` stream in `group` (Sly 2 ambient and other long sounds).
    pub fn play_vpk(&mut self, stream: Arc<VpkStream>, group: u8, looped: bool, emitter: Emitter) -> SoundHandle {
        let mix = mix_for(&self.listener, &emitter).unwrap_or(Mix { gain: 0.0, pan: 0.0, pitch: emitter.pitch });
        let handle = self.new_handle();
        self.active.push(Active { handle, emitter, last: mix });
        self.send(Command::PlayVpk { handle, stream, group, looped, mix });
        SoundHandle(handle)
    }

    /// Sets a stream's layer gains (0..1 per channel group number), fading over `fade` seconds.
    pub fn set_layers(&mut self, handle: SoundHandle, gains: [f32; LAYERS], fade: f32) {
        if handle.is_some() {
            self.send(Command::SetLayers { handle: handle.0, gains, fade });
        }
    }

    /// Fades a stream out over `seconds`, then stops it.
    pub fn fade_out(&mut self, handle: SoundHandle, seconds: f32) {
        if handle.is_some() {
            self.send(Command::FadeOut { handle: handle.0, seconds });
        }
    }

    /// Starts a world's resident stream (Sly 2, audio.md 3.1): every channel loops, in the effects group, at its
    /// stored volume until fall-offs ([`SoundEngine::add_falloff`]) name its channel number. `channel_numbers` are
    /// the world sound section's numbers, one per channel. Replaces any resident stream already playing.
    pub fn play_resident(&mut self, stream: Arc<VpkStream>, channel_numbers: Vec<i16>) -> SoundHandle {
        if let Some(r) = self.resident.take() {
            self.stop(r.0);
        }
        let h = self.play_vpk(stream, group::EFFECTS, true, Emitter::global());
        self.resident = Some((h, channel_numbers, None));
        h
    }

    /// Adds a distance fall-off to a resident-stream channel (an ambient object's option 100 record); returns its
    /// id.
    pub fn add_falloff(&mut self, falloff: Falloff) -> FalloffId {
        let id = self.next_falloff;
        self.next_falloff = self.next_falloff.wrapping_add(1).max(1);
        self.falloffs.push((id, falloff));
        FalloffId(id)
    }

    /// Moves a fall-off's object.
    pub fn set_falloff_position(&mut self, id: FalloffId, position: Vec3) {
        if let Some(f) = self.falloffs.iter_mut().find(|f| f.0 == id.0) {
            f.1.position = position;
        }
    }

    /// Removes a fall-off (its object is gone).
    pub fn remove_falloff(&mut self, id: FalloffId) {
        self.falloffs.retain(|f| f.0 != id.0);
    }

    /// Starts a music stream on top of the music stack (Sly 2, audio.md 3.2): the current top fades out, the new
    /// one loops in the music group with its layers weighted by [`SoundEngine::music_mood`]. The stack holds up to
    /// 8; a ninth push drops the bottom.
    pub fn push_music(&mut self, stream: Arc<VpkStream>) -> SoundHandle {
        if let Some(top) = self.music_stack.last_mut() {
            let h = top.1;
            top.1 = SoundHandle::NONE;
            self.fade_out(h, MUSIC_FADE);
        }
        if self.music_stack.len() == MUSIC_STACK {
            self.music_stack.remove(0);
        }
        let h = self.play_vpk(stream.clone(), group::MUSIC, true, Emitter::global());
        self.music_stack.push((stream, h));
        self.last_layers = None;
        h
    }

    /// Stops the top music stream (fading out) and restarts the one below it, if any.
    pub fn pop_music(&mut self) {
        if let Some((_, h)) = self.music_stack.pop() {
            self.fade_out(h, MUSIC_FADE);
        }
        if let Some(stream) = self.music_stack.last().map(|t| t.0.clone()) {
            let h = self.play_vpk(stream, group::MUSIC, true, Emitter::global());
            if let Some(top) = self.music_stack.last_mut() {
                top.1 = h;
            }
            self.last_layers = None;
        }
    }

    /// The handle of the music stream playing on top of the stack.
    pub fn music_top(&self) -> SoundHandle {
        self.music_stack.last().map_or(SoundHandle::NONE, |t| t.1)
    }

    /// The sneak / normal / alert state that weights the music layers (counters and the character's state).
    pub fn music_mood(&mut self) -> &mut MusicMood {
        &mut self.mood
    }

    /// Positions of the three playable characters (groups 3 and 4 of the music fade with their distance).
    pub fn set_characters(&mut self, characters: [Option<Vec3>; 3]) {
        self.characters = characters;
    }

    /// Stops a sound: its steps end and its voices release.
    pub fn stop(&mut self, handle: SoundHandle) {
        if handle.is_some() {
            self.send(Command::Stop { handle: handle.0 });
        }
    }

    /// Stops every sound and stream.
    pub fn stop_all(&mut self) {
        self.send(Command::StopAll);
    }

    /// Whether a sound is still playing (as of the last [`SoundEngine::update`]).
    pub fn is_playing(&self, handle: SoundHandle) -> bool {
        handle.is_some() && self.active.iter().any(|a| a.handle == handle.0)
    }

    /// Number of sounds and streams playing (as of the last update).
    pub fn playing_count(&self) -> usize {
        self.active.len()
    }

    /// Moves a playing sound's emitter (the mix follows on the next update).
    pub fn set_emitter(&mut self, handle: SoundHandle, emitter: Emitter) {
        if let Some(a) = self.active.iter_mut().find(|a| a.handle == handle.0) {
            a.emitter = emitter;
        }
    }

    /// Sets the listener (the camera).
    pub fn set_listener(&mut self, listener: Listener) {
        self.listener = listener;
    }

    /// The listener.
    pub fn listener(&self) -> &Listener {
        &self.listener
    }

    /// Loads `music` and starts it at segment 0.
    pub fn play_music(&mut self, music: Arc<MusicFile>) {
        self.play_music_at(music, 0);
    }

    /// Loads `music` and starts it at `segment`.
    pub fn play_music_at(&mut self, music: Arc<MusicFile>, segment: u8) {
        self.send(Command::PlayMusic { music, segment });
    }

    /// Stops the music (`unload` also drops its data).
    pub fn stop_music(&mut self, unload: bool) {
        self.send(Command::StopMusic { unload });
    }

    /// Sets a music register (scripts can do this).
    pub fn set_music_register(&mut self, register: u8, value: u8) {
        self.send(Command::SetMusicRegister { register, value });
    }

    /// Sets the world's music volume factor (option 525; 1.0 normal). Use 0 for option 524 "music off".
    pub fn set_music_volume(&mut self, volume: f32) {
        self.send(Command::SetMusicVolume(volume));
    }

    /// Sets a mix group's master volume (0..1).
    pub fn set_group_volume(&mut self, group: u8, volume: f32) {
        self.send(Command::SetGroupVolume { group, volume });
    }

    /// Sets the reverb (option 552).
    pub fn set_reverb(&mut self, kind: u32, depth: u32) {
        self.send(Command::SetReverb { kind, depth });
    }

    /// Sets a global register that voice volumes and pans can read.
    pub fn set_global_register(&mut self, register: u8, value: i32) {
        self.send(Command::SetGlobalRegister { register, value });
    }

    /// Attaches a PCM stream input ([`crate::pcm::pcm_stream`]): its frames are mixed after the groups, before the
    /// output's soft clip, from the next render on. `None` detaches it. One stream at a time; attaching another
    /// replaces it. The reader is dropped on this thread once the renderer gives it back.
    pub fn attach_pcm(&mut self, reader: Option<crate::pcm::PcmReader>) {
        self.send(Command::AttachPcm(reader));
    }

    /// Pauses (silences) or resumes all audio, for example on `Suspended` / `Resumed`.
    pub fn set_paused(&mut self, paused: bool) {
        self.send(Command::Pause(paused));
    }

    /// Requests an intensity level (-20..107); the music follows the highest active request.
    pub fn request_intensity(&mut self, level: i32) -> IntensityRequest {
        let id = self.next_request;
        self.next_request = self.next_request.wrapping_add(1).max(1);
        self.requests.push((id, level.clamp(INTENSITY_HIDING, INTENSITY_ALARM)));
        IntensityRequest(id)
    }

    /// Removes an intensity request.
    pub fn remove_intensity(&mut self, request: IntensityRequest) {
        self.requests.retain(|r| r.0 != request.0);
    }

    /// The current intensity level (-20..107).
    pub fn intensity(&self) -> i32 {
        self.intensity_level
    }

    /// Adds an intermittent world sound.
    pub fn add_intermittent(&mut self, spec: Intermittent) {
        let timer = self.rng.range_f32(spec.interval.0, spec.interval.1);
        self.intermittent.push(IntermittentState { spec, timer });
    }

    /// Removes all intermittent sounds (world change).
    pub fn clear_intermittent(&mut self) {
        self.intermittent.clear();
    }

    /// Once per frame: collects finished sounds and returned data, moves the intensity, fires intermittent sounds
    /// and sends the new mix of every positioned sound.
    pub fn update(&mut self, dt: f32) {
        self.flush();
        while let Some(ev) = self.rx.pop() {
            match ev {
                Event::Ended(h) => self.active.retain(|a| a.handle != h),
                Event::Garbage(g) => drop(g),
            }
        }

        // Intensity: rises at once, falls one step every three updates.
        let target = self.requests.iter().map(|r| r.1).max().unwrap_or(0);
        if target > self.intensity_level {
            self.intensity_level = target;
            self.fall_counter = 0;
        } else if target < self.intensity_level {
            self.fall_counter += 1;
            if self.fall_counter >= 3 {
                self.fall_counter = 0;
                self.intensity_level -= 1;
            }
        }
        let value = (self.intensity_level + 20).clamp(0, 127) as u8;
        if value != self.intensity_sent {
            self.intensity_sent = value;
            self.send(Command::SetIntensity(value));
        }

        // Intermittent sounds: random volume, random direction (pan) around the listener.
        for i in 0..self.intermittent.len() {
            self.intermittent[i].timer -= dt;
            if self.intermittent[i].timer > 0.0 {
                continue;
            }
            let spec = self.intermittent[i].spec;
            self.intermittent[i].timer = self.rng.range_f32(spec.interval.0, spec.interval.1).max(0.05);
            let mut e = Emitter::global();
            e.volume = self.rng.range_f32(spec.volume.0, spec.volume.1).clamp(0.0, 1.0);
            e.pan = self.rng.range_f32(-180.0, 180.0);
            self.play(spec.sound_id, e);
        }

        // Sly 2 music layers: the top music stream's groups follow the mood and the characters' distances.
        self.mood.update(dt);
        let top = self.music_top();
        if top.is_some() {
            let gains = crate::sly2::music::music_layer_gains(&self.mood, &self.listener, &self.characters);
            let changed = self.last_layers.is_none_or(|l| l.iter().zip(&gains).any(|(a, b)| (a - b).abs() > 1e-4));
            if changed {
                self.last_layers = Some(gains);
                self.send(Command::SetLayers { handle: top.0, gains, fade: 0.0 });
            }
        }
        // Sly 2 resident stream: distance layers.
        if let Some((h, numbers, last)) = self.resident.take() {
            let falloffs: Vec<Falloff> = self.falloffs.iter().map(|f| f.1).collect();
            let mix = crate::sly2::music::resident_channels(&numbers, &falloffs, &self.listener);
            if last != Some(mix) {
                self.send(Command::SetChannels { handle: h.0, gains: mix.0, pans: mix.1, fade: 0.0 });
            }
            self.resident = Some((h, numbers, Some(mix)));
        }

        // Positioned sounds follow the listener and their emitters.
        for k in 0..self.active.len() {
            let a = self.active[k];
            if a.emitter.position.is_none() {
                continue;
            }
            let mix = mix_for(&self.listener, &a.emitter).unwrap_or(Mix { gain: 0.0, ..a.last });
            let changed = (mix.gain - a.last.gain).abs() > 1e-4
                || (mix.pan - a.last.pan).abs() > 0.05
                || (mix.pitch - a.last.pitch).abs() > 1e-3;
            if changed {
                self.active[k].last = mix;
                self.send(Command::SetMix { handle: a.handle, mix });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adpcm::tests::block;
    use crate::adpcm::{FLAG_END, FLAG_LOOP_START, FLAG_REPEAT};
    use crate::bank::tests::{build_bank, voice_data};
    use alloc::vec;

    fn test_bank() -> Arc<EffectBank> {
        let mut samples = vec![];
        samples.extend_from_slice(&block(4, 0, 0, &[7; 28]));
        samples.extend_from_slice(&block(4, 0, FLAG_END, &[-7; 28]));
        samples.extend_from_slice(&block(4, 0, FLAG_LOOP_START, &[5; 28]));
        samples.extend_from_slice(&block(4, 0, FLAG_END | FLAG_REPEAT, &[-5; 28]));
        let one_shot = (100, 0, 0, vec![(1, 0, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0))]);
        let looping = (100, 0, 1, vec![(1, 0, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 32))]);
        Arc::new(EffectBank::parse(build_bank(&[one_shot, looping], &samples)).unwrap())
    }

    fn render(r: &mut AudioRenderer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        r.render(&mut out);
        out
    }

    #[test]
    fn plays_and_reports_end() {
        let (mut e, mut r) = create(Config::default());
        e.set_bank_counts([2, 0, 0, 0, 0, 0, 0]);
        e.load_bank(0, Some(test_bank()));
        let h = e.play(0, Emitter::global());
        assert!(h.is_some() && e.is_playing(h));
        let out = render(&mut r, 512);
        assert!(out.iter().any(|&x| x != 0.0));
        render(&mut r, 4096);
        e.update(1.0 / 60.0);
        assert!(!e.is_playing(h), "one-shot sound should have ended");
        // Sound IDs past the loaded banks are ignored.
        assert_eq!(e.play(5, Emitter::global()), SoundHandle::NONE);
    }

    #[test]
    fn stop_ends_a_continuous_sound() {
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(test_bank()));
        let h = e.play(1, Emitter::global());
        render(&mut r, 48_000);
        e.update(0.0);
        assert!(e.is_playing(h));
        e.stop(h);
        render(&mut r, 4800);
        e.update(0.0);
        assert!(!e.is_playing(h));
        assert_eq!(r.active_voices(), 0);
    }

    #[test]
    fn out_of_range_rules() {
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(test_bank()));
        let far = Emitter::at([1000.0, 0.0, 0.0], 100.0, 10.0);
        assert_eq!(e.play(0, far), SoundHandle::NONE, "one-shot out of range is not started");
        let h = e.play(1, far);
        assert!(h.is_some(), "continuous sound starts silently");
        let out = render(&mut r, 2048);
        assert!(out.iter().all(|&x| x == 0.0));
        // Listener walks up to it: it becomes audible.
        e.set_listener(Listener { position: [995.0, 0.0, 0.0], ..Listener::default() });
        e.update(0.0);
        let out = render(&mut r, 2048);
        assert!(out.iter().any(|&x| x != 0.0));
    }

    #[test]
    fn bank_replacement_returns_the_old_bank() {
        let (mut e, mut r) = create(Config::default());
        let bank = test_bank();
        e.load_bank(0, Some(bank.clone()));
        let h = e.play(1, Emitter::global());
        render(&mut r, 1024);
        e.load_bank(0, None);
        render(&mut r, 256);
        e.update(0.0);
        assert!(!e.is_playing(h));
        // Engine slot cleared, renderer gave its copy back and it was dropped here.
        assert_eq!(Arc::strong_count(&bank), 1);
    }

    #[test]
    fn intensity_rises_at_once_and_falls_slowly() {
        let (mut e, _r) = create(Config::default());
        let req = e.request_intensity(INTENSITY_ALARM);
        e.update(0.0);
        assert_eq!(e.intensity(), 107);
        e.remove_intensity(req);
        for _ in 0..30 {
            e.update(0.0);
        }
        assert_eq!(e.intensity(), 97);
        let hide = e.request_intensity(INTENSITY_HIDING);
        for _ in 0..1000 {
            e.update(0.0);
        }
        assert_eq!(e.intensity(), -20);
        e.remove_intensity(hide);
        e.update(0.0);
        assert_eq!(e.intensity(), 0);
    }

    #[test]
    fn streams_play_resampled_and_duck() {
        let (mut e, mut r) = create(Config::default());
        let mut adpcm = vec![];
        for _ in 0..20 {
            adpcm.extend_from_slice(&block(4, 0, 0, &[6; 28]));
        }
        adpcm.extend_from_slice(&block(4, 0, FLAG_END, &[6; 28]));
        let mut bytes = vec![0u8; 48];
        bytes[0..4].copy_from_slice(b"VAGp");
        bytes[12..16].copy_from_slice(&(adpcm.len() as u32).to_be_bytes());
        bytes[16..20].copy_from_slice(&22_050u32.to_be_bytes());
        bytes.extend_from_slice(&adpcm);
        let line = Arc::new(VagSound::new(bytes).unwrap());
        let h = e.play_dialogue(line.clone(), Emitter::global());
        // 588 samples at 22,050 Hz = 1280 frames at 48 kHz.
        let out = render(&mut r, 1200);
        assert!(out[2 * 1100] > 0.0);
        render(&mut r, 400);
        e.update(0.0);
        assert!(!e.is_playing(h));
        assert_eq!(Arc::strong_count(&line), 1);
    }

    fn child(sound: i16) -> (u32, i32, Vec<u8>) {
        let mut d = vec![0u8; 14];
        d[0] = 127;
        d[12..14].copy_from_slice(&sound.to_le_bytes());
        (5, 0, d)
    }

    #[test]
    fn zero_delay_child_loop_with_full_table_is_bounded() {
        // loop start, child of itself, loop end, all without delay: every tick tries to start children until the
        // instance table is full; each instance spends at most its one budget per tick, so render returns.
        let samples = block(4, 0, FLAG_END, &[1; 28]).to_vec();
        let looped = (100, 0, 1, vec![(21, 0, vec![]), child(0), (22, 0, vec![])]);
        let bank = Arc::new(EffectBank::parse(build_bank(&[looped], &samples)).unwrap());
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(bank));
        e.play(0, Emitter::global());
        for _ in 0..20 {
            render(&mut r, 1024);
        }
        assert!(r.instances.iter().all(|i| i.state != State::Free), "the table filled up");
        assert!(r.ticks() > 50);
        e.stop_all();
        render(&mut r, 4096);
        e.update(0.0);
        assert_eq!(e.playing_count(), 0);
    }

    #[test]
    fn grandchild_started_from_an_earlier_slot_plays() {
        let mut samples = vec![];
        samples.extend_from_slice(&block(4, 0, FLAG_END, &[3; 28]));
        samples.extend_from_slice(&block(4, 0, FLAG_LOOP_START | FLAG_END | FLAG_REPEAT, &[5; 28]));
        let short = (100, 0, 0, vec![(1, 0, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0))]);
        // Parent waits 4 ticks, by then slot 0 (the short sound) is free, so its child lands before it.
        let parent = (100, 0, 0, vec![(20, 4, vec![]), child(2)]);
        let middle = (100, 0, 0, vec![child(3)]);
        let leaf = (100, 0, 1, vec![(1, 0, voice_data(127, -60, 0, 0x80FF, 0x9FC0, 16))]);
        let bank = Arc::new(EffectBank::parse(build_bank(&[short, parent, middle, leaf], &samples)).unwrap());
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(bank));
        e.play(0, Emitter::global());
        e.play(1, Emitter::global());
        render(&mut r, 200 * 12);
        assert_eq!(r.active_voices(), 1, "the grandchild's looping voice plays");
        let leaf_playing = r.instances.iter().any(|i| i.state != State::Free && i.sound == 3);
        assert!(leaf_playing);
    }

    #[test]
    fn music_registers_saturate() {
        let (mut e, mut r) = create(Config::default());
        e.set_music_register(4, 255);
        render(&mut r, 256);
        assert_eq!(r.sequencer().registers[4], 127);
    }

    #[test]
    fn sly2_sound_map_and_v3_bank() {
        use crate::bank::tests::build_bank_v3;
        let samples = block(4, 0, FLAG_LOOP_START | FLAG_END | FLAG_REPEAT, &[5; 28]).to_vec();
        let area = voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0);
        let bank = build_bank_v3(&[(100, 0, vec![(1 << 24, 0)])], &area, &samples);
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(Arc::new(EffectBank::parse(bank).unwrap())));
        // Game sound ID 7 plays bank sound 0; every other ID plays nothing.
        let mut map = vec![None; 10];
        map[7] = Some(0);
        e.set_sound_map(Some(map));
        assert_eq!(e.resolve(7), Some((0, 0)));
        assert_eq!(e.play(0, Emitter::global()), SoundHandle::NONE);
        let h = e.play(7, Emitter::global());
        assert!(h.is_some());
        let out = render(&mut r, 1024);
        assert!(out.iter().any(|&x| x != 0.0));
        e.set_sound_map(None);
        assert_eq!(e.resolve(7), None);
    }

    fn vpk(channels: usize, per: usize) -> Arc<crate::sly2::vpk::VpkStream> {
        // Each channel: blocks of a constant code (channel k: code k + 1), no flags; groups 0, 1, 2...
        let mut f = crate::sly2::vpk::tests::file(channels, per, 64, 32_000);
        let header = crate::sly2::vpk::HEADER_SIZE;
        for b in f[header..].chunks_mut(16) {
            let code = b[2] & 0x0F;
            b[0] = 12;
            b[1] = 0;
            for x in &mut b[2..] {
                *x = code | code << 4;
            }
        }
        Arc::new(crate::sly2::vpk::VpkStream::parse(&f).unwrap())
    }

    fn stream_gains(r: &AudioRenderer) -> Vec<f32> {
        r.mixer.pool(Pool::Streams).filter(|&i| r.mixer.voices[i].active).map(|i| r.mixer.voices[i].gain[0]).collect()
    }

    #[test]
    fn music_layers_follow_the_mood_and_the_stack() {
        let (mut e, mut r) = create(Config::default());
        // Five channels with groups 0..4 (the synthetic file numbers groups by channel).
        let first = e.push_music(vpk(5, 640));
        e.update(0.0);
        render(&mut r, 4800);
        // Normal: group 1 only of 0..2; groups 3 and 4 silent without characters.
        let a = stream_gains(&r);
        assert_eq!(a.len(), 5);
        assert!(a[0] == 0.0 && a[1] > 0.0 && a[2] == 0.0 && a[3] == 0.0 && a[4] == 0.0, "{a:?}");
        // A chase: alert fades in, normal fades out, equal power.
        e.music_mood().alert = 1;
        for _ in 0..90 {
            e.update(1.0 / 60.0);
            render(&mut r, 800);
        }
        let a = stream_gains(&r);
        assert!(a[2] > 0.0 && a[1] < a[2] * 0.25 && a[0] == 0.0, "{a:?}");
        // A character close by brings group 3 in.
        e.set_characters([Some([500.0, 0.0, 0.0]), None, None]);
        e.update(0.0);
        render(&mut r, 1600);
        assert!(stream_gains(&r)[3] > 0.0);
        // A second piece goes on top: the first fades out and ends.
        let second = e.push_music(vpk(2, 640));
        assert_eq!(e.music_top(), second);
        for _ in 0..90 {
            e.update(1.0 / 60.0);
            render(&mut r, 800);
        }
        assert!(!e.is_playing(first));
        assert!(e.is_playing(second));
        // Popping it restarts the first piece.
        e.pop_music();
        let again = e.music_top();
        assert!(again.is_some() && again != first);
        for _ in 0..90 {
            e.update(1.0 / 60.0);
            render(&mut r, 800);
        }
        assert!(!e.is_playing(second) && e.is_playing(again));
    }

    #[test]
    fn resident_stream_distance_layers() {
        let (mut e, mut r) = create(Config::default());
        let h = e.play_resident(vpk(3, 640), vec![28, 39, 46]);
        e.update(0.0);
        render(&mut r, 2400);
        // No fall-offs: every channel at its stored volume.
        assert!(stream_gains(&r).iter().all(|&g| g > 0.0));
        // A fall-off out of range silences its channel; walking in brings it back.
        let id = e.add_falloff(Falloff {
            channel: 39,
            position: [5000.0, 0.0, 0.0],
            start: 500.0,
            end: 4000.0,
            volume: 1.0,
        });
        e.update(0.0);
        render(&mut r, 2400);
        let a = stream_gains(&r);
        assert!(a[0] > 0.0 && a[1] == 0.0 && a[2] > 0.0, "{a:?}");
        e.set_falloff_position(id, [100.0, 0.0, 0.0]);
        e.update(0.0);
        render(&mut r, 2400);
        assert!(stream_gains(&r)[1] > 0.0);
        e.remove_falloff(id);
        e.fade_out(h, 0.1);
        render(&mut r, 9600);
        e.update(0.0);
        assert!(!e.is_playing(h));
    }

    #[test]
    fn stream_without_enough_voices_is_refused() {
        let (mut e, mut r) = create(Config { stream_voices: 3, ..Config::default() });
        let h = e.push_music(vpk(4, 64));
        render(&mut r, 256);
        e.update(0.0);
        assert!(!e.is_playing(h));
    }

    #[test]
    fn stop_marker_runs_the_stop_steps_and_flags_make_sounds_continuous() {
        use crate::bank::tests::build_bank_v3;
        let mut samples = vec![];
        samples.extend_from_slice(&block(4, 0, FLAG_LOOP_START | FLAG_END | FLAG_REPEAT, &[5; 28]));
        samples.extend_from_slice(&block(4, 0, FLAG_END, &[3; 28]));
        let mut area = voice_data(127, -60, 0, 0x80FF, 0x9FC0, 0);
        area.extend_from_slice(&voice_data(127, -60, 0, 0x80FF, 0x9FC0, 16));
        // Sound 0: a looping voice, the stop marker, then a one-shot voice that plays only when stopped.
        let steps = vec![(1 << 24, 0), (43 << 24, 0), ((1 << 24) | 24, 0)];
        let bank = build_bank_v3(&[(100, 0, steps)], &area, &samples);
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(Arc::new(EffectBank::parse(bank).unwrap())));
        e.set_sound_map(Some(vec![Some(0)]));
        // Out of range: a one-shot sound would not start; flag 2 makes it continuous, so it starts silently.
        let far = Emitter::at([10_000.0, 0.0, 0.0], 100.0, 10.0);
        assert_eq!(e.play(0, far), SoundHandle::NONE);
        e.set_sound_flags(Some(vec![2]));
        let h = e.play(0, Emitter::global());
        assert!(h.is_some());
        render(&mut r, 4800);
        e.update(0.0);
        assert!(e.is_playing(h), "the loop keeps it alive; normal playback ended at the marker");
        assert_eq!(r.active_voices(), 1);
        let serial = r.mixer.voices.iter().filter(|v| v.active).map(|v| v.serial).max().unwrap();
        e.stop(h);
        render(&mut r, 400);
        // The stop steps started the one-shot voice.
        assert!(r.mixer.voices.iter().any(|v| v.serial > serial));
        render(&mut r, 4800);
        e.update(0.0);
        assert!(!e.is_playing(h));
    }

    #[test]
    fn soft_clip_is_monotonic_and_bounded() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.8), -0.8);
        let mut last = 0.0;
        for i in 0..1000 {
            let y = soft_clip(i as f32 * 0.01);
            assert!(y >= last && y < 1.0);
            last = y;
        }
        assert_eq!(soft_clip(-3.0), -soft_clip(3.0));
    }

    #[test]
    fn mixed_output_hash() {
        // A known mix: the looping sound panned by a positioned emitter, the one-shot centred, a pitch offset and
        // reverb, rendered in platform-sized blocks. Pins the whole mixing path (decode, resample, envelope, ramps,
        // pan law, square law, reverb).
        let (mut e, mut r) = create(Config::default());
        e.load_bank(0, Some(test_bank()));
        e.set_reverb(3, 10_000);
        let mut far = Emitter::at([300.0, 100.0, 0.0], 1000.0, 100.0);
        far.pitch = 3.0;
        e.play(1, far);
        e.play(0, Emitter::global());
        let mut pcm = vec![0.0f32; 384 * 2 * 25];
        for (i, block) in pcm.chunks_mut(384 * 2).enumerate() {
            if i == 12 {
                e.set_listener(Listener { position: [0.0, 300.0, 0.0], ..Listener::default() });
                e.update(0.0);
            }
            r.render(block);
        }
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let sha = formats::hash::hex(&formats::hash::sha256(&bytes));
        assert!(pcm.iter().any(|&x| x != 0.0));
        assert_eq!(sha, "26c50a8b50e2291ab29fa0b07b6001e5fadac56a177f2b5d5b32bc93280900cf");
    }

    #[test]
    fn queue_overflow_is_kept_in_order() {
        let (mut e, mut r) = create(Config { queue: 2, ..Config::default() });
        e.load_bank(0, Some(test_bank()));
        let hs: Vec<_> = (0..10).map(|_| e.play(0, Emitter::global())).collect();
        for _ in 0..20 {
            render(&mut r, 256);
            e.update(0.0);
        }
        render(&mut r, 8192);
        e.update(0.0);
        assert!(hs.iter().all(|&h| !e.is_playing(h)));
        assert_eq!(e.playing_count(), 0);
    }
}
