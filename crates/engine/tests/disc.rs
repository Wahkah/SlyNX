//! Tests against the player's own Sly 1 disc image: the test vectors of specs/sly1/animation.md and collision.md,
//! and disc-wide checks of the parsers and run-time structures. They skip (and pass) when `SLYNX_DISC_SLY1` is not
//! set.

use std::sync::OnceLock;

use engine::anim::curve::{euler_zyx, eval_scalar, eval_vector};
use engine::anim::playback::{Binding, apply_segment};
use engine::anim::rig::Rig;
use engine::anim::skin::{Skin, SkinnedSubmesh};
use engine::collision::Materials;
use engine::collision::geom::Triangle;
use engine::collision::mesh::TriMesh;
use engine::collision::volume::{point_in_convex, world_volumes};
use engine::collision::world::{CollisionWorld, QueryFilter};
use engine::math::{affine_inverse, length, sub};
use formats::anim::{ScalarCurve, Segment, TANGENT_STEP, VectorCurve, player_lookup_list};
use formats::collision::{ConvexVolume, MaterialTables, NO_CHILD, Solid, material_of};
use formats::disc::{Disc, Executable};
use formats::options::OptionTable;
use formats::stream::{Layout, ObjectStream, layout_of};
use formats::world::World;
use harness::golden::{Game, require_disc};

const PARIS: usize = 1;
const HIDEOUT: usize = 2;

struct DiscData {
    exe: Executable,
    data: Vec<Vec<u8>>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    let path = require_disc(Game::Sly1, test)?;
    static CELL: OnceLock<DiscData> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let compressed: Vec<Vec<u8>> = (0..d.worlds.len()).map(|i| d.read_world_compressed(i).unwrap()).collect();
        let mut data = vec![Vec::new(); compressed.len()];
        std::thread::scope(|s| {
            for (o, c) in data.iter_mut().zip(&compressed) {
                s.spawn(move || *o = formats::compress::decompress(c).unwrap());
            }
        });
        DiscData { exe: d.executable.clone(), data }
    }))
}

fn worlds(d: &'static DiscData) -> &'static [World<'static>] {
    static CELL: OnceLock<Vec<World<'static>>> = OnceLock::new();
    CELL.get_or_init(|| {
        std::thread::scope(|s| {
            let h: Vec<_> = d.data.iter().map(|w| s.spawn(move || World::load(w).unwrap())).collect();
            h.into_iter().map(|h| h.join().unwrap()).collect()
        })
    })
}

fn record_at(s: &ObjectStream, offset: usize) -> usize {
    s.records.iter().position(|r| r.offset == offset).unwrap_or_else(|| panic!("no record at {offset:#x}"))
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn close3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|i| close(a[i], b[i], tol))
}

/// Every animation segment body of a world: class-131 records, then warp and exit animations.
fn segments(w: &World<'_>, table: &OptionTable) -> Vec<Segment> {
    let s = &w.objects;
    let mut out: Vec<Segment> = s
        .records
        .iter()
        .filter(|r| r.class == 131)
        .map(|r| Segment::parse(w.data, r.offset + 6, table).unwrap())
        .collect();
    out.extend(s.owned_animations.iter().map(|a| Segment::parse(w.data, a.body, table).unwrap()));
    out
}

fn paris_segment(w: &World<'_>, offset: usize, oid: i16, frames: i16) -> Segment {
    let i = record_at(&w.objects, offset);
    let r = &w.objects.records[i];
    assert_eq!((r.class, r.oid), (131, oid));
    let seg = Segment::parse(w.data, r.offset + 6, &OptionTable::sly1()).unwrap();
    assert_eq!(seg.duration_frames, frames);
    assert_eq!(seg.end, r.end);
    seg
}

// ---------------------------------------------------------------------------------------------------------------
// animation.md

#[test]
fn animation_vectors() {
    let Some(d) = disc("animation_vectors") else { return };
    let w = &worlds(d)[PARIS];

    // Hermite scalar, rotation z.
    let seg = paris_segment(w, 0x251A0B, 1361, 180);
    let c = seg.channels.iter().find(|c| c.target == 1356).unwrap();
    let Some(VectorCurve::Components { mask, base, curves }) = &c.rotation else { panic!("rotation curve") };
    assert_eq!((*mask, *base), (0b100, [0.0; 3]));
    let Some(ScalarCurve::Hermite(keys)) = &curves[2] else { panic!("kind 0") };
    assert_eq!(keys.len(), 3);
    assert_eq!((keys[1].frame, keys[1].kind_in, keys[1].kind_out), (60, 1, 1));
    assert!(close(keys[2].value, 50.68658, 1e-4) && close(keys[0].slope_in, -9.42478, 1e-4));
    let e = eval_vector(c.rotation.as_ref().unwrap(), 0.5).unwrap();
    assert!(close3(e, [0.0, 0.0, 3.25423], 2e-5), "{e:?}");
    let r = euler_zyx(e);
    assert!(close3(r[0], [-0.99366, -0.11240, 0.0], 2e-5));
    assert!(close3(r[1], [0.11240, -0.99366, 0.0], 2e-5));
    assert!(close3(r[2], [0.0, 0.0, 1.0], 1e-6));

    // Weighted Bezier, position z.
    let seg = paris_segment(w, 0x265546, 1383, 120);
    let c = seg.channels.iter().find(|c| c.target == 1382).unwrap();
    let Some(VectorCurve::Components { curves, .. }) = &c.position else { panic!("position curve") };
    let z = curves[2].as_ref().unwrap();
    let ScalarCurve::Bezier(keys) = z else { panic!("kind 1") };
    assert_eq!((keys[1].frame, keys[1].kind_in, keys[1].kind_out), (15, 4, 4));
    let a = eval_scalar(z, 0.0625).unwrap();
    let b = eval_scalar(z, 0.125).unwrap();
    // In single precision the bisection meets T(0.25) = tau exactly and stops: 0.46337 (animation.md 9).
    assert!(close(a, 0.46337, 1e-5), "{a}");
    assert!(close(b, 1.47372, 1e-5), "{b}");

    // Vector Hermite, position.
    let seg = paris_segment(w, 0x27AC4A, 1451, 90);
    let c = seg.channels.iter().find(|c| c.target == 54).unwrap();
    let p = c.position.as_ref().unwrap();
    assert!(matches!(p, VectorCurve::Hermite(k) if k.len() == 3));
    assert!(close3(eval_vector(p, 0.0).unwrap(), [-38.9039, -21.6148, 12.2804], 1e-3));
    assert!(close3(eval_vector(p, 0.208333).unwrap(), [-48.0789, -14.4105, -12.0907], 1e-3));
    assert!(close3(eval_vector(p, 0.958333).unwrap(), [-53.6977, -14.7839, -20.2897], 1e-3));

    // Step key, position x.
    let seg = paris_segment(w, 0x3B7332, 239, 530);
    let c = seg.channels.iter().find(|c| c.target == 1836).unwrap();
    let Some(VectorCurve::Components { curves, .. }) = &c.position else { panic!("position curve") };
    let x = curves[0].as_ref().unwrap();
    let ScalarCurve::Hermite(keys) = x else { panic!("kind 0") };
    let k = keys.iter().position(|k| k.frame == 180).unwrap();
    assert_eq!((keys[k].kind_out, keys[k + 1].frame), (TANGENT_STEP, 181));
    assert!(close(keys[k + 1].value, 300.0, 1e-4));
    assert!(close(eval_scalar(x, 3.00833).unwrap(), 203.81694, 1e-4));
}

#[derive(Default, Debug)]
struct AnimStats {
    segments: usize,
    curves: usize,
    non_increasing: usize,
    tangent_kinds: [usize; 9],
    root_motion: usize,
    options: [usize; 12],
    keys_outside: usize,
    pose_curves: usize,
    owned_options: [usize; 12],
    vector_hermite: [usize; 3],
    component: [usize; 3],
    twist: usize,
}

fn scalar_stats(c: &ScalarCurve, duration: i16, st: &mut AnimStats) {
    let frames: Vec<i16> = match c {
        ScalarCurve::Hermite(k) => {
            for k in k {
                st.tangent_kinds[k.kind_in as usize % 9] += 1;
                st.tangent_kinds[k.kind_out as usize % 9] += 1;
            }
            k.iter().map(|k| k.frame).collect()
        }
        ScalarCurve::Bezier(k) => {
            for k in k {
                st.tangent_kinds[k.kind_in as usize % 9] += 1;
                st.tangent_kinds[k.kind_out as usize % 9] += 1;
            }
            k.iter().map(|k| k.frame).collect()
        }
        ScalarCurve::Linear(k) => k.iter().map(|k| k.frame).collect(),
        _ => return,
    };
    frame_stats(&frames, duration, st);
}

fn frame_stats(frames: &[i16], duration: i16, st: &mut AnimStats) {
    st.curves += 1;
    if frames.windows(2).any(|w| w[1] <= w[0]) {
        st.non_increasing += 1;
    }
    if frames.len() < 2 || frames[0] != 0 || *frames.last().unwrap() != duration {
        st.keys_outside += 1;
    }
}

fn vector_stats(c: &VectorCurve, duration: i16, st: &mut AnimStats) {
    match c {
        VectorCurve::Components { curves, .. } => curves.iter().flatten().for_each(|c| scalar_stats(c, duration, st)),
        VectorCurve::Hermite(k) => frame_stats(&k.iter().map(|k| k.frame).collect::<Vec<_>>(), duration, st),
        VectorCurve::Blend => {}
    }
}

#[test]
fn every_segment_parses_and_matches_the_disc_statistics() {
    let Some(d) = disc("every_segment_parses_and_matches_the_disc_statistics") else { return };
    let table = OptionTable::sly1();
    let mut st = AnimStats::default();
    for w in worlds(d) {
        let records = w.objects.records.iter().filter(|r| r.class == 131).count();
        for (k, seg) in segments(w, &table).into_iter().enumerate() {
            st.segments += 1;
            assert!(seg.channels.iter().filter(|c| c.is_root_motion()).count() <= 1);
            for c in &seg.channels {
                st.root_motion += c.is_root_motion() as usize;
                for (slot, v) in [&c.position, &c.rotation, &c.scale].into_iter().enumerate() {
                    let Some(v) = v else { continue };
                    match v {
                        VectorCurve::Hermite(_) => st.vector_hermite[slot] += 1,
                        VectorCurve::Components { curves, .. } => {
                            st.component[slot] += curves.iter().flatten().filter(|c| c.key_count() > 0).count()
                        }
                        VectorCurve::Blend => {}
                    }
                    vector_stats(v, seg.duration_frames, &mut st);
                }
                for s in c.twist.iter() {
                    st.twist += (s.key_count() > 0) as usize;
                    scalar_stats(s, seg.duration_frames, &mut st);
                }
                // Pose curves are kept out of the spec's curve count; their keys follow the same rules.
                let mut pose = AnimStats::default();
                for s in c.poses.iter().flatten() {
                    scalar_stats(s, seg.duration_frames, &mut pose);
                }
                st.pose_curves += pose.curves;
                st.keys_outside += pose.keys_outside;
                st.non_increasing += pose.non_increasing;
                for (a, b) in st.tangent_kinds.iter_mut().zip(pose.tangent_kinds) {
                    *a += b;
                }
            }
            // The spec's option counts are over segment records (warp and exit animations add 1, 2 and 1 to
            // options 178, 180 and 182).
            // Counted once per segment that sets the option.
            let mut seen = [false; 12];
            for item in seg.options.iter(w.data, &table) {
                let (id, _) = item.unwrap();
                if (178..=189).contains(&id) {
                    seen[(id - 178) as usize] = true;
                }
            }
            for (i, &on) in seen.iter().enumerate() {
                if on {
                    if k < records {
                        st.options[i] += 1;
                    } else {
                        st.owned_options[i] += 1;
                    }
                }
            }
        }
    }
    eprintln!("{st:?}");
    // animation.md 1, 2.3, 4.2, 5.1.
    assert_eq!(st.segments, 13_176);
    assert_eq!(st.curves, 140_413);
    assert_eq!(st.pose_curves, 11_094);
    assert_eq!(st.non_increasing, 3);
    assert_eq!(st.keys_outside, 0);
    assert_eq!(st.tangent_kinds, [0, 84_375, 36_232, 151_820, 425_030, 131, 0, 0, 0]);
    assert_eq!(st.root_motion, 7_013);
    // animation.md 1: vector Hermite curves (position, rotation) and scalar curves (per-component position,
    // rotation, scale; twist).
    assert_eq!((st.vector_hermite[0], st.vector_hermite[1]), (32_104, 50_158));
    assert_eq!(st.component, [6_990, 22_173, 6_176]);
    assert_eq!(st.twist, 22_812);
    let opt = |id: usize| st.options[id - 178];
    assert_eq!((opt(178), opt(179), opt(180), opt(181)), (5_810, 162, 216, 1_708));
    assert_eq!((opt(183), opt(184), opt(189)), (26, 8, 601));
    // Counted per segment: two segments set option 182 twice.
    assert_eq!(opt(182), 1_021);
    let own = |id: usize| st.owned_options[id - 178];
    assert_eq!((own(178), own(180), own(182)), (1, 2, 1));
}

#[test]
fn player_lookup_list_resolves_below_the_player() {
    let Some(d) = disc("player_lookup_list_resolves_below_the_player") else { return };
    let list = player_lookup_list(&d.exe).unwrap();
    assert_eq!(list.len(), 151);
    let mut with_player = 0;
    for (i, w) in worlds(d).iter().enumerate() {
        let s = &w.objects;
        let Some(player) = s.records.iter().position(|r| r.class == 7 && r.oid == 5) else { continue };
        with_player += 1;
        // Below the player: its subtree and the child list of its proxy (which the game moves under it).
        let proxy = s.records[player].parent.map_or(player, |p| p as usize);
        let range = proxy + 1..s.records[proxy].subtree_end as usize;
        let mut segments = 0;
        for e in &list {
            let found = range.clone().find(|&k| s.records[k].oid == e.oid);
            assert!(found.is_some() || e.is_optional(), "world {i}: OID {} not below the player", e.oid);
            segments += found.is_some_and(|k| s.records[k].class == 131) as usize;
        }
        if i == PARIS {
            // 70 required and 22 optional segments are present in Paris (animation.md 7).
            assert_eq!(segments, 92);
        }
    }
    assert_eq!(with_player, 41);
}

#[test]
fn skinned_vertices_sit_near_their_bones() {
    // transforms.md 9: vertices weighted more than 0.95 to one bone lie near that bone's origin in the skinned
    // object's space; median distance 28 units over 5,204 vertices in Paris. Bones are found by the nearest-OID rule.
    let Some(d) = disc("skinned_vertices_sit_near_their_bones") else { return };
    let w = &worlds(d)[PARIS];
    let s = &w.objects;
    let world = s.world_transforms(true);
    let mut dists = Vec::new();
    for (i, r) in s.records.iter().enumerate() {
        let Some(set) = &r.mesh_set else { continue };
        if set.bones.is_empty() || !r.in_world {
            continue;
        }
        let inv = affine_inverse(&world[i]).unwrap();
        let bones: Vec<Option<[f32; 3]>> = set
            .bones
            .iter()
            .map(|&oid| s.find_nearest(i as u32, oid, false).map(|b| inv.transform_point(world[b as usize].t)))
            .collect();
        for m in &set.meshes {
            for sm in &m.submeshes {
                for p in 0..sm.np as usize {
                    let (b, wts) = sm.influences(w.data, p);
                    for j in 0..sm.bone_count as usize {
                        if wts[j] > 0.95
                            && let Some(Some(origin)) = bones.get(b[j] as usize)
                        {
                            dists.push(length(sub(sm.position(w.data, p), *origin)));
                        }
                    }
                }
            }
        }
    }
    dists.sort_by(f32::total_cmp);
    let median = dists[dists.len() / 2];
    let mean = dists.iter().sum::<f32>() / dists.len() as f32;
    eprintln!("Paris: {} vertices, median distance {median}, mean {mean}", dists.len());
    assert_eq!(dists.len(), 5_204);
    // The spec's 28 matches the mean of these distances (27.5); their median is 20.2. Either is far below the
    // 116 of the transposed convention.
    assert!((mean - 28.0).abs() < 1.0, "{mean}");
    assert!(median < 28.0);
}

#[test]
fn player_rig_skins_and_animates() {
    let Some(d) = disc("player_rig_skins_and_animates") else { return };
    let w = &worlds(d)[PARIS];
    let s = &w.objects;
    let table = OptionTable::sly1();
    let player = s.records.iter().position(|r| r.class == 7 && r.oid == 5).unwrap();
    let proxy = s.records[player].parent.unwrap();
    let world = s.world_transforms(true);
    let mut rig = Rig::from_stream(s, w.data, proxy);
    let base =
        s.records[proxy as usize].transform_parent.map_or(formats::geometry::Affine::IDENTITY, |p| world[p as usize]);
    rig.update_world(&base);
    // The rig reproduces the stream's world transforms at rest.
    for (k, n) in rig.nodes.iter().enumerate() {
        let a = rig.world[k];
        let b = world[n.record.unwrap() as usize];
        assert!(close3(a.t, b.t, 1e-2), "node {k}");
    }
    // Skins of every skinned set below the player; bind pose gives identity matrices.
    let mut skins = Vec::new();
    for (k, n) in rig.nodes.iter().enumerate() {
        let r = &s.records[n.record.unwrap() as usize];
        if let Some(set) = r.mesh_set.as_ref().filter(|m| !m.bones.is_empty()) {
            let mut skin = Skin::from_stream(&rig, s, n.record.unwrap(), set).unwrap();
            assert_eq!(skin.mesh_node, k as u32);
            assert!(skin.bones.iter().all(Option::is_some), "unresolved bone below the player");
            skin.capture_bind(&rig.world);
            skins.push((skin, set));
        }
    }
    assert!(!skins.is_empty());
    let mut m = Vec::new();
    for (skin, _) in &skins {
        skin.matrices(&rig.world, &mut m);
        for a in &m {
            assert!(close3(a.t, [0.0; 3], 1e-2) && close3(a.cols[0], [1.0, 0.0, 0.0], 1e-4));
        }
    }
    // Apply the player's first required animation (OID 130, 180 frames) and skin.
    let seg_rec = (proxy as usize..s.records[proxy as usize].subtree_end as usize)
        .find(|&k| s.records[k].oid == 130 && s.records[k].class == 131)
        .unwrap();
    let seg = Segment::parse(w.data, s.records[seg_rec].offset + 6, &table).unwrap();
    let binding =
        Binding::resolve(&seg, |oid| s.find_nearest(player as u32, oid, false).and_then(|r| rig.node_of_record(r)));
    let bound = binding.targets.iter().flatten().count();
    assert!(bound > 10, "only {bound} channels bound");
    apply_segment(&seg, &binding, 1.5, &mut rig);
    rig.update_world(&base);
    let (skin, set) = &skins[0];
    skin.matrices(&rig.world, &mut m);
    assert!(m.iter().any(|a| !close3(a.t, [0.0; 3], 1e-3)), "animation moved no bone");
    let mut moved = 0.0f32;
    for mesh in &set.meshes {
        for sm in &mesh.submeshes {
            let sk = SkinnedSubmesh::from_submesh(w.data, sm, set.pose_count() as usize);
            let (mut p, mut n) = (Vec::new(), Vec::new());
            sk.deform(&m, rig.node_pose_weights(skin.mesh_node), &mut p, &mut n);
            for (a, b) in p.iter().zip(&sk.positions) {
                assert!(a.iter().all(|x| x.is_finite()));
                moved = moved.max(length(sub(*a, *b)));
            }
        }
    }
    eprintln!("player: {} channels bound, largest vertex move {moved}", bound);
    assert!(moved > 1.0 && moved < 400.0, "{moved}");
}

// ---------------------------------------------------------------------------------------------------------------
// collision.md

#[test]
fn hideout_world_mesh_vectors() {
    let Some(d) = disc("hideout_world_mesh_vectors") else { return };
    let w = &worlds(d)[HIDEOUT];
    let s = &w.objects;
    let rec = &s.records[s.top_level[0] as usize];
    assert_eq!((rec.class, rec.offset + 6), (4, 0x8659));
    let solid = Solid::parse(w.data, rec.offset + 6).unwrap();
    assert!(solid.is_fixed());
    let m = &solid.mesh;
    assert!(close(m.radius, 1080.9119, 1e-3));
    assert_eq!((m.positions.len(), m.triangles.len(), m.edges.len()), (1347, 1681, 1637));
    assert!(close3(m.positions[0], [-19.0479, 162.2528, -244.6026], 1e-3));
    let t0 = m.triangles[0];
    assert_eq!((t0.corners, t0.edge_count), ([1233, 1234, 382], 0));
    assert!(close(t0.radius, 103.859, 1e-3));
    assert!(close3(t0.center, [-36.580, -130.557, 319.883], 1e-3));
    let [a, b, c] = m.corners(0);
    assert!(close3(Triangle::new(a, b, c).normal, [0.0, 0.11291, -0.99360], 1e-4));
    assert_eq!((solid.tree.nodes.len(), solid.tree.full_count), (4166, 4166));
    let n0 = solid.tree.nodes[0];
    assert_eq!((n0.triangle, n0.negative, n0.positive), (0, NO_CHILD, 1));
    assert!(solid.camera_mesh.is_empty() && solid.camera_tree.nodes.is_empty());
    assert!(solid.fade_groups.is_empty() && solid.fade_node_map.is_empty() && solid.hit_groups == 0);
    // Every node but the root has exactly one parent; almost all children are positive.
    let mut parents = vec![0u32; solid.tree.nodes.len()];
    let (mut pos, mut neg) = (0, 0);
    for n in &solid.tree.nodes {
        for (c, count) in [(n.negative, &mut neg), (n.positive, &mut pos)] {
            if c != NO_CHILD {
                parents[c as usize] += 1;
                *count += 1;
            }
        }
    }
    assert_eq!(parents[0], 0);
    assert!(parents[1..].iter().all(|&p| p == 1));
    // collision.md 2: 2,169 negative and 1,996 positive children.
    assert_eq!((neg, pos), (2_169, 1_996));
    assert_eq!(s.records[s.top_level[0] as usize].oid, 1);
}

#[test]
fn every_solid_parses_and_matches_the_disc_statistics() {
    let Some(d) = disc("every_solid_parses_and_matches_the_disc_statistics") else { return };
    let table = OptionTable::sly1();
    let (mut solids, mut empty, mut fades, mut maps) = (0, 0, 0, 0);
    let (mut surfaces, mut edges, mut cam_surfaces, mut cam_edges) = (0usize, 0usize, 0usize, 0usize);
    let (mut nodes, mut cam_nodes, mut neg, mut pos, mut both) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut materials = std::collections::BTreeMap::new();
    for w in worlds(d) {
        for r in &w.objects.records {
            if !matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor)) {
                continue;
            }
            let s = Solid::parse(w.data, r.offset + 6).unwrap();
            solids += 1;
            empty += s.mesh.is_empty() as usize;
            fades += !s.fade_groups.is_empty() as usize;
            maps += !s.fade_node_map.is_empty() as usize;
            surfaces += s.mesh.triangles.len();
            edges += s.mesh.edges.len();
            cam_surfaces += s.camera_mesh.triangles.len();
            cam_edges += s.camera_mesh.edges.len();
            nodes += s.tree.nodes.len();
            cam_nodes += s.camera_tree.nodes.len();
            for n in s.tree.nodes.iter().chain(&s.camera_tree.nodes) {
                neg += (n.negative != NO_CHILD) as usize;
                pos += (n.positive != NO_CHILD) as usize;
                both += (n.negative != NO_CHILD && n.positive != NO_CHILD) as usize;
            }
            assert!(s.fade_groups.iter().all(|g| g.key == 1 || g.key == -1));
            assert!(s.fade_groups.first().is_none_or(|g| g.key == 1));
            if !s.fade_node_map.is_empty() {
                assert_eq!(s.fade_node_map.len(), s.tree.nodes.len());
            }
            assert_eq!(s.hit_groups, 0);
            assert_eq!(s.tree.full_count as usize, s.tree.nodes.len());
            let m = material_of(w.data, &r.options.unwrap(), &table).unwrap();
            if m != 0 {
                *materials.entry(m).or_insert(0) += 1;
            }
        }
    }
    eprintln!(
        "solids {solids}, empty {empty}, fade groups {fades}, node maps {maps}, surfaces {surfaces} (+{cam_surfaces} \
         camera), edges {edges} (+{cam_edges} camera), materials {materials:?}"
    );
    // collision.md 8 and 6.
    assert_eq!((solids, empty, fades, maps), (4_637, 999, 83, 122));
    // The surface and edge totals count the collision meshes (not the camera meshes).
    assert_eq!((surfaces, edges, nodes), (455_004, 533_267, 1_193_176));
    assert_eq!((cam_surfaces, cam_edges, cam_nodes), (203_374, 216_269, 528_510));
    // collision.md 2: child counts over every collision and camera tree.
    assert_eq!((nodes + cam_nodes, neg, pos, both), (1_721_686, 1_002_561, 715_133, 465_969));
    assert_eq!(materials.into_iter().collect::<Vec<_>>(), [(1, 44), (10, 1), (12, 2)]);
}

#[test]
fn stored_edges_are_the_convex_ones() {
    // collision.md 1, with its definitions: shared edge = a corner pair that is a side of exactly two surfaces of the
    // mesh; convex if n · (q − p) < −ε, concave if > ε, else flat; stored if any edge record of the mesh has the same
    // two position indices. Collision meshes of every solid in worlds 1, 2, 12 and 30.
    let Some(d) = disc("stored_edges_are_the_convex_ones") else { return };
    let count = |eps: f32| {
        let mut c = [[0usize; 2]; 3]; // [convex, concave, flat] x [stored, not]
        for wi in [1usize, 2, 12, 30] {
            let w = &worlds(d)[wi];
            for r in &w.objects.records {
                if !matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor)) {
                    continue;
                }
                let s = Solid::parse(w.data, r.offset + 6).unwrap();
                let m = &s.mesh;
                let mut sides: std::collections::HashMap<(u16, u16), Vec<usize>> = Default::default();
                for (t, tri) in m.triangles.iter().enumerate() {
                    for k in 0..3 {
                        let (a, b) = (tri.corners[k], tri.corners[(k + 1) % 3]);
                        sides.entry((a.min(b), a.max(b))).or_default().push(t);
                    }
                }
                let stored: std::collections::HashSet<(u16, u16)> =
                    m.edges.iter().map(|e| (e.ends[0].min(e.ends[1]), e.ends[0].max(e.ends[1]))).collect();
                for ((a, b), tris) in sides {
                    if tris.len() != 2 {
                        continue;
                    }
                    let [p0, p1, p2] = m.corners(tris[0]);
                    let n = engine::math::normalize(engine::math::cross(sub(p1, p0), sub(p2, p0)));
                    let other = &m.triangles[tris[1]];
                    let qi = other.corners.iter().copied().find(|&c| c != a && c != b).unwrap();
                    let q = m.positions[qi as usize];
                    let x = engine::math::dot(n, sub(q, p0));
                    let kind = if x < -eps {
                        0
                    } else if x > eps {
                        1
                    } else {
                        2
                    };
                    c[kind][!stored.contains(&(a, b)) as usize] += 1;
                }
            }
        }
        c
    };
    let a = count(0.001);
    eprintln!("eps 0.001: {a:?}");
    assert_eq!(a, [[34_688, 7], [43, 18_356], [0, 1]]);
    let b = count(0.1);
    eprintln!("eps 0.1: {b:?}");
    assert_eq!(b, [[34_672, 7], [43, 18_342], [16, 15]]);
}

#[test]
fn material_tables_from_the_executable() {
    let Some(d) = disc("material_tables_from_the_executable") else { return };
    let t = MaterialTables::from_executable(&d.exe).unwrap();
    let e = |x: &formats::collision::MaterialEntry| (x.weight, x.value);
    assert_eq!((e(&t.friction[0]), e(&t.friction[1])), ((1.0, 0.5), (3.0, 2.4)));
    assert_eq!((e(&t.elasticity[0]), e(&t.elasticity[1])), ((1.0, 0.5), (5.0, 1.0)));
    assert_eq!((e(&t.friction[12]), e(&t.elasticity[12])), ((1.0, -10.0), (1.0, 1.0)));
    let m = Materials::new(t);
    for j in 0..13 {
        assert_eq!(m.friction(12, j), 0.0);
    }
    assert_eq!(m.elasticity(12, 0), 0.75);
    // A different executable is refused.
    let mut other = d.exe.clone();
    let last = other.data.len() - 1;
    other.data[last] ^= 1;
    assert!(MaterialTables::from_executable(&other).is_err());
    assert!(player_lookup_list(&other).is_err());
}

#[test]
fn convex_volumes_classify_points() {
    let Some(d) = disc("convex_volumes_classify_points") else { return };
    let (mut total, mut origin_inside) = (0, 0);
    for wi in [1usize, 2, 12, 21] {
        let w = &worlds(d)[wi];
        for v in &w.objects.volumes {
            let cv = ConvexVolume::parse(w.data, v.offset).unwrap();
            if cv.nodes.is_empty() {
                continue;
            }
            total += 1;
            assert!(!point_in_convex(&cv, [1e7, 1e7, 1e7]), "world {wi} record {}", v.record);
            assert!(!point_in_convex(&cv, [-1e7, 3e6, -2e7]));
            origin_inside += point_in_convex(&cv, [0.0; 3]) as usize;
        }
        let placed = world_volumes(w).unwrap();
        assert_eq!(placed.len(), w.objects.volumes.len(), "world {wi}");
    }
    eprintln!("{origin_inside} of {total} volumes contain their own origin");
    assert_eq!((origin_inside, total), (63, 75));
}

/// A small deterministic generator for query positions.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

#[test]
fn collision_tree_and_hierarchy_agree() {
    // The collision tree's positive side is the open side (collision.md 2): pushing a segment through it reaches
    // every triangle the segment crosses, so ray casts through the tree and through the hierarchy agree.
    let Some(d) = disc("collision_tree_and_hierarchy_agree") else { return };
    for wi in [HIDEOUT, PARIS] {
        let w = &worlds(d)[wi];
        let rec = &w.objects.records
            [w.objects.top_level.iter().map(|&i| i as usize).find(|&i| w.objects.records[i].class == 4).unwrap()];
        let solid = Solid::parse(w.data, rec.offset + 6).unwrap();
        let mesh = TriMesh::new(&solid.mesh, &solid.tree);
        let b = mesh.bounds;
        let mut rng = Lcg(wi as u64 + 7);
        let (mut hits, mut agree) = (0, 0);
        for _ in 0..2000 {
            let o = [rng.range(b.min[0], b.max[0]), rng.range(b.min[1], b.max[1]), rng.range(b.min[2], b.max[2])];
            let d = engine::math::normalize([rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)]);
            for two_sided in [false, true] {
                let a = mesh.ray_cast(o, d, 5000.0, two_sided);
                let t = mesh.tree_ray_cast(o, d, 5000.0, two_sided);
                hits += a.is_some() as usize;
                if a.map(|h| h.t) == t.map(|h| h.t) || matches!((a, t), (Some(x), Some(y)) if (x.t - y.t).abs() < 1e-3)
                {
                    agree += 1;
                }
            }
        }
        eprintln!("world {wi}: {hits} hits, {agree} of 4000 agree");
        assert!(hits > 500);
        assert!(agree >= 3990, "world {wi}: only {agree} of 4000 agree");
    }
}

#[test]
fn paris_collision_world_queries() {
    let Some(d) = disc("paris_collision_world_queries") else { return };
    let w = &worlds(d)[PARIS];
    let cw = CollisionWorld::from_world(w, &OptionTable::sly1()).unwrap();
    let f = QueryFilter::default();
    eprintln!("Paris: {} shapes, {} bodies", cw.shapes.len(), cw.bodies().len());
    assert!(cw.bodies().len() > 50);
    let world_mesh = cw.bodies().iter().position(|b| b.fixed).unwrap();
    // Brute force against every body agrees with the hierarchy.
    let mut rng = Lcg(99);
    let b = cw.body(engine::collision::BodyId(world_mesh as u32)).bounds();
    let mut hits = 0;
    for _ in 0..300 {
        let o = [rng.range(b.min[0], b.max[0]), rng.range(b.min[1], b.max[1]), rng.range(b.min[2], b.max[2])];
        let dir = engine::math::normalize([rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)]);
        let h = cw.ray_cast(o, dir, 20_000.0, &f);
        let mut best: Option<f32> = None;
        for body in cw.bodies() {
            let inv = affine_inverse(&body.transform).unwrap();
            let lo = inv.transform_point(o);
            let ld = engine::math::mat_mul_vec(&inv.cols, dir);
            if let Some(x) = cw.shapes[body.shape.0 as usize].main.ray_cast(lo, ld, 20_000.0, false)
                && best.is_none_or(|b| x.t < b)
            {
                best = Some(x.t);
            }
        }
        match (h.map(|h| h.distance), best) {
            (Some(a), Some(b)) => assert!((a - b).abs() <= 1e-3 * b.max(1.0), "{a} {b}"),
            (a, b) => assert_eq!(a, b),
        }
        hits += h.is_some() as usize;
        // A ground probe from a hit floor point raised a little finds that floor.
        if let Some(h) = h
            && h.normal[2] > 0.9
        {
            let g = cw.ground_probe(engine::math::madd(h.point, [0.0, 0.0, 1.0], 5.0), 50.0, &f).unwrap();
            assert!(g.is_ground && g.distance <= 5.0 + 1e-2);
        }
    }
    eprintln!("Paris: {hits} of 300 random rays hit");
    assert!(hits > 30);
}

// ---------------------------------------------------------------------------------------------------------------
// physics.md

#[test]
fn terminal_fall_constants_from_the_executable() {
    // physics.md 2: −750 and 5 at 0x263E78 and 0x263E7C; −750 · √1.3 = −855.1.
    let Some(d) = disc("terminal_fall_constants_from_the_executable") else { return };
    let t = engine::physics::TerminalFall::from_executable(&d.exe).unwrap();
    assert_eq!((t.speed, t.rate), (-750.0, 5.0));
    assert!(close(t.speed_for(1.3), -855.1, 0.05));
    let mut other = d.exe.clone();
    let last = other.data.len() - 1;
    other.data[last] ^= 1;
    assert!(engine::physics::TerminalFall::from_executable(&other).is_err());
}

#[test]
fn the_tree_puts_points_behind_surfaces_inside() {
    // collision.md 2: the tree describes a solid volume. Just behind a surface (1 unit along −n from its centre) is
    // inside, just in front is outside, for nearly every surface of The Hideout's world mesh.
    let Some(d) = disc("the_tree_puts_points_behind_surfaces_inside") else { return };
    let w = &worlds(d)[HIDEOUT];
    let rec = &w.objects.records[w.objects.top_level[0] as usize];
    let solid = Solid::parse(w.data, rec.offset + 6).unwrap();
    let mesh = TriMesh::new(&solid.mesh, &solid.tree);
    let (mut ok_in, mut ok_out, mut n) = (0, 0, 0);
    for t in &mesh.triangles {
        if t.is_degenerate() {
            continue;
        }
        n += 1;
        let c = engine::math::scale(engine::math::add(engine::math::add(t.v[0], t.v[1]), t.v[2]), 1.0 / 3.0);
        ok_in += mesh.point_inside(engine::math::madd(c, t.normal, -1.0)) as usize;
        ok_out += !mesh.point_inside(engine::math::madd(c, t.normal, 1.0)) as usize;
    }
    eprintln!("Hideout: {ok_in} of {n} inside behind, {ok_out} outside in front");
    // Every point behind is inside; a point in front can fall inside other geometry (thin walls, close parallel
    // surfaces), which 139 of 1,681 do here.
    assert_eq!(ok_in, n);
    assert!(ok_out * 100 >= n * 90);
}
