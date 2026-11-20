//! Sly 2 save slots in SlyNX's own container ([`crate::codec`]), keeping the game's slot semantics (saving.md 3
//! and 4): a slot is the progress block plus the two check words, and a slot whose check words do not match the
//! layout of the running disc is cleared. Memory-card files are not written (their layout cannot be checked).
//!
//! Format version 1 sections:
//!
//! | Tag | Content |
//! |---|---|
//! | 1, block | u32 layout word, u32 layout checksum, u32 block length n, n bytes (the progress block) |
//! | 2, order | u64 save serial (the newest slot has the largest; stands in for the card file's modification time) |
//!
//! Files: `saves/sly2/slot1.sav` .. `slot3.sav`, with `.bak` copies of the previous good save, as for Sly 1.
//!
//! Sly 3 slots use the same format and code with their own game byte and folder ([`SlotKind`],
//! `crate::sly3`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use platform_api::{FileSystem, FsError, Root};

use super::progress::Progress;
use crate::autosave::Slot;
use crate::codec::{self, DecodeError, Reader, SLOT_MAGIC, Writer};
use crate::store::LoadError;

/// The container's game byte for Sly 2.
pub const GAME_SLY2: u8 = 2;
/// The format this build writes and the newest it reads fully.
pub const FORMAT_VERSION: u16 = 1;
/// The oldest reader that can read what this build writes.
pub const OLDEST_READER: u16 = 1;
/// The slot folder.
pub const SAVE_DIR: &str = "saves/sly2";

/// Which game's slots: the container's game byte and the slot folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotKind {
    /// The container's game byte.
    pub game: u8,
    /// The slot folder.
    pub dir: &'static str,
}

/// Sly 2's slots.
pub const SLY2: SlotKind = SlotKind { game: GAME_SLY2, dir: SAVE_DIR };

impl SlotKind {
    /// The slot file path.
    pub fn slot_path(self, slot: Slot) -> String {
        format!("{}/slot{}.sav", self.dir, slot.index() + 1)
    }

    /// The backup path.
    pub fn backup_path(self, slot: Slot) -> String {
        format!("{}/slot{}.bak", self.dir, slot.index() + 1)
    }
}

const TAG_BLOCK: u16 = 1;
const TAG_ORDER: u16 = 2;

/// A decoded slot file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotFile {
    /// The layout word and checksum written with the block.
    pub check: (u32, u32),
    /// The progress block.
    pub block: Vec<u8>,
    /// The save serial.
    pub serial: u64,
}

/// Encodes a Sly 2 slot: `progress`'s block with every task's `from_memcard` flag set in the written copy.
pub fn encode(progress: &Progress, serial: u64) -> Vec<u8> {
    encode_for(SLY2, progress, serial)
}

/// Encodes a slot of `kind`'s game.
pub fn encode_for(kind: SlotKind, progress: &Progress, serial: u64) -> Vec<u8> {
    let mut copy = progress.clone();
    copy.mark_from_memcard();
    let mut w = Writer::default();
    w.section(TAG_BLOCK, |w| {
        w.u32(copy.tree.layout_word());
        w.u32(copy.tree.checksum());
        w.u32(copy.block.len() as u32);
        w.buf.extend_from_slice(&copy.block);
    });
    w.section(TAG_ORDER, |w| w.u64(serial));
    codec::seal(SLOT_MAGIC, FORMAT_VERSION, OLDEST_READER, kind.game, &w.buf)
}

/// Decodes a Sly 2 slot file.
pub fn decode(data: &[u8]) -> Result<SlotFile, DecodeError> {
    decode_for(SLY2, data)
}

/// Decodes a slot file of `kind`'s game.
pub fn decode_for(kind: SlotKind, data: &[u8]) -> Result<SlotFile, DecodeError> {
    let opened = codec::open(SLOT_MAGIC, FORMAT_VERSION, kind.game, data)?;
    let mut out = SlotFile { check: (0, 0), block: Vec::new(), serial: 0 };
    let mut have_block = false;
    for s in codec::sections(opened.payload) {
        let (tag, body) = s?;
        let mut r = Reader::new(body);
        match tag {
            TAG_BLOCK => {
                let (word, sum, n) = (r.u32(), r.u32(), r.u32());
                let (Some(word), Some(sum), Some(n)) = (word, sum, n) else { return Err(DecodeError::Truncated) };
                let bytes = body.get(12..12 + n as usize).ok_or(DecodeError::Truncated)?;
                out.check = (word, sum);
                out.block = bytes.to_vec();
                have_block = true;
            }
            TAG_ORDER => out.serial = r.u64().ok_or(DecodeError::Truncated)?,
            _ => {}
        }
    }
    if !have_block {
        return Err(DecodeError::Truncated);
    }
    Ok(out)
}

/// A loaded slot, applied to the running disc's layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// The progress.
    pub progress: Progress,
    /// The check words did not match this disc's layout, so the block was cleared (saving.md 3).
    pub cleared: bool,
    /// The slot is unused (`dt` is 0): reset, keeping the options.
    pub unused: bool,
    /// The save serial.
    pub serial: u64,
    /// Loaded from the backup because the slot file was damaged.
    pub from_backup: bool,
}

/// Puts a decoded slot onto `base` (a progress with this disc's tree): the layout check, then the unused-slot reset.
pub fn apply(base: &Progress, file: &SlotFile) -> (Progress, bool, bool) {
    let mut p = base.clone();
    let matches = file.check == (p.tree.layout_word(), p.tree.checksum());
    if matches {
        let n = file.block.len().min(p.block.len());
        p.block.fill(0);
        p.block[..n].copy_from_slice(&file.block[..n]);
    } else {
        p.block.fill(0);
    }
    let unused = p.is_unused();
    if unused {
        p.reset_keeping_options();
    }
    (p, !matches, unused)
}

/// The Sly 2 slot files in the user folder (or another game's, [`Sly2Store::with_kind`]).
#[derive(Clone, Debug)]
pub struct Sly2Store<F: FileSystem> {
    fs: F,
    kind: SlotKind,
}

/// The Sly 2 slot file path.
pub fn slot_path(slot: Slot) -> String {
    SLY2.slot_path(slot)
}

/// The Sly 2 backup path.
pub fn backup_path(slot: Slot) -> String {
    SLY2.backup_path(slot)
}

impl<F: FileSystem> Sly2Store<F> {
    /// The Sly 2 store over `fs`.
    pub fn new(fs: F) -> Sly2Store<F> {
        Sly2Store::with_kind(fs, SLY2)
    }

    /// The store of `kind`'s game over `fs`.
    pub fn with_kind(fs: F, kind: SlotKind) -> Sly2Store<F> {
        Sly2Store { fs, kind }
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, FsError> {
        match self.fs.read(Root::User, path) {
            Ok(b) => Ok(Some(b)),
            Err(FsError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The decoded slot file, falling back to the backup when the slot is damaged. The bool is "from the backup".
    pub fn load_file(&self, slot: Slot) -> Result<(SlotFile, bool), LoadError> {
        let main = self.read(&self.kind.slot_path(slot)).map_err(LoadError::Io)?;
        let main_err = match &main {
            Some(b) => match decode_for(self.kind, b) {
                Ok(f) => return Ok((f, false)),
                Err(e @ DecodeError::TooNew { .. }) => return Err(LoadError::TooNew(e)),
                Err(e) => Some(e),
            },
            None => None,
        };
        match self.read(&self.kind.backup_path(slot)).map_err(LoadError::Io)? {
            Some(b) => {
                decode_for(self.kind, &b).map(|f| (f, true)).map_err(|e| LoadError::Damaged(main_err.unwrap_or(e)))
            }
            None => Err(main_err.map_or(LoadError::Empty, LoadError::Damaged)),
        }
    }

    /// Loads a slot onto `base` (this disc's tree).
    pub fn load(&self, slot: Slot, base: &Progress) -> Result<Loaded, LoadError> {
        let (file, from_backup) = self.load_file(slot)?;
        let (progress, cleared, unused) = apply(base, &file);
        Ok(Loaded { progress, cleared, unused, serial: file.serial, from_backup })
    }

    /// Saves `progress` to `slot` with the next serial, keeping the previous good save as the backup. A serial
    /// already at `u64::MAX` (only a damaged or forged file) stays there rather than wrapping to 0.
    pub fn save(&self, slot: Slot, progress: &Progress) -> Result<u64, FsError> {
        let serial = self.newest().map_or(1, |(_, s)| s.saturating_add(1));
        if let Some(old) = self.read(&self.kind.slot_path(slot))?
            && decode_for(self.kind, &old).is_ok()
        {
            self.fs.write_user(&self.kind.backup_path(slot), &old)?;
        }
        self.fs.write_user(&self.kind.slot_path(slot), &encode_for(self.kind, progress, serial))?;
        Ok(serial)
    }

    /// Erases a slot and its backup.
    pub fn erase(&self, slot: Slot) -> Result<(), FsError> {
        self.fs.remove_user(&self.kind.slot_path(slot))?;
        self.fs.remove_user(&self.kind.backup_path(slot))
    }

    /// The slot to continue from: the readable slot saved last, with its serial (saving.md 3, "newest slot").
    pub fn newest(&self) -> Option<(Slot, u64)> {
        Slot::ALL.into_iter().filter_map(|s| self.load_file(s).ok().map(|(f, _)| (s, f.serial))).max_by_key(|x| x.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sly2::progress::SaveValue;
    use crate::sly2::tree::{BLOCK_SIZE, VarTree, tests::sample};
    use crate::store::tests::MemFs;
    use alloc::sync::Arc;

    fn base() -> Progress {
        Progress::new(Arc::new(VarTree::parse(&sample()).unwrap().0))
    }

    #[test]
    fn round_trip_and_from_memcard() {
        let mut p = base();
        p.set_named(&["ccoin"], None, SaveValue::Int(77));
        p.set_named(&["dt"], None, SaveValue::Float(12.0));
        let f = decode(&encode(&p, 5)).unwrap();
        assert_eq!((f.serial, f.block.len()), (5, BLOCK_SIZE));
        let (q, cleared, unused) = apply(&base(), &f);
        assert!(!cleared && !unused);
        assert_eq!(q.int(&["ccoin"]), 77);
        // The written copy has the task flags set; the live block does not.
        assert_eq!(q.get_named(&["t1", "from_memcard"], None), Some(SaveValue::Flag(true)));
        assert_eq!(p.get_named(&["t1", "from_memcard"], None), Some(SaveValue::Flag(false)));
    }

    #[test]
    fn layout_mismatch_clears_and_unused_resets() {
        let mut p = base();
        p.set_named(&["ccoin"], None, SaveValue::Int(77));
        p.set_named(&["flags", "lang"], None, SaveValue::Int(2));
        p.set_named(&["dt"], None, SaveValue::Float(1.0));
        let mut f = decode(&encode(&p, 1)).unwrap();
        f.check.1 ^= 1;
        let (q, cleared, _) = apply(&base(), &f);
        assert!(cleared);
        assert_eq!(q.int(&["ccoin"]), 0);
        // dt 0: unused, reset keeping the options.
        p.set_named(&["dt"], None, SaveValue::Float(0.0));
        let (q, cleared, unused) = apply(&base(), &decode(&encode(&p, 1)).unwrap());
        assert!(!cleared && unused);
        assert_eq!((q.int(&["ccoin"]), q.int(&["flags", "lang"])), (0, 2));
    }

    #[test]
    fn store_keeps_backups_and_finds_newest() {
        let store = Sly2Store::new(MemFs::default());
        let s1 = Slot::new(0).unwrap();
        let s2 = Slot::new(1).unwrap();
        assert!(matches!(store.load(s1, &base()), Err(LoadError::Empty)));
        let mut p = base();
        p.set_named(&["dt"], None, SaveValue::Float(3.0));
        p.set_named(&["ccoin"], None, SaveValue::Int(1));
        assert_eq!(store.save(s2, &p).unwrap(), 1);
        p.set_named(&["ccoin"], None, SaveValue::Int(2));
        assert_eq!(store.save(s1, &p).unwrap(), 2);
        assert_eq!(store.newest(), Some((s1, 2)));
        p.set_named(&["ccoin"], None, SaveValue::Int(3));
        store.save(s1, &p).unwrap();
        // Damage the slot: the backup (the previous save) loads.
        store.fs.write_user(&slot_path(s1), b"junk").unwrap();
        let l = store.load(s1, &base()).unwrap();
        assert!(l.from_backup);
        assert_eq!(l.progress.int(&["ccoin"]), 2);
        store.erase(s2).unwrap();
        assert!(matches!(store.load(s2, &base()), Err(LoadError::Empty)));
    }

    #[test]
    fn the_largest_serial_does_not_wrap() {
        let store = Sly2Store::new(MemFs::default());
        let s1 = Slot::new(0).unwrap();
        store.fs.write_user(&slot_path(s1), &encode(&base(), u64::MAX)).unwrap();
        assert_eq!(store.newest(), Some((s1, u64::MAX)));
        assert_eq!(store.save(Slot::new(1).unwrap(), &base()).unwrap(), u64::MAX);
    }
}
