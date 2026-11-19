//! The adaptive music sequencer (audio.md 5.3 and 5.4): segment players reading MIDI-like events, note on/off on the
//! instrument bank, the adaptive commands, segment transitions and channel groups gated by intensity or registers.
//!
//! Runs on the audio thread, one call of [`Sequencer::tick`] per scheduler tick (1/240 s). Event times are kept
//! exactly: each player accumulates `10^6 x ticks-per-quarter` per scheduler tick and spends `tempo x 240` per MIDI
//! tick, so no rounding error builds up. Nothing here allocates.

use crate::bank::VoiceParams;
use crate::envelope::Envelope;
use crate::fmath;
use crate::mixer::{Mixer, Owner, Pool, Source, VoiceStart};
use crate::music::{Command, Event, EventCursor, MusicFile, decode_command};

/// Most segments playing at once.
pub const MAX_PLAYERS: usize = 8;
/// Number of music registers.
pub const REGISTERS: usize = 128;
const MAX_GROUPS: usize = 32;
const MAX_MACROS: usize = 64;
const GROUP_ENTRIES: usize = 16;
/// Most events one player handles in one tick before it is stopped as broken.
const EVENTS_PER_TICK: usize = 4096;
/// Deepest macro nesting.
const MACRO_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug)]
struct Channel {
    program: u8,
    volume: u8,
    pan: u8,
    bend: i16,
}

const CHANNEL_DEFAULT: Channel = Channel { program: 0, volume: 127, pan: 64, bend: 0 };

#[derive(Clone, Copy, Debug)]
struct Player {
    active: bool,
    segment: u8,
    cursor: EventCursor,
    have_delta: bool,
    wait: u32,
    tempo: u32,
    tpq: u32,
    acc: u64,
    loops: u32,
    channels: [Channel; 16],
    group: Option<u8>,
}

const PLAYER_IDLE: Player = Player {
    active: false,
    segment: 0,
    cursor: EventCursor::new(),
    have_delta: false,
    wait: 0,
    tempo: 500_000,
    tpq: 480,
    acc: 0,
    loops: 0,
    channels: [CHANNEL_DEFAULT; 16],
    group: None,
};

#[derive(Clone, Copy, Debug)]
struct Group {
    defined: bool,
    source: u8,
    count: usize,
    entries: [[u8; 3]; GROUP_ENTRIES],
}

const GROUP_EMPTY: Group = Group { defined: false, source: 0, count: 0, entries: [[0; 3]; GROUP_ENTRIES] };

/// Pan in degrees for a controller-10 value: 64..127 maps to 0..+90, 0..63 to 270..359.
pub fn controller_pan(v: u8) -> f32 {
    let v = v.min(127) as f32;
    if v >= 64.0 { (v - 64.0) * 90.0 / 63.0 } else { 270.0 + v * 89.0 / 63.0 }
}

/// The sequencer state.
pub struct Sequencer {
    players: [Player; MAX_PLAYERS],
    /// Music registers (set by commands and by the game).
    pub registers: [u8; REGISTERS],
    macros: [Option<(u32, u32)>; MAX_MACROS],
    groups: [Group; MAX_GROUPS],
    /// The music intensity value, 0..127 (level + 20).
    pub intensity: u8,
    /// Scale non-negative-centre regions by 44,100 / 48,000 (the older tuning convention; unconfirmed).
    pub legacy_tuning: bool,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequencer {
    /// A sequencer with nothing playing.
    pub const fn new() -> Sequencer {
        Sequencer {
            players: [PLAYER_IDLE; MAX_PLAYERS],
            registers: [0; REGISTERS],
            macros: [None; MAX_MACROS],
            groups: [GROUP_EMPTY; MAX_GROUPS],
            intensity: 20,
            legacy_tuning: false,
        }
    }

    /// Forgets everything (registers, macros, groups, players). Voices are the caller's business.
    pub fn reset(&mut self) {
        let (intensity, legacy) = (self.intensity, self.legacy_tuning);
        *self = Sequencer::new();
        self.intensity = intensity;
        self.legacy_tuning = legacy;
    }

    /// Starts the music at `segment` (segment 0 when a world loads).
    pub fn start(&mut self, music: &MusicFile, segment: u8) {
        self.start_segment(music, segment, None);
    }

    /// Whether any segment is playing.
    pub fn playing(&self) -> bool {
        self.players.iter().any(|p| p.active)
    }

    /// Segments playing now, in player order.
    pub fn active_segments(&self) -> impl Iterator<Item = u8> + '_ {
        self.players.iter().filter(|p| p.active).map(|p| p.segment)
    }

    /// Stops every segment (voices are released by the caller).
    pub fn stop_all(&mut self) {
        for p in self.players.iter_mut() {
            p.active = false;
        }
    }

    fn init_player(&mut self, p: usize, music: &MusicFile, segment: u8) -> bool {
        let Some(seg) = music.segments.get(segment as usize) else {
            return false;
        };
        let pl = &mut self.players[p];
        pl.active = true;
        pl.segment = segment;
        pl.cursor = EventCursor::new();
        pl.have_delta = false;
        pl.wait = 0;
        pl.tempo = seg.tempo;
        pl.tpq = seg.ticks_per_quarter;
        pl.loops = 0;
        true
    }

    fn start_segment(&mut self, music: &MusicFile, segment: u8, from: Option<usize>) -> Option<usize> {
        let p = self.players.iter().position(|p| p.active && p.segment == segment).or_else(|| {
            let free = self.players.iter().position(|p| !p.active)?;
            self.players[free].channels = [CHANNEL_DEFAULT; 16];
            self.players[free].acc = 0;
            Some(free)
        })?;
        self.players[p].group = from.and_then(|f| self.players[f].group);
        self.init_player(p, music, segment).then_some(p)
    }

    /// Whether channel `ch` of player `p` may sound under its active channel group.
    pub fn channel_open(&self, p: usize, ch: u8) -> bool {
        let Some(g) = self.players[p].group else { return true };
        let group = &self.groups[g as usize % MAX_GROUPS];
        if !group.defined {
            return true;
        }
        let value =
            if group.source == 0 { self.intensity } else { self.registers[(group.source as usize - 1) % REGISTERS] };
        match group.entries[..group.count].iter().find(|e| e[0] == ch) {
            Some(e) => (e[1]..=e[2]).contains(&value),
            None => true,
        }
    }

    /// Advances one scheduler tick: runs every event that falls due.
    pub fn tick(&mut self, music: &MusicFile, mixer: &mut Mixer) {
        for p in 0..MAX_PLAYERS {
            if self.players[p].active {
                self.players[p].acc += 1_000_000 * self.players[p].tpq as u64;
                self.advance(p, music, mixer);
            }
        }
    }

    fn advance(&mut self, p: usize, music: &MusicFile, mixer: &mut Mixer) {
        let mut budget = EVENTS_PER_TICK;
        loop {
            let pl = &mut self.players[p];
            if !pl.active {
                return;
            }
            let data = music.segment_events(pl.segment as usize);
            if !pl.have_delta {
                match pl.cursor.read_delta(data) {
                    Ok(Some(d)) => {
                        pl.wait = d;
                        pl.have_delta = true;
                    }
                    // Running off the data counts as the end of the segment.
                    Ok(None) => {
                        if !self.end_of_segment(p, music, mixer, &mut budget) {
                            return;
                        }
                        continue;
                    }
                    Err(_) => {
                        self.stop_player(p, mixer);
                        return;
                    }
                }
                continue;
            }
            if pl.wait == 0 {
                pl.have_delta = false;
                let ev = match pl.cursor.read_event(data) {
                    Ok(ev) => ev,
                    Err(_) => {
                        self.stop_player(p, mixer);
                        return;
                    }
                };
                if budget == 0 {
                    self.stop_player(p, mixer);
                    return;
                }
                budget -= 1;
                self.event(p, ev, music, mixer, &mut budget);
                continue;
            }
            // Spend as many MIDI ticks as the accumulated time pays for, in one step.
            let cost = pl.tempo as u64 * 240;
            let take = (pl.acc / cost).min(pl.wait as u64);
            pl.acc -= take * cost;
            pl.wait -= take as u32;
            if pl.wait > 0 {
                return;
            }
        }
    }

    /// Handles the end of player `p`'s segment; returns whether it keeps playing.
    fn end_of_segment(&mut self, p: usize, music: &MusicFile, mixer: &mut Mixer, budget: &mut usize) -> bool {
        let pl = &mut self.players[p];
        pl.loops += 1;
        if music.track.repeat != 0 && pl.loops >= music.track.repeat as u32 {
            self.stop_player(p, mixer);
            return false;
        }
        // An empty segment would loop forever within one tick.
        if *budget == 0 {
            self.stop_player(p, mixer);
            return false;
        }
        *budget -= 1;
        pl.cursor = EventCursor::new();
        pl.have_delta = false;
        true
    }

    fn release_player_notes(p: usize, mixer: &mut Mixer) {
        mixer.release_where(|v| matches!(v.p.owner, Owner::Music { player, .. } if player as usize == p));
    }

    fn stop_player(&mut self, p: usize, mixer: &mut Mixer) {
        self.players[p].active = false;
        Self::release_player_notes(p, mixer);
    }

    fn event(&mut self, p: usize, ev: Event<'_>, music: &MusicFile, mixer: &mut Mixer, budget: &mut usize) {
        match ev {
            Event::NoteOn { channel, note, velocity } => self.note_on(p, channel, note, velocity, music, mixer),
            Event::NoteOff { channel, note } => {
                let owner = Owner::Music { player: p as u8, channel, note };
                if let Some(v) = mixer.voices.iter_mut().find(|v| v.active && !v.released && v.p.owner == owner) {
                    v.release();
                }
            }
            Event::Controller { channel, controller, value } => {
                let ch = &mut self.players[p].channels[channel as usize & 15];
                match controller {
                    7 => ch.volume = value,
                    10 => ch.pan = value,
                    _ => {}
                }
            }
            Event::Program { channel, program } => self.players[p].channels[channel as usize & 15].program = program,
            Event::PitchBend { channel, value } => self.players[p].channels[channel as usize & 15].bend = value,
            Event::Tempo(t) => self.players[p].tempo = crate::music::clamp_tempo(t),
            Event::EndOfSegment => {
                self.end_of_segment(p, music, mixer, budget);
            }
            Event::Adaptive(list) => self.run_commands(list, p, music, mixer, 0, budget),
            Event::KeyPressure { .. } | Event::Meta { .. } | Event::SysEx(_) => {}
        }
    }

    fn note_on(&mut self, p: usize, channel: u8, note: u8, velocity: u8, music: &MusicFile, mixer: &mut Mixer) {
        if !self.channel_open(p, channel) {
            return;
        }
        let ch = self.players[p].channels[channel as usize & 15];
        let Some(inst) = music.instruments.get(ch.program as usize) else { return };
        let track = &music.track;
        let base =
            track.volume.clamp(0, 127) as f32 / 127.0 * inst.volume.max(0) as f32 / 127.0 * velocity as f32 / 127.0;
        for r in &music.regions[inst.regions.clone()] {
            if r.flags & crate::music::REGION_NO_SAMPLE != 0 || !(r.low_note..=r.high_note).contains(&(note as i8)) {
                continue;
            }
            let Some(vi) = mixer.alloc(Pool::Music, r.priority) else { continue };
            mixer.start(vi, self.region_voice(r, p, channel, note, base, track.pan as f32 + inst.pan as f32));
        }
    }

    fn region_voice(&self, r: &VoiceParams, p: usize, channel: u8, note: u8, base: f32, pan: f32) -> VoiceStart {
        VoiceStart {
            source: Source::Music,
            offset: r.sample_offset as usize,
            envelope: Envelope::new(r.adsr),
            owner: Owner::Music { player: p as u8, channel, note },
            priority: r.priority,
            group: 1,
            reverb: false,
            volume: base * (r.volume as i32).unsigned_abs().min(127) as f32 / 127.0,
            pan: pan + r.pan as f32,
            semitones: r.semitones_for(note as i32),
            rate_scale: if self.legacy_tuning && r.centre >= 0 { 44_100.0 / 48_000.0 } else { 1.0 },
            bend_down: r.bend_range_down(),
            bend_up: r.bend_range_up(),
            wrap: false,
        }
    }

    fn test(&self, cmd: &Command<'_>) -> bool {
        let reg = |r: u8| self.registers[r as usize % REGISTERS];
        match *cmd {
            Command::IfIntensityAbove(x) => self.intensity > x,
            Command::IfIntensityIs(x) => self.intensity as u16 == x as u16 + 1,
            Command::IfIntensityBelow(x) => self.intensity < x,
            Command::IfRegisterBelow(r, x) => reg(r) < x,
            Command::IfRegisterAbove(r, x) => reg(r) > x,
            Command::IfRegisterIs(r, x) => reg(r) == x,
            _ => true,
        }
    }

    /// Runs a command list for player `p`. `list` is a slice of `music.bytes` (an event payload or a macro body).
    ///
    /// Every command decoded, skipped or run, inside macros too, is charged to `budget`; when it runs out the player
    /// stops (the data loops without end).
    fn run_commands(
        &mut self,
        list: &[u8],
        p: usize,
        music: &MusicFile,
        mixer: &mut Mixer,
        depth: usize,
        budget: &mut usize,
    ) {
        let mut pos = 0;
        let mut skip = false;
        while pos < list.len() {
            if *budget == 0 {
                self.stop_player(p, mixer);
                return;
            }
            *budget -= 1;
            let Ok((cmd, next)) = decode_command(list, pos) else { return };
            pos = next;
            if skip {
                skip = false;
                if cmd == Command::Begin {
                    // A skipped block: jump past its matching 05.
                    let mut nest = 0;
                    while pos < list.len() {
                        if *budget == 0 {
                            self.stop_player(p, mixer);
                            return;
                        }
                        *budget -= 1;
                        let Ok((c, n)) = decode_command(list, pos) else { return };
                        pos = n;
                        match c {
                            Command::Begin => nest += 1,
                            Command::End if nest == 0 => break,
                            Command::End => nest -= 1,
                            _ => {}
                        }
                    }
                }
                continue;
            }
            if cmd.is_test() {
                skip = !self.test(&cmd);
                continue;
            }
            match cmd {
                Command::Begin | Command::End => {}
                Command::StopSegment(s) => {
                    for q in 0..MAX_PLAYERS {
                        if self.players[q].active && self.players[q].segment == s {
                            self.stop_player(q, mixer);
                        }
                    }
                }
                Command::DefineMacro(m, body) => {
                    let base = music.bytes.as_ptr() as usize;
                    let off = (body.as_ptr() as usize).wrapping_sub(base);
                    if off + body.len() <= music.bytes.len() {
                        self.macros[m as usize % MAX_MACROS] = Some((off as u32, body.len() as u32));
                    }
                }
                Command::RunMacro(m) => {
                    if depth < MACRO_DEPTH
                        && let Some((off, len)) = self.macros[m as usize % MAX_MACROS]
                        && let Some(body) = music.bytes.get(off as usize..(off + len) as usize)
                    {
                        self.run_commands(body, p, music, mixer, depth + 1, budget);
                    }
                }
                Command::JumpToRegister(r) => {
                    if let Some(s) = self.registers[r as usize % REGISTERS].checked_sub(1) {
                        self.jump(p, s, music, mixer);
                    }
                }
                Command::StartFromRegister(r) => {
                    if let Some(s) = self.registers[r as usize % REGISTERS].checked_sub(1) {
                        self.start_segment(music, s, Some(p));
                    }
                }
                Command::DefineGroup { group, source, entries } => {
                    let g = &mut self.groups[group as usize % MAX_GROUPS];
                    g.defined = true;
                    g.source = source;
                    g.count = 0;
                    for e in entries.as_chunks::<3>().0.iter().take(GROUP_ENTRIES) {
                        g.entries[g.count] = [e[0], e[1], e[2]];
                        g.count += 1;
                    }
                }
                Command::ActivateGroup(g) => self.players[p].group = Some(g),
                Command::Jump(s) => self.jump(p, s, music, mixer),
                Command::Start(s) => {
                    self.start_segment(music, s, Some(p));
                }
                Command::SetRegister(r, x) => self.registers[r as usize % REGISTERS] = x.min(127),
                Command::IncrementRegister(r) => {
                    let v = &mut self.registers[r as usize % REGISTERS];
                    *v = v.saturating_add(1).min(127);
                }
                Command::DecrementRegister(r) => {
                    let v = &mut self.registers[r as usize % REGISTERS];
                    *v = v.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    /// Player `p` switches to segment `s` (its notes are released; channel settings carry over).
    fn jump(&mut self, p: usize, s: u8, music: &MusicFile, mixer: &mut Mixer) {
        if s as usize >= music.segments.len() {
            return;
        }
        Self::release_player_notes(p, mixer);
        // If another player already plays `s`, it stops: one player per segment.
        for q in 0..MAX_PLAYERS {
            if q != p && self.players[q].active && self.players[q].segment == s {
                self.stop_player(q, mixer);
            }
        }
        self.init_player(p, music, s);
    }

    /// Sets the targets of every music voice for the coming tick. `master` is the music group's amplitude scale;
    /// voices on a channel that its group has closed are released.
    pub fn update_voices(&self, mixer: &mut Mixer, master: f32) {
        for i in mixer.pool(Pool::Music) {
            let v = mixer.voices[i];
            if !v.active {
                continue;
            }
            let Owner::Music { player, channel, .. } = v.p.owner else { continue };
            let p = player as usize % MAX_PLAYERS;
            let ch = self.players[p].channels[channel as usize & 15];
            if !v.released && self.players[p].active && !self.channel_open(p, channel) {
                mixer.voices[i].release();
            }
            let lin = v.p.volume * ch.volume as f32 / 127.0;
            let amp = lin * lin * master;
            let pan = v.p.pan + controller_pan(ch.pan);
            let semis = v.p.semitones + v.bend_semitones(ch.bend as f32 / 8192.0);
            let ratio = fmath::semitone_ratio(semis as f64) * v.p.rate_scale as f64;
            mixer.set_target(i, amp, pan, ratio, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adpcm::tests::block;
    use crate::adpcm::{FLAG_END, FLAG_REPEAT};
    use crate::music::tests::build_music;
    use alloc::vec;
    use alloc::vec::Vec;

    fn music(segments: &[Vec<u8>]) -> MusicFile {
        let mut samples = vec![];
        samples.extend_from_slice(&block(12, 0, FLAG_END | FLAG_REPEAT, &[1; 28]));
        MusicFile::parse(build_music(&[(127, vec![(-60, 0, 127, 0)])], &samples, segments)).unwrap()
    }

    fn sounding(m: &Mixer) -> Vec<u8> {
        m.voices
            .iter()
            .filter(|v| v.active && !v.released)
            .filter_map(|v| if let Owner::Music { note, .. } = v.p.owner { Some(note) } else { None })
            .collect()
    }

    #[test]
    fn timing_is_exact() {
        // 500,000 us per quarter, 480 ticks: 1 tick = 1041.67 us; 240 Hz scheduler = 4 MIDI ticks per tick.
        // Note on at tick 0, note off after 480 MIDI ticks = 0.5 s = 120 scheduler ticks.
        let ev = vec![0x00, 0x90, 60, 100, 0x83, 0x60, 0xD0, 60, 0x83, 0x60, 0xFF, 0x2F, 0x00];
        let m = music(&[ev]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        let mut off_at = None;
        for t in 0..130 {
            seq.tick(&m, &mut mixer);
            if t == 0 {
                assert_eq!(sounding(&mixer), [60]);
            }
            if off_at.is_none() && sounding(&mixer).is_empty() {
                off_at = Some(t);
            }
        }
        assert_eq!(off_at, Some(119));
        // The segment loops: the note comes back after the end-of-segment event, 240 ticks in.
        for t in 130..245 {
            seq.tick(&m, &mut mixer);
            assert_eq!(sounding(&mixer).is_empty(), t < 239, "tick {t}");
        }
    }

    #[test]
    fn adaptive_jump_registers_and_tests() {
        // Segment 0: set register 3 = 0, jump to 2, set register 1 = 5 (the Paris start pattern).
        let s0 = vec![0x00, 0xF0, 0x75, 0x13, 0x03, 0x00, 0x11, 0x02, 0x13, 0x01, 0x05, 0xF7, 0x00, 0xFF, 0x2F, 0x00];
        let s1 = vec![0x00, 0x90, 40, 100, 0x60, 0xFF, 0x2F, 0x00];
        // Segment 2: if intensity > 100 start segment 1, else note 50; then if register 1 = 5 note 51.
        let s2 = vec![
            0x00, 0xF0, 0x75, 0x00, 100, 0x12, 0x01, 0x04, 0x05, 0x16, 0x01, 0x05, 0x13, 0x07, 0x09, 0xF7, //
            0x00, 0x90, 50, 100, 0x60, 0xFF, 0x2F, 0x00,
        ];
        let m = music(&[s0, s1, s2]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.intensity = 20;
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert_eq!(seq.registers[3], 0);
        assert_eq!(seq.registers[1], 5);
        assert_eq!(seq.registers[7], 9, "register test passed");
        assert_eq!(seq.active_segments().collect::<Vec<_>>(), [2]);
        assert_eq!(sounding(&mixer), [50]);

        // High intensity: segment 1 starts in parallel.
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.intensity = 127;
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        let mut segs: Vec<u8> = seq.active_segments().collect();
        segs.sort();
        assert_eq!(segs, [1, 2]);
        let mut notes = sounding(&mixer);
        notes.sort();
        assert_eq!(notes, [40, 50]);
    }

    #[test]
    fn blocks_run_as_the_body_of_a_test() {
        // if intensity < 10 { r0 = 1; r1 = 3 } ; r2 = 4 ; if r2 = 4 { if intensity > 40 { r3 = 9 } }
        let s0 = vec![
            0x00, 0xF0, 0x75, 0x02, 10, 0x04, 0x13, 0, 1, 0x13, 1, 3, 0x05, 0x13, 2, 4, 0x16, 2, 4, 0x04, 0x00, 40,
            0x13, 3, 9, 0x05, 0xF7, 0x00, 0xFF, 0x2F, 0x00,
        ];
        let m = music(&[s0]);
        for (intensity, expect) in [(5u8, [1u8, 3, 4, 0]), (50, [0, 0, 4, 9])] {
            let mut mixer = Mixer::new(0, 8, 0);
            let mut seq = Sequencer::new();
            seq.intensity = intensity;
            seq.start(&m, 0);
            seq.tick(&m, &mut mixer);
            assert_eq!(&seq.registers[..4], &expect, "intensity {intensity}");
        }
    }

    #[test]
    fn macros_and_channel_groups() {
        // Define macro 1 = "set r5 = 7"; define group 0 on intensity: channel 1 plays at 60..127; run macro 1;
        // activate group 0; notes on channels 0 and 1.
        let s0 = vec![
            0x00, 0xF0, 0x75, 0x0B, 0x01, 0x13, 0x05, 0x07, 0xF7, 0x0F, 0x00, 0x00, 0x01, 60, 127, 0xF7, 0x0C, 0x01,
            0x10, 0x00, 0xF7, //
            0x00, 0x90, 30, 100, 0x00, 0x91, 31, 100, 0x60, 0xFF, 0x2F, 0x00,
        ];
        let m = music(&[s0]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.intensity = 20;
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert_eq!(seq.registers[5], 7);
        assert_eq!(sounding(&mixer), [30], "channel 1 is closed at intensity 20");
        assert!(seq.channel_open(0, 0) && !seq.channel_open(0, 1));
        seq.intensity = 100;
        assert!(seq.channel_open(0, 1));
        // Closing the layer again releases its voices.
        let mut seq2 = Sequencer::new();
        seq2.intensity = 100;
        let mut mixer = Mixer::new(0, 8, 0);
        seq2.start(&m, 0);
        seq2.tick(&m, &mut mixer);
        let mut n = sounding(&mixer);
        n.sort();
        assert_eq!(n, [30, 31]);
        seq2.intensity = 0;
        seq2.update_voices(&mut mixer, 1.0);
        assert_eq!(sounding(&mixer), [30]);
    }

    #[test]
    fn huge_waits_and_tempos_are_cheap() {
        // A note-on after a 0x0FFFFFFF-tick delta: the wait is spent with one division per tick, not tick by tick.
        let ev = vec![0xFF, 0xFF, 0xFF, 0x7F, 0x90, 60, 100, 0x00, 0xFF, 0x2F, 0x00];
        let m = music(&[ev]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        for _ in 0..10_000 {
            seq.tick(&m, &mut mixer);
        }
        assert!(seq.playing());
        assert!(sounding(&mixer).is_empty());
        // Tempo and ticks per quarter are clamped when parsed, and tempo events too.
        assert_eq!(crate::music::clamp_tempo(1), crate::music::MIN_TEMPO);
        let fast = vec![0x00, 0xFF, 0x51, 0x03, 0x00, 0x00, 0x01, 0x83, 0x60, 0x90, 60, 100, 0x00, 0xFF, 0x2F, 0x00];
        let m = music(&[fast]);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert_eq!(seq.players[0].tempo, crate::music::MIN_TEMPO);
    }

    #[test]
    fn command_work_is_bounded() {
        // 5000 increments in one message: charged to the event budget, so the player stops instead of running on.
        let mut ev = vec![0x00, 0xF0, 0x75];
        for _ in 0..5000 {
            ev.extend_from_slice(&[0x14, 0x00]);
        }
        ev.extend_from_slice(&[0xF7, 0x60, 0xFF, 0x2F, 0x00]);
        let m = music(&[ev]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert!(!seq.playing());
        assert_eq!(seq.registers[0], 127, "increments saturate at 127");
        // A macro that calls itself twice: depth-limited and charged per command.
        let ev = vec![
            0x00, 0xF0, 0x75, 0x0B, 0x01, 0x0C, 0x01, 0x0C, 0x01, 0x14, 0x02, 0xF7, 0x0C, 0x01, 0x13, 0x03, 0xC8, 0xF7,
        ];
        let mut ev = ev;
        ev.extend_from_slice(&[0x60, 0xFF, 0x2F, 0x00]);
        let m = music(&[ev]);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert!(seq.registers[2] > 0);
        assert_eq!(seq.registers[3], 127, "SetRegister clamps to 127");
    }

    #[test]
    fn controller_pan_mapping() {
        assert_eq!(controller_pan(64), 0.0);
        assert_eq!(controller_pan(127), 90.0);
        assert_eq!(controller_pan(0), 270.0);
        assert!((controller_pan(63) - 359.0).abs() < 1e-4);
    }

    #[test]
    fn empty_segment_does_not_hang() {
        let m = music(&[vec![0x00, 0xFF, 0x2F, 0x00]]);
        let mut mixer = Mixer::new(0, 8, 0);
        let mut seq = Sequencer::new();
        seq.start(&m, 0);
        seq.tick(&m, &mut mixer);
        assert!(!seq.playing());
    }
}
