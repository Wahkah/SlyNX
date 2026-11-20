//! Camera zones (class 99; camera.md 3, classes/markers-and-volumes.md 7).

use alloc::vec::Vec;

use formats::collision::ConvexVolume;
use formats::geometry::Affine;
use formats::options::OptionTable;
use formats::reader::Reader;
use formats::stream::ObjectStream;

use super::framing::{Directions, Frame, Framing, OffsetFn, ZoneFrame};
use crate::collision::volume::PlacedVolume;
use crate::math::Vec3;

/// Class id of camera zones.
pub const CLASS_CAMERA_ZONE: i16 = 99;

/// One camera zone.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraZone {
    /// Its object-stream record.
    pub record: u32,
    /// Its volume, placed.
    pub volume: PlacedVolume,
    /// Priority (870; higher wins).
    pub priority: i32,
    /// May take over while the player is in the air (871).
    pub air_takeover: bool,
    /// Its framing.
    pub framing: Framing,
    /// Its frame.
    pub frame: ZoneFrame,
    /// Sly 2 / Sly 3: the zone's mesh-and-tree volume, used instead of `volume` when set (`camera::sly2`).
    pub tree: Option<super::sly2::TreeVolume>,
}

impl CameraZone {
    /// Whether the zone contains `p`.
    pub fn contains(&self, p: Vec3) -> bool {
        match &self.tree {
            Some(t) => t.contains(p),
            None => self.volume.contains(p),
        }
    }
}

fn f32_at(b: &[u8], i: usize) -> f32 {
    b.get(i * 4..i * 4 + 4).map_or(0.0, |s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Reads the framing options of a zone into a copy of the default framing.
pub fn framing_from_options(
    data: &[u8],
    options: &formats::options::OptionList,
    table: &OptionTable,
) -> (Framing, i32, bool) {
    let mut f = Framing::DEFAULT;
    let (mut priority, mut air) = (0, false);
    let mut ratio = 1.0f32;
    let mut vector = [1.0f32; 3];
    for (id, v) in options.iter(data, table).flatten() {
        match id {
            851 => f.distance = f32_at(v, 0),
            852 | 854 | 856 => {
                let i = ((id - 852) / 2) as usize;
                f.offsets[i] = OffsetFn { c: [f32_at(v, 0), f32_at(v, 1), f32_at(v, 2)], ..f.offsets[i] };
            }
            858 => f.slack = f32_at(v, 0),
            859 => f.screen[0] = f32_at(v, 0),
            860 => f.screen[1] = f32_at(v, 0),
            861 => f.adjust = f32_at(v, 0),
            862 => f.follow = f32_at(v, 0),
            863 => f.lock_on = f32_at(v, 0),
            864 => ratio = f32_at(v, 0),
            865 => vector = [f32_at(v, 0), f32_at(v, 1), f32_at(v, 2)],
            866 => f.cylindrical = v.first().is_some_and(|&b| b != 0),
            867 => f.directions = Directions(v.get(..2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))),
            868 => f.pan[0] = v.first().is_some_and(|&b| b != 0),
            869 => f.pan[1] = v.first().is_some_and(|&b| b != 0),
            870 => priority = v.get(..4).map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            871 => air = v.first().is_some_and(|&b| b != 0),
            872 => f.side_follow = v.first().is_some_and(|&b| b != 0),
            873 => f.pan_entry[0] = f32_at(v, 0),
            874 => f.pan_entry[1] = f32_at(v, 0),
            875 => f.pan_idle[0] = f32_at(v, 0),
            876 => f.pan_idle[1] = f32_at(v, 0),
            877 => f.cut_on_entry = v.first().is_some_and(|&b| b != 0),
            878 => f.fov = Some(f32_at(v, 0)),
            879 => f.no_clearing = v.first().is_some_and(|&b| b != 0),
            880 => f.follow_target = v.get(..2).map(|b| i16::from_le_bytes([b[0], b[1]])),
            881 => f.follow_speed.c = [f32_at(v, 0), f32_at(v, 1), f32_at(v, 2)],
            882 => {
                f.follow_speed.min = f32_at(v, 0);
                f.follow_speed.max = f32_at(v, 1);
            }
            // 884, a 4-byte integer: −1 no override, 0 forced out, 1 forced in (camera.md 3.5).
            884 => {
                f.state_override = match v.get(..4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])) {
                    Some(0) => Some(false),
                    Some(1) => Some(true),
                    _ => None,
                }
            }
            885 => f.eye_spring[0] = f32_at(v, 0),
            886 => f.eye_spring[1] = f32_at(v, 0),
            _ => {}
        }
    }
    f.focus_scale = crate::math::scale(vector, ratio);
    (f, priority, air)
}

/// Every camera zone of a walked object stream, in the world hierarchy, at its world transform. A zone's path or
/// fixed frame is taken in the zone's own space (the zone's transform applied).
pub fn zones_from_stream(data: &[u8], stream: &ObjectStream, table: &OptionTable) -> formats::Result<Vec<CameraZone>> {
    let world = stream.world_transforms(true);
    let mut out = Vec::new();
    for v in &stream.volumes {
        let rec = &stream.records[v.record as usize];
        if rec.class != CLASS_CAMERA_ZONE || !rec.in_world || rec.template {
            continue;
        }
        let w = world[v.record as usize];
        let volume = ConvexVolume::parse(data, v.offset)?;
        // After the volume: the path kind and the path.
        let size = 2 + 16 * volume.planes.len() + 2 + 6 * volume.nodes.len();
        let mut r = Reader::at(data, v.offset + size);
        let kind = r.i8()?;
        let frame = if kind == -1 {
            let t = formats::geometry::read_transform(&mut r)?;
            ZoneFrame::Fixed(Frame::from_affine(&w.mul(&t)))
        } else {
            let _closed = r.u8()?;
            let n = r.u8()? as usize;
            let stride = if kind == 0 { 16 } else { 40 };
            let mut points = Vec::with_capacity(n);
            for _ in 0..n {
                let at = r.pos();
                r.skip(4)?;
                points.push(w.transform_point(r.vec3()?));
                r.set_pos(at + stride);
            }
            ZoneFrame::Path(points)
        };
        let (framing, priority, air_takeover) = match &rec.options {
            Some(o) => framing_from_options(data, o, table),
            None => (Framing::DEFAULT, 0, false),
        };
        out.push(CameraZone {
            record: v.record,
            volume: PlacedVolume::new(v.record, volume, w),
            priority,
            air_takeover,
            framing,
            frame,
            tree: None,
        });
    }
    Ok(out)
}

/// The zone for a point: the highest priority among those containing it (ties: the first). While airborne, a
/// zone other than `current` takes over only if it allows it (871).
pub fn select_zone(zones: &[CameraZone], p: Vec3, airborne: bool, current: Option<usize>) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, z) in zones.iter().enumerate() {
        let inside = z.framing.state_override.unwrap_or_else(|| z.contains(p));
        if !inside {
            continue;
        }
        if airborne && Some(i) != current && !z.air_takeover {
            continue;
        }
        if best.is_none_or(|b| z.priority > zones[b].priority) {
            best = Some(i);
        }
    }
    if airborne && best.is_none() && current.is_some_and(|c| c < zones.len()) {
        // In the air, keep the current zone rather than drop to the default framing.
        return current;
    }
    best
}

/// A zone for tests and callers that build zones by hand: an axis-aligned box volume.
pub fn box_zone(record: u32, min: Vec3, max: Vec3, framing: Framing, frame: ZoneFrame, priority: i32) -> CameraZone {
    use formats::collision::{Plane, VolumeNode};
    let planes = alloc::vec![
        Plane { normal: [1.0, 0.0, 0.0], d: max[0] },
        Plane { normal: [-1.0, 0.0, 0.0], d: -min[0] },
        Plane { normal: [0.0, 1.0, 0.0], d: max[1] },
        Plane { normal: [0.0, -1.0, 0.0], d: -min[1] },
        Plane { normal: [0.0, 0.0, 1.0], d: max[2] },
        Plane { normal: [0.0, 0.0, -1.0], d: -min[2] },
    ];
    let nodes = (0..6)
        .map(|i| VolumeNode { plane: i as u16, negative: if i < 5 { i + 1 } else { -1 }, positive: -1 })
        .collect();
    CameraZone {
        record,
        volume: PlacedVolume::new(record, ConvexVolume { planes, nodes }, Affine::IDENTITY),
        priority,
        air_takeover: false,
        framing,
        frame,
        tree: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_and_air_takeover() {
        let f = ZoneFrame::Fixed(Frame::level([0.0; 3], 0.0));
        let zones = alloc::vec![
            box_zone(1, [0.0; 3], [100.0; 3], Framing::DEFAULT, f.clone(), 1),
            box_zone(2, [50.0, 0.0, 0.0], [150.0, 100.0, 100.0], Framing::DEFAULT, f, 5),
        ];
        assert_eq!(select_zone(&zones, [10.0; 3], false, None), Some(0));
        assert_eq!(select_zone(&zones, [60.0, 10.0, 10.0], false, None), Some(1));
        // In the air, zone 1 cannot take over from zone 0.
        assert_eq!(select_zone(&zones, [60.0, 10.0, 10.0], true, Some(0)), Some(0));
        assert_eq!(select_zone(&zones, [500.0; 3], false, Some(0)), None);
        assert_eq!(select_zone(&zones, [500.0; 3], true, Some(0)), Some(0));
    }
}
