---
id: sly1-object-stream
title: Sly 1 world file, object stream
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly Cooper and the Thievius Raccoonus, USA, SCUS-97198, disc image sha256
      b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a; executable SCUS_971.98 sha256
      d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c; all 46 world files read through the
      executable's level table and decompressed (disc.md, world-file.md)
    used_for: >-
      every layout in this file was checked by walking all 46 worlds from the first byte to the start of the
      texture block with a throwaway parser; counts, offsets and histograms in worlds.md come from that walk
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files brx, sw, lo, alo, so, proxy, find, splicemap, serialize
    used_for: >-
      order and meaning of the world header, the object record, the option list and the object hierarchy rules.
      This port reads a converted PC format, not the disc format; every field was re-checked against the disc
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (BrxWalker, ObjectWalker,
      OptionWalker, SpliceWalker, AssetWalker, CidLoaders.generated.inc)
    used_for: retail (disc) byte layout where it differs from the PC port's converted format; class-to-layout mapping
date: 2026-11-13
---

# Sly 1 world file: the object stream

A world (level) is one file. Once it is read from the disc ([disc.md](disc.md)) and decompressed
([world-file.md](world-file.md)), it is a single little-endian byte stream with no section table: every part is
found by parsing the part before it. world-file.md gives the outer layout; this spec gives the object stream (the
tree of game objects), the option-list encoding used by the world and by every object, and the rules that tie
objects together. [textures.md](textures.md) covers the material tables and pixel data, and
[geometry.md](geometry.md) the mesh sets inside drawable objects. Together they specify a walk of a whole world
from byte 0 to the texture payload (section 5.3).

Per-class layouts are in [`classes/`](classes/). Option value encodings are in
[`option-encodings.md`](option-encodings.md). Transforms are in [`transforms.md`](transforms.md). Test vectors are in
[`worlds.md`](worlds.md).

## 1. Notation

All values are little-endian. Offsets in this folder are byte offsets from the first byte of the decompressed world
file (the "file start"). Like world-file.md, fields are packed with no padding unless a table says otherwise.

| Name | Size | Meaning |
|---|---|---|
| `u8` `s8` | 1 | unsigned / signed byte |
| `u16` `s16` | 2 | unsigned / signed 16-bit integer |
| `u32` `s32` | 4 | unsigned / signed 32-bit integer |
| `f32` | 4 | IEEE-754 single-precision float |
| `vec3` | 12 | three `f32`: x, y, z |
| `mat3` | 36 | three `vec3`. The first is the image of the local X axis, the second of the local Y axis, the third of the local Z axis (the columns of a column-vector matrix; see transforms.md) |
| `mat4x3` | 48 | four `vec3`: the three axis images as in `mat3`, then a translation |
| `str` | 2 + n | `u16` byte count n, then n bytes. No terminator. Narrow 8-bit text in the USA release |
| `desc32` | 32 | an encoded file descriptor (eight `u32`) pointing at a level, a sound or a video on the disc; [disc.md](disc.md) section 4.1 gives the decoding. All zero means "none" |
| `FLT_MAX` | | the float 3.4028235e38 (bit pattern `0x7F7FFFFF`), used as "infinite / not set" |

Counts are unsigned unless the table says `s8`/`s16`. Where a count is signed, a negative value never occurs on the
disc; treat it as a corrupt file.

## 2. Where the object stream sits

The outer layout of a world file (header, prolog, script table, world option list, descriptors, material tables,
padding, texture payload) is specified in [`world-file.md`](world-file.md) section 4, and the material tables in
[`textures.md`](textures.md) section 2. This spec does not repeat them. Two of those parts use encodings defined
here:

- the **world option list** (world-file.md 4.4) is an option list as in section 4 below; the options that matter for
  a static render are in section 4.4;
- the **object stream** (world-file.md 4.7) is the top-level object list of section 5.

The world itself (the root of the hierarchy) is not a record of the object stream. Its class (ID 108 in the class
table) never appears as a record. Everything that names a shader does so by the shader's position in the shader
table (textures.md 2.3).

Measured on the disc in addition to world-file.md: the sound-bank value in the prolog is 1 in the hub and menu
worlds and 1 + the episode number elsewhere (2 for the first episode's worlds up to 6 for the fifth); no USA world
uses 0. The script table has 1 to 154 scripts per world. A recursive parser of the script lists that recurses for
both the nested list and the "next" node reaches a depth of 72 on the USA disc; an iterative parser is safer.

## 4. Option lists

Most objects, and the world itself, carry an option list: a sequence of (option ID, value) pairs that set
properties. The meaning of most options belongs to later gameplay specs; a loader must still step over every one.

### 4.1 Layout

| Field | Type | Notes |
|---|---|---|
| option ID | `s16` | a negative ID ends the list. Only −1 occurs on the disc |
| value | depends on the ID | the encoding comes from the option table in option-encodings.md; there is no length prefix |

The list repeats until the terminator. An empty list is just `FF FF`.

### 4.2 Value encodings

Every option ID has one fixed encoding:

| Code | Bytes | Value |
|---|---|---|
| `b` | 1 | byte (booleans and small enums) |
| `h` | 2 | `s16`: an object ID, class ID, sound ID, message ID, shader ID or an enumeration value |
| `f` | 4 | `f32` |
| `i` | 4 | `s32` |
| `c` | 4 | packed colour: bytes R, G, B, A in that order. A = 0x80 means opaque (PS2 convention); R, G, B are 0 to 255, except option 988 where 0x80 is full intensity |
| `p` | 8 | two `f32` (a range: min, max; or two parameters) |
| `v` | 12 | three `f32`: a vector, three coefficients, or a rotation vector (axis × angle in radians, see transforms.md) |
| `q` | 16 | four `f32` |
| `0` to `6` | varies | a typed argument list of that many arguments (section 4.3) |

### 4.3 Typed argument lists

Options whose code is a digit N are calls with N arguments. Each argument is a `s16` type tag followed by a value:

| Type tag | Value |
|---|---|
| 0 | 1 byte (boolean) |
| 1 | `f32` |
| 2, 4, 9, 11 | 12 bytes (vector, rotation vector, three coefficients) |
| 3, 12 | 16 bytes |
| 5 | `s32` |
| 6, 7, 14, 15, 16, 17, 20 | `s16` (object ID, class ID, sound ID, and similar IDs) |
| 10 | 8 bytes (range) |
| 13 | `u32` (packed colour) |
| 4096 to 8191 | `s16` (enumeration value) |
| anything else | corrupt |

Tags 8, 18 and 19 never appear as argument types. Tags seen on the disc: 0, 1, 2, 5, 6, 7, 9, 10, 15, 20 and several
in 4096 to 8191.

### 4.4 Options that matter for a static render

World options (world-file.md 4.4) seen on the disc:

| ID | Code | Meaning |
|---|---|---|
| 528 | `c` | sky / clear colour |
| 529 | `f` | camera field of view, radians (1.1 in the Paris hub) |
| 530 | `f` | near clip distance (70 in Paris) |
| 531 | `f` | far clip distance (60000 in Paris) |
| 532 | `f` | fog start distance |
| 533 | `f` | fog end distance |
| 534 | `f` | maximum fog amount, 0 to 1 |
| 535 | `c` | fog colour |
| 526 | `f` | ambient light level for lit areas; the authored value is on a 0 to 255 scale |
| 527 | `f` | ambient light level for shadowed areas; same scale |
| 524, 525 | `b`, `f` | music on/off, music volume |
| 538 | `f` | commentary timing |
| 549, 550, 551 | `h`, `p`, `p` | an intermittent ambient sound: sound ID, volume range, repeat interval range. The triple can repeat |

### 4.5 Options that change the layout

Two option IDs change what follows later in the same object's body:

- **366** and **421** (code `h`): the particle-origin kind of an emitter. When the value in the object's own
  option list is 4 ("mesh origin"), the emitter's body carries a mesh block after its curve (see
  classes/placed-object.md, emitter). Options of child objects do not count. On the disc an object never has both
  IDs, and at most one of them.

No other option affects the byte layout.

## 5. Object lists and object records

### 5.1 Object list

| Field | Type | Notes |
|---|---|---|
| count N | `u16` | |
| N object records | | in stream order |

An **object record** is a 6-byte header followed by a class-specific body:

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 | class ID | `s16` | index into the class table (section 6). Selects the body layout |
| 2 | object ID (OID) | `s16` | 1 to 4960 on the disc; never negative. Not unique (section 7) |
| 4 | script index | `s16` | index into the script table of world-file.md 4.3 (0 to script count − 1) or −1 for none. 51984 of 54059 records use −1 |

The body follows immediately. Bodies have no length prefix, so an unknown class ID makes the rest of the file
unreadable.

### 5.2 Where object records appear

Object records appear in five places. In all of them the 6-byte header is the same; only the framing differs:

| Context | Framing | Effect on the hierarchy |
|---|---|---|
| object list (top level, and the child list at the end of every placed-object body) | `u16` count, then records | the records become children of the owner (the world, or the placed object whose body holds the list) |
| proxy source (classes/proxy.md) | `s16` class ID −1 for a back-reference, else a full record followed by a `s16` clone count | placed in the world through the proxy |
| state-machine transition (classes/animation-and-state.md) | one record per transition whose "has animation" byte is non-zero | owned by the state machine; not in the world |
| explosion group member (classes/markers-and-volumes.md) | `s16` count, then records | owned by the group; not in the world |
| warp and exit animations (classes/markers-and-volumes.md) | `s16` count, then items with a 6-byte header and an animation-segment body | owned by that object; not in the world |

In the last case the 6-byte item header has the same three fields, but the body is always an animation segment
(class 131) whatever the class ID says.

### 5.3 Walking a whole world

A loader that only needs to reach the end of the object stream:

1. Read the outer sections up to the object stream as world-file.md 4.1 to 4.6 says (script table, world option
   list with the encoding of section 4, descriptors, material tables per textures.md 2).
2. Read the top-level object list (world-file.md 4.7): for each record, read the header, look the class ID up in the class table, and
   parse the body with that class's layout. Bodies recursively contain more object records in the contexts of
   section 5.2.
3. Skip zero padding to the next multiple of 16 from the file start (world-file.md 4.8). This is the texture base T.
4. Check: T plus header word 0 must not exceed the file size. On all 46 USA worlds the walk ends exactly at T, and
   the lowest palette or bitmap offset in the material tables is 0 (the first table entry points at T).

Useful checks while walking (all hold on the disc): option IDs are below 1184 and present in the option table;
mesh, edge and tree indices are in range (geometry.md, classes/shared-structures.md); nesting depth of object lists is at most
12; every class ID is one of the 126 classes marked "seen" in section 6.

## 6. Class table

147 class IDs exist (0 to 146). 126 of them occur in at least one USA world. The **layout** column says which body
layout the stream uses for that class.

Layout keys (defined in `classes/`):

- **base**: base object: an option list only (classes/base-object.md).
- **placed**: placed object: transform, render settings, options, model, extensions, children
  (classes/placed-object.md).
- **solid**: solid object: collision data, then the placed layout (classes/solid-object.md).
- Anything else: a class-specific layout named in the column.

"Seen" gives the number of records on the whole USA disc and the number of worlds (of 46) containing the class. A
class marked "not on disc" has no layout here (`—`): a loader for the USA disc treats a record with that class ID as
corrupt input.

Classes are identified by their numeric class ID, which is the game's own identifier and is what the stream
stores. The names are descriptive names of our own. Roles marked "?" are tentative and will be settled by the
gameplay specs.

| ID | Neutral name (role) | Layout | Seen (records / worlds) |
|---:|---|---|---|
| 0 | (not on disc; unnamed) | none (never in the stream) | not on disc |
| 1 | base object | base | not on disc as a record |
| 2 | placed object | placed | 8907 / 46 |
| 3 | solid object | solid | 2003 / 44 |
| 4 | static world mesh | solid | 46 / 46 (one per world, OID 1) |
| 5 | (not on disc; unnamed) | — | not on disc |
| 6 | (not on disc; unnamed) | — | not on disc |
| 7 | player character | solid | 41 / 41 |
| 8 | (not on disc; unnamed) | — | not on disc |
| 9 | guard, smart variant | solid | 18 / 17 |
| 10 | guard variant ? | solid | 61 / 31 |
| 11 | guard variant ? | solid | 1 / 1 |
| 12 | guard variant ? | solid | 1 / 1 |
| 13 | guard variant ? | solid | 1 / 1 |
| 14 | companion character | solid | 4 / 4 |
| 15 | guard variant ? | solid | 1 / 1 |
| 16 | guard variant ? | — | not on disc |
| 17 | guard variant ? | solid | 1 / 1 |
| 18 | guard variant ? | solid | 1 / 1 |
| 19 | boss ? | solid | 1 / 1 |
| 20 | tank | solid | 4 / 4 |
| 21 | vehicle ? | solid | 1 / 1 |
| 22 | turret | solid | 3 / 3 |
| 23 | vault | solid | 20 / 20 |
| 24 | puffer ? | solid | 1 / 1 |
| 25 | vehicle | solid | 8 / 3 |
| 26 | vehicle variant ? | solid | 1 / 1 |
| 27 | jet pack | solid | 1 / 1 |
| 28 | rhythm game object ? | solid | 1 / 1 |
| 29 | minigame vehicle ? | solid | 1 / 1 |
| 30 | (not on disc; unnamed) | — | not on disc (created at run time) |
| 31 | water | solid | 43 / 23 |
| 32 | (not on disc; unnamed) | — | not on disc |
| 33 | breakable | solid | 332 / 37 |
| 34 | breakable alarm ? | solid | 17 / 17 |
| 35 | breakable gun ? | solid | 2 / 2 |
| 36 | breakable variant ? | solid | 2 / 2 |
| 37 | fragile breakable | solid | 25 / 15 |
| 38 | electrified breakable ? | solid | 2 / 1 |
| 39 | breakable piece | solid | 570 / 30 |
| 40 | button | solid | 72 / 23 |
| 41 | trigger volume button | solid | 450 / 41 |
| 42 | trigger volume variant ? | solid | 87 / 6 |
| 43 | physics bone ? | solid | 6 / 6 |
| 44 | (not on disc; unnamed) | — | not on disc |
| 45 | prize variant | — | not on disc |
| 46 | clue | solid | 28 / 19 |
| 47 | alarm | solid | 17 / 17 |
| 48 | (not on disc; unnamed) | — | not on disc |
| 49 | laser sensor | solid | 11 / 11 |
| 50 | camera sensor ? | solid | 38 / 11 |
| 51 | pressure sensor ? | solid | 22 / 2 |
| 52 | barrier | solid | 176 / 36 |
| 53 | (not on disc; unnamed) | — | not on disc |
| 54 | hazard zone ? | solid | 70 / 19 |
| 55 | hazard volume ? | solid | 159 / 33 |
| 56 | checkpoint | solid | 30 / 25 |
| 57 | conveyor | solid + 2 floats (classes/solid-object.md) | 51 / 7 |
| 58 | hiding place ? | solid | 2 / 2 |
| 59 | bomb | solid | 30 / 11 |
| 60 | missile | solid | 13 / 12 |
| 61 | missile variant ? | solid | 21 / 16 |
| 62 | homing missile ? | solid | 32 / 23 |
| 63 | mine | solid | 41 / 41 |
| 64 | fly ? | solid | 2 / 2 |
| 65 | rat | solid | 4 / 4 |
| 66 | minigame actor ? | solid | 1 / 1 |
| 67 | minigame actor ? | solid | 1 / 1 |
| 68 | minigame actor ? | solid | 2 / 2 |
| 69 | minigame actor ? | solid | 1 / 1 |
| 70 | dart | solid | 2 / 2 |
| 71 | jump object ? | solid | 6 / 6 |
| 72 | puffer target ? | solid | 1 / 1 |
| 73 | solid variant ? | solid | 4 / 4 |
| 74 | final boss ? | solid | 1 / 1 |
| 75 | final boss part ? | solid | 1 / 1 |
| 76 | decoy | solid | 41 / 41 |
| 77 | (not on disc; unnamed) | — | not on disc (created at run time) |
| 78 | camera placement | placed | 101 / 42 |
| 79 | skeleton bone | placed | 4046 / 44 |
| 80 | particle emitter | emitter: placed + extras (classes/placed-object.md) | 1321 / 45 |
| 81 | light | placed | 2254 / 44 |
| 82 | skeleton helper ? | placed | 202 / 39 |
| 83 | proxy (instancer) | proxy (classes/proxy.md) | 7925 / 46 |
| 84 | sky | placed | 40 / 39 |
| 85 | (not on disc; unnamed) | — | not on disc |
| 86 | charm | placed | 28 / 28 |
| 87 | life token | placed | 46 / 46 |
| 88 | coin | placed | 48 / 30 |
| 89 | key | placed | 28 / 28 |
| 90 | gold prize ? | placed | 22 / 22 |
| 91 | boost prize ? | placed | 2 / 2 |
| 92 | lock | placed | 8 / 8 |
| 93 | lock group | placed | 12 / 8 |
| 94 | tail / chain segment | placed | 1080 / 45 |
| 95 | placed variant ? | placed | 2 / 1 |
| 96 | flash volume ? | flash: placed + convex volume (classes/placed-object.md) | 65 / 17 |
| 97 | dynamic shadow | placed | 41 / 41 |
| 98 | waypoint | placed | 12 / 6 |
| 99 | camera zone ? | own layout (classes/markers-and-volumes.md) | 442 / 44 |
| 100 | jump location ? | placed | 67 / 5 |
| 101 | dialogue | dialogue: placed + events (classes/placed-object.md) | 822 / 45 |
| 102 | speaker | placed | 97 / 45 |
| 103 | world map ? | placed | 48 / 44 |
| 104 | spawner ? | placed | 1 / 1 |
| 105 | hub selector | placed | 1 / 1 |
| 106 | hub progress | placed | 5 / 1 |
| 107 | integrity check object | placed | 1 / 1 |
| 108 | world | world body (world-file.md 4.2 to 4.6) | never a record |
| 109 | (not on disc; unnamed) | — | not on disc (created at run time) |
| 110 | curve shape | shape (classes/paths-and-maps.md) | 456 / 38 |
| 111 | curve shape variant ? | shape | 75 / 23 |
| 112 | pipe | shape | 93 / 26 |
| 113 | rail | shape | 75 / 11 |
| 114 | (not on disc; unnamed) | — | not on disc |
| 115 | transform marker | marker (classes/markers-and-volumes.md) | 1321 / 41 |
| 116 | warp point | warp | 399 / 41 |
| 117 | target marker | marker | 215 / 34 |
| 118 | handle marker ? | marker | 26 / 19 |
| 119 | (not on disc; unnamed) | — | not on disc |
| 120 | explosion group | explosion group | 267 / 46 |
| 121 | explosion emitter | explosion emitter | 13 / 8 |
| 122 | explosion emitter, simple ? | explosion emitter | 2407 / 46 |
| 123 | volume | volume | 78 / 31 |
| 124 | rat hole | volume | 8 / 4 |
| 125 | level exit | exit | 114 / 43 |
| 126 | point | point | 1716 / 45 |
| 127 | point variant ? | point | 16 / 7 |
| 128 | point variant ? | point | 9 / 8 |
| 129 | perch point ? | point | 42 / 15 |
| 130 | scan point ? | scan point | 81 / 29 |
| 131 | animation segment | animation segment (classes/animation-and-state.md) | 13092 / 46 |
| 132 | (not on disc; unnamed) | — | not on disc |
| 133 | (not on disc; unnamed) | — | not on disc |
| 134 | visibility map | visibility map (classes/paths-and-maps.md) | 19 / 19 |
| 135 | freeze group ? | base | 12 / 6 |
| 136 | state machine | state machine (classes/animation-and-state.md) | 755 / 46 |
| 137 | guard group ? | base | 5 / 4 |
| 138 | path zone (navigation mesh) | path zone (classes/paths-and-maps.md) | 98 / 27 |
| 139 | reach map ? | reach map (classes/paths-and-maps.md) | 41 / 41 |
| 140 | object variant ? | base | 61 / 32 |
| 141 | object variant ? | base | 67 / 33 |
| 142 | screen mask | keyhole (classes/paths-and-maps.md) | 46 / 46 |
| 143 | object variant ? | base | 60 / 16 |
| 144 | rhythm sequence ? | base | 4 / 1 |
| 145 | lightning flash ? | base | 77 / 41 |
| 146 | (not on disc; unnamed) | none (never in the stream) | not on disc |

Class 107 is a tamper check in the original game; it carries no geometry and can be dropped after parsing.

## 7. Identity, hierarchy and references

### 7.1 Object IDs

- The OID names an object for scripts, options and other objects. It is **not unique**: 4805 OIDs occur more than
  once within a world (summed over all worlds). Lookups are scoped by the hierarchy (7.4).
- **OID 1** is the static world mesh (class 4), the first or second record of the top-level list in every world.
- **OID 5** is the player character (class 7), always inside a proxy that also has OID 5.
- **OIDs 12 to 42 are stock templates.** In every USA world the top-level list holds exactly one object with each
  of these 31 OIDs (27 placed objects, two explosion groups, two explosion emitters). On load each is stored in a
  31-slot template table (slot = OID − 12) and taken out of the world. They are not drawn; the game clones them at
  run time (pickups, effects). The rule applies wherever a record with such an OID is loaded through an object list.

### 7.2 Parent and child

- An object in a top-level list has the world as parent.
- An object in the child list of a placed object (the last field of the placed layout and every layout built on it) has
  that object as parent.
- Proxies re-parent their content (classes/proxy.md): a proxy's sources end up as children of the proxy's own
  parent, and the children listed after a single-source proxy end up as children of that source. World transforms
  are kept in both moves.
- Objects owned by a state machine, an explosion group, a warp or an exit are not part of the world hierarchy.
- Only bodies built on the placed layout have child lists. The transform marker, point and volume classes take their
  position relative to their parent placed object.

### 7.3 Order

The hierarchy is built in stream order, depth first: an object's body (including all its descendants) is read
completely before its next sibling. A parent's world transform is known before its children are read.

### 7.4 How objects refer to each other

- **By OID**, in option values (code `h`), in placed-object extensions, proxy overrides and state-machine tables.
  The game resolves an OID with one of these scopes, chosen by the referring field: a direct child of the context
  object; a descendant whose nearest marked ancestor is the context; an ancestor of the context; the nearest match
  (the candidate whose ancestor chain meets the context's ancestor chain closest to the context); or anywhere in the
  world. The per-field scope belongs to the gameplay specs; for a static render only the ancestor scope is needed
  (classes/placed-object.md, extension bits 2 and 3).
- **By proxy-source index**: a back-reference inside a proxy names an earlier proxy source by its position in the
  world-wide list of sources (classes/proxy.md).
- **By script index**: the third header field.
- **Assets** are referred to by index into the world's tables: shaders by their position in the shader list
  (textures.md 2.3); textures only through shaders; sounds and music by IDs or `desc32` descriptors; other levels by
  `desc32` descriptors.
