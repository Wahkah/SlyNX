//! Disc tests: the test vectors of `specs/sly2/`. They run when `SLYNX_DISC_SLY2` names the player's own Sly 2
//! (USA) image and skip (passing) otherwise. Run in release mode: every world is decompressed and walked.

use formats::hash::{hex, sha256};
use formats::reader::Reader;
use formats_sly2::geometry::{Group, INDEXED, RIGID, SKINNED, STREAMED};
use formats_sly2::stream::{Body, Record, visit};
use formats_sly2::textures;
use formats_sly2::{Disc, World};
use std::collections::BTreeMap;
use std::sync::OnceLock;

mod vectors;
use vectors::WORLDS;

const CELLAR: usize = 3;
const INTRO: usize = 0;

fn disc_path() -> Option<String> {
    std::env::var("SLYNX_DISC_SLY2").ok().filter(|p| !p.is_empty())
}

fn disc() -> Disc<std::fs::File> {
    Disc::open_path(disc_path().unwrap()).unwrap()
}

/// Every world, decompressed once for the whole test binary: (row, bytes).
fn worlds() -> &'static Vec<(usize, Vec<u8>)> {
    static WORLDS_DATA: OnceLock<Vec<(usize, Vec<u8>)>> = OnceLock::new();
    WORLDS_DATA.get_or_init(|| {
        let mut disc = disc();
        let rows: Vec<usize> = disc.world_rows().collect();
        rows.into_iter().map(|row| (row, disc.read_world(row).unwrap())).collect()
    })
}

fn world_data(row: usize) -> &'static [u8] {
    &worlds().iter().find(|(r, _)| *r == row).unwrap().1
}

fn options() -> &'static formats_sly2::exe::OptionTable {
    static T: OnceLock<formats_sly2::exe::OptionTable> = OnceLock::new();
    T.get_or_init(|| disc().options)
}

fn parse(row: usize) -> World<'static> {
    World::parse(world_data(row), options()).unwrap()
}

macro_rules! need_disc {
    () => {
        if disc_path().is_none() {
            eprintln!("skipped: SLYNX_DISC_SLY2 is not set");
            return;
        }
    };
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn identity_and_file_table() {
    need_disc!();
    let d = disc();
    assert_eq!(d.volume.system_id, "PLAYSTATION");
    assert_eq!(d.volume.creation_date, "2004081304445000");
    assert_eq!(d.executable.data.len(), 2_597_048);
    assert!(d.executable.is_supported());
    assert_eq!(d.executable.va_to_offset(0x002A_32A8), Some(0x1A3328));
    assert_eq!(d.executable.va_to_offset(0x0047_2200), Some(0x1DD380));
    // disc.md 5: sector, size, identity of the 29 records.
    const TABLE: [(u32, u32, u32); 29] = [
        (1298436, 40452100, 0x7D9),
        (58380, 21381124, 0x7DA),
        (1326206, 42369028, 0x7DE),
        (92343, 20103172, 0x7DF),
        (124137, 38912004, 0x7E3),
        (220097, 33865732, 0x7E4),
        (260612, 46399492, 0x7E7),
        (36306, 25542660, 0x7E8),
        (1268530, 35274756, 0x7EC),
        (1285755, 25968644, 0x7ED),
        (102160, 45006852, 0x7F1),
        (961247, 24248324, 0x7F2),
        (878801, 37683204, 0x7F6),
        (236634, 28672004, 0x7F7),
        (159131, 36126724, 0x7FA),
        (1346895, 85835780, 0x7FB),
        (897202, 61325316, 0x80B),
        (204695, 9666564, 0x802),
        (29785, 13352964, 0x7D4),
        (973088, 345489412, 0x801),
        (1141785, 56918020, 0x7D5),
        (350144, 1949759, 0x039),
        (350144, 1949759, 0x039),
        (959782, 2998276, 0x808),
        (335488, 9969830, 0x05D),
        (1169578, 25688, 0x7CF),
        (350144, 1949759, 0x039),
        (959782, 2998276, 0x808),
        (1388808, 6275076, 0x80A),
    ];
    let got: Vec<_> = d.files.iter().map(|f| (f.sector, f.size, f.identity)).collect();
    assert_eq!(got, TABLE);
    let mut d = d;
    let boot = d.files[24];
    assert_eq!(
        hex(&sha256(&d.read_file(&boot).unwrap())),
        "af84b84e273d38f2ff56736e22fb8396fcd2eca2d31e672546c132c0e015f2d2"
    );
    // The save icon (record 25) starts with the magic 0x00010000.
    let icon = d.read_file(&d.files[25].clone()).unwrap();
    assert_eq!(&icon[..4], &[0, 0, 1, 0]);
    // Movies start with an MPEG-2 pack header.
    let mut movie = [0u8; 4];
    let movie_at = d.files[0].offset();
    formats::iso::ReadAt::read_at(d.source(), movie_at, &mut movie).unwrap();
    assert_eq!(movie, [0, 0, 1, 0xBA]);
}

#[test]
fn level_table() {
    need_disc!();
    let d = disc();
    assert_eq!(d.rows.len(), 44);
    assert!(d.rows[18].file.is_empty());
    for v in WORLDS {
        let r = &d.rows[v.row];
        assert_eq!(
            (r.world_id(), r.episode, r.slot, r.hub, r.file.sector, r.file.size),
            (v.id, v.episode, v.slot, v.hub, v.sector, v.compressed),
            "row {}",
            v.row
        );
        assert_eq!(r.save_path.len(), 4, "row {}", v.row);
        assert_eq!((r.save_path[0], r.save_path[2]), (64, 67));
        assert_eq!(r.save_path[3] == 68, r.hub, "row {}: a hub's path ends with 68", v.row);
    }
    assert_eq!(d.world_rows().count(), 43);
    assert_eq!(d.find_world(0x3B), Some(CELLAR));
    // Every world carries the same level table.
    let boot = formats_sly2::world::read_header_start(world_data(INTRO)).unwrap();
    assert_eq!(boot.level_table_word, 30);
    for (row, data) in worlds() {
        let h = formats_sly2::world::read_header_start(data).unwrap();
        assert_eq!(h.level_rows, d.rows, "row {row}");
    }
}

/// Counts of worlds.md 1 for one world.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    records: usize,
    placements: usize,
    solids: usize,
    meshes: usize,
    groups: usize,
    triangles: usize,
}

fn counts(w: &World<'_>) -> Counts {
    let mut c = Counts::default();
    fn records(rs: &[Record], c: &mut Counts) {
        for r in rs {
            c.records += 1;
            if let Body::Placement(p) = &r.body {
                c.placements += 1;
                records(&p.extra, c);
            }
        }
    }
    for b in &w.bodies {
        records(&b.objects, &mut c);
        visit(&b.objects, &mut |r| {
            if let Body::Solid(s) = &r.body {
                c.solids += 1;
                for m in &s.meshes.meshes {
                    c.meshes += 1;
                    c.groups += m.groups.len();
                    c.triangles += m.triangle_count(w.data);
                }
            }
        });
    }
    c
}

#[test]
fn every_world() {
    need_disc!();
    let mut totals = Counts::default();
    let mut bodies = 0u64;
    let mut undecided = 0usize;
    for v in WORLDS {
        let data = world_data(v.row);
        assert_eq!(data.len(), v.size, "row {} size", v.row);
        assert_eq!(hex(&sha256(data)), v.sha256, "row {} hash", v.row);
        let w = World::parse(data, options()).unwrap();
        assert!(w.errors.is_empty(), "row {}: {:?}", v.row, &w.errors[..w.errors.len().min(3)]);
        assert_eq!(w.header.packages.len(), v.packages, "row {} packages", v.row);
        assert_eq!(w.level_package().0.name, v.name, "row {}", v.row);
        assert_eq!(w.level_package().0.keep, 1);
        assert_eq!(w.tail, v.tail, "row {} tail", v.row);
        assert!(w.bodies.iter().all(|b| b.offset % 16 == 0 && b.debug == 0));
        let c = counts(&w);
        // worlds.md counts the solids among copy-placement extras too (row 32 is the only world with any: 6).
        assert_eq!(
            c,
            Counts {
                records: v.records,
                placements: v.placements,
                solids: v.solids,
                meshes: v.meshes,
                groups: v.groups,
                triangles: v.triangles
            },
            "row {} ({})",
            v.row,
            v.name
        );
        for (a, b) in [
            (&mut totals.records, c.records),
            (&mut totals.solids, c.solids),
            (&mut totals.meshes, c.meshes),
            (&mut totals.triangles, c.triangles),
        ] {
            *a += b;
        }
        bodies += w.object_bodies;
        // The emitter origin-mesh rule decides every emitter (classes/effects-logic-ui.md 1); the only undecided
        // bodies are kind -1 placements naming no package (classes/placement.md 1.3).
        assert!(w.undecided.iter().all(|u| u.0 == 6), "row {}: {:?}", v.row, w.undecided);
        undecided += w.undecided.len();
    }
    assert_eq!((totals.records, totals.solids, totals.meshes, totals.triangles), (42_097, 11_250, 53_994, 2_821_629));
    // Every body at every depth parses field by field to its declared size (worlds.md 5).
    assert_eq!(bodies, 121_153);
    assert_eq!(undecided, 296);
}

#[test]
fn class_totals_at_every_depth() {
    need_disc!();
    let mut per_class: BTreeMap<i16, usize> = BTreeMap::new();
    for (row, _) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            visit(&b.objects, &mut |r| *per_class.entry(r.class).or_default() += 1);
        }
    }
    // Totals stated in classes/*.md and animation.md (every depth).
    for (class, n) in [
        (5, 30),
        (9, 9),
        (10, 3140),
        (11, 246),
        (12, 43),
        (14, 179),
        (15, 619),
        (17, 986),
        (22, 399),
        (23, 42),
        (24, 482),
        (25, 3),
        (26, 370),
        (27, 159),
        (29, 6230),
        (31, 1958),
        (32, 52),
        (33, 8381),
        (41, 228),
        (42, 282),
        (48, 8723),
        (49, 118),
        (98, 617),
        (105, 252),
        (109, 38560),
        (111, 2603),
        (114, 3871),
        (126, 1659),
        (127, 41),
        (128, 41),
        (129, 212),
        (130, 43),
        (131, 1),
        (132, 388),
        (133, 4080),
    ] {
        assert_eq!(per_class.get(&class).copied().unwrap_or(0), n, "class {class}");
    }
}

#[test]
fn section_boundaries() {
    need_disc!();
    // world-file.md 6: (row, sound, save vars, level table, strings, tree, options, options end, first body,
    // level package body start and end, level texture data offset and size, strings, tree nodes, file size).
    let table = [
        (
            0, 0x4E6E, 0xAA94, 0x1D709, 0x1E421, 0x22628, 0x22936, 0x2297C, 0x22980, 0x8FE580, 0x109CF10, 0x904A30,
            1_325_792, 419, 39, 0x109D2B6,
        ),
        (
            1, 0xFE, 0x660, 0x132D5, 0x13FED, 0x17157, 0x17159, 0x1718D, 0x17190, 0x76E30, 0x3E5140, 0x76F30, 34_816,
            359, 0, 0x3E5377,
        ),
        (
            3, 0x3872, 0x15EAB, 0x28B20, 0x29838, 0x31765, 0x31A23, 0x31A75, 0x31A80, 0x7A2120, 0xADF6A0, 0x7A54F0,
            866_096, 605, 35, 0xADFE6B,
        ),
        (
            43, 0x564E, 0x15823, 0x28498, 0x291B0, 0x2FB86, 0x2FC50, 0x2FC9C, 0x2FCA0, 0xE1CBF0, 0xF4ABC0, 0xE1E190,
            111_264, 551, 10, 0xF4B217,
        ),
    ];
    for (
        row,
        sound,
        save,
        level,
        strings,
        tree,
        opts,
        opts_end,
        first,
        lp_start,
        lp_end,
        td,
        td_size,
        nstr,
        nodes,
        size,
    ) in table
    {
        let w = parse(row);
        let s = w.header.sections;
        assert_eq!(
            (s.sound, s.save_variables, s.level_table, s.strings, s.tree, s.options, s.options_end, s.first_body),
            (sound, save, level, strings, tree, opts, opts_end, first),
            "row {row}"
        );
        let lp = w.bodies.last().unwrap();
        assert_eq!((lp.offset, lp.end, lp.texture_data), (lp_start, lp_end, (td, td_size)), "row {row}");
        assert_eq!((w.strings, w.tree.len(), w.data.len()), (nstr, nodes, size), "row {row}");
    }
}

#[test]
fn cellar_world() {
    need_disc!();
    let w = parse(CELLAR);
    let data = w.data;
    // world-file.md 6.
    let names: Vec<&str> = w.header.packages[..3].iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["f_pottedplant_flower", "f_door_transition_1", "abandon_icon"]);
    assert_eq!([w.bodies[0].offset, w.bodies[1].offset, w.bodies[2].offset], [0x31A80, 0x3C480, 0x3F6F0]);
    assert_eq!(hexs(&data[0x31A80..0x31A8C]), "40530000c27b1c4100000000");
    assert_eq!(w.bodies[0].stamp, 0x411C_7BC2);
    let n = w.tree[0];
    assert_eq!((n.normal, n.front, n.back), ([0.0, 1.0, 0.0], 1, 18));
    assert!((n.distance - 538.391).abs() < 1e-3);

    // object-stream.md 6.
    let lp = w.bodies.last().unwrap();
    assert_eq!(lp.objects.len(), 289);
    let first = &lp.objects[0];
    assert_eq!(
        hexs(&data[first.header_offset as usize..first.header_offset as usize + 13]),
        "00310001000000ffffda012600"
    );
    assert_eq!(
        (first.kind, first.class, first.id, first.script, first.body_size, first.body_offset),
        (0, 49, 1, -1, 2_490_842, 0x878C33)
    );
    let mut hist: BTreeMap<i16, usize> = BTreeMap::new();
    for r in &lp.objects {
        *hist.entry(r.class).or_default() += 1;
    }
    assert_eq!(hist.into_iter().collect::<Vec<_>>(), [(4, 91), (6, 186), (13, 1), (43, 10), (49, 1)]);

    // classes/solid-object.md 4 and geometry.md 10: the level geometry.
    let Body::Solid(s) = &first.body else { panic!("class 49 is solid") };
    assert_eq!(s.transforms.transforms.len(), 31);
    let t0 = &s.transforms.transforms[0];
    assert_eq!((t0.parent, t0.local), (-1, formats::geometry::Affine::IDENTITY));
    assert_eq!(s.zone_mask, 0xFF);
    let meshes = &s.meshes.meshes;
    assert_eq!(meshes.len(), 541);
    assert_eq!(meshes.iter().filter(|m| m.body_type == RIGID).count(), 539);
    assert_eq!(meshes.iter().filter(|m| m.body_type == INDEXED).count(), 2);
    assert_eq!(meshes.iter().map(|m| m.triangle_count(data)).sum::<usize>(), 26_908);
    assert_eq!(s.collision.len(), 1);
    let c = &s.collision[0];
    assert_eq!((c.mesh.positions.len(), c.mesh.triangles.len(), c.raw_tree.0), (8_103, 10_386, 30_659));
    assert_eq!(c.shape, -1);
    assert_eq!(s.options.options.len(), 1);
    assert!(s.children.is_empty());
    let m = &meshes[0];
    assert_eq!(m.offset, 0x87933F);
    assert_eq!((m.body_type, m.copies.len(), m.transform, m.pass, m.max_distance), (RIGID, 0, 1, 10, f32::MAX));
    assert!(s.meshes.no_bounds && m.sphere.is_none() && m.bones.is_empty());
    assert_eq!(m.groups.len(), 1);
    let g = &m.groups[0];
    let Group::Rigid { lists, positions, .. } = g else { panic!("rigid group") };
    assert_eq!(*positions, 8);
    assert_eq!(lists.iter().map(|l| (l.count, l.shader, l.flags)).collect::<Vec<_>>(), [(4, 146, 35), (4, 146, 35)]);
    let v = g.vertices(data);
    let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
    assert!(close(v[0].position, [7117.232, 153.992, 312.1772]), "{:?}", v[0].position);
    assert!((v[0].uv[0] - 1.0).abs() < 1e-4 && (v[0].uv[1] - 0.5718).abs() < 1e-4);
    assert!(close(v[0].normal, [-0.67462, -0.73807, -0.01160]));
    assert_eq!(v[0].color, [0xFF; 4]);
    assert_eq!(g.triangles(data)[0].1, [[0, 1, 2], [2, 1, 3]]);
}

fn find_record(w: &World<'_>, class: i16, id: i32) -> Record {
    let mut found = None;
    for b in &w.bodies {
        visit(&b.objects, &mut |r| {
            if found.is_none() && r.class == class && r.id == id {
                found = Some(r.clone());
            }
        });
    }
    found.expect("record")
}

#[test]
fn cellar_player_character() {
    need_disc!();
    let w = parse(CELLAR);
    let data = w.data;
    let r = &find_record(&w, 53, 7);
    assert_eq!((r.class, r.id, r.body_offset, r.body_size), (53, 7, 0x2E9AB3, 1_354_878));
    let Body::Solid(s) = &r.body else { panic!("solid") };
    let (o, l) = s.name.unwrap();
    assert_eq!(&data[o as usize..(o + l) as usize], b"jt");
    // transforms.md 5.
    let t = &s.transforms;
    assert_eq!(
        (t.transforms.len(), t.chains.len(), t.mirrors.len(), t.followers, t.references),
        (139, 4, 1, 4, [1, 2, 2])
    );
    let tr = &t.transforms;
    assert_eq!((tr[0].parent, tr[0].kind, tr[0].id, tr[0].flags), (-1, 0, -1, 0));
    assert_eq!((tr[1].parent, tr[1].kind, tr[1].id, tr[1].local.t), (0, 1, 7, [0.0, 0.0, 75.0]));
    assert_eq!((tr[2].parent, tr[2].kind, tr[2].id), (1, 6, 7));
    assert_eq!((tr[3].parent, tr[3].kind, tr[3].id), (2, 1, 3782));
    assert!((tr[3].local.t[1] + 0.19).abs() < 1e-4 && (tr[3].local.t[2] - 219.511).abs() < 1e-3);
    assert_eq!(t.mirrors[0].source, 10);
    assert_eq!(t.mirrors[0].target, 73);
    // "29 nested objects": the nested solids (the body also holds clips and other objects: 288 children).
    assert_eq!(s.children.iter().filter(|c| matches!(c.body, Body::Solid(_))).count(), 29);
    assert!(
        s.children.iter().filter(|c| matches!(c.body, Body::Solid(_))).all(
            |c| matches!(&c.body, Body::Solid(x) if (x.transforms.transforms[0].parent as u16) & 0xF000 == 0x1000)
        )
    );
    assert_eq!(s.children.iter().filter(|c| c.class == 48).count(), 26);
    assert_eq!(s.children.iter().filter(|c| c.class == 105).count(), 3);
    // geometry.md 10.
    let m = &s.meshes.meshes;
    assert_eq!(m.len(), 76);
    let by = |t| m.iter().filter(|x| x.body_type == t).count();
    assert_eq!((by(RIGID), by(SKINNED), by(STREAMED)), (40, 30, 6));
    assert_eq!(m.iter().filter(|x| !x.conditions.is_empty()).count(), 18);
    assert_eq!(m.iter().map(|x| x.triangle_count(data)).sum::<usize>(), 4_407);
    let skinned = m.iter().find(|x| x.body_type == SKINNED).unwrap();
    assert_eq!(skinned.bones.len(), 24);
    assert_eq!(skinned.bones[0], (7, 39));
    let Group::Outline { vertices, edges, edges_at, pos_at, .. } = &skinned.groups[0] else { panic!("outline group") };
    assert_eq!((*vertices, *edges), (88, 180));
    // Slots and first weights sit between the positions and the edge count: 1 + k + k*n bytes.
    let w_at = *pos_at as usize + 88 * 12;
    assert_eq!(data[w_at], 4);
    assert_eq!(&data[w_at + 1..w_at + 5], &[7, 11, 12, 8]);
    let weights: Vec<u8> = (0..4).map(|k| data[w_at + 5 + k * 88]).collect();
    assert_eq!(weights, [255, 0, 0, 0]);
    let e = *edges_at as usize;
    assert_eq!(&data[e..e + 8], &[0, 1, 2, 3, 1, 2, 0, 4]);
}

fn rgba_hash(w: &World<'_>, pkg: usize, shader: usize) -> (String, String, (u16, u16, u8)) {
    let b = &w.bodies[pkg];
    let s = &b.materials.shaders[shader];
    let (bi, pi) = s.frame(0, 0).unwrap();
    let bmp = &b.materials.bitmaps[bi as usize];
    let pal = &b.materials.palettes[pi.unwrap() as usize];
    let img = textures::decode(bmp, pal, w.data, b.texture_bytes(w.data)).unwrap();
    (hexs(&img.pixels[..8]), hex(&sha256(&img.pixels)), (bmp.width, bmp.height, bmp.format))
}

#[test]
fn cellar_textures() {
    need_disc!();
    let w = parse(CELLAR);
    let n = w.bodies.len() - 1;
    let m = &w.bodies[n].materials;
    assert_eq!((m.palettes.len(), m.bitmaps.len(), m.shaders.len()), (224, 115, 171));
    // textures.md 5: shader, bitmap, palette, first RGBA, hash.
    for (shader, bmp, pal, first, hash) in [
        (0, 5, 49, "40282dff3f3029ff", "283a6fb0a69f53d6926b61c5acf869b1d33b01251734c92ac4ed4f264ad5b67b"),
        (3, 19, 205, "8080800080808000", "b43df195969edc4e9a7715fcbee48a34184f2c8caf4f2ccc6cd60ec14a85884d"),
        (5, 54, 166, "8080800080808000", "27daa1bd2f4a469290b1f93b7ab8cab345fc76decf499e7d4c99b92ee079445e"),
        (24, 61, 131, "766a42ff726338ff", "cf70dc0943ca949aa2173ff865a75b783ba24312752a6b42f05ca8af018fddfa"),
        (68, 29, 167, "807f76ff807f76ff", "08844f180db8bce2edd946f5805adc4946bbac285c7a773fc0faf25c57d368de"),
    ] {
        assert_eq!(m.shaders[shader].frame(0, 0), Some((bmp, Some(pal))), "shader {shader}");
        let (f, h, _) = rgba_hash(&w, n, shader);
        assert_eq!((f.as_str(), h.as_str()), (first, hash), "shader {shader}");
    }
    let s0 = &m.shaders[0];
    assert_eq!((s0.kind, s0.flags, s0.pass, s0.color), (1, 2, 3, [0xFF; 4]));
    let b5 = &m.bitmaps[5];
    assert_eq!((b5.width, b5.height, b5.format, b5.size), (64, 64, textures::FORMAT_4BIT, 2048));
}

#[test]
fn intro_textures_and_placeholder() {
    need_disc!();
    let w = parse(INTRO);
    let n = w.bodies.len() - 1;
    let m = &w.bodies[n].materials;
    assert_eq!((m.palettes.len(), m.bitmaps.len(), m.shaders.len()), (354, 178, 395));
    for (shader, dims, first, hash) in [
        (1, (128, 128, 0x13), "252357ff252357ff", "99b3c1bf62e0eb6443de191e47a5c8ec14f9aec985a6ed1dfad42c521615b39d"),
        (2, (64, 128, 0x14), "3b0407ff3b0407ff", "e51db1bf3364d05bb1a4ace1257f58deea591e099f81b95525736b7c34fb836d"),
        (19, (32, 32, 0x13), "8080800080808000", "13ebd8dbdf9b45f13f668d229acd616892a4c83d6f42cd6ac590e36a4f9dba59"),
        (23, (32, 32, 0x14), "051407ff020502ff", "306c08d5b7faf8d2384f9428e297b40f45ed971c16f0cf0655fd1ea38282d442"),
        (43, (64, 64, 0x14), "8080800080808000", "19de47cf28c8c86219673c73618efe5af7f5bb6330982a79e455bf2c1efe049d"),
    ] {
        let (f, h, d) = rgba_hash(&w, n, shader);
        assert_eq!((d, f.as_str(), h.as_str()), (dims, first, hash), "shader {shader}");
    }
    assert_eq!(m.shaders[43].palettes, [275, 343, 350]);
    // The 8 x 8 placeholder (bitmap 126 here) hashes like the cellar's bitmap 29.
    let pal = m.shaders.iter().find_map(|s| s.frame(0, 0).filter(|(b, _)| *b == 126)).unwrap().1.unwrap();
    let b = &w.bodies[n];
    let img = textures::decode(&m.bitmaps[126], &m.palettes[pal as usize], w.data, b.texture_bytes(w.data)).unwrap();
    assert_eq!(hex(&sha256(&img.pixels)), "08844f180db8bce2edd946f5805adc4946bbac285c7a773fc0faf25c57d368de");
}

#[test]
fn texture_totals_and_first_frames() {
    need_disc!();
    let (mut pal, mut pal3, mut bmp, mut bmp3, mut shaders, mut decoded) = (0, 0, 0, 0, 0, 0);
    let mut sky = false;
    for (row, _) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            let m = &b.materials;
            pal += m.palettes.len();
            pal3 += m.palettes.iter().filter(|p| p.storage == 3).count();
            bmp += m.bitmaps.len();
            bmp3 += m.bitmaps.iter().filter(|p| p.storage == 3).count();
            shaders += m.shaders.len();
            // Storage-1 items end exactly at the texture-data size.
            let end = m
                .palettes
                .iter()
                .filter_map(|p| match p.data {
                    textures::Data::TextureData(o) => Some(o as usize + p.colors as usize * 4),
                    _ => None,
                })
                .chain(m.bitmaps.iter().filter_map(|x| match x.data {
                    textures::Data::TextureData(o) => Some(o as usize + x.size as usize),
                    _ => None,
                }))
                .max()
                .unwrap_or(0);
            assert_eq!(end, b.texture_data.1, "row {row} package at {:#x}", b.offset);
            for s in &m.shaders {
                let (bi, pi) = s.frame(0, 0).unwrap();
                let image = textures::decode(
                    &m.bitmaps[bi as usize],
                    &m.palettes[pi.unwrap() as usize],
                    w.data,
                    b.texture_bytes(w.data),
                );
                assert!(image.is_ok(), "row {row}");
                decoded += 1;
            }
            if *row == INTRO {
                sky |=
                    m.bitmaps.iter().any(|x| x.storage == 3 && x.width == 1024 && x.height == 256 && x.size == 262_144);
            }
        }
    }
    assert_eq!((pal, pal3, bmp, bmp3, shaders, decoded), (71_774, 62, 37_508, 62, 38_750, 38_750));
    assert!(sky, "the intro hub's inline 1024 x 256 sky bitmap");
}

#[test]
fn first_bodies_of_each_layout() {
    need_disc!();
    // classes/paths-and-volumes.md 9, classes/effects-logic-ui.md 5, classes/animation.md 4: the first body of
    // these classes in walk order (package by package, records before their nested ones).
    for (class, row, offset, size) in [
        (5, 0, 141_747, 450),
        (9, 32, 2_146_788, 1_176),
        (10, 0, 750_631, 300),
        (15, 0, 4_596_286, 516),
        (17, 0, 210_169, 1_386),
        (22, 0, 2_678_007, 863),
        (24, 0, 261_880, 463),
        (26, 0, 7_936_364, 595),
        (41, 0, 7_589_952, 72),
        (29, 0, 322_259, 196),
        (31, 0, 198_957, 1_065),
        (33, 0, 146_691, 187),
        (126, 0, 559_213, 9_531),
        (128, 2, 1_240_129, 185_108),
        (132, 0, 569_153, 64),
        (133, 0, 557_882, 69),
        (109, 0, 200_114, 106),
        (111, 0, 262_356, 579),
    ] {
        let w = parse(row);
        let mut first = None;
        for b in &w.bodies {
            visit(&b.objects, &mut |r| {
                if r.class == class && first.is_none() {
                    first = Some((r.body_offset, r.body_size));
                }
            });
        }
        assert_eq!(first, Some((offset, size)), "class {class}");
    }
    let d = world_data(0);
    assert_eq!(&d[200_114..200_120], &[0x55, 0x55, 0x15, 0x3f, 0x01, 0x00]);
    assert_eq!(
        hexs(&d[559_213..559_237]),
        "6304 0f00 0000 6404 01ff ff00 8988 083d 8002 0000 e001 0000".replace(' ', "")
    );
}

#[test]
fn option_table_entry() {
    need_disc!();
    let d = disc();
    // object-stream.md 6: entry 26 at virtual address 0x2ACC90, file offset 0x1ACD10.
    assert_eq!(d.executable.va_to_offset(0x002A_C880 + 26 * 40), Some(0x1ACD10));
    let e = d.options.get(26).unwrap();
    assert!(e.is_typed() && e.arg_count == 0);
    // Option 26 (remove at start, object-stream.md 1.4): 6,167 in object option lists, 338 in placement option
    // blocks. The scope: option lists of solid bodies at every depth (copy extras included) and of top-level
    // simple-layout bodies; option blocks and patches of top-level placements.
    use formats_sly2::stream::{Layout, layout_of};
    let (mut objects, mut placements) = (0usize, 0usize);
    for (row, data) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            visit(&b.objects, &mut |r| {
                if let Body::Solid(s) = &r.body {
                    objects += s.options.options.iter().filter(|x| x.id == 26).count();
                }
            });
            for r in &b.objects {
                if matches!(layout_of(r.class), Some(Layout::Options | Layout::HeaderOptions)) {
                    objects += r.options().unwrap().options.iter().filter(|x| x.id == 26).count();
                }
                if let Body::Placement(_) = &r.body {
                    placements += count_26_in_placement(data, r);
                }
            }
        }
    }
    assert_eq!((objects, placements), (6_167, 338));
}

/// Option 26 occurrences in a placement's option blocks (fresh, kind 5) or patches (copy), as `Placement` keeps them;
/// cross-checked against a direct read of the bytes.
fn count_26_in_placement(data: &[u8], r: &Record) -> usize {
    let Body::Placement(p) = &r.body else { return 0 };
    let n26 = |o: &formats_sly2::options::OptionList| o.options.iter().filter(|x| x.id == 26).count();
    let kept: usize = p.blocks.iter().filter_map(|b| b.options.as_ref()).map(n26).sum::<usize>()
        + p.patches.iter().map(|q| n26(&q.options)).sum::<usize>();
    let t = options();
    let mut rd =
        Reader::at(&data[..(r.body_offset + r.body_size) as usize], r.body_offset as usize + 4 + 1 + 2 + 48 + 4);
    let mut n = 0;
    let blocks = rd.u8().unwrap();
    assert_eq!(blocks as usize, if p.fresh { p.blocks.len() } else { p.patches.len() });
    for _ in 0..blocks {
        let kind = if p.fresh { rd.i32().unwrap() } else { rd.i32().map(|_| 5).unwrap() };
        rd.skip(4).unwrap();
        let size = rd.u16().unwrap() as usize;
        let start = rd.pos();
        if kind == 5 {
            let o = formats_sly2::options::OptionList::read(&mut rd, t).unwrap();
            n += o.options.iter().filter(|x| x.id == 26).count();
        }
        rd.set_pos(start + size);
    }
    assert_eq!(kept, n);
    n
}

#[test]
fn owner_parent_references() {
    need_disc!();
    // transforms.md 1: as a placement parent, 0x2000 occurs 2,656 times: 2,595 in explosion-group lists, 56 in the
    // extra lists of copy placements nested in solids and 5 in those of top-level copy placements; never on a
    // placement outside an owned list.
    // It also occurs outside placements (spec question): on 5,408 transform headers in explosion-group lists (all
    // class 33), 216 headers in copy extras (classes 3, 11, 13, 19, 39), the transform lists of 6 solids in copy
    // extras (class 106), and 215 class-10 curve headers in lists no object owns (202 top-level, 13 nested in solids).
    // Lists: 0 explosion group, 1 extras of a nested copy, 2 extras of a top-level copy, 3 not owned.
    #[derive(Default, Debug, PartialEq)]
    struct Uses {
        placements: [usize; 4],
        headers: [usize; 4],
        solids: [usize; 4],
    }
    fn walk(list: &[Record], owner: usize, nested: bool, u: &mut Uses) {
        for r in list {
            match &r.body {
                Body::Placement(p) => {
                    u.placements[owner] += (p.parent == 0x2000) as usize;
                    walk(&p.extra, if nested { 1 } else { 2 }, nested, u);
                }
                Body::Solid(s) => {
                    u.solids[owner] += s.transforms.transforms.iter().any(|t| t.parent == 0x2000) as usize;
                    walk(&s.children, 3, true, u);
                }
                Body::Other { header, children, .. } => {
                    u.headers[owner] += header.is_some_and(|h| h.parent == 0x2000) as usize;
                    walk(children, if r.class == 31 { 0 } else { owner }, nested, u);
                }
                Body::Unparsed => {}
            }
        }
    }
    let mut u = Uses::default();
    for (row, _) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            walk(&b.objects, 3, false, &mut u);
        }
    }
    assert_eq!(u, Uses { placements: [2_595, 56, 5, 0], headers: [5_408, 16, 200, 215], solids: [0, 3, 3, 0] });
}

#[test]
fn collision_parts() {
    need_disc!();
    // classes/solid-object.md 2: every solid's collision parts, as `formats::collision` meshes and trees. Every tree
    // converts except the 6 level-geometry trees with more than 65,535 nodes, which a 16-bit tree cannot index.
    let (mut parts, mut solids, mut surfaces, mut nodes) = (0usize, 0usize, 0usize, 0usize);
    let mut too_big = Vec::new();
    for (row, _) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            visit(&b.objects, &mut |r| {
                let Body::Solid(s) = &r.body else { return };
                solids += !s.collision.is_empty() as usize;
                for c in &s.collision {
                    parts += 1;
                    surfaces += c.mesh.triangles.len();
                    nodes += c.tree.nodes.len();
                    assert_eq!(c.raw_tree.0, c.raw_tree.1);
                    assert!(c.tree.nodes.len() <= c.raw_tree.0 as usize);
                    if c.tree.nodes.is_empty() && !c.mesh.triangles.is_empty() {
                        assert!(c.raw_tree.0 > 65_535 && r.class == 49, "row {row}: tree did not convert");
                        too_big.push(c.raw_tree.0);
                    }
                    let n = c.tree.nodes.len();
                    for t in &c.tree.nodes {
                        assert!((t.triangle as usize) < c.mesh.triangles.len());
                        assert!([t.negative, t.positive].iter().all(|&k| k == 0xFFFF || (k as usize) < n));
                    }
                    // The bounding radius covers every surface's bounding sphere centre.
                    let r2 = c.mesh.radius * c.mesh.radius * 1.0001 + 1e-3;
                    assert!(c.mesh.triangles.iter().all(|t| t.center.iter().map(|v| v * v).sum::<f32>() <= r2 + 1.0));
                }
            });
        }
    }
    assert_eq!((parts, solids, surfaces), (5_413, 2_928, 500_683));
    assert_eq!(too_big.len(), 6);
    assert_eq!(nodes, 878_394);

    // The cellar's level geometry (solid-object.md 4): its collision lies within the drawn level, and its tree
    // converts.
    let w = parse(CELLAR);
    let geo = &w.level_package().1.objects[0];
    assert_eq!((geo.class, geo.id), (49, 1));
    let Body::Solid(s) = &geo.body else { panic!("solid") };
    let c = &s.collision[0];
    assert_eq!(c.raw_tree.0, 30_659);
    assert!(!c.tree.nodes.is_empty());
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for m in &s.meshes.meshes {
        for g in &m.groups {
            for v in g.vertices(w.data) {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.position[k]);
                    hi[k] = hi[k].max(v.position[k]);
                }
            }
        }
    }
    let inside = c.mesh.positions.iter().filter(|p| (0..3).all(|k| p[k] >= lo[k] - 1.0 && p[k] <= hi[k] + 1.0)).count();
    assert!(inside * 100 >= c.mesh.positions.len() * 99, "{inside} of {}", c.mesh.positions.len());
}

#[test]
fn skin_weights() {
    need_disc!();
    // geometry.md 5: every vertex's weights sum to 255: 330,732 rigid and indexed positions, and every vertex of all
    // 11,418 skinned and streamed groups.
    let (mut positions, mut outline_groups) = (0usize, 0usize);
    for (row, _) in worlds() {
        let w = parse(*row);
        for b in &w.bodies {
            visit(&b.objects, &mut |r| {
                let Body::Solid(s) = &r.body else { return };
                for m in &s.meshes.meshes {
                    for g in &m.groups {
                        let Some(sw) = g.weights(w.data, m.bones.len()) else { continue };
                        let n = sw.weights.len() / sw.per_vertex().max(1);
                        for v in 0..n {
                            assert_eq!(sw.vertex(v).iter().map(|&x| x as u32).sum::<u32>(), 255, "row {row}");
                        }
                        assert!(sw.slots.iter().all(|&k| (k as usize) < m.bones.len()));
                        match g {
                            Group::Outline { .. } => outline_groups += 1,
                            _ => positions += n,
                        }
                    }
                }
            });
        }
    }
    assert_eq!((positions, outline_groups), (330_732, 11_418));
}

#[test]
fn scenes() {
    need_disc!();
    use formats_sly2::scene::{SceneOptions, build};
    for (row, min_instances) in [(CELLAR, 1000usize), (INTRO, 1000), (1, 1), (43, 500)] {
        let w = parse(row);
        let opts = SceneOptions { row: Some(row as i16), ..SceneOptions::default() };
        let b = build(&w, opts);
        let s = &b.scene;
        assert!(s.instances.len() >= min_instances, "row {row}: {} instances", s.instances.len());
        assert!(s.texture_errors.is_empty(), "row {row}");
        for sm in &s.submeshes {
            let idx = &s.indices[sm.first_index as usize..(sm.first_index + sm.index_count) as usize];
            assert!(idx.iter().all(|&i| (i as u32) < sm.vertex_count));
            assert!((sm.base_vertex + sm.vertex_count) as usize <= s.vertices.len());
            assert!((sm.material as usize) < s.materials.len());
        }
        for i in &s.instances {
            assert!((i.mesh as usize) < s.meshes.len());
            let t = i.transform;
            assert!(t.cols.iter().flatten().chain(t.t.iter()).all(|v| v.is_finite()), "row {row}");
        }
        // Every drawn material has its texture.
        let drawn: std::collections::BTreeSet<u16> = s.submeshes.iter().map(|x| x.material).collect();
        let textured = drawn.iter().filter(|&&m| s.materials[m as usize].texture.is_some()).count();
        assert_eq!(textured, drawn.len(), "row {row}");
        eprintln!(
            "row {row}: {} meshes, {} instances, {} triangles drawn, {} textures, {} unresolved placements",
            s.meshes.len(),
            s.instances.len(),
            s.drawn_triangles(),
            s.textures.len(),
            b.unresolved_placements
        );
    }
    // The cellar's level geometry (identity transforms, world space) is drawn once per mesh with an identity
    // transform: its first mesh's instance sits at the identity.
    let w = parse(CELLAR);
    let b = build(&w, SceneOptions { row: Some(CELLAR as i16), ..SceneOptions::default() });
    let lp = w.bodies.last().unwrap();
    let geo = lp.objects[0].header_offset;
    let first = b.scene.instances.iter().find(|i| i.record == geo).unwrap();
    assert_eq!(first.transform, formats::geometry::Affine::IDENTITY);
}

/// Writes a top-down (and a side) orthographic view of a scene as a binary PPM, flat-shaded with each material's
/// average texel colour and a little height shading. For looking at, not for asserting: runs only when
/// `SLYNX_SLY2_RENDER` names an output folder.
#[test]
fn render_overview() {
    need_disc!();
    let Some(dir) = std::env::var_os("SLYNX_SLY2_RENDER") else { return };
    use formats_sly2::scene::{SceneOptions, build};
    for row in [CELLAR, INTRO] {
        let w = parse(row);
        // Without the sky (pass 1), which would cover the level from above.
        let passes = SceneOptions::default().passes & !(1 << 1);
        let s = build(&w, SceneOptions { row: Some(row as i16), passes, ..SceneOptions::default() }).scene;
        let avg: Vec<[f32; 3]> = s
            .materials
            .iter()
            .map(|m| match m.texture {
                Some(t) => {
                    let px = &s.textures[t as usize].image.pixels;
                    let n = (px.len() / 4).max(1) as f32;
                    let mut c = [0f32; 3];
                    for p in px.as_chunks::<4>().0 {
                        for k in 0..3 {
                            c[k] += p[k] as f32;
                        }
                    }
                    c.map(|v| v / n)
                }
                None => [m.color[0] as f32, m.color[1] as f32, m.color[2] as f32],
            })
            .collect();
        for (view, (ax, ay, az)) in [("top", (0usize, 1usize, 2usize)), ("side", (0, 2, 1))] {
            let mut tris = Vec::new();
            for inst in &s.instances {
                let m = &s.meshes[inst.mesh as usize];
                for sm in &s.submeshes[m.submeshes.start as usize..m.submeshes.end as usize] {
                    for t in s.indices[sm.first_index as usize..(sm.first_index + sm.index_count) as usize]
                        .as_chunks::<3>()
                        .0
                    {
                        let p = t.map(|i| {
                            inst.transform.transform_point(s.vertices[(sm.base_vertex + i as u32) as usize].position)
                        });
                        tris.push((p, sm.material));
                    }
                }
            }
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for (p, _) in &tris {
                for v in p {
                    for k in 0..3 {
                        lo[k] = lo[k].min(v[k]);
                        hi[k] = hi[k].max(v[k]);
                    }
                }
            }
            let size = 1024usize;
            let scale = (size as f32 - 1.0) / (hi[ax] - lo[ax]).max(hi[ay] - lo[ay]);
            let mut img = vec![[20u8, 20, 30]; size * size];
            let mut depth = vec![f32::MIN; size * size];
            for (p, mat) in &tris {
                let q = p.map(|v| ((v[ax] - lo[ax]) * scale, (hi[ay] - v[ay]) * scale, v[az]));
                let (x0, x1) = (
                    q.iter().map(|v| v.0).fold(f32::MAX, f32::min).max(0.0) as usize,
                    q.iter().map(|v| v.0).fold(f32::MIN, f32::max).min(size as f32 - 1.0) as usize,
                );
                let (y0, y1) = (
                    q.iter().map(|v| v.1).fold(f32::MAX, f32::min).max(0.0) as usize,
                    q.iter().map(|v| v.1).fold(f32::MIN, f32::max).min(size as f32 - 1.0) as usize,
                );
                let area = (q[1].0 - q[0].0) * (q[2].1 - q[0].1) - (q[2].0 - q[0].0) * (q[1].1 - q[0].1);
                if area.abs() < 1e-6 {
                    continue;
                }
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((q[1].0 - px) * (q[2].1 - py) - (q[2].0 - px) * (q[1].1 - py)) / area;
                        let w1 = ((q[2].0 - px) * (q[0].1 - py) - (q[0].0 - px) * (q[2].1 - py)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let z = w0 * q[0].2 + w1 * q[1].2 + w2 * q[2].2;
                        let i = y * size + x;
                        if z > depth[i] {
                            depth[i] = z;
                            let shade = 0.55 + 0.45 * ((z - lo[az]) / (hi[az] - lo[az]).max(1.0));
                            img[i] = avg[*mat as usize].map(|c| (c * shade).min(255.0) as u8);
                        }
                    }
                }
            }
            let mut ppm = format!(
                "P6
{size} {size}
255
"
            )
            .into_bytes();
            ppm.extend(img.iter().flatten());
            std::fs::write(std::path::Path::new(&dir).join(format!("row{row}_{view}.ppm")), ppm).unwrap();
        }
    }
}

mod anim;
