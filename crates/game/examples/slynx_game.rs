//! The harness's command-line tool with the game registered as `--app sly1` (OpenGL) and `--app sly1-cpu` (no GPU).
//!
//! ```text
//! cargo run --release -p game --example slynx_game -- run --app sly1-cpu --opt world=Paris --frames 600 \
//!     --out DIR --check-determinism
//! ```
//!
//! Every other command (`compare`, `perf`, `list`) is the harness's own; see `docs/testing.md`.

fn main() {
    std::process::exit(harness::cli::main_with(&game::pc::registry()));
}
