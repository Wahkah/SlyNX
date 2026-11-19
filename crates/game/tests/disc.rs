//! The game session on the player's own disc (`SLYNX_DISC_SLY1`), on the renderer's null backend; skipped when the
//! disc is not set.

use engine::game::systems::FrameInput;
use engine::world::Request;
use game::pc::open_disc;
use game::session::{Session, SessionOptions};
use harness::golden::{Game, require_disc};
use render::NullBackend;

fn session(path: &std::path::Path) -> Session<NullBackend, std::fs::File> {
    session_with(path, SessionOptions::default())
}

fn session_with(path: &std::path::Path, options: SessionOptions) -> Session<NullBackend, std::fs::File> {
    let disc = open_disc(path.to_str().unwrap());
    Session::new(NullBackend::new(1280, 720), disc, options)
}

#[test]
fn every_world_loads_on_the_renderer() {
    let Some(path) = require_disc(Game::Sly1, "every_world_loads_on_the_renderer") else { return };
    // The camera does not stand in for the player here, so no exit or volume moves the session elsewhere.
    let options = SessionOptions { camera_triggers: false, camera_collects: false, ..Default::default() };
    let mut s = session_with(&path, options);
    for i in 0..s.world_names().len() {
        s.load_world(i, None, 0).unwrap_or_else(|e| panic!("world {i}: {e}"));
        for _ in 0..3 {
            s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
            s.render(16.7).unwrap();
        }
        let c = s.current().unwrap();
        assert_eq!(c.index, i);
        assert!(c.stats.instances > 0, "{}: nothing drawn", c.name);
    }
}

#[test]
fn paris_runs_deterministically() {
    let Some(path) = require_disc(Game::Sly1, "paris_runs_deterministically") else { return };
    let run = || {
        let mut s = session(&path);
        let i = s.find_world("Paris").unwrap();
        s.load_world(i, None, 0).unwrap();
        let input = FrameInput { left: [0.2, 1.0], right: [0.3, 0.0], ..Default::default() };
        (0..240)
            .map(|_| {
                s.update(&input, 1.0 / 60.0).unwrap();
                s.render(16.7).unwrap();
                s.state_hash()
            })
            .collect::<Vec<u64>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn requests_change_worlds() {
    let Some(path) = require_disc(Game::Sly1, "requests_change_worlds") else { return };
    let mut s = session(&path);
    // Splash's exit leads to Paris at warp 445 (level-flow.md 9).
    s.load_world(0, None, 0).unwrap();
    let paris = s.find_world("Paris").unwrap();
    let identity = {
        let g = s.game.as_ref().unwrap();
        game::session::default_exit(&g.world).expect("Splash has a default exit").0
    };
    assert_eq!(s.world_of_identity(identity), Some(paris));
    s.game.as_mut().unwrap().world.request(Request::DefaultExit);
    // The wipe closes before the world changes (level-flow.md 8).
    for _ in 0..600 {
        s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
        if s.current().unwrap().index == paris {
            break;
        }
    }
    assert_eq!(s.current().unwrap().index, paris);
    // Progress carries over: Paris was visited.
    s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    let g = s.game.as_ref().unwrap();
    assert!(g.progress.has_flag(0x003, engine::game::progress::level_flag::VISITED));
    assert!(g.progress.has_flag(0x002, engine::game::progress::level_flag::VISITED));
}

#[test]
fn movies_play_hold_the_world_and_skip() {
    let Some(path) = require_disc(Game::Sly1, "movies_play_hold_the_world_and_skip") else { return };
    let mut s = session_with(&path, SessionOptions { wipes: false, ..Default::default() });
    let file = path.clone();
    s.set_movie_source(Box::new(move || {
        Some(Box::new(std::fs::File::open(&file).ok()?) as Box<dyn formats::iso::ReadAt>)
    }));
    let exe = open_disc(path.to_str().unwrap()).unwrap().executable;
    let logo = video::movies::movie_table(&exe).unwrap()[0].identity();
    s.load_world(s.find_world("Paris").unwrap(), None, 0).unwrap();
    // An unknown movie is dropped and the game goes on.
    s.game.as_mut().unwrap().world.request(Request::Movie(0xDEAD_BEEF));
    s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    assert!(!s.movie_playing());
    s.game.as_mut().unwrap().world.request(Request::Movie(logo));
    s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    assert!(s.movie_playing());
    let t = s.game.as_ref().unwrap().clock.game_time;
    for _ in 0..60 {
        s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    }
    assert!(s.movies.as_ref().unwrap().frame().is_some(), "frames decode on time");
    // The frame goes to the renderer's video layer (letterboxed, black bars around it).
    s.render(16.0).unwrap();
    assert_eq!(s.game.as_ref().unwrap().clock.game_time, t, "the world waits");
    let skip = FrameInput { pressed: platform_api::Buttons::PLUS.0, ..Default::default() };
    s.update(&skip, 1.0 / 60.0).unwrap();
    assert!(!s.movie_playing());
    s.render(16.0).unwrap();
    s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    assert!(s.game.as_ref().unwrap().clock.game_time > t, "the world resumes");
    // Without a movie source the request is dropped.
    let mut quiet = session_with(&path, SessionOptions { wipes: false, ..Default::default() });
    quiet.load_world(1, None, 0).unwrap();
    quiet.game.as_mut().unwrap().world.request(Request::Movie(logo));
    quiet.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    assert!(!quiet.movie_playing());
}

#[test]
fn saves_follow_the_game_and_continue_at_the_title() {
    let Some(path) = require_disc(Game::Sly1, "saves_follow_the_game_and_continue_at_the_title") else { return };
    let dir = std::env::temp_dir().join(format!("slynx-save-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fs = || platform_pc::PcFs::new(dir.join("romfs"), dir.join("sd"), dir.join("user"));
    let options = SessionOptions { wipes: false, ..Default::default() };
    let mut s = session_with(&path, options);
    s.set_saves(Box::new(save::SaveStore::new(fs())));
    // A new game in slot 2: Paris is saved as the resume point when it loads.
    s.saves.as_mut().unwrap().new_game(save::Slot::new(1));
    let paris = s.find_world("Paris").unwrap();
    s.load_world(paris, None, 0).unwrap();
    let id = s.game.as_ref().unwrap().world.info.id;
    {
        let g = s.game.as_mut().unwrap();
        g.progress.collect_key(id);
        g.progress.coins = 12;
    }
    s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    // The first auto-save of a new game waits for the auto-save notice (prompt 20).
    let store = save::SaveStore::new(fs());
    assert!(matches!(store.load(save::Slot::new(1).unwrap()), Err(save::LoadError::Empty)));
    assert!(s.frontend.as_ref().is_some_and(|f| f.menu_open()), "the notice is up");
    let accept =
        FrameInput { pressed: platform_api::Buttons::A.0, buttons: platform_api::Buttons::A.0, ..Default::default() };
    for _ in 0..30 {
        s.update(&accept, 1.0 / 60.0).unwrap();
        s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
        if store.load(save::Slot::new(1).unwrap()).is_ok() {
            break;
        }
    }
    let saved = store.load(save::Slot::new(1).unwrap()).unwrap().state;
    assert_eq!(game::saving::world_id(saved.episode, saved.level), id);
    assert!(saved.level(saved.episode, saved.level).flags & 2 != 0, "the key auto-saved");
    assert_eq!(saved.coins, 12);
    // A fresh session: the title loads the last slot, Start continues in Paris with the key.
    let mut t = session_with(&path, options);
    t.set_saves(Box::new(save::SaveStore::new(fs())));
    t.start_title().unwrap();
    assert!(t.frontend.as_ref().is_none_or(|f| f.menus.save_loaded));
    assert!(t.saves.as_ref().unwrap().loaded);
    t.continue_save();
    assert_eq!(t.current().unwrap().index, paris);
    let g = t.game.as_ref().unwrap();
    assert!(g.progress.has_flag(id, engine::game::progress::level_flag::KEY));
    assert_eq!(g.progress.coins, 12);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn movie_sound_goes_to_the_pcm_stream_and_clocks_the_frames() {
    let Some(path) = require_disc(Game::Sly1, "movie_sound_goes_to_the_pcm_stream_and_clocks_the_frames") else {
        return;
    };
    let mut s = session_with(&path, SessionOptions { wipes: false, ..Default::default() });
    let (engine, mut mixer) = audio::create(audio::Config::default());
    s.sound = Some(game::sound::Sound::new(engine));
    let file = path.clone();
    s.set_movie_source(Box::new(move || {
        Some(Box::new(std::fs::File::open(&file).ok()?) as Box<dyn formats::iso::ReadAt>)
    }));
    s.load_world(s.find_world("Paris").unwrap(), None, 0).unwrap();
    s.start_movie(game::session::STARTUP_MOVIE);
    assert!(s.movies.as_ref().unwrap().has_audio());
    // Without the mixer running, the sound clock stands still and so do the frames.
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    }
    assert!(s.movies.as_ref().unwrap().frames_shown() <= 1, "no sound mixed, only the first frame is due");
    // The mixer pulls 800 stereo frames per 1/60 s: frames follow.
    let mut out = vec![0.0f32; 800 * 2];
    for _ in 0..60 {
        mixer.render(&mut out);
        s.update(&FrameInput::default(), 1.0 / 60.0).unwrap();
    }
    let shown = s.movies.as_ref().unwrap().frames_shown();
    assert!(shown > 20, "frames follow the sound clock: {shown}");
    let skip = FrameInput { pressed: platform_api::Buttons::PLUS.0, ..Default::default() };
    s.update(&skip, 1.0 / 60.0).unwrap();
    assert!(!s.movie_playing());
}
