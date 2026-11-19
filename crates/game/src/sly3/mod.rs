//! The Sly 3 game session (first stage): the player's own Sly 3 disc, a world loaded into the engine's object model,
//! drawn statically on the renderer, a free camera, and the world's sound.
//!
//! - **Worlds** are the 39 world slots that hold a world. They are loaded with `engine::world::World::load_sly3` at
//!   their slot (a fresh load: option set 0's condition selects the records), and the engine's `Game` runs them:
//!   clocks and transforms. No Sly 3 behaviours or scripts exist yet.
//! - **Drawing**: the world's `formats_sly3::scene` build through the viewer's path (`stage`, with the Sly 2
//!   session's [`crate::sly2::level::upload_stage`]): Sly 3's passes and shader flags, lit kinds with their two
//!   colours, option set 0's fog, merged static geometry.
//! - **Crew** ([`crew`]): Sly, Murray and Bentley (`engine::gang::sly3`) arrive at the checkpoint matching the
//!   arrival id (default 424), move through the world's collision parts, and the d-pad switches between them.
//! - **Camera**: the follow camera on the played character; worlds without the crew, or `follow: false`, get the
//!   free camera (left stick fly, right stick look), placed above the drawn world.
//! - **Sound**: with a [`SoundEngine`] and the audio crate's disc reader, each world's bank and stream are installed
//!   on load (`audio::sly3::WorldAudio`).
//!
//! The Sly 1 and Sly 2 sessions are unchanged; this module only adds files (and the marked `Sly 3 hook` lines in
//! `lib.rs` and `Cargo.toml`).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use audio::SoundEngine;
use audio::sly3::Sly3Disc as AudioDisc;
use engine::game::camera::FreeCamera;
use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats::iso::ReadAt;
use formats::scene::Scene;
use formats_sly3::Disc;
use render::math::Vec3;
use render::{Backend, Camera, FrameStats, RenderError, Renderer};

use crate::sly2::level::{StaticLevel, upload_stage};

pub mod commands;
pub mod crew;
pub mod flow;
pub mod jobs;
pub mod safehouse;

/// Class 40 and every class derived from it (the class table's base links, object-stream.md 3.1).
pub fn checkpoint_classes(classes: &[formats_sly3::exe::ClassEntry]) -> Vec<i16> {
    let (va, _) = formats_sly3::exe::CLASS_TABLE;
    let address = |i: usize| va + (i * formats_sly3::exe::CLASS_ENTRY_SIZE) as u32;
    let Some(root) = classes.iter().position(|c| c.class == crew::CHECKPOINT_CLASS as i32).map(address) else {
        return alloc::vec![crew::CHECKPOINT_CLASS];
    };
    let mut out = Vec::new();
    for (i, c) in classes.iter().enumerate() {
        let mut at = address(i);
        for _ in 0..classes.len() {
            if at == root {
                out.push(c.class as i16);
                break;
            }
            let Some(e) = classes.get(((at.wrapping_sub(va)) / formats_sly3::exe::CLASS_ENTRY_SIZE as u32) as usize)
            else {
                break;
            };
            if e.base == 0 {
                break;
            }
            at = e.base;
        }
    }
    out
}

/// The save-variable tree of a loaded Sly 3 world (saving.md 1; the same in every world), for
/// `save::sly3::progress` and the slot store.
pub fn save_tree(world: &World) -> Option<save::sly2::VarTree> {
    let at = world.sly3.as_deref()?.sections.save_variables;
    save::sly2::VarTree::parse(world.level.bytes.get(at..)?).map(|t| t.0)
}

/// Carries out one script request (commands.rs) on the world, the job system and the level flow. Dialogue,
/// screens, goals and music are not run by the session yet.
fn apply_request(
    r: commands::Request,
    world: &mut World,
    jobs: Option<&mut engine::game::sly2_jobs::Jobs>,
    flow: Option<&mut flow::Flow>,
) {
    use commands::Request;
    match r {
        Request::FireExit { exit, point } => {
            if let Some(f) = flow {
                f.fire(exit, point);
            }
        }
        Request::Present { object, on } => {
            if let Some(h) = world.find_sly3_id(object, None) {
                world.set_in_world(h, on);
            }
        }
        Request::TaskState { task, state } => {
            if let Some(j) = jobs {
                jobs::set_task(j, world, task, state);
            }
        }
        _ => {}
    }
}

/// The world the session starts in when none is named.
pub const FIRST_WORLD: &str = "v_apt";

/// The current Sly 3 world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sly3Current {
    /// World slot.
    pub slot: usize,
    /// World name.
    pub name: String,
    /// World id.
    pub id: u32,
    /// Objects loaded.
    pub objects: usize,
}

/// The Sly 3 session.
pub struct Sly3Session<B: Backend, R: ReadAt> {
    /// The renderer.
    pub renderer: Renderer<B>,
    /// The running world.
    pub game: Option<Game>,
    /// A message for the screen (no disc, a load error).
    pub message: Option<String>,
    /// The sound engine (game-thread half), when there is sound.
    pub sound: Option<SoundEngine>,
    /// The arrival id worlds are entered at (characters.md 2; default 424).
    pub arrival_id: i32,
    /// The follow camera on the played character (off: the free camera).
    pub follow: bool,
    /// The level flow of the running world (exits, the stored checkpoint, the restart).
    pub flow: Option<flow::Flow>,
    /// The last load the flow performed.
    pub last_load: Option<flow::LoadRequest>,
    /// The episode being played (1 to 8; 0 before one starts) and the task its start chose.
    pub episode: usize,
    /// See [`Sly3Session::episode`].
    pub started_task: Option<i32>,
    /// The hub's safe house, and whether the player is in it.
    pub safe_house: Option<safehouse::SafeHouse>,
    /// See [`Sly3Session::safe_house`].
    pub in_safe_house: bool,
    /// The open shop.
    pub shop: Option<safehouse::Shop>,
    /// The job system of the running world (`sly2_jobs` with Sly 3's IDs); it holds the progress block.
    pub jobs: Option<engine::game::sly2_jobs::Jobs>,
    /// The running command scripts.
    pub scripts: commands::Scripts,
    /// The progress block while no world holds it (before the first load).
    progress: Option<save::sly2::Progress>,
    /// A task to request, forced, when the next world loads (an episode start's task), and whether that load
    /// starts fresh (no carry from the previous world).
    queued_task: Option<i32>,
    fresh: bool,
    /// The task current at the end of the last frame (its script, option 1408, starts when it changes).
    last_task: Option<usize>,
    disc: Option<Disc<R>>,
    audio_disc: Option<AudioDisc<R>>,
    gpu: StaticLevel,
    current: Option<Sly3Current>,
    seed: u64,
    rng: audio::rng::Rng,
    wipes: flow::WipeOrder,
    take_out: Option<engine::gang::sly3::Member>,
}

/// Where to look from: the middle of the drawn meshes' centres with the outer tenth on each side left out
/// (outlying helpers would pull a plain bounding box off the level), and a radius covering the rest, at most
/// [`MAX_VIEW_RADIUS`] so the view stays within the free camera's far plane.
fn view_point(scene: &Scene) -> ([f32; 3], f32) {
    let pts: Vec<[f32; 3]> = scene
        .instances
        .iter()
        .filter_map(|i| scene.meshes.get(i.mesh as usize).map(|m| i.transform.transform_point(m.center)))
        .collect();
    if pts.is_empty() {
        return ([0.0; 3], 1000.0);
    }
    let mut c = [0.0f32; 3];
    let mut half = 0.0f32;
    for (k, ck) in c.iter_mut().enumerate() {
        let mut v: Vec<f32> = pts.iter().map(|p| p[k]).collect();
        v.sort_unstable_by(f32::total_cmp);
        let (lo, hi) = (v[v.len() / 10], v[(v.len() * 9 / 10).min(v.len() - 1)]);
        *ck = (lo + hi) * 0.5;
        if k < 2 {
            half = half.max((hi - lo) * 0.5);
        }
    }
    (c, half.clamp(1000.0, MAX_VIEW_RADIUS))
}

/// The largest view radius (the free camera's far plane is 20,000 units).
pub const MAX_VIEW_RADIUS: f32 = 8000.0;

impl<B: Backend, R: ReadAt> Sly3Session<B, R> {
    /// A session on `disc` (or showing why there is none); `seed` seeds the game's random numbers.
    pub fn new(backend: B, disc: Result<Disc<R>, String>, seed: u64) -> Sly3Session<B, R> {
        let (disc, message) = match disc {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e)),
        };
        Sly3Session {
            renderer: Renderer::new(backend),
            game: None,
            message,
            sound: None,
            arrival_id: crew::DEFAULT_ARRIVAL,
            follow: true,
            flow: None,
            last_load: None,
            episode: 0,
            started_task: None,
            safe_house: None,
            in_safe_house: false,
            shop: None,
            jobs: None,
            scripts: commands::Scripts::new(seed ^ 0x00C0_FFEE),
            progress: None,
            queued_task: None,
            fresh: true,
            last_task: None,
            disc,
            audio_disc: None,
            gpu: StaticLevel::default(),
            current: None,
            seed,
            rng: audio::rng::Rng::new(seed ^ 0x5133_F10E),
            wipes: flow::WipeOrder::default(),
            take_out: None,
        }
    }

    /// Adds sound: the engine and the audio crate's reader of the same disc image. Takes effect on the next load.
    pub fn with_sound(mut self, engine: SoundEngine, disc: AudioDisc<R>) -> Self {
        self.sound = Some(engine);
        self.audio_disc = Some(disc);
        self
    }

    /// The disc, when there is one.
    pub fn disc(&self) -> Option<&Disc<R>> {
        self.disc.as_ref()
    }

    /// The world slots that hold a world, with their names.
    pub fn worlds(&self) -> Vec<(usize, String)> {
        self.disc.as_ref().map_or_else(Vec::new, |d| d.world_slots().map(|s| (s, d.worlds[s].name.clone())).collect())
    }

    /// The slot of the world named `name`.
    pub fn find_world(&self, name: &str) -> Option<usize> {
        self.disc.as_ref()?.find_world(name)
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
    pub fn current(&self) -> Option<&Sly3Current> {
        self.current.as_ref()
    }

    /// Loads world `slot`: objects, the drawn scene, the camera above it, the world's sound.
    pub fn load_slot(&mut self, slot: usize) -> Result<(), String> {
        let disc = self.disc.as_mut().ok_or("no disc")?;
        let entry = disc.worlds.get(slot).ok_or_else(|| format!("no slot {slot}"))?.clone();
        if entry.file.is_empty() {
            return Err(format!("slot {slot} holds no world"));
        }
        let label = format!("{} (slot {slot})", entry.name);
        let id = entry.world_id();
        let bytes = disc.read_world(slot).map_err(|e| format!("{label}: {e}"))?;
        let (build, lighting) = {
            let fw = formats_sly3::World::parse_with(&bytes, &disc.options, disc.tangent_multipliers)
                .map_err(|e| format!("{label}: {e}"))?;
            let opts = formats_sly3::scene::SceneOptions {
                context: fw.fresh_context(Some(slot as i16)),
                lights: formats_sly3::scene::light_classes(&disc.classes),
                ..Default::default()
            };
            let build = formats_sly3::scene::build(&fw, opts);
            // Option set 0's fog and shadow colour, and the lights (rendering.md 2 and 6), as the viewer draws them.
            let sets: Vec<_> = fw.option_sets.sets.first().into_iter().collect();
            let lighting =
                stage::light::WorldLighting::with(&build.lights, &sets, fw.data, &stage::light::WorldOptions::SLY3);
            (build, lighting)
        };
        let scene = &build.scene;
        let opts = Sly3Options {
            slot: Some(slot as i16),
            info: WorldInfo { index: slot as u32, id, task_mask: 0, identity: id },
            multipliers: disc.tangent_multipliers,
            ..Sly3Options::default()
        };
        // Sly 3 scripts: the per-class option map from the player's executable (specs/sly3/scripts.md 5).
        let option_map = script::OptionMap::from_sly3_executable(&disc.executable).ok();
        let mut world = World::load_sly3(bytes, &disc.options, opts).map_err(|e| format!("{label}: {e}"))?;
        // The guards' pockets: the executable's guard table, row episode − 2 (guards.md 8), set before the
        // behaviours start in `Game::new`.
        world.shared.pockets =
            engine::gang::sly3::pocket_table(&disc.executable, engine::gang::sly3::GUARD_POCKETS_VA).ok();
        world.shared.episode = entry.episode as u8;
        let cfg = GameConfig { seed: self.seed, option_map, no_scripts: false };
        let mut systems = Systems::null();
        systems.camera = alloc::boxed::Box::new(FreeCamera::default());
        systems.player =
            alloc::boxed::Box::new(crew::CrewPlayer::new(self.arrival_id, checkpoint_classes(&disc.classes)));
        let mut game = Game::new(world, Progress::new_game(), systems, cfg);
        let input = stage::prep::SceneInput::sly3(&build, Some(&lighting));
        self.gpu =
            upload_stage(&mut self.renderer, input, None, &[], |_| Ok(())).map_err(|e| format!("{label}: {e}"))?;
        // A fresh free camera: `Game::new` configured the first one's lens from the world options by their Sly 1
        // meanings, which Sly 3 options do not have. It flies on its own: with `follow` off it must not track the
        // played character either.
        game.systems.camera = alloc::boxed::Box::new(FreeCamera { follow: false, ..FreeCamera::default() });
        if game.world.player.is_some() && self.follow {
            game.systems.camera = alloc::boxed::Box::new(engine::camera::system::CameraSystem::new(
                engine::camera::CameraConstants::SPEC,
                Default::default(),
            ));
            let p = game.world.player.and_then(|h| game.world.position(h)).unwrap_or([0.0; 3]);
            game.systems.camera.cut([p[0], p[1] - 450.0, p[2] + 250.0], p);
        } else {
            let (c, r) = view_point(scene);
            game.systems.camera.cut([c[0], c[1] - r * 0.4, c[2] + r * 1.2], c);
        }
        if let (Some(engine), Some(ad)) = (self.sound.as_mut(), self.audio_disc.as_mut()) {
            engine.stop_all();
            let audio = ad.load_world_audio(slot).map_err(|e| format!("{label}: sound: {e}"))?;
            audio.install(engine);
        }
        let ids: Vec<u32> = disc.worlds.iter().map(|w| w.world_id()).collect();
        let wipes = self.flow.as_ref().map_or_else(|| self.wipes.clone(), flow::Flow::wipes);
        self.flow = Some(flow::Flow::new(&game.world, &ids, slot, self.arrival_id, wipes));
        self.safe_house = safehouse::SafeHouse::from_world(&game.world);
        self.in_safe_house = false;
        self.shop = None;
        // The job system: the progress block goes on from the previous world (with what it carried, unless this
        // load starts fresh); the first world makes a zeroed block from its save tree.
        let (progress, carry) = match self.jobs.take() {
            Some(j) => {
                let c = j.carry();
                (Some(j.progress), Some(c))
            }
            None => (self.progress.take(), None),
        };
        let progress = progress.or_else(|| save_tree(&game.world).map(|t| save::sly3::progress(Arc::new(t))));
        let carry = if core::mem::replace(&mut self.fresh, false) { None } else { carry };
        if let Some(p) = progress {
            self.jobs = Some(jobs::attach(&mut game.world, p, carry, self.queued_task.take()));
        }
        self.last_task = None;
        // Scripts: none carry over; the arrival point's script (option 845) starts.
        self.scripts = commands::Scripts::new(self.seed ^ slot as u64);
        let classes = checkpoint_classes(&disc.classes);
        if let Some(a) = flow::find_arrival(&game.world, self.arrival_id, &classes)
            && let Some(script) = game.world.options(a.handle).and_then(|o| o.i32(flow::opt::ARRIVAL_SCRIPT))
        {
            self.scripts.start(&game.world, script, -1);
        }
        self.current = Some(Sly3Current { slot, name: entry.name, id, objects: game.world.len() });
        self.game = Some(game);
        self.message = None;
        Ok(())
    }

    /// Loads world `slot` entered at arrival point `point` (level-flow.md 6).
    pub fn load_at(&mut self, slot: usize, point: i32) -> Result<(), String> {
        self.arrival_id = point;
        self.load_slot(slot)
    }

    /// Starts episode `episode` (level-flow.md 3): its first world (or the exception), at point 424, or the task the
    /// start chose at its checkpoint when the world holds it.
    pub fn start_episode(&mut self, episode: usize, progress: &flow::EpisodeProgress) -> Result<(), String> {
        let disc = self.disc.as_ref().ok_or("no disc")?;
        let first = flow::first_worlds(&disc.executable).map_err(|e| e.to_string())?;
        let start =
            flow::episode_start(&first, episode, progress).ok_or_else(|| format!("episode {episode}: no start"))?;
        self.episode = episode;
        self.started_task = start.task;
        self.queued_task = start.task;
        self.fresh = true;
        self.load_at(start.slot, start.point)?;
        if let Some(task) = start.task
            && let Some(point) = self.game.as_ref().and_then(|g| flow::task_checkpoint(&g.world, task))
            && point != start.point
        {
            self.load_at(start.slot, point)?;
        }
        Ok(())
    }

    /// The progress block (the save's contents).
    pub fn progress(&self) -> Option<&save::sly2::Progress> {
        self.jobs.as_ref().map(|j| &j.progress).or(self.progress.as_ref())
    }

    fn progress_mut(&mut self) -> Option<&mut save::sly2::Progress> {
        match self.jobs.as_mut() {
            Some(j) => Some(&mut j.progress),
            None => self.progress.as_mut(),
        }
    }

    /// The coins (`loot/ccoin` in the progress block).
    pub fn coins(&self) -> i32 {
        self.progress().map_or(0, save::sly3::coins)
    }

    /// Sets the coins.
    pub fn set_coins(&mut self, coins: i32) {
        if let Some(p) = self.progress_mut() {
            p.set_named(&["loot", "ccoin"], None, save::sly2::SaveValue::Int(coins));
        }
    }

    /// The item-table records owned (`powerUps/mppukfAcquired`).
    pub fn owned(&self) -> Vec<usize> {
        let n = engine::gang::sly3::ITEM_COUNT;
        self.progress().map_or_else(Vec::new, |p| (0..n).filter(|&i| save::sly3::owned(p, i)).collect())
    }

    /// Buys item-table record `item` at the open shop: the coins go down and the item is owned (both in the
    /// progress block).
    pub fn buy(&mut self, item: usize) -> Result<(), safehouse::BuyError> {
        let coins = self.coins();
        let shop = self.shop.as_mut().ok_or(safehouse::BuyError::NotListed)?;
        let left = shop.buy(item, coins)?;
        self.set_coins(left);
        if let Some(p) = self.progress_mut() {
            p.set_named(&["powerUps", "mppukfAcquired"], Some(item), save::sly2::SaveValue::Flag(true));
        }
        Ok(())
    }

    /// Asks exit `exit` to fire at `point` (−1: its own), as command `0x68` does. The load happens at the top of the
    /// next frame.
    pub fn fire_exit(&mut self, exit: i32, point: i32) -> bool {
        let Some(f) = self.flow.as_mut() else { return false };
        if !f.fire(exit, point) {
            return false;
        }
        f.update(None, 0.0);
        true
    }

    /// The restart (game event 12): the current world at the stored checkpoint, with wipe 875.
    pub fn restart(&mut self) {
        if let Some(f) = self.flow.as_mut() {
            f.restart(&mut self.rng);
        }
    }

    /// Enters the hub's safe house (false when the world has none).
    pub fn enter_safe_house(&mut self) -> bool {
        self.in_safe_house = self.safe_house.is_some();
        self.in_safe_house
    }

    /// The pending load of the flow, performed.
    fn perform_load(&mut self) {
        let Some(req) = self.flow.as_mut().and_then(|f| f.request.take()) else { return };
        if let Err(e) = self.load_at(req.slot, req.point) {
            self.message = Some(e);
        }
        self.last_load = Some(req);
    }

    /// The safe house's input (left and right step, Cross fires the shown state's action), and the input the game
    /// gets: the d-pad and Cross are the safe house's while in it; a take-out switches to the character next frame.
    fn safe_house_input(&mut self, input: &FrameInput) -> FrameInput {
        let mut out = *input;
        if let Some(m) = self.take_out.take() {
            use engine::gang::sly3::Member;
            let b = match m {
                Member::Sly => platform_api::Buttons::LEFT.0,
                Member::Murray => platform_api::Buttons::UP.0,
                _ => platform_api::Buttons::RIGHT.0,
            };
            out.pressed |= b;
        }
        if !self.in_safe_house {
            return out;
        }
        let (Some(h), pressed) = (self.safe_house.as_mut(), input.pressed) else { return out };
        use platform_api::Buttons;
        if pressed & Buttons::LEFT.0 != 0 {
            h.step(-1);
        }
        if pressed & Buttons::RIGHT.0 != 0 {
            h.step(1);
        }
        if pressed & Buttons::B.0 != 0 {
            match h.press(safehouse::CROSS) {
                Some(safehouse::Pressed::OpenShop) => {
                    let table = self
                        .disc
                        .as_ref()
                        .and_then(|d| engine::gang::sly3::Sly3ItemTable::from_executable(&d.executable).ok());
                    let owned = self.owned();
                    self.shop = table.map(|t| safehouse::Shop::new(&t, self.episode as i32, &owned));
                }
                Some(safehouse::Pressed::TakeOut { member, script }) => {
                    // The placement's target script (it starts the episode's take-out script, 2061, with the
                    // character), and the switch to the character.
                    self.in_safe_house = false;
                    self.take_out = Some(member);
                    if let Some(g) = self.game.as_ref() {
                        self.scripts.start(&g.world, script, -1);
                    }
                }
                _ => {}
            }
        }
        let mask = Buttons::LEFT.0 | Buttons::RIGHT.0 | Buttons::UP.0 | Buttons::DOWN.0 | Buttons::B.0;
        out.pressed &= !mask;
        out.buttons &= !mask;
        out
    }

    /// Runs one frame for a measured interval (seconds). Returns false when the session should end.
    pub fn update(&mut self, input: &FrameInput, measured: f32) -> bool {
        self.perform_load();
        let input = &self.safe_house_input(input);
        if let Some(g) = self.game.as_mut() {
            g.frame(input, measured);
            let touched = input.buttons != 0 || input.left != [0.0; 2] || input.right != [0.0; 2];
            if let Some(j) = self.jobs.as_mut() {
                jobs::step(j, &mut g.world, measured, touched);
            }
            // Scripts, then what they asked.
            let view = commands::View { line_playing: false };
            self.scripts.update(Some(&g.world), &view, measured);
            for r in core::mem::take(&mut self.scripts.requests) {
                apply_request(r, &mut g.world, self.jobs.as_mut(), self.flow.as_mut());
            }
            // The current task's script (option 1408) starts when the task becomes current.
            if let Some(j) = self.jobs.as_mut() {
                if j.current != self.last_task
                    && let Some(script) = j.current.and_then(|t| j.graph.tasks[t].script)
                {
                    self.scripts.start(&g.world, script, -1);
                }
                self.last_task = j.current;
                // Exits fired by jobs (option 1381) and tasks (1406).
                if let Some(f) = self.flow.as_mut() {
                    for exit in core::mem::take(&mut j.fired_elsewhere) {
                        f.fire(exit, -1);
                    }
                }
            }
            if let Some(f) = self.flow.as_mut() {
                let p = g.world.player.and_then(|h| g.world.position(h));
                f.update(p, measured);
            }
            if let Some(s) = self.sound.as_mut() {
                let c = g.render.camera;
                let fwd = [c.target[0] - c.eye[0], c.target[1] - c.eye[1]];
                let left = [-fwd[1], fwd[0], 0.0];
                let len = render::math::sqrt(left[0] * left[0] + left[1] * left[1]).max(1e-6);
                s.set_listener(audio::Listener {
                    position: c.eye,
                    velocity: [0.0; 3],
                    left: [left[0] / len, left[1] / len, 0.0],
                });
                s.update(measured);
            }
        }
        true
    }

    /// Draws the frame.
    pub fn render(&mut self) -> Result<FrameStats, RenderError> {
        let Some(g) = self.game.as_ref() else { return Ok(FrameStats::default()) };
        let c = g.render.camera;
        let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
        let camera =
            Camera { eye: v(c.eye), target: v(c.target), up: v(c.up), fov_y: c.fov_y, near: c.near, far: c.far };
        self.renderer.render(&camera, self.gpu.clear)
    }

    /// The status lines.
    pub fn status_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        match &self.current {
            Some(c) => v.push(format!(
                "Sly 3 {} (slot {}, world {:#x}): {} objects, {} drawn",
                c.name, c.slot, c.id, c.objects, self.gpu.stats.instances
            )),
            None => v.push("no world".to_string()),
        }
        if let Some(m) = &self.message {
            v.push(m.clone());
        }
        v
    }

    /// Instances drawn.
    pub fn drawn(&self) -> u32 {
        self.gpu.stats.instances
    }

    /// A hash of the simulation for determinism checks.
    pub fn state_hash(&self) -> u64 {
        let mut h = engine::game::Fnv::new();
        h.u64(self.game.as_ref().map_or(0, Game::state_hash));
        h.u64(self.current.as_ref().map_or(u64::MAX, |c| c.slot as u64));
        h.finish()
    }
}
