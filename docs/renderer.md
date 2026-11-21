# Renderer

Status: phase 1, 2026-11-13. Crate: `crates/render`. No spec needed: generic engine code (no game formats). The
level viewer ([viewer.md](viewer.md)) feeds it real Sly 1 levels.

The renderer draws a level of a few thousand static and moving objects with a CPU cost that does not grow with
per-object API state. It has one backend-neutral front end and two backends: deko3d on the Switch and OpenGL 4.3 core
on PC.

## Why it is shaped this way

On the Switch with OpenGL on Mesa, about 1,500 draws cost over 20 ms of CPU per frame. The cost was not the GPU but
per-draw state changes in the driver: rebinding a uniform-buffer range per object, rebinding storage buffers per
object, and switching vertex arrays between models. The design removes every per-draw binding:

| Per-draw cost in the old path | Here |
|---|---|
| Uniform-buffer range per object | One instance buffer per frame, indexed by an instance slot; bound once per frame |
| Storage-buffer rebinds | Same buffer, bound once per frame |
| Vertex array switch per model | One geometry arena (one vertex buffer, one index buffer) per level; each mesh is a range |
| Texture bind per object | Textures grouped into array-texture pools; a batch binds one pool; the layer is per instance |
| One API draw per object | OpenGL: one `glMultiDrawElementsIndirect` per batch. deko3d: one bare `dkCmdBufDrawIndexed` per draw command, no state in between |

## Architecture

```
            level load                                 every frame
  add_mesh / add_texture / add_material      add_instance / set_transform / set_visible
                 |                                            |
                 v                                            v
        Resources (CPU side)                       Renderer::render(camera)
  arena: Vec<Vertex>, Vec<u16>, mesh table          FrameBuilder (backend-neutral):
  pools: textures grouped by size+format+mips        cull (sphere vs frustum) -> sort key -> runs
  materials: texture, pass, tint, cutoff             -> GpuInstance[], DrawCmd[], Batch[]
                 |                                            |
       commit(): one LevelUpload                       FramePacket
                 |                                            |
                 v                                            v
          Backend::upload_level                      Backend::submit
   GlBackend | Deko3dBackend | NullBackend
```

Source layout (`crates/render`):

| Path | What |
|---|---|
| `src/types.rs` | `Vertex` (28 bytes), `MeshData`, `TextureData` (RGBA8 + packed mip chain), `Material`, `Pass`, `BlendMode`, `CullMode`, handles, `RenderError` |
| `src/resources.rs` | Arena building, texture pools, materials, `LevelUpload` |
| `src/frame.rs` | `GpuInstance`, `DrawCmd`, `Batch`, `FrameUniforms`, `FramePacket`, `FrameStats`, and the frame builder |
| `src/renderer.rs` | `Renderer<B: Backend>` and `Camera` |
| `src/backend.rs` | The `Backend` trait and `NullBackend` |
| `src/math.rs` | Vectors, column-major matrices, frustum; trigonometry and square root without libm (same results on PC and Switch) |
| `src/gl/` | OpenGL 4.3 backend: `api.rs` (hand-written entry-point loading), `backend.rs`, `egl.rs` (headless context on Linux, loaded with `dlopen`) |
| `src/deko3d.rs`, `c/slynx_render.{h,c}`, `c/slynx_render_shaders.S` | deko3d backend: Rust wrapper and the C side |
| `shaders/*.glsl` | The shaders, shared by both backends (uam compiles them as is; OpenGL swaps the `#version` line) |
| `src/overlay.rs` | `TextOverlay`: screen-space text (stats, messages) drawn as overlay-pass instances, with its own 5 x 7 font |
| `src/demo.rs`, `examples/cubes.rs`, `src/switch_demo.rs` | Test scene, PC benchmark and screenshot tool (PNG through the harness's encoder), Switch demo entry |
| `tools/build-switch-demo.sh` | Builds `dist/slynx-render-demo.nro` |

The crate is `no_std` + `alloc` on the Switch, like the platform layer.

## Data flow and memory

### Level data (uploaded once per level)

- **Arena.** All meshes' vertices go into one vertex region and all indices (16-bit, relative to the mesh) into one
  index region. A mesh is `(first_index, index_count, base_vertex)` plus a bounding sphere. A mesh has at most
  65,536 vertices; larger meshes are split by the caller.
- **Texture pools.** Each distinct (width, height, mip count, format) is a 2D array texture with up to 256 layers; a
  full pool starts another with the same key. A texture is `(pool, layer)`. RGBA8 now; block-compressed formats get
  their own pools later (the format is part of the key).
- **Second vertex stream.** One 16-byte `VertexExtra` per arena vertex, at the same index: bone indices (`u8 x 4`),
  weights (`unorm16 x 4`) and the vertex's index within its mesh (`u16`). Meshes without skinning data
  (`add_mesh`) get zero weights. It is a separate buffer so the 28-byte vertex stays as it is, and every mesh, skinned
  or not, is drawn with the same vertex layout.
- **Instance slot stream.** A static buffer holding 0, 1, 2, ..., bound as a per-instance vertex attribute (divisor 1).
  A draw's first-instance value offsets it, so instance `k` of a draw reads slot `first_instance + k`.
- After the upload the CPU copies of vertices, indices and texels are freed.

### Per frame

1. **Build (CPU, backend-neutral).** For each live, visible instance: transform the mesh's bounding sphere (or the
   instance's own, see skinning) and test it against the frustum, then make one 64-bit sort key per draw step:
   `order (5 bits) | slot (4) | mirrored (1) | sort value (24) | pool (10) | mesh (20)`.
   - `order` is the material's list ([`Material::order`], 0 to 31): lists are drawn in increasing order.
   - `slot` is the pass, with the steps of a pass split: background, opaque, alpha-tested, two-step prepass,
     blended, volume count, volume paint, post, overlay.
   - The sort value is 0 in `SortMode::Grouped` lists, so equal meshes become one draw with several instances;
     in `SortMode::FarToNear` lists it is the instance's sort key (`set_sort_key`) or the squared distance from
     the camera to its sphere centre, farthest first.
   - Overlay instances are never culled (the transform is already in clip space).

   Sort, then walk the sorted list once and write one `GpuInstance` per draw step (80 bytes: affine transform
   rows; tint; texture layer, flags and alpha cutoff in one word; first morphed vertex; first bone matrix; first
   vertex weight; the two extra image layers and the fog factor in one word; the UV offset), one `DrawCmd` per run
   of equal meshes, and one `Batch` per run of equal (state, pool).

   - `Batch::state` holds the fixed-function state as `STATE_*` bits: depth test (less, less-or-equal, always),
     depth writes, blending (none, alpha over, additive, and Sly 2's reverse alpha `src (1 - As) + dst As`, reverse
     additive `src + dst As` and destination alpha `src Ad + dst (1 - Ad)`), culling (none, back, front), front faces clockwise
     (instances whose transform mirrors: negative determinant, found when the transform is set), stencil (volume
     count, volume paint), colour writes off, the alpha-tested program, screen space. Mirroring and blend mode are
     not in the far-to-near key, so they never change the back-to-front order; a change just starts a new batch.
   - Instance flags: `INSTANCE_LIT` (Lambert), `INSTANCE_SCREEN`, `INSTANCE_SKINNED`, `INSTANCE_MORPHED`,
     `INSTANCE_WEIGHTS` (three images blended with per-vertex weights), `INSTANCE_NO_DARKENING`,
     `INSTANCE_NO_FOG`.
2. **Submit (backend).** Wait for the frame slot's fence, copy uniforms, instance records, bone matrices and morphed
   vertices (+ draw commands on OpenGL) into the slot's memory, bind everything once, then walk the batches.

### Passes

| Pass | Steps (slots) | State |
|---|---|---|
| `Background` | one | no depth test or writes; blending per material (`BlendMode::None` too) |
| `Opaque` | one | depth less, writes |
| `AlphaTest` | one | depth less, writes, discard below the cutoff |
| `Blend` | one, or two with `two_step` | depth less, no writes, alpha over or additive; with `two_step` first a prepass with depth writes that keeps only texels at or above the cutoff (all prepasses of a list, then all blended draws, both in the list's order) |
| `Volume` | two | count: both faces, colour off, depth less, stencil +1 where a back face fails the depth test and -1 where a front face does; paint: back faces, no depth test, only where the stencil is not zero (which is reset), blended |
| `Post` | one | after the scene in world space, no depth test or writes, blending per material |
| `Overlay` | one | screen space, no depth test, alpha over, no culling |
| `Outline` | one | depth less-or-equal with writes, no culling, alpha over; screen-width lines from edge meshes |

The volume count is the usual depth-fail ("z-fail") scheme: it gives the same pixels as marking each volume on its
own, except that overlapping volumes paint once instead of twice. The stencil is cleared every frame.

### Lighting, fog and darkening

- `Lighting::Unlit`: texture x vertex colour x tint. `Lighting::Lambert`: the same, shaded by the frame's light.
- `Lighting::Palettes`: three images of one pool (the material's texture T1 and `palettes` T0, T2), blended per
  pixel as `T0 * wS + T1 * wM + T2 * wH` with alpha `weight alpha * T1.a`. The weights are per vertex of each
  instance, computed by the caller and stored once per level: `add_vertex_weights` before `commit` (8 bytes per
  vertex, `VertexWeights`), then `set_weights(instance, base)`; the shader reads them through the vertex's index in
  its mesh (storage buffer 3). An instance without weights draws T1 x vertex colour.
- `Lighting::TwoColour` (Sly 2 and 3): the weights are two colours per vertex on the 0x80 = 1.0 scale (`shadow` =
  A's alpha, `mid` = A, `high` = B, `alpha` = B's alpha), and the pixel is `min(T0 x A, 1) + min(T1 x B, 1) x T1.a x
  B.a` with alpha `T0.a x A.a`: T0 (`palettes[0]`) is the grey image, T1 the texture. It is flagged in bit 24 of the
  instance record's layers word. On a placed mesh each piece's grey layer rides in its placement record and the
  weights base moves to the record's morph field (placed meshes are never morphed).
- Per-frame two colours (moving characters): `set_shader_light(id, Some(k))` makes the vertex shader compute A and B
  from a 15-row light block stored as matrices `k .. k + 5` of the instance's bones (shadow colour, the starting
  fourth channel, material colour, one-pass flag, and four lights: position or direction, colour, angular offset,
  scale, distance factor). The block is rebuilt on the CPU only when the character moves (`stage::light::light_block`;
  `block_two_colours` is the CPU reference the tests hold equal to the per-frame path); the GPU cost is a few dozen
  vertex-shader instructions per vertex, and nothing is uploaded but 15 rows per part.
- Fog (`set_fog`): per vertex, `t = clamp((1/near - 1/z) / (1/near - 1/far), 0, 1)` with `z` the distance from the
  camera, `fog = clamp(t * cap * strength * instance factor, 0, 1)` (`set_fog_factor`), then per pixel the colour
  moves toward the fog colour. Not for overlays or `no_fog` materials.
- Darkening (`set_darkening`): multiplies the colour before fog, except overlays and `no_darkening` materials.
- Texture animation: `set_material_images` (another frame's images) and `set_material_uv_offset`; every instance of
  the material follows from the next frame, since instance records are written from the materials each frame.

### Outlines

`Renderer::add_outline_mesh(&[[[f32; 3]; 4]])` (before `commit`) takes one edge per entry, as four mesh-space points
`[far corner, end, end, far corner]`, and makes a mesh of one quad per edge; the points go into the level's outline
buffer (storage buffer 4). An instance of it with a `Pass::Outline` material: the vertex shader projects the four
points, keeps the edge only when both far corners are on the same side of the projected edge (a silhouette; a crease
collapses), and widens it to `set_outline_width(px)` pixels of a 448-line frame, scaled with the target height
(4.125 by default). State: depth less-or-equal with writes, alpha over, no culling; colour = the instance tint; not
fogged or darkened. Every outline mesh of a level is one draw command per material, whatever the camera sees; the
silhouette test costs four point transforms per edge vertex on the GPU and nothing on the CPU.

### UI layer

`render::ui2d::UiLayer` draws a 2D list such as `ui::DrawList` (docs/ui.md): `UiLayer::new(r, sheet, max_quads)`
before `commit` adds a glyph sheet (sampled with clamp-to-edge, no mipmaps), a quad mesh, one overlay material
(order 30, under the stats text) and `max_quads` hidden instances; `set_quads(r, &[UiQuad])` each frame places them
(`rect` in pixels, y down; `uv`; `textured`; straight-alpha RGBA colour). Flat quads use `INSTANCE_FLAT` instead of
another texture, so a whole list is one draw command, and the instances keep list order (the overlay slot sorts
equal keys by instance). Textures with `TextureData::clamped()` get their own pools; deko3d has a second sampler
descriptor for them.

### Draw order of the UI

The sort key starts with the material's order (5 bits), then the slot, so a material with a higher order draws
after every draw of a lower one, whatever their passes. Sly 1's lists use orders 1 to 20; the screen-space layers
sit above them: the iris wipe 29 (`iris::IRIS_ORDER`), the UI layer 30 (`ui2d::UI_ORDER`), the stats text 31. So
the UI always draws after every blended, volume, post and outline draw of the world (and of the picture panels),
with no depth test. Keep game materials below 29 and this holds. Within one order, list order is kept: the overlay
slot sorts equal keys by instance index.

### Picture panels

`Renderer::set_panels(&[Panel])` (at most `MAX_PANELS`, 4; an empty list turns them off) draws the scene again from
other cameras into rectangles of the target, for Sly's dialogue panels. A `Panel` is a `Camera` (eye, target, up,
vertical field of view, near, far), a `rect` in pixels (x, y from the top left, width, height; clipped to the
target) and an optional background colour (`clear`; depth is cleared either way). The panel's projection uses the
rectangle's aspect, its own frustum culling and sort, and the frame's light, fog and darkening. Screen-space
(`Pass::Overlay`) materials are not drawn in panels.

The frame is split into views (`FramePacket::views`, `frame::View`: frame constants, viewport, clear bits, a range
of batches): the main view's world, then each panel, then the main view's screen-space draws. So UI frames drawn
around a panel cover it, and the iris covers panels too. Without panels the list is empty and backends draw as
before. OpenGL sets viewport and scissor per view and binds that view's uniform block (one 256-byte block per
view); deko3d does the same with `dkCmdBufSetViewports` / `dkCmdBufSetScissors` and clears limited by the scissor
(`SlynxRenderFrame::views`, up to `SLYNX_RENDER_MAX_VIEWS`, 8). Each panel costs another cull and sort on the CPU
and the draws its camera sees.

Sly 1's dialogue panels (camera.md 7.1) use `Renderer::set_image_panels(&[(Panel, image)])` instead: each panel's
scene is drawn into panel image 0 or 1 (its own colour and depth images, at most `frame::PANEL_IMAGE_SIZE` each way,
before the main view), and `panel::PanelFrames` shows the image in the panel's shape (straight outer side, half
circle inner side) with its outline and 57 drifting bands at alpha 48/128, as screen-space instances (order 27,
under the movie picture, the iris and the UI). OpenGL renders into a framebuffer per image; deko3d switches render
targets with a fragment barrier and binds the images as textures 5 and 6.

### Iris wipe

Sly 1's level-flow.md 5: the world's screen mask (five coloured triangle sets, Sly's mask) drawn over the frame,
shrinking onto a point while the screen around it darkens to black. None of the sets is an opening.
`render::iris::IrisShape::new(r, points, sets, colors, bounds)` before `commit` adds one flat-colour mesh per set
(points in the mask's plane, y up), a quad and hidden screen-space instances (order 29, over the world and the
picture panels, under the UI): the five sets, drawn from set 4 down to set 0, four covers around the shape's
bounding box and a fill of the box itself. `set(r, center_px, pixels_per_unit, u)` places it for a closing amount
`u` from 0 (open) to 1 (closed): every set's alpha times `clamp(2.5 u, 0, 1)`; set 0's colour darkened toward black
(RGB times `1 - u`) with its alpha moved toward 1, and the screen outside the bounding box filled with that colour;
from `u = 0.9` the box is filled too, fully at `u = 1` (`iris::iris_colors` is the rule on its own). `hide(r)`
hides it.

### Masked areas (Sly 1 passes 10 to 12)

`Pass::MaskWrite` and `Pass::MaskFill` draw each mesh twice, depth less-or-equal without writes (rendering.md 3.3).
MaskWrite: back faces writing only the destination alpha (`STATE_ALPHA_ONLY`), then front faces alpha over.
MaskFill: `BlendMode::InvDestAlpha` (`src (1 - Ad) + dst Ad`), then alpha over. Every blend writes the source
alpha into the destination alpha (rendering.md 3.2; `glBlendFuncSeparate` with alpha factors one / zero, the same on
deko3d), which is what the fill step reads.

### Projected shadows

rendering.md 9.2. `Renderer::set_projectors(&[Projector])` (at most `MAX_PROJECTORS`, 4) with
`Projector { origin, near_radius, far_radius, near_distance, far_distance, strength, receivers }`; it casts straight
down (-Z). `set_shadow_caster(id, true)` marks the instances drawn into the 64 x 64 silhouette image (flat, with
their vertex alpha, placed by the first projector); `set_receiver(id, bits)` marks the instances it falls on
(`bits & projector.receivers`; only opaque and alpha-tested materials).

Each frame with casters starts with a silhouette view (`View::target = VIEW_TARGET_SILHOUETTE`, cleared to clear
black). Each receiver inside a projector's reach then gets one more draw at the end of its own list: the same
surface, depth less-or-equal, `BlendMode::Multiply` (`dst x src`, the alpha kept), whose fragment outputs
`1 - m x strength`, with `m` the silhouette's alpha at the projected point (0 outside it). Receiving pixels with
alpha 0.9 or less, and depths more than 50 units past the casting range, are left alone. The projector's
parameters ride in the bone store for that frame (three rows after the instances' matrices), so they cost no
extra buffer. OpenGL keeps the silhouette as a 2D texture on unit 1 with its own framebuffer; deko3d keeps an
RGBA8 image (render target and texture, its descriptor after the pools') and switches render targets with a
fragment barrier. Fixed-texture projectors (kind 3, light patterns) are not built: no object on the disc has one.

### Movie frames

`render::video`: `Renderer::set_video_frame(Some(VideoFrame { width, height, y, u, v }))` copies a 4:2:0 picture
(chroma planes `(w + 1) / 2 x (h + 1) / 2`, rows top first); the next `render` uploads it once. `VideoLayer::new(r)`
(before `commit`) adds the quad (order 28: over the world and the panels, under the iris and the UI);
`layer.show(r, rect, tint)` with `VideoLayer::fit(target, picture)` for the letterboxed rectangle;
`layer.hide(r)`. The fragment shader converts BT.601 video range (`video::yuv_to_rgb` is the CPU reference).
OpenGL: three R8 textures on units 2 to 4 (`glTexSubImage2D`). deko3d: three R8 images; the planes are copied
into the frame slot's staging memory (1.5 bytes per pixel, 430 KiB for 640 x 448) and copied to the images on the
GPU before drawing; nothing is uploaded on frames without a new picture. A size change waits for the GPU once.

### Skinning and morphs

The interface asked for in [engine.md](engine.md), "Renderer interface for skinning":

- `Renderer::add_skinned_mesh(&MeshData, &[SkinVertex])`: a mesh with one `SkinVertex { bones: [u8; 4], weights:
  [u16; 4] }` per vertex (`SkinVertex::new` takes `f32` weights and rounds them to unorm16).
- `Renderer::set_bones(instance, &[BoneMatrix])`: the instance's bone matrices (48-byte `BoneMatrix`, three rows of
  an affine), kept until changed. The renderer owns one bone store; each skinned instance has a range of it, reused
  while the count stays the same, and the store is compacted when half of it is stale. The whole store goes to the
  GPU each frame as the bone buffer (storage buffer 1), and `GpuInstance::bone_base` is the instance's first matrix.
  `clear_bones` makes the instance rigid again.
- `Renderer::set_morph(instance, positions, normals)`: per-instance positions and normals written by the CPU (poses
  applied by the engine), one per vertex of the mesh. They go to the GPU each frame in the morph buffer (storage
  buffer 2); the vertex shader reads them through `GpuInstance::morph_base` plus the vertex's index in its mesh (from
  the second stream), so no `gl_VertexID` semantics are involved. `clear_morph` goes back to the mesh's data.
- `Renderer::set_bounding_sphere(instance, Some((centre, radius)))`: a sphere in model space used for culling in
  place of the mesh's rest sphere, for animated instances.

In the vertex shader, for a skinned instance, `M = sum of w_i * bones[bone_base + idx_i]` over the non-zero weights;
if all weights are zero the vertex stays where it is; weights are not renormalised. Then `p' = M p`,
`n' = normalize(mat3(M) n)`, and the instance transform and the view-projection as for any instance. A morphed
instance replaces the position and normal first.

Batching is unchanged: skinning is per instance, so skinned and rigid instances of the same mesh are one draw, and
nothing in the sort key or batch state depends on it. Limits per frame: `Limits::max_bones` and
`Limits::max_morph_vertices` (OpenGL 65,536 and 262,144; deko3d 4,096 and 8,192 by default, set with
`Deko3dBackend::with_limits`). A Sly 1 mesh set has at most 255 bones; `set_bones` takes at most 256 matrices.

`DrawCmd` has the layout of both `DrawElementsIndirectCommand` and `DkDrawIndexedIndirectData`.

### Frame slots and fences

Both backends keep three frame slots. Each has its own per-frame memory (uniforms, instances, and on OpenGL the
indirect commands; on deko3d also its command memory) and a fence signalled after the frame's commands. A slot is
written only after its fence has signalled, so the CPU never writes memory the GPU may still read. Time spent waiting
on a fence is reported separately (`FrameStats::wait_ns`); it is GPU-bound time, not CPU work.

### Shaders

`shaders/mesh_vsh.glsl` reads the instance record from storage buffer 0 at `inSlot` and the frame constants from
uniform block 0 (view-projection, light, fog, camera position and darkening, target size and outline width;
144 bytes). It applies morphs and
skinning, the affine transform, then the view-projection (or nothing for screen-space instances), Lambert shading
for lit instances, the tint or the vertex weights, the UV offset and the fog factor. `mesh_fsh.glsl` samples the
pool (binding 0, `sampler2DArray`) at the instance's layers, snapping screen-space text to texel centres so it stays
sharp under linear filtering, blends the three images for weighted instances, then darkening and fog;
`mesh_alpha_fsh.glsl` is the same with `discard` below the cutoff. Draws without the alpha test have no `discard`,
so early depth testing stays on.

### Clip space

`Backend::clip_space()` says how the projection must look: OpenGL uses depth -1..1 with y up; deko3d (device flags
`DepthZeroToOne | OriginUpperLeft`) uses depth 0..1 with y = -1 at the top, so the camera flips y. Front faces are
counter-clockwise as seen on screen in both cases.

## Backends

### OpenGL 4.3 (PC)

- Entry points are loaded by hand from the platform's `get_proc_address` (`src/gl/api.rs`, no crates). Calls with
  only value arguments have safe wrappers; calls with pointers or GPU-read offsets are `unsafe`.
- One vertex array object for everything (`glVertexAttribFormat`/`glBindVertexBuffer`: binding 0 = arena,
  binding 1 = slot stream with divisor 1). It never changes during a frame.
- Per frame: one `glMapBufferRange` (unsynchronised, because the fence has passed) of the slot buffer; one
  `glBindBufferRange` for the uniforms, one for the instance storage buffer, one `glBindBuffer` of the indirect buffer;
  per batch, program and fixed-function state only when (pass, flags) changes, a texture bind only when the pool
  changes, and one `glMultiDrawElementsIndirect`. GPU time comes from a `GL_TIME_ELAPSED` query read when the slot comes round again.
- Targets: `GlTarget::Offscreen` (the backend's own RGBA8 + depth framebuffer; `read_pixels` for screenshots) or
  `GlTarget::External { framebuffer, .. }` (the platform's window or headless framebuffer; the platform presents).

### deko3d (Switch)

- The C side (`c/slynx_render.c`) borrows the platform's device, queue and swapchain through `slynx_display_get()` and
  presents through `slynx_display_acquire()` / `slynx_display_present()` (`crates/platform-switch/c/slynx_shim.h`).
- Memory blocks: shader code; a Z24S8 depth image at the framebuffer's allocated size (1920x1080); the slot stream;
  per level the arena (CPU-uncached, GPU-cached) and the pool images (GPU-cached, uploaded through a staging block with
  `dkCmdBufCopyBufferToImage`) plus one image descriptor per pool and one sampler descriptor (written with
  `dkCmdBufPushData`); per frame slot uniforms + instance records (CPU- and GPU-uncached, so no cache maintenance
  between frames) and command memory.
- Per frame, one FFI call. The C side copies the uniforms and instance records into the slot, records one command
  list and submits it. The list does the following:
  - bind the render target, set viewport and scissor, clear;
  - bind the vertex layout, both vertex buffers, the index buffer, the uniform buffer, the storage buffer and the
    descriptor sets, once each;
  - per batch: bind shaders and fixed-function state only when (pass, flags) changes, and one texture handle only
    when the pool changes;
  - per draw command: one `dkCmdBufDrawIndexed`.
- Viewport and scissor follow the platform's current output size (1280x720 handheld, 1920x1080 docked).
- Limits: `max_instances` and `max_draws` from `Deko3dBackend::new`, and at most `min(max_draws, 4096)` batches per
  frame (`SLYNX_RENDER_MAX_BATCHES`). Every draw is checked against the arena and the frame's instance records
  before anything is recorded.
- Command memory: a budget per slot of 64 KiB + 64 bytes per draw + 256 bytes per batch, handed to the command
  buffer in 16 KiB chunks through deko3d's out-of-memory callback, so the bytes a frame used are known
  (`Deko3dBackend::cmd_usage`). If the budget runs out, the callback allocates extra blocks (freed when the slot
  comes round again) and the frame goes on; if that fails too, a 64 KiB reserve finishes the frame without the
  remaining draws, it is still presented, and `render` returns `LimitReached("command memory")`.
- Shutdown: the C side registers `slynx_render_exit` as a display exit hook (platform.md, "Shutdown order: display
  exit hooks"), so it is torn down before the device whichever of the platform and the backend is dropped first.
- deko3d has no multi-draw call. A bare indexed draw is a few command words with no driver work, which is the whole
  point of using deko3d. If recording ever shows up in profiles, the static part of a level's command stream can be
  captured once with `dkCmdBufBeginCaptureCmds` and replayed.

### Null

`NullBackend` draws nothing and keeps a copy of the last packet. The unit tests use it, and so does the CPU benchmark
of the front end (`cubes --null`).

## Frame budget

Target: a static Sly 1 level with about 1,500 sub-objects at 60 fps with large headroom, so well under 8 ms of
renderer CPU per frame on the Switch. The renderer's CPU work per frame is:

- **Build:** per instance, a sphere transform, 6 plane tests, a key and an 80-byte record; one sort of ~N keys. This
  is O(N) memory traffic plus O(N log N) on 8-byte keys.
- **Submit:** one copy of N x 80 bytes (+ 20 bytes per draw command on OpenGL), a constant number of binds, and per
  batch (a handful per frame) a few state calls; on deko3d one draw command per mesh run.

### Measured on PC

Headless, 1280x720, Mesa llvmpipe (software rasteriser, OpenGL 4.5 core context) in WSL2. Each run covers 590
frames after a warm-up. The scene has 3,072 boxes, of which about 2,350 are visible. It uses 1,536 distinct meshes,
16 textures in 2 pools and 3 passes.

| Run | Batches | Draw commands | API draw calls | Build CPU (mean / p99) | Submit CPU (mean / p99) |
|---|---|---|---|---|---|
| animated (all transforms change) | 4 | 1,793 | 4 | 0.13 / 0.71 ms | 1.90 / 2.64 ms |
| static | 4 | 1,793 | 4 | 0.15 / 0.72 ms | 2.02 / 2.83 ms |
| 1 mesh variant | 4 | 4 | 4 | 0.09 / 0.63 ms | 1.69 / 2.37 ms |
| front end only (`--null`) | 4 | 1,793 | 4 | 0.07 / 0.17 ms | 0 |

The submit time on llvmpipe is mostly llvmpipe itself, not API overhead. llvmpipe runs vertex processing for
indirect draws on the calling thread. The evidence:

- Submit time scales with the number of triangles: 0.16 ms for a 64-box scene, 1.7 ms for the full scene.
- Submit time barely changes with the number of draw commands: going from 4 to 1,793 adds about 0.2 ms.
- On a hardware driver this work runs on the GPU.

The backend-neutral cost for about 2,350 visible objects is 0.07 ms per frame on a desktop CPU. That leaves a lot of
headroom for the Switch's Cortex-A57 cores, which are several times slower.

Reproduce with `cargo run --release -p render --example cubes -- --frames 600 [--static|--variants 1|--null]
[--out frame.png]`.

### Real levels on PC (CPU front end)

`cargo run --release -p viewer --example scene_stats -- <disc> --bench <world>` (either game): the renderer's
build time per frame on the null backend (one desktop core), 600 frames each, from the spawn point and from an
overview of the whole level. Submit time is not in these numbers.

| World | Spawn: visible / draws / batches / build mean, p99 | Overview: visible / draws / batches / build mean, p99 | Instance records |
|---|---|---|---|
| Sly 1 Paris, France | 1,523 / 1,052 / 93 / 0.09, 0.17 ms | 6,916 / 3,558 / 548 / 0.33, 0.58 ms | 540 KiB |
| Sly 1 Prowling the Grounds | 3,129 / 1,933 / 216 / 0.14, 0.32 ms | 4,553 / 2,527 / 355 / 0.19, 0.37 ms | 355 KiB |
| Sly 1 A Cunning Disguise | 4,332 / 2,073 / 299 / 0.18, 0.33 ms | 3,374 / 1,623 / 295 / 0.14, 0.25 ms | 338 KiB |
| Sly 1 A Rocky Start | 2,900 / 2,041 / 203 / 0.13, 0.29 ms | 3,535 / 2,321 / 449 / 0.15, 0.26 ms | 276 KiB |
| Sly 2 jb_intro (row 0) | 11,272 / 3,488 / 178 / 0.54, 0.87 ms | 8,424 / 2,662 / 160 / 0.44, 0.84 ms | 880 KiB |
| Sly 2 f_nightclub_exterior (row 2) | 8,805 / 4,461 / 516 / 0.46, 0.75 ms | 15,689 / 6,923 / 937 / 0.76, 1.24 ms | 1,225 KiB |
| Sly 2 p_prison_ext (row 14) | 14,699 / 4,468 / 410 / 0.73, 1.18 ms | 15,632 / 4,605 / 403 / 0.77, 1.26 ms | 1,221 KiB |
| Sly 2 p_castle_ext (row 17) | 10,219 / 3,835 / 248 / 0.51, 0.93 ms | 10,055 / 3,880 / 279 / 0.49, 0.79 ms | 798 KiB |
| Sly 2 a_blimp_ext (row 38) | 8,450 / 3,008 / 269 / 0.43, 0.68 ms | 13,935 / 3,996 / 437 / 0.67, 0.99 ms | 1,088 KiB |

The heaviest Sly 1 worlds from their spawn points (harness, all 46): A Stealthy Approach, Prowling the Grounds,
A Cunning Disguise; the heaviest Sly 2 worlds: rows 14, 26, 37, 6 and 0. Looking away from a level (culling only)
costs 0.02 to 0.09 ms. The sort now compares one packed 128-bit key instead of a tuple, which took about 10 % off
the build time of the heaviest Sly 2 world (0.81 to 0.73 ms).

The GPU times the harness prints on PC come from llvmpipe (software rasterising in WSL2) and say nothing about the
Switch.

**Switch estimate.** With a Cortex-A57 core at 1 GHz about 5 to 8 times slower than the desktop core for this
work, Sly 1 builds in about 1 to 2.5 ms and Sly 2's heaviest views in about 4 to 7 ms. Two more costs grow with
Sly 2's size and need measuring on hardware:

- **Draw commands.** deko3d records one indexed draw per command: about 4,500 to 6,900 in Sly 2 overviews against
  1,000 to 3,500 in Sly 1. Sly 2's world geometry is mostly unique meshes placed once, so instancing does not merge
  them.
- **Instance records.** 0.9 to 1.2 MiB per frame copied into uncached GPU memory in Sly 2.

Static merging (below) cuts both for Sly 2; Sly 1 is well inside the budget as it is.

### Static geometry merging (Sly 2)

`Renderer::add_placed_mesh(mesh, pieces)` makes several placed pieces one mesh: each piece is a vertex range in its
own model space with a placement transform, a texture (one pool per mesh) and a tint. The placements (three
transform rows, texture layer, tint) go in the static outline-point buffer and each vertex finds its piece through
its second-stream bytes, so the backends are unchanged. The vertex shader applies a placement through the same
expression as an instance transform (normals included), so the pixels are bit-identical. Mirroring placements go in
their own meshes, drawn with `Renderer::set_mirrored`.

`stage::prep` (used by the viewer and the game) merges unsorted Sly 2 and Sly 3 draws by drawing state, 4,096-unit cell and winding (the
renderer's rule: the sign of the transform's determinant), and keeps the draw order wherever it can change a
pixel. The renderer draws an unsorted list by mesh, then instance; a group's placed meshes are uploaded right after
the mesh of its first piece, so they take that place in the order. A piece joins its group only if that moves it in
front of nothing it could tie with: blended pieces keep their order always, opaque and cut-out ones when their world
bounds come within 8 units (closer, the depth test could tie); otherwise it starts a new group. `SLYNX_VIEWER_NO_MERGE=1`
turns merging off on PC, for comparing captures.

Captures with and without merging are identical (every pixel, 1280 x 720) for the nightclub cellar and rows 0, 2,
5, 10, 14, 20, 30 and 40 from their spawn points. Measured with the harness (OpenGL on Mesa in WSL2):

| World (spawn view) | Draw commands | Instance records | Build (median) |
|---|---|---|---|
| Row 14 | 4,468 to 2,826 | 14,699 (1.12 MiB) to 2,894 (226 KiB) | 2.4 to 0.2 ms |
| Row 2 | 4,461 to 2,823 | | 1.3 to 0.2 ms |
| Row 0 | 3,488 to 2,312 | | 1.1 to 0.2 ms |

Keeping the order costs merges: without it the same views took about 1,900 draw commands at row 14. The rest are
blended (far-to-near) draws, which cannot be merged without changing their order. Bigger cells save a few hundred
more draws at the cost of coarser culling.

### Sly 2 and Sly 3 game sessions (characters and shader lighting on)

The Sly 2 and Sly 3 game sessions with skinned characters drawn and their two colours lit per frame in the vertex
shader (see Characters). Frame times are the smoke run's (null backend, so CPU only: game update, stage and the
renderer's build) on a desktop CPU, 10 s per world; the counters are the harness's (OpenGL on Mesa in WSL2):

| World (spawn view) | Frame p95 | Draw commands | Triangles | Build (median) |
|---|---|---|---|---|
| Sly 2 hub (row 2) | 0.92 ms | 1,530 | 91,600 | 0.22 ms |
| Sly 2 row 14 (heaviest p95) | 1.15 ms | 554 | 24,000 | |
| Sly 2 row 0 | | 458 | 25,000 | |
| Sly 2 row 3 | | 432 | 19,000 | |
| Sly 3 heaviest (v_ext) | 0.45 ms | | | |
| Sly 3 v_apt | | 294 | 9,700 | |
| Sly 3 slot 15 | | 415 | 33,000 | |
| Sly 3 slot 23 | | 404 | 22,600 | |

Sly 3's first frame after a load used to take up to about 31 ms (m_ext): the follow camera configured itself on its
first update, building its collision world from every solid. The session now configures it with the load, and the
worst single Sly 3 frame is 2.3 ms (c_ext). Sly 2's first frame took 4 to 11 ms: reactive props (class 98) set off
the explosions listed before their first event at load, filling the particle pool (8,192) on frame 0. Only their
intact look applies at load now, and the worst single Sly 2 frame is 2.5 ms (row 14). The light
block adds 15 rows (five bone matrices) per lit part and no per-vertex CPU work.

Switch-class estimate for the hub, the heaviest view:

- CPU: the Switch's A57 cores are about 5 to 8 times slower than the desktop, so a 1.15 ms p95 becomes about 6 to
  9 ms.
- Recording about 1,500 draw commands in deko3d: about 1 to 1.5 ms.
- GPU: vertex work for about 92,000 triangles, character lighting included, is about 0.1 ms; filling 720p at about
  three times overdraw is about 1 to 3 ms, and it overlaps the CPU.

That is well under the 16.7 ms frame at 60 FPS. It is an estimate until the overlay's GPU timestamps (Performance
overlay and log) are read on hardware.

### On the Switch

Not measured on hardware yet (see below). The demo logs the following once a second through `svcOutputDebugString`:

- FPS;
- renderer CPU (build + submit);
- fence wait and update time;
- visible instances, batches, draw commands, triangles.

## Platform interface

What the renderer needs from the platform layer, and what it uses today.

**Switch** (provided by `crates/platform-switch`, `c/slynx_shim.h`, section "Display"):

- `const SlynxDisplay* slynx_display_get(void)`: device, graphics queue, swapchain images, allocated size
  (`fb_width`/`fb_height`) and current output size (`width`/`height`). The renderer allocates its depth buffer at the
  allocated size and renders at the current size.
- `int slynx_display_acquire(void)` and `void slynx_display_present(int)`: the renderer submits its frame between
  the two, on the display's queue.
- Device flags `DepthZeroToOne | OriginUpperLeft` (the platform default). Another choice must be reflected in
  `Deko3dBackend::clip_space`.
- Nothing else may submit to the queue or present while the renderer is alive (the debug draw must not be used at
  the same time).
- Rust: `Deko3dBackend::new(&SwitchPlatform, max_instances, max_draws)`. The platform must outlive the backend.

**PC** (provided by `crates/platform-pc`):

- A current OpenGL 4.3 core context on the render thread, and its `get_proc_address`.
- The framebuffer to draw into and its size. 0 is the window's default framebuffer; in headless mode it is the
  platform's offscreen framebuffer. The platform presents (swaps) after `render`.
- Use: `GlBackend::new(&mut |name| ctx.get_proc_address(name), GlTarget::External { framebuffer:
  ctx.default_framebuffer(), width, height })`.
- The renderer's own `gl::egl::HeadlessGl` exists only so the crate can be tested on its own. It does not replace the
  platform's context.

## Build

- PC: nothing special; the library has no dependencies (the cubes example uses `harness` as a dev-dependency). Headless OpenGL needs `libEGL.so.1` with Mesa at run time
  (opened with `dlopen`); `tests/gl_headless.rs` skips itself when it is missing.
- Switch: any program that uses the renderer is built with `tools/build-switch.sh` and its hooks (see
  [switch-build.md](switch-build.md)): `SLYNX_EXTRA_C=crates/render/c` compiles the deko3d backend's C side, and
  `SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_` compiles the shaders to `slynx_render_<name>.dksh`, which
  `c/slynx_render_shaders.S` embeds. Wrappers that set the variables:
  - `crates/render/tools/build-switch-demo.sh [--debug]`: the box-grid demo (`--features switch-demo`) into
    `dist/slynx-render-demo.nro`;
  - `crates/viewer/tools/build-switch.sh [--debug]`: the level viewer into `dist/slynx-viewer.nro`.

## Performance overlay and log

For hardware tests every Switch program (the viewer and the Sly 1, 2 and 3 game NROs) has the same overlay and log
(`render::perf`):

- **Toggle:** hold L and R and press ZL. Two lines in the bottom-left corner: frames per second, frame time, the
  program's CPU time per frame (update and render, without the wait for the display) and the renderer's share of
  it, the GPU time, draw commands and triangles. Averages over about 16 frames.
- **GPU time** comes from deko3d timestamp reports: one at the top of the pipe when a frame's commands start, one
  at the end of the pipe before its fence; read when the frame slot comes round again, so the number is three
  frames old. The reports are 32 bytes per slot (`SlynxRenderStats::gpu_ns`, `FrameStats::gpu_ns`), in GPU-uncached
  memory so they reach memory without a cache flush before the fence; the timer runs at 614.4 MHz (ticks x 625 /
  384 = ns). OpenGL reports none.
- **Log:** `perf.log` in the user folder (`sdmc:/switch/SlyNX/user/perf.log`), one line every five seconds: time,
  frames, FPS, average and worst frame time, average and worst CPU time, average and worst GPU time, average draw
  commands. The lines are kept all along, but the file is written only while the overlay is shown
  (`PerfMeter::log_due`), so normal play never writes to the card from the game thread; turning the overlay on
  writes what was kept. It keeps its header and its newest 32 to 64 KiB and is rewritten (atomically) each time.
- **Cost:** the averages every frame; while shown, one text update (about 60 glyph instances). The text is the
  renderer's own (`Renderer::enable_perf_text`, `set_perf_text`), enabled before the first level is loaded and
  added with every level commit, so programs need no level resources for it. It is optional: a level whose
  textures already use every pool (`MAX_POOLS`) or instance it needs is committed without it rather than failing.

## Not yet verified on Switch hardware

The `.nro` builds and links. The following have not been checked on hardware:

- **Front-face convention.** The renderer assumes a counter-clockwise on-screen triangle is front-facing under
  `OriginUpperLeft` with the y-flipped projection. In the demo, press X to cycle the cull mode. With "Back" the boxes
  must look solid; if they look hollow, build the C side with `-DSLYNX_RENDER_FRONT_FACE_CW`.
- **Skinning, morphs and the second vertex stream on deko3d.** Attributes 5 to 7 (`4x8` Uint, `4x16` Unorm, `1x16`
  Uint) from vertex buffer 2, and storage buffers 1 and 2 in the frame slot. Checked on OpenGL by
  `tests/gl_headless.rs` (`gl_skins_and_morphs`) and in the viewer; not on the console.
- **Instanced attribute base.** deko3d's divisor-1 vertex stream is assumed to start at the draw's `firstInstance`,
  as on OpenGL. If every instance of a draw shows the first record's transform, this is the cause.
- **Texture upload into array layers.** Each layer and mip is copied with `dkCmdBufCopyBufferToImage`, using
  `DkImageRect.z` as the layer and `mipLevelOffset` on the view as the mip.
- **Cache behaviour.**
  - Per-frame data is in GPU-uncached memory: correct, but measure whether GPU-cached memory plus an L2 invalidate
    per frame is faster.
  - Re-uploading a level invalidates the image, shader, descriptor and L2 caches.
- **Command memory budget.** The budget is 64 KiB + 64 bytes per draw + 256 bytes per batch per slot (at most 320 bytes per draw when every draw is its own batch). Check the real per-draw
  size.
- **Docked and handheld switching.** Viewport and aspect follow the platform's current size.
- **CPU timings.** None measured on the Switch yet.
- **Batch state on deko3d:** depth functions, the stencil volume steps (two-sided stencil ops with
  `dkCmdBufSetStencil`), colour writes off, front-face culling; and the per-level weight buffer (storage
  buffer 3, in the arena memory block). Checked on OpenGL only.
- **Silhouette render target, video planes and masked areas on deko3d.** The silhouette image (render target, then
  texture after a fragment barrier), the per-frame plane copies and their descriptors, the alpha-only colour mask
  and the separate alpha blend factors are checked on OpenGL only (`gl_projected_shadow`, `gl_video_frame`, the
  masked-area state test).
- **Views and the new blend modes on deko3d.** Picture panels rely on deko3d's clears being limited to the scissor
  rectangle; the reverse-alpha, reverse-additive and destination-alpha blend factors and the per-view uniform
  blocks are checked on OpenGL only (`gl_panels_iris_and_blends`). The C side's batch check now accepts the outline
  pass (7); before, a frame with outlines was refused on the Switch.
- **Overlay pass, mirrored and additive batches.** The overlay disables the depth test and face culling; mirrored
  batches swap the front face; additive batches set the colour destination factor to `One`. Checked on OpenGL
  only. In the viewer, text that is upside down would mean the overlay's clip-space y is wrong for deko3d.

## Sly 2 and Sly 3 levels (`stage`)

The `stage` crate turns a `formats-sly2` or `formats-sly3` scene build into renderer resources the same way for the
viewer and the game: drawing state by pass and shader flags, the two colours of every vertex (`stage::light`), merged
static geometry, kinds 2 and 3 as stencil volumes, texture animation (`stage::texanim`). It depends on `formats*`
and `render` only. A world loads in four calls:

```rust
let lighting = stage::light::WorldLighting::new(&build.lights, &world.options, world.data);   // Sly 2
// Sly 3: WorldLighting::with(&build.lights, &[option set 0], world.data, &WorldOptions::SLY3)
let input = stage::prep::SceneInput { skip: &character_ranges, ..stage::prep::SceneInput::sly2(&build, Some(&lighting)) };
let prepared = stage::prep::prepare(input)?;                 // any thread
r.clear_level();
let up = stage::geometry::upload(r, prepared.geometry.as_ref())?;   // before commit
// ... other resources (the game's skinned characters use up.textures) ...
r.commit()?;
let instances = stage::geometry::place(r, prepared.geometry.as_ref(), &up)?;
r.set_fog(prepared.fog);
r.set_fog_rising(true);
let mut anims = stage::texanim::Animator::new(&prepared.anims, &up.materials, &up.textures);
// every frame: anims.animate(r, dt)?; render with prepared.clear as the clear colour
```

`SceneInput::skip` leaves out scene instances whose record falls in the given byte ranges (drawn by another path).
The game wraps these calls in `game::sly2::level::upload_stage` (Sly 2 and Sly 3 sessions).

**Zone masks** (Sly 2 camera.md 0): every draw carries its record's zone mask (`SceneBuild::instance_zones`,
`Geometry::draw_zones`), and static groups never mix masks (the mask is part of the group key, so merging still pays:
a world has few distinct masks). `ZoneVisibility::new(geo, &up, &draws, &statics)` with `place_all`'s instances,
then once per frame `zones.apply(r, world.shared.visibility_mask)` shows only the draws sharing a bit with the
camera's mask (0xFF, outside every visibility volume, shows all); it does nothing while the mask is unchanged.

**Characters** (`game::sly2::skin`, both games): each character's rigid meshes become skinned meshes posed every
frame from their bone tables (`CharacterSkins::update`, culled by a sphere around the posed mesh). Only what is drawn
as the character is kept: meshes admitted by the row (Sly 2 level conditions) or the world filter (Sly 3), whose
shader kind and pass are drawn as the level's are (shadow hulls and projected volumes, kinds 2 and 3, are left
out); each gets the material its shader's look calls for and two colours per vertex as a moving object (four
strongest lights at the character's centre) computed in the vertex shader every frame, so characters relight as
they move. Sly 3's crew (classes 52 to 60) is drawn the same
way (`crew_draws`). 

**Attached objects** (`game::sly2::attached`, Sly 2): the objects attached under a character in the object tree
(gadgets parked on them, such as Bentley's bombs with their blast volumes and the trap; switch rings and bursts;
held items) are not static. Their scene draws stay instances (`SceneInput::dynamic`, which wins over the
characters' skipped ranges since these records nest in theirs) and are listed in `Prepared::dynamic`. Each keeps
its placement relative to its object at load. Several objects can share a record (a pool's copies are proxies of
it), so each object sharing it gets an instance of the record's draws. Every frame, `AttachedParts::update`
moves each part to its object's world transform and shows it while the object is in the world, active and visible,
and:

- a gadget object (classes 100 to 102) only while a copy of it is out of its pool (`World::shared.gadget_copies`,
  `engine::gang::pool`); the templates left in the pools' groups are not drawn;
- a bomb's mesh groups (gadgets.md 7.1.1: per bomb kind a sphere and four light cards, all hidden at load) only for
  the copy's own kind (3968 Trigger, red 325; 3965 Snooze, green 325; 3962 Reduction, green 620; 3947 Rage, purple
  620), told apart by the cards' transforms (15–18, 11–14, 7–10, 3–6) and the spheres' radius and colour
  (`attached::bomb_group`): hidden while held or flying, shown while landed (armed, state 2), blinking for
  `BLINK_TIME` (1.87 s; hidden over frames 20–30, 50–59, 68–75, 84–90, 98–103 and from 108) once triggered (state
  3), hidden after. The other kinds' groups never show.

The harness options `place_bomb=1` and `trigger_bomb=FRAME` (Sly 2 app) have Bentley take a Trigger Bomb and place
it between him and the player (`gang::pool`), then trigger it: in the cellar (row 3) the red sphere shows while
armed and blinks in the countdown. Drawn in place at the spawn instead (before this), these objects made a flat
green shape over Sly's head there.

## Hooking up `crates/game`

The game renders through the stable subset: `Renderer::new`, `add_mesh` / `add_skinned_mesh`, `add_texture`,
`add_material`, `commit`, `add_instance`, `set_transform`, `set_visible`, `set_tint`, `set_material`, `render`,
`clear_level`, `TextOverlay`. Changes since the first version that callers must follow:

- `Material` has new fields; build it from `Material::opaque(texture)` and `with_pass(pass)` and override fields
  with `..`. `lit: bool` became `lighting: Lighting` (`lit: false` is `lighting: Lighting::Unlit`). `Pass` gained
  `Background`, `Volume`, `Post`, `Outline`; `BlendMode` gained `None`, and for Sly 2 `ReverseAlpha`,
  `ReverseAdd` and `DestAlpha`.
- `FramePacket` has `views` (empty unless picture panels or shadow casters are set) and `video`; only backends see
  them. `Pass` gained `MaskWrite` and `MaskFill`, `BlendMode` gained `InvDestAlpha` and `Multiply`.

**Movies**: `let video = VideoLayer::new(r)?` with the level's resources; per decoded frame
`r.set_video_frame(Some(VideoFrame { width, height, y, u, v }))?` and
`video.show(r, VideoLayer::fit(r.backend().target_size(), (width, height)), [1.0; 4])?`; `video.hide(r)?` and
`r.set_video_frame(None)?` at the end.

**Projected shadow**: `r.set_projectors(&[Projector { .. }])` from the player's options 43 to 48 (and the shadow
shader's material alpha), `r.set_shadow_caster(id, true)` on the player's instances, `r.set_receiver(id, bits)`
with the mesh flags byte's bits 0x01 and 0x02; `viewer::level::player_projector` and `upload_with_ui` show it.
Move the projector's `origin` with the player every frame.
- `GpuInstance` is 80 bytes and `FrameUniforms` 144 (only backends see them).

**Picture panels from `RenderScene::camera_extras`**: each `Some((eye, target, fov, screen))` of
`CameraExtras::panels` is one `Panel { camera: Camera { eye, target, up: Vec3::Z, fov_y: fov, near, far }, rect,
clear }`; pass them to `Renderer::set_panels` every frame (an empty slice when there are none). `screen` is the
panel's position (options 991 and 992); the panel's size and the units of `screen` are the game's to choose,
converted to target pixels for `rect`. Draw the panel frames with the UI layer: they cover the panels.

**Wipes**: create `iris::IrisShape::new(r, &points, sets, colors, bounds)` with the level's resources (before
`commit`) from the world's screen mask (class 142) and the colours of shaders 1166 to 1170
(`viewer::iris::ScreenMask::of_world`, `ScreenMask::colors_of_world`, `ScreenMask::set_slices`). Each frame of an
iris wipe call `iris.set(r, centre, pixels_per_unit, u)` with `u` the closing amount (0 open, 1 closed);
`viewer::iris::iris_placement(player_px, u, target)` gives the centre (the player's screen position kept off the
edges) and the scale. `iris.hide(r)` when it ends. Fades stay a flat full-screen UI quad.

**Dialogue panels**: `panel::PanelFrames::new(r)` with the level's resources. Each frame a panel is up:
`r.set_image_panels(&[PanelFrames::view(Side::Left, camera, rect, clear)])`, `frames.update(dt)` and
`frames.show(r, Side::Left, rect, outline_rgba)`; `PanelFrames::rect(target, side, scale)` is the rectangle (HUD
elements 26 and 27). `frames.hide(r, side)` and an empty `set_image_panels` when it closes. The spec gives no
outline colour; the caller chooses it.

**Skinning from `RenderScene.skins`** (engine `SkinFrame`): for each `(drawable, range)` in `skins.skins`, take
`skins.matrices[range]` (mesh-space `Affine`s), convert each to `render::BoneMatrix` (three rows:
`[c0.x c1.x c2.x t.x]`, `[c0.y c1.y c2.y t.y]`, `[c0.z c1.z c2.z t.z]`; `viewer::anim::bone_rows` does it) and call
`set_bones(instance, &bones)` for every instance drawn from that drawable's skinned submeshes. Their meshes must have
been added with `add_skinned_mesh` and the submesh's influences (`engine::anim::SkinnedSubmesh::influences`, converted with
`SkinVertex::new(bones, weights)`), and the instance transform is the drawable's world transform. Give animated
instances a culling sphere with `set_bounding_sphere` (`viewer::anim::posed_sphere` grows the rest sphere by the
matrices). Same matrix count each frame: the bone store range is reused, so the per-frame cost is a copy of 48 bytes
per bone. `viewer::anim::PlayerAnim::apply` is a complete example.

## Real levels

The level viewer draws every Sly 1 world through this renderer; [viewer.md](viewer.md) has the numbers. In short:
Paris is 6,400 submesh instances (about 185k triangles); from the spawn point 1,175 are visible in 721 draw commands
and 42 batches, built in 0.045 ms on a desktop CPU; with the whole level in view, 6,100 visible, 2,895 draw commands,
243 batches, 0.17 ms. Translucent-heavy worlds have the most batches (Boneyard Casino: about 450 from its spawn),
because blended draws are strictly back to front and every texture-pool change along that order is a batch.

## Next steps

- Lighting and fog for the Sly 1 levels (needs the rendering spec); BC1/BC3 pools.
- Static and dynamic instance split: upload static transforms once, and write only moving objects per frame.
- Fewer blended batches if hardware shows a need (depth buckets, or a pool-indexed texture table).
- Optional: capture and replay of static command streams on deko3d.

## Projectile streaks (`game::shot_marks`)

For the renderer owner: the machines' projectiles (`engine::behavior::vehicles::shots`) have no mesh or effect in
the specs yet, so the Sly 2 and Sly 3 sessions draw each one as a placeholder: a unit box stretched along the shot's
velocity (0.02 s of flight, 40 to 400 units long, 10 wide), unlit, warm yellow (255, 220, 120), not fogged or
darkened, its head on the shot. The mesh, a 1x1 white texture and the material are made in `upload_stage`'s
before-commit closure (next to the 2D layer, marked `Vehicles hook`); instances are pooled and hidden when unused,
and placed in `render` before the 2D layer. To replace it with the game's projectile meshes or effects later, only
`ShotMarks::upload` and `streak` change.
