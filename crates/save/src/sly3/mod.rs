//! Sly 3 saving (`specs/sly3/saving.md`), on the Sly 2 code: the save-variable section has Sly 2's layout
//! ([`crate::sly2::VarTree`] reads it), the progress block is Sly 2's [`crate::sly2::Progress`] at Sly 3's size,
//! and slots are Sly 2's slot files with Sly 3's game byte and folder ([`SLOT`]).
//!
//! Every Sly 3 world carries the same section: 1,201 names, 6,559 records, a used size of 21,764 bytes and the
//! layout checksum 2,978,085 ([`USED_SIZE`], [`CHECKSUM`]). The block is 26,624 bytes ([`BLOCK_SIZE`]); a slot
//! whose check words differ from the running disc's is cleared, as in Sly 2.
//!
//! Files: `saves/sly3/slot1.sav` .. `slot3.sav`, with `.bak` copies.

use alloc::sync::Arc;

use platform_api::FileSystem;

use crate::sly2::slot::{SlotKind, Sly2Store};
use crate::sly2::{Progress, VarTree};

/// The container's game byte for Sly 3.
pub const GAME_SLY3: u8 = 3;
/// The slot folder.
pub const SAVE_DIR: &str = "saves/sly3";
/// Sly 3's slots.
pub const SLOT: SlotKind = SlotKind { game: GAME_SLY3, dir: SAVE_DIR };
/// The progress block (saving.md 4: `0x6800` bytes, of which the tree describes [`USED_SIZE`]).
pub const BLOCK_SIZE: usize = 0x6800;
/// The used size (the terminator's offset).
pub const USED_SIZE: u32 = 21_764;
/// The layout checksum.
pub const CHECKSUM: u32 = 2_978_085;

/// Sly 3 slot files (the Sly 2 store with Sly 3's kind).
pub type Sly3Store<F> = Sly2Store<F>;

/// The Sly 3 slot store over `fs`.
pub fn store<F: FileSystem>(fs: F) -> Sly3Store<F> {
    Sly2Store::with_kind(fs, SLOT)
}

/// A zeroed Sly 3 progress block for `tree`.
pub fn progress(tree: Arc<VarTree>) -> Progress {
    Progress::with_size(tree, BLOCK_SIZE)
}

/// Where a continued game starts (saving.md 3, interpretation): `widCheckpointForce` and `oidCheckpointForce`.
pub fn checkpoint(p: &Progress) -> (i32, i32) {
    (p.int(&["widCheckpointForce"]), p.int(&["oidCheckpointForce"]))
}

/// The coins (`loot/ccoin`).
pub fn coins(p: &Progress) -> i32 {
    p.int(&["loot", "ccoin"])
}

/// Whether item-table record `i` is owned (`powerUps/mppukfAcquired`, one bit per record).
pub fn owned(p: &Progress, i: usize) -> bool {
    p.flag_at(&["powerUps", "mppukfAcquired"], i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autosave::Slot;
    use crate::sly2::SaveValue;
    use crate::sly2::slot::{SLY2, decode_for, encode_for};
    use crate::sly2::tree::tests::sample;
    use crate::store::tests::MemFs;

    fn base() -> Progress {
        progress(Arc::new(VarTree::parse(&sample()).unwrap().0))
    }

    #[test]
    fn sly3_slots_are_their_own() {
        let mut p = base();
        assert_eq!(p.block.len(), BLOCK_SIZE);
        p.set_named(&["ccoin"], None, SaveValue::Int(77));
        p.set_named(&["dt"], None, SaveValue::Float(5.0));
        let file = encode_for(SLOT, &p, 3);
        let f = decode_for(SLOT, &file).unwrap();
        assert_eq!((f.serial, f.block.len()), (3, BLOCK_SIZE));
        assert!(decode_for(SLY2, &file).is_err(), "a Sly 3 slot is not a Sly 2 slot");
        let s = store(MemFs::default());
        let slot = Slot::new(0).unwrap();
        s.save(slot, &p).unwrap();
        let l = s.load(slot, &base()).unwrap();
        assert_eq!((l.progress.int(&["ccoin"]), l.cleared, l.progress.block.len()), (77, false, BLOCK_SIZE));
        assert_eq!(SLOT.slot_path(slot), "saves/sly3/slot1.sav");
    }
}
