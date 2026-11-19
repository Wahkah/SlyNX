//! Reading worlds from the disc image, on the I/O thread.
//!
//! The disc is a Sly 1 or a Sly 2 disc ([`GameDisc`]; [`GameDisc::open`] tells them apart by their executables).
//!
//! A load is one job: read and decompress the world, parse it, build the scene and [`prepare`] it for the renderer.
//! The job owns the [`Disc`] while it runs and hands it back with the result, so only one load runs at a time and
//! the disc needs no lock. The result comes back through a [`Mailbox`].

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use formats::disc::Disc;
use formats::iso::ReadAt;
use formats::scene::{Scene, SceneOptions};
use formats::world::World;
use platform_api::{Core, DataFile, FileSystem, JoinHandle, Platform, Priority, Root, ThreadSpec};

use crate::level::{PreparedLevel, prepare};
use crate::sly2::prepare_sly2;

/// A Sly 2 disc.
pub type Sly2Disc<R> = formats_sly2::disc::Disc<R>;

/// Reads a disc image through the platform's file API (Switch: SD card or RomFS).
pub struct FsReader<F: DataFile>(pub F);

impl<F: DataFile> ReadAt for FsReader<F> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> formats::Result<()> {
        self.0.read_exact_at(offset, buf).map_err(|e| formats::Error::Io(e.to_string()))
    }
}

/// Where the disc image is, as a platform path: a root and a path relative to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscPath {
    pub root: Root,
    pub path: String,
}

/// The default disc locations on the Switch, in order: `sdmc:/switch/SlyNX/sly1.iso` and `sly2.iso`, then the
/// `.nro`'s RomFS (`romfs:/sly1.iso` and `sly2.iso`, for packaged builds).
pub fn default_disc_paths() -> [DiscPath; 4] {
    [
        DiscPath { root: Root::Sd, path: String::from("switch/SlyNX/sly1.iso") },
        DiscPath { root: Root::Sd, path: String::from("switch/SlyNX/sly2.iso") },
        DiscPath { root: Root::RomFs, path: String::from("sly1.iso") },
        DiscPath { root: Root::RomFs, path: String::from("sly2.iso") },
    ]
}

/// The disc of either game.
pub enum GameDisc<R: ReadAt> {
    Sly1(Disc<R>),
    Sly2(Sly2Disc<R>),
    // Sly 3 hook.
    Sly3(crate::sly3::Sly3Disc<R>),
}

impl<R: ReadAt> From<Disc<R>> for GameDisc<R> {
    fn from(d: Disc<R>) -> Self {
        GameDisc::Sly1(d)
    }
}

impl<R: ReadAt> From<Sly2Disc<R>> for GameDisc<R> {
    fn from(d: Sly2Disc<R>) -> Self {
        GameDisc::Sly2(d)
    }
}

impl<R: ReadAt> GameDisc<R> {
    /// Opens a disc image of either game: Sly 1 first, then Sly 2. `source` opens the image afresh for each try.
    pub fn open(mut source: impl FnMut() -> Result<R, String>) -> Result<GameDisc<R>, String> {
        let sly1 = match Disc::open(source()?) {
            Ok(d) => return Ok(GameDisc::Sly1(d)),
            Err(e) => e,
        };
        let sly2 = match Sly2Disc::open(source()?) {
            Ok(d) => return Ok(GameDisc::Sly2(d)),
            Err(e) => e,
        };
        // Sly 3 hook.
        match crate::sly3::Sly3Disc::open(source()?) {
            Ok(d) => Ok(GameDisc::Sly3(d)),
            Err(e) => Err(format!("not a supported disc (as Sly 1: {sly1}; as Sly 2: {sly2}; as Sly 3: {e})")),
        }
    }

    /// 1 or 2.
    pub fn game(&self) -> u8 {
        match self {
            GameDisc::Sly1(_) => 1,
            GameDisc::Sly2(_) => 2,
            GameDisc::Sly3(_) => 3, // Sly 3 hook.
        }
    }

    /// The worlds the viewer cycles through. Sly 1: the world table's names. Sly 2: the level table's rows that hold
    /// a world (43), as `Row 3 (0x3B)` (the names are in the worlds themselves, shown once one is loaded).
    pub fn world_names(&self) -> Vec<String> {
        match self {
            GameDisc::Sly1(d) => d.worlds.iter().map(|w| w.name.clone()).collect(),
            GameDisc::Sly2(d) => d.world_rows().map(|r| sly2_label(r, d.rows[r].world_id())).collect(),
            GameDisc::Sly3(d) => crate::sly3::world_names(d), // Sly 3 hook.
        }
    }

    /// Finds a world: as [`crate::world_index`] for Sly 1; for Sly 2 a list index, a world id (`0x3B`), a row
    /// (`row3`) or part of a label.
    pub fn world_index(&self, which: &str) -> Option<usize> {
        match self {
            GameDisc::Sly1(d) => crate::world_index(&d.worlds, which),
            GameDisc::Sly2(d) => {
                let which = which.trim();
                let row = if let Some(hex) = which.strip_prefix("0x").or_else(|| which.strip_prefix("0X")) {
                    d.find_world(u32::from_str_radix(hex, 16).ok()?)?
                } else if let Some(r) = which.strip_prefix("row") {
                    r.trim().parse().ok()?
                } else {
                    return crate::find_by_name(&self.world_names(), which);
                };
                d.world_rows().position(|r| r == row)
            }
            GameDisc::Sly3(d) => crate::sly3::world_index(d, which), // Sly 3 hook.
        }
    }

    /// Reads, parses and prepares world `index` of [`GameDisc::world_names`].
    pub fn load(&mut self, index: usize) -> Result<PreparedLevel, String> {
        match self {
            GameDisc::Sly1(d) => load_world(d, index),
            GameDisc::Sly2(d) => load_world_sly2(d, index),
            GameDisc::Sly3(d) => crate::sly3::load_world_sly3(d, index), // Sly 3 hook.
        }
    }
}

fn sly2_label(row: usize, id: u32) -> String {
    format!("Row {row} (0x{id:02X})")
}

/// Parses `sdmc:/a/b.iso`, `romfs:/a.iso` or a plain SD-relative path.
pub fn parse_disc_path(s: &str) -> Option<DiscPath> {
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
    Some(DiscPath { root, path: String::from(rest) })
}

/// Viewer settings from the user folder's `viewer.cfg` (Switch: `sdmc:/switch/SlyNX/user/viewer.cfg`), lines of
/// `key = value`, `#` comments: `disc` (a disc path, see [`parse_disc_path`]) and `world` (a world name or index).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub disc: Option<DiscPath>,
    pub world: Option<String>,
}

impl Settings {
    pub const FILE: &'static str = "viewer.cfg";

    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "disc" => s.disc = parse_disc_path(v),
                "world" => s.world = Some(String::from(v.trim())).filter(|w| !w.is_empty()),
                _ => {}
            }
        }
        s
    }

    /// Reads the settings file; missing or unreadable means defaults.
    pub fn load<F: FileSystem>(fs: &F) -> Settings {
        match fs.read(Root::User, Self::FILE) {
            Ok(bytes) => Settings::parse(&String::from_utf8_lossy(&bytes)),
            Err(_) => Settings::default(),
        }
    }
}

/// Opens the disc (either game) through the platform's files: the configured path, or the first default that
/// exists. The error says which paths were tried, for the on-screen message.
pub fn open_disc_fs<F: FileSystem>(fs: &F, settings: &Settings) -> Result<GameDisc<FsReader<F::File>>, String> {
    let mut tried = String::new();
    let candidates: alloc::vec::Vec<DiscPath> = match &settings.disc {
        Some(p) => alloc::vec![p.clone()],
        None => default_disc_paths().into_iter().collect(),
    };
    for c in &candidates {
        let shown = match c.root {
            Root::Sd => format!("sdmc:/{}", c.path),
            Root::RomFs => format!("romfs:/{}", c.path),
            Root::User => c.path.clone(),
        };
        match fs.open(c.root, &c.path) {
            Ok(f) => {
                let mut first = Some(f);
                let reopen = || match first.take() {
                    Some(f) => Ok(FsReader(f)),
                    None => fs.open(c.root, &c.path).map(FsReader).map_err(|e| e.to_string()),
                };
                return GameDisc::open(reopen).map_err(|e| format!("{shown}: {e}"));
            }
            Err(e) => {
                if !tried.is_empty() {
                    tried.push_str(", ");
                }
                tried.push_str(&format!("{shown} ({e})"));
            }
        }
    }
    Err(format!("no disc image found: {tried}"))
}

/// Reads, parses and prepares world `index`.
pub fn load_world<R: ReadAt>(disc: &mut Disc<R>, index: usize) -> Result<PreparedLevel, String> {
    let name = disc.worlds.get(index).map(|w| w.name.clone()).ok_or_else(|| format!("no world {index}"))?;
    let bytes = disc.read_world(index).map_err(|e| format!("{name}: {e}"))?;
    let world = World::load(&bytes).map_err(|e| format!("{name}: {e}"))?;
    let scene = Scene::build(&world, SceneOptions::default());
    let mut level = prepare(index, &name, &world, &scene)?;
    level.hud = crate::hud::HudData::read(&world, &disc.executable);
    Ok(level)
}

/// Reads, parses and prepares the Sly 2 world at place `index` of the rows that hold a world.
pub fn load_world_sly2<R: ReadAt>(disc: &mut Sly2Disc<R>, index: usize) -> Result<PreparedLevel, String> {
    let row = disc.world_rows().nth(index).ok_or_else(|| format!("no world {index}"))?;
    let label = sly2_label(row, disc.rows[row].world_id());
    let bytes = disc.read_world(row).map_err(|e| format!("{label}: {e}"))?;
    let world = formats_sly2::world::World::parse(&bytes, &disc.options).map_err(|e| format!("{label}: {e}"))?;
    let name = format!("{} - {label}", world.level_package().0.name);
    // The default passes plus the shadow hulls (7) and projected volumes (11, 13), rendering.md 5.
    let defaults = formats_sly2::scene::SceneOptions::default();
    let opts = formats_sly2::scene::SceneOptions {
        row: Some(row as i16),
        passes: defaults.passes | (1 << 7) | (1 << 11) | (1 << 13),
        ..defaults
    };
    let build = formats_sly2::scene::build(&world, opts);
    let lighting = crate::sly2_light::Sly2Lighting::new(&build.lights, &world.options, world.data);
    prepare_sly2(index, &name, &build, Some(&lighting))
}

/// A single-value hand-over from one thread to another: the producer [`put`](Mailbox::put)s once, the consumer
/// polls with [`take`](Mailbox::take).
pub struct Mailbox<T> {
    slot: AtomicPtr<T>,
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Mailbox { slot: AtomicPtr::new(ptr::null_mut()) }
    }
}

impl<T> Mailbox<T> {
    /// Stores `value`, replacing (and dropping) one that was not taken.
    pub fn put(&self, value: T) {
        let old = self.slot.swap(Box::into_raw(Box::new(value)), Ordering::AcqRel);
        if !old.is_null() {
            // SAFETY: every non-null pointer in the slot came from Box::into_raw above, and the swap removed it
            // from the slot, so this is its only owner.
            drop(unsafe { Box::from_raw(old) });
        }
    }

    /// The value, if one has been stored.
    pub fn take(&self) -> Option<T> {
        let p = self.slot.swap(ptr::null_mut(), Ordering::AcqRel);
        // SAFETY: as in `put`: a non-null pointer came from Box::into_raw and the swap made us its only owner.
        (!p.is_null()).then(|| *unsafe { Box::from_raw(p) })
    }
}

impl<T> Drop for Mailbox<T> {
    fn drop(&mut self) {
        drop(self.take());
    }
}

// SAFETY: the mailbox moves a `T` from one thread to another and never shares a reference to it.
unsafe impl<T: Send> Send for Mailbox<T> {}
// SAFETY: as above; all access to the slot is through atomic swaps.
unsafe impl<T: Send> Sync for Mailbox<T> {}

/// A job for the I/O thread.
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// What the viewer needs from its host: running a job (on the I/O thread, or right away) and CPU boost.
pub trait Host {
    /// Starts `job`; returns a function that waits for it, or `None` if the job already ran.
    fn start_job(&mut self, job: Job) -> Result<Option<Box<dyn FnOnce()>>, String>;
    /// CPU boost on or off (loading).
    fn cpu_boost(&mut self, on: bool);
}

impl<P: Platform> Host for P
where
    P::Thread: 'static,
{
    fn start_job(&mut self, job: Job) -> Result<Option<Box<dyn FnOnce()>>, String> {
        let spec = ThreadSpec::new("loader", Core::Io).priority(Priority::Low).stack_size(1 << 20);
        let handle = self.spawn(spec, job).map_err(|e| e.to_string())?;
        Ok(Some(Box::new(move || handle.join())))
    }

    fn cpu_boost(&mut self, on: bool) {
        self.set_cpu_boost(on);
    }
}

/// Runs jobs at once on the calling thread: deterministic loading for the test harness.
pub struct Blocking;

impl Host for Blocking {
    fn start_job(&mut self, job: Job) -> Result<Option<Box<dyn FnOnce()>>, String> {
        job();
        Ok(None)
    }

    fn cpu_boost(&mut self, _on: bool) {}
}

/// A finished load: the disc back, and the level or an error.
pub type LoadResult<R> = (GameDisc<R>, Result<PreparedLevel, String>);

/// The disc and the load in flight.
pub struct Loader<R: ReadAt + Send + 'static> {
    disc: Option<GameDisc<R>>,
    mailbox: Arc<Mailbox<LoadResult<R>>>,
    join: Option<Box<dyn FnOnce()>>,
    loading: Option<usize>,
}

impl<R: ReadAt + Send + 'static> Loader<R> {
    pub fn new(disc: GameDisc<R>) -> Loader<R> {
        Loader { disc: Some(disc), mailbox: Arc::new(Mailbox::default()), join: None, loading: None }
    }

    /// The world being loaded.
    pub fn loading(&self) -> Option<usize> {
        self.loading
    }

    /// The disc (`None` while a load holds it).
    pub fn disc(&self) -> Option<&GameDisc<R>> {
        self.disc.as_ref()
    }

    /// Starts loading world `index`. Returns false if a load is already running.
    pub fn start(&mut self, host: &mut dyn Host, index: usize) -> Result<bool, String> {
        let Some(mut disc) = self.disc.take() else { return Ok(false) };
        let mailbox = self.mailbox.clone();
        host.cpu_boost(true);
        self.loading = Some(index);
        let job: Job = Box::new(move || {
            let result = disc.load(index);
            mailbox.put((disc, result));
        });
        match host.start_job(job) {
            Ok(join) => {
                self.join = join;
                Ok(true)
            }
            Err(e) => {
                // The job (and the disc in it) is gone; nothing more can be loaded.
                host.cpu_boost(false);
                self.loading = None;
                Err(e)
            }
        }
    }

    /// The finished load, if there is one: the disc is back, and the world index with its result.
    pub fn poll(&mut self, host: &mut dyn Host) -> Option<(usize, Result<PreparedLevel, String>)> {
        let (disc, result) = self.mailbox.take()?;
        if let Some(join) = self.join.take() {
            join();
        }
        host.cpu_boost(false);
        self.disc = Some(disc);
        self.loading.take().map(|i| (i, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_paths_and_settings() {
        assert_eq!(
            parse_disc_path("sdmc:/switch/SlyNX/my.iso"),
            Some(DiscPath { root: Root::Sd, path: "switch/SlyNX/my.iso".into() })
        );
        assert_eq!(parse_disc_path("romfs:/sly1.iso"), Some(DiscPath { root: Root::RomFs, path: "sly1.iso".into() }));
        assert_eq!(parse_disc_path("/games/sly.iso"), Some(DiscPath { root: Root::Sd, path: "games/sly.iso".into() }));
        assert_eq!(parse_disc_path("sdmc:/../x.iso"), None);
        let s = Settings::parse("# comment\n disc = sdmc:/isos/sly1.iso \nworld=The Hideout # start here\nother=1\n");
        assert_eq!(s.disc, Some(DiscPath { root: Root::Sd, path: "isos/sly1.iso".into() }));
        assert_eq!(s.world.as_deref(), Some("The Hideout"));
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn mailbox_hands_over_once() {
        let m: Mailbox<alloc::vec::Vec<u8>> = Mailbox::default();
        assert!(m.take().is_none());
        m.put(alloc::vec![1]);
        m.put(alloc::vec![2]);
        assert_eq!(m.take(), Some(alloc::vec![2]));
        assert!(m.take().is_none());
        m.put(alloc::vec![3]); // dropped with the mailbox
    }

    #[test]
    fn mailbox_across_threads() {
        let m = Arc::new(Mailbox::default());
        let m2 = m.clone();
        let t = std::thread::spawn(move || m2.put(42u32));
        t.join().unwrap();
        assert_eq!(m.take(), Some(42));
    }
}
