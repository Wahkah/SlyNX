//! The animation system over `engine::anim`: plays segments on world objects with an [`Animator`], resolves channel
//! targets with the world's lookups (animation.md 4.3), writes the values into local transforms, turns events into
//! script events and requests (animation.md 5.3), and computes skinning matrices (animation.md 8).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::anim::Segment;
use formats::stream::Context;
use script::Value;

use super::render_scene::SkinFrame;
use super::systems::{AnimationSystem, StepInfo};
use crate::anim::playback::PlaybackId;
use crate::anim::playback::rotation_matrix;
use crate::anim::{AnimTarget, Animator, Binding, Skin};
use crate::math::{Mat3, Vec3, scale_columns};
use crate::object::Handle;
use crate::object::class::id;
use crate::world::{AnimCommand, Request, World};

/// Event types acted on here (animation.md 5.3).
mod kind {
    pub const SOUND: i8 = 0;
    pub const WIPE: i8 = 25;
    pub const WIPE_WAIT: i8 = 26;
    pub const DEFAULT_EXIT: i8 = 27;
    pub const SCRIPT: i8 = 40;
    pub const MACHINE_GOAL: i8 = 46;
    pub const MACHINE_RETRACT: i8 = 47;
    pub const EXPLOSION: i8 = 48;
    pub const LETTERBOX_ON: i8 = 49;
    pub const LETTERBOX_OFF: i8 = 50;
    pub const MOVIE: i8 = 51;
    pub const DARKEN_NOW: i8 = 57;
    pub const DARKEN_TO: i8 = 58;
    pub const DIALOGUE: i8 = 6;
    pub const CAMERA_ON: i8 = 12;
    pub const CAMERA_OFF: i8 = 13;
}

struct Play {
    id: PlaybackId,
    owner: Handle,
    tag: u32,
    /// The segment object, for segments that are objects (script events 9 to 11 are raised on it).
    segment: Option<Handle>,
    root: Option<Handle>,
}

/// Writes animated values into the world's local transforms.
struct WorldTarget<'a> {
    world: &'a mut World,
    rotation: &'a mut BTreeMap<u32, Mat3>,
    scale: &'a mut BTreeMap<u32, Vec3>,
}

impl WorldTarget<'_> {
    fn ext_rotations(&self, h: Handle) -> (Option<Mat3>, Option<Mat3>) {
        let Some(rec) = self.world.record(h) else { return (None, None) };
        let r = self.world.level.record(rec);
        match r.extension {
            Some(e) => (e.pre_rotation(&self.world.level.bytes), e.post_rotation(&self.world.level.bytes)),
            None => (None, None),
        }
    }
}

impl AnimTarget for WorldTarget<'_> {
    fn set_position(&mut self, node: u32, position: Vec3) {
        if let Some(h) = self.world.handle(node) {
            self.world.set_position(h, position);
        }
    }

    fn set_rotation(&mut self, node: u32, euler: Vec3) {
        let Some(h) = self.world.handle(node) else { return };
        let (pre, post) = self.ext_rotations(h);
        let r = rotation_matrix(euler, pre.as_ref(), post.as_ref());
        self.rotation.insert(node, r);
        let cols = match self.scale.get(&node) {
            Some(&s) => scale_columns(&r, s),
            None => r,
        };
        self.world.set_rotation(h, cols);
    }

    fn set_scale(&mut self, node: u32, scale: Vec3) {
        let Some(h) = self.world.handle(node) else { return };
        let base = match self.rotation.get(&node) {
            Some(r) => *r,
            None => {
                let r = self.world.local(h).map_or(crate::math::MAT3_IDENTITY, |l| l.cols);
                self.rotation.insert(node, r);
                r
            }
        };
        self.scale.insert(node, scale);
        self.world.set_rotation(h, scale_columns(&base, scale));
    }

    fn set_twist(&mut self, _node: u32, _twist: f32) {}

    fn set_pose_weight(&mut self, _node: u32, _pose: usize, _weight: f32) {}
}

/// The segment player.
#[derive(Default)]
pub struct SegmentPlayer {
    animator: Animator,
    lib: Vec<Segment>,
    lib_index: BTreeMap<usize, u32>,
    plays: Vec<Play>,
    events: Vec<crate::anim::playback::AnimatorEvent>,
    rotation: BTreeMap<u32, Mat3>,
    scale: BTreeMap<u32, Vec3>,
    skins: Vec<(u32, Skin)>,
    finished_now: Vec<(Handle, u32)>,
    plays_segment: BTreeMap<u32, u32>,
}

impl SegmentPlayer {
    /// An empty player.
    pub fn new() -> SegmentPlayer {
        SegmentPlayer::default()
    }

    /// Number of playing segments.
    pub fn playing(&self) -> usize {
        self.animator.len()
    }

    /// The library index of the segment body at `offset`, parsing it on first use.
    fn segment_at(&mut self, world: &World, offset: usize) -> Option<u32> {
        if let Some(&i) = self.lib_index.get(&offset) {
            return Some(i);
        }
        let seg = Segment::parse(&world.level.bytes, offset, &world.level.table).ok()?;
        let i = self.lib.len() as u32;
        self.lib.push(seg);
        self.lib_index.insert(offset, i);
        Some(i)
    }

    /// The body offset of an owned animation: the `index`-th of a warp or exit, or the `index`-th animated
    /// transition of a state machine.
    fn owned_offset(world: &World, owner: Handle, index: u32) -> Option<usize> {
        let rec = world.record(owner)?;
        let stream = &world.level.stream;
        if world.class(owner)? == id::STATE_MACHINE {
            let end = stream.records[rec as usize].subtree_end;
            return (rec + 1..end)
                .filter(|&r| {
                    let x = &stream.records[r as usize];
                    x.context == Context::StateMachine && x.parent == Some(rec)
                })
                .nth(index as usize)
                .map(|r| stream.records[r as usize].offset + 6);
        }
        stream.owned_animations.iter().filter(|a| a.owner == rec).nth(index as usize).map(|a| a.body)
    }

    /// Resolves a segment's root and channel targets (animation.md 4.3): the root is the object with the segment's
    /// root OID nearest to `context`; each channel's target is the object with its OID nearest to the root (or to
    /// `context` without a root).
    fn bind(world: &World, seg: &Segment, context: Handle) -> (Option<Handle>, Binding) {
        let root = if seg.root > 0 { world.find_nearest(seg.root, Some(context)) } else { None };
        let anchor = root.unwrap_or(context);
        let binding = Binding::resolve(seg, |oid| world.find_nearest(oid, Some(anchor)).map(|h| h.index()));
        (root, binding)
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        world: &mut World,
        offset: usize,
        owner: Handle,
        segment: Option<Handle>,
        tag: u32,
        speed: f32,
        time: Option<f32>,
        reverse: bool,
    ) {
        let Some(seg) = self.segment_at(world, offset) else {
            // Nothing to play: report it finished so the owner goes on.
            self.finished_now.push((owner, tag));
            return;
        };
        let (root, binding) = Self::bind(world, &self.lib[seg as usize], segment.unwrap_or(owner));
        let speed = if reverse { -speed.abs() } else { speed };
        let id = self.animator.play(&self.lib, seg, binding, speed, &mut self.events);
        self.plays_segment.insert(id.0, seg);
        if let Some(p) = self.animator.get_mut(id) {
            if reverse {
                let d = p.duration();
                p.seek(d);
            } else if let Some(t) = time {
                p.seek(t);
            }
        }
        if let Some(s) = segment {
            world.raise(s, script::event::SEGMENT_APPLIED, alloc::vec![Value::Int(id.0 as i32)]);
        }
        self.plays.push(Play { id, owner, tag, segment, root });
    }

    fn handle_events(&mut self, world: &mut World) {
        for e in core::mem::take(&mut self.events) {
            let Some(play) = self.plays.iter().find(|p| p.id == e.playback) else { continue };
            let Some(ev) = e.event.get(&self.lib[e.segment as usize]) else { continue };
            let anchor = play.root.unwrap_or(play.owner);
            match ev.kind {
                kind::SCRIPT => {
                    if let Some(s) = play.segment {
                        let v = ev.u16_at(0).unwrap_or(0) as i32;
                        world.raise(
                            s,
                            script::event::SEGMENT_MARKER,
                            alloc::vec![Value::Int(e.playback.0 as i32), Value::Int(v)],
                        );
                    }
                }
                kind::DEFAULT_EXIT => world.request(Request::DefaultExit),
                kind::WIPE | kind::WIPE_WAIT => {
                    // animation.md 5.3: the wipe kind, the arrival warp's OID, then a 32-byte level name.
                    let wipe = ev.i16_at(0).unwrap_or(0).clamp(0, 3) as u8;
                    let warp = ev.i16_at(2).filter(|&w| w > 0);
                    if let Some(raw) = ev.payload.get(4..36) {
                        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
                        let name: alloc::string::String = raw[..end].iter().map(|&b| b as char).collect();
                        world.request(Request::LoadWorldNamed { name, warp, wipe });
                    }
                }
                kind::DARKEN_NOW => {
                    if let Some(v) = ev.f32_at(0) {
                        world.shared.screen.darkening = v;
                        world.shared.screen.darkening_target = v;
                    }
                }
                kind::DARKEN_TO => {
                    if let Some(v) = ev.f32_at(0) {
                        world.shared.screen.darkening_target = v;
                    }
                }
                kind::LETTERBOX_ON => world.shared.screen.letterbox = true,
                kind::LETTERBOX_OFF => world.shared.screen.letterbox = false,
                kind::MOVIE => {
                    if let Some(raw) = ev.payload.get(0..32)
                        && let Ok(Some(d)) = formats::disc::Descriptor::decode(raw)
                    {
                        world.request(Request::Movie(d.identity));
                    }
                }
                kind::CAMERA_ON => {
                    world.shared.camera_placement = ev.i16_at(0).and_then(|oid| world.find_nearest(oid, Some(anchor)));
                }
                kind::CAMERA_OFF => {
                    let off = ev.i16_at(0).and_then(|oid| world.find_nearest(oid, Some(anchor)));
                    if off.is_none() || off == world.shared.camera_placement {
                        world.shared.camera_placement = None;
                    }
                }
                kind::DIALOGUE => {
                    if let Some(oid) = ev.i16_at(0) {
                        world.request(Request::Dialogue(oid));
                    }
                }
                kind::MACHINE_GOAL => {
                    // animation.md 5.3: machine OID, root OID, state OID: start a run in that state if there is
                    // none, then set its goal to it.
                    if let (Some(m), Some(st)) = (ev.i16_at(0), ev.i16_at(4))
                        && let Some(m) = world.find_nearest(m, Some(anchor))
                        && let Some(s) = world.slot(m)
                    {
                        let mut b = world.take_behavior(s);
                        if let crate::behavior::Behavior::Machine(mc) = &mut b {
                            mc.start_in(world, m, st);
                        }
                        world.put_behavior(s, b);
                    }
                }
                kind::MACHINE_RETRACT => {
                    if let Some(m) = ev.i16_at(0).and_then(|m| world.find_nearest(m, Some(anchor))) {
                        let s = world.slot(m);
                        if let Some(s) = s {
                            let mut b = world.take_behavior(s);
                            if let crate::behavior::Behavior::Machine(mc) = &mut b {
                                mc.stop(world, m);
                            }
                            world.put_behavior(s, b);
                        }
                    }
                }
                kind::EXPLOSION => {
                    if let Some(e) = ev.i16_at(0).and_then(|e| world.find_nearest(e, Some(anchor))) {
                        let at = world.position(e).unwrap_or([0.0; 3]);
                        crate::behavior::effects::set_off(world, e, at);
                    }
                }
                kind::SOUND => {
                    if let Some(sound) = ev.i16_at(0) {
                        let emitter = ev.i16_at(2).and_then(|oid| world.find_nearest(oid, Some(anchor)));
                        world.play_sound(sound as i32, emitter);
                    }
                }
                _ => {}
            }
        }
    }

    fn remove(&mut self, world: &mut World, i: usize) {
        let p = self.plays.remove(i);
        self.animator.remove(&self.lib, p.id, &mut self.events);
        if let Some(s) = p.segment {
            let root = p.root.map_or(Value::Object(None), |r| Value::Object(Some(r.to_script())));
            world.raise(s, script::event::SEGMENT_REMOVED, alloc::vec![root]);
        }
    }
}

impl SegmentPlayer {
    fn command_inner(&mut self, world: &mut World, cmd: AnimCommand) {
        match cmd {
            AnimCommand::Play { segment, owner, time, speed, tag } => {
                // A segment already playing for this owner and tag continues unless a start time is given.
                if time < 0.0
                    && self.plays.iter().any(|p| p.owner == owner && p.tag == tag && p.segment == Some(segment))
                {
                    return;
                }
                let Some(rec) = world.record(segment) else { return };
                let offset = world.level.record(rec).offset + 6;
                self.start(world, offset, owner, Some(segment), tag, speed, (time >= 0.0).then_some(time), false);
            }
            AnimCommand::PlayOwned { owner, index, reverse, speed, tag } => {
                match Self::owned_offset(world, owner, index) {
                    Some(offset) => self.start(world, offset, owner, None, tag, speed, None, reverse),
                    None => self.finished_now.push((owner, tag)),
                }
            }
            AnimCommand::Stop { owner, tag } => {
                while let Some(i) = self.plays.iter().position(|p| p.owner == owner && p.tag == tag) {
                    self.remove(world, i);
                }
                self.handle_events(world);
            }
            AnimCommand::Finish { owner, tag } => {
                for p in self.plays.iter().filter(|p| p.owner == owner && p.tag == tag) {
                    if let Some(pb) = self.animator.get_mut(p.id) {
                        let d = pb.duration();
                        pb.seek(d);
                    }
                }
            }
        }
    }
}

impl AnimationSystem for SegmentPlayer {
    fn load(&mut self, world: &mut World) {
        // Auto-play segments (option 179) are applied once the world has loaded (animation.md 5.1).
        let segs: Vec<u32> = world.slots_of_class(id::SEGMENT).to_vec();
        for s in segs {
            let Some(h) = world.handle(s) else { continue };
            if world.options(h).is_some_and(|o| o.bool_or(179, false)) {
                world.anim.push(AnimCommand::Play { segment: h, owner: h, time: 0.0, speed: 1.0, tag: u32::MAX });
            }
        }
        // Skins: every drawable whose mesh set has bones (animation.md 8.1).
        self.skins.clear();
        world.update_transforms();
        for (d, dr) in world.drawables().iter().enumerate() {
            let set = world.level.record(dr.record).mesh_set.as_ref();
            let Some(set) = set.filter(|s| !s.bones.is_empty()) else { continue };
            let bones =
                set.bones.iter().map(|&oid| world.find_nearest(oid, Some(dr.object)).map(|h| h.index())).collect();
            let mut skin = Skin::new(dr.object.index(), bones);
            skin.capture_bind(world.world_transforms());
            self.skins.push((d as u32, skin));
        }
    }

    fn command(&mut self, world: &mut World, cmd: AnimCommand) {
        self.command_inner(world, cmd);
        self.handle_events(world);
    }

    fn advance(&mut self, world: &mut World, step: &StepInfo) {
        if self.animator.is_empty() {
            return;
        }
        let mut target = WorldTarget { world, rotation: &mut self.rotation, scale: &mut self.scale };
        self.animator.update(&self.lib, step.game_dt, step.real_dt, &mut target, &mut self.events);
        // Playbacks the animator removed itself (event 8) are gone: report them.
        let alive: Vec<bool> = self.plays.iter().map(|p| self.animator.get(p.id).is_some()).collect();
        let mut k = 0;
        self.plays.retain(|p| {
            let keep = alive[k];
            k += 1;
            if !keep {
                self.finished_now.push((p.owner, p.tag));
            }
            keep
        });
        self.handle_events(world);
    }

    fn retire(&mut self, world: &mut World) {
        let mut i = 0;
        while i < self.plays.len() {
            let stopped = self.animator.get(self.plays[i].id).is_none_or(|p| p.is_stopped());
            if stopped {
                let (owner, tag) = (self.plays[i].owner, self.plays[i].tag);
                self.remove(world, i);
                self.finished_now.push((owner, tag));
            } else {
                i += 1;
            }
        }
        self.handle_events(world);
        for (owner, tag) in core::mem::take(&mut self.finished_now) {
            world.animation_finished(owner, tag);
        }
    }

    fn root_motion(&self, owner: Handle) -> Option<[f32; 3]> {
        self.plays.iter().filter(|p| p.owner == owner).find_map(|p| {
            let pb = self.animator.get(p.id)?;
            let seg = self.plays_segment.get(&p.id.0).and_then(|&s| self.lib.get(s as usize))?;
            crate::anim::root_motion_velocity(seg, pb.time, pb.speed())
        })
    }

    fn is_playing(&self, owner: Handle, tag: u32) -> bool {
        self.plays
            .iter()
            .any(|p| p.owner == owner && p.tag == tag && self.animator.get(p.id).is_some_and(|b| !b.is_stopped()))
    }

    fn skin(&self, world: &World, out: &mut SkinFrame) {
        out.clear();
        let mut tmp = Vec::new();
        for (d, skin) in &self.skins {
            skin.matrices(world.world_transforms(), &mut tmp);
            let start = out.matrices.len() as u32;
            out.matrices.extend_from_slice(&tmp);
            out.skins.push((*d, start..out.matrices.len() as u32));
        }
    }
}
