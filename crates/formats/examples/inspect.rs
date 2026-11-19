//! Prints a summary of a Sly 1 world read from the player's disc image.
//!
//! ```text
//! cargo run --release -p formats --example inspect -- <disc.iso> [world]
//! cargo run --release -p formats --example inspect -- <disc.iso> --list
//! cargo run --release -p formats --example inspect -- <disc.iso> --bench
//! ```
//!
//! `--bench` decompresses, parses and builds the scene of all 46 worlds on one thread and prints the throughput.
//!
//! `world` is a table index (0..45), a world id (`0x103`) or a display name; default 0. The disc path may be left
//! out when `SLYNX_DISC_SLY1` is set.

use std::process::ExitCode;
use std::time::Instant;

use formats::disc::Disc;
use formats::geometry::GeometryStats;
use formats::hash::{crc32, hex, sha256};
use formats::scene::{Scene, SceneOptions};
use formats::world::World;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let disc_path = if args.first().is_some_and(|a| std::path::Path::new(a).is_file()) {
        args.remove(0)
    } else {
        std::env::var("SLYNX_DISC_SLY1")
            .map_err(|_| "usage: inspect <disc.iso> [world | --list | --bench]".to_string())?
    };
    let mut disc = Disc::open_path(&disc_path).map_err(|e| e.to_string())?;
    let which = args.first().map(String::as_str).unwrap_or("0");

    if which == "--list" {
        println!("{:>2}  {:>5}  {:>3}  {:>7}  {:>9}  name", "#", "id", "key", "lba", "size");
        for w in &disc.worlds {
            println!(
                "{:>2}  {:#05x}  {:>3}  {:>7}  {:>9}  {}",
                w.index,
                w.id,
                w.key(),
                w.file.sector,
                w.file.size,
                w.name
            );
        }
        return Ok(());
    }

    if which == "--bench" {
        return bench(&mut disc);
    }

    let index = which
        .parse::<usize>()
        .ok()
        .filter(|&i| i < disc.worlds.len())
        .or_else(|| disc.find_world(which))
        .ok_or_else(|| format!("no world {which:?}"))?;
    let entry = disc.worlds[index].clone();
    let compressed = disc.read_world_compressed(index).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let data = formats::compress::decompress(&compressed).map_err(|e| e.to_string())?;
    let dt = t0.elapsed().as_secs_f64();
    println!("World {index}: {} (id {:#05x}, key {})", entry.name, entry.id, entry.key());
    println!(
        "  compressed {} bytes at LBA {}, decompressed {} bytes in {:.1} ms ({:.0} MB/s of output)",
        compressed.len(),
        entry.file.sector,
        data.len(),
        dt * 1e3,
        data.len() as f64 / dt / 1e6
    );
    println!("  SHA-256 {}  CRC-32 {:08x}", hex(&sha256(&data)), crc32(&data));

    let t0 = Instant::now();
    let world = World::load(&data).map_err(|e| e.to_string())?;
    let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
    let l = &world.layout;
    let m = &l.materials;
    println!("  parsed in {parse_ms:.1} ms");
    println!(
        "  header: texture size {:#x}, word 1 {:#x}; debug {}, sound bank {}, name count {}",
        l.header.texture_size, l.header.timestamp, l.debug, l.sound_bank, l.name_count
    );
    println!(
        "  scripts {:#x}..{:#x}: {} scripts, {} symbols, {} extra",
        l.scripts.range.start, l.scripts.range.end, l.scripts.count, l.scripts.symbols, l.scripts.extra
    );
    println!(
        "  world options {:#x}..{:#x} ({} entries)",
        l.world_options.start, l.world_options.end, l.world_options.count
    );
    match l.music {
        Some(d) => println!("  music: sector {:#x}, size {:#x}, identity {:#x}", d.sector, d.size, d.identity),
        None => println!("  music: none"),
    }
    println!(
        "  materials {:#x}..{:#x}: {} palettes, {} bitmaps, {} shaders ({} animated), {} fonts",
        l.materials_range.start,
        l.materials_range.end,
        m.palettes.len(),
        m.bitmaps.len(),
        m.shaders.len(),
        m.animated_shader_count,
        m.fonts.len()
    );
    let s = &world.objects;
    println!(
        "  object stream {:#x}..{:#x}; texture base T {:#x}; tail {} bytes",
        s.start,
        s.end,
        world.texture_base,
        world.tail_bytes()
    );
    println!(
        "  records {} (list-loaded {}, top level {}), max depth {}, drawable {} (live {}), proxy sources {}, \
         back-references {} (drawable in clones {}), options {}",
        s.records.len(),
        s.list_loaded(),
        s.top_level.len(),
        s.max_depth(),
        s.drawable(),
        s.drawable_live(),
        s.sources.len(),
        s.back_references(),
        s.drawable_in_clones(),
        s.counts.options + l.world_options.count as u64
    );
    let mut g = GeometryStats::default();
    for r in &s.records {
        if let Some(set) = &r.mesh_set {
            g.add(&data, set);
        }
    }
    println!(
        "  geometry: {} mesh sets ({} non-empty), {} meshes, {} instances, {} submeshes, {} strip entries, \
         {} positions, {} triangles, {} outline parts",
        g.mesh_sets,
        g.nonempty_sets,
        g.meshes,
        g.instances,
        g.submeshes,
        g.strip_entries,
        g.positions,
        g.triangles,
        g.outline_parts
    );

    let t0 = Instant::now();
    let scene = Scene::build(&world, SceneOptions::default());
    let scene_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "  scene ({scene_ms:.1} ms): {} vertices, {} indices, {} meshes, {} draws ({} mirrored, {} from clones), \
         {} drawn triangles, {} textures ({} failed)",
        scene.vertices.len(),
        scene.indices.len(),
        scene.meshes.len(),
        scene.instances.len(),
        scene.instances.iter().filter(|i| i.mirrored).count(),
        scene.instances.iter().filter(|i| i.clone_of_proxy.is_some()).count(),
        scene.drawn_triangles(),
        scene.textures.len(),
        scene.texture_errors.len()
    );
    Ok(())
}

fn bench(disc: &mut Disc<std::fs::File>) -> Result<(), String> {
    let compressed: Vec<Vec<u8>> = (0..disc.worlds.len())
        .map(|i| disc.read_world_compressed(i))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let in_bytes: usize = compressed.iter().map(Vec::len).sum();
    let mut best = f64::MAX;
    let mut worlds = Vec::new();
    for _ in 0..3 {
        let t0 = Instant::now();
        worlds = compressed
            .iter()
            .map(|c| formats::compress::decompress(c))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        best = best.min(t0.elapsed().as_secs_f64());
    }
    let out_bytes: usize = worlds.iter().map(Vec::len).sum();
    println!(
        "decompress: {} worlds, {:.1} MB in, {:.1} MB out, {:.0} ms, {:.0} MB/s of output ({:.0} MB/s of input)",
        worlds.len(),
        in_bytes as f64 / 1e6,
        out_bytes as f64 / 1e6,
        best * 1e3,
        out_bytes as f64 / best / 1e6,
        in_bytes as f64 / best / 1e6
    );
    let t0 = Instant::now();
    let parsed: Vec<World<'_>> =
        worlds.iter().map(|w| World::load(w)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let parse = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let mut draws = 0;
    for w in &parsed {
        draws += Scene::build(w, SceneOptions::default()).instances.len();
    }
    let scene = t0.elapsed().as_secs_f64();
    println!("parse (outer layout + object stream): {:.0} ms for all worlds", parse * 1e3);
    println!("scene build (vertex arenas + texture decode): {:.0} ms for all worlds, {draws} draws", scene * 1e3);
    Ok(())
}
