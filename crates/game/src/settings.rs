//! Where the disc is and which world to start in.
//!
//! On the Switch the disc image is `sdmc:/switch/SlyNX/sly1.iso`, or the path given in
//! `sdmc:/switch/SlyNX/user/game.cfg`, or `romfs:/sly1.iso` in a packaged build:
//!
//! ```text
//! # key = value, # starts a comment
//! disc = sdmc:/isos/Sly Cooper.iso
//! world = Paris                      # a name, part of a name, an id (0x103) or a table index
//! triggers = 1                       # the free camera triggers exits and volumes (default 1)
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use formats::disc::{Disc, WorldEntry};
use formats::iso::ReadAt;
use platform_api::{DataFile, FileSystem, Root};

/// The settings file, relative to the user folder.
pub const SETTINGS_FILE: &str = "game.cfg";
/// The world started when the settings name none.
pub const FIRST_WORLD: &str = "Paris";

/// Reads a disc image through the platform's file API.
pub struct FsReader<F: DataFile>(pub F);

impl<F: DataFile> ReadAt for FsReader<F> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> formats::Result<()> {
        self.0.read_exact_at(offset, buf).map_err(|e| formats::Error::Io(e.to_string()))
    }
}

/// The settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Disc image location: a root and a path below it.
    pub disc: Option<(Root, String)>,
    /// First world.
    pub world: Option<String>,
    /// The free camera triggers player-only objects.
    pub triggers: Option<bool>,
}

/// Parses `sdmc:/a/b.iso`, `romfs:/a.iso` or a plain SD-relative path.
pub fn parse_path(s: &str) -> Option<(Root, String)> {
    let s = s.trim();
    let (root, rest) = if let Some(r) = s.strip_prefix("sdmc:/") {
        (Root::Sd, r)
    } else if let Some(r) = s.strip_prefix("romfs:/") {
        (Root::RomFs, r)
    } else {
        (Root::Sd, s)
    };
    let rest = rest.trim_start_matches('/');
    platform_api::fs::validate(rest).ok()?;
    Some((root, String::from(rest)))
}

impl Settings {
    /// Parses `key = value` lines.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "disc" => s.disc = parse_path(v),
                "world" if !v.is_empty() => s.world = Some(String::from(v)),
                "triggers" => s.triggers = Some(v == "1" || v == "true"),
                _ => {}
            }
        }
        s
    }

    /// Reads the settings file from the user folder (defaults when it is missing).
    pub fn load<F: FileSystem>(fs: &F) -> Settings {
        let path = alloc::format!("{}{}", platform_api_user_dir(), SETTINGS_FILE);
        match fs.read(Root::Sd, &path) {
            Ok(b) => Settings::parse(&String::from_utf8_lossy(&b)),
            Err(_) => Settings::default(),
        }
    }
}

/// The user folder on the SD card.
fn platform_api_user_dir() -> &'static str {
    "switch/SlyNX/user/"
}

/// The disc locations tried, in order.
pub fn disc_candidates(settings: &Settings) -> Vec<(Root, String)> {
    let mut v = Vec::new();
    if let Some(d) = &settings.disc {
        v.push(d.clone());
    }
    v.push((Root::Sd, String::from("switch/SlyNX/sly1.iso")));
    v.push((Root::RomFs, String::from("sly1.iso")));
    v
}

/// Opens the first disc image found, or says which paths were tried.
pub fn open_disc_fs<F: FileSystem>(fs: &F, settings: &Settings) -> Result<Disc<FsReader<F::File>>, String> {
    let mut tried = Vec::new();
    for (root, path) in disc_candidates(settings) {
        if !fs.exists(root, &path) {
            tried.push(alloc::format!("{root:?}:/{path}"));
            continue;
        }
        let f = fs.open(root, &path).map_err(|e| alloc::format!("{path}: {e}"))?;
        return Disc::open(FsReader(f)).map_err(|e| alloc::format!("{path}: {e}"));
    }
    Err(alloc::format!("no disc image; tried {}", tried.join(", ")))
}

/// Opens the disc image the game uses again (the first location found), for streaming movies from it.
pub fn open_disc_reader<F: FileSystem>(fs: &F, settings: &Settings) -> Option<FsReader<F::File>> {
    disc_candidates(settings)
        .into_iter()
        .find(|(root, path)| fs.exists(*root, path))
        .and_then(|(root, path)| fs.open(root, &path).ok())
        .map(FsReader)
}

/// Finds a world by table index, id (`0x103`), name, or a case-insensitive part of the name (a name that starts with
/// the text wins over one that only contains it).
pub fn world_index(worlds: &[WorldEntry], which: &str) -> Option<usize> {
    let which = which.trim();
    if let Some(hex) = which.strip_prefix("0x")
        && let Ok(id) = u32::from_str_radix(hex, 16)
    {
        return worlds.iter().position(|w| w.id == id);
    }
    if let Ok(i) = which.parse::<usize>() {
        return (i < worlds.len()).then_some(i);
    }
    if which.is_empty() {
        return None;
    }
    let want = which.to_ascii_lowercase();
    let names: Vec<String> = worlds.iter().map(|w| w.name.to_ascii_lowercase()).collect();
    names
        .iter()
        .position(|n| *n == want)
        .or_else(|| names.iter().position(|n| n.starts_with(&want)))
        .or_else(|| names.iter().position(|n| n.contains(&want)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_paths() {
        let s = Settings::parse("# c\n disc = sdmc:/isos/sly.iso\nworld = The Hideout # here\ntriggers=0\n");
        assert_eq!(s.disc, Some((Root::Sd, "isos/sly.iso".into())));
        assert_eq!(s.world.as_deref(), Some("The Hideout"));
        assert_eq!(s.triggers, Some(false));
        assert_eq!(parse_path("romfs:/a.iso"), Some((Root::RomFs, "a.iso".into())));
        assert_eq!(parse_path("sdmc:/../x.iso"), None);
        assert_eq!(disc_candidates(&Settings::default()).len(), 2);
        assert_eq!(Settings::parse(""), Settings::default());
    }
}
