//! Harness apps for the Sly 2 and Sly 3 sessions on PC (OpenGL, offscreen when headless), with captures: what the
//! Switch builds draw, for checking lighting, characters and HUD before a hardware test.
//!
//! - `sly2`: options `world=ROW` (a level-table row, or a world id such as `0x41`; default the nightclub hub),
//!   `disc=PATH` (default `SLYNX_DISC_SLY2`);
//! - `sly3`: options `world=NAME` (a world name such as `v_apt`, or `slotN`; default v_apt), `disc=PATH` (default
//!   `SLYNX_DISC_SLY3`).

use std::fs::File;

use harness::perf::{DRAW_CALLS, TRIANGLES};
use harness::{App, AppConfig, AppError, FrameContext, Image};
use platform_pc::{PcConfig, PcPlatform};
use render::FrameStats;
use render::gl::GlBackend;

use crate::pc::{clock_ns, gl_backend, harness_input};
use crate::sly2::{HUB_WORLD_ID, Sly2Session};
use crate::sly3::Sly3Session;

fn disc_path(config: &AppConfig, var: &str) -> Result<String, AppError> {
    if let Some(p) = config.option("disc").filter(|p| !p.is_empty()) {
        return Ok(p.to_string());
    }
    std::env::var(var).ok().filter(|p| !p.is_empty()).ok_or_else(|| AppError::from(format!("no disc: set {var}")))
}

fn platform(config: &AppConfig) -> Result<PcPlatform, AppError> {
    let mut pc = PcConfig::headless(config.width, config.height);
    if !config.headless {
        pc = PcConfig { headless: false, vsync: false, audio: false, controllers: false, ..pc };
    }
    Ok(PcPlatform::new(pc)?)
}

fn capture(p: &PcPlatform) -> Result<Image, AppError> {
    let (w, h) = p.gl().size();
    Image::from_rgba(w, h, p.read_pixels()).ok_or_else(|| AppError::from("bad capture size"))
}

fn report(ctx: &mut FrameContext<'_>, stats: &FrameStats) {
    ctx.probe.add_time_ns("render.build", stats.cpu_build_ns);
    ctx.probe.add_time_ns("render.submit", stats.cpu_submit_ns);
    ctx.probe.count(DRAW_CALLS, stats.api_draw_calls as u64);
    ctx.probe.count(TRIANGLES, stats.triangles);
    ctx.probe.count("draw_commands", stats.draw_commands as u64);
}

/// The Sly 2 session as a harness app.
pub struct Sly2App {
    // Field order is drop order: the session (and its backend) before the platform.
    session: Sly2Session<GlBackend, File>,
    platform: PcPlatform,
}

impl Sly2App {
    pub const NAME: &'static str = "sly2";
    pub const DESCRIPTION: &'static str =
        "SlyNX on Sly 2 (OpenGL); options: world=ROW or 0xID, disc=PATH, place_bomb=1 (a Trigger Bomb placed ahead)";

    pub fn new(config: &AppConfig) -> Result<Sly2App, AppError> {
        let platform = platform(config)?;
        let backend = gl_backend(&platform)?;
        let path = disc_path(config, "SLYNX_DISC_SLY2")?;
        let disc = formats_sly2::Disc::open_path(&path).map_err(|e| format!("{path}: {e}"));
        let mut session = Sly2Session::new(backend, disc, config.seed);
        session.renderer.set_clock(clock_ns);
        let w = config.option("world");
        let row = match w {
            Some(s) if s.starts_with("0x") => {
                u32::from_str_radix(&s[2..], 16).ok().and_then(|id| session.find_world(id))
            }
            Some(s) => s.trim_start_matches("row").parse::<usize>().ok(),
            None => session.find_world(HUB_WORLD_ID),
        }
        .ok_or_else(|| format!("no world `{}`", w.unwrap_or("hub")))?;
        session.load_row(row)?;
        if config.option("place_bomb").is_some() {
            place_bomb(&mut session);
        }
        Ok(Sly2App { session, platform })
    }
}

impl App for Sly2App {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let input = harness_input(ctx.input);
        self.session.update(&input, ctx.dt as f32);
        Ok(())
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let stats = self.session.render().map_err(|e| e.to_string())?;
        report(ctx, &stats);
        self.platform.present();
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        capture(&self.platform)
    }
}

/// Testing aid for the attached objects' drawing: takes Bentley's Trigger Bomb out of his pool (out of the
/// characters, as a copy would be) and places it halfway between Bentley and the player, on the player's floor, as
/// if thrown and landed.
/// The engine does not place gadget copies yet (gadgets.md 6).
fn place_bomb(session: &mut Sly2Session<GlBackend, File>) {
    use crate::sly2::attached::{BOMB_CLASS, pool_template};
    let Some(g) = session.game.as_mut() else { return };
    let w = &mut g.world;
    let Some(bomb) = w.handles().find(|&h| w.class(h) == Some(BOMB_CLASS) && pool_template(w, h) && w.is_active(h))
    else {
        return;
    };
    let Some(p) = w.player.and_then(|h| w.world_transform(h)) else { return };
    let Some(carrier) = w.world_transform(bomb) else { return };
    w.reparent(bomb, None, false);
    // Halfway from its carrier to the player, on the player's floor.
    let mut at = p;
    at.t = [(p.t[0] + carrier.t[0]) * 0.5, (p.t[1] + carrier.t[1]) * 0.5, p.t[2]];
    w.set_local(bomb, at);
    w.set_in_world(bomb, true);
    w.update_transforms();
}

/// The Sly 3 session as a harness app.
pub struct Sly3App {
    session: Sly3Session<GlBackend, File>,
    platform: PcPlatform,
}

impl Sly3App {
    pub const NAME: &'static str = "sly3";
    pub const DESCRIPTION: &'static str = "SlyNX on Sly 3 (OpenGL); options: world=NAME or slotN, disc=PATH";

    pub fn new(config: &AppConfig) -> Result<Sly3App, AppError> {
        let platform = platform(config)?;
        let backend = gl_backend(&platform)?;
        let path = disc_path(config, "SLYNX_DISC_SLY3")?;
        let disc = formats_sly3::Disc::open_path(&path).map_err(|e| format!("{path}: {e}"));
        let mut session = Sly3Session::new(backend, disc, config.seed);
        session.renderer.set_clock(clock_ns);
        let w = config.option("world").unwrap_or("v_apt");
        let slot = match w.strip_prefix("slot") {
            Some(n) => n.parse::<usize>().ok(),
            None => session.find_world(w),
        }
        .ok_or_else(|| format!("no world `{w}`"))?;
        session.load_slot(slot)?;
        Ok(Sly3App { session, platform })
    }
}

impl App for Sly3App {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let input = harness_input(ctx.input);
        self.session.update(&input, ctx.dt as f32);
        Ok(())
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let stats = self.session.render().map_err(|e| e.to_string())?;
        report(ctx, &stats);
        self.platform.present();
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        capture(&self.platform)
    }
}
