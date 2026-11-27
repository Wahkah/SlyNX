//! The Sly 2 playable characters (characters.md, gadgets.md): Sly, Murray and Bentley, their shared rules
//! (health, gadgets, throw aiming) and character switching.
//!
//! The Sly 1 controller in [`crate::player`] is unchanged; this module is its Sly 2 counterpart. Each
//! [`Character`] is a state machine with the spec's state numbers; [`Gang`] owns the three, feeds the current
//! player the pad and the others an idle input, lists each character to the others as a target, and runs the
//! hand-offs between them (Murray lifting, carrying and throwing Bentley, Bentley becoming the player on
//! landing).

use alloc::vec::Vec;

pub mod character;
pub mod combo;
pub mod consts;
pub mod gadgets;
pub mod health;
pub mod input;
pub mod items;
pub mod parts;
pub mod pocket;
pub mod pool;
pub mod sly3;
pub mod throw;

#[cfg(test)]
mod tests;

pub use character::{Character, Combo, GangEvent, GangGame, Shared, bentley, murray, sly};

/// Sly's binocucom (Sly 2 and Sly 3; camera.md 6): R3 turns it on with `focus` (the played character), and again
/// off. The camera reads `World::shared.camera_binocucom`; while it is on the left stick turns the view, so the
/// player systems give the gang a still stick ([`binocucom_on`]).
pub fn toggle_binocucom(world: &mut crate::world::World, focus: crate::object::Handle, invert: bool) {
    let s = &mut world.shared.camera_binocucom;
    *s = if s.is_some() { None } else { Some((focus, invert)) };
}

/// Whether the binocucom is on.
pub fn binocucom_on(world: &crate::world::World) -> bool {
    world.shared.camera_binocucom.is_some()
}
pub use consts::{GangConstants, GangTunables};
pub use gadgets::{Assignment, CarryState, Carryable, Effect, GadgetConstants, PayAt, Refusal, WorldEffect};
pub use health::{Health, Hit, Reaction};
pub use input::{GangInput, GangWorld, PressBuffer, Target, TargetKind, button};
pub use items::{ButtonUse, Item, ItemTable, Owned, Power, Slots};
pub use pocket::{Entry, LootTable, Pocket, PocketTable, Purse};
pub use throw::{Flight, ThrowAim};

use crate::math::{Vec3, add};

/// One of the gang.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    /// Sly.
    Sly,
    /// Murray.
    Murray,
    /// Bentley.
    Bentley,
}

impl Who {
    /// Index into the per-character arrays: 0 Sly, 1 Murray, 2 Bentley.
    pub const fn index(self) -> usize {
        match self {
            Who::Sly => 0,
            Who::Murray => 1,
            Who::Bentley => 2,
        }
    }

    /// The target id a character is listed under to the others.
    pub const fn target_id(self) -> u32 {
        0x8000_0000 | self.index() as u32
    }

    /// The character listed under a target id.
    pub fn from_target_id(id: u32) -> Option<Who> {
        match id {
            0x8000_0000 => Some(Who::Sly),
            0x8000_0001 => Some(Who::Murray),
            0x8000_0002 => Some(Who::Bentley),
            _ => None,
        }
    }
}

/// The gang: up to one of each character, the current player and the shared rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Gang {
    /// The characters present.
    pub members: Vec<Character>,
    /// The current player.
    pub player: Who,
    /// Shared rules and what the gang owns.
    pub shared: Shared,
    /// The player's left stick as the games read it (Sly 1 and 2's rule, Sly 3's with a Sly 3 item table).
    pub stick: crate::player::stick::StickReader,
    /// Whether the input's stick is raw (the pad's) and goes through [`Gang::stick`]; off for inputs made by hand.
    pub raw_stick: bool,
}

/// Height above Murray's feet where a carried character rides.
const CARRY_HEIGHT: f32 = 150.0;

impl Gang {
    /// A gang with these members; the first is the player.
    pub fn new(members: Vec<Character>, shared: Shared) -> Gang {
        let player = members.first().map_or(Who::Sly, |c| c.who);
        let rule = if shared.items.is_sly3() {
            crate::player::stick::StickRule::Sly3
        } else {
            crate::player::stick::StickRule::Normalised
        };
        Gang { members, player, shared, stick: crate::player::stick::StickReader::new(rule), raw_stick: true }
    }

    /// A member.
    pub fn get(&self, who: Who) -> Option<&Character> {
        self.members.iter().find(|c| c.who == who)
    }

    /// A member, mutable.
    pub fn get_mut(&mut self, who: Who) -> Option<&mut Character> {
        self.members.iter_mut().find(|c| c.who == who)
    }

    /// The current player's character.
    pub fn current(&self) -> Option<&Character> {
        self.get(self.player)
    }

    /// Switches the player to `who` if present and not being carried or thrown; returns whether it switched.
    pub fn switch_to(&mut self, who: Who) -> bool {
        match self.get(who) {
            Some(c) if c.who == Who::Bentley && (c.state == bentley::CARRIED || c.tossed) => false,
            Some(_) => {
                self.player = who;
                true
            }
            None => false,
        }
    }

    /// Sly 2: Murray takes Bentley up ready to throw (script ID 939, stream option 1051; characters.md 4.6 and 7,
    /// missions.md 4.3: `t1_heist_open_piperoom`): Bentley is carried and Murray is in the throw aim (state 15). The
    /// played character does not change. Returns whether both are there.
    pub fn hold_bentley(&mut self) -> bool {
        let id = Who::Bentley.target_id();
        let mut out = Vec::new();
        let shared = &self.shared;
        let Some(m) = self.members.iter_mut().find(|c| c.who == Who::Murray) else { return false };
        m.hold_to_throw(shared, id, &mut out);
        let Some(b) = self.members.iter_mut().find(|c| c.who == Who::Bentley) else { return false };
        b.velocity = [0.0; 3];
        b.tossed = false;
        b.set_state(bentley::CARRIED, &mut out);
        true
    }

    /// One frame: the player reads `input`, the others an idle input.
    pub fn update(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>) -> Vec<(Who, GangEvent)> {
        let mut events = Vec::new();
        // The pad's stick as the game reads it (player-moves.md 1.2; Sly 2 characters.md 12; Sly 3 11).
        let read;
        let input = if self.raw_stick {
            let (dir, u) = self.stick.read_axes(input.stick);
            read = GangInput { stick: dir, deflection: u, ..*input };
            &read
        } else {
            input
        };
        // Each member sees the others as targets.
        let snapshot: Vec<(Who, Vec3, f32)> = self.members.iter().map(|c| (c.who, c.position, c.yaw)).collect();
        let idle = GangInput {
            camera_yaw: input.camera_yaw,
            camera_eye: input.camera_eye,
            camera_forward: input.camera_forward,
            ..GangInput::default()
        };
        for i in 0..self.members.len() {
            let who = self.members[i].who;
            let mut targets: Vec<Target> = world.targets.to_vec();
            targets.extend(snapshot.iter().filter(|o| o.0 != who).map(|&(w, p, y)| Target {
                id: w.target_id(),
                position: p,
                yaw: y,
                kind: TargetKind::Character(w),
            }));
            let w = GangWorld { targets: &targets, ..*world };
            let inp = if who == self.player { input } else { &idle };
            let mut out = Vec::new();
            self.members[i].update(dt, inp, &w, &self.shared, &mut out);
            events.extend(out.into_iter().map(|e| (who, e)));
        }
        self.hand_offs(&mut events);
        events
    }

    /// Lifting, carrying, dropping and throwing a character, and the player change when Bentley lands.
    fn hand_offs(&mut self, events: &mut Vec<(Who, GangEvent)>) {
        let mut extra = Vec::new();
        for &(who, e) in events.iter() {
            match e {
                GangEvent::Lift(id) | GangEvent::GrabbedLaunched(id) => {
                    if let Some(w) = Who::from_target_id(id)
                        && let Some(c) = self.get_mut(w)
                    {
                        c.velocity = [0.0; 3];
                        c.set_state(bentley::CARRIED, &mut extra);
                    }
                }
                GangEvent::Thrown { id, velocity } => {
                    if let Some(w) = Who::from_target_id(id)
                        && let Some(c) = self.get_mut(w)
                    {
                        c.velocity = velocity;
                        c.grounded = false;
                        c.leave_ground();
                        c.tossed = true;
                        c.set_state(bentley::AIR, &mut extra);
                    }
                }
                GangEvent::BecamePlayer(w) if who == w => self.player = w,
                _ => {}
            }
        }
        // Ride on Murray while carried. When the toss ends without a throw (Murray hit, or the throw cancelled),
        // Bentley flies off at 400 along the direction from Murray to him (+X at the same spot) and 600 up, in
        // world space (characters.md 4.6).
        let cancelled = events.iter().any(|(w, e)| *w == Who::Murray && *e == GangEvent::ThrowCancelled);
        let bentley_id = Who::Bentley.target_id();
        if cancelled
            && let Some(m) = self.get_mut(Who::Murray)
            && m.carrying == Some(bentley_id)
        {
            m.carrying = None;
            m.set_state(murray::STAND, &mut Vec::new());
        }
        let murray = self.get(Who::Murray).map(|m| (m.position, m.carrying));
        if let Some(c) = self.get_mut(Who::Bentley)
            && c.state == bentley::CARRIED
        {
            match murray {
                Some((p, Some(id))) if id == bentley_id => {
                    c.position = add(p, [0.0, 0.0, CARRY_HEIGHT]);
                }
                other => {
                    let from = other.map_or(c.position, |m| m.0);
                    let d = [c.position[0] - from[0], c.position[1] - from[1]];
                    let l = crate::math::sqrt(d[0] * d[0] + d[1] * d[1]);
                    let dir = if l > 1e-3 { [d[0] / l, d[1] / l] } else { [1.0, 0.0] };
                    let [h, up] = consts::spec::BENTLEY_DROP;
                    c.velocity = [h * dir[0], h * dir[1], up];
                    c.leave_ground();
                    c.tossed = true;
                    c.set_state(bentley::AIR, &mut extra);
                }
            }
        }
        events.extend(extra.into_iter().map(|e| (Who::Bentley, e)));
    }
}
