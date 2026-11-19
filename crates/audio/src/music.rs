//! Music files (audio.md 5): the instrument bank, its key regions, the segment directory, the segments' MIDI-like
//! events and the adaptive commands carried in system-exclusive messages.

use alloc::vec::Vec;
use core::ops::Range;

use crate::Error;
use crate::bank::{VOICE_PARAMS_SIZE, VoiceParams};
use crate::bytes::{i8_at, i16_at, magic, u16_at, u32_at};

/// Shortest tempo accepted, microseconds per quarter note (6,000 beats per minute); shorter values are raised.
pub const MIN_TEMPO: u32 = 10_000;
/// Longest tempo accepted (6 beats per minute).
pub const MAX_TEMPO: u32 = 10_000_000;
/// Most ticks per quarter note accepted (the game uses 480).
pub const MAX_TICKS_PER_QUARTER: u32 = 9600;

/// Clamps a tempo to [`MIN_TEMPO`]..=[`MAX_TEMPO`].
pub fn clamp_tempo(t: u32) -> u32 {
    t.clamp(MIN_TEMPO, MAX_TEMPO)
}

/// Key-region flag: the region has no sample and is skipped.
pub const REGION_NO_SAMPLE: u16 = 0x80;

/// The track record (audio.md 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackRecord {
    /// Volume (127).
    pub volume: i16,
    /// How many times a segment plays before it ends (0 = forever).
    pub repeat: u8,
    /// Mix group (1).
    pub group: i8,
    /// Pan in degrees.
    pub pan: i16,
}

/// An instrument: a volume, a pan and a run of key regions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    /// Volume 0..127.
    pub volume: i8,
    /// Pan in degrees.
    pub pan: i16,
    /// Indices into [`MusicFile::regions`].
    pub regions: Range<usize>,
}

/// One sequence segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Byte range of the event data in the music file.
    pub events: Range<usize>,
    /// Initial tempo, microseconds per quarter note.
    pub tempo: u32,
    /// Ticks per quarter note.
    pub ticks_per_quarter: u32,
    /// The handle word (`0x03ss0000`).
    pub handle: u32,
}

/// A parsed music file. Holds the whole file; samples are decoded while they play.
#[derive(Clone, Debug)]
pub struct MusicFile {
    /// The file.
    pub bytes: Vec<u8>,
    /// Four-letter track name (`PARI`).
    pub name: [u8; 4],
    /// The track record.
    pub track: TrackRecord,
    /// Instruments, selected by program number.
    pub instruments: Vec<Instrument>,
    /// All key regions.
    pub regions: Vec<VoiceParams>,
    /// Byte range of the sample section.
    pub samples: Range<usize>,
    /// Segments.
    pub segments: Vec<Segment>,
}

fn section(b: &[u8], i: usize) -> Result<Range<usize>, Error> {
    let off = u32_at(b, 8 + i * 8)? as usize;
    let size = u32_at(b, 12 + i * 8)? as usize;
    off.checked_add(size).filter(|&e| e <= b.len()).map(|e| off..e).ok_or(Error::Invalid("music section"))
}

impl MusicFile {
    /// Parses a music file (audio.md 5.1 to 5.3).
    pub fn parse(bytes: Vec<u8>) -> Result<MusicFile, Error> {
        let b = &bytes;
        if u32_at(b, 0)? != 1 || u32_at(b, 4)? != 3 {
            return Err(Error::Invalid("music file: file kind 1 with 3 sections expected"));
        }
        let inst_sec = section(b, 0)?;
        let samples = section(b, 1)?;
        let seq_sec = section(b, 2)?;

        let s = &b[inst_sec.clone()];
        magic(s, 0, b"SBv2")?;
        let mut name = [0u8; 4];
        name.copy_from_slice(s.get(12..16).ok_or(Error::Invalid("instrument section"))?);
        let instrument_count = u16_at(s, 22)? as usize;
        let track_off = u32_at(s, 28)? as usize;
        let inst_table = u32_at(s, 32)? as usize;
        let region_table = u32_at(s, 36)? as usize;
        let track = TrackRecord {
            volume: i16_at(s, track_off + 16)?,
            repeat: s.get(track_off + 18).copied().ok_or(Error::Invalid("track record"))?,
            group: i8_at(s, track_off + 19)?,
            pan: i16_at(s, track_off + 20)?,
        };
        let mut instruments = Vec::with_capacity(instrument_count);
        let mut region_end = 0usize;
        for i in 0..instrument_count {
            let o = inst_table + i * 8;
            let count = *s.get(o).ok_or(Error::Invalid("instrument table"))? as usize;
            if count == 0 {
                // An empty instrument's region offset points anywhere; it has no regions.
                instruments.push(Instrument { volume: i8_at(s, o + 1)?, pan: i16_at(s, o + 2)?, regions: 0..0 });
                continue;
            }
            let first = u32_at(s, o + 4)? as usize;
            let rel = first.checked_sub(region_table).ok_or(Error::Invalid("key region before its table"))?;
            if rel % VOICE_PARAMS_SIZE != 0 {
                return Err(Error::Invalid("key region not on a 24-byte boundary"));
            }
            let start = rel / VOICE_PARAMS_SIZE;
            region_end = region_end.max(start + count);
            instruments.push(Instrument {
                volume: i8_at(s, o + 1)?,
                pan: i16_at(s, o + 2)?,
                regions: start..start + count,
            });
        }
        if region_end
            .checked_mul(VOICE_PARAMS_SIZE)
            .and_then(|n| n.checked_add(region_table))
            .is_none_or(|end| end > s.len())
        {
            return Err(Error::Invalid("key regions run past the instrument section"));
        }
        let mut regions = Vec::with_capacity(region_end);
        for i in 0..region_end {
            regions.push(VoiceParams::parse(s, region_table + i * VOICE_PARAMS_SIZE)?);
        }

        let q = &b[seq_sec.clone()];
        let dir_off = u32_at(q, 8)? as usize;
        magic(q, dir_off, b"MMID")?;
        let seg_count = *q.get(dir_off + 7).ok_or(Error::Invalid("segment directory"))? as usize;
        let mut offsets = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            offsets.push(dir_off + u32_at(q, dir_off + 16 + i * 4)? as usize);
        }
        let mut segments = Vec::with_capacity(seg_count);
        for (i, &start) in offsets.iter().enumerate() {
            // Each segment ends where the next begins; the last at the end of the section.
            let end = offsets.get(i + 1).copied().unwrap_or(q.len());
            if end < start || end > q.len() {
                return Err(Error::Invalid("segment range"));
            }
            magic(q, start, b"MID ")?;
            let ev = start + u32_at(q, start + 24)? as usize;
            if ev > end {
                return Err(Error::Invalid("segment event data starts past its end"));
            }
            let tpq = u32_at(q, start + 36)?;
            segments.push(Segment {
                events: seq_sec.start + ev..seq_sec.start + end,
                tempo: clamp_tempo(u32_at(q, start + 32)?),
                ticks_per_quarter: tpq.clamp(1, MAX_TICKS_PER_QUARTER),
                handle: u32_at(q, start + 40)?,
            });
        }
        Ok(MusicFile { bytes, name, track, instruments, regions, samples, segments })
    }

    /// The sample section.
    pub fn sample_data(&self) -> &[u8] {
        &self.bytes[self.samples.clone()]
    }

    /// The event bytes of segment `i`.
    pub fn segment_events(&self, i: usize) -> &[u8] {
        self.segments.get(i).map_or(&[], |s| &self.bytes[s.events.clone()])
    }

    /// The key regions of instrument `program`.
    pub fn instrument_regions(&self, program: u8) -> &[VoiceParams] {
        self.instruments.get(program as usize).map_or(&[], |i| &self.regions[i.regions.clone()])
    }
}

/// One event of a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// Note on with a non-zero velocity.
    NoteOn {
        /// Channel 0..15.
        channel: u8,
        /// Note number.
        note: u8,
        /// Velocity 1..127.
        velocity: u8,
    },
    /// Note release (channel pressure `0xDn` naming the note, `0x9n` with velocity 0, or `0x8n`).
    NoteOff {
        /// Channel.
        channel: u8,
        /// Note number.
        note: u8,
    },
    /// Controller change.
    Controller {
        /// Channel.
        channel: u8,
        /// Controller number (7 volume, 10 pan).
        controller: u8,
        /// Value 0..127.
        value: u8,
    },
    /// Program change: selects the instrument.
    Program {
        /// Channel.
        channel: u8,
        /// Instrument index.
        program: u8,
    },
    /// Pitch bend, -8192..8191.
    PitchBend {
        /// Channel.
        channel: u8,
        /// Signed bend.
        value: i16,
    },
    /// Polyphonic key pressure (ignored).
    KeyPressure {
        /// Channel.
        channel: u8,
    },
    /// Meta 0x51: microseconds per quarter note.
    Tempo(u32),
    /// Meta 0x2F: end of the segment.
    EndOfSegment,
    /// Any other meta event.
    Meta {
        /// Meta type.
        kind: u8,
        /// Its data.
        data: &'a [u8],
    },
    /// An adaptive command list (`F0 75 ... F7`): the bytes between `75` and the closing `F7`.
    Adaptive(&'a [u8]),
    /// Another system-exclusive message (its bytes after `F0`, up to the next `F7`).
    SysEx(&'a [u8]),
}

/// Reads a variable-length quantity at `*pos`.
pub fn read_vlq(data: &[u8], pos: &mut usize) -> Result<u32, Error> {
    let mut v = 0u32;
    for _ in 0..4 {
        let b = *data.get(*pos).ok_or(Error::Invalid("variable-length quantity runs off the segment"))?;
        *pos += 1;
        v = (v << 7) | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(Error::Invalid("variable-length quantity longer than 4 bytes"))
}

/// Number of argument bytes of a fixed-size adaptive command, or `None` for the variable ones (`0B`, `0F`) and
/// unknown codes.
pub fn command_args(code: u8) -> Option<usize> {
    match code {
        0x04 | 0x05 => Some(0),
        0x00..=0x03 | 0x0C | 0x0D | 0x0E | 0x10 | 0x11 | 0x12 | 0x14 | 0x15 => Some(1),
        0x06 | 0x07 | 0x13 | 0x16 => Some(2),
        _ => None,
    }
}

/// Byte offset just past the command starting at `pos` of a command list. Variable commands (`0B`, `0F`) run up
/// to and including their own `F7`.
pub fn command_end(data: &[u8], pos: usize) -> Result<usize, Error> {
    let code = *data.get(pos).ok_or(Error::Invalid("adaptive command runs off the data"))?;
    match code {
        0x0B | 0x0F => {
            let rel =
                data[pos + 1..].iter().position(|&b| b == 0xF7).ok_or(Error::Invalid("unterminated definition"))?;
            Ok(pos + 1 + rel + 1)
        }
        _ => {
            let n = command_args(code).ok_or(Error::Invalid("unknown adaptive command"))?;
            let end = pos + 1 + n;
            if end > data.len() {
                return Err(Error::Invalid("adaptive command arguments run off the data"));
            }
            Ok(end)
        }
    }
}

/// One adaptive command (audio.md 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    /// `00 x`: skip the next command unless intensity > x.
    IfIntensityAbove(u8),
    /// `01 x`: skip the next command unless intensity = x + 1.
    IfIntensityIs(u8),
    /// `02 x`: skip the next command unless intensity < x.
    IfIntensityBelow(u8),
    /// `03 s`: stop segment s.
    StopSegment(u8),
    /// `04`: start of a block that runs up to the matching `05`. A failed test right before it skips the whole
    /// block. (audio.md calls `04` "else"; every use on the disc follows a test, as the body of that test, for
    /// example `16 01 00 04 02 12 13 01 01 05` = "if register 1 = 0 { if intensity < 18 { register 1 = 1 } }".)
    Begin,
    /// `05`: end of a block.
    End,
    /// `06 r x`: skip the next command unless register r < x.
    IfRegisterBelow(u8, u8),
    /// `07 r x`: skip the next command unless register r > x.
    IfRegisterAbove(u8, u8),
    /// `16 r x`: skip the next command unless register r = x.
    IfRegisterIs(u8, u8),
    /// `0B m ... F7`: define macro m (the body, without the `F7`).
    DefineMacro(u8, &'a [u8]),
    /// `0C m`: run macro m.
    RunMacro(u8),
    /// `0D r`: start segment (register r - 1) and stop this one.
    JumpToRegister(u8),
    /// `0E r`: start segment (register r - 1).
    StartFromRegister(u8),
    /// `0F g b (ch lo hi)* F7`: define channel group g tested against intensity (b = 0) or register b - 1.
    DefineGroup {
        /// Group number.
        group: u8,
        /// 0 = intensity, otherwise register + 1.
        source: u8,
        /// `(channel, low, high)` triples.
        entries: &'a [u8],
    },
    /// `10 g`: activate channel group g.
    ActivateGroup(u8),
    /// `11 s`: start segment s and stop this one.
    Jump(u8),
    /// `12 s`: start segment s.
    Start(u8),
    /// `13 r x`: set register r to x.
    SetRegister(u8, u8),
    /// `14 r`: increment register r.
    IncrementRegister(u8),
    /// `15 r`: decrement register r.
    DecrementRegister(u8),
}

impl Command<'_> {
    /// The command code.
    pub fn code(&self) -> u8 {
        match self {
            Command::IfIntensityAbove(_) => 0x00,
            Command::IfIntensityIs(_) => 0x01,
            Command::IfIntensityBelow(_) => 0x02,
            Command::StopSegment(_) => 0x03,
            Command::Begin => 0x04,
            Command::End => 0x05,
            Command::IfRegisterBelow(..) => 0x06,
            Command::IfRegisterAbove(..) => 0x07,
            Command::DefineMacro(..) => 0x0B,
            Command::RunMacro(_) => 0x0C,
            Command::JumpToRegister(_) => 0x0D,
            Command::StartFromRegister(_) => 0x0E,
            Command::DefineGroup { .. } => 0x0F,
            Command::ActivateGroup(_) => 0x10,
            Command::Jump(_) => 0x11,
            Command::Start(_) => 0x12,
            Command::SetRegister(..) => 0x13,
            Command::IncrementRegister(_) => 0x14,
            Command::DecrementRegister(_) => 0x15,
            Command::IfRegisterIs(..) => 0x16,
        }
    }

    /// Whether this is a test (`00`, `01`, `02`, `06`, `07`, `16`).
    pub fn is_test(&self) -> bool {
        matches!(self.code(), 0x00 | 0x01 | 0x02 | 0x06 | 0x07 | 0x16)
    }
}

/// Decodes the command at `pos`, returning it and the offset just past it.
pub fn decode_command(data: &[u8], pos: usize) -> Result<(Command<'_>, usize), Error> {
    let end = command_end(data, pos)?;
    let a = |i: usize| data[pos + 1 + i];
    let cmd = match data[pos] {
        0x00 => Command::IfIntensityAbove(a(0)),
        0x01 => Command::IfIntensityIs(a(0)),
        0x02 => Command::IfIntensityBelow(a(0)),
        0x03 => Command::StopSegment(a(0)),
        0x04 => Command::Begin,
        0x05 => Command::End,
        0x06 => Command::IfRegisterBelow(a(0), a(1)),
        0x07 => Command::IfRegisterAbove(a(0), a(1)),
        0x16 => Command::IfRegisterIs(a(0), a(1)),
        0x0B => {
            let body = &data[pos + 1..end - 1];
            let (&m, body) = body.split_first().ok_or(Error::Invalid("macro definition without a number"))?;
            Command::DefineMacro(m, body)
        }
        0x0C => Command::RunMacro(a(0)),
        0x0D => Command::JumpToRegister(a(0)),
        0x0E => Command::StartFromRegister(a(0)),
        0x0F => {
            let body = &data[pos + 1..end - 1];
            if body.len() < 2 || (body.len() - 2) % 3 != 0 {
                return Err(Error::Invalid("channel group definition"));
            }
            Command::DefineGroup { group: body[0], source: body[1], entries: &body[2..] }
        }
        0x10 => Command::ActivateGroup(a(0)),
        0x11 => Command::Jump(a(0)),
        0x12 => Command::Start(a(0)),
        0x13 => Command::SetRegister(a(0), a(1)),
        0x14 => Command::IncrementRegister(a(0)),
        0x15 => Command::DecrementRegister(a(0)),
        _ => return Err(Error::Invalid("unknown adaptive command")),
    };
    Ok((cmd, end))
}

/// Iterates the commands of a command list (an [`Event::Adaptive`] payload or a macro body).
pub fn commands(list: &[u8]) -> impl Iterator<Item = Result<Command<'_>, Error>> {
    let mut pos = 0;
    let mut failed = false;
    core::iter::from_fn(move || {
        if failed || pos >= list.len() {
            return None;
        }
        match decode_command(list, pos) {
            Ok((c, end)) => {
                pos = end;
                Some(Ok(c))
            }
            Err(e) => {
                failed = true;
                Some(Err(e))
            }
        }
    })
}

/// A cursor over a segment's events: delta times, running status and the game's message variants.
#[derive(Clone, Copy, Debug)]
pub struct EventCursor {
    /// Offset of the next delta time, relative to the event data.
    pub pos: usize,
    running: u8,
}

impl EventCursor {
    /// A cursor at the first event.
    pub const fn new() -> EventCursor {
        EventCursor { pos: 0, running: 0 }
    }

    /// Reads the next delta time and event, or `None` at the end of the data.
    pub fn next<'a>(&mut self, data: &'a [u8]) -> Result<Option<(u32, Event<'a>)>, Error> {
        let Some(delta) = self.read_delta(data)? else { return Ok(None) };
        Ok(Some((delta, self.read_event(data)?)))
    }

    /// Reads the next delta time, or `None` at the end of the data. The event follows with
    /// [`EventCursor::read_event`].
    pub fn read_delta(&mut self, data: &[u8]) -> Result<Option<u32>, Error> {
        if self.pos >= data.len() {
            return Ok(None);
        }
        read_vlq(data, &mut self.pos).map(Some)
    }

    /// Reads the event after a delta time.
    pub fn read_event<'a>(&mut self, data: &'a [u8]) -> Result<Event<'a>, Error> {
        let mut p = self.pos;
        let ev = self.read_event_at(data, &mut p)?;
        self.pos = p;
        Ok(ev)
    }

    fn read_event_at<'a>(&mut self, data: &'a [u8], p: &mut usize) -> Result<Event<'a>, Error> {
        let byte = |p: &mut usize| -> Result<u8, Error> {
            let b = *data.get(*p).ok_or(Error::Invalid("event runs off the segment"))?;
            *p += 1;
            Ok(b)
        };
        let mut status = byte(p)?;
        match status {
            0xFF => {
                self.running = 0;
                let kind = byte(p)?;
                let len = read_vlq(data, p)? as usize;
                let body = data.get(*p..*p + len).ok_or(Error::Invalid("meta event runs off the segment"))?;
                *p += len;
                return Ok(match kind {
                    0x51 if len >= 3 => Event::Tempo(u32::from_be_bytes([0, body[0], body[1], body[2]])),
                    0x2F => Event::EndOfSegment,
                    _ => Event::Meta { kind, data: body },
                });
            }
            0xF0 => {
                self.running = 0;
                let start = *p;
                if data.get(start) == Some(&0x75) {
                    // Commands up to the F7 that is not part of a definition.
                    let mut q = start + 1;
                    loop {
                        match data.get(q) {
                            Some(0xF7) => break,
                            Some(_) => q = command_end(data, q)?,
                            None => return Err(Error::Invalid("unterminated adaptive message")),
                        }
                    }
                    *p = q + 1;
                    return Ok(Event::Adaptive(&data[start + 1..q]));
                }
                let rel = data[start..].iter().position(|&b| b == 0xF7).ok_or(Error::Invalid("unterminated SysEx"))?;
                *p = start + rel + 1;
                return Ok(Event::SysEx(&data[start..start + rel]));
            }
            0xF7 => {
                self.running = 0;
                let len = read_vlq(data, p)? as usize;
                let body = data.get(*p..*p + len).ok_or(Error::Invalid("escape runs off the segment"))?;
                *p += len;
                return Ok(Event::SysEx(body));
            }
            0x80..=0xEF => self.running = status,
            0xF1..=0xFE => return Err(Error::Invalid("system message in a segment")),
            _ => {
                // Running status: this byte is the first data byte.
                if self.running == 0 {
                    return Err(Error::Invalid("data byte without a running status"));
                }
                status = self.running;
                *p -= 1;
            }
        }
        let channel = status & 0x0F;
        let d1 = byte(p)? & 0x7F;
        Ok(match status & 0xF0 {
            0x80 => {
                byte(p)?;
                Event::NoteOff { channel, note: d1 }
            }
            0x90 => {
                let velocity = byte(p)? & 0x7F;
                if velocity == 0 {
                    Event::NoteOff { channel, note: d1 }
                } else {
                    Event::NoteOn { channel, note: d1, velocity }
                }
            }
            0xA0 => {
                byte(p)?;
                Event::KeyPressure { channel }
            }
            0xB0 => Event::Controller { channel, controller: d1, value: byte(p)? & 0x7F },
            0xC0 => Event::Program { channel, program: d1 },
            0xD0 => Event::NoteOff { channel, note: d1 },
            _ => {
                let d2 = byte(p)? & 0x7F;
                Event::PitchBend { channel, value: (((d2 as i32) << 7 | d1 as i32) - 8192) as i16 }
            }
        })
    }
}

impl Default for EventCursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// A test instrument: volume and key regions as (centre, low note, high note, sample offset).
    pub(crate) type TestInstrument = (i8, Vec<(i8, i8, i8, u32)>);

    /// Builds a music file with `instruments` (volume, regions as (centre, lo, hi, sample offset)) and segments of
    /// raw event bytes.
    pub(crate) fn build_music(instruments: &[TestInstrument], samples: &[u8], segments: &[Vec<u8>]) -> Vec<u8> {
        // Instrument section.
        let region_total: usize = instruments.iter().map(|i| i.1.len()).sum();
        let track_off = 0x28;
        let inst_table = track_off + 28;
        let region_table = inst_table + instruments.len() * 8;
        let mut s = vec![0u8; region_table + region_total * 24];
        s[0..4].copy_from_slice(b"SBv2");
        s[4..8].copy_from_slice(&2u32.to_le_bytes());
        s[8..12].copy_from_slice(&4u32.to_le_bytes());
        s[12..16].copy_from_slice(b"TEST");
        s[20..22].copy_from_slice(&1u16.to_le_bytes());
        s[22..24].copy_from_slice(&(instruments.len() as u16).to_le_bytes());
        s[28..32].copy_from_slice(&(track_off as u32).to_le_bytes());
        s[32..36].copy_from_slice(&(inst_table as u32).to_le_bytes());
        s[36..40].copy_from_slice(&(region_table as u32).to_le_bytes());
        s[track_off..track_off + 4].copy_from_slice(&5u32.to_le_bytes());
        s[track_off + 16] = 127;
        s[track_off + 19] = 1;
        let mut ri = 0;
        for (i, (vol, regions)) in instruments.iter().enumerate() {
            let o = inst_table + i * 8;
            s[o] = regions.len() as u8;
            s[o + 1] = *vol as u8;
            s[o + 4..o + 8].copy_from_slice(&((region_table + ri * 24) as u32).to_le_bytes());
            for &(centre, lo, hi, off) in regions {
                let r = region_table + ri * 24;
                s[r + 1] = 127;
                s[r + 2] = centre as u8;
                s[r + 6] = lo as u8;
                s[r + 7] = hi as u8;
                s[r + 10..r + 12].copy_from_slice(&0x80FFu16.to_le_bytes());
                s[r + 12..r + 14].copy_from_slice(&0x9FCBu16.to_le_bytes());
                s[r + 16..r + 20].copy_from_slice(&off.to_le_bytes());
                ri += 1;
            }
        }
        // Sequence section.
        let dir_head = 16 + segments.len() * 4;
        let mut seg_bytes = vec![];
        let mut offs = vec![];
        for (i, ev) in segments.iter().enumerate() {
            offs.push(dir_head + seg_bytes.len());
            let mut h = vec![0u8; 0xDC];
            h[0..4].copy_from_slice(b"MID ");
            h[24..28].copy_from_slice(&0xDCu32.to_le_bytes());
            h[32..36].copy_from_slice(&500_000u32.to_le_bytes());
            h[36..40].copy_from_slice(&480u32.to_le_bytes());
            h[40..44].copy_from_slice(&(0x0300_0000u32 | (i as u32) << 16).to_le_bytes());
            seg_bytes.extend_from_slice(&h);
            seg_bytes.extend_from_slice(ev);
        }
        let mut dir = vec![0u8; dir_head];
        dir[0..4].copy_from_slice(b"MMID");
        dir[4] = 1;
        dir[7] = segments.len() as u8;
        dir[8..12].copy_from_slice(b"TEST");
        for (i, o) in offs.iter().enumerate() {
            dir[16 + i * 4..20 + i * 4].copy_from_slice(&(*o as u32).to_le_bytes());
        }
        dir.extend_from_slice(&seg_bytes);
        let mut q = vec![0u8; 16];
        q[0..4].copy_from_slice(&2u32.to_le_bytes());
        q[4..8].copy_from_slice(&1u32.to_le_bytes());
        q[8..12].copy_from_slice(&16u32.to_le_bytes());
        q[12..16].copy_from_slice(&(dir.len() as u32).to_le_bytes());
        q.extend_from_slice(&dir);

        let inst_off = 0x20;
        let samp_off = inst_off + s.len();
        let seq_off = samp_off + samples.len();
        let mut f = vec![0u8; 0x20];
        f[0..4].copy_from_slice(&1u32.to_le_bytes());
        f[4..8].copy_from_slice(&3u32.to_le_bytes());
        for (i, (o, l)) in [(inst_off, s.len()), (samp_off, samples.len()), (seq_off, q.len())].iter().enumerate() {
            f[8 + i * 8..12 + i * 8].copy_from_slice(&(*o as u32).to_le_bytes());
            f[12 + i * 8..16 + i * 8].copy_from_slice(&(*l as u32).to_le_bytes());
        }
        f.extend_from_slice(&s);
        f.extend_from_slice(samples);
        f.extend_from_slice(&q);
        f
    }

    #[test]
    fn parses_synthetic_music() {
        let f = build_music(
            &[(90, vec![(-48, 36, 36, 0), (-60, 40, 50, 16)]), (100, vec![(60, 0, 127, 0)])],
            &[0u8; 32],
            &[vec![0x00, 0xFF, 0x2F, 0x00], vec![0x00, 0x90, 60, 100, 0x10, 0xD0, 60, 0x00, 0xFF, 0x2F, 0x00]],
        );
        let m = MusicFile::parse(f).unwrap();
        assert_eq!(&m.name, b"TEST");
        assert_eq!(m.track, TrackRecord { volume: 127, repeat: 0, group: 1, pan: 0 });
        assert_eq!(m.instruments.len(), 2);
        assert_eq!(m.regions.len(), 3);
        assert_eq!(m.instrument_regions(0)[1].low_note, 40);
        assert_eq!(m.instrument_regions(1)[0].centre, 60);
        assert_eq!(m.segments.len(), 2);
        assert_eq!(m.segments[1].ticks_per_quarter, 480);
        assert_eq!(m.segments[1].handle, 0x0301_0000);
        assert_eq!(m.segment_events(0), &[0x00, 0xFF, 0x2F, 0x00]);
        assert_eq!(m.sample_data().len(), 32);
    }

    #[test]
    fn rejects_regions_past_the_section_and_clamps_header() {
        let mut f = build_music(&[(90, vec![(-48, 36, 36, 0)])], &[0u8; 16], &[vec![0x00, 0xFF, 0x2F, 0x00]]);
        // Instrument 0 claims 255 key regions.
        let inst = 0x20 + 0x28 + 28;
        f[inst] = 255;
        assert!(MusicFile::parse(f.clone()).is_err());
        f[inst] = 1;
        // Segment 0's tempo 1 and ticks per quarter u32::MAX are clamped.
        let seq_off = u32::from_le_bytes(f[24..28].try_into().unwrap()) as usize;
        let seg = seq_off + 16 + 16 + 4;
        f[seg + 32..seg + 36].copy_from_slice(&1u32.to_le_bytes());
        f[seg + 36..seg + 40].copy_from_slice(&u32::MAX.to_le_bytes());
        let m = MusicFile::parse(f).unwrap();
        assert_eq!(m.segments[0].tempo, MIN_TEMPO);
        assert_eq!(m.segments[0].ticks_per_quarter, MAX_TICKS_PER_QUARTER);
        // The last segment runs to the end of the section.
        assert_eq!(m.segments[0].events.end, m.bytes.len());
    }

    #[test]
    fn events_with_running_status_and_note_off_as_pressure() {
        let data = [
            0x00, 0x90, 60, 100, // note on
            0x10, 62, 90, // running status note on
            0x81, 0x00, 0xD0, 60, // delta 128, note off by channel pressure
            0x00, 62, // running status 0xD0
            0x00, 0x91, 64, 0, // velocity 0
            0x00, 0xB0, 7, 100, 0x00, 0xC1, 5, 0x00, 0xE0, 0x00, 0x60, // controller, program, bend
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, 0x00, 0xF0, 0x75, 0x13, 0x03, 0x00, 0x11, 0x07, 0xF7, 0x00, 0xFF,
            0x2F, 0x00,
        ];
        let mut c = EventCursor::new();
        let mut got = vec![];
        while let Some(e) = c.next(&data).unwrap() {
            got.push(e);
        }
        assert_eq!(got[0], (0, Event::NoteOn { channel: 0, note: 60, velocity: 100 }));
        assert_eq!(got[1], (0x10, Event::NoteOn { channel: 0, note: 62, velocity: 90 }));
        assert_eq!(got[2], (128, Event::NoteOff { channel: 0, note: 60 }));
        assert_eq!(got[3], (0, Event::NoteOff { channel: 0, note: 62 }));
        assert_eq!(got[4], (0, Event::NoteOff { channel: 1, note: 64 }));
        assert_eq!(got[5].1, Event::Controller { channel: 0, controller: 7, value: 100 });
        assert_eq!(got[6].1, Event::Program { channel: 1, program: 5 });
        assert_eq!(got[7].1, Event::PitchBend { channel: 0, value: (0x60 << 7) - 8192 });
        assert_eq!(got[8].1, Event::Tempo(500_000));
        assert_eq!(got[9].1, Event::Adaptive(&[0x13, 0x03, 0x00, 0x11, 0x07]));
        assert_eq!(got[10].1, Event::EndOfSegment);
        assert_eq!(got.len(), 11);
    }

    #[test]
    fn adaptive_commands_with_definitions() {
        // A macro definition and a group definition contain their own F7.
        let data = [
            0x00, 0xF0, 0x75, 0x0B, 0x02, 0x13, 0x01, 0x05, 0xF7, 0x0F, 0x01, 0x00, 0x03, 0x10, 0x40, 0xF7, 0x10, 0x01,
            0xF7,
        ];
        let mut c = EventCursor::new();
        let (_, ev) = c.next(&data).unwrap().unwrap();
        let Event::Adaptive(list) = ev else { panic!("{ev:?}") };
        let cmds: Vec<_> = commands(list).collect::<Result<_, _>>().unwrap();
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0], Command::DefineMacro(2, &[0x13, 0x01, 0x05]));
        assert_eq!(cmds[1], Command::DefineGroup { group: 1, source: 0, entries: &[0x03, 0x10, 0x40] });
        assert_eq!(cmds[2], Command::ActivateGroup(1));
        assert!(commands(&[0x42]).next().unwrap().is_err());
        let all: Vec<_> =
            commands(&[0x00, 5, 0x04, 0x12, 3, 0x05, 0x16, 1, 2, 0x14, 9]).map(|c| c.unwrap().code()).collect();
        assert_eq!(all, [0x00, 0x04, 0x12, 0x05, 0x16, 0x14]);
    }
}
