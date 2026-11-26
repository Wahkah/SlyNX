//! The game loop: one loaded world, its scripts, the clocks, the systems, and the per-frame order of
//! clock-and-loop.md 4 / gameplay-overview.md 2.
//!
//! [`Game::frame`] runs one displayed frame from a measured interval; [`Game::step_fixed`] runs the deterministic
//! fixed step (1/60 s) the test harness uses. Within a frame:
//!
//! 1. the game state's timers (the sprint countdown);
//! 2. the clocks advance (clamped step, time scale, pause);
//! 3. the real-clock pass: objects on the real clock update;
//! 4. if game time advanced: the player decides; kinematic movers move; every ticking object updates in tree order
//!    (parent before children, siblings in load order), raising event 1 where a module asks for it; animations
//!    advance and fire events; physics steps; scheduled script calls run;
//! 5. world transforms are brought up to date; the camera updates (also while paused); the audio frame is built;
//! 6. finished animation playbacks are retired and reported;
//! 7. the render scene lists what changed.
//!
//! Script events raised by an object are dispatched right after that object's step (and synchronously while a
//! script runs), so every event is handled before the next object updates.

pub mod anim_bridge;
pub mod audio_frame;
pub mod camera;
pub mod clock;
pub mod difficulty;
pub mod physics_bridge;
pub mod player_bridge;
pub mod progress;
pub mod render_scene;
pub mod rng;
pub mod script_host;
pub mod sly2_anim; // Sly 2 hook: state-machine and clip playbacks on world objects.
pub mod sly2_hud; // Sly 2 hook: what the HUD shows (vitals, prompts).
pub mod sly2_jobs; // Sly 2 hook: the Sly 2 job and progression system.
pub mod systems;

use alloc::vec;
use alloc::vec::Vec;

use script::{Interpreter, OptionMap, Value};

use crate::behavior::{Arrival, Cx};
use crate::object::{Handle, flags};
use crate::world::{AnimCommand, Request, World, opt};

use audio_frame::{AudioFrame, Listener};
use clock::{Clock, FIXED_STEP};
use progress::{Progress, level_flag};
use render_scene::RenderScene;
use rng::Rng;
use script_host::WorldHost;
use systems::{FrameInput, StepInfo, Systems};

/// Most script events dispatched in one flush; a runaway chain of events beyond this is dropped (and counted).
pub const MAX_EVENTS_PER_FLUSH: u32 = 100_000;

/// How a game is set up.
#[derive(Default)]
pub struct GameConfig {
    /// Seed of the game's random numbers.
    pub seed: u64,
    /// The script option mapping read from the player's executable (scripts.md 8). Without it, scripts can still
    /// run but every option operation on a built-in option fails.
    pub option_map: Option<OptionMap>,
    /// Do not run the world's scripts.
    pub no_scripts: bool,
}

/// Counters of the last frame, for the overlay and performance reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounters {
    /// Objects whose behaviour or event-1 handler ran.
    pub updated: u32,
    /// Script events dispatched.
    pub events: u32,
    /// Script error values created.
    pub script_errors: u32,
    /// Events dropped by the flush limit.
    pub dropped_events: u32,
    /// Drawables whose transform or visibility changed.
    pub render_changes: u32,
}

/// One running world.
pub struct Game {
    /// The world.
    pub world: World,
    /// The player's progress.
    pub progress: Progress,
    /// The clocks.
    pub clock: Clock,
    /// The systems.
    pub systems: Systems,
    /// What the renderer needs this frame.
    pub render: RenderScene,
    /// What the sound engine needs this frame.
    pub audio: AudioFrame,
    /// The last frame's counters.
    pub counters: FrameCounters,
    /// Every kind of script error reported since the world started, with its count (diagnostics).
    pub script_error_kinds: Vec<(script::ScriptError, u32)>,
    scripts: Option<Interpreter>,
    rng: Rng,
    frame: u64,
    input: FrameInput,
    ticking: Vec<u32>,
    ambient: Vec<u32>,
    ambient_prev: Vec<[f32; 3]>,
    last_eye: [f32; 3],
    placement_was_set: bool,
    /// Override modules whose outer module the interpreter refused (a second outer, or a loop).
    pub outer_refused: u32,
}

impl Game {
    /// Starts a loaded world (gameplay-overview.md 3): builds and runs the script modules, attaches objects to
    /// them, raises event 0 for every object in load order, runs the behaviours' post-load step (auto-start state
    /// machines, pickups already collected, key exits), starts option-23 and auto-play animations, and computes the
    /// first transforms.
    pub fn new(world: World, progress: Progress, systems: Systems, config: GameConfig) -> Game {
        let mut g = Game {
            world,
            progress,
            clock: Clock::new(),
            systems,
            render: RenderScene::default(),
            audio: AudioFrame::default(),
            counters: FrameCounters::default(),
            script_error_kinds: Vec::new(),
            scripts: None,
            rng: Rng::new(config.seed),
            frame: 0,
            input: FrameInput::default(),
            ticking: Vec::new(),
            ambient: Vec::new(),
            ambient_prev: Vec::new(),
            last_eye: [0.0; 3],
            placement_was_set: false,
            outer_refused: 0,
        };
        let world_id = g.world.info.id;
        g.progress.level(world_id).flags |= level_flag::VISITED;
        // Sly 1 world option meanings (sky, fog, lens) only for Sly 1 worlds: the same IDs mean other things in
        // Sly 2 and Sly 3 worlds, whose lens and fog stay at the defaults until their options are mapped.
        if g.world.sly1_meanings() {
            g.render.configure(&g.world.world_options);
        }
        g.systems.camera.configure(&g.world);
        g.world.update_transforms();
        g.systems.physics.load(&mut g.world);

        if !config.no_scripts
            && let Ok(program) = crate::world::sly2::program(&mut g.world)
        // Sly 2 hook (Sly 1: Program::from_world)
        {
            let mut interp = Interpreter::new(program, config.option_map);
            crate::world::sly2::configure_interpreter(&g.world, &mut interp); // Sly 2 hook (no-op for Sly 1)
            {
                let mut host = Self::host(&mut g.world, &mut g.rng, 0.0, g.systems.physics.queries());
                interp.load(&mut host);
            }
            let modules = interp.program().module_count();
            let event1: Vec<bool> = (0..modules as u32)
                .map(|m| interp.lookup(m, script::event::UPDATE).is_some_and(|v| v.is_callable()))
                .collect();
            g.world.set_event1_modules(event1);
            let handles: Vec<Handle> = g.world.handles().collect();
            // Sly 2 hook: each package copy past the first runs in its own clone of the module frames (scripts.md
            // 1); objects a placement bound, and Sly 1 worlds, keep the shared frame.
            let ordinals = crate::world::sly2::copy_ordinals(&g.world);
            let sly3_bound = g.world.sly3_copy_ordinals_with(true);
            for &h in &handles {
                let m = g.world.module(h);
                if m >= 0 && (m as usize) < modules {
                    // A module given by a proxy override continues in the object's earlier module (scripts.md
                    // 11.3); the interpreter keeps the first outer module a module gets and counts refusals.
                    let outer = g.world.outer_module(h).filter(|&o| o >= 0 && (o as usize) < modules);
                    // Sly 3 hook (classes/placement.md 1): a bind's override continues in its copy's own frame of the
                    // earlier module.
                    let override_copy =
                        outer.filter(|_| g.world.sly3.is_some()).and_then(|_| sly3_bound.get(&h.index()).copied());
                    match (outer, ordinals.get(&h.index())) {
                        (Some(o), _) if override_copy.is_some() => {
                            let copy = override_copy.unwrap_or(0);
                            if !interp.attach_override(h.to_script(), m as u32, o as u32, copy) {
                                g.outer_refused += 1;
                            }
                        }
                        (None, Some(&copy)) => interp.attach_copy(h.to_script(), m as u32, copy),
                        _ => {
                            if !interp.attach_with_outer(h.to_script(), m as u32, outer.map(|o| o as u32)) {
                                g.outer_refused += 1;
                            }
                        }
                    }
                }
            }
            // Sly 3 hook (classes/placement.md 1): a proxy override's module sees its outer module's event 1 too
            // (v_canal's boat: override 335 over 330), so the table is read again now that the outer modules are set.
            if g.world.sly3.is_some() {
                let event1: Vec<bool> = (0..modules as u32)
                    .map(|m| interp.lookup(m, script::event::UPDATE).is_some_and(|v| v.is_callable()))
                    .collect();
                g.world.set_event1_modules(event1);
            }
            {
                // Sly 2 script hook: custom properties declared by the option lists (none for other worlds).
                let props = crate::world::sly2::custom_properties(&g.world);
                let mut host = Self::host(&mut g.world, &mut g.rng, 0.0, g.systems.physics.queries());
                crate::world::sly2::seed_custom_properties(&props, &mut interp, &mut host);
            }
            g.scripts = Some(interp);
            // Event 0, once per object, in load order.
            for h in handles {
                g.world.raise(h, script::event::LOADED, Vec::new());
                g.flush_events();
            }
        }

        // Sly 2 worlds get their behaviours here (the Sly 2 loader attaches none).
        crate::behavior::sly2::attach(&mut g.world);
        crate::behavior::sly3::attach(&mut g.world); // Sly 3 hook: Sly 3 worlds' behaviours (behavior::sly3)
        crate::behavior::vehicles::attach(&mut g.world); // Vehicles hook: Sly 2 and Sly 3 machines (behavior::vehicles)
        // Post-load behaviour, in load order.
        let slots: Vec<u32> = g.world.handles().map(|h| h.index()).collect();
        for s in slots {
            g.run_behaviour(s, 0.0, true);
        }
        // Animations named by option 23 (with option 24 as the start time).
        let animated: Vec<Handle> =
            g.world.handles().filter(|&h| g.world.sly1_meanings() /* Sly 2 and Sly 3 hook */ && g.world.options(h).is_some_and(|o| o.has(opt::ANIMATION))).collect();
        for h in animated {
            let o = g.world.options(h).expect("live");
            let (oid, time) = (o.i16(opt::ANIMATION).unwrap_or(-1), o.f32_or(opt::ANIMATION_TIME, 0.0));
            if let Some(seg) = g.world.find_nearest(oid, Some(h)) {
                g.world.anim.push(AnimCommand::Play { segment: seg, owner: h, time, speed: 1.0, tag: u32::MAX - 1 });
            }
        }
        g.systems.anim.load(&mut g.world);
        g.systems.player.load(&mut g.world, g.systems.physics.queries());
        if let Some(p) = g.world.player {
            g.systems.physics.exclude(&g.world, p);
        }
        g.process_anim_commands();
        g.flush_events();
        g.ambient = audio_frame::ambient_slots(&g.world);
        g.world.update_transforms();
        g.ambient_prev = g
            .ambient
            .iter()
            .map(|&s| g.world.handle(s).and_then(|h| g.world.position(h)).unwrap_or([0.0; 3]))
            .collect();
        g
    }

    fn host<'a>(
        world: &'a mut World,
        rng: &'a mut Rng,
        time: f32,
        collision: &'a dyn systems::CollisionQueries,
    ) -> WorldHost<'a> {
        WorldHost { world, rng, time, collision, errors: 0, kinds: Vec::new() }
    }

    /// The frame number (frames run so far).
    pub fn frame_number(&self) -> u64 {
        self.frame
    }

    /// The interpreter, if the world's scripts run.
    pub fn scripts(&self) -> Option<&Interpreter> {
        self.scripts.as_ref()
    }

    /// The game's random number generator.
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    /// Sly 3 hook (sly3/scripts.md 7.1): raises `event` on `h` at once and returns its handler's value (a command
    /// script's condition that asks its module). `None` without a handler.
    pub fn raise_now(&mut self, h: crate::object::Handle, event: u32, args: &[script::Value]) -> Option<script::Value> {
        let interp = self.scripts.as_mut()?;
        let mut host = Self::host(&mut self.world, &mut self.rng, self.clock.game_time, self.systems.physics.queries());
        let r = interp.raise_event(&mut host, h.to_script(), event, args);
        self.counters.script_errors += host.errors;
        merge_kinds(&mut self.script_error_kinds, host.kinds);
        r
    }

    /// Dispatches the queued script events (scripts.md 6).
    pub fn flush_events(&mut self) {
        let Some(interp) = self.scripts.as_mut() else {
            self.world.events.clear();
            return;
        };
        let mut n = 0u32;
        while let Some(e) = self.world.events.pop_front() {
            n += 1;
            if n > MAX_EVENTS_PER_FLUSH {
                self.counters.dropped_events += self.world.events.len() as u32 + 1;
                self.world.events.clear();
                break;
            }
            let mut host =
                Self::host(&mut self.world, &mut self.rng, self.clock.game_time, self.systems.physics.queries());
            interp.raise_event(&mut host, e.object, e.event, &e.args);
            self.counters.script_errors += host.errors;
            merge_kinds(&mut self.script_error_kinds, host.kinds);
        }
        self.counters.events += n.min(MAX_EVENTS_PER_FLUSH);
    }

    /// Hands the queued animation requests to the animation system.
    pub fn process_anim_commands(&mut self) {
        // Commands can queue more (a finished playback starting the next transition): loop until none are left.
        for _ in 0..64 {
            if self.world.anim.is_empty() {
                break;
            }
            for cmd in core::mem::take(&mut self.world.anim) {
                self.systems.anim.command(&mut self.world, cmd);
            }
        }
    }

    /// Runs one object's behaviour (post-load or a step of `dt`), then its event 1, then the events it raised.
    fn run_behaviour(&mut self, slot: u32, dt: f32, post_load: bool) {
        let Some(me) = self.world.handle(slot) else { return };
        let mut b = self.world.take_behavior(slot);
        {
            let mut cx = Cx {
                world: &mut self.world,
                me,
                dt,
                time: self.clock.game_time,
                progress: &mut self.progress,
                collision: self.systems.physics.queries(),
                rng: &mut self.rng,
                anim: &*self.systems.anim,
            };
            if post_load {
                b.post_load(&mut cx);
            } else {
                b.update(&mut cx);
            }
        }
        self.world.put_behavior(slot, b);
        if !post_load && self.world.raises_event1(slot) {
            self.world.raise(me, script::event::UPDATE, vec![Value::Float(dt)]);
        }
        if !self.world.events.is_empty() {
            self.flush_events();
        }
    }

    /// Updates the ticking objects of one clock: the real-clock ones (`real`) or the others.
    fn update_objects(&mut self, real: bool, dt: f32) {
        let mut ticking = core::mem::take(&mut self.ticking);
        self.world.ticking_snapshot(&mut ticking);
        let t = self.clock.game_time;
        // Sly 2 / Sly 3: inside a visibility volume only objects sharing a bit of its mask are active.
        let mask = self.world.shared.visibility_mask;
        let by_mask = self.world.sly2.is_some() && mask != 0xFF;
        for &s in &ticking {
            let f = self.world.flags_column()[s as usize];
            if f & flags::ACTIVE == 0 || (f & flags::REAL_CLOCK != 0) != real || self.world.is_postponed(s, t) {
                continue;
            }
            // Vehicles hook: the driven machine runs wherever it is (the hacking ship flies far above the level).
            if by_mask
                && self
                    .world
                    .handle(s)
                    .is_some_and(|h| self.world.shared.driving != Some(h) && self.world.sly2_zone_mask(h) & mask == 0)
            {
                continue;
            }
            self.run_behaviour(s, dt, false);
            self.counters.updated += 1;
        }
        self.ticking = ticking;
    }

    /// Runs one frame for a measured interval in seconds (clamped to [1/60, 1/30]).
    pub fn frame(&mut self, input: &FrameInput, measured: f32) {
        self.counters = FrameCounters::default();
        self.input = *input;
        // What behaviours read of the frame: the controller, the camera, the progress' vitals.
        {
            let s = &mut self.world.shared;
            s.input = *input;
            s.confirm_pressed = input.pressed & platform_api::Buttons::B.0 != 0;
            s.camera_eye = self.render.camera.eye;
            s.camera_fov = self.render.camera.fov_y;
            s.time = self.clock.game_time;
            s.difficulty = self.progress.level_ref(self.world.info.id).map_or(0.0, |l| l.difficulty.level);
            let d = crate::math::sub(self.render.camera.target, self.render.camera.eye);
            s.camera_yaw = crate::math::atan2(d[1], d[0]);
            s.vitals = (self.progress.charms, self.progress.lives);
            s.screen.tints.clear();
            if s.dialogue.is_none() {
                s.subtitle = None;
            }
        }
        if self.world.player.is_some() {
            self.clock.power = self.world.shared.time_scale;
        }
        let alarms_before = self.world.shared.alarms_triggered;
        self.world.begin_notice_frame();
        let steps = self.clock.advance(measured);
        let step = StepInfo { frame: self.frame, game_dt: steps.game, real_dt: steps.real, time: self.clock.game_time };

        // 1. Game state.
        // A sprint that runs out simply ends (pickups-and-progress.md 5); the HUD and menus read the progress.
        if steps.game > 0.0 {
            self.progress.tick_sprint(steps.game);
        }
        self.world.update_transforms();

        // 3. Real-clock pass.
        if steps.real > 0.0 {
            self.update_objects(true, steps.real);
        }

        // 4. Game-clock pass.
        if steps.game > 0.0 {
            self.systems.player.decide(&mut self.world, &self.input, &step, self.systems.physics.queries());
            self.flush_events();
            self.apply_player_events();
            self.world.integrate_movers(steps.game, steps.real, self.clock.game_time);
            self.update_objects(false, steps.game);
            crate::behavior::vehicles::shots::step(&mut self.world, steps.game); // Vehicles hook: projectiles
            self.world.clear_inboxes();
            self.process_anim_commands();
        }
        // Animations advance every frame: real-clock playbacks keep going while paused.
        self.systems.anim.advance(&mut self.world, &step);
        self.flush_events();
        self.process_anim_commands();
        if steps.game > 0.0 {
            self.systems.physics.step(&mut self.world, &step);
            self.systems.player.after_physics(&mut self.world, &step, self.systems.physics.queries());
            self.flush_events();
            if let Some(interp) = self.scripts.as_mut() {
                let mut host =
                    Self::host(&mut self.world, &mut self.rng, self.clock.game_time, self.systems.physics.queries());
                interp.tick(&mut host, steps.game);
                self.counters.script_errors += host.errors;
                merge_kinds(&mut self.script_error_kinds, host.kinds);
            }
            self.flush_events();
            // Sly 2 hook: the job system's frame (no effect until one is attached).
            if self.world.sly2.as_deref().is_some_and(|l| l.jobs.is_some()) {
                let i = self.input;
                let touched = i.buttons != 0 || i.left != [0.0; 2] || i.right != [0.0; 2];
                sly2_jobs::step(self, steps.game, touched);
                self.flush_events();
                // Vehicles hook: a job's machine is handed to the player while the job runs.
                let (job, task) = self
                    .world
                    .sly2
                    .as_deref()
                    .and_then(|l| l.jobs.as_deref())
                    .map_or((None, None), crate::behavior::vehicles::shots::job_of);
                crate::behavior::vehicles::shots::follow_job(&mut self.world, job, task.as_deref());
            }
            // Sly 2 hook: state-machine and clip playbacks.
            if self.world.sly2.is_some() {
                sly2_anim::step(&mut self.world, steps.game);
                crate::anim::sly2_pose::step(&mut self.world); // Sly 2 hook: the clips applied to the poses
                crate::behavior::sly2_commands::animation_sequence_starts(&mut self.world);
                crate::behavior::sly2_commands::clip_object_events(&mut self.world);
            }
            self.process_anim_commands();
            self.render.texture_time += steps.game;
            crate::behavior::effects::step_particles(&mut self.world.shared.particles, steps.game);
        }
        self.world.shared.screen.step(steps.real);
        self.route_dialogue_requests();
        // Alarms during a sprint speed its countdown up (pickups-and-progress.md 5).
        let new_alarms = self.world.shared.alarms_triggered - alarms_before;
        if let Some(s) = self.progress.sprint.as_mut() {
            s.alarms += new_alarms;
        }

        // 5. Camera, audio.
        self.world.update_transforms();
        // A cutscene placement drives the camera while one is enabled (animation events 12 and 13).
        let placement = self.world.shared.camera_placement.and_then(|h| self.world.world_transform(h));
        if placement.is_some() || self.placement_was_set {
            self.systems.camera.set_placement(placement);
            self.placement_was_set = placement.is_some();
        }
        let view = self.systems.camera.update(&self.world, &self.input, steps.real, self.systems.physics.queries());
        self.render.camera_extras = self.systems.camera.extras();
        if let Some(m) = self.render.camera_extras.visibility_mask {
            self.world.shared.visibility_mask = m;
        }
        self.render.camera = view;
        let fwd = crate::math::normalize(crate::math::sub(view.target, view.eye));
        let left = crate::math::normalize(crate::math::cross(view.up, fwd));
        let vel = if steps.real > 0.0 {
            crate::math::scale(crate::math::sub(view.eye, self.last_eye), 1.0 / steps.real)
        } else {
            [0.0; 3]
        };
        self.last_eye = view.eye;
        let listener = Listener { position: view.eye, velocity: vel, left };
        self.audio.build(&mut self.world, &self.ambient, listener, &self.ambient_prev, steps.real);
        self.audio.music_scale = self.clock.music_scale();
        self.ambient_prev.clear();
        self.ambient_prev.extend(self.audio.emitters.iter().map(|e| e.position));

        // 6. Retire finished playbacks.
        self.systems.anim.retire(&mut self.world);
        self.flush_events();
        self.process_anim_commands();

        // 7. Render scene.
        self.world.update_transforms();
        self.world.render_changes(&mut self.render.changed);
        self.counters.render_changes = self.render.changed.len() as u32;
        self.systems.anim.skin(&self.world, &mut self.render.skins);
        self.frame += 1;
    }

    /// Applies the player controller's events to the progress: charms and lives lost, deaths, respawns.
    fn apply_player_events(&mut self) {
        use crate::player::{DamageOutcome, PlayerEvent};
        for e in core::mem::take(&mut self.world.shared.player_events) {
            match e {
                PlayerEvent::Damaged(_, DamageOutcome::CharmLost) => {
                    self.progress.charms = (self.progress.charms - 1).max(0)
                }
                PlayerEvent::Damaged(_, DamageOutcome::LifeLost | DamageOutcome::GameOver) => {
                    let over = self.progress.lose_life();
                    let world_id = self.world.info.id;
                    let mask = self.world.info.task_mask;
                    let key = self.progress.has_flag(world_id, progress::level_flag::KEY);
                    if let Some(t) = self.progress.difficulty_tables.clone() {
                        let s = *t.record(mask, key);
                        let lives = self.progress.lives;
                        self.progress.level(world_id).difficulty.on_death(&s, lives, over);
                    }
                    if over {
                        self.world.request(Request::GameOver);
                    }
                }
                PlayerEvent::RespawnRequested => self.world.request(Request::Respawn),
                _ => {}
            }
        }
    }

    /// Dialogue requests from objects (buttons, bottles, vault hints) trigger the dialogue objects they name.
    fn route_dialogue_requests(&mut self) {
        let mut keep = Vec::new();
        for r in core::mem::take(&mut self.world.requests) {
            if let Request::Dialogue(oid) = r {
                let target = self
                    .world
                    .slots_with_oid(oid)
                    .iter()
                    .copied()
                    .find(|&s| self.world.class_of_slot(s) == crate::object::class::id::DIALOGUE);
                if let Some(h) = target.and_then(|s| self.world.handle(s))
                    && let Some(crate::behavior::Behavior::Dialogue(d)) = self.world.behavior_mut(h)
                {
                    d.trigger();
                    self.world.mark_ticking_dirty();
                    continue;
                }
            }
            keep.push(r);
        }
        self.world.requests = keep;
    }

    /// One fixed 1/60 s frame (the deterministic path of the test harness).
    pub fn step_fixed(&mut self, input: &FrameInput) {
        self.frame(input, FIXED_STEP);
    }

    /// The requests objects made since the last call (world changes, rumble, dialogue), in order.
    pub fn take_requests(&mut self) -> Vec<Request> {
        core::mem::take(&mut self.world.requests)
    }

    /// Arrives at a warp after loading (level-flow.md 3): picks the warp, places the player there (or cuts the
    /// camera behind the warp when there is no player), blocks the exits the player stands in. Without a usable
    /// warp, returns `None` and nothing moves.
    pub fn arrive(&mut self, warp: Option<i16>, map_wipe: bool) -> Option<Arrival> {
        let a = self.world.arrive(warp, map_wipe)?;
        self.systems.player.place(&mut self.world, a.transform);
        let fwd = a.transform.cols[0];
        let dir = crate::math::normalize([fwd[0], fwd[1], 0.0]);
        let (s, c) = crate::math::sin_cos(a.camera_angle);
        let dir = [dir[0] * c - dir[1] * s, dir[0] * s + dir[1] * c, 0.0];
        let focus = crate::math::add(a.transform.t, [0.0, 0.0, 125.0]);
        let eye = crate::math::add(crate::math::madd(a.transform.t, dir, -450.0), [0.0, 0.0, 250.0]);
        self.systems.camera.cut(eye, focus);
        self.flush_events();
        self.process_anim_commands();
        self.world.update_transforms();
        Some(a)
    }

    /// A hash of the simulation state for determinism checks: clocks, progress, random state, and every object's
    /// world position and flags.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.f32(self.clock.game_time).f32(self.clock.real_time).u64(self.frame).u64(self.progress.hash());
        for x in self.rng.state() {
            h.u32(x);
        }
        let flags = self.world.flags_column();
        for (s, w) in self.world.world_transforms().iter().enumerate() {
            h.f32(w.t[0]).f32(w.t[1]).f32(w.t[2]).u32(flags[s] as u32);
        }
        h.u32(self.render.camera.eye[0].to_bits()).u32(self.render.camera.eye[1].to_bits());
        h.finish()
    }
}

fn merge_kinds(into: &mut Vec<(script::ScriptError, u32)>, from: Vec<(script::ScriptError, u32)>) {
    for (e, n) in from {
        match into.iter_mut().find(|(x, _)| *x == e) {
            Some((_, m)) => *m += n,
            None => into.push((e, n)),
        }
    }
}

/// FNV-1a, 64 bits: stable across Rust versions (testing.md).
#[derive(Clone, Copy, Debug)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv::new()
    }
}

impl Fnv {
    /// A fresh hash.
    pub const fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    /// Adds bytes.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
        self
    }

    /// Adds a `u32`.
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Adds an `i32`.
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Adds a `u64`.
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Adds an `f32` by its bits.
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.u32(v.to_bits())
    }

    /// The hash.
    pub fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
