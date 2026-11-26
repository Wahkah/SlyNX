//! The three characters' controllers (characters.md 1 to 5, gadgets.md 2 to 4): shared movement and rules, then
//! Sly's, Murray's and Bentley's state machines with the spec's state numbers.

use alloc::vec::Vec;

use super::Who;
use super::consts::{GangConstants, GangTunables, clip, gait, jump_speed, spec};
use super::gadgets::{Effect, PARAGLIDE_LANDING, POOL_MIN_FREE, PayAt, Refusal, WorldEffect, effect};
use super::health::{Health, Hit, Reaction};
use super::input::{GangInput, GangWorld, PressBuffer, Target, TargetKind, button};
use super::items::{ButtonUse, ItemTable, Owned, Power, Slots, behaviour, id};
use super::throw::{ThrowAim, turn_toward};
use crate::math::{Vec3, add, atan2, cos, cross, dot, length, madd, normalize, scale, sin, sub};
use crate::physics::character::{CharacterShape, SlideParams, ground_below, move_and_slide};

/// Sly's state numbers (characters.md 3).
pub mod sly {
    /// Standing.
    pub const STAND: u8 = 0;
    /// Running (R1).
    pub const RUN: u8 = 1;
    /// In the air.
    pub const AIR: u8 = 2;
    /// Swinging or hanging on a hook.
    pub const HOOK: u8 = 3;
    /// Damage reaction.
    pub const DAMAGE: u8 = 4;
    /// Fight stance near a guard.
    pub const FIGHT: u8 = 5;
    /// Combat dodge.
    pub const DODGE: u8 = 6;
    /// Interaction.
    pub const INTERACT: u8 = 7;
    /// Climbing a pipe.
    pub const CLIMB: u8 = 8;
    /// Sneaking near a guard.
    pub const SNEAK: u8 = 10;
    /// Pickpocketing.
    pub const PICKPOCKET: u8 = 11;
    /// Knockout Dive.
    pub const KNOCKOUT_DIVE: u8 = 9;
    /// Running (sneaking off).
    pub const SNEAK_RUN: u8 = 12;
    /// The juggle strike (finishing an enemy knocked into the air).
    pub const JUGGLE_STRIKE: u8 = 16;
    /// Air attacks (Square, Triangle).
    pub const AIR_SQUARE: u8 = 13;
    /// Air attack (Triangle).
    pub const AIR_TRIANGLE: u8 = 14;
    /// Throw aim.
    pub const THROW_AIM: u8 = 23;
    /// Stealth Slide.
    pub const SLIDE: u8 = 24;
    /// Sly 3: the combo attack (Sly 3's state 23, generic 9; `combo::SLY3_STATE`).
    pub const COMBO: u8 = 25;
}

/// Murray's state numbers (characters.md 4.2).
pub mod murray {
    /// Standing.
    pub const STAND: u8 = 0;
    /// Running.
    pub const RUN: u8 = 1;
    /// In the air.
    pub const AIR: u8 = 2;
    /// Thunder Flop.
    pub const FLOP: u8 = 3;
    /// Lifting.
    pub const LIFT: u8 = 4;
    /// Carrying.
    pub const CARRY: u8 = 5;
    /// Held by a script.
    pub const HELD: u8 = 6;
    /// Diablo Fire Slam.
    pub const SLAM: u8 = 8;
    /// Stance near an object or guard ahead.
    pub const STANCE: u8 = 9;
    /// Walking.
    pub const WALK: u8 = 10;
    /// Punch combo.
    pub const PUNCH: u8 = 11;
    /// Berserker Charge.
    pub const CHARGE: u8 = 12;
    /// Interaction.
    pub const INTERACT: u8 = 13;
    /// Damage reaction.
    pub const DAMAGE: u8 = 14;
    /// Throw aim.
    pub const THROW_AIM: u8 = 15;
    /// Turnbuckle Launch stance.
    pub const TURNBUCKLE: u8 = 21;
}

/// Bentley's state numbers (characters.md 5.2).
pub mod bentley {
    /// Standing.
    pub const STAND: u8 = 0;
    /// Moving.
    pub const MOVE: u8 = 1;
    /// In the air.
    pub const AIR: u8 = 2;
    /// Interaction.
    pub const INTERACT: u8 = 3;
    /// Damage reaction.
    pub const DAMAGE: u8 = 4;
    /// Near an object ahead.
    pub const NEAR: u8 = 5;
    /// Holding a summoned gadget.
    pub const HOLDING: u8 = 6;
    /// Walking.
    pub const WALK: u8 = 7;
    /// Carried by Murray.
    pub const CARRIED: u8 = 8;
    /// Throw aim.
    pub const THROW_AIM: u8 = 12;
    /// The timed state that returns to standing after 2 s.
    pub const TIMED: u8 = 14;
}

/// What a character did, for the object model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GangEvent {
    /// The state changed.
    State {
        /// From.
        from: u8,
        /// To.
        to: u8,
    },
    /// Jumped.
    Jumped,
    /// Landed.
    Landed,
    /// Opened the binocucom.
    Binocucom,
    /// Used an interaction.
    Interact(u32),
    /// Started a pickpocket on a guard.
    Pickpocket(u32),
    /// An air attack (Sly 13 or 14).
    AirAttack(u8),
    /// A punch (0 to 4) at a target.
    Punch {
        /// Punch clip 0 to 4.
        index: u8,
        /// Target, if any.
        target: Option<u32>,
    },
    /// A Thunder Flop started.
    FlopStarted,
    /// A Thunder Flop landed here (with the ring of fire when Raging Inferno Flop is owned).
    FlopLanded {
        /// Position.
        position: Vec3,
        /// Raging Inferno Flop.
        inferno: bool,
    },
    /// Lifted an object or guard.
    Lift(u32),
    /// Picked up a launched guard.
    GrabbedLaunched(u32),
    /// Threw what was carried, or a gadget.
    Thrown {
        /// What (an id, or the gadget item number for Bentley's gadgets).
        id: u32,
        /// Launch velocity.
        velocity: Vec3,
    },
    /// Cancelled a throw.
    ThrowCancelled,
    /// Bentley's crossbow swing (mode 0 standing, 1 moving, 2 air) with the high clip's weight.
    Crossbow {
        /// Target.
        target: Option<u32>,
        /// Weight of the high swing clip.
        high: f32,
        /// Mode.
        mode: u8,
    },
    /// A dart fired along the screen centre.
    Dart {
        /// From.
        from: Vec3,
        /// Where it stops (the first solid hit, or the far end).
        to: Vec3,
        /// It hit a solid.
        hit: bool,
    },
    /// A gadget was summoned and held.
    Summoned(u8),
    /// A thrown gadget was triggered.
    Triggered(u8),
    /// An item started (the mission system receives event 60 with the character).
    ItemStarted(u8),
    /// An item reached its activation event (or started, for items with none) and paid its cost.
    ItemActivated(u8),
    /// A held item ended.
    ItemEnded(u8),
    /// An item was refused (sound 46; for power, the meter flashes too).
    ItemRefused {
        /// The item.
        item: u8,
        /// Why.
        reason: Refusal,
    },
    /// An activated item acts on the world (smoke, freeze, fright, shrink).
    World(WorldEffect),
    /// A cane or fist overlay animation was applied or removed (items 15, 34, 35).
    Overlay {
        /// The item.
        item: u8,
        /// Applied.
        on: bool,
    },
    /// A special jump started, by its jump kind (14 Feral Pounce, 9 Mega Jump, 15 Atlas Strength, 17 Hover Pack,
    /// 19 Paraglide).
    SpecialJump(u8),
    /// A move item's state started (10, 19, 20, 24, 33, 39).
    Move(u8),
    /// Murray's throw with Juggernaut Throw owned: the thrown object explodes on impact.
    Explosive(u32),
    /// Murray dropped what he carried (hit or falling): its velocity, (−400, 0, 600) in his frame.
    Dropped {
        /// What.
        id: u32,
        /// Velocity.
        velocity: Vec3,
    },
    /// Sly's double jump (jump kind 6).
    DoubleJump,
    /// Sly 3: invisible at this Shadow Power level (0: visible again; 2 also attacks while invisible).
    Invisible(u8),
    /// Sly 3: a disguise put on (0 Venice, 1 China, 2 pirates) or taken off.
    Disguise(Option<u8>),
    /// Sly 3: a cane combo finisher: kind 0 spin (Square), 1 jump (Cross), 2 push (Circle), at level 1 to 3.
    ComboFinisher {
        /// Kind.
        kind: u8,
        /// Level.
        level: u8,
    },
    /// Sly 3: a combo definition's clip to play (`combo::Cue::Clip`): the definition's object ID and the clip.
    ComboClip {
        /// The definition's object ID.
        def: i16,
        /// The clip's object ID.
        clip: i32,
    },
    /// Sly 3: the combo ended (back from state 23).
    ComboEnded,
    /// A pop-up hit's second, held reaction begins (sound 0x6A; characters.md 2.2).
    HeldReaction,
    /// Sly 3: the Grapple-Cam was put away (`grabbed`: after the grab, phase 7).
    GrappleCamEnded {
        /// It ended through the grab.
        grabbed: bool,
    },
    /// Sly 3: Bentley's chair melee (Insanity Strike 34, Size Destabilizer 8): the clip played and the hit kind its
    /// hits carry (`sly3::hit`).
    ChairMelee {
        /// The item (behaviour).
        item: u8,
        /// The clip's object ID.
        clip: i32,
        /// The hit kind.
        hit: u32,
    },
    /// Sly 3: the treasure map shown or hidden.
    TreasureMap(bool),
    /// Sly 3: Murray rolling as a ball (Be The Ball) or not.
    Rolling(bool),
    /// Sly 3: the Grapple-Cam launched (its controls are the object model's).
    GrappleCam,
    /// Took a hit.
    Hurt(Reaction),
    /// Bentley landed after a toss and becomes the player.
    BecamePlayer(Who),
}

/// The rules the gang shares: constants, tunables, the item table and what the gang owns.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    /// Executable values.
    pub consts: GangConstants,
    /// Tunables.
    pub tun: GangTunables,
    /// The item table.
    pub items: ItemTable,
    /// Items owned.
    pub owned: Owned,
}

impl Shared {
    /// Whether the item with behaviour `b` is owned (for Sly 3 through its own item number).
    pub fn has(&self, b: u8) -> bool {
        self.items.actual(b).is_some_and(|n| self.owned.has(n))
    }
}

/// Sly 3's Fishing Pole line (gadgets.md 6.5): hook state 0 searching, 1 hooked, 2 held (reeling).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fishing {
    /// The slot button holding the line out.
    pub button: u32,
    /// Hook state.
    pub hook: u8,
    /// The hooked target.
    pub target: Option<u32>,
    /// Time in the hook state.
    pub time: f32,
}

impl Fishing {
    /// A target within this distance of the tip is hooked.
    pub const HOOK_REACH: f32 = 250.0;
    /// Hooked this long while still in reach, the target is held.
    pub const HOLD_TIME: f32 = 0.35;
    /// While reeling, the line lets go of a target farther than this from the tip.
    pub const LET_GO: f32 = 400.0;
}

/// Contact kind `0x8000`: the swarmers' pop-up cobra and tentacle (characters.md 2.2).
pub const POPUP_KIND: u32 = 0x8000;
/// Sound of a pop-up hit's second reaction.
pub const HELD_REACTION_SOUND: i32 = 0x6A;

/// The hook swing's values (characters.md 9.1.1) [D] [R].
pub mod swing {
    /// The catch distance from the cane tip's segments to the hook's point.
    pub const CATCH: f32 = 15.0;
    /// A cane-reach target is within this distance.
    pub const REACH: f32 = 1000.0;
    /// How far above a target Sly may be (not given) [I].
    pub const REACH_ABOVE: f32 = 300.0;
    /// The jump times a target must be reachable within.
    pub const JUMP_TIME: [f32; 2] = [0.25, 3.0];
    /// The cane tip's height and reach ahead at Sly's hands [I].
    pub const TIP_HEIGHT: f32 = 150.0;
    /// See [`TIP_HEIGHT`].
    pub const TIP_AHEAD: f32 = 40.0;
    /// The stick counts past this deflection.
    pub const STICK: f32 = 0.3;
    /// Within this of the heading it pumps ahead (60°), beyond [`BACK`] back (120°).
    pub const AHEAD: f32 = core::f32::consts::FRAC_PI_3;
    /// See [`AHEAD`].
    pub const BACK: f32 = 2.0 * core::f32::consts::FRAC_PI_3;
    /// A pump adds this (rad/s per second) about the swing axis for its first [`PUMP_TIME`].
    pub const PUMP_RATE: f32 = 5.0;
    /// See [`PUMP_RATE`].
    pub const PUMP_TIME: f32 = 0.25;
    /// The pump's scale clamp(0.5 − 0.01·h, 0, 1).
    pub const PUMP_GAIN: [f32; 2] = [0.5, 0.01];
    /// Held longer than this, the pump turns to coasting.
    pub const COAST_AFTER: f32 = 1.4;
    /// In-plane damping clamp(1.5 − 0.35|ω|, −5, −0.25).
    pub const DAMP_IN: [f32; 2] = [1.5, 0.35];
    /// Out-of-plane damping while pumping, else.
    pub const DAMP_OUT: [f32; 2] = [-2.5, -1.0];
    /// The rope's length and the shortest length the rescale divides by.
    pub const ROPE: f32 = 230.0;
    /// See [`ROPE`].
    pub const ROPE_MIN: f32 = 75.0;
    /// How fast the rope eases to its length, per second (not given) [I].
    pub const ROPE_EASE: f32 = 5.0;
    /// Cross jumps off only after this long on the hook.
    pub const RELEASE_AFTER: f32 = 0.25;
    /// The jump off keeps the swing's velocity with at least this upward.
    pub const RELEASE_UP: f32 = 700.0;
}

/// Sly 3's Rocket Boots (gadgets.md 6.2): the start-up lasts at least this long.
pub const ROCKET_START: f32 = 0.07;
/// Rocket Boots: the start-up also lasts until Sly's heading is within this of the stick's direction.
pub const ROCKET_ALIGN: f32 = 0.1;
/// Rocket Boots: a wall stops the run when the cosine between Sly's facing and its normal is below this (q(u) at
/// u = 0; the factor u is not given).
pub const ROCKET_WALL: f32 = -0.6;
/// Sly 3's treasure map stays out at least this long (gadgets.md 6.3).
pub const MAP_MIN_TIME: f32 = 0.5;
/// Sly 3's Be The Ball (gadgets.md 6.4): the ball's collision radius.
pub const BALL_RADIUS: f32 = 75.0;
/// Be The Ball (gadgets.md 6.10): Square's spin target per unit of stick magnitude (rad/s), about the axis at the
/// heading + π/2.
pub const BALL_SPIN: f32 = 14.0;
/// Be The Ball: the most one Square press changes the spin by (rad/s).
pub const BALL_SPIN_STEP: f32 = 4.0;
/// Be The Ball: the random spin added on entering the ball (length range, rad/s; any horizontal direction).
pub const BALL_TUMBLE: [f32; 2] = [1.5, 2.5];
/// Sly 3's treasure map snaps Sly's heading to this many directions (45°; written by the map's set-up, 6.10).
pub const MAP_DIRECTIONS: u32 = 8;
/// Be The Ball: Cross jumps to this height (the launch speed is √(|g| × 630)).
pub const BALL_JUMP: f32 = 315.0;

/// Murray's punch combo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Combo {
    /// Current punch clip.
    pub index: u8,
    /// The next punch queued.
    pub queued: Option<u8>,
    /// Punches thrown in this combo.
    pub count: u8,
}

/// One character.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    /// Which one.
    pub who: Who,
    /// Feet position.
    pub position: Vec3,
    /// Velocity.
    pub velocity: Vec3,
    /// Heading.
    pub yaw: f32,
    /// State number.
    pub state: u8,
    /// Time in the state.
    pub state_time: f32,
    /// On the ground.
    pub grounded: bool,
    off_floor: f32,
    /// Sly 2 hook (physics.md 5.1): the contact normals of the last move (pointing at the character), for the stuck
    /// test; the state time of the last test; and the last floor it stood on, where a stuck character is returned.
    contacts: Vec<Vec3>,
    stuck_checked: f32,
    /// See `contacts`.
    pub safe: Option<Vec3>,
    /// Health.
    pub health: Health,
    /// Gadget power.
    pub power: Power,
    /// Gadget slots.
    pub slots: Slots,
    buffer: PressBuffer,
    /// What is carried (Murray), or the held gadget's item number (Bentley).
    pub carrying: Option<u32>,
    /// Throw aim while aiming.
    pub aim: Option<ThrowAim>,
    /// Murray's combo.
    pub combo: Combo,
    flop_phase: u8,
    hold_idle: f32,
    /// Hold level while held by a script (Murray 6).
    pub hold_level: u8,
    /// Bentley's dart reload time left.
    pub dart_reload: f32,
    /// Bentley's darts in flight or stuck: (position, age).
    pub darts: Vec<(Vec3, f32)>,
    /// Bentley's thrown gadgets still out (item numbers).
    pub gadgets_out: Vec<u8>,
    hover: Option<f32>,
    /// The held (button-use 1 or draining) item and the slot button holding it.
    pub active: Option<(u8, u32)>,
    /// Game-clock scale asked by an item (Thief Reflexes 0.25, Time Rush 1.35).
    pub time_scale: f32,
    /// Bentley was thrown by Murray and becomes the player on landing.
    pub tossed: bool,
    /// An item waiting for its activation event, and how long it has waited.
    pub pending: Option<(u8, f32)>,
    /// The overlay applied (15, 34, 35).
    pub overlay: Option<u8>,
    /// Time since the last landing.
    pub since_landed: f32,
    /// Paragliding.
    pub gliding: bool,
    /// A move item running, and its time left.
    pub moving: Option<(u8, f32)>,
    /// Height of the last floor stood on (jumps are measured from it).
    pub floor_z: f32,
    /// The kind of the current jump (characters.md 1.2; 0 when falling without a jump).
    pub jump_kind: u8,
    /// Sly's double jump was used in this jump.
    pub double_used: bool,
    /// Something Murray dropped this frame (for the object model), with its velocity.
    pub dropped: Option<(u32, Vec3)>,
    /// This jump's air-steering factor (1; Feral Pounce 1.75; Mega Jump 1.5).
    pub steer_factor: f32,
    /// Physical size (Sly is 0.75 during a Mega Jump).
    pub size: f32,
    /// The juggle strike's or the dodge's target.
    pub move_target: Option<u32>,
    /// The dodge's side (+1 left of the target, −1 right).
    pub dodge_side: f32,
    /// The stick last frame, as a unit world direction.
    pub last_stick: Vec3,
    /// The stick's deflection last frame.
    pub last_deflection: f32,
    /// Sly 3: Shadow Power level while invisible (0 visible).
    pub invisible: u8,
    /// Sly 3: the disguise worn.
    pub disguise: Option<u8>,
    /// Sly 3: Rocket Boots on.
    pub rocket: bool,
    /// Sly 3: Rocket Boots' start-up time so far, until the run starts (sub-state 6).
    pub rocket_start: Option<f32>,
    /// Sly 3: the most head-on wall met by the last move (its normal), if any.
    pub wall: Option<Vec3>,
    /// Sly 3: the Fishing Pole's line, while out.
    pub fishing: Option<Fishing>,
    /// Sly 3: time since the treasure map was taken out.
    pub map_time: f32,
    /// Sly 3: Be The Ball's spin (angular velocity, rad/s; the ball moves only by rolling on it).
    pub ball_spin: Vec3,
    /// Sly 3: the state of the ball's random spins (deterministic).
    pub ball_seed: u32,
    /// The hook Sly swings on and the rope's length (state 3).
    pub hook: Option<(u32, f32)>,
    /// The swing angle θ: the hook direction's pitch from straight up in the heading's frame (characters.md 9.1.1;
    /// positive with the hook ahead). 0 when not swinging.
    pub swing_angle: f32,
    /// The stick's angle against the heading while pushed past 0.3, for the swing blend (`SwingBlend::step`).
    pub swing_stick: Option<f32>,
    /// The swing's angular velocity about the hook.
    pub swing_omega: Vec3,
    /// Time on the hook.
    pub swing_time: f32,
    /// The pump: direction (1 ahead, −1 back, 0 none), time in it, and how long it has been held.
    pub pump: (i8, f32, f32),
    /// Cane-reach mode: the hook a jump is aimed at.
    pub reach: Option<u32>,
    /// The cane tip last step (its path is tested for a catch).
    pub last_tip: Vec3,
    /// While aiming a throw, the predicted flight's points (for the throw marker, `gang::parts`).
    pub aim_path: Vec<Vec3>,
    /// A pop-up hit (contact kind `0x8000`, characters.md 2.2): 1 while the knock-back reaction runs, 2 during the
    /// second, held reaction that follows it; 0 otherwise.
    pub held_reaction: u8,
    /// Sly 3: Bentley's Grapple-Cam controller while the camera is out, and the time in its phase.
    pub grapple: Option<(super::sly3::GrappleCam, f32)>,
    /// Sly 3: Murray rolling (Be The Ball).
    pub rolling: bool,
    /// Sly 3: the cane combo: Triangle presses so far and the time since the last.
    pub cane: (u8, f32),
    /// Sly 3: the character's combo definitions (class 148; `ComboSet::from_world`). Empty: finishers only.
    pub combo_defs: super::combo::ComboSet,
    /// Sly 3: the combo runner.
    pub combo_runner: super::combo::Runner,
}

fn heading(yaw: f32) -> Vec3 {
    [cos(yaw), sin(yaw), 0.0]
}

impl Character {
    /// A character standing at `position`.
    pub fn new(who: Who, position: Vec3, yaw: f32) -> Character {
        Character {
            who,
            position,
            velocity: [0.0; 3],
            yaw,
            state: 0,
            state_time: 0.0,
            grounded: true,
            off_floor: 0.0,
            contacts: Vec::new(),
            stuck_checked: 0.0,
            safe: None,
            health: Health::default(),
            power: Power::new(spec::POWER_START, spec::POWER_MAX),
            slots: Slots::default(),
            buffer: PressBuffer::default(),
            carrying: None,
            aim: None,
            combo: Combo::default(),
            flop_phase: 0,
            hold_idle: 0.0,
            hold_level: 0,
            dart_reload: 0.0,
            darts: Vec::new(),
            gadgets_out: Vec::new(),
            hover: None,
            active: None,
            time_scale: 1.0,
            tossed: false,
            pending: None,
            overlay: None,
            since_landed: 10.0,
            gliding: false,
            moving: None,
            floor_z: position[2],
            jump_kind: 0,
            double_used: false,
            dropped: None,
            steer_factor: 1.0,
            size: 1.0,
            move_target: None,
            dodge_side: 1.0,
            last_stick: [0.0; 3],
            last_deflection: 0.0,
            invisible: 0,
            disguise: None,
            rocket: false,
            rocket_start: None,
            wall: None,
            fishing: None,
            map_time: 0.0,
            ball_spin: [0.0; 3],
            ball_seed: 0x9E37_79B9,
            held_reaction: 0,
            aim_path: Vec::new(),
            hook: None,
            swing_angle: 0.0,
            swing_stick: None,
            swing_omega: [0.0; 3],
            swing_time: 0.0,
            pump: (0, 0.0, 0.0),
            reach: None,
            last_tip: [0.0; 3],
            grapple: None,
            rolling: false,
            cane: (0, 0.0),
            combo_defs: super::combo::ComboSet::default(),
            combo_runner: super::combo::Runner::default(),
        }
    }

    fn idx(&self) -> usize {
        self.who.index()
    }

    fn shape(&self, s: &Shared) -> CharacterShape {
        let i = self.idx();
        if self.rolling {
            return CharacterShape::Capsule { height: 2.0 * BALL_RADIUS, radius: BALL_RADIUS };
        }
        CharacterShape::Capsule { height: s.tun.height[i] * self.size, radius: s.tun.radius[i] * self.size }
    }

    /// The body's radius (Be The Ball's 75 while rolling).
    fn body_radius(&self, s: &Shared) -> f32 {
        if self.rolling { BALL_RADIUS } else { s.tun.radius[self.idx()] }
    }

    fn origin(&self, s: &Shared) -> Vec3 {
        add(self.position, [0.0, 0.0, self.body_radius(s)])
    }

    /// Changes state, reporting the change.
    pub fn set_state(&mut self, to: u8, out: &mut Vec<GangEvent>) {
        if to != self.state {
            out.push(GangEvent::State { from: self.state, to });
            self.state = to;
            self.state_time = 0.0;
        }
    }

    /// The air state number (2 for all three).
    fn air_state(&self) -> u8 {
        2
    }

    /// Whether the state is one of the ground states that read the ground choices.
    fn on_ground_choices(&self) -> bool {
        match self.who {
            Who::Sly => matches!(self.state, sly::STAND | sly::RUN | sly::SNEAK | sly::SNEAK_RUN | sly::FIGHT),
            Who::Murray => matches!(self.state, murray::STAND | murray::RUN | murray::WALK | murray::STANCE),
            Who::Bentley => matches!(self.state, bentley::STAND | bentley::MOVE | bentley::WALK | bentley::NEAR),
        }
    }

    /// The nearest target accepted by `pick` within `reach`, ahead within `angle` of the heading.
    fn nearest(&self, world: &GangWorld<'_>, reach: f32, angle: f32, pick: impl Fn(&Target) -> bool) -> Option<Target> {
        let f = heading(self.yaw);
        let mut best: Option<(f32, Target)> = None;
        for t in world.targets.iter().filter(|t| pick(t)) {
            let d = sub(t.position, self.position);
            let hd = length([d[0], d[1], 0.0]);
            if hd > reach {
                continue;
            }
            let a = if hd > 1e-3 { crate::math::acos(dot(normalize([d[0], d[1], 0.0]), f)) } else { 0.0 };
            if a > angle {
                continue;
            }
            if best.is_none_or(|(b, _)| hd < b) {
                best = Some((hd, *t));
            }
        }
        best.map(|b| b.1)
    }

    /// The guard the dodge circles (gadgets.md 5.4): of every active guard, with no distance limit, the lowest
    /// score d − c·(500 + 0.5·d), d the horizontal distance and c = clamp(0.5 − 0.5·k, 0, 1), k the stick's
    /// deflection times the cosine between the stick and the direction from the guard to the player.
    pub fn dodge_target(&self, world: &GangWorld<'_>) -> Option<Target> {
        let [a, b] = spec::DODGE_SCORE;
        let mut best: Option<(f32, Target)> = None;
        for t in world.targets.iter().filter(|t| matches!(t.kind, TargetKind::Guard { .. })) {
            let away = sub(self.position, t.position);
            let away = [away[0], away[1], 0.0];
            let d = length(away);
            let k = if d > 1e-3 && self.last_deflection > 0.0 {
                self.last_deflection
                    * dot(normalize([self.last_stick[0], self.last_stick[1], 0.0]), scale(away, 1.0 / d))
            } else {
                0.0
            };
            let c = (0.5 - 0.5 * k).clamp(0.0, 1.0);
            let score = d - c * (a + b * d);
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, *t));
            }
        }
        best.map(|b| b.1)
    }

    /// Applies a hit (characters.md 2).
    pub fn hit(&mut self, c: &GangConstants, h: &Hit, world: &GangWorld<'_>, out: &mut Vec<GangEvent>) -> Reaction {
        let r = self.health.hit(c, h, self.position, world.help, world.safehouse);
        if let Reaction::Hurt { velocity, .. } = r {
            self.velocity = velocity;
            if velocity[2] > 0.0 {
                self.grounded = false;
            }
            let dmg = match self.who {
                Who::Sly => sly::DAMAGE,
                Who::Murray => murray::DAMAGE,
                Who::Bentley => bentley::DAMAGE,
            };
            if self.who == Who::Murray
                && let Some(what) = self.carrying.take()
            {
                // Dropped: (−400, 0, 600) in Murray's frame (characters.md 4.6); Bentley's own drop is the gang's.
                let (sy, cy) = (sin(self.yaw), cos(self.yaw));
                let velocity = [-400.0 * cy, -400.0 * sy, 600.0];
                if Who::from_target_id(what).is_none() {
                    out.push(GangEvent::Dropped { id: what, velocity });
                }
                self.aim = None;
            }
            self.set_state(dmg, out);
            self.held_reaction = u8::from(h.kind & POPUP_KIND != 0);
        }
        if r != Reaction::Ignored {
            out.push(GangEvent::Hurt(r));
        }
        r
    }

    /// Marks the character airborne (a launch by a hit or a throw): the 0.1 s off-floor grace is spent.
    pub fn leave_ground(&mut self) {
        self.grounded = false;
        self.off_floor = spec::FALL_DELAY + 1.0;
    }

    /// Puts Murray in his held-by-script state at `level` (characters.md 4.6).
    pub fn hold(&mut self, level: u8, out: &mut Vec<GangEvent>) {
        self.hold_level = level;
        self.hold_idle = 0.0;
        self.set_state(murray::HELD, out);
    }

    /// Puts Bentley in the timed state (back to standing after 2 s).
    pub fn enter_timed(&mut self, out: &mut Vec<GangEvent>) {
        self.set_state(bentley::TIMED, out);
    }

    /// One frame for the current player (`input`) or a waiting character (default input).
    pub fn update(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        self.state_time += dt;
        self.buffer.update(input, dt);
        self.health.step(dt);
        self.time_scale = 1.0;
        let (stick_dir, stick_u) = self.stick(input, s);
        self.last_stick = stick_dir;
        self.last_deflection = stick_u;
        self.update_gadgets(dt, input, world, s, out);
        match self.who {
            Who::Sly => self.sly(dt, input, world, s, out),
            Who::Murray => self.murray(dt, input, world, s, out),
            Who::Bentley => self.bentley(dt, input, world, s, out),
        }
        // Sly 2 hook (physics.md 5.1): a character hanging stuck in a jump, fall or hit reaction is recovered.
        if !s.items.is_sly3() {
            self.stuck_recovery(s, out);
        }
        for d in &mut self.darts {
            d.1 += dt;
        }
        self.darts.retain(|d| d.1 < spec::DART[0]);
        self.dart_reload = (self.dart_reload - dt).max(0.0);
    }

    /// The stuck test and the recovery (physics.md 5.1). In a jump or fall or a hit reaction, off the floor, at least
    /// 1 s after the state began and at most once a second: the last contact normals, combined with non-negative
    /// weights, cancel "down" as far as they can; if what is left of it has z above −0.1, the character is stuck. It
    /// is then returned to the last floor it stood on: by a jump there, or, when that jump would take more than
    /// 1.25 s, placed there (the camera cut and fade are the game's).
    fn stuck_recovery(&mut self, s: &Shared, out: &mut Vec<GangEvent>) {
        let damage = match self.who {
            Who::Sly => sly::DAMAGE,
            Who::Murray => murray::DAMAGE,
            Who::Bentley => bentley::DAMAGE,
        };
        if self.grounded || (self.state != self.air_state() && self.state != damage) {
            return;
        }
        if self.state_time < self.stuck_checked {
            self.stuck_checked = 0.0;
        }
        if self.state_time < STUCK_AFTER
            || (self.stuck_checked > 0.0 && self.state_time - self.stuck_checked < STUCK_AFTER)
        {
            return;
        }
        self.stuck_checked = self.state_time;
        if down_left(&self.contacts)[2] <= STUCK_DOWN {
            return;
        }
        let Some(to) = self.safe else { return };
        let d = sub(to, self.position);
        // Jump kind 10's speed is not given: the ordinary jump's [I].
        let v0 = jump_speed(s.consts.jump_heights[0], 0.0);
        let g = spec::GRAVITY;
        let disc = v0 * v0 + 2.0 * g * d[2];
        let t = if disc >= 0.0 { (-v0 - crate::math::sqrt(disc)) / g } else { f32::INFINITY };
        if t.is_finite() && t > 0.0 && t <= RECOVER_JUMP {
            self.velocity = [d[0] / t, d[1] / t, v0];
            self.jump_kind = 10;
            self.set_state(self.air_state(), out);
        } else {
            self.position = to;
            self.velocity = [0.0; 3];
            self.grounded = true;
            self.off_floor = 0.0;
            self.set_state(0, out);
        }
    }

    /// The stick as (unit world direction, deflection): the gang's stick reader has applied the games' rule
    /// (`player::stick`); without one (a hand-made input) the stick's length is the deflection.
    fn stick(&self, input: &GangInput, s: &Shared) -> (Vec3, f32) {
        let _ = s;
        let w = input.world_stick();
        let l = length(w);
        if l <= 1e-4 {
            return ([0.0; 3], 0.0);
        }
        let u = if input.deflection > 0.0 { input.deflection } else { l.min(1.0) };
        (normalize(w), u.min(1.0))
    }

    /// Ground or air movement with collisions; returns whether the character is on the ground afterwards.
    fn move_body(&mut self, dt: f32, world: &GangWorld<'_>, s: &Shared, want: Vec3, gravity: bool) -> bool {
        let shape = self.shape(s);
        let params = if self.grounded {
            SlideParams { step_up: Some(25.0), snap_down: Some(30.0), ..SlideParams::default() }
        } else {
            SlideParams::default()
        };
        let vz0 = self.velocity[2];
        let g = if gravity && !self.grounded { spec::GRAVITY } else { 0.0 };
        let vz1 = if self.grounded { 0.0 } else { vz0 + g * dt };
        let delta = [want[0] * dt, want[1] * dt, if self.grounded { 0.0 } else { 0.5 * (vz0 + vz1) * dt }];
        let start_z = self.position[2];
        let r = move_and_slide(world.cw, &world.filter, shape, self.origin(s), delta, &params);
        self.position = sub(r.position, [0.0, 0.0, self.body_radius(s)]);
        self.velocity = [want[0], want[1], vz1];
        // Sly 2 hook (physics.md 5.1): a blocked contact takes away only the part of the velocity going into the
        // surface, so the character slides along it; held between two steep surfaces it hangs, its fall speed no
        // longer growing.
        self.contacts.clear();
        self.contacts.extend(r.hits.iter().map(|h| h.normal));
        if !s.items.is_sly3() {
            for _ in 0..2 {
                for n in self.contacts.iter().filter(|n| n[2] <= crate::collision::mesh::GROUND_Z) {
                    let into = dot(self.velocity, *n);
                    if into < 0.0 {
                        self.velocity = madd(self.velocity, *n, -into);
                    }
                }
            }
        }
        // The most head-on wall (a contact with |normal z| < 0.7), for Rocket Boots.
        let facing = heading(self.yaw);
        self.wall = r
            .hits
            .iter()
            .filter(|h| h.normal[2].abs() < 0.7)
            .map(|h| h.normal)
            .min_by(|a, b| dot(*a, facing).total_cmp(&dot(*b, facing)));
        for h in &r.hits {
            if h.normal[2] < -0.3 && self.velocity[2] > 0.0 {
                self.velocity[2] = 0.0;
            }
        }
        let mut on = r.ground.is_some() && self.velocity[2] <= 0.0;
        // A floor whose stored surfaces do not cover this point (an incomplete outline): the collision tree's
        // one-sided rule finds it (TriMesh::tree_floor). Only where no stored surface is below within the reach, so
        // a floor the mesh has keeps its own height.
        // It also wins over a lower surface the capsule's snap found beside the hole.
        if self.velocity[2] <= 0.0 {
            // From just above the feet (or, after a snap down, above where the step began) down this frame's drop
            // and the 30-unit snap while walking: a floor the character reaches now, never one above its feet.
            let from = [self.position[0], self.position[1], start_z.max(self.position[2]) + 2.0];
            let reach = (from[2] - self.position[2]) + if self.grounded { 30.0 } else { 0.0 } + (-delta[2]).max(0.0);
            if world.cw.ground_probe(from, reach + 50.0, &world.filter).is_none()
                && let Some(g) = world.cw.tree_ground_probe(from, reach, &world.filter)
                && g.is_ground
                && (!on || (g.point[2] > self.position[2] && g.point[2] - self.position[2] <= 8.0))
            {
                self.position[2] = g.point[2];
                on = true;
            }
        }
        if on {
            self.reach = None;
            self.velocity[2] = 0.0;
            self.off_floor = 0.0;
            self.floor_z = self.position[2];
            self.safe = Some(self.position);
            self.jump_kind = 0;
            self.double_used = false;
            self.steer_factor = 1.0;
            self.size = 1.0;
        } else {
            self.off_floor += dt;
        }
        // Off the floor for more than 0.1 s starts a fall.
        self.grounded = on || (self.grounded && self.off_floor <= spec::FALL_DELAY);
        on
    }

    /// Walk or run along the stick at `speed` (the gait's root speed), turning toward it.
    fn walk(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, speed: f32) -> bool {
        let (dir, u) = self.stick(input, s);
        if u > 0.0 {
            self.yaw = turn_toward(self.yaw, atan2(dir[1], dir[0]), s.tun.turn_rate, dt);
        }
        let v = if u > 0.0 { scale(heading(self.yaw), speed) } else { [0.0; 3] };
        self.move_body(dt, world, s, v, true)
    }

    /// The stick's deflection after the dead zone.
    fn deflection(&self, input: &GangInput, s: &Shared) -> f32 {
        self.stick(input, s).1
    }

    /// The launch speed of jump `kind` (characters.md 1.2): kinds 1, 7 and 15 bring the base point to 215 above the
    /// last floor; 13, 14 and 9 to 500, 250 and 1,200; 2 is 700, 3 is 800, 18 is 900.
    pub fn jump_kind_speed(&self, c: &GangConstants, kind: u8) -> f32 {
        let h = (self.position[2] - self.floor_z).max(0.0);
        match kind {
            1 | 7 | 15 => jump_speed(c.jump_heights[0], h),
            13 => jump_speed(500.0, h),
            14 => jump_speed(250.0, h),
            9 => jump_speed(1200.0, h),
            2 => 700.0,
            3 => 800.0,
            18 => 900.0,
            _ => 0.0,
        }
    }

    fn jump(&mut self, s: &Shared, kind: u8, out: &mut Vec<GangEvent>) {
        let speed = self.jump_kind_speed(&s.consts, kind);
        self.jump_kind = kind;
        self.double_used = false;
        self.steer_factor = 1.0;
        self.velocity[2] = self.velocity[2].max(speed);
        self.grounded = false;
        self.off_floor = spec::FALL_DELAY + 1.0;
        out.push(GangEvent::Jumped);
        let a = self.air_state();
        self.set_state(a, out);
    }

    /// Air physics and landing; returns true on the frame of landing.
    fn air(
        &mut self,
        dt: f32,
        input: &GangInput,
        world: &GangWorld<'_>,
        s: &Shared,
        steer: f32,
        out: &mut Vec<GangEvent>,
    ) -> bool {
        let (dir, u) = self.stick(input, s);
        let want = scale(dir, steer * u);
        let h = [self.velocity[0], self.velocity[1], 0.0];
        let h = madd(h, sub(want, h), (3.0 * dt).min(1.0));
        let landed = self.move_body(dt, world, s, h, true);
        if landed {
            self.since_landed = 0.0;
            self.gliding = false;
            out.push(GangEvent::Landed);
        }
        landed
    }

    /// Paragliding (gadgets.md 5.1): in the glider's frame each velocity component eases toward a target set by
    /// f = (forward part of the stick + 1) / 2: forward 300 to 800, vertical −200 to −400, sideways 0, with the easings
    /// (800, 0, 0.1) and (5000, 0, 0.1); the acceleration is capped at 8,000 and is never downward (gravity pulls
    /// down, the easing holds the sink at its target). The heading turns at rate × s, with s = stick magnitude ×
    /// sin(stick heading − body heading) (the stick's sideways part relative to the body) and the rate blended by f
    /// from 5 (stick back) to 12 rad/s (stick forward); headings run counter-clockwise, so a stick to the left turns
    /// Sly left (gadgets.md 5.4). Returns true on landing.
    fn glide(
        &mut self,
        dt: f32,
        input: &GangInput,
        world: &GangWorld<'_>,
        s: &Shared,
        out: &mut Vec<GangEvent>,
    ) -> bool {
        let c = &s.consts;
        let w = input.world_stick();
        let fwd_axis = heading(self.yaw);
        let left_axis = [-fwd_axis[1], fwd_axis[0], 0.0];
        let (sf, ss) = (dot(w, fwd_axis).clamp(-1.0, 1.0), dot(w, left_axis).clamp(-1.0, 1.0));
        let f = (sf + 1.0) * 0.5;
        let mix = |p: [f32; 2]| p[0] * (1.0 - f) + p[1] * f;
        self.yaw += mix(c.glide[2]) * ss * dt;
        let (fwd_axis, left_axis) = (heading(self.yaw), [-heading(self.yaw)[1], heading(self.yaw)[0], 0.0]);
        let v = self.velocity;
        // Vertically the easing acts on the speed gravity leaves this frame.
        let comps = [dot(v, fwd_axis), dot(v, left_axis), v[2] + spec::GRAVITY * dt];
        let targets = [mix(c.glide[0]), 0.0, mix(c.glide[1])];
        let mut acc = [0.0f32; 3];
        for k in 0..3 {
            let e = c.glide_ease[k];
            let p = crate::physics::SmoothParams { fast: e[0], slow: e[1], fast_time: e[2], max_speed_change: None };
            let d = targets[k] - comps[k];
            // The law gives the rate of change; within the frame it never passes the target.
            acc[k] = (p.speed_at(d.abs()) * d.signum()).clamp(-d.abs() / dt, d.abs() / dt);
        }
        let mut a = add(add(scale(fwd_axis, acc[0]), scale(left_axis, acc[1])), [0.0, 0.0, acc[2]]);
        let al = length(a);
        if al > c.glide_accel {
            a = scale(a, c.glide_accel / al);
        }
        // The easing never pulls downward; gravity does.
        a[2] = a[2].max(0.0) + spec::GRAVITY;
        let v = madd(v, a, dt);
        let r = move_and_slide(
            world.cw,
            &world.filter,
            self.shape(s),
            self.origin(s),
            scale(v, dt),
            &SlideParams::default(),
        );
        self.position = sub(r.position, [0.0, 0.0, s.tun.radius[self.idx()] * self.size]);
        self.velocity = v;
        if r.ground.is_some() && v[2] <= 0.0 {
            self.gliding = false;
            self.grounded = true;
            self.velocity = [0.0; 3];
            self.since_landed = 0.0;
            self.floor_z = self.position[2];
            out.push(GangEvent::Landed);
            return true;
        }
        false
    }

    /// Starts Sly's juggle strike (state 16) on an enemy he knocked into the air; the object model calls it.
    pub fn juggle_strike(&mut self, target: u32, out: &mut Vec<GangEvent>) {
        if self.who == Who::Sly {
            self.move_target = Some(target);
            self.set_state(sly::JUGGLE_STRIKE, out);
        }
    }

    /// Will the character land within `t` at its current velocity?
    fn lands_within(&self, world: &GangWorld<'_>, s: &Shared, t: f32) -> bool {
        let drop = (-self.velocity[2] * t - 0.5 * spec::GRAVITY * t * t).max(1.0);
        ground_below(world.cw, &world.filter, self.shape(s), self.origin(s), drop, crate::collision::mesh::GROUND_Z)
            .is_some()
    }

    // ---- gadgets -------------------------------------------------------------------------------------------

    /// Whether the character can use `item` in its current state (gadgets.md 2 and 4).
    fn usable(&self, item: u8) -> bool {
        let hurt = match self.who {
            Who::Sly => self.state == sly::DAMAGE,
            Who::Murray => self.state == murray::DAMAGE,
            Who::Bentley => matches!(self.state, bentley::DAMAGE | bentley::CARRIED),
        };
        if hurt {
            return false;
        }
        match (self.who, item) {
            // Combat Dodge only in combat.
            (Who::Sly, 24) => self.state == sly::FIGHT,
            // Thief Reflexes and Time Rush: not while the other one sets the game's rate (state 4 is excluded above).
            (Who::Sly, 29) => self.active.is_none_or(|a| a.0 != id::TIME_RUSH),
            (Who::Sly, 42) => self.active.is_none_or(|a| a.0 != id::THIEF_REFLEXES),
            (Who::Bentley, id::HOVER_PACK) => self.state == bentley::AIR && !self.tossed,
            // Diablo Fire Slam only while carrying an enemy.
            (Who::Murray, 19) => self.carrying.is_some() && self.state == murray::CARRY,
            (Who::Murray, 22) => self.state == murray::AIR,
            _ => true,
        }
    }

    fn update_gadgets(
        &mut self,
        dt: f32,
        input: &GangInput,
        world: &GangWorld<'_>,
        s: &Shared,
        out: &mut Vec<GangEvent>,
    ) {
        let c = &s.consts;
        self.since_landed += dt;
        // A held or draining item.
        if let Some((item, b)) = self.active {
            // Sly 3: field +16 of the item's record, 0 included (the Treasure Map is held without cost).
            let own = s.items.actual(item).and_then(|n| s.items.drain(n));
            let own = if s.items.is_sly3() { Some(own.unwrap_or(0.0)) } else { own };
            let drain = match item {
                _ if own.is_some() => own.unwrap_or(0.0),
                id::THIEF_REFLEXES => c.drains[0],
                id::TIME_RUSH => c.drains[1],
                id::SHADOW_POWER => c.drains[2],
                id::HOVER_PACK => c.drains[3],
                id::STEALTH_SLIDE => spec::SLIDE_DRAIN,
                _ => 0.0,
            };
            let alive = self.power.drain(drain, dt);
            let held = input.holds(b);
            let hover_min = item == id::HOVER_PACK && self.hover.is_some_and(|t| t < c.hover[3]);
            // The treasure map stays out at least 0.5 s.
            self.map_time += dt;
            let map_min = item == behaviour::TREASURE_MAP && self.map_time < MAP_MIN_TIME;
            if !alive || (!held && !hover_min && !map_min) {
                self.active = None;
                self.hover = None;
                out.push(GangEvent::ItemEnded(item));
                if self.invisible > 0 {
                    self.invisible = 0;
                    out.push(GangEvent::Invisible(0));
                }
                if item == behaviour::TREASURE_MAP {
                    out.push(GangEvent::TreasureMap(false));
                }
                self.rocket = false;
                self.rocket_start = None;
            } else {
                self.time_scale = match item {
                    id::THIEF_REFLEXES => c.time_scales[0],
                    id::TIME_RUSH => c.time_scales[1],
                    _ => 1.0,
                };
            }
        }
        // An item waiting for its activation event (when no animation driver reports it first).
        if let Some((item, t)) = self.pending {
            let t = t + dt;
            if t >= s.tun.activation_time {
                self.activation_event(s, out);
            } else {
                self.pending = Some((item, t));
            }
        }
        // Sly 3's Fishing Pole.
        if self.fishing.is_some() {
            self.fish(dt, input, world, s, out);
        }
        // Sly 3's Grapple-Cam.
        if self.grapple.is_some() {
            self.grapple_cam(dt, s, out);
        }
        // A move item running.
        if let Some((item, t)) = self.moving {
            let t = t - dt;
            self.moving = if t > 0.0 { Some((item, t)) } else { None };
        }
        // Paraglide (27): R1 in the air, not within 0.3 s of landing.
        if self.who == Who::Sly
            && s.has(27)
            && !self.gliding
            && !self.grounded
            && self.since_landed >= PARAGLIDE_LANDING
            && self.state == sly::AIR
            && self.buffer.take(button::R1, spec::WINDOW_SHORT)
        {
            self.gliding = true;
            out.push(GangEvent::SpecialJump(19));
        }
        for (slot, &b) in button::SLOTS.iter().enumerate() {
            if !self.buffer.peek(b, spec::WINDOW_SHORT) {
                continue;
            }
            let Some(number) = self.slots.0[slot] else { continue };
            self.buffer.take(b, spec::WINDOW_SHORT);
            let Some(rec) = s.items.get(number).copied() else { continue };
            // The behaviour it runs with (Sly 3 items map onto the Sly 2 rules that carry over).
            let item = s.items.behaviour(number);
            // Bentley's thrown gadgets: a press triggers the one out instead of taking another.
            if self.who == Who::Bentley
                && matches!(item, id::TRIGGER_BOMB | id::HEALTH_EXTRACTOR)
                && let Some(i) = self.gadgets_out.iter().position(|&g| g == item)
            {
                self.gadgets_out.remove(i);
                out.push(GangEvent::Triggered(item));
                continue;
            }
            // An applied overlay is removed by using the item again.
            if effect::of(item) == Effect::Overlay && self.overlay == Some(item) {
                self.overlay = None;
                out.push(GangEvent::Overlay { item, on: false });
                continue;
            }
            // Sly 3: while a costume is on, every item but item 3 and the disguises is refused (gadgets.md 6.9).
            let costume = s.items.is_sly3() && self.disguise.is_some() && !matches!(number, 3 | 38..=40);
            let refuse = if !self.usable(item) || costume {
                Some(Refusal::State)
            } else if self.power.value < rec.cost as f32 {
                Some(Refusal::Power)
            } else if effect::pooled(item) && world.free_copies(item) < POOL_MIN_FREE {
                Some(Refusal::Pool)
            } else {
                None
            };
            if let Some(reason) = refuse {
                out.push(GangEvent::ItemRefused { item, reason });
                continue;
            }
            if effect::pay_at(item) == PayAt::Start {
                self.power.pay(rec.cost as f32);
            }
            self.start_item(item, rec.button_use(), b, world, s, out);
        }
    }

    /// The activation event of the item waiting for it (from the animation, or after the activation time): it pays
    /// and takes effect.
    pub fn activation_event(&mut self, s: &Shared, out: &mut Vec<GangEvent>) {
        let Some((item, _)) = self.pending.take() else { return };
        let cost = s.items.actual(item).and_then(|n| s.items.get(n)).map_or(0.0, |r| r.cost as f32);
        self.power.pay(cost);
        out.push(GangEvent::ItemActivated(item));
        // Sly 3's Insanity Strike is a chair blow, not an overlay (characters.md 10).
        if effect::of(item) == Effect::Overlay && !(s.items.is_sly3() && self.who == Who::Bentley) {
            self.overlay = Some(item);
            out.push(GangEvent::Overlay { item, on: true });
        }
        if let Some(w) = effect::world(item, s.consts.smoke_resume, s.consts.temporal_lock) {
            out.push(GangEvent::World(w));
        }
    }

    fn start_item(
        &mut self,
        item: u8,
        use_: ButtonUse,
        b: u32,
        _world: &GangWorld<'_>,
        s: &Shared,
        out: &mut Vec<GangEvent>,
    ) {
        out.push(GangEvent::ItemStarted(item));
        if effect::pay_at(item) == PayAt::Start {
            out.push(GangEvent::ItemActivated(item));
        }
        if use_ == ButtonUse::Hold || matches!(item, id::SHADOW_POWER | id::STEALTH_SLIDE | id::HOVER_PACK) {
            self.active = Some((item, b));
        }
        match (self.who, item) {
            (Who::Bentley, id::HOVER_PACK) => {
                self.hover = Some(0.0);
                out.push(GangEvent::SpecialJump(17));
            }
            (Who::Bentley, 7 | 9 | 11 | 13) => {
                self.carrying = Some(item as u32);
                out.push(GangEvent::Summoned(item));
                self.set_state(bentley::HOLDING, out);
            }
            (Who::Bentley, 10) => {
                // Adrenaline Burst: his sprint, taken as the timed state 14 (1,500 for 2 s) [I].
                self.set_state(bentley::TIMED, out);
                out.push(GangEvent::Move(item));
            }
            // Sly's pocket items: aimed and thrown, paying on the throw.
            (_, 26 | 37 | 38 | 41) => {
                self.carrying = Some(item as u32);
                let aim = match self.who {
                    Who::Sly => sly::THROW_AIM,
                    Who::Murray => murray::THROW_AIM,
                    Who::Bentley => bentley::THROW_AIM,
                };
                self.start_aim(s, aim, out);
            }
            // Sly 3: Bentley's chair melee (characters.md 10): the item's clip, a third of a second, standing.
            (Who::Bentley, 34 | 8) if s.items.is_sly3() => {
                let (clip, time, hit) = super::sly3::chair_melee(item).unwrap_or((-1, 0.0, 0));
                self.moving = Some((item, time));
                out.push(GangEvent::ChairMelee { item, clip, hit });
            }
            (_, code) if code >= behaviour::FISHING_POLE => self.start_sly3_item(code, b, out),
            (Who::Sly, id::SHADOW_POWER) if s.items.is_sly3() => {
                self.invisible = 1;
                out.push(GangEvent::Invisible(1));
            }
            (Who::Sly, id::STEALTH_SLIDE) => {
                self.velocity = scale(heading(self.yaw), gait::SLY_RUN.speed(1.0) * s.consts.run_factor);
                self.set_state(sly::SLIDE, out);
            }
            (Who::Sly, 24) => {
                // The side: the stick against the direction to the target, else the last side.
                let target = self.dodge_target(_world);
                if let Some(g) = target {
                    let d = sub(g.position, self.position);
                    let left = [-d[1], d[0], 0.0];
                    if self.last_deflection > 0.0 {
                        self.dodge_side = if dot(self.last_stick, left) >= 0.0 { 1.0 } else { -1.0 };
                    }
                    self.move_target = Some(g.id);
                } else {
                    self.move_target = None;
                }
                self.moving = Some((item, gait::SLY_DODGE.slow[1]));
                self.set_state(sly::DODGE, out);
                out.push(GangEvent::Move(item));
            }
            (Who::Sly, 30) => {
                // Jump kind 14 to 250, steering × 1.75; with the stick nearly centred a push of 515 × 1.75.
                self.velocity[2] = self.velocity[2].max(self.jump_kind_speed(&s.consts, 14));
                if self.last_deflection < 0.2 {
                    let d = if self.last_stick != [0.0; 3] { self.last_stick } else { heading(self.yaw) };
                    let push = scale(d, s.consts.air_steer * s.consts.pounce_steer);
                    self.velocity[0] += push[0];
                    self.velocity[1] += push[1];
                }
                self.jump_kind = 14;
                self.steer_factor = s.consts.pounce_steer;
                self.leave_ground();
                self.set_state(sly::AIR, out);
                out.push(GangEvent::SpecialJump(14));
            }
            (Who::Sly, 31) => {
                // Jump kind 9 to 1,200, steering × 1.5, Sly at 0.75 of his size until he lands.
                self.velocity[2] = self.velocity[2].max(self.jump_kind_speed(&s.consts, 9));
                self.jump_kind = 9;
                self.steer_factor = s.consts.mega[0];
                self.size = s.consts.mega[2];
                self.leave_ground();
                self.set_state(sly::AIR, out);
                out.push(GangEvent::SpecialJump(9));
            }
            (Who::Sly, 33) => {
                // Knockout Dive: a rush at the nearest guard with clip 189's root speed, ending with the clip.
                self.move_target = self
                    .nearest(_world, s.tun.reach * 4.0, core::f32::consts::PI, |g| {
                        matches!(g.kind, TargetKind::Guard { .. })
                    })
                    .map(|g| g.id);
                self.moving = Some((item, gait::KNOCKOUT_DIVE.slow[1]));
                self.set_state(sly::KNOCKOUT_DIVE, out);
                out.push(GangEvent::Move(item));
            }
            (Who::Sly, 39) => {
                self.moving = Some((item, spec::LIGHTNING_SPIN_TIME));
                out.push(GangEvent::Move(item));
            }
            (Who::Murray, 22) => self.start_flop(out),
            (Who::Murray, 20) => {
                self.set_state(murray::CHARGE, out);
                out.push(GangEvent::Move(item));
            }
            (Who::Murray, 19) => {
                let what = self.carrying.take();
                self.moving = Some((item, spec::SLAM_TIME));
                out.push(GangEvent::Move(item));
                if let Some(w) = what {
                    out.push(GangEvent::Thrown { id: w, velocity: [0.0, 0.0, -500.0] });
                }
                // The slam (state 8), standing still.
                self.set_state(murray::SLAM, out);
            }
            (Who::Murray, 16) => self.set_state(murray::TURNBUCKLE, out),
            _ => {}
        }
        if effect::pay_at(item) == PayAt::Event {
            self.pending = Some((item, 0.0));
        }
    }

    /// The Sly 3 items with no Sly 2 counterpart (specs/sly3/gadgets.md 4, characters.md 5), as far as specified.
    fn start_sly3_item(&mut self, b: u8, btn: u32, out: &mut Vec<GangEvent>) {
        match b {
            // Holding the button keeps the line out (gadgets.md 6.5).
            behaviour::FISHING_POLE => self.fishing = Some(Fishing { button: btn, ..Fishing::default() }),
            // The item's own button deploys the camera straight into phase 4 (control); pressed again in phases 4
            // to 6 it goes to phase 7 (gadgets.md 6.10).
            behaviour::GRAPPLE_CAM => match &mut self.grapple {
                None => {
                    let mut g = super::sly3::GrappleCam::default();
                    g.deploy();
                    self.grapple = Some((g, 0.0));
                    out.push(GangEvent::GrappleCam);
                }
                Some((g, t)) => {
                    if g.item_pressed() {
                        *t = 0.0;
                    }
                }
            },
            behaviour::BE_THE_BALL => {
                self.rolling = !self.rolling;
                self.ball_spin = [0.0; 3];
                if self.rolling {
                    self.tumble();
                }
                out.push(GangEvent::Rolling(self.rolling));
            }
            behaviour::ROCKET_BOOTS => {
                self.rocket = true;
                self.rocket_start = Some(0.0);
            }
            behaviour::TREASURE_MAP => {
                self.map_time = 0.0;
                out.push(GangEvent::TreasureMap(true));
            }
            behaviour::SHADOW_POWER_2 => {
                self.invisible = 2;
                out.push(GangEvent::Invisible(2));
            }
            d if behaviour::DISGUISES.contains(&d) => {
                let k = behaviour::DISGUISES.iter().position(|&x| x == d).map(|i| i as u8);
                self.disguise = if self.disguise == k { None } else { k };
                out.push(GangEvent::Disguise(self.disguise));
            }
            _ => {}
        }
    }

    /// Be The Ball's random spin (gadgets.md 6.10): a horizontal vector at a random angle with a random length of
    /// 1.5 to 2.5, added to the spin.
    fn tumble(&mut self) {
        let mut next = || {
            self.ball_seed ^= self.ball_seed << 13;
            self.ball_seed ^= self.ball_seed >> 17;
            self.ball_seed ^= self.ball_seed << 5;
            (self.ball_seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let a = (next() * 2.0 - 1.0) * core::f32::consts::PI;
        let l = BALL_TUMBLE[0] + next() * (BALL_TUMBLE[1] - BALL_TUMBLE[0]);
        self.ball_spin = add(self.ball_spin, [l * cos(a), l * sin(a), 0.0]);
    }

    /// Sly 3's Grapple-Cam while out (gadgets.md 6.9, 6.10): in control (phases 4 to 6) Cross or Square within
    /// 0.1 s goes to phase 7 (the grab) and Triangle or Circle releases; phase 7 ends after its animation (taken as
    /// `activation_time` when no animation driver reports it).
    fn grapple_cam(&mut self, dt: f32, s: &Shared, out: &mut Vec<GangEvent>) {
        let Some((mut g, t)) = self.grapple else { return };
        let t = t + dt;
        let i = super::sly3::GrappleCamInput {
            playback_done: t >= s.tun.activation_time,
            continue_press: self.buffer.take(button::CROSS, 0.1) || self.buffer.take(button::SQUARE, 0.1),
            release_press: self.buffer.take(button::TRIANGLE, 0.1) || self.buffer.take(button::CIRCLE, 0.1),
            ..super::sly3::GrappleCamInput::default()
        };
        let phase = g.phase;
        let cue = g.step(&i);
        let t = if g.phase != phase { 0.0 } else { t };
        self.grapple = Some((g, t));
        match cue {
            Some(super::sly3::GrappleCamCue::Release) => out.push(GangEvent::GrappleCamEnded { grabbed: false }),
            Some(super::sly3::GrappleCamCue::Exit) => {
                self.grapple = None;
                out.push(GangEvent::GrappleCamEnded { grabbed: phase == 7 });
            }
            Some(super::sly3::GrappleCamCue::ReleaseAndExit) => {
                self.grapple = None;
                out.push(GangEvent::GrappleCamEnded { grabbed: false });
            }
            None => {}
        }
    }

    /// Cane reach (characters.md 9.1.1): a jump at a hook starts it when the hook is a fair target: within 1,000,
    /// Sly not more than `REACH_ABOVE` above it, reachable by a jump of 0.25 to 3 s, and the path clear. The fit
    /// score and the targets' enable flag and mask are not modelled [I].
    fn reach_target(&self, world: &GangWorld<'_>) -> Option<u32> {
        world
            .targets
            .iter()
            .filter(|t| matches!(t.kind, TargetKind::Hook))
            .filter(|t| {
                let d = sub(t.position, self.position);
                let h = length([d[0], d[1], 0.0]);
                if length(d) > swing::REACH || -d[2] > swing::REACH_ABOVE {
                    return false;
                }
                // Reachable by a jump of 0.25 to 3 s: the jump's vertical speed (215 up) and the 515 air steering.
                let t_flight = h / 515.0;
                (swing::JUMP_TIME[0]..=swing::JUMP_TIME[1]).contains(&t_flight.max(swing::JUMP_TIME[0]))
                    && !world.cw.segment_blocked(add(self.position, [0.0, 0.0, 100.0]), t.position, &world.filter)
            })
            .min_by(|a, b| length(sub(a.position, self.position)).total_cmp(&length(sub(b.position, self.position))))
            .map(|t| t.id)
    }

    /// The cane hook's tip (object 317) in the world, taken at Sly's hands [I: the tip object's pose is not read].
    fn cane_tip(&self) -> Vec3 {
        add(add(self.position, [0.0, 0.0, swing::TIP_HEIGHT]), scale(heading(self.yaw), swing::TIP_AHEAD))
    }

    /// In cane-reach mode, each step tests the body centre → cane tip segment and the tip's path over the step
    /// against the hook's point: within 15 catches it (kinds 0 to 2 swing; follow-hangs, kind 3, are not told apart
    /// yet).
    fn try_catch(&mut self, prev_tip: Vec3, world: &GangWorld<'_>, out: &mut Vec<GangEvent>) -> bool {
        let Some(id) = self.reach else { return false };
        let Some(h) = world.targets.iter().find(|t| t.id == id && matches!(t.kind, TargetKind::Hook)) else {
            return false;
        };
        let tip = self.cane_tip();
        let centre = add(self.position, [0.0, 0.0, 100.0]);
        let near = |a: Vec3, b: Vec3| {
            let ab = sub(b, a);
            let l2 = dot(ab, ab);
            let t = if l2 > 0.0 { (dot(sub(h.position, a), ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            length(sub(h.position, madd(a, ab, t)))
        };
        if near(centre, tip).min(near(prev_tip, tip)) > swing::CATCH {
            return false;
        }
        let r = sub(self.position, h.position);
        let len = length(r).max(1.0);
        // The swing keeps the body's motion: ω = r × v / |r|².
        self.swing_omega = scale(cross(r, self.velocity), 1.0 / (len * len));
        self.hook = Some((id, len));
        self.reach = None;
        self.swing_time = 0.0;
        self.pump = (0, 0.0, 0.0);
        self.set_state(sly::HOOK, out);
        true
    }

    /// The hook swing (state 3; characters.md 9.1.1): an angular velocity ω about the hook. Gravity, the stick's
    /// pumps (5·dt rad/s about the swing axis for the first 0.25 s of a pump, scaled by clamp(0.5 − 0.01·h, 0, 1), h
    /// the energy height; coasting after 1.4 s held), damping in the swing plane by clamp(1.5 − 0.35|ω|, −5, −0.25)
    /// and out of it by −2.5 while pumping, else −1.0; the rope eases to 230 and ω is rescaled by
    /// min(230, r) / max(75, new length). Cross after 0.25 s jumps off with the swing's velocity, at least 700 up;
    /// Square lets go (unconfirmed).
    fn swing(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let pivot = self.hook.and_then(|(id, _)| world.targets.iter().find(|t| t.id == id)).map(|t| t.position);
        let (Some(pivot), Some((id, len))) = (pivot, self.hook) else {
            self.leave_hook(out);
            return;
        };
        self.swing_time += dt;
        if self.buffer.take(button::SQUARE, spec::WINDOW_SHORT) {
            self.leave_hook(out);
            return;
        }
        if self.swing_time >= swing::RELEASE_AFTER && self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
            let r = sub(self.position, pivot);
            let mut v = cross(self.swing_omega, r);
            v[2] = v[2].max(swing::RELEASE_UP);
            self.leave_hook(out);
            self.velocity = v;
            out.push(GangEvent::Jumped);
            return;
        }
        let r = sub(self.position, pivot);
        let up = [0.0, 0.0, 1.0];
        let fwd = heading(self.yaw);
        let axis = cross(up, fwd); // left of the heading: the swing axis
        // Gravity's angular acceleration on a point mass: (r × g) / |r|².
        let g = [0.0, 0.0, spec::GRAVITY];
        let mut w = madd(self.swing_omega, cross(r, g), dt / (len * len));
        // The stick's pumps.
        let (dir, u) = self.stick(input, s);
        let stick = (u > swing::STICK).then(|| {
            let off = atan2(dir[1], dir[0]) - self.yaw;
            atan2(sin(off), cos(off))
        });
        self.swing_stick = stick;
        let want = match stick {
            Some(a) if a.abs() <= swing::AHEAD => 1i8,
            Some(a) if a.abs() >= swing::BACK => -1,
            _ => 0,
        };
        let (mut pdir, mut ptime, mut held) = self.pump;
        if want != pdir {
            (pdir, ptime, held) = (want, 0.0, 0.0);
        } else {
            ptime += dt;
            held += dt;
        }
        let coasting = held > swing::COAST_AFTER;
        let pumping = pdir != 0 && !coasting;
        if pumping && ptime < swing::PUMP_TIME {
            let v = cross(w, r);
            let h = (self.position[2] - (pivot[2] - len)) + dot(v, v) / (2.0 * -spec::GRAVITY);
            let k = (swing::PUMP_GAIN[0] - swing::PUMP_GAIN[1] * h).clamp(0.0, 1.0);
            // Ahead: ω about −axis (r below the hook turns toward the heading).
            w = madd(w, axis, -f32::from(pdir) * swing::PUMP_RATE * dt * k);
        }
        self.pump = (pdir, ptime, held);
        // Damping, in the swing plane (about the axis) and out of it.
        let w_in = dot(w, axis);
        let w_out = madd(w, axis, -w_in);
        let k_in = 1.0 + dt * (swing::DAMP_IN[0] - swing::DAMP_IN[1] * w_in.abs()).clamp(-5.0, -0.25);
        let k_out = 1.0 + dt * if pumping { swing::DAMP_OUT[0] } else { swing::DAMP_OUT[1] };
        w = add(scale(axis, w_in * k_in), scale(w_out, k_out));
        // The rope eases toward 230 (less the hook's offset, not read); ω rescaled for the new length.
        let new_len = len + (swing::ROPE - len) * (swing::ROPE_EASE * dt).min(1.0);
        w = scale(w, swing::ROPE.min(len) / swing::ROPE_MIN.max(new_len));
        // Turn r by ω·dt about the hook, at the new length.
        let angle = length(w) * dt;
        let r1 = if angle > 1e-7 {
            let m = crate::math::axis_angle(scale(w, dt));
            crate::math::mat_mul_vec(&m, r)
        } else {
            r
        };
        let r1 = scale(normalize(r1), new_len);
        self.position = add(pivot, r1);
        self.velocity = cross(w, r1);
        self.swing_omega = w;
        let hv = [self.velocity[0], self.velocity[1], 0.0];
        if length(hv) > 1.0 {
            self.yaw = atan2(hv[1], hv[0]);
        }
        let d = sub(pivot, self.position);
        self.swing_angle = atan2(dot(d, heading(self.yaw)), d[2]);
        self.hook = Some((id, new_len));
    }

    fn leave_hook(&mut self, out: &mut Vec<GangEvent>) {
        self.hook = None;
        self.swing_angle = 0.0;
        self.swing_stick = None;
        self.swing_omega = [0.0; 3];
        self.leave_ground();
        self.set_state(sly::AIR, out);
    }

    /// Sly 3's Fishing Pole line (gadgets.md 6.5): while the button is held the line stays out. A robbable guard
    /// within 250 of the tip (`fishing_range` ahead of Bentley) is hooked; still in reach 0.35 s later it is held
    /// and reeled in, taking a pocket item from it (`Pickpocket`, as Sly 2's pickpocketing [R]); while reeling, the
    /// line lets go past 400. Releasing the button ends the item.
    fn fish(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let Some(mut f) = self.fishing else { return };
        if !input.holds(f.button) {
            self.fishing = None;
            out.push(GangEvent::ItemEnded(behaviour::FISHING_POLE));
            return;
        }
        let tip = madd(self.position, heading(self.yaw), s.tun.fishing_range);
        let dist = |id: u32| {
            world.targets.iter().find(|g| g.id == id).map(|g| {
                let d = sub(g.position, tip);
                length([d[0], d[1], 0.0])
            })
        };
        f.time += dt;
        match f.hook {
            0 => {
                let near = world
                    .targets
                    .iter()
                    .filter(|g| matches!(g.kind, TargetKind::Guard { robbable: true, .. }))
                    .map(|g| (dist(g.id).unwrap_or(f32::MAX), g.id))
                    .filter(|&(d, _)| d <= Fishing::HOOK_REACH)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, id)) = near {
                    f = Fishing { hook: 1, target: Some(id), time: 0.0, ..f };
                }
            }
            1 => {
                let d = f.target.and_then(dist);
                if !d.is_some_and(|d| d <= Fishing::HOOK_REACH) {
                    f = Fishing { hook: 0, target: None, time: 0.0, ..f };
                } else if f.time >= Fishing::HOLD_TIME {
                    f = Fishing { hook: 2, time: 0.0, ..f };
                    if let Some(id) = f.target {
                        out.push(GangEvent::Pickpocket(id));
                    }
                }
            }
            _ => {
                if !f.target.and_then(dist).is_some_and(|d| d <= Fishing::LET_GO) {
                    f = Fishing { hook: 0, target: None, time: 0.0, ..f };
                }
            }
        }
        self.fishing = Some(f);
    }

    /// Sly 3's cane combo (characters.md 5): Triangle presses build the combo, then Square, Cross or Circle finish it
    /// with the spin, jump or push attack at the level of the Triangle count (1 to 3), if that upgrade is owned (the
    /// first spin level always is). Returns true when a press was used.
    fn cane_combo(&mut self, dt: f32, s: &Shared, out: &mut Vec<GangEvent>) -> bool {
        let (count, t) = self.cane;
        let t = t + dt;
        if count > 0 && t > s.tun.combo_window {
            self.cane = (0, 0.0);
            return false;
        }
        self.cane = (count, t);
        if self.buffer.take(button::TRIANGLE, spec::WINDOW_SHORT) {
            self.cane = ((count + 1).min(3), 0.0);
            self.combo_press(button::TRIANGLE, s, out);
            return true;
        }
        if count == 0 {
            return false;
        }
        for (kind, b) in [(0u8, button::SQUARE), (1, button::CROSS), (2, button::CIRCLE)] {
            if self.buffer.take(b, spec::WINDOW_SHORT) {
                // The upgrades are items 41 to 49: spin 41 to 43, jump 44 to 46, push 47 to 49.
                let item = 41 + kind * 3 + (count - 1);
                let owned = s.owned.has(item) || item == 41;
                if owned {
                    out.push(GangEvent::ComboFinisher { kind, level: count });
                    self.combo_press(b, s, out);
                }
                self.cane = (0, 0.0);
                return true;
            }
        }
        false
    }

    /// Drives the combo runner with a press of `b` (Sly 3 with combo definitions): the definition's clip and the
    /// combo state.
    fn combo_press(&mut self, b: u32, s: &Shared, out: &mut Vec<GangEvent>) {
        if self.combo_defs.is_empty() {
            return;
        }
        let moving = self.last_deflection > 0.0;
        let cue = self.combo_runner.press(&self.combo_defs, b, moving, |n| s.owned.has(n));
        self.combo_cue(cue, out);
    }

    fn combo_cue(&mut self, cue: Option<super::combo::Cue>, out: &mut Vec<GangEvent>) {
        match cue {
            Some(super::combo::Cue::Clip { def, clip }) => {
                let oid = self.combo_defs.defs.get(def).map_or(-1, |d| d.oid);
                if self.state != sly::COMBO {
                    self.set_state(sly::COMBO, out);
                }
                out.push(GangEvent::ComboClip { def: oid, clip });
            }
            Some(super::combo::Cue::Ended) => {
                out.push(GangEvent::ComboEnded);
                self.set_state(sly::FIGHT, out);
            }
            None => {}
        }
    }

    /// Sly 3: the combo attack hit something; plays the definition's follow-up clip, if any.
    pub fn combo_hit(&mut self, out: &mut Vec<GangEvent>) {
        let cue = self.combo_runner.hit(&self.combo_defs);
        self.combo_cue(cue, out);
    }

    /// Sly 3's combo state (state 23): the runner's clip times, presses continuing the combo, and the moving overlay
    /// (the stick moves Sly at his run speed).
    fn combo_state(&mut self, dt: f32, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let cue = self.combo_runner.update(&self.combo_defs, dt, s.tun.combo_clip_time);
        self.cane_combo(dt, s, out);
        self.combo_cue(cue, out);
        let v = if self.combo_runner.phase == super::combo::Phase::Moving {
            scale(self.last_stick, gait::SLY_RUN.speed(self.last_deflection))
        } else {
            [0.0; 3]
        };
        self.move_body(dt, world, s, v, true);
    }

    /// Takes a medicine pickup (10 health, 20 power); refused when both are full.
    pub fn take_medicine(&mut self) -> bool {
        super::gadgets::take_medicine(&mut self.health, &mut self.power)
    }

    // ---- throw aim (shared, characters.md 7) ---------------------------------------------------------------

    fn throw_aim(
        &mut self,
        dt: f32,
        input: &GangInput,
        world: &GangWorld<'_>,
        s: &Shared,
        back: u8,
        out: &mut Vec<GangEvent>,
    ) {
        let Some(mut aim) = self.aim else {
            self.set_state(back, out);
            return;
        };
        aim.steer(input.stick, s.tun.aim_speed, dt);
        // The throw marker follows the predicted flight (characters.md 9.3.1).
        self.aim_path =
            aim.predict(add(self.position, [0.0, 0.0, 100.0]), spec::GRAVITY, Some(world.cw), &world.filter).points;
        self.yaw = turn_toward(self.yaw, aim.yaw, spec::THROW_TURN[1], dt);
        self.aim = Some(aim);
        self.move_body(dt, world, s, [0.0; 3], true);
        let release = self.buffer.take(button::SQUARE, 0.1) || self.buffer.take(button::CROSS, 0.1);
        let cancel = self.buffer.take(button::CIRCLE, 0.1) || self.buffer.take(button::TRIANGLE, 0.1);
        if release {
            let from = add(self.position, [0.0, 0.0, 100.0]);
            let f = aim.predict(from, spec::GRAVITY, Some(world.cw), &world.filter);
            if let Some(what) = self.carrying.take() {
                out.push(GangEvent::Thrown { id: what, velocity: f.velocity });
                if self.who == Who::Bentley && what <= 255 {
                    self.gadgets_out.push(what as u8);
                }
                // Pocket items pay when thrown.
                if what <= 255 && effect::pay_at(what as u8) == PayAt::Throw {
                    let cost = s.items.actual(what as u8).and_then(|n| s.items.get(n)).map_or(0.0, |r| r.cost as f32);
                    self.power.pay(cost);
                    out.push(GangEvent::ItemActivated(what as u8));
                }
                if self.who == Who::Murray && s.has(17) {
                    out.push(GangEvent::Explosive(what));
                }
            }
            self.aim = None;
            self.set_state(0, out);
        } else if cancel {
            self.aim = None;
            out.push(GangEvent::ThrowCancelled);
            let st = if self.who == Who::Murray { murray::CARRY } else { 0 };
            self.set_state(st, out);
        }
    }

    fn start_aim(&mut self, s: &Shared, state: u8, out: &mut Vec<GangEvent>) {
        let long = s.has(id::LONG_TOSS);
        self.aim = Some(ThrowAim::new(s.tun.throw_range, s.tun.throw_time, self.yaw, long));
        self.set_state(state, out);
    }

    // ---- Sly (characters.md 3) -----------------------------------------------------------------------------

    fn sly(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let t = &s.tun;
        let reach = t.reach;
        let guard_near =
            self.nearest(world, reach, core::f32::consts::PI, |g| matches!(g.kind, TargetKind::Guard { .. }));
        if self.state == sly::COMBO {
            self.combo_state(dt, world, s, out);
            return;
        }
        if self.on_ground_choices() {
            // Sly 3: the cane combo takes its presses first.
            if s.items.is_sly3() && self.cane_combo(dt, s, out) {
                self.move_body(dt, world, s, [0.0; 3], true);
                return;
            }
            // First match wins.
            if self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                self.jump(s, 1, out);
                return;
            }
            if guard_near.is_some() && !input.holds(button::R1) && self.state != sly::FIGHT {
                self.set_state(sly::FIGHT, out);
            } else if self.buffer.take(button::R3, spec::WINDOW_SHORT) {
                out.push(GangEvent::Binocucom);
            } else if let Some(g) = self.robbable_from_behind(world, reach)
                && self.buffer.take(button::CIRCLE, spec::WINDOW_SHORT)
            {
                out.push(GangEvent::Pickpocket(g.id));
                self.set_state(sly::PICKPOCKET, out);
                return;
            } else if self.buffer.take(button::CIRCLE, spec::WINDOW_INTERACT)
                && let Some(i) =
                    self.nearest(world, reach, core::f32::consts::PI, |g| g.kind == TargetKind::Interactable)
            {
                out.push(GangEvent::Interact(i.id));
                self.set_state(sly::INTERACT, out);
                return;
            }
            if self.state == sly::FIGHT && (guard_near.is_none() || input.holds(button::R1)) {
                self.set_state(sly::STAND, out);
            }
            if self.state != sly::FIGHT {
                let (_, u) = self.stick(input, s);
                let st = if u <= 0.1 {
                    sly::STAND
                } else if input.holds(button::R1) {
                    sly::RUN
                } else if guard_near.is_some() {
                    sly::SNEAK
                } else {
                    sly::SNEAK_RUN
                };
                self.set_state(st, out);
            }
            // The gaits (characters.md 1.1): the run at weight 1 × 1.3, the move by the stick (× 0.75 with Shadow
            // Power).
            let u = self.deflection(input, s);
            let speed = match self.state {
                sly::RUN => gait::SLY_RUN.speed(1.0) * s.consts.run_factor,
                sly::FIGHT => 0.0,
                _ if self.active.is_some_and(|a| a.0 == id::SHADOW_POWER) => {
                    gait::SLY_MOVE.speed(u * s.consts.shadow_speed)
                }
                _ => gait::SLY_MOVE.speed(u),
            };
            let speed = if self.rocket { speed * t.rocket_factor } else { speed };
            // Rocket Boots' start-up (sub-state 6): Sly turns in place for at least 0.07 s and until he faces the
            // stick within 0.1 rad; then the run (sub-state 7).
            let speed = match self.rocket_start {
                Some(st) if self.rocket => {
                    let st = st + dt;
                    let (dir, u) = self.stick(input, s);
                    let off = atan2(dir[1], dir[0]) - self.yaw;
                    let aligned = u > 0.0 && atan2(sin(off), cos(off)).abs() <= ROCKET_ALIGN;
                    self.rocket_start = if st >= ROCKET_START && aligned { None } else { Some(st) };
                    0.0
                }
                _ => speed,
            };
            let moved = self.walk(dt, input, world, s, speed);
            // The treasure map: with the stick forward, the heading snaps to n directions (gadgets.md 6.3).
            if self.active.is_some_and(|a| a.0 == behaviour::TREASURE_MAP) && input.stick[1] > 0.0 {
                let w = input.world_stick();
                let step = core::f32::consts::TAU / MAP_DIRECTIONS as f32;
                let q = atan2(w[1], w[0]) / step;
                self.yaw = (q + 0.5 - crate::math::rem_euclid(q + 0.5, 1.0)) * step;
            }
            // Rocket Boots: a near head-on wall stops the run (gadgets.md 6.2); Cross within 0.1 s (a jump out) or
            // Circle within 0.2 s (an attack) leaves it (6.9).
            let leave = self.rocket
                && self.rocket_start.is_none()
                && (self.buffer.peek(button::CROSS, 0.1) || self.buffer.peek(button::CIRCLE, 0.2));
            if self.rocket
                && self.rocket_start.is_none()
                && (leave || self.wall.is_some_and(|n| dot(heading(self.yaw), n) < ROCKET_WALL))
            {
                self.rocket = false;
                if self.active.is_some_and(|a| a.0 == behaviour::ROCKET_BOOTS) {
                    self.active = None;
                }
                out.push(GangEvent::ItemEnded(behaviour::ROCKET_BOOTS));
            }
            if !moved && !self.grounded {
                self.set_state(sly::AIR, out);
            }
            return;
        }
        match self.state {
            sly::AIR | sly::AIR_SQUARE | sly::AIR_TRIANGLE => {
                // Cane reach: a jump aimed at a hook catches it when the cane tip passes within 15 of it.
                if self.state == sly::AIR {
                    if self.reach.is_none() && self.state_time <= dt + 1e-6 {
                        self.reach = self.reach_target(world);
                    }
                    let prev_tip = if self.state_time <= dt + 1e-6 { self.cane_tip() } else { self.last_tip };
                    self.last_tip = self.cane_tip();
                    if self.reach.is_some() && self.try_catch(prev_tip, world, out) {
                        return;
                    }
                }
                if self.state == sly::AIR {
                    if self.buffer.take(button::SQUARE, spec::WINDOW_SHORT) {
                        out.push(GangEvent::AirAttack(sly::AIR_SQUARE));
                        self.set_state(sly::AIR_SQUARE, out);
                    } else if self.buffer.take(button::TRIANGLE, spec::WINDOW_SHORT) {
                        out.push(GangEvent::AirAttack(sly::AIR_TRIANGLE));
                        self.set_state(sly::AIR_TRIANGLE, out);
                    }
                } else if self.state_time >= clip::SLY_AIR_ATTACK[usize::from(self.state == sly::AIR_TRIANGLE)] {
                    self.set_state(sly::AIR, out);
                }
                if self.gliding {
                    if self.glide(dt, input, world, s, out) {
                        self.set_state(sly::STAND, out);
                    }
                    return;
                }
                let steer = s.consts.air_steer * self.steer_factor;
                // Sly's double jump (kind 6): Cross in the air, to a peak 315 above the first jump's floor.
                if self.state == sly::AIR
                    && !self.gliding
                    && !self.double_used
                    && self.buffer.take(button::CROSS, spec::WINDOW_JUMP)
                {
                    let z = (self.position[2] - self.floor_z).max(0.0);
                    self.velocity[2] = jump_speed(s.consts.jump_heights[1], z);
                    self.double_used = true;
                    self.jump_kind = 6;
                    out.push(GangEvent::DoubleJump);
                }
                let landed = self.air(dt, input, world, s, steer, out);
                if landed {
                    self.set_state(sly::STAND, out);
                }
            }
            sly::PICKPOCKET | sly::INTERACT => {
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.state_time >= clip::SLY_PICKPOCKET {
                    self.set_state(sly::STAND, out);
                }
            }
            sly::SLIDE => {
                let v =
                    scale(heading(self.yaw), (length([self.velocity[0], self.velocity[1], 0.0]) - 200.0 * dt).max(0.0));
                self.move_body(dt, world, s, v, true);
                if self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                    self.active = None;
                    out.push(GangEvent::ItemEnded(id::STEALTH_SLIDE));
                    self.jump(s, 1, out);
                } else if self.active.is_none() || length(v) < spec::SLIDE_MIN_SPEED {
                    if self.active.take().is_some() {
                        out.push(GangEvent::ItemEnded(id::STEALTH_SLIDE));
                    }
                    self.set_state(sly::STAND, out);
                }
            }
            sly::HOOK => self.swing(dt, input, world, s, out),
            sly::THROW_AIM => self.throw_aim(dt, input, world, s, sly::STAND, out),
            sly::DODGE => {
                // Combat Dodge (gadgets.md 5.1): the clip's 1,500 sideways; around a target Sly circles it, the
                // horizontal distance eased toward 250 with (500, 0, 0.5), the radial correction at most 1,500/s.
                // It ends with the clip and the velocity is halved.
                let speed = gait::SLY_DODGE.speed(1.0);
                // The target is chosen again every frame (gadgets.md 5.4).
                let target = self.dodge_target(world);
                self.move_target = target.map(|g| g.id);
                let v = match target {
                    Some(g) => {
                        // Around the target: 1,500 along the circle, the radius eased toward 250.
                        let d = sub(self.position, g.position);
                        let dist = length([d[0], d[1], 0.0]).max(1e-3);
                        let [f, sl, ft] = s.consts.dodge_ease;
                        let p =
                            crate::physics::SmoothParams { fast: f, slow: sl, fast_time: ft, max_speed_change: None };
                        let radial_speed = (p.speed_at((dist - 250.0).abs())).min(speed) * dt;
                        let r1 = if dist > 250.0 {
                            (dist - radial_speed).max(250.0)
                        } else {
                            (dist + radial_speed).min(250.0)
                        };
                        let a0 = atan2(d[1], d[0]);
                        let a1 = a0 + self.dodge_side * speed * dt / dist;
                        let next = [g.position[0] + r1 * cos(a1), g.position[1] + r1 * sin(a1), self.position[2]];
                        self.yaw = a1 + core::f32::consts::PI;
                        scale(sub(next, self.position), 1.0 / dt)
                    }
                    None => {
                        let d = if self.last_deflection > 0.0 {
                            self.last_stick
                        } else {
                            [-sin(self.yaw), cos(self.yaw), 0.0]
                        };
                        scale(d, speed)
                    }
                };
                self.move_body(dt, world, s, v, true);
                if self.moving.is_none() {
                    self.velocity = scale(self.velocity, s.consts.dodge_end);
                    self.set_state(sly::FIGHT, out);
                }
            }
            sly::KNOCKOUT_DIVE => {
                // A rush at the target with clip 189's root speed; it ends with the clip.
                if let Some(g) = self.move_target.and_then(|id| world.targets.iter().find(|g| g.id == id)) {
                    let d = sub(g.position, self.position);
                    self.yaw = atan2(d[1], d[0]);
                }
                let v = scale(heading(self.yaw), gait::KNOCKOUT_DIVE.speed(1.0));
                self.move_body(dt, world, s, v, true);
                if self.moving.is_none() {
                    self.set_state(sly::STAND, out);
                }
            }
            sly::JUGGLE_STRIKE => {
                // The juggle strike (gadgets.md 5.2). With Silent Obliteration Sly homes in: he faces the target and
                // the distance eases toward 200 with (1000, 0, 0.5) at half the frame step. Without it the clip's root
                // motion moves him (the caller's).
                if s.has(28)
                    && let Some(g) = self.move_target.and_then(|id| world.targets.iter().find(|g| g.id == id))
                {
                    let d = sub(g.position, self.position);
                    let dist = length([d[0], d[1], 0.0]);
                    self.yaw = atan2(d[1], d[0]);
                    let [goal, f, sl, ft] = s.consts.juggle_home;
                    let p = crate::physics::SmoothParams { fast: f, slow: sl, fast_time: ft, max_speed_change: None };
                    let mut sm = crate::physics::Smoother::default();
                    let next = sm.step(&p, dist, goal, dt * 0.5);
                    let v = scale(heading(self.yaw), (dist - next) / dt);
                    self.move_body(dt, world, s, v, true);
                } else {
                    self.move_body(dt, world, s, [0.0; 3], true);
                }
                if self.state_time >= spec::JUGGLE_TIME[usize::from(s.has(28))] {
                    self.move_target = None;
                    self.set_state(sly::STAND, out);
                }
            }
            sly::DAMAGE => self.recover(dt, input, world, s, sly::STAND, out),
            _ => {
                self.move_body(dt, world, s, [0.0; 3], true);
            }
        }
    }

    /// A robbable guard within reach that Sly stands behind.
    fn robbable_from_behind(&self, world: &GangWorld<'_>, reach: f32) -> Option<Target> {
        self.nearest(world, reach, core::f32::consts::PI, |g| {
            matches!(g.kind, TargetKind::Guard { robbable: true, .. })
                && dot(heading(g.yaw), sub(self.position, g.position)) < 0.0
        })
    }

    /// Getting up after a reaction: stand if the counter is above 0 (characters.md 2).
    fn recover(
        &mut self,
        dt: f32,
        input: &GangInput,
        world: &GangWorld<'_>,
        s: &Shared,
        stand: u8,
        out: &mut Vec<GangEvent>,
    ) {
        let _ = input;
        let h = [self.velocity[0], self.velocity[1], 0.0];
        let landed = self.move_body(dt, world, s, h, true);
        if landed {
            self.velocity[0] *= 0.5;
            self.velocity[1] *= 0.5;
        }
        if self.grounded && self.state_time >= s.tun.hurt_time && self.health.value > 0.0 {
            self.velocity = [0.0; 3];
            if self.held_reaction == 1 {
                // A pop-up hit: sound 0x6A and a second, held reaction before getting up (characters.md 2.2).
                self.held_reaction = 2;
                self.state_time = 0.0;
                out.push(GangEvent::HeldReaction);
                return;
            }
            self.held_reaction = 0;
            self.set_state(stand, out);
        }
    }

    // ---- Murray (characters.md 4) --------------------------------------------------------------------------

    fn start_flop(&mut self, out: &mut Vec<GangEvent>) {
        self.flop_phase = 0;
        out.push(GangEvent::FlopStarted);
        self.set_state(murray::FLOP, out);
    }

    fn punch(&mut self, index: u8, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let target = self.nearest(world, s.tun.reach, 1.0, Target::attackable).map(|t| t.id);
        self.combo = Combo { index, queued: None, count: self.combo.count + 1 };
        out.push(GangEvent::Punch { index, target });
        self.state_time = 0.0;
        self.set_state(murray::PUNCH, out);
    }

    /// The first punch of a pair: by which side the target is on (left: the first clip).
    fn first_of_pair(&self, world: &GangWorld<'_>, s: &Shared, base: u8) -> u8 {
        match self.nearest(world, s.tun.reach, 1.0, Target::attackable) {
            Some(t) => {
                let d = sub(t.position, self.position);
                let left = [-sin(self.yaw), cos(self.yaw), 0.0];
                if dot(d, left) >= 0.0 { base } else { base + 1 }
            }
            None => base,
        }
    }

    fn murray(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let t = &s.tun;
        let c = &s.consts;
        // Grabbing or juggling a launched guard above him.
        if matches!(self.state, murray::STAND | murray::RUN | murray::WALK | murray::STANCE | murray::PUNCH) {
            for g in world.targets {
                if !matches!(g.kind, TargetKind::Guard { launched: true, .. }) {
                    continue;
                }
                let d = sub(g.position, self.position);
                let hd = length([d[0], d[1], 0.0]);
                let [gr, glo, ghi] = spec::GRAB_LAUNCHED;
                let [jr, jlo, jhi] = spec::JUGGLE;
                if hd < gr && d[2] >= glo && d[2] <= ghi && self.buffer.take(button::CIRCLE, spec::WINDOW_SHORT) {
                    self.carrying = Some(g.id);
                    out.push(GangEvent::GrabbedLaunched(g.id));
                    self.set_state(murray::CARRY, out);
                    return;
                }
                if hd < jr && d[2] >= jlo && d[2] <= jhi && self.buffer.take(button::SQUARE, spec::WINDOW_SHORT) {
                    self.punch(4, world, s, out);
                    return;
                }
            }
        }
        // Sly 3's Be The Ball (gadgets.md 6.4) on the ground: Square kicks the ball along his heading (56 × the
        // stick's forward part), Cross jumps to 315 (within 0.25 s, at least 0.15 s after landing); the roll eases
        // toward the stick at `ball[0]` (the rolling speed is not given).
        if self.rolling && self.grounded && self.on_ground_choices() {
            if self.since_landed >= 0.15 && self.buffer.take(button::CROSS, 0.25) {
                self.velocity[2] = crate::math::sqrt(2.0 * -spec::GRAVITY * BALL_JUMP);
                self.leave_ground();
                self.set_state(murray::AIR, out);
                return;
            }
            let (dir, u) = self.stick(input, s);
            if u > 0.0 {
                self.yaw = turn_toward(self.yaw, atan2(dir[1], dir[0]), t.turn_rate, dt);
            }
            // Square: the spin moves by at most 4 toward 14 × the stick's magnitude about the sideways axis.
            if self.buffer.take(button::SQUARE, spec::WINDOW_SHORT) {
                let h = heading(self.yaw);
                let target = scale(
                    [-h[1], h[0], 0.0],
                    BALL_SPIN * crate::math::length([input.stick[0], input.stick[1], 0.0]).min(1.0),
                );
                let mut d = sub(target, self.ball_spin);
                let l = length(d);
                if l > BALL_SPIN_STEP {
                    d = scale(d, BALL_SPIN_STEP / l);
                }
                self.ball_spin = add(self.ball_spin, d);
            }
            // Rolling without slipping on the 75-unit sphere: v = R · (ω × up).
            let w = self.ball_spin;
            let v = scale([w[1], -w[0], 0.0], BALL_RADIUS);
            self.move_body(dt, world, s, v, true);
            return;
        }
        if self.on_ground_choices() {
            let ahead = self.nearest(world, t.reach, 0.5, |g| {
                matches!(
                    g.kind,
                    TargetKind::Guard { .. } | TargetKind::Object { .. } | TargetKind::Character(Who::Bentley)
                )
            });
            if !input.holds(button::R1) && ahead.is_some() && self.state != murray::STANCE {
                self.set_state(murray::STANCE, out);
            } else if self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                self.jump(s, 1, out);
                return;
            } else if self.buffer.take(button::R3, spec::WINDOW_SHORT) {
                out.push(GangEvent::Binocucom);
            } else if self.buffer.take(button::TRIANGLE, spec::WINDOW_SHORT) {
                let i = self.first_of_pair(world, s, 2);
                self.combo.count = 0;
                self.punch(i, world, s, out);
                return;
            } else if self.buffer.take(button::CIRCLE, spec::WINDOW_INTERACT) {
                if let Some(i) =
                    self.nearest(world, t.reach, core::f32::consts::PI, |g| g.kind == TargetKind::Interactable)
                {
                    out.push(GangEvent::Interact(i.id));
                    self.set_state(murray::INTERACT, out);
                    return;
                }
                if let Some(a) = self.nearest(world, t.reach, 0.5, |g| {
                    matches!(
                        g.kind,
                        TargetKind::Guard { .. }
                            | TargetKind::Object { liftable: true, .. }
                            | TargetKind::Character(Who::Bentley)
                    )
                }) {
                    self.carrying = Some(a.id);
                    out.push(GangEvent::Lift(a.id));
                    self.set_state(murray::LIFT, out);
                    return;
                }
            } else if self.buffer.take(button::SQUARE, spec::WINDOW_SHORT) {
                let i = self.first_of_pair(world, s, 0);
                self.combo.count = 0;
                self.punch(i, world, s, out);
                return;
            }
            if self.state == murray::STANCE && (ahead.is_none() || input.holds(button::R1)) {
                self.set_state(murray::STAND, out);
            }
            if self.state != murray::STANCE {
                let (_, u) = self.stick(input, s);
                let st = if u == 0.0 {
                    murray::STAND
                } else if input.holds(button::R1) {
                    murray::RUN
                } else {
                    murray::WALK
                };
                self.set_state(st, out);
            }
            let u = self.deflection(input, s);
            let speed =
                if self.state == murray::RUN { gait::MURRAY_RUN.speed(1.0) } else { gait::MURRAY_WALK.speed(u) };
            if !self.walk(dt, input, world, s, speed) && !self.grounded {
                self.set_state(murray::AIR, out);
            }
            return;
        }
        match self.state {
            murray::AIR => {
                let flop_press = self.buffer.peek(button::SQUARE, c.flop_window[0])
                    || self.buffer.peek(button::TRIANGLE, c.flop_window[0]);
                let kind_ok = !matches!(self.jump_kind, 0 | 11 | 15 | 16);
                if flop_press && kind_ok && !self.lands_within(world, s, c.flop_window[1]) {
                    self.buffer.take(button::SQUARE, c.flop_window[0]);
                    self.buffer.take(button::TRIANGLE, c.flop_window[0]);
                    self.start_flop(out);
                    return;
                }
                if self.air(dt, input, world, s, s.consts.air_steer, out) {
                    self.set_state(murray::STAND, out);
                }
            }
            murray::FLOP => self.flop(dt, input, world, s, out),
            murray::PUNCH => {
                // Queue the next punch of the pair within 0.25 s.
                let pair = self.combo.index / 2;
                let button_of_pair = if pair == 0 { button::SQUARE } else { button::TRIANGLE };
                if self.combo.queued.is_none()
                    && self.combo.index < 4
                    && self.buffer.take(button_of_pair, spec::PUNCH_QUEUE)
                {
                    self.combo.queued = Some(pair * 2 + (1 - self.combo.index % 2));
                }
                let (dir, u) = self.stick(input, s);
                // The stick steers the punch with blend 0.5 + 0.5 × deflection (the punch's own travel is the
                // clip's root motion, applied by the caller); after the "can steer" point (half the punch),
                // moving the stick ends the combo.
                let punch_time = clip::MURRAY_PUNCHES[usize::from(self.combo.index).min(4)];
                let can_steer = self.state_time >= punch_time * 0.5;
                if u > 0.0 {
                    let g = (0.5 + 0.5 * u).clamp(0.0, 1.0);
                    self.yaw = turn_toward(self.yaw, atan2(dir[1], dir[0]), s.tun.turn_rate * g, dt);
                }
                self.move_body(dt, world, s, [0.0; 3], true);
                if can_steer && u > 0.0 && self.combo.queued.is_none() {
                    self.combo = Combo::default();
                    self.set_state(murray::WALK, out);
                } else if self.state_time >= punch_time {
                    match self.combo.queued.take() {
                        Some(next) if self.combo.count < 5 => self.punch(next, world, s, out),
                        _ => {
                            self.combo = Combo::default();
                            self.set_state(murray::STAND, out);
                        }
                    }
                }
            }
            murray::LIFT => {
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.state_time >= clip::MURRAY_PICK_UP[0] {
                    self.set_state(murray::CARRY, out);
                }
            }
            murray::CARRY => {
                // Atlas Strength (18): while carrying, Cross jumps (jump kind 15) and R1 runs.
                let atlas = s.has(18);
                if atlas && self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                    self.velocity[2] = self.velocity[2].max(self.jump_kind_speed(&s.consts, 15));
                    self.jump_kind = 15;
                    self.leave_ground();
                    out.push(GangEvent::SpecialJump(15));
                    return;
                }
                if self.buffer.take(button::CIRCLE, 0.1) || self.buffer.take(button::CROSS, 0.1) {
                    self.start_aim(s, murray::THROW_AIM, out);
                    return;
                }
                let u = self.deflection(input, s);
                let speed = if atlas && input.holds(button::R1) {
                    gait::MURRAY_RUN.speed(1.0)
                } else {
                    gait::MURRAY_CARRY.speed(u)
                };
                if !self.grounded {
                    self.air(dt, input, world, s, s.consts.air_steer, out);
                } else {
                    self.walk(dt, input, world, s, speed);
                }
            }
            murray::THROW_AIM => self.throw_aim(dt, input, world, s, murray::STAND, out),
            murray::HELD => {
                let any = input.pressed & (button::CROSS | button::CIRCLE | button::SQUARE | button::TRIANGLE) != 0;
                if any {
                    self.hold_idle = 0.0;
                } else {
                    self.hold_idle += dt;
                    if self.hold_idle > c.hold_drop {
                        self.hold_idle = 0.0;
                        self.hold_level = self.hold_level.saturating_sub(1);
                        if self.hold_level == 0 {
                            self.set_state(murray::STAND, out);
                        }
                    }
                }
            }
            murray::TURNBUCKLE => {
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                    self.jump(s, 18, out);
                } else if !self.slots.0.iter().zip(button::SLOTS).any(|(i, b)| *i == Some(16) && input.holds(b)) {
                    self.set_state(murray::STAND, out);
                }
            }
            murray::CHARGE => {
                // Berserker Charge (gadgets.md 5.1): clip 441's 700/s for 0.66 s, then the stop clip 443 (250/s for
                // 1 s). Hitting a wall ends it.
                let [charge, stop] = gait::CHARGE;
                let t = self.state_time;
                let speed = if t < gait::CHARGE_TIME { charge.speed(1.0) } else { stop.speed(1.0) };
                let v = scale(heading(self.yaw), speed);
                self.move_body(dt, world, s, v, true);
                let blocked = t < gait::CHARGE_TIME && length([self.velocity[0], self.velocity[1], 0.0]) < 1.0;
                if blocked || t >= gait::CHARGE_TIME + stop.slow[1] {
                    self.set_state(murray::STAND, out);
                }
            }
            murray::SLAM => {
                // Diablo Fire Slam: the slam clip, standing still.
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.moving.is_none() {
                    self.set_state(murray::STAND, out);
                }
            }
            murray::DAMAGE => self.recover(dt, input, world, s, murray::STAND, out),
            _ => {
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.state_time > 0.5 && self.state == murray::INTERACT {
                    self.set_state(murray::STAND, out);
                }
            }
        }
    }

    /// The Thunder Flop (characters.md 4.5): wind-up with gravity suspended and no horizontal speed, then a fall at
    /// −500 steered toward 0.75 × 515 × stick with gain 2.5, then the landing.
    fn flop(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let c = &s.consts;
        match self.flop_phase {
            0 => {
                self.velocity = [0.0; 3];
                if self.state_time >= s.tun.flop_windup {
                    self.flop_phase = 1;
                }
            }
            _ => {
                let (dir, u) = self.stick(input, s);
                let target = scale(dir, c.flop_steer[0] * c.flop_steer[1] * u);
                let h = [self.velocity[0], self.velocity[1], 0.0];
                let h = madd(h, sub(target, h), (c.flop_gain * dt).min(1.0));
                self.velocity = [h[0], h[1], c.flop_fall];
                let shape = self.shape(s);
                let delta = scale(self.velocity, dt);
                let r = move_and_slide(world.cw, &world.filter, shape, self.origin(s), delta, &SlideParams::default());
                self.position = sub(r.position, [0.0, 0.0, s.tun.radius[self.idx()]]);
                if r.ground.is_some() {
                    self.flop_phase = 0;
                    self.velocity = [0.0; 3];
                    self.grounded = true;
                    let inferno = s.has(id::INFERNO_FLOP);
                    out.push(GangEvent::FlopLanded { position: self.position, inferno });
                    self.set_state(murray::STAND, out);
                }
            }
        }
    }

    // ---- Bentley (characters.md 5) -------------------------------------------------------------------------

    fn bentley(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>, s: &Shared, out: &mut Vec<GangEvent>) {
        let t = &s.tun;
        let c = &s.consts;
        // Sly 3: while the Grapple-Cam is out the buttons are the camera's.
        if self.grapple.is_some() {
            self.move_body(dt, world, s, [0.0; 3], true);
            return;
        }
        // Crossbow swing (Square within 0.1 s).
        // Sly 3's Bentley is in his wheelchair: no crossbow swing.
        if !s.items.is_sly3()
            && !matches!(self.state, bentley::CARRIED | bentley::DAMAGE | bentley::THROW_AIM | bentley::HOLDING)
            && self.buffer.take(button::SQUARE, 0.1)
        {
            self.crossbow(world, input, s, out);
        }
        // Darts (R1 within 0.25 s) once reloaded.
        if self.dart_reload <= 0.0 && self.buffer.take(button::R1, spec::WINDOW_DART) {
            let from = madd(input.camera_eye, input.camera_forward, c.dart_depth[0]);
            let far = madd(input.camera_eye, input.camera_forward, c.dart_depth[1]);
            let d = sub(far, from);
            let len = length(d);
            let (to, hit) = match world.cw.ray_cast(from, d, len, &world.filter) {
                Some(h) => (h.point, true),
                None => (far, false),
            };
            self.darts.push((to, 0.0));
            self.dart_reload = spec::DART[1];
            out.push(GangEvent::Dart { from, to, hit });
        }
        if self.on_ground_choices() {
            if self.buffer.take(button::CROSS, spec::WINDOW_JUMP) {
                self.jump(s, 1, out);
                return;
            }
            if self.buffer.take(button::R3, spec::WINDOW_SHORT) {
                out.push(GangEvent::Binocucom);
            } else if self.buffer.take(button::CIRCLE, spec::WINDOW_INTERACT)
                && let Some(i) =
                    self.nearest(world, t.reach, core::f32::consts::PI, |g| g.kind == TargetKind::Interactable)
            {
                out.push(GangEvent::Interact(i.id));
                self.set_state(bentley::INTERACT, out);
                return;
            }
            let (_, u) = self.stick(input, s);
            let st = if u == 0.0 {
                bentley::STAND
            } else if input.holds(button::R1) {
                bentley::MOVE
            } else {
                bentley::WALK
            };
            self.set_state(st, out);
            let u = self.deflection(input, s);
            let speed =
                if self.state == bentley::MOVE { gait::BENTLEY_RUN.speed(1.0) } else { gait::BENTLEY_WALK.speed(u) };
            if !self.walk(dt, input, world, s, speed) && !self.grounded {
                self.set_state(bentley::AIR, out);
            }
            return;
        }
        match self.state {
            bentley::AIR if self.tossed => {
                // Thrown by Murray: ballistic until the landing, then Bentley becomes the player.
                let h = [self.velocity[0], self.velocity[1], 0.0];
                if self.move_body(dt, world, s, h, true) {
                    self.tossed = false;
                    self.velocity = [0.0; 3];
                    out.push(GangEvent::Landed);
                    out.push(GangEvent::BecamePlayer(Who::Bentley));
                    self.set_state(bentley::STAND, out);
                }
            }
            bentley::AIR => {
                // Sly 3: the Hover Pack is passive: Cross while jumping deploys it (gadgets.md 1, string 294).
                if s.items.is_sly3()
                    && self.hover.is_none()
                    && !self.tossed
                    && s.has(id::HOVER_PACK)
                    && self.buffer.take(button::CROSS, spec::WINDOW_JUMP)
                {
                    self.hover = Some(0.0);
                    self.active = Some((id::HOVER_PACK, button::CROSS));
                    out.push(GangEvent::SpecialJump(17));
                }
                if let Some(ht) = self.hover {
                    // Hover Pack: 300 × stick horizontally, −100 vertically, facing the motion above 200.
                    let ht = ht + dt;
                    self.hover = Some(ht);
                    let (dir, u) = self.stick(input, s);
                    let h = scale(dir, c.hover[0] * u);
                    if length(h) > c.hover[2] {
                        self.yaw = atan2(h[1], h[0]);
                    }
                    self.velocity = [h[0], h[1], c.hover[1]];
                    let r = move_and_slide(
                        world.cw,
                        &world.filter,
                        self.shape(s),
                        self.origin(s),
                        scale(self.velocity, dt),
                        &SlideParams::default(),
                    );
                    self.position = sub(r.position, [0.0, 0.0, t.radius[2]]);
                    let landed = r.ground.is_some();
                    if ht >= c.hover[4] || (ht >= c.hover[3] && landed) {
                        self.hover = None;
                        if self.active.take().is_some() {
                            out.push(GangEvent::ItemEnded(id::HOVER_PACK));
                        }
                    }
                    if landed {
                        self.grounded = true;
                        self.velocity = [0.0; 3];
                        self.set_state(bentley::STAND, out);
                    }
                    return;
                }
                if self.air(dt, input, world, s, s.consts.air_steer, out) {
                    self.set_state(bentley::STAND, out);
                }
            }
            bentley::HOLDING => {
                self.move_body(dt, world, s, [0.0; 3], true);
                // Releasing the button (or Triangle) throws the gadget: the aim state.
                let held = self.carrying.and_then(|g| self.slots.0.iter().position(|&i| i == Some(g as u8)));
                let still_held = held.is_some_and(|k| input.holds(button::SLOTS[k])) || input.holds(button::TRIANGLE);
                if !still_held {
                    self.start_aim(s, bentley::THROW_AIM, out);
                }
            }
            bentley::THROW_AIM => self.throw_aim(dt, input, world, s, bentley::STAND, out),
            bentley::TIMED => {
                // The timed state's gait: 1,500 along the heading, which follows the stick directly.
                if self.last_deflection > 0.0 {
                    self.yaw = atan2(self.last_stick[1], self.last_stick[0]);
                }
                let v = scale(heading(self.yaw), gait::BENTLEY_TIMED.speed(1.0));
                self.move_body(dt, world, s, v, true);
                if self.state_time >= spec::BENTLEY_TIMED {
                    self.set_state(bentley::STAND, out);
                }
            }
            bentley::CARRIED => {}
            bentley::DAMAGE => self.recover(dt, input, world, s, bentley::STAND, out),
            _ => {
                self.move_body(dt, world, s, [0.0; 3], true);
                if self.state == bentley::INTERACT && self.state_time > 0.5 {
                    self.set_state(bentley::STAND, out);
                }
            }
        }
    }

    /// The crossbow swing (characters.md 5.3): the nearest attackable target near the point 175 ahead within
    /// 1.5 rad of the heading (3.15 standing); its height blends the high and low clips; standing, Bentley turns
    /// to face it.
    fn crossbow(&mut self, world: &GangWorld<'_>, input: &GangInput, s: &Shared, out: &mut Vec<GangEvent>) {
        let standing = matches!(self.state, bentley::STAND | bentley::NEAR);
        let mode = if self.state == bentley::AIR {
            2
        } else if standing {
            0
        } else {
            1
        };
        let angle = if standing { spec::CROSSBOW_ANGLE[1] } else { spec::CROSSBOW_ANGLE[0] };
        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let p = s.consts.crossbow_point;
        let point = add(self.position, [p[0] * cy - p[1] * sy, p[0] * sy + p[1] * cy, p[2]]);
        let f = heading(self.yaw);
        let mut best: Option<(f32, Target)> = None;
        for tg in world.targets.iter().filter(|t| t.attackable()) {
            let d = sub(tg.position, self.position);
            let hd = length([d[0], d[1], 0.0]);
            let a = if hd > 1e-3 { crate::math::acos(dot(normalize([d[0], d[1], 0.0]), f)) } else { 0.0 };
            if a > angle || hd > s.tun.reach * 2.0 {
                continue;
            }
            let dist = length(sub(tg.position, point));
            if best.is_none_or(|(b, _)| dist < b) {
                best = Some((dist, *tg));
            }
        }
        let _ = input;
        let (target, high) = match best {
            Some((_, tg)) => {
                let dz = tg.position[2] - self.position[2];
                let [lo, hi] = spec::CROSSBOW_HEIGHT;
                if standing {
                    let d = sub(tg.position, self.position);
                    self.yaw = atan2(d[1], d[0]);
                }
                (Some(tg.id), ((dz - lo) / (hi - lo)).clamp(0.0, 1.0))
            }
            None => (None, 0.5),
        };
        out.push(GangEvent::Crossbow { target, high, mode });
    }
}

/// Sly 2 (physics.md 5.1): what is left of "down" once the contact normals, with non-negative weights, cancel as
/// much of it as they can (each normal alone, and each pair).
fn down_left(normals: &[Vec3]) -> Vec3 {
    let d = [0.0, 0.0, -1.0];
    let mut best = d;
    let mut keep = |r: Vec3| {
        if dot(r, r) < dot(best, best) {
            best = r;
        }
    };
    for (i, a) in normals.iter().enumerate() {
        let w = -dot(d, *a);
        if w > 0.0 {
            keep(madd(d, *a, w));
        }
        for b in &normals[i + 1..] {
            let (aa, ab, bb) = (dot(*a, *a), dot(*a, *b), dot(*b, *b));
            let det = aa * bb - ab * ab;
            if det.abs() < 1e-6 {
                continue;
            }
            let (ra, rb) = (-dot(d, *a), -dot(d, *b));
            let wa = (ra * bb - rb * ab) / det;
            let wb = (rb * aa - ra * ab) / det;
            if wa >= 0.0 && wb >= 0.0 {
                keep(add(madd(d, *a, wa), scale(*b, wb)));
            }
        }
    }
    best
}

/// Sly 2 (physics.md 5.1): the stuck test runs this long into the state, and at most this often.
const STUCK_AFTER: f32 = 1.0;
/// Sly 2 (physics.md 5.1): stuck when what is left of "down" has z above this.
const STUCK_DOWN: f32 = -0.1;
/// Sly 2 (physics.md 5.1): a recovery jump longer than this places the character instead.
const RECOVER_JUMP: f32 = 1.25;
