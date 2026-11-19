//! Tests against the player's own Sly 1 disc image, checking every test vector in `specs/sly1/`.
//!
//! They skip (and pass) when `SLYNX_DISC_SLY1` is not set. The disc is read once, and its 46 worlds are
//! decompressed in parallel, by whichever test gets there first.

mod vectors;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use formats::disc::{AMBIENT_TABLE, Disc, MOVIE_TABLE, SOUND_BANK_TABLE, WorldEntry};
use formats::geometry::{Affine, GeometryStats, MeshSet};
use formats::hash::{crc32, hex, sha256};
use formats::iso;
use formats::scene::{Scene, SceneOptions};
use formats::stream::{Context, Extension, ObjectStream, PlacementOptions, ProxyEntry};
use formats::textures::{MaterialTables, PixelFormat, TexturePayload};
use formats::world::World;
use harness::golden::{Game, require_disc};

struct DiscData {
    volume: iso::VolumeInfo,
    root: Vec<iso::DirEntry>,
    system_cnf: Vec<u8>,
    exe: formats::disc::Executable,
    worlds: Vec<WorldEntry>,
    compressed_sha: Vec<[u8; 32]>,
    data: Vec<Vec<u8>>,
}

fn load(path: std::path::PathBuf) -> DiscData {
    let mut disc = Disc::open_path(&path).expect("open disc");
    let volume = disc.volume.clone();
    let root = iso::read_dir(disc.source(), &volume.root).expect("root directory");
    let cnf = iso::find_root_file(disc.source(), "SYSTEM.CNF").unwrap();
    let system_cnf = iso::read_file(disc.source(), &cnf).unwrap();
    let compressed: Vec<Vec<u8>> = (0..disc.worlds.len()).map(|i| disc.read_world_compressed(i).unwrap()).collect();
    let compressed_sha = compressed.iter().map(|c| sha256(c)).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let mut data: Vec<Vec<u8>> = vec![Vec::new(); compressed.len()];
    std::thread::scope(|s| {
        let chunk = compressed.len().div_ceil(threads);
        for (out, input) in data.chunks_mut(chunk).zip(compressed.chunks(chunk)) {
            s.spawn(move || {
                for (o, i) in out.iter_mut().zip(input) {
                    *o = formats::compress::decompress(i).expect("decompress");
                }
            });
        }
    });
    DiscData {
        volume,
        root,
        system_cnf,
        exe: disc.executable.clone(),
        worlds: disc.worlds.clone(),
        compressed_sha,
        data,
    }
}

fn disc(test: &str) -> Option<&'static DiscData> {
    let path = require_disc(Game::Sly1, test)?;
    static CELL: OnceLock<DiscData> = OnceLock::new();
    Some(CELL.get_or_init(|| load(path)))
}

/// Parses every world, in parallel.
fn all_worlds(d: &'static DiscData) -> &'static [World<'static>] {
    static CELL: OnceLock<Vec<World<'static>>> = OnceLock::new();
    CELL.get_or_init(|| {
        std::thread::scope(|s| {
            let handles: Vec<_> = d
                .data
                .iter()
                .enumerate()
                .map(|(i, w)| s.spawn(move || World::load(w).unwrap_or_else(|e| panic!("world {i}: {e}"))))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        })
    })
}

fn approx(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn approx3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|i| approx(a[i], b[i], tol))
}

// ---------------------------------------------------------------------------------------------------------------
// disc.md

#[test]
fn disc_identity_and_files() {
    let Some(d) = disc("disc_identity_and_files") else { return };
    assert_eq!(d.volume.volume_id, "SLY");
    assert_eq!(d.volume.system_id, "PLAYSTATION");
    assert_eq!(d.volume.creation_date, "2002082412000000");
    let files: Vec<(&str, u32, u32)> =
        d.root.iter().filter(|e| !e.is_dir).map(|e| (e.name.as_str(), e.lba, e.size)).collect();
    assert_eq!(
        files,
        [
            ("SYSTEM.CNF;1", 275, 51),
            ("SCUS_971.98;1", 276, 1_712_328),
            ("IOPRP243.IMG;1", 1113, 253_201),
            ("SIO2MAN.IRX;1", 1237, 6553),
            ("PADMAN.IRX;1", 1241, 44_917),
            ("MCMAN.IRX;1", 1263, 90_805),
            ("MCSERV.IRX;1", 1308, 7353),
            ("LIBSD.IRX;1", 1312, 25_397),
            ("989SND.IRX;1", 1325, 154_600),
        ]
    );
    assert_eq!(d.system_cnf, b"BOOT2=cdrom0:\\SCUS_971.98;1\r\nVER=1.00\r\nVMODE=NTSC\r\n");
    assert_eq!(d.exe.data.len(), 1_712_328);
    assert!(d.exe.is_supported());
    assert_eq!(hex(&sha256(&d.exe.data)), "d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c");
    assert_eq!((d.exe.seg_offset, d.exe.seg_vaddr), (0x1000, 0x0010_0000));
    assert_eq!((d.exe.seg_filesz, d.exe.seg_memsz), (1_553_044, 5_424_772));
    assert_eq!(d.exe.va_to_offset(0x0024_7AF0), Some(0x148AF0));
    // Other descriptor tables (disc.md 4.3): first entries.
    for ((va, n), want) in [
        (SOUND_BANK_TABLE, (0xBD9A3, 0xBBA90, 0x3D)),
        (MOVIE_TABLE, (0xF6A14, 0xAB4004, 0x344)),
        (AMBIENT_TABLE, (0x3AA4B, 0x23A0, 0x23F)),
    ] {
        let t = d.exe.descriptor_table(va, n).unwrap();
        assert_eq!(t.len(), n);
        let f = t[0].unwrap();
        assert_eq!((f.sector, f.size, f.identity), want);
    }
}

#[test]
fn world_table_matches() {
    let Some(d) = disc("world_table_matches") else { return };
    assert_eq!(d.worlds.len(), 46);
    for (w, v) in d.worlds.iter().zip(vectors::WORLDS) {
        let (index, id, name, key, lba, size, ..) = v;
        assert_eq!(w.index, index);
        assert_eq!((w.id, w.name.as_str(), w.key(), w.file.sector, w.file.size), (id, name, key, lba, size), "{index}");
        assert!(matches!(w.task_mask, 0 | 2 | 0xE));
        assert_eq!(w.episode(), id >> 8);
    }
    // Worked example: entry 0's name pointer and task mask.
    let raw = d.exe.bytes_at(0x0024_7AF0, 44).unwrap();
    assert_eq!(u32::from_le_bytes(raw[36..40].try_into().unwrap()), 0x0024_A2D0);
    assert_eq!(d.worlds[0].task_mask, 0);
    // The compressed worlds lie between LBA 768,999 and 875,147 and do not overlap.
    let mut spans: Vec<(u64, u64)> = d
        .worlds
        .iter()
        .map(|w| (w.file.sector as u64, w.file.sector as u64 + (w.file.size as u64).div_ceil(2048)))
        .collect();
    spans.sort();
    assert_eq!(spans[0].0, 768_999);
    assert!(spans.last().unwrap().1 <= 875_148);
    assert!(spans.windows(2).all(|p| p[0].1 <= p[1].0));
}

#[test]
fn decompressed_worlds_match_hashes() {
    let Some(d) = disc("decompressed_worlds_match_hashes") else { return };
    for (data, v) in d.data.iter().zip(vectors::WORLDS) {
        let (index, .., len, sha, crc) = v;
        assert_eq!(data.len(), len, "world {index} size");
        assert_eq!(hex(&sha256(data)), sha, "world {index} SHA-256");
        assert_eq!(crc32(data), crc, "world {index} CRC-32");
    }
    let raw = [
        "d446f62e5784f9a724644465a2042b9deeca81e2fd1eca373677538165025d3c",
        "d061cd1f3a86dfb926503e3a5825dc9adda5afbf35be8f96a40b9e40026ca17c",
        "497541391d6eedbac71609b82b8c933f0df97efc99c963d88aed553481b26e0d",
    ];
    for (i, h) in raw.iter().enumerate() {
        assert_eq!(hex(&d.compressed_sha[i]), *h, "compressed world {i}");
    }
    // world-file.md 2, worked example: the first 24 decompressed bytes of world 0.
    assert_eq!(hex(&d.data[0][..24]), "f0f40500bb76683d000100d3050000010000004400000000");
}

// ---------------------------------------------------------------------------------------------------------------
// world-file.md

#[test]
fn outer_layout_matches() {
    let Some(d) = disc("outer_layout_matches") else { return };
    let worlds = all_worlds(d);
    struct V {
        word0: u32,
        word1: u32,
        names: u32,
        scripts: (usize, usize),
        counts: (u32, u32, u32),
        options: (usize, usize),
        desc: (usize, usize),
        music: Option<(u32, u32, u32)>,
        materials: (usize, usize),
        stream: (usize, usize),
        t: usize,
        size: usize,
        tail: usize,
        objects: (usize, usize),
    }
    let vs = [
        V {
            word0: 0x0005_F4F0,
            word1: 0x3D68_76BB,
            names: 1491,
            scripts: (0xF, 0x41),
            counts: (1, 68, 0),
            options: (0x41, 0x49),
            desc: (0x49, 0x8B),
            music: None,
            materials: (0x8B, 0x14D5),
            stream: (0x14D5, 0x9DFF),
            t: 0x9E00,
            size: 0x6930A,
            tail: 26,
            objects: (36, 39),
        },
        V {
            word0: 0x0022_4410,
            word1: 0x3D68_76B4,
            names: 4641,
            scripts: (0xF, 0x3187),
            counts: (68, 221, 9),
            options: (0x3187, 0x31C8),
            desc: (0x31C8, 0x320A),
            music: Some((0xC8E0E, 0x7A6A0, 0x55)),
            materials: (0x320A, 0xFCBC),
            stream: (0xFCBC, 0x500EEA),
            t: 0x500EF0,
            size: 0x72556C,
            tail: 620,
            objects: (385, 1295),
        },
        V {
            word0: 0x003D_EA60,
            word1: 0x3D68_767A,
            names: 2814,
            scripts: (0xF, 0x41),
            counts: (1, 68, 0),
            options: (0x41, 0x6D),
            desc: (0x6D, 0xAF),
            music: Some((0xD3411, 0x6F7A0, 0x43)),
            materials: (0xAF, 0x8651),
            stream: (0x8651, 0x16C2A6),
            t: 0x16C2B0,
            size: 0x54ADC5,
            tail: 181,
            objects: (94, 557),
        },
    ];
    for (i, v) in vs.iter().enumerate() {
        let w = &worlds[i];
        let l = &w.layout;
        assert_eq!((l.header.texture_size, l.header.timestamp), (v.word0, v.word1), "world {i}");
        assert_eq!((l.debug, l.sound_bank, l.name_count), (0, 1, v.names), "world {i}");
        assert_eq!((l.scripts.range.start, l.scripts.range.end), v.scripts, "world {i}");
        assert_eq!((l.scripts.count, l.scripts.symbols, l.scripts.extra), v.counts, "world {i}");
        assert_eq!((l.world_options.start, l.world_options.end), v.options, "world {i}");
        assert_eq!((l.descriptors.start, l.descriptors.end), v.desc, "world {i}");
        assert_eq!(l.music.map(|m| (m.sector, m.size, m.identity)), v.music, "world {i}");
        assert_eq!((l.materials_range.start, l.materials_range.end), v.materials, "world {i}");
        assert_eq!((w.objects.start, w.objects.end), v.stream, "world {i}");
        assert_eq!(w.texture_base, v.t, "world {i}");
        assert_eq!(w.data.len(), v.size, "world {i}");
        assert_eq!(w.tail_bytes(), v.tail, "world {i}");
        assert_eq!((w.objects.top_level.len(), w.objects.list_loaded()), v.objects, "world {i}");
    }
    // Paris music descriptor words 3 and 4 (world-file.md 6).
    assert_eq!(worlds[1].layout.music.unwrap().unknown, [0x0100_1000, 0x6000_0004]);
    // All 46 worlds: debug flag 0, empty audio descriptor and string, padding under 16, word 0 = texel extent,
    // payload starts at offset 0 of the tables, timestamps on 2002-08-25, sound bank 1 or 1 + episode.
    for (i, w) in worlds.iter().enumerate() {
        let l = &w.layout;
        assert_eq!(l.debug, 0, "world {i}");
        assert_eq!(l.audio, None, "world {i}");
        assert!(l.speakers.is_empty(), "world {i}");
        assert!(w.texture_base - w.objects.end < 16, "world {i}");
        assert_eq!(l.header.texture_size as u64, l.materials.texel_extent(), "world {i}");
        let lowest =
            l.materials.bitmaps.iter().map(|b| b.offset).chain(l.materials.palettes.iter().map(|p| p.offset)).min();
        assert_eq!(lowest, Some(0), "world {i}");
        assert!((26..=747).contains(&w.tail_bytes()), "world {i}: tail {}", w.tail_bytes());
        let day = l.header.timestamp / 86_400;
        assert_eq!(day, 11_924, "world {i}: timestamp day"); // 2002-08-25
        let ep = d.worlds[i].episode() as i16;
        assert!(l.sound_bank == 1 || l.sound_bank == 1 + ep, "world {i}: bank {}", l.sound_bank);
        assert!((1..=154).contains(&l.scripts.count), "world {i}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// textures.md

#[test]
fn material_tables_match() {
    let Some(d) = disc("material_tables_match") else { return };
    let worlds = all_worlds(d);
    let counts = [(63, 61, 44, 2, 4), (905, 390, 858, 16, 4), (768, 335, 469, 4, 5)];
    for (i, c) in counts.iter().enumerate() {
        let m = &worlds[i].layout.materials;
        assert_eq!(
            (m.palettes.len(), m.bitmaps.len(), m.shaders.len(), m.animated_shader_count as usize, m.fonts.len()),
            *c,
            "world {i}"
        );
    }
    let mut kinds = BTreeMap::new();
    for s in &worlds[0].layout.materials.shaders {
        *kinds.entry(s.kind).or_insert(0) += 1;
    }
    assert_eq!(kinds.into_iter().collect::<Vec<_>>(), [(0, 13), (1, 22), (2, 1), (4, 2), (7, 5), (9, 1)]);

    // Surveys over all 46 worlds.
    let mut formats = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut anim_kinds = BTreeMap::new();
    let (mut dangling_b, mut dangling_p) = (0, 0);
    let (mut distinct_b, mut distinct_p, mut dangling_shaders) = (0, 0, 0);
    for (i, w) in worlds.iter().enumerate() {
        let m = &w.layout.materials;
        let mut db = std::collections::BTreeSet::new();
        let mut dp = std::collections::BTreeSet::new();
        for b in &m.bitmaps {
            *formats.entry(b.format_code).or_insert(0) += 1;
            let f = b.format().expect("known format");
            assert_eq!(b.size as usize, b.width as usize * b.height as usize * f.bits() / 8);
            if f == PixelFormat::Rgba32 {
                assert_eq!((b.width, b.height), (64, 64));
            }
        }
        for p in &m.palettes {
            assert!((1..=256).contains(&p.count));
        }
        let animated = m.shaders.iter().filter(|s| s.animation.is_animated()).count();
        assert_eq!(animated, m.animated_shader_count as usize, "world {i}");
        for s in &m.shaders {
            *kinds.entry(s.kind).or_insert(0) += 1;
            if s.animation.is_animated() {
                *anim_kinds.entry(s.animation.kind).or_insert(0) += 1;
            }
            assert_eq!(s.textures.len(), 1);
            let t = &s.textures[0];
            assert_eq!(t.palettes.len(), t.bitmaps.len() * s.palettes_per_frame());
            let bad_b: Vec<u16> = t.bitmaps.iter().copied().filter(|&b| b as usize >= m.bitmaps.len()).collect();
            let bad_p: Vec<u16> =
                t.palettes.iter().copied().filter(|&p| p != 0xFFFF && p as usize >= m.palettes.len()).collect();
            dangling_b += bad_b.len();
            dangling_p += bad_p.len();
            dangling_shaders += (!bad_b.is_empty() || !bad_p.is_empty()) as usize;
            db.extend(bad_b);
            dp.extend(bad_p);
        }
        distinct_b += db.len();
        distinct_p += dp.len();
    }
    assert_eq!(formats.into_iter().collect::<Vec<_>>(), [(0x00, 41), (0x13, 2570), (0x14, 11_003)]);
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        [(0, 17_795), (1, 4571), (2, 88), (3, 3), (4, 483), (5, 41), (6, 79), (7, 230), (8, 1056), (9, 46), (10, 4)]
    );
    assert_eq!(anim_kinds.into_iter().collect::<Vec<_>>(), [(1, 427), (2, 56), (3, 22), (5, 1), (6, 156), (7, 2)]);
    // textures.md 2.5: index entries at or past the end of their table, distinct values per world, and shaders.
    assert_eq!((dangling_b, dangling_p), (505, 1900));
    assert_eq!((distinct_b, distinct_p, dangling_shaders), (373, 1118, 646));
}

struct ImageVector {
    world: usize,
    bitmap: usize,
    palette: Option<usize>,
    entry: (u16, u16, u32, u8, u8, u16, u32, u32),
    pal: Option<(u32, u16, u16, u32)>,
    first_index: &'static str,
    first_palette: &'static str,
    first_rgba: &'static str,
    sha: &'static str,
}

#[test]
fn decoded_images_match() {
    let Some(d) = disc("decoded_images_match") else { return };
    let worlds = all_worlds(d);
    let z = 0x1000_0000;
    let vs = [
        ImageVector {
            world: 0,
            bitmap: 45,
            palette: Some(62),
            entry: (8, 8, z, 0x14, 2, 1, 32, 389_872),
            pal: Some((z, 1, 1, 8640)),
            first_index: "00000000",
            first_palette: "ffffff80",
            first_rgba: "ffffffffffffffff",
            sha: "3d6876a0146de8576eb2395a858de1213d1b92c65b779df3a331cfd5a4584546",
        },
        ImageVector {
            world: 0,
            bitmap: 16,
            palette: Some(3),
            entry: (32, 32, z, 0x13, 2, 4, 1024, 374_224),
            pal: Some((z, 128, 2, 3072)),
            first_index: "7f7f7f7f",
            first_palette: "0000008040a8ff80",
            first_rgba: "ffffff00ffffff00",
            sha: "7c1adc669cd859a0b2033e34fffccc16a386e79a3710636d3bb8d8d7f1b69ef8",
        },
        ImageVector {
            world: 1,
            bitmap: 7,
            palette: Some(1),
            entry: (256, 128, z, 0x13, 4, 128, 32_768, 701_968),
            pal: Some((z, 256, 4, 1024)),
            first_index: "ffffffff",
            first_palette: "819bdd045d81b180",
            first_rgba: "0000000000000000",
            sha: "200b26ff045819a9aba35abe7c05082274661d557c9f4292c9c17a1cc1f4e4d0",
        },
        ImageVector {
            world: 1,
            bitmap: 149,
            palette: Some(215),
            entry: (64, 64, z, 0x14, 2, 8, 2048, 2_046_480),
            pal: Some((z, 16, 1, 166_528)),
            first_index: "ffffffff",
            first_palette: "2f2f2f188f8f8f48",
            first_rgba: "ffffff00ffffff00",
            sha: "1daa7946ff8000c2195791564e951589ac14aa8eadef5de69dda6697efef5378",
        },
        ImageVector {
            world: 1,
            bitmap: 160,
            palette: Some(418),
            entry: (64, 64, z, 0x14, 2, 8, 2048, 2_069_008),
            pal: Some((z, 16, 1, 179_520)),
            first_index: "20404424",
            first_palette: "2e23438045346380",
            first_rgba: "2e2343ff3c2d56ff",
            sha: "51d74b33d94027ab2d6a711d8264955096fe14861d51132d318cfe56910f6586",
        },
        ImageVector {
            world: 1,
            bitmap: 160,
            palette: Some(419),
            entry: (64, 64, z, 0x14, 2, 8, 2048, 2_069_008),
            pal: Some((z, 16, 1, 179_584)),
            first_index: "20404424",
            first_palette: "244e0380386e0e80",
            first_rgba: "244e03ff355b0bff",
            sha: "1a95e6d55f64e547f977cd1106f5c59e73782f5d4d635de36f41a6205a4cbfdd",
        },
        ImageVector {
            world: 1,
            bitmap: 160,
            palette: Some(420),
            entry: (64, 64, z, 0x14, 2, 8, 2048, 2_069_008),
            pal: Some((z, 16, 1, 179_648)),
            first_index: "20404424",
            first_palette: "292929803e3e3e80",
            first_rgba: "292929ff333333ff",
            sha: "08f947812ca44808111ea5f80edc11a339b391d9616e72a5ff0486dfcd914394",
        },
        ImageVector {
            world: 1,
            bitmap: 27,
            palette: None,
            entry: (64, 64, z, 0x00, 1, 64, 16_384, 1_226_256),
            pal: None,
            first_index: "000000ff000000ff",
            first_palette: "",
            first_rgba: "000000ff000000ff",
            sha: "62fb561c59d0cea247fc588f3311ee665375f35d8675b186e2792cb7dfcff88c",
        },
    ];
    for v in &vs {
        let w = &worlds[v.world];
        let m = &w.layout.materials;
        let p = w.payload();
        let b = &m.bitmaps[v.bitmap];
        let tag = format!("world {} bitmap {} palette {:?}", v.world, v.bitmap, v.palette);
        assert_eq!(
            (b.width, b.height, b.zone_mask, b.format_code, b.gs_width, b.gs_blocks, b.size, b.offset),
            v.entry,
            "{tag}"
        );
        let texels = p.texels(b).unwrap();
        assert_eq!(hex(&texels[..v.first_index.len() / 2]), v.first_index, "{tag}");
        let pal = v.palette.map(|i| &m.palettes[i]);
        if let (Some(pl), Some(want)) = (pal, v.pal) {
            assert_eq!((pl.zone_mask, pl.count, pl.size_class, pl.offset), want, "{tag}");
            let fp = v.first_palette.replace(' ', "");
            assert_eq!(hex(&p.colors(pl).unwrap()[..fp.len() / 2]), fp, "{tag}");
        }
        let img = p.decode(b, pal).unwrap();
        assert_eq!(hex(&img.pixels[..8]), v.first_rgba, "{tag}");
        assert_eq!(hex(&sha256(&img.pixels)), v.sha, "{tag}");
    }
    let paris = &worlds[1];
    let m = &paris.layout.materials;
    let p = paris.payload();
    let texel = |img: &formats::textures::RgbaImage, x: usize, y: usize| {
        let o = (y * img.width as usize + x) * 4;
        hex(&img.pixels[o..o + 4])
    };
    let logo = p.decode(&m.bitmaps[7], Some(&m.palettes[1])).unwrap();
    assert_eq!(texel(&logo, 128, 64), "010101ff");
    let img = p.decode(&m.bitmaps[149], Some(&m.palettes[215])).unwrap();
    assert_eq!(texel(&img, 32, 32), "14141414");
    // Shader bindings named in the notes.
    let s120 = &m.shaders[120];
    assert_eq!(s120.kind, 0);
    assert_eq!(
        (s120.textures[0].bitmaps.as_slice(), s120.textures[0].palettes.as_slice()),
        (&[160u16][..], &[418u16, 419, 420][..])
    );
    assert_eq!(m.shader_texture(120), Some((160, Some(419))));
    assert_eq!(m.shaders[41].kind, 1);
    assert_eq!(m.shader_texture(41), Some((7, Some(1))));
    assert_eq!(m.shader_texture(15), Some((149, Some(215))));
}

#[test]
fn every_referenced_texture_decodes() {
    let Some(d) = disc("every_referenced_texture_decodes") else { return };
    let worlds = all_worlds(d);
    let mut pairs = 0usize;
    let mut distinct = 0usize;
    let mut max_alpha = 0u8;
    for (i, w) in worlds.iter().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        let m: &MaterialTables = &w.layout.materials;
        let p: TexturePayload<'_> = w.payload();
        for pl in &m.palettes {
            for c in p.colors(pl).unwrap().as_chunks::<4>().0 {
                max_alpha = max_alpha.max(c[3]);
            }
        }
        for s in &m.shaders {
            let t = &s.textures[0];
            for (f, &b) in t.bitmaps.iter().enumerate() {
                if b as usize >= m.bitmaps.len() {
                    continue;
                }
                for slot in 0..s.palettes_per_frame() {
                    let pi = t.palettes[f * s.palettes_per_frame() + slot];
                    let pal = if pi == 0xFFFF {
                        None
                    } else if (pi as usize) < m.palettes.len() {
                        Some(&m.palettes[pi as usize])
                    } else {
                        continue;
                    };
                    p.decode(&m.bitmaps[b as usize], pal)
                        .unwrap_or_else(|e| panic!("world {i} bitmap {b} palette {pi}: {e}"));
                    pairs += 1;
                    seen.insert((b, pi));
                }
            }
        }
        distinct += seen.len();
    }
    assert_eq!(max_alpha, 0x80);
    // textures.md 2.5, pair counts: 66,119 decodable pairs, 29,070 distinct within their world.
    assert_eq!((pairs, distinct), (66_119, 29_070));
}

// ---------------------------------------------------------------------------------------------------------------
// geometry.md

fn mesh_sets<'a>(w: &'a World<'_>) -> impl Iterator<Item = &'a MeshSet> + 'a {
    w.objects.records.iter().filter_map(|r| r.mesh_set.as_ref())
}

#[test]
fn geometry_totals_match() {
    let Some(d) = disc("geometry_totals_match") else { return };
    let worlds = all_worlds(d);
    let want = [
        GeometryStats {
            mesh_sets: 29,
            nonempty_sets: 28,
            meshes: 33,
            instances: 0,
            submeshes: 41,
            strip_entries: 1382,
            positions: 605,
            triangles: 764,
            outline_parts: 1,
        },
        GeometryStats {
            mesh_sets: 727,
            nonempty_sets: 327,
            meshes: 2464,
            instances: 1464,
            submeshes: 2786,
            strip_entries: 150_125,
            positions: 80_525,
            triangles: 91_763,
            outline_parts: 193,
        },
        GeometryStats {
            mesh_sets: 382,
            nonempty_sets: 290,
            meshes: 537,
            instances: 164,
            submeshes: 1006,
            strip_entries: 51_537,
            positions: 27_550,
            triangles: 33_777,
            outline_parts: 70,
        },
    ];
    for (i, want) in want.iter().enumerate() {
        let w = &worlds[i];
        let mut g = GeometryStats::default();
        for s in mesh_sets(w) {
            g.add(w.data, s);
        }
        assert_eq!(&g, want, "world {i}");
    }
    // Surveys over all 46 worlds.
    let (mut submeshes, mut instances, mut mirrored, mut bb0, mut bb1) = (0, 0, 0, 0, 0);
    for (i, w) in worlds.iter().enumerate() {
        let shaders = &w.layout.materials;
        for s in mesh_sets(w) {
            for m in &s.meshes {
                match m.billboard {
                    0 => bb0 += 1,
                    1 => bb1 += 1,
                    b => panic!("world {i}: billboard {b}"),
                }
                if let Some(inst) = m.instance {
                    instances += 1;
                    mirrored += inst.matrix.is_mirrored() as usize;
                }
                for sm in &m.submeshes {
                    submeshes += 1;
                    assert!((sm.shader as usize) < shaders.shaders.len(), "world {i}");
                    assert!(shaders.shader_texture(sm.shader as usize).is_some(), "world {i}: shader {}", sm.shader);
                }
            }
        }
    }
    assert_eq!(submeshes, 68_242);
    assert_eq!(instances, 36_578);
    assert_eq!(mirrored, 40);
    assert_eq!((bb0, bb1), (61_464, 3982));
}

#[test]
fn first_submeshes_match() {
    let Some(d) = disc("first_submeshes_match") else { return };
    let worlds = all_worlds(d);
    struct V {
        set: usize,
        owner: (i16, i16),
        header: (u8, u8, usize, usize, usize),
        mesh: (usize, u16),
        centre: [f32; 3],
        radius: f32,
        bpf: (u8, u8, u8),
        sub: usize,
        counts: (u8, u8, u8, u8, u8, u8),
        pos_bytes: &'static str,
        pos: [f32; 3],
        normal: [f32; 3],
        color: Option<[u8; 4]>,
        uv: [f32; 2],
        strip: &'static str,
        tris: [[u16; 3]; 4],
        ntris: usize,
        shader: (u16, u8, u16, [u16; 3]),
        sub_end: usize,
        set_end: usize,
    }
    let vs = [
        V {
            set: 0x1623,
            owner: (87, 1356),
            header: (0, 1, 0, 0, 6),
            mesh: (0x1629, 0x000),
            centre: [-5.5996, 15.4781, 27.2721],
            radius: 70.6037,
            bpf: (0, 7, 0x10),
            sub: 0x1642,
            counts: (74, 60, 5, 67, 148, 1),
            pos_bytes: "b2ae0a42545267c091eda240",
            pos: [34.6706, -3.6144, 5.0915],
            normal: [0.737117, -0.362808, 0.570113],
            color: Some([0xFF; 4]),
            uv: [0.4074, 0.3062],
            strip: "000000800101018002020200030303000404048105050580",
            tris: [[0, 1, 2], [1, 3, 2], [4, 5, 6], [5, 7, 6]],
            ntris: 98,
            shader: (34, 1, 4, [15, 0, 0]),
            sub_end: 0x2120,
            set_end: 0x3D9E,
        },
        V {
            set: 0xA2BCD,
            owner: (4, 1),
            header: (0, 0, 0, 0, 1910),
            mesh: (0xA2BD3, 0x002),
            centre: [1298.4694, 17_678.611, -4424.877],
            radius: 6_180.602,
            bpf: (0, 3, 0x13),
            sub: 0xA2BF0,
            counts: (4, 1, 0, 4, 4, 3),
            pos_bytes: "174285457e344e4604478ac5",
            pos: [4_264.261, 13_197.123, -4_424.877],
            normal: [0.0, 0.0, 1.0],
            color: None,
            uv: [1.0, 1.0],
            strip: "000000ff010001ff0200027f0300037f",
            tris: [[0, 1, 2], [1, 3, 2], [0; 3], [0; 3]],
            ntris: 2,
            shader: (120, 0, 160, [418, 419, 420]),
            sub_end: 0xA2C78,
            set_end: 0x24EF23,
        },
        V {
            set: 0x1E531,
            owner: (4, 1),
            header: (0, 0, 0, 0, 68),
            mesh: (0x1E537, 0x002),
            centre: [-628.347, 6.3999, -100.5143],
            radius: 437.0908,
            bpf: (0, 3, 0x13),
            sub: 0x1E554,
            counts: (51, 18, 0, 52, 85, 3),
            pos_bytes: "3a1b5cc4e59b35c3b176ad42",
            pos: [-880.4254, -181.609, 86.7318],
            normal: [-0.999972, -0.004500, 0.006000],
            color: None,
            uv: [1.0, 0.8333],
            strip: "000000ff010001ff0200027f0300037f0400047f0500057f",
            tris: [[0, 1, 2], [1, 3, 2], [2, 3, 4], [3, 5, 4]],
            ntris: 58,
            shader: (53, 0, 220, [507, 508, 509]),
            sub_end: 0x1EBA0,
            set_end: 0x60D1B,
        },
    ];
    for (i, v) in vs.iter().enumerate() {
        let w = &worlds[i];
        let data = w.data;
        let s = &w.objects;
        // The first mesh set with a submesh, in stream order, and its owner (a top-level object).
        let (ri, rec) = s
            .records
            .iter()
            .enumerate()
            .find(|(_, r)| r.mesh_set.as_ref().is_some_and(|m| m.meshes.iter().any(|m| !m.submeshes.is_empty())))
            .unwrap();
        let set = rec.mesh_set.as_ref().unwrap();
        assert_eq!(set.offset, v.set, "world {i}");
        assert_eq!((rec.class, rec.oid), v.owner, "world {i}");
        let _ = ri;
        // The owner's stored transform is the identity (geometry.md 7.2).
        assert_eq!(rec.local, Some(Affine::IDENTITY), "world {i}");
        assert_eq!(
            (set.relight, set.unknown, set.bones.len(), set.pose_weights.len(), set.meshes.len()),
            v.header,
            "world {i}"
        );
        let m = &set.meshes[0];
        assert_eq!((m.offset, m.flags), v.mesh, "world {i}");
        assert!(approx3(m.center, v.centre, 1e-3) && approx(m.radius, v.radius, 1e-3), "world {i}");
        assert_eq!((m.billboard, m.pass, m.mesh_flags), v.bpf, "world {i}");
        let sm = &m.submeshes[0];
        assert_eq!(sm.offset, v.sub, "world {i}");
        assert_eq!((sm.np, sm.nn, sm.nc, sm.nt, sm.ns, sm.padding), v.counts, "world {i}");
        assert_eq!(hex(&data[sm.data..sm.data + 12]), v.pos_bytes, "world {i}");
        assert!(approx3(sm.position(data, 0), v.pos, 1e-3), "world {i}");
        assert!(approx3(sm.normal(data, 0), v.normal, 1e-6), "world {i}");
        assert_eq!((sm.nc > 0).then(|| sm.color(data, 0)), v.color, "world {i}");
        let uv = sm.uv(data, 0);
        assert!(approx(uv[0], v.uv[0], 1e-4) && approx(uv[1], v.uv[1], 1e-4), "world {i}");
        assert_eq!(hex(&sm.strip(data)[..v.strip.len() / 2]), v.strip, "world {i}");
        let tris = sm.triangles(data);
        let n = if i == 1 { 2 } else { 4 };
        assert_eq!(&tris[..n], &v.tris[..n], "world {i}");
        assert_eq!(tris.len(), v.ntris, "world {i}");
        let (si, kind, bitmap, pals) = v.shader;
        assert_eq!(sm.shader, si, "world {i}");
        let sh = &w.layout.materials.shaders[si as usize];
        assert_eq!(sh.kind, kind, "world {i}");
        assert_eq!(sh.textures[0].bitmaps, [bitmap], "world {i}");
        let np = sh.palettes_per_frame();
        assert_eq!(sh.textures[0].palettes, pals[..np], "world {i}");
        assert_eq!(sm.end, v.sub_end, "world {i}");
        assert_eq!(set.end, v.set_end, "world {i}");
        let gpu = sm.to_gpu(data);
        assert_eq!(gpu.vertices.len(), v.counts.4 as usize);
        assert_eq!(gpu.indices.len(), v.ntris * 3);
    }
    // geometry.md 6.3: the first submesh of Paris lies in the plane z = -4424.877, normal (0, 0, 1).
    let w = &worlds[1];
    let sm =
        &w.objects.records.iter().find_map(|r| r.mesh_set.as_ref().filter(|m| m.offset == 0xA2BCD)).unwrap().meshes[0]
            .submeshes[0];
    for k in 0..4 {
        assert!(approx(sm.position(w.data, k)[2], -4424.877, 1e-3));
    }
}

#[test]
fn instance_example_matches() {
    let Some(d) = disc("instance_example_matches") else { return };
    let w = &all_worlds(d)[2];
    let set = mesh_sets(w).find(|s| s.offset == 0x1E531).unwrap();
    let (i, m) = set.meshes.iter().enumerate().find(|(_, m)| m.instance.is_some()).unwrap();
    assert_eq!((m.offset, m.flags), (0x5D2BE, 0x003));
    let inst = m.instance.unwrap();
    assert_eq!(inst.source, 11);
    assert!(i > 11);
    for (c, want) in inst.matrix.cols.iter().zip([[-1., 0., 0.], [0., -1., 0.], [0., 0., 1.]]) {
        assert!(approx3(*c, want, 1e-6), "{c:?}");
    }
    assert!(approx3(inst.matrix.t, [512.015, 3.3878, 6.457], 1e-3));
    let src = &set.meshes[11];
    assert!(approx3(src.center, [256.007, -162.353, 170.941], 1e-3));
    assert!(approx3(inst.matrix.transform_point(src.center), [256.008, 165.741, 177.398], 2e-3));
    assert!(approx3(m.center, [256.008, 165.741, 177.398], 1e-3));
    assert!(approx(m.radius, 142.362, 1e-3) && approx(src.radius, 142.362, 1e-3));
}

// ---------------------------------------------------------------------------------------------------------------
// worlds.md and object-stream.md

#[test]
fn walks_match_all_worlds() {
    let Some(d) = disc("walks_match_all_worlds") else { return };
    let worlds = all_worlds(d);
    let mut failures = Vec::new();
    let mut clones_total = 0;
    for (w, v) in worlds.iter().zip(vectors::WALKS) {
        let (i, start, end, want) = v;
        let s = &w.objects;
        let got = [
            s.records.len() as u64,
            s.list_loaded() as u64,
            s.top_level.len() as u64,
            s.max_depth() as u64,
            s.drawable() as u64,
            s.drawable_live() as u64,
            s.sources.len() as u64,
            s.back_references() as u64,
            s.drawable_in_clones() as u64,
            s.counts.options + w.layout.world_options.count as u64,
        ];
        assert_eq!((s.start, s.end), (start, end), "world {i}");
        assert_eq!(w.texture_base, end.div_ceil(16) * 16, "world {i}");
        clones_total += got[8];
        if got != want {
            failures.push(format!("world {i}: got {got:?}, want {want:?}"));
        }
        assert_eq!(s.stock_templates(), 31, "world {i}");
        let template_drawables = s.records.iter().filter(|r| r.template && r.is_drawable()).count();
        assert_eq!(template_drawables, 27, "world {i}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // worlds.md 5: 14,805 drawable records in clones over the disc, nested back-reference clones included, the
    // child lists of the sources' own proxies left out (classes/proxy.md 3); Back Alley Heist gives 384.
    assert_eq!(clones_total, 14_805);
    assert_eq!(worlds[19].objects.drawable_in_clones(), 384);
}

#[test]
fn walk_details_match() {
    let Some(d) = disc("walk_details_match") else { return };
    let worlds = all_worlds(d);
    let table = formats::options::OptionTable::sly1();
    for (i, counts, object_options, world_ids, wm, first) in vectors::DETAILS {
        let w = &worlds[i];
        let s = &w.objects;
        let ctx = |c: Context| s.records.iter().filter(|r| r.context == c).count() as u64;
        let got = [
            s.records.len() as u64,
            s.list_loaded() as u64,
            s.sources.len() as u64,
            ctx(Context::StateMachine),
            ctx(Context::GroupMember),
        ];
        assert_eq!(got, counts, "world {i}");
        assert_eq!(s.counts.options, object_options, "world {i}");
        let ids: Vec<i16> = w.layout.world_options.iter(w.data, &table).map(|e| e.unwrap().0).collect();
        assert_eq!(ids, world_ids, "world {i}");
        let rec = s.records.iter().find(|r| r.class == 4).unwrap();
        let set = rec.mesh_set.as_ref().unwrap();
        let mut g = GeometryStats::default();
        g.add(w.data, set);
        assert_eq!(
            (rec.offset, g.meshes, g.instances, g.submeshes, g.strip_entries, g.positions),
            wm,
            "world {i} world mesh"
        );
        assert_eq!((rec.oid, rec.script, rec.children_count(s)), (1, -1, 0), "world {i} world mesh");
        let got: Vec<_> = s.records[..4].iter().map(|r| (r.offset, r.class, r.oid, r.script)).collect();
        assert_eq!(got, first, "world {i}");
    }
    for (i, hist) in vectors::HISTOGRAMS {
        let mut got = BTreeMap::new();
        for r in &worlds[i].objects.records {
            *got.entry(r.class).or_insert(0usize) += 1;
        }
        let want: BTreeMap<i16, usize> = hist.iter().copied().collect();
        assert_eq!(got, want, "world {i} class histogram");
    }
}

trait ChildCount {
    fn children_count(&self, s: &ObjectStream) -> usize;
}

impl ChildCount for formats::stream::Record {
    fn children_count(&self, s: &ObjectStream) -> usize {
        let me = s.records.iter().position(|r| std::ptr::eq(r, self)).unwrap() as u32;
        s.records.iter().filter(|r| r.parent == Some(me) && r.context == Context::Child).count()
    }
}

#[test]
fn disc_wide_object_surveys() {
    let Some(d) = disc("disc_wide_object_surveys") else { return };
    let worlds = all_worlds(d);
    let mut per_class: BTreeMap<i16, (u64, u64)> = BTreeMap::new();
    let (mut records, mut listed, mut sources, mut sm, mut members) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut degenerate, mut bit3, mut bit2, mut script_none) = (0, 0, 0, 0);
    let mut use_count_exceptions = Vec::new();
    for (i, w) in worlds.iter().enumerate() {
        let s = &w.objects;
        let mut seen_here = std::collections::BTreeSet::new();
        for r in &s.records {
            let e = per_class.entry(r.class).or_default();
            e.0 += 1;
            if seen_here.insert(r.class) {
                e.1 += 1;
            }
            records += 1;
            listed += r.context.is_list_loaded() as u64;
            sm += (r.context == Context::StateMachine) as u64;
            members += (r.context == Context::GroupMember) as u64;
            script_none += (r.script == -1) as usize;
            assert!(r.script >= -1 && (r.script as i64) < w.layout.scripts.count as i64, "world {i}");
            assert!((1..=4960).contains(&r.oid), "world {i}: oid {}", r.oid);
            if r.has_degenerate_matrix() {
                degenerate += 1;
                assert!(!r.is_drawable(), "world {i}");
            }
            if let Some(ext) = r.extension {
                bit3 += (ext.flags & Extension::ROTATION_FROM_BASIS != 0) as usize;
                bit2 += (ext.flags & Extension::POSITION_FROM_BASIS != 0) as usize;
                if let Some(b) = ext.basis {
                    let me = s.records.iter().position(|x| std::ptr::eq(x, r)).unwrap() as u32;
                    assert!(s.find_ancestor(me, b).is_some(), "world {i}: basis {b} not an ancestor");
                }
            }
            if r.class == 4 {
                assert_eq!(r.local, Some(Affine::IDENTITY), "world {i}: world mesh transform");
            }
        }
        sources += s.sources.len() as u64;
        // Use counts: 1 + later back-references, with one exception on the disc.
        let mut backrefs = vec![0i16; s.sources.len()];
        for p in &s.proxies {
            for e in &p.entries {
                if let ProxyEntry::BackRef { source_index } = e {
                    backrefs[*source_index as usize] += 1;
                }
            }
        }
        for p in &s.proxies {
            for e in &p.entries {
                if let ProxyEntry::Source { source_index, use_count, .. } = e
                    && *use_count != 1 + backrefs[*source_index as usize]
                {
                    use_count_exceptions.push((i, *use_count, backrefs[*source_index as usize]));
                }
            }
        }
    }
    assert_eq!((records, listed, sources, sm, members), (54_059, 45_093, 2889, 5398, 679));
    assert_eq!(script_none, 51_984);
    assert_eq!(per_class.len(), 126);
    let want: BTreeMap<i16, (u64, u64)> = vectors::CLASS_SEEN.iter().map(|&(c, n, w)| (c, (n, w))).collect();
    assert_eq!(per_class, want);
    assert_eq!(degenerate, 48);
    assert_eq!((bit3, bit2), (50, 0));
    assert_eq!(use_count_exceptions, [(34, 2, 0)]);
}

// ---------------------------------------------------------------------------------------------------------------
// transforms.md

fn record_at(s: &ObjectStream, offset: usize) -> usize {
    s.records.iter().position(|r| r.offset == offset).unwrap_or_else(|| panic!("no record at {offset:#x}"))
}

#[test]
fn transform_vectors_match() {
    let Some(d) = disc("transform_vectors_match") else { return };
    let worlds = all_worlds(d);
    type Row = (usize, i16, i16, [[f32; 3]; 3], [f32; 3], [[f32; 3]; 3], [f32; 3]);
    let paris: [Row; 3] = [
        (
            0x2A6F11,
            83,
            1392,
            [[-0.525311, 0.85091, 0.], [-0.85091, -0.525311, 0.], [0., 0., 1.]],
            [1380.6582, 176.8276, -2388.4368],
            [[-0.525311, 0.85091, 0.], [-0.85091, -0.525311, 0.], [0., 0., 1.]],
            [1380.658, 176.828, -2388.437],
        ),
        (
            0x2A6F48,
            33,
            1637,
            Affine::IDENTITY.cols,
            [0., 0., -20.1402],
            [[-0.52531, 0.85091, 0.], [-0.85091, -0.52531, 0.], [0., 0., 1.]],
            [1380.658, 176.828, -2408.577],
        ),
        (
            0x2A9BF1,
            39,
            1640,
            [[-0.164567, 0.986366, 0.], [-0.986366, -0.164567, 0.], [0., 0., 1.]],
            [0., 0., -14.589],
            [[-0.75286, -0.65818, 0.], [0.65818, -0.75286, 0.], [0., 0., 1.]],
            [1380.658, 176.828, -2423.166],
        ),
    ];
    let mugg: [Row; 3] = [
        (
            0x3140F4,
            3,
            1399,
            [[-0.318368, -0.886405, 0.336048], [0.941137, -0.338026, 0.], [0.113593, 0.316267, 0.941845]],
            [-1886.7853, -2937.6765, 1050.5444],
            [[-0.318368, -0.886405, 0.336048], [0.941137, -0.338026, 0.], [0.113593, 0.316267, 0.941845]],
            [-1886.785, -2937.677, 1050.544],
        ),
        (
            0x323097,
            83,
            1401,
            [[0.867224, 0.497918, -0.000345], [-0.497918, 0.867223, -0.001285], [-0.00034, 0.001286, 0.999999]],
            [-194.9142, -89.0327, -572.6522],
            [[0.19247, -0.93713, 0.2911], [0.97455, 0.14781, -0.16853], [0.11491, 0.31613, 0.94173]],
            [-1973.572, -2915.919, 445.694],
        ),
        (
            0x3230CE,
            33,
            1402,
            Affine::IDENTITY.cols,
            [0., 0.0027, 47.0392],
            [[0.19247, -0.93713, 0.2911], [0.97455, 0.14781, -0.16853], [0.11491, 0.31613, 0.94173]],
            [-1968.164, -2901.048, 489.992],
        ),
    ];
    for (wi, rows) in [(1usize, &paris), (13, &mugg)] {
        let s = &worlds[wi].objects;
        let world = s.world_transforms(true);
        for &(off, class, oid, lm, lp, wm, wp) in rows {
            let i = record_at(s, off);
            let r = &s.records[i];
            assert_eq!((r.class, r.oid), (class, oid), "world {wi} {off:#x}");
            let l = r.local.unwrap();
            for c in 0..3 {
                assert!(approx3(l.cols[c], lm[c], 2e-6), "world {wi} {off:#x} local col {c}");
                assert!(
                    approx3(world[i].cols[c], wm[c], 2e-5),
                    "world {wi} {off:#x} world col {c}: {:?}",
                    world[i].cols[c]
                );
            }
            assert!(approx3(l.t, lp, 1e-3), "world {wi} {off:#x} local p");
            assert!(approx3(world[i].t, wp, 2e-3), "world {wi} {off:#x} world p {:?}", world[i].t);
        }
    }
    // Parent relations of the vectors: source of the proxy, child of the breakable / of the solid.
    let s = &worlds[1].objects;
    let (p, b, c) = (record_at(s, 0x2A6F11), record_at(s, 0x2A6F48), record_at(s, 0x2A9BF1));
    assert_eq!(s.records[b].context, Context::ProxySource);
    assert_eq!(s.records[b].transform_parent, Some(p as u32));
    assert_eq!(s.records[c].transform_parent, Some(b as u32));
}

#[test]
fn drawable_origin_bounds_match() {
    let Some(d) = disc("drawable_origin_bounds_match") else { return };
    let worlds = all_worlds(d);
    let want = [
        (1, [-8328.4, -4665.6, -6671.0], [8707.7, 8445.7, 2419.0]),
        (2, [-1090.4, -747.7, -485.8], [2399.7, 586.8, 418.5]),
        (3, [-15830.8, -38740.9, -1645.6], [5999.8, 1889.4, 6126.7]),
    ];
    for (wi, lo, hi) in want {
        let s = &worlds[wi].objects;
        let pl = s.placements(PlacementOptions { ancestor_overrides: false, clones: false });
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for p in &pl {
            for k in 0..3 {
                mn[k] = mn[k].min(p.world.t[k]);
                mx[k] = mx[k].max(p.world.t[k]);
            }
        }
        assert!(approx3(mn, lo, 0.06) && approx3(mx, hi, 0.06), "world {wi}: {mn:?} {mx:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Scene output

#[test]
fn scenes_build_for_every_world() {
    let Some(d) = disc("scenes_build_for_every_world") else { return };
    let worlds = all_worlds(d);
    let mut mirrored_draws = 0;
    let mut skipped = 0;
    for (i, w) in worlds.iter().enumerate() {
        let scene = Scene::build(w, SceneOptions::default());
        assert!(scene.texture_errors.is_empty(), "world {i}: {:?}", scene.texture_errors);
        let live = w.objects.drawable_live() + w.objects.drawable_in_clones();
        let placed: std::collections::BTreeSet<(u32, Option<u32>)> =
            scene.instances.iter().map(|x| (x.record, x.clone_of_proxy)).collect();
        assert!(placed.len() <= live, "world {i}");
        for sm in &scene.submeshes {
            let m = &scene.materials[sm.material as usize];
            assert!(m.texture.is_some(), "world {i}: submesh without texture");
            let end = sm.first_index as usize + sm.index_count as usize;
            assert!(scene.indices[sm.first_index as usize..end].iter().all(|&x| (x as u32) < sm.vertex_count));
        }
        for inst in &scene.instances {
            assert!(!inst.transform.is_degenerate(), "world {i}");
        }
        skipped += scene.skipped_degenerate;
        mirrored_draws += scene.instances.iter().filter(|x| x.mirrored).count();
        if i == 0 {
            // Splash: the world mesh is empty; one live drawable object with six meshes.
            assert_eq!(scene.instances.len(), 6);
        }
    }
    eprintln!("mirrored draws over all worlds: {mirrored_draws}; degenerate instance draws skipped: {skipped}");
    assert!(mirrored_draws > 0);
}

// ---------------------------------------------------------------------------------------------------------------
// transforms.md 4: sentinels and the rotation override

#[test]
fn sentinels_and_overrides_match() {
    let Some(d) = disc("sentinels_and_overrides_match") else { return };
    let worlds = all_worlds(d);
    // Unplaced instance meshes: 201 in 10 worlds, 190 of them in Bentley Comes Through (world 42), over the six
    // placed objects with OIDs 1464 to 1469; all 60 instance meshes of OID 1464 are unplaced.
    let mut per_world = BTreeMap::new();
    let mut owners = std::collections::BTreeSet::new();
    for (i, w) in worlds.iter().enumerate() {
        for r in &w.objects.records {
            let Some(set) = &r.mesh_set else { continue };
            let n = set.meshes.iter().filter(|m| m.instance.is_some_and(|x| x.matrix.is_degenerate())).count();
            if n > 0 {
                *per_world.entry(i).or_insert(0) += n;
                if i == 42 {
                    owners.insert(r.oid);
                }
                if i == 42 && r.oid == 1464 {
                    assert_eq!(n, set.meshes.iter().filter(|m| m.instance.is_some()).count());
                    assert_eq!(n, 60);
                }
            }
        }
    }
    assert_eq!(per_world.values().sum::<usize>(), 201);
    assert_eq!(per_world.len(), 10);
    assert_eq!(per_world[&42], 190);
    assert_eq!(owners.into_iter().collect::<Vec<_>>(), (1464..=1469).collect::<Vec<i16>>());

    // The parked proxy of A Grave Undertaking is not placed, and nothing it places reaches the placements.
    let s = &worlds[24].objects;
    let i = record_at(s, 0x511D19);
    let r = &s.records[i];
    assert_eq!((r.class, r.oid, r.context), (83, 2274, Context::TopLevel));
    let local = r.local.unwrap();
    assert_eq!(local.cols, Affine::IDENTITY.cols);
    assert_eq!(local.t, [0.0, 0.0, f32::MAX]);
    let world = s.world_transforms(true);
    let placed = s.placed_flags(&world);
    assert!(!placed[i]);
    let pl = s.placements(PlacementOptions::default());
    assert!(pl.iter().all(|p| p.clone_of_proxy != Some(i as u32)));
    for (wi, w) in worlds.iter().enumerate() {
        let pl = w.objects.placements(PlacementOptions::default());
        assert!(pl.iter().all(|p| !formats::stream::is_sentinel_position(p.world.t)), "world {wi}");
        let scene = Scene::build(w, SceneOptions::default());
        assert!(scene.instances.iter().all(|x| !x.transform.is_degenerate()), "world {wi}");
    }

    // Rotation override (extension bit 3) on 50 objects, all drawable: world rotation = basis world rotation times
    // the local rotation.
    let mut n = 0;
    for w in worlds {
        let s = &w.objects;
        let with = s.world_transforms(true);
        for (i, r) in s.records.iter().enumerate() {
            let Some(ext) = r.extension else { continue };
            if ext.flags & Extension::ROTATION_FROM_BASIS == 0 {
                continue;
            }
            n += 1;
            assert!(r.is_drawable());
            let b = s.find_ancestor(i as u32, ext.basis.unwrap()).unwrap() as usize;
            let local = r.local_transform();
            let want = with[b].mul(&Affine { cols: local.cols, t: [0.0; 3] }).cols;
            for c in 0..3 {
                assert!(approx3(with[i].cols[c], want[c], 1e-4));
            }
            // The position still comes from the parent.
            let parent = r.transform_parent.map_or(Affine::IDENTITY, |p| with[p as usize]);
            assert!(approx3(with[i].t, parent.transform_point(local.t), 1e-2));
        }
    }
    assert_eq!(n, 50);
}

#[test]
fn proxy_overrides_warps_exits_and_issuing_meshes() {
    let Some(d) = disc("proxy_overrides_warps_exits_and_issuing_meshes") else { return };
    let table = formats::options::OptionTable::sly1();
    let (mut overrides, mut warps, mut exits, mut named_exits) = (0usize, 0usize, 0usize, 0usize);
    for (i, w) in all_worlds(d).iter().enumerate() {
        let s = &w.objects;
        for p in &s.proxies {
            let o = s.proxy_overrides(w.data, p, &table).unwrap_or_else(|e| panic!("world {i}: {e}"));
            assert_eq!(o.len(), p.overrides as usize, "world {i}");
            assert!(o.iter().all(|e| e.distance_mode != 2 || e.max_distance.is_some()));
            // With one source the proxy's child list starts right after the overrides.
            if p.entries.len() == 1
                && let Some(last) = o.last()
            {
                let list = formats::reader::le_u16(w.data, last.options.end).unwrap() as usize;
                let rec = &s.records[p.record as usize];
                assert!(list <= (rec.subtree_end - p.record) as usize, "world {i}");
            }
            overrides += o.len();
        }
        for r in &s.records {
            if let Some(info) = r.warp_info(w.data, &table).unwrap() {
                warps += 1;
                assert!(info.hidden.iter().all(|&h| h >= 0), "world {i}");
            }
            if let Some(dest) = r.exit_destinations(w.data).unwrap() {
                exits += 1;
                named_exits += dest[0].is_some() as usize;
            }
        }
        // The issuing mesh of every draw carries the draw's pass and billboard mode.
        let scene = Scene::build(w, SceneOptions { decode_textures: false, ..Default::default() });
        for inst in &scene.instances {
            let m = inst.issuing(w).unwrap();
            assert_eq!((m.pass, m.billboard), (inst.pass, inst.billboard), "world {i}");
        }
    }
    eprintln!("proxy overrides {overrides}, warps {warps}, exits {exits} ({named_exits} with a destination)");
    assert!(overrides > 0 && warps > 0 && named_exits > 0);
}

#[test]
fn glint_records_read() {
    // rendering.md 6.2: every mesh with flag 0x080 carries a direction and four coefficients.
    let Some(d) = disc("glint_records_read") else { return };
    let (mut n, mut unit) = (0usize, 0usize);
    for w in all_worlds(d) {
        for set in mesh_sets(w) {
            for m in &set.meshes {
                if let Some(g) = m.glint(w.data).unwrap() {
                    n += 1;
                    let l = (g.direction.iter().map(|x| x * x).sum::<f32>()).sqrt();
                    unit += ((l - 1.0).abs() < 1e-2) as usize;
                    assert!(g.c.iter().all(|x| x.is_finite()));
                    let f = g.factor(0.5);
                    assert!((0.0..=1.0).contains(&f));
                }
            }
        }
    }
    eprintln!("glint meshes {n}, {unit} with a unit direction");
    assert!(n > 0);
}
