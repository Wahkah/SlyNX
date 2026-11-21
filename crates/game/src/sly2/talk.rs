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
    /// Where this run stops: the end of the list for the main line, a thread's closing 626.
    pub end: usize,
    /// The thread this run is (625's ID), or −1 for the main line.
    pub thread: i32,
}

/// A sequence command the session runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeqCommand {
    /// A dialogue command (dialogue-and-voice.md 3).
    Talk(TalkCommand),
    /// Option 642 (command 0x11): set a task's state (task object ID, state; level-flow.md 4.1), as a request to the
    /// job system.
    Task {
        /// The task's object ID.
        task: i32,
        /// The state asked for.
        state: i32,
    },
    /// Option 670: add an object to the world (object, time, flag; missions.md 4.1).
    AddObject(i32),
    /// Option 671: remove an object from the world.
    RemoveObject(i32),
    /// Option 634: raise script event 4 on the sequence's own module with this value (missions.md 4.1).
    Value(i32),
    /// Option 635: play a clip (clip, object or −1 for the clip's own rig, no-wait flag, apply flags; camera.md
    /// 7.2.1). With the no-wait flag 0 the sequence goes on only after the clip (missions.md 4.1).
    PlayClip {
        /// The clip's object ID.
        clip: i32,
        /// The target's object ID, or −1.
        object: i32,
        /// Wait for the clip (the no-wait flag is 0).
        wait: bool,
        /// Apply flags (option 106's: 8 loop, 16 back and forth, 32 hold).
        flags: i32,
    },
    /// Option 627: stop the named thread (a sequence run), retracting the playback it waits for.
    StopThread(i32),
    /// Option 631 (record kind 6): stop the named sequence: all its runs stop and retract their waited playbacks
    /// (animation.md 2).
    StopSequence(i32),
    /// Option 625: a thread (its ID) runs the commands up to the matching 626 beside the line that reaches it
    /// (animation.md 2); `end` is the index of that 626.
    Thread {
        /// The thread's ID.
        id: i32,
        /// The index of its closing 626.
        end: usize,
    },
    /// Option 626: the end of a thread's commands.
    ThreadEnd,
    /// Option 682 (command 0x38): open a camera block, a timed move from `from` to `to` (the matching 683's values)
    /// over `duration` seconds (0: the block's contents) on curve `curve` (camera.md 2.1.1).
    CameraOpen {
        /// State A: position, Euler angles (radians), field of view; near-zero parts mean "the current value".
        from: CamArgs,
        /// State B (the matching end's), if there is one.
        to: Option<CamArgs>,
        /// Seconds.
        duration: f32,
        /// One of the four curves.
        curve: i32,
    },
    /// Option 683 (command 0x39): the block's end: removes the timed view when `remove` (cutting when `cut`).
    CameraEnd {
        /// State B.
        state: CamArgs,
        /// "End the timed view here".
        remove: bool,
        /// "Cut".
        cut: bool,
    },
    /// Option 684 (command 0x3A): end the sequence's view (`cut`: at once).
    CameraEndView {
        /// "Cut".
        cut: bool,
    },
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
/// Option 642 (command 0x11): set a task's state (task, state).
pub const TASK_STATE: i16 = 642;
/// Option 670: add an object (object, time, flag).
pub const ADD_OBJECT: i16 = 670;
/// Option 671: remove an object.
pub const REMOVE_OBJECT: i16 = 671;
/// Option 634: call the sequence's script with a value (event 4).
pub const SCRIPT_VALUE: i16 = 634;
/// Option 635: play a clip.
pub const PLAY_CLIP: i16 = 635;
/// Option 627: stop a thread.
pub const STOP_THREAD: i16 = 627;
/// Option 631: stop a sequence.
pub const STOP_SEQUENCE: i16 = 631;
/// Option 625: open a thread.
pub const THREAD: i16 = 625;
/// Option 626: close a thread.
pub const THREAD_END: i16 = 626;
/// The main line's thread ID.
pub const MAIN: i32 = -1;
/// Camera block options (camera.md 2.1.1): open (0x38), end (0x39), end the runner's view (0x3A).
pub const CAMERA_OPEN: i16 = 682;
/// See [`CAMERA_OPEN`].
pub const CAMERA_END: i16 = 683;
/// See [`CAMERA_OPEN`].
pub const CAMERA_END_VIEW: i16 = 684;

/// A camera block's state: position, Euler angles (radians), field of view.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamArgs {
    /// Position.
    pub position: [f32; 3],
    /// Euler angles, radians.
    pub angles: [f32; 3],
    /// Field of view, radians (0: the current one).
    pub fov: f32,
}

fn floats<const N: usize>(v: &[u8]) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = f32::from_le_bytes(v.get(4 * i..4 * i + 4)?.try_into().ok()?);
    }
    Some(out)
}

/// Position, angles (degrees in the data) and field of view of a camera block's first three arguments.
fn cam_args(a: &[(u16, &[u8])]) -> Option<CamArgs> {
    let position = floats::<3>(a.first()?.1)?;
    let deg = floats::<3>(a.get(1)?.1)?;
    let fov = floats::<1>(a.get(2)?.1)?[0];
    Some(CamArgs { position, angles: deg.map(|d| d.to_radians()), fov })
}

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
    let mut out = list
        .options
        .iter()
        .filter_map(|o| {
            let (at, len) = (o.value.0 as usize, o.value.1 as usize);
            let v = bytes.get(at..at + len)?;
            if o.id == PLAY_CLIP {
                let a = typed_args(v)?;
                return Some(SeqCommand::PlayClip {
                    clip: int_of(a.first()?.1),
                    object: a.get(1).map_or(-1, |x| int_of(x.1)),
                    // Until the playback is retracted (animation.md 2): a looping, back-and-forth or holding clip
                    // is retracted only when the run is stopped (627) or some other retract.
                    wait: a.get(2).is_none_or(|x| int_of(x.1) == 0),
                    flags: a.get(3).map_or(0, |x| int_of(x.1)),
                });
            }
            if o.id == THREAD {
                return Some(SeqCommand::Thread { id: int_of(typed_args(v)?.first()?.1), end: usize::MAX });
            }
            if o.id == THREAD_END {
                return Some(SeqCommand::ThreadEnd);
            }
            if o.id == STOP_SEQUENCE {
                return Some(SeqCommand::StopSequence(int_of(typed_args(v)?.first()?.1)));
            }
            if o.id == STOP_THREAD {
                return Some(SeqCommand::StopThread(int_of(typed_args(v)?.first()?.1)));
            }
            if o.id == ADD_OBJECT || o.id == REMOVE_OBJECT || o.id == SCRIPT_VALUE {
                let n = int_of(typed_args(v)?.first()?.1);
                return Some(match o.id {
                    ADD_OBJECT => SeqCommand::AddObject(n),
                    REMOVE_OBJECT => SeqCommand::RemoveObject(n),
                    _ => SeqCommand::Value(n),
                });
            }
            if o.id == CAMERA_OPEN {
                let a = typed_args(v)?;
                let from = cam_args(&a)?;
                let duration = floats::<1>(a.get(3)?.1)?[0];
                let curve = int_of(a.get(4)?.1);
                return Some(SeqCommand::CameraOpen { from, to: None, duration, curve });
            }
            if o.id == CAMERA_END {
                let a = typed_args(v)?;
                let remove = a.get(3).is_some_and(|x| x.1.first() == Some(&1));
                let cut = a.get(4).is_some_and(|x| x.1.first() == Some(&1));
                return Some(SeqCommand::CameraEnd { state: cam_args(&a)?, remove, cut });
            }
            if o.id == CAMERA_END_VIEW {
                let a = typed_args(v)?;
                return Some(SeqCommand::CameraEndView { cut: a.first().is_some_and(|x| x.1.first() == Some(&1)) });
            }
            if o.id == TASK_STATE {
                let a = typed_args(v)?;
                return Some(SeqCommand::Task { task: int_of(a.first()?.1), state: int_of(a.get(1)?.1) });
            }
            if o.id == START_SUB_SEQUENCE {
                let a = typed_args(v)?;
                let sequence = world.find_sly2_id(int_of(a.first()?.1), Some(h))?;
                let no_wait = a.get(2).is_some_and(|x| int_of(x.1) != 0);
                return Some(SeqCommand::Start { sequence, no_wait });
            }
            TalkCommand::decode(o.id, v).map(SeqCommand::Talk)
        })
        .collect::<Vec<_>>();
    pair_camera_blocks(&mut out);
    pair_threads(&mut out);
    out
}

/// Gives each thread's 625 the index of its matching 626 (threads nest; an unclosed thread runs to the end).
fn pair_threads(cmds: &mut [SeqCommand]) {
    let mut open: Vec<usize> = Vec::new();
    let len = cmds.len();
    for i in 0..len {
        match cmds[i] {
            SeqCommand::Thread { .. } => open.push(i),
            SeqCommand::ThreadEnd => {
                if let Some(o) = open.pop()
                    && let SeqCommand::Thread { end, .. } = &mut cmds[o]
                {
                    *end = i;
                }
            }
            _ => {}
        }
    }
    for o in open {
        if let SeqCommand::Thread { end, .. } = &mut cmds[o] {
            *end = len;
        }
    }
}

/// Gives each camera block's open the state of its matching end (the next end at the same nesting depth).
fn pair_camera_blocks(cmds: &mut [SeqCommand]) {
    let mut open: Vec<usize> = Vec::new();
    for i in 0..cmds.len() {
        match cmds[i] {
            SeqCommand::CameraOpen { .. } => open.push(i),
            SeqCommand::CameraEnd { state, .. } => {
                if let Some(o) = open.pop()
                    && let SeqCommand::CameraOpen { to, .. } = &mut cmds[o]
                {
                    *to = Some(state);
                }
            }
            _ => {}
        }
    }
}

/// What a sequence asked of the camera, for the session (it has the camera's current state).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraCue {
    /// A timed move from A to B.
    Move {
        /// State A.
        from: CamArgs,
        /// State B (A when the block has no end).
        to: CamArgs,
        /// Seconds (0: the block's contents).
        duration: f32,
        /// Curve index.
        curve: i32,
    },
    /// The timed view ends (`cut`: at once).
    End {
        /// "Cut".
        cut: bool,
    },
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
    /// Task states asked for (option 642).
    task_requests: Vec<(i32, i32)>,
    /// Object and script commands (670, 671, 634, 635) with their run, for the session.
    world_cues: Vec<(Handle, i32, SeqCommand)>,
    /// Runs (sequence, thread) waiting for a clip the session plays (635 with no-wait 0); [`Sly2Sound::release`]
    /// lets them go on.
    held: Vec<(Handle, i32)>,
    /// Runs stopped this frame (627, or their line ended), whose waited playbacks the session retracts.
    stopped: Vec<(Handle, i32)>,
    /// Threads the lines asked to stop this frame (627).
    stop_threads: Vec<(Handle, i32)>,
    /// Threads the lines opened this frame (625), started after this frame's runs.
    spawned: Vec<Sequence>,
    /// Camera blocks run (options 682 to 684).
    camera_cues: Vec<CameraCue>,
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
            task_requests: Vec::new(),
            world_cues: Vec::new(),
            held: Vec::new(),
            stopped: Vec::new(),
            stop_threads: Vec::new(),
            spawned: Vec::new(),
            camera_cues: Vec::new(),
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

    /// Starts sequence object `h`; one already running is left as it is (animation.md 2). Returns whether it has
    /// commands.
    pub fn start_sequence(&mut self, world: &World, h: Handle) -> bool {
        if self.sequences.iter().any(|s| s.object == h) {
            return true;
        }
        let commands = sequence_commands(world, h);
        let any = !commands.is_empty();
        if any {
            let end = commands.len();
            self.sequences.push(Sequence { object: h, commands, at: 0, started_sub: false, end, thread: MAIN });
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

    /// Task states the running sequences asked for this frame (option 642), for the job system.
    pub fn take_task_requests(&mut self) -> Vec<(i32, i32)> {
        core::mem::take(&mut self.task_requests)
    }

    /// The length of the camera block opened at `cmds[open]`: the commands up to its matching end at the same depth,
    /// summed. Waits for a line (708) last the line just played (its longest track; a non-negative time instead),
    /// a wait without a line nothing; nested blocks count with their own durations. Lines are timed only with the
    /// world's sound loaded.
    pub fn block_length(&self, cmds: &[SeqCommand], open: usize) -> f32 {
        let line_length = |line: i16| {
            self.world
                .as_ref()
                .and_then(|w| w.line(line))
                .map_or(0.0, |l| l.tracks.iter().map(|t| t.duration).fold(0.0, f32::max))
        };
        let mut depth = 0usize;
        let mut total = 0.0;
        let mut last = None;
        for c in &cmds[open + 1..] {
            match *c {
                SeqCommand::CameraOpen { duration, .. } => {
                    if depth == 0 && duration > 0.0 {
                        total += duration;
                    }
                    depth += 1;
                }
                SeqCommand::CameraEnd { .. } if depth == 0 => break,
                SeqCommand::CameraEnd { .. } => depth -= 1,
                SeqCommand::Talk(TalkCommand::PlayLine { line, wait, .. }) if depth == 0 => {
                    last = Some(line);
                    if wait {
                        total += line_length(line);
                        last = None;
                    }
                }
                SeqCommand::Talk(TalkCommand::WaitLine { time }) if depth == 0 => {
                    total += if time >= 0.0 { time } else { last.take().map_or(0.0, line_length) };
                }
                _ => {}
            }
        }
        total
    }

    /// Lets run (`h`, `thread`) go on after the clip it waits for (635) was retracted.
    pub fn release(&mut self, h: Handle, thread: i32) {
        self.held.retain(|&x| x != (h, thread));
    }

    /// The runs stopped since the last call (627, a sequence stopped or its line ended): their waited playbacks are
    /// retracted by the session (animation.md 2).
    pub fn take_stopped(&mut self) -> Vec<(Handle, i32)> {
        core::mem::take(&mut self.stopped)
    }

    /// Object and script commands the running sequences ran this frame (options 670, 671, 634), with their sequence.
    pub fn take_world_cues(&mut self) -> Vec<(Handle, i32, SeqCommand)> {
        core::mem::take(&mut self.world_cues)
    }

    /// What the running sequences asked of the camera this frame (options 682 to 684).
    pub fn take_camera_cues(&mut self) -> Vec<CameraCue> {
        core::mem::take(&mut self.camera_cues)
    }

    /// Stops sequence `h`.
    pub fn stop_sequence(&mut self, h: Handle) {
        for s in self.sequences.iter().filter(|s| s.object == h) {
            self.stopped.push((h, s.thread));
        }
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
        // Runs that passed their end or that a 627 stopped; a sequence ends when none of its runs is left.
        let stops = core::mem::take(&mut self.stop_threads);
        // A sequence ends when none of its runs is left: its threads may outlast its main line.
        for s in &running {
            if stops.contains(&(s.object, s.thread)) || s.at >= s.end {
                self.stopped.push((s.object, s.thread));
            }
        }
        running.retain(|s| s.at < s.end && !stops.contains(&(s.object, s.thread)));
        self.spawned.retain(|s| !stops.contains(&(s.object, s.thread)));
        // Threads opened this frame, then sequences started while these ran, come after them.
        running.append(&mut self.spawned);
        running.append(&mut self.sequences);
        self.sequences = running;
        self.engine.update(dt);
    }

    fn run(&mut self, seq: &mut Sequence, confirm: bool) {
        while seq.at < seq.end
            && let Some(&cmd) = seq.commands.get(seq.at)
        {
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
                SeqCommand::Task { task, state } => {
                    self.task_requests.push((task, state));
                    seq.at += 1;
                    continue;
                }
                SeqCommand::Thread { id, end } => {
                    // The thread runs beside this line; the line goes on after its 626.
                    self.spawned.push(Sequence {
                        object: seq.object,
                        commands: seq.commands.clone(),
                        at: seq.at + 1,
                        started_sub: false,
                        end: end.min(seq.commands.len()),
                        thread: id,
                    });
                    seq.at = end.saturating_add(1);
                    continue;
                }
                SeqCommand::ThreadEnd => {
                    seq.at += 1;
                    continue;
                }
                SeqCommand::StopThread(id) => {
                    self.stop_threads.push((seq.object, id));
                    seq.at += 1;
                    continue;
                }
                SeqCommand::PlayClip { wait: true, .. } => {
                    // Played by the session, which holds the run until the clip is retracted.
                    let run = (seq.object, seq.thread);
                    if !seq.started_sub {
                        self.world_cues.push((seq.object, seq.thread, cmd));
                        self.held.push(run);
                        seq.started_sub = true;
                        break;
                    }
                    if self.held.contains(&run) {
                        break;
                    }
                    seq.started_sub = false;
                    seq.at += 1;
                    continue;
                }
                SeqCommand::AddObject(_)
                | SeqCommand::RemoveObject(_)
                | SeqCommand::Value(_)
                | SeqCommand::StopSequence(_)
                | SeqCommand::PlayClip { .. } => {
                    // Done by the session (it has the world), in order with the task requests.
                    self.world_cues.push((seq.object, seq.thread, cmd));
                    seq.at += 1;
                    continue;
                }
                SeqCommand::CameraOpen { from, to, duration, curve } => {
                    // A duration of 0 lasts as long as the block's contents (camera.md 2.1.1).
                    let duration = if duration > 0.0 { duration } else { self.block_length(&seq.commands, seq.at) };
                    self.camera_cues.push(CameraCue::Move { from, to: to.unwrap_or(from), duration, curve });
                    seq.at += 1;
                    continue;
                }
                SeqCommand::CameraEnd { remove, cut, .. } => {
                    if remove {
                        self.camera_cues.push(CameraCue::End { cut });
                    }
                    seq.at += 1;
                    continue;
                }
                SeqCommand::CameraEndView { cut } => {
                    self.camera_cues.push(CameraCue::End { cut });
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
