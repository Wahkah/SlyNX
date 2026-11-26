//! Sly 3 jobs and tasks on the Sly 2 job system (`engine::game::sly2_jobs`): the same graph, task rules, progress
//! block and effects, read with Sly 3's classes and option IDs ([`IDS`]).
//!
//! The IDs are specs/sly3/missions.md 2 and 3:
//!
//! | Sly 2 | Sly 3 | Meaning |
//! |---|---|---|
//! | job keep-open 1242 | 1361 | the job does not close its last task or autosave |
//! | job first / last task 1243, 1244 | 1364, 1365 | |
//! | job title / text 1245, 1246 | 1370, 1371 | string IDs |
//! | job exit 1259 | 1381 | fired when the job ends with no next task |
//! | job keys 1260 to 1263 | 1387 to 1390 | `time_played`, `suck_value`, `focus_count`, `is_complete` |
//! | task's job 1264, objective 1265 | 1391, 1392 | |
//! | counts as done 1271, current when available 1272 | 1398, 1399 | |
//! | checkpoint 1274, world 1275, character 1276 | 1400, 1401, 1402 | (1402 is a two-argument method) |
//! | task exit 1278, waypoint 1279 | 1406, 1407 | 1407 names the class-120 mission waypoint |
//! | switches 1287, goals 1288 | 1416, 1417 | Sly 2's arguments plus one more value |
//! | task keys 1295 to 1299 | 1423 to 1427 | |
//! | — | 1408 | a command script started when the task becomes current (missions.md 4) |
//! | — | 1405 | the task is done as soon as it becomes current |
//!
//! Sly 3 has no "resume after loading" (Sly 2's 1273), no job switches (1255) and no ending or closing event lists
//! (0 below: no record has option 0). On completion (missions.md 4.1): 1367 = 2 sends the first task back to 0,
//! 1385 grants an item, and a following job's resume task (1366) becomes the forced checkpoint; 1368's task-event
//! list is kept on the job. 1362 makes a job ignore the player's defeat ([`defeat`]). Not read yet: the job's state-machine goal 1379 (Sly 2's 1256), 1372 and 1386.

use alloc::vec::Vec;
use engine::game::sly2_jobs::graph::Ids;
use engine::game::sly2_jobs::{Carry, Jobs, state};
use engine::world::World;
use save::sly2::{Progress, SaveValue};

/// The macro-package listener class: its objects in the world hear every task event (missions.md 4.2).
pub const MACRO_LISTENER_CLASS: i16 = 147;

/// Sly 3's classes and option IDs for the job system.
pub const IDS: Ids = Ids {
    job_class: 168,
    task_class: 169,
    end_events: 0,
    close_events: 0,
    keep_open: 1361,
    first: 1364,
    last: 1365,
    title: 1370,
    text: 1371,
    screen: 0,
    job_exit: 1381,
    job_switch: 0,
    job_keys: [1387, 1388, 1389, 1390],
    job: 1391,
    objective: 1392,
    counts_done: 1398,
    auto_current: 1399,
    resume: 0,
    checkpoint: 1400,
    row: 1401,
    character: 1402,
    task_exit: 1406,
    marker: 1407,
    switch: 1416,
    goal: 1417,
    task_keys: [1423, 1424, 1425, 1426, 1427],
    task_script: 1408,
    done_when_current: 1405,
    completion_mode: 1367,
    item: 1385,
    resume_task: 1366,
    event_list: 1368,
    ignore_defeat: 1362,
    switch_trailing: 1,
};

/// Sly 3's script events for the job system's (scripts.md 6, missions.md 4): a task event is event 20 (Sly 2's 17),
/// becoming current or a job beginning event 22 (Sly 2's 19). The others' Sly 3 numbers are not established and
/// are not raised (Sly 3's 21 to 24 mean other things than Sly 2's).
pub fn script_events(e: u32) -> Option<u32> {
    use engine::game::sly2_jobs::script_event;
    match e {
        script_event::TASK_EVENT => Some(20),
        script_event::BEGIN => Some(22),
        // No longer current (interactables.md 5a: task 2245's module answers it by removing its photo target).
        script_event::END => Some(23),
        // Around a state change of a live task (missions.md 4.1a): event 25 before, 24 after, on the task.
        script_event::STATE_BEFORE => Some(25),
        script_event::STATE => Some(24),
        _ => None,
    }
}

/// Sly 3's task-event codes where they differ from Sly 2's (missions.md 4.1a): a state change is broadcast with 42
/// before and 41 after.
pub fn task_codes(e: u32) -> u32 {
    use engine::game::sly2_jobs::event;
    match e {
        event::STATE_BEFORE => 42,
        event::STATE_AFTER => 41,
        e => e,
    }
}

/// The job system of a freshly loaded Sly 3 world with `progress`, restored: `carry` from the previous world (none
/// after a new game or a load), `queued` a task requested, forced (an episode start's task). The world start's steps
/// 1 and 2 (missions.md 4.1a): the tasks' events are raised in mode 1 and run with the world's next frame; step 3 is
/// [`finish_world_start`], after them.
pub fn attach(world: &mut World, progress: Progress, carry: Option<Carry>, queued: Option<i32>) -> Jobs {
    let mut j = Jobs::new_with(world, progress, &IDS);
    j.script_events = script_events;
    j.task_codes = task_codes;
    j.sly3_rules = true; // Sly 3's order of becoming current and world start (sly3/missions.md 4, 7).
    // The macro packages (class 147) hear every task event too (missions.md 4.2).
    j.listener_class = Some(MACRO_LISTENER_CLASS);
    j.restore(queued, carry);
    j.apply(world);
    mirror(&j, world);
    // Scripts read the tasks' mode (script ID 1280) as 1 until step 3.
    world.shared.sly3_restore_mode = true;
    j
}

/// World-start step 3 (missions.md 4.1a), part one: the requests queued in mode 1 (by the restore's and step 2's
/// events) run in order, in mode 1. The events they raise are still mode 1's; the session runs them and calls this
/// again while they queue more, then [`finish_world_start`].
pub fn run_queued(j: &mut Jobs, world: &mut World) {
    j.in_restoring_mode(|j| run_requests(j, world));
    j.apply(world);
    mirror(j, world);
}

/// World-start step 3, part two: the "current when available" pass. Scripts then read the mode as 0 (live play).
pub fn finish_world_start(j: &mut Jobs, world: &mut World) {
    world.shared.sly3_restore_mode = false;
    current_when_available(j);
    j.apply(world);
    mirror(j, world);
}

/// Sly 3 hook: the task states, mirrored into the world for scripts (task option 1415; missions.md 4.1a).
pub fn mirror(j: &Jobs, world: &mut World) {
    let m = &mut world.shared.sly3_task_states;
    m.clear();
    m.extend(j.graph.tasks.iter().zip(&j.states).map(|(t, &s)| (t.id, s)));
    // The current job and task, for game operations 104 and 105.
    world.shared.sly3_current = [j.job.map(|k| j.graph.jobs[k].slot), j.current.map(|t| j.graph.tasks[t].slot)];
}

/// The world start's "current when available" pass (missions.md 4.1a step 3): the last available task with 1399
/// set that is not the current task, in the available list's order ([`visit_order`]), gets a forced request for
/// state 4 (made current). The available tasks are every available task, whatever its job (missions.md 4.1a).
/// Steps 1 and 2 are [`Jobs::restore`]: the tasks restored without events, then the carried job and task (a world
/// change) re-begun and made current.
pub fn current_when_available(j: &mut Jobs) {
    // The game repeats the pass while its request queue holds requests; here requests apply at once, so the queue
    // is empty after the forced request and the pass ends.
    let next = visit_order(j)
        .into_iter()
        .rev()
        .find(|&t| j.states[t] == state::AVAILABLE && j.graph.tasks[t].auto_current && j.current != Some(t));
    if let Some(t) = next {
        j.request_current(t, true);
    }
}

/// The restore's visit order (missions.md 4.1a, "Visit order"), which is also the order the restore fills the
/// available list. The walk starts at the roots and reaches a task once all its prerequisites are visited, recording
/// its depth (the longest path from a root) and its job-change depth (along such a path, one more at each step from a
/// task with a job to a task of a different job). The order is sorted by the job-change depth, the job's object ID
/// (−1 for none), the depth, then the task's object ID.
pub fn visit_order(j: &Jobs) -> Vec<usize> {
    let g = &j.graph;
    let (depth, changes) = visit_depths(j);
    let mut order: Vec<usize> = g.prerequisite_order();
    order.sort_by_key(|&t| {
        let d = &g.tasks[t];
        (changes[t], d.job.map_or(-1, |k| g.jobs[k].id), depth[t], d.id)
    });
    order
}

/// Each task's depth and job-change depth (see [`visit_order`]): two separate maxima over the task's prerequisites,
/// each taken once all of them are visited.
pub fn visit_depths(j: &Jobs) -> (Vec<u32>, Vec<u32>) {
    let g = &j.graph;
    let n = g.tasks.len();
    let (mut depth, mut changes) = (alloc::vec![0u32; n], alloc::vec![0u32; n]);
    // From the prerequisite lists alone (each prerequisite once), in prerequisite-first order.
    for t in g.prerequisite_order() {
        let ps: alloc::collections::BTreeSet<usize> = g.tasks[t].prereq.iter().copied().filter(|&p| p < n).collect();
        for p in ps {
            let from = g.tasks[p].job;
            let step = u32::from(from.is_some() && g.tasks[t].job != from);
            depth[t] = depth[t].max(depth[p] + 1);
            changes[t] = changes[t].max(changes[p] + step);
        }
    }
    (depth, changes)
}

/// One frame: play time (total and the episode's), the job timer, the task requests of the frame, the effects.
pub fn step(j: &mut Jobs, world: &mut World, dt: f32, touched: bool) {
    let total = j.progress.float(&["dt"]) + dt;
    j.progress.set_named(&["dt"], None, SaveValue::Float(total));
    if let Some(p) = j.episode_key("dt") {
        let v = j.progress.get(&p, None).map_or(0.0, SaveValue::as_f32);
        j.progress.set(&p, None, SaveValue::Float(v + dt));
    }
    j.idle = if touched { 0.0 } else { j.idle + dt };
    if j.job.is_some() && j.idle < engine::game::sly2_jobs::IDLE_LIMIT {
        j.timer += dt;
    }
    run_requests(j, world);
    j.apply(world);
    mirror(j, world);
}

/// The frame's task requests: from props, trigger shapes and use points, then the task methods scripts called.
fn run_requests(j: &mut Jobs, world: &mut World) {
    // Task-state requests from props and trigger shapes (option 489: (task, state), state 4 meaning "current", as
    // for the task methods).
    for (task, s) in core::mem::take(&mut world.shared.task_requests) {
        if let Some(&t) = j.graph.task_ids.get(&task) {
            if s == state::BECAME_CURRENT || s == -1 {
                j.request_current(t, false);
            } else {
                j.request(t, s as i8, false);
            }
        }
    }
    // The task methods scripts called (engine::game::script_host::sly3: 1409, 1410, 1418 to 1422).
    for call in core::mem::take(&mut world.shared.task_calls) {
        use engine::world::shared::TaskCall;
        match call {
            TaskCall::Request { task, state, forced } => {
                if let Some(&t) = j.graph.task_ids.get(&task) {
                    j.request(t, state as i8, forced);
                }
            }
            TaskCall::Force { task, state } => {
                if let Some(&t) = j.graph.task_ids.get(&task) {
                    j.force_state(t, state);
                }
            }
            TaskCall::MakeCurrent(task) => {
                if let Some(&t) = j.graph.task_ids.get(&task) {
                    j.request_current(t, false);
                }
            }
        }
    }
}

/// Command `0x1E` (scripts.md 7.2) and the exit and arrival pairs (841, 847): task `task` asked for state `s` as
/// at a checkpoint, an ordinary checked request (sly3/missions.md 4.1a); 4 or −1 asks for it to become current.
pub fn set_task(j: &mut Jobs, world: &mut World, task: i32, s: i32) {
    let Some(&t) = j.graph.task_ids.get(&task) else { return };
    if s == state::BECAME_CURRENT || s == -1 {
        j.request_current(t, false);
    } else if (0..=3).contains(&s) {
        j.request(t, s as i8, false);
    }
    j.apply(world);
    mirror(j, world);
}

/// The player's defeat (game event 12; missions.md 2, option 1362): `true` when the job reacts, counting a failure
/// (the difficulty help) so that the caller reloads the checkpoint; `false` when the current job ignores it.
pub fn defeat(j: &mut Jobs, world: &mut World) -> bool {
    if j.job.is_some_and(|k| j.graph.jobs[k].ignore_defeat) {
        return false;
    }
    j.task_event(engine::game::sly2_jobs::event::DEFEAT, None);
    j.apply(world);
    mirror(j, world);
    true
}
