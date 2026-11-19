//! The follow camera and its modes (camera.md 2 to 7): zone and default framing, springs, orbit, automatic follow,
//! recentre, path lock, view clearing, camera-fade groups, look-around, cutscene placements, aligned views, panels
//! and the letterbox.

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::consts::{CameraConstants, CameraTunables};
use super::fade::{FadeSource, FadeTracker, FadedMesh};
use super::framing::{Frame, Framing, ZoneFrame, framed_eye};
use super::zone::{CameraZone, select_zone};
use crate::collision::world::{CollisionWorld, QueryFilter};
use crate::math::{Vec3, add, atan2, cos, cross, dot, length, madd, normalize, scale, sin, sqrt, sub};
use crate::physics::spring::{SmoothParams, Smoother, Spring};

/// The lens and fog (camera.md 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    /// Vertical field of view (radians).
    pub fov: f32,
    /// Near clip.
    pub near: f32,
    /// Far clip.
    pub far: f32,
    /// Fog start, end, maximum.
    pub fog: [f32; 3],
    /// Fog colour (option 535), if the world sets one.
    pub fog_color: Option<[u8; 4]>,
}

impl Lens {
    /// The defaults of camera.md 1.
    pub fn defaults(c: &CameraConstants) -> Lens {
        Lens { fov: c.fov, near: c.near, far: c.far, fog: c.fog, fog_color: None }
    }
}

/// The camera's input (the right stick, the recentre press, the left stick for look-around and manual modes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Left stick, x right and y up.
    pub left: [f32; 2],
    /// Right stick.
    pub right: [f32; 2],
    /// Right-stick press: recentre behind the player.
    pub recentre: bool,
    /// Invert the horizontal orbit (an option of the menus).
    pub invert_x: bool,
}

/// What the camera follows (camera.md 4.1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Subject {
    /// Origin (the feet).
    pub position: Vec3,
    /// Velocity.
    pub velocity: Vec3,
    /// Facing heading.
    pub yaw: f32,
    /// In the air (the ground height is kept).
    pub airborne: bool,
    /// The clamped state: the focus offset only decays (otherwise, with no focus point of a move, it springs back
    /// to zero).
    pub clamped: bool,
    /// Height of the ground contact point while standing (`None`: the origin's height).
    pub ground: Option<f32>,
    /// On a hook: the hook's height (the ground height is 300 below it).
    pub hook: Option<f32>,
    /// A move's own focus point (flattened against a wall: 50 above the origin, shifted toward the side looked at).
    pub focus_point: Option<Vec3>,
    /// Flattened against a wall (the lock speed is measured against 200 instead of 600).
    pub flattened: bool,
    /// In a hiding place (no look-ahead; the path weight is 3).
    pub hidden: bool,
    /// The follow target's position (option 880): the automatic follow measures the motion toward it.
    pub follow_target: Option<Vec3>,
    /// The automatic-follow factor eases toward 0 (camera.md 4.3): in an ordinary jump before its boost, on a hook,
    /// in a damage reaction, sidestepping, or hiding in the casual, reach or vault kinds.
    pub follow_off: bool,
}

/// One step of the camera's eye spring on one coordinate (camera.md 4.2): `v ← (1 + c·dt)·v + k·(target − x)·dt`,
/// then `x ← x + v·dt` (c negative). Returns the new (x, v).
pub fn eye_spring_step(x: f32, v: f32, target: f32, k: f32, c: f32, dt: f32) -> (f32, f32) {
    let v = (1.0 + c * dt) * v + k * (target - x) * dt;
    (x + v * dt, v)
}

/// The clearance radius of view clearing (camera.md 7): the near-clip rectangle's corner radius plus 1,
/// `tan(fov/2)·√(aspect² + 1)·near + 1` (90.5 with fov 1.0, aspect 1.2987, near 100).
pub fn clearance_radius(fov: f32, aspect: f32, near: f32) -> f32 {
    tan(fov * 0.5) * sqrt(aspect * aspect + 1.0) * near + 1.0
}

/// One speaker of a dialogue picture panel (camera.md 7.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelSubject {
    /// The speaker's position.
    pub position: Vec3,
    /// Its facing heading.
    pub yaw: f32,
    /// Field of view (option 989; default 0.5).
    pub fov: Option<f32>,
    /// Distance (option 990; default 400).
    pub distance: Option<f32>,
    /// Screen position (options 991, 992).
    pub screen: [f32; 2],
    /// Light offset (option 993).
    pub light_offset: Vec3,
}

/// The view of one picture panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelView {
    /// Eye.
    pub eye: Vec3,
    /// Target.
    pub target: Vec3,
    /// Field of view.
    pub fov: f32,
    /// Where the panel is drawn.
    pub screen: [f32; 2],
    /// Light position for the panel.
    pub light: Vec3,
}

/// The world as the camera sees it this frame.
#[derive(Clone, Copy, Default)]
pub struct CameraScene<'a> {
    /// Camera zones.
    pub zones: &'a [CameraZone],
    /// Collision, for keeping the view clear (camera meshes are used where solids have them).
    pub collision: Option<&'a CollisionWorld>,
    /// Solids with camera-fade groups and their current world transforms.
    pub fades: &'a [(FadeSource, Affine)],
    /// Dialogue picture panels (left, right).
    pub panels: [Option<PanelSubject>; 2],
    /// Letterbox bars shown.
    pub letterbox: bool,
}

/// Camera modes (camera.md 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Zone or default framing around the subject.
    Follow,
    /// Look-around (and, inferred, the binocucom view): `zoom` 0 near to 1 far.
    Look {
        /// Look heading.
        yaw: f32,
        /// Look pitch.
        pitch: f32,
        /// 0 near, 1 far.
        zoom: f32,
    },
    /// A cutscene: the camera follows a camera placement's world transform (forward, left, up columns).
    Animation {
        /// The placement's world transform.
        placement: Affine,
    },
    /// A fixed alignment to an anchor (scripted moments, confrontation dialogues): eye at `offset` in the anchor's
    /// frame (forward, left, up), looking at the anchor.
    Aligned {
        /// The anchor's world transform.
        anchor: Affine,
        /// Eye offset in the anchor's frame.
        offset: Vec3,
    },
    /// Free development camera.
    Manual {
        /// Heading.
        yaw: f32,
        /// Pitch.
        pitch: f32,
    },
}

/// One frame of the camera's output.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraFrame {
    /// Eye.
    pub eye: Vec3,
    /// Point looked at.
    pub target: Vec3,
    /// Up (+Z, or the placement's up in a cutscene).
    pub up: Vec3,
    /// Vertical field of view.
    pub fov: f32,
    /// Near clip.
    pub near: f32,
    /// Far clip.
    pub far: f32,
    /// Fog start, end, maximum.
    pub fog: [f32; 3],
    /// Fog colour.
    pub fog_color: Option<[u8; 4]>,
    /// The focus point.
    pub focus: Vec3,
    /// Meshes faded this frame.
    pub faded: Vec<FadedMesh>,
    /// Letterbox bars.
    pub letterbox: bool,
    /// Picture panels.
    pub panels: [Option<PanelView>; 2],
    /// The camera cut this frame (no smoothing between this frame and the last).
    pub cut: bool,
    /// The active zone's record, if any.
    pub zone: Option<u32>,
}

/// `tan` from the engine's sine and cosine.
fn tan(x: f32) -> f32 {
    let (s, c) = crate::math::sin_cos(x);
    s / c
}

fn wrap(a: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    crate::math::rem_euclid(a + core::f32::consts::PI, tau) - core::f32::consts::PI
}

fn heading(yaw: f32) -> Vec3 {
    [cos(yaw), sin(yaw), 0.0]
}

impl CameraFrame {
    /// The view matrix (column-major) in a right-handed Y-up space: game space is converted with
    /// (x, y, z) → (x, z, −y) (transforms.md 7.1), then the camera looks down −Z.
    pub fn view_matrix(&self) -> [f32; 16] {
        let c = |v: Vec3| [v[0], v[2], -v[1]];
        let (eye, target, up) = (c(self.eye), c(self.target), c(self.up));
        let f = normalize(sub(target, eye));
        let s = normalize(cross(f, up));
        let u = cross(s, f);
        [
            s[0],
            u[0],
            -f[0],
            0.0,
            s[1],
            u[1],
            -f[1],
            0.0,
            s[2],
            u[2],
            -f[2],
            0.0,
            -dot(s, eye),
            -dot(u, eye),
            dot(f, eye),
            1.0,
        ]
    }

    /// A perspective projection (column-major) for `aspect` (width / height); depth 0..1 when `zero_to_one`
    /// (deko3d), −1..1 otherwise (OpenGL).
    pub fn projection(&self, aspect: f32, zero_to_one: bool) -> [f32; 16] {
        let f = 1.0 / tan(self.fov * 0.5);
        let (n, fa) = (self.near, self.far);
        let (a, b) = if zero_to_one {
            (fa / (n - fa), n * fa / (n - fa))
        } else {
            ((fa + n) / (n - fa), 2.0 * fa * n / (n - fa))
        };
        [f / aspect, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 0.0, a, -1.0, 0.0, 0.0, b, 0.0]
    }

    /// Where a game-space point lands in normalised device coordinates (x right, y up, each −1 to 1 on screen).
    pub fn project(&self, p: Vec3, aspect: f32) -> [f32; 2] {
        let v = self.view_matrix();
        let c = [p[0], p[2], -p[1]];
        let x = v[0] * c[0] + v[4] * c[1] + v[8] * c[2] + v[12];
        let y = v[1] * c[0] + v[5] * c[1] + v[9] * c[2] + v[13];
        let z = v[2] * c[0] + v[6] * c[1] + v[10] * c[2] + v[14];
        let f = 1.0 / tan(self.fov * 0.5);
        [x * f / aspect / -z, y * f / -z]
    }
}

/// The follow camera.
#[derive(Clone, Debug)]
pub struct FollowCamera {
    /// Spec values.
    pub consts: CameraConstants,
    /// Port tunables.
    pub tun: CameraTunables,
    /// The world's lens.
    pub lens: Lens,
    /// Mode.
    pub mode: Mode,
    /// Eye (after the near-clip push).
    pub eye: Vec3,
    /// Eye before the near-clip push: the framed, sprung and cleared position (the centre of the near-clip plane).
    pub raw_eye: Vec3,
    /// Focus point.
    pub focus: Vec3,
    /// Orbit heading (the direction the camera looks).
    pub yaw: f32,
    /// Manual orbit speed.
    pub orbit_speed: f32,
    /// Eye's horizontal distance from the focus.
    pub rho: f32,
    rho_vel: f32,
    /// Eye's height above the focus.
    pub height: f32,
    height_vel: f32,
    ground: f32,
    base_z: f32,
    base_smoother: Smoother,
    adjust: Vec3,
    adjust_vel: Vec3,
    offset: Vec3,
    offset_vel: Vec3,
    since_manual: f32,
    idle: f32,
    manual_framing: bool,
    manual_anchor: Vec3,
    follow_factor: f32,
    follow_smoother: Smoother,
    lock_smoother: Smoother,
    recentre: Option<Smoother>,
    zone: Option<usize>,
    zone_time: f32,
    zone_fov: Option<f32>,
    cyl_angle: Option<f32>,
    check_delay: f32,
    blocked_for: f32,
    fades: FadeTracker,
    fov: f32,
    fov_smoother: Smoother,
    cut_pending: bool,
}

impl FollowCamera {
    /// A camera with the spec's constants, default tunables and default lens.
    pub fn new() -> FollowCamera {
        FollowCamera::with(CameraConstants::SPEC, CameraTunables::default())
    }

    /// A camera with given constants and tunables.
    pub fn with(consts: CameraConstants, tun: CameraTunables) -> FollowCamera {
        FollowCamera {
            consts,
            tun,
            lens: Lens::defaults(&consts),
            mode: Mode::Follow,
            eye: [0.0; 3],
            raw_eye: [0.0; 3],
            focus: [0.0; 3],
            yaw: 0.0,
            orbit_speed: 0.0,
            rho: -consts.framing.distance,
            rho_vel: 0.0,
            height: consts.framing.offsets[2].c[0],
            height_vel: 0.0,
            ground: 0.0,
            base_z: 0.0,
            base_smoother: Smoother::default(),
            adjust: [0.0; 3],
            adjust_vel: [0.0; 3],
            offset: [0.0; 3],
            offset_vel: [0.0; 3],
            since_manual: 10.0,
            idle: 0.0,
            manual_framing: false,
            manual_anchor: [0.0; 3],
            follow_factor: 1.0,
            follow_smoother: Smoother::default(),
            lock_smoother: Smoother::default(),
            recentre: None,
            zone: None,
            zone_time: 0.0,
            zone_fov: None,
            cyl_angle: None,
            check_delay: 0.0,
            blocked_for: 0.0,
            fades: FadeTracker::default(),
            fov: consts.fov,
            fov_smoother: Smoother::default(),
            cut_pending: true,
        }
    }

    /// Sets the lens (from the world options); the field of view eases toward it.
    pub fn set_lens(&mut self, lens: Lens) {
        self.lens = lens;
    }

    /// Switches mode; the next frame cuts.
    pub fn set_mode(&mut self, mode: Mode) {
        if core::mem::discriminant(&mode) != core::mem::discriminant(&self.mode) {
            self.cut_pending = true;
        }
        self.mode = mode;
    }

    /// Cuts to `eye` looking at `target` (a warp arrival, a level start): the next frame starts there with the
    /// springs at rest, and the orbit heading looks the same way.
    pub fn cut_to(&mut self, eye: Vec3, target: Vec3) {
        self.eye = eye;
        self.raw_eye = eye;
        let d = sub(target, eye);
        self.yaw = atan2(d[1], d[0]);
        self.cut_pending = true;
    }

    /// The manual orbit's speed step (camera.md 5): target `x·|x|·2.5`, changing by at most 15 rad/s² × dt.
    pub fn orbit_step(&mut self, x: f32, dt: f32) -> f32 {
        let target = x * x.abs() * self.consts.orbit_speed;
        let step = self.consts.orbit_accel * dt;
        self.orbit_speed += (target - self.orbit_speed).clamp(-step, step);
        self.orbit_speed
    }

    /// One frame (real clock).
    pub fn update(&mut self, dt: f32, input: &CameraInput, subject: &Subject, scene: &CameraScene<'_>) -> CameraFrame {
        let cut = core::mem::replace(&mut self.cut_pending, false);
        let (eye, target, up, zone) = match self.mode {
            Mode::Follow => self.follow(dt, input, subject, scene, cut),
            Mode::Look { yaw, pitch, zoom } => {
                let yaw = yaw - input.left[0] * self.tun.look_rate * dt;
                let lim = self.tun.look_pitch;
                let pitch = (pitch + input.left[1] * self.tun.look_rate * dt).clamp(-lim, lim);
                self.mode = Mode::Look { yaw, pitch, zoom };
                let dir = [cos(pitch) * cos(yaw), cos(pitch) * sin(yaw), sin(pitch)];
                let head = add(subject.position, [0.0, 0.0, self.consts.look_height]);
                let off = self.consts.look_near + (self.consts.look_far - self.consts.look_near) * zoom.clamp(0.0, 1.0);
                let eye = madd(head, dir, off);
                (eye, add(eye, dir), [0.0, 0.0, 1.0], None)
            }
            Mode::Animation { placement } => {
                let f = Frame::from_affine(&placement);
                (f.origin, add(f.origin, f.forward), f.up, None)
            }
            Mode::Aligned { anchor, offset } => {
                let f = Frame::from_affine(&anchor);
                (f.to_world(offset), anchor.t, [0.0, 0.0, 1.0], None)
            }
            Mode::Manual { yaw, pitch } => {
                let yaw = yaw - input.right[0] * 2.0 * dt;
                let pitch = (pitch + input.right[1] * 2.0 * dt).clamp(-1.5, 1.5);
                self.mode = Mode::Manual { yaw, pitch };
                let dir = [cos(pitch) * cos(yaw), cos(pitch) * sin(yaw), sin(pitch)];
                let left = cross([0.0, 0.0, 1.0], heading(yaw));
                self.eye = add(
                    add(self.eye, scale(dir, input.left[1] * 1200.0 * dt)),
                    scale(left, -input.left[0] * 1200.0 * dt),
                );
                (self.eye, add(self.eye, dir), [0.0, 0.0, 1.0], None)
            }
        };
        if !matches!(self.mode, Mode::Follow) {
            self.eye = eye;
            self.raw_eye = eye;
            self.zone_fov = None;
        }
        // Lens: the field of view eases toward the zone's (878) or the world's, with the four-value smoothing.
        let fov_target = self.zone_fov.unwrap_or(self.lens.fov);
        self.fov =
            if cut { fov_target } else { self.fov_smoother.step(&self.consts.fov_smooth, self.fov, fov_target, dt) };
        // Camera-fade groups along the view.
        let mut faded_now: Vec<u16> = Vec::new();
        for (src, t) in scene.fades {
            faded_now.clear();
            src.faded_meshes(t, eye, self.focus, &mut faded_now);
            for &m in &faded_now {
                self.fades.touch(FadedMesh { record: src.record, mesh: m }, self.consts.fade_time);
            }
        }
        let faded = self.fades.step(dt);
        let panels = scene.panels.map(|p| p.map(|p| self.panel(&p)));
        CameraFrame {
            eye,
            target,
            up,
            fov: self.fov,
            near: self.lens.near,
            far: self.lens.far,
            fog: self.lens.fog,
            fog_color: self.lens.fog_color,
            focus: self.focus,
            faded,
            letterbox: scene.letterbox,
            panels,
            cut,
            zone,
        }
    }

    fn panel(&self, p: &PanelSubject) -> PanelView {
        let [fov, dist] = self.consts.panel_lens;
        let head = add(p.position, [0.0, 0.0, self.tun.panel_head]);
        let eye = madd(head, heading(p.yaw), p.distance.unwrap_or(dist));
        PanelView { eye, target: head, fov: p.fov.unwrap_or(fov), screen: p.screen, light: add(head, p.light_offset) }
    }

    /// The focus point (camera.md 4.1): base point, look-ahead, nose, the zone's focus scale, the focus offset.
    fn update_focus(&mut self, dt: f32, subject: &Subject, framing: &Framing, zone: Option<&CameraZone>, cut: bool) {
        let c = self.consts;
        let p = subject.position;
        let origin = p[2] + c.origin_height;
        // 1. Base point: the ground height G (the hook's − 300; while standing the contact's; kept in the air; on a
        //    cut the origin's − 75), then B_z eased toward G + 125.
        if let Some(h) = subject.hook {
            self.ground = h - c.hook_drop;
        } else if !subject.airborne {
            self.ground = subject.ground.unwrap_or(p[2]);
        } else if cut {
            self.ground = origin - c.cut_drop;
        }
        self.ground = self.ground.clamp(origin + c.ground_limits[0], origin + c.ground_limits[1]);
        let bt = self.ground + c.base_height;
        self.base_z = if cut { bt } else { self.base_smoother.step(&c.base_smooth, self.base_z, bt, dt) };
        self.base_z = self.base_z.clamp(origin + c.base_limits[0], origin + c.base_limits[1]);
        let base = [p[0], p[1], self.base_z];
        // 2. Look-ahead: 100 ahead along the facing (none in a hiding place), spring (5, −3.5).
        let ahead = if subject.hidden { 0.0 } else { c.look_ahead };
        let adj_target = scale(heading(subject.yaw), ahead);
        if cut {
            self.adjust = adj_target;
            self.adjust_vel = [0.0; 3];
        } else {
            let [k, d] = c.look_ahead_spring;
            (self.adjust, self.adjust_vel) = Spring::new(k, d).step(self.adjust, self.adjust_vel, adj_target, dt);
        }
        // 3. Nose, 4. focus scale in the zone frame.
        let mut nose = madd(base, self.adjust, framing.adjust);
        if let Some(z) = zone
            && framing.focus_scale != [1.0, 1.0, 1.0]
        {
            let fr = z.frame.frame_for(nose);
            let l = fr.to_local(nose);
            let sc = framing.focus_scale;
            nose = fr.to_world([l[0] * sc[0], l[1] * sc[1], l[2] * sc[2]]);
        }
        // 5. Focus offset: toward a move's focus point (8, −6), back to zero when relaxed (5, −3.5), or decaying.
        let (target, spring) = match (subject.focus_point, subject.clamped) {
            (Some(fp), _) => (sub(fp, nose), Spring::new(c.focus_spring[0], c.focus_spring[1])),
            (None, false) => ([0.0; 3], Spring::new(c.relax_spring[0], c.relax_spring[1])),
            (None, true) => (self.offset, Spring::new(0.0, c.clamped_damping)),
        };
        if cut {
            self.offset = target;
            self.offset_vel = [0.0; 3];
        } else {
            (self.offset, self.offset_vel) = spring.step(self.offset, self.offset_vel, target, dt);
        }
        self.focus = add(nose, self.offset);
    }

    fn follow(
        &mut self,
        dt: f32,
        input: &CameraInput,
        subject: &Subject,
        scene: &CameraScene<'_>,
        cut: bool,
    ) -> (Vec3, Vec3, Vec3, Option<u32>) {
        let c = self.consts;
        let mut cut = cut;
        // Zone.
        let prev = self.zone;
        self.zone = select_zone(scene.zones, subject.position, subject.airborne, self.zone);
        if self.zone != prev {
            self.cyl_angle = None;
            self.zone_time = 0.0;
            self.check_delay = 0.0;
            cut |= self.zone.is_some_and(|i| scene.zones[i].framing.cut_on_entry);
        }
        self.zone_time += dt;
        let zone = self.zone.map(|i| &scene.zones[i]);
        let framing: Framing = zone.map_or(c.framing, |z| z.framing);
        self.zone_fov = framing.fov;
        self.update_focus(dt, subject, &framing, zone, cut);
        let focus = self.focus;
        // Automatic-follow factor eases toward the zone's.
        let follow_goal = if subject.follow_off { 0.0 } else { framing.follow };
        self.follow_factor = self.follow_smoother.step(&c.follow_smooth, self.follow_factor, follow_goal, dt);
        // Manual orbit.
        let x = if input.invert_x { -input.right[0] } else { input.right[0] };
        let manual = self.orbit_step(x, dt);
        if x != 0.0 || manual != 0.0 {
            self.since_manual = 0.0;
            if !self.manual_framing {
                self.manual_framing = true;
                self.manual_anchor = subject.position;
            }
            self.recentre = None;
        } else {
            self.since_manual += dt;
            let moved = length(sub(subject.position, self.manual_anchor));
            if self.manual_framing && moved > self.tun.manual_release {
                self.manual_framing = false;
            }
        }
        let hspeed = length([subject.velocity[0], subject.velocity[1], 0.0]);
        self.idle = if hspeed > 1.0 { 0.0 } else { self.idle + dt };
        let use_zone = zone.filter(|_| !self.manual_framing || !framing.directions.manual());
        // Automatic orbit (manual framing only): sideways speed over the distance, capped at 2.5 rad/s.
        if use_zone.is_none() {
            let fwd = heading(self.yaw);
            let right = [fwd[1], -fwd[0], 0.0];
            let motion = match subject.follow_target {
                Some(t) => {
                    let d = sub(t, subject.position);
                    let l = length(d);
                    if l > 0.0 { scale(d, framing.follow_speed.eval(l) / l) } else { [0.0; 3] }
                }
                None => subject.velocity,
            };
            let side = if framing.side_follow { dot(motion, fwd) } else { dot(motion, right) };
            let y = input.left[1];
            let [a0, a1, a2, lo, hi] = c.orbit_toward;
            let toward = (a0 + a1 * y + a2 * y * y).clamp(lo, hi);
            let [r0, r1, r2] = c.orbit_ramp;
            let t = self.since_manual;
            let ramp = (r0 + r1 * t + r2 * t * t).clamp(0.0, 1.0);
            let auto =
                (-side / self.rho.max(1.0)).clamp(-c.orbit_speed, c.orbit_speed) * self.follow_factor * toward * ramp;
            self.yaw = wrap(self.yaw + auto * dt);
        }
        self.yaw = wrap(self.yaw + manual * dt);
        // Recentre behind the player.
        if input.recentre {
            self.recentre = Some(Smoother::default());
        }
        if let Some(mut s) = self.recentre {
            let d = wrap(subject.yaw - self.yaw);
            let moved = s.step(&c.recentre_smooth, 0.0, d, dt);
            self.yaw = wrap(self.yaw + moved);
            self.recentre = if (d - moved).abs() < 1e-3 { None } else { Some(s) };
        }
        // Framing targets (ρ, z) and, in a zone, the turn toward the framed direction.
        let (rho_t, z_t) = match use_zone {
            Some(z) => {
                let mut frame = z.frame.frame_for(focus);
                if framing.directions.reverse() && dot(heading(subject.yaw), frame.forward) < 0.0 {
                    frame = frame.reversed();
                }
                let eye = framed_eye(&framing, &frame, focus, &mut self.cyl_angle);
                let d = sub(focus, eye);
                let framed_yaw = atan2(d[1], d[0]);
                let div = if subject.flattened { c.lock_flattened_speed } else { c.lock_full_speed };
                let mut lock = (hspeed / div).min(1.0) * framing.lock_on;
                if framing.pan[0] {
                    let [t, strength] = framing.pan_entry;
                    lock = lock.max(if t <= 0.0 { strength } else { strength * (self.zone_time / t).min(1.0) });
                }
                let [idle_after, quiet] = c.idle_pan_delay;
                if framing.pan[1] && self.idle >= idle_after && self.since_manual >= quiet {
                    let [t, strength] = framing.pan_idle;
                    let r = if t <= 0.0 { 1.0 } else { ((self.idle - idle_after) / t).min(1.0) };
                    lock = lock.max(strength * r);
                }
                let weight = if subject.hidden {
                    3.0
                } else {
                    match z.frame {
                        ZoneFrame::Fixed(_) => 1.0,
                        ZoneFrame::Path(_) => {
                            let cs = dot(heading(subject.yaw), frame.forward);
                            cs * cs * (3.0 - cs.abs())
                        }
                    }
                };
                let dyaw = wrap(framed_yaw - self.yaw);
                if cut {
                    self.yaw = framed_yaw;
                } else {
                    let p = SmoothParams { fast: c.lock_smooth.fast * lock * weight, ..c.lock_smooth };
                    let m = self.lock_smoother.step(&p, 0.0, dyaw, dt);
                    self.yaw = wrap(self.yaw + m);
                }
                (length([d[0], d[1], 0.0]), eye[2] - focus[2])
            }
            None => {
                let eye = framed_eye(&framing, &Frame::level(focus, self.yaw), focus, &mut None);
                let d = sub(eye, focus);
                (length([d[0], d[1], 0.0]), d[2])
            }
        };
        // The eye spring on ρ and z (15·s₈₈₅, −8·s₈₈₆), velocity first.
        let k = c.eye_spring[0] * framing.eye_spring[0];
        let damp = c.eye_spring[1] * framing.eye_spring[1];
        if cut {
            (self.rho, self.rho_vel, self.height, self.height_vel) = (rho_t, 0.0, z_t, 0.0);
        } else {
            (self.rho, self.rho_vel) = eye_spring_step(self.rho, self.rho_vel, rho_t, k, damp, dt);
            (self.height, self.height_vel) = eye_spring_step(self.height, self.height_vel, z_t, k, damp, dt);
        }
        let fwd = heading(self.yaw);
        let want = add(madd(focus, fwd, -self.rho), [0.0, 0.0, self.height]);
        // View clearing, then the near-clip push.
        let raw = if framing.no_clearing { want } else { self.clear_view(dt, scene, subject, want, cut) };
        self.raw_eye = raw;
        let e = sub(raw, focus);
        let el = length(e);
        self.eye = if el > 0.0 { madd(focus, e, (el + self.lens.near) / el) } else { raw };
        let target = self.aim(self.eye, focus, framing.screen);
        (self.eye, target, [0.0, 0.0, 1.0], zone.map(|z| z.record))
    }

    /// How far a line from `from` along unit `dir` is clear of camera meshes, up to `len` (solids with camera-fade
    /// groups fade instead and do not block).
    fn clear_along(&self, cw: &CollisionWorld, scene: &CameraScene<'_>, from: Vec3, dir: Vec3, len: f32) -> f32 {
        let filter = QueryFilter { camera: true, ..QueryFilter::default() };
        let mut done = 0.0;
        for _ in 0..8 {
            let origin = madd(from, dir, done);
            let Some(h) = cw.ray_cast(origin, dir, len - done, &filter) else { return len };
            if scene.fades.iter().any(|(f, _)| f.record == cw.body(h.body).user) {
                done += h.distance + 0.5;
                continue;
            }
            return done + h.distance;
        }
        len
    }

    fn segment_clear(&self, cw: &CollisionWorld, scene: &CameraScene<'_>, a: Vec3, b: Vec3) -> bool {
        let d = sub(b, a);
        let l = length(d);
        l <= 0.0 || self.clear_along(cw, scene, a, scale(d, 1.0 / l), l) >= l - 1e-3
    }

    /// View clearing (camera.md 7): the squish every frame, and the cut when the player has been hidden for 2 s.
    fn clear_view(&mut self, dt: f32, scene: &CameraScene<'_>, subject: &Subject, want: Vec3, cut: bool) -> Vec3 {
        let Some(cw) = scene.collision else { return want };
        let c = self.consts;
        let r_near = clearance_radius(self.fov, c.aspect, self.lens.near);
        if cut {
            self.blocked_for = 0.0;
            self.check_delay = 0.0;
            return self.cut_position(cw, scene, subject, want, r_near);
        }
        self.check_delay += dt;
        if self.check_delay >= c.squish_recheck {
            if self.player_visible(cw, scene, subject, self.raw_eye) {
                self.blocked_for = 0.0;
            } else {
                self.blocked_for += dt;
            }
        }
        if self.blocked_for > c.squish_cut {
            self.blocked_for = 0.0;
            self.check_delay = 0.0;
            self.cut_pending = false;
            return self.cut_position(cw, scene, subject, want, r_near);
        }
        self.squish(cw, scene, self.raw_eye, want, dt, r_near)
    }

    /// The squish (camera.md 7): limits the eye's step from `prev` toward `want` so it keeps `r_near` from the
    /// camera meshes within 250 of `prev`. Each surface (normal n, distance g from the sphere) asks for a velocity
    /// along n of at least the speed the smoothing (2500, 0, 0.25) gives for going from g to r_near − 250; the
    /// smallest correction that is a non-negative sum of the normals is found by coordinate ascent on the weights,
    /// and capped at the eye's own speed + 1500.
    fn squish(
        &self,
        cw: &CollisionWorld,
        scene: &CameraScene<'_>,
        prev: Vec3,
        want: Vec3,
        dt: f32,
        r_near: f32,
    ) -> Vec3 {
        if dt <= 0.0 {
            return want;
        }
        let c = self.consts;
        let [radius, allowance, _] = c.squish_record;
        let filter = QueryFilter { camera: true, ..QueryFilter::default() };
        let mut overlaps = Vec::new();
        cw.sphere_overlaps(prev, radius.max(c.squish_box), &filter, &mut overlaps);
        let goal = r_near - radius;
        let planes: Vec<(Vec3, f32)> = overlaps
            .iter()
            .filter(|o| !scene.fades.iter().any(|(f, _)| f.record == cw.body(o.body).user))
            .map(|o| {
                let g = -o.depth;
                let d = g - goal;
                let s = if d > 0.0 { -c.squish_smooth.speed_at(d) } else { c.squish_smooth.speed_at(-d) };
                (normalize(o.normal), s)
            })
            .collect();
        let v = scale(sub(want, prev), 1.0 / dt);
        if planes.is_empty() {
            return want;
        }
        let mut w = alloc::vec![0.0f32; planes.len()];
        let mut corr = [0.0f32; 3];
        for _ in 0..32 {
            for (i, (n, s)) in planes.iter().enumerate() {
                let deficit = s - dot(add(v, corr), *n);
                let nw = (w[i] + deficit).max(0.0);
                corr = madd(corr, *n, nw - w[i]);
                w[i] = nw;
            }
        }
        let cap = length(v) + allowance;
        let cl = length(corr);
        if cl > cap {
            corr = scale(corr, cap / cl);
        }
        madd(prev, add(v, corr), dt)
    }

    /// Whether the player can be seen: five probe points around him (origin, ±50 sideways, 75 up, 50 down) each
    /// with a clear line to the matching point on the near-clip plane (centred on `near_centre`).
    fn player_visible(
        &self,
        cw: &CollisionWorld,
        scene: &CameraScene<'_>,
        subject: &Subject,
        near_centre: Vec3,
    ) -> bool {
        let [side, up, down] = self.consts.probe_offsets;
        let view = normalize(sub(self.focus, near_centre));
        let right = normalize(cross(view, [0.0, 0.0, 1.0]));
        let offsets = [[0.0; 3], scale(right, side), scale(right, -side), [0.0, 0.0, up], [0.0, 0.0, -down]];
        offsets.iter().any(|o| self.segment_clear(cw, scene, add(subject.position, *o), add(near_centre, *o)))
    }

    /// The nearest clear point around the player's origin (camera.md 7, step 1): rays of 500 to the sample
    /// sphere's points (tilts −90° to +90° by 22.5°, the pan counts of [`CameraConstants::cut_samples`], pans
    /// i·180°/n); along each, the start of the first clear stretch longer than 10 (0 when the origin is clear, else
    /// plus 5); the point with the smallest start, or the origin.
    fn nearest_clear(&self, cw: &CollisionWorld, scene: &CameraScene<'_>, origin: Vec3) -> Vec3 {
        let filter = QueryFilter { camera: true, ..QueryFilter::default() };
        let len = self.consts.cut_search;
        let mut best: Option<(f32, Vec3)> = None;
        for (k, &n) in self.consts.cut_samples.iter().enumerate() {
            let tilt = (-90.0 + 22.5 * k as f32) * core::f32::consts::PI / 180.0;
            let (st, ct) = (sin(tilt), cos(tilt));
            for i in 0..n {
                let pan = core::f32::consts::PI * i as f32 / n as f32;
                let dir = [ct * cos(pan), ct * sin(pan), st];
                // Walk the ray: alternate hits and gaps until a gap longer than 10 is found.
                let mut at = 0.0f32;
                let mut start = None;
                for _ in 0..16 {
                    if at >= len {
                        break;
                    }
                    let gap = self.clear_along(cw, scene, madd(origin, dir, at), dir, len - at);
                    if gap > 10.0 {
                        start = Some(if at == 0.0 { 0.0 } else { at + 5.0 });
                        break;
                    }
                    // Step through the surface hit and look again beyond it.
                    at += gap + 1.0;
                    let _ = &filter;
                }
                if let Some(u) = start
                    && best.is_none_or(|(b, _)| u < b)
                {
                    best = Some((u, madd(origin, dir, u)));
                }
            }
        }
        best.map_or(origin, |b| b.1)
    }

    /// The eye position after a cut (camera.md 7): from the nearest clear point C around the player, the framed eye
    /// if the line from C to it is clear; otherwise the smallest turn around the focus (steps of about 16°, up to
    /// ±180°) with a clear line; otherwise the turn with the longest clear line, kept `r_near` short of the surface.
    /// Then pushed out of surfaces within `r_near`.
    fn cut_position(
        &self,
        cw: &CollisionWorld,
        scene: &CameraScene<'_>,
        subject: &Subject,
        want: Vec3,
        r_near: f32,
    ) -> Vec3 {
        let origin = add(subject.position, [0.0, 0.0, self.consts.origin_height]);
        let c = self.nearest_clear(cw, scene, origin);
        if self.segment_clear(cw, scene, c, want) {
            return self.push_out(cw, scene, want, r_near);
        }
        let focus = self.focus;
        let e = sub(want, focus);
        let len = length(e);
        if len <= 0.0 {
            return want;
        }
        let rot = |a: f32| {
            let (s, co) = (sin(a), cos(a));
            [e[0] * co - e[1] * s, e[0] * s + e[1] * co, e[2]]
        };
        let steps = (core::f32::consts::PI / self.consts.cut_angle_step) as i32;
        let mut best: Option<(f32, Vec3)> = None;
        let mut chosen = None;
        'search: for k in 0..=steps {
            for sign in [1.0f32, -1.0] {
                if k == 0 && sign < 0.0 {
                    continue;
                }
                let off = rot(sign * k as f32 * self.consts.cut_angle_step);
                let dir = scale(off, 1.0 / len);
                let clear = self.clear_along(cw, scene, focus, dir, len.min(self.consts.cut_search.max(len)));
                if clear >= len - 1e-3 {
                    chosen = Some(add(focus, off));
                    break 'search;
                }
                if best.is_none_or(|(b, _)| clear > b) {
                    best = Some((clear, madd(focus, dir, (clear - r_near).max(0.0))));
                }
            }
        }
        let p = chosen.or(best.map(|b| b.1)).unwrap_or(want);
        self.push_out(cw, scene, p, r_near)
    }

    /// Pushes `p` out of the camera meshes within `r_near`.
    fn push_out(&self, cw: &CollisionWorld, scene: &CameraScene<'_>, p: Vec3, r_near: f32) -> Vec3 {
        let mut p = p;
        let filter = QueryFilter { camera: true, ..QueryFilter::default() };
        let mut overlaps = Vec::new();
        cw.sphere_overlaps(p, r_near, &filter, &mut overlaps);
        for o in overlaps.iter().filter(|o| !scene.fades.iter().any(|(f, _)| f.record == cw.body(o.body).user)) {
            p = madd(p, normalize(o.normal), o.depth.max(0.0));
        }
        p
    }

    /// The point to look at so that `focus` appears at screen position `screen` (y −0.1: slightly below centre).
    fn aim(&self, eye: Vec3, focus: Vec3, screen: [f32; 2]) -> Vec3 {
        let d = sub(focus, eye);
        let l = length(d);
        if l <= 0.0 {
            return add(eye, [1.0, 0.0, 0.0]);
        }
        let d = scale(d, 1.0 / l);
        let t = tan(self.fov * 0.5);
        // Pitch the view by the angle at which a point at screen y appears: tan(α) = −y · tan(fov / 2).
        let pitch = atan2(-screen[1] * t, 1.0);
        let yaw = atan2(screen[0] * t, 1.0);
        let base_pitch = atan2(d[2], sqrt(d[0] * d[0] + d[1] * d[1]));
        let base_yaw = atan2(d[1], d[0]);
        let (p, y) = (base_pitch + pitch, base_yaw - yaw);
        add(eye, [cos(p) * cos(y), cos(p) * sin(y), sin(p)])
    }
}

impl Default for FollowCamera {
    fn default() -> Self {
        FollowCamera::new()
    }
}
