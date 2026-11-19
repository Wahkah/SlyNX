//! Movie decode benchmark: finds the player's own Sly 1, 2 and 3 disc images through the platform's file system,
//! decodes their movies, and reports decode times per frame. On the Switch it is `dist/slynx-video-bench.nro`
//! (`src/switch.rs`); the same code runs on a PC in the tests. See `docs/video.md`, "Switch decode benchmark".
//!
//! Disc images are looked up as the game does for Sly 1 (`disc = ...` in `user/game.cfg`, else
//! `sdmc:/switch/SlyNX/sly1.iso`), and at `sdmc:/switch/SlyNX/sly2.iso` and `sly3.iso`; `user/video-bench.cfg` may
//! give `sly1 =`, `sly2 =`, `sly3 =` paths and `frames = N` (frames decoded per movie, 0 for all; default 900).
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

#[cfg(target_os = "horizon")]
mod switch;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use formats::iso::ReadAt;
use platform_api::{DataFile, FileSystem, Root};
use video::decoder::{Decoded, Decoder};
use video::demux::{Demuxer, Packet};
use video::movies::{Region, movie_table, sly2_movies, sly3_movies};

/// The log file, in the user folder.
pub const LOG_FILE: &str = "video-bench.log";
/// The optional settings file, in the user folder.
pub const SETTINGS_FILE: &str = "video-bench.cfg";
/// Frames decoded per movie unless the settings say otherwise (30 seconds).
pub const DEFAULT_FRAMES: u32 = 900;
/// The frame budget the decoder must meet on the Switch (docs/video.md, Performance).
pub const TARGET_MS: f64 = 12.0;

/// Reads a disc image through the platform's file API.
pub struct FsReader<F: DataFile>(pub F);

impl<F: DataFile> ReadAt for FsReader<F> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> formats::Result<()> {
        self.0.read_exact_at(offset, buf).map_err(|e| formats::Error::Io(e.to_string()))
    }
}

/// What to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Disc image per game (Sly 1, 2, 3): root and path.
    pub discs: [Vec<(Root, String)>; 3],
    /// Frames decoded per movie (0: all).
    pub frames: u32,
}

/// `sdmc:/a/b.iso`, `romfs:/a.iso` or a plain SD-relative path.
pub fn parse_path(s: &str) -> Option<(Root, String)> {
    let s = s.trim();
    let (root, rest) = if let Some(r) = s.strip_prefix("sdmc:/") {
        (Root::Sd, r)
    } else if let Some(r) = s.strip_prefix("romfs:/") {
        (Root::RomFs, r)
    } else {
        (Root::Sd, s)
    };
    platform_api::fs::validate(rest).ok()?;
    Some((root, rest.to_string()))
}

/// `key = value` lines; `#` starts a comment.
fn settings_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let line = line.split('#').next()?;
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| v.trim())
    })
}

impl Settings {
    /// The candidates for each game, from the two settings files' text (either may be empty).
    pub fn parse(game_cfg: &str, bench_cfg: &str) -> Settings {
        let mut discs: [Vec<(Root, String)>; 3] = Default::default();
        for (g, list) in discs.iter_mut().enumerate() {
            let key = format!("sly{}", g + 1);
            if let Some(p) = settings_value(bench_cfg, &key).and_then(parse_path) {
                list.push(p);
            }
            if g == 0
                && let Some(p) = settings_value(game_cfg, "disc").and_then(parse_path)
            {
                list.push(p);
            }
            list.push((Root::Sd, format!("switch/SlyNX/{key}.iso")));
        }
        let frames = settings_value(bench_cfg, "frames").and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_FRAMES);
        Settings { discs, frames }
    }

    /// Reads both settings files from the user folder.
    pub fn load<F: FileSystem>(fs: &F) -> Settings {
        let text = |path: &str| fs.read(Root::User, path).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
        Settings::parse(&text("game.cfg").unwrap_or_default(), &text(SETTINGS_FILE).unwrap_or_default())
    }
}

/// One movie's result.
#[derive(Clone, Debug, Default)]
pub struct MovieResult {
    pub game: u8,
    pub index: usize,
    pub identity: u32,
    pub width: u32,
    pub height: u32,
    /// Decode time of each frame returned (the `decode` call that returned it), nanoseconds.
    pub frame_ns: Vec<u64>,
    /// Time spent reading and demultiplexing, nanoseconds.
    pub read_ns: u64,
    pub slice_errors: u64,
}

impl MovieResult {
    fn percentile(&self, p: f64) -> f64 {
        let mut v = self.frame_ns.clone();
        v.sort_unstable();
        v.get(((v.len().max(1) - 1) as f64 * p) as usize).map_or(0.0, |&n| n as f64 / 1e6)
    }

    /// One log line.
    pub fn line(&self, boost: bool) -> String {
        let n = self.frame_ns.len().max(1) as f64;
        let total: u64 = self.frame_ns.iter().sum();
        let mean = total as f64 / n / 1e6;
        format!(
            "sly{} movie {:2} {:#05x} {}x{} boost {}: {} frames, decode mean {:.2} ms, p50 {:.2}, p95 {:.2}, p99 {:.2}, \
             max {:.2} ms, {:.0} fps; read+demux {:.2} ms/frame; errors {}; {}",
            self.game,
            self.index,
            self.identity,
            self.width,
            self.height,
            if boost { "on" } else { "off" },
            self.frame_ns.len(),
            mean,
            self.percentile(0.5),
            self.percentile(0.95),
            self.percentile(0.99),
            self.percentile(1.0),
            if total > 0 { n * 1e9 / total as f64 } else { 0.0 },
            self.read_ns as f64 / n / 1e6,
            self.slice_errors,
            if mean <= TARGET_MS { "within 12 ms" } else { "OVER 12 ms" },
        )
    }
}

/// One movie of any game.
struct Movie {
    index: usize,
    identity: u32,
    offset: u64,
    size: u64,
}

/// The game's distinct movies, from its disc image.
fn movies<R: ReadAt>(game: u8, src: R) -> Result<Vec<Movie>, String> {
    let from_records = |r: Vec<(usize, formats_sly2::record::FileRecord)>| -> Vec<Movie> {
        r.into_iter()
            .map(|(index, f)| Movie { index, identity: f.identity, offset: f.offset(), size: u64::from(f.size) })
            .collect()
    };
    Ok(match game {
        1 => {
            let d = formats::disc::Disc::open(src).map_err(|e| e.to_string())?;
            movie_table(&d.executable)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|m| Movie {
                    index: m.index,
                    identity: m.identity(),
                    offset: m.descriptor.offset(),
                    size: u64::from(m.descriptor.size),
                })
                .collect()
        }
        2 => from_records(sly2_movies(&formats_sly2::Disc::open(src).map_err(|e| e.to_string())?.files)),
        _ => from_records(sly3_movies(&formats_sly3::Disc::open(src).map_err(|e| e.to_string())?.movies)),
    })
}

/// Decodes up to `frames` frames (0: all) of one movie, timing each `decode` call that returns a frame. `keep_going`
/// is called after each frame (the platform's message pump); returning false stops.
fn bench_movie<R: ReadAt>(
    src: R,
    m: &Movie,
    frames: u32,
    now: &dyn Fn() -> u64,
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<MovieResult, String> {
    let mut region = Region::new(src, m.offset, m.size);
    let mut demux = Demuxer::new();
    let mut dec = Decoder::new();
    let mut buf = alloc::vec![0u8; 64 * 1024];
    let mut pos = 0u64;
    let mut out = MovieResult { identity: m.identity, index: m.index, ..Default::default() };
    let mut finished = false;
    loop {
        let t = now();
        let r = dec.decode().map_err(|e| e.to_string())?;
        let dt = now() - t;
        match r {
            Decoded::Frame(f) => {
                out.frame_ns.push(dt);
                out.width = f.width as u32;
                out.height = f.height as u32;
                if (frames > 0 && out.frame_ns.len() >= frames as usize) || !keep_going() {
                    break;
                }
            }
            Decoded::NeedData => {
                let t = now();
                let n = region.read(pos, &mut buf).map_err(|e| e.to_string())?;
                pos += n as u64;
                demux.push(&buf[..n]);
                while let Some(p) = demux.next_packet().map_err(|e| e.to_string())? {
                    if let Packet::Video { data, .. } = p {
                        dec.push(data);
                    }
                }
                if n == 0 || pos >= region.len() {
                    if finished {
                        break;
                    }
                    finished = true;
                    dec.finish();
                }
                out.read_ns += now() - t;
            }
            Decoded::End => break,
        }
    }
    out.slice_errors = dec.stats().slice_errors;
    Ok(out)
}

/// Runs the benchmark over every game whose disc image is found: each movie, decoded with `frames` frames.
/// `log` receives every line (and the caller writes it out); `keep_going` is the message pump. Returns the results.
pub fn run<F: FileSystem>(
    fs: &F,
    settings: &Settings,
    boost: bool,
    now: &dyn Fn() -> u64,
    log: &mut dyn FnMut(&str),
    keep_going: &mut dyn FnMut() -> bool,
) -> Vec<MovieResult> {
    let mut results = Vec::new();
    for (g, candidates) in settings.discs.iter().enumerate() {
        let game = g as u8 + 1;
        let Some((root, path)) = candidates.iter().find(|(r, p)| fs.exists(*r, p)) else {
            log(&format!(
                "sly{game}: no disc image (tried {})",
                candidates.iter().map(|c| c.1.as_str()).collect::<Vec<_>>().join(", ")
            ));
            continue;
        };
        let open = || fs.open(*root, path).map(FsReader).map_err(|e| format!("{path}: {e}"));
        let list = match open().and_then(|r| movies(game, r)) {
            Ok(l) => l,
            Err(e) => {
                log(&format!("sly{game}: {e}"));
                continue;
            }
        };
        log(&format!("sly{game}: {path}, {} movies, boost {}", list.len(), if boost { "on" } else { "off" }));
        for m in &list {
            let r = open().and_then(|f| bench_movie(f, m, settings.frames, now, keep_going));
            match r {
                Ok(mut r) => {
                    r.game = game;
                    log(&r.line(boost));
                    results.push(r);
                }
                Err(e) => log(&format!("sly{game} movie {} {:#x}: {e}", m.index, m.identity)),
            }
            if !keep_going() {
                return results;
            }
        }
    }
    let frames: usize = results.iter().map(|r| r.frame_ns.len()).sum();
    let total: u64 = results.iter().flat_map(|r| &r.frame_ns).sum();
    let worst = results.iter().map(|r| r.percentile(0.99)).fold(0.0f64, f64::max);
    log(&format!(
        "summary boost {}: {} movies, {frames} frames, decode mean {:.2} ms, worst movie p99 {:.2} ms",
        if boost { "on" } else { "off" },
        results.len(),
        total as f64 / frames.max(1) as f64 / 1e6,
        worst
    ));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings() {
        let s = Settings::parse("disc = sdmc:/isos/sly.iso\n", "sly2 = romfs:/s2.iso # here\nframes = 30\n");
        assert_eq!(s.frames, 30);
        assert_eq!(s.discs[0], [(Root::Sd, "isos/sly.iso".into()), (Root::Sd, "switch/SlyNX/sly1.iso".into())]);
        assert_eq!(s.discs[1], [(Root::RomFs, "s2.iso".into()), (Root::Sd, "switch/SlyNX/sly2.iso".into())]);
        assert_eq!(s.discs[2], [(Root::Sd, "switch/SlyNX/sly3.iso".into())]);
        assert_eq!(Settings::parse("", "").frames, DEFAULT_FRAMES);
        assert_eq!(parse_path("sdmc:/../x.iso"), None);
    }
}
