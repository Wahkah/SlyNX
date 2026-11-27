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

/// A waypoint for one character (custom property 1991, else option 1483; missions.md 4.2) when another is played: the route goes through the safe house (entered at once, standing in for the walk to it) and takes that
/// character out (characters.md 6), as the start does for Sly.
fn take_out_for(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    h: engine::object::Handle,
    f: u32,
    journal: &mut Vec<String>,
) {
    // The character the waypoint lets start its task: custom property 1991 (an owner code: Sly 24, Bentley 25,
    // Murray 26; missions.md 4.2), on the waypoint or its placement, else its class (option 1483).
    let Some(class) = s.waypoints.iter().find(|w| w.handle == h).map(|w| w.player_class) else { return };
    let owner = s.game.as_ref().and_then(|g| {
        let w = &g.world;
        let l = w.sly3.as_deref()?;
        [Some(h), w.parent(h)].into_iter().flatten().find_map(|o| {
            let slot = w.slot(o);
            l.actions.iter().find_map(|a| match (&a.2[..], Some(a.0) == slot) {
                ([engine::object::Arg::Short(21, 1991), engine::object::Arg::Int(v)], true) => Some(*v),
                _ => None,
            })
        })
    });
    let (want, code) = match (owner, class) {
        (Some(24), _) | (None, 52) => (52, 24),
        (Some(25), _) | (None, 54) => (54, 25),
        (Some(26), _) | (None, 53) => (53, 26),
        _ => return,
    };
    let playing = s.game.as_ref().and_then(|g| g.world.player.and_then(|p| g.world.class(p)));
    if playing == Some(want) {
        return;
    }
    // A script holding the pad (the arrival's) is waited out first: the safe house reads no input meanwhile.
    for _ in 0..20 * FPS {
        if !s.script_paused && !s.screens_hold() {
            break;
        }
        s.update(&FrameInput::default(), 1.0 / FPS as f32);
    }
    if !s.enter_safe_house() {
        return;
    }
    for _ in 0..6 {
        if s.safe_house.as_ref().and_then(|x| x.state()) == Some(code) {
            break;
        }
        let b = platform_api::Buttons::RIGHT.0;
        s.update(&FrameInput { pressed: b, buttons: b, ..Default::default() }, 1.0 / FPS as f32);
    }
    let shown = s.safe_house.as_ref().and_then(|x| x.state());
    let b = platform_api::Buttons::B.0;
    s.update(&FrameInput { pressed: b, buttons: b, ..Default::default() }, 1.0 / FPS as f32);
    for _ in 0..3 {
        s.update(&FrameInput::default(), 1.0 / FPS as f32);
    }
    journal.push(format!(
        "{:.1} s: waypoint for class {want}: took out {code} at the safe house (shown {shown:?}), playing {:?}",
        f as f32 / FPS as f32,
        s.game.as_ref().and_then(|g| g.world.player.and_then(|p| g.world.class(p)))
    ));
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
            // Or a use point the next task's switches bring in while it is available (its own script answers the
            // use), then such a trigger shape.
            let brought: Vec<engine::object::Handle> = next
                .iter()
                .filter_map(|id| j.graph.task_ids.get(id))
                .flat_map(|&k| j.graph.tasks[k].switches.iter())
                .filter(|sw| sw.states.contains(&1))
                .filter_map(|sw| g.find_sly3_id(sw.object, None))
                .filter(|&h| g.is_active(h))
                .collect();
            for class in [33, 21] {
                if let Some(&h) = brought.iter().find(|&&h| g.class(h) == Some(class)) {
                    return Some(h);
                }
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

/// The gauntlet's bomb doors (missions.md 4.1a): a door whose placement names the current task (custom property 2731)
/// opens when a bomb hits it. Stand-in (how play makes Bentley the player here is not traced): the route switches to
/// Bentley and puts him at the door (no path-finding). It then presses L to plant his bomb and L again half a second
/// later to set it off, and switches back to Sly once the task is done.
fn bomb_input(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    bombing: &mut (Option<(i32, u32)>, i32),
) -> Option<FrameInput> {
    use engine::gang::sly3::Member;
    let (state, checked) = (&mut bombing.0, &mut bombing.1);
    let time = f as f32 / FPS as f32;
    if let Some((task, _)) = *state
        && s.jobs.as_ref().and_then(|j| j.task_state(task)).is_some_and(|st| st >= 2)
    {
        *state = None;
        s.game.as_mut()?.world.shared.sly3_switch_to = Some(Member::Sly);
        journal.push(format!("{time:.1} s: bomb door {task} done; stand-in: back to Sly"));
        return None;
    }
    let task = s.jobs.as_ref().and_then(|j| j.current.map(|t| j.graph.tasks[t].id))?;
    if state.is_none() {
        // Each current task is looked at once.
        if *checked == task {
            return None;
        }
        *checked = task;
        let g = s.game.as_mut()?;
        let w = &g.world;
        let l = w.sly3.as_deref()?;
        let names = |p: engine::object::Handle| {
            let slot = w.slot(p);
            l.actions.iter().any(|a| {
                Some(a.0) == slot
                    && a.1 == 8
                    && matches!(a.2.first(), Some(engine::object::Arg::Short(_, 2731)))
                    && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if *v == task))
            })
        };
        let placement = w.handles().find(|&h| w.class(h) == Some(8) && names(h))?;
        let door = w.children(placement).find(|&c| w.module(c) >= 0)?;
        let at = w.world_transform(door)?;
        g.world.shared.sly3_switch_to = Some(Member::Bentley);
        journal.push(format!(
            "{time:.1} s: bomb door {:?} for task {task}; stand-in: switched to Bentley",
            g.world.sly3_id(placement)
        ));
        *state = Some((task, f + FPS));
        // Placed next frame, once Bentley plays.
        let _ = at;
        return Some(FrameInput::default());
    }
    // The set-up scene holds the pad: the presses wait until it is free.
    if s.script_paused || s.screens_hold() {
        if let Some((_, first)) = state.as_mut() {
            *first = f + FPS;
        }
        return Some(FrameInput::default());
    }
    let (_, first) = (*state)?;
    if f + FPS / 2 == first {
        // Bentley at the door (no path-finding).
        let g = s.game.as_mut()?;
        let at = {
            let w = &g.world;
            let l = w.sly3.as_deref()?;
            let slot_names = |p: engine::object::Handle| {
                let slot = w.slot(p);
                l.actions.iter().any(|a| {
                    Some(a.0) == slot
                        && a.1 == 8
                        && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(v) if *v == task))
                })
            };
            w.handles()
                .find(|&h| w.class(h) == Some(8) && slot_names(h))
                .and_then(|p| w.children(p).find(|&c| w.module(c) >= 0))
                .and_then(|door| w.world_transform(door))
        };
        if let Some(at) = at {
            g.systems.player.place(&mut g.world, at);
            journal.push(format!("{time:.1} s: Bentley at the door (placed, no path-finding)"));
        }
    }
    if f == first || f == first + FPS / 2 {
        journal.push(format!(
            "{time:.1} s: pressed L as {:?}: {}",
            s.game.as_ref().and_then(|g| g.world.player.and_then(|p| g.world.class(p))),
            if f == first { "plant the bomb" } else { "set it off" }
        ));
        let b = platform_api::Buttons::L.0;
        return Some(FrameInput { pressed: b, buttons: b, ..Default::default() });
    }
    Some(FrameInput::default())
}

/// Guards a tracking trigger is waiting on (missions.md 4.1a, `t1_gauntlet_lasers_2`: 139753 counts guards, class 58,
/// and its script starts once none is left inside): while such a trigger is in the world with guards inside, Sly
/// stands beside the nearest one that is not knocked out (put there, no path-finding) and jumps and attacks
/// (Cross, then Square) once a second, which reaches the guard as an attack (`game::sly3::crew`).
fn knockout_input(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    knocking: &mut Option<(engine::object::Handle, u32)>,
) -> Option<FrameInput> {
    use engine::behavior::Behavior;
    let time = f as f32 / FPS as f32;
    let g = s.game.as_mut()?;
    let w = &g.world;
    let up = |h: engine::object::Handle| matches!(w.behavior(h), Some(Behavior::Sly2Guard(x)) if !x.knocked_out());
    let counts_guards = |t: engine::object::Handle| {
        w.options(t).is_some_and(|o| o.bool_or(443, false) && o.all(433).any(|v| v.as_i32() == Some(58)))
    };
    let waiting: Vec<engine::object::Handle> = w
        .handles()
        .filter(|&t| w.in_world(t) && counts_guards(t))
        .filter_map(|t| match w.behavior(t) {
            Some(Behavior::Sly2Volume(v)) => Some(v.members.clone()),
            _ => None,
        })
        .flatten()
        .filter(|&h| up(h))
        .collect();
    if let Some((h, _)) = *knocking
        && !waiting.contains(&h)
    {
        let out = matches!(w.behavior(h), Some(Behavior::Sly2Guard(x)) if x.knocked_out());
        journal.push(format!(
            "{time:.1} s: guard {:?} {} ({} guards knocked out so far)",
            w.sly3_id(h).or(w.parent(h).and_then(|p| w.sly3_id(p))),
            if out { "knocked out" } else { "no longer inside the trigger" },
            w.shared.guards_knocked_out
        ));
        *knocking = None;
    }
    let me = w.position(w.player?)?;
    let d = |h: &engine::object::Handle| {
        w.position(*h).map_or(f32::MAX, |p| (p[0] - me[0]).powi(2) + (p[1] - me[1]).powi(2))
    };
    // Inside such a trigger with no guard to fight yet (the brawl between waves), Sly waits where he stands rather
    // than wander off the arena.
    let inside = w.handles().filter(|&t| w.in_world(t) && counts_guards(t)).any(|t| {
        engine::behavior::sly2_props::volume_box(w, t).is_some_and(|b| engine::behavior::sly2_props::in_volume(&b, me))
    });
    let target = match *knocking {
        Some((h, _)) => h,
        None => {
            let Some(h) = waiting.iter().copied().min_by(|a, b| d(a).total_cmp(&d(b))) else {
                return inside.then(FrameInput::default);
            };
            journal.push(format!(
                "{time:.1} s: guard {:?} ({h:?}, parent {:?}) inside a guard-counting trigger; Sly goes to knock it out",
                w.sly3_id(h).or(w.parent(h).and_then(|p| w.sly3_id(p))),
                w.parent(h).and_then(|p| w.sly3_id(p))
            ));
            *knocking = Some((h, f));
            h
        }
    };
    let (_, from) = (*knocking)?;
    let k = f - from;
    if k.is_multiple_of(FPS) {
        // Beside the guard on the side Sly comes from, facing it (no path-finding; behind it could be off the
        // floor: the brawl's arena edge).
        let at = w.position(target)?;
        let (dx, dy) = (me[0] - at[0], me[1] - at[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1.0);
        let floor = [at[0] + dx / l * SLY2_ATTACK_STANDOFF, at[1] + dy / l * SLY2_ATTACK_STANDOFF, at[2] + 5.0];
        let yaw = (at[1] - floor[1]).atan2(at[0] - floor[0]);
        let (sn, cs) = (yaw.sin(), yaw.cos());
        let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: floor };
        g.systems.player.place(&mut g.world, to);
    }
    Some(sly2_attack_input(k))
}

/// What [`ball_input`] last saw: launches and tanks hit, and the frame Murray was last put on the launcher.
#[derive(Default)]
struct Balling {
    launches: u32,
    hit: usize,
    placed: Option<u32>,
}

/// m1_ball (missions.md 4.1a, `game::sly3::ball`), while Murray is played in the job: with a tank left (its task
/// available), the route puts Murray on the launcher (no path-finding) a second after each landing; in flight it
/// steers him at the nearest tank left, the stick easing off within 1,000 units of it. Launches and hits are
/// journalled.
fn ball_input(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    state: &mut Balling,
) -> Option<FrameInput> {
    use game::sly3::ball;
    let time = f as f32 / FPS as f32;
    let j = s.jobs.as_ref()?;
    j.job.filter(|&k| j.graph.jobs[k].id == ball::JOB)?;
    let left: Vec<i32> = ball::TANKS.iter().filter(|t| j.task_state(t.1) == Some(1)).map(|t| t.0).collect();
    let b = s.ball.clone();
    if b.launches != state.launches {
        state.launches = b.launches;
        journal.push(format!("{time:.1} s: ball: launch {} ({} tanks left)", b.launches, left.len()));
    }
    if b.hit.len() != state.hit {
        state.hit = b.hit.len();
        journal.push(format!("{time:.1} s: ball: hit tank {:?}", b.hit.last()));
    }
    let g = s.game.as_mut()?;
    let me = g.world.player.filter(|&p| g.world.class(p) == Some(ball::MURRAY_CLASS))?;
    if left.is_empty() {
        return None;
    }
    let at = g.world.position(me)?;
    if !b.flying {
        // A second after landing (or at once before the first launch), onto the launcher.
        let due = b.since_landed >= 1.0 && state.placed.is_none_or(|p| f >= p + FPS);
        if due && let Some((_, pad)) = ball::launchers(&g.world).first().copied() {
            let to = g.world.world_transform(pad)?;
            g.systems.player.place(&mut g.world, to);
            state.placed = Some(f);
        }
        return Some(FrameInput::default());
    }
    let d2 = |p: [f32; 3]| (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2);
    let target = left
        .iter()
        .filter_map(|&id| ball::tank(&g.world, id).and_then(|t| g.world.position(t)))
        .min_by(|a, b| d2(*a).total_cmp(&d2(*b)))?;
    let (dx, dy) = (target[0] - at[0], target[1] - at[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let k = (len / 1000.0).min(1.0);
    let yaw = g.world.shared.camera_yaw;
    let (fw, r) = ([yaw.cos(), yaw.sin()], [yaw.sin(), -yaw.cos()]);
    Some(FrameInput {
        left: [k * (dx * r[0] + dy * r[1]) / len, k * (dx * fw[0] + dy * fw[1]) / len],
        ..Default::default()
    })
}

/// What [`carmelita_input`] last saw: whether Carmelita was driven, her health, and each enemy's state (2665) by the
/// object holding it.
#[derive(Default)]
struct Battle {
    driven: bool,
    health: Option<f32>,
    states: std::collections::BTreeMap<u32, i32>,
}

/// How near the route takes Carmelita to an enemy before it stops and fires (within the shot stand-in's reach, its
/// speed times the projectiles' lifetime; shots do not collide with the world), and how far off her heading the
/// enemy may be (radians) for a shot.
const CARMELITA_RANGE: f32 = 7500.0;
const CARMELITA_AIM: f32 = 0.25;

/// The turf-war battles (missions.md 4.1a), while player 0 drives Carmelita (`game::sly3::carmelita`): the route
/// walks her toward the nearest enemy that is not defeated (a guard whose placement holds an enemy state, 2665, of 0
/// to 2) until it is within [`CARMELITA_RANGE`], turns her to face it, and fires (Square, the shot stand-in) while it
/// is within [`CARMELITA_AIM`] of her heading. With no enemy up she stands still. Enemy state changes and her health
/// are journalled.
fn carmelita_input(
    s: &game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    battle: &mut Battle,
) -> Option<FrameInput> {
    let time = f as f32 / FPS as f32;
    let g = s.game.as_ref()?;
    let w = &g.world;
    let Some(me) = game::sly3::carmelita::driven(w) else {
        if battle.driven {
            battle.driven = false;
            journal.push(format!("{time:.1} s: Carmelita no longer driven"));
        }
        return None;
    };
    let it = g.scripts()?;
    let health = w.options(me).and_then(|o| o.f32(game::sly3::carmelita::HEALTH));
    if !battle.driven || health != battle.health {
        journal.push(format!("{time:.1} s: driving Carmelita {:?}, health {health:?}", w.sly3_id(me)));
        battle.driven = true;
        battle.health = health;
    }
    // The enemies: guards in the world whose placement (their parent, or the one they are an instance of) holds a
    // state.
    let mut up: Vec<(engine::object::Handle, [f32; 3])> = Vec::new();
    for k in w.slots_of_class(58) {
        let Some(h) = w.handle(*k) else { continue };
        let holder = w
            .parent(h)
            .filter(|&p| w.class(p) == Some(8))
            .or_else(|| w.shared.sly3_instance_of.get(k).and_then(|&p| w.handle(p)));
        let Some(holder) = holder else { continue };
        let Some(state) = it.custom_property(holder.to_script(), 2665).and_then(|v| v.as_i32()) else { continue };
        let slot = w.slot(holder).unwrap_or(u32::MAX);
        if battle.states.get(&slot) != Some(&state) {
            journal.push(format!(
                "{time:.1} s: enemy {:?} (placement {:?}) state {state}, health {:?}",
                w.sly3_id(h),
                w.sly3_id(holder),
                w.options(h).and_then(|o| o.f32(game::sly3::carmelita::HEALTH))
            ));
            battle.states.insert(slot, state);
        }
        if (0..=2).contains(&state)
            && w.in_world(h)
            && let Some(p) = w.position(h)
        {
            up.push((h, p));
        }
    }
    let at = w.world_transform(me)?;
    let d2 = |p: [f32; 3]| (p[0] - at.t[0]).powi(2) + (p[1] - at.t[1]).powi(2);
    let Some(&(_, p)) = up.iter().min_by(|a, b| d2(a.1).total_cmp(&d2(b.1))) else {
        return Some(FrameInput::default());
    };
    let (dx, dy) = (p[0] - at.t[0], p[1] - at.t[1]);
    let dist = (dx * dx + dy * dy).sqrt().max(1e-3);
    let heading = at.cols[0][1].atan2(at.cols[0][0]);
    // The bearing off her heading, in [−π, π).
    use std::f32::consts::{PI, TAU};
    let off = (dy.atan2(dx) - heading + PI).rem_euclid(TAU) - PI;
    // The stick is relative to the camera, as `steer_to` has it; a short push (one frame in three) mostly turns her.
    let yaw = w.shared.camera_yaw;
    let (fw, r) = ([yaw.cos(), yaw.sin()], [yaw.sin(), -yaw.cos()]);
    let stick = |k: f32| [k * (dx * r[0] + dy * r[1]) / dist, k * (dx * fw[0] + dy * fw[1]) / dist];
    if dist > CARMELITA_RANGE {
        return Some(FrameInput { left: stick(1.0), ..Default::default() });
    }
    if off.abs() > CARMELITA_AIM {
        let k = if f.is_multiple_of(3) { 1.0 } else { 0.0 };
        return Some(FrameInput { left: stick(k), ..Default::default() });
    }
    let b = if f.is_multiple_of(FPS / 3) { platform_api::Buttons::Y.0 } else { 0 };
    Some(FrameInput { pressed: b, buttons: b, ..Default::default() })
}

/// The canal chase (missions.md 4.1a), while the player drives the boat (control handed to the player record): the
/// route steers back to the middle of the canal (the boat's offset across its owner's track), holds Square to fire,
/// lets go for a second every three so the gun cools, and jumps (Cross) now and then.
fn boat_input(s: &game::sly3::Sly3Session<NullBackend, DiscFile>, f: u32) -> Option<FrameInput> {
    let w = &s.game.as_ref()?.world;
    w.shared.driving?;
    let owner = w.find_sly3_id(game::sly3::boat::OWNER_RIG, None)?;
    let boat = w.slots_of_class(47).iter().filter_map(|&k| w.handle(k)).find(|&h| {
        w.options(h).and_then(|o| o.vec3(77)).is_some() && w.parent(h).and_then(|p| w.parent(p)) == Some(owner)
    })?;
    let (b, o) = (w.world_transform(boat)?, w.world_transform(owner)?);
    let d = [b.t[0] - o.t[0], b.t[1] - o.t[1]];
    // Across the track: the boat's y axis.
    let side = d[0] * b.cols[1][0] + d[1] * b.cols[1][1];
    let mut i = FrameInput { left: [(side / 300.0).clamp(-1.0, 1.0), 0.0], ..Default::default() };
    if f % (3 * FPS) >= FPS {
        i.buttons |= platform_api::Buttons::Y.0;
    }
    if f.is_multiple_of(4 * FPS) {
        i.pressed |= platform_api::Buttons::B.0;
        i.buttons |= platform_api::Buttons::B.0;
    }
    Some(i)
}

/// The gauntlet's timed buttons (missions.md 4.1a): while script 2291 runs, Sly stands at his side's button (the
/// use point whose copy's placement sets custom property 2199 to 2291; stand-in: put there, no path-finding) and
/// presses Circle 0.2 s after Bentley's last countdown line (1290), when Bentley's side records its time.
fn button_input(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    state: &mut Option<(engine::object::Handle, Option<u32>)>,
) -> Option<FrameInput> {
    use engine::behavior::sly3::interactables::class;
    if !s.scripts.is_running(2291) {
        *state = None;
        return None;
    }
    let time = f as f32 / FPS as f32;
    if state.is_none() {
        let g = s.game.as_mut()?;
        let w = &g.world;
        let l = w.sly3.as_deref()?;
        // Nearest the door's side Bentley's script works with (2291's context, which its module sets from the
        // current task), else nearest Sly.
        let context = s.scripts.running.iter().find(|r| r.oid == 2291).map(|r| r.context).filter(|&c| c != -1);
        let me = context.and_then(|c| w.find_sly3_id(c, None)).and_then(|h| w.position(h)).or(w.position(w.player?))?;
        // Sly's side: a button copy whose placement names script 2291 (custom property 2199).
        let sly_side = |p: engine::object::Handle| {
            let slot = w.slot(p);
            l.actions.iter().any(|a| {
                Some(a.0) == slot && a.1 == 8 && a.2.iter().any(|x| matches!(x, engine::object::Arg::Int(2291)))
            })
        };
        let point = w
            .handles()
            .filter(|&h| w.class(h) == Some(class::USE_POINT) && w.is_active(h))
            .filter(|&h| {
                let root = w.parent(h);
                root.and_then(|r| w.parent(r)).is_some_and(sly_side)
            })
            .min_by(|&a, &b| {
                let d = |h| w.position(h).map_or(f32::MAX, |p| (p[0] - me[0]).powi(2) + (p[1] - me[1]).powi(2));
                d(a).total_cmp(&d(b))
            })?;
        let at = w.world_transform(point)?;
        g.systems.player.place(&mut g.world, at);
        journal.push(format!(
            "{time:.1} s: at Sly's button (use point {:?} of placement {:?}; placed, no path-finding; played class {:?})",
            g.world.sly3_id(point),
            g.world.parent(point).and_then(|r| g.world.parent(r)).and_then(|p| g.world.sly3_id(p)),
            g.world.player.and_then(|p| g.world.class(p))
        ));
        *state = Some((point, None));
        s.last_line = None;
    }
    let (point, press) = state.as_mut()?;
    // Stand-in: a character that slid off the button after landing (door 3's, 145224) is put back on it.
    {
        use engine::behavior::sly3::interactables::UsePoint;
        let point = *point;
        let g = s.game.as_mut()?;
        let on = UsePoint::new(&g.world, point).user_of(&g.world, point).is_some_and(|u| Some(u) == g.world.player);
        if !on && f.is_multiple_of(FPS / 2) {
            let at = g.world.world_transform(point)?;
            g.systems.player.place(&mut g.world, at);
            journal.push(format!("{time:.1} s: off Sly's button; put back on it (no path-finding)"));
        }
    }
    if press.is_none() && s.last_line == Some(1290) {
        *press = Some(f + FPS / 5);
        s.last_line = None;
    }
    if *press == Some(f) {
        *press = None;
        journal.push(format!("{time:.1} s: pressed Circle at Sly's button, in time with Bentley"));
        let b = platform_api::Buttons::A.0;
        return Some(FrameInput { pressed: b, buttons: b, ..Default::default() });
    }
    Some(FrameInput::default())
}

/// The route's input for a photo target that names the current task (option 1308), if there is one: see the loop.
fn photo_input(
    s: &mut game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    last: &mut u8,
    walk: &mut Option<(u32, f32)>,
) -> Option<FrameInput> {
    let mut place: Option<engine::math::Affine> = None;
    let r = photo_step(s, f, journal, last, walk, &mut place);
    if let Some(to) = place
        && let Some(g) = s.game.as_mut()
    {
        g.systems.player.place(&mut g.world, to);
    }
    r
}

fn photo_step(
    s: &game::sly3::Sly3Session<NullBackend, DiscFile>,
    f: u32,
    journal: &mut Vec<String>,
    last: &mut u8,
    walk: &mut Option<(u32, f32)>,
    place: &mut Option<engine::math::Affine>,
) -> Option<FrameInput> {
    use game::sly3::photo::{self, View};
    let g = &s.game.as_ref()?.world;
    let j = s.jobs.as_ref()?;
    let task = j.current.map(|t| j.graph.tasks[t].id);
    let Some(t) = photo::targets(g).into_iter().find(|t| task.is_some() && t.task == task) else {
        // The binocucom open (a script's 0x46) with nothing to photograph for the current task: close it (R3).
        return s.photo.as_ref().map(|_| {
            let b = platform_api::Buttons::RSTICK.0;
            journal.push(format!("{:.1} s: binocucom closed (no target for the current task)", f as f32 / FPS as f32));
            FrameInput { pressed: b, buttons: b, ..Default::default() }
        });
    };
    let at = g.position(t.handle)?;
    let time = f as f32 / FPS as f32;
    let Some(p) = s.photo.as_ref() else {
        *last = 0;
        // The distance where the target's size spans the frame, looking level.
        let params = photo::params(s.game.as_ref()?.render.camera.fov_y);
        let ideal = View { eye: [0.0; 3], yaw: 0.0, pitch: 0.0, params }.focal() * t.size / game::photo::FRAME;
        let me = g.position(g.player?)?;
        let d = ((at[0] - me[0]).powi(2) + (at[1] - me[1]).powi(2)).sqrt();
        if f.is_multiple_of(5 * FPS) {
            journal.push(format!(
                "{time:.1} s: photo target {:?}: {d:.0} units away, framing distance {ideal:.0}, height {:.0}",
                g.sly3_id(t.handle),
                at[2] - me[2]
            ));
        }
        // Inside the size match, with a margin: the size is ideal / d, good from 1 − tolerance to 1 + tolerance.
        let margin = 0.8 * t.tolerance;
        // Stand-in (no path-finding): a walk that has not closed 50 units in 5 s puts the character at the framing
        // distance on the line to the target, facing it.
        match *walk {
            Some((since, from)) if f >= since + 5 * FPS => {
                if from - d < 50.0 && d > ideal / (1.0 - margin) {
                    let k = ideal / d;
                    let p = [at[0] + (me[0] - at[0]) * k, at[1] + (me[1] - at[1]) * k, me[2]];
                    let yaw = (at[1] - p[1]).atan2(at[0] - p[0]);
                    let (sn, cs) = (yaw.sin(), yaw.cos());
                    *place =
                        Some(engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: p });
                    journal.push(format!(
                        "{time:.1} s: walk to the photo target stuck; put at {ideal:.0} units (no path-finding)"
                    ));
                }
                *walk = Some((f, d));
            }
            None => *walk = Some((f, d)),
            _ => {}
        }
        if d > ideal / (1.0 - margin) {
            return steer_to(s, t.handle);
        }
        if d < ideal / (1.0 + margin) {
            let mut i = steer_to(s, t.handle)?;
            i.left = [-i.left[0], -i.left[1]];
            return Some(i);
        }
        journal
            .push(format!("{time:.1} s: binocucom opened for photo target {:?} at {d:.0} units", g.sly3_id(t.handle)));
        let b = platform_api::Buttons::RSTICK.0;
        return Some(FrameInput { pressed: b, buttons: b, ..Default::default() });
    };
    if p.state != *last {
        journal.push(format!("{time:.1} s: binocucom prompt {}", p.state));
        *last = p.state;
    }
    match p.state {
        photo::state::READY => {
            let b = platform_api::Buttons::R.0;
            Some(FrameInput { pressed: b, buttons: b, ..Default::default() })
        }
        photo::state::TOO_SMALL | photo::state::TOO_BIG => {
            // Walk closer or farther: close the binocucom first.
            let b = platform_api::Buttons::RSTICK.0;
            Some(FrameInput { pressed: b, buttons: b, ..Default::default() })
        }
        _ => {
            // Turn the view toward the target.
            let e = p.view.eye;
            let (dx, dy, dz) = (at[0] - e[0], at[1] - e[1], at[2] - e[2]);
            let yaw = dy.atan2(dx);
            let pitch = dz.atan2((dx * dx + dy * dy).sqrt());
            let wrap = |a: f32| (a + core::f32::consts::PI).rem_euclid(core::f32::consts::TAU) - core::f32::consts::PI;
            let (ey, ep) = (wrap(yaw - p.view.yaw), pitch - p.view.pitch);
            let k = 4.0;
            Some(FrameInput { left: [(-ey * k).clamp(-1.0, 1.0), (ep * k).clamp(-1.0, 1.0)], ..Default::default() })
        }
    }
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
    // The intro (missions.md 4.1a): the restore opening `t1_intro` (2064) raises event 24 on it, which starts script
    // 2065. The route waits for it to end, pressing Cross once a second for its prompt (2048).
    let mut started = false;
    for f in 0..90 * FPS {
        let running = s.scripts.is_running(2065);
        started |= running;
        if started && !running {
            break;
        }
        let b = if f % FPS == FPS - 1 { platform_api::Buttons::B.0 } else { 0 };
        s.update(&FrameInput { pressed: b, buttons: b, ..Default::default() }, 1.0 / FPS as f32);
    }
    journal.push(format!(
        "intro: script 2065 ran {started}, still running {}, 2064 state {:?}",
        s.scripts.is_running(2065),
        s.jobs.as_ref().and_then(|j| j.task_state(2064))
    ));
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
    // The cell lock's stick angle and last state seen.
    let mut lock_stick = 0.0f32;
    let mut lock_state = 0;
    // The binocucom's last prompt state seen, and the guard command lists running.
    let mut photo_state = 0u8;
    let mut photo_walk: Option<(u32, f32)> = None;
    let mut guard_lists: Vec<i32> = Vec::new();
    let mut listened = false;
    // The gauntlet's timed buttons: Sly at his button, and the frame to press it.
    let mut button: Option<(engine::object::Handle, Option<u32>)> = None;
    // The bomb door: the task it waits on and the frame of the first press.
    let mut bombing: (Option<(i32, u32)>, i32) = (None, -1);
    let mut knocking: Option<(engine::object::Handle, u32)> = None;
    // The turf-war battles: what the route last saw of Carmelita and the enemies.
    let mut battle = Battle::default();
    // m1_ball: the frame of the last landing seen, launches and tanks journalled so far.
    let mut balling = Balling::default();
    // The frame of the last load into another world, the character's height just after it, and since when the route
    // has had nothing to do.
    let mut loaded: Option<u32> = None;
    let mut arrival_z: Option<f32> = None;
    let mut idle_since: Option<u32> = None;
    for f in 0..frames {
        let t = Instant::now();
        // While Carmelita is driven (the turf-war battles) the route heads for no waypoint: `carmelita_input` plays.
        let carmelita = s.game.as_ref().and_then(|g| game::sly3::carmelita::driven(&g.world));
        let now = if carmelita.is_some() { None } else { route_target(&s) };
        if now != target {
            target = now;
            steering = 0;
            placed = false;
            if let Some(h) = now {
                take_out_for(&mut s, h, f, &mut journal);
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
        if placed && steering == 10 * FPS + 30 {
            // Half a second after placing at a use point: how far the character is from it, against its reach.
            if let Some(h) = target {
                use engine::behavior::sly3::interactables::{UsePoint, class};
                let g = &s.game.as_ref().unwrap().world;
                if g.class(h) == Some(class::USE_POINT) {
                    let (p, q) = (g.position(h), g.player.and_then(|p| g.position(p)));
                    let d = p.zip(q).map(|(p, q)| [q[0] - p[0], q[1] - p[1], q[2] - p[2]]);
                    journal.push(format!(
                        "{:.1} s: at use point {:?}: character offset {d:?}, reach {}",
                        f as f32 / FPS as f32,
                        g.sly3_id(h),
                        UsePoint::new(g, h).radius
                    ));
                }
            }
        }
        // After a load, with nothing to head for and no script running, the character stands still instead of
        // walking the gang pattern (it walked off v_ext's collision); the route stops 30 s later (below).
        let wander = loaded.is_none() || target.is_some() || !s.scripts.running.is_empty();
        let mut input = steer.unwrap_or_else(|| if wander { gang_pattern(f) } else { FrameInput::default() });
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
        // A cell lock running (game::sly3::lock): the stick turns the dial through the combination, up the marks for
        // tumblers 0 and 2 and down for tumbler 1, a little over the dead zone each frame.
        if let Some(l) = s.lock.as_ref().filter(|l| (1..=4).contains(&l.state)) {
            input = FrameInput::default();
            if l.state == 3 && l.dial.active < 3 {
                lock_stick += 0.15 * game::sly3::lock::Dial::direction(l.dial.active);
                input.left = [lock_stick.cos(), lock_stick.sin()];
            }
            if l.state != lock_state {
                journal.push(format!("{:.1} s: lock state {}", f as f32 / FPS as f32, l.state));
                if l.state == 4 {
                    journal.push(format!(
                        "{:.1} s: lock solved; stand-in: it raises event 5 on itself (the link from its part 113853 is not traced)",
                        f as f32 / FPS as f32
                    ));
                }
            }
            lock_state = l.state;
        } else if s.lock.is_none() && lock_state != 0 {
            journal.push(format!("{:.1} s: lock done", f as f32 / FPS as f32));
            lock_state = 0;
        }
        // A photo target for the current task (game::sly3::photo): walk to where its size fills the frame, open the
        // binocucom (R3), aim with the stick until the prompt says ready, then press R.
        if let Some(i) = photo_input(&mut s, f, &mut journal, &mut photo_state, &mut photo_walk) {
            input = i;
        }
        if let Some(i) = button_input(&mut s, f, &mut journal, &mut button) {
            input = i;
        }
        if let Some(i) = bomb_input(&mut s, f, &mut journal, &mut bombing) {
            input = i;
        }
        if let Some(i) = knockout_input(&mut s, f, &mut journal, &mut knocking) {
            input = i;
        }
        // Driving a machine (the canal chase's boat): the route's own pad replaces the wander input.
        if let Some(i) = boat_input(&s, f) {
            input = i;
        }
        if let Some(i) = carmelita_input(&s, f, &mut journal, &mut battle) {
            input = i;
        }
        if let Some(i) = ball_input(&mut s, f, &mut journal, &mut balling) {
            input = i;
        }
        s.update(&input, 1.0 / FPS as f32);
        // Stand-in (missions.md 4.1a, open question: what raises event 5 on path 139626, probably a character's path
        // movement reaching the listening spot): once `t1_follow_listen_to_lion_boss` (2268) is available, Sly is put
        // at the path (no path-finding) and the path gets event 5. Its code-29 broadcast is what 2268's module answers.
        if !listened && s.jobs.as_ref().and_then(|j| j.task_state(2268)) == Some(1) {
            let g = s.game.as_mut().unwrap();
            if let Some(path) = g.world.find_sly3_id(139626, None)
                && let Some(at) = g.world.world_transform(path)
            {
                g.systems.player.place(&mut g.world, at);
                g.world.raise(path, 5, Vec::new());
                listened = true;
                journal.push(format!(
                    "{:.1} s: stand-in: Sly put at path 139626 and the path given event 5 (the listening trigger is not traced)",
                    f as f32 / FPS as f32
                ));
            }
        }
        let lists: Vec<i32> = s.guards.iter().map(|r| r.list).collect();
        if lists != guard_lists {
            journal.push(format!("{:.1} s: guard command lists {lists:?}", f as f32 / FPS as f32));
            guard_lists = lists;
        }
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
            loaded = Some(f);
            arrival_z = None;
            // Handles of the world left behind mean nothing in the new one.
            knocking = None;
            journal.push(format!("{:.1} s: load {:?}", f as f32 / FPS as f32, last_load));
        }
        // After a load the route goes on only while it has something to do. With no target (waypoint, trigger or
        // use point for the current task) and no script running for 30 s, or once the character has fallen 20,000
        // units below where it arrived (with no target, or again after being put at the target), it stops rather
        // than wander: the gang pattern walks off the world's
        // collision, and falling out of the world is not modelled (spec open).
        let idle = target.is_none() && s.scripts.running.is_empty() && !s.script_paused;
        idle_since = if idle { idle_since.or(Some(f)) } else { None };
        let z = s.game.as_ref().and_then(|g| g.world.player.and_then(|p| g.world.position(p))).map(|p| p[2]);
        if let Some(l) = loaded {
            // The height a second after the load, once the character stands at the arrival.
            if f == l + FPS {
                arrival_z = z;
            }
            let mut fell = arrival_z.zip(z).is_some_and(|(a, z)| z < a - 20_000.0);
            // With a target not yet stood at, a fall puts the character there at once (as the 10 s placing would);
            // a second fall, or one with no target, stops the route.
            if fell
                && !placed
                && let Some(to) = target.and_then(|h| s.game.as_ref().unwrap().world.world_transform(h))
            {
                placed = true;
                let g = s.game.as_mut().unwrap();
                g.systems.player.place(&mut g.world, to);
                journal.push(format!("{:.1} s: fell; put at the target (no path-finding)", f as f32 / FPS as f32));
                fell = false;
            }
            if fell || idle_since.is_some_and(|i| f >= i.max(l) + 30 * FPS) {
                journal.push(format!(
                    "{:.1} s: stopped: {} in the world loaded at {:.1} s",
                    f as f32 / FPS as f32,
                    if fell { "fell out of the world" } else { "nothing to do" },
                    l as f32 / FPS as f32
                ));
                break;
            }
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
/// While a sequence waits for a clip that ends by itself, a task is looked at only after this long.
pub const SLY2_CUTSCENE_STALL: f32 = 60.0;
/// Jobs to play through: episode 1's intro job and the first hub job (`SLYNX_SLY2_JOBS` asks for more).
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
    // Also those whose own script answers the entry (animation.md 2: 150312 starts 1676, 150314 stops it).
    let added_volume = |h: engine::object::Handle| {
        matches!(w.class(h), Some(24 | 25))
            && (w.options(h).and_then(|o| o.i32(441)).is_some() || w.module(h) >= 0)
            && w.sly2.as_deref().is_some_and(|l| l.actions.iter().any(|a| Some(a.0) == w.slot(h) && a.1 == 26))
    };
    let fallback = w.handles().find(|&h| !visited.contains(&h) && w.in_world(h) && added_volume(h));
    let spot_names = |h: engine::object::Handle| {
        w.sly2.as_deref().is_some_and(|l| {
            let slot = w.slot(h);
            l.actions.iter().any(|a| {
                Some(a.0) == slot && a.1 == 1076 && matches!(a.2.first(), Some(engine::object::Arg::Int(n)) if *n == id)
            })
        })
    };
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
                // An interaction spot of a derived class whose task requests (1076) name the task: the player uses it
                // (missions.md 4.3: m1_bug's spot 17366 asks for 1815).
                c if engine::world::sly2::is_a(c, 35) && spot_names(h) => Some((h, 4)),
                // A photo target whose task (1201) is this one: the player takes its picture (missions.md 4.3).
                c if c == game::sly2::photo::CLASS => {
                    (o.i32(game::sly2::photo::OPTIONS.task) == Some(id)).then_some((h, 3))
                }
                _ => None,
            }
        })
        .or(fallback.map(|h| (h, 0)));
    let pick = pick?;
    visited.push(pick.0);
    let oid = w.sly2_id(pick.0);
    if pick.1 == 3 {
        sly2_frame_photo(s, pick.0)?;
        return Some(format!("photographed target {oid:?}"));
    }
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
    if pick.1 == 4 {
        let user = g.world.player;
        engine::behavior::sly2_commands::use_spot(&mut g.world, pick.0, user);
        return Some(format!("used interaction spot {oid:?}, whose task requests name the task"));
    }
    if pick.1 == 0 {
        sly2_enter_volume(s, pick.0)
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

/// Puts the played character in the middle of volume `h` (standing in for the walk), so its entry goes through the
/// real rules (the camera's visibility mask, the classes that count); a volume whose box is not read, or with no
/// floor in it, is entered by its visit flag instead.
fn sly2_enter_volume(s: &mut Sly2, h: engine::object::Handle) -> Option<String> {
    use engine::behavior::Behavior;
    let g = s.game.as_mut()?;
    let oid = g.world.sly2_id(h);
    // No path-finding: the played character is put in the middle of the volume (standing in for the walk), so
    // its entry goes through the real rules (the camera's visibility mask, the classes that count); a volume
    // whose box is not read is entered by its visit flag instead.
    if let Some((w, lo, hi)) = engine::behavior::sly2_props::volume_box(&g.world, h) {
        let mid = [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            ((lo[2] + hi[2]) * 0.5 - engine::behavior::sly2_sensors::CHARACTER_CENTRE).max(lo[2]),
        ];
        // On a floor: probed down from the top of the volume through it (none: the visit flag below).
        let top = w.transform_point([mid[0], mid[1], hi[2]]);
        let floor = g.systems.physics.queries().ground(&g.world, top, hi[2] - lo[2]).map(|f| f.0);
        let Some(floor) = floor else {
            if let Some(Behavior::Sly2Volume(v)) = g.world.behavior_mut(h) {
                v.visit = true;
            }
            return Some(format!("walked into trigger volume {oid:?} (no floor in it: entered by flag)"));
        };
        // A volume that counts one of the gang (option 432: 7 Sly, 8 Bentley, 9 Murray) is walked into by that one:
        // the route switches to it first (m1_heist's 124142 counts only Bentley).
        {
            use engine::game::systems::PlayerSystem;
            use engine::gang::Who;
            let named: Vec<i32> =
                g.world.options(h).map(|o| o.all(432).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
            let who = |id: i32| match id {
                7 => Some(Who::Sly),
                8 => Some(Who::Bentley),
                9 => Some(Who::Murray),
                _ => None,
            };
            let wanted: Vec<Who> = named.iter().filter_map(|&id| who(id)).collect();
            if let Some(gang) = g.systems.player.gang_mut()
                && !wanted.is_empty()
                && !wanted.contains(&gang.player)
            {
                gang.switch_to(wanted[0]);
            }
        }
        let mut to = w;
        to.t = floor;
        g.systems.player.place(&mut g.world, to);
        let who = g.world.player.and_then(|p| g.world.class(p));
        let zone = g.world.sly2_zone_mask(h);
        return Some(format!(
            "walked into trigger volume {oid:?} (placed in it; player class {who:?}, camera mask {:#x}, volume zone {zone:#x})",
            g.world.shared.visibility_mask
        ));
    }
    if let Some(Behavior::Sly2Volume(v)) = g.world.behavior_mut(h) {
        v.visit = true;
    }
    Some(format!("walked into trigger volume {oid:?}"))
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
    let jobs_wanted = std::env::var("SLYNX_SLY2_JOBS").ok().and_then(|v| v.parse().ok()).unwrap_or(SLY2_JOBS);
    let mut forced = 0u32;
    let mut findings: Vec<String> = Vec::new();
    let mut visited: Vec<engine::object::Handle> = Vec::new();
    let mut played = 0u32;
    let mut hold_until = 0u32;
    let mut in_cutscene = false;
    let mut placed_here = false;
    let mut in_follow = false;
    let mut trail: Vec<[f32; 3]> = Vec::new();
    let mut attack_from: Option<u32> = None;
    let mut attack_target: Option<engine::object::Handle> = None;
    let mut photo_from: Option<(u32, engine::object::Handle)> = None;
    let mut toss: Option<(u32, engine::object::Handle, engine::object::Handle)> = None;
    let mut crawl: Option<(u32, engine::object::Handle, engine::object::Handle)> = None;
    let mut pickpocket: Option<(u32, engine::object::Handle)> = None;
    let mut marker_walked: Option<usize> = None;
    let mut traveled_for: Option<usize> = None;
    let mut tosses = 0u32;
    let mut bentley_tosses = 0u32;
    let mut advanced = 0u32;
    let mut seen_sequences: Vec<engine::object::Handle> = Vec::new();
    let mut namers: std::collections::BTreeMap<i16, u32> = std::collections::BTreeMap::new();
    for f in 0..frames {
        let t = Instant::now();
        // The plain pattern: jobs and tasks choose the character (missions.md: a task's character), and volumes that
        // count only one character must see that one.
        // After being placed somewhere the character stands still for a moment (the camera catches up and the
        // volume sees it), then the pattern goes on, without walking once placed in a volume of this world (it
        // walked the character back into the volume it had just left, making that task current again).
        // A follow (missions.md 4.3, `t1_follow_dimitri_on_streets`): while a volume is attached to a character (the
        // leash), the player stays quiet and keeps behind it on a floor, and is inside the volume its list's script
        // tests when the list ends.
        let following = sly2_follow(&mut s, f, &mut trail) || sly2_escort(&mut s, f);
        let input = if following || f < hold_until {
            FrameInput::default()
        } else if let Some((k, h)) =
            photo_from.and_then(|(a, h)| Some((f.checked_sub(a)?, h))).filter(|p| p.0 < 3 * FPS)
        {
            // Aimed at the target (the stick's stand-in), then the shutter (R) once the framing has settled.
            if let (Some(p), Some(at)) = (s.photo.as_mut(), s.game.as_ref().and_then(|g| g.world.position(h))) {
                p.look_at(at);
            }
            let b = if k == FPS / 2 { platform_api::Buttons::R.0 } else { 0 };
            FrameInput { pressed: b, buttons: b, ..Default::default() }
        } else if let Some((k, h)) =
            pickpocket.and_then(|(a, h)| Some((f.checked_sub(a)?, h))).filter(|t| t.0 < SLY2_PICKPOCKET_WINDOW)
        {
            sly2_pickpocket_input(&mut s, k, h)
        } else if let Some((k, c, v)) =
            crawl.and_then(|(a, c, v)| Some((f.checked_sub(a)?, c, v))).filter(|t| t.0 < SLY2_CRAWL_WINDOW)
        {
            sly2_crawl_input(&mut s, k, c, v)
        } else if let Some((k, c, v)) =
            toss.and_then(|(a, c, v)| Some((f.checked_sub(a)?, c, v))).filter(|t| t.0 < SLY2_TOSS_WINDOW)
        {
            sly2_toss_input(&mut s, k, c, v)
        } else if let Some(k) = attack_from.map(|a| f - a).filter(|&k| k < 3 * FPS) {
            // Each jump starts beside the target again (it moves; no path-finding).
            if k % FPS == 0
                && let Some(h) = attack_target
            {
                sly2_stand_by(&mut s, h);
            }
            sly2_attack_input(k)
        } else if placed_here {
            FrameInput { left: [0.0; 2], right: [0.0; 2], ..game::smoke::pattern(f) }
        } else {
            game::smoke::pattern(f)
        };
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
        for &h in seen_sequences.iter().filter(|h| !s.running.contains(h)) {
            let id = s.game.as_ref().and_then(|g| g.world.sly2_id(h));
            journal.push(format!("{:.1} s: sequence {id:?} ended", f as f32 / FPS as f32));
        }
        seen_sequences.retain(|h| s.running.contains(h));
        let row = s.current().map(|c| c.row);
        if row != last_row {
            journal.push(format!("{at:.1} s: load row {row:?}"));
            last_row = row;
            placed_here = false;
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
        } else if toss.is_some_and(|(a, ..)| f.saturating_sub(a) < SLY2_TOSS_WINDOW) {
            // A toss waits for the creatures to come to Murray.
            since = 0.0;
        } else if following {
            // A follow takes as long as the followed character's walk.
            if !in_follow {
                journal.push(format!("{at:.1} s: following (the stall timer waits)"));
                in_follow = true;
            }
            since = 0.0;
        } else if s.waits_on_ending_clip() {
            // A cutscene waiting for a clip that ends by itself counts slowly (up to SLY2_CUTSCENE_STALL).
            if !in_cutscene {
                journal.push(format!(
                    "{:.1} s: a sequence waits for a clip that ends: stall counted slowly",
                    f as f32 / FPS as f32
                ));
                in_cutscene = true;
            }
            since += 1.0 / FPS as f32 * SLY2_STALL_AFTER / SLY2_CUTSCENE_STALL;
        } else {
            in_cutscene = false;
            since += 1.0 / FPS as f32;
        }
        if jobs_done.len() >= jobs_wanted {
            break;
        }
        // A stalled task: what would finish it, then finish it as its event would.
        if since >= SLY2_STALL_AFTER {
            since = 0.0;
            // First as the player would: walk into a trigger volume, or use an interaction spot, that finishes it.
            // A done task: walk on to what makes one of its successors current.
            // A task that counts as done while available (option 1271, missions.md 3) is walked on from as well: its
            // successors whose prerequisites are met can be made current (the recon job's chain of volumes).
            let target = task.and_then(|t| {
                let j = sly2_jobs(&s)?;
                let next = |k: &usize| {
                    j.states[*k] == state::AVAILABLE || (j.states[*k] == state::LOCKED && j.prereqs_met(*k))
                };
                let succ = j.graph.tasks[t].succ.iter().copied().filter(next);
                if j.states[t] < state::DONE {
                    let on = j.states[t] == state::AVAILABLE && j.graph.tasks[t].counts_done;
                    return Some(core::iter::once(t).chain(succ.filter(|_| on)).collect::<Vec<usize>>());
                }
                Some(succ.collect())
            });
            // What names a target first (volumes, exits, spots), then what its scripts ask for (props, creatures,
            // doors), so a walk on to a successor's volume comes before leaving by a door.
            let mut did = None;
            let target = target.unwrap_or_default();
            for &k in &target {
                if let Some(what) = sly2_play_for(&mut s, k, &mut visited) {
                    did = Some((k, what));
                    break;
                }
            }
            // A task whose row (its checkpoint's world) is not this world's: the player leaves by an exit of this
            // world to that row (level-flow.md 5: m1_neela's intro is in the hub, entered after the moulin).
            if did.is_none() {
                for &k in &target {
                    if let Some(what) = sly2_travel_for(&mut s, k) {
                        traveled_for = Some(k);
                        did = Some((k, what));
                        break;
                    }
                }
            }
            // A guard holding the task's item in its pocket: Sly picks its pocket (pickpocket-and-pickups.md 5).
            if did.is_none() {
                for &k in &target {
                    let Some(id) = sly2_jobs(&s).map(|j| j.graph.tasks[k].id) else { continue };
                    if let Some(h) = sly2_pocket_target(&s, id) {
                        let gid = s.game.as_ref().and_then(|g| g.world.sly2_id(h));
                        pickpocket = Some((f + 1, h));
                        did = Some((k, format!("picking the pocket of guard {gid:?}")));
                        break;
                    }
                }
            }
            // A volume a crawl space's script brings in, whose script asks for the task: Sly crawls in, then into it.
            if did.is_none() {
                for &k in &target {
                    let Some(id) = sly2_jobs(&s).map(|j| j.graph.tasks[k].id) else { continue };
                    if let Some((c, v)) = sly2_crawl_target(&s, id) {
                        let g = s.game.as_ref().unwrap();
                        let (co, vo) = (g.world.sly2_id(c), g.world.sly2_id(v));
                        crawl = Some((f + 1, c, v));
                        did = Some((k, format!("crawling in crawl space {co:?} to volume {vo:?}")));
                        break;
                    }
                }
            }
            // Creatures a volume counts, for a task whose script names their placement: Murray tosses them in.
            // Or Bentley, for a task whose prop takes only thrown objects (each kind of toss has its own limit).
            if did.is_none() {
                for &k in &target {
                    let Some(id) = sly2_jobs(&s).map(|j| j.graph.tasks[k].id) else { continue };
                    let creature = if tosses < SLY2_TOSSES_MAX { sly2_toss_target(&s, id) } else { None };
                    let bentley = if creature.is_none() && bentley_tosses < SLY2_BENTLEY_TOSSES_MAX {
                        sly2_bentley_toss_target(&s, id)
                    } else {
                        None
                    };
                    if let Some((c, v)) = creature.or(bentley) {
                        let g = s.game.as_ref().unwrap();
                        let (co, vo) = (g.world.sly2_id(c), g.world.sly2_id(v));
                        if creature.is_some() {
                            tosses += 1;
                        } else {
                            bentley_tosses += 1;
                        }
                        toss = Some((f + 1, c, v));
                        let what = if g.world.class(c) == Some(55) {
                            format!("tossing Bentley at prop {vo:?} as Murray")
                        } else {
                            format!("tossing creature {co:?} into volume {vo:?} as Murray")
                        };
                        did = Some((k, what));
                        break;
                    }
                }
            }
            if did.is_none() {
                for &k in &target {
                    let Some(id) = sly2_jobs(&s).map(|j| j.graph.tasks[k].id) else { continue };
                    if let Some(what) = sly2_play_by_script(&mut s, id, &mut visited, &mut attack_target) {
                        did = Some((k, what));
                        break;
                    }
                }
            }
            // Last, for a task the player came to this world for: its marker, far from the player: the player walks
            // there (m1_neela's intro plays by marker 97772, where Neyla's zone is active).
            if did.is_none()
                && traveled_for.is_some_and(|k| sly2_jobs(&s).is_some_and(|j| j.current == Some(k)))
                && let Some(what) = sly2_marker_walk(&mut s, &mut marker_walked)
            {
                did = Some((target.first().copied().unwrap_or(0), what));
            }
            if let Some((k, what)) = did {
                journal.push(format!("{at:.1} s: played for {}: {what}", name(&s, Some(k))));
                played += 1;
                hold_until = f + 2 * FPS;
                placed_here |= what.contains("placed in it");
                if what.starts_with("tossing") || what.starts_with("crawling") || what.starts_with("picking") {
                    hold_until = f + 1;
                }
                if what.starts_with("photographed") {
                    hold_until = f + FPS / 2;
                    photo_from = s
                        .game
                        .as_ref()
                        .and_then(|g| g.world.player)
                        .and(visited.last().copied())
                        .map(|h| (hold_until, h));
                }
                if what.starts_with("attacked") || what.starts_with("fought") {
                    // Brief settle, then the attack.
                    hold_until = f + FPS / 2;
                    attack_from = Some(hold_until);
                }
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
/// Objects of the world whose own script module names `id` (an object ID literal in one of its handlers).
/// Whether a handler's text uses the number `needle` other than as a property ID: the second argument of operations
/// 108, 109 and 111 (set, set and read a property) is a property, not an object or task (`m1_disco`'s prop 131625
/// reads property 1872 of object 1747; task 1872 is another thing).
fn names_literal(text: &str, needle: &str) -> bool {
    // A stack of (operator, argument index) for the open forms.
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut token = String::new();
    let flush = |token: &mut String, stack: &mut Vec<(String, usize)>| -> bool {
        if token.is_empty() {
            return false;
        }
        let t = core::mem::take(token);
        let mut hit = false;
        if let Some((op, arg)) = stack.last_mut() {
            if *arg == 0 {
                *op = t.clone();
            } else if t == needle {
                hit = !(*arg == 2 && matches!(op.as_str(), "op108" | "op109" | "op111"));
            }
            *arg += 1;
        } else {
            hit = t == needle;
        }
        hit
    };
    for c in text.chars() {
        match c {
            '(' => {
                if flush(&mut token, &mut stack) {
                    return true;
                }
                stack.push((String::new(), 0));
            }
            ')' => {
                if flush(&mut token, &mut stack) {
                    return true;
                }
                stack.pop();
                if let Some((_, arg)) = stack.last_mut() {
                    *arg += 1;
                }
            }
            c if c.is_whitespace() => {
                if flush(&mut token, &mut stack) {
                    return true;
                }
            }
            c => token.push(c),
        }
    }
    flush(&mut token, &mut stack)
}

fn sly2_scripted_for(s: &Sly2, id: i32) -> Vec<engine::object::Handle> {
    let Some(g) = s.game.as_ref() else { return Vec::new() };
    let Some(p) = g.scripts().map(|i| i.program()) else { return Vec::new() };
    let w = &g.world;
    let needle = id.to_string();
    let names = |m: i16| {
        m >= 0
            && (m as usize) < p.module_count()
            && p.definitions(m as usize).iter().any(|&(_, n)| names_literal(&format!("{}", p.display(n)), &needle))
    };
    let mut out: Vec<engine::object::Handle> = w.handles().filter(|&h| w.in_world(h) && names(w.module(h))).collect();
    // Also the objects a declared custom property ties to it (option 8: a box prop's 1374 holds its task, set by its
    // placement for the copy's root).
    for p in engine::world::sly2::custom_properties(w) {
        let v = match p.value {
            engine::world::sly2::CustomValue::Int(i) | engine::world::sly2::CustomValue::Object(i) => i,
            _ => continue,
        };
        if v != id {
            continue;
        }
        let h = if w.class(p.object) == Some(6) { w.children(p.object).next().unwrap_or(p.object) } else { p.object };
        if w.in_world(h) && !out.contains(&h) {
            out.push(h);
        }
    }
    out
}

/// When no volume, exit or spot names the task: what the player would do instead.
/// - a reactive prop (class 98) whose script asks for the task is attacked (the cellar's vent panel, missions.md
///   4.3);
/// - swarming creatures (class 57) still standing are fought (`t1_recon_brawl`: the job counts them, missions.md
///   4.3);
/// - a door (class 26) whose script asks for a task of the current job is walked through (the cellar's door 136718
///   to the photo world).
fn sly2_play_by_script(
    s: &mut Sly2,
    id: i32,
    visited: &mut Vec<engine::object::Handle>,
    attack: &mut Option<engine::object::Handle>,
) -> Option<String> {
    use engine::behavior::Behavior;
    // A running sequence waits for a signal (629) that a volume's script sends on entry (640): the player walks in
    // (row 0: 1674 waits for 152661 from volume 150308).
    let waited: Vec<i32> = {
        let g = s.game.as_ref()?;
        s.silent_runs()
            .iter()
            .filter_map(|r| match game::sly2::talk::sequence_commands(&g.world, r.sequence).get(r.at) {
                Some(game::sly2::talk::SeqCommand::WaitSignal { value, .. }) => Some(*value),
                _ => None,
            })
            .collect()
    };
    for v in waited {
        let volume = sly2_scripted_for(s, v).into_iter().find(|&h| {
            !visited.contains(&h) && s.game.as_ref().is_some_and(|g| matches!(g.world.class(h), Some(24 | 25)))
        });
        if let Some(h) = volume {
            visited.push(h);
            return sly2_enter_volume(s, h).map(|w| format!("{w}, which signals {v}"));
        }
        // Or an interaction spot whose sequence (1070) sends it (630): the player uses the spot (row 0: 1674 waits
        // for 152638 from 1675, spot 80398's sequence).
        let g = s.game.as_mut()?;
        let w = &g.world;
        let sends = |q: engine::object::Handle| {
            game::sly2::talk::sequence_commands(w, q)
                .iter()
                .any(|c| matches!(c, game::sly2::talk::SeqCommand::SendSignal { value, .. } if *value == v))
        };
        let spot = w.handles().find_map(|h| {
            let q = w.options(h).and_then(|o| o.i32(1070)).and_then(|q| w.find_sly2_id(q, Some(h)))?;
            (w.class(h) == Some(35) && w.in_world(h) && !visited.contains(&h) && sends(q)).then_some((h, q))
        });
        if let Some((h, q)) = spot {
            visited.push(h);
            let oid = g.world.sly2_id(h);
            let starter = g.world.player;
            g.world
                .shared
                .sequence_requests
                .push(engine::behavior::sly2_commands::SequenceRequest::Start { sequence: q, starter });
            return Some(format!("used interaction spot {oid:?}, whose sequence signals {v}"));
        }
    }
    // Photo targets of the job's tasks wait in this world and the binocucom is off: the player opens it (R3; the
    // spy-camera tutorial 1791 is signalled when it opens, missions.md 4.3).
    if let Some(g) = s.game.as_mut()
        && !engine::gang::binocucom_on(&g.world)
        && let Some(p) = g.world.player
        && g.world.handles().any(|h| {
            g.world.class(h) == Some(game::sly2::photo::CLASS)
                && g.world.options(h).and_then(|o| o.i32(game::sly2::photo::OPTIONS.task)).is_some_and(|t| {
                    g.world.sly2.as_deref().and_then(|l| l.jobs.as_deref()).and_then(|j| j.task_state(t))
                        < Some(engine::game::sly2_jobs::state::DONE)
                })
        })
    {
        engine::gang::toggle_binocucom(&mut g.world, p, false);
        return Some("opened the binocucom (photo targets wait)".into());
    }
    // A task starting at a checkpoint elsewhere: a sequence whose script sends the player there (fires a door at that
    // point), started by an interaction spot the player uses (m1_bug: spot 123837's 132610 fires door 1734 at 1811's
    // checkpoint 398).
    let checkpoint = sly2_jobs(s).and_then(|j| j.graph.task_ids.get(&id).and_then(|&t| j.graph.tasks[t].checkpoint));
    if let Some(cp) = checkpoint {
        let sequences: Vec<i32> = sly2_scripted_for(s, cp)
            .into_iter()
            .filter_map(|h| s.game.as_ref().filter(|g| g.world.class(h) == Some(114)).and_then(|g| g.world.sly2_id(h)))
            .collect();
        let g = s.game.as_mut()?;
        let w = &g.world;
        let spot = w.handles().find(|&h| {
            w.class(h) == Some(35)
                && w.in_world(h)
                && !visited.contains(&h)
                && w.options(h).and_then(|o| o.i32(1070)).is_some_and(|q| sequences.contains(&q))
        });
        if let Some(h) = spot {
            visited.push(h);
            let oid = g.world.sly2_id(h);
            let user = g.world.player;
            engine::behavior::sly2_commands::use_spot(&mut g.world, h, user);
            return Some(format!("used interaction spot {oid:?}, whose sequence sends the player to checkpoint {cp}"));
        }
    }
    // A volume whose script starts or signals a sequence that sets the task: the player walks in (m1_heist's
    // rendezvous: 124131 starts 1903, 124132 sends it 124137, and 1903 sets 1902).
    let sequences: Vec<i32> = {
        let g = s.game.as_ref()?;
        let w = &g.world;
        w.handles()
            .filter(|&h| w.class(h) == Some(114))
            .filter(|&h| {
                game::sly2::talk::sequence_commands(w, h)
                    .iter()
                    .any(|c| matches!(c, game::sly2::talk::SeqCommand::Task { task, .. } if *task == id))
            })
            .filter_map(|h| w.sly2_id(h))
            .collect()
    };
    for q in sequences {
        let g = s.game.as_ref()?;
        // The volume that starts the sequence (method 235) before those that signal it.
        let starts = |h: engine::object::Handle| {
            let m = g.world.module(h);
            g.scripts().is_some_and(|i| {
                let p = i.program();
                let needle = format!("{q} null-object) 235)");
                m >= 0
                    && (m as usize) < p.module_count()
                    && p.definitions(m as usize).iter().any(|&(_, n)| format!("{}", p.display(n)).contains(&needle))
            })
        };
        let mut candidates: Vec<engine::object::Handle> = sly2_scripted_for(s, q)
            .into_iter()
            .filter(|&h| !visited.contains(&h) && matches!(g.world.class(h), Some(24 | 25)))
            .collect();
        candidates.sort_by_key(|&h| !starts(h));
        let volume = candidates.first().copied();
        if let Some(h) = volume {
            visited.push(h);
            return sly2_enter_volume(s, h).map(|w| format!("{w}, whose script drives sequence {q}"));
        }
    }
    let scripted = sly2_scripted_for(s, id);
    let g = s.game.as_ref()?;
    let w = &g.world;
    let me = w.position(w.player?)?;
    let prop = scripted.iter().copied().find(|&h| matches!(w.behavior(h), Some(Behavior::Sly2Prop(p)) if !p.done()));
    // Fighting only for a task the current job's own script finishes (its event handlers count the creatures), or
    // whose own script counts knock-outs (task events 23 and 24: m1_heist's siege counts nine of the generator's
    // rats).
    let by_job = scripted.iter().any(|&h| w.class(h) == Some(132));
    let counts_kills = w.handles().find(|&h| w.class(h) == Some(133) && w.sly2_id(h) == Some(id)).is_some_and(|t| {
        let m = w.module(t);
        g.scripts().is_some_and(|i| {
            let p = i.program();
            m >= 0
                && (m as usize) < p.module_count()
                && p.definitions(m as usize).iter().any(|&(e, n)| {
                    let text = format!("{}", p.display(n));
                    e == 17 && (text.contains("((23 24)") || text.contains("((24 23)"))
                })
        })
    });
    let creature = || {
        if !by_job && !counts_kills {
            return None;
        }
        w.handles()
            .filter(|&h| w.in_world(h))
            .filter(|&h| {
                matches!(w.behavior(h), Some(Behavior::Sly2Guard(g))
                    if ((by_job && g.swarmer) || counts_kills) && !g.knocked_out() && g.escort.is_none())
            })
            .filter_map(|h| Some((h, w.position(h)?)))
            .min_by(|a, b| d2(a.1, me).total_cmp(&d2(b.1, me)))
            .map(|x| x.0)
    };
    if let Some(h) = prop.or_else(creature) {
        let what = if prop.is_some() { "attacked prop" } else { "fought creature" };
        let oid = w.sly2_id(h);
        sly2_stand_by(s, h)?;
        *attack = Some(h);
        return Some(format!("{what} {oid:?}"));
    }
    // A door of this world whose script asks for a task of the current job.
    let (job_tasks, here) = {
        let j = sly2_jobs(s)?;
        let job = j.job?;
        let ids: Vec<i32> = j.graph.tasks.iter().filter(|t| t.job == Some(job)).map(|t| t.id).collect();
        (ids, j.row)
    };
    let door = job_tasks
        .iter()
        .flat_map(|&t| sly2_scripted_for(s, t))
        .find(|&h| !visited.contains(&h) && s.game.as_ref().is_some_and(|g| g.world.class(h) == Some(26)))?;
    visited.push(door);
    let g = s.game.as_mut()?;
    let oid = g.world.sly2_id(door)?;
    engine::game::sly2_jobs::with(&mut g.world, |j, w| {
        arm(j, oid);
        j.fire_exit_id(w, oid)
    });
    Some(format!("left by door {oid:?} (its script asks for a task of the job; from row {here:?})"))
}

/// A volume that counts creatures (option 433 naming a class other than the playable families) for task `id`, whose
/// script names the placement of a creature of that class: the creature standing free nearest the volume, and the
/// volume (missions.md 4.3: 1884 names rat placement 123920, and water main
/// volume 125273 counts class 57).
fn sly2_toss_target(s: &Sly2, id: i32) -> Option<(engine::object::Handle, engine::object::Handle)> {
    use engine::behavior::Behavior;
    let g = s.game.as_ref()?;
    let w = &g.world;
    let task = w.handles().find(|&h| w.class(h) == Some(133) && w.sly2_id(h) == Some(id))?;
    let names_placement = |h: engine::object::Handle| {
        w.parent(h)
            .filter(|&p| w.class(p) == Some(6))
            .and_then(|p| w.sly2_id(p))
            .is_some_and(|p| sly2_scripted_for(s, p).contains(&task))
    };
    for v in w.handles().filter(|&h| w.in_world(h) && matches!(w.class(h), Some(24 | 25))) {
        let classes: Vec<i16> =
            w.options(v).map(|o| o.all(433).filter_map(|x| x.as_i32()).map(|c| c as i16).collect()).unwrap_or_default();
        for c in classes.into_iter().filter(|c| !matches!(c, 51..=54)) {
            let Some(at) = w.position(v) else { continue };
            let all: Vec<engine::object::Handle> = w.slots_of_class(c).iter().filter_map(|&sl| w.handle(sl)).collect();
            if !all.iter().any(|&h| names_placement(h)) {
                continue;
            }
            let free = all
                .into_iter()
                .filter(|&h| w.in_world(h))
                .filter(|&h| {
                    matches!(w.behavior(h), Some(Behavior::Sly2Guard(g)) if !g.knocked_out() && !g.held && g.flight.is_none())
                })
                .filter_map(|h| Some((h, w.position(h)?)))
                .min_by(|a, b| d2(a.1, at).total_cmp(&d2(b.1, at)));
            if let Some((h, _)) = free {
                return Some((h, v));
            }
        }
    }
    None
}

/// The pad for a toss, `k` frames after it starts: Murray (Up), placed beside the creature `c` facing volume `v`
/// across it and lifting it (A, the gang's Circle) once he stands, then aiming (A) and, with the stick turning the
/// aim toward the volume and setting its distance to the volume's (characters.md 7: the aim lands at its distance on
/// level ground), throwing (Y, the gang's Square). No path-finding: the character is placed beside the creature and throws from there.
fn sly2_toss_input(s: &mut Sly2, k: u32, c: engine::object::Handle, v: engine::object::Handle) -> FrameInput {
    use engine::game::systems::PlayerSystem;
    use engine::gang::murray;
    if s.game.as_ref().is_some_and(|g| g.world.class(c) == Some(55)) {
        return sly2_bentley_toss_input(s, k, v);
    }
    if k == 0 {
        return FrameInput {
            pressed: platform_api::Buttons::UP.0,
            buttons: platform_api::Buttons::UP.0,
            ..Default::default()
        };
    }
    let Some(g) = s.game.as_ref() else { return FrameInput::default() };
    let Some(m) = g.systems.player.gang().and_then(|gg| gg.get(engine::gang::Who::Murray)) else {
        return FrameInput::default();
    };
    let (state, carrying, me, aim, my_yaw) = (m.state, m.carrying, m.position, m.aim, m.yaw);
    // Wedged in the air in a hit reaction (between two steep surfaces near the generator, neither counting as
    // ground, his reaction never ends): put back on his spot, as a player would steer out.
    let wedged = state == murray::DAMAGE && !m.grounded && m.state_time > SLY2_TOSS_WEDGED;
    let target = g.world.position(v);
    let a = platform_api::Buttons::A.0;
    let press = |b: u32| FrameInput { pressed: b, buttons: b, ..Default::default() };
    let class = g.world.class(c);
    let guard = |h: engine::object::Handle| match g.world.behavior(h) {
        Some(engine::behavior::Behavior::Sly2Guard(gd)) => Some(gd),
        _ => None,
    };
    // While a thrown creature flies, nothing (Circle would lift it again as it leaves); a miss is lifted again
    // once it lands.
    let creatures: Vec<engine::object::Handle> =
        g.world.handles().filter(|&h| g.world.class(h) == class && g.world.in_world(h)).collect();
    if creatures.iter().any(|&h| guard(h).is_some_and(|gd| gd.flight.is_some())) {
        return FrameInput::default();
    }
    if carrying.is_none() {
        let Some(t) = target else { return FrameInput::default() };
        // Murray waits on a floor SLY2_TOSS_DISTANCE from the volume, toward where the creature was loaded, for the
        // creatures that pursue him; the nearest free one in reach he faces and lifts.
        let post = guard(c).map_or(t, |gd| gd.post);
        let (dx, dy) = (post[0] - t[0], post[1] - t[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1.0);
        let spot = [t[0] + dx / l * SLY2_TOSS_DISTANCE, t[1] + dy / l * SLY2_TOSS_DISTANCE];
        let free = creatures
            .iter()
            .copied()
            .filter(|&h| guard(h).is_some_and(|gd| !gd.knocked_out() && !gd.held))
            .filter_map(|h| Some((h, g.world.position(h)?)))
            .min_by(|a, b| d2(a.1, me).total_cmp(&d2(b.1, me)));
        let near = free.filter(|(_, p)| d2([p[0], p[1], me[2]], me) <= SLY2_TOSS_REACH * SLY2_TOSS_REACH);
        let away = (me[0] - spot[0]).powi(2) + (me[1] - spot[1]).powi(2) > SLY2_TOSS_SPOT * SLY2_TOSS_SPOT;
        let ready = matches!(state, murray::STAND | murray::RUN | murray::STANCE | murray::WALK);
        let face = |to: [f32; 3]| (to[1] - me[1]).atan2(to[0] - me[0]);
        let place = |s: &mut Sly2, x: f32, y: f32, yaw: f32, above: f32| {
            let Some(g) = s.game.as_mut() else { return };
            let floor = g.systems.physics.queries().ground(&g.world, [x, y, above + 100.0], 1000.0).map(|f| f.0[2]);
            if let Some(z) = floor {
                let (sn, cs) = (yaw.sin(), yaw.cos());
                let to = engine::math::Affine {
                    cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]],
                    t: [x, y, z + 60.0],
                };
                g.systems.player.place(&mut g.world, to);
            }
        };
        // Fallen far below his spot (knocked off a ledge over a drop): put back, standing in for the controller's
        // return to safe ground (interactables.md 8, physics.md 5.1; not built).
        let fallen = me[2] < post[2] - SLY2_TOSS_FALLEN;
        // A free creature on a floor within a throw's reach of the volume (and not in reach yet): Murray is put
        // beside it, facing it, rather than waiting for it (it may stand still, or chase someone else).
        let floor_under =
            |p: [f32; 3]| g.systems.physics.queries().ground(&g.world, [p[0], p[1], p[2] + 100.0], 250.0).is_some();
        let fetch = free.filter(|(_, p)| {
            near.is_none()
                && (p[0] - t[0]).powi(2) + (p[1] - t[1]).powi(2) <= SLY2_TOSS_FETCH * SLY2_TOSS_FETCH
                && floor_under(*p)
        });
        if let Some((_, p)) = fetch
            && k % FPS == 20
            && ready
        {
            let (dx, dy) = (me[0] - p[0], me[1] - p[1]);
            let l = (dx * dx + dy * dy).sqrt().max(1.0);
            let (x, y) = (p[0] + dx / l * SLY2_ATTACK_STANDOFF, p[1] + dy / l * SLY2_ATTACK_STANDOFF);
            let yaw = (p[1] - y).atan2(p[0] - x);
            place(s, x, y, yaw, p[2]);
            return FrameInput::default();
        }
        let away = away && fetch.is_none();
        if k == 10 || (away && k % FPS == 10 && state != murray::LIFT) || (wedged && k % FPS == 10) || fallen {
            let yaw =
                free.map_or((t[1] - spot[1]).atan2(t[0] - spot[0]), |(_, p)| (p[1] - spot[1]).atan2(p[0] - spot[0]));
            place(s, spot[0], spot[1], yaw, post[2]);
            return FrameInput::default();
        }
        let Some((_, p)) = near else { return FrameInput::default() };
        if !ready {
            return FrameInput::default();
        }
        // Turned to it where he stands (the lift takes what is within 0.5 rad of his heading).
        let heading = my_yaw;
        let want = face(p);
        let off = (want - heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        if off.abs() > 0.3 {
            if k % 10 == 0 {
                place(s, me[0], me[1], want, me[2]);
            }
            return FrameInput::default();
        }
        return if k % 2 == 0 { press(a) } else { FrameInput::default() };
    }
    // Carrying, farther from the volume than a throw reaches: carried to a floor SLY2_TOSS_DISTANCE from it, on the
    // line from the volume, facing it (no path-finding).
    if state == murray::CARRY
        && let Some(t) = target
        && d2([me[0], me[1], t[2]], t) > SLY2_TOSS_RANGE * SLY2_TOSS_RANGE
    {
        let (dx, dy) = (me[0] - t[0], me[1] - t[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1.0);
        let (x, y) = (t[0] + dx / l * SLY2_TOSS_DISTANCE, t[1] + dy / l * SLY2_TOSS_DISTANCE);
        let Some(g) = s.game.as_mut() else { return FrameInput::default() };
        let floor = g.systems.physics.queries().ground(&g.world, [x, y, me[2] + 200.0], 600.0).map(|f| f.0[2]);
        if let Some(z) = floor {
            let yaw = (-dy).atan2(-dx);
            let (sn, cs) = (yaw.sin(), yaw.cos());
            let to =
                engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: [x, y, z + 60.0] };
            g.systems.player.place(&mut g.world, to);
        }
        return FrameInput::default();
    }
    match (state, aim, target) {
        // Spaced presses: a second Circle within the aim's 0.1 s buffer would cancel the aim.
        (murray::CARRY, _, _) if k.is_multiple_of(FPS / 2) => press(a),
        (murray::THROW_AIM, Some(aim), Some(t)) => {
            let (dx, dy) = (t[0] - me[0], t[1] - me[1]);
            let want_yaw = dy.atan2(dx);
            let want = (dx * dx + dy * dy).sqrt().clamp(aim.range[0], aim.range[1]);
            // Wrapped into -PI..PI by a remainder; a non-finite aim gives no input.
            let dyaw =
                (want_yaw - aim.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            if !dyaw.is_finite() {
                return FrameInput::default();
            }
            let dt = 1.0 / FPS as f32;
            let ddist = want - aim.distance;
            if dyaw.abs() < 0.02 && ddist.abs() < 10.0 {
                return press(platform_api::Buttons::Y.0);
            }
            // The aim turns by -2 x stick x rad/s and moves out by 800 x stick y per second.
            let sx = (-dyaw / (2.0 * dt)).clamp(-1.0, 1.0);
            let sy = (ddist / (800.0 * dt)).clamp(-1.0, 1.0);
            FrameInput { left: [sx, sy], ..Default::default() }
        }
        _ => FrameInput::default(),
    }
}

/// A Bentley toss for task `id` (missions.md 4.3, 1895): a reactive prop the task's script names that takes thrown
/// objects (contact kind 0x100) and not strikes. Returns (Bentley, the prop).
fn sly2_bentley_toss_target(s: &Sly2, id: i32) -> Option<(engine::object::Handle, engine::object::Handle)> {
    use engine::behavior::Behavior;
    use engine::behavior::sly2_props::contact;
    let g = s.game.as_ref()?;
    let w = &g.world;
    // The prop asks for the task itself (an event's task request, 1045: 123934 asks for 1895), or its script does.
    let asks = |p: &engine::behavior::sly2_props::Prop| {
        p.events
            .iter()
            .flat_map(|e| &e.items)
            .any(|i| matches!(i, engine::behavior::sly2_props::Item::Task(t, _) if *t == id))
    };
    let scripted = sly2_scripted_for(s, id);
    let prop = w.handles().filter(|&h| w.in_world(h)).find(|&h| {
        matches!(w.behavior(h), Some(Behavior::Sly2Prop(p))
            if !p.done() && p.accepts(Some(contact::THROWN)) && !p.accepts(Some(contact::STRIKE))
                && (asks(p) || scripted.contains(&h)))
    })?;
    let bentley = w.handles().find(|&h| w.class(h) == Some(55) && w.in_world(h))?;
    Some((bentley, prop))
}

/// The pad for a Bentley toss at prop `v`, `k` frames after it starts: Murray (Up) is put on a floor
/// [`SLY2_BENTLEY_TOSS_FROM`] from the prop facing it (no path-finding), takes Bentley up to throw (the toss that the
/// task's script ID 939 starts; the placement put him at rest, so the route takes it again), turns the aim onto the
/// prop and throws (Y, the gang's Square).
fn sly2_bentley_toss_input(s: &mut Sly2, k: u32, v: engine::object::Handle) -> FrameInput {
    use engine::game::systems::PlayerSystem;
    use engine::gang::murray;
    let press = |b: u32| FrameInput { pressed: b, buttons: b, ..Default::default() };
    if k == 0 {
        return press(platform_api::Buttons::UP.0);
    }
    let Some(t) = s.game.as_ref().and_then(|g| g.world.position(v)) else { return FrameInput::default() };
    if k == 10 {
        let Some(g) = s.game.as_mut() else { return FrameInput::default() };
        let Some(me) = g.systems.player.gang().and_then(|gg| gg.get(engine::gang::Who::Murray)).map(|m| m.position)
        else {
            return FrameInput::default();
        };
        let (dx, dy) = (me[0] - t[0], me[1] - t[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1.0);
        let (x, y) = (t[0] + dx / l * SLY2_BENTLEY_TOSS_FROM, t[1] + dy / l * SLY2_BENTLEY_TOSS_FROM);
        let floor = g.systems.physics.queries().ground(&g.world, [x, y, t[2] + 400.0], 1200.0).map(|f| f.0[2]);
        if let Some(z) = floor {
            let yaw = (-dy).atan2(-dx);
            let (sn, cs) = (yaw.sin(), yaw.cos());
            let to =
                engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: [x, y, z + 60.0] };
            g.systems.player.place(&mut g.world, to);
        }
        return FrameInput::default();
    }
    if k == 20 {
        if let Some(gg) = s.game.as_mut().and_then(|g| g.systems.player.gang_mut()) {
            gg.hold_bentley();
        }
        return FrameInput::default();
    }
    let Some(m) =
        s.game.as_ref().and_then(|g| g.systems.player.gang()).and_then(|gg| gg.get(engine::gang::Who::Murray))
    else {
        return FrameInput::default();
    };
    let (state, aim, me) = (m.state, m.aim, m.position);
    match (state, aim) {
        (murray::THROW_AIM, Some(aim)) if k > 20 => {
            let (dx, dy) = (t[0] - me[0], t[1] - me[1]);
            let want_yaw = dy.atan2(dx);
            let want = (dx * dx + dy * dy).sqrt().clamp(aim.range[0], aim.range[1]);
            let dyaw =
                (want_yaw - aim.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            if !dyaw.is_finite() {
                return FrameInput::default();
            }
            let dt = 1.0 / FPS as f32;
            let ddist = want - aim.distance;
            if dyaw.abs() < 0.02 && ddist.abs() < 10.0 {
                return press(platform_api::Buttons::Y.0);
            }
            let sx = (-dyaw / (2.0 * dt)).clamp(-1.0, 1.0);
            let sy = (ddist / (800.0 * dt)).clamp(-1.0, 1.0);
            FrameInput { left: [sx, sy], ..Default::default() }
        }
        _ => FrameInput::default(),
    }
}

/// The most Bentley tosses one run plays.
const SLY2_BENTLEY_TOSSES_MAX: u32 = 4;
/// See [`sly2_bentley_toss_input`]: how far from the prop Murray throws Bentley from.
const SLY2_BENTLEY_TOSS_FROM: f32 = 700.0;

/// See [`sly2_toss_input`]: how far from the volume a creature may be for Murray to be put beside it.
const SLY2_TOSS_FETCH: f32 = 2000.0;
/// How long one toss play lasts: the creatures pursuing Murray may come from far.
const SLY2_TOSS_WINDOW: u32 = 20 * FPS;
/// See [`sly2_toss_input`]: how near the creature Murray must be to lift it.
const SLY2_TOSS_REACH: f32 = 250.0;
/// See [`sly2_toss_input`]: how far below the creature's height Murray may fall before he is put back.
const SLY2_TOSS_FALLEN: f32 = 400.0;
/// See [`sly2_toss_input`]: how long Murray may hang in a hit reaction off the ground before he is put back.
const SLY2_TOSS_WEDGED: f32 = 2.0;
/// See [`sly2_toss_input`]: how far Murray may be from his waiting spot before he is put back on it.
const SLY2_TOSS_SPOT: f32 = 300.0;
/// [`sly2_to_marker`] as a play, once per task: when the current task's marker is in this world and more than
/// [`SLY2_MARKER_FAR`] from the player.
fn sly2_marker_walk(s: &mut Sly2, walked: &mut Option<usize>) -> Option<String> {
    let g = s.game.as_ref()?;
    let (t, marker) = g.world.sly2.as_deref().and_then(|l| l.jobs.as_deref()).and_then(|j| {
        let t = j.current?;
        Some((t, j.graph.tasks[t].marker?))
    })?;
    let h = g.world.find_sly2_id(marker, None)?;
    let h = engine::behavior::sly2_commands::placed_root(&g.world, h).unwrap_or(h);
    let at = g.world.position(h)?;
    let me = g.world.position(g.world.player?)?;
    if d2(at, me) <= SLY2_MARKER_FAR * SLY2_MARKER_FAR || *walked == Some(t) {
        return None;
    }
    *walked = Some(t);
    sly2_to_marker(s);
    Some(format!("walked to the task's marker {marker}"))
}

/// See [`sly2_marker_walk`].
const SLY2_MARKER_FAR: f32 = 1500.0;

/// Puts the played character at the current task's marker (option 1279) when the marker is in this world: the
/// marker placement's copy root, on the floor under it. No path-finding.
fn sly2_to_marker(s: &mut Sly2) {
    let Some(g) = s.game.as_mut() else { return };
    let Some(marker) = g.world.sly2.as_deref().and_then(|l| l.jobs.as_deref()).and_then(|j| {
        let t = j.current?;
        j.graph.tasks[t].marker
    }) else {
        return;
    };
    let Some(h) = g.world.find_sly2_id(marker, None) else { return };
    let h = engine::behavior::sly2_commands::placed_root(&g.world, h).unwrap_or(h);
    let Some(at) = g.world.position(h) else { return };
    let Some(floor) = g.systems.physics.queries().ground(&g.world, [at[0], at[1], at[2] + 100.0], 600.0) else {
        return;
    };
    let to = engine::math::Affine { t: [floor.0[0], floor.0[1], floor.0[2] + 5.0], ..engine::math::Affine::IDENTITY };
    g.systems.player.place(&mut g.world, to);
}

/// Leaves by an exit of this world to the current task `t`'s row when that is not this world's (as the player would
/// walk out).
fn sly2_travel_for(s: &mut Sly2, t: usize) -> Option<String> {
    let g = s.game.as_mut()?;
    let (row, here, exit) = {
        let j = g.world.sly2.as_deref()?.jobs.as_deref()?;
        // Only for the current task (a hub's choice of next jobs in other worlds is not a reason to leave).
        if j.current != Some(t) {
            return None;
        }
        let row = j.graph.tasks[t].row?;
        let exit = j.exits.iter().find(|e| e.row == Some(row)).map(|e| e.id);
        (row, j.row, exit)
    };
    if row == here {
        return None;
    }
    let id = exit?;
    engine::game::sly2_jobs::with(&mut g.world, |j, w| {
        arm(j, id);
        j.fire_exit_id(w, id)
    });
    Some(format!("left by exit {id} to row {row} (from row {here}), where the task is"))
}

/// A guard in the world, not knocked out, whose pocket holds a task item for task `id` (the item's or its placement's
/// option 906).
fn sly2_pocket_target(s: &Sly2, id: i32) -> Option<engine::object::Handle> {
    use engine::behavior::Behavior;
    let w = &s.game.as_ref()?.world;
    let names = |h: engine::object::Handle| {
        let own = w.options(h).and_then(|o| o.i32(906)) == Some(id);
        own || w.parent(h).and_then(|p| w.options(p)).and_then(|o| o.i32(906)) == Some(id)
    };
    w.handles().find(|&h| {
        w.in_world(h)
            && matches!(w.behavior(h), Some(Behavior::Sly2Guard(g)) if !g.knocked_out() && g.pocket_item.is_some_and(names))
    })
}

/// The pad for a pickpocket, `k` frames after it starts: Sly (Left) put behind guard `h` within reach, facing its
/// back, then R1 held (sneaking, so the guard ahead does not start a fight) and Circle (A) pressed; again every
/// second while the guard moves. No path-finding.
fn sly2_pickpocket_input(s: &mut Sly2, k: u32, h: engine::object::Handle) -> FrameInput {
    use engine::behavior::Behavior;
    if k == 0 {
        let b = platform_api::Buttons::LEFT.0;
        return FrameInput { pressed: b, buttons: b, ..Default::default() };
    }
    let r1 = platform_api::Buttons::ZR.0;
    if k % FPS == 10
        && let Some(g) = s.game.as_mut()
        && let (Some(at), Some(Behavior::Sly2Guard(gd))) = (g.world.position(h), g.world.behavior(h))
    {
        let (dx, dy) = (gd.yaw.cos(), gd.yaw.sin());
        let from = [at[0] - dx * SLY2_PICKPOCKET_BEHIND, at[1] - dy * SLY2_PICKPOCKET_BEHIND, at[2] + 5.0];
        let (sn, cs) = (gd.yaw.sin(), gd.yaw.cos());
        let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: from };
        g.systems.player.place(&mut g.world, to);
        return FrameInput { buttons: r1, ..Default::default() };
    }
    let a = if k % FPS == 30 { platform_api::Buttons::A.0 } else { 0 };
    FrameInput { pressed: a, buttons: r1 | a, ..Default::default() }
}

/// See [`sly2_pickpocket_input`]: how far behind the guard Sly stands, and how long a pickpocket play lasts.
const SLY2_PICKPOCKET_BEHIND: f32 = 80.0;
const SLY2_PICKPOCKET_WINDOW: u32 = 5 * FPS;

/// A crawl space (class 27, in the world) whose script brings in a volume (out of the world) whose script asks for
/// task `id` (interactables.md 9, missions.md 4.3: crawl space 124027 adds 133043 while Sly crawls, and 133043 asks
/// for 1813).
fn sly2_crawl_target(s: &Sly2, id: i32) -> Option<(engine::object::Handle, engine::object::Handle)> {
    let g = s.game.as_ref()?;
    let p = g.scripts()?.program();
    let w = &g.world;
    let names = |m: i16, needle: i32| {
        let needle = needle.to_string();
        m >= 0
            && (m as usize) < p.module_count()
            && p.definitions(m as usize).iter().any(|&(_, n)| {
                format!("{}", p.display(n)).split(|c: char| !c.is_ascii_digit() && c != '-').any(|t| t == needle)
            })
    };
    w.handles().filter(|&v| matches!(w.class(v), Some(24 | 25)) && !w.in_world(v) && names(w.module(v), id)).find_map(
        |v| {
            let vid = w.sly2_id(v)?;
            let c = w.handles().find(|&c| w.class(c) == Some(27) && w.in_world(c) && names(w.module(c), vid))?;
            Some((c, v))
        },
    )
}

/// The pad for a crawl, `k` frames after it starts (interactables.md 9): Sly (Left) put in the middle of crawl space
/// `c` on its floor, Circle (A) there starts the crawl, which brings volume `v` in; then Sly reaches `v` while still
/// crawling (its visit flag: the crawl's movement is not played, and putting him in the volume's middle, out of
/// cover, ended the crawl and took the volume out before it saw him), and is put in it afterwards.
fn sly2_crawl_input(s: &mut Sly2, k: u32, c: engine::object::Handle, v: engine::object::Handle) -> FrameInput {
    let b = match k {
        0 => platform_api::Buttons::LEFT.0,
        20 | 22 | 24 => platform_api::Buttons::A.0,
        _ => 0,
    };
    if k == 10 {
        sly2_enter_volume(s, c);
    }
    if k == SLY2_CRAWL_INTO
        && let Some(g) = s.game.as_mut()
        && g.world.in_world(v)
        && let Some(engine::behavior::Behavior::Sly2Volume(t)) = g.world.behavior_mut(v)
    {
        t.visit = true;
    }
    if k == SLY2_CRAWL_INTO + 10 && s.game.as_ref().is_some_and(|g| g.world.in_world(v)) {
        sly2_enter_volume(s, v);
    }
    FrameInput { pressed: b, buttons: b, ..Default::default() }
}

/// See [`sly2_crawl_input`]: when Sly is put in the volume the crawl brought in, and how long a crawl play lasts.
const SLY2_CRAWL_INTO: u32 = 60;
const SLY2_CRAWL_WINDOW: u32 = 3 * FPS;

/// See [`sly2_toss_input`]: the farthest Murray throws (characters.md 7: range 200 to 1500).
const SLY2_TOSS_RANGE: f32 = 1500.0;
/// See [`sly2_toss_input`]: how far from the volume Murray is carried to throw when he is beyond the range.
const SLY2_TOSS_DISTANCE: f32 = 1000.0;
/// The most tosses one run plays.
const SLY2_TOSSES_MAX: u32 = 12;

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Puts the played character on the floor a short way from `h`, facing it, for an attack (no path-finding).
fn sly2_stand_by(s: &mut Sly2, h: engine::object::Handle) -> Option<()> {
    let g = s.game.as_mut()?;
    let at = g.world.position(h)?;
    let me = g.world.position(g.world.player?)?;
    let (dx, dy) = (me[0] - at[0], me[1] - at[1]);
    let l = (dx * dx + dy * dy).sqrt().max(1.0);
    // At the target's own height (a creature stands on the floor; a probe found floors below it and the character
    // fell through), a little above it.
    let floor = [at[0] + dx / l * SLY2_ATTACK_STANDOFF, at[1] + dy / l * SLY2_ATTACK_STANDOFF, at[2] + 5.0];
    let from = floor;
    let yaw = (at[1] - from[1]).atan2(at[0] - from[0]);
    let (sn, cs) = (yaw.sin(), yaw.cos());
    let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: floor };
    g.systems.player.place(&mut g.world, to);
    Some(())
}

/// Stands the played character where photo target `h` fills the frame (its size over the frame's 120 units at the
/// camera's focal length), facing it, and turns the binocucom on (R3 in play). No path-finding.
fn sly2_frame_photo(s: &mut Sly2, h: engine::object::Handle) -> Option<()> {
    // The binocucom is closed first, so it opens again at the new eye.
    s.photo = None;
    let g = s.game.as_mut()?;
    let at = g.world.position(h)?;
    let t = game::sly2::photo::Target::new(&g.world, h, &game::sly2::photo::OPTIONS);
    let params = game::sly2::photo::params(g.render.camera.fov_y);
    let half = params.fov_y / 2.0;
    let focal = params.screen[1] / 2.0 * half.cos() / half.sin();
    let want = focal * t.size / game::photo::FRAME;
    let player = g.world.player?;
    let me = g.world.position(player)?;
    let (dx, dy) = (me[0] - at[0], me[1] - at[1]);
    let l = (dx * dx + dy * dy).sqrt().max(1.0);
    // The eye is EYE_HEIGHT above the feet; level with the target when it can be, else on the near floor.
    let z = at[2] - game::sly2::photo::EYE_HEIGHT;
    let from = [at[0] + dx / l * want, at[1] + dy / l * want, z];
    let yaw = (at[1] - from[1]).atan2(at[0] - from[0]);
    let (sn, cs) = (yaw.sin(), yaw.cos());
    let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: from };
    g.systems.player.place(&mut g.world, to);
    g.world.shared.camera_binocucom = Some((player, false));
    Some(())
}

/// The follow (missions.md 4.3): a trigger volume whose parent is a character (class 56 and derived) is a leash. Every
/// [`SLY2_FOLLOW_EVERY`] frames the played character is put on the floor [`SLY2_FOLLOW_BEHIND`] units behind the
/// character (standing in for walking after it out of sight); once the character's command list is at its last
/// command, inside the volume (class 22 and derived) that list's script names, if there is one. Returns whether a
/// follow is on.
fn sly2_follow(s: &mut Sly2, f: u32, trail: &mut Vec<[f32; 3]>) -> bool {
    use engine::behavior::Behavior;
    let Some(g) = s.game.as_mut() else { return false };
    let w = &g.world;
    let leash = w.handles().find(|&h| {
        matches!(w.class(h), Some(24 | 25))
            && w.in_world(h)
            && w.parent(h).is_some_and(|p| w.class(p).is_some_and(|c| engine::world::sly2::is_a(c, 56)))
    });
    let Some(who) = leash.and_then(|h| w.parent(h)) else {
        trail.clear();
        return false;
    };
    // The followed character's trail: floor it has walked on, behind it.
    if let Some(p) = w.position(who)
        && trail.last().is_none_or(|l| d2(*l, p) >= SLY2_TRAIL_STEP * SLY2_TRAIL_STEP)
    {
        trail.push(p);
    }
    if !f.is_multiple_of(SLY2_FOLLOW_EVERY) {
        return true;
    }
    let (Some(at), Some(Behavior::Sly2Guard(gd))) = (w.position(who), w.behavior(who)) else { return true };
    // At the end of its list: inside the volume the list's script tests.
    let end_volume = gd.commands.as_ref().and_then(|run| {
        let Some(Behavior::Sly2Commands(l)) = w.behavior(run.list) else { return None };
        (run.at + 1 >= l.commands.len()).then_some(run.list)?;
        let m = w.module(run.list);
        let p = g.scripts()?.program();
        let text: String = if m >= 0 && (m as usize) < p.module_count() {
            p.definitions(m as usize).iter().map(|&(_, n)| format!("{}", p.display(n))).collect()
        } else {
            String::new()
        };
        w.handles().find(|&v| {
            w.class(v).is_some_and(|c| engine::world::sly2::is_a(c, 22))
                && w.sly2_id(v).is_some_and(|id| text.split(|c: char| !c.is_ascii_digit()).any(|t| t == id.to_string()))
        })
    });
    let yaw = gd.yaw;
    let q = g.systems.physics.queries();
    let view = gd.view.and_then(|v| engine::behavior::sly2_props::volume_box(w, v));
    let out_of_view = |p: [f32; 3]| {
        let c = [p[0], p[1], p[2] + engine::behavior::sly2_sensors::CHARACTER_CENTRE];
        view.as_ref().is_none_or(|v| !engine::behavior::sly2_props::in_volume(v, c))
    };
    let floor = match end_volume.and_then(|v| engine::behavior::sly2_props::volume_box(w, v)) {
        // Inside the volume on a floor, out of its view, as far from it as the volume allows (a 5 × 5 grid).
        Some((m, lo, hi)) => (0..25)
            .filter_map(|i| {
                let (u, v) = ((i % 5) as f32 / 4.0, (i / 5) as f32 / 4.0);
                let top = m.transform_point([lo[0] + (hi[0] - lo[0]) * u, lo[1] + (hi[1] - lo[1]) * v, hi[2]]);
                q.ground(w, top, hi[2] - lo[2]).map(|x| x.0)
            })
            .filter(|&p| out_of_view(p))
            .max_by(|a, b| d2(*a, at).total_cmp(&d2(*b, at))),
        // On its trail (floor it walked on), the newest point that is out of its view volume (the detector's own
        // box, guards.md 3), inside the leash and at least SLY2_FOLLOW_NEAR away.
        None => {
            use engine::behavior::sly2_props::{in_volume, volume_box};
            let view = gd.view.and_then(|v| volume_box(w, v));
            let leash_box = leash.and_then(|l| volume_box(w, l));
            let ok = |p: [f32; 3]| {
                let c = [p[0], p[1], p[2] + engine::behavior::sly2_sensors::CHARACTER_CENTRE];
                view.as_ref().is_none_or(|v| !in_volume(v, c))
                    && leash_box.as_ref().is_none_or(|l| in_volume(l, c))
                    && d2(p, at) >= SLY2_FOLLOW_NEAR * SLY2_FOLLOW_NEAR
            };
            // Farther than the view volume reaches to its sides, so a turn does not bring the player into it; none
            // such yet: the player stays where it is (out of view).
            let back = trail.iter().rev().copied().find(|&p| ok(p));
            back.map(|p| q.ground(w, [p[0], p[1], p[2] + 100.0], 300.0).map_or(p, |x| x.0))
        }
    };
    let Some(floor) = floor else { return true };
    let (sn, cs) = (yaw.sin(), yaw.cos());
    let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: floor };
    g.systems.player.place(&mut g.world, to);
    true
}

/// An escort (guards.md 4.4, m1_neela's chase): while a character walks a list as an escort, the played character is
/// kept inside its view volume, on a floor, at least [`SLY2_ESCORT_NEAR`] from it (not hitting it), with no pad input.
/// No path-finding: it is placed.
fn sly2_escort(s: &mut Sly2, f: u32) -> bool {
    use engine::behavior::Behavior;
    let Some(g) = s.game.as_mut() else { return false };
    let w = &g.world;
    let who = w
        .handles()
        .find(|&h| w.in_world(h) && matches!(w.behavior(h), Some(Behavior::Sly2Guard(gd)) if gd.escort.is_some()));
    let Some(who) = who else { return false };
    let Some(Behavior::Sly2Guard(gd)) = w.behavior(who) else { return true };
    let Some(at) = w.position(who) else { return true };
    let Some((m, lo, hi)) = gd.view.and_then(|v| engine::behavior::sly2_props::volume_box(w, v)) else { return true };
    let q = g.systems.physics.queries();
    // The point of the volume farthest from the character that has a floor, of a 5 × 5 grid at its middle height.
    let spot = (0..25)
        .filter_map(|i| {
            let (u, v) = ((i % 5) as f32 / 4.0, (i / 5) as f32 / 4.0);
            let top = m.transform_point([lo[0] + (hi[0] - lo[0]) * u, lo[1] + (hi[1] - lo[1]) * v, hi[2]]);
            q.ground(w, top, hi[2] - lo[2]).map(|x| x.0)
        })
        .filter(|&p| {
            let c = [p[0], p[1], p[2] + engine::behavior::sly2_sensors::CHARACTER_CENTRE];
            engine::behavior::sly2_props::in_volume(&(m, lo, hi), c)
                && d2(p, at) >= SLY2_ESCORT_NEAR * SLY2_ESCORT_NEAR
                && (p[2] - at[2]).abs() <= SLY2_ESCORT_LEVEL
        })
        .min_by(|a, b| d2(*a, at).total_cmp(&d2(*b, at)));
    let me = w.player.and_then(|h| w.position(h));
    // No floor on its level inside its view volume (it has not come down to where the chase starts yet): not
    // following, so the stall timer runs and the other plays (the walk to the task's marker) come.
    let Some(p) = spot else { return false };
    if me.is_some_and(|me| {
        let c = [me[0], me[1], me[2] + engine::behavior::sly2_sensors::CHARACTER_CENTRE];
        engine::behavior::sly2_props::in_volume(&(m, lo, hi), c)
            && d2(me, at) >= SLY2_ESCORT_NEAR * SLY2_ESCORT_NEAR
            && (me[2] - at[2]).abs() <= SLY2_ESCORT_LEVEL
    }) {
        return true;
    }
    if !f.is_multiple_of(SLY2_FOLLOW_EVERY) {
        return true;
    }
    let yaw = (at[1] - p[1]).atan2(at[0] - p[0]);
    let (sn, cs) = (yaw.sin(), yaw.cos());
    let to = engine::math::Affine { cols: [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]], t: p };
    g.systems.player.place(&mut g.world, to);
    true
}

/// See [`sly2_escort`]: on a floor within this height of the escorted character's (its own level, not one below).
const SLY2_ESCORT_LEVEL: f32 = 150.0;
/// See [`sly2_escort`]: never nearer to the escorted character than this.
const SLY2_ESCORT_NEAR: f32 = 250.0;

/// See [`sly2_follow`]: frames between placements.
pub const SLY2_FOLLOW_EVERY: u32 = FPS / 4;
/// See [`sly2_follow`]: distance kept behind the followed character.
pub const SLY2_FOLLOW_BEHIND: f32 = 400.0;
/// See [`sly2_follow`]: never nearer to the followed character than this (Dimitri's view volume reaches about
/// 1,160 ahead and 1,000 to each side, so a turn would bring a nearer follower into it).
pub const SLY2_FOLLOW_NEAR: f32 = 1300.0;
/// See [`sly2_follow`]: spacing of the trail's points.
pub const SLY2_TRAIL_STEP: f32 = 50.0;

/// How far from its target an attack starts.
pub const SLY2_ATTACK_STANDOFF: f32 = 60.0;

/// An attack as the pad gives it, `k` frames after it starts: jump (B, the gang's Cross) and, in the air, attack (Y,
/// Square), three times a second apart.
fn sly2_attack_input(k: u32) -> FrameInput {
    let b = match k % FPS {
        0 => platform_api::Buttons::B.0,
        12 => platform_api::Buttons::Y.0,
        _ => 0,
    };
    FrameInput { pressed: b, buttons: b, ..Default::default() }
}

fn arm(j: &mut engine::game::sly2_jobs::Jobs, id: i32) {
    if let Some(e) = j.exits.iter_mut().find(|e| e.id == id) {
        e.state = engine::game::sly2_jobs::flow::ExitState::Armed;
    }
}
