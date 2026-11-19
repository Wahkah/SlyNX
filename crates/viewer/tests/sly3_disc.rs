//! Sly 3 viewer tests on the player's disc image. They run when `SLYNX_DISC_SLY3` names the image and skip
//! otherwise.

use harness::golden::{Game, require_disc};
use render::NullBackend;
use viewer::camera::Controls;
use viewer::loader::Blocking;
use viewer::session::Session;

#[test]
fn every_sly3_world_prepares_and_renders() {
    let Some(path) = require_disc(Game::Sly3, "every_sly3_world_prepares_and_renders") else { return };
    let mut disc = viewer::pc::open_disc(path.to_str().unwrap()).unwrap();
    assert_eq!(disc.game(), 3);
    let names = disc.world_names();
    assert_eq!(names.len(), 39);
    assert_eq!(disc.world_index("v_apt"), Some(4));
    assert_eq!(disc.world_index("slot5"), Some(4));
    assert_eq!(disc.world_index("slot2"), Some(1), "slot 1 is empty, so slot 2 is second");
    for (i, name) in names.iter().enumerate() {
        let level = disc.load(i).unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = &level.stats;
        // The menu world's static meshes are all hidden or outlines: it draws nothing.
        assert_eq!(s.draws == 0, name.starts_with("dvd_menu"), "{name}: {} draws", s.draws);
        assert_eq!(s.draws as usize, level.draws.len());
        assert_eq!(s.opaque + s.blended + s.backdrops + s.volumes + s.post, s.draws);
        assert!(level.draws.iter().all(|d| (d.mesh as usize) < level.meshes.len()));
        assert!(level.draws.iter().all(|d| (d.material as usize) < level.materials.len()));
        assert!(level.materials.iter().all(|m| (m.texture as usize) < level.textures.len()));
        assert!(s.draws < 1 << 15, "{name}: {} draws, more than the Switch build's instance slots", s.draws);
    }
    // One frame of every world, headless.
    let mut s = Session::new(NullBackend::new(1280, 720), Ok(disc)).unwrap();
    for (i, name) in names.iter().enumerate() {
        s.load_now(&mut Blocking, i).unwrap_or_else(|e| panic!("{name}: {e}"));
        let stats = s.render(16_666_667).unwrap_or_else(|e| panic!("{name}: {e}"));
        if !name.starts_with("dvd_menu") {
            assert!(stats.visible > 0, "{name}: {stats:?}");
        }
    }
}

#[test]
fn sly3_apartment_renders_and_switches_worlds() {
    let Some(path) = require_disc(Game::Sly3, "sly3_apartment_renders_and_switches_worlds") else { return };
    let disc = viewer::pc::open_disc(path.to_str().unwrap()).unwrap();
    let apt = disc.world_index("v_apt").unwrap();
    let mut s = Session::new(NullBackend::new(1280, 720), Ok(disc)).unwrap();
    s.load_now(&mut Blocking, apt).unwrap();
    assert!(s.current().unwrap().name.contains("v_apt"));
    let stats = s.render(16_666_667).unwrap();
    assert!(stats.visible > 100, "{stats:?}");
    let next = Controls { next_world: true, ..Default::default() };
    s.update(&mut Blocking, &next, 1.0 / 60.0).unwrap();
    assert_eq!(s.current().unwrap().index, apt + 1);
}
