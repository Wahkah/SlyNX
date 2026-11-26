//! The gang in a Sly 2 world: `engine::gang` (Sly, Murray and Bentley) behind the game loop's [`PlayerSystem`].
//!
//! - **Collision** ([`collision_from_world`]): every active solid's collision parts (`formats-sly2`'s
//!   `Solid::collision`: mesh and tree) as fixed bodies at the solid's world transform, the characters' own solids
//!   left out. A world without any falls back to its drawn geometry ([`collision_from_scene`]).
//! - **Characters.** The three character records (classes 53 Sly, 54 Murray, 55 Bentley) are world objects; the gang
//!   starts beside Sly's drawn position (Murray and Bentley are stored parked away from the play area), drops to the
//!   floor, and each frame writes every member's position and heading into its object. `World::player` is the
//!   current member's object, so the follow camera follows whoever is played.
//! - **Input**: the pad as the gang's buttons (B jump, A interact, Y attack, X triangle, ZR run, L / ZL / R the
//!   gadget slots, right stick click the binocucom), camera-relative.
//! - **Switching** (characters.md 8): the game offers it at the safe house and per job task, which do not exist
//!   here yet; until they do, the d-pad switches (left Sly, up Murray, right Bentley). Hand-offs inside the gang
//!   (Bentley landing after a toss) change the player as `Gang::update` decides.
//!
//! - **Guards** (`engine::behavior::sly2`): every guard within [`TARGET_RANGE`] that is not knocked out is a target
//!   for the gang (robbable when it has something in its pocket). The gang's attacks become `Notice::Attack` on the
//!   guards they reach (a punch or crossbow swing on its target; an air attack, a flop landing or a dart around the
//!   point), a pickpocket `Notice::Pickpocket`; the guards' attacks on the player (`World::hurt_player`) become hits
//!   on the played character (the guards' strikes come as `World::shared.gang_hits` with their contact kind and value,
//!   guards.md 6.3; other damage as [`GUARD_HIT`]).
//!
//! The characters are not drawn: the Sly 2 scene build leaves skinned meshes out.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use engine::behavior::Behavior;
use engine::collision::TriMesh;
use engine::collision::world::{CollisionWorld, QueryFilter, Shape};
use engine::game::systems::{CollisionQueries, FrameInput, PlayerSystem, StepInfo};
use engine::gang::{
    Character, Entry, Gang, GangEvent, GangInput, GangWorld, Hit, Purse, Shared, Target, TargetKind, Who, button,
};
use engine::object::Handle;
use engine::world::Notice;
use engine::world::World;
use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle};
use formats::geometry::Affine;
use formats::scene::Scene;
use platform_api::Buttons;

/// Classes of the three characters' records.
pub const CHARACTER_CLASSES: [(Who, i16); 3] = [(Who::Sly, 53), (Who::Murray, 54), (Who::Bentley, 55)];

/// Where Murray and Bentley start beside Sly (y offsets), and how high above the drawn position everyone drops from.
const START_OFFSETS: [f32; 3] = [0.0, 120.0, -120.0];
const DROP_HEIGHT: f32 = 50.0;

/// Guards farther than this from the played character are not offered to the gang as targets.
pub const TARGET_RANGE: f32 = 3000.0;
/// Reach around an air attack, a dart's end and a flop landing. *Placeholder*: characters.md gives no radii.
pub const AREA_REACH: f32 = 250.0;
/// A guard attack on the gang. *Placeholder*: guards.md 11 leaves the damage open.
pub const GUARD_HIT: f32 = 1.0;

/// How close the played character must come to spilled coins to collect them. *Placeholder* until the coins are
/// launched as pickups (`gang::pocket::give`).
pub const COLLECT_REACH: f32 = 150.0;

/// How long a character flashes after a hit, taking no further damage (characters.md 2).
const FLASH_TIME: f32 = 2.0;

/// Guards and creatures: class 56 and its derived classes (object-stream.md 3.1).
const ACTOR_CLASS: i32 = 56;

/// Interaction spots and the option naming the sequence they start (options-by-class.md 2.3).
pub const SPOT_CLASS: i16 = 35;
/// See [`SPOT_CLASS`].
pub const SPOT_SEQUENCE: u16 = 1070;

/// The pad as the gang's buttons.
pub fn gang_buttons(bits: u32) -> u32 {
    let map = [
        (Buttons::B, button::CROSS),
        (Buttons::A, button::CIRCLE),
        (Buttons::Y, button::SQUARE),
        (Buttons::X, button::TRIANGLE),
        (Buttons::ZR, button::R1),
        (Buttons::RSTICK, button::R3),
        (Buttons::L, button::L1),
        (Buttons::ZL, button::L2),
        (Buttons::R, button::R2),
    ];
    map.iter().filter(|(b, _)| bits & b.0 != 0).fold(0, |acc, (_, g)| acc | g)
}

/// The collision world of a Sly 2 world: each active solid's collision parts at its world transform, except the
/// characters' (classes 53 to 55 and what hangs below them). Returns `None` when the world has none.
pub fn collision_from_world(world: &World) -> Option<CollisionWorld> {
    let mut cw = CollisionWorld::new();
    let mut shapes = BTreeMap::new();
    let mut bodies = 0;
    for h in world.handles() {
        if world.flags(h) & engine::object::flags::ACTIVE == 0 {
            continue;
        }
        // Not the characters' bodies, nor the guards' and creatures' (class 56 and derived): they move, and fixed
        // bodies left where they were loaded are floors and walls that are not there (a rat's at its post).
        let mut a = Some(h);
        let mut held = false;
        while let Some(x) = a {
            held |= world.class(x).is_some_and(|c| {
                CHARACTER_CLASSES.iter().any(|k| k.1 == c) || engine::world::sly2::is_a(c, ACTOR_CLASS)
            });
            a = world.parent(x);
        }
        let Some(r) = world.sly2_record(h) else { continue };
        let formats_sly2::stream::Body::Solid(s) = &r.body else { continue };
        if held || s.collision.is_empty() {
            continue;
        }
        // Water's parts are sensors; barriers only stop what their lists say (interactables.md 6, 7).
        match world.class(h) {
            Some(89) => continue,
            Some(86) if !engine::behavior::sly2_world::barrier_blocks_characters(world, h) => continue,
            _ => {}
        }
        let Some(at) = world.world_transform(h) else { continue };
        for (k, part) in s.collision.iter().enumerate() {
            let shape = *shapes.entry((r.header_offset, k)).or_insert_with(|| {
                // Gang hook: each stored surface is a convex polygon, not only its three corners (the floor holes
                // a character fell through; formats_sly2::collision::with_polygons).
                let mesh = formats_sly2::collision::with_polygons(&part.mesh);
                cw.add_shape(Shape { main: TriMesh::new(&mesh, &part.tree), camera: None })
            });
            cw.add_body(shape, at, 0, h.index(), true);
            bodies += 1;
        }
    }
    if bodies == 0 {
        return None;
    }
    cw.rebuild();
    Some(cw)
}

/// The world's drawn geometry as a collision world (the fallback for worlds without collision parts), leaving out the draws of
/// records inside `exclude` (byte ranges of the characters' records). Also returns where each excluded range was
/// first drawn.
pub fn collision_from_scene(scene: &Scene, exclude: &[(u32, u32)]) -> (CollisionWorld, Vec<(usize, [f32; 3])>) {
    let mut cw = CollisionWorld::new();
    let mut shapes = BTreeMap::new();
    let mut found: Vec<(usize, [f32; 3])> = Vec::new();
    for inst in &scene.instances {
        if let Some(k) = exclude.iter().position(|&(a, b)| inst.record >= a && inst.record < b) {
            if !found.iter().any(|f| f.0 == k) {
                found.push((k, inst.transform.t));
            }
            continue;
        }
        let shape = *shapes.entry(inst.mesh).or_insert_with(|| {
            let m = &scene.meshes[inst.mesh as usize];
            let mut positions: Vec<[f32; 3]> = Vec::new();
            let mut triangles = Vec::new();
            for sm in &scene.submeshes[m.submeshes.start as usize..m.submeshes.end as usize] {
                // Shadow hulls and projected volumes (shader kinds 2 and 3, rendering.md 3.4 and 3.5) are not solid.
                let kind = scene.materials.get(sm.material as usize).map(|m| m.kind);
                if positions.len() + sm.vertex_count as usize > 65_000 || matches!(kind, Some(2 | 3)) {
                    continue;
                }
                let base = positions.len() as u32;
                positions.extend(
                    scene.vertices[sm.base_vertex as usize..(sm.base_vertex + sm.vertex_count) as usize]
                        .iter()
                        .map(|v| v.position),
                );
                let idx = &scene.indices[sm.first_index as usize..(sm.first_index + sm.index_count) as usize];
                for tri in idx.as_chunks::<3>().0 {
                    let corners = tri.map(|c| (base + c as u32) as u16);
                    let p = corners.map(|c| positions[c as usize]);
                    let center = [0, 1, 2].map(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.0);
                    let radius = p.iter().map(|q| dist(*q, center)).fold(0.0, f32::max);
                    triangles.push(CollisionTriangle { corners, radius, center, first_edge: 0, edge_count: 0 });
                }
            }
            if triangles.is_empty() {
                return None;
            }
            let radius = positions.iter().map(|p| dist(*p, [0.0; 3])).fold(0.0, f32::max);
            let mesh = CollisionMesh { radius, positions, triangles, edges: Vec::new() };
            Some(cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None }))
        });
        if let Some(shape) = shape {
            cw.add_body(shape, inst.transform, 0, u32::MAX, true);
        }
    }
    cw.rebuild();
    (cw, found)
}

/// Contact kind `0x8000` (the pop-up cobra and tentacle, characters.md 2.2): knocked back exactly as `0x200`.
pub const KNOCK_POPUP: u32 = 0x8000;
/// Knock-back (`0x200`).
pub const KNOCK: u32 = 0x200;
/// The speed of a knock-back away from its sender. *Placeholder*: the game uses the sender's own motion, which the
/// guards (moved without their clips' root motion) do not have yet.
pub const KNOCK_SPEED: f32 = 600.0;
/// A walking sender's knock-back boost (characters.md 2.2): times 1 + 0.25, at most 1,200, at least 700 up.
pub const KNOCK_BOOST: (f32, f32, f32) = (1.25, 1200.0, 700.0);

/// The kind bits the gang's hit rules see: `0x8000` is a knock-back.
pub fn hit_kind(kind: u32) -> u32 {
    if kind & KNOCK_POPUP != 0 { kind | KNOCK } else { kind }
}

/// The knock-back vector of a hit from `from` on a character at `to` (characters.md 2.2): away from the sender,
/// boosted as a walking sender's; no vector for other kinds.
pub fn knock_vector(from: [f32; 3], to: [f32; 3], kind: u32) -> [f32; 3] {
    if kind & (KNOCK | KNOCK_POPUP) == 0 {
        return [0.0; 3];
    }
    let d = [to[0] - from[0], to[1] - from[1], 0.0];
    let l = engine::math::length(d);
    let away = if l > 1e-3 { [d[0] / l, d[1] / l, 0.0] } else { [1.0, 0.0, 0.0] };
    let (gain, max, up) = KNOCK_BOOST;
    let mut v = [away[0] * KNOCK_SPEED * gain, away[1] * KNOCK_SPEED * gain, 0.0];
    let s = engine::math::length(v);
    if s > max {
        v = [v[0] * max / s, v[1] * max / s, 0.0];
    }
    v[2] = v[2].max(up);
    v
}

/// Changes a guard's or creature's behaviour; false when `h` is none.
fn set_guard(world: &mut World, h: Handle, f: impl FnOnce(&mut engine::behavior::sly2::Sly2Guard)) -> bool {
    match world.behavior_mut(h) {
        Some(Behavior::Sly2Guard(g)) => {
            f(g);
            true
        }
        _ => false,
    }
}

/// How high above Murray's base a carried guard rides. *Placeholder*: the carry attaches it to his hands.
const CARRY_HEIGHT: f32 = 200.0;
/// A dropped load's velocity in Murray's frame, 400 backward and 600 up (characters.md 4.6).
const DROP_BACK: f32 = 400.0;
const DROP_UP: f32 = 600.0;

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    engine::math::length(d)
}

/// The gang as the world's player.
pub struct GangPlayer {
    /// The gang, once loaded.
    pub gang: Option<Gang>,
    shared: Option<Shared>,
    collision: CollisionWorld,
    /// Sly's drawn position.
    start: Option<[f32; 3]>,
    /// Each member's world object.
    handles: [Option<Handle>; 3],
    /// Coins and loot collected (pickpocketed entries, coins spilled by knocked-out guards).
    pub purse: Purse,
    /// The safe house was the current player last frame.
    in_safe_house: bool,
    /// Sly 2 pose hook: the characters' clips by state (`engine::anim::sly2_gang`).
    pub clips: engine::anim::sly2_gang::GangClips,
    /// The guard or creature Murray carries (characters.md 4.6).
    carried: Option<Handle>,
    /// Seconds each member still flashes after a hit, taking no further damage (characters.md 2), by member.
    flashing: [f32; 3],
}

impl GangPlayer {
    /// The gang with the executable's shared values, moving through `collision`, starting near `start`.
    pub fn new(shared: Option<Shared>, collision: CollisionWorld, start: Option<[f32; 3]>) -> GangPlayer {
        GangPlayer {
            gang: None,
            shared,
            collision,
            start,
            handles: [None; 3],
            purse: Purse::default(),
            in_safe_house: false,
            clips: Default::default(), // Sly 2 pose hook
            carried: None,
            flashing: [0.0; 3],
        }
    }

    /// The current member.
    pub fn current(&self) -> Option<Who> {
        self.gang.as_ref().map(|g| g.player)
    }

    fn write_back(&self, world: &mut World) {
        let Some(g) = self.gang.as_ref() else { return };
        for c in &g.members {
            let Some(h) = self.handles[c.who.index()] else { continue };
            let (s, co) = (engine::math::sin(c.yaw), engine::math::cos(c.yaw));
            let t = Affine { cols: [[co, s, 0.0], [-s, co, 0.0], [0.0, 0.0, 1.0]], t: c.position };
            world.set_world_transform(h, t);
            // Gang hook: the canes and the throw marker by the character's state (characters.md 9.3.1).
            let hiding = c.who == g.player && world.shared.player.hiding != engine::world::shared::Hiding::None;
            engine::gang::parts::update(world, h, c, hiding);
        }
        world.player = self.handles[g.player.index()];
        // Sly 2 hook (HUD): the current character's vitals.
        if let (Some(l), Some(c)) = (world.sly2.as_deref_mut(), g.members.iter().find(|c| c.who == g.player)) {
            l.hud.who = Some(g.player.index() as u8);
            l.hud.health = (c.health.value, c.health.capacity);
            l.hud.power = (c.power.max > 0.0).then_some((c.power.value, c.power.max));
        }
    }
}

impl PlayerSystem for GangPlayer {
    fn gang_mut(&mut self) -> Option<&mut Gang> {
        self.gang.as_mut() // Sly 2 hook (menus).
    }

    fn gang(&self) -> Option<&Gang> {
        self.gang.as_ref()
    }

    fn load(&mut self, world: &mut World, _collision: &dyn CollisionQueries) {
        let Some(shared) = self.shared.take() else { return };
        for (who, class) in CHARACTER_CLASSES {
            self.handles[who.index()] = world.slots_of_class(class).first().and_then(|&s| world.handle(s));
        }
        let start = self.start.or_else(|| self.handles[0].and_then(|h| world.position(h)));
        let Some(p0) = start else { return };
        let members = CHARACTER_CLASSES
            .iter()
            .filter(|(who, _)| self.handles[who.index()].is_some())
            .map(|&(who, _)| {
                let mut c = Character::new(who, [p0[0], p0[1] + START_OFFSETS[who.index()], p0[2] + DROP_HEIGHT], 0.0);
                c.leave_ground();
                c
            })
            .collect::<Vec<_>>();
        if members.is_empty() {
            return;
        }
        let mut gang = Gang::new(members, shared);
        if gang.get(Who::Sly).is_some() {
            gang.player = Who::Sly;
        }
        self.gang = Some(gang);
        self.write_back(world);
    }

    fn decide(&mut self, world: &mut World, input: &FrameInput, step: &StepInfo, _collision: &dyn CollisionQueries) {
        // Vehicles hook: while the player drives a machine the gang gets an idle pad.
        let idle = FrameInput::default();
        let input = if world.shared.driving.is_some() { &idle } else { input };
        let Some(gang) = self.gang.as_mut() else { return };
        for (b, who) in [(Buttons::LEFT, Who::Sly), (Buttons::UP, Who::Murray), (Buttons::RIGHT, Who::Bentley)] {
            if input.pressed & b.0 != 0 {
                gang.switch_to(who);
            }
        }
        let gi = GangInput {
            // Gang hook: the binocucom's view takes the left stick.
            stick: if engine::gang::binocucom_on(world) { [0.0; 2] } else { input.left },
            camera_yaw: world.shared.camera_yaw,
            held: gang_buttons(input.buttons),
            pressed: gang_buttons(input.pressed),
            camera_eye: world.shared.camera_eye,
            camera_forward: {
                let (s, c) = (engine::math::sin(world.shared.camera_yaw), engine::math::cos(world.shared.camera_yaw));
                [c, s, 0.0]
            },
            // Gang hook: the gang's stick reader fills the deflection (`player::stick`).
            deflection: 0.0,
        };
        // Guards as targets.
        let at = gang.current().map_or([0.0; 3], |c| c.position);
        let mut targets: Vec<Target> = Vec::new();
        for h in world.handles() {
            let Some(Behavior::Sly2Guard(g)) = world.behavior(h) else { continue };
            if g.knocked_out() {
                continue;
            }
            let Some(p) = world.position(h) else { continue };
            if dist(p, at) > TARGET_RANGE {
                continue;
            }
            targets.push(Target {
                id: h.index(),
                position: p,
                yaw: g.yaw,
                kind: TargetKind::Guard { robbable: !g.pocket.entries.is_empty(), launched: false },
            });
        }
        // Reactive props (class 98) with events left: an attack contact is a hit (interactables.md 3.2; the cellar's
        // vent panel, missions.md 4.3).
        for h in world.handles() {
            let Some(Behavior::Sly2Prop(p)) = world.behavior(h) else { continue };
            if p.done() || !world.in_world(h) {
                continue;
            }
            let Some(p) = world.position(h).filter(|&p| dist(p, at) <= TARGET_RANGE) else { continue };
            let kind = TargetKind::Object { liftable: false, attackable: true };
            targets.push(Target { id: h.index(), position: p, yaw: 0.0, kind });
        }
        // Interaction spots (class 35) that start a sequence (option 1070).
        for &s in world.slots_of_class(SPOT_CLASS) {
            let Some(h) = world.handle(s).filter(|&h| world.in_world(h)) else { continue };
            if world.options(h).and_then(|o| o.i32(SPOT_SEQUENCE)).is_none() {
                continue;
            }
            let Some(p) = world.position(h).filter(|&p| dist(p, at) <= TARGET_RANGE) else { continue };
            targets.push(Target { id: h.index(), position: p, yaw: 0.0, kind: TargetKind::Interactable });
        }
        // Gang hook: hooks Sly can swing on (class 40), as targets.
        for &s in world.slots_of_class(engine::gang::parts::HOOK_CLASS) {
            let Some(h) = world.handle(s).filter(|&h| world.in_world(h)) else { continue };
            let Some(p) = world.position(h).filter(|&p| dist(p, at) <= TARGET_RANGE) else { continue };
            targets.push(Target { id: h.index(), position: p, yaw: 0.0, kind: TargetKind::Hook });
        }
        let gw = GangWorld {
            cw: &self.collision,
            filter: QueryFilter::default(),
            targets: &targets,
            help: 0.0,
            safehouse: world.shared.safe_house,
            // No pooled items are limited yet (gadget pools are not tracked in the world).
            pools: &[],
        };
        // The guards' attacks on the player.
        let player = world.player;
        let mut hurt = Vec::new();
        for d in world.take_damage_events() {
            if d.target.is_some() && d.target == player {
                hurt.push(d);
            } else {
                world.shared.damage.push(d);
            }
        }
        let mut events = Vec::new();
        let world_hits = core::mem::take(&mut world.shared.gang_hits);
        if let Some(who) = self.gang.as_ref().map(|g| g.player) {
            let gang = self.gang.as_mut().unwrap();
            let consts = gang.shared.consts;
            // After a hit a character flashes for 2 s and takes no further damage (characters.md 2).
            for f in &mut self.flashing {
                *f = (*f - step.game_dt).max(0.0);
            }
            let flashing = &mut self.flashing;
            for d in hurt {
                let source = world.position(d.source).unwrap_or(at);
                let h = Hit { amount: GUARD_HIT, kind: 0, source, source_yaw: 0.0, vector: [0.0; 3], shrunk: false };
                if flashing[who.index()] <= 0.0
                    && let Some(c) = gang.get_mut(who)
                {
                    c.hit(&consts, &h, &gw, &mut events);
                    flashing[who.index()] = FLASH_TIME;
                }
            }
            // Hits from the world (lasers): on whichever member they reached.
            for wh in world_hits {
                let Some(member) = CHARACTER_CLASSES.iter().find(|c| world.class(wh.target) == Some(c.1)).map(|c| c.0)
                else {
                    continue;
                };
                let to = world.position(wh.target).unwrap_or(wh.source);
                let h = Hit {
                    amount: wh.amount,
                    kind: hit_kind(wh.kind),
                    source: wh.source,
                    source_yaw: 0.0,
                    vector: knock_vector(wh.source, to, wh.kind),
                    shrunk: false,
                };
                if flashing[member.index()] <= 0.0
                    && let Some(c) = gang.get_mut(member)
                {
                    c.hit(&consts, &h, &gw, &mut events);
                    flashing[member.index()] = FLASH_TIME;
                }
            }
        }
        let gang = self.gang.as_mut().unwrap();
        if step.game_dt > 0.0 {
            let done = gang.update(step.game_dt, &gi, &gw);
            // Sly 2 pose hook: each character's clips for its state, posed by this frame's pose step.
            self.clips.drive(world, &gang.members, &self.handles, &done, step.game_dt);
            events.extend(done.into_iter().map(|(_, e)| e));
            // Gang hook: triggered bombs count down (1.87 s) before they go off and return to the pool.
            engine::gang::pool::step(world, step.game_dt);
        }
        // The gang's attacks on the guards.
        let Some(me) = player else {
            self.write_back(world);
            return;
        };
        let around = |world: &mut World, p: [f32; 3]| {
            for t in &targets {
                if dist(t.position, p) <= AREA_REACH
                    && let Some(h) = world.handle(t.id)
                {
                    world.notify(h, Notice::Attack { by: me, kind: 1 });
                }
            }
        };
        for e in events {
            match e {
                GangEvent::Punch { target: Some(id), .. } | GangEvent::Crossbow { target: Some(id), .. } => {
                    if let Some(h) = world.handle(id) {
                        world.notify(h, Notice::Attack { by: me, kind: 1 });
                    }
                }
                GangEvent::Interact(id) => {
                    // An interaction spot runs its "used by" routine with the character using it.
                    if let Some(h) = world.handle(id).filter(|&h| world.class(h) == Some(SPOT_CLASS)) {
                        engine::behavior::sly2_commands::use_spot(world, h, Some(me));
                    }
                }
                GangEvent::Pickpocket(id) => {
                    if let Some(h) = world.handle(id) {
                        world.notify(h, Notice::Pickpocket(me));
                    }
                }
                // Gang hook: gadget copies leave their pools (gadgets.md 6, `engine::gang::pool`): taken when the item
                // starts or is summoned, placed where a throw lands, returned when the bomb goes off.
                GangEvent::Summoned(item) | GangEvent::ItemStarted(item)
                    if engine::gang::pool::class_of_item(item).is_some()
                        && engine::gang::pool::copy_of(world, item).is_none_or(|g| g.state != 0) =>
                {
                    engine::gang::pool::take(world, me, item);
                }
                // A guard or creature Murray lifts is his to place until he throws it (characters.md 4.6): it flies
                // from above him with the throw's velocity (missions.md 4.3: the rats tossed into the water main).
                GangEvent::Lift(id) => {
                    if let Some(h) = world.handle(id)
                        && set_guard(world, h, |g| {
                            g.held = true;
                            g.flight = None;
                        })
                    {
                        self.carried = Some(h);
                    }
                }
                GangEvent::Thrown { id, velocity } if self.carried.is_some_and(|h| h.index() == id) => {
                    if let Some(h) = self.carried.take() {
                        set_guard(world, h, |g| {
                            g.held = false;
                            g.flight = Some((velocity, 0.0));
                        });
                    }
                }
                GangEvent::Thrown { id, velocity } if id <= 255 => {
                    if let Some(g) = engine::gang::pool::copy_of(world, id as u8).filter(|g| g.state == 0)
                        && let Some(t) = world.world_transform(me)
                    {
                        let from = [t.t[0], t.t[1], t.t[2] + 100.0];
                        let mut land = engine::gang::pool::landing(from, velocity, -2275.0, 3.0);
                        land[2] = t.t[2];
                        let yaw = engine::math::atan2(t.cols[0][1], t.cols[0][0]);
                        engine::gang::pool::release(world, g.object, land, yaw, true);
                    }
                }
                GangEvent::Triggered(item) => {
                    if let Some(g) = engine::gang::pool::copy_of(world, item).filter(|g| g.countdown.is_none()) {
                        engine::gang::pool::trigger(world, g.object);
                    }
                }
                // Gang hook: R3's binocucom (camera.md 6).
                GangEvent::Binocucom => engine::gang::toggle_binocucom(world, me, false),
                GangEvent::AirAttack(_) => around(world, at),
                GangEvent::FlopLanded { position, .. } => around(world, position),
                GangEvent::Dart { to, .. } => around(world, to),
                _ => {}
            }
        }
        // The carried guard rides above Murray; dropped without a throw (Murray hit or falling), it gets (−400, 0, 600)
        // in his frame (characters.md 4.6).
        if let Some(h) = self.carried {
            let murray = self.gang.as_ref().and_then(|g| g.get(Who::Murray));
            match murray {
                Some(m) if m.carrying == Some(h.index()) => {
                    let (s, c) = (engine::math::sin(m.yaw), engine::math::cos(m.yaw));
                    let p = [m.position[0], m.position[1], m.position[2] + CARRY_HEIGHT];
                    world.set_world_transform(h, Affine { cols: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]], t: p });
                }
                _ => {
                    let back = murray.map_or([0.0, 0.0, DROP_UP], |m| {
                        let (s, c) = (engine::math::sin(m.yaw), engine::math::cos(m.yaw));
                        [-DROP_BACK * c, -DROP_BACK * s, DROP_UP]
                    });
                    set_guard(world, h, |g| {
                        g.held = false;
                        g.flight = Some((back, 0.0));
                    });
                    self.carried = None;
                }
            }
        }
        // A character whose health ran out stays in its hit reaction (it gets up only with health left). Stand-in
        // for the defeat (characters.md 2.1: task event 0x10, then the job restarts at its checkpoint or the world
        // reloads), held back until the controller returns characters from drops to safe ground (physics.md 5.1):
        // without that, a knock off a ledge ends the job. Once it lies on the ground past the reaction time, its
        // counters are refilled (health to the capacity, power to the most) and it gets up where it is.
        if let Some(g) = self.gang.as_mut() {
            let hurt = g.shared.tun.hurt_time;
            let who = g.player;
            if let Some(c) = g.get_mut(who)
                && c.health.value <= 0.0
                && c.grounded
                && c.state_time >= hurt
            {
                c.health.value = c.health.capacity;
                c.power.value = c.power.max;
            }
        }
        // Medicine the played character touches: taken unless its health and power are both full.
        // Entering the safe house refills every character's health and gadget power (safehouse-and-shop.md 2).
        if world.shared.safe_house && !self.in_safe_house {
            for who in [Who::Sly, Who::Bentley, Who::Murray] {
                if let Some(c) = self.gang.as_mut().and_then(|g| g.get_mut(who)) {
                    c.health.value = c.health.capacity;
                    c.power.value = c.power.max;
                }
            }
        }
        self.in_safe_house = world.shared.safe_house;
        for m in core::mem::take(&mut world.shared.medicine) {
            if let Some(c) = self.gang.as_mut().and_then(|g| {
                let who = g.player;
                g.get_mut(who)
            }) && c.take_medicine()
            {
                world.set_in_world(m, false);
            }
        }
        // What was taken from pockets goes to the purse at once (the launches of `gang::pocket::give` are not
        // simulated yet); spilled coins when the played character reaches them.
        for e in core::mem::take(&mut world.shared.picked) {
            self.purse.receive(e);
        }
        let purse = &mut self.purse;
        world.shared.coin_drops.retain(|&(p, n)| {
            if dist(p, at) <= COLLECT_REACH {
                purse.receive(Entry::Coins(n));
                false
            } else {
                true
            }
        });
        self.write_back(world);
    }

    fn place(&mut self, world: &mut World, transform: Affine) {
        if let Some(c) = self.gang.as_mut().and_then(|g| {
            let who = g.player;
            g.get_mut(who)
        }) {
            // Placed at rest, facing the transform's heading (an arrival point, a warp): a fall in progress does not
            // carry on through the new floor.
            c.position = transform.t;
            c.velocity = [0.0; 3];
            c.yaw = engine::math::atan2(transform.cols[0][1], transform.cols[0][0]);
            c.leave_ground();
        }
        self.write_back(world);
    }
}

#[cfg(test)]
mod knock_tests {
    use super::*;

    #[test]
    fn popup_hits_knock_back_like_0x200() {
        assert_eq!(hit_kind(KNOCK_POPUP), KNOCK_POPUP | KNOCK);
        let v = knock_vector([0.0; 3], [10.0, 0.0, 0.0], KNOCK_POPUP);
        assert!((v[0] - 750.0).abs() < 1e-3 && v[1] == 0.0 && v[2] == 700.0);
        assert_eq!(knock_vector([0.0; 3], [10.0, 0.0, 0.0], 0x800), [0.0; 3]);
    }
}
