//! Sly 2 reactive props and trigger volumes (specs/sly2/interactables.md 3 and 4), and the volumes' shapes.
//!
//! - **Reactive props** (class 98, section 3): the options, in stream order, build an event list (1030 starts an
//!   event, 1031 the every-event entry, 1032 its hit count; items 1039 add an object, 1042 remove one (−1 the prop),
//!   1033 the source for explosions and drops, 1034 explode, 1037 drops, 1038 script event 4 with a value, 1045 a task
//!   request). Items read before any event set up the intact look and run at load. A hit (an attack, or a touch by
//!   an active trigger class of option 1025) runs the current event's items, then counts toward its hit count; at
//!   the count the next event becomes current; after the last one the prop raises script event 5. An event with a hit
//!   count of −1 or 0 never advances and runs only its explosions, drops and events. Each hit not by a guard makes
//!   noise 706. Drops come only from hits by the played character: a default drop (−1) spills 2 to 3 coins
//!   (pickpocket-and-pickups.md 4.1, the ordinary drop).
//!   Mesh and collision groups (1040, 1041, 1043, 1044) are not drawn or collided per group yet.
//! - **Trigger volumes** (classes 24 and 25, section 4): states 0 empty, 1 occupied, 2 holding; an object that counts
//!   (option 432 objects, 433 classes; 52 is the playable characters' family) entering raises script event 5, the
//!   tasks' event 0x1A and the enter list (task requests 440), then holds for the hold time (434, default 1 s);
//!   with nothing counting inside it goes back to empty: script event 6 and task event 0x1B. A tracking volume (443)
//!   raises script event 9 whenever its members change.
//! - **Volume shapes**: a volume's collision mesh is read from its body; "inside" is inside the box of its points in
//!   the volume's frame (*the mesh's own faces are not tested yet*).

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::sly2_sensors::characters;
use super::{ClassBehavior, Cx};
use crate::math::sub;
use crate::object::{Handle, OptionValue};
use crate::world::{Notice, World};

/// Task events (missions.md) the props and volumes send, through `World::shared.task_events`.
pub mod task_event {
    /// A prop went past its last event.
    pub const PROP_DONE: u32 = 0x11;
    /// A volume was entered.
    pub const ENTERED: u32 = 0x1A;
    /// A volume was left.
    pub const LEFT: u32 = 0x1B;
}

/// Sends a task event to the job system (`Sly2Level::task_events`, oldest first).
pub fn send_task_event(world: &mut World, e: u32) {
    send_task_event_from(world, e, None);
}

/// Sends a task event concerning `object` (missions.md 4.3: the object that raised script event 5 or 6 goes with
/// task event 26 or 27).
pub fn send_task_event_from(world: &mut World, e: u32, object: Option<Handle>) {
    if let Some(l) = world.sly2.as_deref_mut() {
        l.task_events.push((e, object));
    }
}

/// A Sly 2 volume's box: its frame and the box of its collision points in that frame.
pub fn volume_box(world: &World, h: Handle) -> Option<(Affine, [f32; 3], [f32; 3])> {
    // Sly 3 hook: a Sly 3 trigger shape (class 21, layout "shape": header, options, then the same collision mesh).
    let end = match world.sly3 {
        Some(_) => world.sly3_record(h)?.options()?.end,
        None => world.sly2_record(h)?.options()?.end,
    };
    let data = &world.level.bytes;
    let mut rd = formats::reader::Reader::at(data, end);
    let n = rd.u16().ok()? as usize;
    if n == 0 || n > 4096 {
        return None;
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for _ in 0..n {
        let p = rd.vec3().ok()?;
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((world.world_transform(h)?, lo, hi))
}

/// Whether `p` is inside a volume's box.
pub fn in_volume(b: &(Affine, [f32; 3], [f32; 3]), p: [f32; 3]) -> bool {
    let (f, lo, hi) = b;
    let d = sub(p, f.t);
    let c = &f.cols;
    let n = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-12);
    let l = [0, 1, 2].map(|k| (d[0] * c[k][0] + d[1] * c[k][1] + d[2] * c[k][2]) / n(c[k]));
    (0..3).all(|k| l[k] >= lo[k] && l[k] <= hi[k])
}

/// One item of a prop's event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    /// Add an object to the world.
    Add(i32),
    /// Remove an object (−1: the prop).
    Remove(i32),
    /// The source for the next explosion or drop.
    Source(i32),
    /// Explode an explosion object.
    Explode(i32),
    /// Drops: −1 the default, 0 none.
    Drop(i32),
    /// Script event 4 with a value.
    Event(i32),
    /// A task-state request.
    Task(i32, i32),
}

impl Item {
    /// Whether the item sets the prop's look (objects added or removed), the only kind applied at load.
    pub fn sets_look(&self) -> bool {
        matches!(self, Item::Add(_) | Item::Remove(_))
    }
}

/// One event of a prop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropEvent {
    /// Hit count (−1 or 0: never advances).
    pub hits: i32,
    /// Items.
    pub items: Vec<Item>,
}

/// A reactive prop (class 98).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prop {
    /// Items before any event (the intact look).
    pub setup: Vec<Item>,
    /// The events.
    pub events: Vec<PropEvent>,
    /// Items run with every hit.
    pub every: Vec<Item>,
    /// The current event (past the last: done).
    pub current: usize,
    /// Hits counted on the current event.
    pub counted: i32,
    /// Classes that set it off by touching, active.
    pub triggers: Vec<i16>,
    /// Hits taken.
    pub hits_taken: u32,
}

fn int_of(v: &OptionValue) -> Option<i32> {
    match v {
        OptionValue::Args(a) => a.first().and_then(|x| match *x {
            crate::object::Arg::Int(n) => Some(n),
            crate::object::Arg::Short(_, n) => Some(i32::from(n)),
            crate::object::Arg::Float(f) => Some(f as i32),
            _ => None,
        }),
        _ => v.as_i32(),
    }
}

fn pair_of(v: &OptionValue) -> (i32, i32) {
    match v {
        OptionValue::Args(a) => {
            let n = |k: usize| {
                a.get(k).and_then(|x| match *x {
                    crate::object::Arg::Int(n) => Some(n),
                    crate::object::Arg::Short(_, n) => Some(i32::from(n)),
                    crate::object::Arg::Bool(b) => Some(b as i32),
                    _ => None,
                })
            };
            (n(0).unwrap_or(0), n(1).unwrap_or(0))
        }
        _ => (v.as_i32().unwrap_or(0), 0),
    }
}

/// A trigger volume's task-state requests on enter (option 440, up to 4): stored values and, as on the disc, the
/// loader's action entries (typed argument lists).
pub fn volume_requests(world: &World, h: Handle) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = world.options(h).map(|o| o.all(440).map(pair_of).collect()).unwrap_or_default();
    if let (Some(l), Some(slot)) = (world.sly2.as_deref(), world.slot(h)) {
        out.extend(
            l.actions.iter().filter(|a| a.0 == slot && a.1 == 440).map(|a| pair_of(&OptionValue::Args(a.2.clone()))),
        );
    }
    out.truncate(4);
    out
}

impl Prop {
    /// The event list of prop `h` (options in stream order; actions from `Sly2Level::actions` in their order).
    pub fn new(world: &World, h: Handle) -> Prop {
        let mut list: Vec<(u16, OptionValue)> = world.options(h).map(|o| o.entries().to_vec()).unwrap_or_default();
        if let Some(l) = world.sly2.as_deref() {
            // Typed argument lists are kept apart; put them back in stream order is not possible here, so they
            // follow the stored values (the props' items are plain values on the disc).
            list.extend(l.actions.iter().filter(|a| a.0 == h.index()).map(|a| (a.1, OptionValue::Args(a.2.clone()))));
        }
        let mut p = Prop::default();
        let mut cur: Option<usize> = None;
        let mut in_every = false;
        for (id, v) in &list {
            let item = match id {
                1025 => {
                    let (class, on) = pair_of(v);
                    if on != 0 && class != 0 {
                        p.triggers.push(class as i16);
                    }
                    None
                }
                1030 => {
                    // With no hit count (option 1032) one hit advances the event. An inference [I] from one prop
                    // (missions.md 4.3: the vent panel 120985 breaks at the first hit); it changes every such prop's
                    // events, in Sly 2 and in Sly 3 (whose props use this behaviour). An explicit −1 or 0 never
                    // advances (interactables.md 3.2).
                    p.events.push(PropEvent { hits: 1, items: Vec::new() });
                    cur = Some(p.events.len() - 1);
                    in_every = false;
                    None
                }
                1031 => {
                    in_every = true;
                    None
                }
                1032 => {
                    if let Some(c) = cur {
                        p.events[c].hits = int_of(v).unwrap_or(-1);
                    }
                    None
                }
                1039 => int_of(v).map(Item::Add),
                1042 => int_of(v).map(Item::Remove),
                1033 => int_of(v).map(Item::Source),
                1034 => int_of(v).map(Item::Explode),
                1037 => int_of(v).map(Item::Drop),
                1038 => int_of(v).map(Item::Event),
                1045 => {
                    let (t, s) = pair_of(v);
                    Some(Item::Task(t, s))
                }
                _ => None,
            };
            if let Some(item) = item {
                match (in_every, cur) {
                    (true, _) => p.every.push(item),
                    (false, Some(c)) => p.events[c].items.push(item),
                    (false, None) => p.setup.push(item),
                }
            }
        }
        p.events.truncate(32);
        p
    }

    /// Past its last event.
    pub fn done(&self) -> bool {
        self.current >= self.events.len()
    }

    fn run(cx: &mut Cx<'_>, items: &[Item], structural: bool, drops: bool) {
        let mut source = cx.my_position();
        for &item in items {
            match item {
                Item::Add(id) if structural => {
                    if let Some(h) = cx.world.find_sly2_id(id, Some(cx.me)) {
                        cx.world.set_in_world(h, true);
                    }
                }
                Item::Remove(id) if structural => {
                    let h = if id < 0 { Some(cx.me) } else { cx.world.find_sly2_id(id, Some(cx.me)) };
                    if let Some(h) = h {
                        cx.world.set_in_world(h, false);
                    }
                }
                Item::Source(id) => {
                    if let Some(p) = cx.world.find_sly2_id(id, Some(cx.me)).and_then(|h| cx.world.position(h)) {
                        source = p;
                    }
                }
                Item::Explode(id) => {
                    if let Some(h) = cx.world.find_sly2_id(id, Some(cx.me)) {
                        cx.world.notify(h, Notice::Blast(cx.me, [0.0; 3]));
                    }
                }
                Item::Drop(d) if drops && d != 0 => {
                    let n = 2 + (cx.rng.next_u32() % 2);
                    cx.world.shared.coin_drops.push((source, n));
                }
                Item::Event(v) => cx.raise(4, alloc::vec![script::Value::Int(v)]),
                Item::Task(t, s) => cx.world.shared.task_requests.push((t, s)),
                _ => {}
            }
        }
    }

    fn hit(&mut self, cx: &mut Cx<'_>, by: Option<Handle>) {
        self.hits_taken += 1;
        let by_guard = by.and_then(|b| cx.world.class(b)).is_some_and(|c| matches!(c, 56..=68));
        let by_player = by.is_some() && by == cx.world.player;
        if !by_guard {
            let at = cx.my_position();
            super::sly2::make_noise(cx.world, 706, at, by, None, cx.time);
        }
        let every = self.every.clone();
        Self::run(cx, &every, true, by_player);
        let Some(ev) = self.events.get(self.current).cloned() else { return };
        let advances = ev.hits > 0;
        Self::run(cx, &ev.items, advances, by_player);
        if !advances {
            return;
        }
        self.counted += 1;
        if self.counted >= ev.hits {
            self.counted = 0;
            self.current += 1;
            if self.done() {
                cx.raise(5, Vec::new());
                send_task_event(cx.world, task_event::PROP_DONE);
            }
        }
    }
}

impl ClassBehavior for Prop {
    fn ticks(&self) -> bool {
        !self.events.is_empty()
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        // Only the intact look applies at once (interactables.md 3.1): explosions, drops, events and task requests
        // read before any event are not set off at load (they blew up hundreds of explosion groups on frame 0).
        let setup: Vec<Item> = self.setup.iter().copied().filter(Item::sets_look).collect();
        Self::run(cx, &setup, true, false);
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices: Vec<Notice> = cx.world.notices(cx.me).to_vec();
        for n in notices {
            match n {
                Notice::Attack { by, .. } => self.hit(cx, Some(by)),
                Notice::Hit(by) => self.hit(cx, Some(by)),
                _ => {}
            }
        }
        // Touches by an active trigger class (the playable characters, 52; guards, 56).
        if !self.triggers.is_empty() && !self.done() {
            let me = cx.my_position();
            let touching = characters(cx.world).into_iter().find(|&(h, c)| {
                let class = cx.world.class(h).unwrap_or(0);
                self.triggers.iter().any(|&t| t == class || t == 52 || t == 51)
                    && super::dist_sq(c, me) <= 120.0 * 120.0
            });
            if let Some((h, _)) = touching {
                self.hit(cx, Some(h));
            }
        }
    }
}

/// Sly 3: the most members a tracking trigger keeps (interactables.md 6, option 478).
const MAX_MEMBERS: usize = 64;

/// A trigger volume (classes 24 and 25).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriggerVolume {
    /// 0 empty, 1 occupied, 2 holding.
    pub state: u8,
    /// Seconds left holding.
    pub hold: f32,
    /// Members inside (tracking).
    pub members: Vec<Handle>,
    /// Sly 3 hook: in the world at the last update (leaving it raises script event 9 once with an empty set).
    pub was_in: bool,
    /// The objects option 432 names, resolved on the first update.
    pub named: Option<Vec<Handle>>,
    /// The played character counts as inside next frame (a route standing in for the player walking in; the smoke
    /// tool's job route).
    pub visit: bool,
}

impl TriggerVolume {
    fn counts(world: &World, me: Handle, h: Handle) -> bool {
        let o = world.options(me);
        let ids: Vec<i32> = o.map(|o| o.all(432).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
        let classes: Vec<i32> = o.map(|o| o.all(433).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
        if ids.is_empty() && classes.is_empty() {
            return world.class(h).is_some_and(|c| super::sly3::character_classes(world).contains(&c)); // Sly 3 hook
        }
        let class = world.class(h).unwrap_or(0) as i32;
        let character = super::sly3::character_classes(world).contains(&(class as i16)); // Sly 3 hook
        ids.iter().any(|&id| world.sly2_id(h).or_else(|| world.sly3_id(h)) == Some(id)) // Sly 3 hook: Sly 3 IDs
            || classes.iter().any(|&c| c == class || (character && (c == 52 || c == 51)))
    }
}

impl ClassBehavior for TriggerVolume {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        // Sly 3 hook: a trigger out of the world does nothing and forgets who was inside, so being added counts as
        // an entry for whoever stands in it (v_ext's 140745, which 2245's switch brings in). A tracking trigger
        // leaving the world raises script event 9 once more with an empty set (interactables.md 6, option 478).
        if cx.world.sly3.is_some() {
            let was_in = core::mem::replace(&mut self.was_in, cx.world.in_world(cx.me));
            if !self.was_in {
                self.state = 0;
                self.members.clear();
                if was_in && cx.world.options(cx.me).is_some_and(|o| o.bool_or(443, false)) {
                    cx.raise(9, Vec::new());
                }
                return;
            }
        }
        let shape = volume_box(cx.world, cx.me);
        if shape.is_none() && !self.visit {
            return;
        }
        let mut inside: Vec<Handle> = match &shape {
            Some(shape) => {
                // The characters, and the objects option 432 names (missions.md 4.1: 150658 waits for the van's
                // carrier 150849), by their position.
                let mut candidates = characters(cx.world);
                // Sly 3 hook (missions.md 4.1a): the objects of the classes the shape counts (option 433) count by
                // their position too (v_gauntlet's 139753 counts guards, class 58).
                if cx.world.sly3.is_some() {
                    let classes: Vec<i16> = cx
                        .world
                        .options(cx.me)
                        .map(|o| o.all(433).filter_map(|v| v.as_i32()).map(|c| c as i16).collect())
                        .unwrap_or_default();
                    for c in classes.into_iter().filter(|&c| !matches!(c, 51..=54)) {
                        for &s in cx.world.slots_of_class(c) {
                            if let Some(h) = cx.world.handle(s)
                                && cx.world.is_active(h)
                                && let Some(p) = cx.world.position(h)
                            {
                                candidates.push((h, p));
                            }
                        }
                    }
                }
                if self.named.is_none() {
                    let ids: Vec<i32> = cx
                        .world
                        .options(cx.me)
                        .map(|o| o.all(432).filter_map(|v| v.as_i32()).collect())
                        .unwrap_or_default();
                    self.named =
                        Some(ids.into_iter().filter_map(|id| cx.world.find_sly2_id(id, Some(cx.me))).collect());
                }
                for &h in self.named.as_deref().unwrap_or_default() {
                    if !candidates.iter().any(|c| c.0 == h)
                        && let Some(p) = cx.world.position(h)
                    {
                        candidates.push((h, p));
                    }
                }
                candidates
                    .into_iter()
                    .filter(|&(h, c)| in_volume(shape, c) && Self::counts(cx.world, cx.me, h))
                    .map(|(h, _)| h)
                    .collect()
            }
            None => Vec::new(),
        };
        if core::mem::take(&mut self.visit)
            && let Some(p) = cx.world.player
            && !inside.contains(&p)
        {
            inside.push(p);
        }
        let o = cx.world.options(cx.me);
        let hold_time = o.and_then(|o| o.f32(434)).unwrap_or(1.0);
        let tracking = o.is_some_and(|o| o.bool_or(443, false));
        let stays = o.is_some_and(|o| o.bool_or(438, false));
        // Sly 3 hook (interactables.md 6, option 478): the set is collected each frame (up to 64 members) and sorted,
        // so only a change of who is inside raises script event 9, either way.
        if tracking && cx.world.sly3.is_some() {
            inside.sort_unstable();
            inside.dedup();
            inside.truncate(MAX_MEMBERS);
        }
        if tracking && inside != self.members {
            self.members = inside.clone();
            cx.raise(9, Vec::new());
        }
        match self.state {
            0 if !inside.is_empty() => {
                cx.raise(5, Vec::new());
                send_task_event_from(cx.world, task_event::ENTERED, Some(cx.me));
                // Option 441: the sequence it starts, with the entering object as starter.
                if let Some(seq) = cx
                    .world
                    .options(cx.me)
                    .and_then(|o| o.i32(441))
                    .and_then(|id| cx.world.find_sly2_id(id, Some(cx.me)))
                {
                    let starter = inside.first().copied();
                    cx.world
                        .shared
                        .sequence_requests
                        .push(super::sly2_commands::SequenceRequest::Start { sequence: seq, starter });
                }
                let requests: Vec<(i32, i32)> = volume_requests(cx.world, cx.me);
                cx.world.shared.task_requests.extend(requests);
                self.state = 2;
                self.hold = hold_time;
            }
            2 => {
                if hold_time >= 0.0 {
                    self.hold -= cx.dt;
                }
                if self.hold <= 0.0 && hold_time >= 0.0 && !(tracking && !inside.is_empty()) {
                    self.state = 1;
                }
            }
            1 if inside.is_empty() && !stays => {
                cx.raise(6, Vec::new());
                send_task_event_from(cx.world, task_event::LEFT, Some(cx.me));
                self.state = 0;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Item;

    #[test]
    fn only_the_look_applies_at_load() {
        assert!(Item::Add(5).sets_look());
        assert!(Item::Remove(-1).sets_look());
        for i in [Item::Explode(7), Item::Source(7), Item::Drop(-1), Item::Event(3), Item::Task(1, 2)] {
            assert!(!i.sets_look(), "{i:?}");
        }
    }
}
