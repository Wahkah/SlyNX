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
use engine::gang::sly3::Member;
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats::iso::ReadAt;
use formats::scene::Scene;
use formats_sly3::Disc;
use render::math::Vec3;
use render::{Backend, Camera, FrameStats, RenderError, Renderer};

use crate::sly2::level::{StaticLevel, upload_stage};

pub mod clips; // Sly 3 hook: clips' add and remove events (knock-out clips).
pub mod commands;
pub mod crew;
pub mod flow;
pub mod front; // Sly 3 hook: the title, the save slots, the episode menu and the movies.
pub mod grid; // Sly 3 hook: the gadget assignment screen.
pub mod guards; // Sly 3 hook: guard command lists (0x62).
pub mod jobs;
pub mod layer; // Sly 3 hook: the 2D layer (fonts, HUD, pause menu, job screens; `ui::sly2`).
pub mod lock; // Sly 3 hook: the cell lock minigame (class 79).
pub mod photo; // Sly 3 hook: the binocucom's photo mode and photo targets (class 43).
pub mod safehouse;
pub mod waypoints;

/// Class 40 and every class derived from it (the class table's base links, object-stream.md 3.1).
pub fn checkpoint_classes(classes: &[formats_sly3::exe::ClassEntry]) -> Vec<i16> {
    derived_classes(classes, crew::CHECKPOINT_CLASS)
}

/// Class `root` and every class derived from it, by the executable's class table (each entry's base pointer).
pub fn derived_classes(classes: &[formats_sly3::exe::ClassEntry], root: i16) -> Vec<i16> {
    let (va, _) = formats_sly3::exe::CLASS_TABLE;
    let address = |i: usize| va + (i * formats_sly3::exe::CLASS_ENTRY_SIZE) as u32;
    let Some(root_at) = classes.iter().position(|c| c.class == root as i32).map(address) else {
        return alloc::vec![root];
    };
    let root = root_at;
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

/// What a script request asks of the session itself.
enum SessionAction {
    /// Switch to a crew member (`0x13` on a playable character).
    TakeOut(engine::gang::sly3::Member),
    /// A script holds (true) or releases the pad (`0x1F`).
    Pause(i32, bool),
    /// `0x46` / `0x6E`: the binocucom opens (looking at the object, −1 none) or closes.
    Binocucom(bool, i32),
    /// `0x62`: a guard gets a command list.
    GuardList(i32, i32),
    /// Control handed to the safe house (method 983 on safe house 15065, characters.md 6).
    SafeHouse,
    /// `0x60` / `0x61`: a hint shown (string, seconds) or cleared.
    Hint(Option<(i32, f32)>),
}

/// Bentley's bomb stand-in (see the session's update): the first press plants, the second sets it off.
fn bomb_press(bomb: &mut Option<engine::object::Handle>, world: &mut World) {
    use engine::gang::sly3::gadget_class;
    match bomb.take() {
        None => {
            // Bentley's pooled bomb (the copy his spawner 623 holds; `world::sly3::load`).
            let (Some(b), Some(at)) = (
                world.handles().find(|&h| world.class(h) == Some(gadget_class::BOMB)),
                world.player.and_then(|p| world.position(p)),
            ) else {
                return;
            };
            world.set_world_transform(b, formats::geometry::Affine { t: at, ..formats::geometry::Affine::IDENTITY });
            world.set_in_world(b, true);
            *bomb = Some(b);
        }
        Some(b) => {
            let radius = engine::gang::gadgets::blast_of(7).map_or(300.0, |s| s.radius as f32);
            let Some(at) = world.position(b) else { return };
            let hit: Vec<engine::object::Handle> = world
                .handles()
                .filter(|&h| h != b && world.module(h) >= 0 && world.in_world(h))
                .filter(|&h| world.class(h).is_some_and(|c| !matches!(c, 135 | 169 | 168)))
                .filter(|&h| {
                    world.position(h).is_some_and(|p| {
                        let d = [p[0] - at[0], p[1] - at[1], p[2] - at[2]];
                        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= radius * radius
                    })
                })
                .collect();
            // The last blast's hit records are freed first, so they do not pile up.
            world.sly3_free_hit_records();
            for h in hit {
                if let Some(record) = world.sly3_hit_record(h, b) {
                    world.raise(h, 12, alloc::vec![script::Value::Object(Some(record.to_script()))]);
                }
            }
            world.set_in_world(b, false);
        }
    }
}

/// Player 0's control handed to object `h` (command 0x13, the world object's method 983; characters.md 6): a crew
/// member is switched to, the safe house (class 69) is entered, anything else is driven (and brought into the world
/// with what holds it).
fn control_to(world: &mut World, h: engine::object::Handle) -> Option<SessionAction> {
    match world.class(h) {
        Some(safehouse::SAFEHOUSE_CLASS) => return Some(SessionAction::SafeHouse),
        c => {
            if let Some(m) = c.and_then(engine::gang::sly3::Member::from_class).filter(|m| m.core().is_some()) {
                return Some(SessionAction::TakeOut(m));
            }
        }
    }
    let chain: Vec<_> = core::iter::successors(Some(h), |&q| world.parent(q)).collect();
    for q in chain {
        if !world.in_world(q) {
            world.set_in_world(q, true);
        }
    }
    world.shared.driving = Some(h);
    None
}

/// Carries out one script request (commands.rs) on the world, the job system, the level flow and the player.
/// Dialogue, screens (the 2D layer takes those), goals and music are not run by the session yet.
fn apply_request(
    r: commands::Request,
    world: &mut World,
    player: &mut dyn engine::game::systems::PlayerSystem,
    jobs: Option<&mut engine::game::sly2_jobs::Jobs>,
    flow: Option<&mut flow::Flow>,
    waypoints: &[waypoints::Waypoint],
) -> Option<SessionAction> {
    use commands::Request;
    match r {
        Request::Place { target, at } => {
            let to = world.find_sly3_id(at, None).and_then(|h| world.world_transform(h))?;
            let h = world.find_sly3_id(target, None)?;
            if world.player == Some(h) {
                player.place(world, to);
            } else if world.class(h).and_then(Member::from_class).is_some() {
                // Another crew member: the crew moves it (it writes the members' objects every frame).
                world.shared.sly3_crew_moves.push((h, Some(to.t), None));
            } else {
                world.set_world_transform(h, to);
            }
        }
        // A placement's run-time instance (missions.md 4.1a). Stand-in: one per placement, the copy the load made
        // (out of the world until taken), brought into the world at the placement and taken out when released. Flags
        // 1 (the current instance) and 2 (locked) change nothing here.
        Request::Instance { placement, take } => {
            let h = world
                .slots_with_sly3_id(placement)
                .iter()
                .filter_map(|&k| world.handle(k))
                .find(|&h| world.class(h) == Some(formats_sly3::stream::PLACEMENT))?;
            world.set_in_world(h, take);
        }
        Request::Face { target, at } => {
            let (Some(h), Some(to)) = (world.find_sly3_id(target, None), world.find_sly3_id(at, None)) else {
                return None;
            };
            if world.class(h).and_then(Member::from_class).is_some()
                && let (Some(p), Some(q)) = (world.position(h), world.position(to))
            {
                world.shared.sly3_crew_moves.push((h, None, Some(engine::math::atan2(q[1] - p[1], q[0] - p[0]))));
            }
        }
        Request::Control { object, .. } => {
            // The object with that ID; a placement (class 8; its ID names its copy's root) stands for the object
            // it placed.
            let slots = world.slots_with_sly3_id(object);
            let handles: Vec<_> = slots.iter().filter_map(|&k| world.handle(k)).collect();
            let h = handles
                .iter()
                .copied()
                .find(|&h| world.class(h) != Some(formats_sly3::stream::PLACEMENT))
                .or_else(|| handles.first().and_then(|&p| world.children(p).next()))?;
            return control_to(world, h);
        }
        Request::Activate { object } => {
            // scripts.md 7.3: by the object's class. Exits fire, points become the checkpoint, a screen fade (138)
            // comes into the world (the 2D layer draws it); the classes with routines of their own (21, 33, 71,
            // 77, 80, 93, 113) are not modelled; any other gets script event 5.
            let h = world.find_sly3_id(object, None)?;
            match world.class(h)? {
                24 => {
                    if let Some(f) = flow {
                        f.fire(object, -1);
                    }
                }
                39 | 40 => {
                    if let Some(f) = flow {
                        f.checkpoint = object;
                    }
                }
                138 => world.set_in_world(h, true),
                21 | 33 | 71 | 77 | 80 | 93 | 113 => {}
                _ => world.raise(h, 5, Vec::new()),
            }
        }
        Request::ReturnToWaypoint => {
            // scripts.md 7.3 (interpretation): the nearest active waypoint of kind 0 or 1, else of kind −1; the
            // player is put 300 units above it.
            let at = world.player.and_then(|p| world.position(p))?;
            let near = |kinds: &[i32]| {
                waypoints
                    .iter()
                    .filter(|w| kinds.contains(&w.kind) && world.is_active(w.handle))
                    .filter_map(|w| world.world_transform(w.handle))
                    .min_by(|a, b| {
                        let d = |t: &formats::geometry::Affine| {
                            (0..3).map(|k| (t.t[k] - at[k]) * (t.t[k] - at[k])).sum::<f32>()
                        };
                        d(a).total_cmp(&d(b))
                    })
            };
            let mut to = near(&[0, 1]).or_else(|| near(&[-1]))?;
            to.t[2] += 300.0;
            player.place(world, to);
        }
        Request::Module { script, value, copy } => {
            let h = copy.or_else(|| world.find_sly3_id(script, None))?;
            world.raise(h, 4, alloc::vec![script::Value::Int(value)]);
        }
        Request::Hint { string, seconds } => return Some(SessionAction::Hint(Some((string, seconds)))),
        Request::ClearHint => return Some(SessionAction::Hint(None)),
        Request::Pause { script, on } => return Some(SessionAction::Pause(script, on)),
        Request::Binocucom { open, look_at, .. } => return Some(SessionAction::Binocucom(open, look_at)),
        Request::GuardList { guard, list } => return Some(SessionAction::GuardList(guard, list)),
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
    None
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
    /// Vehicles hook: the projectiles' streaks (`crate::shot_marks`).
    shot_marks: Option<crate::shot_marks::ShotMarks>,
    /// The last frame's interval (texture animations advance by it when the frame is drawn).
    frame_dt: f32,
    /// A script paused the run (`0x1F`): the game gets no pad input while one of these scripts runs.
    pub script_paused: bool,
    /// Sly 3 hook (interactables.md 5): the cell lock started by method 1600, while one runs.
    pub lock: Option<lock::Lock>,
    /// Sly 3 hook (interactables.md 5a): the binocucom's photo mode, while open.
    pub photo: Option<photo::Photo>,
    /// Sly 3 hook (guards.md 4a): the guards running command lists.
    pub guards: Vec<guards::GuardRun>,
    /// Clips playing for their add and remove events (the guards' knock-out clips, guards.md 4b).
    pub clips: clips::Clips,
    /// Sly 3 hook (missions.md 4.1a): the hint scripts show (string, seconds left); drawing it is the 2D layer's.
    pub hint: Option<(i32, f32)>,
    /// The last dialogue line asked for (line ID), for tests.
    pub last_line: Option<i32>,
    /// Sly 3 hook (gadgets.md 3, 7): Bentley's planted bomb (class 114, `bentley_mine`), while one is planted.
    pub bomb: Option<engine::object::Handle>,
    paused_by: alloc::collections::BTreeSet<i32>,
    /// The world's mission waypoints (class 120) and, by base class, the classes deriving from it.
    pub waypoints: Vec<waypoints::Waypoint>,
    derived: alloc::collections::BTreeMap<i16, Vec<i16>>,
    /// Sly 3 hook (2D layer): the fonts, the layer and the screens.
    pub ui: layer::Sly3Ui,
    /// Sly 3 hook (front end): the title, the slots, the episode menu, the movies.
    front: front::Front,
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
        let grid = Self::grid_state(disc.as_ref().map(|d| &d.executable)); // Sly 3 hook (gadget grids)
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
            shot_marks: None,
            ui: layer::Sly3Ui::with_grid(grid), // Sly 3 hook (gadget grids)
            front: front::Front::default(),
            frame_dt: 0.0,
            script_paused: false,
            lock: None,
            photo: None,
            guards: Vec::new(),
            clips: clips::Clips::default(),
            hint: None,
            last_line: None,
            bomb: None,
            paused_by: alloc::collections::BTreeSet::new(),
            waypoints: Vec::new(),
            derived: alloc::collections::BTreeMap::new(),
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

    /// Vehicles hook: makes the task named `name` current, forced (testing and the smoke run start a job's machine
    /// this way). Returns whether the task exists.
    pub fn start_task(&mut self, name: &str) -> bool {
        let (Some(g), Some(j)) = (self.game.as_mut(), self.jobs.as_mut()) else { return false };
        let Some(t) = j.graph.tasks.iter().position(|t| t.name == name) else { return false };
        j.request_current(t, true);
        j.apply(&mut g.world);
        true
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
        // The camera follows the driven machine (`camera::sly2`'s follow target), the characters again without one.
        g.world.shared.camera_follow = machine;
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
        let (build, lighting, glyphs, strings, set, context, grid_files) = {
            let fw = formats_sly3::World::parse_with(&bytes, &disc.options, disc.tangent_multipliers)
                .map_err(|e| format!("{label}: {e}"))?;
            // The option set from the task progress (conditions.md 3), the first one on a fresh load.
            let progress = self.jobs.as_ref().map(|j| &j.progress).or(self.progress.as_ref());
            let set = flow::choose_option_set(&fw.option_sets, progress);
            let context = fw.context_for_set(Some(slot as i16), set);
            // Sly 3 hook (2D layer): the fonts (read once) and the world's strings.
            let glyphs = if self.ui.font.is_none() { layer::font_glyphs(&fw) } else { None };
            // Sly 3 hook (gadget grids): the grid screens' files, read with the fonts for their icons.
            let grid_files = if glyphs.is_some() { grid::screen_files(&fw) } else { Vec::new() };
            let strings = layer::strings_of(&fw);
            let opts = formats_sly3::scene::SceneOptions {
                context: context.clone(),
                lights: formats_sly3::scene::light_classes(&disc.classes),
                // The default passes plus the shadow hulls (6), projected volumes (10, 14) and kind 5's pass 15
                // (rendering.md 5), as the viewer draws them.
                passes: formats_sly3::scene::SceneOptions::default().passes
                    | (1 << 6)
                    | (1 << 10)
                    | (1 << 14)
                    | (1 << 15),
                ..Default::default()
            };
            let build = formats_sly3::scene::build(&fw, opts);
            // The chosen option set's fog and shadow colour, and the lights (rendering.md 2 and 6).
            let sets: Vec<_> = fw.option_sets.sets.get(set).into_iter().collect();
            let lighting =
                stage::light::WorldLighting::with(&build.lights, &sets, fw.data, &stage::light::WorldOptions::SLY3);
            (build, lighting, glyphs, strings, set, context, grid_files)
        };
        if let Some(mut g) = glyphs {
            // Sly 3 hook (gadget grids): the grids' icons as a third font of the sheet.
            let screens: Vec<(i32, formats_sly2::screen::Screen)> = grid_files
                .into_iter()
                .filter_map(|(id, f)| {
                    let file = formats::compress::decompress(&disc.read_file(&f).ok()?).ok()?;
                    Some((id, formats_sly3::screen::read_file(&file).ok()?))
                })
                .collect();
            let (icons, art) = grid::bake(&self.ui.grid.grids, &screens);
            self.ui.grid.art = art;
            g.truncate(2);
            g.push(icons);
            self.ui.font = Some(ui::sly2::font::FontSheet::build(&g));
        }
        self.ui.strings = strings;
        self.ui.subtitle = None;
        let scene = &build.scene;
        let opts = Sly3Options {
            slot: Some(slot as i16),
            context: Some(context),
            set,
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
        systems.player = {
            let mut crew = crew::CrewPlayer::new(self.arrival_id, checkpoint_classes(&disc.classes));
            // Sly 3 hook (gadget grids): the crew's item table.
            crew.items = self.ui.grid.items.as_ref().map(engine::gang::sly3::Sly3ItemTable::to_item_table);
            alloc::boxed::Box::new(crew)
        };
        let mut game = Game::new(world, Progress::new_game(), systems, cfg);
        // The crew's skinned meshes (`crate::sly2::skin::crew_draws`), posed every frame; their scene draws are left
        // out.
        let skinned = crate::sly2::skin::crew_draws(&game.world, &build.material_base);
        let crew_records: Vec<(u32, u32)> = if skinned.is_empty() {
            Vec::new()
        } else {
            let w = &game.world;
            w.handles()
                .filter(|&h| w.class(h).is_some_and(|c| engine::gang::sly3::Member::from_class(c).is_some()))
                .filter_map(|h| w.sly3_record(h))
                .map(|r| (r.header_offset, r.body_offset + r.body_size))
                .collect()
        };
        let input =
            stage::prep::SceneInput { skip: &crew_records, ..stage::prep::SceneInput::sly3(&build, Some(&lighting)) };
        let sheet = self.ui.font.as_ref().map(|f| (f.width, f.height, f.pixels.clone()));
        let mut ui_layer = None;
        let mut marks = None;
        let mut video = None;
        let made = upload_stage(&mut self.renderer, input, Some(&game.world), &skinned, |r| {
            ui_layer = Some(crate::ui_layer::UiLayer::new(r, sheet)?); // Sly 3 hook: the 2D layer.
            video = Some(render::video::VideoLayer::new(r)?); // Sly 3 hook: the movies' picture.
            marks = Some(crate::shot_marks::ShotMarks::upload(r)?); // Vehicles hook: the projectiles' streaks.
            Ok(())
        });
        self.shot_marks = marks;
        self.ui.layer = ui_layer;
        self.front.movies.set_layer(video);
        self.gpu = match made {
            Ok(gpu) => gpu,
            Err(e) => {
                // The renderer holds an empty level now: nothing may keep handles into the old one (the layers made
                // before the failure included).
                self.gpu = StaticLevel::default();
                self.shot_marks = None;
                self.ui.layer = None;
                self.front.movies.set_layer(None);
                self.game = None;
                self.current = None;
                return Err(format!("{label}: {e}"));
            }
        };
        // A fresh free camera: `Game::new` configured the first one's lens from the world options by their Sly 1
        // meanings, which Sly 3 options do not have. It flies on its own: with `follow` off it must not track the
        // played character either.
        game.systems.camera = alloc::boxed::Box::new(FreeCamera { follow: false, ..FreeCamera::default() });
        if game.world.player.is_some() && self.follow {
            game.systems.camera = alloc::boxed::Box::new(engine::camera::system::CameraSystem::new(
                engine::camera::CameraConstants::SPEC,
                Default::default(),
            ));
            // Configured here, with the load, not on the first frame: it builds the camera's collision world from
            // every solid (up to about 30 ms in m_ext), which would otherwise land on one frame.
            game.systems.camera.configure(&game.world);
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
        self.waypoints = waypoints::waypoints(&game.world);
        for w in &self.waypoints {
            self.derived.entry(w.player_class).or_insert_with(|| derived_classes(&disc.classes, w.player_class));
        }
        // Sly 3 hook (missions.md 4.1a): every class's derived classes, for scripts' "derived from class n".
        game.world.shared.sly3_derived =
            (0..256i16).map(|c| (c, derived_classes(&disc.classes, c))).filter(|(_, d)| d.len() > 1).collect();
        self.bomb = None;
        self.clips = clips::Clips::default();
        self.guards.clear();
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
        if let Some(a) = flow::find_arrival(&game.world, self.arrival_id, &classes) {
            // The point's (task, state) pairs (option 847; level-flow.md 5.2), applied on arrival.
            if let Some(j) = self.jobs.as_mut() {
                for &(task, state) in &a.task_states {
                    jobs::set_task(j, &mut game.world, task, state);
                }
            }
            if let Some(script) = game.world.options(a.handle).and_then(|o| o.i32(flow::opt::ARRIVAL_SCRIPT)) {
                self.scripts.start(&game.world, script, -1);
            }
        }
        self.current = Some(Sly3Current { slot, name: entry.name, id, objects: game.world.len() });
        self.game = Some(game);
        self.message = None;
        self.ui.grid.open = None;
        self.sync_crew(); // Sly 3 hook (gadget grids): owned items and slots from the progress block.
        self.front_world_loaded(slot); // Sly 3 hook: the menu world shows the episode menu.
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

    /// The player's defeat (game event 12): unless the current job ignores it (option 1362), the job counts a
    /// failure and the world restarts at the stored checkpoint. Returns whether it restarts.
    pub fn defeat(&mut self) -> bool {
        let reacts = match (self.jobs.as_mut(), self.game.as_mut()) {
            (Some(j), Some(g)) => jobs::defeat(j, &mut g.world),
            _ => true,
        };
        if reacts {
            self.restart();
        }
        reacts
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
        // The member taken out (or handed control by 0x13) is named for the crew (characters.md 6).
        if let Some(m) = self.take_out.take()
            && let Some(g) = self.game.as_mut()
        {
            g.world.shared.sly3_switch_to = Some(m);
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
        self.frame_dt = measured;
        self.perform_load();
        // Sly 3 hook (front end): a movie holds everything; the title and the episode menu take the input.
        let front = self.front_open();
        let Some(input) = self.update_front(input, measured).or_else(|| (!self.movie_playing()).then_some(*input))
        else {
            return !self.ui.quit;
        };
        let input = &input;
        // Sly 3 hook (2D layer): the lines and screens scripts ask for; the game gets no input while a screen holds.
        let mut lines: Vec<Option<(i32, i32)>> = Vec::new();
        let mut screens: Vec<(i32, bool)> = Vec::new();
        let raw = *input;
        // Player 0's holds (missions.md 4.2): one per pausing script, one while a screen holds the pad.
        let holds = self.paused_by.len() as u32 + u32::from(self.screens_hold());
        let input = &if self.screens_hold() || self.script_paused {
            FrameInput::default()
        } else {
            // Sly 3 hook (2D layer): the job-failed screen keeps its dismiss button.
            self.safe_house_input(&layer::without_dismiss(input, self.ui.failed.is_some()))
        };
        if let Some(g) = self.game.as_mut() {
            g.world.shared.sly3_holds = holds;
            // Sly 3 hook (photo mode, interactables.md 5a): R3 opens and closes the binocucom on the controlled
            // character (Sly 2's rule); while it is open it takes the pad and the crew stands still.
            if input.pressed & platform_api::Buttons::RSTICK.0 != 0 && self.lock.is_none() {
                self.photo = match self.photo.take() {
                    Some(_) => None,
                    None => photo::Photo::open(&g.world, photo::params(g.render.camera.fov_y)),
                };
            }
            let pad = *input;
            let idle = FrameInput::default();
            let input = if self.photo.is_some() { &idle } else { input };
            g.frame(input, measured);
            // World-start step 3 (missions.md 4.1a), once the frame has run the restore's events: the queued requests
            // run in mode 1, and the events they raise run while the mode still reads 1, until nothing more is
            // queued (at most 16 rounds); then the "current when available" pass.
            if g.world.shared.sly3_restore_mode
                && let Some(j) = self.jobs.as_mut()
            {
                for _ in 0..16 {
                    jobs::run_queued(j, &mut g.world);
                    if g.world.events.is_empty() {
                        break;
                    }
                    g.flush_events();
                    let s = &g.world.shared;
                    if s.task_calls.is_empty() && s.task_requests.is_empty() {
                        break;
                    }
                }
                jobs::finish_world_start(j, &mut g.world);
            }
            // Stand-in (gadgets.md: the gadget slots' pad path is not traced): as Bentley, L (L1) plants the world's
            // bomb (class 114, `bentley_mine`) at his feet, and L again sets it off, as the Trigger Bomb does. The blast
            // (radius 300, gadgets.md 7.1) hits every object with a script within that radius (standing in for the
            // blast's rays): each gets script event 12 with a hit record (class 196; missions.md 4.1a).
            if input.pressed & platform_api::Buttons::L.0 != 0
                && g.world.player.and_then(|p| g.world.class(p)).and_then(Member::from_class) == Some(Member::Bentley)
            {
                bomb_press(&mut self.bomb, &mut g.world);
            }
            // Sly 3 hook (use points): the use button (Circle) uses the nearest active point within reach. Working
            // choice (the candidate search is not traced): nearest horizontally, reach as `UsePoint::user_of` has it.
            if input.pressed & platform_api::Buttons::A.0 != 0
                && let Some(p) = g.world.player.and_then(|h| g.world.position(h))
            {
                use engine::behavior::sly3::interactables::{class, use_point_used};
                let w = &g.world;
                let mut points: Vec<(f32, engine::object::Handle)> = w
                    .handles()
                    .filter(|&h| w.class(h) == Some(class::USE_POINT) && w.is_active(h))
                    .filter_map(|h| {
                        let q = w.position(h)?;
                        let d = [q[0] - p[0], q[1] - p[1]];
                        Some((d[0] * d[0] + d[1] * d[1], h))
                    })
                    .collect();
                points.sort_by(|a, b| a.0.total_cmp(&b.0));
                // The point's command script is its copy's (the gauntlet's buttons share one script ID per copy).
                if let Some((point, script)) =
                    points.into_iter().find_map(|(_, h)| use_point_used(&mut g.world, h).map(|s| (h, s)))
                    && let Some(script) = script
                {
                    let me = g.world.player.and_then(|h| g.world.sly3_id(h)).unwrap_or(-1);
                    self.scripts.start_in(&g.world, script, me, Some(point));
                }
            }
            let touched = input.buttons != 0 || input.left != [0.0; 2] || input.right != [0.0; 2];
            // Sly 3 hook (missions.md 4.1a): task event 29 for every object that received script event 5 (mission
            // waypoints broadcast their own, with 30 on leaving).
            let fifth: Vec<_> = core::mem::take(&mut g.world.shared.sly3_event5)
                .into_iter()
                .filter(|&h| g.world.class(h) != Some(waypoints::WAYPOINT_CLASS))
                .collect();
            if let Some(j) = self.jobs.as_mut() {
                for h in fifth {
                    j.task_event(waypoints::TASK_ENTER, Some(h));
                }
            }
            // Sly 3 hook: a cell lock started (method 1600) takes player 0's control and runs with the pad.
            for (h, user) in core::mem::take(&mut g.world.shared.sly3_lock_starts) {
                self.lock = Some(lock::Lock::start(h, user, &mut g.world, &mut self.scripts));
            }
            if let Some(l) = self.lock.as_mut() {
                l.update(input, measured, &mut g.world, &mut self.scripts);
                // Done (5) or given up (0): the lock no longer runs.
                if matches!(l.state, 0 | 5) {
                    self.lock = None;
                }
            }
            // Sly 3 hook (photo mode): an accepted shot plays the target's line, asks for state 2 on its task (a
            // checked request) and broadcasts task event 10; the binocucom closes once no listed target is pending.
            let targets = if self.photo.is_some() { photo::targets(&g.world) } else { Vec::new() };
            if let Some(p) = self.photo.as_mut()
                && let Some(shot) = p.update(&pad, measured, &g.world, &targets)
            {
                if let Some(line) = shot.line {
                    lines.push(Some((line, g.world.sly3_id(shot.target).unwrap_or(-1))));
                }
                if let Some(task) = shot.task {
                    g.world.shared.task_requests.push((task, 2));
                }
                if let Some(j) = self.jobs.as_mut() {
                    j.task_event(photo::TASK_EVENT, Some(shot.target));
                }
                if !p.listed_pending(&targets) {
                    self.photo = None;
                }
            }
            // Sly 3 hook (guards.md 4b): a knocked-out guard plays its knock-out clip, whose remove event takes it
            // out of the world.
            for h in core::mem::take(&mut g.world.shared.sly3_knocked_out) {
                if let Some(clip) = clips::knockout_clip(&g.world, h) {
                    self.clips.play(&g.world, h, clip);
                }
            }
            self.clips.step(&mut g.world, measured);
            // Sly 3 hook (guards.md 4a): the guards' command lists; their alarms are the command scripts'.
            for r in &mut self.guards {
                r.update(measured, &mut g.world, &mut self.scripts.alarms);
            }
            self.guards.retain(|r| !r.done());
            if let Some(j) = self.jobs.as_mut() {
                jobs::step(j, &mut g.world, measured, touched);
                // Vehicles hook: a job's machine is handed to the player while the job runs.
                let (job, task) = engine::behavior::vehicles::shots::job_of(j);
                engine::behavior::vehicles::shots::follow_job(&mut g.world, job, task.as_deref());
            }
            // Mission waypoints (missions.md 4.2): walking in raises the events that start a job.
            let derived = &self.derived;
            let derives = |c: i16, b: i16| derived.get(&b).is_some_and(|v| v.contains(&c));
            waypoints::update(&mut self.waypoints, &mut g.world, self.jobs.as_mut(), &derives);
            // Sly 3 hook: command scripts other scripts started (method 747, missions.md 4.1a).
            // Each on the script object called (its own copy: v_interpol's 88261, one per cell door).
            for (script, with, h) in core::mem::take(&mut g.world.shared.sly3_script_starts) {
                self.scripts.start_in(&g.world, script, with, Some(h));
            }
            // And those they stopped (method 749).
            for script in core::mem::take(&mut g.world.shared.sly3_script_stops) {
                self.scripts.stop(script);
            }
            // Alarms scripts set on command scripts (method 735) and runner contexts modules wrote (script ID 741).
            for (_, alarm) in core::mem::take(&mut g.world.shared.sly3_alarms) {
                self.scripts.alarms.insert(alarm);
            }
            for (script, with) in core::mem::take(&mut g.world.shared.sly3_context_writes) {
                if let Some(r) = self.scripts.running.iter_mut().find(|r| r.oid == script) {
                    r.context = with;
                }
            }
            // A hint's time counts down; a time that is not a finite number clears it now.
            if let Some((_, t)) = self.hint.as_mut() {
                *t = if t.is_finite() { (*t - measured).max(0.0) } else { 0.0 };
                if *t <= 0.0 {
                    self.hint = None;
                }
            }
            // Each running script's object, for scripts (script ID 740).
            g.world.shared.sly3_script_objects = self.scripts.running.iter().map(|r| (r.oid, r.object)).collect();
            // A command script starting gets script event 5 (missions.md 4.1a); the event broadcasts task event 29
            // with the script (`sly3_event5`).
            for (_, h) in core::mem::take(&mut self.scripts.started) {
                g.world.raise(h, 5, Vec::new());
            }
            // A command script that ran to its end gets script event 6 (working reading, `commands::Scripts::ended`).
            for (_, h) in core::mem::take(&mut self.scripts.ended) {
                g.world.raise(h, 6, Vec::new());
            }
            // Control scripts handed over (method 983).
            for h in core::mem::take(&mut g.world.shared.sly3_control_to) {
                match control_to(&mut g.world, h) {
                    Some(SessionAction::TakeOut(m)) => self.take_out = Some(m),
                    Some(SessionAction::SafeHouse) => self.in_safe_house = self.safe_house.is_some(),
                    _ => {}
                }
            }
            // Scripts, then what they asked.
            // The tasks' play counts (each focus key of the progress block) and the restore mode, for the object
            // tests 1888 to 1890.
            let plays = self.jobs.as_ref().map_or_else(Default::default, |j| {
                j.graph
                    .tasks
                    .iter()
                    .map(|t| {
                        let n = t.keys.focus.as_ref().and_then(|k| j.progress.get(k, None)).map_or(0, |v| v.as_i32());
                        (t.id, n)
                    })
                    .collect()
            });
            let view = commands::View { line_playing: false, plays, restoring: g.world.shared.sly3_restore_mode };
            self.scripts.update(Some(&g.world), &view, measured);
            for r in core::mem::take(&mut self.scripts.requests) {
                // Sly 3 hook (2D layer): the lines' subtitles.
                match &r {
                    commands::Request::PlayLine { line, object, .. } => {
                        lines.push(Some((*line, *object)));
                        self.last_line = Some(*line);
                    }
                    commands::Request::ClearSubtitle => lines.push(None),
                    commands::Request::Screen { screen, on } => screens.push((*screen, *on)),
                    _ => {}
                }
                match apply_request(
                    r,
                    &mut g.world,
                    &mut *g.systems.player,
                    self.jobs.as_mut(),
                    self.flow.as_mut(),
                    &self.waypoints,
                ) {
                    Some(SessionAction::TakeOut(m)) => self.take_out = Some(m),
                    Some(SessionAction::Pause(script, on)) => {
                        if on {
                            self.paused_by.insert(script);
                        } else {
                            self.paused_by.remove(&script);
                        }
                    }
                    Some(SessionAction::Binocucom(true, look_at)) => {
                        // The view opens with the photo mode (as R3 opens both), so a later R3 closes both.
                        if let (None, Some(p)) = (g.world.shared.camera_binocucom, g.world.player) {
                            engine::gang::toggle_binocucom(&mut g.world, p, false);
                        }
                        let fov = g.render.camera.fov_y;
                        self.photo = photo::Photo::open(&g.world, photo::params(fov));
                        let at = g.world.find_sly3_id(look_at, None).and_then(|h| g.world.position(h));
                        if let (Some(p), Some(at)) = (self.photo.as_mut(), at) {
                            p.look_at(at);
                        }
                    }
                    Some(SessionAction::Binocucom(false, _)) => {
                        // A script's close (0x6E) closes the view itself too, not only the photo mode.
                        self.photo = None;
                        g.world.shared.camera_binocucom = None;
                    }
                    Some(SessionAction::SafeHouse) => self.in_safe_house = self.safe_house.is_some(),
                    Some(SessionAction::Hint(h)) => self.hint = h,
                    Some(SessionAction::GuardList(guard, list)) => {
                        if let Some(run) = guards::GuardRun::new(&g.world, guard, list) {
                            self.guards.retain(|r| r.guard != run.guard);
                            self.guards.push(run);
                        }
                    }
                    None => {}
                }
            }
            // Conditions that asked a script's module (scripts.md 7.1): answered now, for the runner's next frame.
            for (oid, copy, value) in core::mem::take(&mut self.scripts.questions) {
                if let Some(h) = copy.or_else(|| g.world.find_sly3_id(oid, None)) {
                    let v = g.raise_now(h, 4, &[script::Value::Int(value)]);
                    self.scripts.answers.insert((oid, value), v.is_some_and(|v| v.is_true()));
                }
            }
            // A script's pause lasts while it runs.
            let scripts = &self.scripts;
            self.paused_by.retain(|o| scripts.is_running(*o));
            self.script_paused = !self.paused_by.is_empty();
            // The current task's script (option 1408) starts when the task becomes current.
            if let Some(j) = self.jobs.as_mut() {
                if j.current != self.last_task
                    && let Some(script) = j.current.and_then(|t| j.graph.tasks[t].script)
                {
                    // With player 0's controlled object as its context (missions.md 4).
                    let me = g.world.player.and_then(|h| g.world.sly3_id(h)).unwrap_or(-1);
                    self.scripts.start(&g.world, script, me);
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
        // Sly 3 hook (2D layer): subtitles, then the screens' frame.
        for l in lines {
            match l {
                Some((line, object)) => self.show_line(line, object),
                None => self.ui.subtitle = None,
            }
        }
        for (screen, on) in screens {
            self.screen_request(screen, on);
        }
        if !front && !self.front_open() {
            self.update_ui(&raw, engine::game::clock::frame_step(measured));
        }
        !self.ui.quit
    }

    /// Draws the frame.
    pub fn render(&mut self) -> Result<FrameStats, RenderError> {
        if self.game.is_none() {
            return Ok(FrameStats::default());
        }
        // The world's texture animations, once per rendered frame.
        self.gpu.animate(&mut self.renderer, self.frame_dt)?;
        let Some(g) = self.game.as_ref() else { return Ok(FrameStats::default()) };
        // The crew's bones follow their objects.
        self.gpu.characters.update(&mut self.renderer, &g.world)?;
        let c = g.render.camera;
        let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
        let camera =
            Camera { eye: v(c.eye), target: v(c.target), up: v(c.up), fov_y: c.fov_y, near: c.near, far: c.far };
        if let Some(m) = self.shot_marks.as_mut() {
            m.draw(&mut self.renderer, &g.world)?; // Vehicles hook: the projectiles' streaks.
        }
        self.draw_ui()?; // Sly 3 hook: the 2D layer.
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
