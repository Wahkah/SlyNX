---
id: sly1-animation
title: Sly 1 animation, skinning and poses
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds; executable SCUS_971.98 sha256
      d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      curve-kind and key-kind statistics over all 13,176 animation segments; key-time ranges; option values on
      segments and state machines; the player-character binding table read from the executable and checked
      against every world that has the player; the test vectors in section 9 (keys read from the disc, values
      computed with the formulas of this file)
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files ac, acg*, acp*, acr*, acs*, akvb, apacg, bez, math,
      aseg, asega, actseg, alo, glob, glob.vert, jt, jt.h, spaprops
    used_for: >-
      evaluation formulas, playback (repeat modes, speed, events), channel target resolution, skinning and pose
      formulas, the meaning of segment options. Every layout and range was re-checked on the disc
date: 2026-11-14
---

# Animation, skinning and poses

Animation in a world is data: **animation segments** (class 131, classes/animation-and-state.md) hold keyed curves
for a set of objects. Gameplay code and scripts start ("apply") a segment on a root object, which creates a
**playback** that advances every frame and writes local transforms, twist angles and pose weights into the target
objects. Skinned meshes then follow the moved bone objects. This file gives the formulas and the playback rules;
the byte layouts are in classes/animation-and-state.md and classes/shared-structures.md.

## 1. Time

- Key times and segment durations are stored as `s16` frame counts at **60 frames per second**: t = frames / 60.
- A playback keeps a local time t (seconds) and a speed s (local seconds per game second, default 1). Each game
  frame t ← t + dt · s, where dt is the game-clock step (gameplay-overview.md section 2).
- On the disc every curve has at least two keys, its first key at time 0 and its last key exactly at the segment's
  duration; key times are strictly increasing in all but 3 of 140,413 curves (those three have two keys at the same
  time; see 2.1). That count covers all 13,176 segments (13,092 class-131 records and 84 warp and exit
  animations): 82,262 vector Hermite curves (rotation 50,158, position 32,104) and 58,151 scalar curves (per-component
  rotation 22,173, position 6,990, scale 6,176, twist 22,812). The 11,094 pose curves are not included [D].

## 2. Scalar curves

A scalar curve (shared-structures.md) is one of five kinds. Keys are records (t_k, g_k, …).

### 2.1 Finding the segment

For a time t, use the segment between keys k and k + 1 where k is the largest index with t_k ≤ t, limited to
0 ≤ k ≤ n − 2 (so a t past the last key uses the last segment, and a t before the first key the first). Let
Δ = t_{k+1} − t_k and τ = t − t_k. Playback keeps t inside [0, duration] (section 5), so with the disc data τ stays
inside [0, Δ]. If Δ is below 0.0001 (the three degenerate curves), use g_k.

At a time equal to a key's time t_k (k < n − 1) the segment starting at that key is used, so a step takes effect
at its own key. At t equal to the last key's time (t = duration) the last segment is used (k = n − 2): if key n − 2
has outgoing kind 5, the value at the very end is still g_{n−2}, not g_{n−1}. An outgoing step on the last key
itself has no effect [R]. On the disc, of the 131 outgoing step tangents, 49 are on the second-to-last key
(so they hold through the end) and 82 on earlier keys; none is on a last key [D].

### 2.2 Kind 2: linear keys

Key: `s16` time, `f32` value. g(t) = (1 − τ/Δ) · g_k + (τ/Δ) · g_{k+1}.

### 2.3 Kind 0: Hermite keys (called Bezier keys in the layout files)

Key (16 bytes): `s16` time, `f32` value g, `s8` incoming tangent kind, `s8` outgoing tangent kind, `f32` incoming
slope m_in, `f32` outgoing slope m_out. Slopes are in value units per second.

- If the **outgoing tangent kind of key k is 5 (step)**, g(t) = g_k for the whole segment.
- Otherwise, with u = τ/Δ and v = 1 − u, the cubic Hermite curve through (t_k, g_k) and (t_{k+1}, g_{k+1}):

  g(t) = (v³ + 3uv²) · g_k + uv²Δ · m_out,k − u²vΔ · m_in,k+1 + (u³ + 3u²v) · g_{k+1}

  Its derivative (used for velocities): g′ = (6uv/Δ)(g_{k+1} − g_k) + (1 − 4u + 3u²) m_out,k + (3u² − 2u) m_in,k+1.

Tangent-kind codes on the disc: 1 (fixed) 84,375 times, 2 (linear) 36,232, 3 (flat) 151,820, 4 (smooth)
425,030, 5 (step) 131; 0, 6, 7 and 8 do not occur. The slopes are already stored, so the kind only matters for
the step rule. All other kinds are authoring hints.

### 2.4 Kind 1: weighted Bezier keys

Key (24 bytes): `s16` time, `f32` value g, `s8` incoming kind, `s8` outgoing kind, `f32` incoming handle length
a_in, `f32` incoming handle drop d_in, `f32` outgoing handle length a_out, `f32` outgoing handle rise d_out. The
handles are time/value offsets: the outgoing control point of key k is (t_k + a_out,k, g_k + d_out,k), the
incoming control point of key k + 1 is (t_{k+1} − a_in,k+1, g_{k+1} − d_in,k+1).

- Step rule as in 2.3 (outgoing kind 5 holds g_k).
- If τ < 0.0001, g = g_k; if Δ − τ < 0.0001, g = g_{k+1}.
- Otherwise the segment is a 2D cubic Bezier. In segment-relative time its control times are 0, P1 = a_out,k,
  P2 = Δ − a_in,k+1, Δ, so T(w) = 3P1·w(1 − w)² + 3P2·w²(1 − w) + Δ·w³ for w in [0, 1]. Find w with T(w) = τ by
  **16 steps of bisection** on [0, 1]: start with lo = 0, hi = 1; at step i (1 to 16) test m = lo + 2⁻ⁱ; if
  T(m) < τ set lo = m, if T(m) > τ set hi = m, if equal set lo = hi = m and stop. Then w = (lo + hi) / 2. (This exact
  procedure matters only at the last bit; any root finder accurate to 2⁻¹⁷ gives the same picture.)
- With control values G0 = g_k, G1 = g_k + d_out,k, G2 = g_{k+1} − d_in,k+1, G3 = g_{k+1}:
  g = (1 − w)³G0 + 3w(1 − w)²G1 + 3w²(1 − w)G2 + w³G3.

### 2.5 Kinds 3 and 4

Blend placeholders: no data in the stream. They mean "this value is supplied by a run-time blend" (section 6).
They occur only as twist and pose curves built at run time; on the disc no curve of kind 3 or 4 has data.

## 3. Vector curves

### 3.1 Kind 0: per-component

`u8` mask, `vec3` base value, then one scalar curve for each of x, y, z whose mask bit (bit 0 for x, 1 for y, 2
for z) is set. The value is the base vector with each animated component replaced by its scalar curve's value.

### 3.2 Kind 1: vector Hermite keys

Key (38 bytes): `s16` time, `vec3` value p, `vec3` incoming tangent v_in, `vec3` outgoing tangent v_out
(units per second). With u = τ/Δ, v = 1 − u, per component:

p(t) = (v³ + 3uv²) · p_k + uv²Δ · v_out,k − u²vΔ · v_in,k+1 + (u³ + 3u²v) · p_{k+1}

No step rule applies to vector keys.

### 3.3 Kind 2

Blend placeholder, as 2.5.

## 4. Channels and what they drive

A segment has a list of channels (classes/animation-and-state.md). Each names a **target object** by OID and
carries any of these curves. All values are **local** (relative to the target's parent), and they replace the
target's stored local values while the segment drives it:

| Channel curve | Value | Effect |
|---|---|---|
| position (vector) | local position p | the object's position in its parent's space |
| rotation (vector) | Euler angles (x, y, z) in radians | local rotation R = Rz(z) · Ry(y) · Rx(x); see 4.1 |
| scale (vector) | (sx, sy, sz) | a scale applied along the object's local axes |
| twist (scalar) | angle in radians | the twist of an inverse-kinematics chain (placed-object extension bit 5) |
| pose weights (one scalar per pose; `−1` kind = not animated) | weight | the target's mesh-set pose weights (section 8) |

### 4.1 Euler rotation

R = Rz(z) · Ry(y) · Rx(x) with right-handed axis rotations. Its stored columns (transforms.md section 1) are:

- c0 = (cos y cos z, cos y sin z, −sin y)
- c1 = (sin x sin y cos z − cos x sin z, sin x sin y sin z + cos x cos z, sin x cos y)
- c2 = (cos x sin y cos z + sin x sin z, cos x sin y sin z − sin x cos z, cos x cos y)

If the target's extension block has a pre-rotation (bit 0) and/or post-rotation (bit 1) (classes/placed-object.md
section 3), the rotation written to the object is Post · R · Pre (either factor omitted when absent). The same
post-rotation is applied to the rotation's angular velocity.

### 4.2 The root-motion channel

A channel whose flag bit 5 is set (7,013 channels on the disc) is taken out of the normal channel list. It drives
no object directly. Gameplay code reads it to move a character by the animation: the velocity of its position curve
(the derivative of 3.1/3.2 times the playback speed), turned from the character's facing frame into world space,
becomes the character's velocity while the segment plays (labels 312 and similar, section 7). At most one channel
per segment carries the bit.

### 4.3 Finding the target objects

When a segment is applied to a root object, each channel's OID is resolved in this order: an explicit override
supplied by the caller; the object with that OID nearest to the root (or to the segment itself when no root is
given; the "nearest" scope of object-stream.md 7.4); a descendant of one of the segment's search roots (each search
root is itself found by OID, nearest to the root); finally the nearest such object including objects not currently
in the world. A channel whose target is not found is skipped.

## 5. Playback

### 5.1 Segment settings (options on the segment record)

| Option | Code | Meaning | On the disc |
|---|---|---|---|
| 178 | `h` | repeat mode: 0 once, 1 loop, 2 back and forth, 3 manual | 5,810 segments set it (loop and once dominate) |
| 179 | `b` | auto-play: apply the segment once when the world has loaded | 162 segments |
| 180 | `b` | "hands off": while it plays, the player has no control | 216 |
| 181 | `i` | priority for driving shared targets (higher wins) | 1,708 |
| 182 | `f` | base speed multiplier | 1,021 |
| 183, 184 | `v`, `p` | difficulty speed: 183 holds the coefficients (g0, g1, g2) in that order (x, y, z of the vector), 184 the range (min, max). Speed factor = clamp(g0 + g1·u + g2·u², min, max), u the level's difficulty factor (pickups-and-progress.md), computed once when the world loads. It **multiplies** the speed together with 182. Defaults (1, 0, 0) and (0, 1) | 26, 8 |
| 188 | — | duration in seconds, a float that scripts can read; set from the record, never written in a stream (no stream encoding; option-encodings.md lists none) | 0 |
| 189 | `b` | runs on the real clock (keeps playing while game time is paused) | 601 |

A segment with no option 178 plays once. The playback speed set by the caller is multiplied by the base speed and by
the difficulty speed factor (183, 184).

The counts above are for the 13,092 class-131 segment records. The 84 warp and exit animations (owned animations,
object-stream.md) add option 178 once, 180 twice and 182 once [D].

### 5.2 Advancing and wrapping

Each frame, t ← t + dt · s (section 1). Then, when t leaves [0, duration D]:

| Mode | t ≥ D | t < 0 |
|---|---|---|
| once | t = D, speed becomes 0; the playback reports "reached the end" | t = 0, speed 0; reports "reached the end" |
| loop | t = t mod D; reports "wrapped" | t = t mod D (into [0, D)); reports "wrapped" |
| back and forth | with period 2D, fold t into [0, 2D); if t ≥ D, t = 2D − t and the speed changes sign | mirror: t = −t (or the same folding) and the speed changes sign |
| manual | no change; the caller sets t | no change |

A playback whose speed is effectively zero (below 0.0001 in magnitude) does not advance. A playback can also be
paused for a time; it then starts advancing again when the pause runs out.

Events 4, 5 and 7 set the speed through the same scaling: the event's speed is multiplied by the base speed and the
difficulty speed factor [R]. Event 11 sets the time to the event's own time (clamped to [0, D]) and the speed to 0;
it has no length: the playback stays still until something sets a speed again (an event, a message or the
caller) [R].

On a loop wrap, objects whose position or rotation curve does not end where it starts (the first and last values
differ by 1 unit or more, or 0.01 radians) are snapped to the curve's value at the new time instead of easing.

### 5.3 Events

Events (shared-structures.md, animation events) fire as follows:

- the "apply" list when the segment is applied;
- the framed list as the playback crosses each event's frame (in either direction, following the sign of the
  speed; after a loop wrap the scan restarts from the beginning, or from the end when playing backwards). An event
  fires when the playback time moves strictly past its time, or reaches it exactly while moving toward it. So an
  event at frame 0 does **not** fire when the segment is applied; it fires on the first update that advances the
  time (the same game frame if the update runs after the apply), and again after each forward loop wrap [R];
- the "retract" list when the playback is removed.

Event types used by gameplay and presentation (payload layouts in shared-structures.md):

| Type | Effect |
|---|---|
| 0 | play a sound effect (sound ID, emitting object OID, full-volume and cut-off distances, volume, pitch, Doppler) |
| 1 | stop that sound |
| 2, 3 | start, stop music |
| 4, 7 | seek to a frame and set the speed (7: only if a message arrives) |
| 5 | set the speed |
| 6 | show a dialogue object (OID) |
| 8 | remove this playback |
| 9, 10 | show, hide an object (OID) over a time |
| 11 | pause the playback |
| 12, 13 | enable, disable a camera placement (OID) |
| 14, 15 | enable, disable "hands off" |
| 16 | set an object's interaction limits |
| 17 to 24 | hand an object's position/rotation to, or take it back from, the physics (drive, puppet, error-function modes) |
| 25, 26 | screen wipe to a level or warp (26 waits for a button). Payload: `s16` wipe kind (0 fade, 1 keyhole, 2 world map, 3 frozen), `s16` OID of the warp to arrive at, then a 32-byte level name (empty: the current level) [R] |
| 27 | trigger the level's default exit |
| 28 | start another segment on a root (OIDs, time, speed) |
| 29 to 32 | enable, disable scale and mesh on an object |
| 33, 34 | start, stop a timer |
| 35, 36 | snap an object's position, rotation |
| 37 | start a controller rumble |
| 38 | send a user message (target OID, label) |
| 39 | **label**: a named marker (label OID) that gameplay code listens for |
| 40 | call the script event "segment event" (scripts.md section 6) with a 16-bit argument |
| 41 | waypoint (OID) |
| 42, 43, 44 | wait for, stop, preload a voice clip |
| 45 | set a pose combination on a target |
| 46, 47 | 46: `s16` machine OID, `s16` root object OID, `s16` state OID: if that machine has no run on that root, start one in that state; then set the run's goal to that state. 47: `s16` machine OID, `s16` root OID: stop that run [R] |
| 48 | trigger an explosion (OIDs and scale) |
| 49, 50 | show, hide the letterbox bars |
| 51 | play a movie |
| 52 to 54 | place, clear, set end-credit lines |
| 55, 56 | start the splash screen, the attract mode |
| 57, 58 | set the screen darkening (instant, smoothed) |
| 59 | run a script (OID) |

The exact behaviour of each event belongs to the specs of the systems they touch; this table is the map.

### 5.4 Several segments on the same object

Each driven property (position, rotation, scale, twist, each pose) of an object is driven by at most one playback
at a time: the one with the highest priority (option 181). *Inferred:* a newer playback of equal priority takes
over; to be confirmed by observation. When a
segment is applied "smoothly" (a flag of the caller), position and rotation ease from their current value toward
the curve instead of jumping; otherwise they snap. The easing law belongs to the physics spec (open question 2).
*Inferred:* when a playback ends or is removed, the next playback that drives the property (if any) takes over again.

## 6. Run-time blends

Some animation is a weighted mix of several segments built by gameplay code at run time, not stored in the
stream: for example the player's run cycle is a mix of up to six run segments chosen by speed. A blend has its own
duration; each member segment is evaluated at the same **phase** (local time / its own duration), or, when the
segments carry matching markers, at the same position between corresponding markers. The blended value is the
weighted sum Σ wᵢ · valueᵢ (positions, rotation Euler angles, twist), with the weights set by gameplay; a member
with weight 0 is skipped. Which segments each move blends, and how the weights follow the input, belongs to the
per-move specs.

## 7. How gameplay finds a character's animations

Animations are ordinary objects of the stream, children (direct or deeper) of the character they belong to, and
they are found **by OID**. The game holds, for each character type, a fixed list of (flags, OID, slot) entries and
looks each OID up below the character when the world loads:

- **Flags**: bit 0: search the whole world instead of only below the character; bit 1: optional (a missing object
  is not an error); bit 2: leave the object in the world (otherwise it is taken out and kept as a private part of
  the character); bit 3: look it up but do not keep a reference; bit 4: subscribe the character to the object's
  messages; bit 5: also find objects not in the world.
- **The player character's list** is a table in the executable at address `0x262E40` (file offset `0x163E40` in
  SCUS_971.98): 151 records of 12 bytes: `u32` flags, `u32` OID, `u32` (an internal slot; not needed). A packager
  can read it from the player's own disc. Checked on the disc: in all 41 worlds that have the player, every record
  without the optional flag (bit 1) names an object below the player character. In Paris 92 animation segments
  are found: 70 from required records and 22 from optional ones. The other required records are solid collision parts,
  effects, the tail chain, the idle state machine, the reach map and the shadow.
- The other character types (guards, bosses, vehicles) have their own lists, read the same way. Their addresses are
  an open question for this round.
- Before reading this or any other table by address, the port checks the executable's SHA-256 against the value in
  disc.md (`d4ba3ce3…796c`) and refuses on a mismatch.

What the list holds, as found below the player character in Paris, France (the OIDs are data; the moves they
belong to will be named by the per-move specs from watching the game). "opt." marks the optional flag; durations are
in 1/60 s frames:

| Kind of object found | OIDs (duration in frames) |
|---|---|
| animation segments, required | 130 (180), 131 (32), 139 (380), 140 (170), 141 (190), 142 (190), 144 (160), 145 (190), 146 to 151 (30 each), 152, 153 (60), 154 (300), 155 (60), 156 (30), 157 (20), 159 to 164 (80 each), 165, 166 (32), 167 (180), 168, 169 (150), 170 to 175 (30 each), 176 to 178 (60), 179 (50), 180 (60), 181 (38), 182 (18), 187 (60), 188, 189 (77), 226 (60), 227 (60), 228 (80), 229 (220), 230 (120), 231 (40), 232 (169), 233 (90), 234 (120), 235 (302), 236 (140), 237 (229), 238 (240), 243, 245, 247, 249 (40 each), 261 (60), 263 (40), 343, 344, 345 (40 each), 440 (120) |
| animation segments, optional, present in Paris | 190 (180), 191 (32), 221 (25), 239 (530), 241 (51), 242 (345), 251, 253, 255, 257 (40 each), 259, 260 (65), 265 (65), 266 (35), 282 (30), 283 to 285 (240 each), 339 (120), 340 (88), 341 (130), 342 (90) |
| optional, absent in Paris | 101, 109 to 113, 115 to 117, 121, 122, 132 to 135, 158, 183 to 186, 192 to 196, 222, 262, 264, 384 to 388 |
| state machine (class 136) | 325 |
| solid collision parts (class 3) | 100 (required); 102 to 108, 114, 118 to 120, 123, 124 (optional) |
| other | 125 reach map (class 139); 81, 90 chain segments (class 94); 370 dynamic shadow (class 97); 381 to 383 explosion groups and 389, 390 explosion emitters; 395 emitter; 397 proxy (flag bit 5) |

Groups of equal duration with consecutive OIDs (146 to 153, 159 to 166, 170 to 177) look like sets of directional
variants blended at run time (section 6); the per-move specs will confirm.

## 8. Skinning and poses

### 8.1 Bones

A mesh set lists B bone OIDs (geometry.md 2). Each is resolved to the object with that OID **nearest** to the
mesh set's owner (object-stream.md 7.4). Bone objects are placed objects (mostly class 79) in the owner's hierarchy.

When the world has loaded (after all objects are placed and before the first frame), the game records for each
bone b its **bind matrix**:

  K_b = W_b⁻¹ · W_mesh

where W_mesh and W_b are the 4×4 world transforms of the owner and of the bone at that moment (transforms.md
section 2). The stored vertices are in the owner's local space in this pose.

### 8.2 Per frame

For each bone, with the current world transforms:

  S_b = W_mesh⁻¹ · W_b · K_b

(identity while nothing has moved). A vertex is transformed by the weighted sum of its bone matrices:

  M = Σᵢ wᵢ · S_{bᵢ},  position′ = M · position,  normal′ = normalise(M₃ₓ₃ · normal)

over the vertex's up to four (bone, weight) pairs (geometry.md 3.1 gives how a submesh maps its local bone slots to
the set's bone list). Pairs with weight 0 are skipped. The weights are not renormalised; if they are all 0 the
vertex is left in place. The result is in the owner's local space and is then drawn with the owner's world transform
as usual.

A placed object whose extension has bit 7 set (a joint) may supply its own influence matrix in place of W_b;
this is part of the joint behaviour of a later spec.

### 8.3 Poses (morph targets)

A mesh set with P poses has P weights: the initial values in the mesh set, overridden by pose-weight sets
(placed-object layout row 14) and by animation pose curves (section 4).

A pose-weight set belongs to the object that carries it and drives that object's own mesh set. Its OID is only the
set's name; it does not point at another object. Event 45 (or option 116) selects a set by that name on a target
object; the object's weights then ease toward the set's P values, and selecting no set (OID −1) releases them [R].

Before skinning:

  position′ = position + Σ_p w_p · Δpos_p,  normal′ = normal + Σ_p w_p · Δnormal_p

where Δpos_p and Δnormal_p are the differences between pose p's position and normal and the base ones (geometry.md
section 4 gives where they are stored). Poses are applied first, skinning second.

## 9. Test vectors (Paris, France)

Keys are as stored; values were computed with the formulas above. File offsets are of the segment's record header.

**Hermite scalar, rotation component** (segment record `0x251A0B`, OID 1361, 180 frames; channel target OID 1356,
rotation, per-component with base (0, 0, 0), z animated by a kind-0 curve):

| Key | Frame | Value | In kind | Out kind | In slope | Out slope |
|---|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 0.0 | 2 | 2 | −9.42478 | 7.50489 |
| 1 | 60 | 7.50492 | 1 | 1 | 11.49073 | 11.49073 |
| 2 | 180 | 50.68658 | 1 | 1 | 40.93951 | 40.93951 |

At t = 0.5 s: Euler (0, 0, 3.25423); rotation columns (−0.99366, −0.11240, 0), (0.11240, −0.99366, 0), (0, 0, 1).

**Weighted Bezier, position component** (segment `0x265546`, OID 1383, 120 frames; target OID 1382, position z):
key 0 at frame 0: value 0.0, kinds 3/3, a_in 0.16667, d_in 0.0, a_out 0.08333, d_out 0.0; key 1 at frame 15:
value 2.85681, kinds 4/4, a_in 0.08333, d_in −0.12084, a_out 0.22222, d_out −0.32224.
At t = 0.0625 s: 0.46337. At t = 0.125 s: 1.47372. (At 0.0625 s the exact root is w = 0.25. In single precision
the bisection of 2.4 hits T(0.25) = τ exactly at step 2 and stops; carried out in double precision it never hits
equality and ends at w = 0.249992, giving 0.46334. The game's value is 0.46337.)

**Vector Hermite, position** (segment `0x27AC4A`, OID 1451, 90 frames; target OID 54, position):

| Key | Frame | Value | Incoming tangent | Outgoing tangent |
|---|---:|---|---|---|
| 0 | 0 | (−38.9039, −21.6148, 12.2804) | (−34.1395, 72.7469, −155.2521) | same |
| 1 | 25 | (−53.6977, −14.7839, −20.2897) | 0 | 0 |
| 2 | 90 | (−53.6977, −14.7839, −20.2897) | 0 | 0 |

At t = 0: (−38.9039, −21.6148, 12.2804). At t = 0.208333 s (frame 12.5): (−48.0789, −14.4105, −12.0907). At
t = 0.958333 s: (−53.6977, −14.7839, −20.2897).

**Step key** (segment `0x3B7332`, OID 239, 530 frames; target OID 1836, position x): the key at frame 180 has value
203.81694 and outgoing kind 5; the next key, at frame 181, has value 300.0. At t = 3.00833 s (frame 180.5) the value
is 203.81694.

## 10. Open questions

1. The easing law used when a segment is applied "smoothly", and the force/spring modes of events 17 to 24, belong
   to the physics and actuator spec of the next round.
2. The run-time blend weights for each move (section 6) are per-move behaviour.
3. Addresses of the other character types' lookup lists (section 7).
4. The marker mechanism used to align blended segments (section 6) is described from the reference only; no
   stream data carries it.
5. The moves that the player's animation OIDs belong to (section 7) are to be named by observation.
