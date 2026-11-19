//! Per-class behaviour.
//!
//! Every object has a [`Behavior`]: `None` for the many objects that only hold data (static scenery, markers,
//! lights, bones, segments), or the state of its class's behaviour. Dispatch is a `match` over the enum, so the
//! common case costs nothing and the per-frame loop touches only the objects whose behaviour ticks (the world's
//! ticking list, in update order).
//!
//! # Adding a class
//!
//! 1. Write a struct with the class's state and implement [`ClassBehavior`] for it (only the hooks it needs).
//! 2. Add a variant to [`Behavior`] (boxed, so the enum stays small) and to the `dispatch!` macro.
//! 3. Return it from [`Behavior::for_class`] for the class IDs it serves.
//!
//! A behaviour that is not ready for its own variant can use [`Behavior::Custom`] with any `Box<dyn ClassBehavior>`.
//! The player, the camera's target, guards and sensors are planned as their own variants (see `docs/engine.md`).

pub mod breakable;
pub mod button;
pub mod checkpoint;
pub mod dialogue;
pub mod effects;
pub mod exit;
pub mod guard;
pub mod hazard;
pub mod machine;
pub mod pickup;
pub mod sensor;
pub mod sly2;
pub mod sly2_commands;
pub mod sly2_pickups;
pub mod sly2_props;
pub mod sly2_sensors;
pub mod sly2_world;
pub mod sly3; // Sly 3 hook: Sly 3 behaviours (behavior::sly3)
pub mod vault;
pub mod vehicles; // Vehicles hook: driven machines and minigames (behavior::vehicles)
pub mod volume;
pub mod warp;

use alloc::boxed::Box;

use crate::game::progress::Progress;
use crate::game::rng::Rng;
use crate::game::systems::CollisionQueries;
use crate::object::class::id;
use crate::object::{Arg, Handle, OptionValue};
use crate::world::World;

pub use button::Button;
pub use checkpoint::Checkpoint;
pub use exit::Exit;
pub use machine::Machine;
pub use pickup::{Bottle, Pickup, PickupKind};
pub use volume::TriggerVolume;
pub use warp::{Arrival, Warp};

/// What a behaviour gets during post-load and its per-frame update.
pub struct Cx<'a> {
    /// The world (the behaviour itself is taken out of it while it runs).
    pub world: &'a mut World,
    /// The object.
    pub me: Handle,
    /// This step's time in seconds: the game step, or the real step for objects on the real clock.
    pub dt: f32,
    /// Game time in seconds since the world started.
    pub time: f32,
    /// The player's progress (counters, level records).
    pub progress: &'a mut Progress,
    /// Collision queries (contacts and containment against collision meshes).
    pub collision: &'a dyn CollisionQueries,
    /// The game's deterministic random number generator.
    pub rng: &'a mut Rng,
    /// The animation system (root motion, playing state).
    pub anim: &'a dyn crate::game::systems::AnimationSystem,
}

impl Cx<'_> {
    /// Where the player is for triggers: the player object, else the probe (the free camera in development).
    pub fn player_position(&self) -> Option<[f32; 3]> {
        match self.world.player {
            Some(p) => self.world.position(p),
            None => self.world.probe,
        }
    }

    /// Where a pickup collector is: the player object, or the probe when it may collect.
    pub fn collector_position(&self) -> Option<[f32; 3]> {
        match self.world.player {
            Some(p) => self.world.position(p),
            None if self.world.probe_collects => self.world.probe,
            None => None,
        }
    }

    /// This object's world position.
    pub fn my_position(&self) -> [f32; 3] {
        self.world.position(self.me).unwrap_or([0.0; 3])
    }

    /// Raises a script event on this object.
    pub fn raise(&mut self, event: u32, args: alloc::vec::Vec<script::Value>) {
        self.world.raise(self.me, event, args);
    }
}

/// The hooks a class behaviour can implement. Every hook has a default that does nothing.
pub trait ClassBehavior {
    /// Whether the object needs [`ClassBehavior::update`] every frame. Asked when the world rebuilds its ticking
    /// list (after the tree or a behaviour changes); a behaviour that stops ticking keeps being called until then,
    /// so `update` must be harmless when idle.
    fn ticks(&self) -> bool {
        false
    }

    /// Once, after the world has loaded and event 0 was raised (gameplay-overview.md 3, "post-load").
    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let _ = cx;
    }

    /// One step (game or real clock, as the object runs).
    fn update(&mut self, cx: &mut Cx<'_>) {
        let _ = cx;
    }

    /// An option was set (by a script or an option list after load) and stored.
    fn option_changed(&mut self, world: &mut World, me: Handle, id: u16, value: &OptionValue) {
        let _ = (world, me, id, value);
    }

    /// An action option other than option 2 was called. Returns whether it was handled.
    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[Arg]) -> bool {
        let _ = (world, me, id, args);
        false
    }

    /// A playback this object started (with `tag`) ended.
    fn animation_finished(&mut self, world: &mut World, me: Handle, tag: u32) {
        let _ = (world, me, tag);
    }

    /// A live value for an option read by a script, overriding the stored one.
    fn get_option(&self, world: &World, me: Handle, id: u16) -> Option<OptionValue> {
        let _ = (world, me, id);
        None
    }
}

/// An object's behaviour.
#[derive(Default)]
pub enum Behavior {
    /// Data only.
    #[default]
    None,
    /// Sly 2 guard or swarming creature (classes 58, 57; specs/sly2/guards.md).
    Sly2Guard(Box<sly2::Sly2Guard>),
    /// Sly 2 noise template (class 117; specs/sly2/guards.md 4.1).
    Sly2Noise(Box<sly2::NoiseTemplate>),
    /// Sly 2 laser beams (class 85; specs/sly2/alarms-sensors.md 2).
    Sly2Laser(Box<sly2_sensors::Laser>),
    /// Sly 2 touch pad (class 83; alarms-sensors.md 3).
    Sly2Pad(Box<sly2_sensors::TouchPad>),
    /// Sly 2 stand-alone detector (class 16; alarms-sensors.md 4).
    Sly2Detector(Box<sly2_sensors::Detector>),
    /// Sly 2 coin, task item, medicine, loot item, clue bottle (classes 91 to 95; pickpocket-and-pickups.md 3 to 6).
    Sly2Pickup(Box<sly2_pickups::Sly2Pickup>),
    /// Sly 2 reactive prop (class 98; interactables.md 3).
    Sly2Prop(Box<sly2_props::Prop>),
    /// Sly 2 sequence (class 114): its start and stop calls (dialogue-and-voice.md 2.1).
    Sly2Sequence(Box<sly2_commands::Sequence>),
    /// Sly 2 guard command list (class 113; options-by-class.md 2.2).
    Sly2Commands(Box<sly2_commands::CommandList>),
    /// Sly 2 continuous emitter (class 29; effects.md 1).
    Sly2Emitter(Box<sly2_world::Emitter>),
    /// Sly 2 explosion group or part (classes 31 to 33; effects.md 1.6, 1.7).
    Sly2Explosion(Box<sly2_world::Explosion>),
    /// Sly 2 water (class 89; interactables.md 7).
    Sly2Water(Box<sly2_world::Water>),
    /// Sly 2 hiding barrel (class 88; interactables.md 5).
    Sly2Barrel(Box<sly2_world::Barrel>),
    /// Sly 2 thrown object, dart, bullet or missile (classes 79, 81, 82; interactables.md 8).
    Sly2Thrown(Box<sly2_world::Thrown>),
    /// Sly 2 sound manager (class 152): queues music streams (action 146).
    Sly2SoundManager(Box<sly2_world::SoundManager>),
    /// Sly 2 trigger volume (classes 24, 25; interactables.md 4).
    Sly2Volume(Box<sly2_props::TriggerVolume>),
    /// Coin, charm, life token, key, sprint hourglass (pickups-and-progress.md 2).
    Pickup(Box<Pickup>),
    /// Clue bottle (pickups-and-progress.md 4.1).
    Bottle(Box<Bottle>),
    /// Button (interactables.md 1).
    Button(Box<Button>),
    /// Trigger volume (interactables.md 2).
    Volume(Box<TriggerVolume>),
    /// Level exit (level-flow.md 2).
    Exit(Box<Exit>),
    /// Warp point (level-flow.md 3).
    Warp(Box<Warp>),
    /// Checkpoint (level-flow.md 4).
    Checkpoint(Box<Checkpoint>),
    /// State machine (scripts.md 9).
    Machine(Box<Machine>),
    /// Guard (guards.md).
    Guard(Box<guard::Guard>),
    /// Alarm (alarms-sensors.md 2).
    Alarm(Box<sensor::Alarm>),
    /// Laser, searchlight, pressure plate (alarms-sensors.md 3 to 6).
    Sensor(Box<sensor::Sensor>),
    /// Breakable (interactables.md 4).
    Breakable(Box<breakable::Breakable>),
    /// Breakable piece.
    Piece(Box<breakable::Piece>),
    /// Hazard (interactables.md 5).
    Hazard(Box<hazard::Hazard>),
    /// Water.
    Water(Box<hazard::Water>),
    /// Barrier (interactables.md 8).
    Barrier(Box<hazard::Barrier>),
    /// Conveyor (interactables.md 9).
    Conveyor(Box<hazard::Conveyor>),
    /// Hiding basket (interactables.md 7).
    Basket(Box<hazard::Basket>),
    /// Bomb, missile, mine (interactables.md 6).
    Bomb(Box<hazard::Bomb>),
    /// Dialogue (dialogue-and-voice.md).
    Dialogue(Box<dialogue::Dialogue>),
    /// Particle emitter (effects.md 1).
    Emitter(Box<effects::Emitter>),
    /// Explosion (effects.md 2).
    Explosion(Box<effects::Explosion>),
    /// Lightning flash (effects.md 4).
    Lightning(Box<effects::Lightning>),
    /// Sky (effects.md 6).
    Sky(Box<effects::Sky>),
    /// Vault (pickups-and-progress.md 4.2).
    Vault(Box<vault::Vault>),
    /// Lock group (level-flow.md 7.1).
    LockGroup(Box<vault::LockGroup>),
    /// Any other behaviour.
    Custom(Box<dyn ClassBehavior>),
}

macro_rules! dispatch {
    ($self:expr, $b:ident => $e:expr, $none:expr) => {
        match $self {
            Behavior::None => $none,
            Behavior::Pickup($b) => $e,
            Behavior::Bottle($b) => $e,
            Behavior::Button($b) => $e,
            Behavior::Volume($b) => $e,
            Behavior::Exit($b) => $e,
            Behavior::Warp($b) => $e,
            Behavior::Checkpoint($b) => $e,
            Behavior::Machine($b) => $e,
            Behavior::Guard($b) => $e,
            Behavior::Alarm($b) => $e,
            Behavior::Sensor($b) => $e,
            Behavior::Breakable($b) => $e,
            Behavior::Piece($b) => $e,
            Behavior::Hazard($b) => $e,
            Behavior::Water($b) => $e,
            Behavior::Barrier($b) => $e,
            Behavior::Conveyor($b) => $e,
            Behavior::Basket($b) => $e,
            Behavior::Bomb($b) => $e,
            Behavior::Dialogue($b) => $e,
            Behavior::Emitter($b) => $e,
            Behavior::Explosion($b) => $e,
            Behavior::Lightning($b) => $e,
            Behavior::Sky($b) => $e,
            Behavior::Vault($b) => $e,
            Behavior::LockGroup($b) => $e,
            Behavior::Sly2Guard($b) => $e,
            Behavior::Sly2Noise($b) => $e,
            Behavior::Sly2Laser($b) => $e,
            Behavior::Sly2Pad($b) => $e,
            Behavior::Sly2Detector($b) => $e,
            Behavior::Sly2Pickup($b) => $e,
            Behavior::Sly2Prop($b) => $e,
            Behavior::Sly2Sequence($b) => $e,
            Behavior::Sly2Commands($b) => $e,
            Behavior::Sly2Emitter($b) => $e,
            Behavior::Sly2Explosion($b) => $e,
            Behavior::Sly2Water($b) => $e,
            Behavior::Sly2Barrel($b) => $e,
            Behavior::Sly2Thrown($b) => $e,
            Behavior::Sly2SoundManager($b) => $e,
            Behavior::Sly2Volume($b) => $e,
            Behavior::Custom($b) => $e,
        }
    };
}

impl Behavior {
    /// The behaviour of a freshly loaded object, from its class.
    pub fn for_class(world: &World, h: Handle) -> Behavior {
        let Some(class) = world.class(h) else { return Behavior::None };
        match class {
            id::COIN => Behavior::Pickup(Box::new(Pickup::new(PickupKind::Coin))),
            id::CHARM => Behavior::Pickup(Box::new(Pickup::new(PickupKind::Charm))),
            id::LIFE => Behavior::Pickup(Box::new(Pickup::new(PickupKind::Life))),
            id::KEY => Behavior::Pickup(Box::new(Pickup::new(PickupKind::Key))),
            id::HOURGLASS => Behavior::Pickup(Box::new(Pickup::new(PickupKind::Hourglass))),
            id::CLUE => Behavior::Bottle(Box::new(Bottle::new(world, h))),
            id::BUTTON => Behavior::Button(Box::new(Button::new(world, h))),
            id::TRIGGER_VOLUME | id::TRIGGER_VOLUME_2 => Behavior::Volume(Box::new(TriggerVolume::new(world, h))),
            id::EXIT => Exit::new(world, h).map_or(Behavior::None, |e| Behavior::Exit(Box::new(e))),
            id::WARP => Behavior::Warp(Box::new(Warp::new(world, h))),
            id::CHECKPOINT => Behavior::Checkpoint(Box::new(Checkpoint::new())),
            id::STATE_MACHINE => Machine::new(world, h).map_or(Behavior::None, |m| Behavior::Machine(Box::new(m))),
            c if guard::is_guard_class(c) => Behavior::Guard(Box::new(guard::Guard::new(world, h))),
            47 => Behavior::Alarm(Box::default()),
            49..=51 => Behavior::Sensor(Box::new(sensor::Sensor::new(world, h))),
            33..=38 => Behavior::Breakable(Box::new(breakable::Breakable::new(class))),
            id::PIECE => Behavior::Piece(Box::new(breakable::Piece { released: false, age: 0.0 })),
            54 | 55 => Behavior::Hazard(Box::new(hazard::Hazard::new(class))),
            31 => Behavior::Water(Box::default()),
            52 => Behavior::Barrier(Box::default()),
            57 => Behavior::Conveyor(Box::new(hazard::Conveyor::new(world, h))),
            58 => Behavior::Basket(Box::default()),
            59..=63 => Behavior::Bomb(Box::default()),
            101 => Behavior::Dialogue(Box::new(dialogue::Dialogue::new(world, h))),
            80 => Behavior::Emitter(Box::default()),
            120..=122 => Behavior::Explosion(Box::default()),
            145 => Behavior::Lightning(Box::default()),
            84 => Behavior::Sky(Box::default()),
            23 => Behavior::Vault(Box::default()),
            93 => Behavior::LockGroup(Box::default()),
            _ => Behavior::None,
        }
    }

    /// Whether the object updates every frame.
    pub fn ticks(&self) -> bool {
        dispatch!(self, b => b.ticks(), false)
    }

    /// See [`ClassBehavior::post_load`].
    pub fn post_load(&mut self, cx: &mut Cx<'_>) {
        dispatch!(self, b => b.post_load(cx), ())
    }

    /// See [`ClassBehavior::update`].
    pub fn update(&mut self, cx: &mut Cx<'_>) {
        dispatch!(self, b => b.update(cx), ())
    }

    /// See [`ClassBehavior::option_changed`].
    pub fn option_changed(&mut self, world: &mut World, me: Handle, id: u16, value: &OptionValue) {
        dispatch!(self, b => b.option_changed(world, me, id, value), ())
    }

    /// See [`ClassBehavior::action`].
    pub fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[Arg]) -> bool {
        dispatch!(self, b => b.action(world, me, id, args), false)
    }

    /// See [`ClassBehavior::animation_finished`].
    pub fn animation_finished(&mut self, world: &mut World, me: Handle, tag: u32) {
        dispatch!(self, b => b.animation_finished(world, me, tag), ())
    }

    /// See [`ClassBehavior::get_option`].
    pub fn get_option(&self, world: &World, me: Handle, id: u16) -> Option<OptionValue> {
        dispatch!(self, b => b.get_option(world, me, id), None)
    }

    /// The exit behaviour, if this is one.
    pub fn as_exit_mut(&mut self) -> Option<&mut Exit> {
        match self {
            Behavior::Exit(e) => Some(e),
            _ => None,
        }
    }

    /// The state machine, if this is one.
    pub fn as_machine_mut(&mut self) -> Option<&mut Machine> {
        match self {
            Behavior::Machine(m) => Some(m),
            _ => None,
        }
    }
}

/// Plays every child animation segment (class 131) of `me`, tagged `tag`. Returns how many were started.
pub(crate) fn play_child_segments(world: &mut World, me: Handle, tag: u32, restart: bool) -> u32 {
    let segs = world.children_of_class(me, id::SEGMENT);
    for &s in &segs {
        world.anim.push(crate::world::AnimCommand::Play {
            segment: s,
            owner: me,
            time: if restart { 0.0 } else { -1.0 },
            speed: 1.0,
            tag,
        });
    }
    segs.len() as u32
}

/// Plays the animations an "add" option lists (OIDs), tagged `tag`.
pub(crate) fn play_listed(world: &mut World, me: Handle, option: u16, tag: u32, restart: bool) {
    let oids: alloc::vec::Vec<i16> = world.options(me).map(|o| o.shorts(option).take(16).collect()).unwrap_or_default();
    for oid in oids {
        if let Some(seg) = world.find_nearest(oid, Some(me)) {
            world.anim.push(crate::world::AnimCommand::Play {
                segment: seg,
                owner: me,
                time: if restart { 0.0 } else { -1.0 },
                speed: 1.0,
                tag,
            });
        }
    }
}

/// Applies the goal records of an "add" option (options-by-class.md 1.2: on-press flag, root OID, machine OID,
/// goal OID; up to 7) for a press (`pressed`) or a release, to machines that already run.
pub(crate) fn apply_goal_records(world: &mut World, me: Handle, option: u16, pressed: bool) {
    let records: alloc::vec::Vec<(bool, i16, i16)> = world
        .options(me)
        .map(|o| {
            o.all(option)
                .take(7)
                .filter_map(|v| {
                    let OptionValue::Args(a) = v else { return None };
                    let flag = matches!(a.first(), Some(Arg::Bool(true)) | Some(Arg::Int(1)));
                    let shorts: alloc::vec::Vec<i16> =
                        a.iter().filter_map(|x| if let Arg::Short(_, n) = x { Some(*n) } else { None }).collect();
                    Some((flag, *shorts.get(1)?, *shorts.get(2)?))
                })
                .collect()
        })
        .unwrap_or_default();
    for (flag, machine, goal) in records {
        if flag != pressed {
            continue;
        }
        let Some(m) = world.find_nearest(machine, Some(me)) else { continue };
        let running = matches!(world.behavior(m), Some(Behavior::Machine(mc)) if mc.run.running);
        if running {
            world.set_option(m, 516, OptionValue::Short(goal));
        }
    }
}

/// Whether an object may press a button or trigger volume: the allowed list (objects, classes) when there is one,
/// else anything; never the denied ones.
pub(crate) fn may_press(
    world: &World,
    me: Handle,
    who: Option<Handle>,
    allow: u16,
    deny: Option<u16>,
    classes: Option<(u16, u16)>,
) -> bool {
    let Some(o) = world.options(me) else { return true };
    let oid = who.and_then(|h| world.oid(h)).unwrap_or(5);
    let class = who.and_then(|h| world.class(h)).unwrap_or(id::PLAYER);
    if deny.is_some_and(|d| o.shorts(d).any(|x| x == oid)) {
        return false;
    }
    if let Some((_, deny_c)) = classes
        && o.shorts(deny_c).any(|x| x == class)
    {
        return false;
    }
    let listed_objects = o.has(allow);
    let listed_classes = classes.is_some_and(|(a, _)| o.has(a));
    if !listed_objects && !listed_classes {
        return true;
    }
    o.shorts(allow).any(|x| x == oid) || classes.is_some_and(|(a, _)| o.shorts(a).any(|x| x == class))
}

/// The squared distance between two points.
#[inline]
pub(crate) fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    crate::math::length_sq(crate::math::sub(a, b))
}
