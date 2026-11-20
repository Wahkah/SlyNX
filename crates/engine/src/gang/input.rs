//! The Sly 2 controller as the characters read it, press windows, and the world they act in.

use super::Who;
use crate::collision::world::{CollisionWorld, QueryFilter};
use crate::math::{Vec3, add, cos, scale, sin};

/// Buttons, named after the controller (characters.md).
pub mod button {
    /// Cross (jump).
    pub const CROSS: u32 = 1 << 0;
    /// Circle (interact, lift).
    pub const CIRCLE: u32 = 1 << 1;
    /// Square.
    pub const SQUARE: u32 = 1 << 2;
    /// Triangle.
    pub const TRIANGLE: u32 = 1 << 3;
    /// R1 (run; Bentley's darts).
    pub const R1: u32 = 1 << 4;
    /// R3 (binocucom).
    pub const R3: u32 = 1 << 5;
    /// L1 (gadget slot 0).
    pub const L1: u32 = 1 << 6;
    /// L2 (gadget slot 1).
    pub const L2: u32 = 1 << 7;
    /// R2 (gadget slot 2).
    pub const R2: u32 = 1 << 8;
    /// The three gadget slot buttons, in slot order.
    pub const SLOTS: [u32; 3] = [L1, L2, R2];
    /// Number of buttons.
    pub const COUNT: usize = 9;
}

/// One frame of input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GangInput {
    /// Left stick, x right and y up.
    pub stick: [f32; 2],
    /// Camera heading (the stick is camera-relative).
    pub camera_yaw: f32,
    /// Buttons held.
    pub held: u32,
    /// Buttons that went down this frame.
    pub pressed: u32,
    /// Camera eye (Bentley's darts fly along the screen centre).
    pub camera_eye: Vec3,
    /// Camera forward (unit).
    pub camera_forward: Vec3,
    /// The stick's deflection as the game reads it (filled by `Gang::update` from its stick reader; with the Sly 3
    /// rule the stick vector is shorter than this).
    pub deflection: f32,
}

impl GangInput {
    /// The stick as a world direction scaled by its deflection.
    pub fn world_stick(&self) -> Vec3 {
        let (s, c) = (sin(self.camera_yaw), cos(self.camera_yaw));
        add(scale([c, s, 0.0], self.stick[1]), scale([s, -c, 0.0], self.stick[0]))
    }

    /// Whether `b` is held.
    pub fn holds(&self, b: u32) -> bool {
        self.held & b != 0
    }
}

/// "Pressed within t": the time since each button's last press, consumed by the action that uses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressBuffer {
    since: [f32; button::COUNT],
}

impl Default for PressBuffer {
    fn default() -> Self {
        PressBuffer { since: [f32::INFINITY; button::COUNT] }
    }
}

impl PressBuffer {
    /// Ages the buffer and records this frame's presses.
    pub fn update(&mut self, input: &GangInput, dt: f32) {
        for (i, s) in self.since.iter_mut().enumerate() {
            *s = if input.pressed & (1 << i) != 0 { 0.0 } else { *s + dt };
        }
    }

    /// Whether `b` was pressed within `t`; consumes the press when it was.
    pub fn take(&mut self, b: u32, t: f32) -> bool {
        let i = b.trailing_zeros() as usize;
        if i < button::COUNT && self.since[i] <= t {
            self.since[i] = f32::INFINITY;
            return true;
        }
        false
    }

    /// Whether `b` was pressed within `t`, without consuming it.
    pub fn peek(&self, b: u32, t: f32) -> bool {
        let i = b.trailing_zeros() as usize;
        i < button::COUNT && self.since[i] <= t
    }
}

/// What a nearby thing is, for the characters' choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TargetKind {
    /// A guard.
    Guard {
        /// Of the "can be robbed" rank.
        robbable: bool,
        /// Airborne after Murray launched it.
        launched: bool,
    },
    /// An object.
    Object {
        /// Murray can lift it.
        liftable: bool,
        /// Attacks can hit it.
        attackable: bool,
    },
    /// Something the interaction button uses (doors, levers, hiding places).
    Interactable,
    /// One of the gang.
    Character(Who),
}

/// A nearby thing (the object model lists them each frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// Caller's id.
    pub id: u32,
    /// Position.
    pub position: Vec3,
    /// Heading (guards: for pickpocketing from behind).
    pub yaw: f32,
    /// What it is.
    pub kind: TargetKind,
}

impl Target {
    /// Whether attacks can hit it.
    pub fn attackable(&self) -> bool {
        matches!(self.kind, TargetKind::Guard { .. } | TargetKind::Object { attackable: true, .. })
    }
}

/// The world the gang moves in this frame.
#[derive(Clone, Copy)]
pub struct GangWorld<'a> {
    /// Collision (the characters' own solids disabled).
    pub cw: &'a CollisionWorld,
    /// Query filter.
    pub filter: QueryFilter,
    /// Nearby things.
    pub targets: &'a [Target],
    /// The job's difficulty help (0 to 1).
    pub help: f32,
    /// The safe house is the current player (no damage).
    pub safehouse: bool,
    /// Free copies in the pools of the pooled items, as (item, free copies); an item not listed is not limited.
    pub pools: &'a [(u8, u32)],
}

impl GangWorld<'_> {
    /// Free copies in the pool of `item` (`u32::MAX` when not listed).
    pub fn free_copies(&self, item: u8) -> u32 {
        self.pools.iter().find(|p| p.0 == item).map_or(u32::MAX, |p| p.1)
    }
}
