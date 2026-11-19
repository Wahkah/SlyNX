//! Tests against the player's own Sly 1 disc: the fonts (ui-and-menus.md 1.1) and the text and placement tables
//! (ui-and-menus.md 2, hud.md 2). They skip (and pass) when `SLYNX_DISC_SLY1` is not set. Only numbers, hashes and
//! the few strings the specs quote live here.

use formats::disc::Disc;
use formats::hash::{hex, sha256};
use formats::world::World;
use harness::golden::{Game, require_disc};
use ui::font::{FontSet, read_font};
use ui::tables::{Edge, TextTables, read_placements, resp};
use ui::text::{TextStyle, layout};

fn open(test: &str) -> Option<Disc<std::fs::File>> {
    Some(Disc::open_path(require_disc(Game::Sly1, test)?).expect("open disc"))
}

fn hash_strings<'a>(items: impl Iterator<Item = &'a [u8]>) -> String {
    let mut all = Vec::new();
    for s in items {
        all.extend_from_slice(s);
        all.push(0);
    }
    hex(&sha256(&all))
}

/// (bitmap width, height, cell, space, line, scale, glyph count, glyph codes or "", SHA-256 of the upright RGBA)
type FontVector = (u32, u32, u8, u8, u8, f32, usize, &'static str, &'static str);

const FONTS: [FontVector; 5] = [
    (512, 512, 25, 10, 41, 1.0, 133, "", "92d36bca7e92621f07bfc6b10da9d3f872012c1d611b57bba9c1b44809092eec"),
    (
        256,
        128,
        32,
        32,
        32,
        2.0,
        19,
        "!()+<>LORSTX^_ablrv",
        "8761b54af623c78a976e21de5424d2e77a8c659ed4017b99e2e64725df2d2ef3",
    ),
    (
        512,
        128,
        39,
        21,
        62,
        1.0,
        24,
        "%,-./0123456789:=?BHLThi",
        "6c963c54f48ccaff035481ac4506b4f045575ce1fcf605256652698dafa65a73",
    ),
    (512, 64, 39, 21, 62, 1.0, 7, "!CKklvw", "44014c6b3a7ce6ad72247efa9d069536700f9b58269d0a3db13fdf6ce018aa52"),
    (
        512,
        256,
        39,
        21,
        62,
        1.0,
        21,
        "_abcdefghijklmnopqrst",
        "6ee4096ac7530b453ee68f6583bbe2b39866c66a4257b0c143be44eabac95051",
    ),
];

/// What the walk records per font: bitmap size, metrics, scale, glyph count, glyph codes, picture hash.
type FontFacts = (u32, u32, u8, u8, u8, f32, usize, String, String);

/// Number of distinct pictures of font 3 over the 46 worlds.
const ICON_PICTURES: usize = 6;

#[test]
fn fonts_of_every_world() {
    let Some(mut disc) = open("fonts_of_every_world") else { return };
    let n = disc.worlds.len();
    let worlds: Vec<(String, Vec<u8>)> =
        (0..n).map(|i| (disc.worlds[i].name.clone(), disc.read_world(i).unwrap())).collect();
    let results: Vec<(String, Vec<FontFacts>)> = std::thread::scope(|s| {
        let hs: Vec<_> = worlds
            .iter()
            .map(|(name, bytes)| {
                s.spawn(move || {
                    let w = World::load(bytes).unwrap();
                    let fonts = (0..w.layout.materials.fonts.len())
                        .map(|i| {
                            let f = read_font(&w, i).unwrap();
                            // Metric byte 3 is the font's own number, equal to its place in the table.
                            assert_eq!(f.metrics.id as usize, i, "font number");
                            let codes: String =
                                f.glyphs.iter().map(|g| char::from_u32(g.code as u32).unwrap()).collect();
                            (
                                f.image.width,
                                f.image.height,
                                f.metrics.cell_width,
                                f.metrics.space_width,
                                f.metrics.line_height,
                                f.scale,
                                f.glyphs.len(),
                                codes,
                                hex(&sha256(&f.image.pixels)),
                            )
                        })
                        .collect();
                    (name.clone(), fonts)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut report = String::new();
    for (name, fonts) in &results {
        assert!(fonts.len() == 4 || fonts.len() == 5, "{name}: font count");
        for (i, f) in fonts.iter().enumerate() {
            // Font 4 differs: the hideout's pictures, or another world's own fifth font.
            if i == 4 && name != "The Hideout" {
                continue;
            }
            let v = FONTS[i];
            assert_eq!((f.0, f.1, f.2, f.3, f.4, f.5, f.6), (v.0, v.1, v.2, v.3, v.4, v.5, v.6), "{name} font {i}");
            if !v.7.is_empty() {
                assert_eq!(f.7, v.7, "{name} font {i} glyph codes");
            }
            // Font metric byte 3 is the font's own number.
            if name == "Paris, France" || name == "The Hideout" {
                report.push_str(&format!("{name} font {i}: {}\n", f.8));
            }
        }
        // Fonts 0 to 2 are the same picture in every world; the icon font (3) differs between worlds.
        assert_eq!(
            fonts[..3].iter().map(|f| &f.8).collect::<Vec<_>>(),
            results[0].1[..3].iter().map(|f| &f.8).collect::<Vec<_>>(),
            "{name}"
        );
    }
    let five: Vec<&str> = results.iter().filter(|r| r.1.len() == 5).map(|r| r.0.as_str()).collect();
    report.push_str(&format!(
        "worlds with a fifth font: {five:?}
"
    ));
    // ui-and-menus.md 1.1 names only the hideout; 13 worlds have a fifth font.
    assert_eq!(five.len(), 13);
    assert!(five.contains(&"The Hideout"));
    let icon_pictures: std::collections::BTreeSet<&String> = results.iter().map(|r| &r.1[3].8).collect();
    report.push_str(&format!(
        "distinct icon-font pictures: {}
",
        icon_pictures.len()
    ));
    eprintln!("{report}");
    assert_eq!(icon_pictures.len(), ICON_PICTURES);
    let paris = results.iter().find(|r| r.0 == "Paris, France").unwrap();
    let hideout = results.iter().find(|r| r.0 == "The Hideout").unwrap();
    for i in 0..5 {
        let h = if i < 4 { &paris.1[i].8 } else { &hideout.1[4].8 };
        assert_eq!(h, FONTS[i].8, "font {i} image hash");
    }
}

#[test]
fn font_set_and_layout_from_the_disc() {
    let Some(mut disc) = open("font_set_and_layout_from_the_disc") else { return };
    let i = disc.find_world("Paris, France").unwrap();
    let bytes = disc.read_world(i).unwrap();
    let w = World::load(&bytes).unwrap();
    let set = FontSet::from_world(&w).unwrap();
    assert_eq!(set.fonts.len(), 4);
    assert_eq!((set.sheet.width, set.sheet.height), (1024, 1024));
    for (k, f) in set.fonts.iter().enumerate() {
        assert_eq!(f.metrics.id as usize, k, "metric byte 3 is the font's number");
    }
    // The spec's example: the Cross picture, then ": New Game".
    let l = layout(&set, b"&2X&. : New Game", &TextStyle::default());
    assert_eq!(l.glyphs.len(), 9);
    assert_eq!(l.glyphs[0].font, 1);
    assert_eq!(l.glyphs[1].glyph.code, b':' as u16);
    assert!(l.glyphs.iter().all(|g| (g.rect.h - 41.0 * 0.5).abs() < 1e-4));
}

#[test]
fn text_tables() {
    let Some(disc) = open("text_tables") else { return };
    let t = TextTables::read(&disc.executable).unwrap();
    assert_eq!(t.prompts.len(), 33);
    assert_eq!(t.prompts[0].title, b"Paused");
    assert_eq!(t.prompts[0].responses.len(), 8);
    assert_eq!(t.prompts[23].responses.len(), 9);
    assert_eq!(t.prompts[1].responses, [resp::YES, resp::NO]);
    for p in [11, 16, 17] {
        assert_eq!(t.prompts[p].responses, [10, 11, 12, 17]);
        assert_eq!((t.prompts[p].title_scale, t.prompts[p].response_scale), (1.0, 0.7));
    }
    assert_eq!(t.prompts[31].responses, [24, 25, 26, 17]);
    assert_eq!(t.prompts[20].title.windows(10).filter(|w| w == b"^15&2a&.^.").count(), 1);
    // Layout 1 (vertical list) on 21 prompts; the yes/no questions and the sprint result have 0 (side by side).
    assert_eq!(t.prompts.iter().filter(|p| p.vertical()).count(), 21);
    let side: Vec<usize> = (0..33).filter(|&i| !t.prompts[i].vertical()).collect();
    assert_eq!(side, [1, 2, 4, 7, 14, 18, 25, 26, 27, 28, 29, 30]);
    assert_eq!(t.responses.len(), 27);
    // Run-time texts: quit, the slots, the two-state options.
    let empty: Vec<usize> = (0..27).filter(|&i| t.responses[i].is_empty()).collect();
    assert_eq!(empty, [9, 10, 11, 12, 13, 15, 16, 24, 25, 26]);
    assert_eq!(t.two_state.len(), 8);
    assert_eq!(t.two_state[0].1, b"Empty");
    assert_eq!(t.slot_format, b"%d%% - %s - %d:%02d");
    assert_eq!(t.timer_format, b"^10~b2a83d%d:%02d~.^.");
    assert_eq!(t.timer_tenths_format, b"^10~b2a83d0.%d~.^.");
    assert_eq!(t.start_mission, b"Press &2X&. to Start Mission!");
    assert_eq!((t.bentley.as_slice(), t.murray.as_slice()), (&b"Bentley"[..], &b"Murray"[..]));
    assert_eq!(t.episodes.len(), 5);
    assert_eq!(t.levels.len(), 46);
    assert_eq!(t.movie_titles.len(), 16);
    assert_eq!(t.movie_titles[0], b"The Secret Police File");
    assert_eq!(t.movie_titles[15], b"Credits");
    assert_eq!(t.movie_titles[14], b"The Thievius Raccoonus");
    let eps: Vec<&[u8]> = t.episodes.iter().map(|e| e.as_slice()).collect();
    assert_eq!(
        eps,
        [
            &b"Tide of Terror"[..],
            b"Sunset Snake Eyes",
            b"Vicious Voodoo",
            b"Fire in the Sky",
            b"The Cold Heart of Hate"
        ]
    );
    let hashes = [
        hash_strings(t.prompts.iter().map(|p| p.title.as_slice())),
        hash_strings(t.responses.iter().map(|r| r.as_slice())),
        hash_strings(t.two_state.iter().flat_map(|p| [p.0.as_slice(), p.1.as_slice()])),
        hash_strings(t.movie_titles.iter().map(|n| n.as_slice())),
        hash_strings(t.episodes.iter().map(|n| n.as_slice())),
        hash_strings([t.title.as_slice(), &t.press_start_new, &t.press_start_resume, &t.press_select].into_iter()),
    ];
    eprintln!("{hashes:#?}");
    assert_eq!(
        hashes,
        [
            "d53e44b6104951f3d844f31cce93b0d7b92ecb38fef01cb0228ab00ca51863f4",
            "be14c5f2d6e5320a0fe00f9d5d0b179d08cff2538109ed1537ebb7d4eb26c6e3",
            "41ef59947cd0f5b1d627ac4081e45ca4cf3b00d41874bf00fd0f7447cb72eb2d",
            "a3e010922ae682a7010c1234904639bb0ae318edb5a3e9a91e280413e6adb21e",
            "f713a70cd588412f2a90f40f36a95cf4d752cf74a7a550f6d547b3b56d8b5c39",
            "e876d2747f272a7a699a813a0da55321044cda0586a0d8fcc9bd98aa6ceea3ec",
        ]
    );
}

#[test]
fn placement_table() {
    let Some(disc) = open("placement_table") else { return };
    let p = read_placements(&disc.executable).unwrap();
    assert_eq!(p.len(), 37);
    let rec = |i: usize| (p[i].x, p[i].y, p[i].slide_from, p[i].against, p[i].against_edge);
    use Edge::*;
    assert_eq!(rec(3), (0.0, 20.0, Some(Top), None, None));
    assert_eq!(rec(4), (-18.0, 20.0, Some(Right), None, None));
    assert_eq!(rec(5), (-18.0, 20.0, Some(Right), Some(4), Some(Top)));
    assert_eq!(rec(6), (18.0, 20.0, Some(Left), None, None));
    assert_eq!(rec(7), (-18.0, 20.0, Some(Right), Some(5), Some(Top)));
    assert_eq!(rec(16), (18.0, -20.0, Some(Left), None, None));
    assert_eq!(rec(17), (18.0, -20.0, Some(Left), Some(16), Some(Bottom)));
    assert_eq!(rec(19), (-18.0, -20.0, Some(Right), None, None));
    assert_eq!(rec(25), (0.0, 0.0, None, None, None));
    assert_eq!(rec(26), (1.0, -20.0, Some(Left), None, None));
    assert_eq!(rec(27), (-1.0, -20.0, Some(Right), None, None));
    assert_eq!(rec(35), (18.0, -20.0, Some(Bottom), None, None));
}

/// Glyph cells are measured from the top of the upright picture (textures.md 2.6): on Paris, font 0's `'` has ink
/// in rows 6 to 12 of its cell, `.` in rows 29 to 32 and `T` in rows 7 to 32.
#[test]
fn glyph_cells_are_upright() {
    let Some(mut disc) = open("glyph_cells_are_upright") else { return };
    let paris = disc.worlds.iter().position(|w| w.name == "Paris, France").unwrap();
    let bytes = disc.read_world(paris).unwrap();
    let w = World::load(&bytes).unwrap();
    let f = read_font(&w, 0).unwrap();
    let line = f.metrics.line_height as usize;
    let width = f.image.width as usize;
    let ink_rows = |code: u8| -> (usize, usize) {
        let g = f.glyphs.iter().find(|g| g.code == code as u16).unwrap();
        let inked: Vec<usize> = (0..line)
            .filter(|&k| {
                let y = g.y as usize + k;
                (g.x as usize..(g.x + g.w) as usize).any(|x| f.image.pixels[(y * width + x) * 4 + 3] > 0)
            })
            .collect();
        (*inked.first().unwrap(), *inked.last().unwrap())
    };
    assert_eq!(ink_rows(b'\''), (6, 12));
    assert_eq!(ink_rows(b'.'), (29, 32));
    assert_eq!(ink_rows(b'T'), (7, 32));
}
