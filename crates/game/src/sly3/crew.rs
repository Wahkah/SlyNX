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
//!   player instead, so the d-pad can switch between the core three (left Sly, up Murray, right Bentley) until the
//!   safe house and the tasks hand control as the game does.
//! - Every frame each core member's position and heading are written into its object, and `World::player` is the
//!   current member's object, so the follow camera follows whoever is played.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use engine::collision::TriMesh;
use engine::collision::world::{CollisionWorld, QueryFilter, Shape};
use engine::game::systems::{CollisionQueries, FrameInput, PlayerSystem, StepInfo};
use engine::gang::sly3::{Crew, Member, shared};
use engine::gang::{GangInput, GangWorld, Who};
use engine::object::Handle;
use engine::world::World;
use formats::geometry::Affine;
use formats_sly3::stream::Body;
use platform_api::Buttons;

use crate::sly2::gang::gang_buttons;

/// The default arrival id (characters.md 2).
pub const DEFAULT_ARRIVAL: i32 = 424;
/// The checkpoint class.
pub const CHECKPOINT_CLASS: i16 = 40;
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
                    .map(|p| cw.add_shape(Shape { main: TriMesh::new_wide(&p.mesh, p.tree.clone()), camera: None }))
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
    collision: Option<CollisionWorld>,
    handles: [Option<Handle>; 3],
}

impl CrewPlayer {
    /// A crew arriving at `arrival_id`, checkpoints being objects of `checkpoint_classes`.
    pub fn new(arrival_id: i32, checkpoint_classes: Vec<i16>) -> CrewPlayer {
        CrewPlayer { crew: None, arrival: None, arrival_id, checkpoint_classes, collision: None, handles: [None; 3] }
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
        let mut crew = Crew::new(&present, shared());
        for c in &mut crew.gang.members {
            c.leave_ground();
        }
        self.crew = Some(crew);
        self.write_back(world);
    }

    fn decide(&mut self, world: &mut World, input: &FrameInput, step: &StepInfo, _collision: &dyn CollisionQueries) {
        // Vehicles hook: while the player drives a machine the crew gets an idle pad.
        let idle = FrameInput::default();
        let input = if world.shared.driving.is_some() { &idle } else { input };
        let (Some(crew), Some(cw)) = (self.crew.as_mut(), self.collision.as_ref()) else { return };
        for (b, m) in [(Buttons::LEFT, Member::Sly), (Buttons::UP, Member::Murray), (Buttons::RIGHT, Member::Bentley)] {
            if input.pressed & b.0 != 0 {
                crew.switch_to(m);
            }
        }
        let yaw = world.shared.camera_yaw;
        let gi = GangInput {
            stick: input.left,
            camera_yaw: yaw,
            held: gang_buttons(input.buttons),
            pressed: gang_buttons(input.pressed),
            camera_eye: world.shared.camera_eye,
            camera_forward: [engine::math::cos(yaw), engine::math::sin(yaw), 0.0],
        };
        let gw =
            GangWorld { cw, filter: QueryFilter::default(), targets: &[], help: 0.0, safehouse: false, pools: &[] };
        if step.game_dt > 0.0 {
            crew.update(step.game_dt, &gi, &gw);
        }
        self.write_back(world);
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
