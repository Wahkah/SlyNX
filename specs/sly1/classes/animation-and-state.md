---
id: sly1-classes-animation-and-state
title: Sly 1 object stream, animation segments and state machines
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layouts verified by full walks; event-type statistics
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files aseg, sm, ac, acgb, acgl, acgbw
    used_for: field order and meaning
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker)
    used_for: retail layout of curves and events
date: 2026-11-13
---

# Animation segments and state machines

Layouts only; playback belongs to the animation specs of a later phase.

## 1. Animation segment (class 131)

The most common record on the disc (13092). An animation of a set of objects ("channels"), each found
by OID under a root. Appears in object lists, in state-machine transitions, and inside warps and exits.

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | root OID | `s16` | |
| 2 | duration | `s16` | in frames of 1/60 s |
| 3 | search-root count R | `u16` | |
| 4 | search roots | R × `s16` OID | |
| 5 | channel count C | `s8` | |
| 6 | C channels | below | |
| 7 | events on apply | event list (shared-structures.md, animation events), not framed | |
| 8 | events per frame | event list, framed (each event has a `s16` frame number after its type byte) | |
| 9 | events on retract | event list, not framed | |
| 10 | options | option list | |

**Channel:**

| Order | Field | Present when | Type |
|---|---|---|---|
| 1 | target OID | always | `s16` |
| 2 | flags | always | `s8` |
| 3 | position curve | bit 0 | `s8` vector-curve kind, then that vector curve |
| 4 | rotation curve | bit 1 | `s8` vector-curve kind, then that vector curve |
| 5 | scale curve | bit 2 | `s8` vector-curve kind, then that vector curve |
| 6 | twist curve | bit 3 | `s8` scalar-curve kind, then that scalar curve |
| 7 | pose curves | bit 4 | `u8` count, then per pose: `s8` scalar-curve kind; if not −1, that scalar curve |

Bit 5 marks a channel to be stripped by the game; it adds no data. Curve formats are in shared-structures.md.

## 2. State machine (class 136)


| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | state count S | `u8` | |
| 2 | S states | S × (`s16` state OID, `s16` next-state OID or −1) | |
| 3 | transition count T | `u8` | |
| 4 | T transitions | T × 13 bytes: `s8` has-animation flag, `s16` from-state OID, `s16` to-state OID, `s32` flags, `f32` probability | |
| 5 | options | option list | |
| 6 | unknown | `s16` | read and not used |
| 7 | animations | for each transition in order whose has-animation flag is non-zero: a full object record (6-byte header, normally class 131, then its body) | owned by the state machine; not in the world |
