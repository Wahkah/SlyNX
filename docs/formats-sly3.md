# The `formats-sly3` crate

`crates/formats-sly3` reads Sly 3: Honor Among Thieves (USA, SCUS-97464) from the player's own disc image: the
executable's world, movie, option and class tables, the worlds with their groups, textures, object stream, geometry
and animation, and builds a scene in the shared `formats::scene::Scene` form. It is written from the specs in
`specs/sly3/` only.

- `#![no_std]` with `alloc`, `#![forbid(unsafe_code)]`, no third-party crates. Builds for
  `aarch64-nintendo-switch-freestanding` with `--no-default-features`.
- It builds on `formats` (decompression, ISO reader, SHA-256, `Reader`, `Error`, `Scene`, `formats::anim` curves) and
  reuses `formats-sly2` where Sly 3 is the same: file records, the executable and its option table, option lists,
  transform lists and matrices, collision meshes and trees, strip decoding, partition-tree nodes.
- Feature `std` (default): `Disc::open_path`.

## Quick start

```rust
use formats_sly3::{Disc, World};
use formats_sly3::scene::{SceneOptions, build};

let mut disc = Disc::open_path(iso_path)?;                  // checks the executable's SHA-256
let slot = disc.find_world("v_apt").unwrap();               // world slot 5
let data = disc.read_world(slot)?;                          // read + decompress
let world = World::parse_with(&data, &disc.options, disc.tangent_multipliers)?;
let ctx = world.fresh_context(Some(slot as i16));           // slot + option set 0's condition
let scene = build(&world, SceneOptions { context: ctx, ..SceneOptions::default() }).scene;
```

`world.errors` lists bodies that did not parse to their size; on the disc that is exactly one 360-byte class-157
body in `dvd_menu` (its layout is open, classes/effects-logic-ui.md 5). It is skipped by its size.

## Modules

| Module | Main items | Spec |
|---|---|---|
| `exe` | world slots (`WorldSlot`), `movie_table`, `option_table`, `class_table` (`ClassEntry`), `tangent_multipliers`, `is_supported` | disc.md 1, 3, 5; object-stream.md 1.1, 3.1; animation.md 2.1 |
| `disc` | `Disc<R>` (`open`, `open_unchecked`, `open_path`, `world_slots`, `find_world`, `read_world`, `read_file`) | disc.md |
| `world` | `World::parse` / `parse_with`, `KeyEntry`, `GroupEntry`, `WideString`, `OptionSets`, `PartitionSet`, `Sections`, `GroupBody`, `copy_counts`, `fresh_context`, `typed_ints` | world-file.md |
| `filter` | `Filter` (world slots + conditions), `Context` | world-file.md 6 |
| `options` | `read` (Sly 3 value sizes into Sly 2's `OptionList`), `value_size`, `arg_size`, `first_argument`, option ids | object-stream.md 1 |
| `textures` | `MaterialTables` (two copies; `palettes()` / `bitmaps()` are copy 1), `Shader::frame`, `Grading`, `GRADING`, `decode`, `alpha` | textures.md |
| `stream` | `Walk`, `Record`, `Body` (`Solid`, `Placement`, `Emitter`, `Effect`, `Screen`, `Clip`, `Other`), `Layout`, `layout_of`, `command_args`, `visit` | object-stream.md 2, 3; classes/ |
| `geometry` | `MeshSet`, `Mesh` (filter, bone table, fog, billboard), `Group` (strips, indexed, outline), `ColorTable`, `MeshCopy`, `color15` | geometry.md |
| `collision` | `read_part` → `CollisionPart` with the mesh as `formats::collision::CollisionMesh` and the tree as `formats::collision::CollisionTree` (Sly 2's readers; `convert_tree` also takes a stored `0xFFFFFFFF` as no child) | classes/solid-object.md 2 |
| `anim` | `ClipData` (time lists, channels, parts, events), `read_vector`, `read_scalar`, `packed_tangent`, `segment` | animation.md |
| `scene` | `build`, `SceneOptions`, `SceneBuild` | (see below) |

## The walk

`World::parse` reads the header sections in order (two key tables, the 88-byte group list, save variables, Sly 2's
sound tables, the UTF-16 string table with its per-language character lists, one option list, the option sets, the
partition trees per condition), pads to 16, then reads each group body inside a reader limited to its stated body
size and checks it ends exactly there. A group body is the texture-data size, stamp, debug flag, the script block
(Sly 2's node kinds renumbered), the material tables, the texture data, the graph block and the object list.

Every object record carries a mode byte (and for mode 1 a world filter and condition list) and a body size. Each
body is parsed with its class's layout in a reader limited to the body; a body that fails or ends early goes to
`World::errors` and becomes `Body::Unparsed`. Nested lists (solid children, copy extras, fresh-placement data, zones,
groupers, state machines) are walked the same way; effect components are read with their own record headers and
counted in `World::component_bodies` (not in `object_bodies`).

Placements are fresh when their mode is above 0, or −1 with a streaming group; otherwise copies. A fresh
placement's data is parsed as an object list (the game skips it).

## Conventions

- Offsets in parsed structures are absolute in the decompressed world (`Record::header_offset`, `body_offset`,
  `Mesh::offset`, `Shader::offset`, texture and palette data offsets).
- `Mesh::offset` is where the mesh starts, after the two filter-count bytes; for a filtered mesh after its filter
  lists and size.
- Emitter and component "position kind" options (617, 670, 558) are plain enumeration values (value type `0x101E`),
  read by `options::first_argument`, which handles both plain values and typed lists.
- Palette alpha converts as `a × 255 / 128` rounded down (textures.md 3); grading is applied only when asked and
  the palette's flag bit 0 is set.

## Animation

Clips (classes 130, 131) read their option list first, then the deferred flag; deferred clips (468) carry no data.
Clip data keeps the shared time lists; vector tracks 0 to 3 become constant `VectorCurve::Components`, tracks 4 to 7
become per-axis `ScalarCurve::Hermite` curves at the frames of their time list (packed values scaled by 1/16 or
1/1024, packed tangents `±tan(i·π/32) × multiplier`). Scalar tracks 0 and 1 become Hermite curves, 2 becomes
`TwistBlend`. Events keep their raw payloads. `anim::segment` is the key search of animation.md 3.

## The scene

- Shared groups are drawn where they are stored; other groups are drawn once per placement naming them (modes −1,
  0, 1; mode-2 spawners are left to gameplay), at the placement matrix; copy extras are drawn with the copy.
- Records whose mode-1 filter fails, filtered meshes that fail, objects and placements with option 27 (a
  placement: in a bind for target 7 or 8), hidden meshes and copies, and groupers' objects are left out.
- Strip and indexed meshes in passes 0, 2, 3, 4, 7 and 9 (default) are drawn; outlines are not. Zones place their
  objects by their matrix.
- Textures are frame 0, palette slot 0, graded (`SceneOptions::grade`).
- Kinds 1 and 7 also get palette slot 1 (`SceneBuild::slot1`); `SceneBuild::self_illum` is each shader's
  parameter bytes as a `u16`; `SceneBuild::lights` holds the lights as `formats_sly2` `SceneLight`s (options 159 to 165,
  rendering.md 2.1): records of class 4 and its derived classes (`SceneOptions::lights`, from the executable's class
  table with `light_classes`).
- `SceneBuild::baked` / `baked_colours`: the colour table in effect (geometry.md 4.2) per scene instance and submesh
  whose strip list has flag `0x20`; `mesh_flags`, `submesh_flags` and `color2` as in `formats-sly2`.

## Tests

`cargo test -p formats-sly3` runs the unit tests (option sizes, filters, script nodes, key tables, packed tracks,
colour grading, strip vectors). `tests/disc` runs every vector of `specs/sly3/` when `SLYNX_DISC_SLY3` names the
player's image (and passes, skipping, otherwise); run it with `--release`:

- identity, executable tables (option and class entries, multipliers), the 40 world slots, the 23 movies, the menu
  world's compressed bytes;
- per world: sizes, SHA-256, groups, shared groups, top-level records and placements, shaders, tails, solid bodies,
  meshes, filtered meshes, triangles and outline edges (all 39 rows and the totals);
- 125,693 object bodies parsed field-exactly, the only error the one class-157 body;
- section boundaries of slots 0, 2, 5; v_apt's key tables, groups, save variables, strings, partition tree;
- class counts (top-level, nested, worlds) for all 155 classes; record modes and filter kinds;
- placement counts, binds and targets; v_apt's first record and placement;
- the five v_apt shader vectors (headers, first pixels, raw and graded hashes), texture totals and kinds;
- the level object and Sly: transforms, meshes, triangles, collision; geometry surveys (body types, hidden, no-body,
  copies, colour tables, bones, weights summing to 255, passes);
- clips, deferred clips, state machines, command scripts, effects and components, emitters with meshes, screen
  objects and element kinds;
- a scene for v_apt (textures decode without error) and for every world.

## Collision surfaces

`collision::with_polygons(mesh)` is the shared `formats::collision::CollisionMesh::with_polygons`: a stored
surface is a convex polygon in the plane of its corners, the convex hull of its corners and the ends of every edge
naming it (its own and its neighbours'). The stored surfaces stay first, at their indices; the hull's fan
triangles are appended. The v_apt floor vector of `specs/sly3/worlds.md` is a disc test. Spec question: 2.1 calls
the surfaces triangles.

## Exits

`stream::exit_body(data, body_offset, table)` reads a class-24 body: the header, the options, the trigger volume
(mesh and tree, the tree as a `WideCollisionTree`), the destination's file record and the extra file records
(movies).

## Where the disc differs from the spec text

These are pinned by the disc tests and reported as spec questions:

- Sly 2 solid-object.md 2.2 (used by Sly 3): a missing tree child is stored as `0xFFFFFFFF` as well as by a leaf
  node. With that, all 5,632 trees convert, to `formats::collision::WideCollisionTree` (32-bit node indices): three
  level objects' trees have 75,558 to 103,211 stored nodes, too many for 16-bit indices. A tree that does not
  convert makes `read_part` fail. The engine builds them with `TriMesh::new_wide`.

- object-stream.md 1.1/6: the option table at `0x32CA80` is at file offset `0x22DA80` (the stated rule gives that,
  the text says `0x22BA80`); the 20 bytes given for entry 27 are entry 26's. Entry 27 has the same type and flags.
- world-file.md 3.2: the copy-count survey (0: 36) matches the counts after option set 0; stored counts are never 0.
- geometry.md 2.2: "14,846 trees" is the node total; 63 sets have a tree.
- geometry.md 8: the stated mesh sizes run to the next mesh's filter-count bytes (the next mesh starts 2 bytes later).
- classes/effects-logic-ui.md 6.1: element counts include kind-8 glyph elements (7,410 kind 1 and all 78 kind 0).
- classes/placement.md 1: the copy/fresh and bind counts match the top-level placements plus 10 nested ones; on the
  disc 17 nested placements (outside fresh data) have parent `0x2000`, 7 of them copies. Class 8 has 11,061 nested
  records (object-stream.md), not 11,071.
- classes/effects-logic-ui.md 1, 2: the position kind of options 617, 670 and 558 is a plain enumeration value, not
  a first typed argument.
- textures.md 3: alpha `a × 255 / 128` is rounded down (Sly 1's helper rounds to nearest; shader 22's hash needs the
  floor).
