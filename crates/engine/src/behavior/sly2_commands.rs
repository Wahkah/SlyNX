//! Sly 2 sequence starts and guard command lists (specs/sly2/dialogue-and-voice.md 2.1, options-by-class.md 2.1
//! to 2.3), and the generic object actions of options-by-class.md 2.1.
//!
//! - **Sequences** (class 114): option 613 starts one with a starter (its argument; none without one), 614 with the
//!   current character as starter, 615 stops it. The requests go to `World::shared.sequence_requests` for the game's
//!   sequence player. Trigger volumes start the sequence of option 441 with the entering object, interaction spots
//!   (class 35) that of option 1070 with the character using it (the gang system), and guard command lists signal
//!   sequences (command 15).
//! - **Guard command lists** (class 113): options 595 to 612 each append one command, in stream order at load and
//!   when a script calls them. A guard told to run a list ([`run_commands`]), or running its patrol route (option
//!   273), works through it, waiting on commands 5 (wait), 6 and 13 (follow a path, a ladder, pipe or rail: along the
//!   curve, start to end with direction flag 1, facing along it), 7 (go to) and 14 (play a clip; done at once until
//!   Sly 2 clips play on guards). Commands 1 and 2 set the random run and pause times: an idle guard alternates
//!   between running its list for a run time and standing still for a pause time.
//! - **Generic actions** on placed objects: 20 set the parent (keep flags: 0 nothing, 3 world position and rotation,
//!   15 also velocities), 24 move to another object's place, 25 add to the world, 44 / 45 show / hide the meshes
//!   with a mesh object ID (kept in `World::shared.hidden_meshes` for drawing).

use alloc::vec::Vec;

use super::{ClassBehavior, Cx};
use crate::object::{Arg, Handle};
use crate::world::World;

/// A request to the sequence player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceRequest {
    /// Start a sequence with a starter (the commands' default target), or none.
    Start {
        /// The sequence object.
        sequence: Handle,
        /// The starter.
        starter: Option<Handle>,
    },
    /// Start a sequence with the character the player controls as starter.
    StartWithPlayer(Handle),
    /// Stop a sequence.
    Stop(Handle),
    /// Raise a numbered alarm on a sequence (guard command 15).
    Alarm(Handle, i32),
}

/// Option IDs.
pub mod opt {
    /// Sequences: start with a starter, start with the current character, stop.
    pub const START: u16 = 613;
    /// See [`START`].
    pub const START_WITH_PLAYER: u16 = 614;
    /// See [`START`].
    pub const STOP: u16 = 615;
    /// Trigger volume: the sequence it starts.
    pub const VOLUME_SEQUENCE: u16 = 441;
    /// Interaction spot: the sequence it starts.
    pub const SPOT_SEQUENCE: u16 = 1070;
    /// Generic actions.
    pub const SET_PARENT: u16 = 20;
    /// See [`SET_PARENT`].
    pub const MOVE_TO: u16 = 24;
    /// See [`SET_PARENT`].
    pub const ADD: u16 = 25;
    /// See [`SET_PARENT`].
    pub const SHOW_MESH: u16 = 44;
    /// See [`SET_PARENT`].
    pub const HIDE_MESH: u16 = 45;
    /// First and last guard command options.
    pub const FIRST_COMMAND: u16 = 595;
    /// See [`FIRST_COMMAND`].
    pub const LAST_COMMAND: u16 = 612;
}

fn object(world: &World, me: Handle, a: &Arg) -> Option<Handle> {
    match *a {
        Arg::Int(id) => world.find_sly2_id(id, Some(me)),
        Arg::Short(6, oid) if oid >= 0 => world.find_nearest(oid, Some(me)),
        _ => None,
    }
}

fn number(a: &Arg) -> Option<f32> {
    match *a {
        Arg::Float(f) => Some(f),
        Arg::Int(n) => Some(n as f32),
        Arg::Short(_, n) => Some(f32::from(n)),
        Arg::Bool(b) => Some(b as i32 as f32),
        _ => None,
    }
}

/// The generic object actions (options-by-class.md 2.1) on a Sly 2 world; true when handled.
pub fn generic_action(world: &mut World, h: Handle, id: u16, args: &[Arg]) -> bool {
    match id {
        opt::SET_PARENT => {
            let parent = args.first().and_then(|a| object(world, h, a));
            let keep = args.get(1).and_then(number).unwrap_or(0.0) as i32;
            world.reparent(h, parent, keep & 3 != 0);
            true
        }
        opt::MOVE_TO => {
            if let Some(t) = args.first().and_then(|a| object(world, h, a)).and_then(|t| world.world_transform(t)) {
                world.set_world_transform(h, t);
            }
            true
        }
        opt::ADD => {
            world.set_in_world(h, true);
            true
        }
        opt::SHOW_MESH | opt::HIDE_MESH => {
            if let Some(mesh) = args.first().and_then(number).map(|n| n as i32) {
                let key = (h.index(), mesh);
                let hidden = &mut world.shared.hidden_meshes;
                hidden.retain(|&k| k != key);
                if id == opt::HIDE_MESH {
                    hidden.push(key);
                }
            }
            true
        }
        _ => false,
    }
}

/// A sequence object (class 114): takes the script calls that start and stop it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sequence {
    /// Times started.
    pub started: u32,
}

impl ClassBehavior for Sequence {
    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[Arg]) -> bool {
        let r = match id {
            opt::START => {
                let starter = args.first().and_then(|a| object(world, me, a));
                SequenceRequest::Start { sequence: me, starter }
            }
            opt::START_WITH_PLAYER => SequenceRequest::StartWithPlayer(me),
            opt::STOP => SequenceRequest::Stop(me),
            _ => return false,
        };
        if id != opt::STOP {
            self.started += 1;
        }
        world.shared.sequence_requests.push(r);
        true
    }
}

/// One guard command (options-by-class.md 2.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Command number (0 to 15).
    pub kind: u8,
    /// Its arguments.
    pub args: Vec<Arg>,
}

/// A guard command list (class 113).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandList {
    /// The commands, in order.
    pub commands: Vec<Command>,
}

/// The command number of a command option (595 to 612).
pub fn command_kind(id: u16) -> Option<u8> {
    Some(match id {
        595 => 0,
        596 => 1,
        597 => 2,
        598 => 3,
        599 => 4,
        600 => 5,
        601 => 6,
        602 => 7,
        603 => 8,
        604 => 9,
        605 => 10,
        607 => 11,
        608 => 12,
        606 => 13,
        611 => 14,
        612 => 15,
        _ => return None,
    })
}

impl CommandList {
    /// The list of object `h`, from its own options (actions, in stream order).
    pub fn new(world: &World, h: Handle) -> CommandList {
        let commands = world
            .sly2
            .as_deref()
            .map(|l| {
                l.actions
                    .iter()
                    .filter(|a| a.0 == h.index())
                    .filter_map(|a| command_kind(a.1).map(|kind| Command { kind, args: a.2.to_vec() }))
                    .collect()
            })
            .unwrap_or_default();
        CommandList { commands }
    }
}

impl ClassBehavior for CommandList {
    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, args: &[Arg]) -> bool {
        match command_kind(id) {
            Some(kind) => {
                self.commands.push(Command { kind, args: args.to_vec() });
                true
            }
            None => false,
        }
    }
}

/// A guard working through a command list.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandRun {
    /// The list.
    pub list: Handle,
    /// The next command.
    pub at: usize,
    /// Seconds left on a wait.
    pub wait: f32,
    /// The context object (command 0).
    pub context: Option<Handle>,
    /// Run and pause time ranges (commands 1 and 2).
    pub run_range: Option<[f32; 2]>,
    /// See [`CommandRun::run_range`].
    pub pause_range: Option<[f32; 2]>,
    /// The path being followed (commands 6 and 13) and the next point on it.
    pub path: Vec<[f32; 3]>,
    /// See [`CommandRun::path`].
    pub path_at: usize,
}

impl CommandRun {
    /// A run of `list` from its start.
    pub fn new(list: Handle) -> CommandRun {
        CommandRun {
            list,
            at: 0,
            wait: 0.0,
            context: None,
            run_range: None,
            pause_range: None,
            path: Vec::new(),
            path_at: 0,
        }
    }
}

/// Tells guard `guard` to run command list `list` from its start.
pub fn run_commands(world: &mut World, guard: Handle, list: Handle) -> bool {
    match world.behavior_mut(guard) {
        Some(super::Behavior::Sly2Guard(g)) => {
            g.commands = Some(CommandRun::new(list));
            true
        }
        _ => false,
    }
}

/// Speed along a followed path. *Placeholder*: the guard's gait sets it (root motion, not read yet).
pub const PATH_SPEED: f32 = 250.0;

/// What a running command asks of the guard this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Nothing (the list ended or waits).
    Idle,
    /// Turn to look at a point.
    Look([f32; 3]),
    /// Walk to a point at a speed; the list waits until arrival.
    GoTo([f32; 3], f32),
}

/// Runs a guard's list until a command waits. Returns what the guard should do now, and whether the run goes on.
pub fn step_commands(cx: &mut Cx<'_>, run: &mut CommandRun, arrived: bool) -> (Step, bool) {
    let commands = match cx.world.behavior(run.list) {
        Some(super::Behavior::Sly2Commands(l)) => l.commands.clone(),
        _ => return (Step::Idle, false),
    };
    let mut guard = 0;
    while let Some(c) = commands.get(run.at) {
        guard += 1;
        if guard > 64 {
            return (Step::Idle, true);
        }
        let me = cx.me;
        match c.kind {
            0 => run.context = c.args.first().and_then(|a| object(cx.world, me, a)),
            1 | 2 => {
                let r = [c.args.first().and_then(number).unwrap_or(0.0), c.args.get(1).and_then(number).unwrap_or(0.0)];
                if c.kind == 1 {
                    run.run_range = Some(r);
                } else {
                    run.pause_range = Some(r);
                }
            }
            6 | 13 => {
                // Follow the path (a curve: a route, ladder, pipe or rail) point by point.
                if run.path.is_empty() && run.path_at == 0 {
                    let path = c.args.first().and_then(|a| object(cx.world, me, a));
                    let mut pts = path.map(|p| super::sly2_sensors::sly2_curve(cx.world, p)).unwrap_or_default();
                    let forward = c.kind == 13 || c.args.get(1).and_then(number).is_none_or(|d| d == 1.0);
                    if !forward {
                        pts.reverse();
                    }
                    run.path = pts;
                }
                if arrived {
                    run.path_at += 1;
                }
                if let Some(&p) = run.path.get(run.path_at) {
                    return (Step::GoTo(p, PATH_SPEED), true);
                }
                run.path.clear();
                run.path_at = 0;
            }
            3 => {
                let at = c.args.first().and_then(|a| object(cx.world, me, a)).and_then(|t| cx.world.position(t));
                run.at += 1;
                if let Some(p) = at {
                    return (Step::Look(p), true);
                }
                continue;
            }
            5 => {
                if run.wait <= 0.0 {
                    run.wait = c.args.first().and_then(number).unwrap_or(0.0);
                }
                run.wait -= cx.dt;
                if run.wait > 0.0 {
                    return (Step::Idle, true);
                }
                run.wait = 0.0;
            }
            7 => {
                let target = c.args.first().and_then(|a| object(cx.world, me, a)).and_then(|t| cx.world.position(t));
                let speed = c.args.get(1).and_then(number).unwrap_or(40.0);
                if let Some(p) = target
                    && !arrived
                {
                    return (Step::GoTo(p, speed), true);
                }
            }
            10 => {
                let label = c.args.first().and_then(number);
                if let Some(i) = commands.iter().position(|k| k.kind == 9 && k.args.first().and_then(number) == label) {
                    run.at = i + 1;
                    continue;
                }
            }
            15 => {
                if let Some(s) = c.args.first().and_then(|a| object(cx.world, me, a)) {
                    let n = c.args.get(1).and_then(number).unwrap_or(0.0) as i32;
                    cx.world.shared.sequence_requests.push(SequenceRequest::Alarm(s, n));
                }
            }
            _ => {}
        }
        run.at += 1;
    }
    (Step::Idle, false)
}
