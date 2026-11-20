//! The player controller: states, movement, jumps, context moves, attacks, power-ups, damage and death
//! (player-moves.md, physics.md 2, guards.md 4.3, interactables.md 5 and 7).

use alloc::vec::Vec;

use super::anim::{AnimOids, AnimRequest};
use super::consts::{PlayerConstants, Tunables};
use super::input::{
    Feature, FeatureKind, PlayerInput, PlayerWorld, closest_on_segment, horizontal_distance, polyline_length,
};
use super::status::{DamageKind, DamageOutcome, Hiding, PowerUp, Vitals};
use crate::collision::Ledge;
use crate::collision::world::SweepHit;
use crate::math::{Vec3, add, atan2, cos, cross, dot, length, madd, normalize, scale, sin, sqrt, sub};
use crate::physics::character::{CharacterShape, SlideParams, ground_below, move_and_slide};
use crate::physics::spring::{SmoothParams, Smoother};

/// Which jump rule started a jump (player-moves.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpKind {
    /// From the ground: rises the jump height.
    Ground,
    /// From a swing.
    Swing,
    /// Out of water.
    Water,
    /// From a ledge, pipe or edge.
    Edge,
    /// Toward a perch point or target.
    Target,
    /// The boosted second press.
    Boosted,
}

/// Attacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackKind {
    /// Sweep while moving.
    SweepMoving,
    /// Sweep standing.
    SweepStanding,
    /// Rush.
    Rush,
    /// Smash (from the air).
    Smash,
    /// Dive (power-up).
    Dive,
}

/// The player's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// On the ground (standing, walking, running, sneaking).
    Ground,
    /// In the air. `target` is the perch feature a jump is aimed at.
    Air {
        /// Aimed-at feature.
        target: Option<u32>,
    },
    /// Hanging from a ledge.
    LedgeHang {
        /// The ledge.
        ledge: Ledge,
    },
    /// On a pipe; `s` is the feet's distance from the bottom.
    Pipe {
        /// Feature id.
        id: u32,
        /// Feet distance along the pipe.
        s: f32,
        /// Last climb direction (+1 up, −1 down).
        dir: f32,
    },
    /// On a rail or rope; `d` is the distance along it, `dir` the walking direction.
    Rail {
        /// Feature id.
        id: u32,
        /// Distance along.
        d: f32,
        /// Direction (+1 or −1).
        dir: f32,
    },
    /// Standing on a perch point.
    Perch {
        /// Feature id.
        id: u32,
    },
    /// Swinging from a hook.
    Swing {
        /// Feature id.
        id: u32,
        /// Rope length.
        length: f32,
    },
    /// Flattened against a wall with outward normal `normal`.
    Flatten {
        /// Wall normal.
        normal: Vec3,
    },
    /// Ducking.
    Duck,
    /// In a hiding place.
    Hiding {
        /// Feature id.
        id: u32,
    },
    /// A sweep attack; `t` counts up.
    Sweep {
        /// Time in the attack.
        t: f32,
        /// Moving variant.
        moving: bool,
    },
    /// Rush attack.
    Rush {
        /// Time in the attack.
        t: f32,
    },
    /// Smash, falling.
    Smash,
    /// Dive: a hover, then the plunge (player-moves.md 6).
    Dive,
    /// Rolling-ball power-up (`t`: time rolling).
    Ball,
    /// Hit reaction.
    Hurt {
        /// Damage kind.
        kind: DamageKind,
        /// Time left.
        t: f32,
    },
    /// Dead; respawn is requested once `t` runs out.
    Dead {
        /// Damage kind.
        kind: DamageKind,
        /// Time left.
        t: f32,
        /// Whether the respawn request was raised.
        requested: bool,
    },
}

/// Something that happened this frame, for the object model to act on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerEvent {
    /// A jump started.
    Jumped(JumpKind),
    /// Landed on the ground, at this downward speed.
    Landed(f32),
    /// Grabbed a ledge.
    LedgeGrabbed,
    /// Attached to a feature (pipe, rail, rope, perch, hook).
    Attached(u32),
    /// Left a feature.
    Detached(u32),
    /// An attack started.
    Attack(AttackKind),
    /// A smash landed here: hits in the window that follows count (player-moves.md 6).
    SmashLanded {
        /// Position.
        position: Vec3,
        /// Contact window (0.25 s).
        window: f32,
    },
    /// A dive landed here.
    DiveLanded {
        /// Position.
        position: Vec3,
        /// Objects affected within this radius (300).
        affect_radius: f32,
        /// Coins knocked loose within this radius (500).
        coin_radius: f32,
    },
    /// Took damage.
    Damaged(DamageKind, DamageOutcome),
    /// Died.
    Died(DamageKind),
    /// The death has played: respawn the player (at the last checkpoint) with [`Player::respawn`].
    RespawnRequested,
    /// A power-up was used.
    PowerUsed(PowerUp),
    /// Spawn the power-up's object (decoy, mine) here.
    SpawnPowerObject(PowerUp, Vec3),
    /// Entered a hiding place.
    HidingEntered(u32),
    /// Left a hiding place.
    HidingLeft(u32),
}

/// What one update produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerOutput {
    /// Events, in order.
    pub events: Vec<PlayerEvent>,
    /// The animation to show.
    pub anim: Option<AnimRequest>,
    /// Restart the animation (the state changed).
    pub restart_anim: bool,
    /// Game-clock scale from power-ups (slow motion 0.25, speed-up 1.5, else 1).
    pub time_scale: f32,
}

/// How a jump steers in the air (player-moves.md 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SteerMode {
    /// Not decided yet: `full_since` is when the stick reached full deflection.
    Undecided {
        /// Jump time at which the stick reached full deflection.
        full_since: Option<f32>,
    },
    /// Short-steered: the coast target is the stick's wish itself.
    Short,
    /// Momentum is kept (the coast rule).
    Coast,
}

/// Sly.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    /// The spec's constants.
    pub consts: PlayerConstants,
    /// The port's tunables.
    pub tun: Tunables,
    /// Lookup-list OIDs of the animations.
    pub oids: AnimOids,
    /// Feet position (bottom of the collision capsule).
    pub position: Vec3,
    /// Velocity.
    pub velocity: Vec3,
    /// The left stick's reader (player-moves.md 1.2), and this frame's (direction, deflection).
    pub stick_reader: super::stick::StickReader,
    /// See [`Player::stick_reader`].
    pub stick_now: ([f32; 2], f32),
    /// Facing heading (radians about +z, 0 = +x).
    pub yaw: f32,
    /// State.
    pub state: State,
    /// Charms and lives.
    pub vitals: Vitals,
    /// Reward mask (vault power-up bits).
    pub reward_mask: u32,
    /// The power-up selected for the power button.
    pub selected_power: Option<PowerUp>,
    /// Time spent in the air in this jump or fall.
    pub air_time: f32,
    /// Time since the current jump started (None: falling without a jump).
    pub jump_time: Option<f32>,
    /// Whether this jump's boost (the second press) was used.
    pub second_used: bool,
    /// Air-steer factor for this jump (0.7 after water, 0.6 after a ledge jump with no stick, else 1).
    pub steer_factor: f32,
    /// How this jump steers (player-moves.md 3: the 0.07 s and 0.30 s stick timers).
    pub steer_mode: SteerMode,
    /// The jump button has been held since the take-off began.
    pub jump_held: bool,
    /// Ledge hang easing.
    pub hang_smoother: Smoother,
    /// The current death is the backward one (segment 234).
    pub backward_death: bool,
    /// Current turning speed (rad/s).
    pub turn_speed: f32,
    /// Invulnerability time left after losing a charm.
    pub flash: f32,
    /// Active timed power-up and its time left.
    pub active_power: Option<(PowerUp, f32)>,
    /// Time in the current state.
    pub state_time: f32,
    /// Stick deflection used on the ground this frame (after dead zone and caps).
    pub deflection: f32,
}

fn heading(yaw: f32) -> Vec3 {
    [cos(yaw), sin(yaw), 0.0]
}

fn yaw_of(v: Vec3) -> f32 {
    atan2(v[1], v[0])
}

fn wrap_angle(a: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    crate::math::rem_euclid(a + core::f32::consts::PI, tau) - core::f32::consts::PI
}

fn horizontal(v: Vec3) -> Vec3 {
    [v[0], v[1], 0.0]
}

impl Player {
    /// A player standing at `position` facing `yaw`, with the spec's constants and default tunables.
    pub fn new(position: Vec3, yaw: f32) -> Player {
        Player::with(PlayerConstants::SPEC, Tunables::default(), position, yaw)
    }

    /// A player with given constants and tunables.
    pub fn with(consts: PlayerConstants, tun: Tunables, position: Vec3, yaw: f32) -> Player {
        Player {
            consts,
            tun,
            oids: AnimOids::default(),
            position,
            velocity: [0.0; 3],
            yaw,
            stick_reader: super::stick::StickReader::default(),
            stick_now: ([0.0; 2], 0.0),
            state: State::Air { target: None },
            vitals: Vitals::default(),
            reward_mask: 0,
            selected_power: None,
            air_time: 0.0,
            jump_time: None,
            second_used: false,
            steer_factor: 1.0,
            steer_mode: SteerMode::Undecided { full_since: None },
            jump_held: false,
            hang_smoother: Smoother::default(),
            backward_death: false,
            turn_speed: 0.0,
            flash: 0.0,
            active_power: None,
            state_time: 0.0,
            deflection: 0.0,
        }
    }

    /// The collision shape.
    pub fn shape(&self) -> CharacterShape {
        match self.state {
            State::Ball => CharacterShape::Sphere { radius: self.consts.ball_radius },
            _ => CharacterShape::Capsule { height: self.tun.height, radius: self.tun.radius },
        }
    }

    fn shape_origin(&self) -> Vec3 {
        let r = match self.shape() {
            CharacterShape::Sphere { radius } | CharacterShape::Capsule { radius, .. } => radius,
        };
        add(self.position, [0.0, 0.0, r])
    }

    fn set_from_origin(&mut self, origin: Vec3) {
        let r = match self.shape() {
            CharacterShape::Sphere { radius } | CharacterShape::Capsule { radius, .. } => radius,
        };
        self.position = sub(origin, [0.0, 0.0, r]);
    }

    /// The hidden rule that applies now (guards.md 4.3).
    pub fn hiding(&self, world_in_cover: bool) -> Hiding {
        match self.state {
            State::Flatten { .. } => Hiding::Flattened { facing: heading(self.yaw), detectable: false },
            State::Duck if world_in_cover => Hiding::Ducking { facing: heading(self.yaw), detectable: false },
            State::Hiding { .. } => Hiding::InHidingPlace { still: length(horizontal(self.velocity)) < 1.0 },
            _ => Hiding::None,
        }
    }

    /// Whether the player is in the air.
    pub fn airborne(&self) -> bool {
        matches!(self.state, State::Air { .. } | State::Smash | State::Dive)
    }

    fn set_state(&mut self, s: State, out: &mut PlayerOutput) {
        if core::mem::discriminant(&s) != core::mem::discriminant(&self.state) {
            out.restart_anim = true;
            self.state_time = 0.0;
        }
        self.state = s;
    }

    /// The stick as the game reads it this frame ([`super::stick`], player-moves.md 1.2), with `cap`: (unit world
    /// direction, deflection 0..1).
    pub fn stick(&self, input: &PlayerInput, cap: f32) -> (Vec3, f32) {
        let (dir, u) = self.stick_now;
        if u <= 0.0 {
            return ([0.0; 3], 0.0);
        }
        let w = PlayerInput { stick: dir, ..*input }.world_stick();
        (normalize(w), u.min(cap))
    }

    /// Run speed for deflection `u` (player-moves.md 2): the two locomotion segments blended with weights 1 − u
    /// and u, each speed weighted by its left foot's plant-to-lift interval (131 and 7 frames):
    /// `((1 − u)·131·v₁₃₀ + u·7·v₁₃₁) / ((1 − u)·131 + u·7)`. 38.3 at u = 0, 68.7 at 0.5, 638.4 at 1.
    pub fn run_speed(&self, u: f32) -> f32 {
        let c = &self.consts;
        let (d0, d1) = (c.run_slow_interval, c.run_fast_interval);
        let blended = (1.0 - u) * d0 + u * d1;
        if blended <= 0.0 {
            return c.run_slow;
        }
        ((1.0 - u) * c.run_slow * d0 + u * c.run_fast * d1) / blended
    }

    /// Starts a jump of `kind` (player-moves.md 3). The jump never lowers an upward speed the player already has.
    pub fn start_jump(&mut self, kind: JumpKind, out: &mut PlayerOutput) {
        let c = self.consts;
        let vz = self.velocity[2];
        let rr = c.root_ratio();
        let new = match kind {
            JumpKind::Ground => c.ground_jump_speed(),
            JumpKind::Swing => c.swing_jump * rr,
            JumpKind::Water => c.water_jump * rr,
            JumpKind::Edge => c.edge_jump * rr,
            JumpKind::Target => c.target_jump.eval(vz),
            JumpKind::Boosted => c.boosted_jump.eval(vz),
        };
        self.velocity[2] = vz.max(new);
        self.steer_factor = match kind {
            JumpKind::Water => c.air_steer_after_water,
            JumpKind::Edge => c.air_steer_after_edge,
            _ => 1.0,
        };
        self.jump_time = Some(0.0);
        self.air_time = 0.0;
        self.second_used = matches!(kind, JumpKind::Boosted | JumpKind::Target);
        self.jump_held = true;
        self.steer_mode = SteerMode::Undecided { full_since: None };
        let st = State::Air { target: None };
        self.set_state(st, out);
        out.events.push(PlayerEvent::Jumped(kind));
    }

    /// Applies damage of `kind` (player-moves.md 7). With a charm left, every kind uses one charm and flashes the
    /// player for 2 s (water and pits put him back afterwards); otherwise a life is lost and the player dies.
    /// Ignored while flashing after an earlier hit, and when already dead.
    pub fn damage(&mut self, kind: DamageKind, out: &mut PlayerOutput) -> DamageOutcome {
        if self.ignores(kind) {
            return DamageOutcome::Ignored;
        }
        let r = self.vitals.hit(kind);
        out.events.push(PlayerEvent::Damaged(kind, r));
        // A crush sets the velocity straight down, with or without a charm (player-moves.md 7).
        if kind == DamageKind::Crush {
            self.velocity = self.consts.crush_velocity;
        }
        match r {
            DamageOutcome::CharmLost => {
                self.flash = self.consts.charm_flash;
                let t = self.tun.hurt_time;

                if matches!(kind, DamageKind::Water | DamageKind::Pit) {
                    self.velocity = [0.0; 3];
                }
                self.set_state(State::Hurt { kind, t }, out);
            }
            DamageOutcome::LifeLost | DamageOutcome::GameOver => {
                // Moving backward faster than 250 relative to the facing: the backward death, turned away from
                // the motion by clamp(−1 + 0.004·speed, 0, 1) of the angle.
                let h = horizontal(self.velocity);
                let speed = length(h);
                let back = -dot(h, heading(self.yaw));
                self.backward_death = back > 250.0;
                if self.backward_death {
                    let [a, b, c] = self.consts.death_turn;
                    let f = (a + b * speed + c * speed * speed).clamp(0.0, 1.0);
                    let away = yaw_of(scale(h, -1.0));
                    self.yaw = wrap_angle(self.yaw + wrap_angle(away - self.yaw) * f);
                }
                self.velocity = [0.0; 3];
                let t = self.tun.death_time;
                self.set_state(State::Dead { kind, t, requested: false }, out);
                out.events.push(PlayerEvent::Died(kind));
            }
            DamageOutcome::Ignored => {}
        }
        r
    }

    /// Whether a hit of `kind` is ignored now (player-moves.md 7): in a damage reaction or dying, or flashing after a
    /// charm hit (blunt, electric, fire, crush, thrown). Water and pits ignore the flash; they are ignored instead on
    /// a hook, pipe or ledge, in an aimed jump, already in a water or pit reaction, or hidden in a barrel.
    pub fn ignores(&self, kind: DamageKind) -> bool {
        if matches!(self.state, State::Dead { .. }) {
            return true;
        }
        match kind {
            DamageKind::Water | DamageKind::Pit => matches!(
                self.state,
                State::Swing { .. }
                    | State::Pipe { .. }
                    | State::LedgeHang { .. }
                    | State::Air { target: Some(_) }
                    | State::Hiding { .. }
                    | State::Hurt { kind: DamageKind::Water | DamageKind::Pit, .. }
            ),
            _ => self.flash > 0.0 || matches!(self.state, State::Hurt { .. }),
        }
    }

    /// The player's origin: 90 above the feet (player-moves.md 1.1). Specs give heights relative to it.
    pub fn origin(&self) -> Vec3 {
        add(self.position, [0.0, 0.0, self.consts.origin_height])
    }

    /// Turns the facing toward `target` with the turn smoothing (player-moves.md 2): the smoothing of the heading
    /// error gives a target turning speed, reached with at most the rate limit.
    fn turn_toward(&mut self, target: f32, running: bool, speed: f32, u: f32, dt: f32) {
        let p = self.consts.turn_params(running, speed, u);
        let err = wrap_angle(target - self.yaw);
        let want = p.speed_at(err.abs()) * err.signum();
        let step = p.max_speed_change.unwrap_or(f32::INFINITY) * dt;
        let w1 = self.turn_speed + (want - self.turn_speed).clamp(-step, step);
        let mut turn = 0.5 * (self.turn_speed + w1) * dt;
        if turn.abs() > err.abs() {
            turn = err;
        }
        self.turn_speed = if (err - turn).abs() < 1e-6 { 0.0 } else { w1 };
        self.yaw = wrap_angle(self.yaw + turn);
    }

    /// Puts the player back at `position` facing `yaw`, alive and falling onto the ground there.
    pub fn respawn(&mut self, position: Vec3, yaw: f32) {
        self.position = position;
        self.yaw = yaw;
        self.velocity = [0.0; 3];
        self.state = State::Air { target: None };
        self.state_time = 0.0;
        self.air_time = 0.0;
        self.jump_time = None;
        self.flash = 0.0;
    }

    /// One frame.
    pub fn update(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>) -> PlayerOutput {
        let mut out = PlayerOutput { time_scale: 1.0, ..Default::default() };
        self.stick_now = self.stick_reader.read_axes(input.stick);
        self.state_time += dt;
        self.flash = (self.flash - dt).max(0.0);
        self.update_power(dt, input, &mut out);
        match self.state {
            State::Ground => self.ground(dt, input, world, &mut out),
            State::Air { target } => self.air(dt, input, world, target, &mut out),
            State::LedgeHang { ledge } => self.ledge(dt, input, world, ledge, &mut out),
            State::Pipe { id, s, dir } => self.pipe(dt, input, world, id, s, dir, &mut out),
            State::Rail { id, d, dir } => self.rail(dt, input, world, id, d, dir, &mut out),
            State::Perch { id } => self.perch(input, world, id, &mut out),
            State::Swing { id, length } => self.swing(dt, input, world, id, length, &mut out),
            State::Flatten { normal } => self.flatten(dt, input, world, normal, &mut out),
            State::Duck => self.duck(dt, input, world, &mut out),
            State::Hiding { id } => self.hiding_place(dt, input, world, id, &mut out),
            State::Sweep { t, moving } => {
                let t = t + dt;
                self.walk(dt, world, horizontal(scale(self.velocity, 0.9)));
                if t >= self.tun.sweep_time - 1e-4 {
                    self.set_state(State::Ground, &mut out);
                } else {
                    self.state = State::Sweep { t, moving };
                }
            }
            State::Rush { t } => {
                let t = t + dt;
                let v = scale(heading(self.yaw), self.consts.rush_speed);
                self.velocity = [v[0], v[1], self.velocity[2]];
                self.walk(dt, world, v);
                if t >= self.consts.rush_time - 1e-4 {
                    self.set_state(State::Ground, &mut out);
                } else {
                    self.state = State::Rush { t };
                }
            }
            State::Smash | State::Dive => self.air(dt, input, world, None, &mut out),
            State::Ball => self.ball(dt, input, world, &mut out),
            State::Hurt { kind, t } => {
                let t = t - dt;
                if !matches!(kind, DamageKind::Water | DamageKind::Pit) {
                    self.fall_or_walk(dt, world, &mut out);
                }
                if t <= 0.0 {
                    if matches!(kind, DamageKind::Water | DamageKind::Pit) {
                        // With a charm, water and pits put the player back (player-moves.md 8.1).
                        out.events.push(PlayerEvent::RespawnRequested);
                    }
                    let s = if self.airborne_now(world) { State::Air { target: None } } else { State::Ground };
                    self.set_state(s, &mut out);
                } else {
                    self.state = State::Hurt { kind, t };
                }
            }
            State::Dead { kind, t, requested } => {
                let t = t - dt;
                let mut requested = requested;
                if t <= 0.0 && !requested {
                    requested = true;
                    out.events.push(PlayerEvent::RespawnRequested);
                }
                self.state = State::Dead { kind, t, requested };
            }
        }
        if !matches!(self.state, State::Dead { .. } | State::Hurt { kind: DamageKind::Water, .. })
            && self.in_water(world)
        {
            self.damage(DamageKind::Water, &mut out);
        }
        out.anim = Some(self.anim_request());
        if let Some((p, _)) = self.active_power {
            out.time_scale = match p {
                PowerUp::SlowMotion => self.consts.slow_motion_scale,
                PowerUp::SpeedUp => 1.0 + 0.5,
                _ => 1.0,
            };
        }
        out
    }

    fn in_water(&self, world: &PlayerWorld<'_>) -> bool {
        world.features.iter().any(|f| match f.kind {
            FeatureKind::Water { surface, min, max } => {
                let p = self.position;
                p[2] < surface && p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1]
            }
            _ => false,
        })
    }

    fn airborne_now(&self, world: &PlayerWorld<'_>) -> bool {
        ground_below(world.cw, &world.filter, self.shape(), self.shape_origin(), 1.0, self.tun_ground_z()).is_none()
    }

    fn tun_ground_z(&self) -> f32 {
        self.consts.ground_z
    }

    fn update_power(&mut self, dt: f32, input: &PlayerInput, out: &mut PlayerOutput) {
        if let Some((p, t)) = self.active_power {
            let t = t - dt;
            self.active_power = if t > 0.0 { Some((p, t)) } else { None };
        }
        if !input.power.pressed || matches!(self.state, State::Dead { .. } | State::Ball) {
            return;
        }
        let Some(p) = self.selected_power.filter(|p| p.available(self.reward_mask)) else { return };
        match p {
            PowerUp::SlowMotion | PowerUp::SpeedUp => self.active_power = Some((p, self.tun.power_time)),
            PowerUp::Decoy | PowerUp::Mine => out.events.push(PlayerEvent::SpawnPowerObject(p, self.position)),
            PowerUp::RollingBall => {
                // Launch: keep the motion's direction at 850 or more, or 850 along the facing from below 150.
                let c = self.consts;
                let h = horizontal(self.velocity);
                let s = length(h);
                let v = if s >= c.ball_launch_min {
                    scale(h, s.max(c.ball_launch) / s)
                } else {
                    scale(heading(self.yaw), c.ball_launch)
                };
                self.velocity = [v[0], v[1], self.velocity[2]];
                self.set_state(State::Ball, out);
            }
            PowerUp::Dive => {
                if !matches!(self.state, State::Air { .. }) || self.air_time < self.consts.dive_min_air {
                    return;
                }
                self.set_state(State::Dive, out);
                out.events.push(PlayerEvent::Attack(AttackKind::Dive));
            }
            PowerUp::Stun => {}
        }
        out.events.push(PlayerEvent::PowerUsed(p));
    }

    /// Moves along the ground by `v` for `dt` (physics.md 5.1): the leg meets obstacles up to its length (35) and
    /// steps over them; there is no snap beyond the leg's 2-unit allowance. Returns the slide.
    fn walk(&mut self, dt: f32, world: &PlayerWorld<'_>, v: Vec3) -> crate::physics::SlideResult {
        let params = SlideParams {
            ground_z: self.consts.ground_z,
            step_up: Some(self.consts.leg_length),
            snap_down: Some(self.tun.ground_probe),
            ..SlideParams::default()
        };
        let r = move_and_slide(world.cw, &world.filter, self.shape(), self.shape_origin(), scale(v, dt), &params);
        self.set_from_origin(r.position);
        r
    }

    fn fall_or_walk(&mut self, dt: f32, world: &PlayerWorld<'_>, out: &mut PlayerOutput) {
        if self.airborne_now(world) {
            self.integrate_air(dt, world, [0.0; 3], 0.0, out);
        } else {
            self.velocity = [0.0; 3];
        }
    }

    /// The ground's normal under the player (up when there is none).
    fn ground_normal(&self, world: &PlayerWorld<'_>) -> Vec3 {
        ground_below(
            world.cw,
            &world.filter,
            self.shape(),
            self.shape_origin(),
            self.tun.ground_probe + 1.0,
            self.consts.ground_z,
        )
        .map_or([0.0, 0.0, 1.0], |h| h.normal)
    }

    fn ground(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, out: &mut PlayerOutput) {
        let c = self.consts;
        let cap = if world.in_shadow { c.sneak_cap } else { 1.0 };
        let (dir, u) = self.stick(input, cap);
        self.deflection = u;
        // Context moves first.
        if input.action.pressed && self.try_context_ground(world, out) {
            return;
        }
        if input.jump.pressed {
            if let Some(t) = self.aim_target(world, dir) {
                self.start_jump(JumpKind::Ground, out);
                self.aim_at(t, world);
            } else {
                self.start_jump(JumpKind::Ground, out);
            }
            return;
        }
        if input.rush.pressed {
            self.set_state(State::Rush { t: 0.0 }, out);
            out.events.push(PlayerEvent::Attack(AttackKind::Rush));
            return;
        }
        if input.attack.pressed {
            let moving = length(horizontal(self.velocity)) > 50.0;
            self.set_state(State::Sweep { t: 0.0, moving }, out);
            out.events.push(PlayerEvent::Attack(if moving {
                AttackKind::SweepMoving
            } else {
                AttackKind::SweepStanding
            }));
            return;
        }
        if input.duck.held {
            self.set_state(State::Duck, out);
            return;
        }
        // Turn toward the stick with the turn smoothing; the speed is scaled by clamp(1 − |Δθ|, 0, 1).
        let mut turn = 1.0;
        if u > 0.0 {
            let target = yaw_of(dir);
            let speed = length(horizontal(self.velocity));
            self.turn_toward(target, true, speed, u, dt);
            turn = c.turn_factor(wrap_angle(target - self.yaw));
        } else {
            self.turn_speed = 0.0;
        }
        // The wanted velocity along the facing, projected onto the ground plane, reached through the ground
        // contact's friction (1.5 × |g|).
        let n = self.ground_normal(world);
        let speed = if u > 0.0 { self.run_speed(u) * turn } else { 0.0 };
        let f = heading(self.yaw);
        let want = scale(madd(f, n, -dot(f, n)), speed);
        let diff = sub(want, self.velocity);
        let dl = length(diff);
        let step = c.ground_accel() * dt;
        let v = if dl > step { madd(self.velocity, diff, step / dl) } else { want };
        self.velocity = v;
        let r = self.walk(dt, world, v);
        if r.ground.is_none() {
            // The ground fell away beyond the leg: fall.
            self.jump_time = None;
            self.air_time = 0.0;
            self.steer_factor = 1.0;
            self.second_used = false;
            self.jump_held = false;
            self.steer_mode = SteerMode::Coast;
            self.set_state(State::Air { target: None }, out);
        }
    }

    /// Air physics (physics.md 2 and 3.1, player-moves.md 3): gravity plus the fall drag, integrated exactly over
    /// the step; air steering toward the coast target; collisions; landing. `wish` is the stick's direction scaled
    /// by its deflection and `steer_speed` the air-steer speed (0: no steering at all, as in aimed jumps and
    /// attacks). Returns the ground hit when the player lands.
    fn integrate_air(
        &mut self,
        dt: f32,
        world: &PlayerWorld<'_>,
        wish: Vec3,
        steer_speed: f32,
        out: &mut PlayerOutput,
    ) -> Option<SweepHit> {
        let c = self.consts;
        let vz0 = self.velocity[2];
        let az = c.gravity() + c.terminal.drag(c.gravity_ratio, vz0);
        let ah = if steer_speed > 0.0 { self.steer_acceleration(scale(wish, steer_speed)) } else { [0.0; 3] };
        let h0 = horizontal(self.velocity);
        let disp = add(madd(scale(self.velocity, dt), [ah[0], ah[1], az], 0.5 * dt * dt), [0.0; 3]);
        self.velocity = add(madd(h0, ah, dt), [0.0, 0.0, vz0 + az * dt]);
        let params = SlideParams { ground_z: c.ground_z, ..SlideParams::default() };
        let r = move_and_slide(world.cw, &world.filter, self.shape(), self.shape_origin(), disp, &params);
        self.set_from_origin(r.position);
        for hit in &r.hits {
            if hit.normal[2] < -0.3 && self.velocity[2] > 0.0 {
                self.velocity[2] = 0.0;
            }
            // Walls take the horizontal speed into them.
            if hit.normal[2].abs() < c.ground_z {
                let n = normalize(horizontal(hit.normal));
                let into = dot(self.velocity, n);
                if into < 0.0 {
                    self.velocity = madd(self.velocity, n, -into);
                }
            }
        }
        self.air_time += dt;
        if let Some(t) = &mut self.jump_time {
            *t += dt;
        }
        let _ = out;
        if self.velocity[2] <= 0.0 { r.ground } else { None }
    }

    /// The horizontal air-steer acceleration toward the coast target for the stick's wish `w` (player-moves.md 3).
    fn steer_acceleration(&self, w: Vec3) -> Vec3 {
        let c = self.consts;
        let h = horizontal(self.velocity);
        let s = length(h);
        let dir = if s > 1e-4 { scale(h, 1.0 / s) } else { heading(self.yaw) };
        let takeoff = self.jump_time.is_some_and(|t| t < c.takeoff_time);
        let wf = dot(w, dir);
        let target = if takeoff || self.steer_mode == SteerMode::Short || wf >= s || wf <= -0.25 * s {
            w
        } else {
            // Momentum kept: the current speed along the motion plus the wish's sideways part.
            add(scale(dir, s), madd(w, dir, -wf))
        };
        let cf = dot(target, dir);
        let side = madd(target, dir, -cf);
        let r = &c.air_steer_rates;
        let (up, down, across) = if takeoff { (r[3], r[4], r[5]) } else { (r[0], r[1], r[2]) };
        let rate_f = if s <= cf { up } else { down };
        add(scale(dir, rate_f * (cf - s)), scale(side, across))
    }

    /// Follows the stick and the jump button after a jump starts, deciding the steer mode (player-moves.md 3).
    fn update_steer_mode(&mut self, u: f32, jump_held: bool) {
        let c = self.consts;
        let Some(t) = self.jump_time else { return };
        if !jump_held {
            if self.jump_held && t < c.takeoff_time {
                self.steer_mode = SteerMode::Short;
            }
            self.jump_held = false;
        }
        if let SteerMode::Undecided { full_since } = self.steer_mode {
            let full = u >= 0.999;
            self.steer_mode = match full_since {
                None if full => SteerMode::Undecided { full_since: Some(t) },
                None if t > c.steer_full_window => SteerMode::Short,
                Some(f) if !full && t - f < c.steer_full_hold => SteerMode::Short,
                Some(f) if t - f >= c.steer_full_hold => SteerMode::Coast,
                m => SteerMode::Undecided { full_since: m },
            };
        }
    }

    /// Whether the ground is about to be reached: the base point moved by `v·0.1 + ½·g·0.1²` hits something.
    fn landing_soon(&self, world: &PlayerWorld<'_>) -> bool {
        let t = 0.1;
        let base = sub(self.position, [0.0, 0.0, self.consts.leg_length]);
        let d = madd(scale(self.velocity, t), [0.0, 0.0, self.consts.gravity()], 0.5 * t * t);
        let len = length(d);
        len > 0.0
            && world
                .cw
                .ray_cast(
                    add(self.position, [0.0, 0.0, 1.0]),
                    sub(add(base, d), self.position),
                    len + self.consts.leg_length,
                    &world.filter,
                )
                .is_some()
    }

    fn air(
        &mut self,
        dt: f32,
        input: &PlayerInput,
        world: &PlayerWorld<'_>,
        target: Option<u32>,
        out: &mut PlayerOutput,
    ) {
        let c = self.consts;
        let in_attack = matches!(self.state, State::Smash | State::Dive);
        let (dir, u) = self.stick(input, 1.0);
        self.update_steer_mode(u, input.jump.held || input.jump.pressed);
        if self.state == State::Dive {
            self.dive(dt, world, out);
            return;
        }
        if !in_attack {
            // Context: the context button catches hooks, pipes, rails, ropes and perch points.
            if input.action.pressed && self.try_context_air(world, out) {
                return;
            }
            if input.attack.pressed && self.air_time >= c.smash_min_air {
                // The smash has no speed of its own: the fall goes on under gravity and the drag.
                self.set_state(State::Smash, out);
                out.events.push(PlayerEvent::Attack(AttackKind::Smash));
                return;
            }
            // The second press is the boosted jump: once per jump, not in an aimed jump, not just before landing.
            if input.jump.pressed && !self.second_used && target.is_none() && !self.landing_soon(world) {
                let vz = self.velocity[2];
                self.velocity[2] = vz.max(c.boosted_jump.eval(vz));
                self.second_used = true;
                out.events.push(PlayerEvent::Jumped(JumpKind::Boosted));
            }
        }
        // Facing: the direction of motion above 300 units/s, else the stick.
        let h = horizontal(self.velocity);
        if !in_attack {
            if length(h) > c.face_motion_speed {
                self.yaw = yaw_of(h);
            } else if u > 0.0 {
                self.yaw = yaw_of(dir);
            }
        }
        let steer_speed =
            if target.is_some() || in_attack { 0.0 } else { c.air_steer_speed * c.root_ratio() * self.steer_factor };
        let landed = self.integrate_air(dt, world, scale(dir, u), steer_speed, out);
        // Landing on an aimed perch.
        if let Some(t) = target
            && self.velocity[2] <= 0.0
            && let Some(f) = world.features.iter().find(|f| f.id == t)
            && let FeatureKind::Perch { position } = f.kind
            && length(sub(position, self.position)) < self.tun.catch_radius
        {
            self.land_on_perch(t, position, out);
            return;
        }
        if landed.is_some() {
            self.land(out);
            return;
        }
        // Ledge grab while falling.
        if !in_attack
            && self.velocity[2] <= 0.0
            && let Some(l) = self.find_ledge(world)
        {
            self.grab_ledge(l, out);
        }
    }

    fn land(&mut self, out: &mut PlayerOutput) {
        let c = self.consts;
        let speed = -self.velocity[2];
        match self.state {
            State::Smash => {
                out.events.push(PlayerEvent::SmashLanded { position: self.position, window: c.smash_window })
            }
            State::Dive => out.events.push(PlayerEvent::DiveLanded {
                position: self.position,
                affect_radius: c.dive_affect_radius,
                coin_radius: c.dive_coin_radius,
            }),
            _ => {}
        }
        self.velocity[2] = 0.0;
        self.jump_time = None;
        self.second_used = false;
        self.set_state(State::Ground, out);
        out.events.push(PlayerEvent::Landed(speed));
    }

    /// The dive (player-moves.md 6): gravity off until frame 100; the velocity pulled toward zero at 10 per second
    /// (vertically only until frame 20); from frame 20 to 100 the root motion's 1556.3 units/s straight down; then
    /// ordinary gravity until the landing.
    fn dive(&mut self, dt: f32, world: &PlayerWorld<'_>, out: &mut PlayerOutput) {
        let c = self.consts;
        let t = self.state_time;
        if t <= c.dive_end {
            let k = (c.dive_damping * dt).min(1.0);
            let mut v = self.velocity;
            v[0] -= v[0] * k;
            v[1] -= v[1] * k;
            v[2] = if t <= c.dive_hover { v[2] - v[2] * k } else { -c.dive_plunge };
            self.velocity = v;
            let params = SlideParams { ground_z: c.ground_z, ..SlideParams::default() };
            let r = move_and_slide(world.cw, &world.filter, self.shape(), self.shape_origin(), scale(v, dt), &params);
            self.set_from_origin(r.position);
            self.air_time += dt;
            if r.ground.is_some() && v[2] <= 0.0 && t > c.dive_hover {
                self.land(out);
            }
            return;
        }
        if self.integrate_air(dt, world, [0.0; 3], 0.0, out).is_some() {
            self.land(out);
        }
    }

    /// The nearest ledge in the grab box around the hands (player-moves.md 3).
    fn find_ledge(&self, world: &PlayerWorld<'_>) -> Option<(Ledge, Vec3)> {
        let hands = add(self.position, [0.0, 0.0, self.tun.hand_height]);
        let mut best: Option<(f32, Ledge, Vec3)> = None;
        for l in world.ledges {
            let (q, _) = closest_on_segment(hands, l.ends[0], l.ends[1]);
            let dh = horizontal_distance(hands, q);
            let dv = (hands[2] - q[2]).abs();
            // Only from the wall's open side, facing the wall (the spec gives only the grab box; the port adds
            // these so a fall beside a wall from behind it does not grab).
            let open_side = dot(sub(hands, q), l.outward) >= 0.0;
            let facing = dot(heading(self.yaw), l.outward) <= 0.0;
            if open_side
                && facing
                && dh <= self.consts.ledge_grab_horizontal
                && dv <= self.consts.ledge_grab_vertical
                && best.is_none_or(|(b, ..)| dh < b)
            {
                best = Some((dh, *l, q));
            }
        }
        best.map(|(_, l, q)| (l, q))
    }

    /// The feet position for a hang at edge point `q`: the origin 37 out from the edge and 25 below it
    /// (player-moves.md 4), the feet 90 below the origin.
    fn hang_position(&self, ledge: &Ledge, q: Vec3) -> Vec3 {
        let [out_, _, down] = self.consts.hang_offset;
        sub(madd(q, ledge.outward, out_), [0.0, 0.0, down + self.consts.origin_height])
    }

    fn grab_ledge(&mut self, (ledge, q): (Ledge, Vec3), out: &mut PlayerOutput) {
        self.velocity = [0.0; 3];
        self.yaw = yaw_of(scale(ledge.outward, -1.0));
        self.hang_smoother = Smoother::default();
        self.set_state(State::LedgeHang { ledge }, out);
        let _ = q;
        out.events.push(PlayerEvent::LedgeGrabbed);
    }

    /// Hanging on a ledge: held still (no shimmy in Sly 1), eased onto the hang position with smoothing
    /// (1000, 0, 0.25); the only way off is the jump button, the ledge jump.
    fn ledge(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, ledge: Ledge, out: &mut PlayerOutput) {
        let (dir, u) = self.stick(input, 1.0);
        if input.jump.pressed {
            if u > 0.0 {
                self.velocity = scale(dir, self.consts.air_steer_speed * self.consts.root_ratio());
                self.start_jump(JumpKind::Edge, out);
                self.steer_factor = 1.0;
                self.yaw = yaw_of(dir);
            } else {
                // With no stick: straight up, short-steered, with the 0.6 air-steer factor.
                self.velocity = [0.0; 3];
                self.start_jump(JumpKind::Edge, out);
                self.steer_mode = SteerMode::Short;
            }
            return;
        }
        let hands = add(self.position, [0.0, 0.0, self.tun.hand_height]);
        let (q, _) = closest_on_segment(hands, ledge.ends[0], ledge.ends[1]);
        let goal = self.hang_position(&ledge, q);
        let [f, sl, t] = self.consts.hang_smooth;
        let p = SmoothParams { fast: f, slow: sl, fast_time: t, max_speed_change: None };
        self.position = self.hang_smoother.step_vec3(&p, self.position, goal, dt);
        let _ = world;
    }

    fn feature<'w>(world: &PlayerWorld<'w>, id: u32) -> Option<&'w Feature> {
        world.features.iter().find(|f| f.id == id)
    }

    /// The perch point the stick points at, within the target range (player-moves.md 3: a jump near a perch point
    /// is aimed at it).
    fn aim_target(&self, world: &PlayerWorld<'_>, dir: Vec3) -> Option<u32> {
        let mut best: Option<(f32, u32)> = None;
        for f in world.features {
            if let FeatureKind::Perch { position } = f.kind {
                let d = sub(position, self.position);
                let hd = horizontal_distance(position, self.position);
                if hd < 1.0 || hd > self.tun.target_range.min(self.consts.target_search) {
                    continue;
                }
                if dot(normalize(horizontal(d)), dir) < 0.8 {
                    continue;
                }
                if best.is_none_or(|(b, _)| hd < b) {
                    best = Some((hd, f.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Sets the horizontal velocity so the current upward speed carries the player onto feature `id` on the way
    /// down. If the target is out of reach of this speed, the jump flies at the air-steer speed toward it.
    fn aim_at(&mut self, id: u32, world: &PlayerWorld<'_>) {
        let Some(FeatureKind::Perch { position }) = Self::feature(world, id).map(|f| f.kind.clone()) else { return };
        let g = self.consts.gravity();
        let vz = self.velocity[2];
        let dz = position[2] - self.position[2];
        // z(t) = vz t + g t² / 2 = dz, later root.
        let disc = vz * vz + 2.0 * g * dz;
        let d = horizontal(sub(position, self.position));
        let t = if disc >= 0.0 { (-vz - sqrt(disc)) / g } else { -1.0 };
        let v = if t > 0.0 {
            scale(d, 1.0 / t)
        } else {
            scale(normalize(d), self.consts.air_steer_speed * self.consts.root_ratio())
        };
        self.velocity[0] = v[0];
        self.velocity[1] = v[1];
        self.yaw = yaw_of(d);
        self.state = State::Air { target: Some(id) };
    }

    fn land_on_perch(&mut self, id: u32, position: Vec3, out: &mut PlayerOutput) {
        self.position = position;
        self.velocity = [0.0; 3];
        self.jump_time = None;
        self.set_state(State::Perch { id }, out);
        out.events.push(PlayerEvent::Attached(id));
    }

    fn try_context_ground(&mut self, world: &PlayerWorld<'_>, out: &mut PlayerOutput) -> bool {
        // Hiding places.
        for f in world.features {
            if let FeatureKind::HidingPlace { position } = f.kind
                && horizontal_distance(position, self.position) < self.tun.hide_reach
            {
                self.position = [position[0], position[1], self.position[2]];
                self.velocity = [0.0; 3];
                self.set_state(State::Hiding { id: f.id }, out);
                out.events.push(PlayerEvent::HidingEntered(f.id));
                return true;
            }
        }
        if self.try_pipe(world, out) {
            return true;
        }
        // Flatten against a wall ahead.
        let chest = add(self.position, [0.0, 0.0, self.tun.height * 0.5 + self.tun.radius]);
        if let Some(h) =
            world.cw.ray_cast(chest, heading(self.yaw), self.tun.radius + self.tun.flatten_reach, &world.filter)
            && h.normal[2] < crate::collision::mesh::WALL_Z
        {
            let n = normalize(horizontal(h.normal));
            self.position = madd([h.point[0], h.point[1], self.position[2]], n, self.tun.radius + 1.0);
            self.yaw = yaw_of(n);
            self.velocity = [0.0; 3];
            self.set_state(State::Flatten { normal: n }, out);
            return true;
        }
        false
    }

    fn try_pipe(&mut self, world: &PlayerWorld<'_>, out: &mut PlayerOutput) -> bool {
        for f in world.features {
            if let FeatureKind::Pipe { bottom, top } = f.kind {
                let (q, t) =
                    closest_on_segment(add(self.position, [0.0, 0.0, self.tun.hand_height * 0.5]), bottom, top);
                if horizontal_distance(q, self.position) <= self.consts.pipe_probe + self.tun.radius {
                    let len = length(sub(top, bottom));
                    let s = (t * len - self.tun.hand_height * 0.5).clamp(0.0, len);
                    self.attach_pipe(f.id, bottom, top, s, out);
                    return true;
                }
            }
        }
        false
    }

    fn pipe_limits(&self, bottom: Vec3, top: Vec3) -> (f32, f32) {
        let len = length(sub(top, bottom));
        let lo = self.consts.pipe_feet_end.min(len);
        let hi = (len - self.consts.pipe_hands_end - self.tun.hand_height).max(lo);
        (lo, hi)
    }

    fn attach_pipe(&mut self, id: u32, bottom: Vec3, top: Vec3, s: f32, out: &mut PlayerOutput) {
        let (lo, hi) = self.pipe_limits(bottom, top);
        let s = s.clamp(lo, hi);
        self.position = madd(bottom, normalize(sub(top, bottom)), s);
        self.velocity = [0.0; 3];
        self.set_state(State::Pipe { id, s, dir: 1.0 }, out);
        out.events.push(PlayerEvent::Attached(id));
    }

    #[allow(clippy::too_many_arguments)]
    fn pipe(
        &mut self,
        dt: f32,
        input: &PlayerInput,
        world: &PlayerWorld<'_>,
        id: u32,
        s: f32,
        dir: f32,
        out: &mut PlayerOutput,
    ) {
        let Some(FeatureKind::Pipe { bottom, top }) = Self::feature(world, id).map(|f| f.kind.clone()) else {
            self.set_state(State::Air { target: None }, out);
            return;
        };
        let axis = normalize(sub(top, bottom));
        if input.jump.pressed {
            out.events.push(PlayerEvent::Detached(id));
            let away = scale(heading(self.yaw), -1.0);
            self.velocity =
                scale(away, self.consts.air_steer_speed * self.consts.root_ratio() * self.consts.air_steer_after_edge);
            self.yaw = yaw_of(away);
            self.start_jump(JumpKind::Edge, out);
            return;
        }
        if input.duck.pressed {
            out.events.push(PlayerEvent::Detached(id));
            self.jump_time = None;
            self.air_time = 0.0;
            self.set_state(State::Air { target: None }, out);
            return;
        }
        // Climb with the stick's forward axis; reversing needs a change of 0.5 (player-moves.md 4).
        let v = input.stick[1];
        let mut dir = dir;
        if v * dir < 0.0 && v.abs() >= self.consts.pipe_reverse {
            dir = -dir;
        }
        let u = if v * dir > 0.0 { v.abs() } else { 0.0 };
        let (lo, hi) = self.pipe_limits(bottom, top);
        let [s155, s156] = self.consts.pipe_speeds;
        let s = (s + dir * (s155 + (s156 - s155) * u) * if u > 0.0 { 1.0 } else { 0.0 } * dt).clamp(lo, hi);
        self.position = madd(bottom, axis, s);
        self.deflection = u;
        self.state = State::Pipe { id, s, dir };
    }

    fn try_context_air(&mut self, world: &PlayerWorld<'_>, out: &mut PlayerOutput) -> bool {
        let hands = add(self.position, [0.0, 0.0, self.tun.hand_height]);
        for f in world.features {
            match &f.kind {
                FeatureKind::Hook { position } => {
                    let d = length(sub(*position, hands));
                    if d <= self.consts.hook_catch_radius + self.tun.hook_reach {
                        let len = length(sub(*position, self.position)).max(1.0);
                        self.set_state(State::Swing { id: f.id, length: len }, out);
                        out.events.push(PlayerEvent::Attached(f.id));
                        return true;
                    }
                }
                FeatureKind::Perch { position } => {
                    if length(sub(*position, self.position)) <= self.tun.catch_radius {
                        self.land_on_perch(f.id, *position, out);
                        return true;
                    }
                }
                FeatureKind::Rail { points } | FeatureKind::Rope { points } => {
                    if let Some((d, q)) = nearest_on_polyline(points, self.position)
                        && length(sub(q, self.position)) <= self.tun.catch_radius
                    {
                        let tangent = polyline_tangent(points, d);
                        let dir = if dot(tangent, heading(self.yaw)) >= 0.0 { 1.0 } else { -1.0 };
                        self.position = q;
                        self.velocity = [0.0; 3];
                        self.set_state(State::Rail { id: f.id, d, dir }, out);
                        out.events.push(PlayerEvent::Attached(f.id));
                        return true;
                    }
                }
                _ => {}
            }
        }
        self.try_pipe(world, out)
    }

    #[allow(clippy::too_many_arguments)]
    fn rail(
        &mut self,
        dt: f32,
        input: &PlayerInput,
        world: &PlayerWorld<'_>,
        id: u32,
        d: f32,
        dir: f32,
        out: &mut PlayerOutput,
    ) {
        let points = match Self::feature(world, id).map(|f| &f.kind) {
            Some(FeatureKind::Rail { points }) | Some(FeatureKind::Rope { points }) => points,
            _ => {
                self.set_state(State::Air { target: None }, out);
                return;
            }
        };
        if input.jump.pressed {
            out.events.push(PlayerEvent::Detached(id));
            self.start_jump(JumpKind::Ground, out);
            return;
        }
        let (sdir, u) = self.stick(input, 1.0);
        let tangent = scale(polyline_tangent(points, d), dir);
        let along = dot(sdir, tangent);
        let mut dir = dir;
        if u > 0.0 && along < self.consts.rail_reverse {
            dir = -dir;
        }
        let tangent = scale(polyline_tangent(points, d), dir);
        let speed = if u > 0.0 { u * self.tun.rail_speed } else { 0.0 };
        let total = polyline_length(points);
        let nd = d + dir * speed * dt;
        if nd < 0.0 || nd > total {
            // Off the end: fall with the walking velocity.
            out.events.push(PlayerEvent::Detached(id));
            self.velocity = scale(tangent, speed);
            self.jump_time = None;
            self.air_time = 0.0;
            self.set_state(State::Air { target: None }, out);
            return;
        }
        self.position = polyline_point(points, nd);
        self.velocity = scale(tangent, speed);
        if speed > 0.0 {
            self.yaw = yaw_of(tangent);
        }
        self.deflection = u;
        self.state = State::Rail { id, d: nd, dir };
    }

    fn perch(&mut self, input: &PlayerInput, world: &PlayerWorld<'_>, id: u32, out: &mut PlayerOutput) {
        let (dir, u) = self.stick(input, 1.0);
        if input.jump.pressed {
            out.events.push(PlayerEvent::Detached(id));
            match self.aim_target(world, if u > 0.0 { dir } else { heading(self.yaw) }) {
                Some(t) if t != id => {
                    self.start_jump(JumpKind::Ground, out);
                    self.aim_at(t, world);
                }
                _ => self.start_jump(JumpKind::Ground, out),
            }
            return;
        }
        if u > 0.5 {
            // Step off.
            out.events.push(PlayerEvent::Detached(id));
            self.yaw = yaw_of(dir);
            self.velocity = scale(dir, self.run_speed(u));
            self.jump_time = None;
            self.air_time = 0.0;
            self.set_state(State::Air { target: None }, out);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn swing(
        &mut self,
        dt: f32,
        input: &PlayerInput,
        world: &PlayerWorld<'_>,
        id: u32,
        len: f32,
        out: &mut PlayerOutput,
    ) {
        let Some(FeatureKind::Hook { position: pivot }) = Self::feature(world, id).map(|f| f.kind.clone()) else {
            self.set_state(State::Air { target: None }, out);
            return;
        };
        if input.jump.pressed {
            out.events.push(PlayerEvent::Detached(id));
            self.start_jump(JumpKind::Swing, out);
            return;
        }
        // A pendulum: gravity plus a push from the stick, kept on the sphere of radius `len` around the hook.
        let (dir, u) = self.stick(input, 1.0);
        let a = add([0.0, 0.0, self.consts.gravity()], scale(dir, u * self.tun.swing_push * len));
        let v = madd(self.velocity, a, dt);
        let p = madd(self.position, v, dt);
        let r = sub(p, pivot);
        let rn = normalize(r);
        self.position = madd(pivot, rn, len);
        self.velocity = madd(v, rn, -dot(v, rn));
        if length(horizontal(self.velocity)) > 1.0 {
            self.yaw = yaw_of(horizontal(self.velocity));
        }
        self.state = State::Swing { id, length: len };
    }

    fn flatten(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, normal: Vec3, out: &mut PlayerOutput) {
        let (dir, u) = self.stick(input, self.consts.sneak_cap);
        if input.action.pressed || (u > 0.0 && dot(dir, normal) > 0.7) {
            self.set_state(State::Ground, out);
            return;
        }
        // Sidestep along the wall: the faster measured speed scaled by the stick, at least the minimum.
        let tangent = normalize(cross([0.0, 0.0, 1.0], normal));
        let s = dot(dir, tangent) * u;
        let speed = if s != 0.0 {
            (s.abs() * self.consts.flatten_speeds[1] / self.consts.sneak_cap).max(self.consts.flatten_min_speed)
        } else {
            0.0
        };
        let v = scale(tangent, speed * s.signum());
        self.velocity = v;
        self.deflection = u;
        let r = self.walk(dt, world, v);
        if r.ground.is_none() {
            self.set_state(State::Air { target: None }, out);
        }
    }

    fn duck(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, out: &mut PlayerOutput) {
        if !input.duck.held {
            self.set_state(State::Ground, out);
            return;
        }
        let (dir, u) = self.stick(input, 1.0);
        if u > 0.0 {
            self.yaw = yaw_of(dir);
        }
        let v = scale(dir, u * self.tun.duck_speed);
        self.velocity = v;
        self.deflection = u;
        let r = self.walk(dt, world, v);
        if r.ground.is_none() {
            self.set_state(State::Air { target: None }, out);
        }
    }

    fn hiding_place(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, id: u32, out: &mut PlayerOutput) {
        if input.action.pressed {
            self.set_state(State::Ground, out);
            out.events.push(PlayerEvent::HidingLeft(id));
            return;
        }
        let (dir, u) = self.stick(input, 1.0);
        let [slow, fast] = self.consts.barrel_speeds;
        let speed = if u > 0.0 { slow + (fast - slow) * u } else { 0.0 };
        if u > 0.0 {
            self.yaw = yaw_of(dir);
        }
        let v = scale(dir, speed);
        self.velocity = v;
        self.deflection = u;
        self.walk(dt, world, v);
    }

    /// The rolling ball (player-moves.md 8): the stick asks for a rolling speed of 1000·u along it (a spin of
    /// 20·u rad/s on the 50-unit ball), reached at a rate of 2.5·u per second; walls bounce it back with
    /// elasticity 0.5. Releasing the power button after 0.25 s ends the ball.
    fn ball(&mut self, dt: f32, input: &PlayerInput, world: &PlayerWorld<'_>, out: &mut PlayerOutput) {
        let c = self.consts;
        if !input.power.held && self.state_time >= c.ball_min_time {
            let s = if self.airborne_now(world) { State::Air { target: None } } else { State::Ground };
            self.set_state(s, out);
            return;
        }
        let (dir, u) = self.stick(input, 1.0);
        let h = horizontal(self.velocity);
        let want = scale(dir, c.ball_roll_speed * u);
        let k = (c.ball_damping * u * dt).min(1.0);
        let h = madd(h, sub(want, h), k);
        self.velocity = [h[0], h[1], self.velocity[2]];
        let hits = if self.airborne_now(world) {
            self.integrate_air(dt, world, [0.0; 3], 0.0, out);
            Vec::new()
        } else {
            self.velocity[2] = 0.0;
            self.walk(dt, world, h).hits
        };
        for hit in hits {
            if hit.normal[2].abs() < c.ground_z {
                let n = normalize(horizontal(hit.normal));
                let into = dot(self.velocity, n);
                if into < 0.0 {
                    self.velocity = madd(self.velocity, n, -(1.0 + c.ball_contact[1]) * into);
                }
            }
        }
    }

    /// The animation for the current state.
    pub fn anim_request(&self) -> AnimRequest {
        let o = &self.oids;
        let speed = length(horizontal(self.velocity));
        let c = &self.consts;
        let run_w = ((speed - c.run_slow) / (c.run_fast - c.run_slow)).clamp(0.0, 1.0);
        match self.state {
            State::Ground => {
                if speed > 1.0 {
                    AnimRequest::blend(o.run[0], o.run[1], run_w)
                } else {
                    AnimRequest::looped(o.idle.unwrap_or(o.run[0]))
                }
            }
            State::Air { .. } => {
                AnimRequest::once(if self.velocity[2] > 0.0 { o.jump } else { o.fall }.unwrap_or(o.run[0]))
            }
            State::LedgeHang { .. } => AnimRequest::looped(o.hang.unwrap_or(o.run[0])),
            State::Pipe { .. } => AnimRequest::blend(o.pipe[0], o.pipe[1], self.deflection),
            State::Rail { .. } => AnimRequest::blend(o.rail[0], o.rail[1], self.deflection),
            State::Perch { .. } => AnimRequest::looped(o.idle.unwrap_or(o.run[0])),
            State::Swing { .. } => AnimRequest::looped(o.swing.unwrap_or(o.run[0])),
            State::Flatten { .. } => {
                if speed > 0.0 {
                    AnimRequest::looped(o.flatten[0])
                } else {
                    AnimRequest::looped(o.flatten[1])
                }
            }
            State::Duck => AnimRequest::looped(o.duck),
            State::Hiding { .. } => {
                let [slow, fast] = c.barrel_speeds;
                AnimRequest::blend(o.barrel[0], o.barrel[1], ((speed - slow) / (fast - slow)).clamp(0.0, 1.0))
            }
            State::Sweep { moving, .. } => AnimRequest::once(if moving { o.sweep[0] } else { o.sweep[1] }),
            State::Rush { .. } => AnimRequest::once(o.rush),
            State::Smash => AnimRequest::once(o.smash),
            State::Dive => AnimRequest::once(o.dive),
            State::Ball => AnimRequest::looped(o.ball),
            State::Hurt { kind, .. } => AnimRequest::once(o.hurt[kind as usize]),
            State::Dead { kind, .. } => AnimRequest::once(match kind {
                DamageKind::Water => o.water_death,
                DamageKind::Crush => o.crush_death,
                DamageKind::Pit => o.pit_death,
                _ if self.backward_death => o.death_backward,
                _ => o.death,
            }),
        }
    }
}

/// The distance along a polyline nearest to `p`, and that point.
pub fn nearest_on_polyline(points: &[Vec3], p: Vec3) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, f32, Vec3)> = None;
    let mut acc = 0.0;
    for w in points.windows(2) {
        let (q, t) = closest_on_segment(p, w[0], w[1]);
        let l = length(sub(w[1], w[0]));
        let dist = length(sub(q, p));
        if best.is_none_or(|(b, ..)| dist < b) {
            best = Some((dist, acc + t * l, q));
        }
        acc += l;
    }
    best.map(|(_, d, q)| (d, q))
}

/// The point at distance `d` along a polyline (clamped).
pub fn polyline_point(points: &[Vec3], d: f32) -> Vec3 {
    let mut acc = 0.0;
    for w in points.windows(2) {
        let l = length(sub(w[1], w[0]));
        if d <= acc + l && l > 0.0 {
            return madd(w[0], sub(w[1], w[0]), (d - acc) / l);
        }
        acc += l;
    }
    points.last().copied().unwrap_or([0.0; 3])
}

/// The unit tangent at distance `d` along a polyline.
pub fn polyline_tangent(points: &[Vec3], d: f32) -> Vec3 {
    let mut acc = 0.0;
    for w in points.windows(2) {
        let l = length(sub(w[1], w[0]));
        if d <= acc + l {
            return normalize(sub(w[1], w[0]));
        }
        acc += l;
    }
    points.windows(2).last().map_or([1.0, 0.0, 0.0], |w| normalize(sub(w[1], w[0])))
}
