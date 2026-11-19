//! Sly 2 dialogue (specs/sly2/dialogue-and-voice.md): the dialogue commands of sequence objects (class 114), and a
//! line player that plays a voice line, fires its timed records and reveals its subtitle.
//!
//! - [`Command::decode`] reads one sequence option (an option ID and its typed-argument bytes) into a dialogue
//!   command (section 3); other options are `None` (camera, moves and so on belong to other specs).
//! - [`Talk`] plays one line at a time (section 4): starting a line stops the one playing (its remaining timed
//!   records fire, then the mouth closes), the voice goes out as dialogue, the timed records fire as the line plays,
//!   and the subtitle (the world string whose id is the line number; none for a quiet line) is revealed letter by
//!   letter over the line's duration minus 0.5 s, followed by a `|` cursor that blinks at 4 Hz once the text is
//!   whole. [`speaker_colour`] colours it by speaker class.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::engine::{SoundEngine, SoundHandle};
use crate::sly2::sound::{LineCues, TimedRecord, VoiceLine};
use crate::spatial::Emitter;
use crate::vag::VagSound;

/// Option 695: play a voice line (line number, quiet, wait flag).
pub const PLAY_LINE: i16 = 695;
/// Option 708: wait for the line (time, −1 = to its end).
pub const WAIT_LINE: i16 = 708;
/// Option 692: wait for a button (mask, pad).
pub const WAIT_BUTTON: i16 = 692;
/// Option 696: clear the subtitle.
pub const CLEAR_SUBTITLE: i16 = 696;
/// Option 685: open the binocucom for a radio talk (viewer, other speaker, window, flag).
pub const OPEN_BINOCUCOM: i16 = 685;
/// Option 686: close the binocucom (flag).
pub const CLOSE_BINOCUCOM: i16 = 686;
/// Option 679: close an object's eyes (object, shader animation, amount).
pub const CLOSE_EYES: i16 = 679;
/// Option 697: hold a talk effect (object, kind, flag).
pub const HOLD_EFFECT: i16 = 697;
/// Option 698: release a talk effect (object, kind, flag).
pub const RELEASE_EFFECT: i16 = 698;
/// Sequence objects (class 114).
pub const SEQUENCE_CLASS: i16 = 114;
/// The talk-target table in the executable: state-machine object IDs for targets 0 to 5 (−1 for target 3).
pub const TALK_TARGETS_VA: u32 = 0x002C_3C40;
/// The subtitle cursor.
pub const CURSOR: char = '|';
/// The reveal ends this long before the line does.
pub const REVEAL_LEAD: f32 = 0.5;

/// A dialogue command of a sequence (dialogue-and-voice.md 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// 695.
    PlayLine { line: i16, quiet: bool, wait: bool },
    /// 708: −1 waits for the end of the line.
    WaitLine { time: f32 },
    /// 692.
    WaitButton { mask: i32, pad: i32 },
    /// 696.
    ClearSubtitle,
    /// 685.
    OpenBinocucom { viewer: i32, other: i32, window: i32, flag: bool },
    /// 686.
    CloseBinocucom { flag: bool },
    /// 679.
    CloseEyes { object: i32, animation: i32, amount: f32 },
    /// 697 (`hold` true) and 698.
    TalkEffect { hold: bool, object: i32, kind: i32, flag: bool },
}

/// One typed argument: its tag and value bytes.
fn args(mut b: &[u8]) -> Option<Vec<(u16, &[u8])>> {
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
        let count = if tag & 0x8000 != 0 {
            let c = *b.first()? as usize;
            b = &b[1..];
            c
        } else {
            1
        };
        for _ in 0..count {
            out.push((tag & 0x7FFF, b.get(..size)?));
            b = &b[size..];
        }
    }
    Some(out)
}

fn int(v: &[u8]) -> i32 {
    match v.len() {
        1 => v[0] as i8 as i32,
        2 => i16::from_le_bytes([v[0], v[1]]) as i32,
        _ => i32::from_le_bytes([v[0], v[1], v[2], v[3]]),
    }
}

fn float(v: &[u8]) -> f32 {
    if v.len() == 4 { f32::from_le_bytes([v[0], v[1], v[2], v[3]]) } else { int(v) as f32 }
}

impl Command {
    /// Decodes option `id` with its value bytes (the typed argument list); `None` for another option or bytes that
    /// do not match the command's arguments.
    pub fn decode(id: i16, value: &[u8]) -> Option<Command> {
        let a = args(value)?;
        let i = |k: usize| a.get(k).map(|x| int(x.1));
        let f = |k: usize| a.get(k).map(|x| float(x.1));
        Some(match (id, a.len()) {
            (PLAY_LINE, 3) => Command::PlayLine { line: i(0)? as i16, quiet: i(1)? != 0, wait: i(2)? != 0 },
            (WAIT_LINE, 1) => Command::WaitLine { time: f(0)? },
            (WAIT_BUTTON, 2) => Command::WaitButton { mask: i(0)?, pad: i(1)? },
            (CLEAR_SUBTITLE, 0) => Command::ClearSubtitle,
            (OPEN_BINOCUCOM, 4) => {
                Command::OpenBinocucom { viewer: i(0)?, other: i(1)?, window: i(2)?, flag: i(3)? != 0 }
            }
            (CLOSE_BINOCUCOM, 1) => Command::CloseBinocucom { flag: i(0)? != 0 },
            (CLOSE_EYES, 3) => Command::CloseEyes { object: i(0)?, animation: i(1)?, amount: f(2)? },
            (HOLD_EFFECT | RELEASE_EFFECT, 3) => {
                Command::TalkEffect { hold: id == HOLD_EFFECT, object: i(0)?, kind: i(1)?, flag: i(2)? != 0 }
            }
            _ => return None,
        })
    }
}

/// The subtitle colour of a speaker class (RGBA, alpha on the 0x80 scale).
pub fn speaker_colour(class: i16) -> [u8; 4] {
    match class {
        53 => [0x33, 0x66, 0x99, 0x7F],
        55 => [0x33, 0x99, 0x33, 0x7F],
        54 => [0x99, 0x33, 0x66, 0x7F],
        63 => [0x99, 0x33, 0x33, 0x7F],
        60 => [0x99, 0x33, 0x99, 0x7F],
        _ => [0x66, 0x66, 0x66, 0x7F],
    }
}

/// Characters of a `length`-character subtitle shown `t` seconds into a line of `duration` seconds:
/// `⌊t / (duration − 0.5) × length⌋`, at most `length` (all of it for a line of 0.5 s or less).
pub fn revealed(t: f32, duration: f32, length: usize) -> usize {
    let span = duration - REVEAL_LEAD;
    if span <= 0.0 {
        return length;
    }
    let n = (t.max(0.0) / span * length as f32) as usize;
    n.min(length)
}

/// Subtitle text as the game shows it: the stored bytes, one character each.
pub fn subtitle_text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// The subtitle on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Subtitle {
    /// The visible text, with the cursor when it shows.
    pub text: String,
    /// The speaker's class.
    pub speaker: i16,
    /// Its colour ([`speaker_colour`]).
    pub colour: [u8; 4],
}

/// The line playing.
#[derive(Clone, Debug)]
struct Playing {
    number: i16,
    speaker: i16,
    duration: f32,
    time: f32,
    cues: LineCues,
    handle: SoundHandle,
    text: Option<String>,
}

/// Plays voice lines one at a time (dialogue-and-voice.md 4).
#[derive(Clone, Debug, Default)]
pub struct Talk {
    playing: Option<Playing>,
    subtitle: Option<(String, i16, f32)>,
}

impl Talk {
    /// Starts `line` (stopping the line playing; its remaining timed records go to `out`). `voice` is the line's
    /// VAG file (first track), `subtitle` its string (ignored when `quiet`).
    pub fn play(
        &mut self,
        engine: &mut SoundEngine,
        line: &VoiceLine,
        voice: Option<Arc<VagSound>>,
        subtitle: Option<&[u8]>,
        quiet: bool,
        out: &mut Vec<TimedRecord>,
    ) {
        let text = subtitle.map(subtitle_text);
        self.play_text(engine, line, voice, text.as_deref(), quiet, out);
    }

    /// As [`Talk::play`] with the subtitle as text (Sly 3's strings are UTF-16).
    pub fn play_text(
        &mut self,
        engine: &mut SoundEngine,
        line: &VoiceLine,
        voice: Option<Arc<VagSound>>,
        subtitle: Option<&str>,
        quiet: bool,
        out: &mut Vec<TimedRecord>,
    ) {
        self.play_text_at(engine, line, voice, subtitle, quiet, Emitter::global(), out);
    }

    /// As [`Talk::play_text`], played from `emitter` (Sly 3 lines with a source object; the caller moves it with
    /// [`SoundEngine::set_emitter`] on [`Talk::handle`]).
    #[allow(clippy::too_many_arguments)]
    pub fn play_text_at(
        &mut self,
        engine: &mut SoundEngine,
        line: &VoiceLine,
        voice: Option<Arc<VagSound>>,
        subtitle: Option<&str>,
        quiet: bool,
        emitter: Emitter,
        out: &mut Vec<TimedRecord>,
    ) {
        self.stop(engine, out);
        let duration = line.tracks.first().map_or(0.0, |t| t.duration);
        let handle = match voice {
            Some(v) => engine.play_dialogue(v, emitter),
            None => SoundHandle::NONE,
        };
        let text = if quiet { None } else { subtitle.map(String::from) };
        self.subtitle = text.clone().map(|t| (t, line.speaker, duration));
        self.playing = Some(Playing {
            number: line.number,
            speaker: line.speaker,
            duration,
            time: 0.0,
            cues: LineCues::new(line),
            handle,
            text,
        });
    }

    /// Stops the line playing: its remaining timed records fire into `out`, the mouth closes. The subtitle stays.
    pub fn stop(&mut self, engine: &mut SoundEngine, out: &mut Vec<TimedRecord>) {
        if let Some(mut p) = self.playing.take() {
            p.cues.stop(out);
            engine.stop(p.handle);
        }
    }

    /// Advances the line by `dt` seconds; the timed records that fire go to `out`. The line ends after its duration.
    pub fn update(&mut self, dt: f32, out: &mut Vec<TimedRecord>) {
        let Some(p) = self.playing.as_mut() else { return };
        p.time += dt;
        p.cues.advance(dt, out);
        if p.time >= p.duration && p.cues.done() {
            self.playing = None;
        }
    }

    /// The number of the line playing.
    pub fn line(&self) -> Option<i16> {
        self.playing.as_ref().map(|p| p.number)
    }

    /// The sound of the line playing (`None` when none plays or it has no voice).
    pub fn handle(&self) -> Option<SoundHandle> {
        self.playing.as_ref().map(|p| p.handle).filter(|&h| h != SoundHandle::NONE)
    }

    /// The speaker class of the line playing.
    pub fn speaker(&self) -> Option<i16> {
        self.playing.as_ref().map(|p| p.speaker)
    }

    /// Seconds into the line playing.
    pub fn time(&self) -> Option<f32> {
        self.playing.as_ref().map(|p| p.time)
    }

    /// Whether a wait-for-line (command 708 with time −1) is over; with a non-negative `time` the wait ends that many
    /// seconds into the line.
    pub fn wait_done(&self, time: f32) -> bool {
        match &self.playing {
            None => true,
            Some(p) => time >= 0.0 && p.time >= time,
        }
    }

    /// Clears the subtitle (command 696, or closing the binocucom).
    pub fn clear_subtitle(&mut self) {
        self.subtitle = None;
        if let Some(p) = self.playing.as_mut() {
            p.text = None;
        }
    }

    /// The subtitle to draw: the revealed text of the line playing (or of the last line, whole, once it ended), and
    /// the cursor: shown while revealing, then blinking (shown when `⌊4 game_time⌋` is odd).
    pub fn subtitle(&self, game_time: f32) -> Option<Subtitle> {
        let (text, speaker, duration) = self.subtitle.as_ref()?;
        let len = text.chars().count();
        let t = self.playing.as_ref().map_or(*duration, |p| p.time);
        let n = revealed(t, *duration, len);
        let mut visible: String = text.chars().take(n).collect();
        let cursor = n < len || ((4.0 * game_time) as i64) % 2 == 1;
        if cursor {
            visible.push(CURSOR);
        }
        Some(Subtitle { text: visible, speaker: *speaker, colour: speaker_colour(*speaker) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sly2::sound::VoiceTrack;

    #[test]
    fn line_1300_bytes_and_reveal() {
        // dialogue-and-voice.md 8: line 1300's argument bytes, and its reveal after 0.6 s of 1.28 s.
        let b = [0x10, 0, 0x14, 0x05, 0x5d, 0x10, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(Command::decode(PLAY_LINE, &b), Some(Command::PlayLine { line: 1300, quiet: false, wait: false }));
        assert_eq!(Command::decode(PLAY_LINE + 1, &b), None);
        assert_eq!(revealed(0.6, 1.28, 29), 22);
        assert_eq!(revealed(5.0, 1.28, 29), 29);
        assert_eq!(revealed(0.1, 0.4, 7), 7);
        let mut w = Vec::new();
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&(-1.0f32).to_le_bytes());
        assert_eq!(Command::decode(WAIT_LINE, &w), Some(Command::WaitLine { time: -1.0 }));
        assert_eq!(Command::decode(CLEAR_SUBTITLE, &[]), Some(Command::ClearSubtitle));
        assert_eq!(speaker_colour(53), [0x33, 0x66, 0x99, 0x7F]);
        assert_eq!(speaker_colour(-1), [0x66, 0x66, 0x66, 0x7F]);
    }

    #[test]
    fn talk_plays_one_line_at_a_time() {
        let (mut engine, _r) = crate::create(crate::Config::default());
        let line = |n, d| VoiceLine {
            number: n,
            speaker: 53,
            tracks: alloc::vec![VoiceTrack { file: None, headset: 1, duration: d }],
            records: alloc::vec![TimedRecord { delay: 30, target: 5, value: crate::sly2::sound::CueValue::Goal(7) }],
        };
        let mut t = Talk::default();
        let mut out = Vec::new();
        t.play(&mut engine, &line(1, 1.5), None, Some(b"abcd"), false, &mut out);
        assert_eq!(t.subtitle(0.0).unwrap().text, "|");
        t.update(0.5, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(t.subtitle(0.0).unwrap().text, "ab|");
        assert!(!t.wait_done(-1.0) && t.wait_done(0.25));
        // A new line stops this one: its mouth closes.
        t.play(&mut engine, &line(2, 1.0), None, Some(b"xy"), true, &mut out);
        assert_eq!(out.last().unwrap().value, crate::sly2::sound::CueValue::Goal(1553));
        assert_eq!(t.subtitle(0.0), None, "quiet line: no subtitle");
        t.update(2.0, &mut out);
        assert!(t.wait_done(-1.0));
        assert_eq!(t.line(), None);
    }
}
