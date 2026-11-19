//! Sly 2 worlds in the viewer: the scene `formats-sly2` builds, prepared by `stage::prep` (drawing state, lighting,
//! merged static geometry) and wrapped as a [`PreparedLevel`].

use alloc::string::String;

use formats_sly2::scene::SceneBuild;

use crate::level::{LevelStats, PreparedLevel, spawn_point, trimmed_bounds};
use crate::look::WorldLook;
use crate::sly2_light::WorldLighting;

pub use stage::prep::{
    MAX_MERGED_VERTICES, ORDER_MARGIN, SLY2_ALPHA_TEST, STATIC_CELL, classify_sly2, flag_blend, light_vertices,
    vertex_colour,
};

/// Converts a Sly 2 scene for the renderer; `index` is the world's place in the viewer's list. With `lighting`,
/// lit kinds get their two colours.
pub fn prepare_sly2(
    index: usize,
    name: &str,
    build: &SceneBuild,
    lighting: Option<&WorldLighting>,
) -> Result<PreparedLevel, String> {
    prepare_sly2_with(index, name, build, lighting, merge_wanted())
}

/// Whether to merge static geometry: on the PC, `SLYNX_VIEWER_NO_MERGE` keeps every draw an instance (for comparing
/// captures).
pub fn merge_wanted() -> bool {
    #[cfg(not(target_os = "horizon"))]
    return std::env::var_os("SLYNX_VIEWER_NO_MERGE").is_none();
    #[cfg(target_os = "horizon")]
    return true;
}

/// [`prepare_sly2`], merging static geometry or not.
pub fn prepare_sly2_with(
    index: usize,
    name: &str,
    build: &SceneBuild,
    lighting: Option<&WorldLighting>,
    merge: bool,
) -> Result<PreparedLevel, String> {
    let input = stage::prep::SceneInput { merge, ..stage::prep::SceneInput::sly2(build, lighting) };
    Ok(wrap(index, name, stage::prep::prepare(input)?))
}

/// A prepared Sly 2 or Sly 3 scene as the viewer's level.
pub(crate) fn wrap(index: usize, name: &str, p: stage::prep::Prepared) -> PreparedLevel {
    let s = p.stats;
    let bounds = trimmed_bounds(&p.centres);
    let g = p.geometry;
    PreparedLevel {
        world: index,
        name: String::from(name),
        vertices: g.vertices,
        skin: g.skin,
        indices: g.indices,
        meshes: g.meshes,
        textures: g.textures,
        materials: g.materials,
        draws: g.draws,
        weights: g.weights,
        look: WorldLook { clear: p.clear, fog: p.fog, shadow_level: 0.0, mid_level: 0.0, fog_rising: true },
        skies: alloc::vec::Vec::new(),
        billboards: alloc::vec::Vec::new(),
        animations: p.anims,
        spawn: spawn_point(bounds, None),
        bounds,
        stats: LevelStats {
            scene_instances: s.scene_instances,
            draws: s.draws,
            skipped_draws: s.skipped_draws,
            triangles: s.triangles,
            meshes: s.meshes,
            vertices: s.vertices,
            textures: s.textures,
            opaque: s.opaque,
            blended: s.blended,
            backdrops: s.backdrops,
            post: s.post,
            weights: s.weights,
            static_pieces: s.static_pieces,
            static_groups: s.static_groups,
            ..Default::default()
        },
        player: None,
        fades: alloc::vec::Vec::new(),
        outline_meshes: alloc::vec::Vec::new(),
        outlines: alloc::vec::Vec::new(),
        hud: None,
        extras: crate::level::LevelExtras { static_groups: g.static_groups, ..Default::default() },
    }
}
