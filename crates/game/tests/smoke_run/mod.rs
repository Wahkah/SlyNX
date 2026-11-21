//! The end-to-end smoke runs on PC, shared by `tests/smoke.rs` (short, gated on the disc variables) and the `smoke`
//! example (the full, rerunnable tool). The Sly 1 route, the input patterns and the checks are `game::smoke`, which
//! the Switch smoke build (`dist/slynx-smoke.nro`) runs too.
//!
//! **Sly 1** ([`run`]): `game::smoke::run_sly1`. **Sly 2** ([`run_gang`], [`Title::Sly2`]): the episode 1 hub, then
//! each episode's first job world after its hub ([`SLY2_ROUTE`]). **Sly 3** ([`Title::Sly3`]): v_apt, v_ext, then
//! each later episode's hub ([`SLY3_ROUTE`]). Sly 2 and 3 stages are loaded directly and played with
//! [`gang_pattern`] (the Sly 1 pattern plus d-pad character switching); a panic in a stage is caught and reported.

// Each test binary that includes this module uses a different part of it.
#![allow(dead_code, unused_imports)]

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
    /// Sly 2 and 3: machine stages after the route, (world, machine class, the task that starts its job's machine)
    /// ([`SLY2_VEHICLES`], [`SLY3_VEHICLES`]).
    pub vehicles: Vec<Vehicle>,
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

/// A machine stage: the world, the machine's class, and the task that starts the job bringing it in (`None`: drive
/// one already there).
pub type Vehicle = (usize, i16, Option<&'static str>);

/// Sly 2 machine stages, each started through its job: the combat car (row 33, `t7_robot_war`), the hacking ship
/// (row 14, `t4_hack_station1`), the turret (row 8, `t2_turret_attack_chopper`).
pub const SLY2_VEHICLES: [Vehicle; 3] =
    [(33, 77, Some("t7_robot_war")), (14, 72, Some("t4_hack_station1")), (8, 69, Some("t2_turret_attack_chopper"))];
/// Sly 3 machine stages: the truck (o_quarry), the remote car and turret (c_ext), the biplane (h_dogfight), the raft
/// (h_sewer), the jolly boat (p_ext), the swimmer (p_dive) and a ship (p_at_sea). No task found brings a Sly 3
/// machine in yet (their command scripts or world changes do), so these drive the machines already there.
pub const SLY3_VEHICLES: [Vehicle; 8] = [
    (9, 81, None),
    (23, 85, None),
    (23, 68, None),
    (21, 82, None),
    (20, 83, None),
    (31, 84, None),
    (33, 87, None),
    (32, 88, None),
];

/// What a gang route needs from a Sly 2 or Sly 3 session.
trait GangSession {
    fn load(&mut self, world: usize) -> Result<String, String>;
    /// Drives the first active machine of `class` (`None`: the characters again); whether one was found.
    fn drive(&mut self, class: Option<i16>) -> bool;
    /// Makes the named task current, forced; whether it exists.
    fn start_task(&mut self, name: &str) -> bool;
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
    fn drive(&mut self, class: Option<i16>) -> bool {
        game::sly2::Sly2Session::drive(self, class)
    }
    fn start_task(&mut self, name: &str) -> bool {
        game::sly2::Sly2Session::start_task(self, name)
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
    fn drive(&mut self, class: Option<i16>) -> bool {
        game::sly3::Sly3Session::drive(self, class)
    }
    fn start_task(&mut self, name: &str) -> bool {
        game::sly3::Sly3Session::start_task(self, name)
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
/// interpreter, so the error counts are the stage's own). With `machine`, the player drives the first active machine
/// of that class after one settling frame (objects removed at start leave the world in it). With a task, that task is
/// made current first and the job hands its machine to the player (up to 1 s); a stage whose machine never becomes
/// active is noted in the stage's name ("no active machine"), not failed.
fn gang_stage(
    s: &mut dyn GangSession,
    world: usize,
    machine: Option<(i16, Option<&str>)>,
    seconds: f32,
    st: &mut Stage,
) {
    let t = Instant::now();
    match s.load(world) {
        Ok(name) => st.name = name,
        Err(e) => {
            st.failure = Some(format!("load: {e}"));
            return;
        }
    }
    st.load_ms = t.elapsed().as_secs_f64() * 1000.0;
    if let Some((class, task)) = machine {
        s.step(&FrameInput::default());
        let driving_class = |s: &dyn GangSession| {
            s.game().is_some_and(|g| {
                g.world.shared.driving.is_some_and(|m| g.world.is_active(m) && g.world.class(m) == Some(class))
            })
        };
        let mut how = "driving";
        if let Some(t) = task
            && s.start_task(t)
        {
            how = "job";
            for _ in 0..FPS {
                if driving_class(s) {
                    break;
                }
                s.step(&FrameInput::default());
            }
        }
        if !driving_class(s) {
            s.drive(Some(class));
            how = if driving_class(s) { "driving" } else { "no active machine" };
        }
        st.name = format!("{how} {class}: {}", st.name);
    }
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
    let plain = cfg.worlds.iter().map(|&w| (w, None));
    let driven = cfg.vehicles.iter().map(|&(w, c, t)| (w, Some((c, t))));
    for (w, machine) in plain.chain(driven) {
        let mut st = Stage { name: format!("world {w}"), ..Default::default() };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gang_stage(session.as_mut(), w, machine, cfg.seconds, &mut st)
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

/// Where the route heads (missions.md 4.2): with no job running, the waypoint of the first available task that has
/// one; during a job, a trigger shape naming the current task's successor, else the nearest active waypoint used
/// during jobs (kind 1). `None` when there is none or the
/// player is already in it.
fn route_target(s: &game::sly3::Sly3Session<NullBackend, DiscFile>) -> Option<engine::object::Handle> {
    let j = s.jobs.as_ref()?;
    let g = &s.game.as_ref()?.world;
    let wp = if j.job.is_none() {
        // 1407 names the waypoint's placement (its ID names the copy's root) or the waypoint itself.
        let named = |h, m: i32| g.sly3_id(h) == Some(m) || g.parent(h).and_then(|p| g.sly3_id(p)) == Some(m);
        (0..j.states.len())
            .filter(|&k| j.states[k] == 1)
            .filter_map(|k| j.graph.tasks[k].marker)
            .find_map(|m| s.waypoints.iter().find(|w| named(w.handle, m) && g.is_active(w.handle)))?
    } else {
        // A trigger shape (class 21, interactables.md 6) whose task requests name the current task's successor.
        let next: Vec<i32> = match j.current {
            Some(t) => j.graph.tasks[t].succ.iter().map(|&k| j.graph.tasks[k].id).collect(),
            None => (0..j.states.len())
                .filter(|&k| j.states[k] == 1 && j.graph.tasks[k].job == j.job)
                .map(|k| j.graph.tasks[k].id)
                .collect(),
        };
        {
            let l = g.sly3.as_deref()?;
            let names = |h| {
                let slot = g.slot(h);
                l.actions.iter().any(|a| {
                    Some(a.0) == slot
                        && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if next.contains(v)))
                }) || g
                    .options(h)
                    .is_some_and(|o| o.entries().iter().any(|(_, v)| v.as_i32().is_some_and(|v| next.contains(&v))))
            };
            // Or a use point (class 33) whose (task, state) pairs (1170) name it.
            if let Some(h) = g.handles().find(|&h| matches!(g.class(h), Some(21 | 33)) && g.is_active(h) && names(h)) {
                return Some(h);
            }
        }
        let at = g.position(g.player?)?;
        let d = |w: &&game::sly3::waypoints::Waypoint| {
            let p = g.position(w.handle).unwrap_or(at);
            (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)
        };
        s.waypoints.iter().filter(|w| w.kind == 1 && g.is_active(w.handle)).min_by(|a, b| d(a).total_cmp(&d(b)))?
    };
    (!wp.inside).then_some(wp.handle)
}

/// The stick that walks the played character straight to `to` (relative to the camera, as the crew reads it).
fn steer_to(s: &game::sly3::Sly3Session<NullBackend, DiscFile>, to: engine::object::Handle) -> Option<FrameInput> {
    let g = &s.game.as_ref()?.world;
    let to = g.position(to)?;
    let at = g.position(g.player?)?;
    let (dx, dy) = (to[0] - at[0], to[1] - at[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    // The stick is relative to the camera: up is the camera's heading, right its right.
    let yaw = g.shared.camera_yaw;
    let (f, r) = ([yaw.cos(), yaw.sin()], [yaw.sin(), -yaw.cos()]);
    let (x, y) = ((dx * r[0] + dy * r[1]) / len, (dx * f[0] + dy * f[1]) / len);
    Some(FrameInput { left: [x, y], ..Default::default() })
}

/// What the Sly 3 job route saw: the stage (timings and hashes) and a journal of what happened, in order.
pub struct JobRun {
    pub stage: Stage,
    pub journal: Vec<String>,
    /// The tasks that became current, by name, in order.
    pub current: Vec<String>,
    /// Where it stopped: the current task and what it waits for, as far as the route can tell.
    pub stall: String,
}

/// Sly 3, the first Venice job by input and scripts: episode 3 from a new game (v_ext at point 424), the safe house
/// (Sly taken out with Cross), then `seconds` of [`gang_pattern`]. Task changes, scripts started, loads and pauses
/// are journalled; the run reports where the job stalls.
pub fn run_sly3_job(cfg: &Config) -> JobRun {
    let disc = formats_sly3::Disc::open_path(&cfg.disc).map_err(|e| e.to_string());
    let mut s = game::sly3::Sly3Session::new(NullBackend::new(1280, 720), disc, 7);
    let mut st = Stage { name: "Venice job 1".into(), ..Default::default() };
    let mut journal = Vec::new();
    let mut current = Vec::new();
    let task_name = |s: &game::sly3::Sly3Session<NullBackend, DiscFile>, t: Option<usize>| -> String {
        let Some(j) = s.jobs.as_ref() else { return "-".into() };
        t.map_or("-".into(), |t| format!("{} ({})", j.graph.tasks[t].name, j.graph.tasks[t].id))
    };
    let t = Instant::now();
    if let Err(e) = s.start_episode(3, &game::sly3::flow::EpisodeProgress::default()) {
        st.failure = Some(format!("episode start: {e}"));
        return JobRun { stage: st, journal, current, stall: "no world".into() };
    }
    st.load_ms = t.elapsed().as_secs_f64() * 1000.0;
    st.world = s.current().map(|c| c.slot);
    if let Some(j) = s.jobs.as_ref() {
        let open: Vec<String> =
            (0..j.states.len()).filter(|&t| j.states[t] == 1).map(|t| j.graph.tasks[t].name.clone()).collect();
        journal.push(format!("start: current {}, available {open:?}", task_name(&s, j.current)));
    }
    // Stand-in for the episode intro (missions.md 4.1a): the macro module plays it and advances `t1_intro` (2064),
    // which opens 2083. Its handler is not decoded yet, so the route asks for 2064's state 3 as Sly 2's intro
    // sequence does (a checked request, command 0x1E).
    if let (Some(j), Some(g)) = (s.jobs.as_mut(), s.game.as_mut()) {
        game::sly3::jobs::set_task(j, &mut g.world, 2064, 3);
        journal.push(format!("intro stand-in: 2064 state {:?}", j.task_state(2064)));
    }
    // The safe house: step to Sly (24) and take him out.
    if s.enter_safe_house() {
        for _ in 0..6 {
            if s.safe_house.as_ref().and_then(|h| h.state()) == Some(24) {
                break;
            }
            let b = platform_api::Buttons::RIGHT.0;
            s.update(&FrameInput { pressed: b, buttons: b, ..Default::default() }, 1.0 / FPS as f32);
        }
        let b = platform_api::Buttons::B.0;
        s.update(&FrameInput { pressed: b, buttons: b, ..Default::default() }, 1.0 / FPS as f32);
        journal.push(format!("safe house: took out Sly, in safe house {}", s.in_safe_house));
        if let Some(j) = s.jobs.as_ref() {
            let open: Vec<String> = (0..j.states.len())
                .filter(|&t| j.states[t] == 1)
                .map(|t| format!("{} ({})", j.graph.tasks[t].name, j.graph.tasks[t].id))
                .collect();
            journal.push(format!("safe house: available {open:?}"));
        }
    }
    let frames = (cfg.seconds * FPS as f32) as u32;
    let mut last_task = s.jobs.as_ref().and_then(|j| j.current);
    let mut last_load = s.last_load.clone();
    let mut scripts: std::collections::BTreeSet<i32> = s.scripts.running.iter().map(|r| r.oid).collect();
    let mut paused = false;
    let mut inside: Vec<bool> = s.waypoints.iter().map(|w| w.inside).collect();
    // Steering toward the current target, and since when (frames); placing once per target.
    let mut target: Option<engine::object::Handle> = None;
    let mut steering = 0u32;
    let mut placed = false;
    for f in 0..frames {
        let t = Instant::now();
        let now = route_target(&s);
        if now != target {
            target = now;
            steering = 0;
            placed = false;
            if let Some(h) = now {
                let g = &s.game.as_ref().unwrap().world;
                journal.push(format!(
                    "{:.1} s: heading for {:?} (class {:?})",
                    f as f32 / FPS as f32,
                    g.sly3_id(h),
                    g.class(h)
                ));
            }
        }
        let steer = target.and_then(|h| steer_to(&s, h));
        steering += u32::from(steer.is_some());
        // No path-finding: after 10 s of walking straight at it, the route puts Sly at the waypoint (standing in for
        // the walk) and goes on by input and scripts from there.
        if steering > 10 * FPS && !placed {
            placed = true;
            if let Some(to) = target.and_then(|h| s.game.as_ref().unwrap().world.world_transform(h)) {
                let g = s.game.as_mut().unwrap();
                g.systems.player.place(&mut g.world, to);
                journal.push(format!("{:.1} s: placed at the waypoint (no path-finding)", f as f32 / FPS as f32));
            }
        }
        let mut input = steer.unwrap_or_else(|| gang_pattern(f));
        // At a use point within reach: stand and press the use button (Circle) twice a second.
        if let Some(h) = target {
            use engine::behavior::sly3::interactables::{UsePoint, class};
            let g = &s.game.as_ref().unwrap().world;
            if g.class(h) == Some(class::USE_POINT)
                && UsePoint::new(g, h).user_of(g, h).is_some_and(|u| Some(u) == g.player)
            {
                let b = if f % (FPS / 2) == 0 { platform_api::Buttons::A.0 } else { 0 };
                input = FrameInput { pressed: b, buttons: b, ..Default::default() };
                if b != 0 {
                    journal.push(format!(
                        "{:.1} s: pressed Circle at use point {:?}",
                        f as f32 / FPS as f32,
                        g.sly3_id(h)
                    ));
                }
            }
        }
        s.update(&input, 1.0 / FPS as f32);
        for (k, w) in s.waypoints.iter().enumerate() {
            if inside.get(k) != Some(&w.inside) {
                let g = &s.game.as_ref().unwrap().world;
                journal.push(format!(
                    "{:.1} s: waypoint {:?} (of {:?}) {}",
                    f as f32 / FPS as f32,
                    g.sly3_id(w.handle),
                    g.parent(w.handle).and_then(|p| g.sly3_id(p)),
                    if w.inside { "entered" } else { "left" }
                ));
            }
        }
        inside = s.waypoints.iter().map(|w| w.inside).collect();
        st.update_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let _ = s.render();
        st.ms.push(t.elapsed().as_secs_f64() * 1000.0);
        st.hashes.push(s.state_hash());
        let now = s.jobs.as_ref().and_then(|j| j.current);
        if now != last_task {
            let name = task_name(&s, now);
            journal.push(format!("{:.1} s: current task {name}", f as f32 / FPS as f32));
            current.push(name);
            last_task = now;
        }
        for r in &s.scripts.running {
            if scripts.insert(r.oid) {
                journal.push(format!("{:.1} s: script {} started", f as f32 / FPS as f32, r.oid));
            }
        }
        scripts.retain(|o| s.scripts.running.iter().any(|r| r.oid == *o));
        if s.last_load != last_load {
            last_load = s.last_load.clone();
            journal.push(format!("{:.1} s: load {:?}", f as f32 / FPS as f32, last_load));
        }
        if s.script_paused != paused {
            paused = s.script_paused;
            journal.push(format!("{:.1} s: run {}", f as f32 / FPS as f32, if paused { "paused" } else { "resumed" }));
        }
    }
    // Where the player is against the waypoint it was sent to.
    {
        let g = &s.game.as_ref().unwrap().world;
        let at = g.player.and_then(|p| g.position(p));
        let wps: Vec<String> = s
            .waypoints
            .iter()
            .map(|w| {
                let p = g.position(w.handle).unwrap_or_default();
                let d = at.map_or(f32::NAN, |a| ((a[0] - p[0]).powi(2) + (a[1] - p[1]).powi(2)).sqrt());
                format!(
                    "{:?} active {} at {:.0} units (enter {})",
                    g.sly3_id(w.handle),
                    g.is_active(w.handle),
                    d,
                    w.enter
                )
            })
            .collect();
        journal.push(format!("end: player at {at:?}; {} waypoints: {wps:?}", s.waypoints.len()));
    }
    let stall = match s.jobs.as_ref() {
        None => "no job system".into(),
        Some(j) => {
            let t = j.current;
            let what = t.map(|t| {
                let d = &j.graph.tasks[t];
                format!(
                    "objective string {:?}, checkpoint {:?}, waypoint {:?}, script {:?} (running: {})",
                    d.objective,
                    d.checkpoint,
                    d.marker,
                    d.script,
                    d.script.is_some_and(|x| s.scripts.is_running(x))
                )
            });
            // Its successors and what in the world names it (task requests of trigger shapes and the like).
            let succ: Vec<String> = t.map_or_else(Vec::new, |t| {
                j.graph.tasks[t]
                    .succ
                    .iter()
                    .map(|&k| format!("{} (state {})", j.graph.tasks[k].name, j.states[k]))
                    .collect()
            });
            // The available tasks' mission waypoints (option 1407): where the player would start them.
            let waypoints: Vec<String> = (0..j.states.len())
                .filter(|&k| j.states[k] == 1)
                .filter_map(|k| j.graph.tasks[k].marker.map(|m| format!("{} at {m}", j.graph.tasks[k].name)))
                .collect();
            // What in the world names the current task and its successors (trigger shapes, scripts, ...).
            let w = &s.game.as_ref().unwrap().world;
            // With no current task: the current job's available tasks.
            let ids: Vec<i32> = t.map_or_else(
                || {
                    (0..j.states.len())
                        .filter(|&k| j.states[k] == 1 && j.job.is_some() && j.graph.tasks[k].job == j.job)
                        .map(|k| j.graph.tasks[k].id)
                        .collect()
                },
                |t| {
                    core::iter::once(t)
                        .chain(j.graph.tasks[t].succ.iter().copied())
                        .map(|k| j.graph.tasks[k].id)
                        .collect()
                },
            );
            let mut naming: Vec<String> = Vec::new();
            for &id in &ids {
                let mut by: Vec<String> = Vec::new();
                for h in w.handles() {
                    let by_value =
                        w.options(h).is_some_and(|o| o.entries().iter().any(|(_, v)| v.as_i32() == Some(id)));
                    let by_action = w.sly3.as_deref().is_some_and(|l| {
                        let slot = w.slot(h);
                        l.actions.iter().any(|a| {
                            Some(a.0) == slot
                                && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if *v == id))
                        })
                    });
                    if (by_value || by_action) && w.class(h) != Some(169) && by.len() < 8 {
                        by.push(format!("class {:?} oid {:?}", w.class(h), w.sly3_id(h)));
                    }
                }
                let def = j.graph.task_ids.get(&id).map(|&k| {
                    let d = &j.graph.tasks[k];
                    format!(
                        "{} state {} auto-current {} done-when-current {} checkpoint {:?} script {:?}",
                        d.name, j.states[k], d.auto_current, d.done_on_current, d.checkpoint, d.script
                    )
                });
                naming.push(format!("{id} ({}): {by:?}", def.unwrap_or_default()));
            }
            format!(
                "current {} in job {:?}; {}; successors {succ:?}; named by {naming:?}; open waypoints {waypoints:?}; scripts running {:?}; commands not run {:?}",
                task_name(&s, t),
                j.job.map(|k| j.graph.jobs[k].name.clone()),
                what.unwrap_or_default(),
                s.scripts.running.iter().map(|r| r.oid).collect::<Vec<_>>(),
                s.scripts.unhandled
            )
        }
    };
    JobRun { stage: st, journal, current, stall }
}

/// An in-memory user folder for the Sly 2 route's save and load.
#[derive(Clone, Default)]
pub struct MemFs(std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>>);
/// A file of [`MemFs`].
pub struct MemFile(Vec<u8>);
impl platform_api::DataFile for MemFile {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, platform_api::FsError> {
        let start = (offset as usize).min(self.0.len());
        let n = buf.len().min(self.0.len() - start);
        buf[..n].copy_from_slice(&self.0[start..start + n]);
        Ok(n)
    }
}
impl platform_api::FileSystem for MemFs {
    type File = MemFile;
    fn open(&self, _: platform_api::Root, path: &str) -> Result<MemFile, platform_api::FsError> {
        self.0.lock().unwrap().get(path).cloned().map(MemFile).ok_or(platform_api::FsError::NotFound)
    }
    fn exists(&self, _: platform_api::Root, path: &str) -> bool {
        self.0.lock().unwrap().contains_key(path)
    }
    fn write_user(&self, path: &str, data: &[u8]) -> Result<(), platform_api::FsError> {
        self.0.lock().unwrap().insert(path.to_string(), data.to_vec());
        Ok(())
    }
    fn remove_user(&self, path: &str) -> Result<(), platform_api::FsError> {
        self.0.lock().unwrap().remove(path);
        Ok(())
    }
}

/// A task with no progress for this long is looked at (what would finish it) and then finished by request.
pub const SLY2_STALL_AFTER: f32 = 12.0;
/// Jobs to play through: episode 1's intro job and the first hub job.
pub const SLY2_JOBS: usize = 2;

type Sly2 = game::sly2::Sly2Session<NullBackend, DiscFile>;

fn sly2_jobs(s: &Sly2) -> Option<&engine::game::sly2_jobs::Jobs> {
    s.game.as_ref()?.world.sly2.as_deref()?.jobs.as_deref()
}

/// What in the world would finish task `t`: its exit, marker, checkpoint, row and character, and the objects whose
/// option values or action entries name the task's ID (the scripts' and volumes' requests).
fn sly2_task_needs(s: &Sly2, t: usize) -> String {
    let (Some(j), Some(g)) = (sly2_jobs(s), s.game.as_ref()) else { return "no job system".into() };
    let d = &j.graph.tasks[t];
    let w = &g.world;
    let mut naming: Vec<String> = Vec::new();
    for h in w.handles() {
        if w.class(h) == Some(133) {
            continue;
        }
        let by_value = w.options(h).is_some_and(|o| o.entries().iter().any(|(_, v)| v.as_i32() == Some(d.id)));
        let by_action = w.sly2.as_deref().is_some_and(|l| {
            let slot = w.slot(h);
            l.actions.iter().any(|a| {
                Some(a.0) == slot && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if *v == d.id))
            })
        });
        if (by_value || by_action) && naming.len() < 8 {
            naming.push(format!("class {:?} id {:?} in world {}", w.class(h), w.sly2_id(h), w.in_world(h)));
        }
    }
    let script = d.script.map(|m| format!("module {m}")).unwrap_or_else(|| "none".into());
    format!(
        "task {} ({}) in job {:?}: exit {:?}, marker {:?}, checkpoint {:?}, row {:?}, character {:?}, script {script}; \
         named by {naming:?}; successors {:?}",
        d.name,
        d.id,
        d.job.map(|k| j.graph.jobs[k].name.clone()),
        d.exit,
        d.marker,
        d.checkpoint,
        d.row,
        d.character,
        d.succ.iter().map(|&k| format!("{} (state {})", j.graph.tasks[k].name, j.states[k])).collect::<Vec<_>>()
    )
}

/// What a player would do for task `t`: walk into a trigger volume (24, 25) whose task requests (440) or sequence
/// (441, its option-642 commands) finish the task, else use an interaction spot (35) whose sequence (1070) does.
/// Each object is used once (`visited`). Returns what was done.
fn sly2_play_for(s: &mut Sly2, t: usize, visited: &mut Vec<engine::object::Handle>) -> Option<String> {
    use engine::behavior::Behavior;
    let id = sly2_jobs(s)?.graph.tasks[t].id;
    let g = s.game.as_mut()?;
    let w = &g.world;
    let names = |seq: Option<i32>, near: engine::object::Handle| -> bool {
        seq.and_then(|q| w.find_sly2_id(q, Some(near))).is_some_and(|q| {
            game::sly2::talk::sequence_commands(w, q)
                .iter()
                .any(|c| matches!(c, game::sly2::talk::SeqCommand::Task { task, .. } if *task == id))
        })
    };
    // Volumes removed at level start (option 26) that a sequence has since added (command 670) are where the
    // player is being sent next (missions.md 4.1: 1690 adds 150620, whose sequence 1697 finishes 1696 by script).
    let added_volume = |h: engine::object::Handle| {
        matches!(w.class(h), Some(24 | 25))
            && w.options(h).and_then(|o| o.i32(441)).is_some()
            && w.sly2.as_deref().is_some_and(|l| l.actions.iter().any(|a| Some(a.0) == w.slot(h) && a.1 == 26))
    };
    let fallback = w.handles().find(|&h| !visited.contains(&h) && w.in_world(h) && added_volume(h));
    let pick = w
        .handles()
        .filter(|h| !visited.contains(h) && w.in_world(*h))
        .find_map(|h| {
            let o = w.options(h)?;
            match w.class(h)? {
                24 | 25 => {
                    let by_request = engine::behavior::sly2_props::volume_requests(w, h).iter().any(|r| r.0 == id);
                    (by_request || names(o.i32(441), h)).then_some((h, 0))
                }
                // An exit whose task states (788) name the task: the player leaves through it.
                26 => w
                    .sly2
                    .as_deref()?
                    .actions
                    .iter()
                    .any(|a| {
                        Some(a.0) == w.slot(h)
                            && a.1 == 788
                            && a.2.iter().any(|x| matches!(*x, engine::object::Arg::Int(n) if n == id))
                    })
                    .then_some((h, 2)),
                35 => names(o.i32(1070), h).then_some((h, 1)),
                _ => None,
            }
        })
        .or(fallback.map(|h| (h, 0)))?;
    visited.push(pick.0);
    let oid = w.sly2_id(pick.0);
    if pick.1 == 2 {
        let id = oid?;
        // As if the player had walked out and back in: the exit is armed, then fired (only an armed exit fires,
        // level-flow.md 5.5); the load follows in a later update.
        engine::game::sly2_jobs::with(&mut g.world, |j, w| {
            arm(j, id);
            j.fire_exit_id(w, id)
        });
        return Some(format!("left by exit {oid:?}"));
    }
    if pick.1 == 0 {
        // No path-finding: the played character is put in the middle of the volume (standing in for the walk), so
        // its entry goes through the real rules (the camera's visibility mask, the classes that count); a volume
        // whose box is not read is entered by its visit flag instead.
        if let Some((w, lo, hi)) = engine::behavior::sly2_props::volume_box(&g.world, pick.0) {
            let mid = [
                (lo[0] + hi[0]) * 0.5,
                (lo[1] + hi[1]) * 0.5,
                ((lo[2] + hi[2]) * 0.5 - engine::behavior::sly2_sensors::CHARACTER_CENTRE).max(lo[2]),
            ];
            // On a floor: probed down from the top of the volume through it (none: the visit flag below).
            let top = w.transform_point([mid[0], mid[1], hi[2]]);
            let floor = g.systems.physics.queries().ground(&g.world, top, hi[2] - lo[2]).map(|f| f.0);
            let Some(floor) = floor else {
                if let Some(Behavior::Sly2Volume(v)) = g.world.behavior_mut(pick.0) {
                    v.visit = true;
                }
                return Some(format!("walked into trigger volume {oid:?} (no floor in it: entered by flag)"));
            };
            let mut to = w;
            to.t = floor;
            g.systems.player.place(&mut g.world, to);
            let who = g.world.player.and_then(|p| g.world.class(p));
            let zone = g.world.sly2_zone_mask(pick.0);
            return Some(format!(
                "walked into trigger volume {oid:?} (placed in it; player class {who:?}, camera mask {:#x}, volume zone {zone:#x})",
                g.world.shared.visibility_mask
            ));
        }
        if let Some(Behavior::Sly2Volume(v)) = g.world.behavior_mut(pick.0) {
            v.visit = true;
        }
        Some(format!("walked into trigger volume {oid:?}"))
    } else {
        let seq = g.world.options(pick.0).and_then(|o| o.i32(1070)).and_then(|q| g.world.find_sly2_id(q, Some(pick.0)));
        let starter = g.world.player;
        if let Some(seq) = seq {
            g.world
                .shared
                .sequence_requests
                .push(engine::behavior::sly2_commands::SequenceRequest::Start { sequence: seq, starter });
        }
        Some(format!("used interaction spot {oid:?}"))
    }
}

/// Takes an exit as the player walking into it would: the menu world's episode 1 exit (1329), else the world's first
/// exit. Returns whether one was fired.
fn sly2_take_exit(s: &mut Sly2, journal: &mut Vec<String>, at: f32) -> bool {
    let Some(g) = s.game.as_mut() else { return false };
    let r = engine::game::sly2_jobs::with(&mut g.world, |j, w| {
        let here = j.row;
        let e = j
            .exits
            .iter()
            .find(|e| e.id == 1329)
            .or_else(|| j.exits.iter().find(|e| e.row != Some(here)))
            .map(|e| (e.id, e.row, e.handle))?;
        arm(j, e.0);
        j.fire_exit_id(w, e.0);
        // An exit whose destination did not resolve loads nothing: journal it, and go on to the row level-flow.md
        // gives (the intro's exit to the menu world, the menu's episode 1 exit to the hub) so the route continues.
        let mut stuck = None;
        if e.1.is_none() {
            let records = engine::game::sly2_jobs::flow::exit_records(w, e.2);
            let row = SLY2_EXIT_ROWS.iter().find(|x| x.0 == e.0).map(|x| x.1);
            stuck = Some((records, row));
            if let Some(row) = row {
                j.load = Some(engine::game::sly2_jobs::flow::LoadRequest {
                    row,
                    point: engine::game::sly2_jobs::flow::DEFAULT_POINT,
                    wipe: None,
                    movies: Vec::new(),
                    task: None,
                });
            }
        }
        Some((e.0, e.1, stuck))
    })
    .flatten();
    match r {
        Some((id, row, stuck)) => {
            journal.push(format!("{at:.1} s: took exit {id} (to row {row:?})"));
            if let Some((records, spec_row)) = stuck {
                journal.push(format!(
                    "{at:.1} s: exit {id} loads nothing: flow::exit_records gives {records:?}, so no row; loading row {spec_row:?} (level-flow.md 3 and 4) to go on"
                ));
            }
            true
        }
        None => false,
    }
}

/// The exits the route takes and their rows (level-flow.md 3 and 4): the intro's exit to the menu world, the menu
/// world's episode 1 exit to the hub.
pub const SLY2_EXIT_ROWS: [(i32, i16); 2] = [(150652, 1), (1329, 2)];

/// Sly 2, episode 1 as far as scripted play and the job system go: a new game, then [`game::smoke::pattern`] input. A task
/// that does not move for [`SLY2_STALL_AFTER`] seconds is journalled with what would finish it, then requested done
/// (forced), as its event would; exits and loads are the job system's. After [`SLY2_JOBS`] jobs (or the time), the
/// progress is saved to an in-memory slot and continued, and the reloaded task states are compared.
pub fn run_sly2_job(cfg: &Config) -> JobRun {
    use engine::game::sly2_jobs::state;
    let disc = formats_sly2::Disc::open_path(&cfg.disc).map_err(|e| e.to_string());
    let mut s: Sly2 = game::sly2::Sly2Session::new(NullBackend::new(1280, 720), disc, 7);
    let mut st = Stage { name: "Sly 2 episode 1 jobs".into(), ..Default::default() };
    let mut journal = Vec::new();
    let mut current = Vec::new();
    let mut stall = String::new();
    let name = |s: &Sly2, t: Option<usize>| -> String {
        let Some(j) = sly2_jobs(s) else { return "-".into() };
        t.map_or("-".into(), |t| format!("{} ({})", j.graph.tasks[t].name, j.graph.tasks[t].id))
    };
    let t0 = Instant::now();
    if let Err(e) = s.new_game(None) {
        st.failure = Some(format!("new game: {e}"));
        return JobRun { stage: st, journal, current, stall: "no world".into() };
    }
    st.load_ms = t0.elapsed().as_secs_f64() * 1000.0;
    st.world = s.current().map(|c| c.row);
    journal.push(format!("new game: row {:?}, current {}", st.world, name(&s, sly2_jobs(&s).and_then(|j| j.current))));
    let frames = (cfg.seconds * FPS as f32) as u32;
    let mut last_task = sly2_jobs(&s).and_then(|j| j.current);
    let mut last_job_seen: Option<(String, bool)> = None;
    let mut last_row = s.current().map(|c| c.row);
    let mut since = 0.0f32;
    let mut jobs_done: Vec<String> = Vec::new();
    let mut forced = 0u32;
    let mut findings: Vec<String> = Vec::new();
    let mut visited: Vec<engine::object::Handle> = Vec::new();
    let mut played = 0u32;
    let mut hold_until = 0u32;
    let mut advanced = 0u32;
    let mut seen_sequences: Vec<engine::object::Handle> = Vec::new();
    let mut namers: std::collections::BTreeMap<i16, u32> = std::collections::BTreeMap::new();
    for f in 0..frames {
        let t = Instant::now();
        // The plain pattern: jobs and tasks choose the character (missions.md: a task's character), and volumes that
        // count only one character must see that one.
        // After being placed somewhere the character stands still for a moment (the camera catches up and the
        // volume sees it), then the pattern goes on.
        let input = if f < hold_until { FrameInput::default() } else { game::smoke::pattern(f) };
        if f == hold_until
            && f > 0
            && let Some(g) = s.game.as_ref()
        {
            let p = g.world.player.and_then(|p| g.world.position(p));
            journal.push(format!(
                "{:.1} s: after the placement: player at {p:?}, camera mask {:#x}",
                f as f32 / FPS as f32,
                g.world.shared.visibility_mask
            ));
        }
        s.update(&input, 1.0 / FPS as f32);
        st.update_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let _ = s.render();
        st.ms.push(t.elapsed().as_secs_f64() * 1000.0);
        st.hashes.push(s.state_hash());
        let at = f as f32 / FPS as f32;
        if let Some(m) = &s.message {
            journal.push(format!("{at:.1} s: message {m}"));
            st.failure = Some(m.clone());
            break;
        }
        // Task states asked for by sequences and volumes (applied by the job system next frame).
        if let Some(g) = s.game.as_ref()
            && !g.world.shared.task_requests.is_empty()
        {
            journal.push(format!("{:.1} s: task requests {:?}", f as f32 / FPS as f32, g.world.shared.task_requests));
        }
        // Sequences started (scripts, volumes, spots, animation events).
        for &h in &s.running {
            if !seen_sequences.contains(&h) {
                seen_sequences.push(h);
                let id = s.game.as_ref().and_then(|g| g.world.sly2_id(h));
                journal.push(format!("{:.1} s: sequence {id:?} started", f as f32 / FPS as f32));
            }
        }
        seen_sequences.retain(|h| s.running.contains(h));
        let row = s.current().map(|c| c.row);
        if row != last_row {
            journal.push(format!("{at:.1} s: load row {row:?}"));
            last_row = row;
        }
        let task = sly2_jobs(&s).and_then(|j| j.current);
        // The current job by name, and whether its last task is done (a load rebuilds the graph and its indices).
        // Done: its last task done or current (a job's end closes it in the same frame).
        let job_now: Option<(String, bool)> = sly2_jobs(&s).and_then(|j| {
            let d = &j.graph.jobs[j.job?];
            Some((d.name.clone(), d.last.is_some_and(|l| j.states[l] >= state::DONE || j.current == Some(l))))
        });
        if job_now.as_ref().map(|x| &x.0) != last_job_seen.as_ref().map(|x| &x.0) {
            // Looked up again in this world's graph when it has the job (the frame that closed it).
            let again = last_job_seen.as_ref().and_then(|(n, _)| {
                let j = sly2_jobs(&s)?;
                let d = j.graph.jobs.iter().find(|d| &d.name == n)?;
                Some(d.last.is_some_and(|l| j.states[l] >= state::DONE))
            });
            if let Some((n, done)) = &last_job_seen {
                let done = &(*done || again == Some(true));
                journal.push(format!("{at:.1} s: job {n} ended (last task done: {done})"));
                if *done && !jobs_done.contains(n) {
                    jobs_done.push(n.clone());
                }
            }
            if let Some((n, _)) = &job_now {
                journal.push(format!("{at:.1} s: job {n} began"));
            }
        }
        last_job_seen = job_now;
        if task != last_task {
            let n = name(&s, task);
            journal.push(format!("{at:.1} s: current task {n}"));
            current.push(n);
            last_task = task;
            since = 0.0;
        } else {
            since += 1.0 / FPS as f32;
        }
        if jobs_done.len() >= SLY2_JOBS {
            break;
        }
        // A stalled task: what would finish it, then finish it as its event would.
        if since >= SLY2_STALL_AFTER {
            since = 0.0;
            // First as the player would: walk into a trigger volume, or use an interaction spot, that finishes it.
            // A done task: walk on to what makes one of its successors current.
            let target = task.and_then(|t| {
                let j = sly2_jobs(&s)?;
                if j.states[t] < state::DONE {
                    return Some(vec![t]);
                }
                Some(j.graph.tasks[t].succ.iter().copied().filter(|&k| j.states[k] == state::AVAILABLE).collect())
            });
            let mut did = None;
            for k in target.unwrap_or_default() {
                if let Some(what) = sly2_play_for(&mut s, k, &mut visited) {
                    did = Some((k, what));
                    break;
                }
            }
            if let Some((k, what)) = did {
                journal.push(format!("{at:.1} s: played for {}: {what}", name(&s, Some(k))));
                played += 1;
                hold_until = f + 2 * FPS;
                continue;
            }
            match task {
                Some(t) => {
                    let why = sly2_task_needs(&s, t);
                    journal.push(format!("{at:.1} s: stalled: {why}"));
                    if stall.is_empty() {
                        stall = why;
                    }
                    // Which kinds of objects name the stalled task (what would have finished it).
                    if let (Some(j), Some(g)) = (sly2_jobs(&s), s.game.as_ref()) {
                        let id = j.graph.tasks[t].id;
                        let w = &g.world;
                        for h in w.handles() {
                            let named = w
                                .options(h)
                                .is_some_and(|o| o.entries().iter().any(|(_, v)| v.as_i32() == Some(id)))
                                || w.sly2.as_deref().is_some_and(|l| {
                                    let slot = w.slot(h);
                                    l.actions.iter().any(|a| {
                                        Some(a.0) == slot
                                            && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if *v == id))
                                    })
                                });
                            if named && let Some(c) = w.class(h).filter(|&c| c != 133 && c != 132) {
                                *namers.entry(c).or_insert(0) += 1;
                            }
                        }
                    }
                    let g = s.game.as_mut().unwrap();
                    // Done, then (as the player reaching the next task's marker would) its first available successor
                    // current when the done task stays current.
                    let r = engine::game::sly2_jobs::with(&mut g.world, |j, _| {
                        // Already done by play, or a reminder outside jobs that leads nowhere: only the walk to the
                        // next task's marker is stood in for.
                        let reminder = j.graph.tasks[t].job.is_none() && j.graph.tasks[t].succ.is_empty();
                        let was_done = j.states[t] >= state::DONE || reminder;
                        let ok = was_done || j.request(t, state::DONE, true);
                        // A reminder stays as it is; the next task is any available job task.
                        // Successors in a job first (reminders and side tasks after), then any available job task
                        // (the next job's marker, as the player would walk to it).
                        let succ = &j.graph.tasks[t].succ;
                        let open = |k: &usize| j.states[*k] == state::AVAILABLE;
                        let in_job = |k: &usize| j.graph.tasks[*k].job.is_some();
                        let next = (j.current == Some(t))
                            .then(|| {
                                succ.iter()
                                    .copied()
                                    .find(|k| open(k) && in_job(k))
                                    .or_else(|| (0..j.states.len()).find(|k| open(k) && in_job(k)))
                                    .or_else(|| succ.iter().copied().find(open))
                            })
                            .flatten();
                        if let Some(k) = next {
                            j.request_current(k, true);
                        }
                        (ok, was_done, next.map(|k| j.graph.tasks[k].name.clone()))
                    });
                    let (ok, was_done, next) = r.unwrap_or((false, false, None));
                    if was_done {
                        advanced += 1;
                        journal.push(format!("{at:.1} s: done by play; next made current: {next:?}"));
                    } else {
                        forced += 1;
                        journal.push(format!("{at:.1} s: forced done: {ok}; next made current: {next:?}"));
                    }
                    // Only a task with no successors ends the world's play: then the player leaves by an exit.
                    let last = sly2_jobs(&s).is_some_and(|j| j.graph.tasks[t].succ.is_empty());
                    if next.is_none() && last && !sly2_take_exit(&mut s, &mut journal, at) {
                        journal.push(format!("{at:.1} s: no successor and no exit to take"));
                    }
                }
                None => {
                    // No current task: start the first available job task (the hub's job markers are not reached).
                    let next = sly2_jobs(&s).and_then(|j| {
                        let open = |k: &usize| j.states[*k] == state::AVAILABLE;
                        (0..j.states.len())
                            .find(|k| open(k) && j.graph.tasks[*k].job.is_some())
                            // A briefing outside jobs that leads on (reminders lead nowhere).
                            .or_else(|| (0..j.states.len()).find(|k| open(k) && !j.graph.tasks[*k].succ.is_empty()))
                    });
                    match next {
                        Some(k) => {
                            let n = name(&s, Some(k));
                            let g = s.game.as_mut().unwrap();
                            engine::game::sly2_jobs::with(&mut g.world, |j, _| j.request_current(k, true));
                            journal.push(format!("{at:.1} s: no current task; started {n} (its marker not reached)"));
                        }
                        None => {
                            // What the world's tasks are waiting for (their states and prerequisites).
                            if let Some(j) = sly2_jobs(&s) {
                                let locked: Vec<String> = (0..j.states.len())
                                    .filter(|&k| j.states[k] == state::LOCKED && j.graph.tasks[k].job.is_some())
                                    .take(6)
                                    .map(|k| {
                                        let pre: Vec<String> = j.graph.tasks[k]
                                            .prereq
                                            .iter()
                                            .map(|&p| format!("{} (state {})", j.graph.tasks[p].name, j.states[p]))
                                            .collect();
                                        format!("{} needs {pre:?}", j.graph.tasks[k].name)
                                    })
                                    .collect();
                                journal.push(format!("{at:.1} s: no task available; locked job tasks: {locked:?}"));
                            }
                            // The head of the world's locked graph (a task whose prerequisites are done or have no
                            // job, e.g. a briefing the opening sequence would finish), started by force.
                            let head = sly2_jobs(&s).and_then(|j| {
                                (0..j.states.len()).find(|&k| {
                                    j.states[k] == state::LOCKED
                                        && j.graph.tasks[k]
                                            .prereq
                                            .iter()
                                            .all(|&p| j.states[p] >= state::DONE || j.graph.tasks[p].prereq.is_empty())
                                })
                            });
                            if let Some(k) = head {
                                let n = name(&s, Some(k));
                                let g = s.game.as_mut().unwrap();
                                engine::game::sly2_jobs::with(&mut g.world, |j, _| j.request_current(k, true));
                                forced += 1;
                                journal.push(format!("{at:.1} s: forced start of the locked graph's head {n}"));
                            } else if !sly2_take_exit(&mut s, &mut journal, at) {
                                journal.push(format!("{at:.1} s: no current task, none available, no exit"));
                                if stall.is_empty() {
                                    stall = "no current task, none available, no exit".into();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    journal.push(format!(
        "jobs done {jobs_done:?}, tasks forced {forced}, played for {played}, done by play then advanced {advanced}"
    ));
    findings.insert(
        0,
        format!(
            "{forced} tasks finished by request; the classes naming the stalled tasks (count): {namers:?} (114 sequences, 24/25 trigger volumes, 26 exits, 35 interaction spots, 129 and others)"
        ),
    );
    // Save and continue.
    let store = save::sly2::Sly2Store::new(MemFs::default());
    let slot = save::Slot::new(0).expect("slot 1");
    let by_id = |s: &Sly2| -> std::collections::BTreeMap<i32, i8> {
        sly2_jobs(s)
            .map_or_else(Default::default, |j| j.graph.tasks.iter().zip(&j.states).map(|(t, &st)| (t.id, st)).collect())
    };
    let before = by_id(&s);
    match s.save_game(&store, slot) {
        Ok(n) => journal.push(format!("saved (store result {n})")),
        Err(e) => journal.push(format!("save failed: {e}")),
    }
    let row_before = s.current().map(|c| c.row);
    match s.continue_game(&store, slot) {
        Ok(()) => {
            let after = by_id(&s);
            // Tasks of both worlds keep their states (a different world after continuing holds other tasks).
            let names: std::collections::BTreeMap<i32, String> = sly2_jobs(&s)
                .map(|j| j.graph.tasks.iter().map(|t| (t.id, t.name.clone())).collect())
                .unwrap_or_default();
            let differ: Vec<String> = before
                .iter()
                .filter_map(|(id, st)| {
                    let a = after.get(id).filter(|a| *a != st)?;
                    Some(format!("{} ({id}): {st} before, {a} after", names.get(id).map_or("?", |n| n.as_str())))
                })
                .collect();
            let same = differ.is_empty();
            if !same {
                journal.push(format!("states differ after the load for tasks {differ:?}"));
            }
            journal.push(format!(
                "continued: row {:?} (saved in {row_before:?}), task states kept {same}, current {}",
                s.current().map(|c| c.row),
                name(&s, sly2_jobs(&s).and_then(|j| j.current))
            ));
            if !same {
                findings.push(format!("save and load: task states differ: {differ:?}"));
            }
        }
        Err(e) => {
            journal.push(format!("continue failed: {e}"));
            st.failure.get_or_insert(format!("continue: {e}"));
        }
    }
    let seen: Vec<String> = journal
        .iter()
        .filter(|l| l.contains("loads nothing") || l.contains("forced start"))
        .map(|l| l.split_once(": ").map_or(l.clone(), |x| x.1.to_string()))
        .collect();
    findings.extend(seen);
    for f in &findings {
        journal.push(format!("finding: {f}"));
    }
    if stall.is_empty() {
        stall = "none".into();
    }
    let stall = format!("first stall: {stall}; findings: {findings:?}");
    JobRun { stage: st, journal, current, stall }
}

/// Arms exit `id` (as the player leaving its volume does), so that firing it is not ignored (level-flow.md 5.5).
fn arm(j: &mut engine::game::sly2_jobs::Jobs, id: i32) {
    if let Some(e) = j.exits.iter_mut().find(|e| e.id == id) {
        e.state = engine::game::sly2_jobs::flow::ExitState::Armed;
    }
}
