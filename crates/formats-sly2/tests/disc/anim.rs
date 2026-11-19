//! Animation vectors (animation.md 11): the nightclub hub world.

use super::*;
use engine::anim::curve::{eval_scalar, eval_vector};
use formats_sly2::anim::{Clip, character_clips};

/// The nightclub hub, `f_nightclub_exterior` (world id 0x41, level-table row 2).
pub const HUB: usize = 2;

pub fn character(w: &World<'_>, class: i16) -> Record {
    let mut found = None;
    for b in &w.bodies {
        visit(&b.objects, &mut |r| {
            if found.is_none() && r.class == class {
                found = Some(r.clone());
            }
        });
    }
    found.unwrap()
}

/// (class, body object, transforms, chains, clips, explicit loops, cycles, total length, keys, min, median, max).
type CharacterVector = (i16, i32, usize, usize, usize, usize, usize, f32, usize, f32, f32, f32);

#[test]
fn characters() {
    need_disc!();
    let w = parse(HUB);
    // animation.md 11 (cycles: see the comment below).
    let table: [CharacterVector; 3] = [
        (53, 7, 139, 4, 294, 25, 136, 449.42, 30_419, 0.0833, 1.0, 9.333),
        (55, 8, 76, 2, 174, 10, 108, 166.33, 11_872, 0.0833, 0.5, 8.333),
        (54, 9, 145, 2, 198, 8, 109, 243.73, 28_090, 0.0667, 0.667, 9.167),
    ];
    for (class, id, transforms, chains, n, loops, cycles, total, keys, min, median, max) in table {
        let c = character(&w, class);
        assert_eq!(c.id, id);
        let Body::Solid(s) = &c.body else { panic!("solid") };
        assert_eq!((s.transforms.transforms.len(), s.transforms.chains.len()), (transforms, chains));
        // The table counts clips with at least one channel; with the channel-less ones (events only) the counts are
        // 296, 175 and 203 and the total lengths 450.67, 167.17 and 244.73 s (animation.md 11).
        let all: Vec<Clip> =
            character_clips(&c, None).iter().map(|r| Clip::of_record(w.data, r, options()).unwrap()).collect();
        let (n_all, total_all) = match class {
            53 => (296, 450.67),
            55 => (175, 167.17),
            _ => (203, 244.73),
        };
        assert_eq!(all.len(), n_all, "class {class}");
        assert!((all.iter().map(|c| c.duration).sum::<f32>() - total_all).abs() < 0.01, "class {class}");
        if class == 54 {
            assert_eq!(all.iter().map(|c| c.duration).fold(f32::MAX, f32::min), 0.0);
        }
        let clips: Vec<Clip> = all.into_iter().filter(|c| !c.channels.is_empty()).collect();
        assert_eq!(clips.len(), n, "class {class}");
        assert_eq!(clips.iter().map(|c| c.keys).sum::<usize>(), keys, "class {class}");
        assert_eq!(clips.iter().filter(|c| c.explicit_loop().is_some()).count(), loops, "class {class}");
        let sum: f32 = clips.iter().map(|c| c.duration).sum();
        assert!((sum - total).abs() < 0.01, "class {class}: {sum}");
        let mut d: Vec<f32> = clips.iter().map(|c| c.duration).collect();
        d.sort_by(f32::total_cmp);
        for (got, want) in [(d[0], min), (d[d.len() / 2], median), (d[d.len() - 1], max)] {
            assert!((got - want).abs() < 1e-3, "class {class}: {got} vs {want}");
        }
        // Cycles (animation.md 7): channels on the character's own objects (its body and the solids nested in it at
        // any depth) with a curve, root-motion channels skipped (vector channels on kinds 1, 4, 5, 9, 10 whose parent
        // is transform 0); kind 6 and 7 vector channels and every scalar channel compared modulo 2 pi; every
        // difference within 0.01.
        let mut own: BTreeMap<i32, Vec<(i16, i16)>> = BTreeMap::new();
        fn solids(r: &Record, own: &mut BTreeMap<i32, Vec<(i16, i16)>>) {
            if let Body::Solid(s) = &r.body {
                own.entry(r.id).or_insert_with(|| s.transforms.transforms.iter().map(|t| (t.kind, t.parent)).collect());
                for c in &s.children {
                    solids(c, own);
                }
            }
        }
        solids(&c, &mut own);
        let kinds: Vec<i16> = s.transforms.transforms.iter().map(|t| t.kind).collect();
        let wrap = |d: f32| {
            let tau = std::f32::consts::TAU;
            let d = d.rem_euclid(tau);
            if d > std::f32::consts::PI { d - tau } else { d }
        };
        let cyc = clips
            .iter()
            .filter(|cl| {
                cl.duration > 0.0
                    && cl.channels.iter().all(|ch| {
                        let Some(&(kind, parent)) = own.get(&ch.target).and_then(|t| t.get(ch.transform as usize))
                        else {
                            return true;
                        };
                        let vector_ok = ch.vector.as_ref().is_none_or(|v| {
                            if [1, 4, 5, 9, 10].contains(&kind) && parent == 0 {
                                return true;
                            }
                            let (a, b) = (eval_vector(v, 0.0).unwrap(), eval_vector(v, cl.duration).unwrap());
                            a.iter().zip(b).all(|(x, y)| {
                                let d = y - x;
                                (if kind == 6 || kind == 7 { wrap(d) } else { d }).abs() <= 0.01
                            })
                        });
                        let scalar_ok = ch.scalar.as_ref().is_none_or(|sc| {
                            wrap(eval_scalar(sc, cl.duration).unwrap() - eval_scalar(sc, 0.0).unwrap()).abs() <= 0.01
                        });
                        vector_ok && scalar_ok
                    })
            })
            .count();
        assert_eq!(cyc, cycles, "class {class}");
        // Vector channels on the character itself target only kinds 1, 6 and 12; scalar channels only the upper
        // bones of its chains (animation.md 4).
        let uppers: Vec<i16> = s.transforms.chains.iter().map(|ch| ch.upper).collect();
        for cl in &clips {
            for ch in cl.channels.iter().filter(|ch| ch.target == c.id) {
                let kind = kinds[ch.transform as usize];
                if ch.vector.is_some() {
                    assert!([1, 6, 12].contains(&kind), "class {class}: vector channel on kind {kind}");
                }
                if ch.scalar.is_some() {
                    assert!(uppers.contains(&ch.transform), "class {class}: scalar channel off a chain");
                }
            }
        }
    }
    // Sly's chains and the three mirror pairs (animation.md 6, 11).
    let sly = character(&w, 53);
    let Body::Solid(s) = &sly.body else { panic!() };
    let chains: Vec<(i16, i16, i16, i16)> =
        s.transforms.chains.iter().map(|c| (c.upper, c.lower, c.end, c.handle)).collect();
    assert_eq!(chains, [(55, 56, 56, 15), (57, 58, 58, 21), (68, 69, 69, 29), (65, 66, 66, 36)]);
    for (class, source, target) in [(53, 10, 73), (55, 75, 52), (54, 11, 84)] {
        let c = character(&w, class);
        let Body::Solid(s) = &c.body else { panic!() };
        let m = s.transforms.mirrors[0];
        assert_eq!((m.source, m.target, m.copy_position, m.copy_rotation), (source, target, false, true));
    }
    let t1 = &s.transforms.transforms[1];
    assert_eq!((t1.kind, t1.parent, t1.local.t), (1, 0, [0.0, 0.0, 75.0]));
}

fn close3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

#[test]
fn sly_run_clip() {
    need_disc!();
    let w = parse(HUB);
    let sly = character(&w, 53);
    let run = character_clips(&sly, None).into_iter().find(|r| r.id == 126).unwrap();
    // animation.md 11 says "record at 0x8013EB": that is the body; the 13-byte record header starts at 0x8013DE.
    assert_eq!((run.header_offset, run.body_offset, run.body_size), (0x8013DE, 0x8013EB, 6_723));
    let clip = Clip::of_record(w.data, run, options()).unwrap();
    assert_eq!((clip.duration, clip.channels.len()), (0.5, 72));
    assert_eq!((clip.on_apply.len(), clip.timed.len(), clip.on_retract.len()), (1, 4, 1));
    let timed: Vec<(i8, i16, i32)> = clip
        .timed
        .iter()
        .map(|e| (e.kind, e.frame.unwrap(), i32::from_le_bytes(e.payload[..4].try_into().unwrap())))
        .collect();
    assert_eq!(timed, [(11, 5, 383), (11, 15, 384), (11, 20, 381), (11, 30, 382)]);
    let ch = |transform: i16| clip.channels.iter().find(|c| c.target == 7 && c.transform == transform).unwrap();
    // animation.md 11: channel values at t = 0, 0.025, 0.25, 0.5.
    for (transform, values) in [
        (1, [[0.0, 0.0, 75.0], [16.0044, 0.0, 75.0], [160.0, 0.0, 75.0], [320.0, 0.0, 75.0]]),
        (
            10,
            [
                [1.6423, 4.8024, -0.0568],
                [1.6423, 4.8036, -0.0591],
                [1.6414, 4.8131, -0.0689],
                [1.6423, 4.8024, -0.0568],
            ],
        ),
        (
            12,
            [
                [55.7351, -13.2539, -15.3383],
                [64.6980, -15.1424, -4.3464],
                [-75.7771, -18.0971, -50.7465],
                [55.7351, -13.2539, -15.3383],
            ],
        ),
    ] {
        let v = ch(transform).vector.as_ref().unwrap();
        for (t, want) in [0.0, 0.025, 0.25, 0.5].into_iter().zip(values) {
            let got = eval_vector(v, t).unwrap();
            assert!(close3(got, want, 2e-3), "transform {transform} at {t}: {got:?} vs {want:?}");
        }
    }
    // The root channel is per-axis (vector kind 4); transform 10 has two one-slope keys; transform 12's first key.
    use formats::anim::{ScalarCurve, VectorCurve};
    assert!(matches!(ch(1).vector, Some(VectorCurve::Components { .. })));
    let Some(VectorCurve::Hermite(k10)) = &ch(10).vector else { panic!("hermite") };
    assert_eq!((k10.len(), k10[0].frame, k10[1].frame), (2, 0, 30));
    assert!(close3(k10[0].value, [1.642331, 4.802381, -0.056751], 1e-5) && k10[0].tangent_in == k10[0].tangent_out);
    assert!(close3(k10[0].tangent_in, [0.0, 0.047374, -0.097288], 1e-5));
    let Some(VectorCurve::Hermite(k12)) = &ch(12).vector else { panic!("hermite") };
    assert!(close3(k12[0].value, [55.73511, -13.25388, -15.33829], 1e-4));
    assert!(close3(k12[0].tangent_in, [1335.1135, -257.7299, 416.3185], 1e-3));
    assert!(close3(k12[0].tangent_out, [242.7754, -75.5506, 256.9352], 1e-3));
    // The swivel of Sly's first chain (upper bone 55): keys at frames 0, 10, 25; -0.21512 at 0.25 s.
    let sw = ch(55).scalar.as_ref().unwrap();
    let ScalarCurve::Hermite(k) = sw else { panic!("hermite") };
    assert_eq!((k[0].frame, k[1].frame, k[2].frame), (0, 10, 25));
    assert!(
        (k[0].value + 0.339796).abs() < 1e-5
            && (k[0].slope_in + 1.821102).abs() < 1e-5
            && (k[0].slope_out - 0.186607).abs() < 1e-5
    );
    assert!(
        (k[1].value + 0.308700).abs() < 1e-5
            && (k[1].slope_in - 0.186622).abs() < 1e-5
            && (k[1].slope_out - 1.122883).abs() < 1e-5
    );
    assert!((eval_scalar(sw, 0.25).unwrap() + 0.21512).abs() < 1e-4);
    // Root motion: transform 1 from (0, 0, 75) to (320, 0, 75) in 0.5 s, a rate of 640 units per second forward.
    let rate = engine::anim::curve::sample_vector(ch(1).vector.as_ref().unwrap(), 0.25).unwrap().rate;
    assert!((rate[0] - 640.0).abs() < 1.0, "{rate:?}");
}

#[test]
fn lookup_tables() {
    need_disc!();
    use formats_sly2::anim::*;
    let d = disc();
    let w = parse(HUB);
    // animation.md 9: per table and character: (records, clips, state machines, other objects found, not found,
    // transform IDs found, transform-ID records, parts).
    type Row = ((u32, usize), i16, usize, usize, usize, usize, usize, usize, usize, usize);
    let rows: [Row; 7] = [
        (SLY_TABLES[0], 53, 173, 129, 4, 10, 6, 2, 2, 22),
        (SLY_TABLES[1], 53, 18, 2, 1, 6, 0, 0, 0, 9),
        (BENTLEY_TABLE, 55, 118, 68, 1, 12, 17, 0, 0, 20),
        (MURRAY_TABLE, 54, 137, 83, 4, 7, 17, 0, 0, 26),
        (SHARED_TABLE, 53, 53, 29, 3, 9, 9, 2, 2, 1),
        (SHARED_TABLE, 55, 53, 26, 3, 6, 15, 2, 2, 1),
        (SHARED_TABLE, 54, 53, 10, 2, 6, 32, 1, 2, 1),
    ];
    for (table, class, n, clips, machines, others, missing, transforms, transform_records, parts) in rows {
        let l = read_lookup_table(&d.executable, table).unwrap();
        assert_eq!(l.len(), n);
        let c = character(&w, class);
        let (mut got, mut tr) = ([0usize; 6], 0usize);
        for x in &l {
            if x.is_transform() {
                tr += 1;
            }
            match resolve_lookup(&c, x) {
                Resolved::Object(r) if r.class == 109 => got[0] += 1,
                Resolved::Object(r) if r.class == 111 => got[1] += 1,
                Resolved::Object(_) => got[2] += 1,
                Resolved::NotFound if !x.is_transform() => got[3] += 1,
                Resolved::Transform(_) => got[4] += 1,
                Resolved::Part => got[5] += 1,
                Resolved::NotFound => {}
            }
        }
        assert_eq!(
            got,
            [clips, machines, others, missing, transforms, parts],
            "table {:#x} for class {class}",
            table.0
        );
        assert_eq!(tr, transform_records);
    }
}

#[test]
fn euler_convention() {
    need_disc!();
    // Constant rotation channels of Sly's clips that equal the loaded rest rotation in exactly one of the two
    // orientations: all of them in the columns form (Affine::cols = Rz Ry Rx), none in the transposed one.
    use formats::anim::VectorCurve;
    use formats_sly2::rig::euler_zyx;
    let w = parse(HUB);
    let sly = character(&w, 53);
    let Body::Solid(s) = &sly.body else { panic!() };
    let (mut cols_only, mut transposed_only) = (0, 0);
    for r in character_clips(&sly, None) {
        let cl = Clip::of_record(w.data, r, options()).unwrap();
        for ch in cl.channels.iter().filter(|c| c.target == 7) {
            let t = &s.transforms.transforms[ch.transform as usize];
            if t.kind != 6 {
                continue;
            }
            if let Some(VectorCurve::Components { mask: 0, base, .. }) = &ch.vector {
                let m = euler_zyx(*base);
                let tr = [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]];
                let eq = |x: [[f32; 3]; 3]| (0..3).all(|i| (0..3).all(|j| (x[i][j] - t.local.cols[i][j]).abs() < 1e-3));
                cols_only += (eq(m) && !eq(tr)) as usize;
                transposed_only += (eq(tr) && !eq(m)) as usize;
            }
        }
    }
    assert_eq!((cols_only, transposed_only), (432, 0));
}

#[test]
fn pose_sly_with_the_run_clip() {
    need_disc!();
    use formats::geometry::Affine;
    use formats_sly2::rig::Rig;
    let w = parse(HUB);
    let sly = character(&w, 53);
    let Body::Solid(s) = &sly.body else { panic!() };
    let mut rig = Rig::new(&s.transforms, &Affine::IDENTITY, &[]);
    // The bind pose is the loaded pose.
    assert_eq!(rig.world, s.transforms.world_matrices(&Affine::IDENTITY, &[]));
    assert!(rig.chain_binds.iter().all(|b| b.is_some()), "all four chains bind");
    let run = character_clips(&sly, None).into_iter().find(|r| r.id == 126).unwrap();
    let clip = Clip::of_record(w.data, run, options()).unwrap();
    let mut reached = 0;
    for t in [0.0f32, 0.1, 0.25, 0.4, 0.5] {
        rig.reset();
        for ch in clip.channels.iter().filter(|c| c.target == 7) {
            if let Some(v) = ch.vector.as_ref().and_then(|v| eval_vector(v, t)) {
                rig.set_vector(ch.transform as usize, v);
            }
            if let Some(a) = ch.scalar.as_ref().and_then(|c| eval_scalar(c, t)) {
                rig.set_swivel(ch.transform as usize, a);
            }
        }
        rig.update(&Affine::IDENTITY, &[]);
        assert!(rig.world.iter().all(|m| m.cols.iter().flatten().chain(m.t.iter()).all(|v| v.is_finite())), "t {t}");
        // Root motion: transform 1 carries the clip's translation (320 units in 0.5 s).
        assert!(
            (rig.world[1].t[0] - 640.0 * t).abs() < 0.05 && (rig.world[1].t[2] - 75.0).abs() < 1e-3,
            "t {t}: {:?}",
            rig.world[1].t
        );
        // The mirror pair: transform 73's world rotation is transform 10's.
        assert_eq!(rig.world[73].cols, rig.world[10].cols);
        // Every chain whose goal is within reach puts its effector on the goal.
        for (c, b) in rig.chains.clone().iter().zip(rig.chain_binds.clone()) {
            let b = b.unwrap();
            let (eff, goal) = (rig.chain_effector(c).unwrap(), rig.chain_goal(c).unwrap());
            let upper = rig.world[c.upper as usize].t;
            let d = ((0..3).map(|k| (goal[k] - upper[k]).powi(2)).sum::<f32>()).sqrt();
            let miss = ((0..3).map(|k| (goal[k] - eff[k]).powi(2)).sum::<f32>()).sqrt();
            if d > (b.a - b.b).abs() + 0.5 && d < b.a + b.b - 0.5 {
                assert!(miss < 0.05, "t {t}, chain {}: effector {miss} from its goal", c.upper);
                reached += 1;
            }
        }
    }
    eprintln!("chain goals within reach and reached: {reached} of 20");
    assert!(reached >= 10, "the chain check ran on {reached} of 20 chain poses");
}
