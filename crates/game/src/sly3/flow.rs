//! Sly 3 level flow (specs/sly3/level-flow.md): starting an episode, exits, arrival points, restarts and the
//! restart wipe. It mirrors the Sly 2 job system's split: what the world's records say is read into plain data here
//! ([`ExitInfo`], [`ExitSources`], [`ArrivalInfo`]); the session runs it ([`Flow`]) and performs the load it asks
//! for at the top of the next frame.
//!
//! - **Episodes** (section 3): the first world of each episode from the executable's table at `0x2EDFB0`, with the
//!   China, pirate, menu and Kaine Island exceptions ([`episode_start`]).
//! - **Exits** (class 24, section 4): a trigger volume, a destination world and movies. The walk-in state machine
//!   (0 inside since the start, 1 outside, 3 armed, 4 fired, 5 load requested) runs on the current player; a fire
//!   request (job option 1381, task option 1406, command `0x68` with its arrival point, option 840) is honoured only
//!   when armed. Exits load at point 424 with no wipe unless the firing names a point.
//! - **Arrival points** (class 40, section 5): matched by object ID, or by alias (option 843 answers 424, 844
//!   answers 425); option 848 places objects at markers on arrival, 847 applies a (task, state) pair.
//! - **Restart** (section 7): the current world reloads at the stored checkpoint with wipe 875, which plays the next
//!   of three out/in pairs from a shuffled order ([`WipeOrder`]).
//!
//! Not here yet: the Sly 3 job and task system (it will call [`Flow::fire`] for options 1381 and 1406), the command
//! runner (command `0x68`), movies (the request carries their identities), and option 845's script.

use alloc::vec::Vec;

use audio::rng::Rng;
use engine::collision::TriMesh;
use engine::math::affine_inverse;
use engine::object::{Arg, Handle};
use engine::world::World;
use formats::geometry::Affine;
use formats_sly3::stream::exit_body;

/// The exit class.
pub const EXIT_CLASS: i16 = 24;
/// The arrival-point class.
pub const ARRIVAL_CLASS: i16 = 40;
/// The warp-point class.
pub const WARP_CLASS: i16 = 39;
/// The default arrival point.
pub const DEFAULT_POINT: i32 = 424;
/// The second default point.
pub const SECOND_POINT: i32 = 425;
/// The job class (option 1381) and the task class (options 1400, 1406).
pub const JOB_CLASS: i16 = 168;
/// See [`JOB_CLASS`].
pub const TASK_CLASS: i16 = 169;
/// The command-script class.
pub const COMMAND_SCRIPT_CLASS: i16 = 135;
/// Command `0x68`: fire an exit (`s32` exit, `s32` point).
pub const FIRE_EXIT: u8 = 0x68;
/// The restart wipe.
pub const RESTART_WIPE: i32 = 875;
/// Its three out/in animation pairs (the table at `0x3297A0`).
pub const WIPE_PAIRS: [(i32, i32); 3] = [(880, 881), (882, 883), (884, 885)];
/// The first-world table: 9 `s32`, indexed by episode.
pub const FIRST_WORLD_VA: u32 = 0x002E_DFB0;
/// Slots of the episode-start exceptions.
pub const SLOT_C_INTRO: usize = 24;
/// See [`SLOT_C_INTRO`].
pub const SLOT_P_AT_SEA: usize = 32;
/// See [`SLOT_C_INTRO`].
pub const SLOT_M_EXT: usize = 35;
/// The prologue task (Kaine Island, before the title).
pub const PROLOGUE_TASK: i32 = 1798;
/// Episode 8's task when its first progress value is 0 or −1.
pub const KAINE_TASK: i32 = 1841;

/// Option IDs (level-flow.md 4.2, 5.2, 5.3, 4.4).
pub mod opt {
    /// Exit: the arrival point in the destination.
    pub const EXIT_POINT: u16 = 834;
    /// Exit: the wipe object.
    pub const EXIT_WIPE: u16 = 835;
    /// Exit: seconds outside before it arms.
    pub const ARM_DELAY: u16 = 836;
    /// Exit: delay between firing and loading.
    pub const LOAD_DELAY: u16 = 839;
    /// Exit: fire (method).
    pub const FIRE: u16 = 840;
    /// Arrival point: also answers to 424.
    pub const ALIAS_424: u16 = 843;
    /// Arrival point: also answers to 425.
    pub const ALIAS_425: u16 = 844;
    /// Arrival point: a (task, state) pair applied on arrival.
    pub const TASK_STATE: u16 = 847;
    /// Arrival point: place the first object at the second on arrival.
    pub const PLACE: u16 = 848;
    /// Arrival point: a command script of the episode's macro group (interpretation: run when the point is
    /// applied).
    pub const ARRIVAL_SCRIPT: u16 = 845;
    /// Task: its checkpoint (an arrival point).
    pub const CHECKPOINT: u16 = 1400;
    /// Job: the exit fired when it ends with no next task.
    pub const JOB_EXIT: u16 = 1381;
    /// Task: the exit fired when it becomes current.
    pub const TASK_EXIT: u16 = 1406;
}

// ---- Episodes ---------------------------------------------------------------------------------------------------

/// The first world of each episode (world slots; −1 none), from the executable.
pub fn first_worlds(exe: &formats_sly3::exe::Executable) -> formats::Result<[i32; 9]> {
    let raw = exe.bytes_at(FIRST_WORLD_VA, 36)?;
    Ok(core::array::from_fn(|i| i32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]])))
}

/// The progress values the episode start reads (level-flow.md 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EpisodeProgress {
    /// `t4_recon_complete` / `is_complete`.
    pub china_recon_complete: bool,
    /// `t5_treasure_complete` / `is_complete`.
    pub treasure_complete: bool,
    /// `t5_heist_complete` / `is_complete`.
    pub heist_complete: bool,
    /// The save test of episode 1 (interpretation: the opening has been played).
    pub opening_played: bool,
    /// Episode 8's first progress value (task 1841 when 0 or −1).
    pub kaine_value: i32,
}

/// Where an episode starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpisodeStart {
    /// The world slot.
    pub slot: usize,
    /// The arrival point.
    pub point: i32,
    /// The task started there, if any (its world is the slot; its checkpoint, when the world holds it, the point).
    pub task: Option<i32>,
}

/// Starting episode `episode` (1 to 8; level-flow.md 3, the routine at `0x289198`). `None` for an episode with no
/// first world, or episode 8 when its task is not established (a first value other than 0 and −1).
pub fn episode_start(first: &[i32; 9], episode: usize, p: &EpisodeProgress) -> Option<EpisodeStart> {
    let mut slot = usize::try_from(*first.get(episode)?).ok()?;
    let mut task = None;
    match episode {
        6 if !p.china_recon_complete => slot = SLOT_C_INTRO,
        7 if p.treasure_complete && !p.heist_complete => slot = SLOT_P_AT_SEA,
        1 if !p.opening_played => {
            slot = SLOT_M_EXT;
            task = Some(PROLOGUE_TASK);
        }
        8 if p.kaine_value == 0 || p.kaine_value == -1 => task = Some(KAINE_TASK),
        8 => return None,
        _ => {}
    }
    Some(EpisodeStart { slot, point: DEFAULT_POINT, task })
}

// ---- Option sets (conditions.md) --------------------------------------------------------------------------------

/// The condition that means none (conditions.md 3.1).
pub const NO_CONDITION: u32 = 12;

/// The option set a world loads with (conditions.md 3.1): the condition is the job key's first table-A entry when
/// a job is set; else the checkpoint task's first table-B entry when one is set; else the last table-B entry whose
/// task's saved state is above 0 (available, done or closed). Condition 12 means none. The set with that ID loads,
/// else the first set (also with no progress: a fresh load).
pub fn choose_option_set(
    sets: &formats_sly3::world::OptionSets,
    progress: Option<&save::sly2::Progress>,
    job: Option<i32>,
    checkpoint: Option<i32>,
) -> usize {
    let by_job = job.and_then(|j| sets.table_a.iter().find(|&&(o, _)| o as i32 == j)).map(|&(_, c)| c);
    let by_checkpoint =
        || checkpoint.and_then(|t| sets.table_b.iter().find(|(_, task, _)| *task as i32 == t)).map(|&(_, _, c)| c);
    let by_states = || {
        let p = progress?;
        sets.table_b
            .iter()
            .rev()
            .find(|(path, _, _)| p.get(path, None).is_some_and(|v| v.as_i32() > 0))
            .map(|&(_, _, c)| c)
    };
    let c = by_job.or_else(by_checkpoint).or_else(by_states).filter(|&c| c != NO_CONDITION);
    c.and_then(|c| sets.ids.iter().position(|&id| id == c as i32)).unwrap_or(0)
}

// ---- Typed-argument helpers -------------------------------------------------------------------------------------

fn arg_i32(a: &Arg) -> Option<i32> {
    match *a {
        Arg::Int(v) => Some(v),
        Arg::Short(_, v) => Some(v as i32),
        Arg::Bool(b) => Some(b as i32),
        _ => None,
    }
}

/// The argument lists of method option `id` on `h` (typed lists kept by the loader), as `i32`.
pub fn actions(world: &World, h: Handle, id: u16) -> Vec<Vec<i32>> {
    let (Some(level), Some(slot)) = (world.sly3.as_deref(), world.slot(h)) else { return Vec::new() };
    level
        .actions
        .iter()
        .filter(|a| a.0 == slot && a.1 == id)
        .map(|a| a.2.iter().filter_map(arg_i32).collect())
        .collect()
}

fn option_i32(world: &World, h: Handle, id: u16) -> Option<i32> {
    world.options(h).and_then(|o| o.i32(id)).or_else(|| actions(world, h, id).first()?.first().copied())
}

// ---- Exits ------------------------------------------------------------------------------------------------------

/// One exit of a loaded world.
#[derive(Clone, Debug)]
pub struct ExitInfo {
    /// The exit object.
    pub handle: Handle,
    /// Its object ID.
    pub oid: i32,
    /// The destination world's id, and its slot.
    pub destination: u32,
    /// See [`ExitInfo::destination`].
    pub slot: Option<usize>,
    /// The movies' identities.
    pub movies: Vec<u32>,
    /// Arrival point (option 834, default 424), wipe (835, default −1), arming and load delays (836, 839).
    pub point: i32,
    /// See [`ExitInfo::point`].
    pub wipe: i32,
    /// See [`ExitInfo::point`].
    pub arm_delay: f32,
    /// See [`ExitInfo::point`].
    pub load_delay: f32,
    /// The trigger volume, in the exit's space, and the exit's world matrix inverted.
    pub volume: TriMesh,
    /// See [`ExitInfo::volume`].
    pub inverse: Affine,
}

impl ExitInfo {
    /// Whether `p` (world space) is inside the trigger volume.
    pub fn contains(&self, p: [f32; 3]) -> bool {
        self.volume.point_inside(self.inverse.transform_point(p))
    }
}

/// The exits of a loaded world, one per object ID (the first loaded), in load order. `world_ids` are the disc's
/// world ids by slot.
pub fn exits(world: &World, world_ids: &[u32]) -> Vec<ExitInfo> {
    let Some(level) = world.sly3.as_deref() else { return Vec::new() };
    let mut out: Vec<ExitInfo> = Vec::new();
    for h in world.handles() {
        if world.class(h) != Some(EXIT_CLASS) {
            continue;
        }
        let (Some(oid), Some(r), Some(t)) = (world.sly3_id(h), world.sly3_record(h), world.world_transform(h)) else {
            continue;
        };
        if out.iter().any(|e| e.oid == oid) {
            continue;
        }
        let Ok(b) = exit_body(&world.level.bytes, r.body_offset, &level.table) else { continue };
        let o = world.options(h);
        let f = |id: u16, d: f32| o.map_or(d, |o| o.f32_or(id, d));
        out.push(ExitInfo {
            handle: h,
            oid,
            destination: b.destination.identity,
            slot: world_ids.iter().position(|&w| w == b.destination.identity),
            movies: b.extra.iter().map(|m| m.identity).collect(),
            point: option_i32(world, h, opt::EXIT_POINT).unwrap_or(DEFAULT_POINT),
            wipe: option_i32(world, h, opt::EXIT_WIPE).unwrap_or(-1),
            arm_delay: f(opt::ARM_DELAY, 0.0),
            load_delay: f(opt::LOAD_DELAY, 0.0),
            volume: TriMesh::new_wide(&b.mesh, b.tree),
            inverse: affine_inverse(&t).unwrap_or(Affine::IDENTITY),
        });
    }
    out
}

/// What fires exits in a world (level-flow.md 4.4): (job, exit) for option 1381, (task, exit) for 1406, and
/// (script, exit, point) for each command `0x68`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExitSources {
    /// See the type.
    pub jobs: Vec<(i32, i32)>,
    /// See the type.
    pub tasks: Vec<(i32, i32)>,
    /// See the type.
    pub commands: Vec<(i32, i32, i32)>,
}

/// Collects the exit sources of a loaded world (each record once).
pub fn exit_sources(world: &World) -> ExitSources {
    let mut s = ExitSources::default();
    let Some(level) = world.sly3.as_deref() else { return s };
    let mut seen = alloc::collections::BTreeSet::new();
    for h in world.handles() {
        let (Some(oid), Some(r)) = (world.sly3_id(h), world.record(h)) else { continue };
        if !seen.insert(r) {
            continue;
        }
        if let Some(e) = option_i32(world, h, opt::JOB_EXIT) {
            s.jobs.push((oid, e));
        }
        if let Some(e) = option_i32(world, h, opt::TASK_EXIT) {
            s.tasks.push((oid, e));
        }
        if world.class(h) == Some(COMMAND_SCRIPT_CLASS)
            && let Some(rec) = world.sly3_record(h)
            && let Ok((_, cmds)) =
                formats_sly3::stream::command_script(&world.level.bytes, rec.body_offset, &level.table)
        {
            for c in cmds.iter().filter(|c| c.id == FIRE_EXIT) {
                let a = &world.level.bytes[c.args.0 as usize..(c.args.0 + c.args.1) as usize];
                if a.len() == 8 {
                    let w = |o: usize| i32::from_le_bytes([a[o], a[o + 1], a[o + 2], a[o + 3]]);
                    s.commands.push((oid, w(0), w(4)));
                }
            }
        }
    }
    s
}

// ---- Arrival points ---------------------------------------------------------------------------------------------

/// An arrival point chosen for an id.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrivalInfo {
    /// The point object and its object ID.
    pub handle: Handle,
    /// See [`ArrivalInfo::handle`].
    pub oid: i32,
    /// Whether it answered by alias (843 or 844) rather than by its own ID.
    pub alias: bool,
    /// Option 848: (object ID, marker object ID) pairs.
    pub place: Vec<(i32, i32)>,
    /// Option 847: (task, state) pairs.
    pub task_states: Vec<(i32, i32)>,
}

/// The arrival point answering to `id` among objects of `classes` (class 40 and the classes derived from it): by
/// object ID first, then (for 424 and 425) by alias option.
pub fn find_arrival(world: &World, id: i32, classes: &[i16]) -> Option<ArrivalInfo> {
    let points: Vec<Handle> =
        world.handles().filter(|&h| world.class(h).is_some_and(|c| classes.contains(&c))).collect();
    let alias = match id {
        DEFAULT_POINT => Some(opt::ALIAS_424),
        SECOND_POINT => Some(opt::ALIAS_425),
        _ => None,
    };
    let (h, by_alias) = match points.iter().copied().find(|&h| world.sly3_id(h) == Some(id)) {
        Some(h) => (h, false),
        None => (points.iter().copied().find(|&h| alias.is_some_and(|a| !actions(world, h, a).is_empty()))?, true),
    };
    let pairs = |o: u16| {
        actions(world, h, o).into_iter().filter(|v| v.len() >= 2).map(|v| (v[0], v[1])).take(4).collect::<Vec<_>>()
    };
    Some(ArrivalInfo {
        handle: h,
        oid: world.sly3_id(h)?,
        alias: by_alias,
        place: pairs(opt::PLACE),
        task_states: pairs(opt::TASK_STATE),
    })
}

/// A task's checkpoint (option 1400), when the task is in the world.
pub fn task_checkpoint(world: &World, task: i32) -> Option<i32> {
    let h = world.find_sly3_id(task, None)?;
    option_i32(world, h, opt::CHECKPOINT)
}

// ---- The restart wipe -------------------------------------------------------------------------------------------

/// The order of the restart wipe's three pairs (level-flow.md 7): each use takes the next pair of a shuffled
/// order; after all three the order is reshuffled, and the pair just used is not allowed first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WipeOrder {
    order: [usize; 3],
    next: usize,
    last: Option<usize>,
}

impl Default for WipeOrder {
    fn default() -> Self {
        WipeOrder { order: [0, 1, 2], next: 3, last: None }
    }
}

impl WipeOrder {
    /// The next (out, in) pair.
    pub fn next(&mut self, rng: &mut Rng) -> (i32, i32) {
        if self.next >= 3 {
            // The dialogue lists' shuffle (dialogue-and-voice.md 1.1): position i swaps with one in 0 to 2 − i.
            for i in 0..3 {
                let j = rng.range_i32(0, 2 - i as i32) as usize;
                self.order.swap(i, j);
            }
            if Some(self.order[0]) == self.last {
                self.order.swap(0, 2);
            }
            self.next = 0;
        }
        let k = self.order[self.next];
        self.next += 1;
        self.last = Some(k);
        WIPE_PAIRS[k]
    }
}

// ---- Running ----------------------------------------------------------------------------------------------------

/// The walk-in state of an exit (level-flow.md 4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExitState {
    /// The player has been inside since the world started.
    InsideAtStart,
    /// Outside, for this many seconds.
    Outside(f32),
    /// Armed: firing is allowed.
    Armed,
    /// Fired; loading after this many more seconds.
    Fired(f32),
    /// The load is requested.
    LoadRequested,
}

/// A world load the flow asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadRequest {
    /// The world slot and arrival point.
    pub slot: usize,
    /// See [`LoadRequest::slot`].
    pub point: i32,
    /// The wipe object (−1 none) and, for the restart wipe, the (out, in) pair.
    pub wipe: i32,
    /// See [`LoadRequest::wipe`].
    pub wipe_pair: Option<(i32, i32)>,
    /// Movies to play first (identities).
    pub movies: Vec<u32>,
    /// The exit that asked, if one did.
    pub exit: Option<i32>,
}

/// The level flow of the loaded world.
#[derive(Clone, Debug)]
pub struct Flow {
    /// The world's exits and their states.
    pub exits: Vec<(ExitInfo, ExitState)>,
    /// The pending load.
    pub request: Option<LoadRequest>,
    /// The stored checkpoint (the point the world was entered at).
    pub checkpoint: i32,
    /// The current world slot.
    pub slot: usize,
    wipes: WipeOrder,
    pending: Option<(usize, i32)>,
}

impl Flow {
    /// The flow of a world just entered at `point`; every exit starts in state 0.
    pub fn new(world: &World, world_ids: &[u32], slot: usize, point: i32, wipes: WipeOrder) -> Flow {
        Flow {
            exits: exits(world, world_ids).into_iter().map(|e| (e, ExitState::InsideAtStart)).collect(),
            request: None,
            checkpoint: point,
            slot,
            wipes,
            pending: None,
        }
    }

    /// The restart-wipe order, to carry into the next world's flow.
    pub fn wipes(&self) -> WipeOrder {
        self.wipes.clone()
    }

    /// Whether the game is paused by a fired exit (states 4 and 5).
    pub fn paused(&self) -> bool {
        self.exits.iter().any(|e| matches!(e.1, ExitState::Fired(_) | ExitState::LoadRequested))
    }

    /// Asks exit `oid` to fire, at `point` when it is not −1 (command `0x68`). Honoured only when it is armed.
    pub fn fire(&mut self, oid: i32, point: i32) -> bool {
        let Some(i) = self.exits.iter().position(|e| e.0.oid == oid) else { return false };
        if self.exits[i].1 != ExitState::Armed {
            return false;
        }
        self.exits[i].1 = ExitState::Fired(self.exits[i].0.load_delay);
        self.pending = Some((i, if point == -1 { self.exits[i].0.point } else { point }));
        true
    }

    /// One frame: the current player at `player` (`None` without one) walks the exits' state machines; a fired
    /// exit counts its delay down and then requests the load.
    pub fn update(&mut self, player: Option<[f32; 3]>, dt: f32) {
        let mut fire = None;
        for (i, (e, s)) in self.exits.iter_mut().enumerate() {
            let inside = player.is_some_and(|p| e.contains(p));
            *s = match *s {
                ExitState::InsideAtStart if !inside => ExitState::Outside(0.0),
                ExitState::Outside(_) if inside => ExitState::InsideAtStart,
                ExitState::Outside(t) if t + dt >= e.arm_delay => ExitState::Armed,
                ExitState::Outside(t) => ExitState::Outside(t + dt),
                ExitState::Armed if inside && fire.is_none() => {
                    fire = Some(i);
                    ExitState::Armed
                }
                other => other,
            };
        }
        if let Some(i) = fire {
            self.fire(self.exits[i].0.oid, -1);
        }
        let Some((i, point)) = self.pending else { return };
        let (e, s) = &mut self.exits[i];
        if let ExitState::Fired(t) = *s {
            if t > 0.0 {
                *s = ExitState::Fired(t - dt);
                return;
            }
            *s = ExitState::LoadRequested;
            self.pending = None;
            if let Some(slot) = e.slot {
                self.request = Some(LoadRequest {
                    slot,
                    point,
                    wipe: e.wipe,
                    wipe_pair: None,
                    movies: e.movies.clone(),
                    exit: Some(e.oid),
                });
            }
        }
    }

    /// The restart (game event 12): the current world at the stored checkpoint, with wipe 875.
    pub fn restart(&mut self, rng: &mut Rng) {
        let pair = self.wipes.next(rng);
        self.request = Some(LoadRequest {
            slot: self.slot,
            point: self.checkpoint,
            wipe: RESTART_WIPE,
            wipe_pair: Some(pair),
            movies: Vec::new(),
            exit: None,
        });
    }
}

/// The current player's object ID of a crew member's object, for option 848's first objects (Sly 24, Bentley 25,
/// Murray 26 in every hub).
pub fn find_by_oid(world: &World, oid: i32) -> Option<Handle> {
    world.find_sly3_id(oid, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [i32; 9] = [-1, 0, 2, 3, 8, 15, 23, 31, 35];

    #[test]
    fn episode_starts() {
        let p = EpisodeProgress { china_recon_complete: true, opening_played: true, ..Default::default() };
        let at = |e| episode_start(&TABLE, e, &p).map(|s| (s.slot, s.point, s.task));
        assert_eq!(at(3), Some((3, 424, None)));
        assert_eq!(at(6), Some((23, 424, None)));
        assert_eq!(at(1), Some((0, 424, None)));
        assert_eq!(at(0), None);
        // China before its recon: c_intro. Pirates between the treasure and the heist: p_at_sea.
        let q = EpisodeProgress { treasure_complete: true, ..Default::default() };
        assert_eq!(episode_start(&TABLE, 6, &q).unwrap().slot, SLOT_C_INTRO);
        assert_eq!(episode_start(&TABLE, 7, &q).unwrap().slot, SLOT_P_AT_SEA);
        let q2 = EpisodeProgress { treasure_complete: true, heist_complete: true, ..q };
        assert_eq!(episode_start(&TABLE, 7, &q2).unwrap().slot, 31);
        // The menus before the opening: Kaine Island with the prologue task.
        let s = episode_start(&TABLE, 1, &EpisodeProgress::default()).unwrap();
        assert_eq!((s.slot, s.task), (SLOT_M_EXT, Some(PROLOGUE_TASK)));
        // Episode 8: task 1841 for a first value of 0 or −1; otherwise not established.
        assert_eq!(episode_start(&TABLE, 8, &EpisodeProgress::default()).unwrap().task, Some(KAINE_TASK));
        let k = EpisodeProgress { kaine_value: 3, ..Default::default() };
        assert_eq!(episode_start(&TABLE, 8, &k), None);
    }

    #[test]
    fn wipe_pairs_cycle_without_repeating_across_shuffles() {
        let mut w = WipeOrder::default();
        let mut rng = Rng::new(9);
        let mut last = None;
        for round in 0..20 {
            let three: Vec<(i32, i32)> = (0..3).map(|_| w.next(&mut rng)).collect();
            let mut sorted = three.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, WIPE_PAIRS, "round {round} uses each pair once");
            assert_ne!(Some(three[0]), last, "round {round}");
            last = Some(three[2]);
        }
    }
}
