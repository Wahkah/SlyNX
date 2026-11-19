//! Viewer tests on the player's disc images. They run when `SLYNX_DISC_SLY1` (Sly 1) or `SLYNX_DISC_SLY2` (Sly 2)
//! names the image and skip otherwise.

use harness::golden::{Game, require_disc};
use render::NullBackend;
use viewer::camera::Controls;
use viewer::loader::{Blocking, GameDisc, load_world};
use viewer::session::Session;
use viewer::world_index;

#[test]
fn every_world_prepares() {
    let Some(path) = require_disc(Game::Sly1, "every_world_prepares") else { return };
    let mut disc = formats::disc::Disc::open_path(path).unwrap();
    for i in 0..disc.worlds.len() {
        let level = load_world(&mut disc, i).unwrap_or_else(|e| panic!("world {i}: {e}"));
        let s = &level.stats;
        assert!(s.draws > 0, "world {i} draws nothing");
        assert_eq!(s.draws as usize, level.draws.len());
        assert_eq!(s.opaque + s.blended + s.backdrops + s.volumes + s.post, s.draws);
        assert!(level.draws.iter().all(|d| (d.mesh as usize) < level.meshes.len()));
        assert!(level.draws.iter().all(|d| (d.material as usize) < level.materials.len()));
        assert!(level.materials.iter().all(|m| (m.texture as usize) < level.textures.len()));
        assert!(s.draws < 1 << 15, "world {i}: {} draws, more than the Switch build's instance slots", s.draws);
    }
}

#[test]
fn paris_renders_and_switches_worlds() {
    let Some(path) = require_disc(Game::Sly1, "paris_renders_and_switches_worlds") else { return };
    let disc = formats::disc::Disc::open_path(path).unwrap();
    let paris = world_index(&disc.worlds, "paris").unwrap();
    let mut s = Session::new(NullBackend::new(1280, 720), Ok(GameDisc::from(disc))).unwrap();
    s.load_now(&mut Blocking, paris).unwrap();
    let cur = s.current().unwrap().clone();
    assert_eq!(cur.name, "Paris, France");
    let stats = s.render(16_666_667).unwrap();
    assert!(stats.visible > 500, "{stats:?}");
    assert!(stats.draw_commands < stats.visible);

    // R: the next world loads (synchronously with the blocking host) and the camera moves to its spawn.
    let next = Controls { next_world: true, ..Default::default() };
    s.update(&mut Blocking, &next, 1.0 / 60.0).unwrap();
    assert_eq!(s.current().unwrap().index, paris + 1);
    assert_eq!(s.camera.pos, s.current().unwrap().spawn.eye);
    let prev = Controls { prev_world: true, ..Default::default() };
    s.update(&mut Blocking, &prev, 1.0 / 60.0).unwrap();
    assert_eq!(s.current().unwrap().index, paris);
}

#[test]
fn paris_player_is_animated_with_skinning() {
    let Some(path) = require_disc(Game::Sly1, "paris_player_is_animated_with_skinning") else { return };
    let mut disc = formats::disc::Disc::open_path(path).unwrap();
    let paris = world_index(&disc.worlds, "paris").unwrap();
    let level = load_world(&mut disc, paris).unwrap();
    let mut anim = level.player.clone().expect("Paris has the player character and its segment");
    assert!(!anim.skins.is_empty());
    let skinned: Vec<_> = anim.parts.iter().filter(|p| p.skin.is_some()).collect();
    assert!(!skinned.is_empty(), "no skinned part");
    assert!(skinned.iter().all(|p| level.meshes[level.draws[p.draw as usize].mesh as usize].skinned));
    // The rig moves under the animation.
    let rest = anim.rig.world.clone();
    anim.advance(0.5);
    assert!(anim.rig.world.iter().zip(&rest).any(|(a, b)| (0..3).any(|k| (a.t[k] - b.t[k]).abs() > 0.5)));

    // Through the renderer: skinned instances get bone matrices and stay in the same draws as rigid ones.
    let mut r = render::Renderer::new(NullBackend::new(1280, 720));
    let up = viewer::level::upload(&mut r, Some(&level), 0).unwrap();
    anim.apply(&mut r, &up.instances).unwrap();
    let mut cam = viewer::camera::FreeCamera::default();
    cam.place(level.spawn);
    let s = r.render(&cam.camera(), [0.0; 4]).unwrap();
    let nb = r.backend();
    assert!(!nb.last_bones.is_empty());
    let skinned_records = nb.last_instances.iter().filter(|i| i.flags() & render::frame::INSTANCE_SKINNED != 0).count();
    assert!(skinned_records > 0, "no skinned instance visible from the spawn point");
    assert!(s.draw_commands < s.visible);
}

#[test]
fn every_sly2_world_prepares() {
    let Some(path) = require_disc(Game::Sly2, "every_sly2_world_prepares") else { return };
    let mut disc = viewer::pc::open_disc(path.to_str().unwrap()).unwrap();
    assert_eq!(disc.game(), 2);
    let names = disc.world_names();
    assert_eq!(names.len(), 43);
    assert_eq!(disc.world_index("0x3B"), Some(3));
    assert_eq!(disc.world_index("row3"), Some(3));
    assert_eq!(disc.world_index("row19"), Some(18), "row 18 is empty, so row 19 is 18th");
    for (i, name) in names.iter().enumerate() {
        let level = disc.load(i).unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = &level.stats;
        assert!(s.draws + s.static_pieces > 0, "{} draws nothing", name);
        assert!(level.extras.static_groups.iter().all(|g| !g.pieces.is_empty()));
        assert_eq!(s.draws as usize, level.draws.len());
        assert_eq!(s.opaque + s.blended + s.backdrops + s.volumes + s.post, s.draws);
        assert!(level.draws.iter().all(|d| (d.mesh as usize) < level.meshes.len()));
        assert!(level.draws.iter().all(|d| (d.material as usize) < level.materials.len()));
        assert!(level.materials.iter().all(|m| (m.texture as usize) < level.textures.len()));
        assert!(s.draws < 1 << 15, "{}: {} draws, more than the Switch build's instance slots", name, s.draws);
    }
}

#[test]
fn sly2_cellar_renders_and_switches_worlds() {
    let Some(path) = require_disc(Game::Sly2, "sly2_cellar_renders_and_switches_worlds") else { return };
    let disc = viewer::pc::open_disc(path.to_str().unwrap()).unwrap();
    let cellar = disc.world_index("0x3B").unwrap();
    let mut s = Session::new(NullBackend::new(1280, 720), Ok(disc)).unwrap();
    s.load_now(&mut Blocking, cellar).unwrap();
    assert!(s.current().unwrap().name.contains("Row 3"));
    let stats = s.render(16_666_667).unwrap();
    assert!(stats.visible > 100, "{stats:?}");
    let next = Controls { next_world: true, ..Default::default() };
    s.update(&mut Blocking, &next, 1.0 / 60.0).unwrap();
    assert_eq!(s.current().unwrap().index, cellar + 1);
}

#[test]
fn screen_mask_shape() {
    let Some(path) = require_disc(Game::Sly1, "screen_mask_shape") else { return };
    let mut disc = formats::disc::Disc::open_path(path).unwrap();
    let paris = world_index(&disc.worlds, "paris").unwrap();
    let bytes = disc.read_world(paris).unwrap();
    let world = formats::world::World::load(&bytes).unwrap();
    // level-flow.md 5: 127 points, bounds -35 to 35, five sets of 47, 53, 22, 22 and 62 triangles.
    let m = viewer::iris::ScreenMask::of_world(&world).expect("Paris has a screen mask");
    assert_eq!(m.points.len(), 127);
    assert_eq!(m.bounds, [-35.0, -35.0, 35.0, 35.0]);
    assert_eq!(m.sets.iter().map(Vec::len).collect::<Vec<_>>(), [47, 53, 22, 22, 62]);
    // classes/paths-and-maps.md 5: set 0 dark blue (shader 1166), 1 and 2 blue, 3 and 4 white.
    let c = viewer::iris::ScreenMask::colors_of_world(&world);
    let (dark, blue, white) = ([0x07, 0x1e, 0x98, 0xbf], [0x00, 0x51, 0xde, 0x7f], [0xff, 0xff, 0xff, 0x7f]);
    assert_eq!(c, [dark, blue, blue, white, white]);
}
