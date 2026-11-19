//! Framing: where the camera stands for a focus point, in the default frame or a zone's (camera.md 3).
//!
//! A frame has three axes: forward (x), left (y), up (z). The focus is expressed in the frame; each component is
//! mapped by its offset function; then the camera steps the framing distance along the frame's forward axis
//! (negative: behind). A cylindrical frame uses (angle, radius, height) around its up axis instead.

use alloc::vec::Vec;

use crate::math::{Vec3, add, atan2, cos, cross, dot, length, madd, normalize, scale, sin, sqrt, sub};

/// `clamp(c0 + c1·x + c2·x², min, max)` (options 852, 854, 856 for the coefficients).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffsetFn {
    /// Coefficients.
    pub c: [f32; 3],
    /// Lower limit.
    pub min: f32,
    /// Upper limit.
    pub max: f32,
}

impl OffsetFn {
    /// The identity, unlimited.
    pub const IDENTITY: OffsetFn = OffsetFn { c: [0.0, 1.0, 0.0], min: -f32::MAX, max: f32::MAX };

    /// The value at `x`.
    pub fn eval(&self, x: f32) -> f32 {
        (self.c[0] + self.c[1] * x + self.c[2] * x * x).max(self.min).min(self.max)
    }
}

/// Allowed framing directions (option 867).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Directions(pub i16);

impl Directions {
    /// Forward framing allowed (bit 0).
    pub fn forward(self) -> bool {
        self.0 == 0 || self.0 & 1 != 0
    }
    /// Reverse framing allowed (bit 1).
    pub fn reverse(self) -> bool {
        self.0 & 2 != 0
    }
    /// Manual framing allowed (bit 2, or no bits set).
    pub fn manual(self) -> bool {
        self.0 == 0 || self.0 & 4 != 0
    }
}

/// A zone's framing settings (camera.md 3; options 851 to 871). [`Framing::DEFAULT`] is the default framing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// Distance along the forward axis (851; −450: behind).
    pub distance: f32,
    /// Offset functions for x (forward), y (left), z (up) (852, 854, 856).
    pub offsets: [OffsetFn; 3],
    /// Preferred screen position of the focus (859, 860).
    pub screen: [f32; 2],
    /// How much the focus follows the player's adjusted point (861).
    pub adjust: f32,
    /// Automatic-follow strength (862).
    pub follow: f32,
    /// Lock-on strength (863).
    pub lock_on: f32,
    /// Focus scale (864 ratio, 865 vector).
    pub focus_scale: Vec3,
    /// Cylindrical framing (866).
    pub cylindrical: bool,
    /// Slack angle of a cylindrical framing (858).
    pub slack: f32,
    /// Allowed directions (867).
    pub directions: Directions,
    /// Pan on entry (868) and pan when idle (869).
    pub pan: [bool; 2],
    /// Pan on entry: ramp time and final strength (873, 874; defaults 2 s and 1).
    pub pan_entry: [f32; 2],
    /// Pan when idle: ramp time and final strength (875, 876; defaults 2 s and 1).
    pub pan_idle: [f32; 2],
    /// Entering the zone cuts the camera (877).
    pub cut_on_entry: bool,
    /// Field of view while in the zone (878).
    pub fov: Option<f32>,
    /// No view clearing in this zone (879).
    pub no_clearing: bool,
    /// Follow target object (880), set by scripts.
    pub follow_target: Option<i16>,
    /// Speed of the motion toward the follow target as a quadratic of the distance, with its limits (881, 882).
    pub follow_speed: OffsetFn,
    /// State override (884): forces the zone in or out regardless of its volume.
    pub state_override: Option<bool>,
    /// Eye spring strength and damping factors (885, 886).
    pub eye_spring: [f32; 2],
    /// Sideways motion for the automatic orbit is measured along the view ("side", 872) instead of across it.
    pub side_follow: bool,
}

impl Framing {
    /// The default framing (camera.md 3, [X 0x275980]).
    pub const DEFAULT: Framing = Framing {
        distance: -450.0,
        offsets: [
            OffsetFn::IDENTITY,
            OffsetFn::IDENTITY,
            OffsetFn { c: [125.0, 1.0, 0.0], min: -f32::MAX, max: f32::MAX },
        ],
        screen: [0.0, -0.1],
        adjust: 1.0,
        follow: 1.0,
        lock_on: 1.0,
        focus_scale: [1.0, 1.0, 1.0],
        cylindrical: false,
        slack: 0.0,
        directions: Directions(0),
        pan: [false, false],
        pan_entry: [2.0, 1.0],
        pan_idle: [2.0, 1.0],
        cut_on_entry: false,
        fov: None,
        no_clearing: false,
        follow_target: None,
        follow_speed: OffsetFn { c: [0.0, 1.0, 0.0], min: 0.0, max: f32::MAX },
        state_override: None,
        eye_spring: [1.0, 1.0],
        side_follow: false,
    };
}

/// A frame: origin and axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Origin.
    pub origin: Vec3,
    /// Forward (x).
    pub forward: Vec3,
    /// Left (y).
    pub left: Vec3,
    /// Up (z).
    pub up: Vec3,
}

impl Frame {
    /// A level frame at `origin` facing heading `yaw`.
    pub fn level(origin: Vec3, yaw: f32) -> Frame {
        let forward = [cos(yaw), sin(yaw), 0.0];
        Frame { origin, forward, left: cross([0.0, 0.0, 1.0], forward), up: [0.0, 0.0, 1.0] }
    }

    /// From an affine transform: columns forward, left, up (the camera basis of transforms.md 7).
    pub fn from_affine(a: &formats::geometry::Affine) -> Frame {
        Frame { origin: a.t, forward: normalize(a.cols[0]), left: normalize(a.cols[1]), up: normalize(a.cols[2]) }
    }

    /// A point in frame coordinates.
    pub fn to_local(&self, p: Vec3) -> Vec3 {
        let d = sub(p, self.origin);
        [dot(d, self.forward), dot(d, self.left), dot(d, self.up)]
    }

    /// Frame coordinates back to world.
    pub fn to_world(&self, c: Vec3) -> Vec3 {
        add(add(madd(self.origin, self.forward, c[0]), scale(self.left, c[1])), scale(self.up, c[2]))
    }

    /// The same frame turned half a turn about its up axis (reverse framing).
    pub fn reversed(&self) -> Frame {
        Frame { forward: scale(self.forward, -1.0), left: scale(self.left, -1.0), ..*self }
    }
}

/// Where a zone's frame comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum ZoneFrame {
    /// A fixed frame (path kind −1 and its stored transform).
    Fixed(Frame),
    /// A path: the frame sits at the point of the polyline closest to the focus, forward along the path.
    Path(Vec<Vec3>),
}

impl ZoneFrame {
    /// The frame for a focus point.
    pub fn frame_for(&self, focus: Vec3) -> Frame {
        match self {
            ZoneFrame::Fixed(f) => *f,
            ZoneFrame::Path(points) => {
                let mut best = (f32::INFINITY, points.first().copied().unwrap_or(focus), [1.0, 0.0, 0.0]);
                for w in points.windows(2) {
                    let d = sub(w[1], w[0]);
                    let l2 = dot(d, d);
                    let t = if l2 > 0.0 { (dot(sub(focus, w[0]), d) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    let q = madd(w[0], d, t);
                    let dist = length(sub(q, focus));
                    if dist < best.0 {
                        best = (dist, q, d);
                    }
                }
                let f = normalize([best.2[0], best.2[1], 0.0]);
                let forward = if f == [0.0; 3] { [1.0, 0.0, 0.0] } else { f };
                Frame { origin: best.1, forward, left: cross([0.0, 0.0, 1.0], forward), up: [0.0, 0.0, 1.0] }
            }
        }
    }
}

/// The eye position for `focus` in `frame` with `framing`. For a cylindrical framing, `angle` carries the
/// camera's angle between frames (the slack: it follows the focus only when they differ by more than the slack).
pub fn framed_eye(framing: &Framing, frame: &Frame, focus: Vec3, angle: &mut Option<f32>) -> Vec3 {
    let c = frame.to_local(focus);
    let o = &framing.offsets;
    if framing.cylindrical {
        let theta = atan2(c[1], c[0]);
        let radius = sqrt(c[0] * c[0] + c[1] * c[1]);
        let a = match *angle {
            Some(prev) => {
                let d = wrap(theta - prev);
                if d.abs() > framing.slack { prev + d - framing.slack * d.signum() } else { prev }
            }
            None => theta,
        };
        *angle = Some(a);
        let (t, r, h) = (o[0].eval(a), o[1].eval(radius) - framing.distance, o[2].eval(c[2]));
        frame.to_world([r * cos(t), r * sin(t), h])
    } else {
        let local = [o[0].eval(c[0]) + framing.distance, o[1].eval(c[1]), o[2].eval(c[2])];
        frame.to_world(local)
    }
}

fn wrap(a: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    crate::math::rem_euclid(a + core::f32::consts::PI, tau) - core::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn default_framing_is_450_behind_125_above() {
        let f = Frame::level([10.0, 20.0, 30.0], 0.0);
        let eye = framed_eye(&Framing::DEFAULT, &f, [10.0, 20.0, 30.0], &mut None);
        assert!(close(eye, [-440.0, 20.0, 155.0]), "{eye:?}");
    }

    #[test]
    fn zone_frames_paths_and_cylinders() {
        // A fixed frame looking along +y; the camera keeps x within [−100, 100] and stays 200 back.
        let frame = Frame::level([0.0; 3], core::f32::consts::FRAC_PI_2);
        let framing = Framing {
            distance: -200.0,
            offsets: [OffsetFn::IDENTITY, OffsetFn { c: [0.0, 1.0, 0.0], min: -100.0, max: 100.0 }, OffsetFn::IDENTITY],
            ..Framing::DEFAULT
        };
        // Focus 300 to the left of the frame (x = −300 in world): clamped to 100.
        let eye = framed_eye(&framing, &frame, [-300.0, 50.0, 0.0], &mut None);
        assert!(close(eye, [-100.0, -150.0, 0.0]), "{eye:?}");
        // Reverse framing steps the other way.
        let eye = framed_eye(&framing, &frame.reversed(), [0.0, 50.0, 0.0], &mut None);
        assert!(close(eye, [0.0, 250.0, 0.0]), "{eye:?}");
        // A path: the frame follows the closest point, forward along the path.
        let path = ZoneFrame::Path(alloc::vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]]);
        let fr = path.frame_for([400.0, 30.0, 10.0]);
        assert!(close(fr.origin, [400.0, 0.0, 0.0]) && close(fr.forward, [1.0, 0.0, 0.0]));
        let eye = framed_eye(&Framing::DEFAULT, &fr, [400.0, 30.0, 10.0], &mut None);
        assert!(close(eye, [-50.0, 30.0, 135.0]), "{eye:?}");
        // A cylinder: the camera stays outside the focus on the same angle, 450 farther out; with slack it lags.
        let cyl = Framing { cylindrical: true, slack: 0.5, offsets: [OffsetFn::IDENTITY; 3], ..Framing::DEFAULT };
        let f0 = Frame::level([0.0; 3], 0.0);
        let mut angle = None;
        let eye = framed_eye(&cyl, &f0, [100.0, 0.0, 0.0], &mut angle);
        assert!(close(eye, [550.0, 0.0, 0.0]), "{eye:?}");
        let eye = framed_eye(&cyl, &f0, [100.0 * cos(0.3), 100.0 * sin(0.3), 0.0], &mut angle);
        assert!(close(eye, [550.0, 0.0, 0.0]), "{eye:?}");
        framed_eye(&cyl, &f0, [100.0 * cos(1.0), 100.0 * sin(1.0), 0.0], &mut angle);
        assert!((angle.unwrap() - 0.5).abs() < 1e-5);
        assert!(Directions(0).forward() && Directions(0).manual() && !Directions(0).reverse());
    }
}
