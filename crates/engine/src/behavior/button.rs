//! Buttons (class 40, interactables.md 1).
//!
//! A button is pressed by some contact, chosen by option 258: contact (2), hit (3), hang (4); push (0) and lever
//! (1) need the physics and are treated as contact until it exists (none are on the disc). Pressing raises event 2,
//! plays sound 2 unless silent (option 260), starts the button's animations (its child segments), asks for its
//! dialogue (option 265) and makes it pressed. After the repush delay (option 257, default 1 s, negative = never)
//! it resets: to touched for contact and hang buttons (released, event 3, once the contact ends), to untouched for
//! hit buttons. A remembered button (option 263) stays pressed across checkpoint restarts and raises event 22 when
//! restored.

use alloc::vec::Vec;

use super::{ClassBehavior, Cx, play_child_segments};
use crate::object::{Handle, OptionValue};
use crate::world::{AnimCommand, Notice, Request, World};

/// Button options (options-by-class.md 2.6, interactables.md 1).
pub mod opt {
    /// Repush delay, seconds (default 1; negative never).
    pub const REPUSH: u16 = 257;
    /// Kind: 0 push, 1 lever, 2 contact, 3 hit, 4 hang.
    pub const KIND: u16 = 258;
    /// Silent.
    pub const SILENT: u16 = 260;
    /// Restart the animations on every press.
    pub const RESTART: u16 = 261;
    /// Stay pressed until a script resets it.
    pub const KEEP: u16 = 262;
    /// Remembered across checkpoint restarts.
    pub const REMEMBERED: u16 = 263;
    /// A dialogue (OID) to play when pressed.
    pub const DIALOGUE: u16 = 265;
    /// "On" animations (add).
    pub const ON_ANIMS: u16 = 251;
    /// "Off" animations (add).
    pub const OFF_ANIMS: u16 = 252;
    /// Objects allowed to press it (add).
    pub const ALLOW: u16 = 253;
    /// Objects that must not press it (add).
    pub const DENY: u16 = 254;
    /// Classes allowed (add).
    pub const ALLOW_CLASS: u16 = 255;
    /// Classes not allowed (add).
    pub const DENY_CLASS: u16 = 256;
    /// Goal records (add).
    pub const GOALS: u16 = 264;
}

/// Animation tag of the "off" animations.
pub const TAG_OFF: u32 = 2;

/// Sound played when a button is pressed.
pub const PRESS_SOUND: i32 = 2;
/// Default repush delay.
pub const DEFAULT_REPUSH: f32 = 1.0;
/// Animation tag of the "on" animations.
pub const TAG_ON: u32 = 1;

/// A button's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonState {
    /// Waiting for a press.
    Untouched,
    /// Re-armed but still in contact: released when the contact ends.
    Touched,
    /// Pressed, for this long.
    Pressed(f32),
    /// Disabled (by a script).
    Disabled,
}

/// Button kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Pressed by contact (also push and lever until the physics exists).
    Contact,
    /// Pressed by an attack.
    Hit,
    /// Pressed by hanging from it.
    Hang,
}

/// A button.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// Kind.
    pub kind: ButtonKind,
    /// State.
    pub state: ButtonState,
}

impl Button {
    /// A button configured from its options.
    pub fn new(world: &World, h: Handle) -> Button {
        let kind = match world.options(h).and_then(|o| o.i32(opt::KIND)).unwrap_or(2) {
            3 => ButtonKind::Hit,
            4 => ButtonKind::Hang,
            _ => ButtonKind::Contact,
        };
        Button { kind, state: ButtonState::Untouched }
    }

    /// Presses the button (or restores a remembered press quietly with event 22).
    pub fn press(&mut self, world: &mut World, me: Handle, restored: bool) {
        let o = world.options(me).cloned().unwrap_or_default();
        let restart = o.bool_or(opt::RESTART, false);
        self.state = ButtonState::Pressed(0.0);
        if restored {
            world.raise(me, script::event::SAVED_TRIGGER, Vec::new());
            play_child_segments(world, me, TAG_ON, true);
            world.anim.push(AnimCommand::Finish { owner: me, tag: TAG_ON });
            return;
        }
        world.raise(me, script::event::TRIGGERED, Vec::new());
        if !o.bool_or(opt::SILENT, false) {
            world.play_sound(PRESS_SOUND, Some(me));
        }
        play_child_segments(world, me, TAG_ON, restart);
        super::play_listed(world, me, opt::ON_ANIMS, TAG_ON, restart);
        super::apply_goal_records(world, me, opt::GOALS, true);
        if let Some(d) = o.i16(opt::DIALOGUE).filter(|&d| d > 0) {
            world.request(Request::Dialogue(d));
        }
    }

    fn release(&mut self, world: &mut World, me: Handle) {
        self.state = ButtonState::Untouched;
        world.raise(me, script::event::UNTRIGGERED, Vec::new());
        super::play_listed(world, me, opt::OFF_ANIMS, TAG_OFF, true);
        super::apply_goal_records(world, me, opt::GOALS, false);
    }
}

impl ClassBehavior for Button {
    fn ticks(&self) -> bool {
        self.state != ButtonState::Disabled
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let remembered = cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::REMEMBERED, false));
        let key = cx.world.persist_key(cx.me);
        let world_id = cx.world.info.id;
        if remembered && key.is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.pressed.contains(&k))) {
            self.press(cx.world, cx.me, true);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices = cx.world.take_inbox(cx.me.index());
        let classes = Some((opt::ALLOW_CLASS, opt::DENY_CLASS));
        let contact = notices.iter().any(|n| {
            let who = match (self.kind, n) {
                (ButtonKind::Contact, Notice::Touched(w)) => Some(*w),
                (ButtonKind::Hit, Notice::Hit(w) | Notice::Attack { by: w, .. }) => Some(*w),
                (ButtonKind::Hang, Notice::Hang(w)) => Some(*w),
                _ => None,
            };
            who.is_some_and(|w| super::may_press(cx.world, cx.me, Some(w), opt::ALLOW, Some(opt::DENY), classes))
        });
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        match self.state {
            ButtonState::Disabled => {}
            ButtonState::Untouched => {
                if contact {
                    self.press(cx.world, cx.me, false);
                    if o.bool_or(opt::REMEMBERED, false)
                        && let Some(k) = cx.world.persist_key(cx.me)
                    {
                        let world_id = cx.world.info.id;
                        cx.progress.level(world_id).pressed.insert(k);
                    }
                }
            }
            ButtonState::Pressed(t) => {
                let t = t + cx.dt;
                let delay = o.f32_or(opt::REPUSH, DEFAULT_REPUSH);
                if delay >= 0.0 && t >= delay && !o.bool_or(opt::KEEP, false) {
                    self.state = match self.kind {
                        ButtonKind::Hit => ButtonState::Untouched,
                        _ => ButtonState::Touched,
                    };
                } else {
                    self.state = ButtonState::Pressed(t);
                }
            }
            ButtonState::Touched => {
                if !contact {
                    self.release(cx.world, cx.me);
                }
            }
        }
    }

    fn option_changed(&mut self, world: &mut World, me: Handle, id: u16, _value: &OptionValue) {
        if id == opt::KIND {
            self.kind = Button::new(world, me).kind;
        }
    }
}
