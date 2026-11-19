# The `formats` crate

`crates/formats` reads Sly 1 (USA, SCUS-97198) from the player's own disc image: the disc layout, the world files,
their textures and geometry, and the object stream, and turns a world into a scene a renderer can upload. It is
written from the specs in `specs/sly1/` only.

- `#![no_std]` with `alloc`, no third-party crates, `#![forbid(unsafe_code)]`. Builds for
  `aarch64-nintendo-switch-freestanding` with `--no-default-features`.
- Feature `std` (default): opening a disc image by path (`Disc::open_path`) and `ReadAt` for `std::fs::File`.
- Every reader is bounds-checked and returns `formats::Error`; nothing panics on bad input.

## Quick start

```rust
use formats::disc::Disc;
use formats::scene::{Scene, SceneOptions};
use formats::world::World;

let mut disc = Disc::open_path(iso_path)?;          // checks the executable's SHA-256
let index = disc.find_world("Paris, France").unwrap();
let bytes = disc.read_world(index)?;                  // read + decompress
let world = World::load(&bytes)?;                     // outer layout, materials, object stream, texture base
let scene = Scene::build(&world, SceneOptions::default());
// scene.vertices / scene.indices: arenas; scene.instances: draws; scene.materials / scene.textures
```

`cargo run --release -p formats --example inspect -- <disc.iso> [world | --list | --bench]` prints a world's summary
(sections, counts, geometry totals, scene size) or the world table, or times all 46 worlds.

## Modules

| Module | Main items | Spec |
|---|---|---|
| `iso` | `ReadAt` (random access, and `image_len` so that reads sized from disc fields are checked before anything is allocated; implemented for `&[u8]`, `&mut R` and `File`), `read_volume`, `read_dir`, `find_root_file`, `read_file` | disc.md 2 |
| `disc` | `Disc<R>` (`open`, `open_unchecked`, `open_path`, `read_world_compressed`, `read_world`, `find_world`, `read_descriptor`), `Executable` (`va_to_offset`, `bytes_at`, `c_string`, `descriptor_table`), `Descriptor::decode`, `WorldEntry`, the table constants | disc.md 4, 5 |
| `compress` | `decompress`, `decompress_into` | world-file.md 2 |
| `hash` | `Sha256`, `sha256`, `Crc32`, `crc32`, `hex`, `parse_hex32` | |
| `reader` | `Reader` (little-endian cursor with absolute offsets, `align_zero`), `Vec3` | |
| `options` | `OptionTable::sly1()` (the encoding grid), `OptionEncoding`, `OptionList::read`, `OptionList::iter`, `typed_arg_size` | object-stream.md 4, option-encodings.md |
| `world` | `World::load` / `load_with`, `WorldLayout::parse`, `Header`, `ScriptTable`, `texture_base` | world-file.md 4 |
| `textures` | `MaterialTables` (palettes, bitmaps, shaders, fonts; `shader_texture`), `Shader` (`frame_texture`, `base_texture`), `AnimRecord`, `TexturePayload::decode` to `RgbaImage`, `convert_alpha`, `csm1_index` | textures.md |
| `geometry` | `MeshSet`, `Mesh`, `Submesh`, `OutlinePart`, `Instance`, `MeshSet::draws`, `Submesh::to_gpu` / `append_gpu`, `strip_triangles`, `GpuVertex`, `GeometryStats`, `Affine`, `Submesh::influences` / `pose_position` / `pose_normal` (skinning and pose data) | geometry.md |
| `stream` | `ObjectStream::walk`, `Record`, `Context`, `Layout` / `layout_of`, `Proxy` / `ProxyEntry`, `Extension`, `world_transforms`, `placed_flags`, `back_reference_pairs`, `clone_range`, `clone_drawables`, `clone_transforms`, `cloned_records`, `placements`, `placements_where`, `find_nearest`, `owned_animations`, `volumes`, `is_sentinel_position`, the worlds.md counters | object-stream.md 5 to 7, classes/, transforms.md |
| `anim` | `Segment::parse`, `Channel`, `ScalarCurve` / `VectorCurve` and their keys, `Event`, `SegmentSettings`, `RepeatMode`, `player_lookup_list` / `read_lookup_list` | classes/animation-and-state.md, classes/shared-structures.md, animation.md |
| `collision` | `Solid::parse`, `CollisionMesh`, `CollisionTree`, `ConvexVolume`, `FadeEntry`, `material_of`, `MaterialTables::from_executable` | classes/solid-object.md, classes/shared-structures.md, collision.md |
| `scene` | `Scene::build`, `SceneMesh`, `SceneSubmesh`, `SceneInstance`, `SceneMaterial`, `SceneTexture`, `SceneOptions` | all of the above |

## Data conventions

- **Offsets** in every parsed structure are absolute offsets into the decompressed world file. Parsed structures keep
  offsets and counts; vertex data is copied out on demand (`Submesh::position`, `normal`, `uv`, `color`, `strip`,
  `weight`), because it is unaligned in the file.
- **Transforms**: `Affine { cols, t }` stores the 3 x 3 part as three **columns** (the images of the local axes), so a
  point maps to `cols[0] * x + cols[1] * y + cols[2] * z + t`. `a.mul(&b)` is `a * b` (parent times child).
  `to_mat4()` gives a column-major 4 x 4 for a GPU uniform. `normal_matrix()` is the inverse transpose.
- **Space**: game space, right-handed, Z up, game units (about centimetres). transforms.md 7.1 gives the conversion to
  a Y-up GPU space; apply it once in the view matrix.
- **Triangles**: counter-clockwise front faces. A draw whose transform has a negative determinant
  (`SceneInstance::mirrored`, 40 instance matrices on the disc) reverses the winding: flip the front-face rule for
  that draw.
- **Images**: `RgbaImage` is RGBA8, rows in **storage order** (row 0 is texture coordinate v = 0). Upload the rows as
  they are and use the mesh UVs unchanged. Alpha is converted from the PS2 scale (0x80 = opaque) to 0..255; colour
  channels are copied. 8-bit bitmaps use the CSM1 index permutation (bits 3 and 4 swapped).
- **Vertex colours** are passed through as stored (0..255); whether 0x80 or 0xFF is 1.0 is open in geometry.md.

## The scene

`Scene::build(&world, SceneOptions)`:

1. `ObjectStream::placements` lists every drawable object to draw: records in the world hierarchy, outside stock
   templates (top-level OIDs 12 to 42 and everything below them), placed, with a mesh set that draws something;
   then, for each back-reference of an in-world proxy, a clone placed at that proxy (`clone_of_proxy`).
   - **Clones** (classes/proxy.md 3, worlds.md 5). A clone copies the records read inside the source's body: its
     children, and the full sources and child lists of the proxies nested in it. It never copies the child list of
     the source's own proxy; each back-referencing proxy carries its own. Back-references nested inside the source
     are resolved once and their clones are copied with the outer clone, depth first in stream order.
     - Placement is iterative.
     - The walk refuses a stream whose clones would copy more than `MAX_CLONED_RECORDS` (2^20) records in total
       (`cloned_records`).
     - `clone_range` and `clone_drawables` expose the rule.
   - **Transforms** follow transforms.md: parent chain, proxies as the parent of their sources and children,
     degenerate (`-FLT_MAX`) object matrices replaced by the identity, and the extension block's ancestor override
     (bit 3 rotation, used by 50 objects; bit 2 position, never on the disc). The static world mesh (class 4) is
     stored at the identity, so it lands in world space with no special case.
   - **Not placed** (transforms.md 4): an object whose world position has a component above 1e30 (one parked proxy
     on the disc) is left out with its descendants and anything it would clone (`placed_flags`,
     `is_sentinel_position`).
2. Each real mesh used is converted once: one `GpuVertex` (position, normal, UV, colour; 36 bytes, `#[repr(C)]`)
   per strip entry into `Scene::vertices`, `u16` triangle indices into `Scene::indices`, relative to the submesh's
   `base_vertex`. A `SceneMesh` is a range of `SceneSubmesh`es, each a range of both arenas with its material.
3. Each mesh draw (real or instance) becomes a `SceneInstance` with `transform` = object world transform times the
   instance matrix (mesh space to world space), the mesh's render pass and billboard mode. Instance meshes whose
   matrix holds the `FLT_MAX` "not placed" sentinel (201 on the disc) are left out and counted in
   `skipped_degenerate`.
4. `Scene::materials[i]` describes shader `i` (kind, blend flags, pass, material colour) and points at the decoded
   texture it shows in an unlit render: frame 0, palette slot 1 for kind-0 shaders, the only palette
   otherwise. Only shaders used by a drawn submesh get a texture; identical (bitmap, palette) pairs are decoded once.

Skinned and morphed meshes are drawn in their bind pose (bones and weights are available on `Submesh` but not in
`GpuVertex`).

## Lower-level use

- Walk only: `WorldLayout::parse(&bytes, &OptionTable::sly1())`, then `ObjectStream::walk(&bytes,
  layout.object_stream, &table)`, then `texture_base(&bytes, stream.end, &layout.header)`.
- One mesh set at a known offset: `MeshSet::parse(&bytes, offset)`.
- One texture: `world.payload().decode(&tables.bitmaps[b], Some(&tables.palettes[p]))`.
- A record's options: `record.options.unwrap().iter(&bytes, &table)` yields `(id, value bytes)`.

## Performance (release, one thread, PC)

| Step | All 46 worlds |
|---|---|
| Decompression | 285 MB of output in about 470 ms: about 610 MB/s of output (390 MB/s of input) |
| Outer layout and object-stream walk | about 50 ms |
| Scene build (arenas and texture decode) | about 180 ms |

Measured with `inspect --bench` in WSL; Switch numbers to come.

## Tests

- Unit tests (`cargo test -p formats`) use synthetic data built in the tests: ISO image, ELF, scrambled
  descriptors, compressed streams (checked against a direct ring-buffer decoder), material tables, mesh sets,
  option lists, an object stream exercising most layouts, nested back-references and a parked proxy, and a whole
  synthetic world through `Scene::build`.
- Disc tests (`tests/disc/`) run when `SLYNX_DISC_SLY1` names the image and skip otherwise. They check the spec test
  vectors: the disc identity, file list and executable mapping; all 46 world-table entries; sizes, SHA-256 and CRC-32
  of all 46 decompressed worlds and three compressed ones; the outer section offsets; table counts, shader-kind and
  pixel-format surveys, dangling-index and (bitmap, palette) pair counts and the eight decoded-image hashes; geometry
  totals, first submeshes and the instance example;
  the worlds.md table for all 46 worlds, the detailed worlds (first records, world mesh, world option ids, class
  histograms), drawable-in-clones per world and over the disc, and the disc-wide class table; the transform vectors,
  origin bounds, the "not placed" sentinels and the rotation override; and a scene for every world.
  The vectors live in `tests/disc/vectors.rs` (numbers and hashes only).

## API changes

Changes since the first version, for code built against it:

- `ObjectStream::clone_transforms(world, proxy_world: &Affine, source, overrides)` takes the placing proxy's world
  transform instead of its record index (so nested clones can be placed), and returns one transform per record of
  `clone_range(source)`.
- `clone_range(source)` is now `source .. subtree_end`. It no longer includes the child list of the source's
  single-source proxy (classes/proxy.md 3). Back Alley Heist has 384 drawable objects in clones, and the disc
  14,805.
- New on `ObjectStream`: `placed_flags`, `back_reference_pairs`, `clone_range`, `clone_drawables`; new free function
  `stream::is_sentinel_position`.
- `placements` now includes clones nested inside clones and leaves out objects that are not placed;
  `Placement::clone_of_proxy` is the outermost in-world proxy for a nested clone.
- `WalkCounts::options` now includes the option lists of warp and exit animations (`animation_options` is that part).
- `OptionTable::sly1()` follows the revised option-encodings.md grid: only the 653 ids that occur on the disc are
  valid (option 96 and ids above 1166, for example, are now rejected).
- `Scene`: no type or field changed. `skipped_degenerate` now counts only instance meshes with the sentinel matrix;
  scenes may hold more instances than before (nested clones).
- New modules `anim` and `collision` (parsers for the engine).
- New on `ObjectStream`:
  - `placements_where(opts, select)`: placements for any record kind, for example solids;
  - `find_nearest(context, oid, include_out_of_world)`: the nearest-OID lookup of object-stream.md 7.4;
  - `cloned_records`;
  - the fields `owned_animations` (warp and exit animation bodies) and `volumes` (convex-volume offsets).
- `Extension` has a new field `offset`, and `pre_rotation(data)` and `post_rotation(data)`.
- `SceneInstance` has a new field, `issuing_mesh`: the index of the mesh that issued the draw in the record's
  mesh set. `SceneInstance::issuing(&world)` returns that mesh (pass, sort key, fog, flags, billboard, texture
  animation).
- Read on demand:
  - `ObjectStream::proxy_overrides(data, &proxy, &table)`: `ProxyOverride` entries (classes/proxy.md 1.2);
  - `Record::warp_info(data, &table)`: a warp's name and hidden OIDs;
  - `Record::exit_destinations(data)`: an exit's two descriptors, decoded;
  - `Record::pose_weight_sets(data)`: pose-weight sets;
  - `Mesh::glint(data)`: the glint of a mesh with flag 0x080 (rendering.md 6.2), a `Glint` with the direction and
    the four coefficients; `Glint::factor(x)` evaluates it. `None` without the flag.

  The disc test reads 8,574 overrides, 399 warps and 114 exits, and 732 glint meshes (all unit directions).
- `ReadAt` has a new provided method `image_len()`. `read_vec`, `read_file` and `Disc::read_descriptor` check the
  read against it before allocating. Readers that do not know their size accept at most 1 GiB per read.
- `hash::parse_hex32` returns `Option<[u8; 32]>` instead of panicking.
- `CollisionTree::read` rejects a child that is not after its parent, and a node with two parents. The disc has
  neither, on all 46 worlds; walks of a parsed tree terminate.
- `SegmentSettings` no longer has `duration`: option 188 has no stream encoding (animation.md 5.1), and the
  duration comes from the record. A setting value of the wrong size is reported as corrupt.
- Neutral names, from the specs' own wording:
  - `textures::shader_kind::{KIND_0, KIND_1, KIND_2, KIND_5}` and `KIND_0_BASE_SLOT` replace the earlier named
    constants;
  - `geometry::mesh_flags::{HIGHLIGHT, DISTORTION}` replace the two earlier flag names.
- Robustness fixes:
  - an odd texel count in 4-bit bitmaps keeps its last pixel;
  - `Submesh::color` and `strip` no longer panic out of range;
  - `strip_triangles` ignores entries past 65,535 instead of wrapping;
  - the ELF reader uses checked arithmetic.

## Spec questions

The questions of earlier rounds are answered in the specs (worlds.md 5, classes/proxy.md 3 and 4, transforms.md 4
and 4.1, textures.md 2.5, option-encodings.md, object-stream.md 6), and the tests assert the corrected vectors,
including the clone rule (384 in Back Alley Heist, 14,805 over the disc). None is open for this crate.
