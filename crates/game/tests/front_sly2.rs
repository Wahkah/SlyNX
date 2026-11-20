//! The Sly 2 front end on the player's own Sly 2 disc (`SLYNX_DISC_SLY2`), on the renderer's null backend; skipped
//! when the disc is not set (level-flow.md 3 and 4):
//!
//! - the start-up: the boot world loads, the logo movie (`0x808`) plays and + skips it, the title shows;
//! - a new game in slot 1 from the title's slot page: the intro hub with task 1583 current;
//! - a save, then the title again: the slot is used, Continue is offered and loads it;
//! - the menu world opens the episode menu; starting episode 1 (never played) plays its opening movie, then its hub
//!   loads.
//!
//! With `SLYNX_TEST_OUT` set, the pages are rasterised on the CPU (`ui::raster`) to `sly2_front_title.png`,
//! `sly2_front_slots.png`, `sly2_front_main.png` and `sly2_front_episodes.png`.

use engine::game::systems::FrameInput;
use game::sly2::Sly2Session;
use harness::golden::{Game, require_disc};
use platform_api::Buttons;
use render::NullBackend;
use ui::draw::Texture;
use ui::sly2::front::{Page, SlotsFor};

type Session = Sly2Session<NullBackend, std::fs::File>;

fn frames(s: &mut Session, n: usize) {
    for _ in 0..n {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
    }
}

fn press(s: &mut Session, b: Buttons) {
    s.update(&FrameInput { pressed: b.0, buttons: b.0, ..Default::default() }, 1.0 / 60.0);
    // Past a page's show time (0.6 s), so the next press counts.
    frames(s, 45);
}

/// The frame's UI rasterised to `name` in `SLYNX_TEST_OUT`; returns the glyph quads drawn.
fn shot(s: &Session, name: &str) -> usize {
    let list = s.ui_draw_list(1280, 720);
    let glyphs = list.quads.iter().filter(|q| q.texture == Texture::Glyphs).count();
    if let (Some(dir), Some(sheet)) = (std::env::var_os("SLYNX_TEST_OUT"), s.font_sheet()) {
        let (w, h) = (1280u32, 720u32);
        let mut px = vec![0u8; (w * h * 4) as usize];
        for p in px.chunks_mut(4) {
            p.copy_from_slice(&[40, 60, 80, 255]);
        }
        let g = ui::font::GlyphSheet { width: sheet.width, height: sheet.height, pixels: sheet.pixels.clone() };
        ui::raster::rasterize(&list, &g, w, h, &mut px);
        let img = harness::image::Image::from_rgba(w, h, px).unwrap();
        harness::png::write_file(std::path::Path::new(&dir).join(name), &img).unwrap();
    }
    glyphs
}

#[test]
fn title_slots_continue_and_episode_menu() {
    let Some(path) = require_disc(Game::Sly2, "title_slots_continue_and_episode_menu") else { return };
    let disc = formats_sly2::Disc::open_path(&path).map_err(|e| e.to_string());
    let mut s: Session = Sly2Session::new(NullBackend::new(1280, 720), disc, 7);
    let (engine, _mixer) = audio::create(audio::Config::default());
    s.set_sound(engine, Box::new(std::fs::File::open(&path).unwrap()));
    // The slots in a fresh user folder.
    let user = std::env::temp_dir().join(format!("slynx-front-sly2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&user);
    std::fs::create_dir_all(&user).unwrap();
    let fs = platform_pc::PcFs::new(user.clone(), user.clone(), user.clone());
    let store = save::sly2::Sly2Store::new(fs);
    s.set_slots(Box::new(store.clone()));
    let movie_path = path.clone();
    s.set_movie_source(Box::new(move || {
        Some(Box::new(std::fs::File::open(&movie_path).ok()?) as Box<dyn formats::iso::ReadAt>)
    }));

    // Start-up: the boot world, the logo movie (+ skips it), the title.
    s.start_front().unwrap();
    assert_eq!(s.current().map(|c| c.row), Some(0), "the boot world is the backdrop");
    assert!(s.movie_playing(), "the logo movie plays");
    frames(&mut s, 10);
    press(&mut s, Buttons::PLUS);
    assert!(!s.movie_playing(), "+ skips the movie");
    assert_eq!(s.front_page(), Some(Page::Title));
    assert!(shot(&s, "sly2_front_title.png") > 20, "the title's texts");

    // Start: the main page (New Game only, no saves), then the new-game slots; slot 1 starts the game.
    press(&mut s, Buttons::PLUS);
    assert_eq!(s.front_page(), Some(Page::Main));
    press(&mut s, Buttons::B);
    assert_eq!(s.front_page(), Some(Page::Slots(SlotsFor::NewGame)));
    assert!(shot(&s, "sly2_front_slots.png") > 10);
    press(&mut s, Buttons::B);
    assert!(!s.front_open(), "the new game leaves the front end");
    assert_eq!(s.current().map(|c| c.row), Some(0));
    let current = |s: &Session| {
        let j = s.game.as_ref()?.world.sly2.as_deref()?.jobs.as_deref()?;
        j.current.map(|t| j.graph.tasks[t].id)
    };
    assert_eq!(current(&s), Some(1583), "a new game starts task 1583");
    frames(&mut s, 60);

    // Save to slot 1; the title again shows it used and offers Continue.
    s.save_game(&store, save::Slot::new(0).unwrap()).unwrap();
    s.open_title();
    frames(&mut s, 40);
    press(&mut s, Buttons::PLUS);
    assert_eq!(s.front_page(), Some(Page::Main));
    assert!(shot(&s, "sly2_front_main.png") > 10);
    press(&mut s, Buttons::B);
    assert!(!s.front_open(), "Continue loads the newest slot");
    assert_eq!(s.current().map(|c| c.row), Some(0), "episode 0 continues in the intro hub");

    // The menu world opens the episode menu; episode 1 has never been played, so its opening movie plays first.
    s.load_row(1).unwrap();
    frames(&mut s, 40);
    assert_eq!(s.front_page(), Some(Page::Episodes));
    assert!(shot(&s, "sly2_front_episodes.png") > 10);
    press(&mut s, Buttons::B);
    assert!(s.movie_playing(), "episode 1's opening movie (0x7D9)");
    press(&mut s, Buttons::PLUS);
    frames(&mut s, 5);
    assert_eq!(s.current().map(|c| c.row), Some(2), "episode 1's hub");
    let _ = std::fs::remove_dir_all(&user);
}
