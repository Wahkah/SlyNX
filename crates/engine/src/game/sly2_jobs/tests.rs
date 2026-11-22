//! Unit tests of the job rules on a small graph: an outside task `t0`, a job `m1` of three tasks `t1` → `t2` → `t3`
//! and a next task `t4` of job `m2`.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use save::sly2::{Progress, SaveValue, VarTree};

use super::graph::{Graph, JobDef, Keys, Switch, TaskDef};
use super::*;

/// A top-level save entry: name, kind, children (name, kind).
type Entry = (u16, i8, Vec<(u16, i8)>);
/// A record: name, kind, count, offset, child, next.
type Rec = (i16, i8, u8, u16, u16, u16);

/// A save-variable section: `dt`, `episodeBest`, `suck_history/mission_0..2`, then a group per job and task with
/// the fields their keys name.
fn tree() -> VarTree {
    const TASK: &[&str] = &["focus_count", "complete_count", "is_complete", "state", "from_memcard"];
    const JOB: &[&str] = &["focus_count", "is_complete", "time_played", "suck_value"];
    let mut names: Vec<String> = Vec::new();
    let id = |n: &str, names: &mut Vec<String>| -> u16 {
        match names.iter().position(|x| x == n) {
            Some(i) => i as u16,
            None => {
                names.push(n.to_string());
                (names.len() - 1) as u16
            }
        }
    };
    // (name, kind, children)
    let mut top: Vec<Entry> = Vec::new();
    top.push((id("dt", &mut names), 3, vec![]));
    top.push((id("episodeBest", &mut names), 0, vec![]));
    let h: Vec<(u16, i8)> = ["mission_0", "mission_1", "mission_2"].iter().map(|m| (id(m, &mut names), 3)).collect();
    top.push((id("suck_history", &mut names), -1, h));
    for g in ["m1", "m2"] {
        let c = JOB
            .iter()
            .map(|f| (id(f, &mut names), if *f == "time_played" || *f == "suck_value" { 3 } else { 0 }))
            .collect();
        top.push((id(g, &mut names), -1, c));
    }
    for g in ["t0", "t1", "t2", "t3", "t4"] {
        let c = TASK.iter().map(|f| (id(f, &mut names), 0)).collect();
        top.push((id(g, &mut names), -1, c));
    }
    // Records: root, then each top entry followed by its children.
    let mut recs: Vec<Rec> = vec![(-1, -1, 0, 0, 1, 0)];
    let mut offset = 0u16;
    for (i, (name, kind, children)) in top.iter().enumerate() {
        let me = recs.len();
        recs.push((*name as i16, *kind, 0, 0, 0, 0));
        if children.is_empty() {
            recs[me].3 = offset;
            offset += 4;
        } else {
            recs[me].4 = (me + 1) as u16;
            for (k, (cn, ck)) in children.iter().enumerate() {
                let next = if k + 1 < children.len() { (recs.len() + 1) as u16 } else { 0 };
                recs.push((*cn as i16, *ck, 0, offset, 0, next));
                offset += 4;
            }
        }
        if i + 1 < top.len() {
            recs[me].5 = recs.len() as u16;
        }
    }
    recs.push((-1, -1, 0, offset, 0, 0));
    let mut d = Vec::new();
    d.extend_from_slice(&(names.len() as u16).to_le_bytes());
    for (i, n) in names.iter().enumerate() {
        d.extend_from_slice(&(i as u16).to_le_bytes());
        d.extend_from_slice(&(n.len() as u16).to_le_bytes());
        d.extend_from_slice(n.as_bytes());
    }
    d.extend_from_slice(&(recs.len() as u16).to_le_bytes());
    for (n, k, c, o, ch, nx) in recs {
        d.extend_from_slice(&n.to_le_bytes());
        d.push(k as u8);
        d.push(c);
        d.extend_from_slice(&o.to_le_bytes());
        d.extend_from_slice(&ch.to_le_bytes());
        d.extend_from_slice(&nx.to_le_bytes());
    }
    VarTree::parse(&d).unwrap().0
}

fn keys(t: &VarTree, group: &str, fields: &[&str]) -> Keys {
    let p = |f: &str| Some(vec![t.name_id(group).unwrap(), t.name_id(f).unwrap()]);
    let mut k = Keys::default();
    for f in fields {
        match *f {
            "focus_count" => k.focus = p(f),
            "complete_count" => k.completed = p(f),
            "is_complete" => k.complete = p(f),
            "state" => k.state = p(f),
            "from_memcard" => k.restarted = p(f),
            "time_played" => k.time = p(f),
            "suck_value" => k.help = p(f),
            _ => {}
        }
    }
    k
}

fn task(t: &VarTree, i: usize, job: Option<usize>, prereq: Vec<usize>, succ: Vec<usize>) -> TaskDef {
    let name = alloc::format!("t{i}");
    TaskDef {
        slot: 100 + i as u32,
        id: 1000 + i as i32,
        keys: keys(t, &name, &["focus_count", "complete_count", "is_complete", "state", "from_memcard"]),
        name,
        active: true,
        job,
        objective: Some(i as i16),
        checkpoint: (i == 2).then_some(5000),
        row: None,
        character: None,
        marker: None,
        exit: None,
        counts_done: false,
        auto_current: false,
        resume: i == 1,
        script: None,
        done_on_current: false,
        prereq,
        succ,
        switches: vec![Switch { object: 7000 + i as i32, states: vec![1], owner: -1, row: -1 }],
        goals: Vec::new(),
    }
}

fn jobs() -> Jobs {
    let t = Arc::new(tree());
    let job = |i: usize, first: usize, last: usize| JobDef {
        slot: 200 + i as u32,
        id: 2000 + i as i32,
        name: alloc::format!("m{i}"),
        first: Some(first),
        last: Some(last),
        title: 0,
        text: 0,
        screen: None,
        exit: Some(9000 + i as i32),
        keep_open: false,
        end_events: Vec::new(),
        close_events: vec![77],
        switches: Vec::new(),
        replay: false,
        item: None,
        resume: None,
        events: Vec::new(),
        ignore_defeat: false,
        keys: keys(&t, &alloc::format!("m{i}"), &["focus_count", "is_complete", "time_played", "suck_value"]),
    };
    let mut g = Graph {
        jobs: vec![job(1, 1, 3), job(2, 4, 4)],
        tasks: vec![
            task(&t, 0, None, vec![], vec![1]),
            task(&t, 1, Some(0), vec![0], vec![2]),
            task(&t, 2, Some(0), vec![1], vec![3]),
            task(&t, 3, Some(0), vec![2], vec![4]),
            task(&t, 4, Some(1), vec![3], vec![]),
        ],
        ..Graph::default()
    };
    for (i, j) in g.jobs.iter().enumerate() {
        g.job_ids.insert(j.id, i);
    }
    for (i, x) in g.tasks.iter().enumerate() {
        g.task_ids.insert(x.id, i);
    }
    let n = g.tasks.len();
    let mut j = Jobs {
        graph: g,
        progress: Progress::new(t),
        states: vec![state::UNKNOWN; n],
        current: None,
        job: None,
        checkpoint: None,
        row: 2,
        episode: 1,
        timer: 0.0,
        idle: 0.0,
        help: 0.0,
        done_in_job: BTreeSet::new(),
        respawn: flow::DEFAULT_POINT,
        exits: Vec::new(),
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
    };
    j.restore(None, None);
    j
}

fn presents(j: &mut Jobs) -> Vec<(i32, bool)> {
    core::mem::take(&mut j.effects)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Present { object, on, .. } => Some((object, on)),
            _ => None,
        })
        .collect()
}

#[test]
fn restore_opens_tasks_without_prerequisites() {
    let mut j = jobs();
    assert_eq!(j.states, vec![1, 0, 0, 0, 0]);
    // The switches of every task's state were applied: t0's object is present, the others' not.
    let p = presents(&mut j);
    assert!(p.contains(&(7000, true)) && p.contains(&(7001, false)));
}

#[test]
fn restore_keeps_saved_locked_tasks_locked() {
    // Task 0 counts as done while available (option 1271): live play does not re-evaluate task 1 then (successors
    // are re-evaluated when a task becomes done or closed), so it stays locked; a save and restore must agree.
    let mut j = jobs();
    j.graph.tasks[0].counts_done = true;
    assert!(j.prereqs_met(1));
    assert_eq!(j.states[1], state::LOCKED);
    j.restore(None, None);
    assert_eq!(j.states, vec![1, 0, 0, 0, 0], "the restore does not open task 1");
}

#[test]
fn a_request_for_minus_one_or_four_makes_current() {
    // Method 1280 (Sly 3 1409): −1 or 4 as the state makes the task current (missions.md 3.1).
    let mut j = jobs();
    assert!(j.request(0, -1, false));
    assert_eq!(j.current, Some(0));
    j.request(0, state::DONE, false);
    assert!(j.request(1, 4, false));
    assert_eq!(j.current, Some(1));
}

#[test]
fn done_at_once_chains_without_recursion_and_does_not_rebegin_the_job() {
    // Tasks 1 to 3 (job 0's) become current when available and ask for "done" at once (Sly 3's option 1405): task
    // 0's completion runs the whole chain, job 0 completes once and is not begun again, and job 1's task 4 is
    // current with job 1 begun.
    let mut j = jobs();
    for t in &mut j.graph.tasks[1..=3] {
        t.done_on_current = true;
        t.auto_current = true;
    }
    j.graph.tasks[4].auto_current = true;
    assert!(j.request(0, state::DONE, false));
    assert!(j.states[1..=3].iter().all(|&s| s >= state::DONE), "{:?}", j.states);
    let completions = j.notices.iter().filter(|n| matches!(n, Notice::JobComplete { job: 2001, .. })).count();
    assert_eq!(completions, 1, "job 0 completes once");
    assert_eq!((j.current, j.job), (Some(4), Some(1)));
    assert!(!j.finishing && j.done_queue.is_empty());
}

#[test]
fn a_long_done_at_once_chain_does_not_nest() {
    // 20,000 tasks in a row, each asking for "done" when it becomes current: one loop, no deep recursion.
    let mut j = jobs();
    let n = 20_000;
    let base = j.graph.tasks[1].clone();
    j.graph.tasks.truncate(1);
    j.graph.tasks[0].succ = vec![1];
    for i in 1..=n {
        let mut t = base.clone();
        t.id = 10_000 + i as i32;
        t.job = None;
        t.exit = None;
        t.checkpoint = None;
        t.done_on_current = true;
        t.auto_current = true;
        t.prereq = vec![i - 1];
        t.succ = if i < n { vec![i + 1] } else { Vec::new() };
        t.switches = Vec::new();
        j.graph.tasks.push(t);
    }
    j.states = vec![state::LOCKED; n + 1];
    j.states[0] = state::AVAILABLE;
    // Nested, this depth would overflow the test thread's stack.
    assert!(j.request(0, state::DONE, false));
    assert!(j.states.iter().all(|&s| s >= state::DONE));
}

#[test]
fn becoming_current_begins_the_job_then_event_19_then_the_exit() {
    // missions.md 4.2: the job begins before the task's switches and event 19, and the task's exit fires last.
    let mut j = jobs();
    j.graph.tasks[1].exit = Some(9100);
    j.request(0, state::DONE, false);
    j.effects.clear();
    assert!(j.request_current(1, false));
    let pos = |f: &dyn Fn(&Effect) -> bool| j.effects.iter().position(f).expect("the effect");
    let job_begin = pos(&|e| matches!(e, Effect::Script { slot: 201, event: script_event::BEGIN, .. }));
    let task_begin = pos(&|e| matches!(e, Effect::Script { slot: 101, event: script_event::BEGIN, .. }));
    let exit = pos(&|e| matches!(e, Effect::FireExit(9100)));
    assert!(job_begin < task_begin && task_begin < exit, "{:?}", j.effects);
}

#[test]
fn world_start_takes_the_remembered_task_and_job_and_1273_wins() {
    // The carry's job begins and its task becomes current (missions.md 7.1).
    let mut j = jobs();
    j.request(0, state::DONE, false);
    j.request(1, state::DONE, false);
    j.request(2, state::AVAILABLE, false);
    let carry = Carry { job: 2001, task: 1002, checkpoint: 1002, timer: 4.0, help: 0.3 };
    let saved = j.progress.clone();
    let mut k = jobs();
    k.progress = saved.clone();
    k.restore(None, Some(carry));
    assert_eq!((k.current, k.job, k.checkpoint), (Some(2), Some(0), Some(2)));
    assert_eq!((k.timer, k.help), (4.0, 0.3), "the carried timer and help go on");
    // A task with option 1273 restored as available replaces the remembered task on every world start.
    let mut k = jobs();
    k.progress = saved;
    k.graph.tasks[3].resume = true;
    let s3 = k.graph.tasks[3].keys.state.clone();
    put(&mut k.progress, &s3, SaveValue::Int(1));
    k.restore(None, Some(carry));
    assert_eq!(k.current, Some(3), "the last 1273 task restored as available");
}

#[test]
fn prerequisite_order_puts_prerequisites_first() {
    let mut j = jobs();
    // Make task 0 depend on task 4: 4 comes before 0, the rest keep their chain order.
    j.graph.tasks[0].prereq = vec![4];
    j.graph.tasks[4].succ.push(0);
    j.graph.tasks[4].prereq.clear();
    j.graph.tasks[3].succ.clear();
    let order = j.graph.prerequisite_order();
    let at = |t: usize| order.iter().position(|&x| x == t).unwrap();
    assert!(at(4) < at(0) && at(0) < at(1) && at(1) < at(2) && at(2) < at(3), "{order:?}");
}

#[test]
fn prerequisite_order_reads_the_prerequisite_lists_alone() {
    let mut j = jobs();
    // The chain 0 → 1 → 2 → 3, with successor lists that no longer name it back and a prerequisite listed twice.
    for t in &mut j.graph.tasks {
        t.succ.clear();
    }
    j.graph.tasks[2].prereq = vec![1, 1];
    let order = j.graph.prerequisite_order();
    let at = |t: usize| order.iter().position(|&x| x == t).unwrap();
    assert!(at(0) < at(1) && at(1) < at(2) && at(2) < at(3), "{order:?}");
}

#[test]
fn requests_step_one_state_at_a_time() {
    let mut j = jobs();
    // Locked with prerequisites not done: refused (and announced).
    assert!(!j.request(1, state::AVAILABLE, false));
    assert!(j.notices.contains(&Notice::Refused(1001)));
    // Done → closed is one step; locked → done is not; a forced request skips the checks; closed never changes.
    assert!(j.request(0, state::DONE, false));
    assert_eq!(j.states[1], state::AVAILABLE, "the successor opened");
    assert!(!j.request(2, state::DONE, false), "two steps up");
    assert!(j.request(2, state::DONE, true), "forced");
    assert!(j.request(0, state::CLOSED, false));
    assert!(!j.request(0, state::AVAILABLE, true), "a closed task never changes");
    assert_eq!(j.states[0], state::CLOSED);
}

#[test]
fn a_job_begins_counts_and_completes() {
    let mut j = jobs();
    j.request(0, state::DONE, false);
    // Task 1 becomes current: its job begins.
    assert!(j.request(1, state::AVAILABLE, false));
    assert_eq!((j.current, j.job), (Some(1), Some(0)));
    assert_eq!(j.progress.get(j.graph.jobs[0].keys.focus.as_ref().unwrap(), None), Some(SaveValue::Int(1)));
    assert_eq!(j.progress.get(j.graph.tasks[1].keys.focus.as_ref().unwrap(), None), Some(SaveValue::Int(1)));
    // Help: +0.1 on a defeat, −0.2 the first time a task of the job is done (not below 0).
    j.task_event(event::DEFEAT, None);
    assert!((j.help - 0.1).abs() < 1e-6);
    j.request(1, state::DONE, false);
    assert_eq!(j.help, 0.0);
    j.request(2, state::AVAILABLE, false);
    assert_eq!(j.checkpoint, Some(2), "a task with a checkpoint becomes the checkpoint task");
    j.request(2, state::DONE, false);
    j.request(3, state::AVAILABLE, false);
    j.timer = 12.5;
    j.request(3, state::DONE, false);
    // The last task done: the job completes, closes its last task, ends, autosaves and requests the next task.
    assert!(j.notices.contains(&Notice::JobComplete { job: 2001, screen: None }));
    assert!(j.notices.contains(&Notice::Autosave));
    assert_eq!(j.states[3], state::CLOSED);
    // The prerequisites of a closed task are raised with it.
    assert_eq!(&j.states[..3], &[state::CLOSED, state::CLOSED, state::CLOSED]);
    assert_eq!((j.current, j.job), (Some(4), Some(1)), "the next job's task is current");
    assert_eq!(j.progress.get(j.graph.jobs[0].keys.time.as_ref().unwrap(), None), Some(SaveValue::Float(12.5)));
    assert_eq!(j.progress.float(&["suck_history", "mission_0"]), 0.0);
    assert_eq!(j.percent(), 100, "every task with successors is closed");
}

#[test]
fn close_events_and_restart_at_checkpoint() {
    let mut j = jobs();
    j.request(0, state::DONE, false);
    j.request(1, state::AVAILABLE, false);
    j.request(1, state::DONE, false);
    j.request(2, state::AVAILABLE, false);
    j.restart_at_checkpoint();
    let load = j.load.take().expect("a reload at the checkpoint");
    assert_eq!((load.row, load.point), (2, 5000));
    assert_eq!(j.current, Some(2));
    // Job 1 closes on task event 77 (its option 1237).
    j.task_event(77, None);
    assert_eq!(j.job, Some(1));
}

#[test]
fn a_world_change_carries_and_a_load_resumes() {
    let mut j = jobs();
    j.request(0, state::DONE, false);
    j.request(1, state::AVAILABLE, false);
    j.timer = 3.0;
    j.store_current();
    // A world change carries the current task and job, the timer and the help.
    let mut k = jobs();
    k.progress = j.progress.clone();
    k.states = vec![state::UNKNOWN; 5];
    k.restore(None, Some(j.carry()));
    assert_eq!(k.states, j.states);
    assert_eq!((k.current, k.job, k.timer), (Some(1), Some(0), 3.0));
    // A load carries nothing: task 1 has option 1273 and was restored as available, so it is current again, and the
    // job's timer comes from its save key.
    let mut l = jobs();
    l.progress = j.progress.clone();
    l.states = vec![state::UNKNOWN; 5];
    l.restore(None, None);
    assert_eq!((l.current, l.job, l.timer), (Some(1), Some(0), 3.0));
}

#[test]
fn script_methods() {
    let mut j = jobs();
    j.force_state(2, state::DONE);
    assert_eq!(j.states[2], state::DONE);
    j.count_failure();
    assert!((j.help - 0.1).abs() < 1e-6);
    j.request(0, state::DONE, false);
    j.request(1, state::AVAILABLE, false);
    j.reset_play_counts(0);
    assert_eq!(j.progress.get(j.graph.tasks[1].keys.focus.as_ref().unwrap(), None), Some(SaveValue::Int(0)));
}
