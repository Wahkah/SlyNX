//! Animation: curve evaluation, playback, a transform rig, skinning and poses (specs/sly1/animation.md).
//!
//! | Module | What |
//! |---|---|
//! | [`curve`] | scalar and vector curve evaluation (values and rates), the Euler rotation |
//! | [`playback`] | time, speed, repeat modes, events, applying channels, root motion, priorities, blends |
//! | [`rig`] | a hierarchy of local and world transforms, twists and pose weights that animations write into |
//! | [`skin`] | bind matrices, per-frame bone matrices, per-vertex influences and pose deltas |
//! | [`xform`] | Sly 2 transform-list rigs: channels by transform kind, mirror pairs, two-bone chains |
//! | [`sly2`] | Sly 2 clip playback (caller-chosen wrap, events, type-5 jumps) and skinning by byte weights |
//!
//! Parsing of segments is in `formats::anim`.

pub mod curve;
pub mod playback;
pub mod rig;
pub mod skin;
pub mod sly2;
pub mod sly2_machine; // Sly 2 hook: state machines (class 111), run by game::sly2_anim.
pub mod sly2_pose;
pub mod xform; // Sly 2 clip playback and skinning (added with the Sly 2 world loader).

pub use curve::{euler_zyx, eval_scalar, eval_vector, sample_scalar, sample_vector};
pub use playback::{AnimTarget, Animator, Binding, Playback, apply_segment, root_motion_velocity};
pub use rig::{LocalTransform, Rig, RigNode};
pub use skin::{BoneMatrix, Skin, SkinVertex, SkinnedSubmesh};
