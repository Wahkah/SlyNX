//! Controllers: buttons, sticks with a deadzone, press/release edges, and a small action-mapping layer.
//!
//! Each backend reads its controllers into a [`RawPad`] per player and hands them to [`InputState::update`], which
//! computes edges and applies the deadzone the same way on every platform.

use alloc::vec::Vec;

/// Players the platform tracks. Player 1 also receives the Switch's handheld controls and, on the PC, the keyboard.
pub const MAX_PLAYERS: usize = 4;

/// Button bits. The layout of the low 16 bits follows the Switch's controller report so the Switch backend maps them
/// with a mask; the PC backend maps its gamepads and keyboard onto the same bits by position (the bottom face button
/// is [`Buttons::B`], the right one [`Buttons::A`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Buttons(pub u32);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);
    /// Right face button.
    pub const A: Buttons = Buttons(1 << 0);
    /// Bottom face button.
    pub const B: Buttons = Buttons(1 << 1);
    /// Top face button.
    pub const X: Buttons = Buttons(1 << 2);
    /// Left face button.
    pub const Y: Buttons = Buttons(1 << 3);
    /// Left stick click.
    pub const LSTICK: Buttons = Buttons(1 << 4);
    /// Right stick click.
    pub const RSTICK: Buttons = Buttons(1 << 5);
    pub const L: Buttons = Buttons(1 << 6);
    pub const R: Buttons = Buttons(1 << 7);
    pub const ZL: Buttons = Buttons(1 << 8);
    pub const ZR: Buttons = Buttons(1 << 9);
    pub const PLUS: Buttons = Buttons(1 << 10);
    pub const MINUS: Buttons = Buttons(1 << 11);
    pub const LEFT: Buttons = Buttons(1 << 12);
    pub const UP: Buttons = Buttons(1 << 13);
    pub const RIGHT: Buttons = Buttons(1 << 14);
    pub const DOWN: Buttons = Buttons(1 << 15);
    /// SL on either Joy-Con.
    pub const SL: Buttons = Buttons(1 << 16);
    /// SR on either Joy-Con.
    pub const SR: Buttons = Buttons(1 << 17);

    /// Every defined bit.
    pub const ALL: Buttons = Buttons((1 << 18) - 1);

    /// Short names in bit order, for debug displays.
    pub const NAMES: [&'static str; 18] =
        ["A", "B", "X", "Y", "LS", "RS", "L", "R", "ZL", "ZR", "+", "-", "LEFT", "UP", "RIGHT", "DOWN", "SL", "SR"];

    /// True if any bit of `other` is set in `self`.
    pub const fn any(self, other: Buttons) -> bool {
        self.0 & other.0 != 0
    }

    /// True if every bit of `other` is set in `self`.
    pub const fn all(self, other: Buttons) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Buttons) -> Buttons {
        Buttons(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::BitOr for Buttons {
    type Output = Buttons;
    fn bitor(self, rhs: Buttons) -> Buttons {
        Buttons(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Buttons) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for Buttons {
    type Output = Buttons;
    fn bitand(self, rhs: Buttons) -> Buttons {
        Buttons(self.0 & rhs.0)
    }
}

/// What kind of controller a player is using.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControllerKind {
    #[default]
    None,
    /// Joy-Cons attached to the console.
    Handheld,
    /// Two Joy-Cons used together as one controller.
    JoyConPair,
    /// One left Joy-Con on its own.
    JoyConLeft,
    /// One right Joy-Con on its own.
    JoyConRight,
    ProController,
    /// A PC gamepad that is not a Nintendo controller.
    Gamepad,
    /// The PC keyboard (player 1 only).
    Keyboard,
}

impl ControllerKind {
    pub const fn name(self) -> &'static str {
        match self {
            ControllerKind::None => "NONE",
            ControllerKind::Handheld => "HANDHELD",
            ControllerKind::JoyConPair => "JOY-CON PAIR",
            ControllerKind::JoyConLeft => "JOY-CON L",
            ControllerKind::JoyConRight => "JOY-CON R",
            ControllerKind::ProController => "PRO CONTROLLER",
            ControllerKind::Gamepad => "GAMEPAD",
            ControllerKind::Keyboard => "KEYBOARD",
        }
    }
}

/// A stick position. `x` is positive to the right, `y` positive up; both in -1..=1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stick {
    pub x: f32,
    pub y: f32,
}

impl Stick {
    pub const ZERO: Stick = Stick { x: 0.0, y: 0.0 };

    /// Length of the vector (0..=1 after the deadzone; raw values may reach about 1.41 on square gates).
    pub fn magnitude(self) -> f32 {
        sqrt(self.x * self.x + self.y * self.y)
    }
}

/// A radial deadzone. Positions with a magnitude below `inner` read as zero; above `outer` as full deflection;
/// in between the magnitude is rescaled linearly so movement starts smoothly at the edge of the deadzone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deadzone {
    pub inner: f32,
    pub outer: f32,
}

impl Default for Deadzone {
    fn default() -> Self {
        Deadzone { inner: 0.15, outer: 0.95 }
    }
}

impl Deadzone {
    /// Applies the deadzone to a raw position.
    pub fn apply(self, raw: Stick) -> Stick {
        let x = if raw.x.is_finite() { raw.x } else { 0.0 };
        let y = if raw.y.is_finite() { raw.y } else { 0.0 };
        let mag = sqrt(x * x + y * y);
        if mag <= self.inner || mag == 0.0 {
            return Stick::ZERO;
        }
        let span = self.outer - self.inner;
        let scaled = if span <= 0.0 { 1.0 } else { ((mag - self.inner) / span).clamp(0.0, 1.0) };
        let k = scaled / mag;
        Stick { x: (x * k).clamp(-1.0, 1.0), y: (y * k).clamp(-1.0, 1.0) }
    }
}

/// One player's controller as a backend reads it, before edges and the deadzone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RawPad {
    pub kind: ControllerKind,
    pub buttons: Buttons,
    /// Raw stick positions, already normalised to about -1..=1 with `y` up.
    pub left: Stick,
    pub right: Stick,
}

impl RawPad {
    pub const DISCONNECTED: RawPad =
        RawPad { kind: ControllerKind::None, buttons: Buttons::NONE, left: Stick::ZERO, right: Stick::ZERO };
}

/// One player's controller after [`InputState::update`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub kind: ControllerKind,
    /// Buttons down now.
    pub held: Buttons,
    /// Buttons that went down since the previous update.
    pub pressed: Buttons,
    /// Buttons that went up since the previous update.
    pub released: Buttons,
    /// Sticks after the deadzone.
    pub left: Stick,
    pub right: Stick,
}

impl PadState {
    pub fn connected(&self) -> bool {
        self.kind != ControllerKind::None
    }

    pub fn stick(&self, id: StickId) -> Stick {
        match id {
            StickId::Left => self.left,
            StickId::Right => self.right,
        }
    }
}

/// Every player's controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    pub players: [PadState; MAX_PLAYERS],
}

impl InputState {
    /// Player 1, which also has the handheld controls (Switch) and the keyboard (PC).
    pub fn p1(&self) -> &PadState {
        &self.players[0]
    }

    /// Takes a new reading. Edges are computed against the previous reading; a controller that disconnects releases
    /// everything it held.
    pub fn update(&mut self, raw: &[RawPad; MAX_PLAYERS], deadzone: Deadzone) {
        for (pad, raw) in self.players.iter_mut().zip(raw.iter()) {
            let held = if raw.kind == ControllerKind::None { Buttons::NONE } else { raw.buttons & Buttons::ALL };
            let prev = pad.held;
            *pad = PadState {
                kind: raw.kind,
                held,
                pressed: Buttons(held.0 & !prev.0),
                released: Buttons(prev.0 & !held.0),
                left: deadzone.apply(raw.left),
                right: deadzone.apply(raw.right),
            };
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickId {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// What can trigger an action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Binding {
    /// Any of these buttons.
    Button(Buttons),
    /// A stick pushed in a direction past [`ActionMap::stick_threshold`] (after the deadzone).
    Stick(StickId, Dir),
}

/// The actions active in one frame, as bit sets indexed by action number (0..64).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    pub held: u64,
    pub pressed: u64,
    pub released: u64,
}

impl Actions {
    pub fn held(&self, action: u8) -> bool {
        self.held & bit(action) != 0
    }
    pub fn pressed(&self, action: u8) -> bool {
        self.pressed & bit(action) != 0
    }
    pub fn released(&self, action: u8) -> bool {
        self.released & bit(action) != 0
    }
}

fn bit(action: u8) -> u64 {
    if action < 64 { 1 << action } else { 0 }
}

/// Maps controller inputs to game actions for one player. Actions are numbers 0..64 the game defines; one action may
/// have several bindings, and one input may drive several actions.
///
/// ```
/// use platform_api::{ActionMap, Binding, Buttons, Dir, StickId};
/// const JUMP: u8 = 0;
/// const MOVE_LEFT: u8 = 1;
/// let mut map = ActionMap::new(0);
/// map.bind(JUMP, Binding::Button(Buttons::B));
/// map.bind(MOVE_LEFT, Binding::Button(Buttons::LEFT));
/// map.bind(MOVE_LEFT, Binding::Stick(StickId::Left, Dir::Left));
/// ```
#[derive(Clone, Debug)]
pub struct ActionMap {
    player: usize,
    bindings: Vec<(u8, Binding)>,
    /// How far a stick must be pushed (after the deadzone) to count as a direction. Default 0.5.
    pub stick_threshold: f32,
    prev: u64,
}

impl ActionMap {
    /// An empty map reading `player` (0-based; values past the last player read as nothing pressed).
    pub fn new(player: usize) -> ActionMap {
        ActionMap { player, bindings: Vec::new(), stick_threshold: 0.5, prev: 0 }
    }

    /// Adds a binding. Actions numbered 64 or more are ignored.
    pub fn bind(&mut self, action: u8, binding: Binding) {
        if action < 64 {
            self.bindings.push((action, binding));
        }
    }

    /// Removes every binding of `action`.
    pub fn unbind(&mut self, action: u8) {
        self.bindings.retain(|(a, _)| *a != action);
    }

    pub fn bindings(&self) -> &[(u8, Binding)] {
        &self.bindings
    }

    /// Evaluates the bindings against this frame's input. Call once per frame; edges are relative to the previous
    /// call.
    pub fn update(&mut self, input: &InputState) -> Actions {
        let mut held = 0u64;
        if let Some(pad) = input.players.get(self.player) {
            for &(action, binding) in &self.bindings {
                let on = match binding {
                    Binding::Button(b) => pad.held.any(b),
                    Binding::Stick(id, dir) => {
                        let s = pad.stick(id);
                        let t = self.stick_threshold;
                        match dir {
                            Dir::Left => s.x <= -t,
                            Dir::Right => s.x >= t,
                            Dir::Up => s.y >= t,
                            Dir::Down => s.y <= -t,
                        }
                    }
                };
                if on {
                    held |= bit(action);
                }
            }
        }
        let actions = Actions { held, pressed: held & !self.prev, released: self.prev & !held };
        self.prev = held;
        actions
    }
}

/// Square root for `no_std` (no libm): Newton's method from a bit-level first guess. Accurate to about 1 ulp for
/// the 0..2 range sticks use.
fn sqrt(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return if v.is_nan() {
            f32::NAN
        } else if v == f32::INFINITY {
            v
        } else {
            0.0
        };
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        x = 0.5 * (x + v / x);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(buttons: Buttons) -> [RawPad; MAX_PLAYERS] {
        let mut r = [RawPad::DISCONNECTED; MAX_PLAYERS];
        r[0] = RawPad { kind: ControllerKind::ProController, buttons, ..RawPad::DISCONNECTED };
        r
    }

    #[test]
    fn sqrt_matches_std() {
        for i in 0..2000 {
            let v = i as f32 * 0.001;
            assert!((sqrt(v) - v.sqrt()).abs() < 1e-6, "{v}");
        }
        assert_eq!(sqrt(-1.0), 0.0);
        assert!(sqrt(f32::NAN).is_nan());
    }

    #[test]
    fn edges() {
        let mut s = InputState::default();
        s.update(&raw(Buttons::A), Deadzone::default());
        assert_eq!(s.p1().pressed, Buttons::A);
        assert_eq!(s.p1().held, Buttons::A);
        s.update(&raw(Buttons::A | Buttons::B), Deadzone::default());
        assert_eq!(s.p1().pressed, Buttons::B);
        assert_eq!(s.p1().released, Buttons::NONE);
        s.update(&raw(Buttons::B), Deadzone::default());
        assert_eq!(s.p1().released, Buttons::A);
        assert!(s.p1().connected());
        assert!(!s.players[1].connected());
    }

    #[test]
    fn disconnect_releases_everything() {
        let mut s = InputState::default();
        s.update(&raw(Buttons::A | Buttons::PLUS), Deadzone::default());
        s.update(&[RawPad::DISCONNECTED; MAX_PLAYERS], Deadzone::default());
        assert_eq!(s.p1().held, Buttons::NONE);
        assert_eq!(s.p1().released, Buttons::A | Buttons::PLUS);
    }

    #[test]
    fn unknown_bits_are_dropped() {
        let mut s = InputState::default();
        s.update(&raw(Buttons(0xFFFF_FFFF)), Deadzone::default());
        assert_eq!(s.p1().held, Buttons::ALL);
    }

    #[test]
    fn deadzone() {
        let dz = Deadzone { inner: 0.2, outer: 0.9 };
        assert_eq!(dz.apply(Stick { x: 0.1, y: 0.1 }), Stick::ZERO);
        assert_eq!(dz.apply(Stick { x: 0.0, y: 0.0 }), Stick::ZERO);
        let full = dz.apply(Stick { x: 1.0, y: 0.0 });
        assert!((full.x - 1.0).abs() < 1e-6 && full.y == 0.0);
        // Halfway between inner and outer reads as half deflection, keeping the direction.
        let mid = dz.apply(Stick { x: 0.0, y: -0.55 });
        assert!((mid.y + 0.5).abs() < 1e-5 && mid.x == 0.0, "{mid:?}");
        // Diagonal at full deflection on a square gate stays within the unit circle.
        let diag = dz.apply(Stick { x: 1.0, y: 1.0 });
        assert!((diag.magnitude() - 1.0).abs() < 1e-5, "{diag:?}");
        assert_eq!(dz.apply(Stick { x: f32::NAN, y: 0.0 }), Stick::ZERO);
        // Just past the inner edge starts near zero, not with a jump.
        assert!(dz.apply(Stick { x: 0.21, y: 0.0 }).x < 0.02);
    }

    #[test]
    fn action_map() {
        const JUMP: u8 = 0;
        const LEFT: u8 = 5;
        const BIG: u8 = 63;
        let mut map = ActionMap::new(0);
        map.bind(JUMP, Binding::Button(Buttons::B));
        map.bind(JUMP, Binding::Button(Buttons::A));
        map.bind(LEFT, Binding::Stick(StickId::Left, Dir::Left));
        map.bind(LEFT, Binding::Button(Buttons::LEFT));
        map.bind(BIG, Binding::Stick(StickId::Right, Dir::Up));
        map.bind(64, Binding::Button(Buttons::A)); // ignored

        let mut s = InputState::default();
        let mut r = raw(Buttons::A);
        s.update(&r, Deadzone::default());
        let a = map.update(&s);
        assert!(a.pressed(JUMP) && a.held(JUMP) && !a.held(LEFT));

        r[0].buttons = Buttons::A | Buttons::B;
        r[0].left = Stick { x: -0.9, y: 0.0 };
        r[0].right = Stick { x: 0.0, y: 0.9 };
        s.update(&r, Deadzone::default());
        let a = map.update(&s);
        assert!(a.held(JUMP) && !a.pressed(JUMP), "second binding does not re-press");
        assert!(a.pressed(LEFT) && a.pressed(BIG));

        r[0] = RawPad { kind: ControllerKind::ProController, ..RawPad::DISCONNECTED };
        s.update(&r, Deadzone::default());
        let a = map.update(&s);
        assert!(a.released(JUMP) && a.released(LEFT) && a.held == 0);

        map.unbind(JUMP);
        assert_eq!(map.bindings().len(), 3);
        // A map for a player past the last one reads nothing.
        assert_eq!(ActionMap::new(9).update(&s), Actions::default());
    }
}
