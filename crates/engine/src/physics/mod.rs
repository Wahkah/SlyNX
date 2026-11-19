//! Physics: bodies, gravity, the contact solver, springs, smoothing and character movement
//! (specs/sly1/physics.md, clock-and-loop.md, collision.md, options-by-class.md 2.2 and 2.7).
//!
//! | Module | What |
//! |---|---|
//! | [`body`] | body settings and state: mass, gravity and gravity ratio, terminal fall, water, axis locks, probes |
//! | [`solver`] | the physics world and its step: integration, contacts, pivot pass, impacts and penetration, rest |
//! | [`spring`] | restoring springs, the smoothing law and the drive modes toward animation or rest goals |
//! | [`character`] | collide-and-slide movement with the ground rule, optional step-up and ground snapping |
//!
//! The module is independent of the object model: the caller owns a [`crate::collision::CollisionWorld`] with the
//! scenery and the bodies' collision entries, creates bodies here, sets their forces, water fraction and goals, and
//! calls [`PhysicsWorld::step`] once per frame with the game step.

pub mod body;
pub mod character;
pub mod solver;
pub mod spring;

pub use body::{AxisLock, Body, BodyDesc, Motion, Probe, TerminalFall};
pub use character::{CharacterShape, SlideParams, SlideResult, move_and_slide};
pub use solver::{BodyHandle, Contact, Impact, Other, PhysicsWorld, Rng, SolverSettings, SplitMix64};
pub use spring::{Drive, DriveMode, DriveState, Goal, SmoothParams, Smoother, Spring, Strength};
