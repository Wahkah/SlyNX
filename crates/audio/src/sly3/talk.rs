//! Sly 3 dialogue (specs/sly3/dialogue-and-voice.md, scripts.md 7.2): the dialogue commands of command scripts
//! (class 135), played with Sly 2's line player ([`crate::sly2::talk::Talk`]).
//!
//! - `0x52` plays a line ([`DialogueCommand::PlayLine`], dialogue-and-voice.md 1). Which entry of its line list plays
//!   is [`LineOrder`] (1.1): rotate by one per run, reshuffle after a full round, never the same line twice in a
//!   row. Unless quiet, the subtitle is the world string whose ID is the line number (1.2), coloured by the
//!   speaker's class ([`speaker_colour`], [`subtitle`]). A source object makes the line positional, with the
//!   command's fall-off ([`line_emitter`], 1.3).
//! - `0x53` stops the playing line when it is in the command's list, or whatever plays for an empty list
//!   ([`stops`], 2). `0x54` clears the subtitle (3). `0x5F` waits until no line plays (5).
//! - `0x55` / `0x56` hold and release the runner's target character's talk effects: at most 4 characters per runner,
//!   each once, released when the runner ends ([`TalkHolds`], 4).

use alloc::vec::Vec;

use crate::rng::Rng;
use crate::sly2::talk::{Subtitle, Talk};
use crate::spatial::Emitter;

/// Command `0x52`: play a dialogue line.
pub const PLAY_LINE: u8 = 0x52;
/// Command `0x53`: stop the dialogue.
pub const STOP_DIALOGUE: u8 = 0x53;
/// Command `0x54`: clear the subtitle.
pub const CLEAR_SUBTITLE: u8 = 0x54;
/// Command `0x55`: hold a character's talk effects.
pub const HOLD_EFFECTS: u8 = 0x55;
/// Command `0x56`: release them.
pub const RELEASE_EFFECTS: u8 = 0x56;
/// Command `0x5F`: wait until no dialogue line is playing.
pub const WAIT_DIALOGUE: u8 = 0x5F;
/// Command scripts.
pub const COMMAND_SCRIPT_CLASS: i16 = 135;
/// Talk-effect holds per runner (dialogue-and-voice.md 4).
pub const MAX_HOLDS: usize = 4;

/// A dialogue command of a command script.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogueCommand {
    /// `0x52`: the line list, the quiet flag (no subtitle), the source object (−1: none), the fall-off start and
    /// end, the volume.
    PlayLine { lines: Vec<i32>, quiet: bool, source: i32, falloff: (i32, i32), volume: f32 },
    /// `0x53`: stop the playing line if it is listed (any line for an empty list).
    Stop { list: Vec<i32> },
    /// `0x54`.
    ClearSubtitle,
    /// `0x5F`.
    Wait,
    /// `0x55` (`hold` true) and `0x56`: the effect kind (`s16`) and flag (`s8`).
    TalkEffects { hold: bool, kind: i16, flag: i8 },
}

impl DialogueCommand {
    /// Decodes command `id` with its argument bytes; `None` for another command or bytes that do not match.
    pub fn decode(id: u8, a: &[u8]) -> Option<DialogueCommand> {
        let w = |o: usize| a.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        Some(match id {
            PLAY_LINE => {
                // `B b w w w f`: s8 count and that many s32, s8, three s32, f32.
                let n = *a.first()? as i8;
                let n = usize::try_from(n).ok()?;
                let lines = (0..n).map(|k| w(1 + 4 * k)).collect::<Option<Vec<_>>>()?;
                let at = 1 + 4 * n;
                if a.len() != at + 1 + 12 + 4 {
                    return None;
                }
                let quiet = a[at] != 0;
                let volume = f32::from_bits(w(at + 13)? as u32);
                DialogueCommand::PlayLine {
                    lines,
                    quiet,
                    source: w(at + 1)?,
                    falloff: (w(at + 5)?, w(at + 9)?),
                    volume,
                }
            }
            STOP_DIALOGUE => {
                let n = usize::try_from(*a.first()? as i8).ok()?;
                if a.len() != 1 + 4 * n {
                    return None;
                }
                DialogueCommand::Stop { list: (0..n).map(|k| w(1 + 4 * k)).collect::<Option<Vec<_>>>()? }
            }
            CLEAR_SUBTITLE if a.is_empty() => DialogueCommand::ClearSubtitle,
            WAIT_DIALOGUE if a.is_empty() => DialogueCommand::Wait,
            HOLD_EFFECTS | RELEASE_EFFECTS if a.len() == 3 => DialogueCommand::TalkEffects {
                hold: id == HOLD_EFFECTS,
                kind: i16::from_le_bytes([a[0], a[1]]),
                flag: a[2] as i8,
            },
            _ => return None,
        })
    }
}

/// The running order of one play command's line list and the lines played since the last shuffle
/// (dialogue-and-voice.md 1.1). It belongs to the command and lasts while the world is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineOrder {
    order: Vec<i32>,
    count: usize,
}

impl LineOrder {
    /// A fresh order: the stored list, count 0.
    pub fn new(lines: &[i32]) -> LineOrder {
        LineOrder { order: lines.to_vec(), count: 0 }
    }

    /// The running order.
    pub fn order(&self) -> &[i32] {
        &self.order
    }

    /// The line to play this run: one line always plays; otherwise the list rotates by one (first to the end) while
    /// the count is below the list's length, and when it reaches it the list is shuffled (each position from the
    /// positions in order, position i swapped with a random one in 0 to n − 1 − i), the count goes back to 0, and the line that played last is
    /// swapped to the end if the shuffle put it first. The line is the first entry after that step. `None` for an
    /// empty list.
    pub fn next(&mut self, rng: &mut Rng) -> Option<i32> {
        let n = self.order.len();
        if n > 1 {
            if self.count < n {
                self.order.rotate_left(1);
                self.count += 1;
            } else {
                let last = self.order[0];
                for i in 0..n {
                    let j = rng.range_i32(0, (n - 1 - i) as i32) as usize;
                    self.order.swap(i, j);
                }
                self.count = 0;
                if self.order[0] == last {
                    self.order.swap(0, n - 1);
                }
            }
        }
        self.order.first().copied()
    }
}

/// Whether command `0x53` with `list` stops the line `playing` (dialogue-and-voice.md 2).
pub fn stops(list: &[i32], playing: Option<i16>) -> bool {
    match playing {
        None => false,
        Some(p) => list.is_empty() || list.contains(&(p as i32)),
    }
}

/// How a line plays (dialogue-and-voice.md 1.3): at `source` (the source object's position, `None` for −1) with the
/// fall-off when its end is above 0 (full level within the start, silent beyond the end, a start at or past the end
/// read as end = start + 1), else unpositioned; `volume` multiplies its level.
pub fn line_emitter(source: Option<[f32; 3]>, falloff: (i32, i32), volume: f32) -> Emitter {
    let (start, end) = falloff;
    let mut e = match source {
        Some(p) if end > 0 => {
            let end = if start >= end { start + 1 } else { end };
            Emitter::at(p, end as f32, start.max(0) as f32)
        }
        _ => Emitter::global(),
    };
    e.volume = volume;
    e
}

/// Subtitle colour of a Sly 3 speaker class (dialogue-and-voice.md 1.2; RGBA, alpha on the 0x80 scale).
pub fn speaker_colour(class: i16) -> [u8; 4] {
    match class {
        52 => [51, 102, 153, 127], // Sly
        53 => [153, 51, 102, 127], // Murray
        54 => [51, 153, 51, 127],  // Bentley
        60 => [153, 51, 51, 127],  // Carmelita
        _ => [102, 102, 102, 127],
    }
}

/// The subtitle to draw, as [`Talk::subtitle`] with Sly 3's colours.
pub fn subtitle(talk: &Talk, game_time: f32) -> Option<Subtitle> {
    talk.subtitle(game_time).map(|s| Subtitle { colour: speaker_colour(s.speaker), ..s })
}

/// One held talk effect: the character (object ID), kind and flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold {
    /// The character.
    pub character: i32,
    /// The effect kind.
    pub kind: i16,
    /// The flag passed on.
    pub flag: i8,
}

/// A runner's talk-effect holds (dialogue-and-voice.md 4): at most [`MAX_HOLDS`] characters, each once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TalkHolds {
    held: Vec<Hold>,
}

impl TalkHolds {
    /// Command `0x55` on `character`: false (nothing held) when it is already held or the runner holds 4.
    pub fn hold(&mut self, character: i32, kind: i16, flag: i8) -> bool {
        if self.held.len() >= MAX_HOLDS || self.held.iter().any(|h| h.character == character) {
            return false;
        }
        self.held.push(Hold { character, kind, flag });
        true
    }

    /// Command `0x56` on `character`: the hold it ends, if any.
    pub fn release(&mut self, character: i32) -> Option<Hold> {
        let i = self.held.iter().position(|h| h.character == character)?;
        Some(self.held.remove(i))
    }

    /// The runner ended: every hold it still has, released.
    pub fn end(&mut self) -> Vec<Hold> {
        core::mem::take(&mut self.held)
    }

    /// The holds.
    pub fn held(&self) -> &[Hold] {
        &self.held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_line_vector() {
        // scripts.md 8: line list (1587), quiet 1, source −1, fall-off 300 to 3,000, volume 1.0.
        let mut a = alloc::vec![1u8];
        a.extend_from_slice(&1587i32.to_le_bytes());
        a.push(1);
        for v in [-1i32, 300, 3000] {
            a.extend_from_slice(&v.to_le_bytes());
        }
        a.extend_from_slice(&1.0f32.to_le_bytes());
        let c = DialogueCommand::decode(PLAY_LINE, &a).unwrap();
        assert_eq!(
            c,
            DialogueCommand::PlayLine {
                lines: alloc::vec![1587],
                quiet: true,
                source: -1,
                falloff: (300, 3000),
                volume: 1.0
            }
        );
        assert_eq!(DialogueCommand::decode(PLAY_LINE, &a[..a.len() - 1]), None);
        assert_eq!(DialogueCommand::decode(WAIT_DIALOGUE, &[]), Some(DialogueCommand::Wait));
        assert_eq!(DialogueCommand::decode(STOP_DIALOGUE, &[0]), Some(DialogueCommand::Stop { list: alloc::vec![] }));
        assert_eq!(
            DialogueCommand::decode(HOLD_EFFECTS, &[2, 0, 1]),
            Some(DialogueCommand::TalkEffects { hold: true, kind: 2, flag: 1 })
        );
    }

    #[test]
    fn a_single_line_always_plays() {
        let mut o = LineOrder::new(&[1587]);
        let mut rng = Rng::new(1);
        assert!((0..10).all(|_| o.next(&mut rng) == Some(1587)));
        assert_eq!(LineOrder::new(&[]).next(&mut rng), None);
    }

    #[test]
    fn a_fresh_list_plays_its_second_entry_first_and_rotates() {
        let mut o = LineOrder::new(&[10, 20, 30, 40]);
        let mut rng = Rng::new(7);
        // One full round by rotation: 20, 30, 40, 10.
        let round: Vec<i32> = (0..4).map(|_| o.next(&mut rng).unwrap()).collect();
        assert_eq!(round, [20, 30, 40, 10]);
        assert_eq!(o.order(), [10, 20, 30, 40]);
        // The fifth run reshuffles; the next round rotates the shuffled order.
        let first = o.next(&mut rng).unwrap();
        let shuffled = o.order().to_vec();
        assert_eq!(first, shuffled[0]);
        let mut sorted = shuffled.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [10, 20, 30, 40], "a shuffle keeps the lines");
        let next: Vec<i32> = (0..3).map(|_| o.next(&mut rng).unwrap()).collect();
        assert_eq!(next, shuffled[1..]);
    }

    #[test]
    fn the_same_line_never_plays_twice_in_a_row() {
        for len in 2..=20usize {
            let lines: Vec<i32> = (0..len as i32).collect();
            for seed in 1..40u64 {
                let mut o = LineOrder::new(&lines);
                let mut rng = Rng::new(seed);
                let mut last = None;
                for _ in 0..(len * 6) {
                    let l = o.next(&mut rng);
                    assert_ne!(l, last, "{len} lines, seed {seed}");
                    last = l;
                }
            }
        }
    }

    #[test]
    fn a_two_line_list_alternates() {
        let mut o = LineOrder::new(&[5, 6]);
        let mut rng = Rng::new(3);
        let seq: Vec<i32> = (0..8).map(|_| o.next(&mut rng).unwrap()).collect();
        assert_eq!(seq, [6, 5, 6, 5, 6, 5, 6, 5]);
    }

    #[test]
    fn stop_lists() {
        assert!(stops(&[], Some(12)));
        assert!(stops(&[11, 12], Some(12)));
        assert!(!stops(&[11], Some(12)));
        assert!(!stops(&[], None));
    }

    #[test]
    fn fall_off_rules() {
        let g = line_emitter(None, (300, 3000), 0.5);
        assert_eq!((g.position, g.volume), (None, 0.5));
        let e = line_emitter(Some([1.0, 2.0, 3.0]), (300, 3000), 1.0);
        assert_eq!((e.position, e.full_distance, e.start_distance), (Some([1.0, 2.0, 3.0]), 300.0, 3000.0));
        // A start at or past the end: end = start + 1.
        let e = line_emitter(Some([0.0; 3]), (500, 400), 1.0);
        assert_eq!((e.full_distance, e.start_distance), (500.0, 501.0));
        // No fall-off end: unpositioned.
        assert_eq!(line_emitter(Some([0.0; 3]), (300, 0), 1.0).position, None);
    }

    #[test]
    fn speaker_colours() {
        assert_eq!(speaker_colour(52), [51, 102, 153, 127]);
        assert_eq!(speaker_colour(53), [153, 51, 102, 127]);
        assert_eq!(speaker_colour(54), [51, 153, 51, 127]);
        assert_eq!(speaker_colour(60), [153, 51, 51, 127]);
        assert_eq!(speaker_colour(61), [102, 102, 102, 127]);
    }

    #[test]
    fn holds() {
        let mut h = TalkHolds::default();
        assert!(h.hold(1, 0, 0));
        assert!(!h.hold(1, 2, 1), "the same character once");
        assert!(h.hold(2, 0, 0) && h.hold(3, 0, 0) && h.hold(4, 0, 0));
        assert!(!h.hold(5, 0, 0), "at most four");
        assert_eq!(h.release(2).map(|x| x.character), Some(2));
        assert_eq!(h.release(2), None);
        assert_eq!(h.end().iter().map(|x| x.character).collect::<Vec<_>>(), [1, 3, 4]);
        assert!(h.held().is_empty());
    }
}
