//! Sly 3 crew disc tests (specs/sly3/gameplay-overview.md 3 and 4, transforms.md 4, animation.md): which worlds
//! have each crew member, the core rigs and clips on `XformRig`, and the core characters moving in v_apt. They run
//! when `SLYNX_DISC_SLY3` names the player's own Sly 3 (USA) image and skip (passing) otherwise.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use engine::anim::xform::XformRig;
use engine::collision::TriMesh;
use engine::collision::world::{CollisionWorld, QueryFilter, Shape};
use engine::gang::sly3::{Crew, Member, shared};
use engine::gang::{GangEvent, GangInput, GangWorld, Who, button};
use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle};
use formats::geometry::Affine;
use formats_sly3::scene::{SceneOptions, build};
use formats_sly3::stream::{Body, Record, visit};
use formats_sly3::{Disc, World};

fn disc_path() -> Option<String> {
    std::env::var("SLYNX_DISC_SLY3").ok().filter(|p| !p.is_empty())
}

macro_rules! need_disc {
    () => {
        if disc_path().is_none() {
            eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
            return;
        }
    };
}

struct Data {
    options: formats_sly2::exe::OptionTable,
    multipliers: [f32; 3],
    /// (slot, name, bytes) for every world.
    worlds: Vec<(usize, String, Vec<u8>)>,
    exe: formats_sly3::exe::Executable,
}

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| {
        let mut d = Disc::open_path(disc_path().unwrap()).unwrap();
        let slots: Vec<usize> = d.world_slots().collect();
        let names: BTreeMap<usize, String> = ["v_apt", "v_ext", "i_trainer", "m_ext"]
            .iter()
            .filter_map(|n| d.find_world(n).map(|s| (s, n.to_string())))
            .collect();
        let worlds = slots
            .into_iter()
            .map(|s| (s, names.get(&s).cloned().unwrap_or_default(), d.read_world(s).unwrap()))
            .collect();
        Data { options: d.options.clone(), multipliers: d.tangent_multipliers, worlds, exe: d.executable.clone() }
    })
}

fn world(name: &str) -> (usize, World<'static>) {
    let d = data();
    let (slot, _, bytes) = d.worlds.iter().find(|w| w.1 == name).expect("world");
    (*slot, World::parse_with(bytes, &d.options, d.multipliers).unwrap())
}

fn each_record<'w>(w: &'w World<'_>, f: &mut impl FnMut(&'w Record)) {
    for b in &w.bodies {
        visit(&b.objects, f);
    }
}

fn first_of_class<'w>(w: &'w World<'_>, class: i16) -> Option<&'w Record> {
    let mut out = None;
    each_record(w, &mut |r| {
        if out.is_none() && r.class == class {
            out = Some(r);
        }
    });
    out
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn crew_presence_by_world() {
    need_disc!();
    // gameplay-overview.md 3: Sly in 36 worlds, Bentley 34, Murray 31; Guru 9, Panda King 6, Penelope 8, Dimitri 6,
    // Carmelita 10.
    let d = data();
    let mut count: BTreeMap<Member, usize> = BTreeMap::new();
    for (_, _, bytes) in &d.worlds {
        let w = World::parse_with(bytes, &d.options, d.multipliers).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        each_record(&w, &mut |r| {
            if let Some(m) = Member::from_class(r.class) {
                seen.insert(m);
            }
        });
        for m in seen {
            *count.entry(m).or_default() += 1;
        }
    }
    eprintln!("worlds per member: {count:?}");
    let want = [
        (Member::Sly, 36),
        (Member::Bentley, 34),
        (Member::Murray, 31),
        (Member::Guru, 9),
        (Member::PandaKing, 6),
        (Member::Penelope, 8),
        (Member::Dimitri, 6),
        (Member::Carmelita, 10),
    ];
    for (m, n) in want {
        assert_eq!(count.get(&m).copied().unwrap_or(0), n, "{m:?}");
    }
}

#[test]
fn sly_rig_and_clips_in_v_apt() {
    need_disc!();
    let (_, w) = world("v_apt");
    // transforms.md 4: Sly in v_apt is class 52, object 24: 146 transforms, 4 chains, one mirror pair 6 → 73.
    let sly = first_of_class(&w, Member::Sly.class()).unwrap();
    assert_eq!(sly.id, 24);
    let Body::Solid(s) = &sly.body else { panic!("solid") };
    let t1 = &s.transforms.transforms[1];
    assert_eq!((t1.parent, t1.kind, t1.id), (0, 1, 24));
    let mut rig = XformRig::from_list(&s.transforms);
    assert_eq!((rig.nodes.len(), rig.chains.len()), (146, 4));
    assert_eq!((rig.mirrors[0].source, rig.mirrors[0].target), (6, 73));
    rig.update(&Affine::IDENTITY);
    assert!((rig.world[1].t[2] - 75.0).abs() < 1e-3, "{:?}", rig.world[1].t);
    // Every clip in Sly's own tree poses the rig: finite matrices, the mirror's rotation copied, and chains whose
    // goal is in reach put their effector on it.
    let mut clips = Vec::new();
    visit(sly.nested(), &mut |r| {
        if let Body::Clip(c) = &r.body
            && let Some(d) = &c.data
        {
            clips.push(d);
        }
    });
    assert!(clips.len() > 100, "{} clips", clips.len());
    let (mut posed, mut reached, mut moving) = (0, 0, 0);
    for clip in &clips {
        if !clip.channels.iter().any(|c| c.target == sly.id) {
            continue;
        }
        let dur = clip.duration();
        for k in 0..4 {
            let t = dur * k as f32 / 4.0;
            rig.reset();
            rig.apply_sly3_clip(clip, sly.id, t);
            rig.update(&Affine::IDENTITY);
            assert!(rig.world.iter().all(|m| m.cols.iter().flatten().chain(m.t.iter()).all(|v| v.is_finite())));
            let same = rig.world[73].cols.iter().flatten().zip(rig.world[6].cols.iter().flatten());
            assert!(same.into_iter().all(|(a, b)| (a - b).abs() < 1e-4), "mirror 6 to 73");
            for c in 0..rig.chains.len() {
                let ch = &rig.chains[c];
                let d = dist(rig.chain_goal(c), rig.world[ch.upper].t);
                if d > (ch.a - ch.b).abs() + 0.5 && d < ch.a + ch.b - 0.5 {
                    assert!(dist(rig.chain_goal(c), rig.chain_effector(c)) < 0.05);
                    reached += 1;
                }
            }
            posed += 1;
        }
        if XformRig::root_velocity_sly3(clip, sly.id, dur * 0.5).is_some_and(|v| v[0].abs() > 100.0) {
            moving += 1;
        }
    }
    eprintln!(
        "{} clips, {posed} poses, {reached} chains reached, {moving} clips with root motion over 100",
        clips.len()
    );
    assert!(posed > 400 && reached > 100 && moving > 0);
}

/// The world's collision for the tests, from the drawn triangles without the crew's own meshes (as in the Sly 2
/// test: Sly 3 collision meshes are read for their counts only). Also each crew member's drawn position.
fn scene_collision(w: &World<'_>, slot: usize) -> (CollisionWorld, Vec<(Member, [f32; 3])>) {
    let b = build(
        w,
        SceneOptions { context: w.fresh_context(Some(slot as i16)), decode_textures: false, ..SceneOptions::default() },
    );
    let scene = &b.scene;
    let mut spans = Vec::new();
    each_record(w, &mut |r| {
        if let Some(m) = Member::from_class(r.class) {
            spans.push((m, r.header_offset, r.body_offset + r.body_size));
        }
    });
    let mut cw = CollisionWorld::new();
    let mut shapes = BTreeMap::new();
    let mut found: Vec<(Member, [f32; 3])> = Vec::new();
    for inst in &scene.instances {
        if let Some(&(m, ..)) = spans.iter().find(|s| inst.record >= s.1 && inst.record < s.2) {
            if !found.iter().any(|f| f.0 == m) {
                found.push((m, inst.transform.t));
            }
            continue;
        }
        let shape = *shapes.entry(inst.mesh).or_insert_with(|| {
            let mesh = &scene.meshes[inst.mesh as usize];
            let mut positions: Vec<[f32; 3]> = Vec::new();
            let mut triangles = Vec::new();
            for sm in &scene.submeshes[mesh.submeshes.start as usize..mesh.submeshes.end as usize] {
                if positions.len() + sm.vertex_count as usize > 65_000 {
                    continue;
                }
                let base = positions.len() as u32;
                for v in &scene.vertices[sm.base_vertex as usize..(sm.base_vertex + sm.vertex_count) as usize] {
                    positions.push(v.position);
                }
                for tri in scene.indices[sm.first_index as usize..(sm.first_index + sm.index_count) as usize]
                    .as_chunks::<3>()
                    .0
                {
                    let corners = tri.map(|i| (base + i as u32) as u16);
                    let p = corners.map(|c| positions[c as usize]);
                    let center = [0, 1, 2].map(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.0);
                    let radius = p.iter().map(|q| dist(*q, center)).fold(0.0, f32::max);
                    triangles.push(CollisionTriangle { corners, radius, center, first_edge: 0, edge_count: 0 });
                }
            }
            let radius = positions.iter().map(|p| dist(*p, [0.0; 3])).fold(0.0, f32::max);
            let m = CollisionMesh { radius, positions, triangles, edges: Vec::new() };
            cw.add_shape(Shape { main: TriMesh::new(&m, &CollisionTree::default()), camera: None })
        });
        cw.add_body(shape, inst.transform, 0, 0, true);
    }
    cw.rebuild();
    (cw, found)
}

#[test]
fn core_crew_moves_in_v_apt() {
    need_disc!();
    let (slot, w) = world("v_apt");
    let (cw, drawn) = scene_collision(&w, slot);
    eprintln!("crew drawn in v_apt: {drawn:?}");
    let sly_at = drawn.iter().find(|d| d.0 == Member::Sly).expect("Sly is drawn").1;
    // Start the core three on open floor near Sly's drawn position: points whose floor below is level (normal z
    // above 0.9) with room above, at least 120 apart.
    let filter = QueryFilter::default();
    let mut spots: Vec<[f32; 3]> = Vec::new();
    'search: for ring in 0..12 {
        for k in 0..(ring * 6).max(1) {
            let a = k as f32 * std::f32::consts::TAU / (ring * 6).max(1) as f32;
            let r = ring as f32 * 80.0;
            let top = [sly_at[0] + r * a.cos(), sly_at[1] + r * a.sin(), sly_at[2] + 300.0];
            let Some(h) = cw.ray_cast(top, [0.0, 0.0, -1.0], 1500.0, &filter) else { continue };
            if h.normal[2] < 0.9 || spots.iter().any(|s| dist(*s, h.point) < 120.0) {
                continue;
            }
            let p = [h.point[0], h.point[1], h.point[2] + 5.0];
            let clear = (0..4).all(|j| {
                let b = j as f32 * std::f32::consts::FRAC_PI_2;
                cw.ray_cast([p[0], p[1], p[2] + 60.0], [b.cos(), b.sin(), 0.0], 80.0, &filter).is_none()
            });
            if clear {
                spots.push(p);
                if spots.len() == 3 {
                    break 'search;
                }
            }
        }
    }
    assert_eq!(spots.len(), 3, "open floor near Sly");
    let present: Vec<(Member, [f32; 3], f32)> =
        [Member::Sly, Member::Murray, Member::Bentley].iter().zip(&spots).map(|(&m, &p)| (m, p, 0.0)).collect();
    let mut crew = Crew::new(&present, shared());
    for c in &mut crew.gang.members {
        c.leave_ground();
    }
    let world =
        GangWorld { cw: &cw, filter: QueryFilter::default(), targets: &[], help: 0.0, safehouse: false, pools: &[] };
    let dt = 1.0 / 60.0;
    for _ in 0..120 {
        crew.update(dt, &GangInput::default(), &world);
    }
    for c in &crew.gang.members {
        assert!(c.grounded, "{:?} lands: {:?}", c.who, c.position);
        let spot = spots[c.who.index().min(2)];
        assert!((c.position[2] - spot[2]).abs() < 10.0, "{:?} on its floor: {:?} vs {spot:?}", c.who, c.position);
    }
    for m in [Member::Sly, Member::Murray, Member::Bentley] {
        assert!(crew.switch_to(m));
        let who = m.core().unwrap();
        let start = crew.gang.get(who).unwrap().position;
        let run = GangInput { stick: [0.0, 1.0], held: button::R1, ..GangInput::default() };
        for _ in 0..30 {
            crew.update(dt, &run, &world);
        }
        let c = crew.gang.get(who).unwrap();
        let moved = dist(c.position, start);
        eprintln!("{m:?} ran {moved:.1} in 0.5 s");
        assert!(moved > 0.0 && moved <= 0.5 * [832.0, 800.0, 750.0][who.index()] * 1.05);
        let mut events =
            crew.update(dt, &GangInput { pressed: button::CROSS, held: button::CROSS, ..GangInput::default() }, &world);
        for _ in 0..150 {
            events.extend(crew.update(dt, &GangInput::default(), &world));
        }
        assert!(events.contains(&(who, GangEvent::Jumped)) && events.contains(&(who, GangEvent::Landed)), "{m:?}");
    }
    assert!(!crew.switch_to(Member::Carmelita));
    assert_eq!(crew.player(), Member::Bentley);
    let _ = Who::Sly;
}

#[test]
fn executable_tables_and_constants() {
    need_disc!();
    use engine::gang::sly3::{self, LOOKUP_TABLES, Sly3ItemTable, read_lookup};
    let exe = &data().exe;
    // characters.md 3.1 and 7.
    let t = Sly3ItemTable::from_executable(exe).unwrap();
    assert_eq!(t.items.len(), 67);
    let rec = |n: usize| {
        let r = t.items[n];
        [r.button, r.owner, r.price, r.cost, r.cost2, r.episode, r.tier, r.name, r.description, r.usage, r.effect]
    };
    assert_eq!(rec(8), [2, 25, 200, 5, 0, 3, 14, 217, 218, 299, 0]);
    assert_eq!(t.items[8].pairs, [0, 0, 0, 0]);
    assert_eq!(rec(30), [2, 24, 0, 34, 0, -1, -1, 227, 228, 287, 1968]);
    assert_eq!(t.items[30].pairs, [324, 4, 324, 4]);
    assert_eq!(rec(41)[..10], [-1, 24, 0, 0, 0, -1, -1, 267, 268, -1]);
    assert_eq!((t.items[25].effect, t.items[25].pairs), (1971, [312, 4, 312, 4]));
    assert_eq!([t.items[32].cost2, t.items[33].cost2, t.items[34].cost2, t.items[35].cost2], [3, 2, 3, 5]);
    // Owners: Sly 25 to 49, Murray 17 to 24, Bentley 6 to 16.
    for (range, m) in [(6..17, Member::Bentley), (17..25, Member::Murray), (25..50, Member::Sly)] {
        for n in range {
            assert_eq!(t.items[n].owner(), Some(m), "item {n}");
        }
    }
    // 3.2: the lookup tables' counts and first records.
    for (what, va, n) in LOOKUP_TABLES {
        assert_eq!(read_lookup(exe, va, n).unwrap().len(), n, "{what}");
    }
    let first3 = |va| read_lookup(exe, va, 3).unwrap();
    assert_eq!(first3(0x0034_2890), [(768, 449), (768, 447), (768, 451)]);
    assert_eq!(first3(0x0034_5FD8), [(768, 449), (768, 447), (768, 451)]);
    assert_eq!(read_lookup(exe, 0x0034_2DD0, 2).unwrap(), [(272, 731), (768, 729)]);
    // 3.3: the constants found at their Sly 3 addresses.
    let c = sly3::constants_from_executable(exe).unwrap();
    assert_eq!(c.murray_knock, [[1000.0, 900.0, 950.0], [1000.0, -900.0, 950.0]]);
    assert_eq!(c.crush, [0.0, 0.0, -1000.0]);
    assert_eq!(c.dart_depth, [10.0, 40000.0]);
    assert_eq!(c.costume_gravity, [0.0, 0.0, -750.0]);
    // A changed executable is refused.
    let mut bytes = exe.data.clone();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let bad = formats_sly3::exe::Executable::parse(bytes).unwrap();
    assert!(Sly3ItemTable::from_executable(&bad).is_err());
}

#[test]
fn clip_counts_by_group() {
    need_disc!();
    // characters.md 4: clips in the character's group, at any depth, with data, driving the character's tree.
    let count = |w: &World<'_>, group: &str, class: i16| -> usize {
        let Some(ch) = first_of_class(w, class) else { return 0 };
        // The character's own tree: its record and every object nested in it.
        let mut ids = std::collections::BTreeSet::from([ch.id]);
        visit(ch.nested(), &mut |r| {
            ids.insert(r.id);
        });
        let mut n = 0;
        for (gi, b) in w.bodies.iter().enumerate() {
            if w.groups[gi].name != group {
                continue;
            }
            visit(&b.objects, &mut |r| {
                if let Body::Clip(c) = &r.body
                    && let Some(d) = &c.data
                    && d.channels.iter().any(|x| ids.contains(&x.target))
                {
                    n += 1;
                }
            });
        }
        n
    };
    for name in ["i_trainer", "v_ext", "v_apt"] {
        let (_, w) = world(name);
        assert_eq!(count(&w, "jt", 52), 342, "{name}");
        assert_eq!(count(&w, "murray", 53), 250, "{name}");
        assert_eq!(count(&w, "bentley", 54), 206, "{name}");
    }
    let (_, w) = world("m_ext");
    for (group, class, n) in [
        ("shaman", 55, 93),
        ("panda_king", 56, 102),
        ("penelope", 57, 115),
        ("dimitri", 59, 83),
        ("carmelita", 60, 134),
    ] {
        assert_eq!(count(&w, group, class), n, "{group}");
    }
}

#[test]
fn crew_arrives_at_checkpoint_424_in_v_ext() {
    need_disc!();
    use engine::gang::sly3::{Checkpoint, DEFAULT_ARRIVAL, find_arrival};
    let (slot, w) = world("v_ext");
    // characters.md 2 and 7: v_ext has a class-40 record with OID 424 in group v_ext_rig.
    let mut checkpoints = Vec::new();
    let mut in_rig = false;
    for (gi, b) in w.bodies.iter().enumerate() {
        visit(&b.objects, &mut |r| {
            if r.class == 40 {
                if r.id == DEFAULT_ARRIVAL && w.groups[gi].name == "v_ext_rig" {
                    in_rig = true;
                }
                checkpoints.push(r.id);
            }
        });
    }
    assert!(in_rig, "checkpoint 424 in v_ext_rig");
    assert_eq!(checkpoints.len(), 35);
    // Place the crew there: the checkpoint's position from the drawn scene is not needed; its record's world
    // position comes from the scene's placement of the rig, so use the arrival rule on the checkpoint list with the
    // player standing where Sly is drawn relative to it.
    let cps: Vec<Checkpoint> = checkpoints
        .iter()
        .map(|&oid| Checkpoint { oid, answers: Vec::new(), position: [0.0; 3], yaw: 0.0, in_key: true })
        .collect();
    let i = find_arrival(&cps, DEFAULT_ARRIVAL).unwrap();
    assert_eq!(cps[i].oid, 424);
    let _ = slot;
}

#[test]
fn gadget_and_pocket_tables() {
    need_disc!();
    use engine::gang::sly3::{self, FIRST_PRIZE_KIND, GUARD_POCKETS_VA, OTHER_POCKETS_VA, Sly3ItemTable, gadget_class};
    let d = data();
    let exe = &d.exe;
    // gadgets.md 1: the usage texts at field +36.
    let t = Sly3ItemTable::from_executable(exe).unwrap();
    let usage: &[(i32, &[usize])] = &[
        (286, &[16]),
        (287, &[9, 12, 14, 18, 20, 21, 22, 23, 24, 25, 29, 30, 31, 37, 38, 39, 40]),
        (288, &[19]),
        (290, &[7, 32, 33, 34, 35]),
        (291, &[6, 10, 13]),
        (292, &[28]),
        (293, &[27]),
        (294, &[11]),
        (298, &[26]),
        (299, &[8, 15]),
        (300, &[17]),
        (301, &[36]),
    ];
    for (string, items) in usage {
        for &n in *items {
            assert_eq!(t.items[n].usage, *string, "item {n}");
        }
    }
    assert!((41..50).all(|n| t.items[n].usage == -1));
    // 2: the slot table.
    assert_eq!(sly3::slot_bits(exe).unwrap(), [4, 1, 2]);
    // guards.md 3: the pocket tables' episode rows.
    let guards = sly3::pocket_table(exe, GUARD_POCKETS_VA).unwrap();
    let others = sly3::pocket_table(exe, OTHER_POCKETS_VA).unwrap();
    assert!(guards.row(0).is_none() && others.row(0).is_none());
    let g1 = guards.row(1).unwrap();
    assert_eq!((g1.coins, g1.chance), ([6, 12], 0.4));
    assert_eq!(g1.prizes.map(|p| p.1), [1263, 1262, 1260, 1259, 1266, 1265]);
    assert_eq!(g1.prizes.map(|p| p.0), [30, 30, 15, 15, 5, 5]);
    let o1 = others.row(1).unwrap();
    assert_eq!((o1.coins, o1.chance), ([4, 8], 0.2));
    assert_eq!(o1.prizes.map(|p| p.1), [1261, 1262, 1258, 1259, 1264, 1265]);
    for e in 1..=5 {
        for r in [guards.row(e).unwrap(), others.row(e).unwrap()] {
            assert!(r.prizes.iter().all(|p| (1258..=1284).contains(&p.1)), "row {e}");
        }
    }
    // pickpocket-and-pickups.md 4 and 8: the loot table.
    let loot = sly3::loot_table(exe).unwrap();
    assert_eq!(loot.records.len(), 64);
    assert_eq!((loot.records[0].value, loot.records[0].name, loot.records[0].kind), (21, 421, 1255));
    assert_eq!((loot.records[63].value, loot.records[63].name, loot.records[63].kind), (736, 484, 1318));
    for (i, r) in loot.records.iter().take(30).enumerate() {
        assert_eq!(r.kind, FIRST_PRIZE_KIND + i as i32);
    }
    assert!(loot.records[30..36].iter().all(|r| r.value == 0));
    let gravity = exe.bytes_at(0x0034_6FD8, 4).unwrap();
    assert_eq!(f32::from_le_bytes(gravity.try_into().unwrap()), -2250.0);
    // A Sly 3 pocket prize reaches its loot record.
    let mut purse = engine::gang::Purse::default();
    assert!(purse.receive_from(engine::gang::Entry::Prize(1260), FIRST_PRIZE_KIND));
    assert_eq!(purse.loot[5], 1);
    // gadgets.md 3: the gang's gadget objects over all worlds.
    let mut count: BTreeMap<i16, usize> = BTreeMap::new();
    for (_, _, bytes) in &d.worlds {
        let w = World::parse_with(bytes, &d.options, d.multipliers).unwrap();
        each_record(&w, &mut |r| {
            if [86, 113, 114, 115, 119, 124].contains(&r.class) {
                *count.entry(r.class).or_default() += 1;
            }
        });
    }
    eprintln!("gadget objects: {count:?}");
    for (class, n) in [
        (gadget_class::GRAPPLE_CAM, 34),
        (gadget_class::CAMERA, 34),
        (gadget_class::REMOTE, 38),
        (gadget_class::THROWN, 68),
        (gadget_class::BOMB, 34),
        (gadget_class::EXTRACTOR, 34),
    ] {
        assert_eq!(count.get(&class).copied().unwrap_or(0), n, "class {class}");
    }
    // The behaviours agree with the table: owners carry over, held items are the held behaviours.
    let shared = sly3::shared_from_executable(exe).unwrap();
    for n in [32u8, 33, 34, 35, 36] {
        assert_eq!(t.items[n as usize].button, 1, "item {n} is held");
    }
    // The Fishing Pole's record has button use 2 (press) although its usage text is the "press and hold" one.
    assert_eq!((t.items[7].button, t.items[7].usage), (2, 290));
    assert_eq!(shared.items.behaviour(8), 26);
    assert_eq!(t.items[8].owner(), Some(Member::Bentley), "the Alarm Clock moved to Bentley");
    assert_eq!(t.items[22].owner(), Some(Member::Murray), "Temporal Lock moved to Murray");
    assert_eq!(shared.items.drain(32), Some(3.0));
}
