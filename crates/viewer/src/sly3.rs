//! Sly 3 worlds in the viewer: the scene `formats-sly3` builds (with the shadow hulls, projected volumes and kind 5's
//! pass added), prepared by `stage::prep` as Sly 2's are (Sly 3's passes are Sly 2's minus one, its shader flags keep
//! Sly 2's meaning) and wrapped as a [`PreparedLevel`]: lit with two colours per vertex (the colour tables in effect
//! where strip lists ask for them, specs/sly3/rendering.md 2), fogged by option set 0, cleared to option 1024's
//! colour. Frame 0, graded palettes.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use formats::iso::ReadAt;
use formats_sly3::scene::SceneBuild;

use crate::level::PreparedLevel;
use crate::sly2_light::{Sly2Lighting, WorldOptions};

/// A Sly 3 disc.
pub type Sly3Disc<R> = formats_sly3::disc::Disc<R>;

pub use stage::prep::{SLY3_ALPHA_CUTOFF, classify_sly3};

/// The label of world slot `slot`.
pub fn sly3_label(slot: usize, name: &str) -> String {
    format!("{name} (slot {slot})")
}

/// The worlds the viewer cycles through: the 39 slots that hold a world, as `v_apt (slot 5)`.
pub fn world_names<R: ReadAt>(disc: &Sly3Disc<R>) -> Vec<String> {
    disc.world_slots().map(|s| sly3_label(s, &disc.worlds[s].name)).collect()
}

/// Finds a world: a list index, a slot (`slot5`), or part of a name.
pub fn world_index<R: ReadAt>(disc: &Sly3Disc<R>, which: &str) -> Option<usize> {
    let which = which.trim();
    if let Some(s) = which.strip_prefix("slot") {
        let slot: usize = s.trim().parse().ok()?;
        return disc.world_slots().position(|x| x == slot);
    }
    crate::find_by_name(&world_names(disc), which)
}

/// Reads, parses and prepares the Sly 3 world at place `index` of the slots that hold a world.
pub fn load_world_sly3<R: ReadAt>(disc: &mut Sly3Disc<R>, index: usize) -> Result<PreparedLevel, String> {
    let slot = disc.world_slots().nth(index).ok_or_else(|| format!("no world {index}"))?;
    let label = sly3_label(slot, &disc.worlds[slot].name);
    let bytes = disc.read_world(slot).map_err(|e| format!("{label}: {e}"))?;
    let world = formats_sly3::world::World::parse_with(&bytes, &disc.options, disc.tangent_multipliers)
        .map_err(|e| format!("{label}: {e}"))?;
    let opts = formats_sly3::scene::SceneOptions {
        context: world.fresh_context(Some(slot as i16)),
        lights: formats_sly3::scene::light_classes(&disc.classes),
        // The default passes plus the shadow hulls (6), projected volumes (10, 14) and kind 5's pass 15.
        passes: formats_sly3::scene::SceneOptions::default().passes | (1 << 6) | (1 << 10) | (1 << 14) | (1 << 15),
        ..Default::default()
    };
    let build = formats_sly3::scene::build(&world, opts);
    // The option set of the world's first condition holds the fog and the shadow colour (rendering.md 6).
    let sets: Vec<_> = world.option_sets.sets.first().into_iter().collect();
    let lighting = Sly2Lighting::with(&build.lights, &sets, world.data, &WorldOptions::SLY3);
    prepare_sly3(index, &label, &build, Some(&lighting))
}

/// Converts a Sly 3 scene for the renderer; `index` is the world's place in the viewer's list. With `lighting`,
/// lit kinds get their two colours.
pub fn prepare_sly3(
    index: usize,
    name: &str,
    build: &SceneBuild,
    lighting: Option<&Sly2Lighting>,
) -> Result<PreparedLevel, String> {
    let input = stage::prep::SceneInput {
        merge: crate::sly2::merge_wanted(),
        ..stage::prep::SceneInput::sly3(build, lighting)
    };
    Ok(crate::sly2::wrap(index, name, stage::prep::prepare(input)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::{BlendMode, Pass};

    #[test]
    fn passes() {
        let c = |k, p, f| classify_sly3(k, p, f).map(|(l, cut)| (l.pass, l.blend, cut));
        assert_eq!(c(1, 2, 0), Some((Pass::AlphaTest, BlendMode::None, SLY3_ALPHA_CUTOFF)));
        assert_eq!(c(1, 9, 0), Some((Pass::Blend, BlendMode::Alpha, 0.0)));
        assert_eq!(c(1, 9, 1), Some((Pass::Blend, BlendMode::Additive, 0.0)));
        assert_eq!(c(4, 0, 0).map(|x| x.0), Some(Pass::Background));
        // Shadow hulls (kind 2, passes 6 and 7) and projected volumes (kind 3, passes 10 and 14): stencil volumes.
        assert_eq!(c(2, 6, 0).map(|x| x.0), Some(Pass::Volume));
        assert_eq!(c(3, 14, 0).map(|x| x.0), Some(Pass::Volume));
        assert_eq!(c(5, 15, 0).map(|x| x.0), Some(Pass::Post));
        assert_eq!(c(1, 255, 0), None);
    }
}
