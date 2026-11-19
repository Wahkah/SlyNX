//! The `harness` command-line tool with the built-in apps. See `docs/testing.md`.

fn main() {
    std::process::exit(harness::cli::main_with(&harness::apps::AppRegistry::builtin()));
}
