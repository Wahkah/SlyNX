//! Sly 2 saving (`specs/sly2/saving.md`): the save-variable tree every world carries, the live progress block it
//! describes, and save slots in SlyNX's own container.
//!
//! - [`tree`]: the save-variable section (names, records, paths, the layout word and checksum);
//! - [`progress`]: the block and typed reads and writes by path, the new-game reset and the `from_memcard` marking;
//! - [`slot`]: the slot file format, the layout check on load, and the slot files in the user folder.

pub mod progress;
pub mod slot;
pub mod tree;

pub use progress::{Progress, SaveValue};
pub use slot::{Loaded, Sly2Store};
pub use tree::{BLOCK_SIZE, Kind, Var, VarTree};
