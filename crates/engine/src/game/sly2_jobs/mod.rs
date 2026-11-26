//! The Sly 2 job and progression system (`specs/sly2/missions.md`, `level-flow.md`, `saving.md`,
//! `safehouse-and-shop.md`, `gameplay-overview.md`).
//!
//! - **Graph** ([`graph`]): every world of an episode carries the episode's jobs (class 132) and tasks (class 133);
//!   they are read into one graph with their switches, goals, links and save keys.
//! - **Progress**: the live progress block (`save::sly2::Progress`), the single source of saved state. Task states,
//!   play and done counts, job play counts, completion, timers and difficulty help are written into it as they
//!   change (missions.md 7), along with the coins, loot, owned items, `widCurrent`, `episodeCurrent`,
//!   `episodeBest`, total and per-episode play time, and the episode's percent complete (the episode's group is the
//!   `episode` group's child chosen by the level table's episode byte). The current job, current task and
//!   checkpoint task are not saved: a world change carries them ([`Carry`]); after a load, a task with option 1273
//!   restored as available is current again.
//! - **Task rules** (missions.md 4): requests, one step up or 1 → 3, forced requests skip the checks, a closed task
//!   never changes; propagation to prerequisites and successors; object switches (limited to their row) and
//!   state-machine goals per state and for groups 4 (became current) and 5 (stopped being current); the marker
//!   (option 1279) present only while available; one current task; the episode percentage; the autosave rule.
//! - **Jobs** (missions.md 5): begin when a task of theirs becomes current, the job timer (only while the
//!   controller was touched in the last 10 s), completion (the "job complete" screen is reported, then the job
//!   closes), the next task or the job's exit, the end (the hub respawn point goes back to 393), restarts at the
//!   checkpoint task, ending and closing events (options 1236, 1237).
//! - **Difficulty help** (missions.md 6): +0.1 on a defeat (task event 0x10) and a job restart (0x1D), −0.2 the first
//!   time a task of the current job is done; the job's own value during a job, else the mean of the last three
//!   jobs' values (`suck_history`), shifted in when a job ends. `Sly2Level::difficulty_help` follows it.
//! - **Events.** Task events from the host (operation 139), the props and volumes (`Sly2Level::task_events`) and
//!   from here are broadcast as script event 17 (event, object) to the current job's available tasks, then the
//!   current job. A task that becomes current gets script event 19, one that stops being current 20; a job that
//!   begins 19 and ends 20; a task's state change 22 before and 21 after. Task-state requests from props and
//!   volumes (`World::shared.task_requests`) are applied each frame.
//! - **Script interface** (through the script host): task options 1280 (script 1104) requests a state
//!   `(state, by)`, 1281 (1105) the same forced, 1282 (1082) reads the state, 1267 (1078) the play count, 1284
//!   raises script event 23 on the task, 1290 to 1293 force state 0 to 3, 1294 makes the task current; job options
//!   1238 (1067) count a failure (help +0.1), 1239 (1068) restart at the checkpoint (the jobs call it on a defeat,
//!   event 16), 1240 end the job, 1250 reset its tasks' play counts, 1255 is a load-time switch list; exits fire
//!   with 786 (script 235) and warp points move the player with 778.
//! - **Worlds** ([`flow`]): exits fire when the player enters their volume after leaving it (or by scripts and
//!   tasks), apply their task states and request a load with an arrival point (393 by default), a wipe and movies.
//!   The session loads the world, attaches the jobs to it again ([`attach`]) and arrives at the point ([`arrive`]):
//!   the player is placed there, the point's lists are applied, every exit goes back to its first state, the point
//!   gets script event 5 and task event 26 is broadcast, then task event 5.
//! - **Safe house** ([`shop`]): buying and selling at the computer, and the job help pages.
//!
//! State-machine goals are collected in [`Jobs::machine_goals`] for the state machines' owner; Sly 2 state machines
//! are not run by the engine yet.

pub mod flow;
pub mod graph;
pub mod shop;
#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

use formats::geometry::Affine;
use save::sly2::{Progress, SaveValue};

use crate::object::Handle;
use crate::world::World;
use flow::{Exit, ExitState, LoadRequest};
use graph::{Graph, Ids};

/// Task states (missions.md 4).
pub mod state {
    /// Not yet restored from the save.
    pub const UNKNOWN: i8 = -1;
    /// A prerequisite is not done.
    pub const LOCKED: i8 = 0;
    /// Can be started.
    pub const AVAILABLE: i8 = 1;
    /// Completed.
    pub const DONE: i8 = 2;
    /// Done, and its job finished.
    pub const CLOSED: i8 = 3;
    /// Switch group: the task became current.
    pub const BECAME_CURRENT: i32 = 4;
    /// Switch group: the task stopped being current.
    pub const STOPPED_CURRENT: i32 = 5;
}

/// Task events (missions.md, level-flow.md, safehouse-and-shop.md).
pub mod event {
    /// The world started (level-flow.md 7, step 8).
    pub const WORLD_STARTED: u32 = 5;
    /// The player was defeated.
    pub const DEFEAT: u32 = 0x10;
    /// The current player's loot is dropped (a job begins).
    pub const DROP_LOOT: u32 = 0x1C;
    /// A job restart.
    pub const JOB_RESTART: u32 = 0x1D;
    /// An arrival point or warp was applied.
    pub const ARRIVED: u32 = 0x1A;
    /// A whole job was restarted.
    pub const JOB_RESTARTED: u32 = 0x1F;
    /// A job closed.
    pub const JOB_CLOSED: u32 = 0x20;
    /// A job began.
    pub const JOB_BEGAN: u32 = 0x21;
    /// A job ended.
    pub const JOB_ENDED: u32 = 0x22;
    /// A task became current.
    pub const TASK_CURRENT: u32 = 0x23;
    /// A task stopped being current.
    pub const TASK_NOT_CURRENT: u32 = 0x24;
    /// A task changed state (after).
    pub const STATE_AFTER: u32 = 0x25;
    /// A task is about to change state.
    pub const STATE_BEFORE: u32 = 0x26;
    /// A request was refused.
    pub const REFUSED: u32 = 0x27;
    /// An item became owned (a purchase, operation 139).
    pub const ITEM_OWNED: u32 = 59;
}

/// Script events sent to job and task objects.
pub mod script_event {
    /// A task event: arguments (event, object).
    pub const TASK_EVENT: u32 = 17;
    /// A task became current; a job began.
    pub const BEGIN: u32 = 19;
    /// A task stopped being current; a job ended.
    pub const END: u32 = 20;
    /// A task changed state (after).
    pub const STATE: u32 = 21;
    /// A task is about to change state.
    pub const STATE_BEFORE: u32 = 22;
    /// Raised on a task by its method 1284.
    pub const TASK_SIGNAL: u32 = 23;
}

/// Script-visible job and task options (stream IDs).
pub mod opt {
    /// Task method: request a state (state, by).
    pub const REQUEST: u16 = 1280;
    /// Task method: request a state, forced.
    pub const FORCE: u16 = 1281;
    /// Task: its state.
    pub const STATE: u16 = 1282;
    /// Task: its play count.
    pub const PLAYED: u16 = 1267;
    /// Job procedure: count a failure (difficulty help +0.1; the job goes on).
    pub const FAILURE: u16 = 1238;
    /// Job procedure: restart at the checkpoint task.
    pub const RESTART: u16 = 1239;
    /// Job procedure: end the job.
    pub const END_JOB: u16 = 1240;
    /// Job procedure: reset its tasks' play counts.
    pub const RESET_COUNTS: u16 = 1250;
    /// Task method: raise script event 23 on the task.
    pub const SIGNAL: u16 = 1284;
    /// Task methods: force state 0, 1, 2, 3 (1290 to 1293).
    pub const FORCE_LOCKED: u16 = 1290;
    pub const FORCE_CLOSED: u16 = 1293;
    /// Task method: make the task current.
    pub const MAKE_CURRENT: u16 = 1294;
}

/// What a world change carries besides the progress block: the current job, task and checkpoint task (object IDs,
/// 0 none), the job timer and help. The game keeps these in memory; they are not saved (saving.md).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Carry {
    pub job: i32,
    pub task: i32,
    pub checkpoint: i32,
    pub timer: f32,
    pub help: f32,
}

/// Seconds without controller input after which the job timer stops (missions.md 5).
pub const IDLE_LIMIT: f32 = 10.0;
/// Difficulty help steps (executable 0x2BDC00).
pub const HELP_DEFEAT: f32 = 0.1;
pub const HELP_DONE: f32 = -0.2;
/// The task a new game queues (level-flow.md 3).
pub const NEW_GAME_TASK: i32 = 1583;
/// The intro hub's row (the boot world).
pub const BOOT_ROW: i16 = 0;

/// Something the rules did that touches the world.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Put an object in or out of the world.
    Present { object: i32, owner: i32, on: bool },
    /// Set a state machine's goal.
    Goal { object: i32, machine: i32, goal: i32, instant: bool },
    /// A script event to a job or task object (slot), with an optional task-event argument pair.
    Script { slot: u32, event: u32, task_event: Option<u32> },
    /// A task event broadcast to the jobs and available tasks, with the object (slot) it is about.
    Broadcast { event: u32, object: Option<u32> },
    /// Fire an exit.
    FireExit(i32),
}

/// What the game shows or does for the job system, for the session and the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// The objective prompt of the current task (none: removed).
    Objective(Option<i16>),
    /// A job completed: its "job complete" screen object and parts.
    JobComplete { job: i32, screen: Option<[i32; 3]> },
    /// The game would autosave now (missions.md 4 and 5).
    Autosave,
    /// A request was refused.
    Refused(i32),
}

/// The job system of the running world.
#[derive(Clone, Debug)]
pub struct Jobs {
    /// The graph.
    pub graph: Graph,
    /// The live progress block.
    pub progress: Progress,
    /// Task states, by task index.
    pub states: Vec<i8>,
    /// Current task, current job, checkpoint task.
    pub current: Option<usize>,
    pub job: Option<usize>,
    pub checkpoint: Option<usize>,
    /// The world's level row and episode.
    pub row: i16,
    pub episode: u8,
    /// The job timer and seconds since the controller was last touched.
    pub timer: f32,
    pub idle: f32,
    /// The difficulty help.
    pub help: f32,
    /// Tasks done at least once during the current job (the −0.2 step applies once each).
    done_in_job: BTreeSet<usize>,
    /// The arrival point a reload uses (393 in a hub after a job).
    pub respawn: i32,
    /// The world's exits.
    pub exits: Vec<Exit>,
    /// Sly 3 hook: the script event a job or task object gets for one of [`script_event`]'s (`None`: not raised).
    /// Sly 2's are as listed; Sly 3 numbers its events differently (`game::sly3::jobs`).
    pub script_events: fn(u32) -> Option<u32>,
    /// Sly 3 hook: the code a task event (`event`) is broadcast with; the identity in Sly 2.
    pub task_codes: fn(u32) -> u32,
    /// Sly 3 hook: the class whose objects in the world also hear every task event, after the tasks and before the
    /// current job (Sly 3's macro-package listeners, class 147; missions.md 4.2). `None` in Sly 2.
    pub listener_class: Option<i16>,
    /// Sly 3 hook (option 1405): tasks made current that ask for "done" at once, finished one after another by the
    /// outermost [`Jobs::make_current`] rather than by recursion (a chain of such tasks would nest calls).
    pub done_queue: alloc::collections::VecDeque<usize>,
    /// Whether the outermost [`Jobs::make_current`] is finishing [`Jobs::done_queue`].
    pub finishing: bool,
    /// Sly 3 hook: Sly 3's order of becoming current (the job begins after the task's events and exit). Sly 2's
    /// (false) follows missions.md 4.2. The world start (the remembered job and task begun again) is both games'.
    pub sly3_rules: bool,
    /// Sly 3 hook: the tasks whose state change is being applied, which back-fill does not raise again.
    pending: Vec<usize>,
    /// The job a queued 1405 "done" has just completed (so that it is not begun again).
    completed_now: Option<usize>,
    /// Sly 3 hook: exits fired that are not in [`Jobs::exits`] (a Sly 3 world's exits are run by the Sly 3
    /// session, which takes these).
    pub fired_elsewhere: Vec<i32>,
    /// A load the world asked for; the session performs it.
    pub load: Option<LoadRequest>,
    /// An exit fired and its load not yet requested (level-flow.md 5.5): the request and the seconds left of its
    /// option-785 delay. The game is paused meanwhile; [`Jobs::step_exit`] makes the request in a later update.
    pub exit_pending: Option<(LoadRequest, f32)>,
    /// The pending exit was fired in the current update (its load is requested in a later one).
    exit_fresh: bool,
    /// Where to put the player (a warp), applied by the game.
    pub place: Option<Affine>,
    /// What the UI and the session should know, oldest first.
    pub notices: Vec<Notice>,
    /// State-machine goals set, for the state machines' owner: (object, machine, goal, instant).
    pub machine_goals: Vec<(i32, i32, i32, bool)>,
    effects: Vec<Effect>,
    /// Applying a restore: goals are instant and no autosave is asked.
    restoring: bool,
    /// The episode's save group (`episode/<k-th child>` for the level table's episode byte k).
    episode_group: Option<Vec<u16>>,
}

fn add(p: &mut Progress, key: &Option<Vec<u16>>, by: i32) {
    if let Some(k) = key {
        let v = p.get(k, None).map_or(0, SaveValue::as_i32);
        p.set(k, None, SaveValue::Int(v + by));
    }
}

fn put(p: &mut Progress, key: &Option<Vec<u16>>, v: SaveValue) {
    if let Some(k) = key {
        p.set(k, None, v);
    }
}

fn get(p: &Progress, key: &Option<Vec<u16>>) -> Option<SaveValue> {
    key.as_ref().and_then(|k| p.get(k, None))
}

impl Jobs {
    /// The job system for `world` with `progress` (state not restored yet; see [`Jobs::restore`]).
    pub fn new(world: &World, progress: Progress) -> Jobs {
        Jobs::new_with(world, progress, &Ids::SLY2)
    }

    /// Sly 3 hook: the job system read with `ids`. For a Sly 3 world the episode is `World::shared.episode` (the
    /// `episode` save group's child of that index), and the exits are left to the Sly 3 session.
    pub fn new_with(world: &World, progress: Progress, ids: &Ids) -> Jobs {
        let tree = progress.tree.clone();
        let names = |id: u16| tree.name(id).map(alloc::string::String::from);
        let graph = Graph::read_with(world, &names, ids);
        let row = world.info.index as i16;
        let sly3 = world.sly3.is_some();
        let episode = match sly3 {
            true => world.shared.episode,
            false => world
                .sly2
                .as_deref()
                .and_then(|l| l.header.level_rows.get(row.max(0) as usize))
                .map_or(0, |r| r.episode),
        };
        let n = graph.tasks.len();
        Jobs {
            graph,
            progress,
            states: vec![state::UNKNOWN; n],
            current: None,
            job: None,
            checkpoint: None,
            row,
            episode,
            timer: 0.0,
            idle: 0.0,
            help: 0.0,
            done_in_job: BTreeSet::new(),
            respawn: flow::DEFAULT_POINT,
            exits: if sly3 { Vec::new() } else { flow::exits(world) },
            fired_elsewhere: Vec::new(),
            script_events: Some,
            task_codes: |e| e,
            listener_class: None,
            done_queue: alloc::collections::VecDeque::new(),
            finishing: false,
            sly3_rules: false,
            pending: Vec::new(),
            completed_now: None,
            load: None,
            exit_pending: None,
            exit_fresh: false,
            place: None,
            notices: Vec::new(),
            machine_goals: Vec::new(),
            effects: Vec::new(),
            restoring: false,
            episode_group: None,
        }
        .with_episode_group()
    }

    /// The episode's save group: the `episode` group's child chosen by the level table's episode byte.
    fn with_episode_group(mut self) -> Jobs {
        let t = self.progress.tree.clone();
        self.episode_group = t.top("episode").and_then(|g| {
            let child = t.children(g).nth(self.episode as usize)?;
            Some(alloc::vec![t.name_id("episode")?, t.records()[child].name as u16])
        });
        self
    }

    // ------------------------------------------------------------------ the progress block

    /// Writes the job timer and help into the block (their save keys).
    pub fn store_current(&mut self) {
        if let Some(j) = self.job {
            let keys = self.graph.jobs[j].keys.clone();
            put(&mut self.progress, &keys.time, SaveValue::Float(self.timer));
            put(&mut self.progress, &keys.help, SaveValue::Float(self.help));
        }
    }

    /// What a world change carries (see [`Carry`]).
    pub fn carry(&self) -> Carry {
        Carry {
            job: self.job.map_or(0, |j| self.graph.jobs[j].id),
            task: self.current.map_or(0, |t| self.graph.tasks[t].id),
            checkpoint: self.checkpoint.map_or(0, |t| self.graph.tasks[t].id),
            timer: self.timer,
            help: self.help,
        }
    }

    /// The episode's save group with `name` appended.
    pub fn episode_key(&self, name: &str) -> Option<Vec<u16>> {
        let mut p = self.episode_group.clone()?;
        p.push(self.progress.tree.name_id(name)?);
        Some(p)
    }

    // ------------------------------------------------------------------ restoring

    /// Restores the progress into the world (missions.md 7, saving.md 3): the world's row and episode, every task's
    /// state in graph order with its switches, the jobs' load-time switches, then the current job, task and
    /// checkpoint task: carried over a world change, or after a load the task with option 1273 restored as
    /// available. `queued` is a task to request afterwards (a new game's first task).
    pub fn restore(&mut self, queued: Option<i32>, carry: Option<Carry>) {
        self.restoring = true;
        let p = &mut self.progress;
        p.set_named(&["widCurrent"], None, SaveValue::Int(self.row as i32));
        p.set_named(&["episodeCurrent"], None, SaveValue::Int(self.episode as i32));
        if p.int(&["episodeBest"]) < self.episode as i32 {
            p.set_named(&["episodeBest"], None, SaveValue::Int(self.episode as i32));
        }
        // Saved states, in prerequisite-first graph order (missions.md 7.1).
        let order = self.graph.prerequisite_order();
        for &t in &order {
            let keys = &self.graph.tasks[t].keys;
            let s = get(&self.progress, &keys.state).map_or(0, SaveValue::as_i32);
            let restarted = get(&self.progress, &keys.restarted).is_some_and(|v| v.as_i32() != 0);
            let mut s = s.clamp(0, 3) as i8;
            if self.sly3_rules {
                // Sly 3 hook (missions.md 4.1a): with `from_memcard` set, a job's task saved as available or done is
                // recomputed from its prerequisites (all at least done: available, else locked).
                if restarted && self.graph.tasks[t].job.is_some() && (s == state::AVAILABLE || s == state::DONE) {
                    let met = self.graph.tasks[t].prereq.iter().all(|&p| self.states[p] >= state::DONE);
                    s = if met { state::AVAILABLE } else { state::LOCKED };
                }
            } else if restarted && s == state::AVAILABLE && !self.prereqs_met(t) {
                s = state::LOCKED;
            }
            self.states[t] = s;
        }
        // A task with no saved state is available if it has no prerequisites, otherwise locked (missions.md 7). The
        // restore does not propagate: a saved locked task whose prerequisites are met stays locked, as it was in
        // live play (where successors are re-evaluated only when a task becomes done or closed, missions.md 4). An
        // earlier fixed-point pass opened such tasks (task 1776, `t1_recon_brawl`, after a save and continue).
        for &t in &order {
            if self.states[t] == state::LOCKED && self.graph.tasks[t].prereq.is_empty() {
                self.states[t] = state::AVAILABLE;
                self.save_state(t);
                // Object-model hook (level-flow.md 4.1): the change raises the task's "after state change" event, which
                // the episodes' macro modules answer by starting the opening sequence. Not for a new game, whose
                // queued task (1583) lies past the intro's attract-mode tasks that would answer it.
                // Sly 3 raises its own events below.
                if queued.is_none() && !self.sly3_rules {
                    let slot = self.graph.tasks[t].slot;
                    self.effects.push(Effect::Script { slot, event: script_event::STATE, task_event: None });
                }
            }
        }
        for t in 0..self.graph.tasks.len() {
            let s = self.states[t] as i32;
            self.switch(t, s, true);
        }
        // Sly 3 hook (sly3/missions.md 4.1a): every task starts at −1 from the load, so the restore's set is a change
        // for each one, whatever it restores, in mode 1, which still raises events: the "before" task event and
        // script event, then the "after" ones (delivery skips off-world tasks).
        if self.sly3_rules {
            for &t in &order {
                let slot = self.graph.tasks[t].slot;
                let here = Some(slot);
                self.effects.push(Effect::Script { slot, event: script_event::STATE_BEFORE, task_event: None });
                self.effects.push(Effect::Broadcast { event: event::STATE_BEFORE, object: here });
                self.effects.push(Effect::Script { slot, event: script_event::STATE, task_event: None });
                self.effects.push(Effect::Broadcast { event: event::STATE_AFTER, object: here });
            }
        }
        // The current job, task and checkpoint from session memory (the carry; none after a card load). A task with
        // option 1273 restored as available replaces the remembered task, the last in graph order winning, on every
        // world start (missions.md 7.1). Option 1272 is not applied here.
        let c = carry.unwrap_or_default();
        let mut current = self.graph.task_ids.get(&c.task).copied();
        if let Some(&t) =
            order.iter().rev().find(|&&t| self.graph.tasks[t].resume && self.states[t] == state::AVAILABLE)
        {
            current = Some(t);
        }
        let job = self.graph.job_ids.get(&c.job).copied();
        for j in 0..self.graph.jobs.len() {
            self.job_switches(j);
        }
        self.checkpoint = self.graph.task_ids.get(&c.checkpoint).copied();
        // Sly 3: step 2 runs in restoring mode too (sly3/missions.md 4.1a, mode 1): events fire, but no back-fill
        // and no follow-ups of the state setter.
        self.restoring = self.sly3_rules;
        // The remembered job begins again, then the task becomes current (which begins its own job when that is
        // another one); the carried timer and help go on; then every task is saved again.
        match job {
            Some(j) => self.begin_job(j),
            None => self.help = self.history_mean(),
        }
        if let Some(t) = current {
            self.make_current(t);
        }
        self.restoring = false;
        if let (Some(c), Some(j)) = (carry, self.job)
            && self.graph.job_ids.get(&c.job) == Some(&j)
        {
            self.timer = c.timer;
            self.help = c.help;
        }
        for t in 0..self.graph.tasks.len() {
            self.save_state(t);
        }
        // A queued task is forced: a new game's task 1583 lies past the attract-mode tasks [working choice].
        if let Some(t) = queued.and_then(|id| self.graph.task_ids.get(&id).copied()) {
            self.request_current(t, true);
        }
        self.update_percent();
    }

    // ------------------------------------------------------------------ task rules

    /// Whether every prerequisite of `t` is done (or available and counting as done, option 1271).
    pub fn prereqs_met(&self, t: usize) -> bool {
        self.graph.tasks[t].prereq.iter().all(|&p| {
            self.states[p] >= state::DONE || (self.states[p] == state::AVAILABLE && self.graph.tasks[p].counts_done)
        })
    }

    /// The state of the task with object ID `id`.
    pub fn task_state(&self, id: i32) -> Option<i8> {
        self.graph.task_ids.get(&id).map(|&t| self.states[t])
    }

    /// Asks for task `t` to become current: a locked task with its prerequisites met becomes available first.
    /// Refused (and announced, event 0x27) otherwise.
    pub fn request_current(&mut self, t: usize, forced: bool) -> bool {
        match self.states[t] {
            state::LOCKED if forced || self.prereqs_met(t) => self.set_state(t, state::AVAILABLE),
            state::AVAILABLE => {}
            _ if forced && self.states[t] != state::CLOSED => self.set_state(t, state::AVAILABLE),
            _ => return self.refuse(t),
        }
        self.make_current(t);
        true
    }

    /// Asks for task `t` to reach state `want` (missions.md 4). A request for "available" is a request to become
    /// current. Returns whether it was applied.
    pub fn request(&mut self, t: usize, want: i8, forced: bool) -> bool {
        // −1 and 4 ask for the task to become current (missions.md 3.1, method 1280's first argument).
        if want == -1 || i32::from(want) == state::BECAME_CURRENT {
            return self.states[t] != state::CLOSED && self.request_current(t, forced);
        }
        let s = self.states[t];
        if s == state::CLOSED || !(state::LOCKED..=state::CLOSED).contains(&want) {
            return want == s;
        }
        if self.sly3_rules {
            // Sly 3 hook (sly3/missions.md 4.1a): a checked request allows 0 to 1 (prerequisites held), 1 to 2, 1 to
            // 3 and 2 to 3; a request for 3 on a job's task asks for 2.
            let want =
                if !forced && want == state::CLOSED && self.graph.tasks[t].job.is_some() { state::DONE } else { want };
            let allowed = forced
                || match (s, want) {
                    (state::LOCKED, state::AVAILABLE) => self.prereqs_met(t),
                    (state::AVAILABLE, state::DONE | state::CLOSED) | (state::DONE, state::CLOSED) => true,
                    _ => false,
                };
            if !allowed {
                return want == s || self.refuse(t);
            }
            self.set_state(t, want);
            return true;
        }
        if want == state::AVAILABLE {
            return self.request_current(t, forced);
        }
        let allowed = forced
            || matches!(
                (s, want),
                (state::AVAILABLE, state::DONE) | (state::DONE, state::CLOSED) | (state::AVAILABLE, state::CLOSED)
            );
        if !allowed {
            return want == s || self.refuse(t);
        }
        self.set_state(t, want);
        true
    }

    fn refuse(&mut self, t: usize) -> bool {
        let task = &self.graph.tasks[t];
        if let Some(j) = task.job {
            let slot = self.graph.jobs[j].slot;
            self.effects.push(Effect::Script {
                slot,
                event: script_event::TASK_EVENT,
                task_event: Some(event::REFUSED),
            });
        }
        self.notices.push(Notice::Refused(task.id));
        false
    }

    fn save_state(&mut self, t: usize) {
        let keys = self.graph.tasks[t].keys.clone();
        put(&mut self.progress, &keys.state, SaveValue::Int(self.states[t] as i32));
    }

    /// The object switches and goals of task `t` for state or group `s` (missions.md 4).
    fn switch(&mut self, t: usize, s: i32, instant: bool) {
        let row = self.row;
        let task = &self.graph.tasks[t];
        if !task.active {
            return;
        }
        for sw in &task.switches {
            if sw.row >= 0 && sw.row != row {
                continue;
            }
            let groups = sw.states.iter().any(|&x| x >= state::BECAME_CURRENT);
            let states = sw.states.iter().any(|&x| x < state::BECAME_CURRENT);
            let on = sw.states.contains(&s);
            let applies = if s >= state::BECAME_CURRENT {
                // Group 4 shows what lists 4 and hides it at 5; group 5 shows what lists 5.
                on || (s == state::STOPPED_CURRENT && sw.states.contains(&state::BECAME_CURRENT))
            } else {
                states || !groups
            };
            if applies {
                self.effects.push(Effect::Present { object: sw.object, owner: sw.owner, on });
            }
        }
        for g in &task.goals {
            if (g.state == s || g.state == -1) && (g.row < 0 || g.row == row) {
                self.effects.push(Effect::Goal {
                    object: g.object,
                    machine: g.machine,
                    goal: g.goal,
                    instant: instant || g.instant,
                });
            }
        }
        if s < state::BECAME_CURRENT
            && let Some(m) = task.marker
        {
            self.effects.push(Effect::Present { object: m, owner: -1, on: s == state::AVAILABLE as i32 });
        }
    }

    /// A job's load-time switches (option 1255): each listed object is present when the job's state is in its
    /// list, the job's state being 0 not begun, 1 current, 2 complete [working choice].
    fn job_switches(&mut self, j: usize) {
        let row = self.row;
        let complete = get(&self.progress, &self.graph.jobs[j].keys.complete).is_some_and(|v| v.as_i32() != 0);
        let s = if complete {
            2
        } else if self.job == Some(j) {
            1
        } else {
            0
        };
        for sw in self.graph.jobs[j].switches.clone() {
            if sw.row < 0 || sw.row == row {
                self.effects.push(Effect::Present { object: sw.object, owner: sw.owner, on: sw.states.contains(&s) });
            }
        }
    }

    /// Changes task `t`'s state with its effects and propagation (missions.md 4).
    fn set_state(&mut self, t: usize, s: i8) {
        let old = self.states[t];
        if old == s {
            return;
        }
        let slot = self.graph.tasks[t].slot;
        let here = Some(slot);
        self.effects.push(Effect::Script { slot, event: script_event::STATE_BEFORE, task_event: None });
        self.effects.push(Effect::Broadcast { event: event::STATE_BEFORE, object: here });
        self.states[t] = s;
        self.switch(t, s as i32, self.restoring);
        self.save_state(t);
        self.effects.push(Effect::Script { slot, event: script_event::STATE, task_event: None });
        self.effects.push(Effect::Broadcast { event: event::STATE_AFTER, object: here });
        // Sly 3 (sly3/missions.md 4.1a, "Clearing the job and task keys"): in live play a task whose new state is 0
        // or 3 stops being current, which clears the remembered task (closing a job's last task leaves none to carry
        // into the next world). Sly 2's spec has no such rule.
        if self.sly3_rules && !self.restoring && (s == state::LOCKED || s == state::CLOSED) && self.current == Some(t) {
            self.stop_current();
        }
        if self.sly3_rules && !self.restoring {
            self.pending.push(t);
            self.backfill(t, s);
            self.pending.pop();
        }
        if s >= state::DONE {
            if s == state::DONE {
                let keys = self.graph.tasks[t].keys.clone();
                add(&mut self.progress, &keys.completed, 1);
                put(&mut self.progress, &keys.complete, SaveValue::Flag(true));
                // The help drops the first time a task of the current job is done.
                if self.job.is_some() && self.graph.tasks[t].job == self.job && self.done_in_job.insert(t) {
                    self.set_help(self.help + HELP_DONE);
                }
            }
            let job = self.graph.tasks[t].job;
            // Sly 2's back-fill (Sly 3's is `backfill`, above).
            let prereqs = if self.sly3_rules { Vec::new() } else { self.graph.tasks[t].prereq.clone() };
            for p in prereqs {
                if self.states[p] < s && self.states[p] != state::UNKNOWN {
                    self.set_state(p, s);
                } else if self.states[p] == state::DONE && self.graph.tasks[p].job != job {
                    self.set_state(p, state::CLOSED);
                }
            }
            for c in self.graph.tasks[t].succ.clone() {
                self.reevaluate(c);
            }
            if s == state::DONE {
                if let Some(j) = job
                    && self.graph.jobs[j].last == Some(t)
                {
                    self.complete_job(j);
                }
                let outside = job.is_none();
                let succ = &self.graph.tasks[t].succ;
                if outside && !succ.is_empty() && succ.iter().all(|&c| self.graph.tasks[c].job.is_some()) {
                    self.autosave();
                }
            }
        }
        self.update_percent();
    }

    /// Sly 3 hook: back-fill (sly3/missions.md 4.1a) for task `t`'s new state `s`, recursing. For 1 or 2, a
    /// prerequisite with no job goes to 3, one of the same job to 2, one of another job to 3 (2 when that job keeps
    /// its last task open, 1361). For 3, every prerequisite goes to 3. For 0 nothing. Only prerequisites below their
    /// target, and not already being changed, are raised.
    fn backfill(&mut self, t: usize, s: i8) {
        if !(state::AVAILABLE..=state::CLOSED).contains(&s) {
            return;
        }
        let job = self.graph.tasks[t].job;
        for p in self.graph.tasks[t].prereq.clone() {
            let target = if s == state::CLOSED {
                state::CLOSED
            } else {
                match self.graph.tasks[p].job {
                    None => state::CLOSED,
                    Some(pj) if Some(pj) == job => state::DONE,
                    Some(pj) if self.graph.jobs[pj].keep_open => state::DONE,
                    Some(_) => state::CLOSED,
                }
            };
            if self.states[p] != state::UNKNOWN && self.states[p] < target && !self.pending.contains(&p) {
                self.set_state(p, target);
            }
        }
    }

    /// Re-evaluates a successor: locked unless its prerequisites are done, available if they are.
    fn reevaluate(&mut self, c: usize) {
        let s = self.states[c];
        if s != state::LOCKED && s != state::AVAILABLE {
            return;
        }
        let want = if self.prereqs_met(c) { state::AVAILABLE } else { state::LOCKED };
        if want != s {
            self.set_state(c, want);
            // Not in restoring mode (Sly 3's mode 1: the state setter's follow-ups are skipped).
            if want == state::AVAILABLE && self.graph.tasks[c].auto_current && !self.restoring {
                self.make_current(c);
            }
        }
    }

    /// The current task, if any, stops being current (missions.md 4, "Becoming current": switch group 5, its script's
    /// end event, event 0x24, its objective prompt removed); none is current then.
    fn stop_current(&mut self) {
        if let Some(prev) = self.current.take() {
            self.switch(prev, state::STOPPED_CURRENT, false);
            let slot = self.graph.tasks[prev].slot;
            self.effects.push(Effect::Script { slot, event: script_event::END, task_event: None });
            self.effects.push(Effect::Broadcast { event: event::TASK_NOT_CURRENT, object: None });
            self.notices.push(Notice::Objective(None));
        }
    }

    /// Makes task `t` the current task (missions.md 4, "Becoming current").
    pub fn make_current(&mut self, t: usize) {
        if self.current == Some(t) {
            return;
        }
        self.stop_current();
        // missions.md 4.2: the new task is current, its checkpoint remembered and its job begun; then (its record
        // being loaded) its play count, switch group 4, goals and prompt, and event 19 to its script; only then its
        // exit (fired by `after_current`, after a 1405 "done").
        self.current = Some(t);
        let task = self.graph.tasks[t].clone();
        if task.checkpoint.is_some() {
            self.checkpoint = Some(t);
        }
        if let Some(j) = task.job
            && task.job != self.job
            && !self.sly3_rules
        {
            self.begin_job(j);
        }
        if self.sly3_rules {
            // Sly 3's order as before: switch group 4, the task's event, the broadcast, the counts and prompt.
            self.switch(t, state::BECAME_CURRENT, false);
            self.effects.push(Effect::Script { slot: task.slot, event: script_event::BEGIN, task_event: None });
            self.effects.push(Effect::Broadcast { event: event::TASK_CURRENT, object: None });
            add(&mut self.progress, &task.keys.focus, 1);
            self.notices.push(Notice::Objective(task.objective));
        } else {
            add(&mut self.progress, &task.keys.focus, 1);
            self.effects.push(Effect::Broadcast { event: event::TASK_CURRENT, object: None });
            self.switch(t, state::BECAME_CURRENT, false);
            self.notices.push(Notice::Objective(task.objective));
            self.effects.push(Effect::Script { slot: task.slot, event: script_event::BEGIN, task_event: None });
        }
        // Sly 3 hook: option 1405 asks for "done" at once, before the task's exit fires (sly3/missions.md 4). The
        // request is queued and finished below by the outermost call, so a chain of such tasks (each done opening the
        // next, which becomes current) runs in a loop instead of nesting calls.
        if task.done_on_current {
            self.done_queue.push_back(t);
        } else {
            self.after_current(t);
        }
        if self.finishing {
            return;
        }
        self.finishing = true;
        while let Some(d) = self.done_queue.pop_front() {
            self.completed_now = None;
            if self.states[d] < state::DONE {
                self.request(d, state::DONE, false);
            }
            self.after_current(d);
        }
        self.completed_now = None;
        self.finishing = false;
    }

    /// The last step of a task becoming current (and, with option 1405, of its "done"): its exit fires. Its job began
    /// before, so a "done" that completes the job cannot begin it again.
    fn after_current(&mut self, t: usize) {
        let (exit, job) = (self.graph.tasks[t].exit, self.graph.tasks[t].job);
        if let Some(e) = exit {
            self.effects.push(Effect::FireExit(e));
        }
        // Sly 3: the job begins last, unless this task's 1405 "done" has just completed it (sly3/missions.md 4).
        if self.sly3_rules
            && let Some(j) = job
            && job != self.job
            && self.completed_now != Some(j)
        {
            self.begin_job(j);
        }
    }

    // ------------------------------------------------------------------ jobs

    fn begin_job(&mut self, j: usize) {
        if let Some(old) = self.job {
            self.end_job(old);
        }
        self.job = Some(j);
        self.done_in_job.clear();
        let def = self.graph.jobs[j].clone();
        add(&mut self.progress, &def.keys.focus, 1);
        self.timer = get(&self.progress, &def.keys.time).map_or(0.0, SaveValue::as_f32);
        self.help = get(&self.progress, &def.keys.help).map_or(self.history_mean(), SaveValue::as_f32);
        self.effects.push(Effect::Broadcast { event: event::DROP_LOOT, object: None });
        self.effects.push(Effect::Script { slot: def.slot, event: script_event::BEGIN, task_event: None });
        self.effects.push(Effect::Broadcast { event: event::JOB_BEGAN, object: None });
    }

    /// Ends job `j` (missions.md 5, "End"): its timer and help are saved, the help history shifts, and the respawn
    /// point in a hub goes back to 393.
    pub fn end_job(&mut self, j: usize) {
        if self.job != Some(j) {
            return;
        }
        let def = self.graph.jobs[j].clone();
        put(&mut self.progress, &def.keys.time, SaveValue::Float(self.timer));
        put(&mut self.progress, &def.keys.help, SaveValue::Float(self.help));
        let h = [0, 1].map(|i| self.progress.float(&["suck_history", ["mission_0", "mission_1"][i]]));
        self.progress.set_named(&["suck_history", "mission_2"], None, SaveValue::Float(h[1]));
        self.progress.set_named(&["suck_history", "mission_1"], None, SaveValue::Float(h[0]));
        self.progress.set_named(&["suck_history", "mission_0"], None, SaveValue::Float(self.help));
        self.job = None;
        // Sly 3 (sly3/conditions.md 3.1): the remembered checkpoint task is forgotten when its job ends.
        if self.sly3_rules && self.checkpoint.is_some_and(|t| self.graph.tasks[t].job == Some(j)) {
            self.checkpoint = None;
        }
        self.timer = 0.0;
        self.help = self.history_mean();
        self.effects.push(Effect::Script { slot: def.slot, event: script_event::END, task_event: None });
        self.effects.push(Effect::Broadcast { event: event::JOB_ENDED, object: None });
        self.respawn = flow::DEFAULT_POINT;
    }

    /// Completes job `j`: the screen is reported, the job closes, the next task is requested or the job's exit
    /// fires (missions.md 5, "Complete").
    fn complete_job(&mut self, j: usize) {
        self.completed_now = Some(j);
        let def = self.graph.jobs[j].clone();
        self.notices.push(Notice::JobComplete { job: def.id, screen: def.screen });
        // Sly 3 hook (sly3/missions.md 4.1): completion mode 2 sends the first task back to 0 instead.
        if def.replay {
            if let Some(first) = def.first {
                self.set_state(first, state::LOCKED);
            }
        } else if let Some(last) = def.last
            && !def.keep_open
        {
            self.set_state(last, state::CLOSED);
        }
        // Sly 3 hook: the item granted, and the following job's resume task as the forced checkpoint.
        if let Some(item) = def.item {
            self.progress.set_named(&["powerUps", "mppukfAcquired"], Some(item as usize), SaveValue::Flag(true));
        }
        if let Some(last) = def.last {
            let resume = self.graph.tasks[last]
                .succ
                .iter()
                .filter_map(|&c| self.graph.tasks[c].job)
                .find_map(|k| self.graph.jobs[k].resume);
            if let Some(r) = resume {
                let t = &self.graph.tasks[r];
                self.progress.set_named(&["oidCheckpointForce"], None, SaveValue::Int(t.id));
                self.progress.set_named(&["widCheckpointForce"], None, SaveValue::Int(t.row.map_or(-1, |w| w as i32)));
            }
        }
        put(&mut self.progress, &def.keys.complete, SaveValue::Flag(true));
        self.effects.push(Effect::Broadcast { event: event::JOB_CLOSED, object: None });
        self.end_job(j);
        if !def.keep_open {
            self.autosave();
        }
        let next = def.last.and_then(|l| {
            self.graph.tasks[l].succ.iter().copied().find(|&c| self.states[c] != state::CLOSED && self.prereqs_met(c))
        });
        match next {
            Some(c) => {
                self.request_current(c, false);
            }
            None => {
                if let Some(e) = def.exit {
                    self.effects.push(Effect::FireExit(e));
                }
            }
        }
    }

    /// Restarts the current job at its checkpoint task (missions.md 5, "Failure and restart"): that task becomes
    /// available and current, and its world loads at its checkpoint.
    pub fn restart_at_checkpoint(&mut self) {
        let Some(j) = self.job else { return };
        let Some(t) = self.checkpoint.filter(|&t| self.graph.tasks[t].job == Some(j)).or(self.graph.jobs[j].first)
        else {
            return;
        };
        if self.states[t] != state::CLOSED {
            self.states[t] = state::LOCKED;
            self.set_state(t, state::AVAILABLE);
        }
        let task = self.graph.tasks[t].clone();
        self.current = None;
        self.make_current(t);
        self.load = Some(LoadRequest {
            row: task.row.unwrap_or(self.row),
            point: task.checkpoint.unwrap_or(flow::DEFAULT_POINT),
            wipe: Some(778),
            movies: Vec::new(),
            task: None,
        });
    }

    /// Sly 3 hook: runs `f` in restoring mode (sly3/missions.md 4.1a, mode 1): no back-fill and no follow-ups of the
    /// state setter, events as usual. World-start step 3 runs the requests queued during the restore this way.
    pub fn in_restoring_mode(&mut self, f: impl FnOnce(&mut Jobs)) {
        let was = core::mem::replace(&mut self.restoring, true);
        f(self);
        self.restoring = was;
    }

    /// Forces task `t` to state `s` (task methods 1290 to 1293; Sly 3's 1314 to 1317): the request skips the checks.
    pub fn force_state(&mut self, t: usize, s: i8) {
        // An index from another world's graph names nothing here (a stale index after a load).
        if self.states.get(t).is_some_and(|&x| x != state::CLOSED) {
            self.set_state(t, s);
        }
    }

    /// A failure counted by the job (job method 1238): the help rises by 0.1; the job goes on.
    pub fn count_failure(&mut self) {
        self.set_help(self.help + HELP_DEFEAT);
    }

    /// Resets the play counts of job `j`'s tasks (job method 1250).
    pub fn reset_play_counts(&mut self, j: usize) {
        for t in self.graph.tasks_of(j).collect::<Vec<_>>() {
            let keys = self.graph.tasks[t].keys.clone();
            put(&mut self.progress, &keys.focus, SaveValue::Int(0));
        }
    }

    /// Restarts the whole current job: its tasks' play counts reset and the first task is available again.
    pub fn restart_job(&mut self) {
        let Some(j) = self.job else { return };
        self.reset_play_counts(j);
        if let Some(first) = self.graph.jobs[j].first {
            self.states[first] = state::LOCKED;
            self.set_state(first, state::AVAILABLE);
        }
        self.effects.push(Effect::Broadcast { event: event::JOB_RESTARTED, object: None });
    }

    // ------------------------------------------------------------------ help, percent, autosave

    fn history_mean(&self) -> f32 {
        let h: f32 =
            ["mission_0", "mission_1", "mission_2"].iter().map(|m| self.progress.float(&["suck_history", m])).sum();
        h / 3.0
    }

    fn set_help(&mut self, v: f32) {
        self.help = v.clamp(0.0, 1.0);
    }

    /// The share of the episode's tasks with successors that are closed (missions.md 4, "Episode percentage").
    pub fn percent(&self) -> i32 {
        let with: Vec<usize> = (0..self.graph.tasks.len()).filter(|&t| !self.graph.tasks[t].succ.is_empty()).collect();
        if with.is_empty() {
            return 0;
        }
        let closed = with.iter().filter(|&&t| self.states[t] == state::CLOSED).count();
        match closed {
            0 => 0,
            c if c == with.len() => 100,
            c => ((c * 100 / with.len()) as i32).clamp(1, 99),
        }
    }

    fn update_percent(&mut self) {
        if let Some(p) = self.episode_key("percent_complete") {
            let v = self.percent();
            self.progress.set(&p, None, SaveValue::Int(v));
        }
    }

    fn autosave(&mut self) {
        if !self.restoring && self.episode != 0 {
            self.notices.push(Notice::Autosave);
        }
    }

    // ------------------------------------------------------------------ events

    /// Handles a task event (missions.md 5, 6): broadcasts it and applies the job system's own reactions.
    pub fn task_event(&mut self, e: u32, object: Option<Handle>) {
        self.effects.push(Effect::Broadcast { event: e, object: object.map(Handle::index) });
        match e {
            event::DEFEAT => self.set_help(self.help + HELP_DEFEAT),
            event::JOB_RESTART => {
                self.set_help(self.help + HELP_DEFEAT);
                self.restart_job();
            }
            _ => {}
        }
        if let Some(j) = self.job {
            let def = &self.graph.jobs[j];
            if def.close_events.contains(&(e as i32)) {
                self.complete_job(j);
            } else if def.end_events.contains(&(e as i32)) {
                self.end_job(j);
            }
        }
    }
}

// ---------------------------------------------------------------------- attaching to a world and running

/// The progress tree of a Sly 2 world (its save-variable section), shared.
pub fn tree_of(world: &World) -> Option<alloc::sync::Arc<save::sly2::VarTree>> {
    let l = world.sly2.as_deref()?;
    let at = l.header.sections.save_variables;
    let (t, _) = save::sly2::VarTree::parse(world.level.bytes.get(at..)?)?;
    Some(alloc::sync::Arc::new(t))
}

/// Attaches the job system to a freshly loaded Sly 2 world with `progress` (a new game's zeroed block, a loaded
/// slot, or the previous world's), restoring the state; `carry` is what the previous world carried over (none after
/// a new game or a load), `queued` a task to request, forced (a new game's first task). Returns false for a world
/// that is not a Sly 2 world.
pub fn attach(world: &mut World, progress: Progress, carry: Option<Carry>, queued: Option<i32>) -> bool {
    if world.sly2.is_none() {
        return false;
    }
    let mut j = Jobs::new(world, progress);
    j.restore(queued, carry);
    j.apply(world);
    j.sync(world);
    world.sly2.as_deref_mut().expect("Sly 2 world").jobs = Some(Box::new(j));
    true
}

/// Takes the job system's progress out of the world (before loading another one), with the job timer and help
/// written into it, and what the next world carries over.
pub fn detach(world: &mut World) -> Option<(Progress, Carry)> {
    let mut j = world.sly2.as_deref_mut()?.jobs.take()?;
    j.store_current();
    let c = j.carry();
    Some((j.progress, c))
}

/// Runs `f` on the world's job system with the world.
pub fn with<R>(world: &mut World, f: impl FnOnce(&mut Jobs, &mut World) -> R) -> Option<R> {
    let mut j = world.sly2.as_deref_mut()?.jobs.take()?;
    let r = f(&mut j, world);
    j.apply(world);
    j.sync(world);
    if let Some(l) = world.sly2.as_deref_mut() {
        l.jobs = Some(j);
    }
    Some(r)
}

/// Arrives at an arrival point after a load (level-flow.md 6, 7): finds the point (393 by default), places the
/// player there (when the game has one), applies the point's lists, resets the exits, raises script event 5 on the
/// point and broadcasts task event 26, then task event 5. Returns the point's transform.
pub fn arrive(game: &mut crate::game::Game, point: i32) -> Option<Affine> {
    let world = &mut game.world;
    let found = flow::find_point(world, point).or_else(|| flow::find_point(world, flow::DEFAULT_POINT));
    let transform = found.and_then(|h| flow::transform_of(world, h));
    if let Some(t) = transform {
        game.systems.player.place(&mut game.world, t);
    }
    let world = &mut game.world;
    with(world, |j, w| {
        j.respawn = point;
        for e in &mut j.exits {
            e.state = ExitState::Inside;
            e.wanted = false;
        }
        if let Some(h) = found {
            let acts = flow::point_actions(w, h);
            for (task, s) in acts.tasks {
                if let Some(&t) = j.graph.task_ids.get(&task) {
                    j.request(t, s as i8, false);
                }
            }
            for (object, machine, goal, instant) in acts.goals {
                j.machine_goals.push((object, machine, goal, instant));
            }
            for (object, target) in acts.place {
                let o = w.find_sly2_id(object, None);
                let t = w.find_sly2_id(target, None).and_then(|t| w.world_transform(t));
                if let (Some(o), Some(t)) = (o, t) {
                    w.set_position(o, t.t);
                }
            }
            w.raise(h, 5, Vec::new());
        }
        j.task_event(event::ARRIVED, found);
        j.task_event(event::WORLD_STARTED, None);
    });
    game.flush_events();
    transform
}

impl Jobs {
    /// Applies the queued effects to the world.
    pub fn apply(&mut self, world: &mut World) {
        let effects = core::mem::take(&mut self.effects);
        for e in effects {
            match e {
                Effect::Present { object, owner, on } => {
                    // Sly 3 hook: Sly 3 object IDs.
                    let find = |w: &World, id: i32, ctx: Option<Handle>| match w.sly3 {
                        Some(_) => w.find_sly3_id(id, ctx),
                        None => w.find_sly2_id(id, ctx),
                    };
                    let ctx = (owner >= 0).then(|| find(world, owner, None)).flatten();
                    if let Some(h) = find(world, object, ctx) {
                        world.set_in_world(h, on);
                    }
                }
                Effect::Goal { object, machine, goal, instant } => {
                    self.machine_goals.push((object, machine, goal, instant))
                }
                Effect::Script { slot, event, task_event } => {
                    if let Some(h) = world.handle(slot)
                        && world.in_world(h)
                    {
                        let args = match task_event {
                            Some(e) => vec![script::Value::Int(e as i32), script::Value::Object(None)],
                            None => Vec::new(),
                        };
                        if let Some(event) = (self.script_events)(event) {
                            world.raise(h, event, args);
                        }
                    }
                }
                Effect::Broadcast { event, object } => {
                    // To the current job's available tasks, then the current job; with no current job, to every
                    // available task with no job (missions.md 3).
                    let job = self.job;
                    let obj = script::Value::Object(object.and_then(|s| world.handle(s)).map(Handle::to_script));
                    let mut slots: Vec<u32> = self
                        .graph
                        .tasks
                        .iter()
                        .enumerate()
                        .filter(|(i, t)| t.job == job && self.states[*i] == state::AVAILABLE)
                        .map(|(_, t)| t.slot)
                        .collect();
                    if let Some(c) = self.listener_class {
                        slots.extend(world.slots_of_class(c).iter().copied());
                    }
                    if let Some(j) = job {
                        slots.push(self.graph.jobs[j].slot);
                    }
                    let Some(task_event) = (self.script_events)(script_event::TASK_EVENT) else { continue };
                    for s in slots {
                        if let Some(h) = world.handle(s)
                            && world.in_world(h)
                        {
                            let code = (self.task_codes)(event);
                            world.raise(h, task_event, vec![script::Value::Int(code as i32), obj.clone()]);
                        }
                    }
                }
                Effect::FireExit(id) => self.want_exit(world, id),
            }
        }
    }

    /// Mirrors the job system into `Sly2Level` (operations 100 to 103).
    pub fn sync(&self, world: &mut World) {
        let slot = |t: Option<usize>| t.map(|t| self.graph.tasks[t].slot);
        let job = self.job.map(|j| self.graph.jobs[j].slot);
        let (cur, cp) = (slot(self.current), slot(self.checkpoint));
        if let Some(l) = world.sly2.as_deref_mut() {
            l.job = [job, cur, cp];
            l.difficulty_help = self.help;
        }
    }

    /// Fires the exit with object ID `id` (task option 1278, job option 1259, scripts' option 786).
    ///
    /// Only an armed exit fires (level-flow.md 5.5): firing an exit in any other state does nothing. So a script's
    /// 786 on an exit the world has just started in (state 0) is ignored; in row 14 that kept a script from loading
    /// row 15 through the door 2379 at world start.
    pub fn fire_exit_id(&mut self, world: &mut World, id: i32) {
        match self.exits.iter().position(|e| e.id == id) {
            Some(i) if self.exits[i].state == ExitState::Armed => self.fire_exit(world, i),
            Some(_) => {}
            None => self.fired_elsewhere.push(id),
        }
    }

    /// A task's or job's exit (options 1278, 1259): fired now if armed, else as soon as it arms (level-flow.md 5.5).
    pub fn want_exit(&mut self, world: &mut World, id: i32) {
        match self.exits.iter().position(|e| e.id == id) {
            Some(i) if self.exits[i].state == ExitState::Armed => self.fire_exit(world, i),
            Some(i) => self.exits[i].wanted = true,
            None => self.fired_elsewhere.push(id),
        }
    }

    /// Fires exit `i` (level-flow.md 5.5): its task states (788) apply and it is fired (state 4, event 5), but the
    /// load is not requested here: it waits for the option-785 delay, in a later update ([`Jobs::step_exit`]). A
    /// firing while a load is pending or requested is dropped.
    fn fire_exit(&mut self, world: &mut World, i: usize) {
        if self.load.is_some() || self.exit_pending.is_some() {
            return;
        }
        self.exits[i].state = ExitState::Fired;
        let exit = self.exits[i].clone();
        world.raise(exit.handle, 5, Vec::new());
        // Object-model hook: event 5 goes through the common script-event routine, which broadcasts task event 26
        // with the door (missions.md 4.3; the m1 jobs' door check).
        self.task_event(event::ARRIVED, Some(exit.handle));
        if let Some((req, pairs)) = flow::fire(world, &exit) {
            for (task, s) in pairs {
                if let Some(&t) = self.graph.task_ids.get(&task) {
                    self.request(t, s as i8, true);
                }
            }
            let delay = world.options(exit.handle).and_then(|o| o.f32(flow::opt::LOAD_DELAY)).unwrap_or(0.0);
            self.exit_pending = Some((req, delay.max(0.0)));
            self.exit_fresh = true;
        }
    }

    /// The pending exit's clock (real seconds `dt`; the game is paused while an exit is pending): the load is
    /// requested ([`Jobs::load`]) once the delay has passed, never in the update that fired the exit. Returns whether
    /// an exit is still pending.
    pub fn step_exit(&mut self, dt: f32) -> bool {
        let Some((_, left)) = self.exit_pending.as_mut() else { return false };
        if core::mem::replace(&mut self.exit_fresh, false) {
            return true;
        }
        *left -= dt;
        if *left <= 0.0 {
            self.load = self.exit_pending.take().map(|p| p.0);
        }
        self.exit_pending.is_some()
    }

    /// The exits' state machine for the player at `p` (level-flow.md 5.2).
    fn step_exits(&mut self, world: &mut World, p: [f32; 3], dt: f32) {
        for i in 0..self.exits.len() {
            let h = self.exits[i].handle;
            if !world.is_active(h) {
                continue;
            }
            // An exit whose volume is not read is never entered, so it arms on its own updates (level-flow.md 5.5)
            // and fires only when a task, job or script fires it. Object-model hook: the hub's job doors are such
            // exits, fired by their sequences' scripts (1735, from 123860's module).
            let inside = match crate::behavior::sly2_props::volume_box(world, h) {
                Some(b) => crate::behavior::sly2_props::in_volume(&b, p),
                None => false,
            };
            let arm = world.options(h).and_then(|o| o.f32(flow::opt::ARM_TIME)).unwrap_or(0.0);
            self.exits[i].state = match (self.exits[i].state, inside) {
                (ExitState::Inside, false) => ExitState::Outside(0.0),
                (ExitState::Outside(_), true) => ExitState::Inside,
                (ExitState::Outside(t), false) if t + dt >= arm && self.exits[i].wanted => {
                    self.fire_exit(world, i);
                    ExitState::Fired
                }
                (ExitState::Outside(t), false) if t + dt >= arm => ExitState::Armed,
                (ExitState::Armed, false) if self.exits[i].wanted => {
                    self.fire_exit(world, i);
                    ExitState::Fired
                }
                (ExitState::Outside(t), false) => ExitState::Outside(t + dt),
                (ExitState::Armed, true) => {
                    self.fire_exit(world, i);
                    ExitState::Fired
                }
                (s, _) => s,
            };
        }
    }

    /// Moves the player to warp point `h` (option 778): the point's script gets event 5 and task event 26 is
    /// broadcast.
    pub fn warp(&mut self, world: &mut World, h: Handle) {
        self.place = world.world_transform(h);
        world.raise(h, 5, Vec::new());
        self.task_event(event::ARRIVED, Some(h));
    }
}

/// One frame of the job system (from `Game::frame`): play time, the job timer, the task events and requests of the
/// frame, the exits, and the effects. `touched` is whether the controller was used this frame.
pub fn step(game: &mut crate::game::Game, dt: f32, touched: bool) {
    let player = game.world.player.and_then(|p| game.world.position(p));
    let place = with(&mut game.world, |j, w| {
        // Play time.
        let total = j.progress.float(&["dt"]) + dt;
        j.progress.set_named(&["dt"], None, SaveValue::Float(total));
        if let Some(p) = j.episode_key("dt") {
            let v = j.progress.get(&p, None).map_or(0.0, SaveValue::as_f32);
            j.progress.set(&p, None, SaveValue::Float(v + dt));
        }
        // The job timer runs while the controller was touched in the last 10 s.
        j.idle = if touched { 0.0 } else { j.idle + dt };
        if j.job.is_some() && j.idle < IDLE_LIMIT {
            j.timer += dt;
        }
        // Requests and events of the frame.
        for (task, s) in core::mem::take(&mut w.shared.task_requests) {
            if let Some(&t) = j.graph.task_ids.get(&task) {
                j.request(t, s as i8, false);
            }
        }
        let events = w.sly2.as_deref_mut().map(|l| core::mem::take(&mut l.task_events)).unwrap_or_default();
        for (e, object) in events {
            j.task_event(e, object);
        }
        if let Some(p) = player {
            j.step_exits(w, p, dt);
        }
        j.place.take()
    });
    if let Some(Some(t)) = place {
        game.systems.player.place(&mut game.world, t);
    }
}
