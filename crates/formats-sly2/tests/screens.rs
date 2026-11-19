//! Screen movies on the player's own Sly 2 (USA) disc (`SLYNX_DISC_SLY2`); skipped when it is not set. Every class
//! 126 to 131 body of every world reads to its end, every streamed screen's file reads to its end, and the fonts of
//! screen object 884 are as user-interface.md 4.2 describes them.

use formats_sly2::screen::{ItemBody, Screen};
use formats_sly2::stream::{Record, visit};
use formats_sly2::{Disc, World};

#[test]
fn every_screen_reads_to_its_end() {
    let Some(path) = std::env::var_os("SLYNX_DISC_SLY2").filter(|p| !p.is_empty()) else { return };
    let mut d = Disc::open_path(path).expect("disc");
    let rows: Vec<usize> = d.world_rows().collect();
    let (mut bodies, mut streamed, mut texts) = (0, 0, 0);
    for row in rows {
        let bytes = d.read_world(row).expect("world");
        let w = World::parse(&bytes, &d.options).expect("parse");
        let mut found: Vec<(i32, i16, Screen)> = Vec::new();
        for b in &w.bodies {
            visit(&b.objects, &mut |r: &Record| {
                if !(126..=131).contains(&r.class) {
                    return;
                }
                let start = r.options().expect("options").end;
                let end = (r.body_offset + r.body_size) as usize;
                let s = Screen::parse(&bytes, start, end).unwrap_or_else(|e| panic!("row {row} object {}: {e}", r.id));
                found.push((r.id, r.class, s));
            });
        }
        for (id, _, s) in &mut found {
            bodies += 1;
            if let Some(f) = s.file {
                let file = d.read_file_decompressed(&f).expect("streamed file");
                s.parse_streamed(&file).unwrap_or_else(|e| panic!("row {row} streamed object {id}: {e}"));
                streamed += 1;
            }
            texts += s.strings().count();
        }
        if row == 0 {
            // The fonts: two item-kind-8 sets in object 884 (user-interface.md 4.2).
            let fonts = found.iter().find(|f| f.0 == 884).expect("object 884");
            let fonts: Vec<_> = fonts.2.fonts().collect();
            assert_eq!(fonts.len(), 2);
            for f in fonts {
                assert_eq!(f.glyphs.len(), f.chars.len());
                assert_eq!(f.chars.len(), f.advances.len());
                assert!(f.chars.windows(2).all(|w| w[0] < w[1]), "sorted characters");
                let (mut lo, mut hi) = ([i16::MAX; 2], [i16::MIN; 2]);
                for g in &f.glyphs {
                    if let ItemBody::Shape(p) = &g.body {
                        for v in p.iter().flat_map(|p| &p.vertices) {
                            lo = [lo[0].min(v.x), lo[1].min(v.y)];
                            hi = [hi[0].max(v.x), hi[1].max(v.y)];
                        }
                    }
                }
                let chars: String = f.chars.iter().filter_map(|&c| char::from_u32(c)).collect();
                let adv = f.advances.iter().fold((f32::MAX, f32::MIN), |a, &v| (a.0.min(v), a.1.max(v)));
                eprintln!(
                    "font: {} glyphs {chars:?}; x {}..{}, y {}..{}; advances {adv:?}",
                    f.chars.len(),
                    lo[0],
                    hi[0],
                    lo[1],
                    hi[1]
                );
            }
        }
    }
    eprintln!("{bodies} screen bodies, {streamed} streamed, {texts} text items with strings");
    assert_eq!(bodies, 1659 + 41 + 41 + 212 + 43 + 1, "user-interface.md 1.1");
    assert_eq!(streamed, 258, "user-interface.md 1.3");
}
