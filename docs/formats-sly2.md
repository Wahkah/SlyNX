# The `formats-sly2` crate

`crates/formats-sly2` reads Sly 2: Band of Thieves (USA, SCUS-97316) from the player's own disc image: the disc and
its executable, the level table, the worlds, their packages, textures, object stream and geometry, and builds a
scene the Sly 1 viewer and renderer can draw. It is written from the specs in `specs/sly2/` only.

- `#![no_std]` with `alloc`, `#![forbid(unsafe_code)]`, no third-party crates. Builds for
  `aarch64-nintendo-switch-freestanding` with `--no-default-features`.
- It builds on `formats` (the Sly 1 crate): decompression (the same algorithm, world-file.md 2), the ISO reader,
  SHA-256, the byte `Reader`, the `Error` type, texture helpers and the `Scene` types.
- Feature `std` (default): `Disc::open_path`.

## Quick start

```rust
use formats_sly2::{Disc, World};
use formats_sly2::scene::{SceneOptions, build};

let mut disc = Disc::open_path(iso_path)?;            // checks the executable's SHA-256
let row = disc.find_world(0x3B).unwrap();              // f_nightclub_cellar: level-table row 3
let data = disc.read_world(row)?;                      // read + decompress
let world = World::parse(&data, &disc.options)?;       // header, packages, materials, every object body
world.check()?;                                        // every body parsed to its declared size
let scene = build(&world, SceneOptions { row: Some(row as i16), ..SceneOptions::default() }).scene;
// scene: formats::scene::Scene (vertex and index arenas, meshes, instances, materials, textures)
```

## Modules

| Module | Main items | Spec |
|---|---|---|
| `record` | `FileRecord::decode` (64 bytes, XOR word pairs, sector base `0xCA0`) | disc.md 4 |
| `exe` | `Executable` (ELF segments, `va_to_offset`, `file_table`, `option_table`), `OptionTable`, `value_size`, `arg_size`, `option_ids` | disc.md 3, 5; object-stream.md 1.1 to 1.3 |
| `disc` | `Disc<R>` (`open`, `open_unchecked`, `open_path`, `rows`, `files`, `read_world`, `find_world`) | disc.md 6 |
| `world` | `World::parse`, `read_header_start`, `PackageEntry`, `LevelRow`, `TreeNode`, `Sections`, `PackageBody`, `leaf_regions` | world-file.md |
| `textures` | `MaterialTables` (palettes, bitmaps, node list, shaders), `Shader::frame`, `decode` | textures.md |
| `options` | `OptionList::read` | object-stream.md 1 |
| `stream` | `Walk`, `Record`, `Body` (`Solid`, `Placement` with its `PlacementBlock`s and `Patch`es, `Other` with its layout and transform `Header`), `layout_of`, `visit` | object-stream.md 2, 3; classes/ |
| `transforms` | `TransformList`, `TransformRef`, `world_matrices`, `resolve`, `read_matrix` | transforms.md |
| `geometry` | `MeshSet`, `Mesh`, `Group` (rigid, indexed, outline; `weights` gives `SkinWeights`), `MeshCopy`, `strip_triangles` | geometry.md |
| `collision` | `CollisionPart` (shape, fields, a `formats::collision::CollisionMesh` and `CollisionTree`), `read_mesh`, `read_raw_tree`, `convert_tree`; counts for zones, volumes, warps and tracks | classes/solid-object.md 2 |
| `anim` | `Clip` (channels, extra curves, three event lists, options, key count, `explicit_loop`), `StateMachine` (states, transitions, `kept_transitions`), `character_clips`, `Lookup`, `read_lookup_table`, `resolve_lookup`, the table addresses | animation.md, classes/animation.md |
| `rig` | `Rig` (`set_vector` by transform kind, `set_swivel`, `update`: mirrors then chains, `chain_goal`, `chain_effector`, `skin_matrix`), `ChainBind`, `euler_zyx`, `axis_angle`, `inverse`, `no_std` trigonometry | animation.md 4 to 6, transforms.md 4 |
| `scene` | `build`, `SceneOptions`, `SceneBuild` | classes/placement.md 2, geometry.md 9 |

## The walk

`World::parse` reads every header section, every package body (script block, material tables, texture data, graph
block, object list) and **every object body at every depth** with its class layout: solids (transform list, mesh
set, collision parts, options, children, name), placements (fresh and copy, with their extra objects), the
options-only and transform-header layouts, curves, tracks, path networks, triangle surfaces, zones, volumes,
warps, emitters (with their origin meshes), explosion groups, jobs, tasks, screen movies (dictionaries, fonts,
sprites), clips (vector and scalar curves, event lists) and state machines.

Each body is parsed inside a reader limited to its declared size and must end exactly there. One that does not is
skipped by its size and listed in `World::errors` (with its class, offset and error), so one bad body never stops
the walk; `World::check` turns a non-empty list into an error. On the USA disc the list is empty for all 43 worlds:
**121,153 bodies, all parsed field-exactly**.

Where the spec's rule cannot decide a layout but exactly one form ends at the declared size, the walker uses that
form and records it in `World::undecided`. On the disc only the 296 kind −1 placements naming no package are recorded
(classes/placement.md 1.3). The emitter origin-mesh rule of classes/effects-logic-ui.md 1 decides every emitter:
class 29 when option 483 is 3, class 33 when option 542 is 5, class 32 never.

Bodies keep what the engine needs to make objects:

- the transform header of the simple layouts (`Body::Other::header`, with the layout);
- a fresh placement's blocks, with the option list of kind-5 blocks (`Placement::blocks`);
- a copy placement's patches, with their target ID and options (`Placement::patches`);
- each vertex group's skin data offset (`Group::weights(data, bones)`: slots and byte weights, geometry.md 5);
- each solid's collision parts (`Solid::collision`) in `formats::collision` form, so `engine::collision::TriMesh::new`
  takes them unchanged:
  - The mesh gets a bounding radius: the largest distance of its positions from the origin, since Sly 2 stores none.
  - The tree is converted to 16-bit form. Children are `u32` node numbers with `0xFFFF_FFFF` for none, and stored
    leaf nodes (surface `0xFFFF`) become `NO_CHILD`.
  - The 6 level-geometry trees with more than 65,535 nodes do not fit and stay empty; the engine's bounding-volume
    hierarchy serves queries without them.

## Conventions

- **Offsets** in parsed structures are absolute offsets into the decompressed world; vertex data is read on demand
  (`Group::vertices`, `Group::triangles`).
- **Matrices**: `formats::geometry::Affine`, three axis images and a translation (transforms.md 1); a transform's
  world matrix is `parent.mul(local)`.
- **Triangles**: strip vertex `i` (from 2) closes `(i−2, i−1, i)` unless its break bit is set, with the winding
  alternating with `i` counted from the start of its strip list; triangles repeating a position are dropped.
- **Images**: RGBA8 in storage order, alpha converted from the PS2 scale, 8-bit indices through the CSM1 swap
  (textures.md 3). Frame 0, palette slot 0 for a first render.

## Animation

### Data

`anim::Clip::of_record(data, record, &options)` parses a class-109 record: the duration in seconds, the channels
(target object ID, transform index, a vector or a scalar curve, the flag byte), the extra-curve records, the apply,
timed and retract event lists, and the option list. **Curves are `formats::anim` curves**, so `engine::anim::curve`
evaluates them unchanged:

| Sly 2 curve | `formats::anim` curve |
|---|---|
| vector 0, constant | `VectorCurve::Components { mask: 0, base, curves: [None; 3] }` |
| vector 1 (two slopes), 2 (one slope) | `VectorCurve::Hermite` (one slope: incoming = outgoing) |
| vector 3, 4 (per axis, one shared time list) | `VectorCurve::Components` with a `ScalarCurve::Hermite` per animated axis |
| scalar 0, constant | `ScalarCurve::Hermite` with one key |
| scalar 1 | `ScalarCurve::Hermite` |
| vector 5, scalar 2 (blend placeholders, not on the disc) | `VectorCurve::Blend`, `ScalarCurve::TwistBlend` |

Key frames are 1/60 s and slopes are units per second in both games; Sly 2 keys have no tangent kinds, so they get
kind 0 (no step rule). Events are `formats::anim::Event` (type, frame for the timed list, payload) with Sly 2's types
and payload sizes (animation.md 2). `Clip::keys` counts the time entries of keyed curves; `explicit_loop` finds a
timed type-5 event at the clip's end (frame f ≥ 60 × duration − 1, animation.md 7) and returns its frame, target
frame and speed.

`anim::StateMachine::of_record` reads a class-111 record's states and transitions (its members are the record's
nested records); `kept_transitions` drops transitions whose object is not a member.

`anim::character_clips(&character, row)` lists a character's clips: class 109 among its nested objects and among the
members of the state machines nested there, with records whose level conditions exclude `row` left out (`None`:
all).

`anim::read_lookup_table(&executable, table)` reads one of the character tables (`SLY_TABLES`, `BENTLEY_TABLE`,
`MURRAY_TABLE`, `SHARED_TABLE`) after checking the executable's SHA-256; `resolve_lookup(&character, &lookup)` finds
a transform ID in the character's transform list or an object ID among its nested objects (depth first), or reports
a part/mesh lookup or "not found" (the game skips those).

### Posing (`rig`)

```rust
let mut rig = Rig::new(&solid.transforms, &root, &enclosing);   // the loaded (bind) pose; chains measured
rig.reset();
for ch in clip.channels.iter().filter(|c| c.target == object_id) {
    if let Some(v) = ch.vector.as_ref().and_then(|v| eval_vector(v, t)) { rig.set_vector(ch.transform as usize, v); }
    if let Some(a) = ch.scalar.as_ref().and_then(|c| eval_scalar(c, t)) { rig.set_swivel(ch.transform as usize, a); }
}
rig.update(&root, &enclosing);                                 // world matrices, mirror pairs, then chains
let skin = rig.skin_matrix(bone, mesh_transform);              // W_bone · W_bone,bind⁻¹ · W_mesh,bind
```

- `set_vector` applies animation.md 4 by the transform's kind: 1 replaces the translation, 6 the rotation part with
  `Rz · Ry · Rx` stored as the axis-image columns (the `engine::anim::curve::euler_zyx` convention; on the disc,
  constant rotation channels match the loaded rest rotation in this form 432 times and in the transposed one never),
  12 the three diagonal entries; 0 and 13 are not animated.
- Mirror pairs copy the source's world rotation (and position, when flagged) to the target and update its
  descendants.
- Two-bone chains follow animation.md 6: the upper and lower lengths, bind bend angle and bend axis (in the upper
  bone's frame) are measured at load; each update resets the two bones to their loaded rotations, bends the lower
  bone to the angle that puts the effector at the goal's distance, turns the upper bone so the effector points at the
  goal, and spins it right-handed about the upper-to-goal axis by the swivel. The bend axis is u × v. With a bone
  shorter than 0.0001 the angle is 0 and the axis (1, 0, 0), and the bend step is skipped. Parallel bones, for which
  the spec gives no rule, use u × X (u × Y when u is along X).
- Root motion is transform 1's translation (animation.md 5); a viewer playing in place keeps transform 1's loaded
  translation.

### What `engine::anim` needs for Sly 2

Curves and their evaluation carry over unchanged. The rest differs from Sly 1. Points 1 to 3 are in `anim::xform`,
and points 4 to 6 in `anim::sly2` (`ClipPlayer`, `SkinPalette`, `blend`); see `docs/engine.md`.

1. **Targets are transforms, not objects.** A Sly 2 channel names (object ID, transform index). `Binding` would map a
   channel to a node of a per-object transform list, not to a stream record; `Rig` would need a constructor from a
   transform list (parents by `TransformRef`, including the enclosing object's transforms).
2. **Per-kind writes.** A Sly 2 value replaces only part of a transform's loaded local matrix (translation, rotation
   from Euler angles with no pre or post rotation, or the three diagonal entries) depending on the transform's kind,
   where `AnimTarget` writes position, rotation and scale separately. An `AnimTarget` for Sly 2 can route
   `set_position`, `set_rotation` and `set_scale` to `Rig::set_vector` by kind.
3. **A post-channel solver step.** After the channels: mirror pairs, then two-bone chains driven by the scalar
   channels (`set_twist` maps to `Rig::set_swivel`). `Rig::update` does both.
4. **Events.** Sly 2's event types differ from Sly 1's (type 5 jumps to a frame at a speed, type 11 carries a marker
   object ID, type 0 a sound request). `Playback::apply_event`'s Sly 1 meanings must not be applied to Sly 2 clips;
   the explicit loop (`Clip::explicit_loop`) is the one playback rule known so far.
5. **Repeat modes** are not stored in Sly 2 clips; the game's code chooses them (animation.md 7).
6. **Skinning** binds bones by (object ID, transform reference) and uses `W_bone · W_bone,bind⁻¹ · W_mesh,bind`
   (`Rig::skin_matrix`); Sly 2 weights are bytes summing to 255 with up to four bones per vertex, and Sly 2 meshes
   have no pose-weight curves.

## The scene

`scene::build(&world, SceneOptions)` returns a `formats::scene::Scene`:

- Packages with the keep flag are drawn where they are stored; every other package is drawn once per placement that
  names it, at the placement's matrix, recursively (up to `max_depth`); a copy placement's extra objects are drawn
  with it.
- Records and meshes whose level conditions exclude `SceneOptions::row` are left out, and so are objects and
  placement roots marked "remove at start" (option 26). `row: None` keeps every variant.
- Rigid and indexed meshes are drawn; skinned and streamed meshes hold outline hulls only. Passes: 1, 3, 4, 5, 8 and
  10 by default (`SceneOptions::passes`; geometry.md 9's working assumption).
- A mesh is drawn with its transform's world matrix in the loaded (bind) pose; a copy with its matrix, then its
  transform's.
- Materials are numbered across packages (`SceneBuild::material_base[package] + shader index`); each used material
  gets frame 0, palette slot 0 of its shader, decoded once per package, bitmap and palette; kinds 1 and 7 also get
  slot 1 (the grey palette, `SceneBuild::slot1`). `SceneBuild::self_illum` is each shader's bytes 20 and 21
  (`Shader::self_illum`, rendering.md 2.4).
- `SceneBuild::lights`: the drawn objects' lights (class 4) as `SceneLight` (world matrix and options 151 to 158 with
  effects.md 2's defaults). `SceneLight::from_values` reads them with other option ids (`LightOptions::SLY3`).
- For lighting (rendering.md 2.3 to 2.5): `SceneBuild::mesh_flags` (each scene mesh's body flags byte; `Mesh::body_flags`),
  `submesh_flags` (each submesh's strip-list flags), `baked` / `baked_colours` (the vertex-map tables' pre-baked
  colours per scene instance and submesh, from the mesh's `VertexMap` or the copy's, `VertexMap::read_at`),
  `color2` (each shader's second colour, `Shader::color2`) and `anims` (`SceneAnim`: each used material's
  texture-animation node record, with every frame's textures for the frame classes 139 to 142).
- `SceneInstance::record` and `SceneMesh::record` hold the record's header offset in the world data;
  `SceneInstance::issuing_mesh` is the mesh's index in that record's mesh set. (The `issuing()` helper of the Sly 1
  scene does not apply.)

Scenes of the cellar (row 3), the intro hub (row 0), the menu (row 1) and the final boss (row 43) build with every
texture decoded and every placement resolved; top-down renders show the expected layouts. Packages with the keep
flag also put a few things outside the playable area where they are stored (in the intro hub, a row of small
objects and some large flat pass-10 quads), which a viewer may want to hide.

## Tests

`cargo test -p formats-sly2`: unit tests on synthetic data (file records, option tables and lists including typed
arguments, script nodes, transform lists and composition, strips and break bits, material rules and 4- and 8-bit
decoding, record framing and the skip-on-error rule, event lists and curves, partition-tree regions).

Disc tests (`tests/disc/`) run when `SLYNX_DISC_SLY2` names the image and skip otherwise (use `--release`):

- disc identity, the executable's segment mapping and the 29 file records, the boot world's hash, the save icon and
  movie magic;
- the level table (44 rows, row 18 empty, ids, episodes, slots, hubs, sectors, sizes, save paths), identical in all
  43 worlds;
- every world: decompressed size and SHA-256, package count, level package name and keep flag, tail size, records,
  placements, solids, meshes, groups and triangles (worlds.md), all bodies field-exact, the only undecided bodies
  the 296 placements naming no package; totals (42,097 records, 11,250 solids, 53,994 meshes, 2,821,629 triangles,
  121,153 bodies);
- the class totals stated in the class documents and animation.md;
- section boundaries of rows 0, 1, 3 and 43 (world-file.md 6);
- the cellar: package names, body offsets and prologue bytes, the first partition node, the level package's 289
  records, first record bytes and class histogram, the level geometry (31 transforms, 541 meshes, 26,908 triangles,
  the collision part, the first mesh and group to the byte), and the player character (139 transforms, chains,
  mirror, followers, references, nested solids, 76 meshes, 4,407 triangles, the first skinned mesh's bones, slots,
  weights and outline edges);
- textures: the five cellar and five intro shaders with header fields, first pixels and RGBA hashes, the shared grey
  placeholder, the intro sky bitmap, the totals over all worlds (71,774 palettes, 37,508 bitmaps, 38,750 shaders,
  every first frame decoded) and the texture-data sizes;
- the first body of each layout (offsets and sizes, classes/ test vectors);
- the option table entry of option 26 (`0x1ACD10`) and the option-26 counts (6,167 in object lists, 338 in placement
  blocks and patches, read through `Placement::blocks` and `patches` and checked against the bytes);
- parent `0x2000`: 2,656 placements (2,595 in explosion-group lists, 56 and 5 in nested and top-level copy extras),
  and its other uses (spec question 1);
- collision: 5,413 parts in 2,928 solids, 500,683 surfaces, 878,394 converted tree nodes (every tree converts but
  the 6 too large), node indices and surfaces in range, the cellar's level collision within its drawn bounds;
- skin weights: every vertex's byte weights sum to 255 (330,732 rigid and indexed positions, 11,418 skinned and
  streamed groups);
- scenes of four worlds (index and material ranges, finite transforms, textures for every drawn material).

- animation, on the nightclub hub: for Sly, Bentley and Murray the body object, transform and chain counts, clips,
  keys, explicit loops, cycles (136, 108 and 109 by the rule of animation.md 7), total length and the minimum,
  median and maximum duration, and the counts with channel-less clips (296, 175, 203); that vector channels of
  each character target only kinds 1, 6 and 12 and scalar channels only chain upper bones; Sly's chains and the three
  mirror pairs; Sly's run clip (record, 72 channels, its events, the root, transform 10 and transform 12 values at
  four times and their keys, the swivel keys and its value at 0.25 s, the root-motion rate), evaluated with
  `engine::anim::curve`; the Euler convention; Sly posed with the run clip through `Rig` (finite, root motion, the
  mirror pair, and 16 chain poses whose effector lands on its goal); the seven lookup-table rows of animation.md 9.

`SLYNX_SLY2_RENDER=<folder>` also writes top-down and side views of two worlds as PPM images.

## Spec questions

1. **Parent `0x2000` outside placements** (transforms.md 1 says it occurs only on placements). It also occurs on
   transform headers: 5,408 in explosion-group lists (all class 33), 216 in copy-placement extras (classes 3, 11,
   13, 19 and 39), and 215 class-10 curves in lists no object owns (202 top-level, 13 nested in solids). It also
   occurs in the transform lists of 6 class-106 solids in copy extras. The engine resolves it as the owner
   everywhere, which is the copy root for unowned lists. What does the game do for curves in unowned lists?
