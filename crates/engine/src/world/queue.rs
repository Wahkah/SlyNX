//! What objects ask of the systems around them during a frame. The world collects these; the game loop hands them
//! to the animation system, the audio frame and the game application in a fixed order.

use crate::object::Handle;

/// A request for the animation system (animation.md 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimCommand {
    /// Apply the animation segment `segment` (a class-131 object) on its root, at `time` seconds into it.
    Play {
        /// The segment object.
        segment: Handle,
        /// The object that asked (a button, an exit, a script); finished playbacks are reported to it.
        owner: Handle,
        /// Start time in seconds.
        time: f32,
        /// Speed multiplier.
        speed: f32,
        /// Owner's tag, given back in [`crate::world::World::animation_finished`].
        tag: u32,
    },
    /// Apply an animation segment that is not an object: one owned by a warp, an exit or a state machine
    /// (`record` is the owner's record; `index` the owned animation's position in its owner's list).
    PlayOwned {
        /// The owning object.
        owner: Handle,
        /// Index of the owned animation (warp and exit animations in order; state-machine transitions with an
        /// animation, in order).
        index: u32,
        /// Play backwards from the end (a two-way state-machine transition walked in reverse).
        reverse: bool,
        /// Speed multiplier.
        speed: f32,
        /// Owner's tag.
        tag: u32,
    },
    /// Stop whatever the owner started with `tag`.
    Stop {
        /// The owner.
        owner: Handle,
        /// The tag given when it was started.
        tag: u32,
    },
    /// Jump the playbacks of `owner` with `tag` to their end (a remembered button restored after a checkpoint).
    Finish {
        /// The owner.
        owner: Handle,
        /// The tag.
        tag: u32,
    },
}

/// A one-shot sound effect (audio.md; scripts.md operation 119).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundRequest {
    /// Effect-bank sound ID.
    pub sound: i32,
    /// The emitting object (positioned at it), or `None` for an unpositioned sound.
    pub emitter: Option<Handle>,
}

/// A request for the game application: things that leave the world or touch the hardware.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Load another world (or reload this one) through a screen wipe (level-flow.md 2 and 5).
    LoadWorld {
        /// The destination level descriptor's identity (the world table's search key).
        identity: u32,
        /// Warp OID or alias to arrive at (exit option 163), or `None` for the default warp.
        warp: Option<i16>,
        /// Wipe kind (exit option 166): 0 fade, 1 iris, 2 world map, 3 freeze.
        wipe: u8,
    },
    /// Load the world with this name (animation events 25 and 26).
    LoadWorldNamed {
        /// The level name in the event.
        name: alloc::string::String,
        /// Arrival warp OID.
        warp: Option<i16>,
        /// Wipe kind.
        wipe: u8,
    },
    /// Leave through the world's default exit (animation event 27, "leave level"), resolved by the game.
    DefaultExit,
    /// Controller rumble: kind and strength (operation 121), or a duration for gameplay rumbles.
    Rumble {
        /// Kind.
        kind: i32,
        /// Strength 0..1.
        strength: f32,
    },
    /// Play a dialogue object (OID) (dialogue-and-voice.md; buttons' option 265, bottle hints).
    Dialogue(i16),
    /// The progress changed in a way the original saves automatically (a key, a vault, a sprint record).
    Autosave,
    /// The key celebration (pickups-and-progress.md 6).
    Celebrate,
    /// The player died and the death has played: reload at the respawn point (level-flow.md 4).
    Respawn,
    /// No lives left (level-flow.md 4: the "game over" prompt).
    GameOver,
    /// Play a movie (descriptor identity; dialogue event 7, animation event 51).
    Movie(u32),
    /// A puff of smoke (operation 122).
    Smoke {
        /// Position.
        position: [f32; 3],
        /// Size.
        size: f32,
    },
}

/// A request for the physics system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsCommand {
    /// Hand the object to the solver: it falls, bounces and rests (a released breakable piece, a thrown bomb).
    /// `radius` gives a contact sphere when the object has no collision mesh of its own.
    Dynamic {
        /// The object.
        object: Handle,
        /// Initial velocity.
        velocity: [f32; 3],
        /// Initial angular velocity.
        angular: [f32; 3],
        /// Contact sphere radius for objects without a collision mesh.
        radius: Option<f32>,
    },
    /// An impulse at a world point (an explosion's push, a whack).
    Impulse {
        /// The object.
        object: Handle,
        /// Impulse (mass × velocity).
        impulse: [f32; 3],
        /// Where.
        point: [f32; 3],
    },
    /// Take the object back from the solver (the object model moves it again).
    Release(Handle),
}

/// Something that happened to an object, delivered to its behaviour on its next update (contacts, hits; sent by
/// the player, physics and collision systems). Contacts are sent every frame they last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Notice {
    /// Another object touches it.
    Touched(Handle),
    /// An attack reached it (the attacker).
    Hit(Handle),
    /// The player hangs from it.
    Hang(Handle),
    /// The player let go.
    Released(Handle),
    /// The object hit something (or was hit) faster than the impact threshold: the other object (`None` for
    /// scenery) and the approach speed.
    Impact(Option<Handle>, f32),
    /// An attack of a kind (options 91 and 115: which attacks affect an object) reached it from an attacker.
    Attack {
        /// Attacker.
        by: Handle,
        /// Attack kind bit (player-moves.md).
        kind: u32,
    },
    /// An explosion reached it: the bomb, and the push direction times strength.
    Blast(Handle, [f32; 3]),
    /// The player picked its pocket (Sly 2, pickpocket-and-pickups.md): the thief.
    Pickpocket(Handle),
}
