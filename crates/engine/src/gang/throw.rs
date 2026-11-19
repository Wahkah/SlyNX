//! Throw aiming, shared by the three characters (characters.md 7).

use alloc::vec::Vec;

use super::consts::spec;
use crate::collision::world::{CollisionWorld, QueryFilter};
use crate::math::{Vec3, add, cos, length, madd, sin, sqrt, sub};

/// The aim state of a throw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrowAim {
    /// Aim distance from the thrower.
    pub distance: f32,
    /// Aim heading (radians).
    pub yaw: f32,
    /// Range (min, max) of the object, already × 1.5 with Long Toss.
    pub range: [f32; 2],
    /// Predicted flight time, already × 1.35 with Long Toss.
    pub flight_time: f32,
}

/// A predicted flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// Launch velocity.
    pub velocity: Vec3,
    /// Launch elevation (radians).
    pub elevation: f32,
    /// The predicted points (up to 29 steps after the start).
    pub points: Vec<Vec3>,
    /// The first surface hit, if any.
    pub hit: Option<Vec3>,
}

impl ThrowAim {
    /// Starts an aim with the object's range and flight time (Long Toss multiplies them by 1.5 and 1.35).
    pub fn new(range: [f32; 2], flight_time: f32, yaw: f32, long_toss: bool) -> ThrowAim {
        let [rf, tf] = if long_toss { spec::LONG_TOSS } else { [1.0, 1.0] };
        let range = [range[0] * rf, range[1] * rf];
        ThrowAim { distance: (range[0] + range[1]) * 0.5, yaw, range, flight_time: flight_time * tf }
    }

    /// Moves the aim with the stick: distance by the stick's forward axis at `speed` per second, heading toward
    /// the stick's sideways axis at most 2 rad/s (the aim blend's turn limit).
    pub fn steer(&mut self, stick: [f32; 2], speed: f32, dt: f32) {
        self.distance = (self.distance + stick[1] * speed * dt).clamp(self.range[0], self.range[1]);
        self.yaw -= (stick[0] * spec::THROW_TURN[0]).clamp(-spec::THROW_TURN[0], spec::THROW_TURN[0]) * dt;
    }

    /// The fraction of the range the aim is at.
    pub fn far(&self) -> f32 {
        let span = self.range[1] - self.range[0];
        if span <= 0.0 { 1.0 } else { ((self.distance - self.range[0]) / span).clamp(0.0, 1.0) }
    }

    /// The launch elevation: `0.15 + 0.55 × far`.
    pub fn elevation(&self) -> f32 {
        spec::THROW_ELEVATION[0] + spec::THROW_ELEVATION[1] * self.far()
    }

    /// The launch velocity at the elevation that lands at the aim distance on level ground, and the predicted
    /// flight over the flight time in 29 steps, stopping at the first surface hit.
    pub fn predict(&self, from: Vec3, gravity: f32, cw: Option<&CollisionWorld>, filter: &QueryFilter) -> Flight {
        let e = self.elevation();
        let (se, ce) = (sin(e), cos(e));
        let g = -gravity;
        // Range on level ground: d = v² sin(2e) / g.
        let v = sqrt((self.distance * g / (2.0 * se * ce)).max(0.0));
        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let velocity = [v * ce * cy, v * ce * sy, v * se];
        let mut points = Vec::with_capacity(spec::THROW_STEPS);
        let mut prev = from;
        let mut hit = None;
        let dt = self.flight_time / spec::THROW_STEPS as f32;
        for i in 1..=spec::THROW_STEPS {
            let t = dt * i as f32;
            let p = add(madd(from, velocity, t), [0.0, 0.0, -0.5 * g * t * t]);
            if let Some(cw) = cw {
                let d = sub(p, prev);
                let len = length(d);
                if len > 0.0
                    && let Some(h) = cw.ray_cast(prev, d, len, filter)
                {
                    hit = Some(h.point);
                    points.push(h.point);
                    break;
                }
            }
            points.push(p);
            prev = p;
        }
        Flight { velocity, elevation: e, points, hit }
    }
}

/// Turns `from` toward `to` by at most `rate × dt` (the body's 3 rad/s while aiming).
pub fn turn_toward(from: f32, to: f32, rate: f32, dt: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    let d = crate::math::rem_euclid(to - from + core::f32::consts::PI, tau) - core::f32::consts::PI;
    from + d.clamp(-rate * dt, rate * dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_long_toss_and_landing() {
        let a = ThrowAim::new([200.0, 1000.0], 1.0, 0.0, false);
        let b = ThrowAim::new([200.0, 1000.0], 1.0, 0.0, true);
        assert_eq!(b.range, [300.0, 1500.0]);
        assert!((b.flight_time - 1.35).abs() < 1e-6);
        let mut a = a;
        a.distance = 200.0;
        assert!((a.elevation() - 0.15).abs() < 1e-6);
        a.distance = 1000.0;
        assert!((a.elevation() - 0.70).abs() < 1e-6);
        // The predicted flight comes back to the launch height at the aim distance.
        a.distance = 600.0;
        let f = a.predict([0.0; 3], -2275.0, None, &QueryFilter::default());
        assert_eq!(f.points.len(), 29);
        let v = f.velocity;
        let t_land = 2.0 * v[2] / 2275.0;
        assert!((v[0] * t_land - 600.0).abs() < 0.5);
        a.steer([0.0, 1.0], 800.0, 10.0);
        assert_eq!(a.distance, 1000.0);
        assert!((turn_toward(0.0, 3.0, 3.0, 0.1) - 0.3).abs() < 1e-6);
    }
}
