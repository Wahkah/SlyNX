//! The SlyNX save system for Sly 1 (`specs/sly1/save-format.md`, `pickups-and-progress.md` 6 and 7), and Sly 2's
//! save-variable tree and slots ([`sly2`]).
//!
//! - [`model`]: what a slot holds, its ranges, and the progress and restore rules the engine applies;
//! - [`completion`]: percent complete and the slot description text;
//! - [`autosave`]: the game in progress ([`autosave::Session`]) and when it saves;
//! - [`slotfile`], [`settings`], [`codec`]: SlyNX's own versioned, checksummed file formats;
//! - [`store`]: the files in the user folder, through `platform_api::FileSystem`.
//!
//! `no_std` + `alloc`; builds for the Switch. See `docs/save.md`.
#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod autosave;
pub mod codec;
pub mod completion;
pub mod model;
pub mod settings;
pub mod slotfile;
pub mod sly2; // Sly 2 hook: Sly 2 saving (sly2/).
pub mod sly3; // Sly 3 hook: Sly 3 saving, on the Sly 2 code (sly3/).
pub mod store;

pub use autosave::{SaveRequest, Session, Slot, Trigger};
pub use completion::{CompletionTable, SlotSummary};
pub use model::{Settings, SlotState};
pub use settings::PortSettings;
pub use store::{LoadError, Loaded, SaveStore};
