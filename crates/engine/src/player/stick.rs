//! The analog stick as the games read it (specs/sly1/player-moves.md 1.2; Sly 2 characters.md 12, the same routine;
//! Sly 3 characters.md 11): bytes to axes, waking up, a self-calibrating range in four directions, a dead zone of
//! 35 % with full deflection from 85 %, and the output (Sly 1 and 2 normalised; Sly 3 scaled by |v| with a release
//! fade). The platform's sticks (−1 … 1 floats) are mapped onto the PlayStation's byte range first
//! ([`StickReader::read_axes`]), so the feel is the same on every pad.

use crate::math::sqrt;

/// The calibration directions (the table at Sly 1 `0x00262A30`, Sly 3 `0x00345108`).
pub const DIRECTIONS: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [0.7, 0.7], [0.7, -0.7]];
/// Each direction's range when the pad is (re)connected.
pub const START_RANGE: f32 = 0.75;
/// The bytes' resting window: a stick wakes when a byte leaves it.
pub const WAKE_WINDOW: (u8, u8) = (96, 160);
/// The dead zone: u = clamp(2·(e − 0.35), 0, 1).
pub const DEAD_ZONE: f32 = 0.35;
/// Sly 3's release fade per frame.
pub const RELEASE_FADE: f32 = 0.2;
/// A direction shorter than this reads as none (`0x38D1B717`).
pub const TINY: f32 = 0.0001;

/// Which game's output rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StickRule {
    /// Sly 1 and Sly 2: the direction normalised and scaled by u.
    #[default]
    Normalised,
    /// Sly 3: (x, y) × u × |v|, fading out over a few frames when released.
    Sly3,
}

/// One stick's reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickReader {
    /// The output rule.
    pub rule: StickRule,
    /// Seen in use.
    pub awake: bool,
    /// Last frame's bytes.
    pub last: Option<[u8; 2]>,
    /// Per direction: [lo, hi].
    pub range: [[f32; 2]; 4],
    /// Sly 3: the remembered deflection and the last direction.
    pub remembered: f32,
    /// See [`StickReader::remembered`].
    pub last_dir: [f32; 2],
}

impl Default for StickReader {
    fn default() -> Self {
        StickReader::new(StickRule::Normalised)
    }
}

impl StickReader {
    /// A freshly connected pad's reader.
    pub fn new(rule: StickRule) -> StickReader {
        StickReader {
            rule,
            awake: false,
            last: None,
            range: [[-START_RANGE, START_RANGE]; 4],
            remembered: 0.0,
            last_dir: [0.0; 2],
        }
    }

    /// The bytes a platform stick (x right, y up, each −1 … 1) stands for.
    pub fn bytes_of(axes: [f32; 2]) -> [u8; 2] {
        let b = |v: f32| ((v.clamp(-1.0, 1.0) + 1.0) * 127.5 + 0.5) as u8;
        [b(axes[0]), b(-axes[1])]
    }

    /// One frame from a platform stick: (direction, deflection u).
    pub fn read_axes(&mut self, axes: [f32; 2]) -> ([f32; 2], f32) {
        self.read(Self::bytes_of(axes))
    }

    /// One frame from the stick's bytes: (direction, deflection u).
    pub fn read(&mut self, bytes: [u8; 2]) -> ([f32; 2], f32) {
        let outside = |b: u8| b < WAKE_WINDOW.0 || b > WAKE_WINDOW.1;
        if !self.awake && (self.last.is_some_and(|l| l != bytes) || outside(bytes[0]) || outside(bytes[1])) {
            self.awake = true;
        }
        self.last = Some(bytes);
        if !self.awake {
            return ([0.0; 2], 0.0);
        }
        let x = 2.0 * f32::from(bytes[0]) / 255.0 - 1.0;
        let y = 1.0 - 2.0 * f32::from(bytes[1]) / 255.0;
        let mut e: f32 = 0.0;
        for (d, r) in DIRECTIONS.iter().zip(self.range.iter_mut()) {
            let p = x * d[0] + y * d[1];
            let ratio = if p >= 0.0 {
                r[1] = r[1].max(p);
                p / r[1]
            } else {
                r[0] = r[0].min(p);
                p / r[0]
            };
            e = e.max(ratio);
        }
        let u = (2.0 * (e - DEAD_ZONE)).clamp(0.0, 1.0);
        let v = sqrt(x * x + y * y);
        let dir = match self.rule {
            StickRule::Normalised => {
                if v <= TINY {
                    [0.0; 2]
                } else {
                    [u * x / v, u * y / v]
                }
            }
            StickRule::Sly3 => {
                if u >= self.remembered - RELEASE_FADE {
                    self.remembered = u;
                    self.last_dir = [x * u * v, y * u * v];
                } else {
                    let s = (self.remembered - RELEASE_FADE).max(0.0);
                    let k = if self.remembered > 0.0 { u / self.remembered } else { 0.0 };
                    self.last_dir = [self.last_dir[0] * k, self.last_dir[1] * k];
                    self.remembered = s;
                }
                self.last_dir
            }
        };
        (dir, u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_dead_zone_and_rules() {
        let mut r = StickReader::default();
        // Resting bytes inside the window: (0, 0) until they change or leave it.
        assert_eq!(r.read([128, 128]), ([0.0; 2], 0.0));
        assert_eq!(r.read([128, 128]), ([0.0; 2], 0.0));
        // Full right: x = 1, beyond the 0.75 range (it grows), u = 1.
        let (d, u) = r.read([255, 128]);
        assert!(r.awake && u == 1.0 && (d[0] - 1.0).abs() < 0.01, "{d:?}");
        // With the range grown to 1, 35 % is the dead zone and 85 % full.
        let mut r = StickReader::default();
        r.read([255, 128]);
        let half = |x: f32| ((x + 1.0) * 127.5) as u8;
        assert_eq!(r.read([half(0.3), 128]).1, 0.0);
        let (_, u) = r.read([half(0.6), 128]);
        assert!((u - 0.5).abs() < 0.02, "{u}");
        assert_eq!(r.read([half(0.9), 128]).1, 1.0);
        // The platform mapping: up is low bytes.
        assert_eq!(StickReader::bytes_of([0.0, 1.0]), [128, 0]);
        // Sly 3: the vector is scaled by |v| and fades when released.
        let mut s = StickReader::new(StickRule::Sly3);
        let (d, u) = s.read([half(0.6), 128]);
        // e = 0.6 / 0.75 = 0.8, u = 0.9, the vector 0.6 × 0.9 × 0.6.
        assert!((u - 0.9).abs() < 0.01 && (d[0] - 0.324).abs() < 0.01, "{d:?} {u}");
        // Released from full: the deflection drops at once, the remembered value by 0.2 a frame.
        s.read([255, 128]);
        let (d1, u1) = s.read([128, 128]);
        assert_eq!((u1, d1), (0.0, [0.0, 0.0]));
        assert!((s.remembered - 0.8).abs() < 1e-6);
    }
}
