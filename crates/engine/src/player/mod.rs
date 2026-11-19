//! Sly, the player character: a controller driven by an input state, moving through the collision world, with the
//! states, jumps, context moves, attacks, power-ups and damage of specs/sly1/player-moves.md (and physics.md 2,
//! guards.md 4.3, interactables.md 5 and 7, animation.md 7).
//!
//! | Module | What |
//! |---|---|
//! | [`consts`] | the spec's constants (read from the executable where it gives an address) and the port's tunables |
//! | [`input`] | the input state, the world features the context moves use, world-space ledges |
//! | [`controller`] | [`Player`]: the state machine, movement and events |
//! | [`status`] | damage kinds, charms and lives, power-ups and their reward bits, the hidden rules |
//! | [`anim`] | the lookup-list OIDs per state, root-motion speeds, and a driver that plays the choice on a rig |
//!
//! The object model owns the player's object; each frame it builds a [`PlayerInput`] and a [`PlayerWorld`] (the
//! collision world with the player's own solids disabled, the context features, ledges, shadow and cover flags),
//! calls [`Player::update`], places the player's object at [`Player::position`] facing [`Player::yaw`], plays
//! [`PlayerOutput::anim`] (for example with [`anim::PlayerAnimator`]) and acts on [`PlayerOutput::events`].

pub mod anim;
pub mod consts;
pub mod controller;
pub mod input;
pub mod status;

#[cfg(test)]
mod tests;

pub use anim::{AnimOids, AnimRequest, PlayerAnimator, root_motion_speed};
pub use consts::{PlayerConstants, Quadratic, Tunables};
pub use controller::{AttackKind, JumpKind, Player, PlayerEvent, PlayerOutput, State, SteerMode};
pub use input::{Button, Feature, FeatureKind, PlayerInput, PlayerWorld, world_ledges};
pub use status::{DamageKind, DamageOutcome, Hiding, PowerUp, Vitals};
