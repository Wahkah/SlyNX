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

pub mod attached; // Sly 2 hook: gadgets, effects and held items on the characters.
pub mod cues; // Sly 2 hook: voice lines drive their speaker's state machines and eyes.
pub mod front; // Sly 2 hook: the title, the save slots, the episode menu and the movies.
pub mod gang;
pub mod layer; // Sly 2 hook: the 2D layer (fonts, HUD, menus, job screens).
pub mod level;
pub mod menus; // Sly 2 hook: gadget assignment, shop and options screens.
pub mod movie; // Sly 2 hook: stills of the screens' sprites (the grids' icons).
pub mod photo; // Sly 2 hook: the binocucom's photo targets (class 46).
pub mod progress; // Sly 2 hook: progress, saving and world changes through the job system.
pub mod safehouse;
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

/// A sequence's object and script commands (missions.md 4.1): 670 adds an object to the world, 671 removes one,
/// 634 raises script event 4 on the sequence with its value.
fn apply_world_cue(
    world: &mut engine::world::World,
    seq: engine::object::Handle,
    c: talk::SeqCommand,
) -> Option<engine::object::Handle> {
    match c {
        talk::SeqCommand::AddObject(id) | talk::SeqCommand::RemoveObject(id) => {
            if let Some(h) = world.find_sly2_id(id, Some(seq)) {
                world.set_in_world(h, matches!(c, talk::SeqCommand::AddObject(_)));
            }
            None
        }
        talk::SeqCommand::Value(v) => {
            world.raise(seq, SEQUENCE_VALUE, alloc::vec![script::Value::Int(v)]);
            None
        }
        // Command 639: show a screen. The job prompt (class 127) takes control from the player until a face button
        // dismisses it, and is waited for (user-interface.md 2.1); other screens are not drawn by sequences yet.
        talk::SeqCommand::ShowScreen { screen, .. } => {
            let s =
                world.find_sly2_id(screen, Some(seq)).filter(|&s| world.class(s) == Some(talk::JOB_PROMPT_CLASS))?;
            if world.shared.job_prompt.is_none() {
                world.shared.job_prompt = Some((s, 0.0));
            }
            Some(s)
        }
        // Command 711: a character runs a command list; a placement stands for its copy (missions.md 4.3).
        talk::SeqCommand::RunList { object, list } => {
            let who = world.find_sly2_id(object, Some(seq));
            let placement = |h| world.class(h).is_some_and(|c| engine::world::sly2::is_a(c, 6));
            let who = who.map(|h| if placement(h) { world.children(h).next().unwrap_or(h) } else { h });
            if let (Some(h), Some(l)) = (who, world.find_sly2_id(list, Some(seq))) {
                engine::behavior::sly2_commands::run_commands(world, h, l);
            }
            None
        }
        // Command 630: a signal to another sequence (dialogue-and-voice.md 2.1).
        talk::SeqCommand::SendSignal { value, sequence } => {
            if let Some(s) = world.find_sly2_id(sequence, Some(seq)).and_then(|h| world.slot(h)) {
                world.shared.send_signal(s, value);
            }
            None
        }
        // Command 631: stop the named sequence (all its runs; their waited playbacks are retracted).
        talk::SeqCommand::StopSequence(id) => {
            if let Some(h) = world.find_sly2_id(id, Some(seq)) {
                world.shared.sequence_requests.push(engine::behavior::sly2_commands::SequenceRequest::Stop(h));
            }
            None
        }
        // Command 635: the clip on the object given, else on the clip's own rig (its parent), at the clip's speed
        // (option 109); its events (the add and remove of camera objects) fire as it is applied and retracted.
        talk::SeqCommand::PlayClip { clip, object, flags, .. } => {
            let c = world.find_sly2_id(clip, Some(seq))?;
            // −1: the object its first channel animates (camera.md 7.2.1: clip 150489 on rig 150483).
            let target = if object >= 0 {
                world.find_sly2_id(object, Some(seq))
            } else {
                let channel = world.sly2.as_deref().zip(world.sly2_record(c)).and_then(|(l, r)| {
                    formats_sly2::anim::Clip::of_record(&world.level.bytes, r, &l.table)
                        .ok()?
                        .channels
                        .first()
                        .map(|ch| ch.target)
                });
                channel.and_then(|id| world.find_sly2_id(id, Some(c))).or(world.parent(c))
            };
            let target = target?;
            let speed = world.options(c).map_or(1.0, |o| o.f32_or(CLIP_SPEED, 1.0));
            engine::game::sly2_anim::with(world, |a, w| a.apply_clip(w, c, target, 0.0, speed, flags)).flatten()
        }
        _ => None,
    }
}

/// Whether clip playback `p` still plays (it is removed when retracted).
fn clip_playing(world: &engine::world::World, p: engine::object::Handle) -> bool {
    world.sly2.as_deref().is_some_and(|l| l.anim.get_option(world, p, engine::game::sly2_anim::opt::CLIP).is_some())
}

/// Whether what a run waits for is still busy: a clip playback until retracted, the job prompt screen until the
/// player dismisses it (user-interface.md 2.1).
fn still_waits(world: &engine::world::World, p: engine::object::Handle) -> bool {
    if world.class(p) == Some(talk::JOB_PROMPT_CLASS) {
        return world.shared.job_prompt.is_some_and(|(s, _)| s == p);
    }
    clip_playing(world, p)
}

/// Retracts the clip playback a stopped run waited for (a screen is left as it is).
fn retract_wait(world: &mut engine::world::World, p: engine::object::Handle) {
    if world.class(p) != Some(talk::JOB_PROMPT_CLASS) {
        engine::game::sly2_anim::with(world, |a, w| a.remove(w, p));
    }
}

/// The job prompt's wait before a press counts, in real seconds (user-interface.md 2.1: idle for more than 0.5 s).
pub const JOB_PROMPT_IDLE: f32 = 0.5;

/// A sequence run without a sound engine: its next command, where it stops, its thread and the clip it waits for
/// (635 with no-wait 0).
#[derive(Clone, Copy, Debug)]
pub struct SilentRun {
    /// The sequence.
    pub sequence: engine::object::Handle,
    /// Its next command.
    pub at: usize,
    /// Where it stops (`usize::MAX`: the end of the list).
    pub end: usize,
    /// The thread (625's ID), or −1 for the main line.
    pub thread: i32,
    /// Where it started (with the sequence, it tells runs of the same thread ID apart).
    pub start: usize,
    /// The clip playback it waits for.
    pub waiting: Option<engine::object::Handle>,
}

impl SilentRun {
    /// The main line of `sequence`.
    pub fn main(sequence: engine::object::Handle) -> SilentRun {
        SilentRun { sequence, at: 0, end: usize::MAX, thread: talk::MAIN, start: 0, waiting: None }
    }
}

/// Runs a silent sequence's commands until one waits (a clip played with no-wait 0) or they run out; returns whether
/// it ended. Lines are not played; sub-sequences start without being waited for. Threads it opens (625) go to
/// `spawn`, threads it stops (627) to `stop`.
fn step_silent(
    world: &mut engine::world::World,
    run: &mut SilentRun,
    spawn: &mut Vec<SilentRun>,
    stop: &mut Vec<(engine::object::Handle, i32)>,
) -> bool {
    if let Some(p) = run.waiting {
        if still_waits(world, p) {
            return false;
        }
        run.waiting = None;
    }
    let cmds = talk::sequence_commands(world, run.sequence);
    while run.at < run.end
        && let Some(&c) = cmds.get(run.at)
    {
        run.at += 1;
        match c {
            talk::SeqCommand::Thread { id, end } => {
                spawn.push(SilentRun {
                    sequence: run.sequence,
                    at: run.at,
                    end,
                    thread: id,
                    start: run.at,
                    waiting: None,
                });
                run.at = end.saturating_add(1);
            }
            talk::SeqCommand::StopThread(id) => stop.push((run.sequence, id)),
            talk::SeqCommand::Task { task, state } => world.shared.task_requests.push((task, state)),
            talk::SeqCommand::Start { sequence, .. } => world
                .shared
                .sequence_requests
                .push(engine::behavior::sly2_commands::SequenceRequest::Start { sequence, starter: None }),
            // Command 629: the run waits until its sequence holds the signal (sent before or after).
            talk::SeqCommand::WaitSignal { value, consume } => {
                let me = world.slot(run.sequence).unwrap_or(u32::MAX);
                if !world.shared.take_signal(me, value, consume) {
                    run.at -= 1;
                    return false;
                }
            }
            talk::SeqCommand::PlayClip { wait, .. } | talk::SeqCommand::ShowScreen { wait, .. } => {
                let p = apply_world_cue(world, run.sequence, c);
                if wait && p.is_some() {
                    run.waiting = p;
                    return false;
                }
            }
            _ => {
                apply_world_cue(world, run.sequence, c);
            }
        }
    }
    true
}

/// A clip's playback speed (option 109; camera.md 7.2.1).
pub const CLIP_SPEED: u16 = 109;

/// Script event a sequence's value command raises (option 634).
pub const SEQUENCE_VALUE: u32 = 4;

/// Removes the characters' objects their lookup tables mark with flag `0x200` when bound at load (characters.md
/// 9.3.1): the second cane 318, the throw marker 586, object 685 and the target-ring placements.
pub fn remove_flagged_parts(world: &mut engine::world::World, exe: &formats_sly2::exe::Executable) {
    use formats_sly2::anim::{self as fa};
    let tables: [(i16, &[(u32, usize)]); 3] = [
        (fa::SLY_CLASS, &[fa::SLY_TABLES[0], fa::SLY_TABLES[1], fa::SHARED_TABLE]),
        (fa::BENTLEY_CLASS, &[fa::BENTLEY_TABLE, fa::SHARED_TABLE]),
        (fa::MURRAY_CLASS, &[fa::MURRAY_TABLE]),
    ];
    for (class, list) in tables {
        let characters: Vec<engine::object::Handle> =
            world.handles().filter(|&h| world.class(h) == Some(class)).collect();
        for t in list {
            let Ok(records) = fa::read_lookup_table(exe, *t) else { continue };
            for r in records.iter().filter(|r| r.flags & REMOVE_AT_BIND != 0 && r.is_object()) {
                for &c in &characters {
                    let Some(h) = world.find_sly2_id(r.key as i32, Some(c)) else { continue };
                    // Only the character's own (below it).
                    let mut p = world.parent(h);
                    while let Some(x) = p.filter(|&x| x != c) {
                        p = world.parent(x);
                    }
                    if p == Some(c) {
                        world.set_in_world(h, false);
                    }
                }
            }
        }
    }
}

/// A lookup record's flag that removes its object from the world when the table is bound at load.
pub const REMOVE_AT_BIND: u32 = 0x200;

/// The camera's current state (position, Euler angles from its forward axis, field of view).
fn current_cam(v: &engine::game::render_scene::CameraView) -> engine::camera::views::CamState {
    let f = engine::math::normalize(engine::math::sub(v.target, v.eye));
    let yaw = engine::math::atan2(f[1], f[0]);
    let pitch = engine::math::atan2(-f[2], engine::math::sqrt(f[0] * f[0] + f[1] * f[1]));
    engine::camera::views::CamState { position: v.eye, angles: [0.0, pitch, yaw], fov: v.fov_y }
}

/// A camera block's state with its "current value" parts (every component below 1e-4, or a field of view of 0)
/// filled from `now` (camera.md 2.1.1).
fn resolve_cam(a: talk::CamArgs, now: engine::camera::views::CamState) -> engine::camera::views::CamState {
    let tiny = |v: [f32; 3]| v.iter().all(|x| x.abs() < 1e-4);
    engine::camera::views::CamState {
        position: if tiny(a.position) { now.position } else { a.position },
        angles: if tiny(a.angles) { now.angles } else { a.angles },
        fov: if a.fov == 0.0 { now.fov } else { a.fov },
    }
}

/// Script event raised on a sequence when it starts (dialogue-and-voice.md 2; scripts.md 6: event 5 on class 114).
pub const SEQUENCE_STARTED: u32 = 5;
/// Script event raised on a sequence when it ends.
pub const SEQUENCE_ENDED: u32 = 6;

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
    /// The hub's safe house, if the world has one.
    pub safe_house: Option<safehouse::SafeHouse>,
    /// The safe house is the current player.
    pub in_safe_house: bool,
    /// The binocucom's photo mode while the binocucom is on (missions.md 4.3).
    pub photo: Option<photo::Photo>,
    /// The binocucom was on last frame (its task events).
    binocucom_was_on: bool,
    /// The vault changes since the last frame (sequences, lines, the dial's screen).
    pub vault_events: Vec<engine::behavior::sly2_vault::VaultEvent>,
    /// A character to switch to next frame (taken out of the safe house).
    switch_to: Option<engine::gang::Who>,
    /// The sequences started and not yet ended (for their events 5 and 6).
    pub running: Vec<engine::object::Handle>,
    /// Without a sound engine: the sequences' runs.
    silent: Vec<SilentRun>,
    /// With a sound engine: (sequence, clip playback) a sequence waits for.
    clip_waits: Vec<(engine::object::Handle, i32, engine::object::Handle)>,
    /// Runs held on a signal (629), with a sound engine: (sequence, thread, value, consume).
    signal_waits: Vec<(engine::object::Handle, i32, i32, bool)>,
    items: Option<ItemTable>,
    pockets: Option<engine::gang::PocketTable>,
    disc: Option<Disc<R>>,
    option_map: Option<OptionMap>,
    builtins: Option<BuiltinTable>,
    gpu: StaticLevel,
    /// Seconds of play since the last rendered frame (for the texture animations).
    unrendered: f32,
    current: Option<Sly2Current>,
    seed: u64,
    pending: progress::Pending,
    screens: ui::sly2::JobScreens,
    ui_paused: bool,
    saver: Option<screens::Saver>,
    font: Option<ui::sly2::font::FontSheet>,
    ui_layer: Option<crate::ui_layer::UiLayer>,
    /// Vehicles hook: the projectiles' streaks (`crate::shot_marks`).
    shot_marks: Option<crate::shot_marks::ShotMarks>,
    hud: ui::sly2::hud::Hud,
    pause: ui::sly2::pause::PauseMenu,
    quit: bool,
    menus: menus::Menus,
    front: front::Front,
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// A session on `disc` (or showing why there is none); `seed` seeds the game's random numbers.
    pub fn new(backend: B, disc: Result<Disc<R>, String>, seed: u64) -> Sly2Session<B, R> {
        let (disc, message) = match disc {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e)),
        };
        let exe = disc.as_ref().map(|d| &d.executable);
        // Sly 2 hook (menus): the gadget grids and the loot table, from the executable.
        let menus = menus::Menus {
            grids: [engine::gang::Who::Sly, engine::gang::Who::Bentley, engine::gang::Who::Murray]
                .map(|w| exe.and_then(|e| engine::gang::gadgets::grid_from_executable(e, w).ok()).unwrap_or_default()),
            loot: exe.and_then(|e| engine::gang::LootTable::from_executable(e).ok()),
            ..menus::Menus::default()
        };
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
            unrendered: 0.0,
            safe_house: None,
            in_safe_house: false,
            photo: None,
            binocucom_was_on: false,
            vault_events: Vec::new(),
            switch_to: None,
            running: Vec::new(),
            silent: Vec::new(),
            clip_waits: Vec::new(),
            signal_waits: Vec::new(),
            current: None,
            seed,
            pending: progress::Pending::default(),
            screens: ui::sly2::JobScreens::default(),
            ui_paused: false,
            saver: None,
            font: None,
            ui_layer: None,
            shot_marks: None,
            hud: ui::sly2::hud::Hud::default(),
            pause: ui::sly2::pause::PauseMenu::default(),
            quit: false,
            menus,
            front: front::Front::default(),
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

    /// Vehicles hook: makes the task named `name` current, forced (testing and the smoke run start a job's machine
    /// this way). Returns whether the task exists.
    pub fn start_task(&mut self, name: &str) -> bool {
        let Some(g) = self.game.as_mut() else { return false };
        let Some(j) = g.world.sly2.as_deref_mut().and_then(|l| l.jobs.as_deref_mut()) else { return false };
        let Some(t) = j.graph.tasks.iter().position(|t| t.name == name) else { return false };
        j.request_current(t, true);
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
            let mut glyphs = if self.font.is_none() { layer::font_glyphs(&fw) } else { None };
            // Sly 2 hook (menus): the gadget grids' icons go into the sheet as a third font.
            if let Some(g) = glyphs.as_mut() {
                let mut icons = Vec::new();
                for (id, screen) in layer::screens(&fw, disc, &menus::GRID_SCREENS) {
                    let Some(k) = menus::GRID_SCREENS.iter().position(|&s| s == id) else { continue };
                    let (shapes, art) = layer::grid_art(&screen, k, &self.menus.grids[k], 3151);
                    icons.extend(shapes);
                    self.menus.art[k] = Some(art);
                }
                g.truncate(2);
                g.push(icons);
            }
            // The default passes plus the shadow hulls (7) and projected volumes (11, 13), rendering.md 5.
            let defaults = formats_sly2::scene::SceneOptions::default();
            let opts = formats_sly2::scene::SceneOptions {
                row: Some(row as i16),
                passes: defaults.passes | (1 << 7) | (1 << 11) | (1 << 13),
                // The characters' textures too: their meshes are drawn skinned (`skin`), not from the scene.
                texture_classes: (skin::CHARACTERS[0], skin::CHARACTERS[2]),
                ..defaults
            };
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
        // The parts the characters' lookup tables take out of the world when bound (characters.md 9.3.1).
        if let Some(d) = self.disc.as_ref() {
            remove_flagged_parts(&mut world, &d.executable);
        }
        let mut game = Game::new(world, Progress::new_game(), systems, cfg);
        // The characters' skinned meshes are not in the scene build: they are added here and posed every frame.
        let skinned = skin::character_draws(&game.world, &build.material_base);
        let skip: &[(u32, u32)] = if skinned.is_empty() { &[] } else { &ranges };
        // The objects attached under the characters follow them and show while in use (`attached`).
        let dynamic: Vec<(u32, u32)> = attached::attached_objects(&game.world).into_iter().map(|o| o.1).collect();
        let input = stage::prep::SceneInput {
            skip,
            dynamic: &dynamic,
            ..stage::prep::SceneInput::sly2(&build, Some(&lighting))
        };
        let sheet = self.font.as_ref().map(|f| (f.width, f.height, f.pixels.clone()));
        let mut ui_layer = None;
        let mut marks = None;
        let mut video = None;
        let uploaded = level::upload_stage(&mut self.renderer, input, Some(&game.world), &skinned, |r| {
            ui_layer = Some(crate::ui_layer::UiLayer::new(r, sheet)?); // Sly 2 hook: the 2D layer.
            video = Some(render::video::VideoLayer::new(r)?); // Sly 2 hook: the movies' picture.
            marks = Some(crate::shot_marks::ShotMarks::upload(r)?); // Vehicles hook: the projectiles' streaks.
            Ok(())
        });
        self.shot_marks = marks;
        self.gpu = match uploaded {
            Ok(gpu) => gpu,
            Err(e) => {
                // The renderer holds an empty level now: nothing may keep handles into the old one.
                self.unload();
                return Err(format!("row {row}: {e}"));
            }
        };
        self.ui_layer = ui_layer;
        self.front.movies.set_layer(video);
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
        self.safe_house = self.game.as_ref().and_then(|g| safehouse::SafeHouse::from_world(&g.world));
        self.in_safe_house = false;
        self.switch_to = None;
        self.running.clear();
        self.silent.clear();
        self.clip_waits.clear();
        self.front_world_loaded(row); // Sly 2 hook: the menu world shows the episode menu.
        Ok(())
    }

    /// Drops the current world (after a failed upload, for example): an empty, committed level, and nothing left
    /// holding handles into the old one, so the next frames render (nothing) instead of failing.
    pub fn unload(&mut self) {
        level::empty_level(&mut self.renderer);
        self.gpu = level::StaticLevel::default();
        self.ui_layer = None;
        self.shot_marks = None;
        self.front.movies.set_layer(None);
        self.game = None;
        self.current = None;
    }

    /// The sequence runs without a sound engine (for tests and the smoke tool).
    pub fn silent_runs(&self) -> &[SilentRun] {
        &self.silent
    }

    /// Whether a sequence run waits for a clip that ends by itself (played once, not looping or holding): a cutscene
    /// that will go on (for the smoke tool's stall rule).
    pub fn waits_on_ending_clip(&self) -> bool {
        let Some(anim) = self.game.as_ref().and_then(|g| g.world.sly2.as_deref()).map(|l| &l.anim) else {
            return false;
        };
        let ends = |p: engine::object::Handle| {
            anim.clips.get(&p.index()).is_some_and(|c| c.wrap == engine::game::sly2_anim::Wrap::Once)
        };
        self.silent.iter().filter_map(|r| r.waiting).any(ends) || self.clip_waits.iter().any(|w| ends(w.2))
    }

    /// Enters the hub's safe house (false when the world has none; safehouse-and-shop.md 2): it becomes the current
    /// player, shows the current character's state, and raises script event 5 and task event 26.
    pub fn enter_safe_house(&mut self) -> bool {
        let (Some(h), Some(g)) = (self.safe_house.as_mut(), self.game.as_mut()) else { return false };
        let current = g
            .world
            .player
            .and_then(|p| g.world.class(p))
            .and_then(|c| gang::CHARACTER_CLASSES.iter().find(|k| k.1 == c).map(|k| safehouse::state_of(k.0)));
        h.show(current.unwrap_or(-1));
        let house = h.handle;
        self.in_safe_house = true;
        g.world.shared.safe_house = true;
        g.world.raise(house, safehouse::EVENT_ENTER, Vec::new());
        engine::game::sly2_jobs::with(&mut g.world, |j, _| j.task_event(safehouse::TASK_ENTER, Some(house)));
        true
    }

    /// Leaves the safe house: script event 6 and task event 27.
    pub fn leave_safe_house(&mut self) {
        let (Some(h), Some(g)) = (self.safe_house.as_ref(), self.game.as_mut()) else { return };
        if !self.in_safe_house {
            return;
        }
        let house = h.handle;
        self.in_safe_house = false;
        g.world.shared.safe_house = false;
        g.world.set_option(house, safehouse::opt::ENABLE, engine::object::OptionValue::Int(0));
        g.world.raise(house, safehouse::EVENT_LEAVE, Vec::new());
        engine::game::sly2_jobs::with(&mut g.world, |j, _| j.task_event(safehouse::TASK_LEAVE, Some(house)));
    }

    /// The safe house's input (left and right step, Cross acts) and what the game gets: the d-pad and Cross are the
    /// safe house's while in it; a character taken out is switched to the next frame (d-pad: left Sly, up Murray,
    /// right Bentley).
    fn safe_house_input(&mut self, input: &FrameInput) -> FrameInput {
        use platform_api::Buttons;
        let mut out = *input;
        if let Some(who) = self.switch_to.take() {
            let b = match who {
                engine::gang::Who::Sly => Buttons::LEFT.0,
                engine::gang::Who::Murray => Buttons::UP.0,
                engine::gang::Who::Bentley => Buttons::RIGHT.0,
            };
            out.pressed |= b;
            out.buttons |= b;
        }
        if !self.in_safe_house || self.menus.open() {
            return out;
        }
        let pressed = input.pressed;
        let (Some(h), Some(g)) = (self.safe_house.as_mut(), self.game.as_mut()) else { return out };
        if pressed & Buttons::LEFT.0 != 0 {
            h.step(-1);
        }
        if pressed & Buttons::RIGHT.0 != 0 {
            h.step(1);
        }
        if pressed & Buttons::B.0 != 0 {
            let character = h.state().and_then(safehouse::who_of_state).and_then(|who| {
                let class = gang::CHARACTER_CLASSES.iter().find(|k| k.0 == who)?.1;
                g.world.handles().find(|&x| g.world.class(x) == Some(class))
            });
            match h.press(&mut g.world, character) {
                Some(safehouse::Pressed::OpenShop) => self.open_shop(),
                Some(safehouse::Pressed::TakeOut(who)) => {
                    self.leave_safe_house();
                    self.switch_to = Some(who);
                }
                _ => {}
            }
        }
        let mask = Buttons::LEFT.0 | Buttons::RIGHT.0 | Buttons::UP.0 | Buttons::DOWN.0 | Buttons::B.0;
        out.pressed &= !mask;
        out.buttons &= !mask;
        out
    }

    /// The job prompt screen (class 127, user-interface.md 2.1): while it is up the player has no control. After it
    /// has been up for more than [`JOB_PROMPT_IDLE`] seconds, any face button gives control back, restarts the
    /// current job, if any, and hides the screen.
    fn job_prompt_input(&mut self, input: &FrameInput, measured: f32) -> FrameInput {
        use platform_api::Buttons;
        let Some(g) = self.game.as_mut() else { return *input };
        let Some((screen, up)) = g.world.shared.job_prompt else { return *input };
        let face = Buttons::A.0 | Buttons::B.0 | Buttons::X.0 | Buttons::Y.0;
        if up > JOB_PROMPT_IDLE && input.pressed & face != 0 {
            g.world.shared.job_prompt = None;
            engine::game::sly2_jobs::with(&mut g.world, |j, _| {
                if j.job.is_some() {
                    j.restart_at_checkpoint();
                }
            });
        } else {
            g.world.shared.job_prompt = Some((screen, up + measured));
        }
        FrameInput::default()
    }

    /// Runs one frame for a measured interval (seconds). Returns false when the session should end.
    pub fn update(&mut self, input: &FrameInput, measured: f32) -> bool {
        // Sly 2 hook (front end): a movie holds everything; the title and the episode menu take the input.
        let front = self.front_open();
        let Some(input) = self.update_front(input, measured).or_else(|| (!self.movie_playing()).then_some(*input))
        else {
            return !self.quit;
        };
        let input = &self.safe_house_input(&input);
        let input = &self.job_prompt_input(input, measured);
        if let Some(g) = self.game.as_mut() {
            g.frame(input, measured);
            // The binocucom's photo mode (missions.md 4.3): open while the binocucom is on; a good picture asks the
            // target's task for state 2, and the binocucom closes once no listed target is pending.
            let on = engine::gang::binocucom_on(&g.world);
            if on != self.binocucom_was_on {
                // Task events 7 then 9 on opening, 8 then 11 on closing, no object (missions.md 4.3). Radio-talk
                // mode (12, 13) is not entered here.
                let es = if on {
                    [photo::BINOCUCOM_OPENED, photo::PROMPT_STARTED]
                } else {
                    [photo::BINOCUCOM_CLOSED, photo::PROMPT_ENDED]
                };
                engine::game::sly2_jobs::with(&mut g.world, |j, _| es.iter().for_each(|&e| j.task_event(e, None)));
                self.binocucom_was_on = on;
            }
            if !on {
                self.photo = None;
            } else if self.photo.is_none() {
                self.photo = photo::Photo::open(&g.world, photo::params(g.render.camera.fov_y));
            }
            let targets = if self.photo.is_some() { photo::targets(&g.world) } else { Vec::new() };
            if let Some(p) = self.photo.as_mut()
                && let Some(shot) = p.update(input, measured, &g.world, &targets)
            {
                if let Some(task) = shot.task {
                    g.world.shared.task_requests.push((task, 2));
                }
                engine::game::sly2_jobs::with(&mut g.world, |j, _| j.task_event(photo::PICTURE_ACCEPTED, None));
                if !p.listed_pending(&targets) {
                    self.photo = None;
                    g.world.shared.camera_binocucom = None;
                }
            }
            self.vault_events.clear();
            self.vault_events.append(&mut g.world.shared.vault_events);
            self.unrendered += engine::game::clock::frame_step(measured);
        }
        // A script enabling the safe house as the current player (option 884 = 1, safehouse-and-shop.md 2.1) enters it.
        let enable = self
            .safe_house
            .as_ref()
            .zip(self.game.as_ref())
            .is_some_and(|(h, g)| g.world.options(h.handle).and_then(|o| o.i32(safehouse::opt::ENABLE)) == Some(1));
        if enable && !self.in_safe_house {
            self.enter_safe_house();
        }
        // Sly 2 hook: a load the job system asked for (an exit, a restart at a checkpoint).
        if let Err(e) = self.follow_load_request(measured) {
            self.message = Some(e);
        }
        // Sequences that ended raise script event 6, which their modules answer (often by asking the current task for
        // its next state; vehicles-and-minigames.md 10.3). Without a sound engine the commands run here in order
        // ([`SilentRun`]): lines are skipped, a clip played with no-wait 0 is waited for.
        if let Some(g) = self.game.as_mut() {
            let mut ended: Vec<engine::object::Handle> = Vec::new();
            // Runs stopped (627, a stopped sequence, a line that ended) retract the playback they wait for; a looping,
            // back-and-forth or holding playback is retracted only this way (animation.md 2).
            let mut retract: Vec<engine::object::Handle> = Vec::new();
            if let Some(s) = self.sound.as_mut() {
                for (h, t) in s.take_stopped() {
                    retract.extend(self.clip_waits.iter().filter(|w| (w.0, w.1) == (h, t)).map(|w| w.2));
                    self.clip_waits.retain(|w| (w.0, w.1) != (h, t));
                }
                // Signals the runs wait for (629): released once their sequence holds them.
                self.signal_waits.retain(|&(seq, t, value, consume)| {
                    let me = g.world.slot(seq).unwrap_or(u32::MAX);
                    let got = g.world.shared.take_signal(me, value, consume);
                    if got {
                        s.release(seq, t);
                    }
                    !got
                });
                // Clips the runs wait for: released once retracted.
                self.clip_waits.retain(|&(seq, t, p)| {
                    let alive = still_waits(&g.world, p);
                    if !alive {
                        s.release(seq, t);
                    }
                    alive
                });
                ended.extend(self.running.iter().copied().filter(|h| !s.sequences().iter().any(|q| q.object == *h)));
            } else {
                let (mut spawn, mut stop) = (Vec::new(), Vec::new());
                let mut finished: Vec<(engine::object::Handle, usize)> = Vec::new();
                for run in &mut self.silent {
                    if step_silent(&mut g.world, run, &mut spawn, &mut stop) {
                        finished.push((run.sequence, run.start));
                    }
                }
                // A sequence ends when none of its runs is left (its threads may outlast its main line).
                self.silent.retain(|r| {
                    let gone = finished.contains(&(r.sequence, r.start)) || stop.contains(&(r.sequence, r.thread));
                    if gone && let Some(p) = r.waiting {
                        retract.push(p);
                    }
                    !gone
                });
                // Threads stopped in the frame they were opened never start.
                spawn.retain(|r| !stop.contains(&(r.sequence, r.thread)));
                self.silent.extend(spawn);
                ended.extend(self.running.iter().copied().filter(|h| !self.silent.iter().any(|r| r.sequence == *h)));
                ended.sort_unstable();
                ended.dedup();
            }
            for p in retract {
                retract_wait(&mut g.world, p);
            }
            self.running.retain(|h| !ended.contains(h));
            for h in ended {
                // A stopped sequence's signal set is cleared (dialogue-and-voice.md 2.1).
                if let Some(s) = g.world.slot(h) {
                    g.world.shared.sequence_signals.remove(&s);
                }
                self.signal_waits.retain(|w| w.0 != h);
                g.world.raise(h, SEQUENCE_ENDED, Vec::new());
            }
        }
        // Sequence starts and stops from scripts, volumes, spots and command lists (dialogue-and-voice.md 2.1); a
        // start raises script event 5 on the sequence.
        if let Some(g) = self.game.as_mut() {
            let requests = core::mem::take(&mut g.world.shared.sequence_requests);
            for r in requests {
                use engine::behavior::sly2_commands::SequenceRequest as Q;
                match r {
                    // Starting a sequence that runs does nothing (dialogue-and-voice.md 2.1, animation.md 2).
                    Q::Start { sequence, .. } | Q::StartWithPlayer(sequence) if self.running.contains(&sequence) => {}
                    Q::Start { sequence, .. } | Q::StartWithPlayer(sequence) => {
                        if let Some(s) = self.sound.as_mut() {
                            s.start_sequence(&g.world, sequence);
                        }
                        if !self.running.contains(&sequence) {
                            self.running.push(sequence);
                        }
                        if self.sound.is_none() {
                            self.silent.retain(|r| r.sequence != sequence);
                            self.silent.push(SilentRun::main(sequence));
                        }
                        g.world.raise(sequence, SEQUENCE_STARTED, Vec::new());
                        self.sequences_started += 1;
                    }
                    Q::Stop(sequence) => {
                        if let Some(s) = self.sound.as_mut() {
                            s.stop_sequence(sequence);
                            for w in self.clip_waits.iter().filter(|w| w.0 == sequence) {
                                s.release(sequence, w.1);
                            }
                        }
                        self.running.retain(|&h| h != sequence);
                        // A stopped run retracts the playback it waits for (animation.md 2).
                        let mut waited: Vec<engine::object::Handle> =
                            self.silent.iter().filter(|r| r.sequence == sequence).filter_map(|r| r.waiting).collect();
                        waited.extend(self.clip_waits.iter().filter(|w| w.0 == sequence).map(|w| w.2));
                        self.silent.retain(|r| r.sequence != sequence);
                        self.clip_waits.retain(|w| w.0 != sequence);
                        for p in waited {
                            retract_wait(&mut g.world, p);
                        }
                    }
                    // A signal from a script (ID 640) or a guard command list (command 15).
                    Q::Alarm(sequence, value) => {
                        if let Some(s) = g.world.slot(sequence) {
                            g.world.shared.send_signal(s, value);
                        }
                    }
                }
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
            // Task states the running sequences asked for (option 642), and their camera blocks (camera.md 2.1.1).
            if let Some(g) = self.game.as_mut() {
                g.world.shared.task_requests.extend(s.take_task_requests());
                for (h, t, c) in s.take_world_cues() {
                    let p = apply_world_cue(&mut g.world, h, c);
                    match (c, p) {
                        (talk::SeqCommand::WaitSignal { value, consume }, _) => {
                            self.signal_waits.push((h, t, value, consume))
                        }
                        (
                            talk::SeqCommand::PlayClip { wait: true, .. }
                            | talk::SeqCommand::ShowScreen { wait: true, .. },
                            Some(p),
                        ) => self.clip_waits.push((h, t, p)),
                        (
                            talk::SeqCommand::PlayClip { wait: true, .. }
                            | talk::SeqCommand::ShowScreen { wait: true, .. },
                            None,
                        ) => s.release(h, t),
                        _ => {}
                    }
                }
                for c in s.take_camera_cues() {
                    g.world.shared.camera_move = match c {
                        talk::CameraCue::Move { from, to, duration, curve } => {
                            let now = current_cam(&g.render.camera);
                            Some(engine::camera::views::TimedMove {
                                from: resolve_cam(from, now),
                                to: resolve_cam(to, now),
                                duration,
                                curve: curve.clamp(0, 3) as usize,
                                time: 0.0,
                            })
                        }
                        talk::CameraCue::End { .. } => None,
                    };
                }
            }
            // Sub-sequences the running ones started (option 636).
            if let Some(g) = self.game.as_mut() {
                for h in s.take_sub_starts() {
                    s.start_sequence(&g.world, h);
                    if !self.running.contains(&h) {
                        self.running.push(h);
                    }
                    g.world.raise(h, SEQUENCE_STARTED, Vec::new());
                    self.sequences_started += 1;
                }
            }
        }
        self.apply_voice_cues(); // Sly 2 hook: lip sync and eyes.
        if !front && !self.front_open() {
            self.update_screens(input, engine::game::clock::frame_step(measured)); // Sly 2 hook: the job screens.
        }
        !self.quit
    }

    /// Draws the frame.
    pub fn render(&mut self) -> Result<FrameStats, RenderError> {
        let Some(g) = self.game.as_ref() else { return Ok(FrameStats::default()) };
        self.gpu.characters.update(&mut self.renderer, &g.world)?;
        // Gadgets, effects and held items on the characters, where the engine has them (`attached`).
        self.gpu.attached.update(&mut self.renderer, &g.world, self.unrendered)?;
        // Draws outside the camera's visibility volumes' zones are hidden (camera.md 0).
        self.gpu.show_zones(&mut self.renderer, g.world.shared.visibility_mask)?;
        // Texture animations (rendering.md 4) by the time played since the last frame.
        self.gpu.animate(&mut self.renderer, core::mem::take(&mut self.unrendered))?;
        let c = g.render.camera;
        let v = |a: [f32; 3]| Vec3::new(a[0], a[1], a[2]);
        let camera =
            Camera { eye: v(c.eye), target: v(c.target), up: v(c.up), fov_y: c.fov_y, near: c.near, far: c.far };
        if let Some(m) = self.shot_marks.as_mut() {
            m.draw(&mut self.renderer, &g.world)?; // Vehicles hook: the projectiles' streaks.
        }
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
