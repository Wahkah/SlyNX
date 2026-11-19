//! Golden (reference) images and tests that need the player's own disc.
//!
//! Images of the game are derived from the game's data, so they are **never committed**. Goldens live in a folder
//! outside the repository named by `SLYNX_GOLDEN_DIR`; the store refuses a folder inside the SlyNX checkout. Tests
//! that need goldens or a disc image skip (pass with a `skipped:` note on stderr) when the variable is not set.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `SLYNX_GOLDEN_DIR` | folder holding golden PNGs, outside the repository |
//! | `SLYNX_GOLDEN_UPDATE=1` | write the actual image as the new golden instead of comparing |
//! | `SLYNX_TEST_OUT` | where mismatch artefacts go (default `<golden dir>/_failures`) |
//! | `SLYNX_DISC_SLY1`, `SLYNX_DISC_SLY2`, `SLYNX_DISC_SLY3` | path to the player's disc image (or extracted folder) for each game |

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::compare::{CompareReport, Tolerance, compare_images, diff_image};
use crate::image::Image;
use crate::png;

/// Folder holding golden images.
pub const ENV_GOLDEN_DIR: &str = "SLYNX_GOLDEN_DIR";
/// Set to `1` to (re)write goldens from the actual images.
pub const ENV_GOLDEN_UPDATE: &str = "SLYNX_GOLDEN_UPDATE";
/// Folder for mismatch artefacts (actual and diff images).
pub const ENV_TEST_OUT: &str = "SLYNX_TEST_OUT";

/// A game whose disc a test may need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Sly1,
    Sly2,
    Sly3,
}

impl Game {
    /// The environment variable holding the path to this game's disc.
    pub const fn env_var(self) -> &'static str {
        match self {
            Game::Sly1 => "SLYNX_DISC_SLY1",
            Game::Sly2 => "SLYNX_DISC_SLY2",
            Game::Sly3 => "SLYNX_DISC_SLY3",
        }
    }
}

/// Interprets a disc variable's value: `Ok(path)` when it names something that exists, `Err(reason)` otherwise.
pub fn disc_path_from(value: Option<OsString>, var: &str) -> Result<PathBuf, String> {
    let Some(v) = value.filter(|v| !v.is_empty()) else {
        return Err(format!("{var} is not set"));
    };
    let p = PathBuf::from(v);
    if p.exists() { Ok(p) } else { Err(format!("{var} points to {}, which does not exist", p.display())) }
}

/// The disc path for `game`, or `None` (with a `skipped:` note on stderr naming `test`) when it is not available.
///
/// ```no_run
/// # use harness::golden::{Game, require_disc};
/// let Some(disc) = require_disc(Game::Sly1, "loads_first_level") else { return };
/// ```
pub fn require_disc(game: Game, test: &str) -> Option<PathBuf> {
    match disc_path_from(std::env::var_os(game.env_var()), game.env_var()) {
        Ok(p) => Some(p),
        Err(why) => {
            eprintln!("skipped: {test}: {why}");
            None
        }
    }
}

/// The workspace root this harness was built from.
fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().ok()
}

/// Whether `dir` is inside a SlyNX checkout: the one this harness was built from, or any folder whose ancestors
/// hold this workspace's `Cargo.toml`.
pub fn is_inside_repository(dir: &Path) -> bool {
    let Ok(dir) = dir.canonicalize() else { return false };
    if workspace_root().is_some_and(|root| dir.starts_with(root)) {
        return true;
    }
    dir.ancestors().any(|a| {
        std::fs::read_to_string(a.join("Cargo.toml"))
            .is_ok_and(|t| t.contains("[workspace]") && t.contains("crates/harness"))
    })
}

/// Checks a golden name: relative, `/`-separated, made of letters, digits, `_`, `-` and `.`, no `..`.
pub fn validate_name(name: &str) -> Result<(), String> {
    let ok_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'));
    let path = Path::new(name);
    let ok_parts = path.components().all(|c| matches!(c, Component::Normal(_)));
    if name.is_empty() || !ok_chars || !ok_parts || name.ends_with('/') {
        return Err(format!("bad golden name `{name}`: use a relative path of [A-Za-z0-9_.-/] without `..`"));
    }
    Ok(())
}

/// The outcome of [`GoldenStore::check`].
#[derive(Debug)]
pub enum GoldenOutcome {
    /// The image matches its golden.
    Matched(CompareReport),
    /// Update mode: the golden was written from the image.
    Updated(PathBuf),
    /// No golden exists for this name (and not in update mode).
    Missing(PathBuf),
    /// The image does not match; the actual and diff images were written next to each other.
    Mismatch { report: CompareReport, golden: PathBuf, actual: PathBuf, diff: PathBuf },
}

impl GoldenOutcome {
    /// Whether the check passed (matched or updated).
    pub fn passed(&self) -> bool {
        matches!(self, GoldenOutcome::Matched(_) | GoldenOutcome::Updated(_))
    }

    /// A human-readable description.
    pub fn describe(&self) -> String {
        match self {
            GoldenOutcome::Matched(_) => "matches golden".into(),
            GoldenOutcome::Updated(p) => format!("golden written to {}", p.display()),
            GoldenOutcome::Missing(p) => {
                format!("no golden at {} (run with {ENV_GOLDEN_UPDATE}=1 to create it)", p.display())
            }
            GoldenOutcome::Mismatch { report, golden, actual, diff } => format!(
                "differs from {}\nactual: {}\ndiff: {}\n{}",
                golden.display(),
                actual.display(),
                diff.display(),
                report.to_text()
            ),
        }
    }
}

/// A folder of golden images outside the repository.
#[derive(Clone, Debug)]
pub struct GoldenStore {
    dir: PathBuf,
    out_dir: PathBuf,
    update: bool,
}

impl GoldenStore {
    /// Opens a store. In update mode the folder is created if needed; otherwise it must exist. Fails if the folder
    /// (or the output folder) is inside the SlyNX repository.
    pub fn open(dir: impl Into<PathBuf>, update: bool, out_dir: Option<PathBuf>) -> Result<GoldenStore, String> {
        let dir = dir.into();
        if update {
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if !dir.is_dir() {
            return Err(format!("golden folder {} does not exist", dir.display()));
        }
        if is_inside_repository(&dir) {
            return Err(format!(
                "golden folder {} is inside the SlyNX repository; game-derived images must stay outside it",
                dir.display()
            ));
        }
        let out_dir = out_dir.unwrap_or_else(|| dir.join("_failures"));
        if let Some(parent) = out_dir.ancestors().find(|a| a.exists())
            && is_inside_repository(parent)
        {
            return Err(format!("output folder {} is inside the SlyNX repository", out_dir.display()));
        }
        Ok(GoldenStore { dir, out_dir, update })
    }

    /// Opens the store named by the environment. `Ok(None)` when `SLYNX_GOLDEN_DIR` is not set.
    pub fn from_env() -> Result<Option<GoldenStore>, String> {
        let Some(dir) = std::env::var_os(ENV_GOLDEN_DIR).filter(|v| !v.is_empty()) else {
            return Ok(None);
        };
        let update = std::env::var(ENV_GOLDEN_UPDATE).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let out = std::env::var_os(ENV_TEST_OUT).filter(|v| !v.is_empty()).map(PathBuf::from);
        GoldenStore::open(dir, update, out).map(Some)
    }

    /// The folder of goldens.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Whether the store writes goldens instead of comparing.
    pub fn is_update_mode(&self) -> bool {
        self.update
    }

    /// The path of a golden.
    pub fn path(&self, name: &str) -> Result<PathBuf, String> {
        validate_name(name)?;
        Ok(self.dir.join(format!("{name}.png")))
    }

    /// Loads a golden.
    pub fn load(&self, name: &str) -> Result<Option<Image>, String> {
        let p = self.path(name)?;
        if !p.exists() {
            return Ok(None);
        }
        png::read_file(&p).map(Some).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Compares `actual` with the golden `name` (or writes it, in update mode). On a mismatch the actual image and
    /// a diff image are written to the output folder as `<name>.actual.png` and `<name>.diff.png`.
    pub fn check(&self, name: &str, actual: &Image, tol: &Tolerance) -> Result<GoldenOutcome, String> {
        let golden_path = self.path(name)?;
        if self.update {
            png::write_file(&golden_path, actual).map_err(|e| format!("{}: {e}", golden_path.display()))?;
            return Ok(GoldenOutcome::Updated(golden_path));
        }
        let Some(golden) = self.load(name)? else {
            return Ok(GoldenOutcome::Missing(golden_path));
        };
        let report = compare_images(&golden, actual, tol);
        if report.passed {
            return Ok(GoldenOutcome::Matched(report));
        }
        let actual_path = self.out_dir.join(format!("{name}.actual.png"));
        let diff_path = self.out_dir.join(format!("{name}.diff.png"));
        png::write_file(&actual_path, actual).map_err(|e| format!("{}: {e}", actual_path.display()))?;
        png::write_file(&diff_path, &diff_image(&golden, actual, tol))
            .map_err(|e| format!("{}: {e}", diff_path.display()))?;
        Ok(GoldenOutcome::Mismatch { report, golden: golden_path, actual: actual_path, diff: diff_path })
    }
}

/// For tests: compares `actual` with the golden `name` from the environment's store. Skips (returns with a
/// `skipped:` note) when `SLYNX_GOLDEN_DIR` is not set; panics on a mismatch, a missing golden or a bad store.
pub fn assert_golden(name: &str, actual: &Image, tol: &Tolerance) {
    let store = match GoldenStore::from_env() {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("skipped: golden `{name}`: {ENV_GOLDEN_DIR} is not set");
            return;
        }
        Err(e) => panic!("golden store: {e}"),
    };
    match store.check(name, actual, tol) {
        Ok(o) if o.passed() => {
            if let GoldenOutcome::Updated(p) = &o {
                eprintln!("golden `{name}` written to {}", p.display());
            }
        }
        Ok(o) => panic!("golden `{name}`: {}", o.describe()),
        Err(e) => panic!("golden `{name}`: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("slynx-harness-golden-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn img(v: u8) -> Image {
        let mut i = Image::new(6, 4, [v, v, v, 255]);
        i.set(2, 1, [255, 0, 0, 255]);
        i
    }

    #[test]
    fn names_are_validated() {
        for good in ["a", "sly1/level-01.start", "x_y"] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", "../x", "a/../b", "/abs", "a b", "a\\b", "c:x", "a/"] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn disc_variable_handling() {
        assert!(disc_path_from(None, "V").unwrap_err().contains("not set"));
        assert!(disc_path_from(Some("".into()), "V").is_err());
        assert!(disc_path_from(Some("/definitely/not/here.iso".into()), "V").unwrap_err().contains("does not exist"));
        let here = std::env::temp_dir();
        assert_eq!(disc_path_from(Some(here.clone().into_os_string()), "V").unwrap(), here);
        assert_eq!(Game::Sly2.env_var(), "SLYNX_DISC_SLY2");
    }

    #[test]
    fn refuses_folders_inside_the_repository() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert!(is_inside_repository(&inside));
        let e = GoldenStore::open(&inside, false, None).unwrap_err();
        assert!(e.contains("inside the SlyNX repository"));
        let outside = temp_dir("outside");
        std::fs::create_dir_all(&outside).unwrap();
        assert!(!is_inside_repository(&outside));
        let e = GoldenStore::open(&outside, false, Some(inside.join("out"))).unwrap_err();
        assert!(e.contains("output folder"));
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn update_match_and_mismatch() {
        let dir = temp_dir("store");
        assert!(GoldenStore::open(&dir, false, None).is_err(), "missing folder without update mode");

        let writer = GoldenStore::open(&dir, true, None).unwrap();
        assert!(writer.is_update_mode());
        let o = writer.check("sly1/test-scene", &img(40), &Tolerance::exact()).unwrap();
        assert!(matches!(o, GoldenOutcome::Updated(_)) && o.passed());

        let store = GoldenStore::open(&dir, false, None).unwrap();
        let o = store.check("sly1/test-scene", &img(40), &Tolerance::exact()).unwrap();
        assert!(matches!(o, GoldenOutcome::Matched(_)));

        let o = store.check("sly1/other", &img(40), &Tolerance::exact()).unwrap();
        assert!(matches!(o, GoldenOutcome::Missing(_)) && !o.passed());
        assert!(o.describe().contains(ENV_GOLDEN_UPDATE));

        let o = store.check("sly1/test-scene", &img(41), &Tolerance::exact()).unwrap();
        match &o {
            GoldenOutcome::Mismatch { report, actual, diff, .. } => {
                assert_eq!(report.diff_pixels, 23);
                assert_eq!(png::read_file(actual).unwrap(), img(41));
                assert!(diff.exists());
                assert!(actual.starts_with(dir.join("_failures")));
            }
            other => panic!("expected a mismatch, got {other:?}"),
        }
        let within = Tolerance { max_channel_delta: 1, ..Tolerance::exact() };
        assert!(store.check("sly1/test-scene", &img(41), &within).unwrap().passed());
        assert!(store.check("../escape", &img(41), &within).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
