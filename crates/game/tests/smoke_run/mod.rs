//! The end-to-end smoke runs on PC, shared by `tests/smoke.rs` (short, gated on the disc variables) and the `smoke`
//! example (the full, rerunnable tool). The Sly 1 route, the input patterns and the checks are `game::smoke`, which
//! the Switch smoke build (`dist/slynx-smoke.nro`) runs too.
//!
//! **Sly 1** ([`run`]): `game::smoke::run_sly1`. **Sly 2** ([`run_gang`], [`Title::Sly2`]): the episode 1 hub, then
//! each episode's first job world after its hub ([`SLY2_ROUTE`]). **Sly 3** ([`Title::Sly3`]): v_apt, v_ext, then
//! each later episode's hub ([`SLY3_ROUTE`]). Sly 2 and 3 stages are loaded directly and played with
//! [`gang_pattern`] (the Sly 1 pattern plus d-pad character switching); a panic in a stage is caught and reported.

#![allow(dead_code)]

use std::time::Instant;

use engine::game::systems::FrameInput;
use game::pc::open_disc;
use game::session::{Session, SessionOptions};
pub use game::smoke::{DEFAULT_WORLDS, FPS, Stage, failures, gang_pattern, report};
use game::smoke::{Hooks, game_errors, run_sly1};
use render::NullBackend;

/// What to run.
#[derive(Clone, Debug)]
pub struct Config {
    pub disc: std::path::PathBuf,
    /// Seconds of play per stage.
    pub seconds: f32,
    /// Sly 1: worlds after Paris, by index; Sly 2: level rows; Sly 3: world slots.
    pub worlds: Vec<usize>,
}

/// Monotonic nanoseconds since the first call.
fn now_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// Runs the whole Sly 1 route; `progress` is told each stage as it finishes.
pub fn run(cfg: &Config, mut progress: impl FnMut(&Stage)) -> Vec<Stage> {
    let disc = open_disc(cfg.disc.to_str().expect("disc path"));
    let mut s = Session::new(NullBackend::new(1280, 720), disc, SessionOptions::default());
    let mut hooks = Hooks { now_ns: &now_ns, frame_done: &mut || true };
    run_sly1(&mut s, cfg.seconds, &cfg.worlds, &mut hooks, &mut progress)
}

/// Which game a gang route runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    Sly2,
    Sly3,
}

/// Sly 2: the episode 1 hub (row 2), then the first job world after each episode's hub (level-flow.md 4: hubs 2,
/// 8, 12, 14, 17, 27, 32, 38): `f_nightclub_cellar`, `i_palace_int`, `i_temple_int`, `p_prison_int`,
/// `p_castle_waterrails`, `c_train_cabins`, `c_sawmill_mulch`, `a_blimp_int`.
pub const SLY2_ROUTE: [usize; 9] = [2, 3, 9, 13, 15, 19, 28, 33, 39];
/// Sly 3: v_apt (slot 5), v_ext (3), then the hubs of the later episodes (gameplay-overview.md 5): o_ext, h_ext,
/// c_ext, p_ext, m_ext.
pub const SLY3_ROUTE: [usize; 7] = [5, 3, 8, 15, 23, 31, 35];

/// What a gang route needs from a Sly 2 or Sly 3 session.
trait GangSession {
    fn load(&mut self, world: usize) -> Result<String, String>;
    fn step(&mut self, input: &FrameInput);
    fn draw(&mut self);
    fn hash(&self) -> u64;
    fn game(&self) -> Option<&engine::game::Game>;
}

type DiscFile = std::fs::File;

impl GangSession for game::sly2::Sly2Session<NullBackend, DiscFile> {
    fn load(&mut self, row: usize) -> Result<String, String> {
        self.load_row(row)?;
        Ok(self.status_lines().first().cloned().unwrap_or_default())
    }
    fn step(&mut self, input: &FrameInput) {
        self.update(input, 1.0 / FPS as f32);
    }
    fn draw(&mut self) {
        self.render().expect("render");
    }
    fn hash(&self) -> u64 {
        self.state_hash()
    }
    fn game(&self) -> Option<&engine::game::Game> {
        self.game.as_ref()
    }
}

impl GangSession for game::sly3::Sly3Session<NullBackend, DiscFile> {
    fn load(&mut self, slot: usize) -> Result<String, String> {
        self.load_slot(slot)?;
        Ok(self.status_lines().first().cloned().unwrap_or_default())
    }
    fn step(&mut self, input: &FrameInput) {
        self.update(input, 1.0 / FPS as f32);
    }
    fn draw(&mut self) {
        self.render().expect("render");
    }
    fn hash(&self) -> u64 {
        self.state_hash()
    }
    fn game(&self) -> Option<&engine::game::Game> {
        self.game.as_ref()
    }
}

/// Loads `world` and plays `seconds` of [`gang_pattern`] in it, timing each frame (each load starts a fresh
/// interpreter, so the error counts are the stage's own).
fn gang_stage(s: &mut dyn GangSession, world: usize, seconds: f32, st: &mut Stage) {
    let t = Instant::now();
    match s.load(world) {
        Ok(name) => st.name = name,
        Err(e) => {
            st.failure = Some(format!("load: {e}"));
            return;
        }
    }
    st.load_ms = t.elapsed().as_secs_f64() * 1000.0;
    st.world = Some(world);
    let frames = (seconds * FPS as f32) as u32;
    let mut host_mid = 0;
    for f in 0..frames {
        let t = Instant::now();
        s.step(&gang_pattern(f));
        st.update_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        s.draw();
        st.ms.push(t.elapsed().as_secs_f64() * 1000.0);
        st.hashes.push(s.hash());
        if f + 1 == frames / 2 {
            host_mid = game_errors(s.game()).1;
        }
    }
    let (total, host, kinds) = game_errors(s.game());
    st.script_errors = total;
    st.host_first_half = host_mid;
    st.host_second_half = host.saturating_sub(host_mid);
    st.top_kinds = kinds;
}

/// Runs a Sly 2 or Sly 3 route (`cfg.worlds`: level rows or world slots) in one session; `progress` is told each
/// stage. A panic ends only its stage.
pub fn run_gang(title: Title, cfg: &Config, mut progress: impl FnMut(&Stage)) -> Vec<Stage> {
    let mut session: Box<dyn GangSession> = match title {
        Title::Sly2 => {
            let disc = formats_sly2::Disc::open_path(&cfg.disc).map_err(|e| e.to_string());
            Box::new(game::sly2::Sly2Session::new(NullBackend::new(1280, 720), disc, 7))
        }
        Title::Sly3 => {
            let disc = formats_sly3::Disc::open_path(&cfg.disc).map_err(|e| e.to_string());
            Box::new(game::sly3::Sly3Session::new(NullBackend::new(1280, 720), disc, 7))
        }
    };
    let mut stages = Vec::new();
    for &w in &cfg.worlds {
        let mut st = Stage { name: format!("world {w}"), ..Default::default() };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gang_stage(session.as_mut(), w, cfg.seconds, &mut st)
        }));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            st.failure = Some(format!("panicked after {} frames: {msg}", st.ms.len()));
        }
        progress(&st);
        stages.push(st);
    }
    stages
}
