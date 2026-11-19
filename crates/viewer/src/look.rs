//! How a world looks: its clear colour, fog and light levels, its lights, and the per-vertex lighting of shader
//! kind 0 (specs/sly1/rendering.md 1, 2 and 4; effects.md 5).

use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::options::{OptionEncoding, OptionList, OptionTable};
use render::Fog;
use render::math::{Vec3, cos, sqrt};

/// A world's or an object's options, decoded by id.
pub struct Options<'a> {
    entries: Vec<(i16, &'a [u8])>,
}

impl<'a> Options<'a> {
    /// The options of `list` (an empty set when there is none or it does not read).
    pub fn read(data: &'a [u8], list: Option<&OptionList>, table: &'a OptionTable) -> Options<'a> {
        let entries = list.map_or_else(Vec::new, |l| l.iter(data, table).filter_map(Result::ok).collect());
        Options { entries }
    }

    fn raw(&self, id: i16) -> Option<&'a [u8]> {
        self.entries.iter().rev().find(|(i, _)| *i == id).map(|(_, v)| *v)
    }

    /// An `f`, `p`, `v` or `q` option's floats.
    pub fn floats(&self, id: i16) -> Option<Vec<f32>> {
        let v = self.raw(id)?;
        Some(v.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect())
    }

    pub fn float(&self, id: i16) -> Option<f32> {
        self.floats(id)?.first().copied()
    }

    /// A `b` or `h` option as an integer.
    pub fn int(&self, id: i16) -> Option<i32> {
        let v = self.raw(id)?;
        match v.len() {
            1 => Some(v[0] as i32),
            2 => Some(i16::from_le_bytes([v[0], v[1]]) as i32),
            4 => Some(i32::from_le_bytes([v[0], v[1], v[2], v[3]])),
            _ => None,
        }
    }

    /// A packed colour (`c`): R, G, B, A bytes.
    pub fn color(&self, id: i16) -> Option<[u8; 4]> {
        let v = self.raw(id)?;
        (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
    }

    pub fn has(&self, id: i16) -> bool {
        self.raw(id).is_some()
    }
}

/// Whether an option id's encoding is a float (for tests of the table).
pub fn is_float(table: &OptionTable, id: i16) -> bool {
    table.get(id) == Some(OptionEncoding::Float)
}

/// World-wide settings of rendering.md 2.3, 3.1 and 4.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLook {
    /// Option 528, alpha x 2/255 (default (16, 16, 40, 80)).
    pub clear: [f32; 4],
    /// Options 532 to 535; `None` when the world has no fog colour.
    pub fog: Option<Fog>,
    /// Ls: option 527 / 255, default 50/255.
    pub shadow_level: f32,
    /// Lm: option 526 / 255, default 0.
    pub mid_level: f32,
    /// Sly 2 and 3: the fog keeps rising past its far distance (`Renderer::set_fog_rising`).
    pub fog_rising: bool,
}

impl WorldLook {
    pub fn from_options(o: &Options<'_>) -> WorldLook {
        let c = o.color(528).unwrap_or([16, 16, 40, 80]);
        let clear =
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, (c[3] as f32 * 2.0 / 255.0).min(1.0)];
        let fog = o.color(535).map(|f| Fog {
            near: o.float(532).unwrap_or(500.0),
            far: o.float(533).unwrap_or(20000.0),
            cap: o.float(534).unwrap_or(0.5),
            color: [f[0] as f32 / 255.0, f[1] as f32 / 255.0, f[2] as f32 / 255.0, f[3] as f32 * 2.0 / 255.0],
        });
        WorldLook {
            clear,
            fog,
            shadow_level: o.float(527).unwrap_or(50.0) / 255.0,
            mid_level: o.float(526).unwrap_or(0.0) / 255.0,
            fog_rising: false,
        }
    }
}

/// `q(theta) = cos(theta/2) + cos^3(theta/2)`, theta in degrees.
pub fn q(theta: f32) -> f32 {
    let c = cos(theta.to_radians() * 0.5);
    c + c * c * c
}

/// The line `y = a x + b` through (x0, y0) and (x1, y1); flat (a = 0, b = y0) when the x values are within 0.0001.
pub fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32) {
    if (x1 - x0).abs() < 1e-4 {
        return (0.0, y0);
    }
    let a = (y1 - y0) / (x1 - x0);
    (a, y0 - a * x0)
}

/// Light kinds (option 491).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    Directional,
    Point,
    /// Frustum and spot lights: treated as point lights (2 spots on the disc; rendering.md 2.4 says low priority).
    Spot,
}

/// One light of the world, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    pub position: Vec3,
    /// Unit direction toward the light (directional lights).
    pub toward: Vec3,
    /// (a, b) of the highlight, mid-tone and shadow lines.
    pub high: (f32, f32),
    pub mid: (f32, f32),
    pub shadow: (f32, f32),
    /// Highlight colour, largest channel 1.
    pub highlight: [f32; 3],
    /// Falloff `clamp(alpha + beta / d, 0, 1)` and its range.
    pub alpha: f32,
    pub beta: f32,
    pub range: f32,
    /// Option 501: does not light moving objects.
    pub skip_moving: bool,
}

/// HSV (hue in degrees, saturation and value 0..1) to RGB.
fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = ((h % 360.0) + 360.0) % 360.0 / 60.0;
    let i = h as i32;
    let f = h - i as f32;
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

impl Light {
    /// From a class-81 record's options and world transform.
    pub fn from_options(o: &Options<'_>, world: &Affine) -> Light {
        let kind = match o.int(491).unwrap_or(0) {
            0 => LightKind::Directional,
            1 => LightKind::Point,
            _ => LightKind::Spot,
        };
        let hsv_v = o.floats(492).unwrap_or_default();
        let (hue, sat, val) = (
            hsv_v.first().copied().unwrap_or(0.0),
            hsv_v.get(1).copied().unwrap_or(0.0) / 255.0,
            hsv_v.get(2).copied().unwrap_or(0.0) / 255.0,
        );
        let mut highlight = hsv(hue, sat, val);
        let m = highlight[0].max(highlight[1]).max(highlight[2]);
        if m > 0.0 {
            highlight = [highlight[0] / m, highlight[1] / m, highlight[2] / m];
        }
        let high = if val > 0.0 { line(q(o.float(495).unwrap_or(180.0)), 0.0, 2.0, val) } else { (0.0, 0.0) };
        let mid = line(q(o.float(496).unwrap_or(240.0)), 0.0, 2.0, o.float(493).unwrap_or(0.0) / 255.0);
        let shadow = line(q(360.0 - o.float(497).unwrap_or(180.0)), 0.0, -2.0, o.float(494).unwrap_or(0.0) / 255.0);
        let range = o.floats(502).filter(|r| r.len() == 2).unwrap_or_else(|| alloc::vec![200.0, 2000.0]);
        let (min, max) = (range[0].max(1e-3), range[1].max(range[0] + 1e-3));
        let beta = min * max / (max - min);
        let z = Vec3::new(world.cols[2][0], world.cols[2][1], world.cols[2][2]).normalize();
        Light {
            kind,
            position: Vec3::new(world.t[0], world.t[1], world.t[2]),
            toward: z,
            high,
            mid,
            shadow,
            highlight,
            alpha: 1.0 - beta / min,
            beta,
            range: max,
            skip_moving: o.int(501).unwrap_or(0) != 0,
        }
    }

    /// Falloff and unit direction toward the light from `p`; `None` when the light does not reach it.
    pub fn reach(&self, p: Vec3) -> Option<(f32, Vec3)> {
        match self.kind {
            LightKind::Directional => Some((1.0, self.toward)),
            _ => {
                let d = self.position - p;
                let dist = d.length();
                let h = (self.alpha + self.beta / dist.max(1e-3)).clamp(0.0, 1.0);
                (h > 0.0).then(|| (h, if dist > 0.0 { d * (1.0 / dist) } else { Vec3::Z }))
            }
        }
    }
}

/// The weights of one vertex for shader kind 0 (rendering.md 2.3): shadow (scalar), mid-tone and highlight (RGB),
/// with `c` the vertex colour after the material (0..1), `e` the emission, `n` the unit world normal. For a moving
/// object, `object` is its bounding-sphere centre: each light is then evaluated once from there (direction and
/// falloff), and lights with option 501 are left out.
pub fn kind0_weights(
    look: &WorldLook,
    lights: &[&Light],
    p: Vec3,
    n: Vec3,
    c: [f32; 4],
    e: f32,
    object: Option<Vec3>,
) -> (f32, [f32; 3], [f32; 3]) {
    let mut sigma = look.shadow_level;
    let mut mu = look.mid_level + e;
    let mut eta = [0.0f32; 3];
    for l in lights {
        if object.is_some() && l.skip_moving {
            continue;
        }
        let Some((h, dir)) = l.reach(object.unwrap_or(p)) else { continue };
        let cdot = n.dot(dir);
        let x = cdot + cdot * cdot * cdot;
        sigma += h * (l.shadow.0 * x + l.shadow.1).max(0.0);
        mu += h * (l.mid.0 * x + l.mid.1).max(0.0);
        let hi = h * (l.high.0 * x + l.high.1).max(0.0);
        for k in 0..3 {
            eta[k] += hi * l.highlight[k];
        }
    }
    let rho = 1.0 - eta[0].max(eta[1]).max(eta[2]);
    let mu = mu.clamp(0.0, rho.max(0.0));
    let g = (c[0] + c[1] + c[2]) / 3.0;
    let wh = [eta[0].min(1.0) * g, eta[1].min(1.0) * g, eta[2].min(1.0) * g];
    let wm = [mu * c[0], mu * c[1], mu * c[2]];
    let ws = (rho - mu).min(sigma).max(0.0) * g;
    (ws, wm, wh)
}

/// Emission of a submesh from its self-illumination byte.
pub fn emission(byte: u8) -> f32 {
    byte as f32 * 0.003973
}

/// Unit length of a vector, or zero.
pub fn unit(v: [f32; 3]) -> Vec3 {
    let l = sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if l > 0.0 { Vec3::new(v[0] / l, v[1] / l, v[2] / l) } else { Vec3::ZERO }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    fn paris_light(distance_test: bool) -> (WorldLook, Light) {
        let look =
            WorldLook { clear: [0.0; 4], fog: None, shadow_level: 60.0 / 255.0, mid_level: 0.0, fog_rising: false };
        let _ = distance_test;
        let mid = line(q(200.0), 0.0, 2.0, 200.0 / 255.0);
        let beta = 200.0 * 2000.0 / 1800.0;
        let light = Light {
            kind: LightKind::Point,
            position: Vec3::ZERO,
            toward: Vec3::Z,
            high: (0.0, 0.0),
            mid,
            shadow: line(q(360.0), 0.0, -2.0, 0.0),
            highlight: [0.0; 3],
            alpha: 1.0 - beta / 200.0,
            beta,
            range: 2000.0,
            skip_moving: false,
        };
        (look, light)
    }

    #[test]
    fn rendering_md_test_vectors() {
        for (t, v) in [(0.0, 2.0), (90.0, 1.0607), (180.0, 0.0), (200.0, -0.1789), (240.0, -0.625), (360.0, -2.0)] {
            assert!(close(q(t), v, 1e-4), "q({t})");
        }
        let (look, light) = paris_light(false);
        assert!(close(light.mid.0, 0.35996, 1e-4) && close(light.mid.1, 0.06439, 1e-4));
        for (d, h) in [(100.0, 1.0), (200.0, 1.0), (600.0, 0.2593), (1000.0, 0.1111), (2000.0, 0.0)] {
            let got = light.reach(Vec3::new(0.0, 0.0, -d)).map_or(0.0, |r| r.0);
            assert!(close(got, h, 1e-3), "falloff at {d}: {got}");
        }
        let white = [1.0; 4];
        // No light: shadow image at Ls.
        let (ws, wm, _) = kind0_weights(&look, &[], Vec3::ZERO, Vec3::Z, white, 0.0, None);
        assert!(close(ws, 0.2353, 1e-3) && wm == [0.0; 3]);
        // Facing the light within 200: wS 0.2157, wM 0.7843.
        let at = Vec3::new(0.0, 0.0, -100.0);
        let (ws, wm, _) = kind0_weights(&look, &[&light], at, Vec3::Z, white, 0.0, None);
        assert!(close(ws, 0.2157, 1e-3) && close(wm[0], 0.7843, 1e-3), "{ws} {wm:?}");
        // At 600: wS 0.2353, wM 0.2033.
        let (ws, wm, _) = kind0_weights(&look, &[&light], Vec3::new(0.0, 0.0, -600.0), Vec3::Z, white, 0.0, None);
        assert!(close(ws, 0.2353, 1e-3) && close(wm[0], 0.2033, 1e-3), "{ws} {wm:?}");
        // Grey vertex: wS 0.1083, wM 0.3937.
        let grey = [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0];
        let (ws, wm, _) = kind0_weights(&look, &[&light], at, Vec3::Z, grey, 0.0, None);
        assert!(close(ws, 0.1083, 1e-3) && close(wm[0], 0.3937, 1e-3), "{ws} {wm:?}");
        // Highlight: option 492 = (30, 200, 250), angle 90.
        let mut hl = light;
        let mut c = hsv(30.0, 200.0 / 255.0, 250.0 / 255.0);
        let m = c[0].max(c[1]).max(c[2]);
        c = [c[0] / m, c[1] / m, c[2] / m];
        assert!(close(c[1], 0.6078, 1e-3) && close(c[2], 0.2157, 1e-3), "{c:?}");
        hl.highlight = c;
        hl.high = line(q(90.0), 0.0, 2.0, 250.0 / 255.0);
        assert!(close(hl.high.0, 1.0437, 1e-3) && close(hl.high.1, -1.1070, 1e-3), "{:?}", hl.high);
        let (ws, wm, wh) = kind0_weights(&look, &[&hl], at, Vec3::Z, white, 0.0, None);
        assert!(close(wh[0], 0.9804, 1e-3) && close(wh[1], 0.5959, 1e-3) && close(wh[2], 0.2115, 1e-3), "{wh:?}");
        assert!(close(wm[0], 0.0196, 1e-3) && close(ws, 0.0, 1e-6));
        assert!(close(emission(60), 0.2384, 1e-4) && close(emission(180), 0.7151, 1e-4));
    }

    #[test]
    fn falloff_300_700() {
        let beta = 300.0 * 700.0 / 400.0;
        let l = Light { alpha: 1.0 - beta / 300.0, beta, ..paris_light(true).1 };
        for (d, h) in [(300.0, 1.0), (500.0, 0.30), (700.0, 0.0)] {
            let got = l.reach(Vec3::new(d, 0.0, 0.0)).map_or(0.0, |r| r.0);
            assert!(close(got, h, 1e-3), "{d}: {got}");
        }
    }
}

#[cfg(test)]
mod moving_tests {
    use super::*;

    #[test]
    fn moving_objects_use_their_centre_and_skip_501_lights() {
        let look = WorldLook { clear: [0.0; 4], fog: None, shadow_level: 0.2, mid_level: 0.0, fog_rising: false };
        let mid = line(q(200.0), 0.0, 2.0, 200.0 / 255.0);
        let beta = 200.0 * 2000.0 / 1800.0;
        let l = Light {
            kind: LightKind::Point,
            position: Vec3::ZERO,
            toward: Vec3::Z,
            high: (0.0, 0.0),
            mid,
            shadow: (0.0, 0.0),
            highlight: [0.0; 3],
            alpha: 1.0 - beta / 200.0,
            beta,
            range: 2000.0,
            skip_moving: false,
        };
        // A vertex far from the light, on an object whose centre is close: lit as at the centre.
        let p = Vec3::new(0.0, 0.0, -1500.0);
        let (_, wm_static, _) = kind0_weights(&look, &[&l], p, Vec3::Z, [1.0; 4], 0.0, None);
        let (_, wm_moving, _) =
            kind0_weights(&look, &[&l], p, Vec3::Z, [1.0; 4], 0.0, Some(Vec3::new(0.0, 0.0, -100.0)));
        assert!(wm_moving[0] > wm_static[0] + 0.3);
        let skip = Light { skip_moving: true, ..l };
        let (_, wm, _) = kind0_weights(&look, &[&skip], p, Vec3::Z, [1.0; 4], 0.0, Some(Vec3::new(0.0, 0.0, -100.0)));
        assert_eq!(wm, [0.0; 3]);
    }
}
