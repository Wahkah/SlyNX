//! A Sly 2 world's sound section (sly2/audio.md 1 and 4, world-file.md 3.2): the world's effect bank, the map
//! from game sound IDs to bank sounds, the music stream and its channels, the voice lines and the other stream
//! files.

use alloc::vec;
use alloc::vec::Vec;

use crate::Error;
use crate::sly2::disc::{Cur, FileRecord};

/// One language track of a voice line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceTrack {
    /// The VAG file.
    pub file: Option<FileRecord>,
    /// Headset byte: with the headset output on, a line whose byte is 0 goes to the USB headset; any other value
    /// keeps it on the speakers.
    pub headset: i8,
    /// Duration in seconds.
    pub duration: f32,
}

impl VoiceTrack {
    /// Whether the line goes to the headset when the headset output is on.
    pub fn to_headset(&self) -> bool {
        self.headset == 0
    }
}

/// Target 3 of a timed record: how far the speaker's eyes are closed.
pub const TARGET_EYES: i16 = 3;
/// Target 5 of a timed record: the mouth (lip sync).
pub const TARGET_MOUTH: i16 = 5;
/// Mouth goal set when a line stops early (the spec's open question; the value the reference uses).
pub const CLOSED_MOUTH_GOAL: i32 = 1553;

/// A timed record's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CueValue {
    /// A goal for a state machine (targets 0, 1, 2, 4 and 5).
    Goal(i32),
    /// Eye closure 0..1 (target 3).
    Eyes(f32),
}

/// A timed record of a voice line (sly2/audio.md 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedRecord {
    /// Delay after the previous record, in 1/60 s.
    pub delay: i16,
    /// Target 0..5 (-1 none): 3 eyes, 5 mouth, the others one of the speaker's state machines.
    pub target: i16,
    /// The value.
    pub value: CueValue,
}

impl TimedRecord {
    /// Decodes the 8 bytes (`s16` delay, `s16` target, `s32` or `f32` value).
    pub fn decode(b: &[u8; 8]) -> TimedRecord {
        let delay = i16::from_le_bytes([b[0], b[1]]);
        let target = i16::from_le_bytes([b[2], b[3]]);
        let raw = [b[4], b[5], b[6], b[7]];
        let value = if target == TARGET_EYES {
            CueValue::Eyes(f32::from_le_bytes(raw))
        } else {
            CueValue::Goal(i32::from_le_bytes(raw))
        };
        TimedRecord { delay, target, value }
    }
}

/// One voice line (sly2/audio.md 4).
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceLine {
    /// Line number (names the same file everywhere).
    pub number: i16,
    /// The speaker's class ID (-1 for none).
    pub speaker: i16,
    /// One per language track.
    pub tracks: Vec<VoiceTrack>,
    /// The timed records (eyes, mouth and the speaker's other state machines).
    pub records: Vec<TimedRecord>,
}

/// Fires a voice line's timed records as the line plays, for the engine to apply to the speaker.
#[derive(Clone, Debug, PartialEq)]
pub struct LineCues {
    records: Vec<TimedRecord>,
    next: usize,
    /// Play time in 1/60 s.
    time: f32,
    /// Play time at which record `next` fires, in 1/60 s.
    due: f32,
}

impl LineCues {
    /// Starts the cues of `line` at play time 0.
    pub fn new(line: &VoiceLine) -> LineCues {
        let due = line.records.first().map_or(0.0, |r| r.delay.max(0) as f32);
        LineCues { records: line.records.clone(), next: 0, time: 0.0, due }
    }

    /// Advances the play time by `dt` seconds and appends every record that fires.
    pub fn advance(&mut self, dt: f32, out: &mut Vec<TimedRecord>) {
        self.time += dt * 60.0;
        while self.next < self.records.len() && self.time >= self.due {
            out.push(self.records[self.next]);
            self.next += 1;
            if let Some(r) = self.records.get(self.next) {
                self.due += r.delay.max(0) as f32;
            }
        }
    }

    /// The line stopped early: the remaining records fire at once, then the mouth closes.
    pub fn stop(&mut self, out: &mut Vec<TimedRecord>) {
        out.extend_from_slice(&self.records[self.next..]);
        self.next = self.records.len();
        out.push(TimedRecord { delay: 0, target: TARGET_MOUTH, value: CueValue::Goal(CLOSED_MOUTH_GOAL) });
    }

    /// Whether every record has fired.
    pub fn done(&self) -> bool {
        self.next >= self.records.len()
    }
}

/// The sound section.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldSound {
    /// The world's effect bank.
    pub bank: Option<FileRecord>,
    /// One flag byte per bank sound: 2 marks a continuous (looping) sound (see [`WorldSound::continuous`]).
    pub flags: Vec<u8>,
    /// Size of the sound-ID space: game sound IDs run 0..`id_space`.
    pub id_space: i16,
    /// Per bank sound, the game sound ID it plays.
    pub sound_ids: Vec<i16>,
    /// The world's resident stream (`VPK `, distance layers), if any.
    pub resident: Option<FileRecord>,
    /// One channel number per channel of the resident stream (ambient objects name these).
    pub resident_channels: Vec<i16>,
    /// Language code of each voice track.
    pub tracks: Vec<u8>,
    /// The voice lines.
    pub voice_lines: Vec<VoiceLine>,
    /// Music stream files by music number (`None` for an empty record).
    pub music: Vec<(i16, Option<FileRecord>)>,
}

impl WorldSound {
    /// Reads the section at the cursor.
    pub(crate) fn read(c: &mut Cur<'_>) -> Result<WorldSound, Error> {
        let bank = c.record()?;
        let b = c.i16()? as i64;
        let b = c.count(b, 3)?;
        let flags = c.bytes(b)?.to_vec();
        let id_space = c.i16()?;
        let mut sound_ids = Vec::with_capacity(b);
        for _ in 0..b {
            sound_ids.push(c.i16()?);
        }
        let m = c.i16()?;
        let (resident, resident_channels) = if m > 0 {
            let rec = c.record()?;
            let m = c.count(m as i64, 2)?;
            let mut s = Vec::with_capacity(m);
            for _ in 0..m {
                s.push(c.i16()?);
            }
            (rec, s)
        } else {
            (None, Vec::new())
        };
        let t = c.u8()? as usize;
        let tracks = c.bytes(t)?.to_vec();
        let e = c.i32()? as i64;
        let e = c.count(e, 8 + t * 69)?;
        let mut voice_lines = Vec::with_capacity(e);
        for _ in 0..e {
            let number = c.i16()?;
            let speaker = c.i16()?;
            let mut tr = Vec::with_capacity(t);
            for _ in 0..t {
                let file = c.record()?;
                let headset = c.i8()?;
                let duration = c.f32()?;
                tr.push(VoiceTrack { file, headset, duration });
            }
            let k = c.i32()? as i64;
            let k = c.count(k, 8)?;
            let mut records = Vec::with_capacity(k);
            for _ in 0..k {
                let b: &[u8; 8] = c.bytes(8)?.try_into().map_err(|_| Error::Invalid("timed record"))?;
                records.push(TimedRecord::decode(b));
            }
            voice_lines.push(VoiceLine { number, speaker, tracks: tr, records });
        }
        let r = c.i32()? as i64;
        let r = c.count(r, 66)?;
        let mut music = Vec::with_capacity(r);
        for _ in 0..r {
            let n = c.i16()?;
            music.push((n, c.record()?));
        }
        Ok(WorldSound { bank, flags, id_space, sound_ids, resident, resident_channels, tracks, voice_lines, music })
    }

    /// Parses a sound section on its own.
    pub fn parse(bytes: &[u8]) -> Result<WorldSound, Error> {
        WorldSound::read(&mut Cur::new(bytes, 0))
    }

    /// The map from game sound ID to bank sound index (`None` where no sound is listed).
    pub fn sound_map(&self) -> Vec<Option<u16>> {
        let mut map = vec![None; self.id_space.max(0) as usize];
        for (j, &id) in self.sound_ids.iter().enumerate() {
            if let Some(slot) = usize::try_from(id).ok().and_then(|i| map.get_mut(i)) {
                *slot = Some(j as u16);
            }
        }
        map
    }

    /// The voice line with `number`.
    pub fn voice_line(&self, number: i16) -> Option<&VoiceLine> {
        self.voice_lines.iter().find(|l| l.number == number)
    }

    /// The music stream file with music number `n`.
    pub fn music_stream(&self, n: i16) -> Option<FileRecord> {
        self.music.iter().find(|s| s.0 == n).and_then(|s| s.1)
    }

    /// Whether bank sound `j` is flagged continuous (flag 2). The game also treats sounds whose record has the
    /// continuous bit as continuous; [`crate::SoundEngine::set_sound_flags`] combines both.
    pub fn continuous(&self, j: usize) -> bool {
        self.flags.get(j) == Some(&2)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::sly2::disc::tests::record;

    /// A sound section with 3 bank sounds, a 2-channel music stream, one voice line and one stream record.
    pub(crate) fn section() -> Vec<u8> {
        let mut s = Vec::new();
        s.extend_from_slice(&record(5000, 1000, 58));
        s.extend_from_slice(&3i16.to_le_bytes());
        s.extend_from_slice(&[0, 2, 0]);
        s.extend_from_slice(&10i16.to_le_bytes());
        for id in [4i16, 0, 9] {
            s.extend_from_slice(&id.to_le_bytes());
        }
        s.extend_from_slice(&2i16.to_le_bytes());
        s.extend_from_slice(&record(6000, 2048 * 3, 60));
        s.extend_from_slice(&7i16.to_le_bytes());
        s.extend_from_slice(&8i16.to_le_bytes());
        s.push(1);
        s.push(0);
        s.extend_from_slice(&1i32.to_le_bytes());
        s.extend_from_slice(&1285i16.to_le_bytes());
        s.extend_from_slice(&55i16.to_le_bytes());
        s.extend_from_slice(&record(7000, 123_968, 439));
        s.push(2);
        s.extend_from_slice(&9.835f32.to_le_bytes());
        s.extend_from_slice(&1i32.to_le_bytes());
        for v in [10i16, 1, 2, 3] {
            s.extend_from_slice(&v.to_le_bytes());
        }
        s.extend_from_slice(&1i32.to_le_bytes());
        s.extend_from_slice(&3i16.to_le_bytes());
        s.extend_from_slice(&record(8000, 4096, 70));
        s
    }

    #[test]
    fn cues_fire_on_time_and_on_stop() {
        let rec = |delay, target, value| TimedRecord { delay, target, value };
        let line = VoiceLine {
            number: 1,
            speaker: 55,
            tracks: vec![],
            records: vec![
                rec(6, TARGET_MOUTH, CueValue::Goal(5372)),
                rec(30, TARGET_EYES, CueValue::Eyes(0.5)),
                rec(60, TARGET_MOUTH, CueValue::Goal(5373)),
            ],
        };
        let mut c = LineCues::new(&line);
        let mut out = vec![];
        c.advance(0.05, &mut out);
        assert!(out.is_empty());
        c.advance(0.06, &mut out);
        assert_eq!(out.len(), 1, "fires at 6/60 s");
        c.advance(0.5, &mut out);
        assert_eq!(out.len(), 2, "the next at 36/60 s");
        c.stop(&mut out);
        assert_eq!(out.len(), 4);
        assert_eq!(out[2].value, CueValue::Goal(5373));
        assert_eq!(out[3], rec(0, TARGET_MOUTH, CueValue::Goal(CLOSED_MOUTH_GOAL)));
        assert!(c.done());
        let raw = [0u8, 0, 3, 0, 0, 0, 0x80, 0x3F];
        assert_eq!(TimedRecord::decode(&raw).value, CueValue::Eyes(1.0));
    }

    #[test]
    fn parses_a_section() {
        let w = WorldSound::parse(&section()).unwrap();
        assert_eq!(w.bank.unwrap().identity, 58);
        assert_eq!(w.flags, [0, 2, 0]);
        assert_eq!(w.sound_ids, [4, 0, 9]);
        let map = w.sound_map();
        assert_eq!(map.len(), 10);
        assert_eq!((map[4], map[0], map[9], map[1]), (Some(0), Some(1), Some(2), None));
        assert_eq!(w.resident.unwrap().identity, 60);
        assert_eq!(w.resident_channels, [7, 8]);
        assert_eq!(w.tracks, [0]);
        assert!(w.continuous(1) && !w.continuous(0));
        let l = w.voice_line(1285).unwrap();
        assert_eq!((l.speaker, l.tracks[0].headset), (55, 2));
        assert!(!l.tracks[0].to_headset());
        assert_eq!(l.records[0], TimedRecord { delay: 10, target: 1, value: CueValue::Goal(3 << 16 | 2) });
        assert!((l.tracks[0].duration - 9.835).abs() < 1e-6);
        assert_eq!(w.music_stream(3).unwrap().identity, 70);
        assert!(w.music_stream(4).is_none());
        // Truncated data is an error, not a panic.
        assert!(WorldSound::parse(&section()[..100]).is_err());
    }
}
