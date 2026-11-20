//! The Sly 3 front end on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`), on the renderer's null backend; skipped
//! when the disc is not set (level-flow.md 3):
//!
//! - the start-up: the menu world (`dvd_menu`) loads, the two logo movies (338, 339) play and + skips each, the
//!   title shows;
//! - a new game in slot 1: episode 1 with the opening not played, so Kaine Island's `m_ext` with the prologue task;
//! - a save, then the title again: the slot is used and Load Game starts its episode;
//! - the menu world opens the episode menu, and choosing the first entry (the Hazard Room) starts episode 2.
//!
//! With `SLYNX_TEST_OUT` set, the pages are rasterised on the CPU (`ui::raster`) to `sly3_front_title.png`,
//! `sly3_front_slots.png` and `sly3_front_episodes.png`.

use engine::game::systems::FrameInput;
use game::sly3::Sly3Session;
use harness::golden::{Game, require_disc};
use platform_api::Buttons;
use render::NullBackend;
use ui::draw::Texture;
use ui::sly2::front::{Page, SlotsFor};

type Session = Sly3Session<NullBackend, std::fs::File>;

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
fn title_new_game_load_and_episode_menu() {
    let Some(path) = require_disc(Game::Sly3, "title_new_game_load_and_episode_menu") else { return };
    let disc = formats_sly3::Disc::open_path(&path).map_err(|e| e.to_string());
    let mut s: Session = Sly3Session::new(NullBackend::new(1280, 720), disc, 7);
    let user = std::env::temp_dir().join(format!("slynx-front-sly3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&user);
    std::fs::create_dir_all(&user).unwrap();
    let fs = platform_pc::PcFs::new(user.clone(), user.clone(), user.clone());
    s.set_slots(Box::new(save::sly3::store(fs)));
    let movie_path = path.clone();
    s.set_movie_source(Box::new(move || {
        Some(Box::new(std::fs::File::open(&movie_path).ok()?) as Box<dyn formats::iso::ReadAt>)
    }));

    s.start_front().unwrap();
    assert_eq!(s.current().map(|c| c.slot), Some(0), "the menu world is the backdrop");
    assert!(s.movie_playing(), "the first logo movie plays");
    press(&mut s, Buttons::PLUS);
    assert!(s.movie_playing(), "then the second");
    press(&mut s, Buttons::PLUS);
    assert!(!s.movie_playing());
    assert_eq!(s.front_page(), Some(Page::Title));
    assert!(shot(&s, "sly3_front_title.png") > 20, "the title's texts");

    // New game in slot 1: the prologue (episode 1, m_ext with task 1798).
    press(&mut s, Buttons::PLUS);
    press(&mut s, Buttons::B);
    assert_eq!(s.front_page(), Some(Page::Slots(SlotsFor::NewGame)));
    assert!(shot(&s, "sly3_front_slots.png") > 10);
    press(&mut s, Buttons::B);
    assert!(!s.front_open());
    assert_eq!(s.current().map(|c| c.slot), Some(game::sly3::flow::SLOT_M_EXT), "Kaine Island's prologue");
    assert_eq!(s.started_task, Some(game::sly3::flow::PROLOGUE_TASK));
    frames(&mut s, 30);

    // Save, then Load Game from the title starts the saved episode.
    s.save_to_slot().unwrap();
    s.open_title();
    frames(&mut s, 40);
    press(&mut s, Buttons::PLUS);
    assert_eq!(s.front_page(), Some(Page::Main));
    // Continue, New Game, Load Game: Load Game is the third.
    press(&mut s, Buttons::DOWN);
    press(&mut s, Buttons::DOWN);
    press(&mut s, Buttons::B);
    assert_eq!(s.front_page(), Some(Page::Slots(SlotsFor::Load)));
    press(&mut s, Buttons::B);
    assert!(!s.front_open(), "the load starts the saved episode");
    assert!(s.current().is_some());

    // The menu world: the episode menu; its first entry is the Hazard Room (episode 2).
    s.load_at(0, game::sly3::flow::DEFAULT_POINT).unwrap();
    frames(&mut s, 40);
    assert_eq!(s.front_page(), Some(Page::Episodes));
    assert!(shot(&s, "sly3_front_episodes.png") > 10);
    press(&mut s, Buttons::B);
    assert!(!s.front_open());
    assert_eq!(s.episode, 2, "the Hazard Room");
    let _ = std::fs::remove_dir_all(&user);
}
