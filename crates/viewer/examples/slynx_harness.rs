//! The harness command-line tool with the viewer registered as `--app viewer`.
//!
//! ```text
//! cargo run --release -p viewer --example slynx_harness -- run --app viewer --opt world=Paris \
//!     --frames 120 --capture 60 --out DIR
//! ```
//!
//! Every other command (`compare`, `perf`, `list`) is the harness's own; see `docs/testing.md`.

fn main() {
    std::process::exit(harness::cli::main_with(&viewer::pc::registry()));
}
