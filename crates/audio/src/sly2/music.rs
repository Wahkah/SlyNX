//! Sly 2 music and resident-stream mixing rules (sly2/audio.md 3.1 and 3.2), computed on the game thread.
//!
//! - The **resident stream** of a world: each channel is a distance layer. Ambient-sound objects add fall-offs to
//!   a channel (by channel number): full volume `g` within `s0`, silent beyond `s1`, linear between. A channel's
//!   volume is the loudest of its fall-offs; with exactly one fall-off it pans toward it; with none it plays at its
//!   stored volume.
//! - **Music streams**: groups 0, 1 and 2 are the sneak, normal and alert versions of the piece, cross-faded with
//!   equal power (their squared weights add up to 1); groups 3 and 4 fade with the distance to the playable
//!   characters (full within 2,000 units, silent beyond 5,000); group 5 is not weighted.

use crate::fmath;
use crate::spatial::{Listener, Vec3, attenuation, position_pan};

/// Full volume within this distance of a character (groups 3 and 4).
pub const CHARACTER_NEAR: f32 = 2000.0;
/// Silent beyond this distance from a character.
pub const CHARACTER_FAR: f32 = 5000.0;
/// Number of music layers (channel group numbers).
pub const LAYERS: usize = 8;

/// What the current player character's state asks of the music (the exact state list is open in the spec).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterMusicState {
    /// Stealth or interaction states: the sneak layer.
    Stealth,
    /// Everything else: the normal layer.
    #[default]
    Normal,
    /// The one alert state.
    Alert,
}

/// Speed-limited smoothing toward a goal: the speed is the remaining distance over `ease` seconds, kept between
/// `slow` and `fast` per second (an approximation of the game's smoothing, whose exact curve is not specified).
pub fn approach(value: f32, goal: f32, dt: f32, fast: f32, slow: f32, ease: f32) -> f32 {
    let diff = goal - value;
    let speed = (diff.abs() / ease.max(1e-6)).clamp(slow, fast);
    let step = speed * dt;
    if diff.abs() <= step {
        goal
    } else if diff > 0.0 {
        value + step
    } else {
        value - step
    }
}

/// The sneak / normal / alert selection and the cross-faded weights of groups 0, 1 and 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicMood {
    /// Alert counter: above zero while a guard chases the player (and when scripts raise it).
    pub alert: i32,
    /// Sneak counter, raised and lowered by scripts.
    pub sneak: i32,
    /// The current player character's state.
    pub state: CharacterMusicState,
    weights: [f32; 3],
}

impl Default for MusicMood {
    fn default() -> Self {
        MusicMood { alert: 0, sneak: 0, state: CharacterMusicState::Normal, weights: [0.0, 1.0, 0.0] }
    }
}

impl MusicMood {
    /// The group chosen now: alert while the alert counter is above zero, else sneak while the sneak counter is,
    /// else by the character's state.
    pub fn chosen(&self) -> usize {
        if self.alert > 0 {
            2
        } else if self.sneak > 0 {
            0
        } else {
            match self.state {
                CharacterMusicState::Stealth => 0,
                CharacterMusicState::Normal => 1,
                CharacterMusicState::Alert => 2,
            }
        }
    }

    /// Advances the weights by `dt` seconds: the chosen group's weight moves toward 1 (sneak: fast 3, slow 0,
    /// ease 0.25 s; normal and alert: fast 1, slow 0, ease 0.25 s), then the other two are scaled together so the
    /// three squares add up to 1.
    pub fn update(&mut self, dt: f32) {
        let c = self.chosen();
        let (fast, slow, ease) = if c == 0 { (3.0, 0.0, 0.25) } else { (1.0, 0.0, 0.25) };
        let w = approach(self.weights[c], 1.0, dt, fast, slow, ease).clamp(0.0, 1.0);
        self.weights[c] = w;
        let rest = 1.0 - w * w;
        let others: f32 = (0..3).filter(|&i| i != c).map(|i| self.weights[i] * self.weights[i]).sum();
        if others > 0.0 {
            let k = fmath::sqrt((rest / others) as f64) as f32;
            for i in (0..3).filter(|&i| i != c) {
                self.weights[i] *= k;
            }
        } else {
            self.weights[c] = 1.0;
        }
    }

    /// The weights of groups 0, 1 and 2.
    pub fn weights(&self) -> [f32; 3] {
        self.weights
    }
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    fmath::sqrt((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64) as f32
}

/// The layer gains of a music stream: groups 0..2 from `mood`, group 3 from the nearer of the first two
/// characters, group 4 from the third (distances to the listener), groups 5..7 at full volume.
pub fn music_layer_gains(mood: &MusicMood, listener: &Listener, characters: &[Option<Vec3>; 3]) -> [f32; LAYERS] {
    let near =
        |c: Option<Vec3>| c.map_or(0.0, |p| attenuation(distance(p, listener.position), CHARACTER_FAR, CHARACTER_NEAR));
    let w = mood.weights();
    let mut g = [1.0; LAYERS];
    g[..3].copy_from_slice(&w);
    g[3] = near(characters[0]).max(near(characters[1]));
    g[4] = near(characters[2]);
    g
}

/// One distance fall-off on a resident-stream channel (an ambient-sound object's option 100 record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    /// The channel number it names (from the world's sound section).
    pub channel: i16,
    /// The object's position.
    pub position: Vec3,
    /// Full volume within this distance.
    pub start: f32,
    /// Silent beyond this distance.
    pub end: f32,
    /// Volume.
    pub volume: f32,
}

impl Falloff {
    /// Volume at the listener: `volume` within `start`, 0 beyond `end`, linear between.
    pub fn gain(&self, listener: &Listener) -> f32 {
        let d = distance(self.position, listener.position);
        self.volume * attenuation(d, self.end, self.start)
    }

    /// Pan in degrees toward the object (the Sly 1 rule of audio.md 3.6).
    pub fn pan(&self, listener: &Listener) -> f32 {
        let rel = [
            self.position[0] - listener.position[0],
            self.position[1] - listener.position[1],
            self.position[2] - listener.position[2],
        ];
        let d = distance(self.position, listener.position);
        if d <= 0.0 {
            return 0.0;
        }
        let dot = (rel[0] * listener.left[0] + rel[1] * listener.left[1] + rel[2] * listener.left[2]) / d;
        position_pan(attenuation(d, self.end, self.start), dot)
    }
}

/// Gain and pan per channel of a resident stream with channel numbers `numbers`.
pub fn resident_channels(
    numbers: &[i16],
    falloffs: &[Falloff],
    listener: &Listener,
) -> ([f32; crate::sly2::vpk::MAX_CHANNELS], [f32; crate::sly2::vpk::MAX_CHANNELS]) {
    let mut gains = [1.0; crate::sly2::vpk::MAX_CHANNELS];
    let mut pans = [0.0; crate::sly2::vpk::MAX_CHANNELS];
    for (c, &n) in numbers.iter().enumerate().take(gains.len()) {
        let mut count = 0;
        let mut loudest = 0.0f32;
        let mut only = None;
        for f in falloffs.iter().filter(|f| f.channel == n) {
            count += 1;
            loudest = loudest.max(f.gain(listener));
            only = Some(f);
        }
        if count > 0 {
            gains[c] = loudest;
            if count == 1
                && let Some(f) = only
            {
                pans[c] = f.pan(listener);
            }
        }
    }
    (gains, pans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_cross_fade_with_equal_power() {
        let mut m = MusicMood::default();
        assert_eq!(m.weights(), [0.0, 1.0, 0.0]);
        m.alert = 1;
        for _ in 0..10 {
            m.update(1.0 / 60.0);
            let w = m.weights();
            let sum: f32 = w.iter().map(|x| x * x).sum();
            assert!((sum - 1.0).abs() < 1e-4, "{w:?}");
        }
        assert!(m.weights()[2] > 0.1 && m.weights()[2] < 1.0);
        for _ in 0..120 {
            m.update(1.0 / 60.0);
        }
        assert!(m.weights()[2] > 0.99 && m.weights()[1] < 0.15, "{:?}", m.weights());
        // The sneak counter loses to the alert counter, then wins over the character state.
        m.sneak = 1;
        assert_eq!(m.chosen(), 2);
        m.alert = 0;
        assert_eq!(m.chosen(), 0);
        m.sneak = 0;
        m.state = CharacterMusicState::Stealth;
        assert_eq!(m.chosen(), 0);
        // Sneak fades in faster (fast speed 3).
        let mut a = MusicMood { state: CharacterMusicState::Stealth, ..MusicMood::default() };
        let mut b = MusicMood { state: CharacterMusicState::Alert, ..MusicMood::default() };
        a.update(0.05);
        b.update(0.05);
        assert!(a.weights()[0] > b.weights()[2]);
    }

    #[test]
    fn character_and_resident_distances() {
        let l = Listener::default();
        let mood = MusicMood::default();
        let g = music_layer_gains(&mood, &l, &[Some([1000.0, 0.0, 0.0]), Some([4000.0, 0.0, 0.0]), None]);
        assert_eq!(g[3], 1.0, "nearest of the first two characters");
        assert_eq!(g[4], 0.0, "no third character");
        let g = music_layer_gains(&mood, &l, &[Some([3500.0, 0.0, 0.0]), None, Some([6000.0, 0.0, 0.0])]);
        assert!((g[3] - 0.5).abs() < 1e-6);
        assert_eq!((g[4], g[5]), (0.0, 1.0));

        let f = |channel, x: f32, start, end, volume| Falloff { channel, position: [x, 0.0, 0.0], start, end, volume };
        let falloffs = [
            f(28, 2250.0, 500.0, 4000.0, 1.0),
            f(39, 100.0, 6000.0, 10_000.0, 1.0),
            f(39, 9000.0, 6000.0, 10_000.0, 0.8),
        ];
        let (gains, pans) = resident_channels(&[28, 39, 46], &falloffs, &l);
        assert!((gains[0] - 0.5).abs() < 1e-6);
        assert!(pans[0] != 0.0, "one fall-off: panned toward it");
        assert_eq!((gains[1], pans[1]), (1.0, 0.0), "loudest of two, centred");
        assert_eq!(gains[2], 1.0, "no fall-off: stored volume");
    }
}
