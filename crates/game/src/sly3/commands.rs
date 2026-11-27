//! Sly 3 command scripts (class 135; specs/sly3/scripts.md 7): the runner.
//!
//! A script's commands form a tree (first child, next sibling; index 0 is the root and never a child). Starting a
//! script makes a runner on the root's children. A runner executes a command, then its children, then the next
//! sibling; when a chain of siblings ends it returns to its parent's next sibling, and when it returns past its
//! root it ends. The block-end commands (`0x01`, `0x03`, `0x05`, `0x0A`) close their blocks in the stored list
//! and are never reached through the links.
//!
//! - **Blocks:** a nested `0x00` starts its children as a new runner named by the command's object (the current
//!   runner goes on); `0x02` keeps its children to run when the script finishes; `0x04` repeats its children while
//!   its condition holds; `0x06` runs the children of its first branch (`0x07`, `0x08`) whose condition holds,
//!   else of its `0x09` branch.
//! - **Conditions:** 1886 true, 1887 false, 1891 / 1892 an alarm set / not set, 1894 / 1895 no line / a line
//!   playing; 1888 to 1890 test a task (the first time current, live, restoring); any other value asks the
//!   script's own module.
//! - **Waits** (flag bit 0): a dialogue line (`0x52`) until it ends; `0x2F` a random time between its two values;
//!   `0x17` until the started script ends; `0x0E` until its alarm is set; `0x0C` until the named runners are gone;
//!   `0x5F` until no line plays. Other commands with the flag complete at once.
//! - **Scripts** start (`0x17`, with the command's object, else the script's own) and stop (`0x11`) here.
//! - **What the world does** goes out as [`Request`]s for the session: dialogue, exits (`0x68` with its point),
//!   adding and removing objects (`0x38`, `0x39`), task states (`0x1E`), state-machine goals (`0x19`), screens
//!   (`0x1A`, `0x1B`) and music (`0x59`, `0x5A`).
//! - **Objects and control:** `0x14` sets the script's default target and `0x18` its context object (kept per
//!   script here, not per runner); `0x22` places the target on an object, and `0x30` walks it there (it arrives
//!   at once: [`Request::Place`]); `0x13` gives a player's control to an object (missions.md 5); `0x12`
//!   activates an object by its class (scripts.md 7.3); `0x69` returns the player to the nearest mission
//!   waypoint; `0x15` calls the script's module with message 4 and a value; `0x1F`
//!   pauses or resumes the run (working reading: the pause lasts until its script ends or resumes it); `0x45` is a camera key (the camera is not moved; with the wait flag the runner
//!   waits for the key's time).
//! - Commands not listed (animation `0x16`: clips and segments are not played by the session yet, and the
//!   unestablished ones) complete at once; they are counted in [`Scripts::unhandled`].
//!
//! Working readings (to confirm with the spec): `0x1E`'s byte is the task state, 4 meaning "current"; `0x0F` /
//! `0x10` set and clear an alarm by ID, alarms being one set for all scripts. Alarms set on a named script (method
//! 735, [`Scripts::set_alarm_of`]) are that script's (scripts.md 7.2: "an alarm of this script or of another
//! script"): only that script's waits see them, and they are cleared when it ends or is stopped, so another script
//! waiting on the same alarm ID does not (the four turf-war battles' scripts all wait on 139255, missions.md 4.1a).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use audio::rng::Rng;
use audio::sly3::talk::{DialogueCommand, LineOrder};
use engine::world::World;
use formats_sly3::stream::{Command, command_script};

/// Conditions (scripts.md 7.1).
pub mod cond {
    pub const TRUE: i32 = 1886;
    pub const FALSE: i32 = 1887;
    pub const ALARM_SET: i32 = 1891;
    pub const ALARM_CLEAR: i32 = 1892;
    pub const NO_LINE: i32 = 1894;
    pub const LINE: i32 = 1895;
    /// Tests on the object the condition's second word names (scripts.md 7.4): 1888 its play count is 1 (a task's:
    /// the first time it is current, inference), 1889 its mode is 0 (live play), 1890 its mode is 1 (restoring).
    pub const FIRST_TIME: i32 = 1888;
    pub const LIVE: i32 = 1889;
    pub const RESTORING: i32 = 1890;
}

/// The longest a runner goes without waiting in one frame (a loop with no wait in it stops here).
const STEP_LIMIT: usize = 512;

/// What a script asks of the world.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// `0x52`: play `line` (chosen from the command's list), from `object` (the command's object, −1 none).
    PlayLine { line: i32, command: DialogueCommand, object: i32 },
    /// `0x53`: stop the playing line if listed (any for an empty list).
    StopLine { list: Vec<i32> },
    /// `0x54`.
    ClearSubtitle,
    /// `0x68`: fire `exit` at `point` (−1: its own).
    FireExit { exit: i32, point: i32 },
    /// `0x38` / `0x39`: put an object in or out of the world.
    Present { object: i32, on: bool },
    /// `0x1E`: a task's state (4: current).
    TaskState { task: i32, state: i32 },
    /// `0x19`: drive `object`'s state machine `machine` to `goal`.
    Goal { object: i32, machine: i32, goal: i32, snap: bool },
    /// `0x1A` / `0x1B`.
    Screen { screen: i32, on: bool },
    /// `0x59` / `0x5A`: push or pop a music track.
    Music { push: Option<i16> },
    /// `0x22`, `0x30`: put `target` where `at` is (an interaction start or a walk, arriving at once).
    Place { target: i32, at: i32 },
    /// `0x77`: a character turns to face an object (missions.md 4.1a).
    Face { target: i32, at: i32 },
    /// `0x16` with an animation-segment object (class 130) and no character (scripts.md 7.3): the segment plays on
    /// its own; flag 8 loops it [I] (2336's owner path, missions.md 4.1a).
    Segment { segment: i32, object: i32, flags: u8 },
    /// `0x6F` / `0x70`: take or release a run-time instance of a placement (class 8; missions.md 4.1a).
    Instance { placement: i32, take: bool },
    /// `0x13`: player `player`'s control goes to `object`.
    Control { object: i32, player: i32 },
    /// `0x12`: activate `object` (scripts.md 7.3: by its class).
    Activate { object: i32 },
    /// `0x69`: return the player to the nearest suitable mission waypoint (scripts.md 7.3, interpretation).
    ReturnToWaypoint,
    /// `0x15`: call `script`'s module with message 4 and `value`; `copy` is the script object the run was started
    /// on, when known (a script has one object per copy of its group).
    Module { script: i32, value: i32, copy: Option<engine::object::Handle> },
    /// `0x60` / `0x61`: show hint `string` for `seconds`, or clear it (missions.md 4.1a).
    Hint { string: i32, seconds: f32 },
    /// See [`Request::Hint`].
    ClearHint,
    /// `0x1F`: `script` pauses (true) or resumes the run; a pause ends with its script.
    Pause { script: i32, on: bool },
    /// `0x46` / `0x6E`: open the binocucom with a scripted view on `character`, looking at `look_at` (−1 none), or
    /// close it (interactables.md 5a).
    Binocucom { open: bool, character: i32, look_at: i32 },
    /// `0x62`: send guard `guard` the command list `list` (guards.md 4a).
    GuardList { guard: i32, list: i32 },
}

/// What scripts can ask about the world this frame.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// A dialogue line is playing.
    pub line_playing: bool,
    /// Each task's play count (how many times it has been current), by task ID, for condition 1888.
    pub plays: BTreeMap<i32, i32>,
    /// The tasks are being restored (task mode 1), for conditions 1889 and 1890.
    pub restoring: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Frame {
    /// The runner's root: returning past it ends the runner.
    Root,
    /// Children of a command; then its next sibling.
    Plain(usize),
    /// Children of a loop; then the condition again.
    Loop(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum Wait {
    Time(f32),
    Line,
    NoLine,
    Script(i32),
    /// An alarm, cleared when the wait passes if the flag is set.
    Alarm(i32, bool),
    /// `0x0D`: a rendezvous of the named runners, by meeting ID (missions.md 4.1a).
    Meet(Vec<i32>, i32),
    Runners(Vec<i32>),
}

#[derive(Clone, Debug)]
struct Runner {
    name: i32,
    pc: Option<usize>,
    stack: Vec<Frame>,
    wait: Option<Wait>,
}

/// One running script.
#[derive(Clone, Debug)]
pub struct ScriptRun {
    /// The script's object ID and the object it was started with.
    pub oid: i32,
    /// See [`ScriptRun::oid`].
    pub object: i32,
    commands: Vec<Command>,
    args: Vec<Vec<u8>>,
    runners: Vec<Runner>,
    finish: Vec<usize>,
    finishing: bool,
    orders: BTreeMap<usize, LineOrder>,
    /// The default target (`0x14`) and context object (`0x18`), −1 none.
    target: i32,
    /// See [`ScriptRun::target`]; a module's script ID 741 writes it (missions.md 4.1a).
    pub context: i32,
    /// The script object started on (its copy), when known.
    pub copy: Option<engine::object::Handle>,
    /// The script object the run started on.
    own: Option<engine::object::Handle>,
    /// Rendezvous (`0x0D`) by meeting ID: the runners arrived, and whether it has opened.
    meets: BTreeMap<i32, (BTreeSet<i32>, bool)>,
}

/// The running scripts of a world.
#[derive(Clone, Debug)]
pub struct Scripts {
    /// Running scripts.
    pub running: Vec<ScriptRun>,
    /// Alarms set.
    pub alarms: BTreeSet<i32>,
    /// What the scripts asked this frame.
    pub requests: Vec<Request>,
    /// Commands completed without being run, by ID.
    pub unhandled: BTreeMap<u8, usize>,
    /// The scripts started since the session last looked, with the script object each run started on (its own copy,
    /// [`ScriptRun::copy`] when known); the session raises event 5 on that object.
    pub started: Vec<(i32, engine::object::Handle)>,
    /// The scripts that ran to their end since the session last looked, with their script object; the session
    /// raises event 6 on it. Working reading [I]: event 5 marks a start (missions.md 4.1a), and the class-135
    /// handlers of event 6 (1,080 on the disc, scripts.md 6) act as on an end (v_interpol's 88261 sets its caller's
    /// alarm, which 2113 waits for). A stopped script (`0x11`) raises none.
    pub ended: Vec<(i32, engine::object::Handle)>,
    /// Conditions that ask a script's module (sly3/scripts.md 7.1: any value but the fixed kinds): (script, its
    /// copy, value), for the session to answer into [`Scripts::answers`] before the next frame.
    pub questions: Vec<(i32, Option<engine::object::Handle>, i32)>,
    /// See [`Scripts::questions`]: (script, value) → the module's answer, used once.
    pub answers: BTreeMap<(i32, i32), bool>,
    rng: Rng,
    stopped: BTreeSet<i32>,
    /// Alarms set on a named script (method 735): alarm → that script.
    alarm_owners: BTreeMap<i32, i32>,
    /// Alarms that waits with the clear flag passed this frame, cleared once every runner has run (working reading
    /// [I]: 2142's runners 15137 and 4904 both wait on 139312, one clearing it, and both go on, missions.md 4.1a).
    clears: Vec<i32>,
}

fn word(a: &[u8], k: usize) -> i32 {
    a.get(k * 4..k * 4 + 4).map_or(-1, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn float(a: &[u8], k: usize) -> f32 {
    f32::from_bits(word(a, k) as u32)
}

impl Scripts {
    /// No scripts; `seed` for the random waits and line orders.
    pub fn new(seed: u64) -> Scripts {
        Scripts {
            running: Vec::new(),
            alarms: BTreeSet::new(),
            requests: Vec::new(),
            unhandled: BTreeMap::new(),
            started: Vec::new(),
            ended: Vec::new(),
            questions: Vec::new(),
            answers: BTreeMap::new(),
            rng: Rng::new(seed),
            stopped: BTreeSet::new(),
            alarm_owners: BTreeMap::new(),
            clears: Vec::new(),
        }
    }

    /// Whether script `oid` runs.
    pub fn is_running(&self, oid: i32) -> bool {
        self.running.iter().any(|s| s.oid == oid)
    }

    /// Each running script's runners, for reports: (script, runner name, the command it is at, what it waits for).
    pub fn runner_states(&self) -> Vec<(i32, i32, Option<usize>, alloc::string::String)> {
        self.running
            .iter()
            .flat_map(|s| s.runners.iter().map(move |r| (s.oid, r.name, r.pc, alloc::format!("{:?}", r.wait))))
            .collect()
    }

    /// Starts script `oid` (a command script of `world`) with `object` as its object. A running script is not
    /// started twice. False when there is no such script.
    pub fn start(&mut self, world: &World, oid: i32, object: i32) -> bool {
        self.start_in(world, oid, object, None)
    }

    /// [`Scripts::start`], the script object found from `near` (its copy: a use point's command script, 1164).
    pub fn start_in(&mut self, world: &World, oid: i32, object: i32, near: Option<engine::object::Handle>) -> bool {
        if self.is_running(oid) {
            return true;
        }
        let Some(level) = world.sly3.as_deref() else { return false };
        let Some(h) =
            world.find_sly3_id(oid, near).filter(|&h| world.class(h) == Some(super::flow::COMMAND_SCRIPT_CLASS))
        else {
            return false;
        };
        let Some(r) = world.sly3_record(h) else { return false };
        let Ok((_, commands)) = command_script(&world.level.bytes, r.body_offset, &level.table) else { return false };
        if commands.first().map(|c| c.id) != Some(0x00) {
            return false;
        }
        let args = commands
            .iter()
            .map(|c| world.level.bytes[c.args.0 as usize..(c.args.0 + c.args.1) as usize].to_vec())
            .collect();
        let child = (commands[0].child & 0x7FF) as usize;
        self.started.push((oid, h));
        self.running.push(ScriptRun {
            oid,
            object,
            runners: alloc::vec![Runner {
                name: commands[0].object,
                pc: (child != 0).then_some(child),
                stack: alloc::vec![Frame::Root],
                wait: None
            }],
            commands,
            args,
            finish: Vec::new(),
            finishing: false,
            orders: BTreeMap::new(),
            target: -1,
            context: -1,
            copy: near.map(|_| h),
            own: Some(h),
            meets: BTreeMap::new(),
        });
        true
    }

    /// Runs script `oid` (already started) for one step now, out of the frame's order, with no time passing: a task's
    /// script takes its first step in the update in which the task becomes current (missions.md 4.1a). Its requests
    /// are added to [`Scripts::requests`].
    pub fn step_now(&mut self, world: Option<&World>, oid: i32, view: &View) {
        let Some(k) = self.running.iter().position(|s| s.oid == oid) else { return };
        let names: BTreeSet<i32> = self.running.iter().map(|s| s.oid).collect();
        let mut s = self.running.remove(k);
        self.step_script(world, &mut s, view, &names, 0.0);
        // The step may have stopped other scripts (`0x11`) or started some.
        let k = k.min(self.running.len());
        self.running.insert(k, s);
    }

    /// Stops script `oid` (its finish blocks are not run).
    pub fn stop(&mut self, oid: i32) {
        self.running.retain(|s| s.oid != oid);
        self.forget_alarms_of(oid);
    }

    /// Sets `alarm` on script `script` (method 735): it stays set until cleared or until that script ends.
    pub fn set_alarm_of(&mut self, script: i32, alarm: i32) {
        self.alarms.insert(alarm);
        self.alarm_owners.insert(alarm, script);
    }

    /// Whether `alarm` is set for script `oid`: set, and not set on another script.
    fn alarm_set_for(&self, oid: i32, alarm: i32) -> bool {
        self.alarms.contains(&alarm) && self.alarm_owners.get(&alarm).is_none_or(|&o| o == oid)
    }

    /// Clears the alarms set on script `oid` (it ended or was stopped).
    fn forget_alarms_of(&mut self, oid: i32) {
        let alarms = &mut self.alarms;
        self.alarm_owners.retain(|a, s| {
            let mine = *s == oid;
            if mine {
                alarms.remove(a);
            }
            !mine
        });
    }

    /// Runs every script for `dt` seconds (`world` is where `0x17` finds the scripts it starts); their requests are added to [`Scripts::requests`]. Scripts
    /// started this frame run from the next.
    pub fn update(&mut self, world: Option<&World>, view: &View, dt: f32) {
        let names: BTreeSet<i32> = self.running.iter().map(|s| s.oid).collect();
        let mut scripts = core::mem::take(&mut self.running);
        for s in &mut scripts {
            self.step_script(world, s, view, &names, dt);
        }
        for a in core::mem::take(&mut self.clears) {
            self.alarms.remove(&a);
            self.alarm_owners.remove(&a);
        }
        let stopped = core::mem::take(&mut self.stopped);
        for s in scripts.iter().filter(|s| s.runners.is_empty() && !stopped.contains(&s.oid)) {
            if let Some(h) = s.own {
                self.ended.push((s.oid, h));
            }
        }
        for s in scripts.iter().filter(|s| s.runners.is_empty() || stopped.contains(&s.oid)) {
            self.forget_alarms_of(s.oid);
        }
        scripts.retain(|s| !s.runners.is_empty() && !stopped.contains(&s.oid));
        // Scripts started this frame were pushed onto `self.running`.
        scripts.append(&mut self.running);
        self.running = scripts;
    }

    fn step_script(&mut self, world: Option<&World>, s: &mut ScriptRun, view: &View, scripts: &BTreeSet<i32>, dt: f32) {
        let mut i = 0;
        while i < s.runners.len() {
            let mut r = s.runners[i].clone();
            let spawned = self.step_runner(world, s, &mut r, view, scripts, dt);
            s.runners[i] = r;
            s.runners.extend(spawned);
            i += 1;
        }
        s.runners.retain(|r| !r.stack.is_empty());
        if s.runners.is_empty() && !s.finishing && !s.finish.is_empty() {
            s.finishing = true;
            for &c in &s.finish.clone() {
                s.runners.push(Runner { name: -1, pc: Some(c), stack: alloc::vec![Frame::Root], wait: None });
            }
        }
    }

    /// A condition's value, or `None` while the script's module is being asked (the runner tries again next frame).
    fn condition(&mut self, s: &ScriptRun, kind: i32, object: i32, view: &View) -> Option<bool> {
        Some(match kind {
            cond::TRUE => true,
            // The object tests read tasks (the play count and mode the job system keeps); another object: false.
            cond::FIRST_TIME => view.plays.get(&object) == Some(&1),
            cond::LIVE => view.plays.contains_key(&object) && !view.restoring,
            cond::RESTORING => view.plays.contains_key(&object) && view.restoring,
            // An alarm of this script (scripts.md 7.4: the condition's second word names it; none named: any).
            cond::ALARM_SET | cond::ALARM_CLEAR => {
                let set = if object == -1 { !self.alarms.is_empty() } else { self.alarm_set_for(s.oid, object) };
                set == (kind == cond::ALARM_SET)
            }
            cond::NO_LINE => !view.line_playing,
            cond::LINE => view.line_playing,
            // Any other value asks the script's own module (scripts.md 7.1).
            k if !(cond::TRUE..=cond::LINE).contains(&k) => {
                if let Some(b) = self.answers.remove(&(s.oid, k)) {
                    b
                } else {
                    if !self.questions.iter().any(|q| q.0 == s.oid && q.2 == k) {
                        self.questions.push((s.oid, s.copy, k));
                    }
                    return None;
                }
            }
            _ => false,
        })
    }

    fn waited(&self, w: &Wait, s: &ScriptRun, view: &View, scripts: &BTreeSet<i32>, dt: f32) -> Option<Wait> {
        match w {
            Wait::Time(t) if *t > dt => Some(Wait::Time(t - dt)),
            Wait::Line if view.line_playing => Some(Wait::Line),
            Wait::NoLine if view.line_playing => Some(Wait::NoLine),
            Wait::Script(id)
                if !self.stopped.contains(id)
                    && (scripts.contains(id) || self.running.iter().any(|r| r.oid == *id)) =>
            {
                Some(w.clone())
            }
            Wait::Alarm(a, _) if !self.alarm_set_for(s.oid, *a) => Some(w.clone()),
            Wait::Runners(n) if s.runners.iter().any(|r| n.contains(&r.name) && !r.stack.is_empty()) => Some(w.clone()),
            _ => None,
        }
    }

    /// Steps one runner; returns the runners it spawned.
    fn step_runner(
        &mut self,
        world: Option<&World>,
        s: &mut ScriptRun,
        r: &mut Runner,
        view: &View,
        scripts: &BTreeSet<i32>,
        dt: f32,
    ) -> Vec<Runner> {
        let mut spawned = Vec::new();
        let link = |v: u16| -> Option<usize> { Some((v & 0x7FF) as usize).filter(|&x| x != 0) };
        for _ in 0..STEP_LIMIT {
            // A rendezvous opens once every named runner still running waits at the same meeting; all then go on.
            if let Some(Wait::Meet(names, id)) = &r.wait {
                let live: Vec<i32> = names
                    .iter()
                    .copied()
                    .filter(|&n| n == r.name || s.runners.iter().any(|o| o.name == n && !o.stack.is_empty()))
                    .collect();
                let m = s.meets.entry(*id).or_default();
                if !(m.1 || live.iter().all(|n| m.0.contains(n))) {
                    return spawned;
                }
                m.1 = true;
                m.0.remove(&r.name);
                if m.0.is_empty() {
                    s.meets.remove(id);
                }
                r.wait = None;
            }
            if let Some(w) = r.wait.take() {
                // Runners this runner has just started count as running for its `0x0C` (they join next frame).
                let started = matches!(&w, Wait::Runners(n) if spawned.iter().any(|x: &Runner| n.contains(&x.name)));
                r.wait = if started { Some(w.clone()) } else { self.waited(&w, s, view, scripts, dt) };
                if r.wait.is_some() {
                    return spawned;
                }
                if let Wait::Alarm(a, true) = w {
                    self.clears.push(a);
                }
            }
            let Some(i) = r.pc else {
                match r.stack.pop() {
                    None | Some(Frame::Root) => {
                        r.stack.clear();
                        return spawned;
                    }
                    Some(Frame::Plain(p)) => r.pc = s.commands.get(p).and_then(|c| link(c.sibling)),
                    Some(Frame::Loop(l)) => {
                        let c = s.commands[l];
                        match self.condition(s, word(&s.args[l], 0), word(&s.args[l], 1), view) {
                            Some(true) => {
                                r.stack.push(Frame::Loop(l));
                                r.pc = link(c.child);
                            }
                            Some(false) => r.pc = link(c.sibling),
                            None => {
                                // Asked: the loop's test runs again next frame.
                                r.stack.push(Frame::Loop(l));
                                r.pc = None;
                                r.wait = Some(Wait::Time(0.0));
                            }
                        }
                        // One pass of a loop per frame at most.
                        return spawned;
                    }
                }
                continue;
            };
            let Some(&c) = s.commands.get(i) else {
                r.pc = None;
                continue;
            };
            let a = s.args[i].clone();
            let next = link(c.sibling);
            let wait = c.flags & 1 != 0;
            r.pc = next;
            match c.id {
                0x00 => {
                    if let Some(ch) = link(c.child) {
                        spawned.push(Runner {
                            name: c.object,
                            pc: Some(ch),
                            stack: alloc::vec![Frame::Root],
                            wait: None,
                        });
                    }
                }
                0x02 => {
                    if let Some(ch) = link(c.child) {
                        s.finish.push(ch);
                    }
                }
                0x04 => match self.condition(s, word(&a, 0), word(&a, 1), view) {
                    Some(true) => {
                        r.stack.push(Frame::Loop(i));
                        r.pc = link(c.child);
                    }
                    Some(false) => {}
                    None => {
                        r.pc = Some(i);
                        r.wait = Some(Wait::Time(0.0));
                        return spawned;
                    }
                },
                0x06 => {
                    let mut b = link(c.child);
                    let mut asked = false;
                    while let Some(k) = b {
                        let bc = s.commands[k];
                        let take = match bc.id {
                            0x07 | 0x08 => match self.condition(s, word(&s.args[k], 0), word(&s.args[k], 1), view) {
                                Some(t) => t,
                                None => {
                                    asked = true;
                                    break;
                                }
                            },
                            0x09 => true,
                            _ => false,
                        };
                        if take {
                            r.stack.push(Frame::Plain(i));
                            r.pc = link(bc.child);
                            break;
                        }
                        b = link(bc.sibling);
                    }
                    if asked {
                        // A branch's test asked the module: the block runs again next frame.
                        r.pc = Some(i);
                        r.wait = Some(Wait::Time(0.0));
                        return spawned;
                    }
                }
                0x0B => {
                    let name = word(&a, 0);
                    if name == -1 || name == r.name {
                        r.stack.clear();
                        r.pc = None;
                        return spawned;
                    }
                    for o in s.runners.iter_mut().filter(|o| o.name == name) {
                        o.stack.clear();
                        o.pc = None;
                    }
                }
                0x0C => {
                    let n = a.first().map_or(0, |&n| (n as i8).max(0) as usize);
                    let names: Vec<i32> = (0..n).map(|k| word(a.get(1..).unwrap_or(&[]), k)).collect();
                    r.wait = Some(Wait::Runners(names));
                }
                0x0D => {
                    // The names as 0x0C has them (a count, then the names), then the meeting ID.
                    let n = a.first().map_or(0, |&n| (n as i8).max(0) as usize);
                    let names: Vec<i32> = (0..n).map(|k| word(a.get(1..).unwrap_or(&[]), k)).collect();
                    let id = word(a.get(1..).unwrap_or(&[]), n);
                    s.meets.entry(id).or_default().0.insert(r.name);
                    r.wait = Some(Wait::Meet(names, id));
                    return spawned;
                }
                // A non-zero second argument clears the alarm once the wait passes (missions.md 4.1a).
                0x0E => r.wait = Some(Wait::Alarm(word(&a, 0), a.get(4).is_some_and(|&b| b != 0))),
                0x0F => {
                    self.alarms.insert(word(&a, 0));
                }
                0x10 => {
                    self.alarms.remove(&word(&a, 0));
                    self.alarm_owners.remove(&word(&a, 0));
                }
                0x11 => {
                    let script = word(&a, 0);
                    self.running.retain(|x| x.oid != script);
                    self.stopped.insert(script);
                }
                0x17 => {
                    let script = word(&a, 0);
                    let object = if c.object != -1 { c.object } else { s.object };
                    let running = scripts.contains(&script) && !self.stopped.contains(&script);
                    let started = running || world.is_some_and(|w| self.start(w, script, object));
                    if wait && started {
                        r.wait = Some(Wait::Script(script));
                    }
                }
                0x19 => self.requests.push(Request::Goal {
                    object: word(&a, 0),
                    machine: word(&a, 1),
                    goal: word(&a, 2),
                    snap: a.get(12).is_some_and(|&b| b != 0),
                }),
                0x1A | 0x1B => self.requests.push(Request::Screen { screen: word(&a, 0), on: c.id == 0x1A }),
                0x1E => {
                    self.requests.push(Request::TaskState { task: word(&a, 0), state: *a.get(4).unwrap_or(&0) as i32 })
                }
                0x2F => {
                    let (lo, hi) = (float(&a, 0), float(&a, 1));
                    let t = if hi > lo { self.rng.range_f32(lo, hi) } else { lo };
                    if wait && t > 0.0 {
                        r.wait = Some(Wait::Time(t));
                    }
                }
                0x38 | 0x39 => self.requests.push(Request::Present { object: word(&a, 0), on: c.id == 0x38 }),
                0x52 => {
                    if let Some(d) = DialogueCommand::decode(c.id, &a) {
                        let DialogueCommand::PlayLine { lines, .. } = &d else { unreachable!() };
                        let rng = &mut self.rng;
                        let order = s.orders.entry(i).or_insert_with(|| LineOrder::new(lines));
                        if let Some(line) = order.next(rng) {
                            self.requests.push(Request::PlayLine { line, command: d, object: c.object });
                            if wait {
                                r.wait = Some(Wait::Line);
                            }
                        }
                    }
                }
                0x53 => {
                    if let Some(DialogueCommand::Stop { list }) = DialogueCommand::decode(c.id, &a) {
                        self.requests.push(Request::StopLine { list });
                    }
                }
                0x54 => self.requests.push(Request::ClearSubtitle),
                0x59 => self.requests.push(Request::Music {
                    push: Some(i16::from_le_bytes([*a.first().unwrap_or(&0), *a.get(1).unwrap_or(&0)])),
                }),
                0x5A => self.requests.push(Request::Music { push: None }),
                0x5F => r.wait = Some(Wait::NoLine),
                0x68 => self.requests.push(Request::FireExit { exit: word(&a, 0), point: word(&a, 1) }),
                0x14 => s.target = word(&a, 0),
                0x18 => s.context = word(&a, 0),
                0x12 => self.requests.push(Request::Activate { object: word(&a, 0) }),
                0x69 => self.requests.push(Request::ReturnToWaypoint),
                0x13 => {
                    self.requests.push(Request::Control { object: word(&a, 0), player: *a.get(4).unwrap_or(&0) as i32 })
                }
                0x15 => self.requests.push(Request::Module { script: s.oid, value: word(&a, 0), copy: s.copy }),
                0x60 => self.requests.push(Request::Hint { string: word(&a, 0), seconds: float(&a, 1) }),
                0x61 => self.requests.push(Request::ClearHint),
                0x1F => self.requests.push(Request::Pause { script: s.oid, on: a.first().is_some_and(|&b| b != 0) }),
                // 0x36 moves a character to an object (missions.md 4.1a), arriving at once here like 0x30.
                0x22 | 0x30 | 0x36 => {
                    let target = [c.object, s.context, s.target, s.object].into_iter().find(|&o| o != -1).unwrap_or(-1);
                    self.requests.push(Request::Place { target, at: word(&a, 0) });
                }
                0x6F | 0x70 => self.requests.push(Request::Instance { placement: word(&a, 0), take: c.id == 0x6F }),
                0x77 => {
                    let target = [c.object, s.context, s.target, s.object].into_iter().find(|&o| o != -1).unwrap_or(-1);
                    self.requests.push(Request::Face { target, at: word(&a, 0) });
                }
                0x16 => {
                    self.requests.push(Request::Segment {
                        segment: word(&a, 0),
                        object: word(&a, 1),
                        flags: a.get(8).copied().unwrap_or(0),
                    });
                    *self.unhandled.entry(c.id).or_default() += 1;
                }
                0x45 => {
                    // The key's time: the third float after the two vectors.
                    let t = float(&a, 8);
                    if wait && t.is_finite() && t > 0.0 {
                        r.wait = Some(Wait::Time(t));
                    }
                }
                0x55 | 0x56 => {}
                // The binocucom (interactables.md 5a): the character (24 Sly), the object looked at (the eleventh
                // word; working reading of the layout).
                0x46 => {
                    self.requests.push(Request::Binocucom { open: true, character: word(&a, 0), look_at: word(&a, 10) })
                }
                0x6E => self.requests.push(Request::Binocucom { open: false, character: -1, look_at: -1 }),
                0x62 => self.requests.push(Request::GuardList { guard: word(&a, 0), list: word(&a, 1) }),
                id => *self.unhandled.entry(id).or_default() += 1,
            }
            // A command with children of its own (other than the blocks above) runs them next.
            if !matches!(c.id, 0x00 | 0x02 | 0x04 | 0x06)
                && let Some(ch) = link(c.child)
            {
                r.stack.push(Frame::Plain(i));
                r.pc = Some(ch);
            }
        }
        spawned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(id: u8, child: u16, sibling: u16, flags: i8) -> Command {
        Command { id, object: -1, flags, child, sibling, args: (0, 0) }
    }

    fn run(commands: Vec<Command>, args: Vec<Vec<u8>>) -> ScriptRun {
        let child = commands[0].child as usize;
        ScriptRun {
            oid: 1,
            object: -1,
            runners: alloc::vec![Runner { name: -1, pc: Some(child), stack: alloc::vec![Frame::Root], wait: None }],
            commands,
            args,
            finish: Vec::new(),
            finishing: false,
            orders: BTreeMap::new(),
            target: -1,
            context: -1,
            copy: None,
            own: None,
            meets: BTreeMap::new(),
        }
    }

    fn ww(a: i32, b: i32) -> Vec<u8> {
        let mut v = a.to_le_bytes().to_vec();
        v.extend_from_slice(&b.to_le_bytes());
        v
    }

    #[test]
    fn a_tree_runs_in_order_with_a_branch_and_a_finish_block() {
        // 0 root → 1 exit A → 2 if (3 branch false → 4 exit B; 5 else → 6 exit C) → 7 finish (8 exit D).
        let c = alloc::vec![
            cmd(0x00, 1, 0, 0),
            cmd(0x68, 0, 2, 0),
            cmd(0x06, 3, 7, 0),
            cmd(0x07, 4, 5, 0),
            cmd(0x68, 0, 0, 0),
            cmd(0x09, 6, 0, 0),
            cmd(0x68, 0, 0, 0),
            cmd(0x02, 8, 0, 0),
            cmd(0x68, 0, 0, 0),
        ];
        let a = alloc::vec![
            alloc::vec![],
            ww(10, -1),
            alloc::vec![],
            ww(cond::FALSE, -1),
            ww(20, -1),
            alloc::vec![],
            ww(30, -1),
            alloc::vec![],
            ww(40, 7),
        ];
        let mut s = Scripts::new(1);
        s.running.push(run(c, a));
        s.update(None, &View::default(), 0.1);
        s.update(None, &View::default(), 0.1);
        let exits: Vec<(i32, i32)> = s
            .requests
            .iter()
            .filter_map(|r| match r {
                Request::FireExit { exit, point } => Some((*exit, *point)),
                _ => None,
            })
            .collect();
        assert_eq!(exits, [(10, -1), (30, -1), (40, 7)]);
        assert!(s.running.is_empty(), "the script ended");
    }

    #[test]
    fn waits_hold_the_runner() {
        // 0 root → 1 wait 0.5 s (flag) → 2 exit.
        let mut wait = 0.5f32.to_le_bytes().to_vec();
        wait.extend_from_slice(&0.5f32.to_le_bytes());
        wait.extend_from_slice(&(-1i32).to_le_bytes());
        let c = alloc::vec![cmd(0x00, 1, 0, 0), cmd(0x2F, 0, 2, 1), cmd(0x68, 0, 0, 0)];
        let mut s = Scripts::new(1);
        s.running.push(run(c, alloc::vec![alloc::vec![], wait, ww(5, -1)]));
        for _ in 0..4 {
            s.update(None, &View::default(), 0.1);
        }
        assert!(s.requests.is_empty(), "still waiting after 0.4 s");
        for _ in 0..2 {
            s.update(None, &View::default(), 0.1);
        }
        assert_eq!(s.requests, [Request::FireExit { exit: 5, point: -1 }]);
    }

    #[test]
    fn a_loop_spawns_and_stops_a_runner_each_pass_until_its_alarm() {
        // 0 root → 1 loop (true) [2 runner 7 (3 exit 1) → 4 wait alarm 9 → 5 stop runner 7].
        let mut c = alloc::vec![
            cmd(0x00, 1, 0, 0),
            cmd(0x04, 2, 0, 0),
            cmd(0x00, 3, 4, 0),
            cmd(0x68, 0, 0, 0),
            cmd(0x0E, 0, 5, 1),
            cmd(0x0B, 0, 0, 0),
        ];
        c[2].object = 7;
        let a = alloc::vec![
            alloc::vec![],
            ww(cond::TRUE, -1),
            alloc::vec![],
            ww(1, -1),
            ww(9, 0),
            7i32.to_le_bytes().to_vec(),
        ];
        let mut s = Scripts::new(1);
        s.running.push(run(c, a));
        s.update(None, &View::default(), 0.1);
        s.update(None, &View::default(), 0.1);
        assert_eq!(s.requests.len(), 1, "one pass: the runner fired once, the loop waits for the alarm");
        s.alarms.insert(9);
        for _ in 0..3 {
            s.update(None, &View::default(), 0.1);
        }
        assert!(s.requests.len() >= 2, "the alarm let the loop go round");
        assert_eq!(s.running.len(), 1, "a forever loop keeps the script");
    }
}
