//! Controller state and scripted input playback.
//!
//! # Script format
//!
//! A script is plain text, one command per line. `#` starts a comment; blank lines are ignored. Each command starts
//! with a frame (`N`, counted from 0) or a half-open frame range (`N..M`, frames `N` to `M - 1`):
//!
//! ```text
//! # walk right for one second, jump at frame 30
//! 0..60   hold right
//! 30      tap a
//! 45      down zr          # held until released
//! 90      up zr
//! 100     stick left 0.5 -1
//! 120..180 stick right 1 0 # set for the range, back to 0 at frame 180
//! ```
//!
//! | Command | Meaning |
//! |---|---|
//! | `down B...` | press the buttons from this frame on |
//! | `up B...` | release the buttons from this frame on |
//! | `tap B...` | press for exactly one frame (or for the range, like `hold`) |
//! | `hold B...` | press for the range (a single frame means one frame) |
//! | `stick left\|right X Y` | set a stick, `X` and `Y` in `[-1, 1]` (`+Y` is up); with a range, reset to `0 0` at its end |
//!
//! Buttons use Switch names, case-insensitive: `a b x y l r zl zr plus minus up down left right lstick rstick sl sr`
//! (`start` and `select` are accepted for `plus` and `minus`). Commands on the same frame apply in file order.

use std::fmt;

/// A controller button. The discriminant is the button's bit in [`InputState::buttons`].
///
/// The bit layout is the platform layer's (`Buttons` in the platform API: the low 16 bits follow the Switch's
/// controller report), so a harness state converts to the platform's type without remapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Button {
    /// Right face button.
    A = 1 << 0,
    /// Bottom face button.
    B = 1 << 1,
    /// Top face button.
    X = 1 << 2,
    /// Left face button.
    Y = 1 << 3,
    LStick = 1 << 4,
    RStick = 1 << 5,
    L = 1 << 6,
    R = 1 << 7,
    ZL = 1 << 8,
    ZR = 1 << 9,
    Plus = 1 << 10,
    Minus = 1 << 11,
    Left = 1 << 12,
    Up = 1 << 13,
    Right = 1 << 14,
    Down = 1 << 15,
    /// SL on either Joy-Con.
    SL = 1 << 16,
    /// SR on either Joy-Con.
    SR = 1 << 17,
}

impl Button {
    /// Every button, in bit order.
    pub const ALL: [Button; 18] = [
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::LStick,
        Button::RStick,
        Button::L,
        Button::R,
        Button::ZL,
        Button::ZR,
        Button::Plus,
        Button::Minus,
        Button::Left,
        Button::Up,
        Button::Right,
        Button::Down,
        Button::SL,
        Button::SR,
    ];

    /// The button's bit mask.
    pub const fn bit(self) -> u32 {
        self as u32
    }

    /// The script name of the button.
    pub const fn name(self) -> &'static str {
        match self {
            Button::A => "a",
            Button::B => "b",
            Button::X => "x",
            Button::Y => "y",
            Button::L => "l",
            Button::R => "r",
            Button::ZL => "zl",
            Button::ZR => "zr",
            Button::Plus => "plus",
            Button::Minus => "minus",
            Button::Up => "up",
            Button::Down => "down",
            Button::Left => "left",
            Button::Right => "right",
            Button::LStick => "lstick",
            Button::RStick => "rstick",
            Button::SL => "sl",
            Button::SR => "sr",
        }
    }

    /// Parses a script button name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Button> {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "start" => return Some(Button::Plus),
            "select" => return Some(Button::Minus),
            _ => {}
        }
        Button::ALL.into_iter().find(|b| b.name() == lower)
    }
}

/// Which analog stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stick {
    Left,
    Right,
}

/// The controller state for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    /// Buttons held this frame (bits from [`Button`]).
    pub buttons: u32,
    /// Buttons that went down this frame.
    pub pressed: u32,
    /// Buttons that went up this frame.
    pub released: u32,
    /// Left stick, `x` right and `y` up, each in `[-1, 1]`.
    pub left_stick: [f32; 2],
    /// Right stick, `x` right and `y` up, each in `[-1, 1]`.
    pub right_stick: [f32; 2],
}

impl InputState {
    /// Whether the button is held.
    pub fn held(&self, b: Button) -> bool {
        self.buttons & b.bit() != 0
    }

    /// Whether the button went down this frame.
    pub fn just_pressed(&self, b: Button) -> bool {
        self.pressed & b.bit() != 0
    }

    /// Whether the button went up this frame.
    pub fn just_released(&self, b: Button) -> bool {
        self.released & b.bit() != 0
    }

    /// Fills in `pressed` and `released` from the previous frame's held buttons.
    pub fn with_edges_from(mut self, previous_buttons: u32) -> Self {
        self.pressed = self.buttons & !previous_buttons;
        self.released = previous_buttons & !self.buttons;
        self
    }
}

/// A script parse error, with its 1-based line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "input script line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Down(u32),
    Up(u32),
    Stick(Stick, [f32; 2]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Event {
    frame: u64,
    /// Order of the event in the script, so same-frame events keep file order.
    seq: usize,
    action: Action,
}

/// A parsed input script: a list of timed events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputScript {
    events: Vec<Event>,
}

fn parse_frame_spec(spec: &str) -> Result<(u64, Option<u64>), String> {
    let num = |s: &str| s.trim().parse::<u64>().map_err(|_| format!("bad frame number `{s}`"));
    match spec.split_once("..") {
        Some((a, b)) => {
            let (a, b) = (num(a)?, num(b)?);
            if b <= a {
                return Err(format!("empty frame range `{spec}`"));
            }
            Ok((a, Some(b)))
        }
        None => Ok((num(spec)?, None)),
    }
}

fn parse_buttons(words: &[&str]) -> Result<u32, String> {
    if words.is_empty() {
        return Err("expected at least one button".into());
    }
    let mut mask = 0;
    for w in words {
        mask |= Button::from_name(w).ok_or_else(|| format!("unknown button `{w}`"))?.bit();
    }
    Ok(mask)
}

fn parse_axis(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| format!("bad stick value `{s}`"))?;
    if !(-1.0..=1.0).contains(&v) {
        return Err(format!("stick value `{s}` outside [-1, 1]"));
    }
    Ok(v)
}

impl InputScript {
    /// An empty script (no input on any frame).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parses a script.
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        let mut events = Vec::new();
        for (idx, raw) in text.lines().enumerate() {
            let line = idx + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let err = |message: String| ScriptError { line, message };
            let words: Vec<&str> = content.split_whitespace().collect();
            if words.len() < 2 {
                return Err(err("expected `<frame> <command> ...`".into()));
            }
            let (start, end) = parse_frame_spec(words[0]).map_err(err)?;
            let args = &words[2..];
            let mut push = |frame: u64, action: Action| {
                let seq = events.len();
                events.push(Event { frame, seq, action });
            };
            match words[1].to_ascii_lowercase().as_str() {
                "down" => {
                    let m = parse_buttons(args).map_err(err)?;
                    if end.is_some() {
                        return Err(err("`down` takes a single frame; use `hold` for a range".into()));
                    }
                    push(start, Action::Down(m));
                }
                "up" => {
                    let m = parse_buttons(args).map_err(err)?;
                    if end.is_some() {
                        return Err(err("`up` takes a single frame".into()));
                    }
                    push(start, Action::Up(m));
                }
                "tap" | "hold" => {
                    let m = parse_buttons(args).map_err(err)?;
                    push(start, Action::Down(m));
                    push(end.unwrap_or(start + 1), Action::Up(m));
                }
                "stick" => {
                    if args.len() != 3 {
                        return Err(err("expected `stick left|right X Y`".into()));
                    }
                    let which = match args[0].to_ascii_lowercase().as_str() {
                        "left" | "l" => Stick::Left,
                        "right" | "r" => Stick::Right,
                        other => return Err(err(format!("unknown stick `{other}`"))),
                    };
                    let v = [parse_axis(args[1]).map_err(err)?, parse_axis(args[2]).map_err(err)?];
                    push(start, Action::Stick(which, v));
                    if let Some(e) = end {
                        push(e, Action::Stick(which, [0.0, 0.0]));
                    }
                }
                other => return Err(err(format!("unknown command `{other}`"))),
            }
        }
        events.sort_by_key(|e| (e.frame, e.seq));
        Ok(InputScript { events })
    }

    /// The last frame on which the script changes anything, if any.
    pub fn last_event_frame(&self) -> Option<u64> {
        self.events.last().map(|e| e.frame)
    }

    /// A player that yields the input state frame by frame.
    pub fn player(&self) -> InputPlayer<'_> {
        InputPlayer { script: self, next: 0, frame: 0, state: InputState::default() }
    }

    /// Builds a script that reproduces a recorded sequence of states (one per frame, starting at frame 0). Only
    /// `buttons` and the sticks are recorded; edges are recomputed on playback.
    pub fn from_states(states: &[InputState]) -> Self {
        let mut events = Vec::new();
        let mut prev = InputState::default();
        for (frame, s) in states.iter().enumerate() {
            let frame = frame as u64;
            let mut push = |action| {
                let seq = events.len();
                events.push(Event { frame, seq, action });
            };
            let up = prev.buttons & !s.buttons;
            let down = s.buttons & !prev.buttons;
            if up != 0 {
                push(Action::Up(up));
            }
            if down != 0 {
                push(Action::Down(down));
            }
            if s.left_stick != prev.left_stick {
                push(Action::Stick(Stick::Left, s.left_stick));
            }
            if s.right_stick != prev.right_stick {
                push(Action::Stick(Stick::Right, s.right_stick));
            }
            prev = *s;
        }
        InputScript { events }
    }

    /// Writes the script in the text format. `parse(to_text())` gives back an equivalent script.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# SlyNX input script\n");
        let names = |mask: u32| {
            Button::ALL.iter().filter(|b| mask & b.bit() != 0).map(|b| b.name()).collect::<Vec<_>>().join(" ")
        };
        for e in &self.events {
            let line = match e.action {
                Action::Down(m) => format!("{} down {}", e.frame, names(m)),
                Action::Up(m) => format!("{} up {}", e.frame, names(m)),
                Action::Stick(s, [x, y]) => {
                    let which = if s == Stick::Left { "left" } else { "right" };
                    format!("{} stick {which} {x} {y}", e.frame)
                }
            };
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

/// Plays a script back one frame at a time.
#[derive(Clone, Debug)]
pub struct InputPlayer<'a> {
    script: &'a InputScript,
    next: usize,
    frame: u64,
    state: InputState,
}

impl InputPlayer<'_> {
    /// The input for the next frame (frame 0 on the first call).
    pub fn next_frame(&mut self) -> InputState {
        let prev_buttons = self.state.buttons;
        while let Some(e) = self.script.events.get(self.next) {
            if e.frame > self.frame {
                break;
            }
            match e.action {
                Action::Down(m) => self.state.buttons |= m,
                Action::Up(m) => self.state.buttons &= !m,
                Action::Stick(Stick::Left, v) => self.state.left_stick = v,
                Action::Stick(Stick::Right, v) => self.state.right_stick = v,
            }
            self.next += 1;
        }
        self.frame += 1;
        self.state = self.state.with_edges_from(prev_buttons);
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(script: &InputScript, frames: usize) -> Vec<InputState> {
        let mut p = script.player();
        (0..frames).map(|_| p.next_frame()).collect()
    }

    #[test]
    fn parses_and_plays_commands() {
        let s = InputScript::parse(
            "# comment\n\n0..3 hold right\n1 tap A\n2 down zr  # trailing\n4 up ZR\n5 stick left 0.5 -1\n6..8 stick r 1 0\n",
        )
        .unwrap();
        let f = play(&s, 10);
        assert!(f[0].held(Button::Right) && f[0].just_pressed(Button::Right));
        assert!(f[2].held(Button::Right) && !f[2].just_pressed(Button::Right));
        assert!(!f[3].held(Button::Right) && f[3].just_released(Button::Right));
        assert!(!f[0].held(Button::A));
        assert!(f[1].held(Button::A) && f[1].just_pressed(Button::A));
        assert!(!f[2].held(Button::A) && f[2].just_released(Button::A));
        assert!(f[2].held(Button::ZR) && f[3].held(Button::ZR) && !f[4].held(Button::ZR));
        assert_eq!(f[4].left_stick, [0.0, 0.0]);
        assert_eq!(f[5].left_stick, [0.5, -1.0]);
        assert_eq!(f[9].left_stick, [0.5, -1.0]);
        assert_eq!(f[5].right_stick, [0.0, 0.0]);
        assert_eq!(f[6].right_stick, [1.0, 0.0]);
        assert_eq!(f[7].right_stick, [1.0, 0.0]);
        assert_eq!(f[8].right_stick, [0.0, 0.0]);
        assert_eq!(s.last_event_frame(), Some(8));
    }

    #[test]
    fn same_frame_keeps_file_order() {
        let s = InputScript::parse("3 down a\n3 up a\n").unwrap();
        assert!(play(&s, 5).iter().all(|f| !f.held(Button::A)));
        let s = InputScript::parse("3 up a\n3 down a\n").unwrap();
        assert!(play(&s, 5)[3].held(Button::A));
    }

    #[test]
    fn aliases_and_case() {
        assert_eq!(Button::from_name("START"), Some(Button::Plus));
        assert_eq!(Button::from_name("Select"), Some(Button::Minus));
        assert_eq!(Button::from_name("ZL"), Some(Button::ZL));
        assert_eq!(Button::from_name("nope"), None);
        assert_eq!(Button::from_name("SR"), Some(Button::SR));
    }

    #[test]
    fn bits_follow_the_platform_layout() {
        let expected = [
            ("a", 0),
            ("b", 1),
            ("x", 2),
            ("y", 3),
            ("lstick", 4),
            ("rstick", 5),
            ("l", 6),
            ("r", 7),
            ("zl", 8),
            ("zr", 9),
            ("plus", 10),
            ("minus", 11),
            ("left", 12),
            ("up", 13),
            ("right", 14),
            ("down", 15),
            ("sl", 16),
            ("sr", 17),
        ];
        for (b, (name, bit)) in Button::ALL.iter().zip(expected) {
            assert_eq!((b.name(), b.bit()), (name, 1 << bit));
        }
    }

    #[test]
    fn errors_name_the_line() {
        for (text, line) in [
            ("0 tap a\n1 jump a", 2),
            ("x tap a", 1),
            ("0 tap", 1),
            ("\n\n5..5 hold a", 3),
            ("0 tap q", 1),
            ("0 stick left 2 0", 1),
            ("0 stick middle 0 0", 1),
            ("0..4 down a", 1),
            ("7", 1),
        ] {
            let e = InputScript::parse(text).unwrap_err();
            assert_eq!(e.line, line, "{text:?}: {e}");
        }
    }

    #[test]
    fn record_and_replay_round_trip() {
        let mut states = Vec::new();
        let mut rng = crate::rng::Rng::new(5);
        for _ in 0..300 {
            let mut s = InputState { buttons: rng.next_u32() & 0x3_FFFF, ..Default::default() };
            if rng.chance(0.7) {
                s.buttons = states.last().map_or(0, |p: &InputState| p.buttons);
            }
            if rng.chance(0.2) {
                s.left_stick = [rng.next_f32() * 2.0 - 1.0, rng.next_f32() * 2.0 - 1.0];
            } else if let Some(p) = states.last() {
                s.left_stick = p.left_stick;
            }
            states.push(s);
        }
        let script = InputScript::from_states(&states);
        let reparsed = InputScript::parse(&script.to_text()).unwrap();
        let played = play(&reparsed, states.len());
        let mut prev = 0;
        for (want, got) in states.iter().zip(&played) {
            assert_eq!(want.with_edges_from(prev), *got);
            prev = want.buttons;
        }
    }
}
