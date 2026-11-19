//! What the sound engine needs from the engine each frame: the listener (the camera), the positioned ambient sounds
//! of objects (placed-object options 101 to 109) and the one-shot sounds raised this frame. The fields follow the
//! sound engine's listener and emitter (audio.md 3.6), so the hand-over is a copy.

use alloc::vec::Vec;

use crate::object::Handle;
use crate::world::{World, opt};

/// The listener.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    /// Position.
    pub position: [f32; 3],
    /// Velocity, units per second (Doppler).
    pub velocity: [f32; 3],
    /// The camera's left axis (unit length).
    pub left: [f32; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Listener { position: [0.0; 3], velocity: [0.0; 3], left: [-1.0, 0.0, 0.0] }
    }
}

/// An object's ambient sound (options 101 to 109).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientEmitter {
    /// The object.
    pub object: Handle,
    /// Sound ID (option 101).
    pub sound: i32,
    /// World position.
    pub position: [f32; 3],
    /// Velocity, units per second.
    pub velocity: [f32; 3],
    /// Silent beyond this distance (option 102).
    pub start_distance: f32,
    /// Full volume within this distance (option 103).
    pub full_distance: f32,
    /// Volume 0..1 (option 104; 1 when unset).
    pub volume: f32,
    /// Pitch offset (option 105; 0 when unset).
    pub pitch: f32,
    /// Doppler factor (option 109; 0 when unset).
    pub doppler: f32,
    /// Whether the object is active this frame (an inactive one should fall silent).
    pub active: bool,
}

/// A one-shot sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OneShot {
    /// Sound ID.
    pub sound: i32,
    /// Where (`None`: unpositioned).
    pub position: Option<[f32; 3]>,
    /// The emitting object.
    pub object: Option<Handle>,
}

/// The sound engine's input for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioFrame {
    /// The listener.
    pub listener: Listener,
    /// Ambient sounds, one per object that has one, in load order.
    pub emitters: Vec<AmbientEmitter>,
    /// One-shots raised this frame, in order.
    pub one_shots: Vec<OneShot>,
    /// Music volume factor (the pause screen's easing).
    pub music_scale: f32,
}

/// The slots of objects with an ambient sound (option 101), in load order.
pub fn ambient_slots(world: &World) -> Vec<u32> {
    world.handles().filter(|&h| world.options(h).is_some_and(|o| o.has(opt::SOUND))).map(|h| h.index()).collect()
}

impl AudioFrame {
    /// Fills the frame: the listener from the camera, the ambient emitters of `slots`, and the world's queued
    /// one-shots (which are taken).
    pub fn build(&mut self, world: &mut World, slots: &[u32], listener: Listener, prev: &[[f32; 3]], dt: f32) {
        self.listener = listener;
        self.emitters.clear();
        for (k, &s) in slots.iter().enumerate() {
            let Some(h) = world.handle(s) else { continue };
            let Some(o) = world.options(h) else { continue };
            let position = world.position(h).unwrap_or([0.0; 3]);
            let velocity = match prev.get(k) {
                Some(p) if dt > 0.0 => crate::math::scale(crate::math::sub(position, *p), 1.0 / dt),
                _ => [0.0; 3],
            };
            self.emitters.push(AmbientEmitter {
                object: h,
                sound: o.i32(opt::SOUND).unwrap_or(-1),
                position,
                velocity,
                start_distance: o.f32_or(opt::SOUND_START, 0.0),
                full_distance: o.f32_or(opt::SOUND_FULL, 0.0),
                volume: o.f32_or(opt::SOUND_VOLUME, 1.0),
                pitch: o.f32_or(opt::SOUND_PITCH, 0.0),
                doppler: o.f32_or(opt::SOUND_DOPPLER, 0.0),
                active: world.is_active(h),
            });
        }
        self.one_shots.clear();
        for s in core::mem::take(&mut world.sounds) {
            let position = s.emitter.and_then(|e| world.position(e));
            self.one_shots.push(OneShot { sound: s.sound, position, object: s.emitter });
        }
    }
}
