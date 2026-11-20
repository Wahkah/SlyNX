//! The camera (specs/sly1/camera.md): the follow camera with zone and default framing, springs and smoothing,
//! orbit, automatic follow, recentre and path lock; view clearing and camera-fade groups; look-around, cutscene
//! placements, aligned and manual views; dialogue picture panels and the letterbox; lens and fog.
//!
//! | Module | What |
//! |---|---|
//! | [`consts`] | the spec's values (read from the executable where it gives an address) and the port's tunables |
//! | [`framing`] | frames, offset functions, path and cylindrical framing |
//! | [`zone`] | camera zones (class 99) read from the level, zone selection |
//! | [`fade`] | camera-fade groups through the collision tree |
//! | [`follow`] | [`FollowCamera`]: the camera itself, independent of the object model |
//! | [`system`] | [`CameraSystem`]: the follow camera behind the game's camera seam |

pub mod consts;
pub mod fade;
pub mod follow;
pub mod framing;
pub mod sly2;
pub mod system;
pub mod views;
pub mod zone;

#[cfg(test)]
mod tests;

pub use consts::{CameraConstants, CameraTunables};
pub use fade::{FadeSource, FadedMesh};
pub use follow::{CameraFrame, CameraInput, CameraScene, FollowCamera, Lens, Mode, PanelSubject, PanelView, Subject};
pub use framing::{Frame, Framing, OffsetFn, ZoneFrame};
pub use system::{CameraSystem, Mount};
pub use zone::CameraZone;
