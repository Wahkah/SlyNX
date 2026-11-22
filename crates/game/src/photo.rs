//! Taking pictures with the binocucom, shared by the Sly 2 and Sly 3 sessions: photo targets (Sly 2 class 46, Sly 3
//! class 43; specs/sly3/interactables.md 5a, Sly 2's picture-taking rules). Each game gives its own option numbers
//! ([`Options`]) and decides which targets are eligible; this module does the rest.
//!
//! - **Picking:** of the eligible targets inside the view (a sphere of the target's radius), the one nearest
//!   horizontally to the eye. The search starts from the largest float, so there is no distance limit.
//! - **Framing** on the game's screen with the reticle at its centre ([`Params`]; Sly 2: 640 × 492.8, Sly 3:
//!   640 × 480):
//!   - centring = clamp(1 − (d − 128) / 128, 0, 1), d being the target's screen distance from the reticle;
//!   - size = the screen length of (target, target + camera up × size) × 1/120, folded to 2 − size above 1;
//!   - a good shot has full centring and a size of at least 1 − tolerance.
//! - **Prompt states:** 2 no target, 4 not centred, 5 too small, 6 too big, 7 ready, 8 shutter, 9 accepted.
//! - **The shutter:** pad mask 0x0A (R1, R2: R and ZR here), 0xFA when ready (the face buttons too), enters 8. After
//!   the flash the shot is judged: good and not "not yet" is 9, otherwise back to framing. Leaving 9 hands the
//!   accepted shot to the session (the target's line, a checked request for state 2 on its task, a task event).
//!
//! The view is the character's eye ([`Params::eye_height`] up) looking along the camera's heading, with the camera's
//! field of view, aimed by the left stick. Working values (not read from the games): the aim speed, the flash and
//! accept times. Not done: the four line-of-sight rays, the clock freeze during the flash, the title overlay.

use alloc::vec::Vec;

use engine::game::systems::FrameInput;
use engine::math::{cos, sin, sqrt};
use engine::object::Handle;
use engine::world::World;
use platform_api::Buttons;

/// A game's photo constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The screen.
    pub screen: [f32; 2],
    /// The reticle.
    pub reticle: [f32; 2],
    /// The eye above the character.
    pub eye_height: f32,
    /// The vertical field of view, radians.
    pub fov_y: f32,
}

/// Centred within this many screen units of the reticle.
pub const CENTRED: f32 = 128.0;
/// The screen length a target's size should span (the size is scaled by its inverse).
pub const FRAME: f32 = 120.0;
/// Most targets gathered.
pub const MAX_TARGETS: usize = 16;
/// Working values (module documentation): the aim speed at full stick, radians per second.
pub const AIM_SPEED: f32 = 1.0;
/// See [`AIM_SPEED`]: how long the flash (state 8) and the acceptance (state 9) last, seconds.
pub const FLASH_TIME: f32 = 0.3;
/// See [`FLASH_TIME`].
pub const ACCEPT_TIME: f32 = 0.5;
/// Defaults of the target options.
pub const RADIUS_DEFAULT: f32 = 75.0;
/// See [`RADIUS_DEFAULT`].
pub const TOLERANCE_DEFAULT: f32 = 0.25;

/// Prompt states.
pub mod state {
    pub const NO_TARGET: u8 = 2;
    pub const NOT_CENTRED: u8 = 4;
    pub const TOO_SMALL: u8 = 5;
    pub const TOO_BIG: u8 = 6;
    pub const READY: u8 = 7;
    pub const SHUTTER: u8 = 8;
    pub const ACCEPTED: u8 = 9;
}

/// A game's option numbers for its photo targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub radius: u16,
    pub size: u16,
    pub tolerance: u16,
    pub list_title: u16,
    pub line: u16,
    pub task: u16,
    pub not_yet: u16,
}

/// A photo target's options.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub handle: Handle,
    pub radius: f32,
    pub size: f32,
    pub tolerance: f32,
    /// In the target list (list title other than −1).
    pub listed: bool,
    pub line: Option<i32>,
    pub task: Option<i32>,
    pub not_yet: bool,
}

impl Target {
    /// From the target's options, with the game's option numbers.
    pub fn new(world: &World, h: Handle, ids: &Options) -> Target {
        let o = world.options(h);
        let f = |id: u16| o.and_then(|o| o.f32(id));
        let i = |id: u16| o.and_then(|o| o.i32(id));
        let radius = f(ids.radius).unwrap_or(RADIUS_DEFAULT);
        Target {
            handle: h,
            radius,
            size: f(ids.size).filter(|&s| s > 0.0).unwrap_or(radius),
            tolerance: f(ids.tolerance).unwrap_or(TOLERANCE_DEFAULT),
            listed: i(ids.list_title).is_none_or(|t| t != -1),
            line: i(ids.line).filter(|&l| l >= 0),
            task: i(ids.task).filter(|&t| t >= 0),
            not_yet: i(ids.not_yet).is_some_and(|v| v != 0),
        }
    }
}

/// How a target sits in the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// Screen position.
    pub at: [f32; 2],
    /// 0 to 1: 1 within [`CENTRED`] of the reticle.
    pub centring: f32,
    /// The size match, folded (module documentation).
    pub size: f32,
    /// The size before folding (above 1: too big).
    pub raw_size: f32,
}

/// `x` squared (`f32::powi` is not in `core`).
fn sq(x: f32) -> f32 {
    x * x
}

/// The view: eye, yaw and pitch, with the game's constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub params: Params,
}

impl View {
    /// The forward, right and up axes.
    pub fn axes(&self) -> [[f32; 3]; 3] {
        let (cy, sy, cp, sp) = (cos(self.yaw), sin(self.yaw), cos(self.pitch), sin(self.pitch));
        let fwd = [cy * cp, sy * cp, sp];
        let right = [sy, -cy, 0.0];
        let up = [-cy * sp, -sy * sp, cp];
        [fwd, right, up]
    }

    /// A world point in camera space: (right, up, forward).
    fn local(&self, p: [f32; 3]) -> [f32; 3] {
        let d = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let [f, r, u] = self.axes();
        let dot = |a: [f32; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        [dot(r), dot(u), dot(f)]
    }

    /// The focal length in screen units: half the screen's height over tan(fov_y / 2).
    pub fn focal(&self) -> f32 {
        let h = self.params.fov_y / 2.0;
        self.params.screen[1] / 2.0 * cos(h) / sin(h)
    }

    /// Whether a sphere is inside the view's side planes and in front of the eye.
    pub fn sees(&self, centre: [f32; 3], radius: f32) -> bool {
        let [x, y, z] = self.local(centre);
        if z + radius <= 0.0 {
            return false;
        }
        let f = self.focal();
        let (screen, reticle) = (self.params.screen, self.params.reticle);
        let half = [reticle[0], screen[1] - reticle[1]];
        [(x, half[0]), (y, half[1])].iter().all(|&(v, h)| (v.abs() * f - z * h) / sqrt(f * f + h * h) <= radius)
    }

    /// The screen position of a world point in front of the eye.
    pub fn project(&self, p: [f32; 3]) -> Option<[f32; 2]> {
        let [x, y, z] = self.local(p);
        (z > 1.0).then(|| {
            let (f, r) = (self.focal(), self.params.reticle);
            [r[0] + f * x / z, r[1] - f * y / z]
        })
    }

    /// How a target at `p` whose size is `size` is framed.
    pub fn frame(&self, p: [f32; 3], size: f32) -> Option<Framing> {
        let at = self.project(p)?;
        let [_, _, up] = self.axes();
        let top = self.project([p[0] + up[0] * size, p[1] + up[1] * size, p[2] + up[2] * size])?;
        let r = self.params.reticle;
        let d = sqrt(sq(at[0] - r[0]) + sq(at[1] - r[1]));
        let raw = sqrt(sq(top[0] - at[0]) + sq(top[1] - at[1])) * (1.0 / FRAME);
        let size = if raw > 1.0 { 2.0 - raw } else { raw };
        Some(Framing { at, centring: (1.0 - (d - CENTRED) / CENTRED).clamp(0.0, 1.0), size, raw_size: raw })
    }
}

/// A shot accepted, for the session to act on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Accepted {
    pub target: Handle,
    pub task: Option<i32>,
    pub line: Option<i32>,
}

/// The open binocucom.
#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    pub view: View,
    /// The prompt state.
    pub state: u8,
    /// The picked target and its framing.
    pub target: Option<(Target, Framing)>,
    /// Time in the shutter or accepted state.
    timer: f32,
    /// Targets whose shots were accepted.
    pub taken: Vec<Handle>,
}

impl Photo {
    /// Opens at the player's eye, looking along the camera's heading (a scripted view then turns it with
    /// [`Photo::look_at`]).
    pub fn open(world: &World, params: Params) -> Option<Photo> {
        let p = world.position(world.player?)?;
        let eye = [p[0], p[1], p[2] + params.eye_height];
        let view = View { eye, yaw: world.shared.camera_yaw, pitch: 0.0, params };
        Some(Photo { view, state: state::NO_TARGET, target: None, timer: 0.0, taken: Vec::new() })
    }

    /// Turns the view toward a point.
    pub fn look_at(&mut self, p: [f32; 3]) {
        let e = self.view.eye;
        let (dx, dy, dz) = (p[0] - e[0], p[1] - e[1], p[2] - e[2]);
        self.view.yaw = engine::math::atan2(dy, dx);
        self.view.pitch = engine::math::atan2(dz, sqrt(dx * dx + dy * dy));
    }

    /// The target to frame among `eligible`: in view, the nearest horizontally (no distance limit).
    fn pick(&self, world: &World, eligible: &[Target]) -> Option<(Target, [f32; 3])> {
        let e = self.view.eye;
        eligible
            .iter()
            .take(MAX_TARGETS)
            .filter(|t| !self.taken.contains(&t.handle))
            .filter_map(|t| {
                let p = world.position(t.handle)?;
                let d = sqrt(sq(p[0] - e[0]) + sq(p[1] - e[1]));
                self.view.sees(p, t.radius).then(|| (d, t.clone(), p))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, t, p)| (t, p))
    }

    /// Whether a framing is a good shot.
    fn good(f: &Framing, t: &Target) -> bool {
        f.centring >= 1.0 && f.size >= 1.0 - t.tolerance
    }

    /// One frame with the pad and the game's eligible targets. Returns a shot accepted this frame.
    pub fn update(&mut self, input: &FrameInput, dt: f32, world: &World, eligible: &[Target]) -> Option<Accepted> {
        match self.state {
            state::SHUTTER => {
                self.timer += dt;
                if self.timer >= FLASH_TIME {
                    self.timer = 0.0;
                    let good = self.target.as_ref().is_some_and(|(t, f)| Photo::good(f, t) && !t.not_yet);
                    self.state = if good { state::ACCEPTED } else { state::NO_TARGET };
                }
                return None;
            }
            state::ACCEPTED => {
                self.timer += dt;
                if self.timer < ACCEPT_TIME {
                    return None;
                }
                self.timer = 0.0;
                self.state = state::NO_TARGET;
                let (t, _) = self.target.take()?;
                self.taken.push(t.handle);
                return Some(Accepted { target: t.handle, task: t.task, line: t.line });
            }
            _ => {}
        }
        // Aim with the stick.
        let [x, y] = input.left;
        self.view.yaw -= x * AIM_SPEED * dt;
        self.view.pitch = (self.view.pitch + y * AIM_SPEED * dt).clamp(-1.2, 1.2);
        self.target = self.pick(world, eligible).and_then(|(t, p)| self.view.frame(p, t.size).map(|f| (t, f)));
        self.state = match &self.target {
            None => state::NO_TARGET,
            Some((_, f)) if f.centring < 1.0 => state::NOT_CENTRED,
            Some((t, f)) if f.size < 1.0 - t.tolerance => {
                if f.raw_size > 1.0 {
                    state::TOO_BIG
                } else {
                    state::TOO_SMALL
                }
            }
            Some(_) => state::READY,
        };
        // Mask 0x0A (R1, R2), 0xFA when ready (the face buttons too).
        let face = Buttons::A.0 | Buttons::B.0 | Buttons::X.0 | Buttons::Y.0;
        let mask = Buttons::R.0 | Buttons::ZR.0 | if self.state == state::READY { face } else { 0 };
        if input.pressed & mask != 0 {
            self.state = state::SHUTTER;
            self.timer = 0.0;
        }
        None
    }

    /// Whether a listed target among `eligible` is still pending (the binocucom closes by itself otherwise).
    pub fn listed_pending(&self, eligible: &[Target]) -> bool {
        eligible.iter().any(|t| t.listed && !self.taken.contains(&t.handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sly 3's screen, with the reticle at its centre and a 60° vertical field of view.
    fn view() -> View {
        let params = Params { screen: [640.0, 480.0], reticle: [320.0, 240.0], eye_height: 20.0, fov_y: 1.047 };
        View { eye: [0.0; 3], yaw: 0.0, pitch: 0.0, params }
    }

    #[test]
    fn a_point_ahead_projects_on_the_reticle() {
        let v = view();
        let at = v.project([1000.0, 0.0, 0.0]).unwrap();
        assert!((at[0] - 320.0).abs() < 1e-3 && (at[1] - 240.0).abs() < 1e-3, "{at:?}");
        // Right of the view is right on the screen, above is up.
        let r = v.project([1000.0, -100.0, 50.0]).unwrap();
        assert!(r[0] > 320.0 && r[1] < 240.0, "{r:?}");
        assert!(v.project([-1000.0, 0.0, 0.0]).is_none(), "behind");
        // The top of the field of view is the screen's top edge.
        let h = 1.047f32 / 2.0;
        let top = v.project([1000.0, 0.0, 1000.0 * sin(h) / cos(h)]).unwrap();
        assert!(top[1].abs() < 0.5, "{top:?}");
    }

    #[test]
    fn size_folds_above_one() {
        let v = view();
        // At the distance where 300 units span exactly 120 screen units.
        let d = v.focal() * 300.0 / FRAME;
        let f = v.frame([d, 0.0, 0.0], 300.0).unwrap();
        assert!((f.size - 1.0).abs() < 1e-3 && f.centring == 1.0, "{f:?}");
        let near = v.frame([d / 2.0, 0.0, 0.0], 300.0).unwrap();
        assert!(near.raw_size > 1.9 && near.size < 0.1, "too close counts as too small: {near:?}");
        assert!(v.sees([d, 0.0, 0.0], 10.0) && !v.sees([0.0, d, 0.0], 10.0));
    }

    #[test]
    fn centring_falls_off_past_128_units() {
        let v = view();
        let d = 1000.0;
        // A point 192 screen units right of the reticle: centring 1 − (192 − 128) / 128 = 0.5.
        let x = 192.0 * d / v.focal();
        let f = v.frame([d, -x, 0.0], 100.0).unwrap();
        assert!((f.centring - 0.5).abs() < 1e-3, "{f:?}");
    }
}
