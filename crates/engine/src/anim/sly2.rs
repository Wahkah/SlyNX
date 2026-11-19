//! Sly 2 clip playback and skinning (specs/sly2/animation.md 2, 7 and 10, geometry.md 5, transforms.md 4): the
//! parts of `engine::anim` that differ from Sly 1. Channels, mirror pairs and chains are in [`super::xform`].
//!
//! - **Repeat modes are not stored** in Sly 2 clips: the caller chooses one ([`Wrap`]) per playback.
//! - **Events.** Timed events fire as playback crosses their frame, in either direction; the apply list fires when
//!   the playback starts and the retract list when it is removed (animation.md 2). Of the event types only type 5 is
//!   acted on here: it jumps the playback to its frame at its speed (an explicit loop when it sits at the end of the
//!   clip, animation.md 7). Every fired event is reported to the caller ([`Fired`]); none of Sly 1's event meanings
//!   are applied.
//! - **Skinning.** A mesh's bones are (object ID, transform reference) pairs; each bone's matrix is
//!   `W_bone · W_bone,bind⁻¹ · W_mesh,bind`, and a vertex blends up to four of them with byte weights `w / 255`
//!   ([`SkinPalette`], [`blend`]).

use alloc::vec::Vec;

use formats::anim::Event;
use formats::geometry::Affine;
use formats_sly2::anim::Clip;
use formats_sly2::geometry::SkinWeights;

use crate::math::affine_inverse;

/// Frames per second of event frames.
pub const FRAMES_PER_SECOND: f32 = 60.0;

/// Event type 5: jump to a frame at a speed.
pub const EVENT_JUMP: i8 = 5;

/// How a playback wraps at the ends of its clip (chosen by the game, not stored in the clip).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wrap {
    /// Stop at the end (or the start, playing backwards) and report finished.
    #[default]
    Once,
    /// Wrap around.
    Loop,
    /// Turn around at each end.
    PingPong,
    /// Stay at the end without finishing.
    Hold,
}

/// Which list a fired event is in, and its index there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fired {
    /// `Clip::on_apply[i]`.
    Apply(u16),
    /// `Clip::timed[i]`.
    Timed(u16),
    /// `Clip::on_retract[i]`.
    Retract(u16),
}

impl Fired {
    /// The event.
    pub fn get<'c>(&self, clip: &'c Clip) -> Option<&'c Event> {
        match *self {
            Fired::Apply(i) => clip.on_apply.get(i as usize),
            Fired::Timed(i) => clip.timed.get(i as usize),
            Fired::Retract(i) => clip.on_retract.get(i as usize),
        }
    }
}

/// The frame and speed of a type-5 event.
pub fn jump_of(e: &Event) -> Option<(i16, f32)> {
    if e.kind != EVENT_JUMP || e.payload.len() < 6 {
        return None;
    }
    let frame = i16::from_le_bytes([e.payload[0], e.payload[1]]);
    let speed = f32::from_le_bytes(e.payload[2..6].try_into().ok()?);
    Some((frame, speed))
}

/// One playback of a Sly 2 clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipPlayer {
    time: f32,
    speed: f32,
    wrap: Wrap,
    finished: bool,
}

/// Most wraps or jumps followed in one step (a clip of near-zero length, or jumps that keep firing).
const MAX_SEGMENTS: u32 = 16;

impl ClipPlayer {
    /// Starts a playback at `time`; fires the apply list.
    pub fn start(clip: &Clip, time: f32, speed: f32, wrap: Wrap, fired: &mut Vec<Fired>) -> ClipPlayer {
        fired.extend((0..clip.on_apply.len()).map(|i| Fired::Apply(i as u16)));
        ClipPlayer { time: time.clamp(0.0, clip.duration.max(0.0)), speed, wrap, finished: false }
    }

    /// Fires the retract list (when the playback is removed).
    pub fn retract(&self, clip: &Clip, fired: &mut Vec<Fired>) {
        fired.extend((0..clip.on_retract.len()).map(|i| Fired::Retract(i as u16)));
    }

    /// The current time in seconds.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// The current speed (negative plays backwards).
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Sets the speed.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// The wrap mode.
    pub fn wrap(&self) -> Wrap {
        self.wrap
    }

    /// Changes the wrap mode.
    pub fn set_wrap(&mut self, wrap: Wrap) {
        self.wrap = wrap;
    }

    /// A `Once` playback reached its end.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Moves the time without firing events.
    pub fn seek(&mut self, clip: &Clip, time: f32) {
        self.time = time.clamp(0.0, clip.duration.max(0.0));
        self.finished = false;
    }

    /// Advances by `dt` seconds of game time: fires the timed events crossed, follows type-5 jumps and wraps.
    pub fn advance(&mut self, clip: &Clip, dt: f32, fired: &mut Vec<Fired>) {
        if self.finished || dt <= 0.0 {
            return;
        }
        let end = clip.duration.max(0.0);
        let mut left = dt * self.speed.abs();
        for _ in 0..MAX_SEGMENTS {
            if left <= 0.0 || self.speed == 0.0 {
                return;
            }
            let forward = self.speed > 0.0;
            let limit = if forward { end } else { 0.0 };
            let room = (limit - self.time).abs();
            let step = left.min(room);
            let from = self.time;
            let to = if forward { from + step } else { from - step };
            // Events crossed on the way, in the order they are reached.
            if let Some((at, frame, speed)) = self.cross(clip, from, to, fired) {
                left -= (at - from).abs().min(step);
                self.time = frame_time(frame).clamp(0.0, end);
                if speed != 0.0 {
                    self.speed = speed;
                }
                continue;
            }
            self.time = to;
            left -= step;
            if left <= 0.0 && step < room {
                return;
            }
            // At an end.
            match self.wrap {
                Wrap::Once => {
                    self.finished = true;
                    return;
                }
                Wrap::Hold => return,
                Wrap::Loop => {
                    if end <= 0.0 {
                        return;
                    }
                    self.time = if forward { 0.0 } else { end };
                }
                Wrap::PingPong => self.speed = -self.speed,
            }
        }
    }

    /// Fires the timed events between `from` and `to` (the end included, the start excluded), in crossing order. A
    /// type-5 event stops the crossing: returns its time and its jump.
    fn cross(&self, clip: &Clip, from: f32, to: f32, fired: &mut Vec<Fired>) -> Option<(f32, i16, f32)> {
        let forward = to >= from;
        let mut order: Vec<(f32, u16)> = clip
            .timed
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let t = frame_time(e.frame?);
                let hit = if forward { t > from && t <= to } else { t < from && t >= to };
                hit.then_some((t, i as u16))
            })
            .collect();
        order.sort_by(|a, b| if forward { a.0.total_cmp(&b.0) } else { b.0.total_cmp(&a.0) });
        for (t, i) in order {
            fired.push(Fired::Timed(i));
            if let Some((frame, speed)) = jump_of(&clip.timed[i as usize]) {
                return Some((t, frame, speed));
            }
        }
        None
    }
}

fn frame_time(frame: i16) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}

/// The bone matrices of one skinned mesh (transforms.md 4): per bone of the mesh's bone table,
/// `W_bone · W_bone,bind⁻¹ · W_mesh,bind`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinPalette {
    /// Per bone: `W_bone,bind⁻¹ · W_mesh,bind`, or `None` for a bone that did not resolve (it uses the mesh's
    /// transform).
    bind: Vec<Option<Affine>>,
    mesh_bind: Affine,
}

impl SkinPalette {
    /// From the mesh's bone table and the world matrices as loaded: `bone_bind` resolves an (object ID, transform
    /// reference) pair, `mesh_bind` is the mesh transform's world matrix.
    pub fn new(bones: &[(i32, i16)], mesh_bind: Affine, bone_bind: impl Fn(i32, i16) -> Option<Affine>) -> SkinPalette {
        let bind = bones
            .iter()
            .map(|&(o, t)| bone_bind(o, t).and_then(|b| affine_inverse(&b)).map(|inv| inv.mul(&mesh_bind)))
            .collect();
        SkinPalette { bind, mesh_bind }
    }

    /// Bones.
    pub fn len(&self) -> usize {
        self.bind.len()
    }

    /// Whether the mesh has no bones.
    pub fn is_empty(&self) -> bool {
        self.bind.is_empty()
    }

    /// This frame's matrices from the bones' current world matrices (`bone` resolves by index in the bone table;
    /// `mesh` is the mesh transform's current world matrix, used for unresolved bones).
    pub fn matrices(&self, mesh: &Affine, bone: impl Fn(usize) -> Option<Affine>, out: &mut Vec<Affine>) {
        out.clear();
        for (i, b) in self.bind.iter().enumerate() {
            out.push(match (b, bone(i)) {
                (Some(b), Some(w)) => w.mul(b),
                _ => *mesh,
            });
        }
    }

    /// The mesh transform's world matrix as loaded.
    pub fn mesh_bind(&self) -> &Affine {
        &self.mesh_bind
    }
}

/// The blended matrix of vertex `v`: the slots' matrices weighted by `w / 255` (a vertex's weights sum to 255).
pub fn blend(matrices: &[Affine], weights: &SkinWeights, v: usize) -> Option<Affine> {
    let w = weights.vertex(v);
    if w.is_empty() {
        return None;
    }
    let mut out = Affine { cols: [[0.0; 3]; 3], t: [0.0; 3] };
    let mut total = 0.0;
    for (k, &wk) in w.iter().enumerate() {
        if wk == 0 {
            continue;
        }
        let m = matrices.get(*weights.slots.get(k)? as usize)?;
        let f = wk as f32 / 255.0;
        total += f;
        for c in 0..3 {
            for r in 0..3 {
                out.cols[c][r] += m.cols[c][r] * f;
            }
            out.t[c] += m.t[c] * f;
        }
    }
    (total > 0.0).then_some(out)
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;
    use formats_sly2::options::OptionList;

    fn timed(frame: i16, kind: i8, payload: Vec<u8>) -> Event {
        Event { kind, frame: Some(frame), offset: 0, payload }
    }

    fn clip(duration: f32, timed: Vec<Event>) -> Clip {
        Clip {
            duration,
            channels: Vec::new(),
            extra: Vec::new(),
            on_apply: vec![Event { kind: 1, frame: None, offset: 0, payload: vec![0, 0] }],
            timed,
            on_retract: vec![Event { kind: 6, frame: None, offset: 0, payload: Vec::new() }],
            options: OptionList::default(),
            keys: 0,
        }
    }

    fn jump(frame: i16, to: i16, speed: f32) -> Event {
        let mut p = to.to_le_bytes().to_vec();
        p.extend_from_slice(&speed.to_le_bytes());
        timed(frame, EVENT_JUMP, p)
    }

    #[test]
    fn once_fires_crossed_events_and_finishes() {
        let c = clip(1.0, vec![timed(30, 11, vec![0; 4]), timed(15, 0, vec![0; 26])]);
        let mut f = Vec::new();
        let mut p = ClipPlayer::start(&c, 0.0, 1.0, Wrap::Once, &mut f);
        assert_eq!(f, vec![Fired::Apply(0)]);
        f.clear();
        p.advance(&c, 0.6, &mut f);
        assert_eq!(f, vec![Fired::Timed(1), Fired::Timed(0)]);
        assert!(!p.is_finished());
        f.clear();
        p.advance(&c, 0.6, &mut f);
        assert!(f.is_empty());
        assert!(p.is_finished());
        assert_eq!(p.time(), 1.0);
        p.retract(&c, &mut f);
        assert_eq!(f, vec![Fired::Retract(0)]);
    }

    #[test]
    fn explicit_loop_jumps_back() {
        // 2.5 s clip, a type-5 event on its last frame jumping back to 1.5 s (frame 90) at speed 1.
        let c = clip(2.5, vec![jump(149, 90, 1.0)]);
        let mut f = Vec::new();
        let mut p = ClipPlayer::start(&c, 0.0, 1.0, Wrap::Once, &mut f);
        for _ in 0..300 {
            p.advance(&c, 1.0 / 60.0, &mut f);
        }
        assert!(!p.is_finished());
        assert!(p.time() >= 1.5 && p.time() <= 149.0 / 60.0 + 1e-4);
        assert!(f.iter().filter(|e| matches!(e, Fired::Timed(0))).count() >= 2);
    }

    #[test]
    fn loop_and_ping_pong_wrap() {
        let c = clip(1.0, vec![timed(6, 11, vec![0; 4])]);
        let mut f = Vec::new();
        let mut p = ClipPlayer::start(&c, 0.9, 1.0, Wrap::Loop, &mut f);
        f.clear();
        p.advance(&c, 0.3, &mut f);
        assert!((p.time() - 0.2).abs() < 1e-5);
        assert_eq!(f, vec![Fired::Timed(0)]);
        let mut q = ClipPlayer::start(&c, 0.9, 1.0, Wrap::PingPong, &mut f);
        q.advance(&c, 0.3, &mut f);
        assert!((q.time() - 0.8).abs() < 1e-5);
        assert!(q.speed() < 0.0);
        let mut h = ClipPlayer::start(&c, 0.9, 1.0, Wrap::Hold, &mut f);
        h.advance(&c, 0.3, &mut f);
        assert_eq!(h.time(), 1.0);
        assert!(!h.is_finished());
    }

    #[test]
    fn skin_palette_is_the_mesh_matrix_at_bind() {
        let t = |x: f32| Affine { t: [x, 0.0, 0.0], ..Affine::IDENTITY };
        let bones = [(7, 3i16), (7, 4), (9, 1)];
        let mesh = t(5.0);
        let pal = SkinPalette::new(&bones, mesh, |o, r| (o == 7).then(|| t(r as f32)));
        let mut out = Vec::new();
        // At bind: every resolved bone gives the mesh matrix.
        pal.matrices(&mesh, |i| Some(t([3.0, 4.0, 0.0][i])), &mut out);
        assert!(out.iter().all(|m| (m.t[0] - 5.0).abs() < 1e-6));
        // Move bone 0 by +2: its matrix moves by +2.
        pal.matrices(&mesh, |i| Some(t([5.0, 4.0, 0.0][i])), &mut out);
        assert!((out[0].t[0] - 7.0).abs() < 1e-6 && (out[1].t[0] - 5.0).abs() < 1e-6);
        let w = SkinWeights { slots: vec![0, 1], weights: vec![255, 0, 128, 127] };
        assert!((blend(&out, &w, 0).unwrap().t[0] - 7.0).abs() < 1e-5);
        let b = blend(&out, &w, 1).unwrap();
        assert!((b.t[0] - (7.0 * 128.0 + 5.0 * 127.0) / 255.0).abs() < 1e-4);
        assert!(blend(&out, &w, 2).is_none());
    }
}
