//! A Sly 2 or Sly 3 world on the renderer, through the same path as the viewer (`stage`): materials by shader and
//! list, lit kinds with their two colours per vertex, the world's fog, and static geometry merged into placed meshes.
//!
//! The engine registers no drawables for these worlds yet, so the scene's draws are placed once at their loaded world
//! matrices. The characters' skinned meshes (Sly 2's and the Sly 3 crew) are added here ([`super::skin`]): only what is
//! drawn as them, with their shaders' looks, lit as moving objects (in the vertex shader, every frame); their scene draws are left
//! out (`SceneInput::skip`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use render::{Backend, InstanceId, MaterialId, RenderError, Renderer};
use stage::prep::{Prepared, SceneInput};

use crate::level::LevelStats;

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
    /// Draws shown by the camera's visibility mask ([`StaticLevel::show_zones`]).
    pub zones: stage::geometry::ZoneVisibility,
    /// The objects attached under the characters, moved and shown every frame (`attached`).
    pub attached: super::attached::AttachedParts,
}

impl StaticLevel {
    /// Advances the world's texture animations by `dt` seconds (call once per rendered frame).
    pub fn animate<B: Backend>(&mut self, r: &mut Renderer<B>, dt: f32) -> Result<(), RenderError> {
        self.anims.animate(r, dt)
    }

    /// Shows only the draws whose zone mask shares a bit with the camera's visibility mask (Sly 2 camera.md 0;
    /// `World::shared.visibility_mask`, 0xFF outside every volume). Call once per frame before rendering; costs
    /// nothing while the mask is unchanged.
    pub fn show_zones<B: Backend>(&mut self, r: &mut Renderer<B>, mask: u32) -> Result<(), RenderError> {
        self.zones.apply(r, mask)
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
    let result = stage::prep::prepare(input)
        .and_then(|p| upload_prepared(r, &p, &input, world, skinned, extra).map_err(|e| format!("{e}")));
    if result.is_err() {
        // Leave an empty, committed level behind, so the next frames still render (nothing) instead of failing.
        empty_level(r);
    }
    result
}

/// How a character's draw is drawn (`None`: left out): its shader's look in its pass, as the level's draws are;
/// shadow hulls and projected volumes (kinds 2 and 3) are not part of the character.
fn character_look(input: &SceneInput<'_>, d: &super::skin::SkinnedDraw) -> Option<(stage::geometry::DrawLook, f32)> {
    let sh = input.scene.materials.get(d.material as usize)?;
    if matches!(sh.kind, 2 | 3) {
        return None;
    }
    match input.game {
        stage::prep::Game::Sly2 => stage::prep::classify_sly2(sh.kind, d.pass, sh.blend),
        stage::prep::Game::Sly3 => stage::prep::classify_sly3(sh.kind, d.pass.checked_sub(1)?, sh.blend),
    }
}

/// Replaces the renderer's level with an empty, committed one.
pub fn empty_level<B: Backend>(r: &mut Renderer<B>) {
    r.clear_level();
    // An empty level always commits.
    let _ = r.commit();
}

/// [`upload_stage`] with the scene already prepared.
pub fn upload_prepared<B: Backend>(
    r: &mut Renderer<B>,
    prepared: &Prepared,
    input: &SceneInput<'_>,
    world: Option<&engine::world::World>,
    skinned: &[super::skin::SkinnedDraw],
    extra: impl FnOnce(&mut Renderer<B>) -> Result<(), RenderError>,
) -> Result<StaticLevel, RenderError> {
    r.clear_level();
    let mut out = StaticLevel::default();
    let geo = prepared.geometry.as_ref();
    let up = stage::geometry::upload(r, geo)?;
    if up.textures.is_empty() {
        // The prepared list always ends with a white texture.
        return Err(RenderError::InvalidHandle);
    }
    // The characters: only what is drawn as them (not hulls or volumes), each with the material its shader and
    // pass call for, lit as a moving object as it moves.
    let scene = input.scene;
    let kept: Vec<super::skin::SkinnedDraw> =
        skinned.iter().filter(|d| character_look(input, d).is_some()).cloned().collect();
    let skin_meshes = super::skin::CharacterSkins::add_meshes(r, &kept)?;
    let mut looks: BTreeMap<(u32, u8), MaterialId> = BTreeMap::new();
    let mut skin_materials: Vec<Option<MaterialId>> = Vec::with_capacity(kept.len());
    let mut skin_lights: Vec<Option<super::skin::PartLight>> = Vec::with_capacity(kept.len());
    for d in &kept {
        let (Some(sh), Some((look, cutoff))) = (scene.materials.get(d.material as usize), character_look(input, d))
        else {
            skin_materials.push(None);
            skin_lights.push(None);
            continue;
        };
        let lit = input.lighting.is_some();
        let m = match looks.get(&(d.material, look.order)) {
            Some(&m) => m,
            None => {
                // The scene's textures come first in the prepared list, then a white one.
                let texture = sh.texture.unwrap_or(up.textures.len() as u32 - 1);
                let grey = input.slot1.get(d.material as usize).copied().flatten().unwrap_or(texture);
                let desc = stage::geometry::MaterialDesc {
                    texture,
                    palettes: lit.then_some([grey, grey]),
                    look,
                    tint: if lit { [255; 4] } else { sh.color },
                    cutoff: (cutoff > 0.0).then_some(cutoff),
                    two_colour: lit,
                };
                let m = match r.add_material(stage::geometry::render_material(&desc, &up.textures, lit)) {
                    Ok(m) => m,
                    // The grey image in another pool: the colour image stands in (as for the level).
                    Err(_) => {
                        let desc = stage::geometry::MaterialDesc { palettes: lit.then_some([texture; 2]), ..desc };
                        r.add_material(stage::geometry::render_material(&desc, &up.textures, lit))?
                    }
                };
                looks.insert((d.material, look.order), m);
                m
            }
        };
        skin_materials.push(Some(m));
        // Two colours per vertex as a moving object (rendering.md 2.3), computed in the vertex shader every
        // frame from the lights at the character's place (`skin::PartLight`).
        let illum = input.self_illum.get(d.material as usize).copied().unwrap_or(0);
        skin_lights.push(lit.then_some(super::skin::PartLight {
            color: sh.color,
            illum,
            one_pass: !matches!(sh.kind, 1 | 7),
        }));
    }
    extra(r)?;
    r.commit()?;
    if let Some(w) = world {
        out.characters = super::skin::CharacterSkins::place(
            r,
            w,
            &kept,
            &skin_meshes,
            |i| skin_materials.get(i).copied().flatten(),
            input.lighting,
            &skin_lights,
        )?;
    }
    let (instances, statics) = stage::geometry::place_all(r, geo, &up)?;
    out.zones = stage::geometry::ZoneVisibility::new(geo, &up, &instances, &statics);
    if let Some(w) = world {
        let mut draws = Vec::with_capacity(prepared.dynamic.len());
        for d in &prepared.dynamic {
            let k = d.draw as usize;
            let (Some(&id), Some(g)) = (instances.get(k), geo.draws.get(k)) else { continue };
            let Some(mesh) = up.meshes.get(g.mesh as usize).copied().flatten() else { continue };
            draws.push(super::attached::PlacedDraw {
                id,
                mesh,
                material: up.materials[g.material as usize],
                weights: up.weight_bases.get(k).copied().flatten(),
                transform: g.transform,
                record: d.record,
                volume: d.volume,
                radius: d.radius,
            });
        }
        out.attached = super::attached::AttachedParts::new(r, w, &draws)?;
    }
    out.instances = instances;
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

#[cfg(test)]
mod tests {
    use super::*;
    use formats::scene::Scene;
    use render::NullBackend;
    use render::math::Vec3;

    #[test]
    fn a_failed_upload_leaves_an_empty_level_that_renders() {
        let mut r = Renderer::new(NullBackend::new(64, 64));
        let scene = Scene::default();
        let input = SceneInput {
            game: stage::prep::Game::Sly2,
            scene: &scene,
            slot1: &[],
            self_illum: &[],
            lighting: None,
            merge: true,
            skip: &[],
            dynamic: &[],
            baked: None,
            mesh_flags: &[],
            submesh_flags: &[],
            anims: &[],
            color2: &[],
            zones: &[],
        };
        // The extra resources fail: the upload reports it, and the renderer still draws a frame.
        assert!(upload_stage(&mut r, input, None, &[], |_| Err(RenderError::InvalidHandle)).is_err());
        let camera = render::Camera {
            eye: Vec3::new(0.0, -10.0, 0.0),
            target: Vec3::ZERO,
            up: Vec3::Z,
            fov_y: 1.0,
            near: 1.0,
            far: 100.0,
        };
        assert!(r.render(&camera, [0.0; 4]).is_ok());
    }
}
