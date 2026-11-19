//! Tests against the player's own Sly 3 disc: every vector of `specs/sly3/audio.md`, the movie table and movie
//! soundtrack vectors of `movies.md`, and the world list of `disc.md`. They skip (and pass) when `SLYNX_DISC_SLY3`
//! is not set. Only numbers and hashes live here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use audio::bank::EffectBank;
use audio::sly2::vpk::VpkHeader;
use audio::sly2::{FileRecord, MovieAudio};
use audio::sly3::{Sly3Disc, WorldHeader};
use formats::hash::{hex, sha256};
use formats::iso::ReadAt;
use harness::golden::{Game, require_disc};

type Disc = Sly3Disc<std::fs::File>;

struct World {
    slot: usize,
    id: u32,
    name: String,
    len: usize,
    sha: String,
    header: WorldHeader,
}

struct Data {
    disc: Mutex<Disc>,
    worlds: Vec<World>,
}

fn data(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly3, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut disc = Sly3Disc::open_path(path).expect("open Sly 3 disc");
        let slots: Vec<(usize, String, FileRecord)> =
            disc.worlds.iter().filter_map(|w| w.file.map(|f| (w.slot, w.name.clone(), f))).collect();
        let compressed: Vec<(usize, String, u32, Vec<u8>)> =
            slots.iter().map(|(s, n, f)| (*s, n.clone(), f.identity, disc.read(f).unwrap())).collect();
        let worlds = std::thread::scope(|sc| {
            let hs: Vec<_> = compressed
                .iter()
                .map(|(slot, name, id, c)| {
                    sc.spawn(move || {
                        let w = formats::compress::decompress(c).unwrap();
                        let header = WorldHeader::parse(&w).unwrap_or_else(|e| panic!("{name}: {e}"));
                        World { slot: *slot, id: *id, name: name.clone(), len: w.len(), sha: hex(&sha256(&w)), header }
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        Data { disc: Mutex::new(disc), worlds }
    }))
}

fn world<'a>(d: &'a Data, name: &str) -> &'a World {
    d.worlds.iter().find(|w| w.name == name).unwrap()
}

fn head(disc: &mut Disc, r: &FileRecord, n: usize) -> Vec<u8> {
    disc.source().read_vec(r.offset(), n.min(r.size as usize)).unwrap()
}

/// (slot, name, episode, world id, sector, compressed, decompressed, SHA-256 of the decompressed world)
type WorldRow = (usize, &'static str, i32, u32, u32, u32, usize, &'static str);

const WORLDS: [WorldRow; 39] = [
    (
        0,
        "dvd_menu",
        1,
        133,
        753440,
        1906746,
        4083477,
        "48718637c810dd5daf7ac1aceca1aefde6dcebe3a385407c7b060d6698eb520b",
    ),
    (
        2,
        "i_trainer",
        2,
        157,
        775552,
        5547748,
        10092456,
        "067fbb0f46879c687cd4da6f5e6735f53a3659fbd67ede9dbe223f0738cbe8a2",
    ),
    (
        3,
        "v_ext",
        3,
        239,
        797472,
        13425318,
        25364277,
        "60d7615df6e7c0a2ad1b9955b41097220eca19d0071bd04e8e55641b32c60200",
    ),
    (
        4,
        "v_canal",
        3,
        237,
        826112,
        9203376,
        18690203,
        "38fff196a20f6c083eaa0a510c80d371bec15641d7f8c34ae3cb8fa23d9051b9",
    ),
    (5, "v_apt", 3, 234, 835648, 4424331, 8320220, "4b8beaf0de21ccf5a3694b9feced105a6dd0a970bcd09be5f1adeccb05f3f017"),
    (
        6,
        "v_gauntlet",
        3,
        242,
        851808,
        8481498,
        14971275,
        "0aebdfec19f143abfcad24d088312969c3f3aa99a1655fcb6d555340617b3349",
    ),
    (
        7,
        "v_interpol",
        3,
        245,
        872448,
        3496739,
        6650373,
        "ae9d224a2932976857c9846421d5bb58464c5697006082220f766197ad876c71",
    ),
    (
        8,
        "o_ext",
        4,
        201,
        901280,
        11982028,
        19997607,
        "cd0f69420512c9d0a7a32ea11e48c8c0ced8b70ac97a7c3c73792d998b0343da",
    ),
    (
        9,
        "o_quarry",
        4,
        204,
        973024,
        7523243,
        13517957,
        "426b3fabb9f11d4d4120987aaff275d9a420df8348d1af77506d174d95d36e6d",
    ),
    (
        10,
        "o_arena",
        4,
        187,
        931104,
        5686984,
        10119876,
        "9bd6e162324ed2a4a04ad948a476cf99eda233187208ecf44669e467a3ea69d6",
    ),
    (
        11,
        "o_cave_a",
        4,
        192,
        942560,
        5911606,
        10194832,
        "c7da26626c96a32f47d7d56b18626041a227d56dc5e0d25d734ab2ee9ef66cdb",
    ),
    (
        12,
        "o_cave_b",
        4,
        195,
        957888,
        5589231,
        9700906,
        "b8fd802e22942a2e285135f52177c3b0744e0716bf0f3fa4e7f816db85363e5b",
    ),
    (
        13,
        "o_bar",
        4,
        189,
        991136,
        5915836,
        10338986,
        "43bee44d7f4ad12289a66c6aebc526a9e3126cb00e14ebbae94c8d59ae59d834",
    ),
    (
        14,
        "o_cave_murray",
        4,
        198,
        1008256,
        6849642,
        11477335,
        "210deb65b97c61243562a4235ffbe105ca2533cfb20d30c95cc1688cdfad352b",
    ),
    (
        15,
        "h_ext",
        5,
        138,
        1035072,
        12650409,
        22409036,
        "00985a4e4c47c0458017ff6230c60b8952089916286b48dfa27be6dc5475114b",
    ),
    (
        16,
        "h_hotel",
        5,
        151,
        1064128,
        6328218,
        11479334,
        "26b44695ed24bd9fc0c2b3f77ce8795ef58c20fc5ca39e3c26db938066c4193b",
    ),
    (
        17,
        "h_hangar_a",
        5,
        143,
        1082848,
        3539566,
        6785858,
        "66bf458a2d2507cb23d80e4d2b43baa28a7f4c0c150767a9fbdc6d71eb83171d",
    ),
    (
        18,
        "h_hangar_b",
        5,
        145,
        1094176,
        4609797,
        8678941,
        "887f47c62b27356350d38c2cc2cd1cda879038790bcb10a2576f4b90a652cef3",
    ),
    (
        19,
        "h_hangar_c",
        5,
        148,
        1112352,
        5638470,
        10570744,
        "635454478fd84d4703b47f2524c701aaede659da3c8610dad0bb2e412a539850",
    ),
    (
        20,
        "h_sewer",
        5,
        154,
        1127264,
        5503683,
        10925099,
        "8eb1874a88309ae233b3d7deae73a7f7011057c93ea78cd5972f7872859e52ce",
    ),
    (
        21,
        "h_dogfight",
        5,
        135,
        1142464,
        8579908,
        14859910,
        "5562dce629dbd18a3388a5423994b6c3ec648ccb6cc7043dc3d85ffd82d75c97",
    ),
    (
        22,
        "h_hack",
        5,
        141,
        1160512,
        1482561,
        3295153,
        "56a7db7a7387c02f330a1eafdbe05d39d1155b47bb928e082cac9e0f58d19d29",
    ),
    (
        23,
        "c_ext",
        6,
        3,
        1180000,
        12375275,
        21378964,
        "5446969784d08c255ce6802e91160252938dbfa38d6ac55752b2a6d9e50fb25e",
    ),
    (
        24,
        "c_intro",
        6,
        17,
        1210880,
        6675411,
        11848300,
        "22b2d1bdaa6caffabf99c50474c8779acb6a94f0ebe9cbe785d050877efc9d1c",
    ),
    (
        25,
        "c_flashback",
        6,
        6,
        1231648,
        4519103,
        8319343,
        "331d7e44ce0ca41b024ce5367453715e2c3be8f89d7461d4e861b254556ad4fc",
    ),
    (
        26,
        "c_forest",
        6,
        9,
        1246464,
        4984308,
        9285172,
        "f0a0c5bc179ea54ad7e1346cb8d924e695acecdc4ec3cbc4645caafb0451f5bc",
    ),
    (27, "c_apt", 6, 1, 1306752, 2059187, 4070050, "9d14c37b374ed92c13a6a7d6868003aee03e2983fc71ac6ecce299191975921c"),
    (
        28,
        "c_hall_a",
        6,
        12,
        1260608,
        6662452,
        11788186,
        "588c822e48f2f6f71f4a5d278714491718a40a1cec2d84be19f5270223f86158",
    ),
    (
        29,
        "c_hall_b",
        6,
        15,
        1275904,
        6081676,
        11097099,
        "94044243d224abb647cd8434440ae263dd6c66de561d0bb3d1aa1bff301731af",
    ),
    (
        30,
        "c_tilt_hall",
        6,
        20,
        1292256,
        5101147,
        9219638,
        "6460a44d41110b26d24a26478fe40ca39e734971eaae8853de1b485d265317c7",
    ),
    (
        31,
        "p_ext",
        7,
        213,
        1325024,
        12876900,
        21858684,
        "f4827e156eff9d45045e9999c712c6040cbcc1f3249ef1a76112bd389d5dc700",
    ),
    (
        32,
        "p_at_sea",
        7,
        207,
        1362880,
        11471010,
        20755500,
        "a575b68af7d62edd1880ba03e2d40776dee3dcfb9c54d9db449184f178d1cbcd",
    ),
    (
        33,
        "p_dive",
        7,
        210,
        1383552,
        7144182,
        12437854,
        "0d538699825fa6819cd49f5438e7bff52f6d69c2a5e07cb4dea4dc0310588764",
    ),
    (
        34,
        "p_island_map",
        7,
        216,
        1396512,
        8666781,
        14547783,
        "e56cb4243c12f3261b84ac21ed5ed62becaf1669fe74f0b0bc79a51575aad40c",
    ),
    (
        35,
        "m_ext",
        8,
        175,
        1428480,
        17612657,
        30254415,
        "870a08e90588f40924cfff973db3a4f2c997f6e15fb47cc418d5cc032f7eb53c",
    ),
    (
        36,
        "m_underwater",
        8,
        181,
        1459520,
        5765765,
        10273746,
        "ae8ddffb493d61222f3f287fc9ae252a78aade3c3c343cbb9c964713fbc00f54",
    ),
    (
        37,
        "m_vault",
        8,
        184,
        1470368,
        6253725,
        11031837,
        "33a692d2d88e8c17b959fde00a552b35ade2ceb27c0f0db8933d836b223bb2e6",
    ),
    (
        38,
        "m_gauntlet",
        8,
        178,
        1487968,
        7095487,
        11432056,
        "d6e8b8cadf73644025e5d370b7e62c3d82e778fc4f0f60803487624364e00974",
    ),
    (
        39,
        "m_boss",
        8,
        172,
        1503904,
        5438399,
        9864023,
        "9d89303d77c62ac34b6062b47c80863026b2610ac1c64879171d8d44dc592160",
    ),
];

/// (identity, sector, bytes, audio rate, audio bytes, SHA-256 of the file)
const MOVIES: [(u32, u32, u32, u32, u32, &str); 21] = [
    (0x14F, 444753, 77496324, 44100, 9230784, "a0227479034c46fec18504778b5301600519a8c65a7eabe05c40cefbce8d423d"),
    (0x150, 482594, 44302340, 44100, 5325824, "aef8dbb2667c9a8a7e1b2679634de2c6d8c7190002cbb205e51f2a64c93ca701"),
    (0x151, 504227, 510345220, 48000, 40949824, "053e8ebe827f62d0adeb2b92df0620af14e2a4c6045ee1035e89912700830291"),
    (0x130, 1526531, 61390852, 44100, 7233920, "92b934ace2fa76f6c6795622ead3c846858f4cb5656d56b1c0b06f47a6993564"),
    (0x12F, 71778, 54394884, 44100, 6451392, "021bd56c048b518e3ff7764380321e3c75f01334268b95cb2785b65c1d5b8131"),
    (0x14E, 3232, 70549508, 44100, 8485632, "6fbfcd80ba6d2af63e66dc0fa08188604eca1cd8ceb6bbaf704792282a61fb0f"),
    (0x134, 1556508, 55754756, 44100, 6598720, "f95e969ca0f7de9f0bd14cc911c1541ce734f2f2160a8d203c6ead1ac5332132"),
    (0x135, 387661, 27705348, 44100, 3309248, "00404d7093f60bbedfd15f40eec23f2d76fa544bd7c1fb5347f9248b3c506238"),
    (0x139, 1583733, 49987588, 44100, 5907392, "551c285fe9b0ad7b65730bb2951c56fa480ed3bb0d2b1fb180e6a71768cc4fb4"),
    (0x13A, 332545, 30507012, 44100, 3695744, "efbad8962dc87cac01c12a5ce47ea0dd4a987634f5f45ede094d91edba4e59b7"),
    (0x13E, 1608142, 63995908, 44100, 7601152, "583f77294963b3cecc67e9adc3ffc16127ac70df319e8c2c21a45842f4de69da"),
    (0x13F, 253157, 35815428, 44100, 4267520, "4099c318365c29bc27fbcc4b476339b33d7ef73715321a34d8dc5a55a06711a2"),
    (0x144, 1639391, 39616516, 44100, 4746624, "f830698db1a0ba5db419061bdea9a285d57dd0e651ffecefa7f57b284fb603cc"),
    (0x145, 177776, 34652164, 44100, 4101888, "fdc27b0c6cc98c9c95e5793e04ec1476fac5ad7ba84c8b020880afec6424cdf7"),
    (0x146, 161495, 33341444, 44100, 3951552, "e23d02ad9c7f3070e35d7906194d674f61ab0f132b7b29e605c95f9cad58959a"),
    (0x14A, 1658736, 45088772, 44100, 5266560, "12949bce32b1e31b6d4ace78d9695e7c752dbd38d1f58c760602f1cffa291511"),
    (0x14B, 98339, 28377092, 44100, 3448512, "57efbebc2ba866cc713e1fcb7e087b9439096bf7bb2146a2c87041e3b15fd62b"),
    (0x14C, 1680753, 49741828, 44100, 5850368, "5452146bbc61025a681e01c754d40b6955fcd3a33dfe4c34e5fb28cf58d64ad3"),
    (0x14D, 37681, 69828612, 44100, 8382848, "01a0a85f24c8094fd6c1d9c672ad002af86c2d443b6a099565b4af9e338b986a"),
    (0x152, 1705042, 1376260, 44100, 222144, "e8ca0371c28b4c08dd8d168eb85dc1efcbba82705bdcb7745286e4a615413ae3"),
    (0x153, 1705715, 5832708, 44100, 736960, "7b2bb9a67517b7d62408e47d2239554718ca52b072abbc50dc94ea2c797b8cc9"),
];

#[test]
fn disc_and_world_list() {
    let Some(d) = data("disc_and_world_list") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(disc.segments.len(), 2);
    assert_eq!((disc.segments[0].0, disc.segments[0].1), (0x1000, 0x0010_0000));
    assert_eq!(disc.worlds.len(), 40);
    assert_eq!(disc.worlds[1].name, "sampler_menu");
    assert!(disc.worlds[1].file.is_none());
    assert_eq!(d.worlds.len(), 39);
    for &(slot, name, ep, id, sector, size, len, sha) in &WORLDS {
        let w = &disc.worlds[slot];
        let f = w.file.unwrap();
        assert_eq!(
            (w.name.as_str(), w.episode, f.identity, f.sector, f.size),
            (name, ep, id, sector, size),
            "slot {slot}"
        );
        let loaded = d.worlds.iter().find(|x| x.slot == slot).unwrap();
        assert_eq!((loaded.len, loaded.sha.as_str()), (len, sha), "{name}");
    }
    let menu = disc.worlds[0].file.unwrap();
    let bytes = disc.read(&menu).unwrap();
    assert_eq!(hex(&sha256(&bytes)), "a27ff71985596e0cf9864c1d9c8d80a6de2614fe50723381e85067eaa96f8b45");
    assert_eq!(hex(&bytes[..16]), "ff00000f00464b2458ff616c6c5f7761");
    // Header sections (world-file.md 7).
    let dvd = &world(d, "dvd_menu").header;
    assert_eq!((dvd.key_tables.clone(), dvd.groups.len(), dvd.save_range.clone()), (0..2, 15, 0x52C..0x17308));
    assert_eq!(dvd.sound_range, 0x17308..0x1C9A0);
    let tr = &world(d, "i_trainer").header;
    assert_eq!(
        (tr.groups.len(), tr.save_range.clone(), tr.sound_range.clone()),
        (189, 0x40FC..0x1AED8, 0x1AED8..0x32960)
    );
    let apt = &world(d, "v_apt").header;
    assert_eq!((apt.key_tables.clone(), apt.groups.len()), (0..0x4A, 130));
    assert_eq!((apt.save_range.clone(), apt.sound_range.clone()), (0x2CFC..0x19AD8, 0x19AD8..0x528D3));
    let g = |i: usize| {
        let g = &apt.groups[i];
        (g.name.as_str(), g.shared, g.streaming, g.id, g.second, g.copies, g.body_size)
    };
    assert_eq!(g(0), ("v_chair_fancy", 0, 0, 138_293, 140_925, 50, 29_328));
    assert_eq!(g(1), ("v_divan", 0, 0, 137_751, 140_926, 2, 40_416));
    assert_eq!(g(129), ("stock_objects", 1, 0, 9900, 16_866, 1, 95_552));
}

#[test]
fn sound_sections() {
    let Some(d) = data("sound_sections") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let mut flags: BTreeMap<u8, usize> = BTreeMap::new();
    let mut channels: BTreeMap<usize, usize> = BTreeMap::new();
    let mut without = BTreeSet::new();
    let mut speakers: BTreeMap<i16, usize> = BTreeMap::new();
    let mut voice_files = BTreeSet::new();
    let mut music_files = BTreeSet::new();
    let mut empty = 0;
    let mut lengths: BTreeMap<u32, usize> = BTreeMap::new();
    let mut no_file = 0;
    let (mut entries_min, mut entries_max, mut map_min, mut map_max) = (usize::MAX, 0, i16::MAX, 0);
    let (mut lines_min, mut lines_max) = (usize::MAX, 0);
    for w in &d.worlds {
        let s = &w.header.sound;
        assert_eq!(s.bank.unwrap().identity, w.id - 1, "{}: bank identity", w.name);
        for &f in &s.flags {
            *flags.entry(f).or_default() += 1;
        }
        entries_min = entries_min.min(s.sound_ids.len());
        entries_max = entries_max.max(s.sound_ids.len());
        map_min = map_min.min(s.id_space);
        map_max = map_max.max(s.id_space);
        let ids: BTreeSet<i16> = s.sound_ids.iter().copied().collect();
        assert_eq!(ids.len(), s.sound_ids.len());
        assert!(s.sound_ids.iter().all(|&i| (0..s.id_space).contains(&i)));
        match s.resident {
            Some(m) => {
                assert_eq!(m.identity, w.id + 1, "{}: stream identity", w.name);
                let vh = VpkHeader::parse(&head(&mut disc, &m, 2048)).unwrap();
                assert_eq!(vh.sample_rate, 44_100);
                *lengths.entry(vh.bytes_per_channel).or_default() += 1;
                // The file holds every channel's bytes (the last round may be shorter than 8 KiB per channel).
                let rounds = (vh.bytes_per_channel as usize).div_ceil(0x2000);
                assert!(m.size as usize >= 0x800 + (rounds - 1) * 0x2000 * vh.channels.len() + 16, "{}", w.name);
                assert_eq!(vh.channels.len(), s.resident_channels.len());
                assert!(
                    vh.channels.iter().enumerate().all(|(k, c)| c.volume == 127 && c.group as usize == k && c.pan == 0)
                );
                *channels.entry(vh.channels.len()).or_default() += 1;
            }
            None => {
                without.insert(w.name.as_str());
            }
        }
        lines_min = lines_min.min(s.voice_lines.len());
        lines_max = lines_max.max(s.voice_lines.len());
        for l in &s.voice_lines {
            *speakers.entry(l.speaker).or_default() += 1;
            match l.tracks[0].file {
                Some(f) => {
                    voice_files.insert(f);
                }
                None => no_file += 1,
            }
        }
        for (_, r) in &s.music {
            match r {
                Some(r) => {
                    music_files.insert(*r);
                }
                None => empty += 1,
            }
        }
    }
    assert_eq!(flags, [(0, 12_182), (1, 448), (2, 898)].into_iter().collect());
    assert_eq!(channels, [(2, 18), (3, 6), (4, 8)].into_iter().collect());
    // audio.md 2 gives 120.0 s (3,024,016 bytes) per channel; only 9 of the 32 world streams have exactly that.
    let expect: BTreeMap<u32, usize> = [
        (2_199_600, 1),
        (2_261_760, 1),
        (2_266_864, 1),
        (2_267_888, 3),
        (2_269_200, 1),
        (2_281_648, 1),
        (2_290_640, 4),
        (3_004_432, 1),
        (3_024_016, 9),
        (3_024_032, 1),
        (3_024_048, 4),
        (3_024_080, 1),
        (3_024_176, 1),
        (3_027_040, 1),
        (3_031_872, 1),
        (5_498_592, 1),
    ]
    .into_iter()
    .collect();
    assert_eq!(lengths, expect);
    // Voice entries whose file record is empty.
    assert_eq!(no_file, 8);
    let expect: BTreeSet<&str> =
        ["dvd_menu", "v_canal", "o_arena", "h_hangar_a", "h_hack", "c_apt", "c_hall_b"].into_iter().collect();
    assert_eq!(without, expect);
    assert_eq!((entries_min, entries_max, map_min, map_max), (181, 502, 319, 858));
    assert_eq!((lines_min, lines_max), (151, 950));
    let expect: BTreeMap<i16, usize> = [
        (52, 7054),
        (54, 7134),
        (53, 2570),
        (60, 2403),
        (59, 1728),
        (57, 1653),
        (56, 479),
        (64, 400),
        (65, 344),
        (55, 328),
        (63, 328),
        (67, 310),
        (66, 280),
        (62, 260),
        (-1, 2078),
    ]
    .into_iter()
    .collect();
    assert_eq!(speakers, expect);
    // Other stream files: all 44.1 kHz; 2, 5, 3 or 7 channels.
    assert_eq!((music_files.len(), empty), (152, 250));
    let mut by_channels: BTreeMap<usize, usize> = BTreeMap::new();
    for f in &music_files {
        let vh = VpkHeader::parse(&head(&mut disc, f, 2048)).unwrap();
        assert_eq!(vh.sample_rate, 44_100);
        *by_channels.entry(vh.channels.len()).or_default() += 1;
    }
    assert_eq!(by_channels, [(2, 124), (3, 2), (5, 25), (7, 1)].into_iter().collect());
    // Voice files: VAG version 0, 22,050 Hz mono.
    assert_eq!(voice_files.len(), 3_654);
    for f in &voice_files {
        let vh = audio::vag::VagHeader::parse(&head(&mut disc, f, 48)).unwrap();
        assert_eq!((vh.version, vh.sample_rate), (0, 22_050));
    }
}

#[test]
fn banks_and_stream_vectors() {
    let Some(d) = data("banks_and_stream_vectors") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    // Every bank is version 3 and parses through its steps.
    for w in &d.worlds {
        let b = disc.read_bank(&w.header.sound.bank.unwrap()).unwrap();
        assert_eq!(b.version, 3, "{}", w.name);
        assert!(b.sounds.len() >= w.header.sound.flags.len());
    }
    let menu = world(d, "dvd_menu").header.sound.bank.unwrap();
    assert_eq!((menu.sector, menu.size, menu.identity), (754_400, 580_996, 132));
    let bytes = disc.read(&menu).unwrap();
    assert_eq!(hex(&bytes[..24]), "0300000002000000180000000c7800002478000060650800");
    let b = EffectBank::parse(bytes).unwrap();
    assert_eq!((b.version, b.sounds.len()), (3, 181));
    let tr = &world(d, "i_trainer").header.sound;
    let tb = tr.bank.unwrap();
    assert_eq!((tb.sector, tb.size, tb.identity), (778_272, 1_476_852, 156));
    assert_eq!((disc.read_bank(&tb).unwrap().sounds.len(), tr.id_space), (361, 858));
    let m = tr.resident.unwrap();
    assert_eq!((m.sector, m.size, m.identity), (781_088, 6_055_936, 158));
    let s = disc.read_vpk(&m).unwrap();
    let h = &s.header;
    assert_eq!((h.bytes_per_channel, h.data_offset, h.interleave, h.sample_rate), (3_024_016, 0x800, 0x2000, 44_100));
    let ch: Vec<(u8, u8, u16)> = h.channels.iter().map(|c| (c.volume, c.group, c.pan)).collect();
    assert_eq!(ch, [(127, 0, 0), (127, 1, 0)]);
    assert!(s.channels.iter().all(|c| c.len() == 3_024_016));
    let v = world(d, "v_ext").header.sound.resident.unwrap();
    assert_eq!((v.sector, v.size, v.identity), (809_472, 9_082_880, 240));
    assert_eq!(VpkHeader::parse(&head(&mut disc, &v, 2048)).unwrap().channels.len(), 3);
}

#[test]
fn movies() {
    let Some(d) = data("movies") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(disc.movies.len(), 23);
    let distinct: BTreeSet<FileRecord> = disc.movies.iter().flatten().copied().collect();
    assert_eq!(distinct.len(), 21);
    assert_eq!((disc.movies[19], disc.movies[20]), (disc.movies[21], disc.movies[22]));
    assert_eq!(disc.movies[5].unwrap().sector, 3232);
    let records: Vec<FileRecord> = disc.movies.iter().flatten().copied().collect();
    let mut files = Vec::new();
    for &(id, sector, size, rate, audio_bytes, sha) in &MOVIES {
        let r = *records.iter().find(|r| r.identity == id).unwrap();
        assert_eq!((r.sector, r.size), (sector, size), "movie {id:#x}");
        files.push((id, disc.read(&r).unwrap(), rate, audio_bytes, sha));
    }
    drop(disc);
    // Hash the files and read each soundtrack's headers, in parallel.
    std::thread::scope(|s| {
        for (id, bytes, rate, audio_bytes, sha) in &files {
            s.spawn(move || {
                assert_eq!(hex(&sha256(bytes)), *sha, "movie {id:#x}");
                assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 1, 0xB9], "end code");
                let mut demux = video::demux::Demuxer::new();
                let mut audio = MovieAudio::new();
                let mut out = Vec::new();
                'scan: for chunk in bytes.chunks(256 * 1024) {
                    demux.push(chunk);
                    while let Some(p) = demux.next_packet().unwrap() {
                        if let video::demux::Packet::Audio(a) = p {
                            audio.push(a, &mut out).unwrap();
                            if audio.header().is_some() {
                                break 'scan;
                            }
                        }
                    }
                }
                let h = *audio.header().unwrap();
                assert_eq!((h.format, h.channels, h.interleave), (16, 2, 32), "movie {id:#x}");
                assert_eq!((h.sample_rate, h.data_size), (*rate, *audio_bytes), "movie {id:#x}");
            });
        }
    });
}

#[test]
fn long_movie_soundtrack_decodes() {
    let Some(d) = data("long_movie_soundtrack_decodes") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let r = disc.movies.iter().flatten().find(|r| r.identity == 0x151).copied().unwrap();
    let bytes = disc.read(&r).unwrap();
    drop(disc);
    let mut demux = video::demux::Demuxer::new();
    let mut audio = MovieAudio::new();
    let mut frames = 0usize;
    let mut out = Vec::new();
    for chunk in bytes.chunks(1 << 20) {
        demux.push(chunk);
        while let Some(p) = demux.next_packet().unwrap() {
            if let video::demux::Packet::Audio(a) = p {
                audio.push(a, &mut out).unwrap();
                frames += out.len() / 2;
                out.clear();
            }
        }
    }
    assert!(audio.done());
    let h = audio.header().unwrap();
    assert_eq!(h.sample_rate, 48_000);
    assert_eq!(frames as u64, h.frames());
    // 40,949,824 bytes / 2 / 16 x 28 samples per channel at 48 kHz: 746.48 s.
    assert!((frames as f64 / 48_000.0 - 746.48).abs() < 0.01);
}

/// Installs a world's sound in a fresh engine, starts its stream and every bank sound with a game ID once, and
/// renders `seconds` of output in 60 Hz frames; returns the output's SHA-256 and peak level. The bank is parsed
/// afresh: random-choice steps keep their last choice in the bank (audio.md 3), so a replay needs a new bank.
fn play_world(audio: &audio::sly3::WorldAudio, seconds: f32) -> (String, f32) {
    let mut audio = audio.clone();
    audio.bank = audio.bank.map(|b| std::sync::Arc::new(EffectBank::parse(b.bytes.clone()).unwrap()));
    let (mut engine, mut renderer) = audio::create(audio::Config::default());
    let stream = audio.install(&mut engine);
    assert_eq!(stream.is_some(), audio.stream.is_some());
    for &id in audio.sound_ids().iter().filter(|&&id| id >= 0) {
        engine.play(id as u16, audio::Emitter::global());
    }
    let frames = (seconds * 60.0) as usize;
    let mut out = Vec::with_capacity(frames * 800 * 2);
    let mut block = vec![0.0f32; 800 * 2];
    for _ in 0..frames {
        engine.update(1.0 / 60.0);
        renderer.render(&mut block);
        out.extend_from_slice(&block);
    }
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let bytes: Vec<u8> = out.iter().flat_map(|s| s.to_le_bytes()).collect();
    (hex(&sha256(&bytes)), peak)
}

#[test]
fn every_world_sound_installs_and_plays() {
    let Some(d) = data("every_world_sound_installs_and_plays") else { return };
    let mut audible = 0;
    for w in &d.worlds {
        let a = {
            let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
            disc.load_world_audio(w.slot).unwrap_or_else(|e| panic!("{}: {e}", w.name))
        };
        assert!(a.bank.is_some(), "{}: no bank", w.name);
        assert_eq!(a.stream.is_some(), w.header.sound.resident.is_some(), "{}", w.name);
        if let Some(s) = &a.stream {
            assert_eq!(s.header.sample_rate, 44_100, "{}", w.name);
            assert_eq!(s.channels.len(), a.sound.resident_channels.len(), "{}", w.name);
        }
        let (sha, peak) = play_world(&a, 1.0);
        assert!(peak.is_finite() && peak <= 1.0, "{}: peak {peak}", w.name);
        if peak > 0.001 {
            audible += 1;
        }
        // Deterministic: the same world renders the same samples again.
        if matches!(w.name.as_str(), "i_trainer" | "v_apt" | "dvd_menu") {
            assert_eq!(play_world(&a, 1.0).0, sha, "{}", w.name);
        }
    }
    // Every world makes sound (its stream or its bank's sounds).
    assert_eq!(audible, d.worlds.len());
}

#[test]
fn trainer_music_and_voice_lines_read() {
    let Some(d) = data("trainer_music_and_voice_lines_read") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let s = &world(d, "i_trainer").header.sound;
    let mut voiced = 0;
    for l in s.voice_lines.iter().take(20) {
        if let Some(v) = disc.read_voice_line(s, l.number).unwrap() {
            assert_eq!(v.header.sample_rate, 22_050);
            voiced += 1;
        }
    }
    assert!(voiced > 0);
    for n in Sly3Disc::<std::fs::File>::music_numbers(s) {
        let m = disc.read_music_stream(s, n).unwrap().unwrap();
        assert_eq!(m.header.sample_rate, 44_100, "music {n}");
    }
}
