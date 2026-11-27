//! Sly 3 state machines (class 132; specs/sly3/classes/animation.md 2, Sly 2's class 111 layout) driven to goals:
//! the job system's state-machine goals (task option 1417, `Jobs::machine_goals`) and command `0x19`.
//!
//! - **The machine's tables**: after its object list, `u8` count × (state ID, object ID) and `u8` count ×
//!   (from, to, flags, a word, a float, object ID) transitions (24 bytes each; flag bit 0: both ways, Sly 2's rule).
//! - **A goal**: the object named (a placement stands for its copy's root; a script's goal looks in the script's
//!   copy first) and its machine (by ID, in that copy or below the object, never another copy's).
//!   The machine moves from its state to the goal along the fewest transitions; each transition's object is the clip
//!   it plays on the object, whose add / remove events (kinds 7 and 8, `clips`) run (missions.md 4.1a: the blimp
//!   anchors' 5099 adds the anchor on 1763 → 1764 and removes it on 1764 → 1765).
//! - **Instant** goals (a snap, or while restoring) apply every transition's add / remove events at once; otherwise
//!   the path's earlier transitions apply at once and the last one's clip plays.
//! - Working choices [I]: a machine starts in its first listed state (where a machine starts is not in the specs);
//!   a goal with no path to it sets the state with no clip; the transition's word, float and other flags are not
//!   read.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use engine::object::Handle;
use engine::world::World;

use super::clips::Clips;

/// The state-machine class.
pub const MACHINE_CLASS: i16 = 132;

/// A transition: from, to, both ways, the clip played (object ID, −1 none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: i32,
    pub to: i32,
    pub both: bool,
    pub clip: i32,
}

/// A machine's states and transitions, read from its record.
pub fn tables(world: &World, h: Handle) -> Option<(Vec<i32>, Vec<Transition>)> {
    let r = world.sly3_record(h)?;
    let bytes = &world.level.bytes;
    let at = r.nested().last().map_or(r.body_offset as usize + 2, |c| (c.body_offset + c.body_size) as usize);
    let word = |k: usize| bytes.get(k..k + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let n = *bytes.get(at)? as usize;
    let states: Vec<i32> = (0..n).filter_map(|k| word(at + 1 + k * 8)).collect();
    let t0 = at + 1 + n * 8;
    let m = *bytes.get(t0)? as usize;
    let transitions = (0..m)
        .filter_map(|k| {
            let b = t0 + 1 + k * 24;
            Some(Transition { from: word(b)?, to: word(b + 4)?, both: word(b + 8)? & 1 != 0, clip: word(b + 20)? })
        })
        .collect();
    Some((states, transitions))
}

/// The fewest transitions from `from` to `to` (empty when already there; `None` with no path).
pub fn path(transitions: &[Transition], from: i32, to: i32) -> Option<Vec<Transition>> {
    let mut came: BTreeMap<i32, Transition> = BTreeMap::new();
    let mut queue = VecDeque::from([from]);
    let mut seen = alloc::vec![from];
    while let Some(s) = queue.pop_front() {
        if s == to {
            let mut out = Vec::new();
            let mut at = to;
            while at != from {
                let t = came[&at];
                out.push(t);
                at = if t.to == at { t.from } else { t.to };
            }
            out.reverse();
            return Some(out);
        }
        for t in transitions {
            let next = if t.from == s {
                Some(t.to)
            } else if t.both && t.to == s {
                Some(t.from)
            } else {
                None
            };
            if let Some(n) = next.filter(|n| !seen.contains(n)) {
                seen.push(n);
                came.insert(n, *t);
                queue.push_back(n);
            }
        }
    }
    None
}

/// The machines' states, by machine slot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Machines {
    pub states: BTreeMap<u32, i32>,
}

impl Machines {
    /// Drives `object`'s machine `machine` to `goal` (module notes). False when either is missing.
    pub fn goal(
        &mut self,
        world: &mut World,
        clips: &mut Clips,
        object: i32,
        machine: i32,
        goal: i32,
        instant: bool,
    ) -> bool {
        self.goal_near(world, clips, (object, None), machine, goal, instant)
    }

    /// [`Machines::goal`], `object` looked up in the copy of the given object first (a copy's script names its own
    /// copy's objects: v_gauntlet's buttons each have a root 145257).
    pub fn goal_near(
        &mut self,
        world: &mut World,
        clips: &mut Clips,
        (object, near): (i32, Option<Handle>),
        machine: i32,
        goal: i32,
        instant: bool,
    ) -> bool {
        let Some(o) = world.find_sly3_id(object, near) else { return false };
        // A placement stands for its copy's root.
        let owner = if world.class(o) == Some(formats_sly3::stream::PLACEMENT) {
            world.children(o).next().unwrap_or(o)
        } else {
            o
        };
        // The machine in the object's own copy (or below it), never another copy's of the same ID.
        let Some(m) = super::clips::near(world, machine, owner).filter(|&m| world.class(m) == Some(MACHINE_CLASS))
        else {
            return false;
        };
        let (Some(slot), Some((states, transitions))) = (world.slot(m), tables(world, m)) else { return false };
        let now = self.states.get(&slot).copied().or_else(|| states.first().copied()).unwrap_or(goal);
        self.states.insert(slot, goal);
        let steps = path(&transitions, now, goal).unwrap_or_default();
        let last = steps.len().checked_sub(1);
        for (k, t) in steps.iter().enumerate() {
            if t.clip == -1 {
                continue;
            }
            if instant || Some(k) != last {
                clips.apply_now(world, owner, t.clip);
            } else {
                clips.play(world, owner, t.clip);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(from: i32, to: i32, both: bool, clip: i32) -> Transition {
        Transition { from, to, both, clip }
    }

    #[test]
    fn paths_take_the_fewest_transitions() {
        // The blimp anchor's 5099 (missions.md 4.1a): 1763 → 1764 → 1765 → 1766, 1764 → 1763 back.
        let ts = [
            t(1763, 1764, false, 163033),
            t(1764, 1763, false, 163034),
            t(1764, 1765, false, 163035),
            t(1765, 1766, false, -1),
        ];
        let clips = |p: Option<Vec<Transition>>| p.map(|p| p.iter().map(|t| t.clip).collect::<Vec<_>>());
        assert_eq!(clips(path(&ts, 1763, 1764)), Some(alloc::vec![163033]));
        assert_eq!(clips(path(&ts, 1763, 1765)), Some(alloc::vec![163033, 163035]));
        assert_eq!(clips(path(&ts, 1764, 1763)), Some(alloc::vec![163034]));
        assert_eq!(clips(path(&ts, 1765, 1765)), Some(Vec::new()));
        assert_eq!(path(&ts, 1766, 1763), None);
        // Both ways.
        let ts = [t(1, 2, true, 7)];
        assert_eq!(clips(path(&ts, 2, 1)), Some(alloc::vec![7]));
    }
}
