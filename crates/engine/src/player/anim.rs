//! Which animation the player shows, the lookup-list OIDs (animation.md 7, player-moves.md), speeds measured from
//! root motion, and a small driver that plays the choice on a rig.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::anim::Segment;

use crate::anim::curve::eval_vector;
use crate::anim::playback::{Binding, BlendAccumulator, BlendMember, apply_blend};
use crate::anim::rig::Rig;
use crate::math::sqrt;

/// The lookup-list OIDs the player's states use. Values from player-moves.md; `None` where no spec names the
/// segment yet (to be named by observation, animation.md 10 question 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimOids {
    /// Slow locomotion (130) and fast locomotion (131), blended by the stick.
    pub run: [i16; 2],
    /// Pipe climbing (155, 156).
    pub pipe: [i16; 2],
    /// Rail balance (283 to 285).
    pub rail: [i16; 3],
    /// Flattened sidesteps (159 to 164) and flattened stand (165).
    pub flatten: [i16; 2],
    /// Duck walk (170).
    pub duck: i16,
    /// Barrel walk (181 slow, 182 fast).
    pub barrel: [i16; 2],
    /// Sweep while moving (243) and standing (251).
    pub sweep: [i16; 2],
    /// Rush (261).
    pub rush: i16,
    /// Smash (263).
    pub smash: i16,
    /// Dive (440).
    pub dive: i16,
    /// Death in a pit (237): the root drops 1920 units over 229 frames.
    pub pit_death: i16,
    /// Death in water (235): sinks, the root drops 337 units over 302 frames.
    pub water_death: i16,
    /// Death by crushing (238).
    pub crush_death: i16,
    /// The reaction with a charm left, by damage kind 0 to 6 (233, 227, 228, 229, 230, 231, 233; player-moves.md 7).
    pub hurt: [i16; 7],
    /// The reaction that follows: 226 after a blunt hit, 232 after being thrown.
    pub hurt_after: [i16; 2],
    /// Death (236), and backward (234) when moving backward faster than 250.
    pub death: i16,
    /// Backward death.
    pub death_backward: i16,
    /// Rolling ball (282).
    pub ball: i16,
    /// Decoy and mine (339 to 342).
    pub decoy: i16,
    /// Idle: 325 is the idle state machine object, not a segment (player-moves.md 8.2); standing is the run blend
    /// at u = 0, so `None` by default.
    pub idle: Option<i16>,
    /// Jump (140 standing; 141 running, 142 from a swing, pipe, water or ledge, 144 aimed, 145 boosted).
    pub jump: Option<i16>,
    /// Fall: the jump segment from its label 308 (frame 25).
    pub fall: Option<i16>,
    /// Ledge hang (139).
    pub hang: Option<i16>,
    /// Swing (343, 344, 345 by the pendulum's phase; 344 neutral).
    pub swing: Option<i16>,
    /// The jump segments by kind: standing, running, from a swing/pipe/water/ledge, aimed, boosted.
    pub jumps: [i16; 5],
}

impl Default for AnimOids {
    fn default() -> Self {
        AnimOids {
            run: [130, 131],
            pipe: [155, 156],
            rail: [283, 284, 285],
            flatten: [159, 165],
            duck: 170,
            barrel: [181, 182],
            sweep: [243, 251],
            rush: 261,
            smash: 263,
            dive: 440,
            pit_death: 237,
            water_death: 235,
            crush_death: 238,
            hurt: [233, 227, 228, 229, 230, 231, 233],
            hurt_after: [226, 232],
            death: 236,
            death_backward: 234,
            ball: 282,
            decoy: 339,
            idle: None,
            jump: Some(140),
            fall: Some(140),
            hang: Some(139),
            swing: Some(344),
            jumps: [140, 141, 142, 144, 145],
        }
    }
}

/// The animation a state asks for: one segment, or two blended (the second with weight `blend`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimRequest {
    /// First segment (OID).
    pub first: i16,
    /// Second segment and its weight, if blended.
    pub second: Option<(i16, f32)>,
    /// Whether it loops (else it holds its last frame).
    pub looped: bool,
}

impl AnimRequest {
    /// One looping segment.
    pub fn looped(oid: i16) -> AnimRequest {
        AnimRequest { first: oid, second: None, looped: true }
    }

    /// One segment played once.
    pub fn once(oid: i16) -> AnimRequest {
        AnimRequest { first: oid, second: None, looped: false }
    }

    /// Two looping segments, the second with weight `w`.
    pub fn blend(a: i16, b: i16, w: f32) -> AnimRequest {
        AnimRequest { first: a, second: Some((b, w)), looped: true }
    }
}

/// The horizontal speed of a segment's root motion: the root-motion channel's horizontal displacement over the
/// segment, divided by its duration (player-moves.md 2). `None` without a root-motion position curve.
pub fn root_motion_speed(seg: &Segment) -> Option<f32> {
    let c = seg.root_motion()?;
    let p = c.position.as_ref()?;
    let d = seg.duration();
    if d <= 0.0 {
        return None;
    }
    let (a, b) = (eval_vector(p, 0.0)?, eval_vector(p, d)?);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    Some(sqrt(dx * dx + dy * dy) / d)
}

/// Plays [`AnimRequest`]s on a rig: segments from the player's lookup list, blended at a common phase.
pub struct PlayerAnimator {
    /// The segments, by index.
    pub lib: Vec<Segment>,
    by_oid: BTreeMap<i16, usize>,
    bindings: Vec<Binding>,
    phase: f32,
    current: Option<AnimRequest>,
    scratch: Vec<BlendAccumulator>,
}

impl PlayerAnimator {
    /// A driver for `segments` (lookup-list OID, segment); `resolve` maps a channel's target OID to a rig node.
    pub fn new(segments: Vec<(i16, Segment)>, mut resolve: impl FnMut(i16) -> Option<u32>) -> PlayerAnimator {
        let mut by_oid = BTreeMap::new();
        let mut lib = Vec::with_capacity(segments.len());
        let mut bindings = Vec::with_capacity(segments.len());
        for (oid, seg) in segments {
            by_oid.insert(oid, lib.len());
            bindings.push(Binding::resolve(&seg, &mut resolve));
            lib.push(seg);
        }
        PlayerAnimator { lib, by_oid, bindings, phase: 0.0, current: None, scratch: Vec::new() }
    }

    /// The segment of a lookup OID.
    pub fn segment(&self, oid: i16) -> Option<&Segment> {
        self.by_oid.get(&oid).map(|&i| &self.lib[i])
    }

    /// The phase (0 to 1) of the current request.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Advances and writes `req` into `rig`. A different request (other segments) restarts at phase 0, as does
    /// `restart`. Segments missing from the list are skipped.
    pub fn update(&mut self, req: &AnimRequest, restart: bool, dt: f32, rig: &mut Rig) {
        let same =
            self.current.is_some_and(|c| c.first == req.first && c.second.map(|s| s.0) == req.second.map(|s| s.0));
        if !same || restart {
            self.phase = 0.0;
        }
        self.current = Some(*req);
        let w2 = req.second.map_or(0.0, |s| s.1);
        let dur = |oid: i16| self.segment(oid).map(Segment::duration);
        let d1 = dur(req.first).unwrap_or(0.0);
        let d2 = req.second.and_then(|s| dur(s.0)).unwrap_or(d1);
        let blended = (1.0 - w2) * d1 + w2 * d2;
        if blended > 0.0 {
            self.phase += dt / blended;
            self.phase = if req.looped { crate::math::rem_euclid(self.phase, 1.0) } else { self.phase.min(1.0) };
        }
        let mut members: Vec<BlendMember<'_>> = Vec::with_capacity(2);
        for (oid, w) in [(req.first, 1.0 - w2)].into_iter().chain(req.second) {
            if let Some(&i) = self.by_oid.get(&oid) {
                members.push(BlendMember { segment: &self.lib[i], binding: &self.bindings[i], weight: w });
            }
        }
        apply_blend(&members, self.phase, &mut self.scratch, rig);
    }
}
