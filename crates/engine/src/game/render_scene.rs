//! What the renderer needs from the engine each frame: the camera, the clear colour and fog from the world options,
//! the drawables whose transform or visibility changed, and skinning matrices. Everything is in game space
//! (right-handed, Z up); the renderer applies the Y-up conversion in its view matrix (transforms.md 7.1).

use alloc::vec::Vec;

use formats::geometry::Affine;

use crate::object::{Handle, OptionStore, OptionValue};

/// The camera's view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    /// Eye position.
    pub eye: [f32; 3],
    /// A point looked at.
    pub target: [f32; 3],
    /// Up direction (+Z).
    pub up: [f32; 3],
    /// Field of view in radians (taken as vertical; see `docs/engine.md`).
    pub fov_y: f32,
    /// Near clip distance.
    pub near: f32,
    /// Far clip distance.
    pub far: f32,
}

impl Default for CameraView {
    fn default() -> Self {
        CameraView {
            eye: [0.0, -1000.0, 500.0],
            target: [0.0; 3],
            up: [0.0, 0.0, 1.0],
            fov_y: DEFAULT_FOV,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
        }
    }
}

/// Lens defaults when the world sets none (camera.md 1).
pub const DEFAULT_FOV: f32 = 1.0;
/// Near clip default.
pub const DEFAULT_NEAR: f32 = 100.0;
/// Far clip default.
pub const DEFAULT_FAR: f32 = 20000.0;
/// Fog defaults: start, end, maximum.
pub const DEFAULT_FOG: [f32; 3] = [500.0, 20000.0, 0.5];

/// World options of the lens, sky and fog (object-stream.md 4.4).
pub mod world_opt {
    /// Sky / clear colour.
    pub const SKY: u16 = 528;
    /// Field of view.
    pub const FOV: u16 = 529;
    /// Near clip.
    pub const NEAR: u16 = 530;
    /// Far clip.
    pub const FAR: u16 = 531;
    /// Fog start.
    pub const FOG_START: u16 = 532;
    /// Fog end.
    pub const FOG_END: u16 = 533;
    /// Fog maximum.
    pub const FOG_MAX: u16 = 534;
    /// Fog colour.
    pub const FOG_COLOR: u16 = 535;
}

/// Distance fog.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    /// Start distance.
    pub start: f32,
    /// End distance.
    pub end: f32,
    /// Maximum amount, 0 to 1.
    pub max: f32,
    /// Colour, RGB 0..1.
    pub color: [f32; 3],
}

/// Skinning matrices for skinned drawables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinFrame {
    /// (drawable index, range of `matrices`) per skinned drawable.
    pub skins: Vec<(u32, core::ops::Range<u32>)>,
    /// Bone matrices, mesh space to mesh space (animation.md 8.2).
    pub matrices: Vec<Affine>,
}

impl SkinFrame {
    /// Empties the frame.
    pub fn clear(&mut self) {
        self.skins.clear();
        self.matrices.clear();
    }
}

/// The renderer's input for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderScene {
    /// The camera.
    pub camera: CameraView,
    /// Clear colour, RGBA 0..1.
    pub clear: [f32; 4],
    /// Fog, if the world sets it.
    pub fog: Option<Fog>,
    /// Drawables (indices into `World::drawables`) whose transform or visibility changed this frame.
    pub changed: Vec<u32>,
    /// Game time for texture animations (shader animation records).
    pub texture_time: f32,
    /// Skinning matrices.
    pub skins: SkinFrame,
    /// The object the camera follows, if any.
    pub focus: Option<Handle>,
    /// Faded meshes and picture panels from the camera.
    pub camera_extras: super::systems::CameraExtras,
}

fn color(v: Option<&OptionValue>) -> Option<[f32; 3]> {
    match v {
        Some(OptionValue::Color(c)) => Some([c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]),
        _ => None,
    }
}

impl RenderScene {
    /// Sets the clear colour and fog from the world options.
    pub fn configure(&mut self, world_options: &OptionStore) {
        let sky = color(world_options.get(world_opt::SKY)).unwrap_or([0.0; 3]);
        self.clear = [sky[0], sky[1], sky[2], 1.0];
        self.fog = color(world_options.get(world_opt::FOG_COLOR)).map(|c| Fog {
            start: world_options.f32_or(world_opt::FOG_START, DEFAULT_FOG[0]),
            end: world_options.f32_or(world_opt::FOG_END, DEFAULT_FOG[1]),
            max: world_options.f32_or(world_opt::FOG_MAX, DEFAULT_FOG[2]),
            color: c,
        });
    }
}

/// The lens from the world options, or the defaults (camera.md 1).
pub fn lens(world_options: &OptionStore) -> (f32, f32, f32) {
    (
        world_options.f32_or(world_opt::FOV, DEFAULT_FOV),
        world_options.f32_or(world_opt::NEAR, DEFAULT_NEAR),
        world_options.f32_or(world_opt::FAR, DEFAULT_FAR),
    )
}
