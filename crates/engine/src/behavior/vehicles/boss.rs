//! Bosses' and story characters' hit points (Sly 2 classes 59 to 67, specs/sly2/vehicles-and-minigames.md 7.2 and
//! 7.4).
//!
//! Hit points are option 254 (read back and set by scripts through 255), the block capacity 256 or 257 (remaining
//! 258). A hit first wears down the block, then the hit points; at 0 the boss is knocked out and the tasks hear
//! task event 23 [I: the knock-out event of guards.md 7; 24 is its other variant]. While option 231 or 1350 (busy)
//! is set, hits are not taken. A hit by the gang counts as the guards' placeholder strength (`sly2::GANG_HIT`);
//! the gang's attacks' real damage is open (7.4). The boss's moves stay with the shared enemy engine.

use super::task_event;
use crate::behavior::{ClassBehavior, Cx};
use crate::object::{Handle, OptionValue};
use crate::world::{Notice, World};

/// Option numbers.
pub mod opt {
    pub const HIT_POINTS: u16 = 254;
    pub const REMAINING: u16 = 255;
    pub const BLOCK: [u16; 2] = [256, 257];
    pub const BLOCK_LEFT: u16 = 258;
    pub const BUSY: [u16; 2] = [231, 1350];
}

/// A boss's health.
#[derive(Clone, Debug, PartialEq)]
pub struct Boss {
    pub hit_points: f32,
    pub block: f32,
    pub knocked_out: bool,
    pub hits: u32,
}

impl Boss {
    pub fn new(world: &World, me: Handle) -> Boss {
        let o = world.options(me);
        let block_cap = opt::BLOCK.iter().find_map(|&id| o.and_then(|o| o.f32(id))).unwrap_or(0.0);
        Boss {
            hit_points: o.and_then(|o| o.f32(opt::HIT_POINTS)).unwrap_or(40.0),
            block: o.and_then(|o| o.f32(opt::BLOCK_LEFT)).unwrap_or(block_cap),
            knocked_out: false,
            hits: 0,
        }
    }

    /// Takes a hit of `damage`: the block first, then the hit points. Returns whether it knocked the boss out.
    pub fn hit(&mut self, damage: f32) -> bool {
        if self.knocked_out {
            return false;
        }
        self.hits += 1;
        let absorbed = damage.min(self.block);
        self.block -= absorbed;
        self.hit_points = (self.hit_points - (damage - absorbed)).max(0.0);
        if self.hit_points <= 0.0 {
            self.knocked_out = true;
            return true;
        }
        false
    }
}

impl ClassBehavior for Boss {
    fn ticks(&self) -> bool {
        !self.knocked_out
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let busy = cx.world.options(cx.me).is_some_and(|o| opt::BUSY.iter().any(|&id| o.bool_or(id, false)));
        for n in cx.world.notices(cx.me).to_vec() {
            if busy || !matches!(n, Notice::Hit(_) | Notice::Attack { .. } | Notice::Blast(..)) {
                continue;
            }
            if self.hit(crate::behavior::sly2::GANG_HIT) {
                task_event(cx.world, 23);
            }
        }
    }

    fn option_changed(&mut self, _world: &mut World, _me: Handle, id: u16, value: &OptionValue) {
        let v = match *value {
            OptionValue::Float(f) => f,
            OptionValue::Int(i) => i as f32,
            OptionValue::Short(s) => s as f32,
            _ => return,
        };
        match id {
            opt::REMAINING | opt::HIT_POINTS => {
                self.hit_points = v.max(0.0);
                self.knocked_out = self.hit_points <= 0.0;
            }
            opt::BLOCK_LEFT => self.block = v.max(0.0),
            _ => {}
        }
    }

    fn get_option(&self, _world: &World, _me: Handle, id: u16) -> Option<OptionValue> {
        match id {
            opt::REMAINING => Some(OptionValue::Float(self.hit_points)),
            opt::BLOCK_LEFT => Some(OptionValue::Float(self.block)),
            _ => None,
        }
    }
}
