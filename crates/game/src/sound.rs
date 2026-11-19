//! The engine's [`AudioFrame`] on the `audio` crate's [`SoundEngine`]: banks and music per world, the listener,
//! objects' ambient sounds (started when they appear, moved every frame, stopped when they go), one-shots, the music
//! intensity requests, and dialogue voice lines.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use audio::{Emitter, Listener, SoundEngine, SoundHandle};
use engine::game::audio_frame::AudioFrame;
use formats::disc::{Descriptor, Disc};
use formats::iso::ReadAt;

/// The game's sound.
pub struct Sound {
    /// The engine (game-thread half).
    pub engine: SoundEngine,
    ambient: BTreeMap<u32, SoundHandle>,
    intensity: Option<(i32, audio::IntensityRequest)>,
    voice: Option<SoundHandle>,
    banks_counted: bool,
}

fn emitter(
    position: [f32; 3],
    velocity: [f32; 3],
    start: f32,
    full: f32,
    doppler: f32,
    volume: f32,
    pitch: f32,
) -> Emitter {
    Emitter {
        position: Some(position),
        velocity,
        start_distance: start,
        full_distance: full,
        doppler,
        volume,
        pitch,
        pan: 0.0,
    }
}

impl Sound {
    /// Wraps a sound engine (made by `audio::create`; its renderer half goes to the platform).
    pub fn new(engine: SoundEngine) -> Sound {
        Sound { engine, ambient: BTreeMap::new(), intensity: None, voice: None, banks_counted: false }
    }

    /// Loads a world's sound: the sound-ID numbering and bank 0 once, the world's bank and its music
    /// (docs/audio.md, quick start). Errors leave the world silent.
    pub fn load_world<R: ReadAt>(&mut self, disc: &mut Disc<R>, world: &[u8]) {
        self.engine.stop_all();
        self.ambient.clear();
        if let Some((_, r)) = self.intensity.take() {
            self.engine.remove_intensity(r);
        }
        if !self.banks_counted {
            if let Ok(c) = audio::disc::bank_sound_counts(disc) {
                self.engine.set_bank_counts(c);
            }
            if let Ok(b) = audio::disc::read_bank(disc, 0) {
                self.engine.load_bank(0, Some(Arc::new(b)));
            }
            self.banks_counted = true;
        }
        let Ok((music, bank)) = audio::disc::world_audio(world) else { return };
        if bank > 0 {
            let b = audio::disc::read_bank(disc, bank as usize).ok().map(Arc::new);
            self.engine.load_bank(bank as usize, b);
        }
        self.engine.stop_music(true);
        if let Some(m) = music
            && let Ok(m) = audio::disc::read_music(disc, &m)
        {
            self.engine.play_music(Arc::new(m));
        }
    }

    /// Plays a frame: listener, ambient sounds, one-shots, intensity, then `update`.
    pub fn frame(&mut self, f: &AudioFrame, intensity: i32, dt: f32) {
        let l = f.listener;
        self.engine.set_listener(Listener { position: l.position, velocity: l.velocity, left: l.left });
        let mut seen: Vec<u32> = Vec::with_capacity(f.emitters.len());
        for e in &f.emitters {
            let key = e.object.index();
            seen.push(key);
            let em = emitter(e.position, e.velocity, e.start_distance, e.full_distance, e.doppler, e.volume, e.pitch);
            match (self.ambient.get(&key), e.active) {
                (Some(&h), true) => self.engine.set_emitter(h, em),
                (Some(&h), false) => {
                    self.engine.stop(h);
                    self.ambient.remove(&key);
                }
                (None, true) if e.sound >= 0 => {
                    let h = self.engine.play(e.sound as u16, em);
                    if h.is_some() {
                        self.ambient.insert(key, h);
                    }
                }
                _ => {}
            }
        }
        let gone: Vec<u32> = self.ambient.keys().copied().filter(|k| !seen.contains(k)).collect();
        for k in gone {
            if let Some(h) = self.ambient.remove(&k) {
                self.engine.stop(h);
            }
        }
        for s in &f.one_shots {
            if s.sound < 0 {
                continue;
            }
            let em = match s.position {
                Some(p) => Emitter::at(p, 3000.0, 300.0),
                None => Emitter::global(),
            };
            self.engine.play(s.sound as u16, em);
        }
        if self.intensity.map(|(v, _)| v) != Some(intensity) {
            if let Some((_, r)) = self.intensity.take() {
                self.engine.remove_intensity(r);
            }
            if intensity != 0 {
                self.intensity = Some((intensity, self.engine.request_intensity(intensity)));
            }
        }
        self.engine.set_music_volume(f.music_scale);
        self.engine.update(dt);
    }

    /// Starts a dialogue voice line.
    pub fn play_voice<R: ReadAt>(&mut self, disc: &mut Disc<R>, d: &Descriptor) {
        if let Some(h) = self.voice.take() {
            self.engine.stop(h);
        }
        if let Ok(v) = audio::disc::read_vag(disc, d) {
            self.voice = Some(self.engine.play_dialogue(Arc::new(v), Emitter::global()));
        }
    }

    /// Whether a voice line is playing.
    pub fn voice_playing(&self) -> bool {
        self.voice.is_some_and(|h| self.engine.is_playing(h))
    }
}
