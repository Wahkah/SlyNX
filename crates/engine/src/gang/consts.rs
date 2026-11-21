//! Sly 2 character constants (characters.md, gadgets.md, physics.md): the values read from the executable after
//! its SHA-256 check, the spec's [R] values, and the port's tunables.

use formats_sly2::exe::Executable;

use crate::math::Vec3;

/// The executable values of characters.md 10 and gadgets.md 8.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GangConstants {
    /// Damage scaling limits (1.0 without help, 0.5 at full help) [D 0x2BDC20].
    pub help_limits: [f32; 2],
    /// Knock-aside velocity by Murray, left and right in Murray's frame [D 0x2BFAE0, 0x2BFAF0].
    pub murray_knock: [Vec3; 2],
    /// Knock-away by a thrown or fired object: horizontal and up [D 0x2BFB00].
    pub thrown_knock: [f32; 2],
    /// Crush velocity [D 0x2BFB10].
    pub crush: Vec3,
    /// Gravity of Sly's hanging costume parts while hurt [D 0x2BFA00].
    pub costume_gravity: Vec3,
    /// Thunder Flop: press window and the "will not land within" time [D 0x2C08DC].
    pub flop_window: [f32; 2],
    /// Thunder Flop fall speed (−500) [D 0x2C09A8].
    pub flop_fall: f32,
    /// Thunder Flop steering fraction (0.75) [D 0x2C09A0] of the speed 515 [D 0x2BF730].
    pub flop_steer: [f32; 2],
    /// Thunder Flop steering gain (2.5) [D 0x2C09A4].
    pub flop_gain: f32,
    /// A script hold drops one step after this long without a press (1 s) [D 0x2C0900].
    pub hold_drop: f32,
    /// Bentley's crossbow aim point ahead (175, 0, 0) [D 0x2BC920].
    pub crossbow_point: Vec3,
    /// Dart line from the screen centre: near and far depth (10, 40000) [D 0x2BC970, 0x2BC980].
    pub dart_depth: [f32; 2],
    /// Thief Reflexes time scale (0.25) [D 0x2BF3EC] and Time Rush (1.35) [D 0x2BF3F0].
    pub time_scales: [f32; 2],
    /// Power drains per second: Thief Reflexes 2, Time Rush 1, Shadow Power 3, Hover Pack 1 [D].
    pub drains: [f32; 4],
    /// Shadow Power speed factor (0.75) [D 0x2BF5C4].
    pub shadow_speed: f32,
    /// Hover Pack: horizontal speed 300, vertical −100, turn above 200, least 0.5 s, most 3 s [D].
    pub hover: [f32; 5],
    /// Smoke Bomb alarm resume delay (2 s) [D 0x2BCC94].
    pub smoke_resume: f32,
    /// Temporal Lock freeze (6 s) [D 0x2BF938].
    pub temporal_lock: f32,
    /// Default contact friction and elasticity (0.5, 0.5) [D 0x2BFD30].
    pub contact: [f32; 2],
    /// Gadget slot buttons L1, L2, R2 as pad bits (4, 1, 2) [D 0x2C3258].
    pub slot_bits: [i32; 3],
    /// Sly's run gait factor (1.3) [D 0x2BF3D4].
    pub run_factor: f32,
    /// The air-steering speed shared by every jump (515) [D 0x2BF730, 0x2BF734].
    pub air_steer: f32,
    /// Jump kind 1's height of the base point above the last floor (215) [D 0x2BF738] and Sly's double jump's peak
    /// above the first jump's floor (315) [D 0x2BF73C].
    pub jump_heights: [f32; 2],
    /// Feral Pounce's air-steering factor (1.75) [D 0x2BF760].
    pub pounce_steer: f32,
    /// Mega Jump: air-steering factor 1.5, a second factor 1.2 (use not established) and Sly's size 0.75
    /// [D 0x2BF758, 0x2BF75C, 0x2BF750].
    pub mega: [f32; 3],
    /// Paraglide targets with the stick back and forward: forward speed (300, 800), vertical speed (−200, −400),
    /// turning rate (5, 12) [D 0x2BF850, 0x2BF858, 0x2BF870].
    pub glide: [[f32; 2]; 3],
    /// Paraglide easings for forward, sideways and vertical speed ((800, 0, 0.1), (800, 0, 0.1), (5000, 0, 0.1))
    /// [D 0x2BF7A8, 0x2BF7B8, 0x2BF7C8].
    pub glide_ease: [[f32; 3]; 3],
    /// Paraglide acceleration cap (8,000) [D 0x2BF7D4].
    pub glide_accel: f32,
    /// Combat Dodge: the circling distance's easing (500, 0, 0.5) [D 0x2BC9A8] and the end velocity factor (0.5)
    /// [D 0x2BC9B8].
    pub dodge_ease: [f32; 3],
    /// Combat Dodge end velocity factor.
    pub dodge_end: f32,
    /// Silent Obliteration: the homing distance (200) [D 0x2BF414] and its easing (1000, 0, 0.5) [D 0x2BF418].
    pub juggle_home: [f32; 4],
}

impl GangConstants {
    /// The spec's values.
    pub const SPEC: GangConstants = GangConstants {
        help_limits: [1.0, 0.5],
        murray_knock: [[1000.0, 900.0, 950.0], [1000.0, -900.0, 950.0]],
        thrown_knock: [600.0, 800.0],
        crush: [0.0, 0.0, -1000.0],
        costume_gravity: [0.0, 0.0, -750.0],
        flop_window: [0.15, 0.1],
        flop_fall: -500.0,
        flop_steer: [0.75, 515.0],
        flop_gain: 2.5,
        hold_drop: 1.0,
        crossbow_point: [175.0, 0.0, 0.0],
        dart_depth: [10.0, 40000.0],
        time_scales: [0.25, 1.35],
        drains: [2.0, 1.0, 3.0, 1.0],
        shadow_speed: 0.75,
        hover: [300.0, -100.0, 200.0, 0.5, 3.0],
        smoke_resume: 2.0,
        temporal_lock: 6.0,
        contact: [0.5, 0.5],
        slot_bits: [4, 1, 2],
        run_factor: 1.3,
        air_steer: 515.0,
        jump_heights: [215.0, 315.0],
        pounce_steer: 1.75,
        mega: [1.5, 1.2, 0.75],
        glide: [[300.0, 800.0], [-200.0, -400.0], [5.0, 12.0]],
        glide_ease: [[800.0, 0.0, 0.1], [800.0, 0.0, 0.1], [5000.0, 0.0, 0.1]],
        glide_accel: 8000.0,
        dodge_ease: [500.0, 0.0, 0.5],
        dodge_end: 0.5,
        juggle_home: [200.0, 1000.0, 0.0, 0.5],
    };

    /// Reads every [D] value from the player's executable, after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<GangConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let i = |va: u32| -> formats::Result<i32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let v = |va: u32| -> formats::Result<Vec3> { Ok([f(va)?, f(va + 4)?, f(va + 8)?]) };
        Ok(GangConstants {
            help_limits: [f(0x2BDC20)?, f(0x2BDC24)?],
            murray_knock: [v(0x2BFAE0)?, v(0x2BFAF0)?],
            thrown_knock: [f(0x2BFB00)?, f(0x2BFB04)?],
            crush: v(0x2BFB10)?,
            costume_gravity: v(0x2BFA00)?,
            flop_window: [f(0x2C08DC)?, f(0x2C08E0)?],
            flop_fall: f(0x2C09A8)?,
            flop_steer: [f(0x2C09A0)?, f(0x2BF730)?],
            flop_gain: f(0x2C09A4)?,
            hold_drop: f(0x2C0900)?,
            crossbow_point: v(0x2BC920)?,
            dart_depth: [f(0x2BC978)?, f(0x2BC988)?],
            time_scales: [f(0x2BF3EC)?, f(0x2BF3F0)?],
            drains: [f(0x2BD260)?, f(0x2BD264)?, f(0x2BF5C0)?, f(0x2BF878)?],
            shadow_speed: f(0x2BF5C4)?,
            hover: [f(0x2BF87C)?, f(0x2BF880)?, f(0x2BF848)?, f(0x2BF82C)?, f(0x2BF830)?],
            smoke_resume: f(0x2BCC94)?,
            temporal_lock: f(0x2BF938)?,
            contact: [f(0x2BFD30)?, f(0x2BFD34)?],
            slot_bits: [i(0x2C3258)?, i(0x2C325C)?, i(0x2C3260)?],
            run_factor: f(0x002B_F3D4)?,
            air_steer: f(0x002B_F730)?,
            jump_heights: [f(0x002B_F738)?, f(0x002B_F73C)?],
            pounce_steer: f(0x002B_F760)?,
            mega: [f(0x002B_F758)?, f(0x002B_F75C)?, f(0x002B_F750)?],
            glide: [
                [f(0x002B_F850)?, f(0x002B_F854)?],
                [f(0x002B_F858)?, f(0x002B_F85C)?],
                [f(0x002B_F870)?, f(0x002B_F874)?],
            ],
            glide_ease: [
                [f(0x002B_F7A8)?, f(0x002B_F7AC)?, f(0x002B_F7B0)?],
                [f(0x002B_F7B8)?, f(0x002B_F7BC)?, f(0x002B_F7C0)?],
                [f(0x002B_F7C8)?, f(0x002B_F7CC)?, f(0x002B_F7D0)?],
            ],
            glide_accel: f(0x002B_F7D4)?,
            dodge_ease: [f(0x002B_C9A8)?, f(0x002B_C9AC)?, f(0x002B_C9B0)?],
            dodge_end: f(0x002B_C9B8)?,
            juggle_home: [f(0x002B_F414)?, f(0x002B_F418)?, f(0x002B_F41C)?, f(0x002B_F420)?],
        })
    }
}

/// [R] values of the spec, kept apart from the executable reads.
pub mod spec {
    /// Health capacity (40 units).
    pub const HEALTH: f32 = 40.0;
    /// Damage flash (2 s).
    pub const FLASH: f32 = 2.0;
    /// Character gravity: −1750 × 1.3.
    pub const GRAVITY: f32 = -2275.0;
    /// Off the floor this long starts a fall (0.1 s).
    pub const FALL_DELAY: f32 = 0.1;
    /// A launch faster than this upward plays the character's cry.
    pub const CRY_SPEED: f32 = 400.0;
    /// Gadget power maximum (100).
    pub const POWER_MAX: f32 = 100.0;
    /// Starting power: option 1305 = 30 on every character record (unconfirmed meaning).
    pub const POWER_START: f32 = 30.0;
    /// Press windows (seconds): jump 0.1, interact 0.2, binocucom, pickpocket, punch and gadgets 0.15, darts 0.25,
    /// throw release and cancel 0.1, crossbow 0.1.
    pub const WINDOW_JUMP: f32 = 0.1;
    /// Interaction window.
    pub const WINDOW_INTERACT: f32 = 0.2;
    /// Binocucom, pickpocket, punch, gadget window.
    pub const WINDOW_SHORT: f32 = 0.15;
    /// Dart window.
    pub const WINDOW_DART: f32 = 0.25;
    /// Murray's queue window for the next punch of a pair.
    pub const PUNCH_QUEUE: f32 = 0.25;
    /// Murray grabs a launched guard: horizontal reach 300, height 100 to 500; juggling punch 400, 0 to 600.
    pub const GRAB_LAUNCHED: [f32; 3] = [300.0, 100.0, 500.0];
    /// Juggling punch reach.
    pub const JUGGLE: [f32; 3] = [400.0, 0.0, 600.0];
    /// Bentley's crossbow target angle (1.5 rad moving, 3.15 standing) and height blend range (−50 to 50).
    pub const CROSSBOW_ANGLE: [f32; 2] = [1.5, 3.15];
    /// Height blend range of the crossbow clips.
    pub const CROSSBOW_HEIGHT: [f32; 2] = [-50.0, 50.0];
    /// Dart lifetime (4 s) and reload (2 s).
    pub const DART: [f32; 2] = [4.0, 2.0];
    /// Bentley's timed state returns to standing after 2 s.
    pub const BENTLEY_TIMED: f32 = 2.0;
    /// Bentley dropped by Murray: 400 horizontal, 600 up.
    pub const BENTLEY_DROP: [f32; 2] = [400.0, 600.0];
    /// Long Toss: range × 1.5, predicted flight time × 1.35.
    pub const LONG_TOSS: [f32; 2] = [1.5, 1.35];
    /// Throw aim: elevation 0.15 + 0.55 × far; 29 prediction steps; aim turns 2/s, body 3/s.
    pub const THROW_ELEVATION: [f32; 2] = [0.15, 0.55];
    /// Prediction steps.
    pub const THROW_STEPS: usize = 29;
    /// Aim and body turn rates.
    pub const THROW_TURN: [f32; 2] = [2.0, 3.0];
    /// A thrown object used up fades over 0.25 s.
    pub const THROW_FADE: f32 = 0.25;
    /// Stealth Slide ends below this forward speed.
    pub const SLIDE_MIN_SPEED: f32 = 150.0;
    /// Stealth Slide drain per second.
    pub const SLIDE_DRAIN: f32 = 1.0;
    /// Lightning Spin's time: clip 348, 1.0 s (gadgets.md 5.3) [D].
    pub const LIGHTNING_SPIN_TIME: f32 = 1.0;
    /// The juggle strike's time: clip 196, 1.1667 s, or with Silent Obliteration clip 198, 1.3333 s (gadgets.md 5.3)
    /// [D].
    pub const JUGGLE_TIME: [f32; 2] = [7.0 / 6.0, 4.0 / 3.0];
    /// Diablo Fire Slam's time: its own clip (key 507) is missing, so it plays the carried slam, clip 492, 0.8333 s
    /// (gadgets.md 5.3) [D].
    pub const SLAM_TIME: f32 = 5.0 / 6.0;
    /// The dodge's target score d − c·(500 + 0.5·d) (gadgets.md 5.4) [D 0x2BD2B0, 0x2BD2A0].
    pub const DODGE_SCORE: [f32; 2] = [500.0, 0.5];
}

/// Values the specs leave open (characters.md 11), chosen for this port; every one a guess to check against the
/// game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GangTunables {
    /// Capsule radii.
    pub radius: [f32; 3],
    /// Capsule heights (between sphere centres).
    pub height: [f32; 3],
    /// Turn rate on the ground (rad/s).
    pub turn_rate: f32,
    /// Reach for guards (fight stance, pickpocket), objects ahead (Murray's stance, lift) and interactions.
    pub reach: f32,
    /// Throw range (min, max) when the object gives none.
    pub throw_range: [f32; 2],
    /// Predicted flight time of a throw.
    pub throw_time: f32,
    /// Throw aim distance speed at full stick (units/s).
    pub aim_speed: f32,
    /// Thunder Flop wind-up time.
    pub flop_windup: f32,
    /// Hurt reaction time before getting up.
    pub hurt_time: f32,
    /// When no animation driver reports it, an item's activation event comes this long after it starts.
    pub activation_time: f32,
    /// Sly 3: how far ahead of Bentley the Fishing Pole's tip is (the tip part's position is not given).
    pub fishing_range: f32,
    /// Sly 3: the time allowed for the next press of a cane combo: 0.35 s, set by each combo definition (class 148,
    /// the definition's press window); option 1668 would override it, but no record uses it (characters.md 5) [X] [D].
    pub combo_window: f32,
    /// Sly 3: a combo attack's or follow-up's length (the clips' lengths are not given).
    pub combo_clip_time: f32,
    /// Sly 3: Rocket Boots' speed factor on the ordinary movement (the run sets no speed of its own and plays state
    /// 0's animation; the speed is to be measured in play, gadgets.md 6.9).
    pub rocket_factor: f32,
}

impl Default for GangTunables {
    fn default() -> Self {
        GangTunables {
            radius: [30.0, 45.0, 35.0],
            height: [90.0, 80.0, 60.0],
            turn_rate: 12.0,
            reach: 200.0,
            throw_range: [200.0, 1500.0],
            throw_time: 1.5,
            aim_speed: 800.0,
            flop_windup: 0.2,
            hurt_time: 0.8,
            activation_time: 0.3,
            fishing_range: 200.0,
            combo_window: 0.35,
            combo_clip_time: 0.5,
            rocket_factor: 1.0,
        }
    }
}

/// A gait: a slow and a fast clip, each (root travel over one play, length in seconds) (characters.md 1.1) [D].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    /// Slow clip.
    pub slow: [f32; 2],
    /// Fast clip.
    pub fast: [f32; 2],
}

impl Gait {
    /// The speed at weight `u` on the fast clip: `((1 − u)·d_slow + u·d_fast) / ((1 − u)·T_slow + u·T_fast)`.
    pub fn speed(&self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let d = (1.0 - u) * self.slow[0] + u * self.fast[0];
        let t = (1.0 - u) * self.slow[1] + u * self.fast[1];
        if t > 0.0 { d / t } else { 0.0 }
    }
}

/// The move clips' lengths in seconds (specs/sly2/characters.md 9.1, the nightclub hub) [D].
pub mod clip {
    /// Sly's pickpocket (state 11), clip 280.
    pub const SLY_PICKPOCKET: f32 = 1.0;
    /// Sly's air attacks: Square (state 13, clip 190) and Triangle (state 14, clip 191).
    pub const SLY_AIR_ATTACK: [f32; 2] = [2.0 / 3.0, 2.0];
    /// Sly's forced move (state 17), clip 237.
    pub const SLY_FORCED_MOVE: f32 = 4.0 / 3.0;
    /// Sly's charged strike (sub-state 8), clip 297.
    pub const SLY_CHARGED_STRIKE: f32 = 1.0;
    /// Murray's five punches, clips 424, 425, 435, 434, 196.
    pub const MURRAY_PUNCHES: [f32; 5] = [2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 11.0 / 12.0];
    /// Murray's Thunder Flop, clip 417.
    pub const THUNDER_FLOP: f32 = 5.0 / 3.0;
    /// Murray's throw, clip 490.
    pub const MURRAY_THROW: f32 = 0.75;
    /// Murray picking up, clips 452 and 453.
    pub const MURRAY_PICK_UP: [f32; 2] = [1.0, 2.0 / 3.0];
    /// Berserker Charge: wind-up 440, charge 441, bounce off a wall 442, stop 443.
    pub const BERSERKER: [f32; 4] = [10.0 / 3.0, 0.5, 2.0, 1.0];
    /// Turnbuckle Launch (state 21, looping), clip 509.
    pub const TURNBUCKLE: f32 = 2.0 / 3.0;
    /// Bentley's crossbow swing: each of the six two-clip blends, clips 560 to 571.
    pub const BENTLEY_SWING: f32 = 0.5;
    /// Bentley's Size Destabilizer (item 8), clip 341.
    pub const SIZE_DESTABILIZER: f32 = 1.0 / 3.0;
    /// Bentley's state 14 (Adrenaline Burst), clip 588.
    pub const ADRENALINE: f32 = 1.0 / 6.0;
    /// Bentley picking up, clip 447.
    pub const BENTLEY_PICK_UP: f32 = 0.75;
    /// Bentley held by Murray, clip 522.
    pub const BENTLEY_HELD: f32 = 0.5;
}

/// The gaits measured on the nightclub hub (characters.md 1.1) [D]: (travel, length) of each clip.
pub mod gait {
    use super::Gait;

    /// Sly's run (state 1), clips 125 and 126, played at weight 1 and × 1.3: 832.
    pub const SLY_RUN: Gait = Gait { slow: [160.0, 1.0], fast: [320.0, 0.5] };
    /// Sly's move (state 12), clips 276 and 277: 225 to 510 (320 at half stick).
    pub const SLY_MOVE: Gait = Gait { slow: [300.0, 4.0 / 3.0], fast: [340.0, 2.0 / 3.0] };
    /// Murray's run (state 1), clips 416 and 414, weight 1: 800.
    pub const MURRAY_RUN: Gait = Gait { slow: [170.0, 1.0 / 3.0], fast: [400.0, 0.5] };
    /// Murray's walk (state 10), clips 489 and 488: 225 to 510.
    pub const MURRAY_WALK: Gait = Gait { slow: [300.0, 4.0 / 3.0], fast: [255.0, 0.5] };
    /// Murray carrying (state 5), clips 482 and 483: 125 to 516.7.
    pub const MURRAY_CARRY: Gait = Gait { slow: [125.0, 1.0], fast: [310.0, 0.6] };
    /// Bentley's move (state 1), clips 416 and 414, weight 1: 750.
    pub const BENTLEY_RUN: Gait = Gait { slow: [100.0, 1.0], fast: [250.0, 1.0 / 3.0] };
    /// Bentley's walk (state 7), clips 489 and 488: 165 to 510.
    pub const BENTLEY_WALK: Gait = Gait { slow: [220.0, 4.0 / 3.0], fast: [170.0, 1.0 / 3.0] };
    /// Bentley's timed state (14), clip 588: 1,500.
    pub const BENTLEY_TIMED: Gait = Gait { slow: [250.0, 1.0 / 6.0], fast: [250.0, 1.0 / 6.0] };
    /// Sly's combat dodge, clips 281 and 282: 500 in 1/3 s each.
    pub const SLY_DODGE: Gait = Gait { slow: [500.0, 1.0 / 3.0], fast: [500.0, 1.0 / 3.0] };
    /// Knockout Dive, clip 189: 660 in 0.917 s (gadgets.md 5.1).
    pub const KNOCKOUT_DIVE: Gait = Gait { slow: [660.0, 11.0 / 12.0], fast: [660.0, 11.0 / 12.0] };
    /// Berserker Charge: the charge, clip 441 (350 in 0.5 s), and the stop, clip 443 (250 over 1.0 s).
    pub const CHARGE: [Gait; 2] =
        [Gait { slow: [350.0, 0.5], fast: [350.0, 0.5] }, Gait { slow: [250.0, 1.0], fast: [250.0, 1.0] }];
    /// The charge lasts this long before the stop (a code value).
    pub const CHARGE_TIME: f32 = 0.66;
}

/// The vertical launch speed that brings the base point to `height` above a floor it is `h` above, under the
/// characters' gravity (characters.md 1.2): `√(2·2275·(height − h))`, 0 once it is there.
pub fn jump_speed(height: f32, h: f32) -> f32 {
    crate::math::sqrt((2.0 * -spec::GRAVITY * (height - h)).max(0.0))
}
