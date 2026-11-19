//! Sly 1, 2 and 3 world scripts (specs/sly1/scripts.md, specs/sly2/scripts.md, specs/sly3/scripts.md). The [`Game`] of a [`Program`]
//! selects the cell numbering, the built-in operations, the option map layout and the custom-property range.
//!
//! | Module | What it does |
//! |---|---|
//! | [`tree`] | reads the script table's cells into a syntax tree ([`Program`]) |
//! | [`value`] | run-time values, the truth rule, "same value" and "equal" |
//! | [`interp`] | the [`Interpreter`]: modules, environments, special forms, calls, events, scheduled calls |
//! | [`builtins`] | the built-in operations |
//! | [`host`] | the engine boundary, [`ScriptHost`] |
//! | [`option_map`] | the per-class script option mapping read from the player's executable |
//! | [`sly2`] | Sly 2's built-in table and event signatures (from the executable) and its new operations |
//! | [`sly3`] | Sly 3's tables, its operation numbering (onto Sly 2's) and its new operations |
//! | [`event`] | the fixed event symbols 0 to 29 |
//! | [`machine`] | state machines (class 136) |
//!
//! `no_std` with `alloc`; no third-party crates. See `docs/script.md`.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod builtins;
mod env;
pub mod event;
pub mod host;
pub mod interp;
pub mod machine;
pub mod math;
pub mod option_map;
pub mod sly2;
pub mod sly3;
pub mod tree;
pub mod value;

pub use env::EnvRef;
pub use host::{NullHost, RaisedEvent, ScriptHost};
pub use interp::{Interpreter, Limits, ScheduledCall};
pub use machine::{MachineHost, MachineRun, StateMachine};
pub use option_map::{MapError, MapLayout, OptionMap};
pub use sly2::{BuiltinTable, EventSignatures};
pub use tree::{Game, Node, NodeId, ParseError, Program};
pub use value::{ObjectId, ScriptError, Value};

#[cfg(test)]
mod tests;
