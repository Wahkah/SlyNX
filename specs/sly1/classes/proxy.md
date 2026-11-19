---
id: sly1-classes-proxy
title: Sly 1 object stream, proxy (instancer) layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layout verified by full walks; source/back-reference accounting; source-count statistics
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files proxy, sw, alo
    used_for: field meaning, the clone pool, how sources and children are placed and re-parented
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker)
    used_for: retail layout
date: 2026-11-13
---

# Proxy (class 83)

A proxy (class 83) places one or more objects (its **sources**) at its own transform. A source is either a full
object record written inside the proxy, or a back-reference to a source written in an earlier proxy, in which case
the proxy gets a copy (clone) of that earlier source. Proxies are the most common class after animation segments:
7925 records on the disc, 5531 of them top-level. Guards, the player character, pickups, breakables and much of the
small scenery are placed through proxies.

The body has a layout of its own; it does not contain the placed layout.

## 1. Layout

After the 6-byte object header:

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | rotation | `mat3` | relative to the proxy's parent |
| 2 | position | `vec3` | relative to the proxy's parent |
| 3 | source count N | `u8` | 1 to 11 on the disc (7620 proxies have 1, 277 have 2, the rest 3 to 11) |
| 4 | N sources | source entries (1.1) | |
| 5 | override count M | `u8` | |
| 6 | M overrides | override entries (1.2) | |
| 7 | children | object list | **only if N = 1**; absent otherwise |

### 1.1 Source entry

| Field | Type | Notes |
|---|---|---|
| class ID | `s16` | −1 for a back-reference |
| if −1: source index | `s16` | position of an earlier source in the world-wide source list (1.3) |
| else: OID, script index | `s16`, `s16` | the rest of a normal object header |
| else: body | the class's layout | a complete object record body, including any children |
| else: use count | `s16` | how many proxies in the world place this source, counting this one (1.3) |

### 1.2 Override entry

Applied after the sources are loaded, to one object found among the sources or their descendants.

| Field | Type | Notes |
|---|---|---|
| target OID | `s16` | the first source with this OID, or the first object with this OID below a source |
| script index | `s16` | −1 for none; otherwise the target is attached to that script |
| zone mask | `u32` | replaces the target's zone mask when the target is a placed object |
| distance mode | `s8` | if 2: the target gets whole-object distance culling and a `f32` follows |
| maximum render distance | `f32` | only if the mode byte is 2; `FLT_MAX` means 1e10 |
| options | option list | applied to the target |

### 1.3 Source list and clones

- The world keeps one list of sources in stream order. Every source written in full (class ID ≠ −1) appends one
  entry, wherever the proxy is in the hierarchy.
- The use count says how many proxies place that source in total. When the source is loaded, the game makes
  (use count − 1) clones of it, including its descendants. Each later back-reference to that source takes one
  unused clone.
- On the disc, every one of the 2889 full sources has use count = 1 + the number of later back-references to it,
  except one (a source in "The King of the Hill" with use count 2 and no back-reference; the spare clone is never
  placed).
- A back-reference always points to an earlier entry.
- For rendering, a back-reference is a full copy of the referenced source's subtree, placed at this proxy's
  transform. World test vectors count these copies separately (worlds.md).

## 2. Placement

1. The proxy's world transform is formed from its own rotation and position and its parent's world transform
   (transforms.md).
2. Each source's stored transform (the local rotation and position in its body) is relative to the proxy. The
   source's world transform is the proxy's world transform composed with that local transform. A clone uses the
   stored transform of the original source.
3. The source is then moved under the proxy's parent, keeping its world transform. Its own children follow it
   unchanged.
4. If N = 1, the children list (row 7) is read with the proxy as parent; each child is then moved under the single
   source, keeping its world transform. Together with step 3 this means: child world = proxy world × child local.
5. The proxy itself has no model and draws nothing. It stays in the world as a named anchor: its OID is used to find
   the objects it placed.

Transforms on the disc's proxies are plain rotations (orthonormal) with translations.

## 3. What a clone contains

- The pool of clones for a source is made right after the source's body has been read completely: after its own
  child list, but before the rest of the proxy that holds it (later sources, the overrides, and the proxy's own
  child list of section 1, row 7). So a clone never contains the children listed after the source's own proxy.
  Each proxy that back-references the source carries its own child list instead, which is placed under its clone
  in the same way (section 2, step 4).
- By then every proxy inside the source's body has finished its own placement. So the source's subtree holds its
  child-list descendants, the full sources and back-reference clones placed by proxies inside it (now children of
  those proxies' parents), and the children of single-source proxies inside it (now children of those proxies'
  sources).
- A clone is a copy of that whole subtree, with each object's stored transform. It does not re-read the stream and
  does not take clones from any other pool: a back-reference nested inside a source is resolved once, when the
  source is loaded, and the outer source's clones carry copies of the result.
- Placing a clone (a back-reference) applies section 2 to the clone's root; its descendants follow.

Checked on the disc: in Back Alley Heist the source at record index 1077 (class 9, OID 1991) sits in a
single-source proxy (OID 1990) whose child list holds one flash volume (class 96, OID 2024). The source is
back-referenced by two proxies (OIDs 2025 and 2026), and each of them lists its own copy of that flash volume (OID
2024) in its own child list. Had the clones carried the original's child, each guard copy would end up with two.
worlds.md section 5 gives counts that follow this rule (384 drawable objects in clones for Back Alley Heist).

## 4. Clarifications

- **A proxy parked at `FLT_MAX`.** One proxy on the disc (A Grave Undertaking, record at 0x511D19, OID 2274, a
  top-level proxy whose only source is a back-reference) has position (0, 0, `FLT_MAX`) and identity rotation.
  Its clone is placed outside any reachable space. Treat any object whose world position has a component of
  magnitude above 1e30 as **not placed**: do not draw it or its descendants, and do not let the value reach the GPU.
  Whether a script moves it later belongs to the gameplay specs.
