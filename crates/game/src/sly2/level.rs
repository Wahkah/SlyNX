//! A Sly 2 or Sly 3 world on the renderer, through the same path as the viewer (`stage`): materials by shader and
//! list, lit kinds with their two colours per vertex, the world's fog, and static geometry merged into placed meshes.
//!
//! The engine registers no drawables for these worlds yet, so the scene's draws are placed once at their loaded world
//! matrices. The Sly 2 characters' skinned meshes are not in the scene build: they are added here
//! ([`super::skin`]) and their scene draws left out (`SceneInput::skip`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use formats::scene::Scene;
use render::{Backend, InstanceId, Material, MaterialId, Pass, RenderError, Renderer};
use stage::prep::{Prepared, SceneInput};

use crate::level::{ALPHA_CUTOFF, LevelStats, has_alpha};

/// The uploaded world.
#[derive(Default)]
pub struct StaticLevel {
    /// Renderer instances of the draws left as instances (static groups are drawn as placed meshes).
    pub instances: Vec<InstanceId>,
    /// Statistics.
    pub stats: LevelStats,
    /// Centre and radius of the drawn instances' positions (for placing the camera).
    pub bounds: ([f32; 3], f32),
    /// The characters' skinned instances.
    pub characters: super::skin::CharacterSkins,
    /// The world's clear colour (black when lit, rendering.md 6).
    pub clear: [f32; 4],
    /// Texture animations (rendering.md 4); play them with [`StaticLevel::animate`].
    pub anims: stage::texanim::Animator,
}

impl StaticLevel {
    /// Advances the world's texture animations by `dt` seconds (call once per rendered frame).
    pub fn animate<B: Backend>(&mut self, r: &mut Renderer<B>, dt: f32) -> Result<(), RenderError> {
        self.anims.animate(r, dt)
    }
}

/// Prepares `input` (`stage::prep::prepare`) and replaces the renderer's level with it: the world's clear colour is
/// the caller's (black when lit), its fog is set here. `world` and `skinned` add the Sly 2 characters' skinned draws
/// (give `input.skip` their records); `extra` adds other resources before the commit.
pub fn upload_stage<B: Backend>(
    r: &mut Renderer<B>,
    input: SceneInput<'_>,
    world: Option<&engine::world::World>,
    skinned: &[super::skin::SkinnedDraw],
    extra: impl FnOnce(&mut Renderer<B>) -> Result<(), RenderError>,
) -> Result<StaticLevel, String> {
    let prepared = stage::prep::prepare(input)?;
    upload_prepared(r, &prepared, input.scene, world, skinned, extra).map_err(|e| format!("{e}"))
}

/// [`upload_stage`] with the scene already prepared.
pub fn upload_prepared<B: Backend>(
    r: &mut Renderer<B>,
    prepared: &Prepared,
    scene: &Scene,
    world: Option<&engine::world::World>,
    skinned: &[super::skin::SkinnedDraw],
    extra: impl FnOnce(&mut Renderer<B>) -> Result<(), RenderError>,
) -> Result<StaticLevel, RenderError> {
    r.clear_level();
    let mut out = StaticLevel::default();
    let geo = prepared.geometry.as_ref();
    let up = stage::geometry::upload(r, geo)?;
    // The scene's textures come first in the prepared list, in order, then a white one.
    let white = *up.textures.last().ok_or(RenderError::InvalidHandle)?;
    // The characters' skinned meshes, and an opaque or cut-out material for each of their scene materials.
    let skin_meshes = super::skin::CharacterSkins::add_meshes(r, skinned)?;
    let mut skin_materials: BTreeMap<u32, MaterialId> = BTreeMap::new();
    for d in skinned {
        if skin_materials.contains_key(&d.material) {
            continue;
        }
        let Some(sh) = scene.materials.get(d.material as usize) else { continue };
        let t = sh.texture.map(|t| t as usize);
        let a = t.and_then(|t| scene.textures.get(t)).is_some_and(|t| has_alpha(&t.image.pixels));
        let texture = t.and_then(|t| up.textures.get(t).copied()).unwrap_or(white);
        let pass = if a { Pass::AlphaTest } else { Pass::Opaque };
        let m = r.add_material(Material {
            tint: sh.color,
            alpha_cutoff: ALPHA_CUTOFF,
            lighting: render::types::Lighting::Unlit,
            ..Material::opaque(texture).with_pass(pass)
        })?;
        skin_materials.insert(d.material, m);
    }
    extra(r)?;
    r.commit()?;
    if let Some(w) = world {
        out.characters =
            super::skin::CharacterSkins::place(r, w, skinned, &skin_meshes, |m| skin_materials.get(&m).copied())?;
    }
    out.instances = stage::geometry::place(r, geo, &up)?;
    out.anims = stage::texanim::Animator::new(&prepared.anims, &up.materials, &up.textures);
    out.clear = prepared.clear;
    r.set_fog(prepared.fog);
    r.set_fog_rising(true);
    let s = prepared.stats;
    out.stats.meshes = s.meshes;
    out.stats.materials = up.materials.len() as u32;
    out.stats.textures = s.textures;
    out.stats.skipped = s.skipped_draws;
    out.stats.instances = out.instances.len() as u32 + up.static_draws.len() as u32;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for c in &prepared.centres {
        for (k, v) in [c.x, c.y, c.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    if !prepared.centres.is_empty() {
        let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
        let r = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.5).max(1000.0);
        out.bounds = (c, r);
    }
    Ok(out)
}
