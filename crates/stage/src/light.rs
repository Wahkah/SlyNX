//! Sly 2 and Sly 3 lighting, fog and clear colour (specs/sly2/rendering.md 2 and 6, effects.md 2): the two colours
//! A and B of every vertex, from the world's lights and shadow colour, for `render::Lighting::TwoColour`.
//!
//! Which vertices are baked (cap 1), lit as moving objects (cap 2, four strongest lights, twice the scale) or take
//! the unlit colour is decided per submesh in [`crate::prep`] (rendering.md 2.3). The viewer and the game keep
//! objects where they are placed, so the per-frame path is computed once at load.

use alloc::vec::Vec;

use formats_sly2::options::OptionList;
use formats_sly2::scene::SceneLight;
use render::math::{Vec3, abs, cos, floor};
use render::{Fog, VertexWeights};

/// One light record (rendering.md 2.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightRecord {
    /// Point light: its position; directional: the unit direction it shines along.
    pub at: Vec3,
    pub point: bool,
    /// `(RGB(HSV), S) / g`.
    pub color: [f32; 4],
    /// `g / 255`, the angular factor facing the light.
    pub peak: f32,
    /// Angular offset `o` and scale `s`: factor `clamp(o + (d + d^3) s, 0, 1)`.
    pub offset: f32,
    pub scale: f32,
    /// Point light: fall-off distances (full, none).
    pub range: [f32; 2],
    pub not_moving: bool,
    pub not_baked: bool,
}

/// HSV (hue 0..360, saturation and value 0..255) to RGB (0..255).
pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let h = (hsv[0] % 360.0 + 360.0) % 360.0 / 60.0;
    let s = (hsv[1] / 255.0).clamp(0.0, 1.0);
    let v = hsv[2].max(0.0);
    let i = h as u32 % 6;
    let f = h - (h as u32) as f32;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

impl LightRecord {
    /// The record of a light (rendering.md 2.1); `None` when its `g` is 0.
    pub fn new(l: &SceneLight) -> Option<LightRecord> {
        let rgb = hsv_to_rgb(l.hsv);
        let g = l.hsv[2].max(l.strength);
        if g <= 0.0 {
            return None;
        }
        let theta = l.angle.max(30.0).to_radians();
        let c = cos(theta * 0.5);
        let x0 = c + c * c * c;
        let peak = g / 255.0;
        let k = peak / (2.0 - x0).max(1e-6);
        let point = l.kind == 1;
        let at = if point {
            Vec3::new(l.matrix.t[0], l.matrix.t[1], l.matrix.t[2])
        } else {
            let a = l.matrix.cols[0];
            let v = Vec3::new(a[0], a[1], a[2]);
            if v.dot(v) > 1e-12 { v.normalize() } else { Vec3::new(0.0, 0.0, -1.0) }
        };
        Some(LightRecord {
            at,
            point,
            color: [rgb[0] / g, rgb[1] / g, rgb[2] / g, l.strength / g],
            peak,
            offset: -k * x0,
            scale: -k,
            range: l.range,
            not_moving: l.not_moving,
            not_baked: l.not_baked,
        })
    }

    /// The default light of a world with no bakeable light: direction normalise(-0.2, 0.3, -1), strength 255,
    /// angle 240, colour black.
    pub fn default_light() -> LightRecord {
        let l = SceneLight {
            matrix: formats::geometry::Affine {
                cols: [[-0.2, 0.3, -1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                t: [0.0; 3],
            },
            kind: 0,
            hsv: [0.0, 0.0, 0.0],
            strength: 255.0,
            angle: 240.0,
            range: [200.0, 2000.0],
            not_moving: false,
            not_baked: false,
        };
        LightRecord::new(&l).expect("strength 255")
    }

    /// Distance factor at `p`: 1 for a directional light; a point light's line through (near, 1) and (far, 0).
    pub fn distance(&self, p: Vec3) -> f32 {
        if !self.point {
            return 1.0;
        }
        let d = (p - self.at).length();
        let [near, far] = self.range;
        if abs(far - near) < 1e-6 {
            return if d <= near { 1.0 } else { 0.0 };
        }
        (1.0 - (d - near) / (far - near)).clamp(0.0, 1.0)
    }

    /// Angular factor for a surface at `p` with unit normal `n` (the bake's: clamped to 0..1).
    pub fn angular(&self, p: Vec3, n: Vec3) -> f32 {
        self.angular_line(p, n).clamp(0.0, 1.0)
    }

    /// The angular line `o + (d + d^3) s` at `p` with normal `n`, unclamped.
    pub fn angular_line(&self, p: Vec3, n: Vec3) -> f32 {
        let l = if self.point {
            let v = p - self.at;
            if v.dot(v) < 1e-12 {
                return self.offset + 2.0 * -self.scale;
            }
            v.normalize()
        } else {
            self.at
        };
        let d = n.dot(l);
        self.offset + (d + d * d * d) * self.scale
    }
}

/// A world's lights, shadow colour and fog.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldLighting {
    pub lights: Vec<LightRecord>,
    /// World shadow colour `W` (0..1; alpha the amount), options 914 to 916.
    pub shadow: [f32; 4],
    /// Options 166 to 169 (camera.md 1's defaults when missing).
    pub fog: Fog,
    /// Clear colour (RGBA 0..1): Sly 3's option 1024 (rendering.md 6, a full-screen sprite at the start of the world
    /// draw), black otherwise.
    pub clear: [f32; 4],
}

fn option_f32(lists: &[&OptionList], data: &[u8], id: i16, k: usize) -> Option<f32> {
    lists
        .iter()
        .find_map(|l| l.value(data, id))
        .and_then(|v| v.get(k * 4..k * 4 + 4))
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn option_u32(lists: &[&OptionList], data: &[u8], id: i16) -> Option<u32> {
    lists.iter().find_map(|l| l.u32_value(data, id))
}

/// Where a game keeps the world shadow colour and fog, and their defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldOptions {
    /// Shadow colour HSV, amount (ambient), scale.
    pub shadow: [i16; 3],
    pub shadow_default: ([f32; 3], f32, f32),
    /// Fog start, end, maximum, packed colour, colour as HSV (if the game has one).
    pub fog: [i16; 4],
    pub fog_hsv: Option<i16>,
    /// The clear colour's option (a packed colour), if the game has one.
    pub clear: Option<i16>,
    pub fog_default: (f32, f32, f32, u32),
}

impl WorldOptions {
    /// Sly 2: world options 914 to 916 (effects.md 2.1) and 166 to 169 (camera.md 1).
    pub const SLY2: WorldOptions = WorldOptions {
        shadow: [914, 915, 916],
        shadow_default: ([180.0, 255.0, 255.0], 0.0, 90.0),
        fog: [166, 167, 169, 168],
        fog_hsv: None,
        clear: None,
        fog_default: (500.0, 20_000.0, 0.5, 0x8030_3030),
    };
    /// Sly 3: the option set's 1015 to 1017 and 1010 to 1014 (its rendering.md 6).
    pub const SLY3: WorldOptions = WorldOptions {
        shadow: [1015, 1016, 1017],
        shadow_default: ([210.0, 150.0, 100.0], 35.0, 60.0),
        fog: [1010, 1011, 1014, 1012],
        fog_hsv: Some(1013),
        clear: Some(1024),
        fog_default: (5_500.0, 60_000.0, 0.4, 0x8096_5515),
    };
}

impl WorldLighting {
    /// From the scene's lights and the world option lists (Sly 2).
    pub fn new(lights: &[SceneLight], options: &[OptionList], data: &[u8]) -> WorldLighting {
        let lists: Vec<&OptionList> = options.iter().collect();
        WorldLighting::with(lights, &lists, data, &WorldOptions::SLY2)
    }

    /// From the scene's lights and the option lists that hold `ids`.
    pub fn with(lights: &[SceneLight], options: &[&OptionList], data: &[u8], ids: &WorldOptions) -> WorldLighting {
        let f = |id, k| option_f32(options, data, id, k);
        let (dh, da, ds) = ids.shadow_default;
        let [h, a, sc] = ids.shadow;
        let hsv = [f(h, 0).unwrap_or(dh[0]), f(h, 1).unwrap_or(dh[1]), f(h, 2).unwrap_or(dh[2])];
        let scale = f(sc, 0).unwrap_or(ds) / 255.0;
        let rgb = hsv_to_rgb(hsv);
        let shadow =
            [rgb[0] / 255.0 * scale, rgb[1] / 255.0 * scale, rgb[2] / 255.0 * scale, f(a, 0).unwrap_or(da) / 255.0];
        let (s, e, m, c) = ids.fog_default;
        let [fs, fe, fm, fc] = ids.fog;
        let packed = option_u32(options, data, fc).map(|v| v.to_le_bytes());
        let from_hsv = ids.fog_hsv.and_then(|id| {
            let v = [f(id, 0)?, f(id, 1)?, f(id, 2)?];
            let rgb = hsv_to_rgb(v);
            Some([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 0x80])
        });
        let c = packed.or(from_hsv).unwrap_or(c.to_le_bytes());
        let fog = Fog {
            near: f(fs, 0).unwrap_or(s),
            far: f(fe, 0).unwrap_or(e),
            cap: f(fm, 0).unwrap_or(m),
            color: [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0],
        };
        // The sprite's colour over black, by its alpha on the 0x80 scale; no option: black (what the game does then
        // is open).
        let clear = ids.clear.and_then(|id| option_u32(options, data, id)).map_or([0.0, 0.0, 0.0, 1.0], |v| {
            let c = v.to_le_bytes();
            let a = (c[3] as f32 / 128.0).min(1.0);
            [c[0] as f32 / 255.0 * a, c[1] as f32 / 255.0 * a, c[2] as f32 / 255.0 * a, 1.0]
        });
        WorldLighting { lights: lights.iter().filter_map(LightRecord::new).collect(), shadow, fog, clear }
    }

    /// The lights a static mesh with bounding sphere (`centre`, `radius`) is baked with: every light with 158 off
    /// whose reach (point lights: the fall-off end) meets the sphere; the default light when the world has none.
    pub fn baked(&self, centre: Vec3, radius: f32) -> Vec<LightRecord> {
        let mut out: Vec<LightRecord> = Vec::new();
        let any = self.lights.iter().any(|l| !l.not_baked);
        if !any {
            out.push(LightRecord::default_light());
            return out;
        }
        for l in self.lights.iter().filter(|l| !l.not_baked) {
            if l.point && (centre - l.at).length() > l.range[1].max(l.range[0]) + radius {
                continue;
            }
            out.push(*l);
        }
        out
    }

    /// The four strongest lights at `centre` with 157 off (strength times the distance factor there), with their
    /// distance factors at the centre.
    pub fn strongest(&self, centre: Vec3) -> Vec<(LightRecord, f32)> {
        let mut c: Vec<(LightRecord, f32)> =
            self.lights.iter().filter(|l| !l.not_moving).map(|l| (*l, l.distance(centre))).collect();
        c.retain(|(_, d)| *d > 0.0);
        c.sort_by(|a, b| (b.0.peak * b.1).total_cmp(&(a.0.peak * a.1)));
        c.truncate(4);
        c
    }
}

/// How a vertex is lit: baked (cap 1) or as a moving object (cap 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightMode {
    Baked,
    Moving,
}

/// The two colours of a vertex (rendering.md 2.4) as `TwoColour` weights: `shadow` = A's alpha, `mid` = A,
/// `high` = B, `alpha` = B's alpha, bytes on the 0x80 = 1.0 scale.
///
/// `v` is the stored vertex colour and `m` the shader's material colour (0xFF = 1.0), `self_illum` the shader's
/// bytes 20 and 21, `lights` each light with its distance factor (`None`: at the vertex).
#[allow(clippy::too_many_arguments)]
pub fn two_colours(
    mode: LightMode,
    v: [u8; 4],
    m: [u8; 4],
    self_illum: u16,
    shadow: [f32; 4],
    lights: &[(LightRecord, Option<f32>)],
    p: Vec3,
    n: Vec3,
) -> VertexWeights {
    let (kappa, eps, sigma_a) = match mode {
        LightMode::Baked => (1.0f32, 1.0 / 127.0, 127.0f32),
        LightMode::Moving => (2.0, 2.0 / 127.0, 63.5),
    };
    // Base colour: R, G, B on the 0xFF scale, alpha on the 0x80 scale.
    let mut b = [0f32; 4];
    for k in 0..3 {
        let x = v[k] as u32 * m[k] as u32;
        b[k] = match mode {
            LightMode::Baked => x.div_ceil(256) as f32,
            LightMode::Moving => ((x + 255) / 512) as f32,
        };
    }
    b[3] = ((v[3] as u32 * m[3] as u32 + 255) / 510) as f32;
    // Accumulate.
    let mut l = [0.0, 0.0, 0.0, self_illum as f32 / 4096.0 + shadow[3]];
    for (light, dist) in lights {
        let f = match mode {
            LightMode::Baked => light.angular(p, n) * dist.unwrap_or_else(|| light.distance(p)),
            // Per frame, at twice the scale (2.4): 2 max(o d + s d x, 0) with the distance factor d at the object's
            // centre, floored at 0 but not clamped at 1.
            LightMode::Moving => {
                let delta = dist.unwrap_or_else(|| light.distance(p));
                2.0 * (light.angular_line(p, n) * delta).max(0.0)
            }
        };
        for k in 0..4 {
            l[k] += light.color[k] * f;
        }
    }
    // Split.
    let big = l[0].max(l[1]).max(l[2]);
    let r = (kappa - big).max(0.0);
    for c in l.iter_mut().take(3) {
        *c = c.clamp(0.0, kappa);
    }
    let mu = l[3].clamp(0.0, r);
    let sh = r - mu;
    let beta = (kappa + eps - sh) * sigma_a;
    let top = b[0].max(b[1]).max(b[2]);
    let i = match mode {
        LightMode::Baked => top,
        LightMode::Moving => floor(top / 255.0 * 127.0),
    };
    let byte = |x: f32| x.clamp(0.0, 255.0) as u8;
    let mut a = [0, 1, 2].map(|k| byte((l[k] + shadow[k] * sh) * i));
    let mut bb = [0, 1, 2].map(|k| byte(b[k] * mu * (b[3] / beta.max(1e-6))));
    if mode == LightMode::Baked {
        // The bake stores its colours with 5 bits per channel (2.3).
        for c in a.iter_mut().chain(bb.iter_mut()) {
            *c &= 0xF8;
        }
    }
    VertexWeights { shadow: byte(b[3]), mid: a, high: bb, alpha: byte(beta) }
}

/// Rows of a shader light block (`render::Renderer::set_shader_light`).
pub const LIGHT_BLOCK_ROWS: usize = 15;

/// The light block of a moving object at `centre` (rendering.md 2.3: the four strongest lights there, their
/// distance factors taken at the centre), for a submesh with material colour `color`, self-illumination `illum`, and
/// colour A alone when `one_pass`. The vertex shader computes [`two_colours`] (`LightMode::Moving`) from it per
/// vertex; [`block_two_colours`] is the same computation on the CPU.
pub fn light_block(
    l: &WorldLighting,
    centre: Vec3,
    color: [u8; 4],
    illum: u16,
    one_pass: bool,
) -> [[f32; 4]; LIGHT_BLOCK_ROWS] {
    let mut rows = [[0.0f32; 4]; LIGHT_BLOCK_ROWS];
    rows[0] = [l.shadow[0], l.shadow[1], l.shadow[2], illum as f32 / 4096.0 + l.shadow[3]];
    rows[1] = color.map(|c| c as f32);
    rows[2] = [if one_pass { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
    for (k, (r, d)) in l.strongest(centre).into_iter().enumerate().take(4) {
        let at = 3 + 3 * k;
        rows[at] = [r.at.x, r.at.y, r.at.z, if r.point { 1.0 } else { 0.0 }];
        rows[at + 1] = r.color;
        rows[at + 2] = [r.offset, r.scale, d, 1.0];
    }
    rows
}

/// The vertex shader's two colours from a light block (CPU reference for the shader, and for tests): vertex colour
/// `v`, world position `p`, unit world normal `n`.
pub fn block_two_colours(rows: &[[f32; 4]; LIGHT_BLOCK_ROWS], v: [u8; 4], p: Vec3, n: Vec3) -> VertexWeights {
    let mc = rows[1];
    let base = [0, 1, 2].map(|k| floor((v[k] as f32 * mc[k] + 255.0) / 512.0));
    let ba = floor((v[3] as f32 * mc[3] + 255.0) / 510.0);
    let mut l = [0.0, 0.0, 0.0, rows[0][3]];
    for k in 0..4 {
        let (pp, c, q) = (rows[3 + 3 * k], rows[4 + 3 * k], rows[5 + 3 * k]);
        if q[3] < 0.5 {
            continue;
        }
        let dir = if pp[3] > 0.5 {
            let d = p - Vec3::new(pp[0], pp[1], pp[2]);
            if d.dot(d) > 1e-12 { d.normalize() } else { Vec3::ZERO }
        } else {
            Vec3::new(pp[0], pp[1], pp[2])
        };
        let d = n.dot(dir);
        let f = 2.0 * ((q[0] + (d + d * d * d) * q[1]) * q[2]).max(0.0);
        for j in 0..4 {
            l[j] += c[j] * f;
        }
    }
    let big = l[0].max(l[1]).max(l[2]);
    let r = (2.0 - big).max(0.0);
    let mu = l[3].clamp(0.0, r);
    let sh = r - mu;
    let beta = (2.0 + 2.0 / 127.0 - sh) * 63.5;
    let i = floor(base[0].max(base[1]).max(base[2]) / 255.0 * 127.0);
    let byte = |x: f32| floor(x).clamp(0.0, 255.0) as u8;
    let a = [0, 1, 2].map(|k| byte((l[k].clamp(0.0, 2.0) + rows[0][k] * sh) * i));
    let b = if rows[2][0] > 0.5 { [0; 3] } else { [0, 1, 2].map(|k| byte(base[k] * mu * (ba / beta.max(1e-6)))) };
    VertexWeights { shadow: byte(ba), mid: a, high: b, alpha: byte(beta) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::geometry::Affine;

    fn light(hsv: [f32; 3], strength: f32, angle: f32) -> SceneLight {
        SceneLight {
            matrix: Affine { cols: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: [0.0; 3] },
            kind: 0,
            hsv,
            strength,
            angle,
            range: [200.0, 2000.0],
            not_moving: false,
            not_baked: true,
        }
    }

    #[test]
    fn light_record_matches_the_test_vector() {
        // Light 7430 (row 3): HSV (50, 150, 0), strength 175, angle 240.
        let r = LightRecord::new(&light([50.0, 150.0, 0.0], 175.0, 240.0)).unwrap();
        assert_eq!(r.color, [0.0, 0.0, 0.0, 1.0]);
        assert!((r.offset - 0.163399).abs() < 1e-5, "{}", r.offset);
        assert!((r.scale + 0.261438).abs() < 1e-5, "{}", r.scale);
        // A surface facing it (normal against the light's direction, +X) gets 175 / 255.
        let f = r.angular(Vec3::ZERO, Vec3::new(-1.0, 0.0, 0.0));
        assert!((f - 175.0 / 255.0).abs() < 1e-5);
        // Facing away: nothing.
        assert_eq!(r.angular(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn shadow_colour_matches_the_test_vector() {
        // Row 3: HSV (260, 180, 180), scale 110, amount 10.
        let rgb = hsv_to_rgb([260.0, 180.0, 180.0]);
        let s = 110.0 / 255.0;
        let w = [rgb[0] / 255.0 * s, rgb[1] / 255.0 * s, rgb[2] / 255.0 * s];
        let want = [0.161205, 0.089558, 0.304498];
        for k in 0..3 {
            assert!((w[k] - want[k]).abs() < 1e-5, "{w:?}");
        }
        assert!((10.0f32 / 255.0 - 0.039216).abs() < 1e-6);
    }

    #[test]
    fn unlit_and_fully_lit_vertices() {
        let white = [255u8; 4];
        // No light: all headroom is shadow, so A is the shadow colour times the brightness (5 bits per channel) and
        // B is nothing.
        let w = two_colours(LightMode::Baked, white, white, 0, [0.5, 0.25, 0.0, 0.0], &[], Vec3::ZERO, Vec3::Z);
        assert_eq!((w.mid, w.high), ([120, 56, 0], [0, 0, 0]));
        assert_eq!(w.shadow, 128, "alpha on the 0x80 scale");
        // A white light at full strength straight on: A at full brightness, no headroom left for B.
        let mut l = light([0.0, 0.0, 255.0], 255.0, 240.0);
        l.not_baked = false;
        let r = LightRecord::new(&l).unwrap();
        let w = two_colours(
            LightMode::Baked,
            white,
            white,
            0,
            [0.0; 4],
            &[(r, None)],
            Vec3::ZERO,
            Vec3::new(-1.0, 0.0, 0.0),
        );
        assert_eq!((w.mid, w.high), ([248, 248, 248], [0, 0, 0]));
        // Per frame, at twice the scale: the same light gives L = 2, still within the cap of 2, so A is the base
        // brightness on the 0..127 scale times 2.
        let w = two_colours(
            LightMode::Moving,
            white,
            white,
            0,
            [0.0; 4],
            &[(r, Some(1.0))],
            Vec3::ZERO,
            Vec3::new(-1.0, 0.0, 0.0),
        );
        // Base 255 * 255 per frame: (65025 + 255) / 512 = 127; I = 127 / 255 * 127 = 63; A = 2 * 63.
        assert_eq!(w.mid, [126, 126, 126]);
        // Half the light, and self-illumination filling the rest: half A, the base colour in B.
        let w = two_colours(LightMode::Baked, white, white, 4096, [0.0; 4], &[], Vec3::ZERO, Vec3::Z);
        assert_eq!(w.mid, [0, 0, 0]);
        assert_eq!(w.high, [248, 248, 248]);
        assert_eq!(w.alpha, 128);
    }

    #[test]
    fn cone_angles_below_30_degrees_count_as_30() {
        let narrow = LightRecord::new(&light([0.0, 0.0, 255.0], 255.0, 10.0)).unwrap();
        let thirty = LightRecord::new(&light([0.0, 0.0, 255.0], 255.0, 30.0)).unwrap();
        assert_eq!((narrow.offset, narrow.scale), (thirty.offset, thirty.scale));
    }

    #[test]
    fn point_lights_fade_with_distance() {
        let mut l = light([0.0, 0.0, 255.0], 255.0, 240.0);
        l.kind = 1;
        let r = LightRecord::new(&l).unwrap();
        assert_eq!(r.distance(Vec3::new(100.0, 0.0, 0.0)), 1.0);
        assert!((r.distance(Vec3::new(1100.0, 0.0, 0.0)) - 0.5).abs() < 1e-5);
        assert_eq!(r.distance(Vec3::new(3000.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn the_shader_block_matches_the_moving_path() {
        // Two lights near a vertex: the block computation equals two_colours (moving), normals unquantised.
        let mut a = light([0.0, 0.0, 255.0], 255.0, 240.0);
        a.not_baked = false;
        let mut b = light([30.0, 200.0, 200.0], 150.0, 200.0);
        b.kind = 1;
        b.matrix.t = [100.0, 0.0, 0.0];
        let lit = WorldLighting {
            lights: [a, b].iter().filter_map(LightRecord::new).collect(),
            shadow: [0.2, 0.1, 0.3, 0.05],
            fog: render::Fog { near: 1.0, far: 2.0, cap: 0.0, color: [0.0; 4] },
            clear: [0.0; 4],
        };
        let centre = Vec3::new(10.0, 0.0, 0.0);
        let rows = light_block(&lit, centre, [255, 200, 180, 255], 300, false);
        let lights: Vec<(LightRecord, Option<f32>)> =
            lit.strongest(centre).into_iter().map(|(r, d)| (r, Some(d))).collect();
        for n in [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.6, 0.8), Vec3::Z] {
            for v in [[255u8, 255, 255, 255], [128, 64, 200, 128]] {
                let p = Vec3::new(12.0, 3.0, 1.0);
                let want = two_colours(LightMode::Moving, v, [255, 200, 180, 255], 300, lit.shadow, &lights, p, n);
                let got = block_two_colours(&rows, v, p, n);
                assert_eq!(got, want, "normal {n:?} colour {v:?}");
            }
        }
    }
}
