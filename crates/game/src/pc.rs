//! The PC side: the harness apps and the windowed game's set-up.
//!
//! Two harness apps (`slynx_game` example, or any binary that registers [`registry`]):
//!
//! - `sly1`: the game rendered with OpenGL (offscreen when headless), with captures;
//! - `sly1-cpu`: the same game on the renderer's null backend: no GPU, captures are blank. For CPU measurements and
//!   determinism runs on machines without a GL context.
//!
//! Options: `world=NAME` (default Paris), `disc=PATH` (default `SLYNX_DISC_SLY1`), `triggers=0` (the camera does
//! not trigger exits and volumes), `collect=0` (nor collect pickups), `overlay=1`, `camera=x,y,z,tx,ty,tz`,
//! `wipes=0` (worlds change at once), `hud=0` (no HUD or menus), `start=title` (the start sequence from Splash),
//! `follow=0` (the free camera instead of the follow camera).

use std::fs::File;
use std::sync::OnceLock;
use std::time::Instant;

use formats::disc::Disc;
use harness::perf::{DRAW_CALLS, TRIANGLES};
use harness::{App, AppConfig, AppError, FrameContext, Image};
use platform_pc::{PcConfig, PcPlatform};
use render::gl::{GlBackend, GlTarget};
use render::{Backend, NullBackend};

use engine::game::systems::FrameInput;

use crate::session::{Session, SessionOptions};
use crate::settings::world_index;

/// Monotonic nanoseconds for the renderer's CPU timings.
pub fn clock_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// The disc image path: `explicit`, else `SLYNX_DISC_SLY1`.
pub fn disc_path(explicit: Option<&str>) -> Result<String, String> {
    if let Some(p) = explicit.filter(|p| !p.is_empty()) {
        return Ok(p.to_string());
    }
    std::env::var("SLYNX_DISC_SLY1")
        .ok()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "no disc image: pass disc=PATH or set SLYNX_DISC_SLY1".to_string())
}

/// Opens the disc image at `path`.
pub fn open_disc(path: &str) -> Result<Disc<File>, String> {
    Disc::open_path(path).map_err(|e| format!("{path}: {e}"))
}

/// The OpenGL backend on the platform's context, drawing into its default framebuffer.
pub fn gl_backend(platform: &PcPlatform) -> Result<GlBackend, String> {
    let ctx = platform.gl();
    let (width, height) = ctx.size();
    let target = GlTarget::External { framebuffer: ctx.default_framebuffer(), width, height };
    // SAFETY: the platform's context is current on this thread and outlives the backend (the app drops the session
    // before the platform); the loader is that context's.
    unsafe { GlBackend::new(&mut |name| ctx.get_proc_address(name), target) }.map_err(|e| e.to_string())
}

/// The engine's input from the harness's.
pub fn harness_input(i: &harness::InputState) -> FrameInput {
    FrameInput {
        buttons: i.buttons,
        pressed: i.pressed,
        released: i.released,
        left: i.left_stick,
        right: i.right_stick,
    }
}

fn flag(config: &AppConfig, key: &str, default: bool) -> bool {
    config.option(key).map_or(default, |v| v == "1" || v == "true")
}

fn parse_camera(s: &str) -> Option<([f32; 3], [f32; 3])> {
    let v: Vec<f32> = s.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    match v.as_slice() {
        [x, y, z, tx, ty, tz] => Some(([*x, *y, *z], [*tx, *ty, *tz])),
        _ => None,
    }
}

/// Makes a session on `backend` from harness options and loads the first world.
pub fn session_from_config<B: Backend>(backend: B, config: &AppConfig) -> Result<Session<B, File>, AppError> {
    let path = disc_path(config.option("disc"))?;
    let disc = open_disc(&path)?;
    let which = config.option("world").unwrap_or(crate::settings::FIRST_WORLD);
    let index = world_index(&disc.worlds, which).ok_or_else(|| format!("no world `{which}` on the disc"))?;
    let options = SessionOptions {
        camera_triggers: flag(config, "triggers", true),
        camera_collects: flag(config, "collect", true),
        overlay: flag(config, "overlay", false),
        seed: config.seed,
        wipes: flag(config, "wipes", true),
        frontend: flag(config, "hud", true),
        follow_camera: flag(config, "follow", true),
    };
    let mut s = Session::new(backend, Ok(disc), options);
    s.renderer.set_clock(clock_ns);
    if config.option("start") == Some("title") {
        s.start_title()?;
    } else {
        s.load_world(index, None, 0)?;
    }
    if let Some(c) = config.option("camera") {
        let (eye, target) = parse_camera(c).ok_or_else(|| format!("bad camera `{c}`"))?;
        if let Some(g) = s.game.as_mut() {
            g.systems.camera.cut(eye, target);
        }
    }
    Ok(s)
}

/// The game as a harness app, generic over the renderer backend.
pub struct GameApp<B: Backend> {
    // Field order is drop order: the session (and its backend) before the platform.
    session: Session<B, File>,
    platform: Option<PcPlatform>,
    size: (u32, u32),
}

impl GameApp<GlBackend> {
    /// Name of the OpenGL app.
    pub const NAME: &'static str = "sly1";
    /// Description.
    pub const DESCRIPTION: &'static str =
        "SlyNX on Sly 1; options: world=NAME, disc=PATH, triggers=0, collect=0, overlay=1, camera=x,y,z,tx,ty,tz";

    /// The game rendered with OpenGL.
    pub fn new(config: &AppConfig) -> Result<GameApp<GlBackend>, AppError> {
        let mut pc = PcConfig::headless(config.width, config.height);
        if !config.headless {
            pc = PcConfig { headless: false, vsync: false, audio: false, controllers: false, ..pc };
        }
        let platform = PcPlatform::new(pc)?;
        let backend = gl_backend(&platform)?;
        let session = session_from_config(backend, config)?;
        Ok(GameApp { session, platform: Some(platform), size: (config.width, config.height) })
    }
}

impl GameApp<NullBackend> {
    /// Name of the CPU-only app.
    pub const NAME: &'static str = "sly1-cpu";
    /// Description.
    pub const DESCRIPTION: &'static str = "SlyNX on Sly 1 without a GPU (null renderer); same options as sly1";

    /// The game on the null renderer.
    pub fn new(config: &AppConfig) -> Result<GameApp<NullBackend>, AppError> {
        let session = session_from_config(NullBackend::new(config.width, config.height), config)?;
        Ok(GameApp { session, platform: None, size: (config.width, config.height) })
    }
}

impl<B: Backend> GameApp<B> {
    /// The session.
    pub fn session(&self) -> &Session<B, File> {
        &self.session
    }

    /// The session, mutably.
    pub fn session_mut(&mut self) -> &mut Session<B, File> {
        &mut self.session
    }
}

impl<B: Backend> App for GameApp<B> {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let input = harness_input(ctx.input);
        let t = ctx.probe.scope("update.engine");
        self.session.update(&input, ctx.dt as f32).map_err(|e| e.to_string())?;
        drop(t);
        if let Some(g) = &self.session.game {
            let c = g.counters;
            ctx.probe.count("objects.updated", c.updated as u64);
            ctx.probe.count("script.events", c.events as u64);
            ctx.probe.count("render.changes", c.render_changes as u64);
        }
        Ok(())
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let stats = self.session.render(ctx.dt * 1e3).map_err(|e| e.to_string())?;
        ctx.probe.add_time_ns("render.build", stats.cpu_build_ns);
        ctx.probe.add_time_ns("render.submit", stats.cpu_submit_ns);
        if let Some(g) = stats.gpu_ns {
            ctx.probe.add_time_ns("render.gpu", g);
        }
        ctx.probe.count(DRAW_CALLS, stats.api_draw_calls as u64);
        ctx.probe.count(TRIANGLES, stats.triangles);
        ctx.probe.count("draw_commands", stats.draw_commands as u64);
        ctx.probe.count("visible", stats.visible as u64);
        if let Some(p) = &mut self.platform {
            p.present();
        }
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        match &self.platform {
            Some(p) => {
                let (w, h) = p.gl().size();
                Image::from_rgba(w, h, p.read_pixels()).ok_or_else(|| AppError::from("bad capture size"))
            }
            None => {
                let (w, h) = self.size;
                Image::from_rgba(w, h, vec![0; (w * h * 4) as usize]).ok_or_else(|| AppError::from("bad size"))
            }
        }
    }

    fn state_hash(&self) -> Option<u64> {
        Some(self.session.state_hash())
    }
}

/// A harness registry with the built-in apps and the game's two apps.
pub fn registry() -> harness::apps::AppRegistry {
    let mut r = harness::apps::AppRegistry::builtin();
    r.register(GameApp::<GlBackend>::NAME, GameApp::<GlBackend>::DESCRIPTION, |c| {
        Ok(Box::new(GameApp::<GlBackend>::new(c)?))
    });
    r.register(GameApp::<NullBackend>::NAME, GameApp::<NullBackend>::DESCRIPTION, |c| {
        Ok(Box::new(GameApp::<NullBackend>::new(c)?))
    });
    r
}
