//! The Sly 1 end-to-end smoke route, shared by the PC tool (`examples/smoke.rs`, `tests/smoke.rs`) and the Switch
//! smoke build (`dist/slynx-smoke.nro`, `switch_smoke.rs`). No `std`: the caller gives the clock and is called back
//! after every frame and every stage.
//!
//! From the title, Start begins a new game and leaves through Splash's exit into Paris; then the Hideout and each
//! episode's first level (the approaches, level-flow.md 1) are entered with a wipe. In each stage the player holds a
//! fixed pattern of forward, jump (B), attack (Y) and turning for a number of seconds. Every frame's update and
//! render time, state hash and script error counts are recorded.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use engine::game::systems::FrameInput;
use formats::iso::ReadAt;
use platform_api::Buttons;
use render::Backend;

use crate::session::Session;

/// Frames per second of the fixed step.
pub const FPS: u32 = 60;
/// The frame budget (60 frames per second).
pub const BUDGET_MS: f64 = 1000.0 / 60.0;
/// The Hideout (world 2) and each episode's first level: worlds 3, 12, 21, 30 and 39 (level-flow.md 1).
pub const DEFAULT_WORLDS: [usize; 6] = [2, 3, 12, 21, 30, 39];

/// One stage of a run.
#[derive(Clone, Debug, Default)]
pub struct Stage {
    /// What the stage is (world name, or "title").
    pub name: String,
    /// World index reached (`None`: the world was never reached).
    pub world: Option<usize>,
    /// Update + render time of each frame, milliseconds.
    pub ms: Vec<f64>,
    /// The update part of each frame's time, milliseconds (the rest is drawing).
    pub update_ms: Vec<f64>,
    /// State hash after each frame.
    pub hashes: Vec<u64>,
    /// Script errors at the end of the stage, and host errors in its first and second half.
    pub script_errors: u64,
    pub host_first_half: u64,
    pub host_second_half: u64,
    /// The most frequent error kinds at the end.
    pub top_kinds: Vec<(String, u32)>,
    /// Load time of the stage's world, milliseconds (when the route measures it).
    pub load_ms: f64,
    /// Why the stage did not complete: a load error or a panic message.
    pub failure: Option<String>,
}

fn percentile_of(v: &[f64], p: f64) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    let i = (v.len().max(1) - 1) as f64 * p + 0.5;
    v.get(i as usize).copied().unwrap_or(0.0)
}

impl Stage {
    /// Percentile `p` (0 to 1) of the frame times.
    pub fn percentile(&self, p: f64) -> f64 {
        percentile_of(&self.ms, p)
    }

    /// Percentile `p` of the update times (0 when not recorded).
    pub fn update_percentile(&self, p: f64) -> f64 {
        percentile_of(&self.update_ms, p)
    }

    pub fn mean(&self) -> f64 {
        self.ms.iter().sum::<f64>() / self.ms.len().max(1) as f64
    }
}

/// The input of frame `f` of a stage: 2 s forward, 2 s forward with a jump every half second, 2 s turning with an
/// attack every half second, 2 s forward with jumps and attacks together, repeated.
pub fn pattern(f: u32) -> FrameInput {
    let t = f % (8 * FPS);
    let tap = f.is_multiple_of(FPS / 2);
    let mut i = FrameInput { left: [0.0, 1.0], ..Default::default() };
    let press = |i: &mut FrameInput, b: Buttons| {
        i.pressed |= b.0;
        i.buttons |= b.0;
    };
    match t / (2 * FPS) {
        0 => {}
        1 => {
            if tap {
                press(&mut i, Buttons::B);
            }
        }
        2 => {
            i.left = [0.7, 0.7];
            i.right = [0.5, 0.0];
            if tap {
                press(&mut i, Buttons::Y);
            }
        }
        _ => {
            if tap {
                press(&mut i, Buttons::B);
                press(&mut i, Buttons::Y);
            }
        }
    }
    i
}

/// [`pattern`] plus a d-pad switch every 4 s (1 s into each 4 s), cycling Bentley, Murray, Sly (left Sly, up Murray,
/// right Bentley in both gang sessions).
pub fn gang_pattern(f: u32) -> FrameInput {
    let mut i = pattern(f);
    if f % (4 * FPS) == FPS {
        let b = [Buttons::RIGHT, Buttons::UP, Buttons::LEFT][(f / (4 * FPS)) as usize % 3];
        i.pressed |= b.0;
        i.buttons |= b.0;
    }
    i
}

/// Script errors, host errors and the commonest error kinds of a running game.
pub fn game_errors(g: Option<&engine::game::Game>) -> (u64, u64, Vec<(String, u32)>) {
    let Some(g) = g else { return (0, 0, Vec::new()) };
    let total = g.scripts().map_or(0, |sc| sc.error_count());
    let host: u64 = g
        .script_error_kinds
        .iter()
        .filter(|(k, _)| matches!(k, script::ScriptError::Host(_)))
        .map(|(_, n)| u64::from(*n))
        .sum();
    let mut kinds: Vec<(String, u32)> = g.script_error_kinds.iter().map(|(k, n)| (format!("{k:?}"), *n)).collect();
    kinds.sort_by_key(|k| core::cmp::Reverse(k.1));
    kinds.truncate(4);
    (total, host, kinds)
}

/// What the route needs from its caller: the clock and a call after every frame.
pub struct Hooks<'a> {
    /// Monotonic nanoseconds.
    pub now_ns: &'a dyn Fn() -> u64,
    /// Called after every frame; `false` stops the route (the Switch's + button, the system asking to quit).
    pub frame_done: &'a mut dyn FnMut() -> bool,
}

/// One frame: update and render, timed; the hash after it. `false` when the caller stops the route.
fn frame<B: Backend, R: ReadAt>(s: &mut Session<B, R>, input: &FrameInput, st: &mut Stage, h: &mut Hooks<'_>) -> bool {
    let t = (h.now_ns)();
    s.update(input, 1.0 / FPS as f32).expect("update");
    let u = (h.now_ns)();
    s.render(BUDGET_MS).expect("render");
    let e = (h.now_ns)();
    st.ms.push((e - t) as f64 * 1e-6);
    st.update_ms.push((u - t) as f64 * 1e-6);
    st.hashes.push(s.state_hash());
    (h.frame_done)()
}

/// Plays `seconds` of the pattern in the current world. `false` when the caller stopped the route.
fn play<B: Backend, R: ReadAt>(s: &mut Session<B, R>, seconds: f32, st: &mut Stage, h: &mut Hooks<'_>) -> bool {
    let frames = (seconds * FPS as f32) as u32;
    let mut host_mid = 0;
    let mut go = true;
    for f in 0..frames {
        go = frame(s, &pattern(f), st, h);
        if f + 1 == frames / 2 {
            host_mid = game_errors(s.game.as_ref()).1;
        }
        if !go {
            break;
        }
    }
    let (total, host, kinds) = game_errors(s.game.as_ref());
    st.script_errors = total;
    st.host_first_half = host_mid;
    st.host_second_half = host.saturating_sub(host_mid);
    st.top_kinds = kinds;
    go
}

/// Runs the Sly 1 route on a session at the start of its disc: the title, Paris, then `worlds` for `seconds` each;
/// `progress` is told each stage as it finishes.
pub fn run_sly1<B: Backend, R: ReadAt>(
    s: &mut Session<B, R>,
    seconds: f32,
    worlds: &[usize],
    hooks: &mut Hooks<'_>,
    progress: &mut dyn FnMut(&Stage),
) -> Vec<Stage> {
    let mut stages = Vec::new();

    // The title: Start every half second until the session leaves Splash for Paris.
    let mut title = Stage { name: "title to Paris".into(), ..Default::default() };
    if let Err(e) = s.start_title() {
        title.failure = Some(format!("title: {e}"));
        progress(&title);
        stages.push(title);
        return stages;
    }
    let paris = s.find_world("Paris");
    for f in 0..20 * FPS {
        let mut i = FrameInput::default();
        if f % (FPS / 2) == FPS / 4 {
            i.pressed = Buttons::PLUS.0;
            i.buttons = Buttons::PLUS.0;
        }
        if !frame(s, &i, &mut title, hooks) {
            break;
        }
        if s.current().map(|c| c.index) == paris && paris.is_some() && !s.wiping() {
            title.world = paris;
            break;
        }
    }
    progress(&title);
    let reached_paris = title.world.is_some();
    stages.push(title);
    if !reached_paris {
        return stages;
    }
    let mut p = Stage { name: "Paris".into(), world: paris, ..Default::default() };
    let go = play(s, seconds, &mut p, hooks);
    progress(&p);
    stages.push(p);
    if !go {
        return stages;
    }

    for &w in worlds {
        let name = s.world_names().get(w).cloned().unwrap_or_else(|| format!("world {w}"));
        let mut st = Stage { name, ..Default::default() };
        let t = (hooks.now_ns)();
        s.go_to(w, None, 1);
        let mut go = true;
        for _ in 0..10 * FPS {
            go = frame(s, &FrameInput::default(), &mut st, hooks);
            if s.current().map(|c| c.index) == Some(w) && !s.wiping() {
                st.world = Some(w);
                break;
            }
            if !go {
                break;
            }
        }
        st.load_ms = ((hooks.now_ns)() - t) as f64 * 1e-6;
        if st.world.is_some() && go {
            go = play(s, seconds, &mut st, hooks);
        }
        progress(&st);
        stages.push(st);
        if !go {
            break;
        }
    }
    stages
}

/// What failed in a run, given a second run for determinism (`None`: not checked) and whether to check the frame
/// budget (release builds).
pub fn failures(a: &[Stage], b: Option<&[Stage]>, check_budget: bool) -> Vec<String> {
    let mut out = Vec::new();
    for (k, st) in a.iter().enumerate() {
        if let Some(why) = &st.failure {
            out.push(format!("{}: {why}", st.name));
        } else if st.world.is_none() {
            out.push(format!("{}: never reached", st.name));
        }
        // The world must move: a stage whose state hash never changes is frozen.
        if st.hashes.len() > 1 && st.hashes.iter().all(|&h| h == st.hashes[0]) {
            out.push(format!("{}: the state never changed", st.name));
        }
        // Host errors may come at a steady rate; a second half with clearly more than the first is growth.
        if st.host_second_half > st.host_first_half + st.host_first_half / 2 + 30 {
            out.push(format!(
                "{}: host errors grew ({} in the first half, {} in the second)",
                st.name, st.host_first_half, st.host_second_half
            ));
        }
        if check_budget && st.percentile(0.95) > BUDGET_MS {
            out.push(format!(
                "{}: 95th-percentile frame {:.2} ms over {:.2} ms",
                st.name,
                st.percentile(0.95),
                BUDGET_MS
            ));
        }
        if let Some(other) = b.and_then(|b| b.get(k)) {
            if let Some(f) = st.hashes.iter().zip(&other.hashes).position(|(x, y)| x != y) {
                out.push(format!("{}: not deterministic from frame {f}", st.name));
            }
            if other.hashes.len() != st.hashes.len() {
                out.push(format!(
                    "{}: runs differ in length ({} and {} frames)",
                    st.name,
                    st.hashes.len(),
                    other.hashes.len()
                ));
            }
        }
    }
    out
}

/// One line per stage.
pub fn report(stages: &[Stage]) -> String {
    let mut s = String::new();
    for st in stages {
        let name: String = st.name.chars().take(28).collect();
        s.push_str(&format!(
            "{:<28} {:>6} frames  mean {:>6.2} ms  p95 {:>6.2} ms  max {:>7.2} ms  update p95 {:>6.2} ms  load {:>6.0} ms  script errors {:>6}  host {}+{}  {:?}\n",
            name,
            st.ms.len(),
            st.mean(),
            st.percentile(0.95),
            st.percentile(1.0),
            st.update_percentile(0.95),
            st.load_ms,
            st.script_errors,
            st.host_first_half,
            st.host_second_half,
            st.top_kinds,
        ));
    }
    s
}
