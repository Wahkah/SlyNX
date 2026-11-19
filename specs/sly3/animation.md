---
id: sly3-formats-animation
title: Sly 3 animation - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97464 image, all 39 worlds; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: every clip (42,891), state machine and command script parsed to its declared size; the tangent multipliers read at their address
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_anim.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release clip loader port
    used_for: the track kinds, the packed-value and tangent scales, the key search and the event payloads; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 animation: differences from Sly 2

Read [../sly2/animation.md](../sly2/animation.md) first. Clips (class 130), state machines (132) and command scripts
(135) are laid out in [classes/animation.md](classes/animation.md). Tags as in [disc.md](disc.md).

## 1. What stays the same

- Channels drive transforms of kind 1 (translation), 6 (Euler rotation) and 12 (scale); scalar channels drive twists.
  The local matrix is rebuilt as in Sly 2. [R]
- The segment between keys is Sly 2's cubic Hermite (out tangent of the earlier key, in tangent of the later one). [R]
- Two-bone chains and mirror pairs of the transform list ([transforms.md](transforms.md)) are Sly 2's limbs and
  mirrors; Sly has 4 limbs and one mirror (transform 6 → 73, rotation), as in Sly 2. [D]
- Root motion on transform 1 (the first child of transform 0). [R]
- Clips live in the character's own tree (its nested objects and state machines). [D]

## 2. Clip data (new layout)

After the clip's option list and a zero deferred flag ([classes/animation.md](classes/animation.md)) [D]:

| Order | Field | Type |
|---:|---|---|
| 1 | frame count | `u16` (duration = frames / 60 s) |
| 2 | time-list count `T` | `u16` |
| 3 | pool size | `u16` |
| 4 | `T` time lists | each `s8` wide flag, `u8` count `c`, then `c` frame numbers (`u16` if wide, else `u8`): **shared key times** |
| 5 | channel count | `s16` |
| 6 | channels | each: `s32` (low 20 bits, sign-extended: the target object ID), `s16` (low 10 bits, sign-extended: the transform), `s8` vector-track kind and the track (2.1), `s8` scalar-track kind (−1 none) and the track (2.2), `u8` flags |
| 7 | parts | `u8` count, each `u8` kind, `s32`, `s8` scalar-track kind and the track |
| 8 | events | three lists as Sly 2 (apply, framed, retract); each `u8` count, per event `s8` kind, (framed list only) `u16` frame, then the kind's payload (2.3) |

When a channel has both tracks, the scalar track is the one used. [R]

### 2.1 Vector tracks

| Kind | Data | Value |
|---:|---|---|
| −1 | none | no track |
| 0 | none | (0, 0, 0) |
| 1 | none | (1, 1, 1) |
| 2 | `vec3` | constant |
| 3 | `s8` scale index, 3 × `s16` | constant, each `s16` × scale |
| 4, 5 | `u32` size, `u16` time list, `u16`, `u8` keyed-axis mask; then one `f32` per unkeyed axis; then per key, per keyed axis: (in tangent, out tangent, value) for kind 4, (tangent, value) for kind 5 | Hermite |
| 6, 7 | `u32` size, `u16` time list, `u16`, `s8` keyed-axis mask, `s8` scale index; then one `s16` per unkeyed axis; then per key: (`u16` in tangent, `u16` out tangent) for kind 6 or one `u16` tangent for kind 7, then one `s16` per keyed axis | Hermite, packed |

[D for the sizes] The size field counts the bytes after the mask byte (kinds 4, 5) or after the scale index (6, 7);
it matches on every keyed track. [D] Keys sit at the frames of the named time list.

**Packed values** [R; D for the use]: `s16` × scale, scale = 1/16 (index 0, translations), 1/1024 (indices 1 and 2,
rotations and scales).

**Packed tangents** [R]: each axis takes 5 bits of the `u16` (axis x bits 0..4, y 5..9, z 10..14): bits 0..3 an index
`i`, bit 4 the sign; tangent = ±tan(i·π/32) × multiplier, multiplier 60, 10 or 10 by the scale index. The three
multipliers are the floats at virtual address `0x0031F690` (60.0, 10.0, 10.0). [X] Tangents come out in units per
second.

### 2.2 Scalar tracks

| Kind | Data |
|---:|---|
| 0 | `f32` constant |
| 1 | `u16` time list, `u16`, then per key `f32` value, `f32` in tangent, `f32` out tangent |
| 2 | none |

### 2.3 Event payloads

As Sly 2 except [R; D that every clip parses]: kind 2 is (`s16`, `s32`); kind 3 is (`u16` frames, `s16`, `s32`);
kind 17 is (`s32`, `s32`, `f32`); new kinds 25 and 26 carry one `s32`. Kind 5 (seek: `u16` frames, `f32` speed) still
marks a looping clip. The full payload table: kind 0 `s16`, 3 × `s32`, 3 × `f32`; 1 and 21 `s16`; 4, 11, 12, 22, 25,
26 `s32`; 7, 8 `s32`, `f32`; 13 to 16 two `s32`; 20 `s32`, `s32`, `f32`; 24 three 8-byte values; others nothing.

## 3. Evaluation

**Key search** [R]: in whole frames: f = floor(t × 60); the segment ends at the first key index `i` in [1, n − 1)
whose frame is greater than f (else `n − 1`); it starts at `i − 1`. Then Sly 2's Hermite between the two keys.

## 4. Deferred clips

468 clips (13 in Sly's group in each of 36 worlds) have a non-zero deferred flag: their data is not in the world
file; the game loads it later from another stream. [R, from the earlier research's census; D that such clips end
right after the flag] A port can ignore them until
their source is found.

## 5. Counts

42,891 clips (174 at top level), 2,846 state machines, 6,411 command scripts. [D] Vector track kinds: 3 (681k),
7 (389k), 1 (296k), 6 (133k), 0 (129k), 4 (7k), 5 (4k), 2 (704); scalar kinds 0 (50k), 1 (25k). [R, from the earlier
research's census] Clips per character group: Sly 342, Murray 250, Bentley 206, Carmelita 134, doberman guard
90. These count the clips of the character's group that have data and drive at least one transform of the
character's tree; deferred clips and event-only clips are not counted (Sly's group: 363 records, 13 deferred, 8
event-only). The figures are the same in every world checked; definition and more counts in
[characters.md](characters.md) 4. [D]

## 6. Open questions

- Where deferred clips come from.
- The parts list (`u8` kind, `s32`, scalar track) and the channel flags.
- How each character's moves find their clips (Sly 2 section 7's lookup lists for Sly 3).
