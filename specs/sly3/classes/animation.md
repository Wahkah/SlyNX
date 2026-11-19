---
id: sly3-classes-animation
title: Sly 3 clips, state machines and command scripts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every clip (42,891), state machine (2,846) and command script (6,411) parsed to its declared size
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_anim.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release animation and script loaders
    used_for: first reading of the field order and of the command argument table; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Clips, state machines and command scripts (Sly 3)

Tags as in [../disc.md](../disc.md). Clip contents: [../animation.md](../animation.md).

## 1. Clip (class 130; class 131 has the same layout and no bodies)

| Order | Field | Type |
|---:|---|---|
| 1 | options | option list (**first**, unlike Sly 2) |
| 2 | deferred flag | `s8`; non-zero: the clip's data is loaded later from elsewhere and nothing follows |
| 3 | clip data | [../animation.md](../animation.md) 2, only when the flag is 0 |

## 2. State machine (class 132)

As Sly 2 ([../../sly2/classes/animation.md](../../sly2/classes/animation.md)): an object list (the states' objects,
clips among them), `u8` count × (`s32`, `s32`), `u8` count × (4 × `s32`, `f32`, `s32`), an option list. [D]

## 3. Command script (class 135; Sly 2's sequence class 114)

An option list, then `s16` command count, then per command: `s8` command id, `s32` object ID, `s8`, `u16`, `u16`
(*interpretation:* child and sibling links), then the command's arguments. [D for the sizes]

Argument letters: `b` `s8`, `h` `s16`, `u` `u16`, `w` `s32`, `f` `f32`, `v` `vec3`, `c` 12 bytes, `e` 8 bytes, `B`
an `s8` count and that many `s32`, `P` an `s8` count and twice that many `s32`. [R; D that all 6,411 scripts parse]

| Command ids | Arguments |
|---|---|
| 0x00 | `u` |
| 0x0C, 0x53 | `B` |
| 0x0D | `Bw` |
| 0x19 | `wwwb` |
| 0x1D | `cef` |
| 0x1F | `bb` |
| 0x22 | `wbb` |
| 0x2E | `www` |
| 0x2F | `ew` |
| 0x30 | `wffwb` |
| 0x33 | `wbbfw` |
| 0x34 | `wfbfw` |
| 0x35, 0x37 | `wff` |
| 0x36 | `wbbb` |
| 0x40 | `fb` |
| 0x43 | `hwwwfff` |
| 0x45 | `vvfffuwwvw` |
| 0x46 | `wwvvfwww` |
| 0x4A | `vvf` |
| 0x4E | `ww` |
| 0x52 | `Bbwwwf` |
| 0x58 | `fw` |
| 0x5C | `wbwb` |
| 0x6B | `wwubw` |
| 0x6C | `wP` |
| 0x76 | `wbbbb` |
| 0x3E, 0x44, 0x59, 0x5A | `h` |
| 0x55, 0x56 | `hb` |
| 0x3F, 0x4F, 0x57, 0x73 | `b` |
| 0x4B, 0x5B | `f` |
| 0x41, 0x42 | `wwf` |
| 0x26, 0x38, 0x39, 0x60 | `wf` |
| 0x04, 0x07, 0x08, 0x0F, 0x10, 0x1A, 0x28, 0x2A, 0x3A to 0x3D, 0x62, 0x63, 0x68 | `ww` |
| 0x0E, 0x13, 0x1C, 0x1E, 0x6F, 0x70, 0x74 | `wb` |
| 0x16, 0x75 | `wwb` |
| 0x0B, 0x11, 0x12, 0x15, 0x17, 0x18, 0x1B, 0x20, 0x21, 0x24, 0x25, 0x29, 0x2B to 0x2D, 0x48, 0x49, 0x4D, 0x51, 0x61, 0x65 to 0x67, 0x6A, 0x71, 0x72, 0x77 | `w` |
| all other ids (and ids 0x78 and up) | none |

The meaning of each command belongs to the scripts spec (later round).
