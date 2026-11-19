//! Positioned sounds (audio.md 3.6): distance attenuation, pan from position, Doppler.

use crate::fmath;

/// Three floats: x, y, z in game units (game space, Z up).
pub type Vec3 = [f32; 3];

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The listener: the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    /// Position.
    pub position: Vec3,
    /// Velocity in game units per second (for Doppler).
    pub velocity: Vec3,
    /// The camera's left axis (unit length).
    pub left: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Listener { position: [0.0; 3], velocity: [0.0; 3], left: [-1.0, 0.0, 0.0] }
    }
}

/// Where and how a sound is played.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    /// World position, or `None` for an unpositioned (2D) sound.
    pub position: Option<Vec3>,
    /// Velocity in game units per second (for Doppler).
    pub velocity: Vec3,
    /// Beyond this distance the sound is silent (`S`).
    pub start_distance: f32,
    /// Within this distance the sound is at full volume (`F`).
    pub full_distance: f32,
    /// Doppler factor (0 = none).
    pub doppler: f32,
    /// Game volume for the sound, 0..1 (before distance).
    pub volume: f32,
    /// Pitch offset in semitones (engines, sliding).
    pub pitch: f32,
    /// Extra pan in degrees.
    pub pan: f32,
}

impl Emitter {
    /// An unpositioned sound at full volume.
    pub const fn global() -> Emitter {
        Emitter {
            position: None,
            velocity: [0.0; 3],
            start_distance: 0.0,
            full_distance: 0.0,
            doppler: 0.0,
            volume: 1.0,
            pitch: 0.0,
            pan: 0.0,
        }
    }

    /// A sound at `position`, silent beyond `start_distance` and at full volume within `full_distance`.
    pub const fn at(position: Vec3, start_distance: f32, full_distance: f32) -> Emitter {
        Emitter { position: Some(position), start_distance, full_distance, ..Emitter::global() }
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Emitter::global()
    }
}

/// What the game side tells the mixer about one playing sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    /// Game volume for the sound including distance, 0..1 (the square law is applied later, with the sound's own
    /// volumes).
    pub gain: f32,
    /// Pan in degrees, added to the sound's and voice's own pans.
    pub pan: f32,
    /// Pitch offset in semitones (game offset plus Doppler).
    pub pitch: f32,
}

impl Mix {
    /// Full volume, centred, no pitch change.
    pub const UNITY: Mix = Mix { gain: 1.0, pan: 0.0, pitch: 0.0 };
}

/// Distance factor: 1 within `full`, 0 beyond `start`, linear between.
pub fn attenuation(d: f32, start: f32, full: f32) -> f32 {
    if d > start {
        0.0
    } else if d > full && start > full {
        (start - d) / (start - full)
    } else {
        1.0
    }
}

/// Pan from position (audio.md 3.6), in degrees: `p = -0.7 (1 - att) (dir . left)`, then `(p + 2) * 179.5`, minus
/// 359 when `p > 0`.
pub fn position_pan(att: f32, dir_dot_left: f32) -> f32 {
    let p = -0.7 * (1.0 - att) * dir_dot_left;
    let deg = (p + 2.0) * 179.5;
    if p > 0.0 { deg - 359.0 } else { deg }
}

/// Doppler shift in semitones for an approach speed `v` (game units per second, positive when closing in):
/// `floor(v * factor * 0.0225) * 1524 / 741 / 128`, the middle division in integers.
pub fn doppler_semitones(v: f32, factor: f32) -> f32 {
    let q = fmath::floor((v * factor * 0.0225) as f64) as i64;
    (q * 1524 / 741) as f32 / 128.0
}

/// The mix for `emitter` heard by `listener`, or `None` when it is out of range (beyond the start distance).
pub fn mix_for(listener: &Listener, emitter: &Emitter) -> Option<Mix> {
    let Some(pos) = emitter.position else {
        return Some(Mix { gain: emitter.volume, pan: emitter.pan, pitch: emitter.pitch });
    };
    let rel = sub(pos, listener.position);
    let d = fmath::sqrt(dot(rel, rel) as f64) as f32;
    if d > emitter.start_distance {
        return None;
    }
    let att = attenuation(d, emitter.start_distance, emitter.full_distance);
    let dir = if d > 0.0 { [rel[0] / d, rel[1] / d, rel[2] / d] } else { [0.0; 3] };
    let pan = position_pan(att, dot(dir, listener.left));
    let mut pitch = emitter.pitch;
    if emitter.doppler != 0.0 {
        // Speed of approach: how fast the distance shrinks.
        let closing = -dot(sub(emitter.velocity, listener.velocity), dir);
        pitch += doppler_semitones(closing, emitter.doppler);
    }
    Some(Mix { gain: emitter.volume * att, pan: pan + emitter.pan, pitch })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuation_ranges() {
        assert_eq!(attenuation(5.0, 100.0, 10.0), 1.0);
        assert_eq!(attenuation(55.0, 100.0, 10.0), 0.5);
        assert_eq!(attenuation(101.0, 100.0, 10.0), 0.0);
        assert_eq!(attenuation(10.0, 10.0, 10.0), 1.0);
    }

    #[test]
    fn pan_rule() {
        // Full volume: p = 0 -> 359 degrees (just left of centre, as the rule gives).
        assert_eq!(position_pan(1.0, 1.0), 359.0);
        // Far away on the right (direction opposite the left axis): p = +0.7 * (1 - att).
        let deg = position_pan(0.0, -1.0);
        assert!((deg - (2.7 * 179.5 - 359.0)).abs() < 1e-3);
        // Far away on the left: p = -0.7.
        let deg = position_pan(0.0, 1.0);
        assert!((deg - 1.3 * 179.5).abs() < 1e-3);
        assert!(fmath::sin_deg(deg as f64) < 0.0);
    }

    #[test]
    fn doppler_rule() {
        assert_eq!(doppler_semitones(0.0, 1.0), 0.0);
        // 1000 units/s, factor 1: floor(22.5) = 22 -> 22 * 1524 / 741 = 45 -> 45 / 128.
        assert!((doppler_semitones(1000.0, 1.0) - 45.0 / 128.0).abs() < 1e-6);
        assert!(doppler_semitones(-1000.0, 1.0) < 0.0);
    }

    #[test]
    fn mix_for_positions() {
        let l = Listener::default();
        let mut e = Emitter::at([50.0, 0.0, 0.0], 100.0, 10.0);
        let m = mix_for(&l, &e).unwrap();
        assert!((m.gain - (50.0 / 90.0)).abs() < 1e-5);
        // +x is to the right of a listener whose left axis is -x.
        assert!(fmath::sin_deg(m.pan as f64) > 0.0);
        e.position = Some([0.0, 200.0, 0.0]);
        assert!(mix_for(&l, &e).is_none());
        assert_eq!(mix_for(&l, &Emitter::global()), Some(Mix::UNITY));
        let mut moving = Emitter::at([100.0, 0.0, 0.0], 1000.0, 10.0);
        moving.velocity = [-1000.0, 0.0, 0.0];
        moving.doppler = 1.0;
        assert!(mix_for(&l, &moving).unwrap().pitch > 0.0);
    }
}
