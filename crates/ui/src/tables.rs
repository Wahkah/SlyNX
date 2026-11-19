//! Text tables and the HUD placement table, read from the player's executable by address (ui-and-menus.md 2,
//! hud.md 2). Every reader checks the executable's SHA-256 first and refuses an unknown executable.

use alloc::string::String;
use alloc::vec::Vec;

use formats::disc::{Executable, parse_world_table};

use crate::Error;

/// Prompt table: address and record count (24-byte records).
pub const PROMPT_TABLE: (u32, usize) = (0x0026_9D50, 33);
/// Response text table: address and count.
pub const RESPONSE_TABLE: (u32, usize) = (0x0026_A068, 27);
/// Two-state response texts: address and number of pairs.
pub const TWO_STATE_TABLE: (u32, usize) = (0x0026_A0D8, 8);
/// The movie menu's titles: pointer table address and count, in the order of movie table entries 1 to 16
/// (ui-and-menus.md 2, movies.md). The strings lie at `0x24B438` to `0x24B578`.
pub const MOVIE_TITLES: (u32, usize) = (0x0026_99C0, 16);
/// Pointer-table slots of the five episode introductions, whose titles are also the episode names.
pub const EPISODE_TITLE_SLOTS: [usize; 5] = [4, 6, 8, 10, 12];
/// HUD placement table: address and record count (20-byte records).
pub const PLACEMENT_TABLE: (u32, usize) = (0x0026_C3E0, 37);

/// Single strings the UI uses, by address (observed in the executable).
pub mod va {
    /// "Sly Cooper\nand the Thievius Raccoonus" (title screen).
    pub const TITLE: u32 = 0x0024_CEF0;
    /// "Press START button for New Game".
    pub const PRESS_START_NEW: u32 = 0x0024_CF18;
    /// "Press START button to Resume Game".
    pub const PRESS_START_RESUME: u32 = 0x0024_CF38;
    /// "Press SELECT button for Menu".
    pub const PRESS_SELECT: u32 = 0x0024_CF60;
    /// The save-slot description format, `%d%% - %s - %d:%02d`.
    pub const SLOT_FORMAT: u32 = 0x0024_C418;
    /// Timer format, minutes and seconds.
    pub const TIMER_FORMAT: u32 = 0x0024_CDE0;
    /// Timer format, tenths below one second.
    pub const TIMER_TENTHS_FORMAT: u32 = 0x0024_CDC8;
    /// "Press &2X&. to Start Mission!".
    pub const START_MISSION: u32 = 0x0024_A7B8;
    /// "Bentley" (hideout totals panel on the prologue).
    pub const BENTLEY: u32 = 0x0024_A7A8;
    /// "Murray" (hideout totals panel on an unvisited episode).
    pub const MURRAY: u32 = 0x0024_A7B0;
}

/// One prompt (menu or message).
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// Title text (with markup).
    pub title: Vec<u8>,
    /// Scale of the title text.
    pub title_scale: f32,
    /// Scale of the response texts.
    pub response_scale: f32,
    /// Response layout: 1 stacked vertically, one per line, chosen with up / down; 0 side by side on one line,
    /// chosen with left / right (ui-and-menus.md 2).
    pub layout: i32,
    /// Response IDs.
    pub responses: Vec<i32>,
}

impl Prompt {
    /// Responses stacked vertically (layout 1); otherwise side by side.
    pub fn vertical(&self) -> bool {
        self.layout != 0
    }
}

/// The edges of the screen or of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Top.
    Top,
    /// Bottom.
    Bottom,
}

impl Edge {
    /// From the table's code (0 left, 1 right, 2 top, 3 bottom; anything else none).
    pub fn from_code(c: i32) -> Option<Edge> {
        match c {
            0 => Some(Edge::Left),
            1 => Some(Edge::Right),
            2 => Some(Edge::Top),
            3 => Some(Edge::Bottom),
            _ => None,
        }
    }
}

/// One record of the HUD placement table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// x hint: > 0 left side that far from the left edge, < 0 right side from the right edge, 0 centred.
    pub x: f32,
    /// y hint: > 0 top from the top, < 0 bottom from the bottom, 0 centred.
    pub y: f32,
    /// Edge it slides in from.
    pub slide_from: Option<Edge>,
    /// Element it is placed against.
    pub against: Option<usize>,
    /// Edge of that element.
    pub against_edge: Option<Edge>,
}

/// The text tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextTables {
    /// The 33 prompts.
    pub prompts: Vec<Prompt>,
    /// Response texts by ID (empty where the game fills the text at run time).
    pub responses: Vec<Vec<u8>>,
    /// Two-state texts: (slot new-game, slot empty), (Quit Game, Save and Quit), vibrations on/off, music on/off,
    /// speakers stereo/mono, then the three invert options (down, up).
    pub two_state: Vec<(Vec<u8>, Vec<u8>)>,
    /// The movie menu's 16 titles, in movie order (bonus movies, each episode's introduction and boss-defeat movie,
    /// The Thievius Raccoonus, Credits).
    pub movie_titles: Vec<Vec<u8>>,
    /// The five episode names (the introductions' titles, [`EPISODE_TITLE_SLOTS`]).
    pub episodes: Vec<Vec<u8>>,
    /// Level names: the world table's display names, by world index.
    pub levels: Vec<Vec<u8>>,
    /// Title text.
    pub title: Vec<u8>,
    /// "Press START button for New Game".
    pub press_start_new: Vec<u8>,
    /// "Press START button to Resume Game".
    pub press_start_resume: Vec<u8>,
    /// "Press SELECT button for Menu".
    pub press_select: Vec<u8>,
    /// Save-slot description format.
    pub slot_format: Vec<u8>,
    /// Timer format (minutes, seconds).
    pub timer_format: Vec<u8>,
    /// Timer format (tenths).
    pub timer_tenths_format: Vec<u8>,
    /// Hideout hint.
    pub start_mission: Vec<u8>,
    /// "Bentley".
    pub bentley: Vec<u8>,
    /// "Murray".
    pub murray: Vec<u8>,
}

/// Indices into [`TextTables::two_state`].
pub mod pair {
    /// Slot: "New Game" / "Empty".
    pub const SLOT: usize = 0;
    /// "Quit Game" / "Save and Quit".
    pub const QUIT: usize = 1;
    /// "Vibrations: On" / "Off".
    pub const VIBRATION: usize = 2;
    /// "Music: On" / "Off".
    pub const MUSIC: usize = 3;
    /// "Speakers: Stereo" / "Mono".
    pub const SPEAKERS: usize = 4;
    /// Binoc-u-com invert (Look Down / Look Up).
    pub const INVERT_BINOC: usize = 5;
    /// Turrets invert.
    pub const INVERT_TURRET: usize = 6;
    /// Jet pack invert.
    pub const INVERT_JETPACK: usize = 7;
}

/// Response IDs (ui-and-menus.md 2).
pub mod resp {
    #![allow(missing_docs)]
    pub const YES: i32 = 0;
    pub const NO: i32 = 1;
    pub const CONTINUE: i32 = 2;
    pub const RETURN_TO_GAME: i32 = 3;
    pub const VIEW_MAP: i32 = 4;
    pub const OPTIONS: i32 = 5;
    pub const NEW_GAME: i32 = 6;
    pub const LOAD_GAME: i32 = 7;
    pub const SAVE_GAME: i32 = 8;
    pub const QUIT: i32 = 9;
    pub const SLOT_1: i32 = 10;
    pub const SLOT_2: i32 = 11;
    pub const SLOT_3: i32 = 12;
    pub const VIBRATION: i32 = 13;
    pub const CENTER_SCREEN: i32 = 14;
    pub const MUSIC: i32 = 15;
    pub const SPEAKERS: i32 = 16;
    pub const BACK: i32 = 17;
    pub const RESTART_RACE: i32 = 18;
    pub const RESTART_SPRINT: i32 = 19;
    pub const RESTART_LEVEL: i32 = 20;
    pub const EXIT_LEVEL: i32 = 21;
    pub const EXIT_TO_HIDEOUT: i32 = 22;
    pub const CONTROLS: i32 = 23;
    pub const INVERT_BINOC: i32 = 24;
    pub const INVERT_TURRET: i32 = 25;
    pub const INVERT_JETPACK: i32 = 26;
}

/// Prompt indices (ui-and-menus.md 2).
pub mod prompt {
    #![allow(missing_docs)]
    pub const PAUSED: usize = 0;
    pub const GAME_OVER: usize = 1;
    pub const REALLY_QUIT: usize = 2;
    pub const SAVE_GAME: usize = 11;
    pub const SAVING: usize = 12;
    pub const GAME_SAVED: usize = 13;
    pub const OVERWRITE: usize = 14;
    pub const NEW_GAME: usize = 16;
    pub const LOAD_GAME: usize = 17;
    pub const ERASE: usize = 18;
    pub const ERASING: usize = 19;
    pub const AUTOSAVE_NOTICE: usize = 20;
    pub const OPTIONS: usize = 23;
    pub const HELP: usize = 24;
    pub const CONTROLS: usize = 31;
}

fn check(exe: &Executable) -> Result<(), Error> {
    if exe.is_supported() { Ok(()) } else { Err(Error::UnknownExecutable) }
}

fn word(exe: &Executable, va: u32) -> Result<u32, Error> {
    let b = exe.bytes_at(va, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The NUL-terminated bytes at `va` (empty for address 0).
pub fn c_bytes(exe: &Executable, va: u32) -> Result<Vec<u8>, Error> {
    if va == 0 {
        return Ok(Vec::new());
    }
    let off = exe.va_to_offset(va).ok_or(Error::Invalid("string address outside the executable"))?;
    let rest = &exe.data[off..];
    let end = rest.iter().position(|&b| b == 0).ok_or(Error::Invalid("unterminated string"))?;
    Ok(rest[..end].to_vec())
}

impl TextTables {
    /// Reads every table from a supported executable.
    pub fn read(exe: &Executable) -> Result<TextTables, Error> {
        check(exe)?;
        let mut prompts = Vec::with_capacity(PROMPT_TABLE.1);
        for i in 0..PROMPT_TABLE.1 as u32 {
            let b = PROMPT_TABLE.0 + i * 24;
            let count = word(exe, b + 16)? as i32;
            let list = word(exe, b + 20)?;
            let mut responses = Vec::new();
            for k in 0..count.clamp(0, 16) as u32 {
                responses.push(word(exe, list + k * 4)? as i32);
            }
            prompts.push(Prompt {
                title: c_bytes(exe, word(exe, b)?)?,
                title_scale: f32::from_bits(word(exe, b + 4)?),
                response_scale: f32::from_bits(word(exe, b + 8)?),
                layout: word(exe, b + 12)? as i32,
                responses,
            });
        }
        let responses = (0..RESPONSE_TABLE.1 as u32)
            .map(|i| c_bytes(exe, word(exe, RESPONSE_TABLE.0 + i * 4)?))
            .collect::<Result<Vec<_>, _>>()?;
        let mut two_state = Vec::with_capacity(TWO_STATE_TABLE.1);
        for i in 0..TWO_STATE_TABLE.1 as u32 {
            let a = c_bytes(exe, word(exe, TWO_STATE_TABLE.0 + i * 8)?)?;
            let b = c_bytes(exe, word(exe, TWO_STATE_TABLE.0 + i * 8 + 4)?)?;
            two_state.push((a, b));
        }
        let movie_titles = (0..MOVIE_TITLES.1 as u32)
            .map(|i| c_bytes(exe, word(exe, MOVIE_TITLES.0 + i * 4)?))
            .collect::<Result<Vec<_>, _>>()?;
        let episodes = EPISODE_TITLE_SLOTS.iter().map(|&i| movie_titles[i].clone()).collect();
        let levels = parse_world_table(exe)?.into_iter().map(|w| w.name.into_bytes()).collect();
        Ok(TextTables {
            prompts,
            responses,
            two_state,
            movie_titles,
            episodes,
            levels,
            title: c_bytes(exe, va::TITLE)?,
            press_start_new: c_bytes(exe, va::PRESS_START_NEW)?,
            press_start_resume: c_bytes(exe, va::PRESS_START_RESUME)?,
            press_select: c_bytes(exe, va::PRESS_SELECT)?,
            slot_format: c_bytes(exe, va::SLOT_FORMAT)?,
            timer_format: c_bytes(exe, va::TIMER_FORMAT)?,
            timer_tenths_format: c_bytes(exe, va::TIMER_TENTHS_FORMAT)?,
            start_mission: c_bytes(exe, va::START_MISSION)?,
            bentley: c_bytes(exe, va::BENTLEY)?,
            murray: c_bytes(exe, va::MURRAY)?,
        })
    }

    /// The text of response `id`, resolving the two-state ones with `state` (false = first text).
    pub fn response(&self, id: i32, state: bool) -> &[u8] {
        let pick =
            |p: usize| self.two_state.get(p).map_or(&[][..], |(a, b)| if state { b.as_slice() } else { a.as_slice() });
        match id {
            resp::QUIT => pick(pair::QUIT),
            resp::VIBRATION => pick(pair::VIBRATION),
            resp::MUSIC => pick(pair::MUSIC),
            resp::SPEAKERS => pick(pair::SPEAKERS),
            resp::INVERT_BINOC => pick(pair::INVERT_BINOC),
            resp::INVERT_TURRET => pick(pair::INVERT_TURRET),
            resp::INVERT_JETPACK => pick(pair::INVERT_JETPACK),
            _ => self.responses.get(id as usize).map_or(&[][..], |r| r.as_slice()),
        }
    }

    /// The description of a save slot: [`SlotInfo`] through the slot format, or "Empty".
    pub fn slot_description(&self, slot: Option<&SlotInfo>) -> Vec<u8> {
        match slot {
            None => self.two_state.get(pair::SLOT).map_or_else(|| b"Empty".to_vec(), |p| p.1.clone()),
            Some(s) => sprintf(
                &self.slot_format,
                &[
                    Arg::Int(s.percent as i64),
                    Arg::Str(&s.world),
                    Arg::Int((s.minutes / 60) as i64),
                    Arg::Int((s.minutes % 60) as i64),
                ],
            ),
        }
    }

    /// The timer text for `seconds` (tenths below one second).
    pub fn timer_text(&self, seconds: f32) -> Vec<u8> {
        let s = seconds.max(0.0);
        if s < 1.0 {
            sprintf(&self.timer_tenths_format, &[Arg::Int((s * 10.0) as i64)])
        } else {
            let whole = s as i64;
            sprintf(&self.timer_format, &[Arg::Int(whole / 60), Arg::Int(whole % 60)])
        }
    }
}

/// What a used save slot shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    /// Completion percent.
    pub percent: u32,
    /// Name of the world being played.
    pub world: Vec<u8>,
    /// Play time in minutes.
    pub minutes: u32,
}

/// A `sprintf` argument.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    /// `%d`.
    Int(i64),
    /// `%s`.
    Str(&'a [u8]),
    /// `%c`.
    Char(u8),
}

/// The subset of C formatting the game's strings use: `%d` (with an optional `0` flag and width), `%s`, `%c`,
/// `%%`. Missing arguments print nothing.
pub fn sprintf(fmt: &[u8], args: &[Arg<'_>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fmt.len() + 16);
    let mut args = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] != b'%' {
            out.push(fmt[i]);
            i += 1;
            continue;
        }
        i += 1;
        let zero = fmt.get(i) == Some(&b'0');
        if zero {
            i += 1;
        }
        let mut width = 0usize;
        while let Some(d @ b'0'..=b'9') = fmt.get(i).copied() {
            width = width * 10 + (d - b'0') as usize;
            i += 1;
        }
        match fmt.get(i).copied() {
            Some(b'%') => out.push(b'%'),
            Some(b'd') => {
                if let Some(Arg::Int(v)) = args.next() {
                    let mut digits = String::new();
                    let neg = *v < 0;
                    let mut n = v.unsigned_abs();
                    loop {
                        digits.insert(0, (b'0' + (n % 10) as u8) as char);
                        n /= 10;
                        if n == 0 {
                            break;
                        }
                    }
                    let len = digits.len() + neg as usize;
                    if neg && zero {
                        out.push(b'-');
                    }
                    for _ in len..width {
                        out.push(if zero { b'0' } else { b' ' });
                    }
                    if neg && !zero {
                        out.push(b'-');
                    }
                    out.extend_from_slice(digits.as_bytes());
                }
            }
            Some(b's') => {
                if let Some(Arg::Str(s)) = args.next() {
                    out.extend_from_slice(s);
                }
            }
            Some(b'c') => {
                if let Some(Arg::Char(c)) = args.next() {
                    out.push(*c);
                }
            }
            Some(other) => out.push(other),
            None => break,
        }
        i += 1;
    }
    out
}

/// Reads the HUD placement table from a supported executable.
pub fn read_placements(exe: &Executable) -> Result<Vec<Placement>, Error> {
    check(exe)?;
    let (base, n) = PLACEMENT_TABLE;
    (0..n as u32)
        .map(|i| {
            let b = base + i * 20;
            let against = word(exe, b + 12)? as i32;
            Ok(Placement {
                x: f32::from_bits(word(exe, b)?),
                y: f32::from_bits(word(exe, b + 4)?),
                slide_from: Edge::from_code(word(exe, b + 8)? as i32),
                against: (against >= 0).then_some(against as usize),
                against_edge: Edge::from_code(word(exe, b + 16)? as i32),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printf_subset() {
        let s = sprintf(b"%d%% - %s - %d:%02d", &[Arg::Int(42), Arg::Str(b"Paris"), Arg::Int(3), Arg::Int(7)]);
        assert_eq!(s, b"42% - Paris - 3:07");
        assert_eq!(sprintf(b"^10~b2a83d0.%d~.^.", &[Arg::Int(5)]), b"^10~b2a83d0.5~.^.");
        assert_eq!(sprintf(b"%c%d/%d", &[Arg::Char(b'L'), Arg::Int(3), Arg::Int(10)]), b"L3/10");
        assert_eq!(sprintf(b"%3d|%-", &[Arg::Int(-4)]), b" -4|-");
        assert_eq!(sprintf(b"%d", &[]), b"");
    }

    #[test]
    fn slots_and_timer() {
        let t = TextTables {
            two_state: alloc::vec![(b"New Game".to_vec(), b"Empty".to_vec())],
            slot_format: b"%d%% - %s - %d:%02d".to_vec(),
            timer_format: b"%d:%02d".to_vec(),
            timer_tenths_format: b"0.%d".to_vec(),
            ..TextTables::default()
        };
        assert_eq!(t.slot_description(None), b"Empty");
        let info = SlotInfo { percent: 12, world: b"Paris, France".to_vec(), minutes: 125 };
        assert_eq!(t.slot_description(Some(&info)), b"12% - Paris, France - 2:05");
        assert_eq!(t.timer_text(65.9), b"1:05");
        assert_eq!(t.timer_text(0.45), b"0.4");
        assert_eq!(Edge::from_code(3), Some(Edge::Bottom));
        assert_eq!(Edge::from_code(-1), None);
    }
}
