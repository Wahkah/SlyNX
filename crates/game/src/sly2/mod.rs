//! The Sly 2 game session (first stage): the player's own Sly 2 disc, a world loaded into the engine's object model
//! with its scripts running, drawn on the renderer, and moving between worlds by level row.
//!
//! - **Worlds** are loaded with `engine::world::World::load_sly2` at their level row (the conditional records of
//!   that row), with the executable's option map and built-in table for the scripts (`engine::world::sly2`). The engine's `Game` runs them: clocks, scripts, transforms. No Sly 2 behaviours exist yet.
//! - **Drawing**: the world's `formats_sly2::scene` build, uploaded as static instances ([`level`]).
//! - **Characters**: Sly, Murray and Bentley (`engine::gang`) as the world's player ([`gang`]): they move through
//!   the world's collision, the d-pad switches between them, and `World::player` follows the one played.
//! - **Camera**: the follow camera (`engine::camera`, with the spec's constants) on the played character; worlds
//!   without the gang, or `follow: false`, get the free camera (left stick fly, right stick look).
//!
//! The Sly 1 session ([`crate::session`]) is unchanged; the two share the renderer helpers of [`crate::level`].

pub mod cues; // Sly 2 hook: voice lines drive their speaker's state machines and eyes.
pub mod gang;
pub mod layer; // Sly 2 hook: the 2D layer (fonts, HUD, menus, job screens).
pub mod level;
pub mod progress; // Sly 2 hook: progress, saving and world changes through the job system.
pub mod screens; // Sly 2 hook: the job screens (job complete, saving, job help).
pub mod skin;
pub mod talk;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use engine::game::camera::FreeCamera;
use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::gang::{GangConstants, GangTunables, ItemTable, Owned, Shared};
use engine::world::sly2::Sly2Options;
use engine::world::{World, WorldInfo};
use formats::iso::ReadAt;
use formats_sly2::Disc;
use render::math::Vec3;
use render::{Backend, Camera, FrameStats, RenderError, Renderer};
use script::{BuiltinTable, OptionMap};

use level::StaticLevel;

/// The world the session starts in when none is named: the nightclub hub (`f_nightclub_exterior`).
pub const HUB_WORLD_ID: u32 = 0x41;

/// The current Sly 2 world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sly2Current {
    /// Level row.
    pub row: usize,
    /// World id.
    pub id: u32,
    /// Objects loaded.
    pub objects: usize,
}

/// The Sly 2 session.
pub struct Sly2Session<B: Backend, R: ReadAt> {
    /// The renderer.
    pub renderer: Renderer<B>,
    /// The running world.
    pub game: Option<Game>,
    /// A message for the screen (no disc, a load error).
    pub message: Option<String>,
    /// Sound and dialogue, when the platform has given a sound engine ([`Sly2Session::set_sound`]).
    pub sound: Option<talk::Sly2Sound>,
    /// Sequence starts requested so far (scripts, volumes, spots).
    pub sequences_started: u32,
    /// The gang's constants, from the executable.
    pub gang_constants: Option<GangConstants>,
    /// The follow camera on the played character (off: the free camera).
    pub follow: bool,
    items: Option<ItemTable>,
    pockets: Option<engine::gang::PocketTable>,
    disc: Option<Disc<R>>,
    option_map: Option<OptionMap>,
    builtins: Option<BuiltinTable>,
    gpu: StaticLevel,
    current: Option<Sly2Current>,
    seed: u64,
    pending: progress::Pending,
    screens: ui::sly2::JobScreens,
    ui_paused: bool,
    saver: Option<screens::Saver>,
    font: Option<ui::sly2::font::FontSheet>,
    ui_layer: Option<crate::ui_layer::UiLayer>,
    hud: ui::sly2::hud::Hud,
    pause: ui::sly2::pause::PauseMenu,
    quit: bool,
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// A session on `disc` (or showing why there is none); `seed` seeds the game's random numbers.
    pub fn new(backend: B, disc: Result<Disc<R>, String>, seed: u64) -> Sly2Session<B, R> {
        let (disc, message) = match disc {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e)),
        };
        let exe = disc.as_ref().map(|d| &d.executable);
        Sly2Session {
            renderer: Renderer::new(backend),
            game: None,
            message,
            sound: None,
            sequences_started: 0,
            gang_constants: exe.and_then(|e| GangConstants::from_executable(e).ok()),
            follow: true,
            items: exe.and_then(|e| ItemTable::from_executable(e).ok()),
            pockets: exe
                .and_then(|e| engine::gang::PocketTable::from_executable(e, engine::gang::pocket::GUARD_TABLE_VA).ok()),
            option_map: exe.and_then(|e| OptionMap::from_sly2_executable(e).ok()),
            builtins: exe.and_then(|e| BuiltinTable::from_executable(e).ok()),
            disc,
            gpu: StaticLevel::default(),
            current: None,
            seed,
            pending: progress::Pending::default(),
            screens: ui::sly2::JobScreens::default(),
            ui_paused: false,
            saver: None,
            font: None,
            ui_layer: None,
            hud: ui::sly2::hud::Hud::default(),
            pause: ui::sly2::pause::PauseMenu::default(),
            quit: false,
        }
    }

    /// Sound through `engine`, reading the disc image again through `image` (worlds' sound, voice lines).
    pub fn set_sound(&mut self, engine: audio::SoundEngine, image: alloc::boxed::Box<dyn formats::iso::ReadAt>) {
        self.sound = talk::Sly2Sound::new(engine, image);
    }

    /// The subtitle of the line playing, revealed so far.
    pub fn subtitle(&self) -> Option<audio::sly2::Subtitle> {
        let t = self.game.as_ref()?.clock.game_time;
        self.sound.as_ref()?.subtitle(t)
    }

    /// The level rows that hold a world, with their world ids.
    pub fn worlds(&self) -> Vec<(usize, u32)> {
        self.disc.as_ref().map_or_else(Vec::new, |d| d.world_rows().map(|r| (r, d.rows[r].world_id())).collect())
    }

    /// The row of world `id`.
    pub fn find_world(&self, id: u32) -> Option<usize> {
        self.disc.as_ref()?.find_world(id)
    }

    /// The characters' skinned instances drawn.
    pub fn character_instances(&self) -> usize {
        self.gpu.characters.len()
    }

    /// Vehicles hook: the player drives the first object of `class` in the running world (`behavior::vehicles`);
    /// `None` hands control back to the characters. Returns whether a machine was found.
    pub fn drive(&mut self, class: Option<i16>) -> bool {
        let Some(g) = self.game.as_mut() else { return false };
        // The first active one (it and its ancestors in the world: a job's machine is held out of it until the job
        // starts), else the first of the class.
        let w = &g.world;
        let all: Vec<_> = class.map_or_else(Vec::new, |c| w.handles().filter(|&h| w.class(h) == Some(c)).collect());
        let machine = all.iter().copied().find(|&h| w.is_active(h)).or_else(|| all.first().copied());
        g.world.shared.driving = machine;
        machine.is_some() || class.is_none()
    }

    /// The current world.
    pub fn current(&self) -> Option<&Sly2Current> {
        self.current.as_ref()
    }

    /// Loads the world of level row `row`: objects, scripts, the drawn scene, the camera above it.
    pub fn load_row(&mut self, row: usize) -> Result<(), String> {
        let disc = self.disc.as_mut().ok_or("no disc")?;
        let level = disc.rows.get(row).ok_or_else(|| format!("no row {row}"))?;
        if level.file.is_empty() {
            return Err(format!("row {row} holds no world"));
        }
        let id = level.world_id();
        let episode = level.episode;
        let bytes = disc.read_world(row).map_err(|e| format!("row {row}: {e}"))?;
        let (build, characters, lighting, glyphs) = {
            let fw = formats_sly2::World::parse(&bytes, &disc.options).map_err(|e| format!("row {row}: {e}"))?;
            // Sly 2 hook (2D layer): the fonts, read once.
            let glyphs = if self.font.is_none() { layer::font_glyphs(&fw) } else { None };
            let opts = formats_sly2::scene::SceneOptions { row: Some(row as i16), ..Default::default() };
            // The characters' records (byte ranges), so their meshes are not collision.
            let mut characters = Vec::new();
            for b in &fw.bodies {
                formats_sly2::stream::visit(&b.objects, &mut |r| {
                    if gang::CHARACTER_CLASSES.iter().any(|c| c.1 == r.class) {
                        characters.push((r.header_offset, r.body_offset + r.body_size, r.class));
                    }
                });
            }
            let build = formats_sly2::scene::build(&fw, opts);
            // The world's lights, shadow colour and fog (rendering.md 2 and 6), as the viewer draws them.
            let lighting = stage::light::WorldLighting::new(&build.lights, &fw.options, fw.data);
            (build, characters, lighting, glyphs)
        };
        if let Some(g) = glyphs {
            self.font = Some(ui::sly2::font::FontSheet::build(&g));
        }
        let ranges: Vec<(u32, u32)> = characters.iter().map(|c| (c.0, c.1)).collect();
        let scene = &build.scene;
        let (collision, drawn) = gang::collision_from_scene(scene, &ranges);
        // Sly's drawn position (else any character's).
        let start = drawn.iter().find(|d| characters[d.0].2 == 53).or(drawn.first()).map(|d| d.1);
        let opts = Sly2Options {
            row: Some(row as i16),
            info: WorldInfo { index: row as u32, id, task_mask: 0, identity: id },
            builtins: self.builtins.clone(),
            ..Sly2Options::default()
        };
        let mut world = World::load_sly2(bytes, &disc.options, opts).map_err(|e| format!("row {row}: {e}"))?;
        // The world's own collision when it has some, else its drawn geometry.
        let collision = gang::collision_from_world(&world).unwrap_or(collision);
        // The guards' pockets come from the executable's tables, by episode (guards.md 8).
        world.shared.pockets = self.pockets.clone();
        world.shared.episode = episode;
        let cfg = GameConfig { seed: self.seed, option_map: self.option_map.clone(), no_scripts: false };
        let mut systems = Systems::null();
        // Physics over the world's collision (thrown objects fly through it; contacts become notices).
        systems.physics = alloc::boxed::Box::new(engine::game::physics_bridge::PhysicsBridge::from_collision(
            gang::collision_from_world(&world).unwrap_or_default(),
        ));
        systems.camera = alloc::boxed::Box::new(FreeCamera::default());
        let shared = match (self.gang_constants, self.items.clone()) {
            (Some(consts), Some(items)) => {
                Some(Shared { consts, tun: GangTunables::default(), items, owned: Owned::default() })
            }
            _ => None,
        };
        let has_gang = !characters.is_empty() && shared.is_some();
        if has_gang {
            systems.player = alloc::boxed::Box::new(gang::GangPlayer::new(shared, collision, start));
            if self.follow {
                systems.camera = alloc::boxed::Box::new(engine::camera::system::CameraSystem::new(
                    engine::camera::CameraConstants::SPEC,
                    Default::default(),
                ));
            }
        }
        let mut game = Game::new(world, Progress::new_game(), systems, cfg);
        // The characters' skinned meshes are not in the scene build: they are added here and posed every frame.
        let skinned = skin::character_draws(&game.world, &build.material_base);
        let skip: &[(u32, u32)] = if skinned.is_empty() { &[] } else { &ranges };
        let input = stage::prep::SceneInput { skip, ..stage::prep::SceneInput::sly2(&build, Some(&lighting)) };
        let sheet = self.font.as_ref().map(|f| (f.width, f.height, f.pixels.clone()));
        let mut ui_layer = None;
        self.gpu = level::upload_stage(&mut self.renderer, input, Some(&game.world), &skinned, |r| {
            ui_layer = Some(crate::ui_layer::UiLayer::new(r, sheet)?); // Sly 2 hook: the 2D layer.
            Ok(())
        })
        .map_err(|e| format!("row {row}: {e}"))?;
        self.ui_layer = ui_layer;
        if game.world.player.is_none() || !self.follow {
            let (c, r) = self.gpu.bounds;
            game.systems.camera.cut([c[0], c[1] - r, c[2] + r * 0.6], c);
        }
        if let Some(s) = self.sound.as_mut() {
            s.load_world(row);
        }
        self.current = Some(Sly2Current { row, id, objects: game.world.len() });
        self.game = Some(game);
        self.message = None;
        self.attach_progress(); // Sly 2 hook: the job system and the arrival point.
        Ok(())
    }

    /// Runs one frame for a measured interval (seconds). Returns false when the session should end.
    pub fn update(&mut self, input: &FrameInput, measured: f32) -> bool {
        if let Some(g) = self.game.as_mut() {
            g.frame(input, measured);
        }
        // Sly 2 hook: a load the job system asked for (an exit, a restart at a checkpoint).
        if let Err(e) = self.follow_load_request() {
            self.message = Some(e);
        }
        // Sequence starts and stops from scripts, volumes, spots and command lists (dialogue-and-voice.md 2.1).
        if let Some(g) = self.game.as_mut() {
            let requests = core::mem::take(&mut g.world.shared.sequence_requests);
            for r in requests {
                use engine::behavior::sly2_commands::SequenceRequest as Q;
                match (r, self.sound.as_mut()) {
                    (Q::Start { sequence, .. } | Q::StartWithPlayer(sequence), Some(s)) => {
                        s.start_sequence(&g.world, sequence);
                    }
                    (Q::Stop(sequence), Some(s)) => s.stop_sequence(sequence),
                    _ => {}
                }
                self.sequences_started += matches!(r, Q::Start { .. } | Q::StartWithPlayer(_)) as u32;
            }
        }
        if let Some(s) = self.sound.as_mut() {
            let dt = engine::game::clock::frame_step(measured);
            s.update(dt, input.pressed & platform_api::Buttons::B.0 != 0);
            // Music streams scripts queued (action 146).
            if let Some(g) = self.game.as_mut() {
                for n in core::mem::take(&mut g.world.shared.music_requests) {
                    s.queue_music(n);
                }
            }
            // Sub-sequences the running ones started (option 636).
            if let Some(g) = self.game.as_ref() {
                for h in s.take_sub_starts() {
                    s.start_sequence(&g.world, h);
                    self.sequences_started += 1;
                }
            }
        }
        self.apply_voice_cues(); // Sly 2 hook: lip sync and eyes.
        self.update_screens(input, engine::game::clock::frame_step(measured)); // Sly 2 hook: the job screens.
        !self.quit
    }

    /// Draws the frame.
    pub fn render(&mut self) -> Result<FrameStats, RenderError> {
        let Some(g) = self.game.as_ref() else { return Ok(FrameStats::default()) };
        self.gpu.characters.update(&mut self.renderer, &g.world)?;
        let c = g.render.camera;
        let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
        let camera =
            Camera { eye: v(c.eye), target: v(c.target), up: v(c.up), fov_y: c.fov_y, near: c.near, far: c.far };
        self.draw_ui()?; // Sly 2 hook: the 2D layer.
        self.renderer.render(&camera, self.gpu.clear)
    }

    /// The status lines.
    pub fn status_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        match (&self.current, &self.game) {
            (Some(c), Some(g)) => {
                v.push(format!(
                    "Sly 2 row {} (world {:#x}): {} objects, {} drawn",
                    c.row, c.id, c.objects, self.gpu.stats.instances
                ));
                v.push(format!("script errors {}", g.scripts().map_or(0, |s| s.error_count())));
            }
            _ => v.push("no world".to_string()),
        }
        if let Some(m) = &self.message {
            v.push(m.clone());
        }
        v
    }

    /// A hash of the simulation for determinism checks.
    pub fn state_hash(&self) -> u64 {
        let mut h = engine::game::Fnv::new();
        h.u64(self.game.as_ref().map_or(0, Game::state_hash));
        h.u64(self.current.as_ref().map_or(u64::MAX, |c| c.row as u64));
        h.finish()
    }
}
