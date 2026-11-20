//! The systems the game loop drives, as traits with do-nothing defaults: the player's controller, animation,
//! physics, collision queries and the camera. The loop calls them in the order of clock-and-loop.md 4; each gets the
//! world and the step and talks back through the world (positions, notices, events, requests).
//!
//! A system that does not exist yet keeps its null implementation, so the loop runs complete from day one.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::object::Handle;
use crate::world::{AnimCommand, World};

use super::camera::FreeCamera;
use super::render_scene::{CameraView, SkinFrame};

/// The controller as the game sees it for one frame (player 1). Button bits are the platform's
/// (`platform_api::Buttons`); sticks have `x` right and `y` up in [−1, 1].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameInput {
    /// Buttons held.
    pub buttons: u32,
    /// Buttons that went down this frame.
    pub pressed: u32,
    /// Buttons that went up this frame.
    pub released: u32,
    /// Left stick.
    pub left: [f32; 2],
    /// Right stick.
    pub right: [f32; 2],
}

impl FrameInput {
    /// Whether any of `bits` is held.
    pub fn held(&self, bits: platform_api::Buttons) -> bool {
        self.buttons & bits.0 != 0
    }

    /// Whether any of `bits` went down this frame.
    pub fn just_pressed(&self, bits: platform_api::Buttons) -> bool {
        self.pressed & bits.0 != 0
    }
}

/// One frame's times.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepInfo {
    /// Frame number from 0.
    pub frame: u64,
    /// Game-clock step (0 while paused).
    pub game_dt: f32,
    /// Real-clock step.
    pub real_dt: f32,
    /// Game time after this step.
    pub time: f32,
}

/// The player's controller (player-moves.md): decides the player's move from the input before the other objects
/// update, using last frame's positions (clock-and-loop.md 4).
///
/// The seam for `engine::player`: `load` finds or spawns the player object and sets `World::player`; `decide` moves
/// it (character movement against `collision.collision_world()`, notices to what it touches and hits, damage from
/// `World::take_damage_events`); `after_physics` reads the solver's results for the player's body.
pub trait PlayerSystem {
    /// The world was loaded and started (after post-load); set `World::player`.
    fn load(&mut self, world: &mut World, collision: &dyn CollisionQueries) {
        let _ = (world, collision);
    }
    /// Called once per game frame (not while paused), first.
    fn decide(&mut self, world: &mut World, input: &FrameInput, step: &StepInfo, collision: &dyn CollisionQueries) {
        let _ = (world, input, step, collision);
    }
    /// Puts the player at a transform (a warp arrival, a respawn).
    fn place(&mut self, world: &mut World, transform: formats::geometry::Affine) {
        let _ = (world, transform);
    }
    /// Sly 2 hook (menus): the Sly 2 gang, for the gadget assignment screen and the shop (none for other players).
    fn gang_mut(&mut self) -> Option<&mut crate::gang::Gang> {
        None
    }
    /// Sly 3 hook (gadget grids): the gang, read-only (none for other players).
    fn gang(&self) -> Option<&crate::gang::Gang> {
        None
    }
    /// After physics, every game frame.
    fn after_physics(&mut self, world: &mut World, step: &StepInfo, collision: &dyn CollisionQueries) {
        let _ = (world, step, collision);
    }
}

/// Animation playback (animation.md 5): segments on objects, state-machine transitions, events.
pub trait AnimationSystem {
    /// The world was loaded (find auto-play segments, bind skins).
    fn load(&mut self, world: &mut World) {
        let _ = world;
    }
    /// A request from an object or script.
    fn command(&mut self, world: &mut World, cmd: AnimCommand);
    /// Advances playbacks: game-clock playbacks by `step.game_dt`, real-clock ones by `step.real_dt`. Fires events
    /// (script events through the world, wipes through requests). Called once per frame.
    fn advance(&mut self, world: &mut World, step: &StepInfo);
    /// End of frame: removes the playbacks that finished and reports them to their owners
    /// ([`World::animation_finished`]).
    fn retire(&mut self, world: &mut World);
    /// The root-motion velocity (animation.md 4.2) of the playbacks `owner` started, in the owner's facing frame,
    /// units per second; `None` when none of them has a root-motion channel.
    fn root_motion(&self, owner: Handle) -> Option<[f32; 3]> {
        let _ = owner;
        None
    }
    /// Whether a playback `owner` started with `tag` is still playing.
    fn is_playing(&self, owner: Handle, tag: u32) -> bool {
        let _ = (owner, tag);
        false
    }
    /// Skinning matrices for the renderer.
    fn skin(&self, world: &World, out: &mut SkinFrame) {
        let _ = world;
        out.clear();
    }
}

/// Physics (physics.md 3): integrates solids, finds and solves contacts, sends touch notices and events 6 and 7.
/// It owns the collision data, so it also answers the collision queries.
pub trait PhysicsSystem {
    /// The world was loaded: build the collision bodies.
    fn load(&mut self, world: &mut World) {
        let _ = world;
    }
    /// One game step, after animation.
    fn step(&mut self, world: &mut World, step: &StepInfo) {
        let _ = (world, step);
    }
    /// Collision queries over the same data.
    fn queries(&self) -> &dyn CollisionQueries;
    /// Leaves an object and its descendants out of the collision world (the player's own solids, which the player
    /// controller moves itself).
    fn exclude(&mut self, world: &World, root: Handle) {
        let _ = (world, root);
    }
}

/// A hit of a swept sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    /// Fraction of the sweep where it hit, 0..1.
    pub t: f32,
    /// Contact point.
    pub point: [f32; 3],
    /// Surface normal (unit length).
    pub normal: [f32; 3],
    /// The object hit.
    pub object: Handle,
}

/// Queries against collision meshes (collision.md), for behaviours and scripts. `None` means "not available", and
/// callers fall back to simpler tests.
pub trait CollisionQueries {
    /// Whether a world point is inside a solid's collision mesh (trigger volumes, interactables.md 2).
    fn point_in_solid(&self, world: &World, solid: Handle, point: [f32; 3]) -> Option<bool> {
        let _ = (world, solid, point);
        None
    }
    /// Whether a sphere touches a solid's collision mesh (pickups touching the player).
    fn sphere_touches(&self, world: &World, solid: Handle, centre: [f32; 3], radius: f32) -> Option<bool> {
        let _ = (world, solid, centre, radius);
        None
    }
    /// The first solid surface a sphere hits moving from `from` to `to`, ignoring `ignore` (falling pickups).
    fn sweep_sphere(
        &self,
        world: &World,
        from: [f32; 3],
        to: [f32; 3],
        radius: f32,
        ignore: Handle,
    ) -> Option<SweepHit> {
        let _ = (world, from, to, radius, ignore);
        None
    }
    /// The solids overlapping a sphere (operation 112). Returns false when not available.
    fn overlap_sphere(&self, world: &World, centre: [f32; 3], radius: f32, out: &mut Vec<Handle>) -> bool {
        let _ = (world, centre, radius, out);
        false
    }
    /// Whether the segment between two points is clear of solids (line of sight), ignoring the listed objects.
    /// `None` when not available.
    fn line_clear(&self, world: &World, from: [f32; 3], to: [f32; 3], ignore: &[Handle]) -> Option<bool> {
        let _ = (world, from, to, ignore);
        None
    }
    /// The ground below a point within `max_drop`: (point, normal, object hit).
    fn ground(&self, world: &World, point: [f32; 3], max_drop: f32) -> Option<([f32; 3], [f32; 3], Option<Handle>)> {
        let _ = (world, point, max_drop);
        None
    }
    /// The underlying collision world, for systems that query it directly (the player's movement). `None` when
    /// collision is not available.
    fn collision_world(&self) -> Option<&crate::collision::CollisionWorld> {
        None
    }
    /// The object a collision body stands for.
    fn object_of_body(&self, world: &World, body: crate::collision::BodyId) -> Option<Handle> {
        let _ = (world, body);
        None
    }
}

/// A picture panel: eye, target, field of view, screen position.
pub type PanelView = ([f32; 3], [f32; 3], f32, [f32; 2]);

/// What the camera gives the renderer besides the view: meshes faded because they block it, and the picture panels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraExtras {
    /// Meshes to fade: (record, mesh index in its mesh set).
    pub faded: Vec<(u32, u16)>,
    /// Picture panels (dialogue speakers): eye, target, field of view, screen position.
    pub panels: [Option<PanelView>; 2],
    /// Sly 2 / Sly 3: the object mask the visibility volumes select (camera.md 7.1), when the camera has them.
    pub visibility_mask: Option<u32>,
}

/// The camera (camera.md): updated every frame after everything else, also while paused, on the real clock. It gets
/// the physics system's collision queries, whose collision world holds the solids where they are now.
pub trait CameraSystem {
    /// Updates and returns the view.
    fn update(
        &mut self,
        world: &World,
        input: &FrameInput,
        real_dt: f32,
        collision: &dyn CollisionQueries,
    ) -> CameraView;
    /// Faded meshes and picture panels of the last update.
    fn extras(&self) -> CameraExtras {
        CameraExtras::default()
    }
    /// A cutscene camera placement's world transform (animation events 12 and 13), or `None` to go back.
    fn set_placement(&mut self, placement: Option<formats::geometry::Affine>) {
        let _ = placement;
    }
    /// Cuts to a new place in one frame (a warp arrival, a level start): `eye` looking at `target`.
    fn cut(&mut self, eye: [f32; 3], target: [f32; 3]) {
        let _ = (eye, target);
    }
    /// Sets the lens from the world options (camera.md 1).
    fn configure(&mut self, world: &World) {
        let _ = world;
    }
}

/// A player system that does nothing (no player yet).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPlayer;
impl PlayerSystem for NoPlayer {}

/// A physics system that does nothing and answers no query.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPhysics;
impl PhysicsSystem for NoPhysics {
    fn queries(&self) -> &dyn CollisionQueries {
        &NoCollision
    }
}

/// Collision queries that are never available.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCollision;
impl CollisionQueries for NoCollision {}

/// An animation system that plays nothing: every playback ends at once (reported at the end of the frame), so
/// objects waiting for animations go on.
#[derive(Clone, Debug, Default)]
pub struct InstantAnimation {
    finished: Vec<(Handle, u32)>,
}

impl AnimationSystem for InstantAnimation {
    fn command(&mut self, _world: &mut World, cmd: AnimCommand) {
        match cmd {
            AnimCommand::Play { owner, tag, .. } | AnimCommand::PlayOwned { owner, tag, .. } => {
                self.finished.push((owner, tag))
            }
            AnimCommand::Stop { owner, tag } => self.finished.retain(|&f| f != (owner, tag)),
            AnimCommand::Finish { .. } => {}
        }
    }

    fn advance(&mut self, _world: &mut World, _step: &StepInfo) {}

    fn retire(&mut self, world: &mut World) {
        for (owner, tag) in core::mem::take(&mut self.finished) {
            world.animation_finished(owner, tag);
        }
    }
}

/// The systems of a game.
pub struct Systems {
    /// Player controller.
    pub player: Box<dyn PlayerSystem>,
    /// Animation.
    pub anim: Box<dyn AnimationSystem>,
    /// Physics and collision queries.
    pub physics: Box<dyn PhysicsSystem>,
    /// Camera.
    pub camera: Box<dyn CameraSystem>,
}

impl Default for Systems {
    /// Sly (`engine::player`), the segment player over `engine::anim`, physics and collision over `engine::physics`
    /// and `engine::collision` (with neutral material tables; see [`Systems::for_game`]), the follow camera
    /// (`engine::camera`).
    fn default() -> Self {
        Systems::for_game(None)
    }
}

impl Systems {
    /// The game's systems, with the material tables read from the player's executable
    /// (`formats::collision::MaterialTables::from_executable`).
    pub fn for_game(materials: Option<formats::collision::MaterialTables>) -> Systems {
        Systems {
            player: Box::new(super::player_bridge::SlyPlayer::new(None)),
            anim: Box::new(super::anim_bridge::SegmentPlayer::new()),
            physics: Box::new(super::physics_bridge::PhysicsBridge::new(materials)),
            camera: Box::new(crate::camera::system::CameraSystem::default()),
        }
    }

    /// Null systems everywhere (instant animations, no physics), with the free camera: the cheapest loop, for
    /// tests.
    pub fn null() -> Systems {
        Systems {
            player: Box::new(NoPlayer),
            anim: Box::new(InstantAnimation::default()),
            physics: Box::new(NoPhysics),
            camera: Box::new(FreeCamera::default()),
        }
    }

    /// The collision queries.
    pub fn collision(&self) -> &dyn CollisionQueries {
        self.physics.queries()
    }
}
