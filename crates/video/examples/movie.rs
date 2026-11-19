//! Lists, benchmarks and dumps frames of the movies on the player's own disc.
//!
//! ```text
//! cargo run --release -p video --example movie -- <disc.iso> list [--game sly2|sly3]
//! cargo run --release -p video --example movie -- <disc.iso> bench [index|all] [--threads N] [--game sly2|sly3]
//! cargo run --release -p video --example movie -- <disc.iso> png <index> <out-dir> <frame>[,<frame>...] [--game ...]
//! ```
//!
//! `index` is the position in the game's list of distinct movies (Sly 1: the movie table, 0 to 16). PNG output must
//! go outside the repository.

use std::fs::File;
use std::time::Instant;

use formats::disc::Disc;
use video::decoder::{Decoded, Decoder};
use video::demux::{Demuxer, Packet};
use video::movies::{Region, movie_table, sly2_movies, sly3_movies};

/// One movie of any game: its list position, identity, place in the image, and title (Sly 1).
struct Movie {
    index: usize,
    identity: u32,
    offset: u64,
    size: u64,
    title: Option<String>,
}

/// The game's distinct movies and the image file.
fn movies(path: &str, game: &str) -> (Vec<Movie>, File) {
    let file = File::open(path).expect("open image");
    let from_records = |records: Vec<(usize, formats_sly2::record::FileRecord)>| -> Vec<Movie> {
        records
            .into_iter()
            .map(|(index, f)| Movie {
                index,
                identity: f.identity,
                offset: f.offset(),
                size: u64::from(f.size),
                title: None,
            })
            .collect()
    };
    let list = match game {
        "sly1" => {
            let disc = Disc::open_path(path).expect("open disc");
            movie_table(&disc.executable)
                .expect("movie table")
                .into_iter()
                .map(|m| Movie {
                    index: m.index,
                    identity: m.identity(),
                    offset: m.descriptor.offset(),
                    size: u64::from(m.descriptor.size),
                    title: m.title.clone(),
                })
                .collect()
        }
        "sly2" => from_records(sly2_movies(&formats_sly2::Disc::open_path(path).expect("open disc").files)),
        "sly3" => from_records(sly3_movies(&formats_sly3::Disc::open_path(path).expect("open disc").movies)),
        _ => usage(),
    };
    (list, file)
}

fn usage() -> ! {
    eprintln!("usage: movie <disc.iso> list | bench [index|all] [--threads N] | png <index> <out-dir> <f1,f2,...>");
    std::process::exit(2)
}

/// Decode timing of one movie (disc reads and demultiplexing excluded).
struct Timing {
    total: f64,
    frames: u64,
    /// Seconds per `decode` call that returned a frame, sorted.
    calls: Vec<f64>,
}

impl Timing {
    fn percentile(&self, p: f64) -> f64 {
        self.calls[((self.calls.len() - 1) as f64 * p).round() as usize]
    }
}

/// Decodes a whole movie, calling `on_frame` for each frame in display order.
fn decode_movie(
    file: &mut File,
    m: &Movie,
    threads: usize,
    mut on_frame: impl FnMut(&video::Frame),
) -> (Timing, Decoder) {
    let mut region = Region::new(file, m.offset, m.size);
    let mut es = Vec::with_capacity(region.len() as usize);
    let mut demux = Demuxer::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut pos = 0;
    loop {
        let n = region.read(pos, &mut buf).expect("read");
        if n == 0 {
            break;
        }
        pos += n as u64;
        demux.push(&buf[..n]);
        while let Some(p) = demux.next_packet().expect("demux") {
            if let Packet::Video { data, .. } = p {
                es.extend_from_slice(data);
            }
        }
    }
    let mut dec = Decoder::new();
    dec.set_threads(threads);
    dec.push(&es);
    dec.finish();
    let mut calls = Vec::new();
    loop {
        let t = Instant::now();
        match dec.decode().expect("decode") {
            Decoded::Frame(f) => {
                calls.push(t.elapsed().as_secs_f64());
                on_frame(f);
            }
            Decoded::NeedData => unreachable!(),
            Decoded::End => break,
        }
    }
    let total = calls.iter().sum();
    calls.sort_by(f64::total_cmp);
    (Timing { total, frames: calls.len() as u64, calls }, dec)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        usage();
    }
    let game = args.iter().position(|a| a == "--game").and_then(|i| args.get(i + 1)).map_or("sly1", String::as_str);
    let (movies, mut file) = movies(&args[0], game);
    let file = &mut file;
    let threads = args.iter().position(|a| a == "--threads").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(1);
    match args[1].as_str() {
        "list" => {
            for m in &movies {
                println!(
                    "{:2} identity {:#x} sector {:#x} {:>11} bytes  {}",
                    m.index,
                    m.identity,
                    m.offset / 2048,
                    m.size,
                    m.title.as_deref().unwrap_or("")
                );
            }
        }
        "bench" => {
            let which: Vec<&Movie> = match args.get(2).map(String::as_str) {
                None | Some("all") | Some("--threads") | Some("--game") => movies.iter().collect(),
                Some(i) => vec![&movies[i.parse::<usize>().unwrap_or_else(|_| usage())]],
            };
            let (mut total_t, mut total_f) = (0.0, 0);
            for m in which {
                let (t, dec) = decode_movie(file, m, threads, |_| {});
                let s = dec.stats();
                println!(
                    "{:2} {:#x} {}x{}: {} frames, {:.0} fps; ms per frame: mean {:.3} p50 {:.3} p99 {:.3} max {:.3};                      errors {} {:?}; I/P/B {}/{}/{}; field mv {} dual-prime {} field dct {}; gops {} closed {} broken {}",
                    m.index,
                    m.identity,
                    dec.sequence().map_or(0, |q| q.width),
                    dec.sequence().map_or(0, |q| q.height),
                    t.frames,
                    t.frames as f64 / t.total,
                    t.total * 1000.0 / t.frames as f64,
                    t.percentile(0.5) * 1000.0,
                    t.percentile(0.99) * 1000.0,
                    t.percentile(1.0) * 1000.0,
                    s.slice_errors,
                    s.first_error,
                    s.i_pictures,
                    s.p_pictures,
                    s.b_pictures,
                    s.mb.field_motion,
                    s.mb.dual_prime,
                    s.mb.field_dct,
                    s.gops,
                    s.closed_gops,
                    s.broken_links,
                );
                total_t += t.total;
                total_f += t.frames;
            }
            println!("total: {total_f} frames in {total_t:.2} s = {:.0} fps", total_f as f64 / total_t);
        }
        "png" => {
            let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            let out = std::path::PathBuf::from(args.get(3).unwrap_or_else(|| usage()));
            let wanted: Vec<u64> =
                args.get(4).unwrap_or_else(|| usage()).split(',').filter_map(|s| s.parse().ok()).collect();
            std::fs::create_dir_all(&out).expect("create output folder");
            let m = &movies[index];
            decode_movie(file, m, threads, |f| {
                if wanted.contains(&f.info.display_index) {
                    let img = harness::image::Image::from_rgba(f.width as u32, f.height as u32, f.to_rgba_vec())
                        .expect("image");
                    let path = out.join(format!("movie{index:02}_{:05}.png", f.info.display_index));
                    harness::png::write_file(&path, &img).expect("write png");
                    println!("{} ({:?})", path.display(), f.info.picture_type);
                }
            });
        }
        _ => usage(),
    }
}
