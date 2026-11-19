//! Restoring springs, the smoothing law and the drive modes that pull objects toward goals (physics.md 4,
//! options-by-class.md 2.7).

use crate::math::{
    Mat3, Vec3, add, axis_angle, dot, length, madd, mat_mul, normalize, orthonormalize, rotation_vector, scale, sub,
    transpose,
};

/// A spring strength or damping: a constant, or three coefficients of the distance `d` to the goal meaning
/// `c0 / d + c1 + c2 · d` (options 57 and 66, physics.md 4.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Strength {
    /// A constant.
    Constant(f32),
    /// `c0 / d + c1 + c2 · d` (the `c0` term is left out at `d = 0`).
    Coefficients([f32; 3]),
}

impl Strength {
    /// The value at distance `d`.
    pub fn at(&self, d: f32) -> f32 {
        match *self {
            Strength::Constant(k) => k,
            Strength::Coefficients([a, b, c]) => (if d > 0.0 { a / d } else { 0.0 }) + b + c * d,
        }
    }
}

/// A spring `a = k · (goal − x) + c · (v_goal − v)` with `c` negative for damping in the camera's notation
/// (physics.md 4.2; so `c · (v − v_goal)` with the object options' positive rate). The damping factor is capped at
/// `1 / dt`. Options 56 to 58 (position) and 63 to 66 (rotation) set it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Strength `k`.
    pub strength: Strength,
    /// Damping `c` (negative damps).
    pub damping: Strength,
}

impl Spring {
    /// A spring with constant strength and damping.
    pub const fn new(k: f32, c: f32) -> Spring {
        Spring { strength: Strength::Constant(k), damping: Strength::Constant(c) }
    }

    /// The acceleration toward a goal at rest for position `x` and velocity `v`.
    pub fn acceleration(&self, x: Vec3, v: Vec3, goal: Vec3) -> Vec3 {
        self.acceleration_to(x, v, goal, [0.0; 3], f32::INFINITY)
    }

    /// The acceleration toward `goal` moving at `goal_velocity`, with the damping capped at `1 / dt`.
    pub fn acceleration_to(&self, x: Vec3, v: Vec3, goal: Vec3, goal_velocity: Vec3, dt: f32) -> Vec3 {
        let e = sub(goal, x);
        let d = length(e);
        let c = self.damping.at(d);
        let cap = if dt > 0.0 && dt.is_finite() { 1.0 / dt } else { f32::INFINITY };
        let c = c.clamp(-cap, cap);
        madd(scale(e, self.strength.at(d)), sub(v, goal_velocity), c)
    }

    /// One camera step (camera.md 4): velocity first, then position. Returns the new (position, velocity).
    pub fn step(&self, x: Vec3, v: Vec3, goal: Vec3, dt: f32) -> (Vec3, Vec3) {
        let v = madd(v, self.acceleration(x, v, goal), dt);
        (madd(x, v, dt), v)
    }

    /// One object step (physics.md 3.1 and 4.2): the acceleration taken at the frame start and integrated exactly,
    /// `x + v·dt + ½·a·dt²`, `v + a·dt`.
    pub fn step_exact(&self, x: Vec3, v: Vec3, goal: Vec3, goal_velocity: Vec3, dt: f32) -> (Vec3, Vec3) {
        let a = self.acceleration_to(x, v, goal, goal_velocity, dt);
        (madd(madd(x, v, dt), a, 0.5 * dt * dt), madd(v, a, dt))
    }

    /// One step of a rotation `r` with angular velocity `w` toward `goal`: the error is the rotation vector of
    /// `goal · rᵀ`; the orientation turns by the average angular velocity. Returns the new (rotation, angular
    /// velocity).
    pub fn step_rotation(&self, r: &Mat3, w: Vec3, goal: &Mat3, dt: f32) -> (Mat3, Vec3) {
        let err = rotation_vector(&mat_mul(goal, &transpose(r)));
        let d = length(err);
        let c = self.damping.at(d);
        let cap = if dt > 0.0 { 1.0 / dt } else { f32::INFINITY };
        let a = madd(scale(err, self.strength.at(d)), w, c.clamp(-cap, cap));
        let w1 = madd(w, a, dt);
        let turn = scale(add(w, w1), 0.5 * dt);
        (orthonormalize(&mat_mul(&axis_angle(turn), r)), w1)
    }
}

/// Smoothing parameters (scripts.md 2 kind 26, physics.md 4.3): fast speed F, slow speed S, fast time T, and for
/// the four-value form a limit A on how fast the speed itself may change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothParams {
    /// Fast speed F (units per second).
    pub fast: f32,
    /// Slow speed S, the speed at which the target is reached.
    pub slow: f32,
    /// Fast time T: the speed falls from F to S over this time.
    pub fast_time: f32,
    /// The four-value form's rate limit A (units/s², rad/s² for angles); `None` for the three-value form.
    pub max_speed_change: Option<f32>,
}

/// Default position smoothing record (1500, 0, 0.5, 4500), scaled by option 67 [X 0x260EB0].
pub const POSITION_SMOOTHING: [f32; 4] = [1500.0, 0.0, 0.5, 4500.0];
/// Default rotation smoothing record (15, 0, 0.5, 45), scaled by option 69 [X 0x260EC0].
pub const ROTATION_SMOOTHING: [f32; 4] = [15.0, 0.0, 0.5, 45.0];
/// Pose-weight easing (3, 1, 0.1) in weight units per second [X 0x260E60].
pub const POSE_WEIGHT_SMOOTHING: SmoothParams =
    SmoothParams { fast: 3.0, slow: 1.0, fast_time: 0.1, max_speed_change: None };

impl SmoothParams {
    /// A four-value record (F, S, T, A).
    pub const fn four(r: [f32; 4]) -> SmoothParams {
        SmoothParams { fast: r[0], slow: r[1], fast_time: r[2], max_speed_change: Some(r[3]) }
    }

    /// Option 67 with value `r`: (1500·r, 0, 0.5, 4500·r).
    pub fn position_option(r: f32) -> SmoothParams {
        let d = POSITION_SMOOTHING;
        SmoothParams::four([d[0] * r, d[1], d[2], d[3] * r])
    }

    /// Option 69 with value `r`: (15·r, 0, 0.5, 45·r).
    pub fn rotation_option(r: f32) -> SmoothParams {
        let d = ROTATION_SMOOTHING;
        SmoothParams::four([d[0] * r, d[1], d[2], d[3] * r])
    }

    /// Options 71 and 72: the rate limit as a ratio, `A = value × (F − S) / T`.
    pub fn with_rate_ratio(self, value: f32) -> SmoothParams {
        let a = if self.fast_time > 0.0 { value * (self.fast - self.slow) / self.fast_time } else { 0.0 };
        SmoothParams { max_speed_change: Some(a), ..self }
    }

    /// The deceleration over the final stretch, `(F − S) / T`.
    fn decel(&self) -> f32 {
        if self.fast_time > 0.0 { (self.fast - self.slow).max(0.0) / self.fast_time } else { 0.0 }
    }

    /// Length of the final stretch, `(F + S)·T / 2`.
    pub fn stretch(&self) -> f32 {
        (self.fast + self.slow) * self.fast_time * 0.5
    }

    /// The speed the law prescribes at distance `d`: F at `d ≥ (F + S)·T / 2`, else `√(S² + 2·((F − S)/T)·d)`.
    pub fn speed_at(&self, d: f32) -> f32 {
        let a = self.decel();
        if a == 0.0 {
            return if d > 0.0 { self.fast.max(self.slow) } else { self.slow };
        }
        if d >= self.stretch() {
            return self.fast;
        }
        crate::math::sqrt(self.slow * self.slow + 2.0 * a * d).min(self.fast)
    }

    /// Distance covered in `dt` from distance `d` along the profile exactly (constant speed F, then the constant
    /// deceleration, then speed S), never past the target; and the speed at the end.
    fn along_profile(&self, d: f32, dt: f32) -> (f32, f32) {
        let a = self.decel();
        let stretch = self.stretch();
        let mut v = self.speed_at(d);
        let mut left = dt;
        let mut moved = 0.0;
        if d > stretch && v > 0.0 {
            let t = ((d - stretch) / v).min(left);
            moved += v * t;
            left -= t;
        }
        if left > 0.0 && a > 0.0 && v > self.slow {
            let t = ((v - self.slow) / a).min(left);
            moved += (v - 0.5 * a * t) * t;
            v -= a * t;
            left -= t;
        }
        moved += v * left;
        (moved.min(d), v)
    }
}

/// A value moving toward a target by the smoothing law of physics.md 4.3. The three-value law has no memory: the
/// speed comes from the remaining distance alone. The four-value form keeps the current speed between frames and
/// changes it by at most `A·dt` per frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Smoother {
    /// Current speed (kept for the four-value form; reported for both).
    pub speed: f32,
}

impl Smoother {
    /// Distance covered in `dt` from distance `d`.
    fn advance(&mut self, p: &SmoothParams, d: f32, dt: f32) -> f32 {
        if d <= 0.0 || dt <= 0.0 {
            self.speed = 0.0;
            return 0.0;
        }
        if let Some(limit) = p.max_speed_change {
            let want = p.speed_at(d);
            let step = limit * dt;
            if (want - self.speed).abs() > step {
                // The speed changes by exactly ±A·dt; the value moves with that constant acceleration.
                let s = if want > self.speed { step } else { -step };
                let v1 = (self.speed + s).max(0.0);
                let moved = (0.5 * (self.speed + v1) * dt).clamp(0.0, d);
                self.speed = v1;
                return moved;
            }
        }
        let (moved, v) = p.along_profile(d, dt);
        self.speed = v;
        moved
    }

    /// One step of a scalar toward `target`; returns the new value.
    pub fn step(&mut self, p: &SmoothParams, value: f32, target: f32, dt: f32) -> f32 {
        let d = (target - value).abs();
        let m = self.advance(p, d, dt);
        if target >= value { value + m } else { value - m }
    }

    /// One step of a point toward `target`, along the straight line to it.
    pub fn step_vec3(&mut self, p: &SmoothParams, value: Vec3, target: Vec3, dt: f32) -> Vec3 {
        let e = sub(target, value);
        let d = length(e);
        let m = self.advance(p, d, dt);
        if d > 0.0 && m < d { madd(value, e, m / d) } else { target }
    }

    /// One step of a rotation toward `target` about the axis of the remaining rotation (angle units).
    pub fn step_rotation(&mut self, p: &SmoothParams, value: &Mat3, target: &Mat3, dt: f32) -> Mat3 {
        let err = rotation_vector(&mat_mul(target, &transpose(value)));
        let d = length(err);
        let m = self.advance(p, d, dt);
        if d <= 0.0 || m >= d {
            return *target;
        }
        orthonormalize(&mat_mul(&axis_angle(scale(normalize(err), m)), value))
    }
}

/// How an animated or restored position or rotation is driven toward its goal (options 55, 61, 73, 74, 76;
/// physics.md 4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveMode {
    /// 0: the drive does not move the object.
    None,
    /// 1: spring toward the goal.
    Spring,
    /// 2: follow: set to the goal (and its velocity) every frame.
    Follow,
    /// 3: smooth along the straight line; becomes locked (7) on arrival.
    Smooth,
    /// 4: a cubic Hermite curve timed to arrive at the goal's match time.
    Curve,
    /// 5: left to the physics step.
    Physics,
    /// 6: smooth, forced to arrive by the match time.
    SmoothForced,
    /// 7: locked (like follow).
    Locked,
    /// 8: spring, locked: the drive does not move the object.
    SpringLocked,
    /// 9: smooth, never lock.
    SmoothNeverLock,
}

impl DriveMode {
    /// From the option value (options 55, 61, 73, 74).
    pub fn from_code(v: i16) -> Option<DriveMode> {
        use DriveMode::*;
        Some(match v {
            0 => None,
            1 => Spring,
            2 => Follow,
            3 => Smooth,
            4 => Curve,
            5 => Physics,
            6 => SmoothForced,
            7 => Locked,
            8 => SpringLocked,
            9 => SmoothNeverLock,
            _ => return Option::None,
        })
    }

    /// Option 76, the look-at drive (rotation only): as [`Self::from_code`], except that 3 means "smooth, never
    /// lock".
    pub fn look_at_from_code(v: i16) -> Option<DriveMode> {
        if v == 3 { Some(DriveMode::SmoothNeverLock) } else { DriveMode::from_code(v) }
    }

    /// The default drive of an animated object with no option: smooth for a solid free to move, else follow.
    pub fn default_for(free_solid: bool) -> DriveMode {
        if free_solid { DriveMode::Smooth } else { DriveMode::Follow }
    }
}

/// A drive's goal this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Goal {
    /// Goal position (or, for rotations, unused).
    pub position: Vec3,
    /// Goal velocity (angular velocity for rotations).
    pub velocity: Vec3,
    /// Time left to the goal's match time (curve and forced smooth drives).
    pub time_left: Option<f32>,
}

/// The state a drive keeps between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DriveState {
    /// Velocity (angular velocity for rotations).
    pub velocity: Vec3,
    /// Smoothing state.
    pub smoother: Smoother,
    /// A smooth drive (3, 6) arrived and now behaves as locked (7).
    pub locked: bool,
}

/// The settings of a drive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    /// Mode.
    pub mode: DriveMode,
    /// Spring for the spring mode.
    pub spring: Spring,
    /// Smoothing for the smooth modes.
    pub smooth: SmoothParams,
}

/// A cubic Hermite from (p0, v0) to (p1, v1) over time `t`, evaluated at `s` seconds: position and velocity.
fn hermite(p0: Vec3, v0: Vec3, p1: Vec3, v1: Vec3, t: f32, s: f32) -> (Vec3, Vec3) {
    let u = (s / t).clamp(0.0, 1.0);
    let (u2, u3) = (u * u, u * u * u);
    let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    let p = add(add(scale(p0, h00), scale(v0, h10 * t)), add(scale(p1, h01), scale(v1, h11 * t)));
    let d00 = (6.0 * u2 - 6.0 * u) / t;
    let d10 = 3.0 * u2 - 4.0 * u + 1.0;
    let d01 = (-6.0 * u2 + 6.0 * u) / t;
    let d11 = 3.0 * u2 - 2.0 * u;
    let v = add(add(scale(p0, d00), scale(v0, d10)), add(scale(p1, d01), scale(v1, d11)));
    (p, v)
}

impl Drive {
    /// The mode in effect (a smooth drive that arrived is locked).
    pub fn effective(&self, state: &DriveState) -> DriveMode {
        match self.mode {
            DriveMode::Smooth | DriveMode::SmoothForced if state.locked => DriveMode::Locked,
            m => m,
        }
    }

    /// One step of a position `x` toward `goal`. Returns the new position, or `None` when the drive leaves the
    /// object where it is (modes 0 and 8) or to the physics step (mode 5).
    pub fn step(&self, state: &mut DriveState, x: Vec3, goal: &Goal, dt: f32) -> Option<Vec3> {
        match self.effective(state) {
            DriveMode::None | DriveMode::SpringLocked | DriveMode::Physics => None,
            DriveMode::Spring => {
                let (x, v) = self.spring.step_exact(x, state.velocity, goal.position, goal.velocity, dt);
                state.velocity = v;
                Some(x)
            }
            DriveMode::Follow | DriveMode::Locked => {
                state.velocity = goal.velocity;
                Some(goal.position)
            }
            DriveMode::Curve => {
                let t = goal.time_left.unwrap_or(0.0);
                if t <= dt {
                    // At or past the match time: the goal plus its velocity times the overshoot.
                    let over = (dt - t).max(0.0);
                    state.velocity = goal.velocity;
                    return Some(madd(goal.position, goal.velocity, over));
                }
                let (p, v) = hermite(x, state.velocity, goal.position, goal.velocity, t, dt);
                state.velocity = v;
                Some(p)
            }
            m @ (DriveMode::Smooth | DriveMode::SmoothForced | DriveMode::SmoothNeverLock) => {
                let e = sub(goal.position, x);
                let d = length(e);
                if d <= 0.0 {
                    state.velocity = goal.velocity;
                    state.locked |= m != DriveMode::SmoothNeverLock;
                    return Some(goal.position);
                }
                let dir = scale(e, 1.0 / d);
                // The goal's own speed toward the object is added to F and S; its sideways velocity is kept.
                let toward = (-dot(goal.velocity, dir)).max(0.0);
                let sideways = madd(goal.velocity, dir, -dot(goal.velocity, dir));
                let p =
                    SmoothParams { fast: self.smooth.fast + toward, slow: self.smooth.slow + toward, ..self.smooth };
                let mut m_dist = state.smoother.advance(&p, d, dt);
                if m == DriveMode::SmoothForced
                    && let Some(t) = goal.time_left
                {
                    m_dist = m_dist.max(d * dt / (t.max(0.0) + dt));
                }
                let m_dist = m_dist.min(d);
                let x1 = add(madd(x, dir, m_dist), scale(sideways, dt));
                state.velocity = add(scale(dir, state.smoother.speed), sideways);
                if m_dist >= d && m != DriveMode::SmoothNeverLock {
                    state.locked = true;
                }
                Some(x1)
            }
        }
    }

    /// One step of a rotation `r` toward `goal` (angular velocity in `goal.velocity`), with the same modes as
    /// [`Self::step`]; the curve mode eases the angle of the remaining rotation along a Hermite to zero.
    pub fn step_rotation(&self, state: &mut DriveState, r: &Mat3, goal: &Mat3, goal_w: &Goal, dt: f32) -> Option<Mat3> {
        match self.effective(state) {
            DriveMode::None | DriveMode::SpringLocked | DriveMode::Physics => None,
            DriveMode::Spring => {
                let (r, w) = self.spring.step_rotation(r, state.velocity, goal, dt);
                state.velocity = w;
                Some(r)
            }
            DriveMode::Follow | DriveMode::Locked => {
                state.velocity = goal_w.velocity;
                Some(*goal)
            }
            DriveMode::Curve => {
                let err = rotation_vector(&mat_mul(goal, &transpose(r)));
                let t = goal_w.time_left.unwrap_or(0.0);
                if t <= dt {
                    state.velocity = goal_w.velocity;
                    return Some(*goal);
                }
                // The remaining rotation vector eases to zero along a Hermite.
                let (rest, v) = hermite(err, scale(state.velocity, -1.0), [0.0; 3], [0.0; 3], t, dt);
                state.velocity = scale(v, -1.0);
                Some(orthonormalize(&mat_mul(&transpose(&axis_angle(rest)), goal)))
            }
            m @ (DriveMode::Smooth | DriveMode::SmoothForced | DriveMode::SmoothNeverLock) => {
                let err = rotation_vector(&mat_mul(goal, &transpose(r)));
                let d = length(err);
                let mut out = state.smoother.step_rotation(&self.smooth, r, goal, dt);
                if m == DriveMode::SmoothForced
                    && let Some(t) = goal_w.time_left
                    && d > 0.0
                {
                    let done = length(rotation_vector(&mat_mul(&out, &transpose(r))));
                    let need = d * dt / (t.max(0.0) + dt);
                    if done < need {
                        out = orthonormalize(&mat_mul(&axis_angle(scale(err, need.min(d) / d)), r));
                    }
                }
                state.velocity = scale(normalize(err), state.smoother.speed);
                if length(rotation_vector(&mat_mul(goal, &transpose(&out)))) <= 1e-6 && m != DriveMode::SmoothNeverLock
                {
                    state.locked = true;
                }
                Some(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{MAT3_IDENTITY, sin_cos};

    #[test]
    fn camera_spring_is_velocity_first_and_object_spring_exact() {
        let s = Spring::new(100.0, -10.0);
        let (x, v) = s.step([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0], 0.1);
        // a = 100, v = 10, x = 1.
        assert_eq!((x[0], v[0]), (1.0, 10.0));
        // Exact: x = ½·100·0.01 = 0.5.
        let (x, v) = s.step_exact([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], 0.1);
        assert!((x[0] - 0.5).abs() < 1e-6 && (v[0] - 10.0).abs() < 1e-5);
        // A damped spring settles on the goal.
        let (mut x, mut v) = ([0.0f32; 3], [0.0f32; 3]);
        for _ in 0..600 {
            (x, v) = s.step_exact(x, v, [5.0, -2.0, 1.0], [0.0; 3], 1.0 / 60.0);
        }
        assert!(length(sub(x, [5.0, -2.0, 1.0])) < 1e-3);
        // Three coefficients of the distance: c0/d + c1 + c2·d.
        assert_eq!(Strength::Coefficients([4.0, 2.0, 3.0]).at(2.0), 10.0);
        // The damping factor is capped at 1/dt.
        let hard = Spring::new(0.0, -1000.0);
        let a = hard.acceleration_to([0.0; 3], [10.0, 0.0, 0.0], [0.0; 3], [0.0; 3], 0.1);
        assert_eq!(a[0], -100.0);
    }

    #[test]
    fn rotation_spring_settles() {
        let s = Spring::new(60.0, -12.0);
        let (sn, cs) = sin_cos(1.0);
        let goal = [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]];
        let (mut r, mut w) = (MAT3_IDENTITY, [0.0f32; 3]);
        for _ in 0..600 {
            (r, w) = s.step_rotation(&r, w, &goal, 1.0 / 60.0);
        }
        assert!(length(rotation_vector(&mat_mul(&goal, &transpose(&r)))) < 1e-3);
    }

    #[test]
    fn three_value_law_vector_and_no_memory() {
        // physics.md 6: (1500, 0, 0.5) from d = 1000: final stretch 375; 625 units at 1500 (0.4167 s), then 0.5 s.
        let p = SmoothParams { fast: 1500.0, slow: 0.0, fast_time: 0.5, max_speed_change: None };
        assert_eq!(p.stretch(), 375.0);
        assert_eq!(p.speed_at(1000.0), 1500.0);
        let mut s = Smoother::default();
        let (mut x, mut t) = (0.0f32, 0.0f32);
        let dt = 1.0 / 600.0;
        while x < 1000.0 - 1e-3 && t < 5.0 {
            x = s.step(&p, x, 1000.0, dt);
            t += dt;
        }
        assert!((t - 0.9167).abs() < 0.003, "{t}");
        assert!(s.speed.abs() < 1.0);
        // No memory: a fresh smoother and a used one move the same from the same distance.
        let mut a = Smoother { speed: 1500.0 };
        let mut b = Smoother::default();
        assert_eq!(a.step(&p, 0.0, 100.0, 0.01), b.step(&p, 0.0, 100.0, 0.01));
        // F = 100, S = 10, T = 0.5: 100 units at 100, then the 27.5-unit stretch in 0.5 s.
        let p = SmoothParams { fast: 100.0, slow: 10.0, fast_time: 0.5, max_speed_change: None };
        let mut s = Smoother::default();
        let (mut x, mut t) = (0.0f32, 0.0f32);
        while x < 127.5 - 1e-4 && t < 10.0 {
            x = s.step(&p, x, 127.5, dt);
            t += dt;
        }
        assert!((t - 1.5).abs() < 0.01, "{t}");
        let mut s = Smoother::default();
        let v = s.step_vec3(&p, [0.0; 3], [0.0, 200.0, 0.0], 0.5);
        assert!((v[1] - 50.0).abs() < 1e-3);
        let mut s = Smoother::default();
        let rp = SmoothParams { fast: 1.0, slow: 0.0, fast_time: 0.1, max_speed_change: None };
        let (sn, cs) = sin_cos(2.0);
        let goal = [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]];
        let r = s.step_rotation(&rp, &MAT3_IDENTITY, &goal, 0.5);
        assert!((rotation_vector(&r)[2] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn four_value_rate_limit_and_options() {
        // The field-of-view smoothing (0.5, 0, 0.1, 10): from rest the speed grows by 10 rad/s² at most.
        let p = SmoothParams::four([0.5, 0.0, 0.1, 10.0]);
        let mut s = Smoother::default();
        let x = s.step(&p, 0.0, 1.0, 0.01);
        assert!((s.speed - 0.1).abs() < 1e-6);
        assert!((x - 0.0005).abs() < 1e-7, "{x}");
        assert_eq!(SmoothParams::position_option(2.0), SmoothParams::four([3000.0, 0.0, 0.5, 9000.0]));
        assert_eq!(SmoothParams::rotation_option(0.5), SmoothParams::four([7.5, 0.0, 0.5, 22.5]));
        let r = SmoothParams { fast: 100.0, slow: 0.0, fast_time: 0.5, max_speed_change: None }.with_rate_ratio(2.0);
        assert_eq!(r.max_speed_change, Some(400.0));
        // Pose weights: 3 per second, slowing over the last 0.2 to 1 per second.
        assert!((POSE_WEIGHT_SMOOTHING.stretch() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn drive_modes() {
        assert_eq!(DriveMode::from_code(3), Some(DriveMode::Smooth));
        assert_eq!(DriveMode::look_at_from_code(3), Some(DriveMode::SmoothNeverLock));
        assert_eq!(DriveMode::from_code(10), None);
        assert_eq!(DriveMode::default_for(true), DriveMode::Smooth);
        assert_eq!(DriveMode::default_for(false), DriveMode::Follow);
        let d = Drive {
            mode: DriveMode::Smooth,
            spring: Spring::new(0.0, 0.0),
            smooth: SmoothParams { fast: 10.0, slow: 10.0, fast_time: 0.0, max_speed_change: None },
        };
        let goal = Goal { position: [100.0, 0.0, 0.0], ..Goal::default() };
        let mut st = DriveState::default();
        assert_eq!(d.step(&mut st, [0.0; 3], &goal, 1.0), Some([10.0, 0.0, 0.0]));
        // Arrival locks the smooth drive: it then follows the goal.
        let near = Goal { position: [5.0, 0.0, 0.0], ..Goal::default() };
        assert_eq!(d.step(&mut st, [0.0; 3], &near, 1.0), Some([5.0, 0.0, 0.0]));
        assert!(st.locked);
        assert_eq!(d.step(&mut st, [0.0; 3], &goal, 1.0), Some([100.0, 0.0, 0.0]));
        // A goal moving toward the object adds its speed; sideways velocity is kept.
        let mut st = DriveState::default();
        let moving = Goal { position: [100.0, 0.0, 0.0], velocity: [-5.0, 2.0, 0.0], time_left: None };
        assert_eq!(d.step(&mut st, [0.0; 3], &moving, 1.0), Some([15.0, 2.0, 0.0]));
        // None and spring-locked leave the object; physics leaves it to the step; follow snaps.
        for m in [DriveMode::None, DriveMode::SpringLocked, DriveMode::Physics] {
            assert_eq!(Drive { mode: m, ..d }.step(&mut DriveState::default(), [0.0; 3], &goal, 1.0), None);
        }
        assert_eq!(Drive { mode: DriveMode::Follow, ..d }.step(&mut st, [0.0; 3], &goal, 1.0), Some(goal.position));
        // Forced smoothing arrives by the match time.
        let slow = Drive {
            mode: DriveMode::SmoothForced,
            smooth: SmoothParams { fast: 1.0, slow: 1.0, fast_time: 0.0, max_speed_change: None },
            ..d
        };
        let mut st = DriveState::default();
        let mut x = [0.0; 3];
        let mut left = 1.0f32;
        while left > -1e-6 {
            x = slow.step(&mut st, x, &Goal { time_left: Some(left), ..goal }, 0.1).unwrap();
            left -= 0.1;
        }
        assert!((x[0] - 100.0).abs() < 1e-3, "{x:?}");
        // The curve arrives at the goal with the goal's velocity at the match time, then runs on with it.
        let c = Drive { mode: DriveMode::Curve, ..d };
        let mut st = DriveState::default();
        let g = Goal { position: [100.0, 0.0, 0.0], velocity: [50.0, 0.0, 0.0], time_left: None };
        let mut x = [0.0; 3];
        let mut left = 1.0f32;
        for _ in 0..10 {
            x = c.step(&mut st, x, &Goal { time_left: Some(left), ..g }, 0.1).unwrap();
            left -= 0.1;
        }
        assert!((x[0] - 100.0).abs() < 0.5 && (st.velocity[0] - 50.0).abs() < 1.0, "{x:?} {:?}", st.velocity);
        let x = c.step(&mut st, x, &Goal { time_left: Some(0.0), ..g }, 0.1).unwrap();
        assert!((x[0] - 105.0).abs() < 1e-3);
    }
}
