//! The PC side: the harness app (`--app viewer`) and the windowed viewer's set-up.

use std::fs::File;
use std::sync::OnceLock;
use std::time::Instant;

use harness::perf::{DRAW_CALLS, TRIANGLES};
use harness::{App, AppConfig, AppError, FrameContext, Image};
use platform_pc::{PcConfig, PcPlatform};
use render::gl::{GlBackend, GlTarget};
use render::math::Vec3;

use crate::camera::{Controls, Spawn};
use crate::loader::{Blocking, GameDisc};
use crate::session::Session;

/// Monotonic nanoseconds for the renderer's CPU timings.
pub fn clock_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// The disc image path: `explicit`, else `SLYNX_DISC_SLY1`.
pub fn disc_path(explicit: Option<&str>) -> Result<String, String> {
    disc_path_for(explicit, 1)
}

/// The disc image path: `explicit`, else `SLYNX_DISC_SLY1` or `SLYNX_DISC_SLY2` for `game` 1 or 2.
pub fn disc_path_for(explicit: Option<&str>, game: u8) -> Result<String, String> {
    if let Some(p) = explicit.filter(|p| !p.is_empty()) {
        return Ok(p.to_string());
    }
    let var = match game {
        2 => "SLYNX_DISC_SLY2",
        3 => "SLYNX_DISC_SLY3", // Sly 3 hook.
        _ => "SLYNX_DISC_SLY1",
    };
    std::env::var(var)
        .ok()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| format!("no disc image: pass disc=PATH or set {var}"))
}

/// Opens the disc image at `path`, of either game.
pub fn open_disc(path: &str) -> Result<GameDisc<File>, String> {
    GameDisc::open(|| File::open(path).map_err(|e| e.to_string())).map_err(|e| format!("{path}: {e}"))
}

/// The `game=` option: `sly2` or `2` for Sly 2, `sly3` or `3` for Sly 3, else Sly 1.
pub fn parse_game(s: Option<&str>) -> u8 {
    match s.map(str::trim) {
        Some("2" | "sly2" | "Sly2" | "SLY2") => 2,
        Some("3" | "sly3" | "Sly3" | "SLY3") => 3, // Sly 3 hook.
        _ => 1,
    }
}

/// Creates the OpenGL backend on the platform's context, drawing into its default framebuffer. Give the renderer
/// [`clock_ns`] for CPU timings.
pub fn gl_backend(platform: &PcPlatform) -> Result<GlBackend, String> {
    let ctx = platform.gl();
    let (width, height) = ctx.size();
    let target = GlTarget::External { framebuffer: ctx.default_framebuffer(), width, height };
    // SAFETY: the platform's context is current on this thread and outlives the backend (callers drop the session
    // before the platform); the loader is that context's.
    unsafe { GlBackend::new(&mut |name| ctx.get_proc_address(name), target) }.map_err(|e| e.to_string())
}

/// Parses `x,y,z,yaw,pitch` (game units and radians), or `x,y,z,tx,ty,tz` (eye and target).
pub fn parse_camera(s: &str) -> Option<Spawn> {
    let v: Vec<f32> = s.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    match v.as_slice() {
        [x, y, z, yaw, pitch] => {
            let eye = Vec3::new(*x, *y, *z);
            let d = Vec3::new(
                render::math::cos(*pitch) * render::math::cos(*yaw),
                render::math::cos(*pitch) * render::math::sin(*yaw),
                render::math::sin(*pitch),
            );
            Some(Spawn { eye, target: eye + d })
        }
        [x, y, z, tx, ty, tz] => Some(Spawn { eye: Vec3::new(*x, *y, *z), target: Vec3::new(*tx, *ty, *tz) }),
        _ => None,
    }
}

/// The viewer as a harness app. Options:
///
/// - `game=sly2` for a Sly 2 disc from `SLYNX_DISC_SLY2` (a disc given with `disc=` is recognised either way);
/// - `world=NAME` (Sly 1: name, part of a name, id or index, default Paris; Sly 2: list index, world id `0x3B`
///   or row `row3`, default the first row);
/// - `disc=PATH` (default `SLYNX_DISC_SLY1`, or `SLYNX_DISC_SLY2` with `game=sly2`);
/// - `overlay=1` to draw the stats overlay (off by default: its timings change from run to run, so captures with
///   it are not reproducible);
/// - `camera=x,y,z,yaw,pitch` or `camera=x,y,z,tx,ty,tz` instead of the spawn point.
///
/// Loading is synchronous, in the constructor. Input comes from the harness (`--input`), with the same controls as
/// on the Switch; L and R switch worlds synchronously too.
pub struct ViewerApp {
    // Field order is drop order: the session (and its GL backend) before the platform.
    session: Session<GlBackend, File>,
    platform: PcPlatform,
    last_world: Option<usize>,
}

impl ViewerApp {
    pub const NAME: &'static str = "viewer";
    pub const DESCRIPTION: &'static str =
        "Sly 1 and 2 level viewer; options: game=sly2, world=NAME, disc=PATH, overlay=1, camera=x,y,z,yaw,pitch";

    pub fn new(config: &AppConfig) -> Result<ViewerApp, AppError> {
        let path = disc_path_for(config.option("disc"), parse_game(config.option("game")))?;
        let disc = open_disc(&path)?;
        let which = config.option("world").unwrap_or(if disc.game() >= 2 { "0" } else { "Paris" }); // Sly 3 hook: `>= 2`.
        let index = disc.world_index(which).ok_or_else(|| format!("no world `{which}` on the disc"))?;

        let mut pc = PcConfig::headless(config.width, config.height);
        if !config.headless {
            pc = PcConfig { headless: false, vsync: false, audio: false, controllers: false, ..pc };
        }
        let platform = PcPlatform::new(pc)?;
        let backend = gl_backend(&platform)?;
        let mut session = Session::new(backend, Ok(disc)).map_err(|e| e.to_string())?;
        session.renderer.set_clock(clock_ns);
        session.show_overlay = config.option("overlay").is_some_and(|v| v == "1" || v == "true");
        session.show_hud = !config.option("hud").is_some_and(|v| v == "0" || v == "false");
        session.load_now(&mut Blocking, index)?;
        if let Some(cam) = config.option("camera") {
            let spawn = parse_camera(cam).ok_or_else(|| format!("bad camera `{cam}`"))?;
            session.camera.place(spawn);
        }
        Ok(ViewerApp { session, platform, last_world: Some(index) })
    }

    /// The session (for tests).
    pub fn session(&self) -> &Session<GlBackend, File> {
        &self.session
    }
}

impl App for ViewerApp {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let i = ctx.input;
        let c = Controls::from_bits(i.buttons, i.pressed, i.left_stick, i.right_stick);
        let c = Controls { quit: false, ..c };
        self.session.update(&mut Blocking, &c, ctx.dt as f32).map_err(|e| e.to_string())?;
        // World switches load synchronously: finish them inside this frame.
        if self.session.loading().is_some() {
            let w = self.session.loading();
            while self.session.loading().is_some() {
                self.session.poll_loader(&mut Blocking).map_err(|e| e.to_string())?;
            }
            self.last_world = w;
        }
        Ok(())
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let stats = self.session.render((ctx.dt * 1e9) as u64).map_err(|e| e.to_string())?;
        ctx.probe.add_time_ns("render.build", stats.cpu_build_ns);
        ctx.probe.add_time_ns("render.submit", stats.cpu_submit_ns);
        ctx.probe.add_time_ns("render.wait", stats.wait_ns);
        if let Some(g) = stats.gpu_ns {
            ctx.probe.add_time_ns("render.gpu", g);
        }
        ctx.probe.count(DRAW_CALLS, stats.api_draw_calls as u64);
        ctx.probe.count(TRIANGLES, stats.triangles);
        ctx.probe.count("draw_commands", stats.draw_commands as u64);
        ctx.probe.count("batches", stats.batches as u64);
        ctx.probe.count("visible", stats.visible as u64);
        ctx.probe.count("instances", stats.instances as u64);
        self.platform.present();
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        let (w, h) = self.platform.gl().size();
        Image::from_rgba(w, h, self.platform.read_pixels()).ok_or_else(|| AppError::from("bad capture size"))
    }

    fn state_hash(&self) -> Option<u64> {
        let c = &self.session.camera;
        let mut h = harness::hash::Fnv64::new();
        h.write_f32(c.pos.x).write_f32(c.pos.y).write_f32(c.pos.z).write_f32(c.yaw).write_f32(c.pitch);
        h.write_u64(self.session.current().map_or(u64::MAX, |c| c.index as u64));
        h.write_u64(self.last_world.map_or(u64::MAX, |w| w as u64));
        Some(h.finish())
    }
}

/// A harness registry with the built-in apps and the viewer.
pub fn registry() -> harness::apps::AppRegistry {
    let mut r = harness::apps::AppRegistry::builtin();
    r.register(ViewerApp::NAME, ViewerApp::DESCRIPTION, |c| Ok(Box::new(ViewerApp::new(c)?)));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_option() {
        let s = parse_camera("1,2,3,0,0").unwrap();
        assert_eq!(s.eye, Vec3::new(1.0, 2.0, 3.0));
        assert!((s.target - Vec3::new(2.0, 2.0, 3.0)).length() < 1e-5);
        assert_eq!(parse_camera("1,2,3,4,5,6").unwrap().target, Vec3::new(4.0, 5.0, 6.0));
        assert!(parse_camera("1,2").is_none());
        assert!(parse_camera("1,2,x,4,5").is_none());
    }
}
