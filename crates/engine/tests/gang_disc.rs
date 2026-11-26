//! Sly 2 character disc tests (characters.md 10, gadgets.md 8, animation.md 11): the executable's constants and item
//! table, the three characters' rigs in the nightclub hub, and the characters moving in the hub. They run when
//! `SLYNX_DISC_SLY2` names the player's own Sly 2 (USA) image and skip (passing) otherwise.

use std::sync::OnceLock;

use engine::anim::curve::{eval_scalar, eval_vector};
use engine::anim::xform::XformRig;
use engine::collision::TriMesh;
use engine::collision::world::{CollisionWorld, QueryFilter, Shape};
use engine::gang::{
    Character, Gang, GangConstants, GangEvent, GangInput, GangTunables, GangWorld, ItemTable, Owned, Shared, Who,
    bentley, button, murray, sly,
};
use engine::object::flags;
use engine::world::World as EngineWorld;
use engine::world::sly2::Sly2Options;
use formats::geometry::Affine;
use formats_sly2::anim::{Clip, character_clips};
use formats_sly2::exe::Executable;
use formats_sly2::scene::{SceneOptions, build};
use formats_sly2::stream::{Body, Record, visit};
use formats_sly2::{Disc, World};

/// The nightclub hub, `f_nightclub_exterior`.
const HUB_WORLD_ID: u32 = 0x41;

fn disc_path() -> Option<String> {
    std::env::var("SLYNX_DISC_SLY2").ok().filter(|p| !p.is_empty())
}

macro_rules! need_disc {
    () => {
        if disc_path().is_none() {
            eprintln!("skipped: SLYNX_DISC_SLY2 is not set");
            return;
        }
    };
}

struct Hub {
    exe: Executable,
    options: formats_sly2::exe::OptionTable,
    row: usize,
    data: Vec<u8>,
}

fn hub() -> &'static Hub {
    static H: OnceLock<Hub> = OnceLock::new();
    H.get_or_init(|| {
        let mut d = Disc::open_path(disc_path().unwrap()).unwrap();
        let row = d.find_world(HUB_WORLD_ID).expect("hub row");
        let data = d.read_world(row).unwrap();
        Hub { exe: d.executable.clone(), options: d.options.clone(), row, data }
    })
}

fn world() -> World<'static> {
    let h = hub();
    World::parse(&h.data, &h.options).unwrap()
}

fn character(w: &World<'_>, class: i16) -> Record {
    let mut found = None;
    for b in &w.bodies {
        visit(&b.objects, &mut |r| {
            if found.is_none() && r.class == class {
                found = Some(r.clone());
            }
        });
    }
    found.expect("character")
}

fn f32s(exe: &Executable, va: u32, n: usize) -> Vec<f32> {
    exe.bytes_at(va, n * 4).unwrap().as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

#[test]
fn constants_and_item_table() {
    need_disc!();
    let exe = &hub().exe;
    let c = GangConstants::from_executable(exe).unwrap();
    assert_eq!(c, GangConstants::SPEC, "executable values match the spec's");
    // characters.md 10 and gadgets.md 8, read raw.
    assert_eq!(f32s(exe, 0x002B_C920, 4), [175.0, 0.0, 0.0, 0.0]);
    assert_eq!(f32s(exe, 0x002C_08DC, 2), [0.15, 0.1]);
    assert_eq!(f32s(exe, 0x002B_FB00, 2), [600.0, 800.0]);
    assert_eq!(f32s(exe, 0x002B_DC20, 2), [1.0, 0.5]);
    let slots: Vec<i32> =
        exe.bytes_at(0x002C_3258, 12).unwrap().as_chunks::<4>().0.iter().map(|c| i32::from_le_bytes(*c)).collect();
    assert_eq!(slots, [4, 1, 2]);
    assert_eq!(c.slot_bits, [4, 1, 2]);
    let t = ItemTable::from_executable(exe).unwrap();
    assert_eq!(t.items.len(), 43);
    let rec = |n: u8| {
        let i = t.get(n).unwrap();
        [i.button, i.owner, i.price, i.cost, i.episode, i.name, i.description, i.usage]
    };
    assert_eq!(rec(7)[..7], [2, 8, 200, 0, 1, 129, 130]);
    assert_eq!(rec(42)[..7], [1, 7, 0, 1, -1, 201, 202]);
    for (n, want) in [
        (9, [2, 8, 550, 5, 3, 133, 134, 204]),
        (10, [2, 8, 1000, 15, 4, 135, 136, 205]),
        (12, [2, 8, 1200, 1, 6, 139, 140, 208]),
        (27, [-1, 7, 600, 0, 5, 171, 172, 211]),
        (29, [1, 7, 1600, 2, 7, 175, 176, 208]),
        (32, [-1, 7, 0, 0, -1, 181, 182, 205]),
        (36, [-1, 7, 0, 0, -1, 189, 190, 206]),
        (41, [2, 7, 0, 5, -1, 199, 200, 209]),
    ] {
        assert_eq!(rec(n), want, "item {n}");
    }
    for n in 0..4 {
        assert_eq!(rec(n), [-1, 0, 0, 0, 0, 0, 0, 0]);
    }
    assert_eq!(rec(4), [2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(rec(6), [1, 0, 0, 0, 0, 0, 0, 0]);
    // A changed executable is refused.
    let mut data = exe.data.clone();
    let last = data.len() - 1;
    data[last] ^= 1;
    let bad = Executable::parse(data).unwrap();
    assert!(GangConstants::from_executable(&bad).is_err());
    assert!(ItemTable::from_executable(&bad).is_err());
}

#[test]
fn gadget_and_pocket_tables() {
    need_disc!();
    use engine::gang::gadgets::{self, Effect, GadgetConstants, effect};
    use engine::gang::pocket::{GUARD_TABLE_VA, GiveConstants, LootTable, OTHER_TABLE_VA, PocketTable};
    let exe = &hub().exe;
    // gadgets.md 8.
    assert_eq!(GadgetConstants::from_executable(exe).unwrap(), GadgetConstants::SPEC);
    // pickpocket-and-pickups.md 8: the launch constants and the pickups' gravity.
    // The gravity is stored as the vector (0, 0, −2250) at 0x2C0D60.
    assert_eq!(f32s(exe, 0x002C_0D60, 3), [0.0, 0.0, -2250.0]);
    let g = GiveConstants::from_executable(exe).unwrap();
    assert_eq!(g, GiveConstants::SPEC);
    // The pocket rows of episode 1, row 0 empty, and the shared rows of episodes 2/3, 4/5 and 6/7 (guards.md 8).
    let guards = PocketTable::from_executable(exe, GUARD_TABLE_VA).unwrap();
    let others = PocketTable::from_executable(exe, OTHER_TABLE_VA).unwrap();
    for (t, coins, chance, prizes) in [
        (&guards, [6, 12], 0.4, [(30, 1116), (30, 1115), (15, 1122), (15, 1121), (5, 1119), (5, 1118)]),
        (&others, [4, 8], 0.2, [(30, 1114), (30, 1115), (15, 1120), (15, 1121), (5, 1117), (5, 1118)]),
    ] {
        let r = t.row(1).unwrap();
        assert_eq!((r.coins, r.chance, r.prizes), (coins, chance, prizes));
        assert!(t.row(0).is_none());
        for (a, b) in [(2, 3), (4, 5), (6, 7)] {
            assert_eq!(t.rows[a], t.rows[b]);
        }
        for e in 1..=8 {
            let r = t.row(e).unwrap();
            assert_eq!(r.coins, coins, "episode {e}");
            assert!(r.prizes.iter().all(|p| LootTable::record_of(p.1).is_some()));
        }
    }
    // safehouse-and-shop.md 5 and 8: 64 loot records; 0 to 5; 30 to 35 worth 0; the sum 15,425; 30 prize kinds.
    let loot = LootTable::from_executable(exe).unwrap();
    assert_eq!(loot.records.len(), 64);
    let values: Vec<i32> = loot.records.iter().map(|r| r.value).collect();
    assert_eq!(values[..6], [11, 19, 27, 17, 40, 60]);
    assert!(values[30..36].iter().all(|&v| v == 0));
    assert_eq!(values.iter().sum::<i32>(), 15_425);
    assert_eq!(values[62], 803);
    for (i, r) in loot.records.iter().take(30).enumerate() {
        assert_eq!(r.kind, 1114 + i as i32);
    }
    // The gadget grids: Sly's first cell is item 23 at (145, 155), flags 1; button use −1 items have flags 0;
    // Bentley's grid has Long Toss.
    let items = ItemTable::from_executable(exe).unwrap();
    for (who, _, n) in gadgets::GRIDS {
        let cells = gadgets::grid_from_executable(exe, who).unwrap();
        assert_eq!(cells.len(), n);
        for c in &cells {
            let rec = items.get(c.item as u8).unwrap();
            assert_eq!(c.assignable(), rec.button != -1, "{who:?} item {}", c.item);
        }
        if who == Who::Sly {
            assert_eq!((cells[0].item, cells[0].position, cells[0].flags), (23, [145.0, 155.0], 1));
        }
        if who == Who::Bentley {
            assert!(cells.iter().any(|c| c.item == 36));
        }
    }
    // The effects agree with the table: passive items have button use −1 and the "automatic" or move texts; held
    // items the "press and hold" text (208); pocket and thrown items the aim texts (209, 216); summoned items 204.
    for n in 7u8..43 {
        let r = items.get(n).unwrap();
        match effect::of(n) {
            Effect::Passive => assert_eq!(r.button, -1, "item {n}"),
            Effect::Held => assert_eq!(r.usage, 208, "item {n}"),
            Effect::Pocket | Effect::ThrownTrigger => assert!(matches!(r.usage, 209 | 216), "item {n}"),
            Effect::Summoned => assert_eq!(r.usage, 204, "item {n}"),
            Effect::None => panic!("item {n} has no effect"),
            _ => assert_ne!(r.button, -1, "item {n}"),
        }
    }
}

#[test]
fn gait_clips_match_the_gaits() {
    need_disc!();
    use engine::gang::consts::gait;
    let w = world();
    // characters.md 1.1 and 10: each gait clip's root travel (transform 1) over one play and its length.
    let checks = [
        (53, 7, 125, gait::SLY_RUN.slow),
        (53, 7, 126, gait::SLY_RUN.fast),
        (53, 7, 276, gait::SLY_MOVE.slow),
        (53, 7, 277, gait::SLY_MOVE.fast),
        (54, 9, 416, gait::MURRAY_RUN.slow),
        (54, 9, 414, gait::MURRAY_RUN.fast),
        (54, 9, 489, gait::MURRAY_WALK.slow),
        (54, 9, 488, gait::MURRAY_WALK.fast),
        (54, 9, 482, gait::MURRAY_CARRY.slow),
        (54, 9, 483, gait::MURRAY_CARRY.fast),
        (55, 8, 416, gait::BENTLEY_RUN.slow),
        (55, 8, 414, gait::BENTLEY_RUN.fast),
        (55, 8, 489, gait::BENTLEY_WALK.slow),
        (55, 8, 488, gait::BENTLEY_WALK.fast),
        (55, 8, 588, gait::BENTLEY_TIMED.fast),
        (53, 7, 281, gait::SLY_DODGE.fast),
    ];
    for (class, object, clip_id, [travel, length]) in checks {
        let c = character(&w, class);
        let r =
            character_clips(&c, None).into_iter().find(|r| r.id == clip_id).unwrap_or_else(|| panic!("clip {clip_id}"));
        let clip = Clip::of_record(w.data, r, &hub().options).unwrap();
        assert!((clip.duration - length).abs() < 1e-3, "class {class} clip {clip_id}: {}", clip.duration);
        let ch = clip.channels.iter().find(|ch| ch.target == object && ch.transform == 1).unwrap();
        let v = ch.vector.as_ref().unwrap();
        let a = eval_vector(v, 0.0).unwrap();
        let b = eval_vector(v, clip.duration).unwrap();
        let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        assert!((d - travel).abs() < 0.5, "class {class} clip {clip_id}: travel {d}");
    }
}

#[test]
fn hub_character_rigs() {
    need_disc!();
    let w = world();
    // (class, object, transforms, chains, first mirror source and target).
    for (class, id, transforms, chains, mirror) in
        [(53, 7, 139, 4, (10, 73)), (55, 8, 76, 2, (75, 52)), (54, 9, 145, 2, (11, 84))]
    {
        let c = character(&w, class);
        assert_eq!(c.id, id);
        let Body::Solid(s) = &c.body else { panic!("solid") };
        let rig = XformRig::from_list(&s.transforms);
        assert_eq!((rig.nodes.len(), rig.chains.len()), (transforms, chains), "class {class}");
        assert_eq!((rig.mirrors[0].source, rig.mirrors[0].target), mirror, "class {class}");
    }
    // Sly's run clip (object 126) posed on Sly's rig, against the formats-sly2 reference posing.
    let sly = character(&w, 53);
    let Body::Solid(s) = &sly.body else { panic!() };
    let run = character_clips(&sly, None).into_iter().find(|r| r.id == 126).unwrap();
    let clip = Clip::of_record(w.data, run, &hub().options).unwrap();
    let mut rig = XformRig::from_list(&s.transforms);
    let mut reference = formats_sly2::rig::Rig::new(&s.transforms, &Affine::IDENTITY, &[]);
    let mut reached = 0;
    for (t, root) in [
        (0.025f32, [16.0044, 0.0, 75.0]),
        (0.25, [160.0, 0.0, 75.0]),
        (0.5, [320.0, 0.0, 75.0]),
        (0.1, [64.0, 0.0, 75.0]),
        (0.4, [256.0, 0.0, 75.0]),
    ] {
        rig.reset();
        rig.apply_clip(&clip, 7, t);
        rig.update(&Affine::IDENTITY);
        let p = rig.world[1].t;
        assert!((0..3).all(|k| (p[k] - root[k]).abs() < 0.1), "t {t}: root {p:?}");
        // The swivel of the first chain (upper 55) at 0.25 s.
        if t == 0.25 {
            assert!((rig.swivel[55] + 0.21512).abs() < 1e-4, "{}", rig.swivel[55]);
        }
        let same = rig.world[73]
            .cols
            .iter()
            .flatten()
            .zip(rig.world[10].cols.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-5);
        assert!(same, "mirror 10 to 73");
        reference.reset();
        for ch in clip.channels.iter().filter(|c| c.target == 7) {
            if let Some(v) = ch.vector.as_ref().and_then(|v| eval_vector(v, t)) {
                reference.set_vector(ch.transform as usize, v);
            }
            if let Some(a) = ch.scalar.as_ref().and_then(|c| eval_scalar(c, t)) {
                reference.set_swivel(ch.transform as usize, a);
            }
        }
        reference.update(&Affine::IDENTITY, &[]);
        let mut worst = 0.0f32;
        for (a, b) in rig.world.iter().zip(&reference.world) {
            for k in 0..3 {
                worst = worst.max((a.t[k] - b.t[k]).abs());
            }
        }
        assert!(worst < 0.5, "t {t}: positions differ from the reference posing by {worst}");
        for k in 0..rig.chains.len() {
            let ch = &rig.chains[k];
            let (goal, eff) = (rig.chain_goal(k), rig.chain_effector(k));
            let up = rig.world[ch.upper].t;
            let d = dist(goal, up);
            if d > (ch.a - ch.b).abs() + 0.5 && d < ch.a + ch.b - 0.5 {
                assert!(dist(goal, eff) < 0.05, "t {t}, chain {k}: {}", dist(goal, eff));
                reached += 1;
            }
        }
        let v = XformRig::root_velocity(&clip, 7, t).unwrap();
        assert!((v[0] - 640.0).abs() < 1.0, "root motion {v:?}");
    }
    assert!(reached >= 10, "chains in reach and reached: {reached}");
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The hub's collision: every in-world solid's collision parts (classes/solid-object.md 2, as `formats-sly2` exposes
/// them) at the solid's world transform, from the hub loaded into the object model at its level row, without the
/// characters and what they hold. Also returns each character's drawn position.
fn hub_collision(w: &World<'_>) -> (CollisionWorld, Vec<(Who, [f32; 3])>) {
    let b = build(w, SceneOptions { row: Some(hub().row as i16), decode_textures: false, ..SceneOptions::default() });
    let scene = &b.scene;
    let chars: Vec<(Who, u32, u32)> = [(Who::Sly, 53), (Who::Murray, 54), (Who::Bentley, 55)]
        .into_iter()
        .map(|(who, class)| {
            let r = character(w, class);
            (who, r.header_offset, r.body_offset + r.body_size)
        })
        .collect();
    let mut found = Vec::new();
    for inst in &scene.instances {
        if let Some(&(who, ..)) = chars.iter().find(|c| inst.record >= c.1 && inst.record < c.2)
            && !found.iter().any(|f: &(Who, [f32; 3])| f.0 == who)
        {
            found.push((who, inst.transform.t));
        }
    }
    let opts = Sly2Options { row: Some(hub().row as i16), ..Sly2Options::default() };
    let ew = EngineWorld::load_sly2(hub().data.clone(), &hub().options, opts).unwrap();
    let mut cw = CollisionWorld::new();
    let mut shapes = std::collections::BTreeMap::new();
    for h in ew.handles() {
        if ew.flags(h) & flags::ACTIVE == 0 {
            continue;
        }
        let mut a = Some(h);
        let mut held = false;
        while let Some(x) = a {
            held |= matches!(ew.class(x), Some(53..=55));
            a = ew.parent(x);
        }
        let Some(r) = ew.sly2_record(h) else { continue };
        let Body::Solid(s) = &r.body else { continue };
        if held || s.collision.is_empty() {
            continue;
        }
        let at = ew.world_transform(h).unwrap();
        for (k, part) in s.collision.iter().enumerate() {
            let shape = *shapes
                .entry((r.header_offset, k))
                .or_insert_with(|| cw.add_shape(Shape { main: TriMesh::new(&part.mesh, &part.tree), camera: None }));
            cw.add_body(shape, at, 0, 0, true);
        }
    }
    cw.rebuild();
    (cw, found)
}

fn shared() -> Shared {
    let exe = &hub().exe;
    Shared {
        consts: GangConstants::from_executable(exe).unwrap(),
        tun: GangTunables::default(),
        items: ItemTable::from_executable(exe).unwrap(),
        owned: Owned::default(),
        game: engine::gang::GangGame::Sly2,
    }
}

const DT: f32 = 1.0 / 60.0;

#[test]
fn gang_moves_in_the_hub() {
    need_disc!();
    let w = world();
    let (cw, placed) = hub_collision(&w);
    eprintln!("characters drawn at {placed:?}");
    assert_eq!(placed.len(), 3, "all three characters are present in the hub");
    let filter = QueryFilter::default();
    let world = GangWorld { cw: &cw, filter, targets: &[], help: 0.0, safehouse: false, pools: &[] };
    let s = shared();
    // Murray and Bentley are drawn parked off the playing area (about x = 40,000) while Sly is the player. Sly is drawn
    // perched over a gap in the level's collision, so start the three on the nearest collision floor within 300 units
    // below his drawn height (searched outward on a 40-unit grid), side by side, and drop them onto it.
    let p0 = placed.iter().find(|p| p.0 == Who::Sly).unwrap().1;
    let floor_at = |x: f32, y: f32| {
        cw.ray_cast([x, y, p0[2] + 100.0], [0.0, 0.0, -1.0], 400.0, &filter)
            .filter(|h| h.normal[2] > 0.7)
            .map(|h| h.point[2])
    };
    let mut start = None;
    'search: for ring in 0..20 {
        for i in -ring..=ring {
            for j in -ring..=ring {
                if i32::max(i32::abs(i), i32::abs(j)) != ring {
                    continue;
                }
                let (x, y) = (p0[0] + 40.0 * i as f32, p0[1] + 40.0 * j as f32);
                if [-120.0, 0.0, 120.0].iter().all(|dy| floor_at(x, y + dy).is_some()) {
                    start = Some([x, y]);
                    break 'search;
                }
            }
        }
    }
    let start = start.expect("a floor near Sly's drawn position");
    let mut members = Vec::new();
    let mut placed = placed.clone();
    for (who, p) in placed.iter_mut() {
        let dy = [0.0, 120.0, -120.0][who.index()];
        let z = floor_at(start[0], start[1] + dy).unwrap();
        *p = [start[0], start[1] + dy, z];
        let mut c = Character::new(*who, [p[0], p[1], p[2] + 50.0], 0.0);
        c.leave_ground();
        members.push(c);
    }
    let mut gang = Gang::new(members, s);
    gang.player = Who::Sly;
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(gang.update(DT, &GangInput::default(), &world));
    }
    for c in &gang.members {
        let p = placed.iter().find(|p| p.0 == c.who).unwrap().1;
        assert!(c.grounded, "{:?} lands: {:?}", c.who, c.position);
        assert!(
            (c.position[2] - p[2]).abs() < 100.0,
            "{:?} on the floor below its start: {:?} vs {p:?}",
            c.who,
            c.position
        );
        assert_eq!(c.state, 0, "{:?} stands", c.who);
    }
    // Each character in turn runs forward for 1 s, then jumps and lands.
    for who in [Who::Sly, Who::Murray, Who::Bentley] {
        assert!(gang.switch_to(who));
        let start = gang.current().unwrap().position;
        let run = GangInput {
            stick: [0.0, 1.0],
            held: button::R1,
            camera_yaw: gang.current().unwrap().yaw,
            ..GangInput::default()
        };
        for _ in 0..60 {
            gang.update(DT, &run, &world);
        }
        let c = gang.current().unwrap();
        let moved = dist(c.position, start);
        // characters.md 1.1: the runs are 832, 800 and 750.
        let want = [832.0, 800.0, 750.0][who.index()];
        eprintln!("{who:?} ran {moved:.1} (state {}, {:?})", c.state, c.position);
        assert!(moved > 0.2 * want, "{who:?} moved only {moved}");
        assert!(moved < 1.05 * want, "{who:?} moved {moved}");
        let run_state = match who {
            Who::Sly => sly::RUN,
            Who::Murray => murray::RUN,
            Who::Bentley => bentley::MOVE,
        };
        assert!(c.state == run_state || c.state == 2, "{who:?} state {}", c.state);
        // Settle: the run may end off a ledge of the hub's real collision; jump once standing again.
        for k in 0..240 {
            gang.update(DT, &GangInput::default(), &world);
            if k >= 30 && gang.current().unwrap().grounded && gang.current().unwrap().state == 0 {
                break;
            }
        }
        let mut jumped = false;
        let mut landed = false;
        events.clear();
        events.extend(gang.update(
            DT,
            &GangInput { pressed: button::CROSS, held: button::CROSS, ..GangInput::default() },
            &world,
        ));
        for _ in 0..180 {
            events.extend(gang.update(DT, &GangInput::default(), &world));
        }
        for (w2, e) in &events {
            if *w2 == who {
                jumped |= *e == GangEvent::Jumped;
                landed |= *e == GangEvent::Landed;
            }
        }
        assert!(jumped && landed, "{who:?} jumped {jumped}, landed {landed}");
        assert!(gang.current().unwrap().grounded);
    }
    // Murray's Thunder Flop from a jump lands on the hub floor.
    assert!(gang.switch_to(Who::Murray));
    events.clear();
    gang.update(DT, &GangInput { pressed: button::CROSS, held: button::CROSS, ..GangInput::default() }, &world);
    for _ in 0..15 {
        gang.update(DT, &GangInput::default(), &world);
    }
    events.extend(gang.update(
        DT,
        &GangInput { pressed: button::SQUARE, held: button::SQUARE, ..GangInput::default() },
        &world,
    ));
    for _ in 0..180 {
        events.extend(gang.update(DT, &GangInput::default(), &world));
    }
    assert!(events.contains(&(Who::Murray, GangEvent::FlopStarted)));
    assert!(events.iter().any(|e| matches!(e, (Who::Murray, GangEvent::FlopLanded { inferno: false, .. }))));
    assert_eq!(gang.current().unwrap().state, murray::STAND);
}

#[test]
fn bentley_takes_places_and_returns_a_bomb() {
    use engine::gang::pool;
    let Some(_) = disc_path() else {
        eprintln!("skipped: SLYNX_DISC_SLY2 is not set");
        return;
    };
    let opts = Sly2Options { row: Some(hub().row as i16), ..Sly2Options::default() };
    let mut w = EngineWorld::load_sly2(hub().data.clone(), &hub().options, opts).unwrap();
    let bentley = w.handles().find(|&h| w.class(h) == Some(55)).expect("Bentley");
    let pool_group = pool::pool_group(&w, bentley, pool::BOMB_CLASS).expect("Bentley's pool");
    // gadgets.md 6: taking the Trigger Bomb moves a copy out of the pool to his hands, in the world, held.
    let copy = pool::take(&mut w, bentley, 7).expect("a bomb copy");
    assert_eq!((w.class(copy), w.parent(copy), w.in_world(copy)), (Some(pool::BOMB_CLASS), Some(bentley), true));
    assert_eq!(pool::copy_of(&w, 7).map(|g| (g.state, g.pool)), Some((0, pool_group)));
    // Placed: at the world root, landed.
    pool::release(&mut w, copy, [100.0, 200.0, 0.0], 0.0, true);
    assert_eq!((w.parent(copy), pool::copy_of(&w, 7).map(|g| g.state)), (None, Some(2)));
    assert_eq!(w.world_transform(copy).map(|t| t.t), Some([100.0, 200.0, 0.0]));
    // Triggered: state 3 and counting down 1.87 s, still placed; then it goes off and returns to the pool's group,
    // out of the world.
    pool::trigger(&mut w, copy);
    assert!(pool::step(&mut w, 1.8).is_empty());
    assert_eq!((pool::copy_of(&w, 7).map(|g| g.state), w.in_world(copy)), (Some(3), true));
    assert_eq!(pool::step(&mut w, 0.1), vec![copy]);
    assert_eq!((w.parent(copy), w.in_world(copy), pool::copy_of(&w, 7)), (Some(pool_group), false, None));
}

#[test]
fn canes_and_throw_marker_follow_the_state() {
    use engine::gang::{Character, parts, sly};
    let Some(_) = disc_path() else {
        eprintln!("skipped: SLYNX_DISC_SLY2 is not set");
        return;
    };
    let opts = Sly2Options { row: Some(hub().row as i16), ..Sly2Options::default() };
    let mut w = EngineWorld::load_sly2(hub().data.clone(), &hub().options, opts).unwrap();
    let sly_h = w.handles().find(|&h| w.class(h) == Some(53)).expect("Sly");
    let find = |w: &EngineWorld, id: i32| w.find_sly2_id(id, Some(sly_h)).unwrap();
    let (shaft, second, marker) = (find(&w, parts::CANE), find(&w, parts::CANE_AIR), find(&w, parts::THROW_MARKER));
    // characters.md 9.3.1: one cane at a time, 318 in the air; none while hiding; the marker only while aiming.
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    parts::update(&mut w, sly_h, &c, false);
    assert_eq!((w.in_world(shaft), w.in_world(second), w.in_world(marker)), (true, false, false));
    // An ordinary jump keeps the first cane; the pipe climb, after its grip, takes the second.
    c.state = sly::AIR;
    parts::update(&mut w, sly_h, &c, false);
    assert_eq!((w.in_world(shaft), w.in_world(second)), (true, false));
    c.state = sly::CLIMB;
    c.state_time = 0.5;
    parts::update(&mut w, sly_h, &c, false);
    assert_eq!((w.in_world(shaft), w.in_world(second)), (false, true));
    c.state = sly::STAND;
    parts::update(&mut w, sly_h, &c, true);
    assert_eq!((w.in_world(shaft), w.in_world(second)), (false, false));
    c.state = sly::THROW_AIM;
    c.aim_path = vec![[0.0; 3], [100.0, 0.0, 50.0]];
    assert_eq!(parts::update(&mut w, sly_h, &c, false), Some(marker));
    assert!(w.in_world(marker) && w.in_world(shaft));
    c.state = sly::STAND;
    parts::update(&mut w, sly_h, &c, false);
    assert!(!w.in_world(marker));
}
