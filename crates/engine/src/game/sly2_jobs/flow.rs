//! Moving between Sly 2 worlds (level-flow.md 5 to 8): exits (class 26), arrival points (class 42), warp points
//! (class 41) and the load requests they make.

use alloc::vec::Vec;

use formats::geometry::Affine;
use formats_sly2::record::FileRecord;

use crate::object::{Arg, Handle};
use crate::world::World;

/// The default arrival point (`ENTRANCE_default`).
pub const DEFAULT_POINT: i32 = 393;
/// Class of exits.
pub const EXIT_CLASS: i16 = 26;
/// Class of warp points.
pub const WARP_CLASS: i16 = 41;
/// Class of arrival points.
pub const POINT_CLASS: i16 = 42;
/// The menu world's row (level-flow.md 2).
pub const MENU_ROW: i16 = 1;

/// Exit and point option IDs (level-flow.md 5.1 and 6).
pub mod opt {
    /// A warp point's method: move the player there.
    pub const WARP: u16 = 778;
    /// An exit's arrival point.
    pub const POINT: u16 = 779;
    /// An exit's wipe screen object.
    pub const WIPE: u16 = 780;
    /// Seconds outside before the exit arms.
    pub const ARM_TIME: u16 = 781;
    /// An exit's method: fire it.
    pub const FIRE: u16 = 786;
    /// (task, state) pairs applied when the exit fires.
    pub const TASK_STATES: u16 = 788;
    /// An arrival point also answers to 393, 394, its argument.
    pub const ALSO_393: u16 = 790;
    pub const ALSO_394: u16 = 791;
    pub const ALSO: u16 = 792;
    /// Arrival point lists: 793 a state-machine goal (object, machine, goal, instant [field order: inference]),
    /// 794 a task-state request (task, state), 795 a move of an object to a target (object, target).
    pub const POINT_GOALS: u16 = 793;
    pub const POINT_TASKS: u16 = 794;
    pub const POINT_PLACE: u16 = 795;
}

/// A request to load a world (level-flow.md 7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadRequest {
    /// The level-table row.
    pub row: i16,
    /// The arrival point (393 by default).
    pub point: i32,
    /// The wipe screen object (777 or 778), if any.
    pub wipe: Option<i32>,
    /// Movies to play first (file identities), at most 4.
    pub movies: Vec<u32>,
    /// A task to request on arrival (a new game queues task 1583).
    pub task: Option<i32>,
}

/// An exit's state (level-flow.md 5.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExitState {
    /// The player has been inside since the world started.
    Inside,
    /// Outside, for this long.
    Outside(f32),
    /// Armed.
    Armed,
    /// Fired.
    Fired,
}

/// An exit of the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Exit {
    /// The object.
    pub handle: Handle,
    /// Object ID.
    pub id: i32,
    /// The destination row and the movies, from the body's file records.
    pub row: Option<i16>,
    pub movies: Vec<u32>,
    /// State.
    pub state: ExitState,
}

/// The body's destination and extra file records: the last bytes of the body are a 64-byte destination record,
/// a `u8` count n and n records (classes/paths-and-volumes.md 6). Returns the destination identity and the extras'.
pub fn exit_records(world: &World, h: Handle) -> Option<(u32, Vec<u32>)> {
    let r = world.sly2_record(h)?;
    let end = (r.body_offset + r.body_size) as usize;
    let start = r.options()?.end;
    let data = world.level.bytes.get(start..end)?;
    for n in 0..=4usize {
        let tail = 64 * n;
        let Some(count_at) = data.len().checked_sub(tail + 1) else { break };
        if data[count_at] as usize != n || count_at < 64 {
            continue;
        }
        let dest = FileRecord::decode(&data[count_at - 64..count_at]);
        let extras = (0..n)
            .map(|i| FileRecord::decode(&data[count_at + 1 + 64 * i..count_at + 65 + 64 * i]))
            .filter(|f| !f.is_empty())
            .map(|f| f.identity)
            .collect();
        if !dest.is_empty() {
            return Some((dest.identity, extras));
        }
    }
    None
}

/// The level row of a world identity, from the world's level table.
pub fn row_of(world: &World, identity: u32) -> Option<i16> {
    let l = world.sly2.as_deref()?;
    l.header.level_rows.iter().position(|r| !r.file.is_empty() && r.world_id() == identity).map(|r| r as i16)
}

/// The world's exits.
pub fn exits(world: &World) -> Vec<Exit> {
    world
        .slots_of_class(EXIT_CLASS)
        .iter()
        .filter_map(|&s| world.handle(s))
        .map(|h| {
            let recs = exit_records(world, h);
            Exit {
                handle: h,
                id: world.sly2_id(h).unwrap_or(-1),
                row: recs.as_ref().and_then(|(id, _)| row_of(world, *id)),
                movies: recs.map(|r| r.1).unwrap_or_default(),
                state: ExitState::Inside,
            }
        })
        .collect()
}

fn args<'a>(world: &'a World, h: Handle, id: u16) -> impl Iterator<Item = &'a [Arg]> + 'a {
    let slot = world.slot(h);
    world.sly2.as_deref().into_iter().flat_map(move |l| {
        l.actions.iter().filter(move |(s, i, _)| Some(*s) == slot && *i == id).map(|(_, _, a)| &a[..])
    })
}

fn ints(a: &[Arg]) -> Vec<i32> {
    a.iter()
        .filter_map(|x| match *x {
            Arg::Int(v) => Some(v),
            Arg::Short(_, v) => Some(v as i32),
            Arg::Bool(b) => Some(b as i32),
            _ => None,
        })
        .collect()
}

/// The load request an exit makes when it fires (level-flow.md 5.2), with its (task, state) pairs to apply.
pub fn fire(world: &World, exit: &Exit) -> Option<(LoadRequest, Vec<(i32, i32)>)> {
    let h = exit.handle;
    let o = world.options(h);
    let point = o.and_then(|o| o.i32(opt::POINT)).filter(|&p| p >= 0).unwrap_or(DEFAULT_POINT);
    let wipe = o.and_then(|o| o.i32(opt::WIPE)).filter(|&w| w > 0);
    let pairs = args(world, h, opt::TASK_STATES)
        .flat_map(|a| ints(a).as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect::<Vec<_>>())
        .collect();
    let mut movies = exit.movies.clone();
    movies.truncate(4);
    Some((LoadRequest { row: exit.row?, point, wipe, movies, task: None }, pairs))
}

/// The IDs an arrival point answers to (level-flow.md 6): its own and its extra IDs.
pub fn point_ids(world: &World, h: Handle) -> Vec<i32> {
    let mut ids = Vec::new();
    ids.extend(world.sly2_id(h));
    if args(world, h, opt::ALSO_393).next().is_some() {
        ids.push(393);
    }
    if args(world, h, opt::ALSO_394).next().is_some() {
        ids.push(394);
    }
    for a in args(world, h, opt::ALSO) {
        ids.extend(ints(a));
    }
    ids.truncate(4);
    ids
}

/// The arrival point for `id`: the last class-42 object, in class order, answering to it.
pub fn find_point(world: &World, id: i32) -> Option<Handle> {
    world
        .slots_of_class(POINT_CLASS)
        .iter()
        .rev()
        .filter_map(|&s| world.handle(s))
        .find(|&h| point_ids(world, h).contains(&id))
}

/// What applying an arrival point does besides placing the player (level-flow.md 6): (object, target) placements,
/// state-machine goals (object, machine, goal, instant) and (task, state) pairs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PointActions {
    pub place: Vec<(i32, i32)>,
    pub goals: Vec<(i32, i32, i32, bool)>,
    pub tasks: Vec<(i32, i32)>,
}

/// An arrival point's lists.
pub fn point_actions(world: &World, h: Handle) -> PointActions {
    let two = |id: u16| -> Vec<(i32, i32)> {
        args(world, h, id)
            .filter_map(|a| {
                let v = ints(a);
                (v.len() >= 2).then(|| (v[0], v[1]))
            })
            .take(4)
            .collect()
    };
    let goals = args(world, h, opt::POINT_GOALS)
        .filter_map(|a| {
            let v = ints(a);
            (v.len() >= 3).then(|| (v[0], v[1], v[2], v.get(3).is_some_and(|&x| x != 0)))
        })
        .take(4)
        .collect();
    PointActions { place: two(opt::POINT_PLACE), goals, tasks: two(opt::POINT_TASKS) }
}

/// Where a point or warp puts the player: its world transform.
pub fn transform_of(world: &World, h: Handle) -> Option<Affine> {
    world.world_transform(h)
}
