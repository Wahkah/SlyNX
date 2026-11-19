//! Sly 2 sound and dialogue in the session (specs/sly2/dialogue-and-voice.md, `audio::sly2`).
//!
//! - **World sound**: each loaded world's sound section, effect bank and stream go to the sound engine
//!   (`WorldAudio::install`).
//! - **Sequences** (class 114): a sequence object's option list, in stream order, is its command list (2). Started
//!   with [`Sly2Sound::start_sequence`], the dialogue commands (3) run one after another, each waiting until done:
//!   play a line (stops the playing one; quiet lines have no subtitle), wait for the line (−1: its end), clear the
//!   subtitle, wait for a button (any of the confirm buttons here). The other commands (camera blocks, moves,
//!   animations, binocucom talk, closing eyes, talk effects) belong to systems that do not exist yet and pass at
//!   once. Several sequences run side by side (each its own command list); starting a running one restarts it.
//!   Lines share one player: starting a line stops the line playing.
//! - **Subtitles**: [`Sly2Sound::subtitle`] gives the line's text revealed so far (with its `|` cursor) and the
//!   speaker's colour, for the HUD.
//!
//! Sequences are started by the hooks of dialogue-and-voice.md 2.1: script calls 613 / 614 / 615 on the sequence,
//! trigger volumes (option 441), interaction spots (option 1070) and guard command lists, through
//! `World::shared.sequence_requests`, which the session hands to [`Sly2Sound`].

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use audio::SoundEngine;
use audio::sly2::{MusicQueue, Sly2Disc, Subtitle, Talk, TalkCommand, TimedRecord, WorldAudio};
use engine::object::Handle;
use engine::world::World;

use crate::movie::MovieReader;

/// A running sequence: its commands and the next one.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    /// The sequence object.
    pub object: Handle,
    /// Its dialogue commands and sub-sequence starts, in order (other options left out).
    pub commands: Vec<SeqCommand>,
    /// The next command.
    pub at: usize,
    /// The current command is a sub-sequence start that already started it.
    pub started_sub: bool,
}

/// A sequence command the session runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeqCommand {
    /// A dialogue command (dialogue-and-voice.md 3).
    Talk(TalkCommand),
    /// Option 636: start a sub-sequence (with the starter passed on), waiting for it to end unless `no_wait`.
    Start {
        /// The sequence.
        sequence: Handle,
        /// Do not wait for it.
        no_wait: bool,
    },
}

/// Option 636: start a sub-sequence (sequence, starter or −1, no-wait flag).
pub const START_SUB_SEQUENCE: i16 = 636;

/// The typed arguments of an option value: (tag, value bytes) (object-stream.md 1).
fn typed_args(mut b: &[u8]) -> Option<Vec<(u16, &[u8])>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let tag = u16::from_le_bytes([*b.first()?, *b.get(1)?]);
        b = &b[2..];
        let size = match tag & 0x7FFF {
            0 => 1,
            7 | 14..=21 => 2,
            1 | 5 | 6 | 0x1000..=0x1FFF => 4,
            2 | 9 => 12,
            10 => 8,
            _ => return None,
        };
        out.push((tag & 0x7FFF, b.get(..size)?));
        b = &b[size..];
    }
    Some(out)
}

fn int_of(v: &[u8]) -> i32 {
    match v.len() {
        1 => v[0] as i8 as i32,
        2 => i16::from_le_bytes([v[0], v[1]]) as i32,
        _ => i32::from_le_bytes([v[0], v[1], v[2], v[3]]),
    }
}

/// The commands of a sequence object (its option list in stream order): the dialogue commands and sub-sequence starts.
pub fn sequence_commands(world: &World, h: Handle) -> Vec<SeqCommand> {
    let Some(r) = world.sly2_record(h) else { return Vec::new() };
    let Some(list) = r.options() else { return Vec::new() };
    let bytes = &world.level.bytes;
    list.options
        .iter()
        .filter_map(|o| {
            let (at, len) = (o.value.0 as usize, o.value.1 as usize);
            let v = bytes.get(at..at + len)?;
            if o.id == START_SUB_SEQUENCE {
                let a = typed_args(v)?;
                let sequence = world.find_sly2_id(int_of(a.first()?.1), Some(h))?;
                let no_wait = a.get(2).is_some_and(|x| int_of(x.1) != 0);
                return Some(SeqCommand::Start { sequence, no_wait });
            }
            TalkCommand::decode(o.id, v).map(SeqCommand::Talk)
        })
        .collect()
}

/// The Sly 2 session's sound.
pub struct Sly2Sound {
    /// The engine (game-thread half).
    pub engine: SoundEngine,
    disc: Sly2Disc<MovieReader>,
    world: Option<WorldAudio>,
    talk: Talk,
    sequences: Vec<Sequence>,
    sub_starts: Vec<Handle>,
    /// The world's music streams queued by scripts (action 146).
    music: MusicQueue,
    /// The sequences running at the start of this update.
    live: Vec<Handle>,
    cues: Vec<TimedRecord>,
    /// Sly 2 hook (lip sync and eyes): the timed records fired, with their line's speaker class, for the session to
    /// apply to the speaker's state machines (`engine::game::sly2_anim`); the session drains it.
    pub fired: Vec<(i16, TimedRecord)>,
    speaker: i16,
    /// Lines started so far.
    pub lines_played: u32,
}

impl Sly2Sound {
    /// Sound through `engine`, reading the disc image `image` (opened again for sound).
    pub fn new(engine: SoundEngine, image: Box<dyn formats::iso::ReadAt>) -> Option<Sly2Sound> {
        let disc = Sly2Disc::open(MovieReader(image)).ok()?;
        Some(Sly2Sound {
            engine,
            disc,
            world: None,
            talk: Talk::default(),
            sequences: Vec::new(),
            sub_starts: Vec::new(),
            music: MusicQueue::new(),
            live: Vec::new(),
            cues: Vec::new(),
            fired: Vec::new(),
            speaker: -1,
            lines_played: 0,
        })
    }

    /// Loads and installs a world's sound (its level row).
    pub fn load_world(&mut self, row: usize) {
        self.sequences.clear();
        self.talk = Talk::default();
        self.music = MusicQueue::new();
        self.world = self.disc.load_world_audio(row).ok();
        if let Some(w) = &self.world {
            w.install(&mut self.engine);
        }
    }

    /// Starts sequence object `h` (restarting it when it runs). Returns whether it has dialogue commands.
    pub fn start_sequence(&mut self, world: &World, h: Handle) -> bool {
        let commands = sequence_commands(world, h);
        let any = !commands.is_empty();
        self.sequences.retain(|s| s.object != h);
        if any {
            self.sequences.push(Sequence { object: h, commands, at: 0, started_sub: false });
        }
        any
    }

    /// Queues music stream `n` of the world (script action 146) on the engine's music stack.
    pub fn queue_music(&mut self, n: i16) {
        let Some(w) = &self.world else { return };
        let disc = &mut self.disc;
        let _ = self.music.queue(&mut self.engine, n, |n| disc.read_music_stream(w.sound(), n));
    }

    /// Sub-sequences the running sequences started this frame, for the session to start (it has the world).
    pub fn take_sub_starts(&mut self) -> Vec<Handle> {
        core::mem::take(&mut self.sub_starts)
    }

    /// Stops sequence `h`.
    pub fn stop_sequence(&mut self, h: Handle) {
        self.sequences.retain(|s| s.object != h);
    }

    /// Sly 2 hook (job screens): world string `id` (first language) as text, when the world's sound is loaded.
    pub fn string(&self, id: i16) -> Option<alloc::string::String> {
        self.world.as_ref()?.subtitle(id).map(audio::sly2::talk::subtitle_text)
    }

    /// The running sequences.
    pub fn sequences(&self) -> &[Sequence] {
        &self.sequences
    }

    /// Runs each sequence's commands until one waits, and advances the line playing. `confirm`: a confirm button
    /// went down this frame.
    pub fn update(&mut self, dt: f32, confirm: bool) {
        self.talk.update(dt, &mut self.cues);
        let speaker = self.speaker;
        self.fired.extend(self.cues.drain(..).map(|c| (speaker, c)));
        let mut running = core::mem::take(&mut self.sequences);
        self.live = running.iter().map(|s| s.object).collect();
        for seq in &mut running {
            self.run(seq, confirm);
        }
        running.retain(|s| s.at < s.commands.len());
        // Sequences started while these ran (none here) come after them.
        running.append(&mut self.sequences);
        self.sequences = running;
        self.engine.update(dt);
    }

    fn run(&mut self, seq: &mut Sequence, confirm: bool) {
        while let Some(&cmd) = seq.commands.get(seq.at) {
            let cmd = match cmd {
                SeqCommand::Start { sequence, no_wait } => {
                    // A sub-sequence: started by the session (it has the world), waited for unless `no_wait`.
                    if !seq.started_sub {
                        self.sub_starts.push(sequence);
                        seq.started_sub = true;
                        if !no_wait {
                            break;
                        }
                    } else if !no_wait && (self.sub_starts.contains(&sequence) || self.live.contains(&sequence)) {
                        break;
                    }
                    seq.started_sub = false;
                    seq.at += 1;
                    continue;
                }
                SeqCommand::Talk(t) => t,
            };
            match cmd {
                TalkCommand::PlayLine { line, quiet, .. } => {
                    if let Some(w) = &self.world
                        && let Some(vl) = w.line(line)
                    {
                        let voice = self.disc.read_line(w, line).ok().flatten().map(Arc::new);
                        self.talk.play(&mut self.engine, vl, voice, w.subtitle(line), quiet, &mut self.cues);
                        // The stopped line's remaining records are its speaker's; the new line's are vl's.
                        let old = self.speaker;
                        self.fired.extend(self.cues.drain(..).map(|c| (old, c)));
                        self.speaker = vl.speaker;
                        self.lines_played += 1;
                    }
                }
                TalkCommand::WaitLine { time } => {
                    if !self.talk.wait_done(time) {
                        break;
                    }
                }
                TalkCommand::WaitButton { .. } => {
                    if !confirm {
                        break;
                    }
                }
                TalkCommand::ClearSubtitle | TalkCommand::CloseBinocucom { .. } => self.talk.clear_subtitle(),
                _ => {}
            }
            seq.at += 1;
        }
    }

    /// The subtitle to show at game time `t`.
    pub fn subtitle(&self, t: f32) -> Option<Subtitle> {
        self.talk.subtitle(t)
    }
}
