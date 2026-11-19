//! A save slot in the container of [`crate::codec`].
//!
//! Format version 1 sections:
//!
//! | Tag | Content |
//! |---|---|
//! | 1, slot | f64 play time; u8 episode, u8 level, i8 lives, u8 charms, u8 coins; u32 settings, u32 reward mask, u32 unlocked movies; u8 milestones; u8 last power-up (0xFF none) |
//! | 2, episode (one per episode) | u8 index, u8 keys, u8 vaults, u8 sprints, u8 flags, f32 time, u8 level count, then that many level records, each u16 length + (u8 flags, f32 time, f32 best sprint, f32 difficulty, f32 raw difficulty, u16 dialogue flags, 4 x (i32 key, i32 value), u64 bottles, u8 checkpoints) |
//!
//! Every loaded slot is clamped to the ranges of save-format.md 1.

use crate::codec::{self, DecodeError, GAME_SLY1, Reader, SLOT_MAGIC, Writer};
use crate::model::{EPISODES, EpisodeRecord, LEVELS_PER_EPISODE, LevelRecord, Settings, SlotState};
use alloc::vec::Vec;

/// The format this build writes and the newest it reads fully.
pub const FORMAT_VERSION: u16 = 1;
/// The oldest reader that can read what this build writes.
pub const OLDEST_READER: u16 = 1;

const TAG_SLOT: u16 = 1;
const TAG_EPISODE: u16 = 2;
const NO_POWER_UP: u8 = 0xFF;

fn write_level(w: &mut Writer, l: &LevelRecord) {
    w.record(|w| {
        w.u8(l.flags);
        w.f32(l.time);
        w.f32(l.best_sprint);
        w.f32(l.difficulty);
        w.f32(l.difficulty_raw);
        w.u16(l.dialogue_played);
        for (k, v) in l.script_values {
            w.i32(k);
            w.i32(v);
        }
        w.u64(l.bottles);
        w.u8(l.checkpoints);
    });
}

/// Encodes a slot.
pub fn encode(state: &SlotState) -> Vec<u8> {
    let mut w = Writer::default();
    w.section(TAG_SLOT, |w| {
        w.f64(state.play_time);
        w.u8(state.episode);
        w.u8(state.level);
        w.i8(state.lives);
        w.u8(state.charms);
        w.u8(state.coins);
        w.u32(state.settings.0);
        w.u32(state.reward_mask);
        w.u32(state.unlocked_movies);
        w.u8(state.milestones);
        w.u8(state.last_power_up.unwrap_or(NO_POWER_UP));
    });
    for (i, e) in state.episodes.iter().enumerate() {
        w.section(TAG_EPISODE, |w| {
            w.u8(i as u8);
            w.u8(e.keys);
            w.u8(e.vaults);
            w.u8(e.sprints);
            w.u8(e.flags);
            w.f32(e.time);
            w.u8(LEVELS_PER_EPISODE as u8);
            for l in &e.levels {
                write_level(w, l);
            }
        });
    }
    codec::seal(SLOT_MAGIC, FORMAT_VERSION, OLDEST_READER, GAME_SLY1, &w.buf)
}

/// A decoded slot and what had to be done to read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub state: SlotState,
    /// The file's format version (lower than [`FORMAT_VERSION`]: upgraded; higher: read by the forward-compatible
    /// rules).
    pub version: u16,
}

fn read_level(rec: &[u8]) -> LevelRecord {
    let mut l = LevelRecord::default();
    // Fields in order; reading stops at the record's end, leaving the rest at their defaults.
    let _ = read_level_fields(&mut Reader::new(rec), &mut l);
    l
}

fn read_level_fields(r: &mut Reader<'_>, l: &mut LevelRecord) -> Option<()> {
    l.flags = r.u8()?;
    l.time = r.f32()?;
    l.best_sprint = r.f32()?;
    l.difficulty = r.f32()?;
    l.difficulty_raw = r.f32()?;
    l.dialogue_played = r.u16()?;
    for pair in &mut l.script_values {
        *pair = (r.i32()?, r.i32()?);
    }
    l.bottles = r.u64()?;
    l.checkpoints = r.u8()?;
    Some(())
}

fn read_episode(body: &[u8], state: &mut SlotState) {
    let mut r = Reader::new(body);
    let Some(index) = r.u8().map(usize::from).filter(|&i| i < EPISODES) else { return };
    let mut e = EpisodeRecord::default();
    let _ = read_episode_fields(&mut r, &mut e);
    state.episodes[index] = e;
}

fn read_episode_fields(r: &mut Reader<'_>, e: &mut EpisodeRecord) -> Option<()> {
    e.keys = r.u8()?;
    e.vaults = r.u8()?;
    e.sprints = r.u8()?;
    e.flags = r.u8()?;
    e.time = r.f32()?;
    let n = r.u8()? as usize;
    let mut slots = e.levels.iter_mut();
    for _ in 0..n {
        let rec = r.record()?;
        // Records past the ninth (a future format with more levels) are skipped.
        if let Some(level) = slots.next() {
            *level = read_level(rec);
        }
    }
    Some(())
}

fn read_slot(body: &[u8], state: &mut SlotState) {
    let _ = read_slot_fields(&mut Reader::new(body), state);
}

fn read_slot_fields(r: &mut Reader<'_>, state: &mut SlotState) -> Option<()> {
    state.play_time = r.f64()?;
    state.episode = r.u8()?;
    state.level = r.u8()?;
    state.lives = r.i8()?;
    state.charms = r.u8()?;
    state.coins = r.u8()?;
    state.settings = Settings(r.u32()?);
    state.reward_mask = r.u32()?;
    state.unlocked_movies = r.u32()?;
    state.milestones = r.u8()?;
    state.last_power_up = Some(r.u8()?).filter(|&p| p != NO_POWER_UP);
    Some(())
}

/// Upgrades a state read from an older format. Each format change adds a step here; version 1 is the first.
fn upgrade(_from: u16, _state: &mut SlotState) {}

/// Decodes and clamps a slot file.
pub fn decode(data: &[u8]) -> Result<Decoded, DecodeError> {
    let opened = codec::open(SLOT_MAGIC, FORMAT_VERSION, GAME_SLY1, data)?;
    let mut state = SlotState::new_game();
    let mut have_slot = false;
    for section in codec::sections(opened.payload) {
        let (tag, body) = section?;
        match tag {
            TAG_SLOT => {
                read_slot(body, &mut state);
                have_slot = true;
            }
            TAG_EPISODE => read_episode(body, &mut state),
            _ => {} // a newer writer's section
        }
    }
    if !have_slot {
        return Err(DecodeError::Truncated);
    }
    if opened.version < FORMAT_VERSION {
        upgrade(opened.version, &mut state);
    }
    state.clamp();
    Ok(Decoded { state, version: opened.version })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{level_flags, milestones};

    /// A slot with something in every field.
    pub(crate) fn busy_state() -> SlotState {
        let mut s = SlotState::new_game();
        s.play_time = 12_345.678;
        s.visit(3, 4);
        s.lives = 17;
        s.charms = 2;
        s.coins = 42;
        s.settings = Settings(Settings::AUTOSAVE | Settings::MONO | Settings::JETPACK_INVERTED | (1 << 30));
        s.reward_mask = 0x8000_0005;
        s.unlocked_movies = 0b1011;
        s.milestones = milestones::FIRST_BOTTLE | milestones::FIRST_VAULT;
        s.last_power_up = Some(7);
        for (ei, e) in s.episodes.iter_mut().enumerate() {
            e.keys = 1;
            e.vaults = 1;
            e.sprints = 1;
            e.flags = 3;
            e.time = 100.0 * ei as f32 + 0.25;
            for (li, l) in e.levels.iter_mut().enumerate() {
                let k = (ei * 9 + li) as u32;
                l.flags = level_flags::ALL;
                l.time = k as f32 * 1.5;
                l.best_sprint = 30.0 + k as f32;
                l.difficulty = 0.5;
                l.difficulty_raw = 0.125;
                l.dialogue_played = (k as u16 * 37) & 0xFFF;
                l.script_values = [(k as i32, -1), (2, i32::MAX), (i32::MIN, 3), (4, 5)];
                l.bottles = 0x0123_4567_89AB_CDEF ^ k as u64;
                l.checkpoints = k as u8;
            }
        }
        s
    }

    #[test]
    fn round_trip() {
        for s in [SlotState::new_game(), busy_state()] {
            let d = decode(&encode(&s)).unwrap();
            assert_eq!(d.state, s);
            assert_eq!(d.version, FORMAT_VERSION);
        }
    }

    #[test]
    fn out_of_range_values_are_clamped_on_load() {
        let mut s = busy_state();
        s.lives = 120;
        s.coins = 200;
        s.episodes[1].levels[2].difficulty = 9.0;
        let d = decode(&encode(&s)).unwrap();
        assert_eq!((d.state.lives, d.state.coins, d.state.episodes[1].levels[2].difficulty), (99, 99, 1.0));
    }

    #[test]
    fn corruption_is_detected() {
        let file = encode(&busy_state());
        for at in [0, 9, 13, 17, 21, 30, file.len() / 2, file.len() - 1] {
            let mut f = file.clone();
            f[at] ^= 0x10;
            assert!(decode(&f).is_err(), "byte {at}");
        }
        assert!(decode(&file[..file.len() - 3]).is_err());
        assert!(decode(b"").is_err());
    }

    /// Re-seals a payload with another header, as another SlyNX version would write it.
    fn reseal(payload: &[u8], version: u16, oldest: u16) -> Vec<u8> {
        codec::seal(SLOT_MAGIC, version, oldest, GAME_SLY1, payload)
    }

    fn payload_of(file: &[u8]) -> Vec<u8> {
        file[codec::HEADER_SIZE..].to_vec()
    }

    #[test]
    fn newer_writer_extra_fields_and_sections_are_ignored() {
        let s = busy_state();
        // A future writer: a new section, extra bytes after the known slot fields and after every level record.
        let mut w = Writer::default();
        w.section(TAG_SLOT, |w| {
            let known = &payload_of(&encode(&s))[6..];
            let slot_len = 8 + 5 + 12 + 2;
            w.buf.extend_from_slice(&known[..slot_len]);
            w.u32(0xDEAD_BEEF); // a field we do not know
        });
        for (i, e) in s.episodes.iter().enumerate() {
            w.section(TAG_EPISODE, |w| {
                w.u8(i as u8);
                w.u8(e.keys);
                w.u8(e.vaults);
                w.u8(e.sprints);
                w.u8(e.flags);
                w.f32(e.time);
                w.u8(LEVELS_PER_EPISODE as u8);
                for l in &e.levels {
                    w.record(|w| {
                        let mut tmp = Writer::default();
                        write_level(&mut tmp, l);
                        w.buf.extend_from_slice(&tmp.buf[2..]);
                        w.u64(77); // new per-level field
                    });
                }
                w.f64(1.0); // new per-episode field
            });
        }
        w.section(999, |w| w.u32(1)); // a new section
        let d = decode(&reseal(&w.buf, 4, 1)).unwrap();
        assert_eq!(d.version, 4);
        assert_eq!(d.state, s);

        // A future writer that says old readers must not read it.
        assert_eq!(decode(&reseal(&w.buf, 4, 2)).err(), Some(DecodeError::TooNew { version: 4, needs_reader: 2 }));
    }

    #[test]
    fn older_writer_missing_fields_get_defaults() {
        // An older layout: level records without the last fields (bottles, checkpoints) and a slot section without
        // the last power-up. Loads, fills defaults, and saves back in the current format.
        let s = busy_state();
        let mut w = Writer::default();
        w.section(TAG_SLOT, |w| {
            let full = &payload_of(&encode(&s))[6..];
            w.buf.extend_from_slice(&full[..8 + 5 + 12 + 1]); // up to and including milestones
        });
        for (i, e) in s.episodes.iter().enumerate() {
            w.section(TAG_EPISODE, |w| {
                w.u8(i as u8);
                w.u8(e.keys);
                w.u8(e.vaults);
                w.u8(e.sprints);
                w.u8(e.flags);
                w.f32(e.time);
                w.u8(LEVELS_PER_EPISODE as u8);
                for l in &e.levels {
                    w.record(|w| {
                        let mut tmp = Writer::default();
                        write_level(&mut tmp, l);
                        let full = &tmp.buf[2..];
                        w.buf.extend_from_slice(&full[..full.len() - 9]); // no bottles, no checkpoints
                    });
                }
            });
        }
        let d = decode(&reseal(&w.buf, 0, 0)).unwrap();
        assert_eq!(d.version, 0);
        assert_eq!(d.state.last_power_up, None, "missing field: new-game default");
        assert_eq!(d.state.episodes[2].levels[3].bottles, 0);
        assert_eq!(d.state.episodes[2].levels[3].checkpoints, 0);
        assert_eq!(d.state.episodes[2].levels[3].best_sprint, s.episodes[2].levels[3].best_sprint);
        assert_eq!(d.state.coins, s.coins);
        // Saving writes the current format.
        let again = decode(&encode(&d.state)).unwrap();
        assert_eq!(again.version, FORMAT_VERSION);
        assert_eq!(again.state, d.state);
    }

    #[test]
    fn a_file_without_the_slot_section_is_rejected() {
        let mut w = Writer::default();
        w.section(TAG_EPISODE, |w| w.u8(0));
        assert_eq!(decode(&reseal(&w.buf, 1, 1)).err(), Some(DecodeError::Truncated));
    }
}
