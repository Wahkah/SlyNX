//! Runtime objects: handles, the class table and option values.
//!
//! An object is a slot in the [`World`](crate::world::World)'s columns (class, OID, transforms, flags, options,
//! behaviour). Everything outside the world names an object by a [`Handle`]: a slot index and a generation, so a
//! handle to a removed object never reaches the object that later reuses its slot.
//!
//! - [`class`]: the 147 class IDs, their body layout and our descriptive names (object-stream.md section 6);
//! - [`options`]: option values and the per-object option store (object-stream.md 4, options-by-class.md).

pub mod class;
pub mod options;

pub use class::{ClassInfo, class_info};
pub use options::{Arg, OptionStore, OptionValue};

use script::ObjectId;

/// Generational handle to an object of a [`World`](crate::world::World).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    index: u32,
    generation: u32,
}

/// Bits of a script [`ObjectId`] that hold the slot index; the rest hold the low bits of the generation.
const SCRIPT_INDEX_BITS: u32 = 20;
const SCRIPT_INDEX_MASK: u32 = (1 << SCRIPT_INDEX_BITS) - 1;

/// The script object ID of the world object (operation 108). It is not a slot.
pub const WORLD_SCRIPT_ID: ObjectId = ObjectId(u32::MAX);

/// Most slots a world can hold: the script ID packs the index into 20 bits.
pub const MAX_OBJECTS: u32 = SCRIPT_INDEX_MASK;

impl Handle {
    /// A handle from its parts (used by the world; elsewhere handles come from the world).
    pub(crate) const fn new(index: u32, generation: u32) -> Handle {
        Handle { index, generation }
    }

    /// The slot index.
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The slot's generation when the handle was made.
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// The script crate's name for this object: the slot index and the low 12 bits of the generation. A handle
    /// that outlives 4,096 reuses of its slot can alias; scripts never keep objects that long in practice.
    pub fn to_script(self) -> ObjectId {
        ObjectId(((self.generation & 0xFFF) << SCRIPT_INDEX_BITS) | (self.index & SCRIPT_INDEX_MASK))
    }

    /// The slot index and generation bits of a script object ID (`None` for the world object).
    pub fn split_script(id: ObjectId) -> Option<(u32, u32)> {
        if id == WORLD_SCRIPT_ID {
            return None;
        }
        Some((id.0 & SCRIPT_INDEX_MASK, id.0 >> SCRIPT_INDEX_BITS))
    }
}

/// Per-object state bits (`World::flags`).
pub mod flags {
    /// The object is part of the world (option 2 and operation "remove" clear it; scripts can add it back).
    pub const IN_WORLD: u16 = 1 << 0;
    /// Hidden (option 125, warp hidden-object lists): not drawn, still updates.
    pub const HIDDEN: u16 = 1 << 1;
    /// Runs on the real clock (option 119).
    pub const REAL_CLOCK: u16 = 1 << 2;
    /// Its world transform must be recomputed (with its descendants).
    pub const DIRTY: u16 = 1 << 3;
    /// Computed: in the world, and every ancestor is too.
    pub const ACTIVE: u16 = 1 << 4;
    /// Computed: active, not hidden (nor any ancestor), and placed (no "not placed" sentinel position).
    pub const VISIBLE: u16 = 1 << 5;
    /// Its stored rotation was degenerate and replaced by the identity (classes/placed-object.md 1.1).
    pub const DEGENERATE: u16 = 1 << 6;
    /// Made by a proxy back-reference (a clone).
    pub const CLONE: u16 = 1 << 7;
    /// Spawned from a stock template at run time.
    pub const SPAWNED: u16 = 1 << 8;
    /// Computed: the world position holds the "not placed" sentinel, or an ancestor's does.
    pub const UNPLACED: u16 = 1 << 9;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_ids_round_trip() {
        let h = Handle::new(12345, 7);
        let id = h.to_script();
        assert_eq!(Handle::split_script(id), Some((12345, 7)));
        assert_eq!(Handle::split_script(WORLD_SCRIPT_ID), None);
        // Generations wrap at 12 bits.
        let h = Handle::new(5, 0x1003);
        assert_eq!(Handle::split_script(h.to_script()), Some((5, 3)));
    }
}
