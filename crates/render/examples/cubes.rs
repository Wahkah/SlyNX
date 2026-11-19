//! Renderer demo and benchmark on PC: thousands of textured boxes, rendered headless with OpenGL 4.3, per-frame
//! statistics printed, and the last frame saved as a PNG.
//!
//! ```sh
//! cargo run --release -p render --example cubes -- --frames 300 --out cubes.png
//! cargo run --release -p render --example cubes -- --null      # front end only, no GPU
//! ```
//!
//! Options: `--width N --height N --frames N --grid XxZ --variants N --static --null --out FILE`.

use render::demo::{DemoParams, DemoScene};
use render::{Backend, FrameStats, NullBackend, Renderer};
use std::sync::OnceLock;
use std::time::Instant;

fn now_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

struct Args {
    width: u32,
    height: u32,
    frames: u32,
    params: DemoParams,
    null: bool,
    out: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args { width: 1280, height: 720, frames: 240, params: DemoParams::default(), null: false, out: None };
    let mut it = std::env::args().skip(1);
    let num = |v: Option<String>, what: &str| -> Result<u32, String> {
        v.ok_or(format!("{what} needs a value"))?.parse().map_err(|e| format!("{what}: {e}"))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--width" => a.width = num(it.next(), "--width")?,
            "--height" => a.height = num(it.next(), "--height")?,
            "--frames" => a.frames = num(it.next(), "--frames")?.max(1),
            "--variants" => a.params.mesh_variants = num(it.next(), "--variants")?.max(1),
            "--grid" => {
                let v = it.next().ok_or("--grid needs XxZ")?;
                let (x, z) = v.split_once('x').ok_or("--grid needs XxZ")?;
                a.params.grid_x = x.parse().map_err(|e| format!("--grid: {e}"))?;
                a.params.grid_z = z.parse().map_err(|e| format!("--grid: {e}"))?;
            }
            "--static" => a.params.animate = false,
            "--null" => a.null = true,
            "--out" => a.out = Some(it.next().ok_or("--out needs a file name")?),
            "-h" | "--help" => {
                println!("options: --width N --height N --frames N --grid XxZ --variants N --static --null --out FILE");
                std::process::exit(0);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

#[derive(Default)]
struct Summary {
    frames: u32,
    build_ns: Vec<u64>,
    submit_ns: Vec<u64>,
    update_ns: Vec<u64>,
    wait_ns: Vec<u64>,
    gpu_ns: Vec<u64>,
    last: FrameStats,
}

fn percentile(v: &[u64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    s[((s.len() - 1) as f64 * p).round() as usize] as f64 * 1e-6
}

fn mean(v: &[u64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<u64>() as f64 / v.len() as f64 * 1e-6 }
}

fn run<B: Backend>(r: &mut Renderer<B>, args: &Args, after_frame: &mut dyn FnMut(&mut Renderer<B>)) -> Summary {
    r.set_clock(now_ns);
    let t0 = now_ns();
    let scene = match DemoScene::build(r, &args.params) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("scene build failed: {e}");
            std::process::exit(1);
        }
    };
    let (arena_v, arena_i) = r.arena_size();
    println!(
        "scene: {} instances, {} meshes ({} vertices, {} indices in the arena), {} textures in {} pools, {} materials; built and uploaded in {:.1} ms",
        scene.instance_count(),
        scene.meshes,
        arena_v,
        arena_i,
        scene.textures,
        r.texture_pool_count(),
        scene.materials,
        (now_ns() - t0) as f64 * 1e-6
    );

    let mut sum = Summary::default();
    let clear = [0.42, 0.62, 0.85, 1.0];
    // Discard the first frames from the timings: driver warm-up (shader compilation, first uploads).
    let warmup = 10.min(args.frames.saturating_sub(1));
    for f in 0..args.frames {
        let t = f as f32 / 60.0;
        let u0 = now_ns();
        scene.update(r, t).expect("update");
        let u1 = now_ns();
        let stats = match r.render(&scene.camera(t), clear) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("frame {f} failed: {e}");
                std::process::exit(1);
            }
        };
        after_frame(r);
        if f >= warmup {
            sum.frames += 1;
            sum.build_ns.push(stats.cpu_build_ns);
            sum.submit_ns.push(stats.cpu_submit_ns);
            sum.update_ns.push(u1 - u0);
            sum.wait_ns.push(stats.wait_ns);
            if let Some(g) = stats.gpu_ns {
                sum.gpu_ns.push(g);
            }
        }
        sum.last = stats;
    }
    sum
}

fn report(name: &str, s: &Summary) {
    let l = &s.last;
    println!("backend: {name}");
    println!(
        "last frame: {} instances, {} visible, {} batches, {} draw commands, {} API draw calls, {} triangles",
        l.instances, l.visible, l.batches, l.draw_commands, l.api_draw_calls, l.triangles
    );
    println!("over {} frames (ms)       mean    p50    p99", s.frames);
    let row = |label: &str, v: &[u64]| {
        println!("  {label:<22} {:6.3} {:6.3} {:6.3}", mean(v), percentile(v, 0.5), percentile(v, 0.99));
    };
    row("render build (CPU)", &s.build_ns);
    row("render submit (CPU)", &s.submit_ns);
    let total: Vec<u64> = s.build_ns.iter().zip(&s.submit_ns).map(|(a, b)| a + b).collect();
    row("render total (CPU)", &total);
    row("scene update (CPU)", &s.update_ns);
    row("fence wait (GPU-bound)", &s.wait_ns);
    if !s.gpu_ns.is_empty() {
        row("GPU", &s.gpu_ns);
    }
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    if args.null {
        let mut nb = NullBackend::new(args.width, args.height);
        nb.record = false;
        let mut r = Renderer::new(nb);
        let s = run(&mut r, &args, &mut |_| {});
        report("null (front end only)", &s);
        return;
    }
    run_gl(&args);
}

#[cfg(target_os = "linux")]
fn run_gl(args: &Args) {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no headless OpenGL context: {e}");
            std::process::exit(1);
        }
    };
    let target = GlTarget::Offscreen { width: args.width, height: args.height };
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend (dropped below,
    // before `ctx`); its get_proc_address is that context's loader.
    let backend = match unsafe { GlBackend::new(&mut |name| ctx.get_proc_address(name), target) } {
        Ok(b) => b,
        Err(e) => {
            eprintln!("OpenGL backend: {e}");
            std::process::exit(1);
        }
    };
    let name = format!("opengl ({})", backend.renderer_name());
    let mut r = Renderer::new(backend);
    let s = run(&mut r, args, &mut |_| {});
    report(&name, &s);
    if let Some(path) = &args.out {
        let px = r.backend().read_pixels();
        let saved = harness::Image::from_rgba(args.width, args.height, px)
            .ok_or_else(|| "bad image size".to_string())
            .and_then(|img| harness::png::write_file(path, &img).map_err(|e| e.to_string()));
        match saved {
            Ok(()) => println!("wrote {path}"),
            Err(e) => eprintln!("cannot write {path}: {e}"),
        }
    }
    drop(r);
    drop(ctx);
}

#[cfg(not(target_os = "linux"))]
fn run_gl(_args: &Args) {
    eprintln!("headless OpenGL is only wired up on Linux here; use --null, or the platform layer's context");
    std::process::exit(1);
}
