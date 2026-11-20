//! Sly 2 and Sly 3 camera zones and lens (specs/sly2/camera.md 1 and 3; specs/sly3/options-by-class.md 2) for the
//! Sly 1 [`super::CameraSystem`], which reads them at `configure` for Sly 2 and Sly 3 worlds (a marked parameter).
//!
//! - **Zones** are Sly 2's class 15 and Sly 3's class 14: a transform header, a collision mesh and tree (the volume),
//!   the options, and at most one attached object, the curve the camera follows. A point is inside when a walk of
//!   the tree from node 0, going to the negative child behind a surface's plane and to the positive one in front,
//!   ends behind a node with no negative child ([`TreeVolume`]).
//! - **Framing** comes from Sly 2's options 176 to 214 into the Sly 1 [`Framing`] ([`framing_from_options`]): 178
//!   distance, 179 / 181 / 183 the offset quadratics, 185 / 186 screen offset, 187 focus adjust, 188 passive-turn
//!   gain (the automatic follow), 189 turning rate (lock-on), 190 / 191 the focus scale, 192 / 193 the eye spring
//!   scales, 195 the cylindrical slack, 196 cylindrical, 197 the allowed behaviour bits (Sly 1's directions), 198 /
//!   199 the two returns (Sly 1's idle and entry pans) with 203 / 204 and 205 / 206 their times and strengths, 201
//!   air takeover, 207 cut, 208 field of view (0 keeps it), 209 no view clearing, 210 the look-toward object, 211 /
//!   212 its lead quadratic and limits, 214 the switching mode (−1 by position, 0 forced off, 1 forced on; read
//!   again every frame, since scripts change it). The priority is option 200 + 2 (the view stack's). Option 194 (tilt
//!   limits) has no Sly 1 counterpart and is kept in [`SlyZoneExtras`]. 180 / 182 / 184 (by Sly 3's order) are the
//!   offset limits.
//! - **Sly 3** zones (options-by-class.md 2, 2.1) carry options 203 to 245: 203 to 224 are Sly 2's 176 to 197
//!   (205 = 178 the distance, 206 / 208 / 210 the offset quadratics with their limits 207 / 209 / 211), 225 is the
//!   priority base (Sly 2's 200), 228 to 239 four return triples (flag, time, strength; the first two are Sly 2's
//!   198 / 203 / 204 and 199 / 205 / 206) and 240 to 245 Sly 2's 207 to 212 ([`sly3_to_sly2`]). Sly 3 has no
//!   switching-mode option: its zones follow the player's position.
//! - **Volumes**: normals are outward, (p1 − p0) × (p2 − p0) in stored order; the positive child is on the normal
//!   side, and a point is inside when the walk ends at a missing negative child.
//! - **Lens** (camera.md 1): Sly 2's far clip is 100,000; the world options 161 (field of view), 164 / 165 (near,
//!   far), 166 / 167 / 169 (fog start, end, maximum) and 168 (fog colour) override it.

use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::reader::Reader;

use super::follow::Lens;
use super::framing::{Directions, Frame, Framing, OffsetFn, ZoneFrame};
use super::zone::CameraZone;
use crate::math::{Vec3, cross, dot, normalize, sub};
use crate::object::{Handle, OptionStore, OptionValue};
use crate::world::World;

/// Sly 2's camera-zone class.
pub const ZONE_CLASS_SLY2: i16 = 15;
/// Sly 3's camera-zone class.
pub const ZONE_CLASS_SLY3: i16 = 14;
/// Sly 2's far clip (Sly 1: 20,000).
pub const FAR_SLY2: f32 = 100_000.0;
/// The view-stack priority a zone adds to its option 200.
pub const ZONE_PRIORITY_BASE: i32 = 2;
/// The transform header's size before a zone's collision mesh (parent `s16`, matrix, zone mask).
const HEADER_SIZE: usize = 2 + 48 + 4;

/// The Sly 2 option a Sly 3 camera-zone option stands for, if any.
pub fn sly3_to_sly2(id: u16) -> Option<u16> {
    match id {
        203..=224 => Some(id - 27),
        225 => Some(200),
        // The first two of the four return triples (flag, time, strength): Sly 2's 198 / 203 / 204 and 199 / 205 /
        // 206.
        228 => Some(198),
        229 => Some(203),
        230 => Some(204),
        231 => Some(199),
        232 => Some(205),
        233 => Some(206),
        240..=245 => Some(id - 33),
        _ => None,
    }
}

/// A zone's volume: its collision mesh's surface planes and tree, in the zone's frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeVolume {
    /// Each surface's plane (normal, d): in front when `n · p > d`.
    pub planes: Vec<(Vec3, f32)>,
    /// Nodes: (surface, negative child, positive child); `u32::MAX` is no child.
    pub nodes: Vec<(u16, u32, u32)>,
    /// The zone's world transform, inverted.
    pub to_local: Affine,
}

impl TreeVolume {
    /// Reads the mesh and tree that follow a zone's transform header at `at`, placed by `world`.
    pub fn read(data: &[u8], at: usize, world: &Affine) -> Option<TreeVolume> {
        let mut r = Reader::at(data, at);
        let mesh = formats_sly2::collision::read_mesh(&mut r).ok()?;
        let (raw, _) = formats_sly2::collision::read_raw_tree(&mut r).ok()?;
        let planes: Vec<(Vec3, f32)> = mesh
            .triangles
            .iter()
            .map(|t| {
                let p = |k: usize| mesh.positions.get(t.corners[k] as usize).copied().unwrap_or([0.0; 3]);
                let n = normalize(cross(sub(p(1), p(0)), sub(p(2), p(0))));
                (n, dot(n, p(0)))
            })
            .collect();
        let nodes: Vec<(u16, u32, u32)> =
            raw.iter().map(|&(s, c)| (s, c.map_or(u32::MAX, |c| c.0), c.map_or(u32::MAX, |c| c.1))).collect();
        if nodes.is_empty() || planes.is_empty() {
            return None;
        }
        // Normals are outward: (p1 − p0) × (p2 − p0) in stored order (Sly 3 options-by-class.md 2.1).
        Some(TreeVolume { planes, nodes, to_local: crate::math::affine_inverse(world)? })
    }

    /// Whether a point in the zone's frame is inside.
    pub fn contains_local(&self, p: Vec3) -> bool {
        let mut i = 0usize;
        for _ in 0..self.nodes.len() + 1 {
            let Some(&(s, neg, pos)) = self.nodes.get(i) else { return false };
            let Some(&(n, d)) = self.planes.get(s as usize) else { return false };
            let behind = dot(n, p) <= d;
            let next = if behind { neg } else { pos };
            if next == u32::MAX {
                return behind;
            }
            i = next as usize;
        }
        false
    }

    /// Whether a world point is inside.
    pub fn contains(&self, p: Vec3) -> bool {
        self.contains_local(self.to_local.transform_point(p))
    }
}

/// What a Sly 2 / Sly 3 zone has beyond the Sly 1 framing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlyZoneExtras {
    /// The zone object.
    pub object: Option<Handle>,
    /// Tilt limits (option 194), if set.
    pub tilt: Option<[f32; 2]>,
    /// The switching mode (option 214): −1 by position, 0 forced off, 1 forced on.
    pub mode: i32,
    /// The zone hangs below a playable character (Sly 3: his state cameras, IDs 1725 to 1733 near the paraglide's
    /// states): it moves with him and always contains him, so it stays off until his state turns it on [I].
    pub on_character: bool,
}

fn f(o: &OptionStore, id: u16) -> Option<f32> {
    o.f32(id)
}

fn quad(o: &OptionStore, id: u16) -> Option<[f32; 3]> {
    match o.get(id)? {
        OptionValue::Vector(v) => Some(*v),
        OptionValue::Pair(p) => Some([p[0], p[1], 0.0]),
        v => v.as_f32().map(|x| [x, 1.0, 0.0]),
    }
}

fn pair(o: &OptionStore, id: u16) -> Option<[f32; 2]> {
    match o.get(id)? {
        OptionValue::Pair(p) => Some(*p),
        OptionValue::Vector(v) => Some([v[0], v[1]]),
        _ => None,
    }
}

/// The state override of switching mode `m`.
pub fn state_of_mode(m: i32) -> Option<bool> {
    match m {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// A zone's framing, priority, air takeover and extras from its options, by their Sly 2 IDs (a Sly 3 zone's options
/// translated with [`sly3_to_sly2`] first).
pub fn framing_from_options(o: &OptionStore) -> (Framing, i32, bool, SlyZoneExtras) {
    let mut fr = Framing::DEFAULT;
    if let Some(v) = f(o, 178) {
        fr.distance = v;
    }
    for (k, id) in [179u16, 181, 183].into_iter().enumerate() {
        if let Some(c) = quad(o, id) {
            fr.offsets[k] = OffsetFn { c, ..fr.offsets[k] };
        }
        // Their limits (Sly 3's 207 / 209 / 211; Sly 2's 180 / 182 / 184 by the same order).
        if let Some([lo, hi]) = pair(o, id + 1) {
            fr.offsets[k].min = lo;
            fr.offsets[k].max = hi;
        }
    }
    if let Some(v) = f(o, 185) {
        fr.screen[0] = v;
    }
    if let Some(v) = f(o, 186) {
        fr.screen[1] = v;
    }
    if let Some(v) = f(o, 187) {
        fr.adjust = v;
    }
    if let Some(v) = f(o, 188) {
        fr.follow = v;
    }
    if let Some(v) = f(o, 189) {
        fr.lock_on = v;
    }
    if let Some(v) = f(o, 190) {
        fr.focus_scale[0] = v;
        fr.focus_scale[1] = v;
    }
    if let Some(v) = quad(o, 191) {
        fr.focus_scale = v;
    }
    if let Some(v) = f(o, 192) {
        fr.eye_spring[0] = v;
    }
    if let Some(v) = f(o, 193) {
        fr.eye_spring[1] = v;
    }
    if let Some(v) = f(o, 195) {
        fr.slack = v;
    }
    fr.cylindrical = o.i32(196).is_some_and(|v| v != 0);
    if let Some(v) = o.i32(197) {
        fr.directions = Directions(v as i16);
    }
    fr.pan = [o.i32(199).is_some_and(|v| v != 0), o.i32(198).is_some_and(|v| v != 0)];
    fr.pan_idle = [f(o, 203).unwrap_or(fr.pan_idle[0]), f(o, 204).unwrap_or(fr.pan_idle[1])];
    fr.pan_entry = [f(o, 205).unwrap_or(fr.pan_entry[0]), f(o, 206).unwrap_or(fr.pan_entry[1])];
    fr.cut_on_entry = o.i32(207).is_some_and(|v| v != 0);
    fr.fov = f(o, 208).filter(|&v| v > 0.0);
    fr.no_clearing = o.i32(209).is_some_and(|v| v != 0);
    fr.follow_target = o.i32(210).map(|v| v as i16);
    if let Some(c) = quad(o, 211) {
        fr.follow_speed.c = c;
    }
    if let Some([lo, hi]) = pair(o, 212) {
        fr.follow_speed.min = lo;
        fr.follow_speed.max = hi;
    }
    let mode = o.i32(214).unwrap_or(-1);
    fr.state_override = state_of_mode(mode);
    let priority = o.i32(200).unwrap_or(0) + ZONE_PRIORITY_BASE;
    let air = o.i32(201).is_some_and(|v| v != 0);
    (fr, priority, air, SlyZoneExtras { object: None, tilt: pair(o, 194), mode, on_character: false })
}

/// A Sly 3 zone's options with their Sly 2 IDs.
pub fn translated(o: &OptionStore) -> OptionStore {
    let mut out = OptionStore::new();
    for (id, v) in o.entries() {
        if let Some(to) = sly3_to_sly2(*id) {
            out.push(to, v.clone());
        }
    }
    out
}

/// Every camera zone of a Sly 2 or Sly 3 world, with the zone objects (empty for other worlds).
pub fn zones(world: &World) -> (Vec<CameraZone>, Vec<SlyZoneExtras>) {
    let (class, sly3) = if world.sly2.is_some() {
        (ZONE_CLASS_SLY2, false)
    } else if world.sly3.is_some() {
        (ZONE_CLASS_SLY3, true)
    } else {
        return (Vec::new(), Vec::new());
    };
    let mut zones = Vec::new();
    let mut extras = Vec::new();
    let data = &world.level.bytes;
    for h in world.handles().filter(|&h| world.class(h) == Some(class)) {
        let Some(w) = world.world_transform(h) else { continue };
        let body = if sly3 {
            world.sly3_record(h).map(|r| r.body_offset as usize)
        } else {
            world.sly2_record(h).map(|r| r.body_offset as usize)
        };
        let Some(tree) = body.and_then(|b| TreeVolume::read(data, b + HEADER_SIZE, &w)) else { continue };
        let empty = OptionStore::new();
        let own = world.options(h).unwrap_or(&empty);
        let o = if sly3 { translated(own) } else { own.clone() };
        let (framing, priority, air_takeover, mut extra) = framing_from_options(&o);
        extra.object = Some(h);
        let characters = crate::behavior::sly3::character_classes(world);
        let mut a = world.parent(h);
        while let Some(x) = a {
            extra.on_character |= world.class(x).is_some_and(|c| characters.contains(&c));
            a = world.parent(x);
        }
        // The attached curve is the path the camera follows (Sly 2 classes 10 to 14; Sly 3 classes 9 to 13, among
        // the zone's placed objects); else the zone's own frame.
        let curves = if sly3 { 9..=13 } else { 10..=14 };
        let path = world
            .children(h)
            .find(|&c| world.class(c).is_some_and(|c| curves.contains(&c)))
            .map(|c| curve_points(world, c))
            .unwrap_or_default();
        let frame = if path.len() >= 2 { ZoneFrame::Path(path) } else { ZoneFrame::Fixed(Frame::from_affine(&w)) };
        let mut z = super::zone::box_zone(h.index(), [0.0; 3], [0.0; 3], framing, frame, priority);
        z.air_takeover = air_takeover;
        z.tree = Some(tree);
        zones.push(z);
        extras.push(extra);
    }
    (zones, extras)
}

/// The points of a Sly 2 or Sly 3 curve object in world space (Sly 2 classes 10 to 14, Sly 3 classes 9 to 13;
/// specs/sly2 and sly3 classes/paths-and-volumes.md): after the transform header, a tangent flag, a byte, the point
/// count, one value per point, then the points.
pub fn curve_points(world: &World, h: Handle) -> Vec<Vec3> {
    let body = world.sly2_record(h).map(|r| r.body_offset).or_else(|| world.sly3_record(h).map(|r| r.body_offset));
    let (Some(b), Some(w)) = (body, world.world_transform(h)) else { return Vec::new() };
    let mut r = Reader::at(&world.level.bytes, b as usize + HEADER_SIZE);
    let parsed = (|| -> formats::Result<Vec<Vec3>> {
        r.skip(2)?;
        let n = r.u16()? as usize;
        r.skip(4 * n)?;
        (0..n).map(|_| Ok(w.transform_point(r.vec3()?))).collect()
    })();
    parsed.unwrap_or_default()
}

/// Reads each zone's switching mode again (scripts set option 214 on Sly 2 zones).
pub fn refresh(world: &World, zones: &mut [CameraZone], extras: &mut [SlyZoneExtras]) {
    if world.sly2.is_none() && world.sly3.is_none() {
        return;
    }
    for (z, e) in zones.iter_mut().zip(extras.iter_mut()) {
        let Some(h) = e.object else { continue };
        // A zone out of the world (in a grouper or a package not placed yet, or removed at start) is off; so is a
        // character's own state zone.
        if !world.in_world(h) || e.on_character {
            z.framing.state_override = Some(false);
            e.mode = i32::MIN;
            continue;
        }
        let m = if world.sly2.is_some() { world.options(h).and_then(|o| o.i32(214)).unwrap_or(-1) } else { -1 };
        if m != e.mode {
            e.mode = m;
            z.framing.state_override = state_of_mode(m);
        }
    }
}

/// The lens of a Sly 2 or Sly 3 world (camera.md 1): `defaults` with Sly 2's far clip and, for Sly 2, the world
/// options' overrides.
pub fn lens(world: &World, defaults: Lens) -> Lens {
    let mut l = Lens { far: FAR_SLY2, ..defaults };
    // Sly 3 keeps the lens on class-5 objects (camera.md 2): the first one sets it.
    if world.sly3.is_some()
        && let Some(o) =
            world.handles().find(|&h| world.class(h) == Some(LENS_CLASS_SLY3)).and_then(|h| world.options(h))
    {
        l.fov = o.f32_or(172, l.fov);
        l.near = o.f32_or(175, l.near);
        l.far = o.f32_or(176, l.far);
    }
    if world.sly2.is_some() {
        let o = &world.world_options;
        l.fov = o.f32_or(161, l.fov);
        l.near = o.f32_or(164, l.near);
        l.far = o.f32_or(165, l.far);
        l.fog = [o.f32_or(166, l.fog[0]), o.f32_or(167, l.fog[1]), o.f32_or(169, l.fog[2])];
        if let Some(OptionValue::Color(c)) = o.get(168) {
            l.fog_color = Some(*c);
        }
    }
    l
}

/// The camera's collision world for a Sly 2 or Sly 3 world (view clearing and cuts, Sly 2 camera.md 7): every solid
/// in the world with collision parts, each surface covered by its polygon, except the playable characters and what
/// hangs below them. `None` for other worlds or none found.
pub fn collision_world(world: &World) -> Option<crate::collision::CollisionWorld> {
    use crate::collision::TriMesh;
    use crate::collision::world::{CollisionWorld, Shape};
    let characters = crate::behavior::sly3::character_classes(world);
    let sly2 = world.sly2.is_some();
    if !sly2 && world.sly3.is_none() {
        return None;
    }
    let mut cw = CollisionWorld::new();
    let mut shapes: alloc::collections::BTreeMap<u32, Vec<crate::collision::world::ShapeId>> = Default::default();
    let mut bodies = 0;
    for h in world.handles() {
        if !world.in_world(h) {
            continue;
        }
        let mut a = Some(h);
        let mut held = false;
        while let Some(x) = a {
            held |= world.class(x).is_some_and(|c| characters.contains(&c));
            a = world.parent(x);
        }
        if held {
            continue;
        }
        let Some(rec) = world.record(h) else { continue };
        let ids = if sly2 {
            let Some(formats_sly2::stream::Body::Solid(s)) = world.sly2_record(h).map(|r| &r.body) else { continue };
            if s.collision.is_empty() {
                continue;
            }
            shapes
                .entry(rec)
                .or_insert_with(|| {
                    s.collision
                        .iter()
                        .map(|p| {
                            let mesh = p.mesh.with_polygons();
                            cw.add_shape(Shape { main: TriMesh::new(&mesh, &p.tree), camera: None })
                        })
                        .collect()
                })
                .clone()
        } else {
            let Some(formats_sly3::stream::Body::Solid(s)) = world.sly3_record(h).map(|r| &r.body) else { continue };
            if s.collision.is_empty() {
                continue;
            }
            shapes
                .entry(rec)
                .or_insert_with(|| {
                    s.collision
                        .iter()
                        .filter(|p| !p.mesh.is_empty())
                        .map(|p| {
                            let mesh = p.mesh.with_polygons();
                            cw.add_shape(Shape { main: TriMesh::new_wide(&mesh, p.tree.clone()), camera: None })
                        })
                        .collect()
                })
                .clone()
        };
        let Some(t) = world.world_transform(h) else { continue };
        for id in ids {
            cw.add_body(id, t, 0, h.index(), true);
            bodies += 1;
        }
    }
    if bodies == 0 {
        return None;
    }
    cw.rebuild();
    Some(cw)
}

/// The Sly 2 / Sly 3 follow-camera and binocucom constants found in both executables (Sly 2 camera.md 4 and 6;
/// Sly 3 camera.md 6, where they equal Sly 2's).
#[derive(Clone, Debug, PartialEq)]
pub struct FollowConstants {
    /// The follow view's tilt limits (−0.5 … 1.5551 rad).
    pub tilt: [f32; 2],
    /// The stick-toward-camera lead factor (a quadratic of the stick's y) and its limits.
    pub lead: [f32; 3],
    /// See [`FollowConstants::lead`].
    pub lead_limits: [f32; 2],
    /// Turn-toward-sample smoothing (× rate).
    pub sample_smoothing: [f32; 3],
    /// Lead easing.
    pub lead_easing: [f32; 3],
    /// Passive tilt toward the limits (× player speed).
    pub passive_tilt: [f32; 3],
    /// The binocucom block (Sly 2 `0x2BD950` … `0x2BD9BF`, 28 floats: limits, zoom, turning, smoothing).
    pub binocucom: [f32; 28],
}

/// Where [`FollowConstants`] are: tilt, lead, lead limits, sample smoothing, lead easing, passive tilt, binocucom.
const SLY2_ADDRESSES: [u32; 7] =
    [0x002C_41B0, 0x002C_41F0, 0x002C_4200, 0x002C_4208, 0x002C_4230, 0x002C_4260, 0x002B_D950];
const SLY3_ADDRESSES: [u32; 7] =
    [0x0032_90E8, 0x0032_90F0, 0x0032_9100, 0x0032_9108, 0x0032_9130, 0x0032_9140, 0x0032_0E80];

impl FollowConstants {
    /// The spec's values (the binocucom block's own values are read from an executable).
    pub const SPEC: FollowConstants = FollowConstants {
        // The spec rounds the upper limit to 1.5551; the stored float is 1.5550884.
        tilt: [-0.5, 1.555_088_4],
        lead: [1.0, -2.0, -2.0],
        lead_limits: [1.0, 2.0],
        sample_smoothing: [0.5, 0.0, 0.5],
        lead_easing: [4.0, 0.0, 0.25],
        passive_tilt: [0.75, 0.0, 2.0],
        binocucom: [0.0; 28],
    };

    fn read(bytes: impl Fn(u32, usize) -> formats::Result<Vec<u8>>, at: [u32; 7]) -> formats::Result<FollowConstants> {
        let fs = |va: u32, n: usize| -> formats::Result<Vec<f32>> {
            let b = bytes(va, n * 4)?;
            Ok(b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect())
        };
        let a3 = |v: Vec<f32>| [v[0], v[1], v[2]];
        let b = fs(at[6], 28)?;
        let mut binocucom = [0.0; 28];
        binocucom.copy_from_slice(&b);
        let t = fs(at[0], 2)?;
        let l = fs(at[2], 2)?;
        Ok(FollowConstants {
            tilt: [t[0], t[1]],
            lead: a3(fs(at[1], 3)?),
            lead_limits: [l[0], l[1]],
            sample_smoothing: a3(fs(at[3], 3)?),
            lead_easing: a3(fs(at[4], 3)?),
            passive_tilt: a3(fs(at[5], 3)?),
            binocucom,
        })
    }

    /// Read from the Sly 2 executable after its hash check.
    pub fn from_sly2_executable(exe: &formats_sly2::exe::Executable) -> formats::Result<FollowConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        Self::read(|va, n| Ok(exe.bytes_at(va, n)?.to_vec()), SLY2_ADDRESSES)
    }

    /// Read from the Sly 3 executable after its hash check.
    pub fn from_sly3_executable(exe: &formats_sly3::exe::Executable) -> formats::Result<FollowConstants> {
        if !formats_sly3::exe::is_supported(exe) {
            return Err(formats::Error::UnknownExecutable);
        }
        Self::read(|va, n| Ok(exe.bytes_at(va, n)?.to_vec()), SLY3_ADDRESSES)
    }
}

/// Sly 3's lens objects (class 5; Sly 3 camera.md 2): field of view 172, near clip 175, far clip 176.
pub const LENS_CLASS_SLY3: i16 = 5;

/// Sly 3's cutscene-camera class (Sly 2's 44) and the Sly 2 IDs of its options 1153 to 1158 (1054 to 1059).
pub const CUTSCENE_CLASS_SLY3: i16 = 42;

/// The Sly 2 camera-object option a Sly 3 cutscene-camera option stands for.
pub fn sly3_cutscene_option(id: u16) -> Option<u16> {
    (1153..=1158).contains(&id).then(|| id - 99)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sly3_ids_and_spec_zone_vector() {
        assert_eq!(sly3_to_sly2(205), Some(178));
        assert_eq!(sly3_to_sly2(241), Some(208));
        assert_eq!((sly3_to_sly2(225), sly3_to_sly2(230), sly3_to_sly2(235)), (Some(200), Some(204), None));
        // camera.md 8: zone 1524 (row 3).
        let mut o = OptionStore::new();
        o.push(214, OptionValue::Int(0));
        o.push(197, OptionValue::Int(4));
        o.push(178, OptionValue::Float(-400.0));
        o.push(183, OptionValue::Vector([150.0, 1.0, 0.0]));
        o.push(208, OptionValue::Float(1.0));
        o.push(186, OptionValue::Float(0.3));
        o.push(200, OptionValue::Int(3));
        let (fr, priority, air, e) = framing_from_options(&o);
        assert_eq!((priority, air, e.mode), (5, false, 0));
        assert_eq!(fr.state_override, Some(false));
        assert_eq!((fr.distance, fr.offsets[2].c, fr.screen[1], fr.fov), (-400.0, [150.0, 1.0, 0.0], 0.3, Some(1.0)));
        assert!(fr.directions.manual() && !fr.directions.reverse());
    }

    #[test]
    fn tree_walk_inside_a_box() {
        // A unit box: one node per face, each behind-child the next face; the last behind is inside.
        let planes = alloc::vec![
            ([1.0, 0.0, 0.0], 1.0),
            ([-1.0, 0.0, 0.0], 1.0),
            ([0.0, 1.0, 0.0], 1.0),
            ([0.0, -1.0, 0.0], 1.0),
            ([0.0, 0.0, 1.0], 1.0),
            ([0.0, 0.0, -1.0], 1.0),
        ];
        let nodes = (0..6u16).map(|i| (i, if i < 5 { u32::from(i) + 1 } else { u32::MAX }, u32::MAX)).collect();
        let v = TreeVolume { planes, nodes, to_local: Affine::IDENTITY };
        assert!(v.contains([0.5, -0.5, 0.0]));
        assert!(!v.contains([1.5, 0.0, 0.0]));
        assert!(!v.contains([0.0, 0.0, -2.0]));
    }
}
