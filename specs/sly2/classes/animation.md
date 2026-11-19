---
id: sly2-classes-animation
title: Sly 2 object stream, clip and state-machine layouts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: every class-109 and class-111 body, at every depth (top level, inside solid bodies, inside state machines), parsed field by field to its declared size by a scratch walker; counts below
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), nonmatching/aseg (clip loader), nonmatching/sm (state-machine loader); Games/tools/sly2_anim.py (earlier reference-side research)
    used_for: first reading of the field order; every field re-walked on the disc
date: 2026-11-14
---

# Clip and state-machine layouts (Sly 2)

Byte layouts of the two animation classes. What the fields do at run time is in [../animation.md](../animation.md).
Notation as in [../object-stream.md](../object-stream.md).

## 1. Clip (class 109)

The body layout is the one given in [../animation.md](../animation.md) section 2, with the curve formats of section
3 there [D]. In order:

| Order | Field | Type |
|---:|---|---|
| 1 | duration | `f32` seconds |
| 2 | channel count | `s16` |
| 3 | channels | count × (`s32` target object ID, `s16` transform index, `s8` vector-curve kind and, if not −1, that curve, `s8` scalar-curve kind and, if not −1, that curve, `u8` flag) |
| 4 | extra-curve count | `u8` |
| 5 | extra curves | count × (`u8`, `s32`, `s8` scalar-curve kind, that scalar curve) |
| 6 | apply events | event list |
| 7 | timed events | event list (an `s16` frame after each type byte) |
| 8 | retract events | event list |
| 9 | options | option list |

Event lists and event payload sizes: [../animation.md](../animation.md) section 2 [D]. Clip records occur at top
level (255), inside solid bodies and inside state machines; the walk parses all of them to their declared size [D].

## 2. State machine (class 111)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | member objects | object list ([../object-stream.md](../object-stream.md) 2.1) | the clips and helpers the machine uses |
| 2 | state count `n` | `u8` | |
| 3 | states | `n` × (`s32` state ID, `s32` object ID) | the object is the state's clip or helper; −1 none [D]; 32 is a special value (see below) [R] |
| 4 | transition count `m` | `u8` | |
| 5 | transitions | `m` × (`s32` from-state, `s32` to-state, `s32` flags, `s32`, `f32`, `s32` object ID) | 24 bytes each; object ID −1 for none |
| 6 | options | option list | |

Run-time rules read from the reference [R], to be confirmed by observation:

- A state's object ID of 32 marks a state with no object of its own, distinct from −1.
- A transition whose object ID is not −1 is kept only if that object is among the member objects; otherwise it is
  dropped at load.
- Transition flag bit 0 makes the transition usable in both directions. Flag bits 2 to 12 are **groups**: for each
  group the game precomputes the number of transitions needed to go from any state to any other using only that
  group's transitions.

## 3. Sequence (class 114)

Option list only ([simple-layouts.md](simple-layouts.md) 1) [D].

## 4. Test vectors

- First clip in the walk: world row 0, body offset 200,114, 106 bytes, starting `55 55 15 3f 01 00 …`: duration
  0.5833 s, one channel [D].
- First state machine: world row 0, body offset 262,356, 579 bytes [D].
- Bodies parsed field-exactly over all worlds and depths [D]: 38,560 clips (class 109), 2,603 state machines
  (class 111) and 3,871 sequences (class 114), with nothing left over in any body.
