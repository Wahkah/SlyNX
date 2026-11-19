//! Sly 3 jobs and tasks on the Sly 2 job system (`engine::game::sly2_jobs`): the same graph, task rules, progress
//! block and effects, read with Sly 3's classes and option IDs ([`IDS`]).
//!
//! The Sly 3 specs name the job's exit (1381), the task's checkpoint (1400) and exit (1406), and the save keys
//! (saving.md 2). The other IDs below were found on the disc by what their values name, and mapped onto Sly 2's
//! by that (working readings until a Sly 3 missions spec exists):
//!
//! | Sly 2 | Sly 3 | What the disc shows (v_ext) |
//! |---|---|---|
//! | job first / last task 1243, 1244 | 1364, 1365 | a task (class 169) on every job |
//! | job title / text 1245, 1246 | 1370, 1371 | a string ID |
//! | job keys 1260 to 1263 | 1387 to 1390 | paths ending `time_played`, `suck_value`, `focus_count`, `is_complete` |
//! | task's job 1264 | 1391 | a job (class 168) |
//! | objective 1265 | 1392 | a string ID |
//! | auto-current 1272, resume 1273 | 1398, 1399 | a byte |
//! | checkpoint 1274, row 1275, character 1276 | 1400, 1401, 1402 | a class-40 point; a world slot (3 in v_ext); (24, −1) |
//! | marker 1279 | 1407 | a placement (class 8) |
//! | switches 1287, goals 1288 | 1416, 1417 | Sly 2's lists with one more flag at the end |
//! | task keys 1295 to 1299 | 1423 to 1427 | `focus_count`, `complete_count`, `is_complete`, `state`, `from_memcard` |
//! | — | 1408 | a command script (class 135): started when the task becomes current [I] |
//!
//! Not found: the job's keep-open flag, ending and closing events, "job complete" screen and load-time switches,
//! and the task's counts-as-done flag (0 below: no record has option 0).

use engine::game::sly2_jobs::graph::Ids;
use engine::game::sly2_jobs::{Carry, Jobs, state};
use engine::world::World;
use save::sly2::{Progress, SaveValue};

/// Sly 3's classes and option IDs for the job system.
pub const IDS: Ids = Ids {
    job_class: 168,
    task_class: 169,
    end_events: 0,
    close_events: 0,
    keep_open: 0,
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
    counts_done: 0,
    auto_current: 1398,
    resume: 1399,
    checkpoint: 1400,
    row: 1401,
    character: 1402,
    task_exit: 1406,
    marker: 1407,
    switch: 1416,
    goal: 1417,
    task_keys: [1423, 1424, 1425, 1426, 1427],
    task_script: 1408,
    switch_trailing: 1,
};

/// The job system of a freshly loaded Sly 3 world with `progress`, restored: `carry` from the previous world (none
/// after a new game or a load), `queued` a task requested, forced (an episode start's task).
pub fn attach(world: &mut World, progress: Progress, carry: Option<Carry>, queued: Option<i32>) -> Jobs {
    let mut j = Jobs::new_with(world, progress, &IDS);
    j.restore(queued, carry);
    j.apply(world);
    j
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
    for (task, s) in core::mem::take(&mut world.shared.task_requests) {
        if let Some(&t) = j.graph.task_ids.get(&task) {
            j.request(t, s as i8, false);
        }
    }
    j.apply(world);
}

/// Command `0x1E` (scripts.md 7.2): task `task` set to `s` as at a checkpoint (4: made current), forced.
pub fn set_task(j: &mut Jobs, world: &mut World, task: i32, s: i32) {
    let Some(&t) = j.graph.task_ids.get(&task) else { return };
    if s == state::BECAME_CURRENT {
        j.request_current(t, true);
    } else if (0..=3).contains(&s) {
        j.force_state(t, s as i8);
    }
    j.apply(world);
}
