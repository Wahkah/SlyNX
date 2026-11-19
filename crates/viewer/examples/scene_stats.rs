//! Prints what the viewer will draw for a world: shader kinds, passes and blend flags of drawn submeshes, texture
//! sizes, vertex colour ranges and the scene's extent.
//!
//! ```text
//! cargo run --release -p viewer --example scene_stats -- [disc.iso] [world | --all]
//! ```
//!
//! `--bench [world]` (a Sly 1 or Sly 2 disc) times the renderer's CPU work per frame on the null backend from the spawn point and from an
//! overview that sees the whole level. `--all` loads every world the way the viewer does and prints one line per world (draws, load time).
//! The disc path may be left out when `SLYNX_DISC_SLY1` is set; `world` is a name, an id or an index (default 1).

use std::collections::BTreeMap;
use std::process::ExitCode;

use formats::disc::Disc;
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
        std::env::var("SLYNX_DISC_SLY1").map_err(|_| "usage: scene_stats <disc.iso> [world]".to_string())?
    };
    let which = args.first().cloned().unwrap_or_else(|| "1".into());
    if which == "--bench" {
        // Either game: the disc is recognised by its executable.
        let w = args.get(1).cloned().unwrap_or_else(|| "paris".into());
        let mut disc = viewer::pc::open_disc(&disc_path)?;
        let index = disc.world_index(&w).ok_or_else(|| format!("no world `{w}`"))?;
        let t = std::time::Instant::now();
        let level = disc.load(index)?;
        return bench(level, t.elapsed().as_secs_f64() * 1e3);
    }
    let mut disc = Disc::open_path(&disc_path).map_err(|e| e.to_string())?;
    if which == "--all" {
        return all_worlds(&mut disc);
    }
    let index = viewer::world_index(&disc.worlds, &which).ok_or_else(|| format!("no world `{which}`"))?;
    let bytes = disc.read_world(index).map_err(|e| e.to_string())?;
    let world = World::load(&bytes).map_err(|e| e.to_string())?;
    let scene = Scene::build(&world, SceneOptions::default());
    println!("world {index}: {}", disc.worlds[index].name);
    println!(
        "instances {}  meshes {}  submeshes {}  vertices {}  triangles drawn {}  textures {}  skipped {}",
        scene.instances.len(),
        scene.meshes.len(),
        scene.submeshes.len(),
        scene.vertices.len(),
        scene.drawn_triangles(),
        scene.textures.len(),
        scene.skipped_degenerate
    );

    // (kind, shader pass, mesh pass, blend) -> (draws, triangles)
    let mut combos: BTreeMap<(u8, u8, u8, u8), (u32, u64)> = BTreeMap::new();
    let mut draws = 0u64;
    let mut mirrored = 0;
    let mut billboard = 0;
    for inst in &scene.instances {
        mirrored += inst.mirrored as u32;
        billboard += (inst.billboard != 0) as u32;
        let m = &scene.meshes[inst.mesh as usize];
        for sm in &scene.submeshes[m.submeshes.start as usize..m.submeshes.end as usize] {
            let mat = &scene.materials[sm.material as usize];
            let e = combos.entry((mat.kind, mat.pass, inst.pass, mat.blend)).or_default();
            e.0 += 1;
            e.1 += sm.index_count as u64 / 3;
            draws += 1;
        }
    }
    println!("submesh draws {draws}  mirrored instances {mirrored}  billboard instances {billboard}");
    println!("kind  shader-pass  mesh-pass  blend  draws  triangles");
    for ((k, sp, mp, b), (n, t)) in &combos {
        println!("{k:>4}  {sp:>11}  {mp:>9}  {b:>5}  {n:>5}  {t:>9}");
    }

    let mut sizes: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut translucent = 0;
    for t in &scene.textures {
        *sizes.entry((t.image.width, t.image.height)).or_default() += 1;
        translucent += t.image.pixels.chunks(4).any(|p| p[3] < 250) as u32;
    }
    println!("texture sizes: {sizes:?}  with alpha < 250 somewhere: {translucent}");

    let mut hist = [0u32; 8];
    let mut alpha_hist = [0u32; 8];
    for v in &scene.vertices {
        hist[(v.color[0].max(v.color[1]).max(v.color[2]) >> 5) as usize] += 1;
        alpha_hist[(v.color[3] >> 5) as usize] += 1;
    }
    println!("vertex max(rgb) histogram by 32s: {hist:?}");
    println!("vertex alpha histogram by 32s: {alpha_hist:?}");
    let mut mat_colors: BTreeMap<[u8; 4], u32> = BTreeMap::new();
    for m in &scene.materials {
        *mat_colors.entry(m.color).or_default() += 1;
    }
    let mut mc: Vec<_> = mat_colors.into_iter().collect();
    mc.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("material colours (top 8): {:?}", &mc[..mc.len().min(8)]);

    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for inst in &scene.instances {
        for k in 0..3 {
            lo[k] = lo[k].min(inst.transform.t[k]);
            hi[k] = hi[k].max(inst.transform.t[k]);
        }
    }
    println!("instance origins: {lo:?} .. {hi:?}");
    Ok(())
}

fn all_worlds(disc: &mut Disc<std::fs::File>) -> Result<(), String> {
    println!(
        "{:>2}  {:<26} {:>5} {:>5} {:>5} {:>6} {:>4} {:>5} {:>3} {:>3} {:>3} {:>4} {:>6} {:>3} {:>3} {:>3} {:>4} {:>6}",
        "#",
        "name",
        "inst",
        "draws",
        "skip",
        "ktris",
        "opq",
        "blend",
        "sky",
        "vol",
        "pst",
        "lit",
        "kwts",
        "blb",
        "bbd",
        "ani",
        "lts",
        "load"
    );
    let mut worst = 0;
    for i in 0..disc.worlds.len() {
        let t = std::time::Instant::now();
        let level = viewer::loader::load_world(disc, i)?;
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let s = &level.stats;
        worst = worst.max(s.draws);
        println!(
            "{i:>2}  {:<26} {:>5} {:>5} {:>5} {:>6} {:>4} {:>5} {:>3} {:>3} {:>3} {:>4} {:>6} {:>3} {:>3} {:>3} {:>4} {ms:>6.1}",
            level.name.chars().take(26).collect::<String>(),
            s.scene_instances,
            s.draws,
            s.skipped_draws,
            s.triangles / 1000,
            s.opaque,
            s.blended,
            s.backdrops,
            s.volumes,
            s.post,
            s.lit,
            s.weights / 1000,
            s.blob_shadows,
            s.billboards,
            s.animations,
            s.lights
        );
    }
    println!("most draws in one world: {worst}");
    Ok(())
}

fn bench(level: viewer::level::PreparedLevel, load_ms: f64) -> Result<(), String> {
    use render::NullBackend;
    use viewer::camera::Spawn;

    fn now() -> u64 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
    }
    let mut backend = NullBackend::new(1280, 720);
    backend.record = false;
    let mut r = render::Renderer::new(backend);
    r.set_clock(now);
    let t = std::time::Instant::now();
    viewer::level::upload(&mut r, Some(&level), viewer::session::OVERLAY_CHARS).map_err(|e| e.to_string())?;
    let upload_ms = t.elapsed().as_secs_f64() * 1e3;
    let arena = level.vertices.len() * 28 + level.indices.len() * 2;
    let texels: usize = level.textures.iter().map(|t| t.data.len()).sum();
    println!(
        "{}: load {load_ms:.1} ms, upload {upload_ms:.1} ms (PC, one thread); arena {:.1} MiB, textures {:.1} MiB",
        level.name,
        arena as f64 / 1048576.0,
        texels as f64 / 1048576.0
    );
    let (lo, hi) = level.bounds;
    let centre = (lo + hi) * 0.5;
    let overview = Spawn {
        eye: centre + render::math::Vec3::new(0.0, 0.0, (0.5 * (hi - lo).length()).min(60_000.0)),
        target: centre,
    };
    // Looking away from the level: the culling cost alone.
    let away = Spawn {
        eye: centre + render::math::Vec3::new(0.0, 0.0, 1e6),
        target: centre + render::math::Vec3::new(0.0, 0.0, 2e6),
    };
    for (what, spawn) in [("spawn", level.spawn), ("overview", overview), ("away", away)] {
        let mut cam = viewer::camera::FreeCamera::default();
        cam.place(spawn);
        let mut build = Vec::new();
        let mut last = render::FrameStats::default();
        for _ in 0..600 {
            last = r.render(&cam.camera(), [0.0; 4]).map_err(|e| e.to_string())?;
            build.push(last.cpu_build_ns);
        }
        build.sort_unstable();
        let mean = build.iter().sum::<u64>() as f64 / build.len() as f64 * 1e-6;
        let p99 = build[build.len() * 99 / 100] as f64 * 1e-6;
        println!(
            "  {what:<8}: build mean {mean:.3} ms, p99 {p99:.3} ms; {} of {} instances visible, {} batches, {} draws, {}k triangles, {} KiB of instance records",
            last.visible,
            last.instances,
            last.batches,
            last.draw_commands,
            last.triangles / 1000,
            last.visible as usize * 80 / 1024
        );
    }
    Ok(())
}
