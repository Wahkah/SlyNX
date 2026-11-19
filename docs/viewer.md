# Level viewer

Status: phase 1 milestone, 2026-11-13. Crate: `crates/viewer`.

The viewer loads a Sly 1 or Sly 2 world from the player's own disc image and shows its static scene with a free
camera (Sly 2: see [Sly 2](#sly-2) below). It is the phase 1 gate ("a Rust build loads and renders a Sly 1 level with a free camera at 60 fps on the Switch") and
the base for the image and performance tests of the renderer on real data.

## Running it

### Switch

```sh
crates/viewer/tools/build-switch.sh            # release -> dist/slynx-viewer.nro (title "SlyNX")
crates/viewer/tools/build-switch.sh --debug    # debug   -> dist/slynx-viewer-debug.nro
```

The wrapper sets `SLYNX_CRATE=viewer`, `SLYNX_FEATURES=switch-app` (the Switch `main`; without the feature the
crate is a plain library, so other Switch programs can use its level conversion), `SLYNX_NAME=slynx-viewer`,
`SLYNX_TITLE=SlyNX`,
`SLYNX_EXTRA_C=crates/render/c` and `SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_` and runs
`tools/build-switch.sh` (see [switch-build.md](switch-build.md)). The release `.nro` is about 670 KB.

Put the disc image at `sdmc:/switch/SlyNX/sly1.iso` (a Sly 2 disc at `sdmc:/switch/SlyNX/sly2.iso`). Another place
can be set in
`sdmc:/switch/SlyNX/user/viewer.cfg`:

```text
# key = value, # starts a comment
disc = sdmc:/isos/Sly Cooper.iso     # sdmc:/..., romfs:/... or a path relative to the SD card's root
world = The Hideout                  # first world: a name, part of a name, or a table index (default Paris)
```

Without a `disc` line the viewer tries `sdmc:/switch/SlyNX/sly1.iso`, `sdmc:/switch/SlyNX/sly2.iso`, then
`romfs:/sly1.iso` and `romfs:/sly2.iso` (for a packaged build that carries the image in its RomFS). The first image
that exists is opened, as Sly 1 or as Sly 2 (the game is told by the disc's executable). Without a disc it shows the paths it tried on screen and keeps running until
**+** is pressed. Panics are written to the debug log (`svcOutputDebugString`), and so is a statistics line every
five seconds.

### PC

```sh
export SLYNX_DISC_SLY1=/path/to/sly1.iso
cargo run --release -p viewer --example viewer_pc -- [disc.iso] [world]            # window
cargo run --release -p viewer --example slynx_harness -- run --app viewer \
    --opt world=Paris --frames 120 --capture 60 --out DIR [--warmup 10]           # headless, captures + perf
cargo run --release -p viewer --example scene_stats -- [disc.iso] [world | --all | --bench world]
```

`slynx_harness` is the harness's command-line tool (see [testing.md](testing.md)) with the viewer registered as
`viewer`. Options of the viewer app:

| Option | Meaning |
|---|---|
| `game=sly2` | take the disc from `SLYNX_DISC_SLY2` (a disc given with `disc=` is recognised either way) |
| `world=NAME` | Sly 1: world name, part of a name (`hideout`), id (`0x103`) or table index; default Paris. Sly 2: list index (0 to 42), world id (`0x3B`) or level-table row (`row3`); default the first row |
| `disc=PATH` | disc image of either game; default `SLYNX_DISC_SLY1` (`SLYNX_DISC_SLY2` with `game=sly2`) |
| `overlay=1` | draw the stats overlay. Off by default: the timings it prints differ between runs, so captures with it are not reproducible |
| `hud=0` | leave out the HUD (on by default) |
| `camera=x,y,z,yaw,pitch` or `camera=x,y,z,tx,ty,tz` | start somewhere other than the spawn point (game units, radians) |

The harness loads worlds synchronously (also for L / R in an input script), so runs are deterministic
(`--check-determinism` passes with the overlay off). Captures are made from the game's data: keep them outside the
repository and never commit them (testing.md, "Golden images: the rules").

`scene_stats` prints what the viewer draws for a world (shader kinds, passes, blend flags, texture sizes, vertex
colour ranges), `--all` loads all 46 worlds the way the viewer does, and `--bench` times the renderer's CPU work per
frame on the null backend.

## Controls

| Switch | PC keyboard (player 1) | Action |
|---|---|---|
| Left stick | WASD | fly forward / back, strafe |
| Right stick | IJKL | look |
| ZR / ZL | 3 / 1 | up / down |
| B (held) | Z | 5x speed |
| D-pad up / down | arrows up / down | double / halve the base speed (12 m/s to start) |
| R / L | E / Q | next / previous world (loads in the background) |
| X | V | stats overlay on / off |
| Y | C | back to the spawn point |
| + | Enter (Esc closes the window) | quit |

## What happens

1. **Disc.** `Disc::open` checks the executable and reads the world table. On the Switch the image is read through
   the platform's file API (`loader::FsReader`, positioned reads, no stdio buffering).
2. **Load, on the I/O thread** (core 1, low priority, CPU boost on): read and decompress the world, `World::load`,
   `Scene::build`, then `level::prepare`:
   - converts each drawn submesh to the renderer's 28-byte vertex (snorm8 normals) once, however many times it is
     instanced;
   - builds full mip chains for the decoded textures and adds a white texture for materials without one;
   - classifies each (shader, mesh) pair (below) and makes one renderer material per (shader, pass);
   - makes one draw per drawn submesh of every scene instance, with the instance's transform;
   - finds the spawn point.

   The job owns the disc while it runs and hands it back with the result through a one-value mailbox. While a world
   loads, the current one stays on screen and the overlay says "loading"; another L / R meanwhile queues the latest
   request. Loading a world takes 40 to 60 ms on a desktop CPU.
3. **Upload, on the game thread**: `level::upload` clears the renderer's level, adds the meshes into one arena, the
   textures into pools (one 2D array per size), the materials and the overlay's glyphs, commits once, then adds the
   instances.
4. **Frames**: controls, camera, poll the loader, update the overlay text, `Renderer::render`.

### Space

Everything stays in game space (right-handed, Z up, about a centimetre per unit). The camera is given +Z as its up
vector, so the view matrix applies the Y-up conversion of `specs/sly1/transforms.md` 7.1; no vertex or transform is
converted, and since the conversion is a rotation the winding is unchanged. Front faces are counter-clockwise and
back faces are culled. Instances whose transform mirrors (negative determinant; 0 to 12 per world) are found by
the renderer, which flips the front-face rule for their batches.

The camera has a vertical field of view of 0.95 rad and depth from 20 to 150,000 units.

### Spawn point

If a drawn object is the player character (class 7) or below one, the camera starts 700 units behind it (along its
local X axis, flattened) and 350 above, looking at it. Otherwise it starts above the centre of the level, looking
at it. The level box is the 2nd to 98th percentile of the drawn instances' centres on each axis, because a few
worlds place an object far outside the playable area (A Rocky Start has one 450,000 units up). The Hideout has no
player character and uses the box.

### How it is drawn

From `specs/sly1/rendering.md`; the section numbers below are that file's.

- **Lists (3.1).** A submesh's list comes from its **mesh's** pass byte, not its shader's, and its state from the
  shader's kind and blend flags (`level::classify`), as the renderer's material order and pass:

  | Mesh pass | Viewer list | Drawn as |
  |---|---|---|
  | 1, blend flag not 2 | 1 | backdrop: no depth test or writes, not blended, drawn first |
  | 1, flag 2 | 2 | backdrop, alpha over, sorted far to near |
  | 3 | 5 | opaque |
  | 4, flag not 2 / flag 2 | 6 / 7 | two-step, additive / alpha over sorted far to near |
  | 6 (kind 4) | 9 | stencil volume (blob shadows, light pools), the shader's second colour |
  | 7 | 10 | opaque |
  | 8 | 11 / 12 | as pass 4 |
  | 13 | 16 | two-step, sorted far to near; flags 2 and 6 alpha over, others additive |
  | 5 / 9 | 8 / 13 | outlines (below) |
  | 17 | 20 | after the scene, no depth test or writes, alpha over, not darkened; only in The Hideout (below) |

  Not drawn: passes 0, 2, 10 to 12, 14 to 16 (16 is the binocucom markers, shown only in that view), outline
  passes 5 and 9 (below), and shader kinds 2, 3, 5, 9 and 10.
- **Sort keys (3.1):** the mesh's draw-order value `g * |g|` when it has one, else the squared camera distance.
- **Two-step (3.2):** a prepass that keeps texels with alpha at least 0.9 and writes depth, then the blended pass
  without depth writes. The renderer runs all prepasses of a list before its blended draws (the reference
  interleaves them per submesh).
- **Colours (1):** vertex and material colours 0xFF = 1.0; palette, fog and clear alpha 0x80 = 1.0.
- **Lighting (2):** kind 0 materials blend their three palette images per pixel with weights computed per vertex
  at load time from the world's lights (class 81: directional and point, spots as points), the world's shadow and
  mid-tone levels (options 527, 526) and the submesh's emission (self-illumination x 0.003973); the weights,
  4 bytes x 2 per vertex per instance, go into the renderer's weight buffer. Kind 1 and the rest: vertex colour x
  material colour x texture. Animated characters are lit once, at rest.
- **Fog (4)** from options 532 to 535 with each mesh's fog factor; **clear colour** from option 528.
- **Backdrops (3.4):** the sky object (class 84) follows the camera in X and Y (and Z with option 608).
- **Billboards (6.1):** turned each frame about their sphere centre: local X toward the viewer along the view axis,
  local Z along the camera's up axis.
- **Texture animation (8):** kinds 1 (loop), 2 (back and forth), 3 (random), 5 (blink) change the frame; 6
  (scroll) and 7 (circle) move the UVs; 8 (eye look) stays at offset 0 without a look target. Both shader-level
  records and mesh-level ones (for the shader they name) are played, with a generator seeded per animation so runs
  are reproducible.
- **Blob shadows (9.1):** the shared shadow mesh (OID 29) under every object with option 41, scaled by
  (radius / 100, radius / 100, depth / 100), drawn as a stencil volume with its second colour (alpha 0.203 for
  `00 00 00 34`). Objects stay at their placed positions.
- **Darkening (10):** the renderer's factor is 1 (nothing in the viewer triggers darkening); mesh-flag bit 0x04 and
  pass 17 are exempt.
- **Outlines (5):** every pass-5 and pass-9 mesh of a placed object without bones becomes a renderer outline mesh
  (one quad per edge). The GPU draws only the silhouette edges (both far corners on the same side of the projected
  edge), 4.125 pixels of a 448-line frame wide (6.6 pixels at 720p), in the object's option-95 colour or
  (16, 16, 16, 255), not darkened or fogged. Beyond the mesh's outline distance plus its radius an outline is hidden;
  over the last 400 units its alpha falls with squared distance. The player character's outlines are left out (they
  would stay at rest while the character moves).
- **Draw distance (5, 6.3):** opaque and cut-out draws of meshes with a draw distance fade the same way, switching to
  a translucent twin material (sorted, two-step) while they fade, and hide beyond.
- **Moving objects (2.3):** the animated player character is lit once per object, from its parts' sphere centres,
  without the lights that have option 501.
- **Pass 17:** the map-screen pieces belong to the map objects (class 103), which the game shows in its map view
  (level-flow.md 8). The Hideout holds all five episode maps as pictures on its walls, so pass 17 is drawn in a world
  with more than one map object (only The Hideout) and left out elsewhere until a map view exists.

### HUD

The game's HUD from `crates/ui` (`viewer::hud`), as a demo of the renderer's UI layer: the fonts of the world and
the tables and placements of the executable, read with the level; demo values (3 lives); the counters slide in at
the start and on X. `ui`'s draw list goes to `render::ui2d::UiLayer` each frame: one draw over a clamped glyph sheet,
flat quads by flag, in list order.

### The player character

When the world has the player character (class 7) and its segment, the viewer animates it (`viewer::anim`, with
`engine::anim`):

- the rig is the proxy holding the character and everything below it (`Rig::from_stream`), posed at rest from the
  stream's world transforms; a `Skin` per skinned mesh set below it, with its bind matrices captured at rest;
- the segment is OID 130, the first required animation segment of the player's lookup list (animation.md 7, 180
  frames in Paris). The spec does not name the moves yet; this one keeps the character in place, so it serves as
  the idle loop. It plays in a loop on the game's 60 Hz clock (`Playback`, `apply_segment`);
- each frame, `Rig::update_world`, then for every drawn submesh of the character: its world transform from its rig
  node (rigid parts hanging from bones follow them), and for skinned meshes the skinning matrices
  (`Skin::matrices`) as the instance's bone matrices, drawn with the renderer's GPU skinning, and a culling sphere
  grown by how far the matrices move and stretch the rest sphere.

The skinning data of every mesh set with bones goes into the arena (`SkinnedSubmesh::from_submesh`); only the
character's instances get bone matrices. Objects below the character of class 63 (mine), 76 (decoy) and 97
(dynamic shadow) are left out: they are its power-ups and effects, which the game keeps with the character but
does not show at rest. Drawn, they covered it (the decoy is a black copy of the character), which made it look
mostly black.

Cost on PC: the animation update (playback, rig, skinning matrices, writing them into the renderer) takes about
0.03 ms per frame in Paris.

### The stats overlay

Drawn by the renderer itself (`render::overlay::TextOverlay`, a 5 x 7 font with a black outline, one instance per
character in the overlay pass), so it costs one draw command. It shows the world and its number, "loading" while a
world loads, the frame rate, the renderer's CPU time (build + submit) and the draw commands, API draw calls,
batches, visible / live instances and triangles of the last frame. Text is 2x on a 720p target and 3x on 1080p.

## Sly 2

`crates/formats-sly2` builds a `formats::scene::Scene` for a Sly 2 world (see [formats-sly2.md](formats-sly2.md));
`src/sly2.rs` turns it into the same `PreparedLevel` as a Sly 1 world, so the session, the renderer and the
controls are shared. `loader::GameDisc` holds the disc of either game: Sly 1 is tried first, then Sly 2.

- **Worlds.** L / R cycle through the 43 level-table rows that hold a world (row 18 is empty). Before a world is
  loaded its name is its row and id (`Row 3 (0x3B)`); once loaded the overlay shows the level package's name too
  (`f_nightclub_cellar - Row 3 (0x3B)`). The scene is built for that row, so per-level variants of meshes and
  textures are picked as the game does.
- **Passes** (geometry.md 9, working assumption): pass 1 is the sky, drawn first as a backdrop at its world
  position; passes 3, 4 and 5 are opaque; pass 10 is blended far to near. Kinds 2 and 3 (shadow shapes and volumes)
  and the special passes (2, 7, 8, 9, 11, 13) are left out.
- **Shader flags** (textures.md 2.4) for kinds 0, 4, 5 and 6: `0x20` alpha test without blending (kind 0 keeps
  texels at alpha 0x7E / 0x80 and up; with `0x04` the failed texels still draw colour without depth, done with the
  renderer's two-step blend with blending off; other kinds never pass the test, so they draw only with `0x04`, as
  post draws without depth); `0x10` reverse blend `(Cd - Cs) As + Cs` (`BlendMode::ReverseAlpha`) or with `0x01`
  `Cd As + Cs` (`ReverseAdd`); `0x80` blend by destination alpha (`DestAlpha`); otherwise alpha over, or additive
  with `0x01`. A flagged shader that blends does so in any pass. Other kinds blend only in pass 10 (additive with
  `0x01`).
- **Shared path.** Sly 2 and Sly 3 scenes are prepared by the `stage` crate (`stage::prep::prepare`, also used by the
  game), then wrapped as a `PreparedLevel`; `src/sly2.rs`, `src/sly3.rs` and `src/sly2_light.rs` are thin layers over
  it ([renderer.md](renderer.md), "Sly 2 and Sly 3 levels").
- **Lighting** (specs/sly2/rendering.md 2 and 2.5). `formats-sly2`'s scene gives the lights (class 4, options 151 to
  158, world space), each shader's self-illumination and second colour, the grey palette (slot 1) of kinds 1 and 7,
  each mesh body's flags byte, each submesh's strip-list flags and the vertex-map tables' pre-baked colours (the
  mesh's or its copy's). Every drawn vertex gets `render::Lighting::TwoColour` weights, chosen per submesh as the
  spec's table says: body flag bit 0 set, lit per frame by the four strongest lights at the object's centre (cap 2,
  twice the scale, normals quantised to 1/127; computed once at load, since the viewer does not move objects);
  otherwise kinds 1 and 7 take the pre-baked colours (strip-list flag `0x20`) or the static bake (every light with
  option 158 off that reaches the mesh, cap 1, 5 bits per channel), and the other kinds the unlit colour
  `(v m + 255) / 256` (5 bits; white gives 248, so skies show at about 1.94x their texels, as authored). Kinds 1 and
  7 draw `grey x A + colour x B x (alpha x B alpha)`, the others `slot 0 x A`.
- **Fog and clear colour:** world options 166 to 169 (camera.md 1's defaults when missing); past the end distance the
  fog keeps rising as the game's does (`Renderer::set_fog_rising`). The clear colour is black.
- **Texture animation** (rendering.md 4): node records of classes 139 to 144 drive their materials as Sly 1's kinds
  1, 2, 3, 5, 6 and 7 do (`stage::texanim::Motion::from_sly2`): frame loops, ping-pong, random frames and blinks swap
  the material's images (every frame of slots 0 and 1 is decoded at load), scrolls and circles move its UVs. Animated
  materials are not merged.
- **Shadow hulls and projected volumes** (rendering.md 3.4, 3.5): kind 2 in passes 7 and 8 is a stencil volume
  (`Pass::Volume`) darkening the scene inside it by the material alpha; kind 3 in passes 11 and 13 fills the scene
  inside with its second colour, or with flag `0x02` (light shafts) draws its surface blended. The viewer adds passes
  7, 11 and 13 to the scene build. Approximations: each hull darkens on its own (the game counts all hulls of the
  shadow phase and darkens once by its fixed ramps), flag `0x08`'s destination-alpha mask is not kept, streamed hulls
  (silhouettes extruded from the light each frame) are not drawn, and a light shaft's front-face fill step is left out.
- **Not done:** kind 7's first pass, which writes only the frame's alpha (no later pass is known to read it,
  rendering.md question 6), and kind 1's second pass for pixels that failed the first pass's alpha test (drawn in the
  same single pass, which tests depth the same way).
- **Static geometry merging:** unsorted draws merged into placed meshes at load, pixel for pixel the same
  ([renderer.md](renderer.md), "Static geometry merging"); the two colours ride along as the placed mesh's weights.

Captures (outside the repository, checked by eye): the nightclub cellar (`world=0x3B`) and row 14 lit warm and dark
by their lights, the intro hub (`row0`) under its purple fog and dusk sky.

## Sly 3

`src/sly3.rs` loads a Sly 3 world from `formats-sly3`'s scene (see [formats-sly3.md](formats-sly3.md)) into the same
`PreparedLevel`. The hooks in the shared code are marked `Sly 3 hook`: the `formats-sly3` dependency, `pub mod sly3`,
the `GameDisc::Sly3` variant (tried after Sly 1 and Sly 2), and `game=sly3` / `SLYNX_DISC_SLY3` in `pc.rs`.

- **Worlds.** L / R cycle through the 39 slots that hold a world (slot 1 is empty), named `v_apt (slot 5)`.
  `world=` takes a list index, `slot5`, or part of a name. The scene is built for the slot with option set 0's
  condition (a fresh load), so world filters and conditions pick what the game shows.
- **Passes.** Sly 3's pass numbers are Sly 2's minus one and its shader flags keep Sly 2's meaning (textures.md 4),
  so `classify_sly3` is `classify_sly2` with the pass moved up by one. Opaque draws (passes 2, 3 and 4) become
  alpha-tested at 0.5, the "alpha cut-off" textures.md 4 asks for (the value is not in the spec yet). Pass 0 is the
  sky, pass 9 blended; blob shadows (6) and projected volumes (10, 14) are left out.
- **Colours.** Vertex colours are on the 255 scale (geometry.md 4.1). Textures are graded (textures.md 5) by
  `formats-sly3`.
- **Lighting** as Sly 2's (specs/sly3/rendering.md 1, 2 and 6): lights are class 4 and its derived classes (options
  159 to 165, `formats_sly3::scene::light_classes` from the executable's class table), the shadow colour and fog come
  from option set 0 (1015 to 1017, 1010 to 1014, with the fog colour as HSV in 1013 when 1012 is missing),
  self-illumination from the shader's parameter bytes. Strip lists with flag `0x20` take the mesh's (or copy's)
  colour table in effect (geometry.md 4.2: A, B and the amount as B's alpha). The clear colour is option 1024
  (rendering.md 6; China's halls dark blue (30, 40, 100)), black when no option set gives it (what the game does then
  is open).
- **Passes.** The viewer adds the shadow hulls (6), projected volumes (10, 14) and kind 5's pass 15 (colour only, no
  depth) to the scene build. Sly 3's texture animation records (classes 175 to 180) are not read yet.
- Frame 0; no outlines, HUD or animation. The menu world (`dvd_menu`) draws nothing: its solids are hidden or
  outline-only.
- The Switch default disc paths are unchanged (`sly1.iso`, `sly2.iso`); a Sly 3 disc is opened when given in
  `viewer.cfg`.

Tests: `tests/sly3_disc.rs` (skips without `SLYNX_DISC_SLY3`) prepares all 39 worlds and renders one frame of
each with the null backend, and checks world switching from v_apt.

### Projected shadow, view-angle fade, masked areas

- **Projected shadow (rendering.md 9.2):** the player character (class 7, options 43 to 48, shadow shader option
  44 with its material alpha as the strength) casts its silhouette straight down onto meshes whose mesh flags
  byte has bit 0x01; the character does not receive its own shadow. The projector stays where the character
  stands (the viewer does not move it). How dark it is depends on the open question of the silhouette's alpha
  (rendering.md question 6): the character's vertex alpha is used as is.
- **View-angle fade (6.2):** meshes with flag 0x080 (`formats::geometry::Mesh::glint`) get their alpha from
  `clamp(c0 + x (c1 + x (c2 + x c3)), 0, 1)`, `x = |direction . camera forward|`, every frame (`live::glint_alpha`).
- **Masked areas (3.3):** passes 10, 11 and 12 in lists 13, 14 and 15 (`Pass::MaskWrite`, alpha-tested at 0.1,
  `Pass::MaskFill`); four worlds use them.
- **Iris shape:** `iris::ScreenMask` reads the world's screen mask (class 142, classes/paths-and-maps.md 5; y up)
  and `ScreenMask::colors_of_world` its five sets' colours (shaders 1166 to 1170), for `render::iris::IrisShape`;
  `iris::iris_placement` gives its centre and scale (pixels per mask unit) from the player's screen position and
  the closing amount (level-flow.md 5). The game drives the wipe.

## What is not rendered yet

- **Fixed-texture projectors** (kind 3, light patterns): none on the disc, not built. The player's projected
  shadow is drawn (below).
- **Skinned outlines** and outlines of the animated player; particles; outlines not masking stencil volumes.
- **Animation.** Only the player character is animated, with one segment in a loop (below). Guards and other
  skinned meshes stay in their bind pose; poses (morph targets) are not applied. Many of the items the character
  holds in its hands (all attached to the same hand bones) are drawn at once, because which one shows depends on
  game state the viewer does not have.
- **Lighting of moving objects** is per object but computed once, at rest.
- **Culling and level of detail:** frustum culling per submesh only; no draw-distance limits, zones or LOD.
- **Material details:** texture flags (wrap or clamp) are unknown, so every texture repeats.
- **PC window:** the OpenGL target keeps its starting size; resizing the window (F11) is not followed.

## Performance

Measured on PC (Intel Core Ultra 9 275HX, WSL2, release). The renderer's CPU work is the build (cull, sort,
instance records) plus the backend's submit. Build time is the backend-neutral part and what carries over to the
Switch; submit on the PC is OpenGL on Mesa, not comparable with deko3d.

Renderer CPU build per frame, null backend, 600 frames (`scene_stats --bench`):

| World | View | Visible / live instances | Batches | Draw commands | Triangles | Build mean / p99 |
|---|---|---|---|---|---|---|
| Paris | spawn | 1,316 / 6,862 | 76 | 845 | 36k | 0.054 / 0.116 ms |
| Paris | overview, whole level | 6,661 / 6,862 | 529 | 3,321 | 191k | 0.223 / 0.403 ms |
| A Rocky Start | spawn | 2,564 / 4,642 | 192 | 1,814 | 80k | 0.082 / 0.178 ms |
| Boneyard Casino | spawn | 8,053 / 9,437 | 695 | 5,162 | 137k | 0.301 / 0.521 ms |

"Visible" counts draw steps: a two-step translucent submesh is two, a stencil volume two.

Live instances include the overlay's 400 glyph slots. Loading a world takes 50 to 110 ms on the I/O thread on PC
(lighting weights and three palette images per kind-0 material included); the upload takes 12 to 20 ms on the game
thread.

Headless harness runs at 1280x720 (OpenGL on Mesa in WSL2, 120 frames, 10 warm-up):

| World | API draw calls (GL multi-draws) | Draw commands | Build mean | GL submit mean |
|---|---|---|---|---|
| Paris | 76 | 845 | 0.155 ms | - |
| The Hideout | 91 | 888 | 0.065 ms | - |
| A Rocky Start | 193 | 1,815 | 0.316 ms | - |

(Per-frame updates in Paris, the player's animation, texture animation, the sky and billboards: 0.05 ms.)

### What this means for the Switch

Not measured on hardware yet. A Cortex-A57 at 1 GHz is roughly 8 to 10 times slower than this desktop core for this
kind of code, which puts the build for Paris at about 0.5 ms from the spawn and 2 to 2.5 ms for the whole level in
view. On deko3d the submit is one copy of the instance records (up to about 520 KiB per frame for Paris) into GPU memory
plus one bare indexed draw per draw command and a texture bind per batch; at 3,000 draw commands that should stay
around 1 ms. API draw calls on the Switch equal the draw commands (deko3d has no multi-draw; the GL count is the
batch count).

The batch count is driven by translucent geometry: blended draws are sorted strictly back to front, and each change
of texture pool along that order starts a batch. Boneyard Casino, with over 3,000 blended submeshes, reaches about
450 batches from its spawn. On deko3d a batch inside one pass costs only a texture bind; on
OpenGL it is one multi-draw. If this shows up on hardware, sorting blended draws by depth buckets would trade exact
order for fewer batches.

## Not verified on hardware

Everything Switch-side builds and links; none of it has run on a console yet:

- the viewer's start-up, disc reading from the SD card through the platform's file API, and the no-disc screen;
- loading on the I/O thread with CPU boost, and the hand-over back to the game thread;
- frame rate and CPU times (the overlay and the debug log report them);
- the renderer changes listed in [renderer.md](renderer.md) (overlay pass, mirrored and additive batches, GPU
  skinning and the second vertex stream);
- the look of the rendering.md features on the console (lighting, fog, two-step, stencil volumes);
- memory: Paris needs 4.4 MiB of arena, 2.3 MiB of vertex weights and 31 MiB of textures with mips (three images
  per kind-0 material), plus a staging copy of the textures during the upload; the frame slots take 3 x 2.5 MiB
  for 32,768 instance records, and command memory 3 x about 2 MiB.

## Tests

- Unit tests (`cargo test -p viewer`): material classification, spawn rules and level bounds, upload into the null
  renderer, the camera and controls, disc paths and settings, the mailbox, the session without a disc.
- Disc tests (`tests/disc.rs`, run when `SLYNX_DISC_SLY1` is set): all 46 worlds prepare with consistent tables and
  fit the Switch build's instance limit; Paris renders and L / R switch worlds; the player character in Paris is
  animated, its skinned instances get bone matrices and share draws with rigid ones.
- Sly 2 disc tests (run when `SLYNX_DISC_SLY2` is set, skipped otherwise): the disc is recognised, the 43 worlds are
  listed and found by id and row, all of them prepare with consistent tables within the instance limit; the
  nightclub cellar renders and R loads the next world. Unit tests cover the flag-to-blend rules and the pass
  classification.
