//! The Sly 3 session's 2D layer on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`), on the renderer's null backend;
//! skipped when the disc is not set. The fonts of object 1061 make the glyph sheet, the level gets the 2D layer, and
//! the pause menu opened with Start draws its entries as text.
//!
//! With `SLYNX_TEST_OUT` set, the frame's UI is rasterised on the CPU (`ui::raster`, the reference of the overlay
//! pass) to `sly3_ui_pause.png`, and the glyph sheet to `sly3_font_sheet.png`, for a look.

use engine::game::systems::FrameInput;
use game::sly3::{FIRST_WORLD, Sly3Session};
use harness::golden::{Game, require_disc};
use platform_api::Buttons;
use render::NullBackend;
use ui::draw::Texture;

#[test]
fn the_pause_menu_draws_in_the_sly3_font() {
    let Some(path) = require_disc(Game::Sly3, "the_pause_menu_draws_in_the_sly3_font") else { return };
    let disc = formats_sly3::Disc::open_path(&path).map_err(|e| e.to_string());
    let mut s: Sly3Session<NullBackend, std::fs::File> = Sly3Session::new(NullBackend::new(1280, 720), disc, 7);
    let slot = s.find_world(FIRST_WORLD).expect("v_apt");
    s.load_slot(slot).unwrap();
    let sheet = s.font_sheet().expect("the fonts of object 1061").clone();
    eprintln!(
        "fonts: {:?} glyphs, sheet {}x{}",
        sheet.fonts.iter().map(|f| f.glyphs.len()).collect::<Vec<_>>(),
        sheet.width,
        sheet.height
    );
    assert!(!sheet.fonts.is_empty());
    assert!(sheet.fonts[0].glyphs.contains_key(&(b'A' as u32)), "the text font has letters");
    assert!(sheet.pixels.chunks(4).filter(|p| p[3] > 0).count() > 10_000, "glyphs rasterised");
    assert!(s.has_ui_layer(), "the level has the 2D layer");
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
    }
    let start = FrameInput { pressed: Buttons::PLUS.0, buttons: Buttons::PLUS.0, ..Default::default() };
    s.update(&start, 1.0 / 60.0);
    for _ in 0..40 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
    }
    assert!(s.screens_hold(), "the pause menu is open");
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
        harness::png::write_file(std::path::Path::new(&dir).join("sly3_ui_pause.png"), &img).unwrap();
        let sheet_img = harness::image::Image::from_rgba(sheet.width, sheet.height, sheet.pixels.clone()).unwrap();
        harness::png::write_file(std::path::Path::new(&dir).join("sly3_font_sheet.png"), &sheet_img).unwrap();
    }
    let shot = |s: &Sly3Session<NullBackend, std::fs::File>, name: &str| {
        let list = s.ui_draw_list(1280, 720);
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
        list.quads.iter().filter(|q| q.texture == Texture::Glyphs).count()
    };
    let frames = |s: &mut Sly3Session<NullBackend, std::fs::File>, n: usize| {
        for _ in 0..n {
            s.update(&FrameInput::default(), 1.0 / 60.0);
        }
    };
    let press = |b: Buttons| FrameInput { pressed: b.0, buttons: b.0, ..Default::default() };
    s.update(&press(Buttons::X), 1.0 / 60.0);
    frames(&mut s, 45);
    // The safe house's shop, as its computer opens it: the items of episodes up to 8 with their prices.
    let exe = formats_sly3::Disc::open_path(&path).unwrap().executable;
    let table = engine::gang::sly3::Sly3ItemTable::from_executable(&exe).unwrap();
    s.shop = Some(game::sly3::safehouse::Shop::new(&table, 8, &[]));
    frames(&mut s, 45);
    let rows = s.menus_open().0.expect("the shop screen is open");
    eprintln!("shop rows {rows}");
    assert!(rows > 0 && shot(&s, "sly3_ui_shop.png") > 10);
    s.update(&press(Buttons::A), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(s.menus_open().0.is_none() && s.shop.is_none(), "Circle closes the shop");
    // The options page.
    s.open_options();
    frames(&mut s, 45);
    assert!(s.menus_open().1 && shot(&s, "sly3_ui_options.png") > 10);
    s.update(&press(Buttons::X), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(!s.menus_open().1);
    // Job failed, as a script shows screen 1067; Cross dismisses it.
    s.screen_request(1067, true);
    frames(&mut s, 45);
    assert!(s.menus_open().2 && shot(&s, "sly3_ui_failed.png") > 10);
    s.update(&press(Buttons::B), 1.0 / 60.0);
    assert!(!s.menus_open().2);
    // The gadget grid (Select): every item owned, the current character's grid opens with a cell selected, and L
    // puts the selected item on L1 (written to the progress block's slots too).
    for item in 0..table.items.len() {
        s.set_owned(item, true);
    }
    s.update(&press(Buttons::MINUS), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(s.grid_open(), "Select opens the gadget grid");
    let view = s.grid_view().expect("the grid's view");
    eprintln!("grid: {} cells, selected {}", view.cells.len(), view.selected);
    assert!(view.cells.len() >= 8 && view.cells.iter().all(|c| c.owned));
    // The icons from the grid screen's own art (the sheet's third font) and the title's place.
    assert!(view.cells.iter().all(|c| c.icon.is_some()), "every cell has its icon");
    assert!(view.title_at.is_some());
    assert_eq!(s.font_sheet().unwrap().fonts.len(), 3);
    assert!(shot(&s, "sly3_ui_grid.png") > 10, "the grid's texts");
    // Move to an assignable cell first if the selected one is not.
    let target = view.cells.iter().position(|c| c.assignable).expect("an assignable cell");
    if target != view.selected {
        // Walk right until it is selected (at most the grid's size).
        for _ in 0..view.cells.len() {
            if s.grid_view().unwrap().cells[s.grid_view().unwrap().selected].assignable {
                break;
            }
            s.update(&press(Buttons::RIGHT), 1.0 / 60.0);
            frames(&mut s, 2);
        }
    }
    s.update(&press(Buttons::L), 1.0 / 60.0);
    frames(&mut s, 2);
    let slots = s.player_slots().expect("the crew's slots");
    assert!(slots[0].is_some(), "L puts the item on L1");
    let v = s.grid_view().unwrap();
    assert!(v.slots[0].is_some());
    s.update(&press(Buttons::MINUS), 1.0 / 60.0);
    frames(&mut s, 45);
    assert!(!s.grid_open(), "Select closes it");
    let back = FrameInput { pressed: Buttons::X.0, buttons: Buttons::X.0, ..Default::default() };
    s.update(&back, 1.0 / 60.0);
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
    assert!(!s.screens_hold());
    assert_eq!(s.game.as_ref().unwrap().clock.rate, 1.0);
}
