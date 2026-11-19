//! Menu input: buttons, the stick and held-direction repeat (ui-and-menus.md 7).

use platform_api::{Buttons, PadState};

/// Delay before the first repeat.
pub const FIRST_REPEAT: f32 = 1.0 / 3.0;
/// Each repeat interval is this fraction of the previous one.
pub const REPEAT_FACTOR: f32 = 2.0 / 3.0;
/// Shortest repeat interval.
pub const MIN_REPEAT: f32 = 0.1;
/// Stick deflection that counts as a direction press.
pub const STICK_PRESS: f32 = 0.8;
/// Stick deflection that keeps a held direction repeating.
pub const STICK_HOLD: f32 = 0.25;

/// A menu direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

/// One frame of menu input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuInput {
    /// Direction pad held: up, down, left, right.
    pub dpad: [bool; 4],
    /// Left stick, x right, y up, -1..1.
    pub stick: (f32, f32),
    /// Accept pressed this frame (Cross on the original).
    pub accept: bool,
    /// Back pressed (Triangle).
    pub back: bool,
    /// Erase pressed (Square, on the slot screens).
    pub erase: bool,
    /// Start pressed.
    pub start: bool,
    /// Select pressed.
    pub select: bool,
}

/// Which buttons do what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonMap {
    /// Accept.
    pub accept: Buttons,
    /// Back.
    pub back: Buttons,
    /// Erase.
    pub erase: Buttons,
    /// Start.
    pub start: Buttons,
    /// Select.
    pub select: Buttons,
}

impl Default for ButtonMap {
    /// The Switch convention: A accepts, B backs out, Y erases, + is Start, - is Select.
    fn default() -> Self {
        ButtonMap {
            accept: Buttons::A,
            back: Buttons::B,
            erase: Buttons::Y,
            start: Buttons::PLUS,
            select: Buttons::MINUS,
        }
    }
}

impl MenuInput {
    /// Reads a controller.
    pub fn from_pad(pad: &PadState, map: &ButtonMap) -> MenuInput {
        let held = |b: Buttons| pad.held.0 & b.0 != 0;
        let pressed = |b: Buttons| pad.pressed.0 & b.0 != 0;
        MenuInput {
            dpad: [held(Buttons::UP), held(Buttons::DOWN), held(Buttons::LEFT), held(Buttons::RIGHT)],
            stick: (pad.left.x, pad.left.y),
            accept: pressed(map.accept),
            back: pressed(map.back),
            erase: pressed(map.erase),
            start: pressed(map.start),
            select: pressed(map.select),
        }
    }
}

/// Turns held directions into moves: one at once, then repeats after 1/3 s, each interval 2/3 of the last, never
/// below 0.1 s. The stick starts a direction past 0.8 and keeps it past 0.25.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Repeat {
    held: Option<Dir>,
    timer: f32,
    interval: f32,
}

impl Repeat {
    /// The move for this frame, if any.
    pub fn update(&mut self, input: &MenuInput, dt: f32) -> Option<Dir> {
        let pad =
            [Dir::Up, Dir::Down, Dir::Left, Dir::Right].into_iter().zip(input.dpad).find(|(_, h)| *h).map(|(d, _)| d);
        let (x, y) = input.stick;
        let stick_dir = |t: f32| -> Option<Dir> {
            if y.abs() >= x.abs() {
                if y > t {
                    Some(Dir::Up)
                } else if y < -t {
                    Some(Dir::Down)
                } else {
                    None
                }
            } else if x > t {
                Some(Dir::Right)
            } else if x < -t {
                Some(Dir::Left)
            } else {
                None
            }
        };
        let dir = pad.or_else(|| {
            let keep = stick_dir(STICK_HOLD);
            if keep.is_some() && keep == self.held { keep } else { stick_dir(STICK_PRESS) }
        });
        if dir != self.held {
            self.held = dir;
            self.timer = FIRST_REPEAT;
            self.interval = FIRST_REPEAT;
            return dir;
        }
        let d = dir?;
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.interval = (self.interval * REPEAT_FACTOR).max(MIN_REPEAT);
            self.timer += self.interval;
            return Some(d);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_timing() {
        let mut r = Repeat::default();
        let input = MenuInput { dpad: [false, true, false, false], ..MenuInput::default() };
        let dt = 1.0 / 600.0;
        let mut times = alloc::vec::Vec::new();
        for i in 0..1200 {
            if r.update(&input, dt).is_some() {
                times.push(i as f32 * dt);
            }
        }
        assert_eq!(times[0], 0.0);
        // 1/3, then 2/9, then 4/27, then 0.1 each.
        let gaps: alloc::vec::Vec<f32> = times.windows(2).map(|w| w[1] - w[0]).collect();
        assert!((gaps[0] - 1.0 / 3.0).abs() < 0.003, "{gaps:?}");
        assert!((gaps[1] - 2.0 / 9.0).abs() < 0.003);
        assert!((gaps[2] - 4.0 / 27.0).abs() < 0.003);
        assert!((gaps[3] - 0.1).abs() < 0.003);
        assert!((gaps[5] - 0.1).abs() < 0.003);
    }

    #[test]
    fn stick_thresholds() {
        let mut r = Repeat::default();
        let at = |x: f32, y: f32| MenuInput { stick: (x, y), ..MenuInput::default() };
        assert_eq!(r.update(&at(0.0, 0.5), 0.01), None, "below 0.8 does not press");
        assert_eq!(r.update(&at(0.0, 0.85), 0.01), Some(Dir::Up));
        // Easing back to 0.3 keeps the hold (no new press).
        assert_eq!(r.update(&at(0.0, 0.3), 0.01), None);
        let mut got = None;
        for _ in 0..40 {
            got = got.or(r.update(&at(0.0, 0.3), 0.01));
        }
        assert_eq!(got, Some(Dir::Up), "held past 0.25 repeats");
        assert_eq!(r.update(&at(0.0, 0.2), 0.01), None);
        assert_eq!(r.update(&at(-0.9, 0.1), 0.01), Some(Dir::Left));
    }
}
