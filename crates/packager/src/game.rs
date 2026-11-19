//! Which disc is this? The packager identifies a disc image by the SHA-256 of its boot executable (named on the
//! `BOOT2` line of `SYSTEM.CNF`). Only hashes are stored here, never game data.

use formats::hash::{hex, sha256};
use formats::iso::{self, ReadAt};
use std::fmt;
use std::path::Path;

/// A game the packager knows about.
#[derive(Debug, PartialEq, Eq)]
pub struct KnownGame {
    /// Short id used in file names and the package manifest.
    pub id: &'static str,
    /// Human-readable name of the release, to tell the player which disc was found.
    pub release: &'static str,
    /// 1, 2 or 3: the number on the default icon.
    pub number: u8,
    /// The boot executable's file name on the disc.
    pub boot_file: &'static str,
    /// SHA-256 of the boot executable, when the specs for this release exist.
    pub executable_sha256: Option<[u8; 32]>,
    /// Whether SlyNX can run this release yet.
    pub supported: bool,
    /// Default title in the Homebrew Menu.
    pub title: &'static str,
    /// Name of the disc image inside the package (RomFS) or next to it on the SD card. The game looks for it there.
    pub image_name: &'static str,
    /// Base name of the `.nro`.
    pub nro_name: &'static str,
}

/// Known releases. Sly 1 and Sly 2 are identified by their executables' SHA-256 (specs/sly1/disc.md 1,
/// specs/sly2/disc.md 1). Sly 3 gets its hash when its disc spec is written; until then a disc whose boot file name
/// matches is reported as "recognised, not supported yet".
pub const GAMES: &[KnownGame] = &[
    KnownGame {
        id: "sly1-usa",
        release: "Sly Cooper and the Thievius Raccoonus (USA)",
        number: 1,
        boot_file: "SCUS_971.98",
        executable_sha256: Some(formats::disc::EXECUTABLE_SHA256),
        supported: true,
        title: "SlyNX 1",
        image_name: "sly1.iso",
        nro_name: "slynx-sly1",
    },
    KnownGame {
        id: "sly2-usa",
        release: "Sly 2: Band of Thieves (USA)",
        number: 2,
        boot_file: "SCUS_973.16",
        executable_sha256: Some(formats_sly2::exe::EXECUTABLE_SHA256),
        supported: true,
        title: "SlyNX 2",
        image_name: "sly2.iso",
        nro_name: "slynx-sly2",
    },
    KnownGame {
        id: "sly3-usa",
        release: "Sly 3: Honor Among Thieves (USA)",
        number: 3,
        boot_file: "SCUS_974.64",
        executable_sha256: Some(formats_sly3::exe::EXECUTABLE_SHA256),
        supported: true,
        title: "SlyNX 3",
        image_name: "sly3.iso",
        nro_name: "slynx-sly3",
    },
];

/// What [`identify`] found.
#[derive(Debug)]
pub enum Identity {
    /// A known, supported release.
    Supported { game: &'static KnownGame, executable_sha256: [u8; 32] },
    /// A known release SlyNX cannot run yet (or a known boot file with a hash we have not verified).
    NotSupported { game: &'static KnownGame, executable_sha256: [u8; 32] },
    /// Not a disc we know.
    Unknown { boot_file: String, executable_sha256: [u8; 32] },
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identity::Supported { game, .. } => write!(f, "{} (supported)", game.release),
            Identity::NotSupported { game, executable_sha256 } => {
                if game.executable_sha256.is_some_and(|h| h != *executable_sha256) {
                    write!(
                        f,
                        "a different revision of {} (executable SHA-256 {})",
                        game.release,
                        hex(executable_sha256)
                    )
                } else {
                    write!(f, "{} (recognised, not supported yet)", game.release)
                }
            }
            Identity::Unknown { boot_file, executable_sha256 } => {
                write!(f, "an unknown disc (boot file {boot_file}, SHA-256 {})", hex(executable_sha256))
            }
        }
    }
}

/// Looks up a boot file name and executable hash in [`GAMES`].
pub fn classify(boot_file: &str, executable_sha256: [u8; 32]) -> Identity {
    if let Some(game) = GAMES.iter().find(|g| g.executable_sha256 == Some(executable_sha256)) {
        return if game.supported {
            Identity::Supported { game, executable_sha256 }
        } else {
            Identity::NotSupported { game, executable_sha256 }
        };
    }
    match GAMES.iter().find(|g| g.boot_file.eq_ignore_ascii_case(boot_file)) {
        Some(game) => Identity::NotSupported { game, executable_sha256 },
        None => Identity::Unknown { boot_file: boot_file.to_string(), executable_sha256 },
    }
}

/// Reads `SYSTEM.CNF` and the boot executable from an ISO image and identifies it.
pub fn identify(src: &mut impl ReadAt) -> Result<Identity, String> {
    iso::read_volume(src).map_err(|e| format!("not a PS2 DVD image: {e}"))?;
    let cnf = iso::find_root_file(src, "SYSTEM.CNF").map_err(|e| format!("not a PS2 disc: {e}"))?;
    let cnf = iso::read_file(src, &cnf).map_err(|e| e.to_string())?;
    let boot = formats::disc::boot_executable_name(&cnf).map_err(|e| e.to_string())?;
    let exe = iso::find_root_file(src, &boot).map_err(|e| format!("boot file {boot}: {e}"))?;
    let exe = iso::read_file(src, &exe).map_err(|e| e.to_string())?;
    Ok(classify(&boot, sha256(&exe)))
}

/// Opens and identifies a disc image file.
pub fn identify_path(path: &Path) -> Result<Identity, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    identify(&mut f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        let sly1 = formats::disc::EXECUTABLE_SHA256;
        assert!(matches!(classify("SCUS_971.98", sly1), Identity::Supported { game, .. } if game.number == 1));
        assert!(matches!(classify("whatever", sly1), Identity::Supported { .. }), "the hash decides");
        let other = [7u8; 32];
        assert!(matches!(classify("SCUS_971.98", other), Identity::NotSupported { game, .. } if game.number == 1));
        assert!(matches!(classify("scus_973.16", other), Identity::NotSupported { game, .. } if game.number == 2));
        let sly2 = formats_sly2::exe::EXECUTABLE_SHA256;
        assert!(
            matches!(classify("SCUS_973.16", sly2), Identity::Supported { game, .. } if game.number == 2 && game.image_name == "sly2.iso")
        );
        assert!(matches!(classify("SCUS_974.64", other), Identity::NotSupported { game, .. } if game.number == 3));
        let sly3 = formats_sly3::exe::EXECUTABLE_SHA256;
        assert!(
            matches!(classify("SCUS_974.64", sly3), Identity::Supported { game, .. } if game.number == 3 && game.image_name == "sly3.iso")
        );
        assert!(matches!(classify("SLES_123.45", other), Identity::Unknown { .. }));
        let text = classify("SCUS_971.98", other).to_string();
        assert!(text.contains("different revision"), "{text}");
    }

    #[test]
    fn table_is_consistent() {
        for (i, g) in GAMES.iter().enumerate() {
            assert_eq!(g.number as usize, i + 1);
            assert!(!g.supported || g.executable_sha256.is_some());
            assert!(g.image_name.ends_with(".iso"));
        }
    }

    #[test]
    fn not_an_iso() {
        let data = vec![0u8; 64 * 1024];
        assert!(identify(&mut &data[..]).is_err());
    }
}
