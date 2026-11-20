//! The player's constants: the values player-moves.md gives (read from the executable where it gives an address),
//! and the tunables the specs leave open, all in one place.

use formats::disc::Executable;

use crate::math::sqrt;
use crate::physics::TerminalFall;

/// `clamp(c0 + c1·x + c2·x², lo, hi)`: the jump-speed rules of player-moves.md 3 (coefficients and limits stored
/// in the executable).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quadratic {
    /// Coefficients c0, c1, c2.
    pub c: [f32; 3],
    /// Lower limit.
    pub lo: f32,
    /// Upper limit.
    pub hi: f32,
}

impl Quadratic {
    /// The value at `x`.
    pub fn eval(&self, x: f32) -> f32 {
        (self.c[0] + self.c[1] * x + self.c[2] * x * x).max(self.lo).min(self.hi)
    }
}

/// The values of player-moves.md. Fields marked [X] are read from the executable by [`Self::from_executable`];
/// the others ([R], [D]) are the spec's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerConstants {
    /// Base gravity before the ratio (−1750) [R].
    pub gravity_base: f32,
    /// Gravity ratio (1.3) [R].
    pub gravity_ratio: f32,
    /// Terminal fall rule (−750, 5) [X 0x263E78, 0x263E7C].
    pub terminal: TerminalFall,
    /// Air-steer speed before √ratio (455) [X 0x262E14].
    pub air_steer_speed: f32,
    /// Air-steer factor after a jump out of water (0.7) [X 0x262E18].
    pub air_steer_after_water: f32,
    /// Air-steer factor after an edge jump (0.6) [X 0x262E1C].
    pub air_steer_after_edge: f32,
    /// Hook catch radius (15) [R].
    pub hook_catch_radius: f32,
    /// Charm flash (invulnerability) after a hit (2 s) [R].
    pub charm_flash: f32,
    /// Ground jump height (245) [X 0x263E84].
    pub jump_height: f32,
    /// Swing jump speed before √ratio (700) [X 0x263E80].
    pub swing_jump: f32,
    /// Jump out of water before √ratio (900) [X 0x263E88].
    pub water_jump: f32,
    /// Ledge or edge jump before √ratio (800) [X 0x263E8C].
    pub edge_jump: f32,
    /// Jump toward a perch point or target: of the current upward speed (682 + 0.5 v, at most 682) [X 0x264050].
    pub target_jump: Quadratic,
    /// Boosted jump (682 + v, at most 682) [X 0x264070].
    pub boosted_jump: Quadratic,
    /// Take-off phase (0.15 s) [X 0x263E50].
    pub takeoff_time: f32,
    /// Above this horizontal speed the facing follows the direction of motion in the air (300) [X 0x263E54].
    pub face_motion_speed: f32,
    /// A jump is short-steered unless the stick reaches full deflection within this time (0.07 s) [X 0x263E70].
    pub steer_full_window: f32,
    /// ... and stays there at least this long (0.30 s) [X 0x263E74].
    pub steer_full_hold: f32,
    /// Air-steer rates: forward when speeding up, forward when slowing, sideways, for the rest of the jump and then
    /// for the take-off (2.5, 3.5, 2.5, 60, 20, 10) [X 0x263E58 to 0x263E6C].
    pub air_steer_rates: [f32; 6],
    /// Least time in the air before a smash (0.35 s) [X 0x264040].
    pub smash_min_air: f32,
    /// Least time in the air before a dive (0.35 s) [X 0x264044].
    pub dive_min_air: f32,
    /// How far a target jump's path is searched (10000) [X 0x264048].
    pub target_search: f32,
    /// Ledge grab box: horizontal reach (60) [R].
    pub ledge_grab_horizontal: f32,
    /// Ledge grab box: vertical reach (70) [R].
    pub ledge_grab_vertical: f32,
    /// Stick cap while sneaking (0.5) [X 0x274E58].
    pub sneak_cap: f32,
    /// Smoothing into and out of a flattened pose (0.3 s) [X 0x274E48].
    pub flatten_smooth: f32,
    /// Least sidestep speed while flattened (25) [X 0x274E4C].
    pub flatten_min_speed: f32,
    /// Hands stop this far from a pipe's top (35) [X 0x274F00].
    pub pipe_hands_end: f32,
    /// Feet stop this far from a pipe's bottom (100) [X 0x274F04].
    pub pipe_feet_end: f32,
    /// Stick change needed to reverse on a pipe (0.5) [X 0x274F08].
    pub pipe_reverse: f32,
    /// Pipes are probed this far around (50) [X 0x274F14].
    pub pipe_probe: f32,
    /// Stick direction against the rail needed to reverse (−0.2) [X 0x263990].
    pub rail_reverse: f32,
    /// Contact window after a smash (0.25 s) [X 0x26399C].
    pub smash_window: f32,
    /// Dive landing: objects affected within (300) [R].
    pub dive_affect_radius: f32,
    /// Dive landing: coins knocked loose within (500) [R].
    pub dive_coin_radius: f32,
    /// A death while moving backward faster than 250 turns the player away from the motion by
    /// `clamp(−1 + 0.004·speed, 0, 1)` of the angle (−1, 0.004, 0) [X 0x2639A0].
    pub death_turn: [f32; 3],
    /// Rolling ball radius (50) [X 0x262E30].
    pub ball_radius: f32,
    /// Rolling ball launch speed (850) [X 0x262E2C].
    pub ball_launch: f32,
    /// Below this speed the ball is launched along the facing; above, the motion keeps its direction (150)
    /// [X 0x2639B0].
    pub ball_launch_min: f32,
    /// The stick's wish for the ball's rolling speed at full deflection (1000; spin `1000·u / 50` rad/s) [R].
    pub ball_roll_speed: f32,
    /// Rate (per second, × deflection) at which the rolling speed approaches the wish (2.5) [R].
    pub ball_damping: f32,
    /// Ball contact friction and elasticity (0.8, 0.5) [R].
    pub ball_contact: [f32; 2],
    /// Releasing the power button ends the ball after this long (0.25 s) [R].
    pub ball_min_time: f32,
    /// Dive: hover (gravity off, velocity pulled to zero) until frame 20 (0.333 s) [D].
    pub dive_hover: f32,
    /// Dive: the plunge speed from the root motion of segment 440 (2075 units in 80 frames: 1556.3) [D].
    pub dive_plunge: f32,
    /// Dive: gravity resumes at frame 100 (1.667 s) [D].
    pub dive_end: f32,
    /// Dive: rate at which the velocity is pulled toward zero during the hover (10 per second) [R].
    pub dive_damping: f32,
    /// Slow run speed, segment 130 (38.3) [D].
    pub run_slow: f32,
    /// Fast run speed, segment 131 (638.4) [D].
    pub run_fast: f32,
    /// The slow segment's left-foot plant-to-lift interval (131 frames) [D].
    pub run_slow_interval: f32,
    /// The fast segment's left-foot plant-to-lift interval (7 frames) [D].
    pub run_fast_interval: f32,
    /// The walking leg: base point this far below the collision shape (option 709 default, 35) [D] [R].
    pub leg_length: f32,
    /// Friction of the leg's ground contact (1.5) [X 0x274B74].
    pub leg_friction: f32,
    /// Normal z from which a contact is ground (0.7) [R].
    pub ground_z: f32,
    /// Turn factor coefficients: speed × clamp(1 − |Δθ|, 0, 1) (1, −1, 0) [X 0x274B30].
    pub turn_slow: [f32; 3],
    /// Ledge hang position: out from the edge and below it (37, 0, 25) [X 0x2636F0].
    pub hang_offset: [f32; 3],
    /// Ledge hang easing (1000, 0, 0.25) [R].
    pub hang_smooth: [f32; 3],
    /// The origin's height above the ground contact point while standing: the standing shape reaches 55 below the
    /// origin and the leg 35 below that (player-moves.md 1.1) [D].
    pub origin_height: f32,
    /// Turn smoothing standing (40, 0, 0.25, 200) [X 0x263740] and running (15, 0, 0.25, 100) [X 0x263750].
    pub turn_smooth: [[f32; 4]; 2],
    /// Running weight `clamp(−0.2 + 0.002·speed, 0, 1)` [X 0x263760].
    pub turn_run_weight: [f32; 3],
    /// Stick factor on the turn's fast and slow speeds, `clamp(0.2 + 0·u + 0.8·u², 0, 1)` [X 0x263770].
    pub turn_stick: [f32; 3],
    /// Pipe climb speeds of segments 155 and 156 (66.0 and 365.3), from the right foot's motion [D].
    pub pipe_speeds: [f32; 2],
    /// Velocity set by a crush hit, world axes (0, 0, −1000) [X 0x275040].
    pub crush_velocity: [f32; 3],
    /// Rush speed (550 units/s for 1 s, segment 261) [D].
    pub rush_speed: f32,
    /// Rush duration (60 frames) [D].
    pub rush_time: f32,
    /// Flattened sidestep speeds (120 and 135) [D].
    pub flatten_speeds: [f32; 2],
    /// Barrel walk speeds (72.6 and 416.7) [D].
    pub barrel_speeds: [f32; 2],
    /// Slow-motion power-up time scale (0.25) [R].
    pub slow_motion_scale: f32,
}

impl PlayerConstants {
    /// The spec's values.
    pub const SPEC: PlayerConstants = PlayerConstants {
        gravity_base: -1750.0,
        gravity_ratio: 1.3,
        terminal: TerminalFall { speed: -750.0, rate: 5.0 },
        air_steer_speed: 455.0,
        air_steer_after_water: 0.7,
        air_steer_after_edge: 0.6,
        hook_catch_radius: 15.0,
        charm_flash: 2.0,
        jump_height: 245.0,
        swing_jump: 700.0,
        water_jump: 900.0,
        edge_jump: 800.0,
        target_jump: Quadratic { c: [682.0, 0.5, 0.0], lo: -f32::MAX, hi: 682.0 },
        boosted_jump: Quadratic { c: [682.0, 1.0, 0.0], lo: -f32::MAX, hi: 682.0 },
        takeoff_time: 0.15,
        face_motion_speed: 300.0,
        steer_full_window: 0.07,
        steer_full_hold: 0.30,
        air_steer_rates: [2.5, 3.5, 2.5, 60.0, 20.0, 10.0],
        smash_min_air: 0.35,
        dive_min_air: 0.35,
        target_search: 10_000.0,
        ledge_grab_horizontal: 60.0,
        ledge_grab_vertical: 70.0,
        sneak_cap: 0.5,
        flatten_smooth: 0.3,
        flatten_min_speed: 25.0,
        pipe_hands_end: 35.0,
        pipe_feet_end: 100.0,
        pipe_reverse: 0.5,
        pipe_probe: 50.0,
        rail_reverse: -0.2,
        smash_window: 0.25,
        dive_affect_radius: 300.0,
        dive_coin_radius: 500.0,
        death_turn: [-1.0, 0.004, 0.0],
        ball_radius: 50.0,
        ball_launch: 850.0,
        ball_launch_min: 150.0,
        ball_roll_speed: 1000.0,
        ball_damping: 2.5,
        ball_contact: [0.8, 0.5],
        ball_min_time: 0.25,
        dive_hover: 20.0 / 60.0,
        dive_plunge: 2075.0 / (80.0 / 60.0),
        dive_end: 100.0 / 60.0,
        dive_damping: 10.0,
        run_slow: 38.3,
        run_fast: 638.4,
        run_slow_interval: 131.0 / 60.0,
        run_fast_interval: 7.0 / 60.0,
        leg_length: 35.0,
        leg_friction: 1.5,
        ground_z: 0.7,
        turn_slow: [1.0, -1.0, 0.0],
        hang_offset: [37.0, 0.0, 25.0],
        hang_smooth: [1000.0, 0.0, 0.25],
        origin_height: 90.0,
        turn_smooth: [[40.0, 0.0, 0.25, 200.0], [15.0, 0.0, 0.25, 100.0]],
        turn_run_weight: [-0.2, 0.002, 0.0],
        turn_stick: [0.2, 0.0, 0.8],
        pipe_speeds: [66.0, 365.3],
        crush_velocity: [0.0, 0.0, -1000.0],
        rush_speed: 550.0,
        rush_time: 1.0,
        flatten_speeds: [120.0, 135.0],
        barrel_speeds: [72.6, 416.7],
        slow_motion_scale: 0.25,
    };

    /// The spec's values with every [X] constant read from the player's executable, after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<PlayerConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let q = |va: u32| -> formats::Result<Quadratic> {
            Ok(Quadratic { c: [f(va)?, f(va + 4)?, f(va + 8)?], lo: f(va + 16)?, hi: f(va + 20)? })
        };
        Ok(PlayerConstants {
            terminal: TerminalFall { speed: f(0x263E78)?, rate: f(0x263E7C)? },
            air_steer_speed: f(0x262E14)?,
            air_steer_after_water: f(0x262E18)?,
            air_steer_after_edge: f(0x262E1C)?,
            jump_height: f(0x263E84)?,
            swing_jump: f(0x263E80)?,
            water_jump: f(0x263E88)?,
            edge_jump: f(0x263E8C)?,
            target_jump: q(0x264050)?,
            boosted_jump: q(0x264070)?,
            takeoff_time: f(0x263E50)?,
            face_motion_speed: f(0x263E54)?,
            steer_full_window: f(0x263E70)?,
            steer_full_hold: f(0x263E74)?,
            air_steer_rates: [f(0x263E58)?, f(0x263E5C)?, f(0x263E60)?, f(0x263E64)?, f(0x263E68)?, f(0x263E6C)?],
            smash_min_air: f(0x264040)?,
            dive_min_air: f(0x264044)?,
            target_search: f(0x264048)?,
            sneak_cap: f(0x274E58)?,
            flatten_smooth: f(0x274E48)?,
            flatten_min_speed: f(0x274E4C)?,
            pipe_hands_end: f(0x274F00)?,
            pipe_feet_end: f(0x274F04)?,
            pipe_reverse: f(0x274F08)?,
            pipe_probe: f(0x274F14)?,
            rail_reverse: f(0x263990)?,
            smash_window: f(0x26399C)?,
            death_turn: [f(0x2639A0)?, f(0x2639A4)?, f(0x2639A8)?],
            ball_radius: f(0x262E30)?,
            ball_launch: f(0x262E2C)?,
            ball_launch_min: f(0x2639B0)?,
            leg_friction: f(0x274B74)?,
            turn_slow: [f(0x274B30)?, f(0x274B34)?, f(0x274B38)?],
            hang_offset: [f(0x2636F0)?, f(0x2636F4)?, f(0x2636F8)?],
            turn_smooth: [
                [f(0x263740)?, f(0x263744)?, f(0x263748)?, f(0x26374C)?],
                [f(0x263750)?, f(0x263754)?, f(0x263758)?, f(0x26375C)?],
            ],
            turn_run_weight: [f(0x263760)?, f(0x263764)?, f(0x263768)?],
            turn_stick: [f(0x263770)?, f(0x263774)?, f(0x263778)?],
            crush_velocity: [f(0x275040)?, f(0x275044)?, f(0x275048)?],
            ..PlayerConstants::SPEC
        })
    }

    /// Gravity along z: −1750 × 1.3 = −2275.
    pub fn gravity(&self) -> f32 {
        self.gravity_base * self.gravity_ratio
    }

    /// `√ratio`, the factor several speeds carry.
    pub fn root_ratio(&self) -> f32 {
        sqrt(self.gravity_ratio)
    }

    /// The ground jump's take-off speed for an origin `h` above the ground contact: enough for the origin to rise
    /// to 245 above the contact, `√(2 · 2275 · (245 − h))`: 839.8 standing (h = 90), 1055.8 for h = 0.
    pub fn ground_jump_speed_from(&self, h: f32) -> f32 {
        sqrt(2.0 * (self.jump_height - h).max(0.0) * -self.gravity())
    }

    /// The standing ground jump's speed (h = 90): 839.8.
    pub fn ground_jump_speed(&self) -> f32 {
        self.ground_jump_speed_from(self.origin_height)
    }

    /// The turn smoothing for a running speed and stick deflection `u` (player-moves.md 2): the standing values
    /// interpolated toward the running ones by `clamp(−0.2 + 0.002·speed, 0, 1)`, the fast and slow speeds × the
    /// stick factor. `running` false gives the standing values (every other state).
    pub fn turn_params(&self, running: bool, speed: f32, u: f32) -> crate::physics::SmoothParams {
        let [st, rn] = self.turn_smooth;
        let q = |c: [f32; 3], x: f32| (c[0] + c[1] * x + c[2] * x * x).clamp(0.0, 1.0);
        let w = if running { q(self.turn_run_weight, speed) } else { 0.0 };
        let k = if running { q(self.turn_stick, u) } else { 1.0 };
        let mix = |i: usize| st[i] + (rn[i] - st[i]) * w;
        crate::physics::SmoothParams {
            fast: mix(0) * k,
            slow: mix(1) * k,
            fast_time: mix(2),
            max_speed_change: Some(mix(3)),
        }
    }

    /// The speed at which the fall drag starts: −750 · √1.3 = −855.1.
    pub fn terminal_speed(&self) -> f32 {
        self.terminal.speed_for(self.gravity_ratio)
    }

    /// The speed the fall settles at: −855.1 + (−2275 / 5) = −1310.1.
    pub fn settling_speed(&self) -> f32 {
        self.terminal.settling_speed(self.gravity_ratio, self.gravity())
    }

    /// The ground contact's acceleration limit: friction 1.5 × |g| = 3412.5 units/s² (physics.md 5).
    pub fn ground_accel(&self) -> f32 {
        self.leg_friction * -self.gravity()
    }

    /// The turn factor for an angle `dtheta` (radians) between the facing and the wanted heading.
    pub fn turn_factor(&self, dtheta: f32) -> f32 {
        let [a, b, c] = self.turn_slow;
        let x = dtheta.abs();
        (a + b * x + c * x * x).clamp(0.0, 1.0)
    }
}

/// Values the specs leave open (player-moves.md 10), chosen for this port. Every one of them is a guess to be
/// checked against the running game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tunables {
    /// Collision capsule radius.
    pub radius: f32,
    /// Distance between the capsule's sphere centres.
    pub height: f32,
    /// How far below the leg's reach the ground still counts as touched (the leg's 2-unit allowance; the port's
    /// tolerance for walking down slopes, not a snap).
    pub ground_probe: f32,
    /// Hand height above the feet (ledge, pipe and hook reach).
    pub hand_height: f32,
    /// Rail and rope walking speed at full deflection.
    pub rail_speed: f32,
    /// Catch distance for rails, ropes and perch points (the in-air context button).
    pub catch_radius: f32,
    /// Extra reach for hooks, added to the hook catch radius.
    pub hook_reach: f32,
    /// Range at which a jump is aimed at a perch point in the stick's direction.
    pub target_range: f32,
    /// Distance at which a wall can be flattened against.
    pub flatten_reach: f32,
    /// Duck-walk speed at full deflection.
    pub duck_speed: f32,
    /// Distance at which a hiding place can be entered.
    pub hide_reach: f32,
    /// Time a death plays before respawn is requested.
    pub death_time: f32,
    /// Time a hurt reaction lasts.
    pub hurt_time: f32,
    /// Duration of a slow-motion or speed-up use.
    pub power_time: f32,
    /// Duration of a standing sweep (40 frames).
    pub sweep_time: f32,
    /// Rate at which the swing angle is driven by the stick (rad/s²).
    pub swing_push: f32,
}

impl Default for Tunables {
    fn default() -> Self {
        Tunables {
            radius: 30.0,
            height: 100.0,
            ground_probe: 2.0,
            hand_height: 160.0,
            rail_speed: 300.0,
            catch_radius: 80.0,
            hook_reach: 120.0,
            target_range: 800.0,
            flatten_reach: 50.0,
            duck_speed: 150.0,
            hide_reach: 120.0,
            death_time: 2.5,
            hurt_time: 0.6,
            power_time: 10.0,
            sweep_time: 40.0 / 60.0,
            swing_push: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_speeds() {
        let c = PlayerConstants::SPEC;
        assert!((c.gravity() + 2275.0).abs() < 1e-3);
        assert!((c.ground_jump_speed() - 839.8).abs() < 0.05);
        assert!((c.ground_jump_speed_from(0.0) - 1055.8).abs() < 0.05);
        let p = c.turn_params(true, 600.0, 1.0);
        assert_eq!((p.fast, p.max_speed_change), (15.0, Some(100.0)));
        assert_eq!(c.turn_params(false, 600.0, 0.0).fast, 40.0);
        assert!((c.swing_jump * c.root_ratio() - 798.1).abs() < 0.05);
        assert!((c.water_jump * c.root_ratio() - 1026.2).abs() < 0.05);
        assert!((c.edge_jump * c.root_ratio() - 912.2).abs() < 0.1);
        assert!((c.air_steer_speed * c.root_ratio() - 518.8).abs() < 0.05);
        assert!((c.terminal_speed() + 855.1).abs() < 0.05);
        assert!((c.settling_speed() + 1310.1).abs() < 0.05);
        assert!((c.ground_accel() - 3412.5).abs() < 1e-3);
        assert!((c.dive_plunge - 1556.25).abs() < 0.01);
        assert_eq!(c.turn_factor(0.25), 0.75);
        assert_eq!(c.turn_factor(-2.0), 0.0);
        assert_eq!(c.target_jump.eval(0.0), 682.0);
        assert_eq!(c.target_jump.eval(-200.0), 582.0);
        assert_eq!(c.boosted_jump.eval(100.0), 682.0);
    }
}
