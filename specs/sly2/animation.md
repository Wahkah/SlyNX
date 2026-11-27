---
id: sly2-formats-animation
title: Sly 2 animation - differences from Sly 1, and how characters find their clips
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every clip (38,560), state machine (2,603) and settings object (3,871) record across the 43 worlds parsed to
      its declared size with no failure; curve-kind totals; per-character clip, channel, event and transform-kind
      statistics (the three playable characters in the nightclub hub world); the four character lookup tables read
      from the executable and every entry checked against the character's objects; all test vectors
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9 (2026-11-12, GPL-3.0, read for facts only): src/aseg, src/asega, src/alo-lo,
      src/joy-jt, src/bbmark-blend, src/mpeg-memcard, src/data (character lookup tables), nonmatching/new_1a6290
    used_for: >-
      first reading of the clip layout, the curve kinds, the event record sizes, the lookup-table record format and
      flag bits, the transform kinds, the chain solver and the mirror rule; layouts re-checked by the walk above,
      behaviour marked [R]
  - kind: reference-project
    what: earlier reference-side research (Games/Sly2-RE-notes.md "Animation" and tools/sly2_anim.py, sly2_geom.py, 2026-11)
    used_for: cross-check of the layouts and of the chain solver; its posing checks (limb ends land on their goals,
      cycles return to their start pose) are cited as [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9, nonmatching/binoc-joy (chain set-up and solve), nonmatching/vec (angle between vectors)
    used_for: bend-axis rule and its degenerate cases, swivel direction (2026-11-14); the parallel-bones rule from the perpendicular-vector routine (nonmatching/vec, checked against its machine code at 0x0011E390, 2026-11-17); marked [R]
  - kind: disc
    what: re-checks of 2026-11-14 on the nightclub hub world (row 2)
    used_for: record-header offset of the run clip, channel-less clips, explicit-loop frame rule, cycle test and its margins, Euler column convention (constant rotation channels against rest matrices, columns match, the transposed form alone never does), bind bend angles of all eight chains
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 src/asega/PloRootOrAseg_file.cpp (time wrap, byte-matched); SafeHouse @ c70803ce Sly2-Release asega.cpp
    used_for: the playback-flags paragraph in section 7 (2026-11-19)
  - kind: disc
    what: >-
      SCUS-97316, whole-disc walk 2026-11-19 of every clip's extra-curve records (43 worlds); the gang's idle state
      machines in the nightclub hub (row 2)
    used_for: the extra-curve record and the transition flag values (sections 2 and 8) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/asega.cpp (extra-curve linking), cmworld.cpp, smrun.cpp (transition masks), sly.cpp (look mask)
    used_for: the extra-curve use and the mask rule (sections 2 and 8) [R]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/asega.cpp (event dispatch, playback update, retract), asegevent.cpp (event kinds 7 and 8), asega.h (clip speed)
    used_for: event types 6, 7 and 8, retract at the clip's end, option 109 (section 2, added 2026-11-20) [R]
  - kind: disc
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…6a73), machine code of 0x00130918 (the 0x38 test at 0x00130A90 before the call
      to 0x00130B18) and option-table entry 109 at 0x002AC880; level row 0 clip 150489's body and a byte search of
      row 0 for type-7 / type-8 events naming its camera objects (2026-11-20)
    used_for: the retract-at-end test and the camera-object clip events (section 2) [D]
  - kind: disc
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…6a73), machine code of 0x00107018, 0x001E04E0, 0x001E02E0, 0x001E1C40,
      0x001E3890 (jump table 0x002A7D90), 0x00131968, 0x001E5628 and 0x001E5370; level row 0 sequence 1674's
      commands (2026-11-21)
    used_for: how command 635 waits for a looping clip (section 2) [D]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (sequence record start, record-done test, sequence run loop)
    used_for: what the record fields mean (section 2) [R]
  - kind: disc
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…6a73), machine code of 0x00106DF8 / 0x001E0420 (command 631), 0x00106788
      (option 615), 0x001DFD58 (running test at 0x001DFD68) and 0x001DFEB8; level row 0 sequences 1674 and 1676,
      clip 7629, task 1583, volumes 150310, 150312 and 150314 and the modules of `jb_intro_rig` (2026-11-21)
    used_for: what stops sequence 1676's looping wait (section 2) [D]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (sequence start and stop, sequence record start)
    used_for: what record kind 6 does (section 2) [R]
reviewed_by: pending
date: 2026-11-26
---

# Sly 2 animation

What changes from Sly 1 ([../sly1/animation.md](../sly1/animation.md)), and how the game finds the clips of its three
playable characters. Notation: Sly 1 conventions; transform lists, references and the matrix convention are in
[transforms.md](transforms.md); class IDs are those of [object-stream.md](object-stream.md). Tags: [D] disc data,
[O] observable, [R] reference behaviour to be confirmed, [I] inferred.

## 1. What changed

| Topic | Sly 1 | Sly 2 |
|---|---|---|
| Clip duration | `s16` frames | `f32` seconds [D] |
| Key times | `s16` frames, per key | `u16` frames at 60 per second [D]; kinds 3 and 4 share one time list for the three axes |
| What a channel drives | an object's position, rotation, scale (separate curves) | **one transform** of a character's transform list ([transforms.md](transforms.md) 2), whose **kind** says what the vector means (section 4) [D] [R] |
| Scalar key kinds | linear, Hermite with tangent kinds and step rule, weighted Bezier | **one keyed kind, Hermite**, no tangent kinds, no step rule [D] |
| Vector key kinds | per-component, Hermite | constant, Hermite (two slopes), Hermite (one slope), per-axis Hermite (two forms) [D] |
| Root motion | a channel flagged by bit 5 | the translation of transform 1 (section 5) [D] [R] |
| Two-bone chains, mirrors | placed-object extension bits | per-object lists in the transform header ([transforms.md](transforms.md) 2) [D]; scalar channels drive a chain's swivel (section 6) [D] [R] |
| Character lookup | one table of 151 entries for the player | one table per playable character plus one shared table (section 9) [D] |

The time rule (local time t, speed s, t ← t + dt · s) is Sly 1 section 1 [R].

## 2. Clip record (class 109)

A clip is an object record of class 109 ([object-stream.md](object-stream.md)). Its body, in order (the full class
layout will be listed with the other classes in [classes/](classes/); this section is what playback needs) [D]:

| Field | Type | Notes |
|---|---|---|
| duration D | `f32` | seconds; the shortest clips are 4 or 5 frames (0.0667 or 0.0833 s) |
| channel count | `s16` | |
| channels | count × channel | below |
| extra-curve count | `u8` | |
| extra curves | count × (`u8` parameter kind, `s32` target object ID, `s8` scalar kind, scalar curve) | parameter curves (below); 151 on the whole disc, all on camera objects |
| apply events | event list | fired when the clip is applied |
| timed events | event list | each event carries an `s16` frame right after its type byte |
| retract events | event list | fired when the playback is removed |
| options | option list ([object-stream.md](object-stream.md)) | |

**Channel**: `s32` target object ID, `s16` transform index, `s8` vector-curve kind (−1 = none) followed by that
curve, `s8` scalar-curve kind (−1 = none) followed by that curve, `u8` flag. On the disc a channel has exactly one
of the two curves [D]. The flag is set on 84 of Sly's 14,306 channels (75 on translation transforms, 9 on rotation
transforms); its meaning is not established [D].

**Extra-curve record** (parameter curve; 2026-11-19) [D for the layout and counts, R for the use]. It links a scalar
curve to a numeric parameter of another object instead of a transform. When the clip is applied, the target object
is looked up (among the playing object's nested objects, else the nearest object with that ID) and asked for its
parameter of the given kind; an object that has none ignores the record. While the playback lives, the parameter
follows the curve at the playback's time and counts as in use; removing the playback releases it [R]. On the disc
every record has kind 0 and targets a camera object (class 44), whose parameter is its focal length
([camera.md](camera.md) 2.1) [D]. Sly's package holds three such records (clips 4324, 642, 4351 on camera object
4332); Bentley's and Murray's hold none [D].

**Event list**: `u8` count, then events. Each event: `s8` type, [`s16` frame in the timed list], then a payload whose
size depends on the type [D]:

| Type | Payload |
|---|---|
| 0 | `s16`, 3 × `s32`, 3 × `f32` (26 bytes) |
| 1, 2, 21 | `s16` |
| 3 | `s16` frame, `s16` |
| 4, 11, 12, 22 | `s32` |
| 5 | `s16` frame, `f32` |
| 7, 8 | `s32`, `f32` |
| 13 to 16 | `s32`, `s32` |
| 17 | 3 × `s32`, `f32` |
| 20 | `s32`, `s32`, `f32` |
| 24 | 6 × `f32` |
| all others that occur (6, 9, 10, 18, 19, 23) | nothing |

Firing follows Sly 1 section 5.3 (crossing a frame in either direction, apply and retract lists) [R]. Meanings known
so far: **type 5** jumps the playback to the given frame at the given speed; a type-5 event at the very end of a
clip is how a clip with an intro loops back into its cycle [R] [D for the placement]. **Type 11** carries an object
ID of a marker; Sly's run clip fires four of them at 0.083, 0.25, 0.333 and 0.5 s [D] (*interpretation: footfall
markers*). **Type 0** (the most frequent timed event: 331 in Sly's clips) has the layout of a sound request [I].
Other meanings belong to the gameplay specs.

Added 2026-11-20:
- **Type 7** adds the object with the given ID to the world, and **type 8** removes it. The `f32` is a fade time.
  When the clip is playing backward the two swap [R].
- **Type 6** retracts the playback [R].
- **Type 12** (one `s32` value) raises **script event 4** with that value (added 2026-11-26) [R; D for the field
  below]. The handler is looked up in the **clip's** module, that is the module of the class-109 clip object. It is
  called with the **playback** (class 134) as its first argument and the value as its second. The same path raises
  event 2 when a playback starts and event 3 when it is retracted.
  - On a playback, script ID 250 (stream option 116, a field at +0x14 holding a class-109 object) is the playback's
    **clip**.
  - So handlers find their own copy's objects with `(op115 id ((op111 playback 250)))`, a nearest search from the
    clip. For example, the pipe-room valve clips 9216 and 88367 step the nearest valve root 142138's property 1737
    by ±0.2 in `pipe_switch`'s modules 1 and 3, then raise event 5 on it (missions.md 4.3).
- A playback whose time passes the clip's end, with none of the loop or hold bits (`0x38`) set in its apply flags, is
  retracted, and retracting fires the retract list. The test is at `0x00130A90` in SCUS_973.16, followed by the call
  to the retract routine `0x00130B18` [D].
- The clip's option 109 is its playback speed: the clip's time advances at that rate times the playback's own speed
  [R].
- Camera objects are usually put in and taken out of the world this way. In level row 0 each of the eight camera
  objects is named by a type-7 and a type-8 event in a clip. For clip 150489 the type-7 event is in the apply list
  and the type-8 event in the retract list ([camera.md](camera.md) 7.2.1) [D].

**Waiting on a looping clip (added 2026-11-21).** A sequence command 635 with the no-wait flag at 0 waits until the
clip's playback has been retracted. A looping playback is never retracted at its end, so the wait **never ends on
its own**: not at once, and not after the first cycle [D unless marked].

- **The record.** Command 635 (`0x00107018`) appends a kind-10 record (`0x001E04E0`): clip, object and apply flags,
  with the no-wait flag in the record's second field [D for the layout, R for the field's meaning].
- **When the run moves past it.** The run loop (`0x001E1C40`) goes past a record only when its no-wait flag is set
  or its done test (`0x001E3890`) is true. For kind 10 (jump table `0x002A7D90`, entry 7, code at `0x001E3960`) the
  test is only "the waited playback is gone". Gone (`0x00131968`) means it is no longer in the active-playback list.
  There is no timer and no cycle count.
- **Why a loop never finishes it.** A playback with any of the bits `0x38` (loop, ping-pong, hold) set in its apply
  flags is not retracted at its end (`0x00130A90`), so it stays in the list.
- **What does end the wait.** Stopping the run that waits (`0x001E5628`), which retracts the waited playback
  (`0x001E5370` calls `0x00130B18`). A run stops when:
  - command 627 stops its thread (record kind 2, `0x001E02E0`);
  - it passes its last record;
  - the sequence or the level ends.

  The playback can also be retracted some other way, such as its own type-6 event or a script call [R].
- **Row 0's sequence 1674** [D]. Each looping clip runs in its own thread (commands 625 … 626), and the main line
  stops that thread later with 627, which retracts the loop:

| Thread | Clip | Played on | Apply flags | Stopped by the main line's |
|---|---|---|---|---|
| 152629 | 150395 | 150371 | 8 (loop) | 627 152629, after clip 150401 (flags 0, 4 s, waited) |
| 152630 | 7631 | 8 (Bentley) | 8 | 627 152630, at the same point |
| 152631 | 7627 | 8 | 8 | 627 152631, later |

  The main line's own waited clips (150401, 150336) have flags 0 and end normally.
- **For an engine.** Keep a 635 wait until the playback is retracted, and retract a looping playback when its run
  stops. Not waiting would let the thread run to its end at once, which stops it and retracts the loop too early
  [I].

**What stops sequence 1676's loop in row 0 (added 2026-11-21).** Sequence 1676 ends its wait only when the whole
sequence is stopped. In play that happens when Sly walks into trigger volume 150314 [D unless marked].

- **The sequence** [D]. A thread (625 … 626) plays loop 150340 on 150338 (apply flags 8). The main line then plays
  clip 7629 on object 8 (Bentley) with apply flags 8 and the no-wait flag 0. 7629 (3.33 s, option 109) has only
  type-0 events and no type-6 event, so it never retracts itself.
- **Two ways to stop a sequence.** Both stop every run of the named sequence, and so retract each run's waited
  playback (the stop path above):
  - **command 631**: its handler (`0x00106DF8`) appends a record of kind 6 (`0x001E0420`) naming the sequence, and
    that record stops it [D for the record, R for its effect];
  - **script ID 530** (option 615): its handler `0x00106788` calls the sequence stop `0x001DFEB8` [D].

  Command 627 is different: it stops only a thread of its own run.
- **Starting a running sequence does nothing** [D]. The start routine `0x001DFD58` returns at once if the sequence
  is running (test at `0x001DFD68`). This holds for command 636 and script IDs 235 and 638. The stop routine likewise
  does nothing to a sequence that isn't running.
- **The route** [D for the data, I for event 5's meaning].
  1. **Task 1583** (`t0_security_shutdown`, option 1272, starts with Sly) switches in volume 150310 in state 1. That
     volume is Sly only and has option 441 = 1674, so entering it starts sequence 1674.
  2. **Sequence 1674** starts 1676 with command 636 (no-wait 1) three times; the later two do nothing while it runs.
     Midway it stops 1676 with command 631. Its last commands start 1676 again and set task 1583 to state 2.
     1674 does not wait on 1676 and does not stop it when 1674 ends, so 1676 is left running with Bentley's loop
     held.
  3. **Task 1583 in states 2 and 3** switches in two Sly-only volumes of `jb_intro_rig`. Both carry option 26, so
     they are absent until then:
     - **150312**, a 700 × 500 × 500 box centred on (5106.2, 438.0, 1246.6). Its module 1 handles event 5 (Sly
       entered [I]): it calls Bentley's script ID 39, then starts 1676 (script ID 235; nothing happens while 1676
       runs).
     - **150314**, the same size, right next to it at (5106.2, 999.8, 1246.6). Its module 2 handles event 5: it
       stops 1676 with **script ID 530**, which retracts 7629 and 150340, then calls Bentley's script ID 40.
  4. Walking back into 150312 starts 1676 and the loop again, and 150314 stops it again. If Sly never enters
     150314, the loop holds until the world is left [R].

## 3. Curves

### 3.1 Vector curves (kind byte, then data) [D]

| Kind | Data | Count on the disc |
|---:|---|---:|
| 0 | `vec3` constant | 936,058 |
| 1 | `u16` n, n × (`u16` frame, `vec3` value, `vec3` incoming slope, `vec3` outgoing slope) | 95,573 |
| 2 | `u16` n, n × (`u16` frame, `vec3` value, `vec3` slope used on both sides) | 244,375 |
| 3 | `vec3` default, `u16` n, n × `u16` frame, `u8` axis mask, then for each set bit (x, y, z in order) n × (`f32` value, `f32` incoming slope, `f32` outgoing slope) | 43,020 |
| 4 | as kind 3, but n × (`f32` value, `f32` slope used on both sides) per animated axis | 195,434 |
| 5 | no data: a value supplied by a run-time blend | 0 |

For kinds 3 and 4, an axis whose mask bit is clear keeps the default's value [D].

### 3.2 Scalar curves [D]

| Kind | Data | Count on the disc |
|---:|---|---:|
| 0 | `f32` constant | 45,139 |
| 1 | `u16` n, n × (`u16` frame, `f32` value, `f32` incoming slope, `f32` outgoing slope) | 28,155 |
| 2 | no data: blend placeholder | 0 |

### 3.3 Evaluation

Every keyed curve is evaluated as Sly 1's Hermite keys: find the segment as in Sly 1 section 2.1, then apply the
Hermite formula of Sly 1 section 2.3 with the outgoing slope of the earlier key and the incoming slope of the later
key (for one-slope keys both are the same value) [R]. Slopes are in value units per second [R]. There is **no step
rule** and no weighted form [D for the absence of tangent-kind bytes]. A curve with one key returns that key's
value [R].

## 4. What a channel drives

A channel names a target object (resolved below the character, as Sly 1 section 4.3) and an index into that object's
transform list ([transforms.md](transforms.md) 2) [R]. The transform's **kind** (the `s16` stored with it) says what
the vector value means; the value **replaces** part of the transform's loaded local matrix [R]:

| Transform kind | Vector value | Effect on the local matrix |
|---:|---|---|
| 1 | translation | the translation becomes the value; the rotation part is kept |
| 6 | Euler angles (x, y, z) in radians | the rotation part becomes R = Rz(z) · Ry(y) · Rx(x) exactly as in Sly 1 section 4.1: the three stored axis vectors of the transform are the **columns** of R, that is, vector i is R applied to axis i ([transforms.md](transforms.md) 1) [D]; the translation is kept |
| 12 | scale (sx, sy, sz) | the three diagonal entries of the rotation part become sx, sy, sz; the six off-diagonal entries and the translation **keep their loaded values** [R]. Every kind-12 transform of the three characters has off-diagonal entries of exactly 0 in its loaded matrix (Sly 34, Murray 39, Bentley 15 transforms, nightclub hub) [D], so there the result is a pure scale |
| 0, 13 | — | not animated by vector channels |

On the disc, vector channels of the three playable characters target only kinds 1, 6 and 12 (Sly: 5,063, 5,621 and
2,687 channels) [D]. Transform kind 11 is a shear in the reference [R] and does not occur on the disc as a target.
Sly's own transform list has kinds 0 (28), 1 (37), 6 (56), 12 (17) and 13 (1) [D].

**Scalar channels** target only the **upper bone of a two-bone chain** (section 6): 935 of Sly's, 258 of Bentley's
and 332 of Murray's scalar channels, and no other transforms [D]. The value is the chain's swivel angle in radians
[R].

## 5. Root motion

A character's transform 0 is its placement (kind 0, no parent) and transform 1 (kind 1, child of 0) carries the body
height: Sly's transform 1 is at (0, 0, 75) [D]. Locomotion clips animate transform 1's translation, and that motion
moves the character: in Sly's run clip (object 126, 0.5 s) transform 1 goes from (0, 0, 75) to (320, 0, 75) [D]
(*the game turns this into the character's velocity, as Sly 1 section 4.2 does for its flagged channel* [R]). A
viewer that plays clips in place removes this translation [I].

## 6. Two-bone chains and mirror pairs

Both lists are part of the transform header ([transforms.md](transforms.md) 2) [D].

**Two-bone chain** (an arm or a leg): references to the upper, lower and end transforms, an effector offset (`vec3`),
a handle transform reference and a target offset (`vec3`) [D]. Sly has 4 chains, Bentley 2 and Murray 2 [D]. Per
frame, after the vector channels [R]:

1. At load, record the upper length a (upper to lower) and lower length b (lower to effector), and the bind bend
   angle and bend axis of the lower bone in the upper bone's frame. The bend angle is the angle between the vectors
   u = upper→lower and v = lower→effector; the bend axis is u × v normalised, then expressed in the upper bone's
   frame [R]. Degenerate cases [R]: if u or v is shorter than 0.0001, the angle is 0 and the axis is (1, 0, 0) (the
   constant at virtual address `0x002A2DA0`, read after the executable hash check [D]); if u and v are parallel
   (|u × v|² ≤ 1e−8) the axis is a unit vector perpendicular to u built from u's **smallest** component [R]
   (routine at virtual address `0x0011E390`, read in the executable after the hash check):
   - choose a coordinate axis e: if |x| ≤ |y| then e = X when |x| ≤ |z|, otherwise e = Z; if |x| > |y| then e = Y
     when |y| ≤ |z|, otherwise e = Z;
   - p = u × e for e = X or Z, but p = e × u for e = Y;
   - axis = normalise(u × p).
   Worked values: u = (0, 0, 1) → (−1, 0, 0); u = (0, 1, 0) → (−1, 0, 0); u = (1, 0, 0) → (0, 1, 0) [I, computed
   from the rule]. A rule such as "u × X, or u × Y when u lies along X" gives a different axis (for u = (0, 0, 1) it
   gives (0, 1, 0)), so it only matches the game when the parallel case never arises, which is true at load for all
   eight playable-character chains (below).
   No chain of the three playable characters is straight at load: their bind bend angles are 21.9°, 21.6°, 42.2°
   and 40.8° (Sly), 63.2° and 53.1° (Bentley), 69.9° and 39.7° (Murray) [D], so the parallel case never arises
   for them.
2. Each frame, reset the upper and lower bones to their bind rotations. The goal is the handle transform's world
   position plus the target offset in the handle's frame.
3. With d the distance from the upper bone to the goal, bend the lower bone about the bend axis by
   acos((d² − a² − b²) / 2ab) minus the bind bend angle.
4. Rotate the upper bone so the effector points at the goal, then spin it about the upper-to-goal axis by the swivel
   angle (the scalar channel of section 4). The spin is right-handed about the unit vector from the upper bone to the
   goal, measured in the upper bone's parent frame: a positive swivel turns the limb counter-clockwise as seen from
   the goal looking back toward the upper bone [R].

The arm and leg bones themselves carry no vector channels in the earlier research's sample: limbs are driven only
through their handles and swivels [R].

**Mirror pair** (`s16` source transform, `s16` target transform, `s8` copy-position flag, `s8` copy-rotation flag):
after the channels, the target's world rotation (and position when its flag is set) is set equal to the source's
[R]. Each playable character has one pair, with only the rotation copied: Sly (10 → 73), Bentley (75 → 52),
Murray (11 → 84) [D]; *it points the head where a look-at transform points* [R].

Order each frame: vector channels → mirror pairs → chains [R].

## 7. Looping and playback modes

How a playback wraps (loop, back and forth, clamp) is chosen by the game's code, not stored in the clip [R]. Two
signals exist in the data [D]:

- **explicit loop**: a timed type-5 event at the end of the clip that jumps back to a loop start (Sly: 25 clips,
  e.g. object 144, 2.5 s, back to 1.5 s). "At the end" means the event's frame f satisfies f ≥ 60 × duration − 1,
  that is the clip's last frame or the one before it [D]: with that rule the three characters have 25, 10 and 8
  such clips; requiring exactly the last frame gives 21, 8 and 8;
- **cycles**: clips whose channels end where they start: 136 of Sly's, 108 of Bentley's, 109 of Murray's [D]; the
  game wraps these in code [R]. The exact test behind these counts [D]:
  - evaluate every channel at t = 0 and at t = duration; a clip of duration 0 is never a cycle;
  - consider only channels whose target transform belongs to the character's own objects (its body and the solids
    nested in it at any depth); skip channels with no curve;
  - skip root-motion channels: vector channels on a transform of kind 1, 4, 5, 9 or 10 whose parent is transform 0;
  - compare each component; for vector channels on transforms of kind 6 or 7, and for every scalar (swivel) channel,
    take the difference modulo 2π into −π…π first;
  - the clip is a cycle when every difference is at most 0.01 in absolute value.

  Margins, for reconciling implementations: among Sly's 136, the largest differences are 0.0079 (object 316,
  transform 1, a kind-6 rotation channel that turns by 12.5743 rad, two full turns plus 0.0079, in clip 4532),
  0.0059 (clip 141, swivel of transform 55) and 0.0051 (clip 621, translation of transform 70). Clip 4532 counts
  only when kind-6 channels of the nested solids are compared modulo 2π; an implementation that does not wrap those,
  or that only looks at the body object's own transforms, finds 135 [I].

**Playback flags.** When a script applies a clip (option 106) it passes a flags value that chooses the mode: 8 loop,
16 back and forth, 32 hold at the end; with none of these the playback stops at the end and is removed. Values 1, 2
and 4 choose easing, start events and snapping. Full table, the seek method (option 123) and the state-machine snap
(option 379): options-by-class.md 2.4 [R].

Several clips on one character, blending and priorities follow Sly 1 sections 5.4 and 6 [R]; blend-placeholder
curves (vector kind 5, scalar kind 2) do not occur on the disc [D].

## 8. Where clips live

A character's clips are nested objects of its own solid body (class 109 records in its nested object list,
[classes/solid-object.md](classes/solid-object.md)) and of the **state machines** nested there (class 111) [D].
A state machine's body is: a nested object list (where most clips sit), a `u8` count of states (`s32`, `s32`
object ID each), a `u8` count of transitions (`s32`, `s32`, `s32`, `s32`, `f32`, `s32` each) and an option list
[D]. No clip carries a name; clips are identified by object ID [D].

**Transition flags and the mask** (2026-11-19) [D for the values, R for the rules]. The third `s32` of a transition
is its flags word. Bit 0 (`0x1`) makes it usable in both directions, as in Sly 1. Bits 2 to 15 (`0x4` to `0x8000`) are
**mask bits**: a transition may be taken only when its flags AND the machine's current mask is non-zero; the
route-distance groups are built from bits 2 to 12 (`0x4` to `0x1000`) only. Bit 1 is not a mask bit. The mask is
set by gameplay when the machine is applied and may be changed while it runs; the gang's idle masks are in
[characters.md](characters.md) 9.2. Values seen in the idle machines: `0xFFFC` (any mask), `0xFFFD` (any mask, two
ways), `0x8`, `0x10`–`0x80`, `0xF0`, `0xF00`, `0xFF0`, `0x1000`, `0x1FF0` [D]. The fourth `s32` is 0 in those
machines, the `f32` is the probability weight and the last `s32` the transition's clip (−1 none) [D].

## 9. How the game finds a character's clips

As in Sly 1, gameplay code looks up objects **by object ID** below the character at load time, using fixed tables
in the executable [R]. Sly 2 has one table per playable character and one table shared by all three [D]:

| Character (class) | Table address(es) | Records |
|---|---|---:|
| Sly (53) | `0x002BE990`, then `0x002BF1B0` | 173, then 18 |
| Bentley (55) | `0x002BC390` | 118 |
| Murray (54) | `0x002C0270` | 137 |
| all three | `0x002BC9F0` | 53 |

Virtual addresses; file offset = address − `0x100000` + `0x80` in SCUS_973.16 ([disc.md](disc.md)). A packager reads
these tables from the player's executable after checking its SHA-256 and refuses on a mismatch.

**Record** (12 bytes): `u32` flags, `u32` key, `u32` an internal slot (not needed) [D]. Flags [R]:

| Bits | Lookup |
|---|---|
| `0x10` set | key is a **transform ID** (the `s32` ID stored with each transform); search the character's own transform list |
| `0x20` or `0x40` set | a part or mesh of the character (key not used as an object ID); 22 Sly, 20 Bentley and 26 Murray records |
| otherwise | key is an **object ID**, searched near the character; bit `0x2` and bit `0x1` widen the search scope; bit `0x400` adds a search option |
| `0x200` | after finding the object, run its post-load set-up |
| `0x100` | set on most records; meaning not established |

A record whose object is not found is skipped; the game does not fail [R]. Checked on the nightclub hub world
(which holds all three characters) [D]:

| Table | Object-ID records that resolve | to clips (109) | to state machines (111) | other classes found | not found in this world | transform-ID records found |
|---|---:|---:|---:|---|---:|---:|
| Sly `0x002BE990` | 143 | 129 | 4 | 6 × class 6, 3 × class 33, 1 × class 29 | 6 | 2 of 2 |
| Sly `0x002BF1B0` | 9 | 2 | 1 | 6 × solid (48) or class 15 | 0 | — |
| Bentley `0x002BC390` | 81 | 68 | 1 | 9 × class 6, class 112, class 3, class 33 | 17 | — |
| Murray `0x002C0270` | 94 | 83 | 4 | 3 × solid (48), 2 × class 6, classes 105, 3 | 17 | — |
| shared, for Sly | 41 | 29 | 3 | classes 15, 105, 117, 29, 3, 48 | 9 | 2 of 2 |
| shared, for Bentley | 35 | 26 | 3 | classes 105, 117, 29, 3, 48 | 15 | 2 of 2 |
| shared, for Murray | 18 | 10 | 2 | classes 105, 117, 29, 3, 48 | 32 | 1 of 2 |

The same object IDs are used by different characters: each looks them up in its own package, so for example key
416 is a different clip of Bentley's and of Murray's [D]. Which move each clip belongs to is for the per-move specs
to name by observation.

## 10. Skinning

Skin binding and bind poses are in [geometry.md](geometry.md) and [transforms.md](transforms.md) 4. Clips carry no
pose-weight curves like Sly 1 section 4's; the extra-curve records are parameter curves for camera lenses (section
2), not pose weights [D].

## 11. Test vectors (nightclub hub world: level-table row 2, `f_nightclub_exterior`, file `00_0x041` in the world list)

| Character | Body object | Transforms | Chains | Clips | With explicit loop | Cycles | Total length | Keys in keyed curves | Durations min / median / max |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| Sly | class 53, object 7 | 139 | 4 | 294 | 25 | 136 | 449.42 s | 30,419 | 0.0833 / 1.0 / 9.333 s |
| Bentley | class 55, object 8 | 76 | 2 | 174 | 10 | 108 | 166.33 s | 11,872 | 0.0833 / 0.5 / 8.333 s |
| Murray | class 54, object 9 | 145 | 2 | 198 | 8 | 109 | 243.73 s | 28,090 | 0.0667 / 0.667 / 9.167 s |

("Keys" counts the time entries of keyed vector and scalar curves.)

**What "Clips" counts** [D]: clip records (classes 109 and 110) in the character's package that have at least one
channel on the character's objects. Each package also holds a few clips with **no channels at all** (events only):
Sly 2 (objects 344, 1.0 s, and 5518, 0.25 s), Bentley 1 (object 4538, 0.8333 s), Murray 5 (object 342, 1.0 s, and
objects 17800 to 17803, 0 s each). They carry events, and lookup entries and state machines can name them like any other clip [I]. Counting them gives 296, 175 and 203 clips and total lengths 450.67 s, 167.17 s and
244.73 s; Murray's minimum duration becomes 0 s. The other columns of the table do not change.

**Sly's run clip** (object 126; its 13-byte record header `00 6d 00 7e 00 00 00 ff ff 43 1a 00 00` is at decompressed offset `0x8013DE`, its body at `0x8013EB`, 6,723 bytes): duration 0.5 s,
72 channels; event lists of 1, 4 and 1 events; timed events all type 11 with keys 383, 384, 381, 382 at frames 5,
15, 20, 30. Channel values (evaluate with section 3.3):

| Channel | Curve | t = 0 | t = 0.025 | t = 0.25 | t = 0.5 |
|---|---|---|---|---|---|
| object 7, transform 1 (kind 1, root) | vector kind 4 | (0, 0, 75) | (16.0044, 0, 75) | (160, 0, 75) | (320, 0, 75) |
| object 7, transform 10 (kind 6) | vector kind 2, 2 keys | (1.6423, 4.8024, −0.0568) | (1.6423, 4.8036, −0.0591) | (1.6414, 4.8131, −0.0689) | (1.6423, 4.8024, −0.0568) |
| object 7, transform 12 (kind 1) | vector kind 1 | (55.7351, −13.2539, −15.3383) | (64.6980, −15.1424, −4.3464) | (−75.7771, −18.0971, −50.7465) | (55.7351, −13.2539, −15.3383) |

Transform 12's first key: frame 0, value (55.73511, −13.25388, −15.33829), incoming slope (1335.1135, −257.7299,
416.3185), outgoing slope (242.7754, −75.5506, 256.9352). Transform 10's key 0: frame 0, value (1.642331, 4.802381,
−0.056751), slope (0, 0.047374, −0.097288); key 1 at frame 30.

**Scalar (swivel) channel** in the same clip: object 7, transform 55 (Sly's first chain's upper bone), kind 1, keys
at frames 0, 10, 25, …; key 0 (−0.339796, −1.821102, 0.186607), key 1 (−0.308700, 0.186622, 1.122883); value at
t = 0.25 s: −0.21512.

Sly's chains: (upper 55, lower 56, end 56, handle 15), (57, 58, 58, 21), (68, 69, 69, 29), (65, 66, 66, 36).

## 12. Open questions

1. The channel flag byte and the meanings of most event types (the extra-curve records are resolved: section 2).
2. Flag bits `0x100`, `0x1`, `0x2`, `0x400` of the lookup records, and the part/mesh lookups (`0x20`, `0x40`).
3. Which moves the looked-up clips belong to (observation, per-move specs).
4. Whether lookups that fail in the hub world resolve in other worlds (only the hub was checked entry by entry).
5. The order of mirror pairs and chains relative to follower chains ([transforms.md](transforms.md) 2), which
   this spec does not cover.
