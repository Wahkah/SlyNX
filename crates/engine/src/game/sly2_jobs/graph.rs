//! The job and task records of a Sly 2 world (missions.md 2 and 3) read into a graph.
//!
//! Every world of an episode carries the whole episode's jobs (class 132) and tasks (class 133). Their options are
//! in the world's option store (single values) and in `Sly2Level::actions` (the typed argument lists: object
//! switches 1287, state-machine goals 1288, the "job complete" screen 1258, the ending events 1236 and 1237); the
//! prerequisite and successor lists and the internal names follow the option list in the body and are read here.
//! The save keys (1260 to 1263, 1295 to 1299) are paths of save-variable name IDs; each key's meaning is taken from
//! the last name of its path (`focus_count`, `is_complete`, ...), so it does not depend on which option holds it.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::object::{Arg, Handle, OptionValue, flags};
use crate::world::World;

/// Job and task option IDs (missions.md 2 and 3; stream IDs).
pub mod opt {
    /// Events that end the job (typed list).
    pub const END_EVENTS: u16 = 1236;
    /// Events that close the job (typed list).
    pub const CLOSE_EVENTS: u16 = 1237;
    /// The job neither closes its last task nor autosaves on completion.
    pub const KEEP_OPEN: u16 = 1242;
    /// First task.
    pub const FIRST: u16 = 1243;
    /// Last task.
    pub const LAST: u16 = 1244;
    /// Display name, description (string IDs).
    pub const TITLE: u16 = 1245;
    /// Description string.
    pub const TEXT: u16 = 1246;
    /// The "job complete" screen and its two parts.
    pub const SCREEN: u16 = 1258;
    /// Level exit fired when the job ends with no next task.
    pub const JOB_EXIT: u16 = 1259;
    /// The task's job.
    pub const JOB: u16 = 1264;
    /// Objective string.
    pub const OBJECTIVE: u16 = 1265;
    /// Counts as done for its successors while available.
    pub const COUNTS_DONE: u16 = 1271;
    /// Becomes current as soon as it is available.
    pub const AUTO_CURRENT: u16 = 1272;
    /// Current again after a load when restored as available.
    pub const RESUME: u16 = 1273;
    /// A job's load-time switch list (repeatable, as 1287).
    pub const JOB_SWITCH: u16 = 1255;
    /// Checkpoint (an arrival point).
    pub const CHECKPOINT: u16 = 1274;
    /// The world row the task starts in.
    pub const ROW: u16 = 1275;
    /// The character the task starts with.
    pub const CHARACTER: u16 = 1276;
    /// Level exit fired when the task becomes current.
    pub const TASK_EXIT: u16 = 1278;
    /// An object present only while the task is available.
    pub const MARKER: u16 = 1279;
    /// Object switch (repeatable).
    pub const SWITCH: u16 = 1287;
    /// State-machine goal (repeatable).
    pub const GOAL: u16 = 1288;
}

/// Sly 3 hook: the classes and option IDs the graph is read with. [`Ids::SLY2`] is Sly 2's (the [`opt`]
/// constants); the Sly 3 session passes its own (`game::sly3::jobs`). An ID of 0 is an option the game does not
/// have (no record carries option 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ids {
    /// The job and task classes.
    pub job_class: i16,
    pub task_class: i16,
    /// Job options.
    pub end_events: u16,
    pub close_events: u16,
    pub keep_open: u16,
    pub first: u16,
    pub last: u16,
    pub title: u16,
    pub text: u16,
    pub screen: u16,
    pub job_exit: u16,
    pub job_switch: u16,
    pub job_keys: [u16; 4],
    /// Task options.
    pub job: u16,
    pub objective: u16,
    pub counts_done: u16,
    pub auto_current: u16,
    pub resume: u16,
    pub checkpoint: u16,
    pub row: u16,
    pub character: u16,
    pub task_exit: u16,
    pub marker: u16,
    pub switch: u16,
    pub goal: u16,
    pub task_keys: [u16; 5],
    /// A command script started with the task (Sly 3; 0 in Sly 2).
    pub task_script: u16,
    /// The task is done as soon as it becomes current (Sly 3's 1405; 0 in Sly 2).
    pub done_when_current: u16,
    /// Sly 3 job options read at completion (missions.md 4.1; 0 in Sly 2): the completion mode (2: the first task
    /// goes back to 0), the item granted, the resume task, the task-event list.
    pub completion_mode: u16,
    pub item: u16,
    pub resume_task: u16,
    pub event_list: u16,
    /// Sly 3: the job ignores the player's defeat (1362; 0 in Sly 2).
    pub ignore_defeat: u16,
    /// Typed values after a switch's (owner, row) pair that are not part of it (Sly 3 adds one flag).
    pub switch_trailing: usize,
}

impl Ids {
    /// Sly 2's.
    pub const SLY2: Ids = Ids {
        job_class: 132,
        task_class: 133,
        end_events: opt::END_EVENTS,
        close_events: opt::CLOSE_EVENTS,
        keep_open: opt::KEEP_OPEN,
        first: opt::FIRST,
        last: opt::LAST,
        title: opt::TITLE,
        text: opt::TEXT,
        screen: opt::SCREEN,
        job_exit: opt::JOB_EXIT,
        job_switch: opt::JOB_SWITCH,
        job_keys: [1260, 1261, 1262, 1263],
        job: opt::JOB,
        objective: opt::OBJECTIVE,
        counts_done: opt::COUNTS_DONE,
        auto_current: opt::AUTO_CURRENT,
        resume: opt::RESUME,
        checkpoint: opt::CHECKPOINT,
        row: opt::ROW,
        character: opt::CHARACTER,
        task_exit: opt::TASK_EXIT,
        marker: opt::MARKER,
        switch: opt::SWITCH,
        goal: opt::GOAL,
        task_keys: [1295, 1296, 1297, 1298, 1299],
        task_script: 0,
        done_when_current: 0,
        completion_mode: 0,
        item: 0,
        resume_task: 0,
        event_list: 0,
        ignore_defeat: 0,
        switch_trailing: 0,
    };
}

/// Where a job's or task's values are saved: paths of save name IDs, by the last name of the path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    /// `focus_count`: times it became current (play count).
    pub focus: Option<Vec<u16>>,
    /// `complete_count`: times done (tasks).
    pub completed: Option<Vec<u16>>,
    /// `is_complete`.
    pub complete: Option<Vec<u16>>,
    /// `state` (tasks).
    pub state: Option<Vec<u16>>,
    /// `from_memcard`, the "restarted" flag (tasks).
    pub restarted: Option<Vec<u16>>,
    /// `time_played`: the job timer.
    pub time: Option<Vec<u16>>,
    /// `suck_value`: the job's difficulty help.
    pub help: Option<Vec<u16>>,
}

/// An object switch (option 1287).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    /// The object.
    pub object: i32,
    /// The task states (and groups 4 and 5) in which it is present.
    pub states: Vec<i32>,
    /// The object the search starts from (−1 none).
    pub owner: i32,
    /// The level row it is limited to (−1 any).
    pub row: i16,
}

/// A state-machine goal (option 1288).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Goal {
    /// The task state (−1: any).
    pub state: i32,
    /// The object.
    pub object: i32,
    /// The state machine.
    pub machine: i32,
    /// The goal state.
    pub goal: i32,
    /// Only set instantly.
    pub instant: bool,
    /// The level row (−1 any).
    pub row: i16,
}

/// A job record.
#[derive(Clone, Debug, PartialEq)]
pub struct JobDef {
    /// Slot of the object in this world.
    pub slot: u32,
    /// Object ID.
    pub id: i32,
    /// Internal name (`m1_sat`).
    pub name: String,
    /// First and last task (indexes into [`Graph::tasks`]).
    pub first: Option<usize>,
    pub last: Option<usize>,
    /// Display name and description string IDs.
    pub title: i16,
    pub text: i16,
    /// The "job complete" screen object and its two parts.
    pub screen: Option<[i32; 3]>,
    /// Exit fired when the job ends with no next task.
    pub exit: Option<i32>,
    /// Option 1242.
    pub keep_open: bool,
    /// Task events that end or close the job.
    pub end_events: Vec<i32>,
    pub close_events: Vec<i32>,
    /// Load-time switches (option 1255).
    pub switches: Vec<Switch>,
    /// Sly 3 hook: on completion the first task goes back to 0 (1367 = 2) so the job can be replayed.
    pub replay: bool,
    /// Sly 3 hook: the item-table record granted on completion (1385).
    pub item: Option<i32>,
    /// Sly 3 hook: the resume task (1366), an index.
    pub resume: Option<usize>,
    /// Sly 3 hook: the task-event numbers of option 1368 (at most 8; their use is an interpretation).
    pub events: Vec<i32>,
    /// Sly 3 hook: the player's defeat does not restart the job (1362).
    pub ignore_defeat: bool,
    /// Save keys.
    pub keys: Keys,
}

/// A task record.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskDef {
    /// Slot of the object in this world.
    pub slot: u32,
    /// Object ID.
    pub id: i32,
    /// Internal name (`t1_sat_intro`).
    pub name: String,
    /// Its loaded for this world's row (not an inactive record).
    pub active: bool,
    /// Its job.
    pub job: Option<usize>,
    /// Objective string.
    pub objective: Option<i16>,
    /// Checkpoint arrival point, start row and character.
    pub checkpoint: Option<i32>,
    pub row: Option<i16>,
    pub character: Option<i32>,
    /// Object present only while available.
    pub marker: Option<i32>,
    /// Exit fired when the task becomes current.
    pub exit: Option<i32>,
    /// Option 1271.
    pub counts_done: bool,
    /// Option 1272.
    pub auto_current: bool,
    /// Option 1273: current again after a load when restored as available.
    pub resume: bool,
    /// Sly 3 hook: the command script started with the task (option 1408 in Sly 3).
    pub script: Option<i32>,
    /// Sly 3 hook: done as soon as it becomes current (option 1405 in Sly 3).
    pub done_on_current: bool,
    /// Prerequisites and successors (indexes).
    pub prereq: Vec<usize>,
    pub succ: Vec<usize>,
    /// Object switches and state-machine goals.
    pub switches: Vec<Switch>,
    pub goals: Vec<Goal>,
    /// Save keys.
    pub keys: Keys,
}

/// An episode's jobs and tasks as one graph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub jobs: Vec<JobDef>,
    pub tasks: Vec<TaskDef>,
    /// Object ID → index.
    pub job_ids: BTreeMap<i32, usize>,
    pub task_ids: BTreeMap<i32, usize>,
}

fn arg_i32(a: &Arg) -> Option<i32> {
    match *a {
        Arg::Int(v) => Some(v),
        Arg::Short(_, v) => Some(v as i32),
        Arg::Bool(b) => Some(b as i32),
        Arg::Float(f) => Some(f as i32),
        _ => None,
    }
}

/// The object switches of option `id` in a slot's typed lists: (object, states..., owner, row), then `trailing`
/// values that are not part of it.
fn switches_of(lists: &[(u16, &[Arg])], id: u16, trailing: usize) -> Vec<Switch> {
    lists
        .iter()
        .filter(|(i, _)| *i == id)
        .filter_map(|(_, a)| {
            let mut v: Vec<i32> = a.iter().filter_map(arg_i32).collect();
            v.truncate(v.len().saturating_sub(trailing));
            (v.len() >= 3).then(|| Switch {
                object: v[0],
                states: v[1..v.len() - 2].to_vec(),
                owner: v[v.len() - 2],
                row: v[v.len() - 1] as i16,
            })
        })
        .collect()
}

/// The values of typed option `id` in a slot's lists.
fn args_of_slot(lists: &[(u16, &[Arg])], id: u16) -> Vec<i32> {
    lists.iter().filter(|(i, _)| *i == id).flat_map(|(_, a)| a.iter().filter_map(arg_i32)).collect()
}

fn path(v: Option<&OptionValue>) -> Option<Vec<u16>> {
    match v? {
        OptionValue::Args(a) => Some(a.iter().filter_map(arg_i32).map(|x| x as u16).collect()),
        _ => None,
    }
}

/// The save keys of an object, by the last name of each path.
fn keys(world: &World, h: Handle, ids: &[u16], names: &dyn Fn(u16) -> Option<String>) -> Keys {
    let mut k = Keys::default();
    let Some(o) = world.options(h) else { return k };
    for &id in ids {
        let Some(p) = path(o.get(id)) else { continue };
        let Some(last) = p.last().and_then(|&n| names(n)) else { continue };
        let slot = match last.as_str() {
            "focus_count" => &mut k.focus,
            "complete_count" => &mut k.completed,
            "is_complete" => &mut k.complete,
            "state" => &mut k.state,
            "from_memcard" => &mut k.restarted,
            "time_played" => &mut k.time,
            "suck_value" => &mut k.help,
            _ => continue,
        };
        *slot = Some(p);
    }
    k
}

/// Reads the bytes after a job's or task's option list: for tasks two `s32` lists, then the name.
fn tail(world: &World, h: Handle, lists: bool) -> (Vec<i32>, Vec<i32>, String) {
    let mut lists_out = [Vec::new(), Vec::new()];
    let mut name = String::new();
    // Sly 3 hook: a Sly 3 record's option list.
    let end = match world.sly3 {
        Some(_) => world.sly3_record(h).and_then(|r| r.options()).map(|o| o.end),
        None => world.sly2_record(h).and_then(|r| r.options()).map(|o| o.end),
    };
    let Some(end) = end else {
        return (Vec::new(), Vec::new(), name);
    };
    let mut r = formats::reader::Reader::at(&world.level.bytes, end);
    let mut ok = true;
    if lists {
        'lists: for list in &mut lists_out {
            let Ok(n) = r.u8() else {
                ok = false;
                break;
            };
            for _ in 0..n {
                let Ok(v) = r.i32() else {
                    ok = false;
                    break 'lists;
                };
                list.push(v);
            }
        }
    }
    if ok
        && let Ok(n) = r.u16()
        && let Ok(b) = r.bytes(n as usize)
    {
        name = b.iter().map(|&c| c as char).collect();
    }
    let [a, b] = lists_out;
    (a, b, name)
}

impl Graph {
    /// Reads the jobs and tasks of `world` (a Sly 2 world). `names` gives a save name ID's name (for the keys).
    pub fn read(world: &World, names: &dyn Fn(u16) -> Option<String>) -> Graph {
        Graph::read_with(world, names, &Ids::SLY2)
    }

    /// Sly 3 hook: reads with `ids` (a Sly 2 or a Sly 3 world).
    pub fn read_with(world: &World, names: &dyn Fn(u16) -> Option<String>, ids: &Ids) -> Graph {
        let mut g = Graph::default();
        let actions = match (world.sly2.as_deref(), world.sly3.as_deref()) {
            (Some(l), _) => &l.actions,
            (None, Some(l)) => &l.actions,
            (None, None) => return g,
        };
        let object_id = |h: Handle| if world.sly3.is_some() { world.sly3_id(h) } else { world.sly2_id(h) };
        // The typed lists, by slot.
        let mut lists: BTreeMap<u32, Vec<(u16, &[Arg])>> = BTreeMap::new();
        for (slot, id, args) in actions {
            lists.entry(*slot).or_default().push((*id, args));
        }
        let pick = |class: i16| -> Vec<(i32, Handle)> {
            // One object per ID: the active one when a world holds more than one.
            let mut by_id: BTreeMap<i32, Handle> = BTreeMap::new();
            for &s in world.slots_of_class(class) {
                let Some(h) = world.handle(s) else { continue };
                let Some(id) = object_id(h) else { continue };
                let active = world.flags(h) & flags::IN_WORLD != 0;
                match by_id.get(&id) {
                    Some(&old) if world.flags(old) & flags::IN_WORLD != 0 || !active => {}
                    _ => {
                        by_id.insert(id, h);
                    }
                }
            }
            by_id.into_iter().collect()
        };
        let jobs = pick(ids.job_class);
        let tasks = pick(ids.task_class);
        for (i, (id, _)) in jobs.iter().enumerate() {
            g.job_ids.insert(*id, i);
        }
        for (i, (id, _)) in tasks.iter().enumerate() {
            g.task_ids.insert(*id, i);
        }
        let no_lists = Vec::new();
        for &(id, h) in &jobs {
            let slot = world.slot(h).unwrap_or(0);
            let o = world.options(h);
            let i32_of = |opt: u16| o.and_then(|o| o.i32(opt));
            let ls = lists.get(&slot).unwrap_or(&no_lists);
            let args_of = |opt: u16| -> Vec<i32> {
                ls.iter().filter(|(i, _)| *i == opt).flat_map(|(_, a)| a.iter().filter_map(arg_i32)).collect()
            };
            let screen = args_of(ids.screen);
            g.jobs.push(JobDef {
                slot,
                id,
                name: tail(world, h, false).2,
                first: i32_of(ids.first).and_then(|t| g.task_ids.get(&t).copied()),
                last: i32_of(ids.last).and_then(|t| g.task_ids.get(&t).copied()),
                title: i32_of(ids.title).map_or(-1, |v| v as i16),
                text: i32_of(ids.text).map_or(-1, |v| v as i16),
                screen: (screen.len() >= 3).then(|| [screen[0], screen[1], screen[2]]),
                exit: i32_of(ids.job_exit).filter(|&v| v >= 0),
                keep_open: o.is_some_and(|o| o.bool_or(ids.keep_open, false)),
                end_events: args_of(ids.end_events),
                close_events: args_of(ids.close_events),
                switches: switches_of(ls, ids.job_switch, ids.switch_trailing),
                keys: keys(world, h, &ids.job_keys, names),
                replay: ids.completion_mode != 0 && i32_of(ids.completion_mode) == Some(2),
                item: (ids.item != 0).then(|| i32_of(ids.item)).flatten().filter(|&v| v >= 0),
                ignore_defeat: ids.ignore_defeat != 0 && o.is_some_and(|o| o.bool_or(ids.ignore_defeat, false)),
                resume: (ids.resume_task != 0)
                    .then(|| i32_of(ids.resume_task))
                    .flatten()
                    .and_then(|t| g.task_ids.get(&t).copied()),
                events: if ids.event_list != 0 {
                    args_of(ids.event_list).into_iter().take(8).collect()
                } else {
                    Vec::new()
                },
            });
        }
        for &(id, h) in &tasks {
            let slot = world.slot(h).unwrap_or(0);
            let o = world.options(h);
            let i32_of = |opt: u16| o.and_then(|o| o.i32(opt));
            let ls = lists.get(&slot).unwrap_or(&no_lists);
            let (pre, suc, name) = tail(world, h, true);
            let switches = switches_of(ls, ids.switch, ids.switch_trailing);
            let goals = ls
                .iter()
                .filter(|(i, _)| *i == ids.goal)
                .filter_map(|(_, a)| {
                    let v: Vec<i32> = a.iter().filter_map(arg_i32).collect();
                    (v.len() >= 6).then(|| Goal {
                        state: v[0],
                        object: v[1],
                        machine: v[2],
                        goal: v[3],
                        instant: v[4] != 0,
                        row: v[5] as i16,
                    })
                })
                .collect();
            let idx = |l: Vec<i32>| l.into_iter().filter_map(|t| g.task_ids.get(&t).copied()).collect::<Vec<_>>();
            g.tasks.push(TaskDef {
                slot,
                id,
                name,
                active: world.flags(h) & flags::IN_WORLD != 0,
                job: i32_of(ids.job).and_then(|j| g.job_ids.get(&j).copied()),
                objective: i32_of(ids.objective).map(|v| v as i16),
                checkpoint: i32_of(ids.checkpoint).filter(|&v| v >= 0),
                row: i32_of(ids.row).map(|v| v as i16).filter(|&v| v >= 0),
                character: i32_of(ids.character)
                    .or_else(|| args_of_slot(ls, ids.character).first().copied())
                    .filter(|&v| v >= 0),
                marker: i32_of(ids.marker).filter(|&v| v >= 0),
                exit: i32_of(ids.task_exit).filter(|&v| v >= 0),
                counts_done: o.is_some_and(|o| o.bool_or(ids.counts_done, false)),
                auto_current: o.is_some_and(|o| o.bool_or(ids.auto_current, false)),
                resume: o.is_some_and(|o| o.bool_or(ids.resume, false)),
                script: (ids.task_script != 0).then(|| i32_of(ids.task_script)).flatten().filter(|&v| v >= 0),
                done_on_current: ids.done_when_current != 0
                    && o.is_some_and(|o| o.bool_or(ids.done_when_current, false)),
                prereq: idx(pre),
                succ: idx(suc),
                switches,
                goals,
                keys: keys(world, h, &ids.task_keys, names),
            });
        }
        g
    }

    /// The tasks of job `j`, in index order.
    pub fn tasks_of(&self, j: usize) -> impl Iterator<Item = usize> + '_ {
        self.tasks.iter().enumerate().filter(move |(_, t)| t.job == Some(j)).map(|(i, _)| i)
    }

    /// Every task in prerequisite-first order (missions.md 7.1): each after all its prerequisites, otherwise in index
    /// order; tasks on a prerequisite cycle (none on the disc) come last, in index order.
    pub fn prerequisite_order(&self) -> Vec<usize> {
        let n = self.tasks.len();
        let mut waiting: Vec<usize> = self.tasks.iter().map(|t| t.prereq.iter().filter(|&&p| p < n).count()).collect();
        let mut placed = alloc::vec![false; n];
        let mut out = Vec::with_capacity(n);
        let mut ready: alloc::collections::BTreeSet<usize> = (0..n).filter(|&t| waiting[t] == 0).collect();
        while let Some(t) = ready.pop_first() {
            placed[t] = true;
            out.push(t);
            for &c in &self.tasks[t].succ {
                if c < n && !placed[c] && self.tasks[c].prereq.contains(&t) {
                    waiting[c] = waiting[c].saturating_sub(1);
                    if waiting[c] == 0 {
                        ready.insert(c);
                    }
                }
            }
        }
        out.extend((0..n).filter(|&t| !placed[t]));
        out
    }
}
