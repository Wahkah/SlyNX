//! The game, platform-independent: the player's disc, the running world, the renderer, the sound, the HUD and menus,
//! and moving between worlds.
//!
//! Each frame: the engine runs one frame (with the pause menu open, its game clock stops), the renderer gets the
//! changed drawables, the camera, fog and darkening, the sound engine gets the audio frame, the frontend draws the
//! HUD, subtitles, menus and wipes, and the requests the world made are handled: another world through a wipe, the
//! default exit, a respawn at the last checkpoint, game over, a movie. Worlds load synchronously while the screen is
//! black. With saving set up ([`Session::set_saves`]) the progress goes to the slot in use as the game plays
//! ([`crate::saving`]); with a movie source ([`Session::set_movie_source`]) movies play while the world waits
//! ([`crate::movie`]). In worlds without Sly, the free camera stands in for the player (the world's probe).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use engine::game::difficulty::DifficultyTables;
use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::object::class::id;
use engine::world::{LevelData, Request, World, WorldInfo};
use formats::disc::Disc;
use formats::iso::ReadAt;
use formats::scene::{Scene, SceneOptions};
use render::math::Vec3;
use render::overlay::TextOverlay;
use render::{Backend, Camera, Fog, FrameStats, RenderError, Renderer};
use script::OptionMap;

use crate::frontend::{FrontAction, Frontend, Wipe, wipe_times};
use crate::level::{LevelGpu, LevelStats, upload};
use crate::movie::{Movies, Opener};
use crate::saving::{Saves, SlotFiles};
use crate::sound::Sound;
use crate::ui_layer::UiLayer;

/// The start-up logo movie's identity (level-flow.md 6).
pub const STARTUP_MOVIE: u32 = 0x344;

/// Characters of the on-screen status lines.
pub const OVERLAY_CHARS: usize = 240;

/// How the session behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionOptions {
    /// The free camera triggers player-only objects (exits, volumes, checkpoints) in worlds without a player.
    pub camera_triggers: bool,
    /// The free camera collects pickups in worlds without a player.
    pub camera_collects: bool,
    /// Draw the debug status lines.
    pub overlay: bool,
    /// Seed of the game's random numbers.
    pub seed: u64,
    /// Screen wipes between worlds (off: worlds change within the frame that asks).
    pub wipes: bool,
    /// The HUD and menus (needs the executable's tables).
    pub frontend: bool,
    /// The follow camera (`engine::camera`); off: the free camera everywhere.
    pub follow_camera: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            camera_triggers: true,
            camera_collects: true,
            overlay: false,
            seed: 1,
            wipes: true,
            frontend: true,
            follow_camera: true,
        }
    }
}

/// The current world.
pub struct Current {
    /// Index in the world table.
    pub index: usize,
    /// Its name.
    pub name: String,
    /// What the upload made.
    pub stats: LevelStats,
}

/// A world waiting for its wipe to reach black.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    index: usize,
    warp: Option<i16>,
    wipe: u8,
}

/// The game session.
pub struct Session<B: Backend, R: ReadAt> {
    /// The renderer.
    pub renderer: Renderer<B>,
    /// The running world.
    pub game: Option<Game>,
    /// Progress between worlds (held here while no world runs).
    pub progress: Progress,
    /// Options.
    pub options: SessionOptions,
    /// A message shown on screen (no disc, a load error).
    pub message: Option<String>,
    /// The sound, when the platform has given one.
    pub sound: Option<Sound>,
    /// The HUD and menus.
    pub frontend: Option<Frontend>,
    /// Saving, when the platform has given slot files.
    pub saves: Option<Saves>,
    /// Movies, when the platform has given a way to read them.
    pub movies: Option<Movies>,
    disc: Option<Disc<R>>,
    option_map: Option<OptionMap>,
    materials: Option<formats::collision::MaterialTables>,
    player_consts: Option<engine::player::PlayerConstants>,
    camera_consts: Option<engine::camera::CameraConstants>,
    gpu: LevelGpu,
    overlay: Option<TextOverlay>,
    ui: Option<UiLayer>,
    iris: Option<render::iris::IrisShape>,
    video: Option<render::video::VideoLayer>,
    video_shown: u64,
    video_size: (u32, u32),
    video_on: bool,
    video_buf: [Vec<u8>; 3],
    held_save: Option<save::SaveRequest>,
    current: Option<Current>,
    wipe: Option<Wipe>,
    pending: Option<Pending>,
    quit: bool,
}

impl<B: Backend, R: ReadAt> Session<B, R> {
    /// A session on `disc` (or showing why there is none).
    pub fn new(backend: B, disc: Result<Disc<R>, String>, options: SessionOptions) -> Session<B, R> {
        let (disc, message) = match disc {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e)),
        };
        let exe = disc.as_ref().map(|d| &d.executable);
        let option_map = exe.and_then(|e| OptionMap::from_executable(e).ok());
        let materials = exe.and_then(|e| formats::collision::MaterialTables::from_executable(e).ok());
        let player_consts = exe.and_then(|e| engine::player::PlayerConstants::from_executable(e).ok());
        let camera_consts = exe.and_then(|e| engine::camera::CameraConstants::from_executable(e).ok());
        let mut progress = Progress::new_game();
        progress.difficulty_tables = exe.and_then(DifficultyTables::from_executable);
        let frontend = if options.frontend { exe.and_then(Frontend::new) } else { None };
        Session {
            renderer: Renderer::new(backend),
            game: None,
            progress,
            options,
            message,
            sound: None,
            frontend,
            saves: None,
            movies: None,
            disc,
            option_map,
            materials,
            player_consts,
            camera_consts,
            gpu: LevelGpu::default(),
            overlay: None,
            ui: None,
            iris: None,
            video: None,
            video_shown: 0,
            video_size: (0, 0),
            video_on: false,
            video_buf: [Vec::new(), Vec::new(), Vec::new()],
            held_save: None,
            current: None,
            wipe: None,
            pending: None,
            quit: false,
        }
    }

    /// Saving through `files` (the PC's user folder, or the Switch's through the platform's file system).
    pub fn set_saves(&mut self, files: alloc::boxed::Box<dyn SlotFiles>) {
        if let Some(d) = self.disc.as_ref() {
            self.saves = Some(Saves::new(files, &d.worlds));
        }
    }

    /// Movies, read through `opener` (which opens the disc image again). Without a movie table nothing is set.
    pub fn set_movie_source(&mut self, opener: Opener) {
        self.movies = self.disc.as_ref().and_then(|d| Movies::new(&d.executable, opener));
    }

    /// Starts a movie, with its sound on the sound engine's PCM stream when there is a sound engine.
    pub fn start_movie(&mut self, identity: u32) {
        let Some(m) = self.movies.as_mut() else { return };
        if m.start(identity)
            && let Some(s) = self.sound.as_mut()
        {
            let (writer, reader) = audio::pcm::pcm_stream(audio::pcm::PCM_RATE as usize / 5, 2);
            s.engine.attach_pcm(Some(reader));
            m.attach_audio(writer);
        }
    }

    /// Whether a movie is playing.
    pub fn movie_playing(&self) -> bool {
        self.movies.as_ref().is_some_and(Movies::playing)
    }

    /// The world table's names.
    pub fn world_names(&self) -> Vec<String> {
        self.disc.as_ref().map_or_else(Vec::new, |d| d.worlds.iter().map(|w| w.name.clone()).collect())
    }

    /// A world by index, id, name or part of a name ([`crate::settings::world_index`]).
    pub fn find_world(&self, which: &str) -> Option<usize> {
        crate::settings::world_index(&self.disc.as_ref()?.worlds, which)
    }

    /// The current world.
    pub fn current(&self) -> Option<&Current> {
        self.current.as_ref()
    }

    /// The world table index of the level with search key `identity`.
    pub fn world_of_identity(&self, identity: u32) -> Option<usize> {
        self.disc.as_ref()?.worlds.iter().position(|w| w.file.identity == identity)
    }

    /// Whether a wipe is running.
    pub fn wiping(&self) -> bool {
        self.wipe.is_some()
    }

    /// The start sequence (level-flow.md 6): Splash with the title menu; Start leaves through Splash's exit.
    pub fn start_title(&mut self) -> Result<(), String> {
        // The slot used last is loaded at the title, so Start resumes it.
        if let Some(s) = self.saves.as_mut() {
            s.load_last();
        }
        self.load_world(0, None, 0)?;
        // The start-up logo movie plays once at power-on (level-flow.md 6).
        self.start_movie(STARTUP_MOVIE);
        self.refresh_slots();
        if let Some(f) = self.frontend.as_mut() {
            f.menus.save_loaded = self.saves.as_ref().is_some_and(|s| s.loaded);
            f.menus.open_title();
        }
        Ok(())
    }

    /// Gives the menus' slot lists what the slots hold.
    fn refresh_slots(&mut self) {
        if let (Some(s), Some(f), Some(d)) = (self.saves.as_ref(), self.frontend.as_mut(), self.disc.as_ref()) {
            f.menus.slots = s.slot_infos(&d.worlds);
        }
    }

    /// Writes the auto-saves asked for. The first one of a game whose notice is pending waits for the notice (prompt
    /// 20) to be answered.
    fn write_saves(&mut self) {
        let Some(s) = self.saves.as_mut() else { return };
        for r in s.take_requests() {
            if self.held_save.is_some() {
                self.held_save = Some(r);
                continue;
            }
            let ask = r.trigger.is_auto()
                && s.notice_pending
                && self.frontend.as_mut().is_some_and(|f| {
                    f.menus.autosave_notice_pending = true;
                    f.menus.before_autosave()
                });
            s.notice_pending = false;
            if ask {
                self.held_save = Some(r);
            } else {
                s.commit(r);
            }
        }
    }

    /// Continues the save loaded at the title (Start on the title screen with a save loaded); without one, starts
    /// through the start-up world's exit.
    pub fn continue_save(&mut self) {
        let at = self.saves.as_ref().filter(|s| s.loaded).map(|s| {
            let st = &s.session.state;
            (crate::saving::restore(st), crate::saving::world_id(st.episode, st.level))
        });
        match at {
            Some((p, world)) => self.resume_save(p, world),
            None => self.default_exit(),
        }
    }

    /// Continues a loaded save: its progress, in the world it was saved in.
    fn resume_save(&mut self, progress: Progress, world: u32) {
        let index = self.disc.as_ref().and_then(|d| d.worlds.iter().position(|w| w.id == world)).unwrap_or(1);
        self.set_progress(progress);
        self.go_to(index, None, 1);
    }

    /// Replaces the progress (the running game's too), keeping the difficulty tables.
    fn set_progress(&mut self, mut progress: Progress) {
        let holder = match self.game.as_mut() {
            Some(g) => &mut g.progress,
            None => &mut self.progress,
        };
        progress.difficulty_tables = holder.difficulty_tables.take();
        *holder = progress;
    }

    /// Loads world `index` at once and arrives at `warp` (or the default warp).
    pub fn load_world(&mut self, index: usize, warp: Option<i16>, wipe: u8) -> Result<(), String> {
        let disc = self.disc.as_mut().ok_or("no disc")?;
        let entry = disc.worlds.get(index).ok_or_else(|| format!("no world {index}"))?.clone();
        let bytes = disc.read_world(index).map_err(|e| format!("{}: {e}", entry.name))?;
        let info =
            WorldInfo { index: index as u32, id: entry.id, task_mask: entry.task_mask, identity: entry.file.identity };
        if let Some(s) = self.sound.as_mut() {
            s.load_world(disc, &bytes);
        }
        // The scene (meshes, materials, textures), the fonts and the objects come from one parse of the world file.
        let (scene, layout, stream, fonts, mask) = {
            let fw = formats::world::World::load(&bytes).map_err(|e| format!("{}: {e}", entry.name))?;
            let scene = Scene::build(&fw, SceneOptions::default());
            let fonts = if self.frontend.is_some() { ui::FontSet::from_world(&fw).ok() } else { None };
            // The iris: the world's screen mask, its five sets coloured by shaders 1166 to 1170 (level-flow.md 5).
            let mask =
                crate::iris::ScreenMask::of_world(&fw).map(|m| (m, crate::iris::ScreenMask::colors_of_world(&fw)));
            (scene, fw.layout, fw.objects, fonts, mask)
        };
        let level = LevelData { bytes, layout, stream, table: formats::options::OptionTable::sly1() };
        let world = World::from_parts(level, info).map_err(|e| format!("{}: {e}", entry.name))?;
        // Progress moves into the new game; the old game's progress comes back first.
        if let Some(old) = self.game.take() {
            self.progress = old.progress;
        }
        let progress = core::mem::take(&mut self.progress);
        let cfg = GameConfig { seed: self.options.seed, option_map: self.option_map.clone(), no_scripts: false };
        let mut systems = Systems::for_game(self.materials.clone());
        systems.player = alloc::boxed::Box::new(engine::game::player_bridge::SlyPlayer::new(self.player_consts));
        if let Some(c) = self.camera_consts {
            systems.camera = alloc::boxed::Box::new(engine::camera::system::CameraSystem::new(c, Default::default()));
        }
        if !self.options.follow_camera {
            systems.camera = alloc::boxed::Box::new(engine::game::camera::FreeCamera::default());
        }
        let mut game = Game::new(world, progress, systems, cfg);
        let overlay = self.options.overlay;
        let sheet = fonts.as_ref().map(|f| (f.sheet.width, f.sheet.height, f.sheet.pixels.clone()));
        let want_ui = self.frontend.is_some();
        let mut ov = None;
        let mut ui_layer = None;
        let mut iris = None;
        let mut video = None;
        self.gpu = upload(&mut self.renderer, &game.world, &scene, |r| {
            if overlay {
                ov = Some(TextOverlay::new(r, OVERLAY_CHARS)?);
            }
            if let Some((m, colors)) = mask.as_ref() {
                iris = Some(render::iris::IrisShape::new(r, &m.points, m.set_slices(), *colors, m.bounds)?);
            }
            video = Some(render::video::VideoLayer::new(r)?);
            if want_ui {
                ui_layer = Some(UiLayer::new(r, sheet)?);
            }
            Ok(())
        })
        .map_err(|e| format!("{}: {e}", entry.name))?;
        self.overlay = ov;
        self.ui = ui_layer;
        self.iris = iris;
        self.video = video;
        self.video_on = false;
        self.video_shown = 0;
        // The player's projected shadow (rendering.md 9.2); its origin follows the player every frame.
        let projectors: Vec<render::Projector> = crate::level::player_projector(&game.world).into_iter().collect();
        self.renderer.set_projectors(&projectors).map_err(|e| format!("{}: {e}", entry.name))?;
        if let (Some(f), Some(fonts)) = (self.frontend.as_mut(), fonts) {
            f.fonts = fonts;
        }
        if game.arrive(warp, wipe == 2).is_none() {
            let (eye, target) = spawn_view(&game.world);
            game.systems.camera.cut(eye, target);
        }
        game.world.probe_collects = self.options.camera_collects;
        if let Some(s) = self.saves.as_mut() {
            s.world_loaded(entry.id, index != 0);
        }
        self.current = Some(Current { index, name: entry.name.clone(), stats: self.gpu.stats });
        self.game = Some(game);
        self.message = None;
        Ok(())
    }

    /// Asks for world `index` through a wipe of `kind` (at once when wipes are off).
    pub fn go_to(&mut self, index: usize, warp: Option<i16>, kind: u8) {
        if !self.options.wipes {
            self.load_or_report(index, warp, kind);
            return;
        }
        if self.pending.is_none() {
            self.pending = Some(Pending { index, warp, wipe: kind });
            self.wipe = Some(Wipe::Out { t: 0.0, kind });
        }
    }

    fn step_wipe(&mut self, dt: f32) {
        self.wipe = match self.wipe {
            Some(Wipe::Out { t, kind }) => {
                let t = t + dt;
                if t >= wipe_times(kind).0 {
                    if let Some(p) = self.pending.take() {
                        self.load_or_report(p.index, p.warp, p.wipe);
                    }
                    Some(Wipe::In { t: 0.0, kind })
                } else {
                    Some(Wipe::Out { t, kind })
                }
            }
            Some(Wipe::In { t, kind }) => {
                let t = t + dt;
                if t >= wipe_times(kind).1 { None } else { Some(Wipe::In { t, kind }) }
            }
            None => None,
        };
    }

    /// Runs one frame of the game for a measured interval (seconds) and handles the world's requests. Returns
    /// false when the session should end.
    pub fn update(&mut self, input: &FrameInput, measured: f32) -> Result<bool, RenderError> {
        let dt = engine::game::clock::frame_step(measured);
        // A movie holds the world (no frame, no input); Start skips it.
        if let Some(m) = self.movies.as_mut()
            && m.playing()
        {
            if !m.update(measured, input.pressed & platform_api::Buttons::PLUS.0 != 0)
                && let Some(s) = self.sound.as_mut()
            {
                s.engine.attach_pcm(None);
            }
            return Ok(true);
        }
        // Menus first: pause, title, their actions.
        let mut actions = Vec::new();
        let slot_in_use = self.saves.as_ref().is_some_and(|s| s.session.slot().is_some());
        if let (Some(f), Some(g)) = (self.frontend.as_mut(), self.game.as_mut()) {
            if !f.menu_open() && input.pressed & platform_api::Buttons::PLUS.0 != 0 && self.wipe.is_none() {
                let name = self.current.as_ref().map_or("", |c| c.name.as_str());
                f.pause(&g.world.info, name, &g.progress, slot_in_use);
            }
            if f.menu_open() {
                actions = f.update_menus(input, dt);
            }
            g.clock.rate = if f.menu_open() { 0.0 } else { 1.0 };
        }
        for a in actions {
            self.front_action(a);
        }
        if self.quit {
            return Ok(false);
        }
        let menu_open = self.frontend.as_ref().is_some_and(Frontend::menu_open);
        let Some(game) = self.game.as_mut() else { return Ok(true) };
        game.world.probe =
            (self.options.camera_triggers && game.world.player.is_none()).then_some(game.render.camera.eye);
        let gi = if menu_open { FrameInput::default() } else { *input };
        if let Some(s) = self.sound.as_ref() {
            game.world.shared.voice_playing = s.voice_playing();
        }
        game.frame(&gi, measured);
        if let Some(s) = self.saves.as_mut() {
            let bottles = game.world.slots_of_class(id::CLUE).len();
            let played = if menu_open { 0.0 } else { dt };
            s.track(&game.progress, game.world.info.id, played, bottles);
        }
        self.gpu.sync(&mut self.renderer, &game.world, &game.render.changed)?;
        self.gpu.fade(&mut self.renderer, &game.world, &game.render.camera_extras.faded)?;
        self.gpu.skin(&mut self.renderer, &game.render.skins)?;

        // Sound.
        if let Some(s) = self.sound.as_mut() {
            let voices = core::mem::take(&mut game.world.shared.voice);
            if let Some(d) = self.disc.as_mut() {
                for v in &voices {
                    s.play_voice(d, v);
                }
            }
            s.frame(&game.audio, game.world.shared.intensity(), dt);
        } else {
            game.world.shared.voice.clear();
            game.world.shared.voice_playing = false;
        }

        // Requests.
        let requests = game.take_requests();
        let current = self.current.as_ref().map(|c| c.index);
        for r in requests {
            match r {
                Request::LoadWorld { identity, warp, wipe } => {
                    if let Some(i) = self.world_of_identity(identity) {
                        self.go_to(i, warp, wipe);
                        break;
                    }
                }
                Request::LoadWorldNamed { name, warp, wipe } => {
                    if let Some(i) = self.find_world(&name) {
                        self.go_to(i, warp, wipe);
                        break;
                    }
                }
                Request::DefaultExit => {
                    self.default_exit();
                    break;
                }
                Request::Respawn => {
                    // The last checkpoint's warp in this world, else the world's start (level-flow.md 4).
                    let warp = self
                        .game
                        .as_ref()
                        .and_then(|g| g.progress.respawn.filter(|r| r.world == g.world.info.id).and_then(|r| r.warp));
                    if let Some(i) = current {
                        self.go_to(i, warp, 1);
                    }
                    break;
                }
                Request::Movie(identity) => self.start_movie(identity),
                Request::GameOver => {
                    if let Some(g) = self.game.as_mut() {
                        let tables = g.progress.difficulty_tables.take();
                        let levels = core::mem::take(&mut g.progress.levels);
                        g.progress = Progress { levels, difficulty_tables: tables, ..Progress::new_game() };
                    }
                    if let Some(i) = current {
                        self.go_to(i, None, 1);
                    }
                    break;
                }
                _ => {}
            }
        }
        self.write_saves();
        self.step_wipe(dt);
        Ok(true)
    }

    fn default_exit(&mut self) {
        let target = self.game.as_ref().and_then(|g| default_exit(&g.world));
        match target {
            Some((identity, warp, wipe)) => {
                if let Some(i) = self.world_of_identity(identity) {
                    self.go_to(i, warp, wipe);
                }
            }
            None => {
                // No default exit: restart the current world (level-flow.md 2).
                if let Some(i) = self.current.as_ref().map(|c| c.index) {
                    self.go_to(i, None, 1);
                }
            }
        }
    }

    fn front_action(&mut self, a: FrontAction) {
        let slot = |i: usize| save::Slot::new(i as u8);
        match a {
            FrontAction::Resume => {}
            FrontAction::Start { resume: true } => self.continue_save(),
            FrontAction::Start { resume: false } => {
                if let Some(s) = self.saves.as_mut() {
                    s.new_game(None);
                }
                self.default_exit();
            }
            FrontAction::NewGame(i) => {
                self.held_save = None;
                if let Some(s) = self.saves.as_mut() {
                    s.new_game(slot(i));
                }
                self.set_progress(Progress::new_game());
                self.default_exit();
            }
            FrontAction::LoadGame(i) => {
                let loaded = self.saves.as_mut().zip(slot(i)).and_then(|(s, sl)| s.load(sl));
                if let Some((p, world)) = loaded {
                    self.resume_save(p, world);
                }
            }
            FrontAction::Save(i) => {
                if let Some((s, sl)) = self.saves.as_mut().zip(slot(i)) {
                    s.save_to(sl);
                }
                self.refresh_slots();
            }
            FrontAction::Erase(i) => {
                if let Some((s, sl)) = self.saves.as_mut().zip(slot(i)) {
                    s.erase(sl);
                }
                self.refresh_slots();
            }
            FrontAction::AutosaveNotice => {
                if let (Some(s), Some(r)) = (self.saves.as_mut(), self.held_save.take()) {
                    s.commit(r);
                }
            }
            FrontAction::ExitLevel => self.default_exit(),
            FrontAction::RestartLevel => {
                if let Some(i) = self.current.as_ref().map(|c| c.index) {
                    self.go_to(i, None, 1);
                }
            }
            FrontAction::ExitToHideout => {
                if let Some(i) = self.find_world("hideout") {
                    self.go_to(i, None, 1);
                }
            }
            FrontAction::Quit { save } => {
                if save && let Some(s) = self.saves.as_mut() {
                    s.save_and_quit();
                }
                self.quit = true;
            }
        }
    }

    fn load_or_report(&mut self, index: usize, warp: Option<i16>, wipe: u8) {
        if let Err(e) = self.load_world(index, warp, wipe) {
            self.message = Some(e);
        }
    }

    /// The debug status lines.
    pub fn status_lines(&self, frame_ms: f64) -> Vec<String> {
        let mut v = Vec::new();
        match (&self.current, &self.game) {
            (Some(c), Some(g)) => {
                v.push(format!("{} ({})", c.name, c.index));
                v.push(format!(
                    "frame {:.2} ms  objects {}  updated {}  drawn {}",
                    frame_ms,
                    g.world.len(),
                    g.counters.updated,
                    c.stats.instances
                ));
                let p = &g.progress;
                v.push(format!("lives {}  charms {}  coins {}", p.lives, p.charms, p.coins));
            }
            _ => v.push("no world".to_string()),
        }
        if let Some(m) = &self.message {
            v.push(m.clone());
        }
        if let Some(e) = self.saves.as_ref().and_then(|s| s.last_error.as_ref()) {
            v.push(e.clone());
        }
        v
    }

    /// Draws the frame.
    pub fn render(&mut self, frame_ms: f64) -> Result<FrameStats, RenderError> {
        if self.overlay.is_some() {
            let lines = self.status_lines(frame_ms);
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let (_, h) = self.renderer.backend().target_size();
            let scale = if h >= 1000 { 3 } else { 2 };
            if let Some(ov) = self.overlay.as_mut() {
                ov.set_text(&mut self.renderer, &refs, 16, 16, scale, [255, 255, 255, 255])?;
            }
        }
        let Some(g) = self.game.as_ref() else { return Ok(FrameStats::default()) };
        let saving = self.saves.as_ref().is_some_and(|s| s.icon > 0.0);
        let c = g.render.camera;
        let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
        let camera =
            Camera { eye: v(c.eye), target: v(c.target), up: v(c.up), fov_y: c.fov_y, near: c.near, far: c.far };
        // The iris wipe (kind 1) is the renderer's mask around the player; the other kinds are the frontend's fade.
        let (w, h) = self.renderer.backend().target_size();
        let iris = self.wipe.and_then(iris_open);
        let fade = self.wipe.filter(|_| iris.is_none() || self.iris.is_none());
        if let Some(mask) = self.iris.as_ref() {
            match iris {
                Some(open) => {
                    let focus = g.world.player.and_then(|p| g.world.position(p)).map(|p| [p[0], p[1], p[2] + 100.0]);
                    let player = focus
                        .and_then(|p| {
                            screen_point(&camera, p, w as f32, h as f32, self.renderer.backend().clip_space())
                        })
                        .unwrap_or([w as f32 * 0.5, h as f32 * 0.5]);
                    let (center, k) = crate::iris::iris_placement(player, 1.0 - open, (w, h));
                    mask.set(&mut self.renderer, center, k, 1.0 - open)?;
                }
                None => mask.hide(&mut self.renderer)?,
            }
        }
        // The player's projected shadow follows the player.
        if let Some(p) = crate::level::player_projector(&g.world) {
            self.renderer.set_projectors(&[p])?;
        }
        // Movie frames on the video layer (4:2:0 BT.601), letterboxed.
        let mut movie_rect = None;
        let movie = self.movies.as_ref().filter(|m| m.playing());
        match (movie, self.video.as_ref()) {
            (Some(m), Some(layer)) => {
                if m.frames_shown() != self.video_shown
                    && let Some(f) = m.frame()
                {
                    let p = f.planes();
                    pack_planes(&p, &mut self.video_buf);
                    let [y, u, v] = &self.video_buf;
                    let (fw, fh) = (p.width as u32, p.height as u32);
                    self.renderer.set_video_frame(Some(render::video::VideoFrame {
                        width: fw,
                        height: fh,
                        y,
                        u,
                        v,
                    }))?;
                    self.video_shown = m.frames_shown();
                    self.video_size = (fw, fh);
                }
                if self.video_shown > 0 {
                    let rect = render::video::VideoLayer::fit((w, h), self.video_size);
                    layer.show(&mut self.renderer, rect, [1.0; 4])?;
                    self.video_on = true;
                    movie_rect = Some(rect);
                }
            }
            (None, Some(layer)) if self.video_on => {
                layer.hide(&mut self.renderer)?;
                self.renderer.set_video_frame(None)?;
                self.video_on = false;
                self.video_shown = 0;
            }
            _ => {}
        }
        // Dialogue picture panels from the camera.
        let panels: Vec<render::Panel> = g
            .render
            .camera_extras
            .panels
            .iter()
            .enumerate()
            .filter_map(|(k, p)| p.map(|p| panel_of(k, p, w as f32, h as f32, c.near, c.far)))
            .collect();
        self.renderer.set_panels(&panels)?;
        let movie = self.movies.as_ref().filter(|m| m.playing());
        if let (Some(f), Some(ui), Some(m)) = (self.frontend.as_mut(), self.ui.as_mut(), movie) {
            let (w, h) = self.renderer.backend().target_size();
            match movie_rect {
                Some(rect) => f.movie_bars(w, h, rect),
                None => f.movie_card(w, h, m.title()),
            }
            ui.draw(&mut self.renderer, &f.list)?;
        } else if let (Some(f), Some(ui)) = (self.frontend.as_mut(), self.ui.as_mut()) {
            let (w, h) = self.renderer.backend().target_size();
            let bottles = g.world.slots_of_class(id::CLUE).len();
            f.build(
                w,
                h,
                &g.progress,
                &g.world.info,
                bottles,
                &g.world.shared.screen,
                g.world.shared.subtitle.as_ref(),
                fade,
                saving,
                (frame_ms * 1e-3) as f32,
            );
            ui.draw(&mut self.renderer, &f.list)?;
        }
        self.renderer.set_fog(g.render.fog.map(|f| Fog {
            near: f.start,
            far: f.end,
            cap: f.max,
            color: [f.color[0], f.color[1], f.color[2], 1.0],
        }));
        self.renderer.set_darkening(g.world.shared.screen.darkening);
        self.renderer.render(&camera, g.render.clear)
    }

    /// A hash of the simulation for determinism checks.
    pub fn state_hash(&self) -> u64 {
        let mut h = engine::game::Fnv::new();
        h.u64(self.game.as_ref().map_or(0, Game::state_hash));
        h.u64(self.current.as_ref().map_or(u64::MAX, |c| c.index as u64));
        h.finish()
    }
}

/// How open the iris is (1 open, 0 closed) during a wipe of kind 1 (level-flow.md 5), or `None` for the other kinds.
pub fn iris_open(wipe: Wipe) -> Option<f32> {
    let (kind, closed) = match wipe {
        Wipe::Out { t, kind } => (kind, t / wipe_times(kind).0),
        Wipe::In { t, kind } => (kind, 1.0 - t / wipe_times(kind).1),
    };
    (kind == 1).then_some(1.0 - closed.clamp(0.0, 1.0))
}

/// Copies a decoded frame's planes without their row padding.
fn pack_planes(p: &video::Planes<'_>, out: &mut [Vec<u8>; 3]) {
    let (w, h) = (p.width, p.height);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let copy = |src: &[u8], stride: usize, w: usize, h: usize, dst: &mut Vec<u8>| {
        dst.clear();
        for row in 0..h {
            dst.extend_from_slice(&src[row * stride..][..w]);
        }
    };
    let [y, u, v] = out;
    copy(p.y, p.y_stride, w, h, y);
    copy(p.cb, p.chroma_stride, cw, ch, u);
    copy(p.cr, p.chroma_stride, cw, ch, v);
}

/// Where a world point lands on the target, in pixels from the top left (`None` behind the camera).
fn screen_point(camera: &Camera, p: [f32; 3], w: f32, h: f32, clip: render::ClipSpace) -> Option<[f32; 2]> {
    let vp = camera.view_proj(w / h.max(1.0), clip);
    let q = vp.transform_point4(Vec3::new(p[0], p[1], p[2]));
    (q[3] > 1e-4).then(|| [(q[0] / q[3] * 0.5 + 0.5) * w, (0.5 - q[1] / q[3] * 0.5) * h])
}

/// Panel size as a fraction of the target height (4:3). *Placeholder*: the spec gives the screen position (options
/// 991 and 992) but not the panel's size.
pub const PANEL_HEIGHT: f32 = 0.3;

/// A dialogue picture panel: its camera, and a 4:3 rectangle centred at its screen position (normalised device
/// coordinates; left and upper corners for panels 0 and 1 when the position is unset).
fn panel_of(
    k: usize,
    (eye, target, fov, screen): engine::game::systems::PanelView,
    w: f32,
    h: f32,
    near: f32,
    far: f32,
) -> render::Panel {
    let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
    let s = if screen == [0.0; 2] { [if k == 0 { -0.6 } else { 0.6 }, 0.5] } else { screen };
    let ph = h * PANEL_HEIGHT;
    let pw = ph * 4.0 / 3.0;
    let (cx, cy) = ((s[0] * 0.5 + 0.5) * w, (0.5 - s[1] * 0.5) * h);
    render::Panel {
        camera: Camera { eye: v(eye), target: v(target), up: Vec3::new(0.0, 0.0, 1.0), fov_y: fov, near, far },
        rect: [cx - pw * 0.5, cy - ph * 0.5, pw, ph],
        clear: Some([0.0, 0.0, 0.0, 1.0]),
    }
}

/// The world's default exit (option 168): its destination identity, warp and wipe.
pub fn default_exit(world: &World) -> Option<(u32, Option<i16>, u8)> {
    world.slots_of_class(id::EXIT).iter().find_map(|&s| {
        let h = world.handle(s)?;
        let o = world.options(h)?;
        if !o.bool_or(engine::behavior::exit::opt::DEFAULT, false) {
            return None;
        }
        let rec = world.record(h)?;
        let r = world.level.record(rec);
        let d = world.level.bytes.get(r.offset + 6..r.offset + 38)?;
        let desc = formats::disc::Descriptor::decode(d).ok()??;
        Some((
            desc.identity,
            o.i16(engine::behavior::exit::opt::WARP),
            o.i32(engine::behavior::exit::opt::WIPE).unwrap_or(0).clamp(0, 3) as u8,
        ))
    })
}

/// Where the camera starts without a warp: behind the player character if the world has one (700 units back along
/// its facing, 350 up), else above the middle of the drawn objects.
pub fn spawn_view(world: &World) -> ([f32; 3], [f32; 3]) {
    let wt = world.world_transforms();
    if let Some(&p) = world.slots_of_class(id::PLAYER).first() {
        let w = wt[p as usize];
        let f = w.cols[0];
        let l = engine::math::length([f[0], f[1], 0.0]).max(1e-6);
        let target = [w.t[0], w.t[1], w.t[2] + 100.0];
        let eye = [w.t[0] - f[0] / l * 700.0, w.t[1] - f[1] / l * 700.0, w.t[2] + 350.0];
        return (eye, target);
    }
    let mut xs: Vec<[f32; 3]> =
        world.drawables().iter().filter(|d| d.visible).map(|d| wt[d.object.index() as usize].t).collect();
    if xs.is_empty() {
        return ([0.0, -1000.0, 500.0], [0.0; 3]);
    }
    let mut mid = [0.0f32; 3];
    let mut span = 0.0f32;
    for (k, m) in mid.iter_mut().enumerate() {
        xs.sort_by(|a, b| a[k].total_cmp(&b[k]));
        let lo = xs[xs.len() * 2 / 100][k];
        let hi = xs[(xs.len() * 98 / 100).min(xs.len() - 1)][k];
        *m = 0.5 * (lo + hi);
        if k < 2 {
            span = span.max(hi - lo);
        }
    }
    let d = (span * 0.5).max(1000.0);
    ([mid[0], mid[1] - d, mid[2] + d * 0.6], mid)
}
