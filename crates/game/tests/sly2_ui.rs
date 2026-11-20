//! The Sly 2 session's 2D layer on the player's own Sly 2 disc (`SLYNX_DISC_SLY2`), on the renderer's null backend;
//! skipped when the disc is not set. The fonts of object 884 make the glyph sheet, the level gets the 2D layer, and
//! the pause menu opened with Start draws its entries as text.
//!
//! With `SLYNX_TEST_OUT` set, the frame's UI is rasterised on the CPU (`ui::raster`, the reference of the overlay
//! pass) to `sly2_ui_pause.png`, and the glyph sheet to `sly2_font_sheet.png`, for a look.

use engine::game::systems::FrameInput;
use game::sly2::{HUB_WORLD_ID, Sly2Session};
use harness::golden::{Game, require_disc};
use platform_api::Buttons;
use render::NullBackend;
use ui::draw::Texture;

#[test]
fn the_pause_menu_draws_in_the_sly2_font() {
    let Some(path) = require_disc(Game::Sly2, "the_pause_menu_draws_in_the_sly2_font") else { return };
    let disc = formats_sly2::Disc::open_path(&path).map_err(|e| e.to_string());
    let mut s: Sly2Session<NullBackend, std::fs::File> = Sly2Session::new(NullBackend::new(1280, 720), disc, 7);
    // Sound too: the world's strings (item names) come with it.
    let (engine, _mixer) = audio::create(audio::Config::default());
    s.set_sound(engine, Box::new(std::fs::File::open(&path).unwrap()));
    let row = s.find_world(HUB_WORLD_ID).expect("hub row");
    s.load_row(row).unwrap();
    let sheet = s.font_sheet().expect("the fonts of object 884").clone();
    // The text font, the icon font and the gadget grids' icons (three states of 18 + 9 + 8 cells).
    assert_eq!(sheet.fonts.len(), 3);
    assert_eq!(sheet.fonts[2].glyphs.len(), 3 * (18 + 9 + 8), "every grid cell's icons");
    assert_eq!(sheet.fonts[0].glyphs.len(), 224);
    assert!(sheet.pixels.chunks(4).filter(|p| p[3] > 0).count() > 10_000, "glyphs rasterised");
    assert!(s.has_ui_layer(), "the level has the 2D layer");
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
    }
    // + opens the pause menu: the game pauses and its entries are drawn as glyphs.
    let start = FrameInput { pressed: Buttons::PLUS.0, buttons: Buttons::PLUS.0, ..Default::default() };
    s.update(&start, 1.0 / 60.0);
    for _ in 0..40 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
    }
    assert_eq!(s.game.as_ref().unwrap().clock.rate, 0.0, "the pause menu pauses the game");
    let list = s.ui_draw_list(1280, 720);
    let glyphs = list.quads.iter().filter(|q| q.texture == Texture::Glyphs).count();
    eprintln!("{} quads, {glyphs} glyphs", list.quads.len());
    assert!(glyphs >= 20, "the entries' text");
    if let Some(dir) = std::env::var_os("SLYNX_TEST_OUT") {
        let (w, h) = (1280u32, 720u32);
        let mut px = vec![0u8; (w * h * 4) as usize];
        for p in px.chunks_mut(4) {
            p.copy_from_slice(&[40, 60, 80, 255]);
        }
        let g = ui::font::GlyphSheet { width: sheet.width, height: sheet.height, pixels: sheet.pixels.clone() };
        ui::raster::rasterize(&list, &g, w, h, &mut px);
        let img = harness::image::Image::from_rgba(w, h, px).unwrap();
        let out = std::path::Path::new(&dir).join("sly2_ui_pause.png");
        harness::png::write_file(&out, &img).unwrap();
        eprintln!("wrote {}", out.display());
        let sheet_img = harness::image::Image::from_rgba(sheet.width, sheet.height, sheet.pixels.clone()).unwrap();
        harness::png::write_file(std::path::Path::new(&dir).join("sly2_font_sheet.png"), &sheet_img).unwrap();
        let q = list.quads.iter().find(|q| q.texture == Texture::Glyphs).unwrap();
        eprintln!("first glyph quad {:?}; sheet {}x{}", q, sheet.width, sheet.height);
    }
    // Triangle closes it again (below); first the other menus, each rasterised with `SLYNX_TEST_OUT`.
    let shot = |s: &Sly2Session<NullBackend, std::fs::File>, name: &str| {
        let list = s.ui_draw_list(1280, 720);
        let glyphs = list.quads.iter().filter(|q| q.texture == Texture::Glyphs).count();
        if let Some(dir) = std::env::var_os("SLYNX_TEST_OUT") {
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
    };
    let frames = |s: &mut Sly2Session<NullBackend, std::fs::File>, n: usize| {
        for _ in 0..n {
            s.update(&FrameInput::default(), 1.0 / 60.0);
        }
    };
    let press = |b: Buttons| FrameInput { pressed: b.0, buttons: b.0, ..Default::default() };
    // Close the pause menu, then Select (−) opens the gadget grid.
    s.update(&press(Buttons::X), 1.0 / 60.0);
    frames(&mut s, 45);
    s.update(&press(Buttons::MINUS), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(s.menus_open().0, "the gadget grid is open");
    assert!(shot(&s, "sly2_ui_grid.png") > 18 + 5, "the grid's icons and texts");
    s.update(&press(Buttons::MINUS), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(!s.menus_open().0);
    // The shop (in the hub): the Buy page lists the open items.
    s.open_shop();
    frames(&mut s, 45);
    let rows = s.menus_open().1.expect("the shop is open");
    eprintln!("shop rows {rows}");
    assert!(shot(&s, "sly2_ui_shop.png") > 10);
    s.update(&press(Buttons::A), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(s.menus_open().1.is_none(), "Circle closes the shop");
    // The options page.
    s.open_options();
    frames(&mut s, 45);
    assert!(s.menus_open().2);
    assert!(shot(&s, "sly2_ui_options.png") > 10);
    s.update(&press(Buttons::X), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(!s.menus_open().2);
    // Pause again for the last check.
    s.update(&press(Buttons::PLUS), 1.0 / 60.0);
    frames(&mut s, 45);
    // Triangle closes it again.
    let back = FrameInput { pressed: Buttons::X.0, buttons: Buttons::X.0, ..Default::default() };
    s.update(&back, 1.0 / 60.0);
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
    assert_eq!(s.game.as_ref().unwrap().clock.rate, 1.0);
}
