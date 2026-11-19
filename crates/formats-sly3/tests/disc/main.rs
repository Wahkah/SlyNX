//! Disc tests: the test vectors of `specs/sly3/`. They run when `SLYNX_DISC_SLY3` names the player's own Sly 3
//! (USA) image and skip (passing) otherwise. Run in release mode: every world is decompressed and walked.

use formats::hash::{hex, sha256};
use formats::iso::ReadAt;
use formats_sly3::geometry::{EFFECT_OUTLINE, Group, INDEXED, OUTLINE, STRIPS};
use formats_sly3::scene::{self, SceneOptions};
use formats_sly3::stream::{Body, Layout, Record, visit};
use formats_sly3::textures::{self, GRADING};
use formats_sly3::{Disc, World};
use std::collections::BTreeMap;
use std::sync::OnceLock;

mod vectors;
use vectors::{CLASSES, WORLDS};

const MENU: usize = 0;
const TRAINER: usize = 2;
const APT: usize = 5;

fn disc_path() -> Option<String> {
    std::env::var("SLYNX_DISC_SLY3").ok().filter(|p| !p.is_empty())
}

fn disc() -> Disc<std::fs::File> {
    Disc::open_path(disc_path().unwrap()).unwrap()
}

/// Every world, decompressed once for the whole test binary: (slot, bytes).
fn worlds() -> &'static Vec<(usize, Vec<u8>)> {
    static DATA: OnceLock<Vec<(usize, Vec<u8>)>> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut disc = disc();
        let slots: Vec<usize> = disc.world_slots().collect();
        slots.into_iter().map(|s| (s, disc.read_world(s).unwrap())).collect()
    })
}

fn world_data(slot: usize) -> &'static [u8] {
    &worlds().iter().find(|(s, _)| *s == slot).unwrap().1
}

fn options() -> &'static formats_sly2::exe::OptionTable {
    static T: OnceLock<formats_sly2::exe::OptionTable> = OnceLock::new();
    T.get_or_init(|| disc().options)
}

fn parse(slot: usize) -> World<'static> {
    World::parse(world_data(slot), options()).unwrap()
}

/// Every world parsed once.
fn parsed() -> &'static Vec<World<'static>> {
    static P: OnceLock<Vec<World<'static>>> = OnceLock::new();
    P.get_or_init(|| worlds().iter().map(|(s, _)| parse(*s)).collect())
}

fn apt() -> &'static World<'static> {
    &parsed()[worlds().iter().position(|(s, _)| *s == APT).unwrap()]
}

macro_rules! need_disc {
    () => {
        if disc_path().is_none() {
            eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
            return;
        }
    };
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

/// Visits every record of every group of `w`.
fn each_record<'w>(w: &'w World<'_>, f: &mut impl FnMut(&'w Record)) {
    for b in &w.bodies {
        visit(&b.objects, f);
    }
}

fn find_record<'w>(w: &'w World<'_>, body_offset: u32) -> &'w Record {
    let mut out = None;
    each_record(w, &mut |r| {
        if r.body_offset == body_offset {
            out = Some(r);
        }
    });
    out.unwrap()
}

#[test]
fn identity_and_executable_tables() {
    need_disc!();
    let d = disc();
    assert_eq!(d.volume.system_id, "PLAYSTATION");
    assert_eq!(d.volume.volume_id, "");
    assert_eq!(d.volume.creation_date, "2005082817072100");
    assert_eq!(d.executable.data.len(), 3_200_076);
    assert_eq!(hex(&d.executable.sha256), "ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338");
    assert_eq!(d.executable.va_to_offset(0x0010_0000), Some(0x1000));
    // object-stream.md 1.1: file offset = address − 0x100000 + 0x1000.
    assert_eq!(d.executable.va_to_offset(0x0032_CA80), Some(0x22DA80));
    // object-stream.md 6 gives these 20 bytes for entry 27 at 0x22BCF8; on the disc they are entry 26's (at
    // 0x22DA80 + 26 × 40). Entry 27 has the same type and flags and a different handler address.
    assert_eq!(hexs(&d.executable.data[0x22DE90..0x22DE90 + 20]), "1200000001440000880a100000000000880a1000");
    assert_eq!(hexs(&d.executable.data[0x22DEB8..0x22DEB8 + 20]), "1200000001440000080b100000000000800b1000");
    let e = d.options.get(27).unwrap();
    assert_eq!((e.value_type, e.flags), (18, 0x4401));
    assert!(e.is_typed());
    assert_eq!(hexs(&d.executable.data[0x2419E0..0x2419E0 + 20]), "0000000000000000f90000001000000000000000");
    assert_eq!(d.classes.len(), 249);
    assert_eq!((d.classes[0].base, d.classes[0].class, d.classes[0].last_derived, d.classes[0].size), (0, 0, 249, 16));
    assert_eq!(
        (d.classes[1].base, d.classes[1].class, d.classes[1].last_derived, d.classes[1].size),
        (0x3409E0, 1, 2, 0x5000)
    );
    assert!(d.classes.iter().any(|c| c.class == 8));
    // animation.md 2.1: the tangent multipliers.
    assert_eq!(d.tangent_multipliers, [60.0, 10.0, 10.0]);
    // disc.md 5.1 and 6: the world slots.
    assert_eq!(d.worlds.len(), 40);
    assert_eq!(d.worlds[1].name, "sampler_menu");
    assert!(d.worlds[1].file.is_empty());
    assert_eq!(d.world_slots().count(), 39);
    for &(slot, name, ep, id, sector, compressed, ..) in WORLDS.iter() {
        let w = &d.worlds[slot];
        assert_eq!((w.name.as_str(), w.episode, w.world_id()), (name, ep, id), "slot {slot}");
        assert_eq!((w.file.sector, w.file.size), (sector, compressed), "slot {slot}");
    }
    assert_eq!(d.find_world("v_apt"), Some(APT));
}

#[test]
fn movie_table() {
    need_disc!();
    let mut d = disc();
    // disc.md 5.2.
    const MOVIES: [(u32, u32, u32); 23] = [
        (444753, 77496324, 0x14F),
        (482594, 44302340, 0x150),
        (504227, 510345220, 0x151),
        (1526531, 61390852, 0x130),
        (71778, 54394884, 0x12F),
        (3232, 70549508, 0x14E),
        (1556508, 55754756, 0x134),
        (387661, 27705348, 0x135),
        (1583733, 49987588, 0x139),
        (332545, 30507012, 0x13A),
        (1608142, 63995908, 0x13E),
        (253157, 35815428, 0x13F),
        (1639391, 39616516, 0x144),
        (177776, 34652164, 0x145),
        (161495, 33341444, 0x146),
        (1658736, 45088772, 0x14A),
        (98339, 28377092, 0x14B),
        (1680753, 49741828, 0x14C),
        (37681, 69828612, 0x14D),
        (1705042, 1376260, 0x152),
        (1705715, 5832708, 0x153),
        (1705042, 1376260, 0x152),
        (1705715, 5832708, 0x153),
    ];
    assert_eq!(d.movies.len(), 23);
    for (i, (m, &(sector, size, id))) in d.movies.iter().zip(MOVIES.iter()).enumerate() {
        assert_eq!((m.sector, m.size, m.identity), (sector, size, id), "movie {i}");
    }
    let m = d.movies[5];
    let mut head = [0u8; 4];
    d.source().read_at(m.offset(), &mut head).unwrap();
    assert_eq!(head, [0, 0, 1, 0xBA]);
}

#[test]
fn menu_world_compressed() {
    need_disc!();
    let mut d = disc();
    let c = d.read_world_compressed(MENU).unwrap();
    assert_eq!(c.len(), 1_906_746);
    assert_eq!(hex(&sha256(&c)), "a27ff71985596e0cf9864c1d9c8d80a6de2614fe50723381e85067eaa96f8b45");
    assert_eq!(hexs(&c[..16]), "ff00000f00464b2458ff616c6c5f7761");
}

#[test]
fn world_list_and_geometry_counts() {
    need_disc!();
    let mut totals = [0usize; 10];
    let mut bodies = 0u64;
    let mut errors = Vec::new();
    for (w, (slot, data)) in parsed().iter().zip(worlds().iter()) {
        let (slot, data) = (*slot, data.as_slice());
        let row = WORLDS.iter().find(|r| r.0 == slot).unwrap();
        let (
            _,
            name,
            _,
            _,
            _,
            _,
            size,
            sha,
            groups,
            shared,
            objects,
            placements,
            shaders,
            tail,
            solids,
            meshes,
            filtered,
            tris,
            edges,
        ) = *row;
        assert_eq!(data.len(), size, "{name}");
        assert_eq!(hex(&sha256(data)), sha, "{name}");
        assert_eq!(w.groups.len(), groups, "{name}");
        assert_eq!(w.groups.iter().filter(|g| g.shared != 0).count(), shared, "{name}");
        let top: usize = w.bodies.iter().map(|b| b.objects.len()).sum();
        assert_eq!(top, objects, "{name}");
        let pl: usize = w.bodies.iter().map(|b| b.objects.iter().filter(|r| r.class == 8).count()).sum();
        assert_eq!(pl, placements, "{name}");
        let sh: usize = w.bodies.iter().map(|b| b.materials.shaders.len()).sum();
        assert_eq!(sh, shaders, "{name}");
        assert_eq!(w.tail, tail, "{name}");
        let (mut s, mut m, mut f, mut t, mut e) = (0, 0, 0, 0, 0);
        each_record(w, &mut |r| {
            if let Body::Solid(b) = &r.body {
                s += 1;
                for mesh in &b.meshes.meshes {
                    m += 1;
                    f += usize::from(!mesh.filter.is_empty());
                    t += mesh.triangle_count(w.data);
                    e += mesh.edge_count();
                }
            }
        });
        assert_eq!((s, m, f, t, e), (solids, meshes, filtered, tris, edges), "{name}");
        for (k, v) in
            [groups, objects, placements, solids, meshes, filtered, tris, edges, shaders, 1].into_iter().enumerate()
        {
            totals[k] += v;
        }
        bodies += w.object_bodies;
        errors.extend(w.errors.iter().map(|e| (slot, e.class, e.body_size)));
        let last = w.bodies.last().unwrap();
        assert_eq!(last.end + w.tail, data.len());
        assert!(w.tail <= 8192);
    }
    assert_eq!(&totals[..8], &[7_151, 46_221, 8_121, 11_524, 51_967, 9_987, 2_692_438, 1_316_380]);
    assert_eq!(totals[9], 39);
    // object-stream.md 2.5: every body parses field-exactly except one class-157 body of 360 bytes.
    assert_eq!(bodies, 125_693);
    assert_eq!(errors, [(MENU, 157, 360)]);
}

#[test]
fn section_boundaries() {
    need_disc!();
    // world-file.md 7.
    type Row = (usize, [usize; 10], usize, usize, &'static str, usize, usize);
    let rows: [Row; 3] = [
        (
            MENU,
            [0, 2, 0x52C, 0x17308, 0x1C9A0, 0x2B754, 0x2B75A, 0x2B7B3, 0x2B7BA, 0x2B7C0],
            920,
            0,
            "dvd_menu",
            0x321790,
            0x321960,
        ),
        (
            TRAINER,
            [0, 2, 0x40FC, 0x1AED8, 0x32960, 0x433D6, 0x43400, 0x4355F, 0x43566, 0x43570],
            923,
            0,
            "i_trainer",
            0x85EA30,
            0x8DDEF0,
        ),
        (
            APT,
            [0, 0x4A, 0x2CFC, 0x19AD8, 0x528D3, 0x70035, 0x7005F, 0x70434, 0x7049F, 0x704A0],
            1465,
            5,
            "v_apt",
            0x61D5B0,
            0x72B0F0,
        ),
    ];
    for (slot, b, strings, nodes, level, start, end) in rows {
        let w = &parsed()[worlds().iter().position(|(s, _)| *s == slot).unwrap()];
        let s = w.sections;
        assert_eq!(
            [
                s.keys,
                s.groups,
                s.save_variables,
                s.sound,
                s.strings,
                s.options,
                s.option_sets,
                s.partitions,
                s.partitions_end,
                s.first_body
            ],
            b,
            "slot {slot}"
        );
        assert_eq!(w.strings.len(), strings);
        assert_eq!(w.partitions.iter().map(|p| p.nodes.len()).sum::<usize>(), nodes);
        let g = w.group(level).unwrap();
        assert_eq!((w.bodies[g].offset, w.bodies[g].end), (start, end), "slot {slot}");
        assert_eq!(w.level_group(level), Some(g));
        assert_eq!(w.bodies[0].offset, s.first_body);
    }
}

#[test]
fn v_apt_header() {
    need_disc!();
    let w = apt();
    // world-file.md 3.1 and 7: key tables.
    assert_eq!(
        hexs(&w.data[..0x4A]),
        hexs(&unhex(
            "02eb0800 00010106 00000069 09000002 00061307 00000200 ec080000 01010600 00001207 00000101 06000000 \
         f8080000 01010600 00000009 00000101 06000000 10090000 01010600 0000"
        ))
    );
    assert_eq!((w.keys[0].len(), w.keys[1].len()), (2, 6));
    assert_eq!((w.keys[0][0].key, w.keys[0][0].value, w.keys[0][0].list.clone()), (2283, 1, vec![6]));
    // 3.2: group entries.
    let g = |i: usize| {
        let e = &w.groups[i];
        (e.name.as_str(), e.shared, e.streaming, e.id, e.second, e.copies, e.body_size)
    };
    assert_eq!(g(0), ("v_chair_fancy", 0, 0, 138293, 140925, 50, 29_328));
    assert_eq!(g(1), ("v_divan", 0, 0, 137751, 140926, 2, 40_416));
    assert_eq!(g(129), ("stock_objects", 1, 0, 9900, 16866, 1, 95_552));
    // 3.3: save variables.
    assert_eq!(w.save_variables.len(), 1201);
    assert_eq!(w.save_variable(0), Some((256, &b"t0_npc_kill"[..])));
    assert_eq!(w.save_variable(1), Some((512, &b"t2_heist_boss_a_hit2"[..])));
    assert_eq!(w.save_variable(2), Some((768, &b"t4_heist_alarm6"[..])));
    // 3.5: UTF-16 strings.
    assert_eq!(w.languages, [0]);
    assert!(w.strings[0].text[0].starts_with("Sly 3 \u{a9} 2005 Sony Computer Entertainment America & Sucker Pu"));
    assert_eq!(w.string(512), Some("Jump (tap twice for double jump)"));
    assert_eq!(w.characters.len(), 1);
    // 3.7 and 3.8.
    assert!((1..=4).contains(&w.option_sets.ids.len()));
    assert_eq!(w.partitions.len(), 1);
    let p = &w.partitions[0];
    assert_eq!((p.condition, p.nodes.len()), (12, 5));
    let n = p.nodes[0];
    assert_eq!(n.normal, [0.0, 1.0, 0.0]);
    assert!(close(n.distance, 2856.19, 0.01));
    assert_eq!((n.front, n.back), (1, 3));
    // 4: the first group's stamp.
    assert_eq!(w.bodies[0].stamp, 0x4310CF2E);
    assert_eq!(w.fresh_context(Some(APT as i16)).condition, w.option_sets.ids[0]);
}

#[test]
fn header_surveys() {
    need_disc!();
    let mut combos = BTreeMap::new();
    let mut copies = [0usize; 3];
    let (mut script_groups, mut modules, mut graphs) = (0, 0u32, 0);
    for w in parsed() {
        assert!(w.keys[0].len() <= 3 && w.keys[1].len() <= 12);
        assert!((15..=401).contains(&w.groups.len()));
        assert!((920..=1631).contains(&w.strings.len()));
        assert!((1..=4).contains(&w.option_sets.sets.len()));
        assert_eq!(w.save_variables.len(), 1201);
        assert_eq!(w.languages, [0]);
        assert!(w.partitions.iter().map(|p| p.nodes.len()).sum::<usize>() <= 104);
        for (g, b) in w.groups.iter().zip(&w.bodies) {
            *combos.entry((g.shared, g.streaming != 0)).or_insert(0) += 1;
            copies[(g.copies.clamp(0, 2)) as usize] += 1;
            assert_eq!(b.end - b.offset, g.body_size as usize);
            assert_eq!(b.debug, 0);
            script_groups += usize::from(b.scripts > 0);
            modules += b.scripts;
            graphs += usize::from(b.graph_entries > 0);
        }
        // Copy counts after set 0 cover every group.
        assert_eq!(w.copy_counts(0).len(), w.groups.len());
    }
    assert_eq!(combos.into_iter().collect::<Vec<_>>(), [((0, false), 6369), ((0, true), 450), ((1, false), 332)]);
    let mut after = [0usize; 3];
    for w in parsed() {
        for c in w.copy_counts(0).values() {
            after[(*c).clamp(0, 2) as usize] += 1;
        }
    }
    // world-file.md 3.2's copy counts (0: 36, 1: 3,713, more: 3,402) are those after option set 0 (3.7); the
    // stored counts have no zero.
    assert_eq!(after, [36, 3713, 3402]);
    assert_eq!(copies, [0, 3732, 3419]);
    assert_eq!((script_groups, modules, graphs), (1524, 11_007, 25));
}

#[test]
fn class_counts() {
    need_disc!();
    let mut top: BTreeMap<i16, usize> = BTreeMap::new();
    let mut all: BTreeMap<i16, usize> = BTreeMap::new();
    let mut worlds_of: BTreeMap<i16, usize> = BTreeMap::new();
    let mut modes = [0usize; 4];
    let (mut only_filter, mut only_cond, mut both) = (0, 0, 0);
    for w in parsed() {
        let mut here = std::collections::BTreeSet::new();
        for b in &w.bodies {
            for r in &b.objects {
                *top.entry(r.class).or_insert(0) += 1;
                modes[r.mode as usize] += 1;
                if r.mode == 1 {
                    match (r.filter.slots.is_empty(), r.filter.conditions.is_empty()) {
                        (false, true) => only_filter += 1,
                        (true, false) => only_cond += 1,
                        (false, false) => both += 1,
                        _ => {}
                    }
                }
            }
            visit(&b.objects, &mut |r: &Record| {
                *all.entry(r.class).or_insert(0) += 1;
                here.insert(r.class);
            });
        }
        for c in here {
            *worlds_of.entry(c).or_insert(0) += 1;
        }
    }
    assert_eq!(modes, [27_779, 16_554, 1_685, 203]);
    assert_eq!((only_filter, only_cond, both), (15_115, 1_362, 77));
    assert_eq!(all.len(), CLASSES.len());
    for &(class, t, n, wc) in CLASSES.iter() {
        let tt = top.get(&class).copied().unwrap_or(0);
        assert_eq!((tt, all[&class] - tt, worlds_of[&class]), (t, n, wc), "class {class}");
    }
}

#[test]
fn v_apt_records_and_placements() {
    need_disc!();
    let w = apt();
    let g = w.group("v_apt").unwrap();
    let objects = &w.bodies[g].objects;
    assert_eq!(objects.len(), 198);
    let mut by_class = BTreeMap::new();
    for r in objects {
        *by_class.entry(r.class).or_insert(0) += 1;
    }
    assert_eq!(by_class.into_iter().collect::<Vec<_>>(), [(4, 35), (5, 1), (8, 160), (48, 1), (136, 1)]);
    let first = &objects[0];
    assert_eq!(first.header_offset, 0x6C4C86);
    assert_eq!(hexs(&w.data[0x6C4C86..0x6C4C86 + 13]), "00300001000000ffff65220600");
    assert_eq!((first.mode, first.class, first.id, first.script, first.body_size), (0, 48, 1, -1, 402_021));
    assert_eq!(first.body_offset, 0x6C4C93);
    let p = objects.iter().find(|r| r.class == 8).unwrap();
    assert_eq!((p.id, p.body_size), (139_880, 66));
    let Body::Placement(pl) = &p.body else { panic!() };
    assert_eq!((pl.group_id, pl.mode, pl.parent, pl.zone_mask, pl.binds.len()), (149_054, -1, -1, 255, 0));
    assert_eq!(w.groups[w.group_by_id(149_054).unwrap()].name, "v_alarm_light");
    assert!(!pl.fresh);
    let m = pl.matrix;
    let eq3 = |a: [f32; 3], b: [f32; 3], eps: f32| a.iter().zip(b).all(|(x, y)| close(*x, y, eps));
    assert!(eq3(m.cols[0], [-0.02794, 0.0, -0.99961], 1e-5));
    assert!(eq3(m.cols[1], [0.0, 1.0, 0.0], 1e-5));
    assert!(eq3(m.cols[2], [0.99961, 0.0, -0.02794], 1e-5));
    assert!(eq3(m.t, [-3481.34, 4987.41, 371.49], 0.01));
}

#[test]
fn placement_counts() {
    need_disc!();
    // Placements the game reads: every depth except inside fresh placements' data (which the game skips).
    fn walk(rs: &[Record], depth: u32, f: &mut impl FnMut(&formats_sly3::stream::Placement, u32)) {
        for r in rs {
            if let Body::Placement(p) = &r.body {
                f(p, depth);
                if p.fresh {
                    continue;
                }
            }
            walk(r.nested(), depth + 1, f);
        }
    }
    let mut all = 0;
    let mut kinds = BTreeMap::new();
    let mut top = BTreeMap::new();
    let mut parents = BTreeMap::new();
    let (mut binds, mut targets) = ([0usize; 6], BTreeMap::new());
    for w in parsed() {
        each_record(w, &mut |r| all += usize::from(r.class == 8));
        for b in &w.bodies {
            walk(&b.objects, 0, &mut |p, depth| {
                let near = p.parent == -1 || p.parent == 0x2000;
                *parents.entry(if near { p.parent } else { 0x1000 }).or_insert(0) += 1;
                if near {
                    *kinds.entry((p.fresh, p.mode)).or_insert(0) += 1;
                    binds[p.binds.len().min(5)] += 1;
                    for b in &p.binds {
                        *targets.entry(b.target.clamp(0, 9)).or_insert(0) += 1;
                    }
                }
                if depth == 0 {
                    *top.entry((p.fresh, p.mode)).or_insert(0) += 1;
                }
            });
        }
    }
    println!("top {top:?} kinds {kinds:?} binds {binds:?} targets {targets:?} parents {parents:?}");
    // object-stream.md 3.2: 8,121 top-level and 11,061 nested records of class 8.
    assert_eq!(all, 19_182);
    assert_eq!(parents[&-1], 8_121);
    // classes/placement.md 1 counts copies 6,657 (mode −1: 6,637, mode 0: 20) and fresh 1,474 (860, 289, 325)
    // over 8,131 placements: the top-level ones and 10 nested with parent 0x2000. On the disc 17 nested placements
    // outside fresh data have parent 0x2000; with them, mode −1 copies are 6,644 (7 more).
    assert_eq!(parents[&0x2000], 17);
    let k = |f: bool, m: i8| kinds.get(&(f, m)).copied().unwrap_or(0);
    assert_eq!((k(false, -1), k(false, 0)), (6_644, 20));
    assert_eq!((k(true, -1), k(true, 1), k(true, 2)), (860, 289, 325));
    assert_eq!(top.values().sum::<usize>(), 8_121);
    assert_eq!(binds, [6_750, 1_081, 267, 25, 6, 9]);
    assert_eq!((targets[&7], targets[&8]), (796, 592));
    // Top level alone: 6,637 and 20 copies; 850, 289 and 325 fresh.
    let t = |f: bool, m: i8| top.get(&(f, m)).copied().unwrap_or(0);
    assert_eq!((t(false, -1), t(false, 0), t(true, -1), t(true, 1), t(true, 2)), (6_637, 20, 850, 289, 325));
}

#[test]
fn v_apt_textures() {
    need_disc!();
    let w = apt();
    let g = w.group("v_apt").unwrap();
    let b = &w.bodies[g];
    assert_eq!(b.offset, 0x61D5B0);
    assert_eq!(b.texture_data, (0x61FF00, 675_200));
    let m = &b.materials;
    assert_eq!((m.palettes().len(), m.bitmaps().len(), m.shaders.len()), (126, 64, 88));
    // textures.md 6.
    #[rustfmt::skip]
    type Row = (usize, u8, u8, &'static str, usize, &'static str, usize, &'static str, &'static str, &'static str, &'static str, &'static str);
    let rows: [Row; 5] = [
        (
            0,
            1,
            2,
            "4128000001020100ffffffffffffffff0000000000000201",
            62,
            "432800000102000010001000140201008000000020010800",
            114,
            "22300000010200008107010020550000",
            "23292cff262b2fff",
            "2c373cff313a42ff",
            "5011f422a92a23fddb499ddf812227a5e1dfabcdbbe45149844aa7c7e0600413",
            "e698f82e3d6c97dd1f93b2846e3f773f33423082a659329321dc1f09a8fc66d8",
        ),
        (
            2,
            1,
            2,
            "0e1d020001020100ffffffffffffffff0000000000000201",
            8,
            "101d020001020000800080001302400000400000a0e90400",
            105,
            "861e02000102000001800400e03e0000",
            "6a5331ff6a5331ff",
            "b6843affb6843aff",
            "b19bbc8404a9893251c2d6f9b90c67b0c581fe68e6fe950a2f5e4b351da2a0e8",
            "efb29efbf8d463f951f8e93c29a70edd99af802077e63821a4f568f860b36a43",
        ),
        (
            7,
            0,
            2,
            "411d020000020100ffffffffffffffff0000000000000201",
            57,
            "c265010001020000200020001402020000020000a0fb0700",
            109,
            "de6701000102000001080100e04e0000",
            "5d3107ff643b09ff",
            "8e4500ff9e5700ff",
            "52f18ab43306a8444cde1297928b45fa48f155dbf9ab5fcb9866573b0c14a9d9",
            "ec5d5d6b07fa36f3bd15545dbc84983fd094775c6c7c21880c18129940141741",
        ),
        (
            22,
            1,
            9,
            "d93a020001020100ffffffffffffffff0000000082020901",
            7,
            "db3a020001010000800080001302400000400000a0a90400",
            103,
            "003f02000101000001800400e0360000",
            "0000000000000000",
            "0000000000000000",
            "45779e69bbab7a1b950732b6919607c53278ca990558b681707ac2fff35f2110",
            "df378fc01d2a8aa453a88b6730b6b06da1e9f7b7d59e0f9ea9ee8b6bcb78e03c",
        ),
        (
            24,
            1,
            2,
            "ed3a020001020100dcdcdcffdcdcdcff0000000000000201",
            63,
            "e207000001020000080008001402010020000000a0010800",
            0,
            "e4070000010200008100010000000000",
            "807f76ff807f76ff",
            "cdcdc7ffcdcdc7ff",
            "08844f180db8bce2edd946f5805adc4946bbac285c7a773fc0faf25c57d368de",
            "20ac5c68a770c5a49e8a47c391ff34cb08df9c99a760f31e5ae5a0e2f69072eb",
        ),
    ];
    let tex = b.texture_bytes(w.data);
    for (si, kind, pass, sh, bi, bh, pi, ph, raw_first, graded_first, raw_sha, graded_sha) in rows {
        let s = &m.shaders[si];
        assert_eq!((s.kind, s.pass), (kind, pass), "shader {si}");
        assert_eq!(hexs(&w.data[s.offset as usize..s.offset as usize + 24]), sh, "shader {si}");
        let (fb, fp) = s.frame(0, 0).unwrap();
        assert_eq!((fb as usize, fp.map(|p| p as usize)), (bi, Some(pi)), "shader {si}");
        let bmp = &m.bitmaps()[bi];
        let pal = &m.palettes()[pi];
        assert_eq!(hexs(&w.data[bmp.offset as usize..bmp.offset as usize + 24]), bh, "shader {si}");
        assert_eq!(hexs(&w.data[pal.offset as usize..pal.offset as usize + 16]), ph, "shader {si}");
        let raw = textures::decode(bmp, pal, w.data, tex, None).unwrap();
        let graded = textures::decode(bmp, pal, w.data, tex, Some(&GRADING)).unwrap();
        assert_eq!(hexs(&raw.pixels[..8]), raw_first, "shader {si}");
        assert_eq!(hexs(&graded.pixels[..8]), graded_first, "shader {si}");
        assert_eq!(hex(&sha256(&raw.pixels)), raw_sha, "shader {si}");
        assert_eq!(hex(&sha256(&graded.pixels)), graded_sha, "shader {si}");
    }
    assert_eq!(m.shaders[22].params, [0x82, 0x02]);
}

#[test]
fn texture_totals() {
    need_disc!();
    let mut t = [0usize; 4];
    let mut kinds = BTreeMap::new();
    for w in parsed() {
        for b in &w.bodies {
            let m = &b.materials;
            t[0] += m.palettes().len();
            t[1] += m.bitmaps().len();
            t[2] += m.shaders.len();
            t[3] += m.animations.len();
            for s in &m.shaders {
                *kinds.entry(s.kind).or_insert(0) += 1;
                // textures.md 4: every frame has a bitmap and its palettes.
                for f in 0..s.frames.len() {
                    assert!(s.frame(f, 0).is_some());
                }
            }
        }
    }
    assert_eq!(t, [69_748, 36_414, 35_655, 545]);
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        [(0, 3527), (1, 29_941), (2, 1060), (3, 565), (4, 74), (5, 44), (6, 309), (7, 135)]
    );
}

#[test]
fn v_apt_geometry_and_transforms() {
    need_disc!();
    let w = apt();
    // geometry.md 8, transforms.md 4, classes/solid-object.md 4: the level object.
    let r = find_record(w, 0x6C4C93);
    let Body::Solid(s) = &r.body else { panic!() };
    let t = &s.transforms;
    assert_eq!(t.transforms.len(), 2);
    assert_eq!((t.transforms[0].parent, t.transforms[0].kind, t.transforms[0].id), (-1, 1, 1));
    assert_eq!((t.transforms[1].parent, t.transforms[1].kind, t.transforms[1].id), (0, 6, 1));
    assert!(t.chains.is_empty() && t.mirrors.is_empty() && t.followers == 0);
    assert_eq!(t.references, [0, 1, 1]);
    assert_eq!(s.zone_mask, 0xFF);
    let meshes = &s.meshes.meshes;
    assert_eq!(meshes.len(), 101);
    assert!(meshes.iter().all(|m| m.body_type == STRIPS && m.filter.is_empty()));
    assert_eq!(meshes.iter().map(|m| m.triangle_count(w.data)).sum::<usize>(), 4447);
    assert_eq!(s.collision.len(), 1);
    let c = &s.collision[0];
    assert_eq!((c.shape, c.mesh.positions.len(), c.mesh.triangles.len(), c.raw_tree.0), (-1, 1390, 1356, 3411));
    assert_eq!(s.options.options.len(), 1);
    assert!(s.children.is_empty());
    let m = &meshes[0];
    assert_eq!(m.offset, 0x6C4D2B);
    // The stated size runs to the next mesh's two filter-count bytes.
    assert_eq!(meshes[1].offset - m.offset, 276 + 2);
    assert!(m.copies.is_empty());
    assert_eq!((m.transform, m.pass, m.flags, m.billboard), (1, 2, 0x08, -1));
    assert_eq!(m.draw_order, f32::MAX);
    assert_eq!(m.fog, f32::MAX);
    assert!(m.bones.is_empty());
    assert_eq!(m.groups.len(), 1);
    let Group::Strips { lists, positions, .. } = &m.groups[0] else { panic!() };
    assert_eq!(*positions, 4);
    assert_eq!(lists.iter().map(|l| (l.count, l.shader, l.flags)).collect::<Vec<_>>(), [(4, 50, 35), (3, 50, 35)]);
    let v = m.groups[0].vertices(w.data);
    assert!(close(v[0].position[0], -296.3776, 1e-3) && close(v[0].position[1], -5000.658, 1e-3));
    assert!(close(v[0].position[2], 1106.9935, 1e-3));
    assert_eq!(v[0].uv, [0.0, 1.0]);
    assert!(close(v[0].normal[1], 0.10029, 1e-5) && close(v[0].normal[2], -0.99496, 1e-5) && v[0].normal[0] == 0.0);
    assert_eq!(v[0].color, [0xFF; 4]);
    let tris = m.groups[0].triangles(w.data);
    assert_eq!(tris, [(50, vec![[2, 1, 3]]), (50, vec![[0, 1, 2]])]);

    // Sly (class 52, object 24).
    let r = find_record(w, 0x37FDE3);
    assert_eq!((r.class, r.id, r.body_size), (52, 24, 1_090_509));
    let Body::Solid(s) = &r.body else { panic!() };
    let name = s.name.unwrap();
    assert_eq!(&w.data[name.0 as usize..(name.0 + name.1) as usize], b"jt");
    let t = &s.transforms;
    assert_eq!(t.transforms.len(), 146);
    let t0 = t.transforms[0];
    assert_eq!((t0.parent, t0.kind, t0.id, t0.flags), (-1, 0, -1, 0));
    let t1 = t.transforms[1];
    assert_eq!((t1.parent, t1.kind, t1.id, t1.local.t), (0, 1, 24, [0.0, 0.0, 75.0]));
    assert_eq!(t.chains.len(), 4);
    assert_eq!(t.mirrors.len(), 1);
    let mi = t.mirrors[0];
    assert_eq!((mi.source, mi.target, mi.copy_position, mi.copy_rotation), (6, 73, false, true));
    assert_eq!(t.followers, 4);
    assert_eq!(t.references, [1, 2, 2]);
    let meshes = &s.meshes.meshes;
    assert_eq!(meshes.len(), 97);
    let count = |k: i8| meshes.iter().filter(|m| m.body_type == k).count();
    assert_eq!((count(STRIPS), count(OUTLINE), count(EFFECT_OUTLINE)), (52, 38, 7));
    assert_eq!(meshes.iter().filter(|m| !m.filter.is_empty()).count(), 4);
    assert_eq!(meshes.iter().map(|m| m.triangle_count(w.data)).sum::<usize>(), 3019);
    assert_eq!(s.collision.len(), 28);
    assert_eq!(s.options.options.len(), 25);
    assert_eq!(s.children.len(), 356);
    let m = &meshes[0];
    assert_eq!(m.offset, 0x382058);
    assert_eq!(meshes[1].offset - m.offset, 3653 + 2);
    assert_eq!((m.body_type, m.transform, m.pass, m.flags), (STRIPS, 4, 3, 0x09));
    let Group::Strips { lists, positions, .. } = &m.groups[0] else { panic!() };
    assert_eq!(*positions, 25);
    assert_eq!(lists.len(), 2);
    assert!(lists.iter().all(|l| l.shader == 7 && l.flags == 3));
    let v = m.groups[0].vertices(w.data);
    assert!(close(v[0].position[0], 5.6830, 1e-3) && close(v[0].position[1], -14.3578, 1e-3));
    assert!(close(v[0].position[2], -1.3138, 1e-3));
    let tris = m.groups[0].triangles(w.data);
    assert_eq!(&tris[0].1[..3], &[[0, 1, 2], [2, 1, 3], [2, 3, 4]]);
}

#[test]
fn geometry_surveys() {
    need_disc!();
    let mut types = [0usize; 4];
    let (mut hidden, mut no_body, mut copies, mut tables, mut boned, mut bones) = (0, 0, 0, 0, 0, 0);
    let (mut no_bounds, mut trees, mut named, mut parts, mut tree_nodes) = (0, 0, 0, 0, 0);
    let mut passes: BTreeMap<(i8, i8), usize> = BTreeMap::new();
    let mut weighted = 0usize;
    for w in parsed() {
        each_record(w, &mut |r| {
            let Body::Solid(s) = &r.body else { return };
            named += usize::from(s.name.is_some());
            parts += s.collision.len();
            no_bounds += usize::from(s.meshes.no_bounds);
            trees += usize::from(s.meshes.tree_nodes > 0);
            tree_nodes += s.meshes.tree_nodes.max(0) as usize;
            for m in &s.meshes.meshes {
                types[m.body_type as usize] += 1;
                hidden += usize::from(m.hidden);
                no_body += usize::from(m.no_body);
                copies += m.copies.len();
                tables += usize::from(!m.color_tables.is_empty());
                boned += usize::from(!m.bones.is_empty());
                bones += m.bones.len();
                *passes.entry((m.body_type, m.pass)).or_insert(0) += 1;
                assert!(m.filter.is_empty() || m.copies.is_empty());
                for g in &m.groups {
                    let n = match g {
                        Group::Strips { positions, weights: Some(_), .. } => *positions as usize,
                        Group::Indexed { positions, weights: Some(_), .. } => *positions as usize,
                        _ => 0,
                    };
                    for v in 0..n {
                        let (_, wt) = g.vertex_weights(w.data, v).unwrap();
                        assert_eq!(wt.iter().map(|&x| x as u32).sum::<u32>(), 255);
                        weighted += 1;
                    }
                }
            }
        });
    }
    assert_eq!(types, [40_078, 9_516, 1_137, 1_236]);
    assert_eq!((hidden, no_body, copies), (8_392, 1_620, 26_084));
    assert_eq!((tables, boned, bones), (16_141, 4_808, 53_377));
    // geometry.md 2.2's "14,846 trees" is the total node count; 63 sets have a non-empty tree.
    assert_eq!((no_bounds, trees, tree_nodes), (63, 63, 14_846));
    assert_eq!((named, parts), (436, 5_632));
    // geometry.md 7: passes by body type.
    let get = |t: i8, p: i8| passes.get(&(t, p)).copied().unwrap_or(0);
    assert_eq!(get(STRIPS, 2), 17_186);
    assert_eq!(get(STRIPS, 9), 11_427);
    assert_eq!(get(OUTLINE, 8), 9_516);
    assert_eq!(get(EFFECT_OUTLINE, 6), 1_137);
    assert_eq!((get(INDEXED, 6), get(INDEXED, 7)), (1_218, 18));
    println!("weighted vertices (strips and indexed) {weighted}");
}

#[test]
fn animation_and_logic_counts() {
    need_disc!();
    let (mut clips, mut top_clips, mut deferred, mut machines, mut scripts) = (0, 0, 0, 0, 0);
    let (mut effects, mut components, mut emitters_with_mesh) = (0, 0, 0);
    let mut component_classes = BTreeMap::new();
    let (mut external, mut inline) = (0, 0);
    let mut elements = BTreeMap::new();
    let mut nested = BTreeMap::new();
    let mut deferred_groups = BTreeMap::new();
    for w in parsed() {
        for (gi, b) in w.bodies.iter().enumerate() {
            top_clips += b.objects.iter().filter(|r| r.class == 130).count();
            visit(&b.objects, &mut |r: &Record| match &r.body {
                Body::Clip(c) => {
                    clips += 1;
                    if c.deferred {
                        deferred += 1;
                        *deferred_groups.entry(w.groups[gi].name.clone()).or_insert(0) += 1;
                        assert!(c.data.is_none());
                    } else {
                        let d = c.data.as_ref().unwrap();
                        assert!(d.duration() >= 0.0);
                    }
                }
                Body::Other { layout: Layout::StateMachine, .. } => machines += 1,
                Body::Other { layout: Layout::CommandScript, .. } => scripts += 1,
                Body::Effect(e) => {
                    effects += 1;
                    for c in &e.components {
                        components += 1;
                        *component_classes.entry(c.class).or_insert(0) += 1;
                    }
                }
                Body::Emitter(e) => emitters_with_mesh += usize::from(e.mesh.is_some()),
                Body::Screen(s) => {
                    if s.external.is_some() {
                        external += 1;
                    } else {
                        inline += 1;
                    }
                    for e in &s.elements {
                        *elements.entry(e.kind).or_insert(0) += 1;
                        for n in &e.nested {
                            *nested.entry(n.kind).or_insert(0) += 1;
                        }
                    }
                }
                _ => {}
            });
        }
        assert_eq!(w.component_bodies as usize, {
            let mut n = 0;
            each_record(w, &mut |r| {
                if let Body::Effect(e) = &r.body {
                    n += e.components.len();
                }
            });
            n
        });
    }
    assert_eq!((clips, top_clips, machines, scripts), (42_891, 174, 2_846, 6_411));
    assert_eq!(deferred, 468);
    assert_eq!(deferred_groups.get("jt"), Some(&(13 * 36)));
    assert_eq!((effects, components), (7_172, 17_768));
    assert_eq!(component_classes.into_iter().collect::<Vec<_>>(), [(191, 15_300), (192, 2_468)]);
    assert_eq!(emitters_with_mesh, 36);
    assert_eq!((external, inline), (234, 2_172));
    // Element counts are over every depth: kind-8 glyphs are 7,410 kind-1 and 78 kind-0 elements.
    for (k, n) in &nested {
        *elements.entry(*k).or_insert(0) += n;
    }
    assert_eq!(nested.into_iter().collect::<Vec<_>>(), [(0, 78), (1, 7_410)]);
    let e = |k: u32| elements.get(&k).copied().unwrap_or(0);
    assert_eq!((e(1), e(3), e(4), e(6), e(7), e(8)), (25_062, 9_531, 9_497, 232, 47, 117));
    assert_eq!((e(0), e(2), e(5), e(9)), (78, 0, 279, 1_526));
}

#[test]
fn v_apt_scene() {
    need_disc!();
    let w = apt();
    let ctx = w.fresh_context(Some(APT as i16));
    let b = scene::build(w, SceneOptions { context: ctx, ..SceneOptions::default() });
    let s = &b.scene;
    println!(
        "v_apt scene: {} meshes, {} instances, {} textures, {} texture errors, {} unresolved",
        s.meshes.len(),
        s.instances.len(),
        s.textures.len(),
        s.texture_errors.len(),
        b.unresolved_placements
    );
    assert!(s.instances.len() > 500);
    assert!(s.textures.len() > 100);
    assert!(s.texture_errors.is_empty());
    assert_eq!(b.unresolved_placements, 0);
    assert_eq!(b.material_base.len(), w.groups.len());
    for sub in &s.submeshes {
        assert!((sub.material as usize) < s.materials.len());
        let idx = &s.indices[sub.first_index as usize..(sub.first_index + sub.index_count) as usize];
        assert!(idx.iter().all(|&i| (i as u32) < sub.vertex_count));
    }
    // Every world builds.
    for (w, (slot, _)) in parsed().iter().zip(worlds()) {
        let b = scene::build(w, SceneOptions { decode_textures: false, ..SceneOptions::default() });
        // The menu world draws nothing with its static meshes (its solids are hidden or outline-only).
        assert_eq!(b.scene.instances.is_empty(), *slot == MENU, "slot {slot}");
    }
}

#[test]
fn collision_meshes_and_trees() {
    need_disc!();
    let (mut parts, mut large, mut triangles, mut nodes, mut stored) = (0usize, 0usize, 0usize, 0usize, 0u64);
    let mut outside = 0usize;
    let mut equal_counts = 0usize;
    for w in parsed() {
        each_record(w, &mut |r| {
            let Body::Solid(s) = &r.body else { return };
            for p in &s.collision {
                parts += 1;
                if p.tree.nodes.len() > 65_534 {
                    // Too many inner nodes for 16-bit node indices; `WideCollisionTree` holds them.
                    large += 1;
                    assert_eq!(r.class, 48, "only level objects have trees that large");
                }
                p.tree.validate(p.mesh.triangles.len()).unwrap();
                triangles += p.mesh.triangles.len();
                nodes += p.tree.nodes.len();
                stored += p.raw_tree.0 as u64;
                assert_eq!(p.raw_tree.0, p.raw_tree.1, "the second count copies the first");
                equal_counts += usize::from(p.raw_tree.2 == p.raw_tree.0);
                // Each surface's sphere holds its corners (within rounding); the mesh radius holds every position.
                for t in 0..p.mesh.triangles.len() {
                    let tri = p.mesh.triangles[t];
                    for c in p.mesh.corners(t) {
                        let d = (0..3).map(|k| (c[k] - tri.center[k]).powi(2)).sum::<f32>().sqrt();
                        outside += usize::from(d > tri.radius * 1.001 + 0.01);
                    }
                }
                let r = p.mesh.radius * 1.0001 + 1e-3;
                assert!(p.mesh.positions.iter().all(|q| q.iter().map(|x| x * x).sum::<f32>().sqrt() <= r));
            }
        });
    }
    eprintln!(
        "{parts} parts, {large} trees past 16-bit indices, {triangles} surfaces, {nodes} inner nodes of {stored} stored,          {equal_counts} third counts equal, {outside} corners outside"
    );
    // classes/solid-object.md 1: 5,632 parts. Every tree converts (children after parents, one parent each); three
    // level objects' trees (75,558 to 103,211 stored nodes) overflow 16-bit node indices.
    assert_eq!((parts, large), (5_632, 3));
    assert_eq!(outside, 0, "surface spheres hold their corners");
    // The v_apt level object's part (classes/solid-object.md 4).
    let w = apt();
    let Body::Solid(s) = &find_record(w, 0x6C4C93).body else { panic!() };
    let p = &s.collision[0];
    assert_eq!((p.shape, p.mesh.positions.len(), p.mesh.triangles.len(), p.raw_tree.0), (-1, 1390, 1356, 3411));
    assert!(!p.tree.nodes.is_empty());
    // A converted tree passes the engine's check: every child after its parent, with one parent.
    p.tree.validate(p.mesh.triangles.len()).unwrap();
}
