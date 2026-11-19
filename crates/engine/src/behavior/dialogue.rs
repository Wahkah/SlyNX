//! Dialogues (class 101, dialogue-and-voice.md).
//!
//! A dialogue is a list of events read from its record: voice lines, picture panels, talk lines with subtitles,
//! script calls, letterbox bars and movies. It is triggered by a script (option 974), by another object (a button's
//! dialogue, a bottle hint: `Request::Dialogue`), or once on load (option 968). One plays at a time; a new one cuts
//! the playing one short unless that one is crucial (option 969). A talk line ends when its subtitle is revealed,
//! its voice line has finished, its minimum time has passed and its button (if any) was pressed. When the last
//! event has run, event 21 is raised and the dialogue is marked played (keyed by its anchor, option 976).

use alloc::vec;
use alloc::vec::Vec;

use formats::disc::Descriptor;
use formats::reader::Reader;
use formats::stream::Extension;
use script::Value;

use super::{ClassBehavior, Cx};
use crate::object::{Arg, Handle, OptionValue};
use crate::world::{Request, World};

/// Dialogue options.
pub mod opt {
    /// Briefing kind.
    pub const BRIEFING: u16 = 971;
    /// Confrontation kind.
    pub const CONFRONTATION: u16 = 972;
    /// Default camera anchor.
    pub const ANCHOR: u16 = 967;
    /// Trigger once on load.
    pub const AUTO: u16 = 968;
    /// Crucial: cannot be cut short.
    pub const CRUCIAL: u16 = 969;
    /// Not skippable.
    pub const NO_SKIP: u16 = 970;
    /// Trigger (script).
    pub const TRIGGER: u16 = 974;
    /// Untrigger (script).
    pub const UNTRIGGER: u16 = 975;
    /// Played-flag key.
    pub const PLAYED_KEY: u16 = 976;
    /// Duck effects and music to 0.6 (confrontations).
    pub const DUCK: u16 = 978;
    /// Speaker subtitle colour.
    pub const SPEAKER_COLOR: u16 = 988;
}

/// Subtitle reveal speed without a minimum time, characters per second.
pub const REVEAL_RATE: f32 = 15.0;
/// A triggered dialogue that cannot start within this long returns to enabled.
pub const START_TIMEOUT: f32 = 5.0;

/// One event of a dialogue (classes/placed-object.md 5.2; dialogue-and-voice.md 3).
#[derive(Clone, Debug, PartialEq)]
pub enum DialogueEvent {
    /// Load and play a voice line.
    Voice(Descriptor),
    /// Show a speaker in the left (`false`) or right (`true`) panel, with an optional replacement picture.
    Panel {
        /// Right panel.
        right: bool,
        /// Speaker OID.
        speaker: i16,
        /// Replacement picture OID.
        picture: i16,
    },
    /// A talk line.
    Talk(Talk),
    /// Run a script label.
    Call(i16),
    /// Letterbox on.
    LetterboxOn,
    /// Letterbox off.
    LetterboxOff,
    /// Play a movie.
    Movie(Descriptor),
}

/// A talk line (dialogue-and-voice.md 3.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Talk {
    /// Binocular zoom, or −1.
    pub zoom: f32,
    /// Minimum time, or −1.
    pub min_time: f32,
    /// Waits for a button.
    pub wait_button: bool,
    /// Which button (3.2).
    pub button: u8,
    /// Speaker OID, or −1.
    pub speaker: i16,
    /// Lip animation OID, or −1.
    pub lip: i16,
    /// Focus OID, or −1.
    pub focus: i16,
    /// Anchor OID, or −1.
    pub anchor: i16,
    /// Subtitle text (8-bit).
    pub text: Vec<u8>,
}

/// The subtitle showing now.
#[derive(Clone, Debug, PartialEq)]
pub struct Subtitle {
    /// The dialogue.
    pub dialogue: Handle,
    /// The text.
    pub text: Vec<u8>,
    /// Characters revealed.
    pub revealed: usize,
    /// Colour RGBA 0..255 (128 = full intensity).
    pub color: [u8; 4],
    /// Speaker OID.
    pub speaker: i16,
    /// The line waits for this button (3.2).
    pub button: Option<u8>,
}

/// The dialogue kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Radio (0).
    Radio,
    /// Briefing (1).
    Briefing,
    /// Confrontation (2).
    Confrontation,
}

/// The dialogue's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DialogueState {
    /// Enabled, waiting for a trigger.
    Enabled,
    /// Triggered, waiting to start (for this long).
    Triggered(f32),
    /// Playing event `index`, for `time` in it.
    Playing {
        /// Event index.
        index: usize,
        /// Time in the event.
        time: f32,
    },
    /// Done (a played confrontation; or unable to call).
    Done,
}

/// A dialogue.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialogue {
    /// Events.
    pub events: Vec<DialogueEvent>,
    /// Kind.
    pub kind: Kind,
    /// State.
    pub state: DialogueState,
    /// Triggered once on load (option 968).
    pub auto: bool,
    /// Cannot be cut short (option 969).
    pub crucial: bool,
    pressed: bool,
}

/// Where the placed part of a record's body ends (after its child list).
pub fn placed_part_end(world: &World, h: Handle) -> Option<usize> {
    let rec = world.record(h)?;
    let r = world.level.record(rec);
    let ext = r.extension?;
    let f = ext.flags;
    let mut pos = ext.offset + 4;
    if f & Extension::PRE_ROTATION != 0 {
        pos += 36;
    }
    if f & Extension::POST_ROTATION != 0 {
        pos += 36;
    }
    if f & (Extension::POSITION_FROM_BASIS | Extension::ROTATION_FROM_BASIS) != 0 {
        pos += 2;
    }
    if f & 0x10 != 0 {
        pos += 2;
    }
    if f & 0x20 != 0 {
        pos += 2 + 12 + 12 + 4;
    }
    if f & 0x40 != 0 {
        pos += 4;
    }
    if f & 0x80 != 0 {
        pos += 1;
    }
    let poses = r.mesh_set.as_ref().map_or(0, |m| m.pose_weights.len());
    let data = &world.level.bytes;
    let k = *data.get(pos)? as usize;
    pos += 1 + k * (2 + 4 * poses);
    let n = u16::from_le_bytes([*data.get(pos)?, *data.get(pos + 1)?]);
    if n == 0 {
        return Some(pos + 2);
    }
    // The child list ends with its last direct child's body.
    let stream = &world.level.stream;
    let mut last = None;
    let mut k = rec + 1;
    while k < r.subtree_end {
        if stream.records[k as usize].parent == Some(rec) {
            last = Some(k);
        }
        k = stream.records[k as usize].subtree_end.max(k + 1);
    }
    last.map(|l| stream.records[l as usize].end)
}

/// Reads a dialogue's events.
pub fn read_events(world: &World, h: Handle) -> Vec<DialogueEvent> {
    let Some(start) = placed_part_end(world, h) else { return Vec::new() };
    let mut r = Reader::at(&world.level.bytes, start);
    let mut out = Vec::new();
    let parsed = (|| -> formats::Result<()> {
        let n = r.i8()?.max(0);
        for _ in 0..n {
            let kind = r.i8()?;
            let e = match kind {
                0 | 7 => {
                    let d = Descriptor::read(&mut r)?;
                    match d {
                        Some(d) if kind == 0 => DialogueEvent::Voice(d),
                        Some(d) => DialogueEvent::Movie(d),
                        None => continue,
                    }
                }
                1 | 2 => DialogueEvent::Panel { right: kind == 2, speaker: r.i16()?, picture: r.i16()? },
                3 => DialogueEvent::Talk(Talk {
                    zoom: r.f32()?,
                    min_time: r.f32()?,
                    wait_button: r.u8()? != 0,
                    button: r.u8()?,
                    speaker: r.i16()?,
                    lip: r.i16()?,
                    focus: r.i16()?,
                    anchor: r.i16()?,
                    text: r.string()?.to_vec(),
                }),
                4 => DialogueEvent::Call(r.i16()?),
                5 => DialogueEvent::LetterboxOn,
                6 => DialogueEvent::LetterboxOff,
                _ => return Ok(()),
            };
            out.push(e);
        }
        Ok(())
    })();
    let _ = parsed;
    out
}

impl Dialogue {
    /// A dialogue read from its record.
    pub fn new(world: &World, h: Handle) -> Dialogue {
        let o = world.options(h);
        let kind = if o.is_some_and(|o| o.has(opt::CONFRONTATION)) {
            Kind::Confrontation
        } else if o.is_some_and(|o| o.has(opt::BRIEFING)) {
            Kind::Briefing
        } else {
            Kind::Radio
        };
        let o = world.options(h);
        let auto = o.is_some_and(|o| o.has(opt::AUTO));
        let crucial = o.is_some_and(|o| o.has(opt::CRUCIAL));
        Dialogue { events: read_events(world, h), kind, state: DialogueState::Enabled, auto, crucial, pressed: false }
    }

    /// Triggers the dialogue.
    pub fn trigger(&mut self) {
        if self.state == DialogueState::Enabled {
            self.state = DialogueState::Triggered(0.0);
        }
    }

    fn key(world: &World, me: Handle) -> i16 {
        world.options(me).and_then(|o| o.i16(opt::PLAYED_KEY)).or_else(|| world.oid(me)).unwrap_or(-1)
    }

    /// Ends the dialogue (finished or cut short): runs the remaining script calls and letterbox events, raises
    /// event 21, marks it played.
    fn finish(&mut self, cx: &mut Cx<'_>, from: usize) {
        for e in self.events.iter().skip(from) {
            match e {
                DialogueEvent::Call(l) => cx.raise(script::event::DIALOGUE_CALL, vec![Value::Int(*l as i32)]),
                DialogueEvent::LetterboxOn => cx.world.shared.screen.letterbox = true,
                DialogueEvent::LetterboxOff => cx.world.shared.screen.letterbox = false,
                _ => {}
            }
        }
        cx.world.shared.subtitle = None;
        cx.world.shared.panels = [None, None];
        if cx.world.shared.dialogue == Some(cx.me) {
            cx.world.shared.dialogue = None;
        }
        if self.kind == Kind::Confrontation {
            cx.world.shared.player.controls_locked = false;
        }
        cx.raise(script::event::DIALOGUE_DONE, Vec::new());
        let key = Self::key(cx.world, cx.me);
        let world_id = cx.world.info.id;
        cx.progress.level(world_id).dialogues.insert(key);
        self.state = if self.kind == Kind::Confrontation { DialogueState::Done } else { DialogueState::Enabled };
    }

    fn speaker_color(world: &World, me: Handle, speaker: i16) -> [u8; 4] {
        world
            .find_nearest(speaker, Some(me))
            .and_then(|s| match world.options(s)?.get(opt::SPEAKER_COLOR) {
                Some(OptionValue::Color(c)) => Some(*c),
                _ => None,
            })
            .unwrap_or([128, 128, 128, 64])
    }

    /// Runs the current event; returns whether it is done.
    fn run(&mut self, cx: &mut Cx<'_>, index: usize, time: f32, first: bool) -> bool {
        let Some(e) = self.events.get(index).cloned() else { return true };
        match e {
            DialogueEvent::Voice(d) => {
                if first {
                    cx.world.shared.voice.push(d);
                    cx.world.shared.voice_playing = true;
                }
                true
            }
            DialogueEvent::Panel { right, speaker, picture } => {
                cx.world.shared.panels[usize::from(right)] = Some((speaker, picture));
                if let Some(s) = cx.world.find_nearest(speaker, Some(cx.me))
                    && let Some(m) = cx.world.options(s).and_then(|o| o.i16(997))
                    && let Some(m) = cx.world.find_nearest(m, Some(s))
                {
                    cx.world.set_option(m, 516, OptionValue::Short(844));
                }
                true
            }
            DialogueEvent::Talk(t) => {
                if first {
                    self.pressed = false;
                    cx.world.shared.subtitle = Some(Subtitle {
                        dialogue: cx.me,
                        text: t.text.clone(),
                        revealed: 0,
                        color: if t.speaker >= 0 {
                            Self::speaker_color(cx.world, cx.me, t.speaker)
                        } else {
                            [128, 128, 128, 64]
                        },
                        speaker: t.speaker,
                        button: t.wait_button.then_some(t.button),
                    });
                }
                let n = t.text.len();
                let rate = if t.min_time > 0.0 { (n as f32 + 10.0) / t.min_time } else { REVEAL_RATE };
                let revealed = ((time * rate) as usize).min(n);
                if let Some(s) = cx.world.shared.subtitle.as_mut() {
                    s.revealed = revealed;
                }
                if t.wait_button && cx.world.shared.confirm_pressed {
                    self.pressed = true;
                }
                revealed >= n
                    && !cx.world.shared.voice_playing
                    && (t.min_time < 0.0 || time >= t.min_time)
                    && (!t.wait_button || self.pressed || t.button == 14)
            }
            DialogueEvent::Call(l) => {
                cx.raise(script::event::DIALOGUE_CALL, vec![Value::Int(l as i32)]);
                true
            }
            DialogueEvent::LetterboxOn => {
                cx.world.shared.screen.letterbox = true;
                true
            }
            DialogueEvent::LetterboxOff => {
                cx.world.shared.screen.letterbox = false;
                true
            }
            DialogueEvent::Movie(d) => {
                cx.world.request(Request::Movie(d.identity));
                true
            }
        }
    }
}

impl ClassBehavior for Dialogue {
    fn ticks(&self) -> bool {
        !matches!(self.state, DialogueState::Enabled | DialogueState::Done)
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let key = Self::key(cx.world, cx.me);
        let world_id = cx.world.info.id;
        let played = cx.progress.level_ref(world_id).is_some_and(|l| l.dialogues.contains(&key));
        if played && self.kind == Kind::Confrontation {
            self.state = DialogueState::Done;
            return;
        }
        if self.auto && !played {
            self.trigger();
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        match self.state {
            DialogueState::Enabled | DialogueState::Done => {}
            DialogueState::Triggered(t) => {
                let playing = cx.world.shared.dialogue.filter(|&d| d != cx.me && cx.world.contains(d));
                let blocked = playing
                    .is_some_and(|d| matches!(cx.world.behavior(d), Some(super::Behavior::Dialogue(x)) if x.crucial));
                if blocked {
                    let t = t + cx.dt;
                    self.state = if t >= START_TIMEOUT { DialogueState::Enabled } else { DialogueState::Triggered(t) };
                    return;
                }
                if let Some(d) = playing {
                    // Cut the playing one short.
                    let s = cx.world.slot(d);
                    if let Some(s) = s {
                        let mut b = cx.world.take_behavior(s);
                        if let super::Behavior::Dialogue(other) = &mut b
                            && let DialogueState::Playing { index, .. } = other.state
                        {
                            let mut sub = Cx {
                                world: cx.world,
                                me: d,
                                dt: 0.0,
                                time: cx.time,
                                progress: cx.progress,
                                collision: cx.collision,
                                rng: cx.rng,
                                anim: cx.anim,
                            };
                            other.finish(&mut sub, index + 1);
                        }
                        cx.world.put_behavior(s, b);
                    }
                }
                cx.world.shared.dialogue = Some(cx.me);
                if self.kind == Kind::Confrontation {
                    cx.world.shared.player.controls_locked = true;
                }
                self.state = DialogueState::Playing { index: 0, time: 0.0 };
                self.step(cx, true);
            }
            DialogueState::Playing { .. } => self.step(cx, false),
        }
    }

    fn get_option(&self, world: &World, me: Handle, id: u16) -> Option<OptionValue> {
        // 977: has been played (scripts.md 11.5); the played flags live in the progress, so report the state.
        (id == 977).then(|| {
            OptionValue::Byte(u8::from(
                self.state == DialogueState::Done || world.options(me).is_some_and(|o| o.bool_or(977, false)),
            ))
        })
    }

    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, _args: &[Arg]) -> bool {
        match id {
            opt::TRIGGER => {
                self.trigger();
                true
            }
            opt::BRIEFING => {
                self.kind = Kind::Briefing;
                true
            }
            opt::CONFRONTATION => {
                self.kind = Kind::Confrontation;
                true
            }
            opt::AUTO => {
                self.auto = true;
                true
            }
            opt::CRUCIAL => {
                self.crucial = true;
                true
            }
            opt::NO_SKIP | opt::DUCK => true,
            opt::UNTRIGGER => {
                if matches!(self.state, DialogueState::Triggered(_)) {
                    self.state = DialogueState::Enabled;
                }
                true
            }
            _ => false,
        }
    }
}

impl Dialogue {
    /// Runs events until one waits.
    fn step(&mut self, cx: &mut Cx<'_>, mut first: bool) {
        for _ in 0..self.events.len() + 1 {
            let DialogueState::Playing { index, time } = self.state else { return };
            if index >= self.events.len() {
                self.finish(cx, index);
                return;
            }
            let time = if first { 0.0 } else { time + cx.dt };
            if self.run(cx, index, time, first) {
                self.state = DialogueState::Playing { index: index + 1, time: 0.0 };
                first = true;
            } else {
                self.state = DialogueState::Playing { index, time };
                return;
            }
        }
    }
}
