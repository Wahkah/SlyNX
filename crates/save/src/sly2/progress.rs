//! The live progress block and its values (saving.md 1.2, 3).

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use super::tree::{BLOCK_SIZE, Kind, Var, VarTree};

/// A value read from or written to the block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SaveValue {
    /// Kinds 0, 2, 9, 10.
    Int(i32),
    /// Kind 3.
    Float(f32),
    /// Kind 4.
    Flag(bool),
}

impl SaveValue {
    /// The value as an integer (a flag is 0 or 1, a float is truncated).
    pub fn as_i32(self) -> i32 {
        match self {
            SaveValue::Int(v) => v,
            SaveValue::Float(f) => f as i32,
            SaveValue::Flag(b) => b as i32,
        }
    }

    /// The value as a float.
    pub fn as_f32(self) -> f32 {
        match self {
            SaveValue::Int(v) => v as f32,
            SaveValue::Float(f) => f,
            SaveValue::Flag(b) => b as i32 as f32,
        }
    }
}

/// The progress of one game: the tree that describes the block, and the block (saving.md 3, "live block").
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// The layout (the same in every world; shared).
    pub tree: Arc<VarTree>,
    /// The block ([`BLOCK_SIZE`] bytes for Sly 2).
    pub block: Vec<u8>,
}

impl Progress {
    /// A zeroed block for `tree`.
    pub fn new(tree: Arc<VarTree>) -> Progress {
        Progress::with_size(tree, BLOCK_SIZE)
    }

    /// A zeroed block of `size` bytes for `tree` (Sly 3's block is larger, `crate::sly3::BLOCK_SIZE`).
    pub fn with_size(tree: Arc<VarTree>, size: usize) -> Progress {
        Progress { tree, block: vec![0; size] }
    }

    fn word(&self, at: usize) -> Option<[u8; 4]> {
        self.block.get(at..at + 4)?.try_into().ok()
    }

    /// Reads a value.
    pub fn read(&self, v: Var) -> Option<SaveValue> {
        let base = v.offset as usize;
        match (v.kind, v.index) {
            (Kind::Flag, Some(i)) => {
                let b = *self.block.get(base + i as usize / 8)?;
                Some(SaveValue::Flag(b >> (i % 8) & 1 != 0))
            }
            (k, i) => {
                let w = self.word(base + 4 * i.unwrap_or(0) as usize)?;
                match k {
                    Kind::Flag => Some(SaveValue::Flag(u32::from_le_bytes(w) != 0)),
                    Kind::Float => Some(SaveValue::Float(f32::from_le_bytes(w))),
                    k if k.is_integer() => Some(SaveValue::Int(i32::from_le_bytes(w))),
                    _ => None,
                }
            }
        }
    }

    /// Writes a value, converted to the variable's kind. Returns whether it was written.
    pub fn write(&mut self, v: Var, value: SaveValue) -> bool {
        let base = v.offset as usize;
        match (v.kind, v.index) {
            (Kind::Flag, Some(i)) => {
                let Some(b) = self.block.get_mut(base + i as usize / 8) else { return false };
                let bit = 1u8 << (i % 8);
                if value.as_i32() != 0 {
                    *b |= bit;
                } else {
                    *b &= !bit;
                }
                true
            }
            (k, i) => {
                let at = base + 4 * i.unwrap_or(0) as usize;
                let bytes = match k {
                    Kind::Flag => ((value.as_i32() != 0) as u32).to_le_bytes(),
                    Kind::Float => value.as_f32().to_le_bytes(),
                    k if k.is_integer() => value.as_i32().to_le_bytes(),
                    _ => return false,
                };
                match self.block.get_mut(at..at + 4) {
                    Some(dst) => {
                        dst.copy_from_slice(&bytes);
                        true
                    }
                    None => false,
                }
            }
        }
    }

    /// Reads the value at a path of name IDs (with the element index for arrays); `None` is "no value".
    pub fn get(&self, path: &[u16], index: Option<usize>) -> Option<SaveValue> {
        self.read(self.tree.var(path, index)?)
    }

    /// Writes the value at a path of name IDs.
    pub fn set(&mut self, path: &[u16], index: Option<usize>, value: SaveValue) -> bool {
        match self.tree.var(path, index) {
            Some(v) => self.write(v, value),
            None => false,
        }
    }

    /// Reads the value at a path of names.
    pub fn get_named(&self, path: &[&str], index: Option<usize>) -> Option<SaveValue> {
        self.read(self.tree.var_named(path, index)?)
    }

    /// Writes the value at a path of names.
    pub fn set_named(&mut self, path: &[&str], index: Option<usize>, value: SaveValue) -> bool {
        match self.tree.var_named(path, index) {
            Some(v) => self.write(v, value),
            None => false,
        }
    }

    /// An integer by names (0 when missing).
    pub fn int(&self, path: &[&str]) -> i32 {
        self.get_named(path, None).map_or(0, SaveValue::as_i32)
    }

    /// A float by names (0 when missing).
    pub fn float(&self, path: &[&str]) -> f32 {
        self.get_named(path, None).map_or(0.0, SaveValue::as_f32)
    }

    /// Element `i` of a flag array by names.
    pub fn flag_at(&self, path: &[&str], i: usize) -> bool {
        self.get_named(path, Some(i)).is_some_and(|v| v.as_i32() != 0)
    }

    /// Whether the slot is unused: its total play time `dt` is 0 (saving.md 3).
    pub fn is_unused(&self) -> bool {
        self.float(&["dt"]) == 0.0
    }

    /// Resets the block for a new game, keeping only the options (the top-level `flags` group, saving.md 3).
    pub fn reset_keeping_options(&mut self) {
        let mut kept: Vec<(usize, Vec<u8>)> = Vec::new();
        if let Some(g) = self.tree.top("flags") {
            for c in self.tree.children(g) {
                let r = self.tree.records()[c];
                let len = match (r.kind, r.count) {
                    (Kind::Flag, n) if n > 0 => (n as usize).div_ceil(8),
                    (_, n) => 4 * (n as usize).max(1),
                };
                let at = r.offset as usize;
                if let Some(b) = self.block.get(at..at + len) {
                    kept.push((at, b.to_vec()));
                }
            }
        }
        self.block.fill(0);
        for (at, b) in kept {
            self.block[at..at + b.len()].copy_from_slice(&b);
        }
    }

    /// Sets every task's `from_memcard` flag (the single flag of that name below each top-level group), as writing
    /// a slot does to the copy it writes (saving.md 3).
    pub fn mark_from_memcard(&mut self) {
        let Some(name) = self.tree.name_id("from_memcard") else { return };
        let tree = self.tree.clone();
        for top in tree.children(0) {
            for c in tree.children(top) {
                let r = tree.records()[c];
                if r.name == name as i16 && r.count == 0 {
                    self.write(Var { kind: r.kind, offset: r.offset, count: 0, index: None }, SaveValue::Flag(true));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sly2::tree::tests::sample;

    fn progress() -> Progress {
        Progress::new(Arc::new(VarTree::parse(&sample()).unwrap().0))
    }

    #[test]
    fn values_round_trip() {
        let mut p = progress();
        assert!(p.set_named(&["ccoin"], None, SaveValue::Int(1234)));
        assert_eq!(p.int(&["ccoin"]), 1234);
        assert!(p.set_named(&["owned"], Some(9), SaveValue::Flag(true)));
        assert!(p.flag_at(&["owned"], 9) && !p.flag_at(&["owned"], 8));
        // Element 9 of a bit array is bit 1 of byte 1.
        assert_eq!(p.block[8 + 1], 0b10);
        assert!(p.set_named(&["dt"], None, SaveValue::Float(2.5)));
        assert_eq!(p.float(&["dt"]), 2.5);
        assert!(!p.set_named(&["nothing"], None, SaveValue::Int(1)));
        assert_eq!(p.get_named(&["flags"], None), None);
    }

    #[test]
    fn new_game_keeps_options() {
        let mut p = progress();
        p.set_named(&["flags", "lang"], None, SaveValue::Int(3));
        p.set_named(&["ccoin"], None, SaveValue::Int(50));
        p.set_named(&["dt"], None, SaveValue::Float(9.0));
        assert!(!p.is_unused());
        p.reset_keeping_options();
        assert_eq!((p.int(&["flags", "lang"]), p.int(&["ccoin"])), (3, 0));
        assert!(p.is_unused());
        p.mark_from_memcard();
        assert_eq!(p.get_named(&["t1", "from_memcard"], None), Some(SaveValue::Flag(true)));
    }
}
