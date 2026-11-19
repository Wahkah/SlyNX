//! The iris ("keyhole") wipe's shape: the world's screen mask (class 142, classes/paths-and-maps.md 5), its five
//! sets' colours, and where and how large it is drawn (level-flow.md 5), for `render::iris::IrisShape`.
//!
//! After the record's option list: `u16` point count, the points (`f32` x, y, y up), the bounds (min x, min y,
//! max x, max y), then five triangle sets (`u16` count, then count x 3 `u16` point indices), one per mask colour.
//! None of the sets is an opening: the wipe covers the whole screen as it closes.

use alloc::vec::Vec;

use formats::stream::Record;
use formats::world::World;

/// Class id of the screen mask.
pub const SCREEN_MASK_CLASS: i16 = 142;

/// The iris shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenMask {
    /// 2D points, y up.
    pub points: Vec<[f32; 2]>,
    /// Min x, min y, max x, max y.
    pub bounds: [f32; 4],
    /// Five triangle sets, one per mask colour; indices into `points`.
    pub sets: [Vec<[u16; 3]>; 5],
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn f32_at(d: &[u8], o: usize) -> Option<f32> {
    Some(f32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

impl ScreenMask {
    /// Reads the shape at `at` (just past the record's option list); `None` if it is cut short or an index is out
    /// of range.
    pub fn read(data: &[u8], at: usize) -> Option<ScreenMask> {
        let n = u16_at(data, at)? as usize;
        let mut o = at + 2;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            points.push([f32_at(data, o)?, f32_at(data, o + 4)?]);
            o += 8;
        }
        let bounds = [f32_at(data, o)?, f32_at(data, o + 4)?, f32_at(data, o + 8)?, f32_at(data, o + 12)?];
        o += 16;
        let mut sets: [Vec<[u16; 3]>; 5] = Default::default();
        for set in &mut sets {
            let c = u16_at(data, o)? as usize;
            o += 2;
            for _ in 0..c {
                let t = [u16_at(data, o)?, u16_at(data, o + 2)?, u16_at(data, o + 4)?];
                if t.iter().any(|&i| i as usize >= n) {
                    return None;
                }
                set.push(t);
                o += 6;
            }
        }
        Some(ScreenMask { points, bounds, sets })
    }

    /// The shape of a class-142 record.
    pub fn of_record(data: &[u8], record: &Record) -> Option<ScreenMask> {
        if record.class != SCREEN_MASK_CLASS {
            return None;
        }
        ScreenMask::read(data, record.options.as_ref()?.end)
    }

    /// The world's screen mask (its first class-142 record).
    pub fn of_world(world: &World<'_>) -> Option<ScreenMask> {
        let r = world.objects.records.iter().find(|r| r.class == SCREEN_MASK_CLASS)?;
        ScreenMask::of_record(world.data, r)
    }

    /// Every triangle of the five sets.
    pub fn triangles(&self) -> impl Iterator<Item = &[u16; 3]> {
        self.sets.iter().flatten()
    }

    /// Half the larger side of the bounds (35 on the disc).
    pub fn half_extent(&self) -> f32 {
        let [x0, y0, x1, y1] = self.bounds;
        ((x1 - x0).abs().max((y1 - y0).abs()) * 0.5).max(1e-3)
    }

    /// The five sets' colours (RGBA8): the world's shaders with object ids 1166 to 1170
    /// (classes/paths-and-maps.md 5); a missing shader is opaque black.
    pub fn colors_of_world(world: &World<'_>) -> [[u8; 4]; 5] {
        let shader =
            |oid: i16| world.layout.materials.shaders.iter().find(|s| s.oid == oid).map_or([0, 0, 0, 255], |s| s.color);
        [1166, 1167, 1168, 1169, 1170].map(shader)
    }

    /// The sets as slices, for `render::iris::IrisShape::new`.
    pub fn set_slices(&self) -> [&[[u16; 3]]; 5] {
        [&self.sets[0], &self.sets[1], &self.sets[2], &self.sets[3], &self.sets[4]]
    }
}

/// Where the iris is drawn and how large (level-flow.md 5), for closing amount `u` (0 open, 1 closed): centred on
/// the player's screen position `player` (pixels from the top left), kept at least 10 pixels from the left and right
/// edges and 16 from the top and bottom (pixels of the 640 x 448 frame, scaled to the target); the scale goes from
/// `2 x screen width / 70` pixels per mask unit open to `4 / 70` (frame pixels, scaled to the target) closed.
/// Returns (centre, pixels per unit) for `render::iris::IrisShape::set`.
pub fn iris_placement(player: [f32; 2], u: f32, target: (u32, u32)) -> ([f32; 2], f32) {
    let (w, h) = (target.0 as f32, target.1 as f32);
    let (mx, my) = (10.0 * w / 640.0, 16.0 * h / 448.0);
    let centre = [player[0].clamp(mx, (w - mx).max(mx)), player[1].clamp(my, (h - my).max(my))];
    let u = u.clamp(0.0, 1.0);
    let closed = 4.0 / 70.0 * w / 640.0;
    let open = 2.0 * w / 70.0;
    (centre, open + (closed - open) * u)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<u8> {
        // A square from -35 to 35 as two triangles in set 0.
        let mut b = Vec::new();
        b.extend_from_slice(&4u16.to_le_bytes());
        for v in [-35.0f32, -35.0, 35.0, -35.0, 35.0, 35.0, -35.0, 35.0, -35.0, -35.0, 35.0, 35.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&2u16.to_le_bytes());
        for i in [0u16, 1, 2, 0, 2, 3] {
            b.extend_from_slice(&i.to_le_bytes());
        }
        for _ in 0..4 {
            b.extend_from_slice(&0u16.to_le_bytes());
        }
        b
    }

    #[test]
    fn reads_the_shape() {
        let b = square();
        let m = ScreenMask::read(&b, 0).unwrap();
        assert_eq!(m.points.len(), 4);
        assert_eq!(m.bounds, [-35.0, -35.0, 35.0, 35.0]);
        assert_eq!(m.sets[0].len(), 2);
        assert_eq!(m.half_extent(), 35.0);
        // Cut short, or an index out of range: refused.
        assert!(ScreenMask::read(&b[..b.len() - 1], 0).is_none());
        let mut bad = b.clone();
        let n = bad.len();
        bad[n - 8 - 2] = 9;
        assert!(ScreenMask::read(&bad, 0).is_none());
    }

    #[test]
    fn sets_as_slices() {
        let m = ScreenMask::read(&square(), 0).unwrap();
        let s = m.set_slices();
        assert_eq!(s[0], &[[0, 1, 2], [0, 2, 3]]);
        assert!(s[1..].iter().all(|x| x.is_empty()));
    }

    #[test]
    fn placement_scales_and_clamps() {
        let (c, k) = iris_placement([0.0, 1000.0], 0.0, (640, 448));
        assert_eq!(c, [10.0, 448.0 - 16.0]);
        // Open: 2 x 640 / 70 pixels per unit, so the 35-unit half extent spans the 640-pixel width.
        assert!((k * 35.0 - 640.0).abs() < 1e-3);
        let (_, k) = iris_placement([320.0, 224.0], 1.0, (640, 448));
        assert!((k * 35.0 - 2.0).abs() < 1e-4, "closed: 4 / 70 x 35 = 2 pixels");
        // At 1280 x 720 the closed size scales with the width.
        let (_, k) = iris_placement([640.0, 360.0], 1.0, (1280, 720));
        assert!((k * 35.0 - 4.0).abs() < 1e-4);
    }
}
