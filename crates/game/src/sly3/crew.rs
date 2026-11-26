//! The Sly 3 crew in a world: `engine::gang::sly3` (Sly, Murray and Bentley playable, the recruits present) behind
//! the game loop's [`PlayerSystem`].
//!
//! - **Collision**: the collision parts of every loaded solid that is in the world (`formats_sly3::collision`, read
//!   as `formats::collision` meshes and trees), each placed at its object's world matrix. The characters' own parts
//!   are left out. One shape per record, shared by its copies.
//! - **Arrival** (specs/sly3/characters.md 2, level-flow.md 5): the arrival id (default [`DEFAULT_ARRIVAL`], 424) is
//!   matched to a checkpoint (class 40 or a class derived from it) by its OID, or by alias (option 843 answers 424,
//!   844 answers 425); option 848 places characters at markers. A world with no such checkpoint is entered at its
//!   first checkpoint (the spec's advice for a port that loads a world directly); a world without checkpoints leaves
//!   the characters at their records' placements.
//! - **Current player**: Sly, else Bentley, else Murray (the first present). It is placed at the checkpoint, facing
//!   its heading. The spec removes the other playable characters from the world; here they stand beside the
//!   player instead.
//! - **Switching** (characters.md 6): only what the game hands over switches the played member: the safe house's
//!   take-out, command 0x13 and the world object's method 983 on a crew member, named in
//!   `World::shared.sly3_switch_to`. The member handed control comes into the world. There is no free d-pad
//!   switching (an inference of the spec: no gameplay code tests the d-pad buttons).
//! - Every frame each core member's position and heading are written into its object, and `World::player` is the
//!   current member's object, so the follow camera follows whoever is played.
//! - **Guards** (guards.md: Sly 2's hit and knock-out rules): the in-world guards (class 58, `Sly2Guard`) that are not
//!   knocked out are the crew's targets, and the crew's attacks reach them as Sly 2's gang's do
//!   (`crate::sly2::gang`): a punch or crossbow swing on its target, an air attack, a flop landing or a dart around
//!   the point. The guards' strikes on the crew are not taken yet.
//! - **Carmelita** (missions.md 4.1a, the turf-war battles: player 0 is handed a class-60 Carmelita copy). Stand-in
//!   [I] (her controls and movement are not traced): while she is driven, the played member takes the real pad and
//!   moves as usual; it starts where she stands, and every frame its position and heading are written into her
//!   object, which is then `World::player` (the camera follows her). Her own clips are not played.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use engine::behavior::Behavior;
use engine::collision::TriMesh;
use engine::collision::world::{CollisionWorld, QueryFilter, Shape};
use engine::game::systems::{CollisionQueries, FrameInput, PlayerSystem, StepInfo};
use engine::gang::sly3::{Crew, Member, shared};
use engine::gang::{GangEvent, GangInput, GangWorld, Target, TargetKind, Who};
use engine::object::Handle;
use engine::world::{Notice, World};
use formats::geometry::Affine;
use formats_sly3::stream::Body;

use crate::sly2::gang::{AREA_REACH, TARGET_RANGE, gang_buttons};

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    engine::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// The default arrival id (characters.md 2).
pub const DEFAULT_ARRIVAL: i32 = 424;
/// The checkpoint class.
pub const CHECKPOINT_CLASS: i16 = 40;
/// Carmelita's class (a guard-family playable character, missions.md 4.1a).
pub const CARMELITA_CLASS: i16 = 60;
/// Where the other core members stand beside the player (offsets along the checkpoint's left axis), and how high
/// above it everyone drops from.
const SIDE_OFFSET: f32 = 120.0;
const DROP_HEIGHT: f32 = 50.0;

/// Where the crew arrived.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arrival {
    /// The checkpoint whose OID is the arrival id.
    Matched { checkpoint: i32 },
    /// A checkpoint answering to the arrival id by alias (option 843 for 424, 844 for 425).
    Alias { checkpoint: i32 },
    /// No checkpoint matched: the world's first checkpoint.
    FirstCheckpoint { checkpoint: i32 },
    /// No checkpoints: the characters' own placements.
    Placement,
}

/// The point chosen for an arrival: its world matrix, how it was chosen, and its option-848 placements
/// ((object ID, marker object ID) pairs).
struct ArrivalAt {
    transform: Affine,
    how: Arrival,
    place: Vec<(i32, i32)>,
}

/// The collision world of a loaded Sly 3 world (module notes).
pub fn collision_from_world(world: &World) -> CollisionWorld {
    let mut cw = CollisionWorld::new();
    let mut shapes: BTreeMap<u32, Vec<engine::collision::world::ShapeId>> = BTreeMap::new();
    for h in world.handles() {
        if !world.in_world(h) || Member::from_class(world.class(h).unwrap_or(-1)).is_some() {
            continue;
        }
        let Some(Body::Solid(s)) = world.sly3_record(h).map(|r| &r.body) else { continue };
        if s.collision.is_empty() {
            continue;
        }
        let Some(rec) = world.record(h) else { continue };
        let ids = shapes
            .entry(rec)
            .or_insert_with(|| {
                s.collision
                    .iter()
                    .filter(|p| !p.mesh.is_empty())
                    .map(|p| {
                        let mesh = formats_sly3::collision::with_polygons(&p.mesh);
                        cw.add_shape(Shape { main: TriMesh::new_wide(&mesh, p.tree.clone()), camera: None })
                    })
                    .collect()
            })
            .clone();
        let Some(t) = world.world_transform(h) else { continue };
        for id in ids {
            cw.add_body(id, t, 0, 0, true);
        }
    }
    cw.rebuild();
    cw
}

/// The heading of a matrix's first axis around +Z.
fn yaw_of(t: &Affine) -> f32 {
    engine::math::atan2(t.cols[0][1], t.cols[0][0])
}

/// The crew as the world's player.
pub struct CrewPlayer {
    /// The crew, once loaded.
    pub crew: Option<Crew>,
    /// Where it arrived.
    pub arrival: Option<Arrival>,
    /// The arrival id asked for.
    pub arrival_id: i32,
    /// Classes counted as checkpoints (40 and the classes derived from it).
    pub checkpoint_classes: Vec<i16>,
    /// The collision world the crew moves through (built on load).
    pub(crate) collision: Option<CollisionWorld>,
    /// The core three's objects (Sly, Murray, Bentley).
    pub(crate) handles: [Option<Handle>; 3],
    /// Sly 3 pose hook: the core three's clips by state (`engine::anim::sly3_pose`).
    pub clips: engine::anim::sly3_pose::CrewClips,
    /// Sly 3 hook (gadget grids): the item table in the gang's form (`Sly3ItemTable::to_item_table`), when the
    /// executable gave one; the crew's slots and ownership read it.
    pub items: Option<engine::gang::ItemTable>,
    /// The Carmelita copy being driven (module notes), once the played member stands in for her.
    pub carmelita: Option<Handle>,
}

impl CrewPlayer {
    /// A crew arriving at `arrival_id`, checkpoints being objects of `checkpoint_classes`.
    pub fn new(arrival_id: i32, checkpoint_classes: Vec<i16>) -> CrewPlayer {
        CrewPlayer {
            crew: None,
            arrival: None,
            arrival_id,
            checkpoint_classes,
            collision: None,
            handles: [None; 3],
            clips: Default::default(), // Sly 3 pose hook
            items: None,
            carmelita: None,
        }
    }

    /// The member being played.
    pub fn current(&self) -> Option<Member> {
        self.crew.as_ref().map(Crew::player)
    }

    /// The collision world the crew moves through.
    pub fn collision(&self) -> Option<&CollisionWorld> {
        self.collision.as_ref()
    }

    fn write_back(&self, world: &mut World) {
        let Some(crew) = self.crew.as_ref() else { return };
        for c in &crew.gang.members {
            let Some(h) = self.handles[c.who.index()] else { continue };
            let (s, co) = (engine::math::sin(c.yaw), engine::math::cos(c.yaw));
            let t = Affine { cols: [[co, s, 0.0], [-s, co, 0.0], [0.0, 0.0, 1.0]], t: c.position };
            world.set_world_transform(h, t);
        }
        world.player = self.handles[crew.gang.player.index()];
    }

    /// The arrival checkpoint: (its transform, how it was chosen).
    /// The point the crew arrives at: by object ID, then by alias (level-flow.md 5.2), else the first checkpoint.
    fn arrival_point(&self, world: &World) -> Option<ArrivalAt> {
        if let Some(a) = super::flow::find_arrival(world, self.arrival_id, &self.checkpoint_classes) {
            let how =
                if a.alias { Arrival::Alias { checkpoint: a.oid } } else { Arrival::Matched { checkpoint: a.oid } };
            return Some(ArrivalAt { transform: world.world_transform(a.handle)?, how, place: a.place });
        }
        let h = world.handles().find(|&h| world.class(h).is_some_and(|c| self.checkpoint_classes.contains(&c)))?;
        let how = Arrival::FirstCheckpoint { checkpoint: world.sly3_id(h)? };
        Some(ArrivalAt { transform: world.world_transform(h)?, how, place: Vec::new() })
    }
}

impl PlayerSystem for CrewPlayer {
    /// Sly 3 hook (gadget grids): the core characters, for the assignment screen.
    fn gang_mut(&mut self) -> Option<&mut engine::gang::Gang> {
        self.crew.as_mut().map(|c| &mut c.gang)
    }

    fn gang(&self) -> Option<&engine::gang::Gang> {
        self.crew.as_ref().map(|c| &c.gang)
    }

    fn load(&mut self, world: &mut World, _collision: &dyn CollisionQueries) {
        self.collision = Some(collision_from_world(world));
        let first = |class: i16| world.slots_of_class(class).first().and_then(|&s| world.handle(s));
        let mut present: Vec<(Member, [f32; 3], f32)> = Vec::new();
        for m in Member::ALL {
            let Some(h) = first(m.class()) else { continue };
            let Some(t) = world.world_transform(h) else { continue };
            present.push((m, t.t, yaw_of(&t)));
            if let Some(w) = m.core() {
                self.handles[w.index()] = Some(h);
            }
        }
        // The current player: Sly, then Bentley, then Murray.
        let Some(current) =
            [Member::Sly, Member::Bentley, Member::Murray].into_iter().find(|m| present.iter().any(|p| p.0 == *m))
        else {
            return;
        };
        match self.arrival_point(world) {
            Some(ArrivalAt { transform: t, how, place }) => {
                self.arrival = Some(how);
                let yaw = yaw_of(&t);
                let left = [-engine::math::sin(yaw), engine::math::cos(yaw)];
                let mut side = 0.0;
                for p in present.iter_mut().filter(|p| p.0.core().is_some()) {
                    let off = if p.0 == current {
                        0.0
                    } else {
                        side = if side > 0.0 { -side } else { -side + SIDE_OFFSET };
                        side
                    };
                    p.1 = [t.t[0] + left[0] * off, t.t[1] + left[1] * off, t.t[2] + DROP_HEIGHT];
                    p.2 = yaw;
                }
                // Option 848: characters placed at markers (Sly 24, Bentley 25, Murray 26 at three markers).
                for (object, marker) in place {
                    let Some(m) = world.find_sly3_id(marker, None).and_then(|h| world.world_transform(h)) else {
                        continue;
                    };
                    for p in present.iter_mut() {
                        let own = first(p.0.class()).and_then(|h| world.sly3_id(h));
                        if own == Some(object) {
                            p.1 = [m.t[0], m.t[1], m.t[2] + DROP_HEIGHT];
                            p.2 = yaw_of(&m);
                        }
                    }
                }
            }
            None => self.arrival = Some(Arrival::Placement),
        }
        // The current player first: `Crew::new` plays the first core member.
        present.sort_by_key(|p| p.0 != current);
        let mut sh = shared();
        if let Some(t) = &self.items {
            sh.items = t.clone(); // Sly 3 hook (gadget grids): the Sly 3 item table.
        }
        let mut crew = Crew::new(&present, sh);
        // Sly's combo definitions (class 148 under his object, characters.md 5).
        let combos = engine::gang::combo::ComboSet::for_class(world, Member::Sly.class());
        for c in &mut crew.gang.members {
            c.leave_ground();
            if c.who == Who::Sly {
                c.combo_defs = combos.clone();
            }
        }
        self.crew = Some(crew);
        self.write_back(world);
    }

    fn decide(&mut self, world: &mut World, input: &FrameInput, step: &StepInfo, _collision: &dyn CollisionQueries) {
        // Vehicles hook: while the player drives a machine the crew gets an idle pad; Carmelita is played by the
        // current member (module notes).
        let idle = FrameInput::default();
        let carmelita = world.shared.driving.filter(|&d| world.class(d) == Some(CARMELITA_CLASS));
        let input = if world.shared.driving.is_some() && carmelita.is_none() { &idle } else { input };
        let (Some(crew), Some(cw)) = (self.crew.as_mut(), self.collision.as_ref()) else { return };
        if carmelita != self.carmelita {
            self.carmelita = carmelita;
            let who = crew.gang.player;
            if let (Some(h), Some(c)) = (carmelita, crew.gang.get_mut(who))
                && let Some(t) = world.world_transform(h)
            {
                c.position = t.t;
                c.yaw = yaw_of(&t);
                c.leave_ground();
            }
        }
        // Bouncers' launches (alarms-sensors.md 5): the launched member leaves the ground at the launch velocity.
        for l in core::mem::take(&mut world.shared.launches) {
            let Some(k) = self.handles.iter().position(|&h| h == Some(l.who)) else { continue };
            let who = [Who::Sly, Who::Murray, Who::Bentley][k];
            if let Some(c) = crew.gang.get_mut(who) {
                c.leave_ground();
                c.velocity = l.velocity;
            }
        }
        // Sly 3 hook (characters.md 6, inference): the member the session names switches; the d-pad does not. The
        // member handed control comes into the world (leaving the safe house with a character adds it).
        if let Some(m) = world.shared.sly3_switch_to.take() {
            crew.switch_to(m);
            if let Some(h) = m.core().and_then(|w| self.handles[w.index()]) {
                world.set_in_world(h, true);
            }
        }
        // Sly 3 hook (missions.md 4.1a): members scripts move (0x36) or turn (0x77).
        for (h, at, yaw) in core::mem::take(&mut world.shared.sly3_crew_moves) {
            let Some(k) = self.handles.iter().position(|&o| o == Some(h)) else { continue };
            if let Some(c) = crew.gang.get_mut([Who::Sly, Who::Murray, Who::Bentley][k]) {
                if let Some(p) = at {
                    c.position = p;
                    c.leave_ground();
                }
                if let Some(y) = yaw {
                    c.yaw = y;
                }
            }
        }
        let yaw = world.shared.camera_yaw;
        let gi = GangInput {
            // Gang hook: the binocucom's view takes the left stick.
            stick: if engine::gang::binocucom_on(world) { [0.0; 2] } else { input.left },
            camera_yaw: yaw,
            held: gang_buttons(input.buttons),
            pressed: gang_buttons(input.pressed),
            camera_eye: world.shared.camera_eye,
            camera_forward: [engine::math::cos(yaw), engine::math::sin(yaw), 0.0],
            // Gang hook: the gang's stick reader fills the deflection (`player::stick`).
            deflection: 0.0,
        };
        // Sly 3 hook: the guards as targets (Sly 2's rule, `crate::sly2::gang`).
        let at = crew.gang.current().map_or([0.0; 3], |c| c.position);
        let mut targets: Vec<Target> = Vec::new();
        for h in world.handles() {
            let Some(Behavior::Sly2Guard(g)) = world.behavior(h) else { continue };
            if g.knocked_out() || !world.in_world(h) {
                continue;
            }
            let Some(p) = world.position(h).filter(|&p| dist(p, at) <= TARGET_RANGE) else { continue };
            targets.push(Target {
                id: h.index(),
                position: p,
                yaw: g.yaw,
                kind: TargetKind::Guard { robbable: !g.pocket.entries.is_empty(), launched: false },
            });
        }
        let gw = GangWorld {
            cw,
            filter: QueryFilter::default(),
            targets: &targets,
            help: 0.0,
            safehouse: false,
            pools: &[],
        };
        let events = if step.game_dt > 0.0 { crew.update(step.game_dt, &gi, &gw) } else { Vec::new() };
        let player = self.handles[crew.gang.player.index()];
        // The crew's attacks on the guards.
        if let Some(me) = player {
            let around = |world: &mut World, p: [f32; 3]| {
                for t in &targets {
                    if dist(t.position, p) <= AREA_REACH
                        && let Some(h) = world.handle(t.id)
                    {
                        world.notify(h, Notice::Attack { by: me, kind: 1 });
                    }
                }
            };
            for (_, e) in &events {
                match *e {
                    GangEvent::Punch { target: Some(id), .. } | GangEvent::Crossbow { target: Some(id), .. } => {
                        if let Some(h) = world.handle(id) {
                            world.notify(h, Notice::Attack { by: me, kind: 1 });
                        }
                    }
                    GangEvent::AirAttack(_) => around(world, at),
                    GangEvent::FlopLanded { position, .. } => around(world, position),
                    GangEvent::Dart { to, .. } => around(world, to),
                    _ => {}
                }
            }
        }
        // Sly 3 pose hook: the core three's clips for their states, posed now.
        if step.game_dt > 0.0 {
            self.clips.drive(world, &crew.gang.members, &self.handles, &events, step.game_dt);
        }
        self.write_back(world);
        // Carmelita stands where the played member is.
        if let (Some(h), Some(c)) = (self.carmelita, self.crew.as_ref().and_then(|crew| crew.gang.current())) {
            let (s, co) = (engine::math::sin(c.yaw), engine::math::cos(c.yaw));
            world
                .set_world_transform(h, Affine { cols: [[co, s, 0.0], [-s, co, 0.0], [0.0, 0.0, 1.0]], t: c.position });
            world.player = Some(h);
        }
        // Gang hook: R3's binocucom on the played member (camera.md 6).
        if let Some(me) = player
            && events.iter().any(|(_, e)| matches!(e, engine::gang::GangEvent::Binocucom))
        {
            engine::gang::toggle_binocucom(world, me, false);
        }
    }

    fn place(&mut self, world: &mut World, transform: Affine) {
        if let Some(c) = self.crew.as_mut().and_then(|crew| {
            let who: Who = crew.gang.player;
            crew.gang.get_mut(who)
        }) {
            c.position = transform.t;
            c.leave_ground();
        }
        self.write_back(world);
    }
}
