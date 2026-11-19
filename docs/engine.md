# The `engine` crate

`crates/engine` holds the engine foundations shared by the games. This file covers these modules:

- `engine::anim`: animation playback, a transform rig, skinning and poses.
- `engine::collision`: collision structures and queries.
- `engine::physics`: bodies, the contact solver, springs, smoothing and character movement.
- `engine::player`: Sly's controller.
- `engine::camera`: the follow camera.
- `engine::gang`: the Sly 2 characters (Sly, Murray, Bentley), and `anim::xform` for Sly 2 rigs.
- `engine::world::sly2` and `anim::sly2`: Sly 2 worlds in the object model, Sly 2 clip playback and skinning.
- `engine::world::sly3`: Sly 3 worlds in the object model.

They are written from `specs/sly1/animation.md`, `collision.md`, `physics.md`, `transforms.md`, `geometry.md`
and `classes/`.

- `#![no_std]` with `alloc`, `#![forbid(unsafe_code)]`, no third-party crates. Builds for
  `aarch64-nintendo-switch-freestanding`.
- Depends on `formats`, which does the parsing: `formats::anim` for segments and `formats::collision` for solids,
  convex volumes and the material tables.
- Independent of the game's object model, which comes later. The modules take parsed data and plain node or body
  indices, and return transforms, matrices and hits.
- `engine::math` has the vector and 3 x 3 helpers (columns, as in `formats::geometry::Affine`). It also has `sqrt`
  and `sin_cos`, done in `f64` with plain IEEE operations, so PC and Switch give the same bits without a C maths
  library.

## Animation (`engine::anim`)

### Parsing (`formats::anim`)

| Item | What |
|---|---|
| `Segment::parse(data, body_offset, &OptionTable)` | One segment body: its channels, the three event lists, its options and its `SegmentSettings`. For a class-131 record the body offset is `record.offset + 6`. For a warp or exit animation, use `ObjectStream::owned_animations[i].body` |
| `Channel` | Target OID, flags, position, rotation, scale (`VectorCurve`), twist (`ScalarCurve`), and poses (`Vec<Option<ScalarCurve>>`, `None` = not animated). `is_root_motion()` checks bit 5 |
| `ScalarCurve` | `Hermite`, `Bezier` (weighted), `Linear`, `TwistBlend`, `PoseBlend`; keys are copied out of the file |
| `VectorCurve` | `Components { mask, base, curves }`, `Hermite`, `Blend` |
| `Event` | Type, frame (framed list only) and raw payload, read with `i16_at`, `u16_at` and `f32_at` |
| `SegmentSettings` | Repeat mode (178), auto-play (179), hands off (180), priority (181), base speed (182), difficulty speed (183, 184), real clock (189). Option 188, the duration, has no stream encoding; it comes from the record. A setting whose value has the wrong size (possible only with a custom option table) is reported as corrupt |
| `player_lookup_list(&Executable)`, `read_lookup_list(exe, va, count)` | A character's (flags, OID) lookup list, read by address after checking the executable's SHA-256 (animation.md 7) |

### Curves (`anim::curve`)

- `sample_scalar(&ScalarCurve, t)` and `sample_vector(&VectorCurve, t)` give the value and the rate (derivative
  per second). `eval_scalar` and `eval_vector` give the value only. Time `t` is in seconds.
- Blend placeholders and empty curves give `None`. In a per-component vector curve, a component without a curve
  keeps its base value.
- The rules follow animation.md 2 and 3:
  - segment search clamped to `0..=n-2`;
  - `Δ < 0.0001` holds `g_k`;
  - Hermite keys with outgoing kind 5 hold their value;
  - weighted Bezier keys use the exact 16-step bisection (`bezier_parameter`) and the `τ` and `Δ − τ` thresholds;
  - linear keys;
  - vector Hermite keys have no step rule.
- `euler_zyx([x, y, z])` gives the columns of `Rz · Ry · Rx`.

### Playback (`anim::playback`)

```rust
let mut fired = Vec::new();
let mut p = Playback::start(&seg, 1.0, &mut fired);          // fires the apply list
let status = p.advance(&seg, dt, &mut fired);                 // wraps, fires framed events in order of travel
let binding = Binding::resolve(&seg, |oid| lookup(oid));      // channel -> node (root-motion channel: None)
apply_segment(&seg, &binding, p.time, &mut rig);              // writes position, rotation, scale, twist, poses
let v = root_motion_velocity(&seg, p.time, p.speed());        // facing-frame velocity (animation.md 4.2)
p.retract(&seg, &mut fired);                                  // fires the retract list
```

- **Speed.** The effective speed is the caller's speed times the base speed (option 182) and the difficulty
  factor `clamp(g0 + g1·u + g2·u², min, max)` (options 183 and 184, `SegmentSettings::speed_scale(u)`).
  `Playback::start_with(seg, time, speed, u, fired)` takes the level's difficulty factor `u`; `start` and
  `start_at` use u = 0, and `Animator::difficulty` sets it for the animator's playbacks. A playback whose speed
  is below 0.0001 does not advance. `pause_for(seconds)` and `resume()` pause and restart it.
- **Repeat modes** (5.2). Once stops at the end and reports `reached_end`. Loop wraps and reports `wrapped`. Back
  and forth folds the time and reports `turned`. Manual never advances; use `seek`.
- **Framed events** fire when playback crosses them:
  - forward, events in `(old, new]`;
  - backward, events in `[new, old)`;
  - the start time itself is included right after an apply or a loop wrap, so frame-0 events fire on the first
    update that advances the time, not on the apply itself (animation.md 5.3); a step that does not move the time
    fires nothing.

  Each list is reported in the order of travel. A `FiredEvent { list, index }` refers to the segment's list.
- **Events that act on the playback.** `Playback::apply_event` handles:
  - 4: seek and set the speed;
  - 5: set the speed;
  - 11: freeze at the event's own time, speed 0, until something sets a speed;
  - 8: returns `EventEffect::Remove`.

  Event 7 is 4 gated on a message: the caller applies it with `apply_message_seek`. Event speeds are scaled like
  every speed of the playback. All other types are left to the systems they belong to.
- **Writing values.** `AnimTarget` is the trait that receives the written values. `Rig` implements it, and so can
  any object model. Rotations arrive as Euler angles; the target composes `Post · R · Pre` with
  `rotation_matrix(euler, pre, post)`.
- **Shared targets** (5.4). `Animator` holds several playbacks over a segment library slice the caller owns.
  `update(lib, dt, real_dt, target, events)` does the following:
  - advances each playback, on the real clock for option 189;
  - applies events 4, 5 and 8;
  - writes the playbacks in increasing (priority, start order), so the highest priority wins and, at equal priority,
    the newest wins.

  `play` and `remove` fire the apply and retract lists.
- **Blends** (section 6). `apply_blend(&[BlendMember { segment, binding, weight }], phase, scratch, target)`
  evaluates every member at the same phase and writes `Σ wᵢ · valueᵢ` of positions, Euler angles and twists.
- `jumps_on_wrap(channel, duration)` applies the 1 unit / 0.01 rad test of 5.2. The playback always snaps, so the
  test only matters once easing exists.

### Rig (`anim::rig`)

A flat hierarchy, parents first, that animations write into:

- `Rig { nodes, local, world, twist, pose_weights }`.
- `RigNode` holds the OID, the source record, the parent, the rest `LocalTransform` (position, rotation, scale),
  the pre- and post-rotation, an ancestor override (`BasisOverride`, extension bits 2 and 3), the rest twist and the
  pose-weight range.
- `Rig::from_stream(&stream, data, root)` makes one node per record of `root .. subtree_end`, so node `i` is record
  `root + i`. It reads the extensions' pre- and post-rotations, basis overrides, IK twists and initial pose weights.
  `node_of_record` and `find_oid` map back to nodes.
- `update_world(&base)` computes `W = W_parent · [R · diag(s), p]` and applies the overrides. `base` is the world
  transform of the root's parent.
- `reset_to_rest()` puts every local transform, twist and pose weight back to its rest value.
- **Pose-weight sets** (animation.md 8.3) belong to the object that carries them and drive its own mesh set.
  `from_stream` reads them (`Record::pose_weight_sets`).
  - `select_pose_set(node, name)` selects one by name, as for event 45 or option 116; `pose_set_event(ev)` decodes
    event 45. A name of −1 releases the weights.
  - `update_pose_sets(dt, rate)` eases the weights toward the selected set. The spec gives no easing law, so the
    linear rate is the caller's choice.

### Skinning and poses (`anim::skin`)

- `Skin::from_stream(&rig, &stream, mesh_record, &mesh_set)` resolves each bone OID to the object nearest to the
  mesh set's owner (`ObjectStream::find_nearest`, object-stream.md 7.4).
- `capture_bind(&world)` records `K_b = W_b⁻¹ · W_mesh`. Call it once the world has loaded.
- Each frame, `matrices(&world, &mut Vec<Affine>)` computes `S_b = W_mesh⁻¹ · W_b · K_b`, and
  `bone_matrices(&world, &mut Vec<BoneMatrix>)` gives the same as GPU rows.
- `SkinnedSubmesh::from_submesh(data, &submesh, pose_count)` holds, per strip entry and in the same order as
  `Submesh::append_gpu` writes the renderer's vertices:
  - the base position and normal;
  - a `SkinVertex { bones: [u8; 4], weights: [f32; 4] }`, with bones as indices into the mesh set's bone list;
  - sparse `PoseDelta`s per pose.
- `morph(weights)` applies the poses. `deform(matrices, weights)` is the CPU path: poses first, then
  `M = Σ wᵢ S_bᵢ`, `p' = M p`, `n' = normalise(M₃ₓ₃ n)`. Zero weights are skipped. Weights are not renormalised, and
  a vertex whose weights are all zero stays in place.

## Renderer interface for skinning

The renderer (`crates/render`) does not have this yet. This is what the engine needs from it.

**Per-vertex attributes.** Skinned meshes need two extra attributes in a second vertex stream, or appended to the
arena vertex, one per renderer vertex (strip entry), in `append_gpu` order:

- `bones`: `u8 x 4`, read as integers;
- `weights`: `f32 x 4`, or `unorm16 x 4` if precision allows.

`SkinnedSubmesh::influences` gives them. Meshes without bones need neither.

**Bone buffer, per frame.** One storage buffer of `BoneMatrix` records holds the bone matrices of every skinned
instance drawn in the frame:

- each record is 48 bytes, three `vec4` rows `[m00 m01 m02 tx]`, `[m10 m11 m12 ty]`, `[m20 m21 m22 tz]`;
- it is written by the CPU each frame into the frame slot's memory, like the instance records;
- it is bound once per frame, next to the instance buffer.

**Per-instance fields.** The instance record (`GpuInstance`) gets a `bone_base: u32`, the index of the instance's
first matrix in the bone buffer, and a skinned flag. Suggested API:

- `Renderer::set_bones(instance, &[BoneMatrix])` copies the instance's matrices for this frame; the renderer packs
  them and fills `bone_base`;
- or a frame-level `bones: &[BoneMatrix]` slice, with `set_bone_base(instance, base)`.

A Sly 1 mesh set has at most 255 bones; the player character uses a few dozen.

**Vertex shader, for skinned instances:**

```
w = weights; if (w == 0) { M = identity } else { M = Σ_{i: w_i ≠ 0} w_i · bones[bone_base + idx_i] }
p' = M · (p, 1); n' = normalize(mat3(M) · n)
```

Then the instance's own affine transform and the view projection apply as now. The skinned position is in the mesh
owner's local space, so the instance transform is the owner's world transform. Weights must not be renormalised.
The all-zero case must leave the vertex in place.

**Poses (morph targets).** Few meshes have them: the face and mouth poses of characters. Two options:

- The CPU writes morphed positions and normals (`SkinnedSubmesh::morph`) into a per-instance dynamic vertex range
  each frame they change. Simplest; the GPU then skins as above.
- Later, if profiles ask for it: a per-mesh delta buffer (entry, Δp, Δn per pose) and per-instance pose weights,
  applied in the vertex shader before skinning.

**Culling.** A skinned instance's bounding sphere must cover its animated extent. The renderer can take a sphere
per instance from the engine, for example the owner's sphere grown by the largest bone offset, instead of the
mesh's rest sphere.

## Collision (`engine::collision`)

### Parsing (`formats::collision`)

| Item | What |
|---|---|
| `Solid::parse(data, record.offset + 6)` | Physics flags (`is_fixed`), `CollisionMesh` and `CollisionTree`, mass, inertia, centre of mass, camera-fade groups and node map, hit-group count, camera mesh and tree, and the offset of the placed part. `camera_collision()` picks the mesh the camera uses |
| `CollisionMesh` | Radius, positions, triangles (corners, radius, centre, edge range) and edges (ends, other triangle); `corners(t)`, `triangle_edges(t)` |
| `ConvexVolume::parse(data, offset)` | Planes `n · x = d` and nodes. `ObjectStream::volumes` lists the offsets of every volume, exit, camera-zone and flash-volume shape |
| `material_of(data, &options, &table)` | Option 128 (default 0) |
| `MaterialTables::from_executable(&exe)` | The 13-entry friction and elasticity tables, read by address after checking the executable's SHA-256 |

### Structures

- `TriMesh::new(&mesh, &tree)` builds one collision mesh in its owner's local space:
  - triangles with unit normals and plane offsets;
  - a bounding-volume hierarchy over the triangles (binned surface-area splits, leaves of 4);
  - the stored edges and the collision tree.
- `Shape { main, camera }` is a solid's collision mesh and its camera mesh, when it has a non-empty one.
- `CollisionWorld` places shapes as bodies and keeps a bounding-volume hierarchy over the bodies' world boxes:
  - `add_shape` and `add_body(shape, transform, material, user, fixed)`, then `rebuild()`;
  - `set_transform` refits the hierarchy; for several bodies, call `set_transform_deferred` on each, then `refit()`
    once;
  - `set_enabled` turns a body on or off.
- `CollisionWorld::from_world(&world, &table)` builds a whole world:
  - every placed solid in the hierarchy (solid and conveyor layouts, outside stock templates), proxy clones
    included (`ObjectStream::placements_where`);
  - each at its world transform, with its material;
  - one shape per source record, shared by its clones;
  - solids with empty meshes left out.

  Body `user` is the object-stream record.

### Large trees

`TriMesh` keeps its tree as `formats::collision::WideCollisionTree`, with 32-bit indices (12 bytes a node instead of
6), because Sly 3 has level objects with 75,558 to 103,211 nodes.

- `TriMesh::new` converts a 16-bit `CollisionTree` losslessly.
- `TriMesh::new_wide` takes a wide tree; `WideCollisionTree::validate` checks it the way `CollisionTree::read` does.
- `tree_segment` reports node indices as `u32`.

### Queries

`QueryFilter { two_sided, camera, ignore }`:

- **Contact is "inside the solid"** (collision.md 2 and 7). The collision tree describes a solid volume:
  `TriMesh::point_inside(p)` and `CollisionWorld::point_inside(p, &filter)` walk it. Overlaps have no one-sided
  test:
  - a centre inside the solid is in contact with the nearest surface, at any distance, and is pushed out along
    that surface's normal by r plus the distance;
  - otherwise every triangle within r counts, from either side.
- Ray casts and sweeps report surfaces met from the open side by default, so a body inside a solid is not stopped
  on its way out, and the overlaps push it out. `two_sided` makes them report back faces too.
- `camera` switches to the camera meshes.

| Query | Result |
|---|---|
| `ray_cast(origin, dir, max_distance, &filter)` | `RayHit`: distance, point, normal, body, triangle, material, front |
| `segment_blocked(from, to, &filter)` | Line of sight; stops at the first hit |
| `sphere_sweep(center, r, delta, &filter)` | `SweepHit`: fraction, contact point, normal (surface to shape), body, triangle, material. Analytic test on faces, edges and corners |
| `capsule_sweep(a, b, r, delta, &filter)` | `SweepHit`. The segment-to-triangle distance is convex in the fraction: golden-section search for the closest approach, then bisection for the first contact. The returned fraction is just before contact (2⁻²⁴) |
| `sphere_overlaps(center, r, &filter, &mut out)` | Every `Overlap` (push-out normal, depth, point) |
| `ground_probe(point, max_drop, &filter)` | `GroundHit` straight down: distance, point, normal, material, body, `is_ground` (normal z > 0.7, collision.md 7) |
| `ground_probe_sphere(center, r, max_drop, &filter)` | The same with a downward sphere sweep |
| `point_in_convex(&volume, local_point)` | The tree walk of collision.md 5 |
| `PlacedVolume::contains(world_point)`, `world_volumes(&world)` | The same for volumes placed in the world |
| `Materials::new(tables)`: `friction(i, j)`, `elasticity(i, j)` | `(a_i + a_j) / (w_i + w_j)`; friction at least 0, elasticity clamped to 0..1 |
| `TriMesh::ledges()` | Stored edges between a floor-like (z > 0.7) and a wall-like (z < 0.55) surface, with the outward horizontal direction (collision.md 1.1) |
| `TriMesh::tree_segment(a, b, eps, visit)` | The collision-tree nodes a segment reaches. Parsed trees have every child after its parent and one parent per node (`formats` rejects anything else; true on all 46 worlds), and the walk also stops after as many visits as there are nodes: positive child for the side the normal points to, negative for behind. For the camera-fade groups (collision.md 4). `tree_ray_cast` uses it for a ray cast |
| `TriMesh::edge_is_convex(t, edge)` | The convexity test of collision.md 1 |

Body transforms are taken as rigid (transforms.md 5). Queries move into body space with the inverse transform,
and distances carry over unchanged.

**Edges in sweeps.** Sweeps test the three edges and corners of every candidate triangle, not only the stored
edges. Stored edges skip concave edges and do not always end at the owning triangle's corners (T-junctions), so
using only them could let a sphere through a narrow gap. They are used for ledges.

### Performance

`cargo run --release -p engine --example collision_bench -- <disc.iso> [world]`, one thread, desktop CPU in WSL,
Paris, France: 404 bodies, 160 shapes, 13,327 triangles placed. The world builds in 5 ms.

| Query | Per ms |
|---|---:|
| Ray from near the ground, 1,000 units (48 % hit) | about 4,800 |
| Ray, 10,000 units | about 4,800 |
| Ground probe, 200 units down | about 5,300 |
| Ray from the top of the level to the bottom | about 5,000 |
| Line of sight, 1,000 units | about 6,300 |
| Sphere sweep, radius 30, 100 units | about 5,400 |
| Capsule sweep, radius 30, height 120, 100 units | about 420 |
| World mesh only: hierarchy ray / collision-tree ray, 1,000 units | about 9,500 / 7,300 |

The Switch's Cortex-A57 is several times slower than a desktop core. Even so, a frame's worth of queries, a few
hundred rays and sweeps, stays well under a millisecond. Capsule sweeps are the slow path; use spheres for the
frequent tests. Switch numbers come with the end-of-project hardware pass.

The collision tree works as a culling structure: ray casts through it agree with the hierarchy on 3,997 of 4,000
random rays in The Hideout and 4,000 of 4,000 in Paris. The differences come from the plane tolerance at grazing
edges. The tree has as many negative children as positive ones, and a node stores one triangle, so it costs more
per query than the hierarchy. The hierarchy is the query path. The tree is kept for the camera-fade groups, which
are defined on its nodes.

## Physics (`engine::physics`)

Written from physics.md, clock-and-loop.md, collision.md and options-by-class.md 2.2 and 2.7. It is independent of
the object model:

- the caller owns the `CollisionWorld` with the scenery and the bodies' collision entries;
- it creates bodies here, sets their forces, water fraction and goals;
- it calls `PhysicsWorld::step(&mut cw, &materials, &mut rng, dt)` once per frame with the game step (after
  animation playback, clock-and-loop.md 4).

### Bodies (`physics::body`)

`BodyDesc` holds the settings of a body:

| Field | Meaning |
|---|---|
| `motion` | `Dynamic`, or `Kinematic` (moved by the caller, infinite mass) |
| `mass`, `inertia`, `center_of_mass` | Solid layout rows 4 to 6, option 127 |
| `gravity` | `[0, 0, −1500]` by default (`WORLD_GRAVITY`); option 129 sets a vector, option 138 zero |
| `gravity_ratio` | Option 711 |
| `terminal` | `Option<TerminalFall>` |
| `material` | Option 128 |
| `buoyancy`, `viscosity` | Options 134 and 135; defaults 0.5 and 1.0 |
| `interacts` | Option 130 clears it |
| `lock_translation`, `lock_rotation` | `AxisLock`, options 131 and 132 |
| `probes` | Contact spheres in local space |
| `collision` | The body's own entry in the collision world |

Presets and helpers:

- `player_desc(terminal)`: gravity −1750 × 1.3 = −2275, with terminal fall.
- `pickup_desc()`: gravity −2250.
- `probes_from_mesh(&collision_mesh, radius)`.
- `sphere_submerged_fraction(z, r, surface)`.

`Body`:

- holds the transform, velocity, angular velocity, accumulated `force` and `torque`, and the water state the
  caller sets from the water volumes: `submerged` (the fraction under water), `current` (the water's velocity),
  `water_normal` and `water_factors` (the water object's buoyancy and viscosity factors);
- `set_gravity_ratio(r)` scales gravity by new/old and mass and inertia by old/new;
- `apply_impulse` applies an impulse and wakes the body;
- `velocity_at` gives the velocity of the body's material at a point;
- `is_at_rest()` and `wake()`.

`TerminalFall::from_executable(&exe)` reads −750 and 5 at `0x263E78` and `0x263E7C` after the hash check. Below
`v_t = speed · √ratio` (−855.1 for the player) `drag` adds `5·(v_t − v_z)` to the frame's acceleration, on top of
gravity, so the fall settles at `settling_speed` = v_t + g/5 = −1310.1 (physics.md 2).

### The step (`physics::solver`)

`PhysicsWorld`:

- `add(body) -> BodyHandle`, `body`, `body_mut`, `set_enabled`;
- `move_kinematic(&mut cw, h, transform, dt)` derives a kinematic body's velocity and angular velocity and moves its
  collision entry;
- `contacts()` and `impacts()` give the step's results. The impacts are physics.md 3 step 6: each solid with a
  recorded impact reacts to it;
- `settings: SolverSettings` holds the spec thresholds (impacts above 5 units/s, penetration beyond 2 units
  pushed to 1 unit deep, restitution factor 0.9 to 1.1, 2 still frames) and the port's own tolerances (4 pivot
  iterations, 0.01 touch tolerance, probes shrunk by 3 for the tunnelling sweep, rest speeds 0.01 units/s and
  0.001 rad/s). `SolverSettings::sly2()` has Sly 2's rest rule;
- the `Rng` trait injects the restitution factors; `SplitMix64` is a ready-made generator. The same seed gives the
  same bits.

One step follows physics.md 3.2:

1. **Contacts at the start-of-frame positions.** Real touches only (no speculative contacts): each awake dynamic
   body's probes are tested with `CollisionWorld::sphere_overlaps`. Contacts against another physics body's entry
   involve both bodies; anything else is static; the body's own entry and bodies that don't interact are ignored.
2. **Integration** (physics.md 3.1). The frame's acceleration is taken once from the start-of-frame state: gravity,
   forces/mass, the terminal drag, and in water the buoyancy (`−buoyancy × f × g`, with an upright turn of
   `3 × buoyancy × f × θ`) and the viscosity, which pulls the velocity toward the current at `3 × viscosity × s_n`
   along the surface normal and `3 × viscosity × f` across it (angular: 0.75 × viscosity × f and 0.75 ×
   viscosity), each rate capped at 1/dt. Then `x ← x + v·dt + ½·a·dt²`, `v ← v + a·dt`; the orientation turns by
   the average angular velocity. The probes, shrunk by 3 units, are swept along the motion so fast bodies can't
   tunnel (the port's guard; a landing body still ends its frame embedded).
3. **Pivot pass** at the start contacts, moved with their bodies: an approaching contact's normal speed is removed
   (never pulled together), with friction limited to μ times the normal impulse; 4 iterations.
4. **Contacts again** at the moved positions. A falling body meets the ground here, embedded.
5. **Penetration.** If a contact of a body is deeper than 2 units, the body's contacts are pushed out by
   depth − 1 each (left 1 unit deep), split by inverse mass; contacts of one surface push once.
6. **Impacts.** If a contact of a body approaches faster than 5 units/s (measured after the integration and the
   pivot pass), every approaching contact of the body gets an impulse to a separation speed of `approach × e × k`,
   `k` drawn in [0.9, 1.1]: a change of approach × (1 + e·k). Friction applies; each is recorded as an `Impact`.
7. **Rest.** Below the rest speeds the velocities snap to zero; after 2 consecutive still frames the body rests and
   is skipped until woken by `apply_impulse`, `wake`, `move_kinematic` nearby, or a solver impulse from a moving
   body. A resting object stays embedded 0 to 2 units, which keeps its contacts alive.

### Springs, smoothing and drives (`physics::spring`)

- `Spring { strength, damping }`: `a = k(d)·(goal − x) + c(d)·(v − v_goal)` (c negative damps), the damping
  factor capped at 1/dt (physics.md 4.2).
  - `Strength::Constant(k)`, or `Strength::Coefficients([c0, c1, c2])` meaning `c0/d + c1 + c2·d` of the
    distance (options 57 and 66).
  - `step_exact` integrates objects' springs as in 3.1; `step` is the camera's velocity-first form;
    `step_rotation` uses the rotation-vector error of `goal · Rᵀ` and turns by the average angular velocity.
- `SmoothParams { fast, slow, fast_time, max_speed_change }` and `Smoother` follow physics.md 4.3: speed F, then
  over the last `(F + S)·T/2` the constant deceleration to S, integrated exactly within a step and never past the
  target.
  - The three-value law has no memory: the speed comes from the remaining distance only.
  - The four-value form keeps the current speed and changes it by at most A·dt, moving with that constant
    acceleration.
  - `SmoothParams::position_option(r)` and `rotation_option(r)` give options 67 and 69: (1500·r, 0, 0.5, 4500·r)
    and (15·r, 0, 0.5, 45·r) from the records at 0x260EB0 and 0x260EC0; `with_rate_ratio` gives options 71 and 72;
    `POSE_WEIGHT_SMOOTHING` is (3, 1, 0.1).
- `DriveMode::from_code` for options 55, 61, 73 and 74; `look_at_from_code` for option 76 (3 is "smooth, never
  lock"); `default_for(free_solid)` (smooth for a free solid, else follow). `Drive::step(&mut state, x, &Goal, dt)`
  and `step_rotation` implement the table of physics.md 4.4 with `Goal { position, velocity, time_left }`:
  - 0 none and 8 spring-locked: `None` (the drive does not move the object); 5 physics: `None` (left to the step);
  - 1 spring (exact integration toward the goal and its velocity);
  - 2 follow and 7 locked: the goal and its velocity;
  - 3 smooth: the goal's speed toward the object added to F and S, its sideways velocity kept; locks (7) on
    arrival; 9 never locks; 6 covers at least `distance × dt / (time left + dt)`;
  - 4 curve: a cubic Hermite from the current position and velocity to the goal's, timed to the match time, then
    the goal plus its velocity × the overshoot.

### Character movement (`physics::character`)

`move_and_slide(&cw, &filter, CharacterShape::{Sphere, Capsule}, position, delta, &SlideParams)` returns a
`SlideResult` with the position, the hits, the ground under the shape (normal z > 0.7) and whether a step was
climbed. It works as follows:

- Collide and slide: sweep, stop at the skin distance, remove the motion into the surface, repeat up to
  `max_iterations` times.
- `step_up: Option<f32>`: up, across, then down onto ground.
- `snap_down: Option<f32>`: pull down to ground within reach.

The specs give no step height, snap distance or slope speed rules, so both are off unless the caller sets them.
`ground_below` is a downward sweep that keeps ground-like surfaces only.

### Physics tests

- **Unit tests** (`physics::*`):
  - physics.md 6: a crate falls 750 units at 1,500 units/s after 1 s at any frame rate; the player's fall follows
    z = −1137.5 t² to 0.3759 s (160.7 down at −855.1), then −1065.5 and 281.2 down at 0.5 s, −1290.1 and 891.4 down at
    1 s, −1310.0 at 2 s; at 60 frames/s z −916.0 and v −1294.1 after 61 frames; a pickup tossed at 500 units/s
    peaks at 55.6 after 0.222 s;
  - a landing frame ends embedded up to 2 units, the bounce ratio within e × [0.9, 1.1], a ball resting on its
    pivot has no impact, rest after exactly two still frames;
  - the push-out to 1 unit deep beyond 2 units, nothing at 1.5;
  - friction stopping distance; buoyancy (net −750 for the crate of physics.md 6) and viscosity toward the current,
    with the partly-under rate;
  - determinism: identical bits for one seed, different paths for another;
  - springs (exact and velocity-first, coefficients, damping cap), the three-value law (1500, 0, 0.5) from 1000
    arriving at 0.9167 s, no memory, the four-value rate limit, options 67, 69, 71/72, pose weights, every drive
    mode, gravity ratio, axis locks;
  - sliding, stepping and snapping on a step mesh; the Sly 2 fall, bounce and rest rule.
- **Disc test:** the terminal constants read from the executable (−750, 5), and a modified executable is refused.

## Player (`engine::player`)

Sly's controller, from player-moves.md, with physics.md 2, guards.md 4.3, interactables.md 5 and 7, and
animation.md 7. It is a self-contained module driven by an input state and the collision world.

### How the object model drives it

Each frame:

1. Build a `PlayerInput`:
   - `stick` (x right, y up), camera-relative through `camera_yaw`;
   - the buttons `jump`, `attack`, `action` (the context button), `duck`, `rush` and `power`, each a
     `Button { held, pressed }`.
2. Build a `PlayerWorld`:
   - the `CollisionWorld`, with the player's own solids disabled;
   - a `QueryFilter`;
   - the context `features`;
   - world-space `ledges`, from `world_ledges(&cw)`;
   - the `in_shadow` and `in_cover` flags.
3. Call `Player::update(dt, &input, &world) -> PlayerOutput`.
4. Place the player's object at `Player::position` (the feet) facing `Player::yaw`.
5. Play `PlayerOutput::anim`, an `AnimRequest`: one lookup-list OID, or two blended with a weight. Restart it when
   `restart_anim` is set. `PlayerAnimator` does this on a `Rig` from the player's subtree.
6. Act on `PlayerOutput::events`, and apply `PlayerOutput::time_scale` (slow motion and speed-up) to the game clock.

The context features are a list of `Feature { id, kind }`:

| Kind | Data |
|---|---|
| `Pipe` | `bottom`, `top` |
| `Rail` | `points` |
| `Rope` | `points` |
| `Perch` | `position` (the perch points the context button lands on) |
| `Hook` | `position` |
| `HidingPlace` | `position` |
| `Water` | `surface`, extent |

Other calls:

- `damage(kind, &mut out)` for hazards and attacks, with `DamageKind` from option 609.
- `respawn(position, yaw)` after `RespawnRequested`.
- `hiding(in_cover).hidden_from(player, guard, reward_mask)` for the guards.md 4.3 rules.
- Fields to set: `vitals` (charms 0 to 2, lives), `reward_mask`, `selected_power`.

### Constants (`player::consts`)

`PlayerConstants::SPEC` holds the spec's values. `PlayerConstants::from_executable(&exe)` reads every [X] constant
after the hash check. The disc test checks that what it reads equals `SPEC`:

| Group | Values |
|---|---|
| Gravity | −1750 × 1.3 = −2275; fall drag below −750·√1.3, rate 5, settling at −1310.1 |
| Air steer | 455·√1.3; factors 0.7 after water, 0.6 after a ledge jump with no stick; rates (2.5, 3.5, 2.5) and in the take-off (60, 20, 10) at 0x263E58 |
| Jumps | the origin to 245 above the contact: 839.8 units/s standing (h = 90); swing 700·√1.3; water 900·√1.3; edge 800·√1.3 |
| Target and boosted jumps | `clamp(c0 + c1·v + c2·v², lo, hi)`: (682, 0.5, 0) and (682, 1, 0), at most 682 |
| Jump timers | take-off 0.15 s; steering decided by full stick within 0.07 s held 0.30 s; smash and dive after 0.35 s |
| Air control | facing follows the motion above 300; target search 10000 |
| Ground | leg 35 below the shape; ground from normal z 0.7; friction 1.5 (0x274B74), so 3412.5 units/s²; turn factor (1, −1, 0) at 0x274B30 |
| Origin | 90 above the ground contact (the standing shape 55 below it, the leg 35 more) [D]; `position` is the feet, `origin()` adds 90 |
| Ledges | the origin 37 out and 25 below the edge (0x2636F0), eased with (1000, 0, 0.25) |
| Turning | four-value smoothing of the heading error: (40, 0, 0.25, 200) standing (0x263740), (15, 0, 0.25, 100) running (0x263750), weighted by `clamp(−0.2 + 0.002·speed, 0, 1)` (0x263760), fast and slow × `clamp(0.2 + 0.8·u², 0, 1)` (0x263770) |
| Pipes and crush | climb speeds 66.0 + 299.3·u; a crush sets the velocity to (0, 0, −1000) (0x275040) |
| Sneaking and flattening | sneak cap 0.5; flatten smoothing 0.3 s; minimum sidestep 25 |
| Pipes | hands end 35, feet end 100, reverse 0.5, probe 50 |
| Rails | reverse −0.2 |
| Attacks | smash window 0.25 s; dive hover to frame 20, 1556.3 units/s to frame 100, damping 10; dive radii 300 and 500 |
| Deaths | backward-death turn (−1, 0.004, 0) |
| Rolling ball | radius 50, launch 850, launch threshold 150, roll 1000·u at rate 2.5·u, contacts 0.8 / 0.5, at least 0.25 s |
| Measured speeds [D] | run 38.3 and 638.4 weighted by the plant-to-lift intervals 131 and 7 frames; rush 550 for 1 s; flatten 120 and 135; barrel 72.6 and 416.7 |

### Tunables (`player::consts::Tunables`)

These are the values the specs leave open. They live in one place, and every one is a guess to check against the
game:

| Tunable | Default |
|---|---:|
| Stick dead zone | 0.15 |
| Stick response exponent | 1 |
| Capsule radius | 30 |
| Capsule height (between the sphere centres) | 100 |
| Ground probe below the leg (walking down slopes) | 2 |
| Hand height | 160 |
| Rail and rope speed | 300 |
| Catch radius (rails, ropes, perches) | 80 |
| Hook reach (added to the 15-unit catch radius) | 120 |
| Perch aim range | 800 |
| Flatten reach | 50 |
| Duck-walk speed | 150 |
| Hiding-place reach | 120 |
| Death time | 2.5 s |
| Hurt time | 0.6 s |
| Power-up duration | 10 s |
| Sweep duration | 40 frames |
| Swing push | 4 rad/s² |

### States (`player::controller::State`)

| State | Behaviour |
|---|---|
| `Ground` | Turn toward the stick; the wanted velocity `run_speed(u) × turn factor` along the facing, projected on the ground plane, reached at friction × \|g\|; the leg steps over 35 units; walking off an edge falls |
| `Air { target }` | Gravity and the fall drag integrated exactly, air steering toward the coast target, the boosted second press, landing, ceilings |
| `LedgeHang` | Held still on the hang position (no shimmy); the jump button does the ledge jump |
| `Pipe` | Climb with the stick within the hands and feet limits; reversing needs a 0.5 change; jump off (edge jump); duck to drop |
| `Rail` | Rails and ropes; reverse below −0.2; falls off the ends |
| `Perch` | Jump (aimed at the next perch); step off |
| `Swing` | A pendulum on the hook, pushed by the stick; swing jump |
| `Flatten` | Sidestep along the wall, sneak-capped, 135 at full deflection, at least 25 |
| `Duck` | Duck walk while the button is held |
| `Hiding` | Walk in the barrel at 72.6 to 416.7 |
| `Sweep`, `Rush`, `Smash`, `Dive` | Attacks |
| `Ball` | Rolling-ball power-up |
| `Hurt` | Hit reaction |
| `Dead` | Death; raises `RespawnRequested` once |

In more detail:

- **Run speed.** The two locomotion segments are blended with weights 1 − u and u, each speed weighted by its left
  foot's plant-to-lift interval (131 and 7 frames): 38.3 at u = 0, 68.7 at 0.5, 638.4 at 1. Sneaking (`in_shadow`
  and flattened) caps u at 0.5.
- **Jumps.** `start_jump(kind)` sets `vz = max(vz, rule)` for the ground, swing, water, edge, target and boosted
  rules.
  - A jump with the stick toward a perch within range is aimed at it: the horizontal speed is set so the jump comes
    down on the point, and it lands there (`Perch`).
  - The second press is the boosted jump: once per jump, at any time after take-off, not in an aimed jump or an
    attack, and not when the base point moved by `v·0.1 + ½·g·0.1²` would hit something.
  - `SteerMode`: a jump is short-steered when the stick does not reach full deflection within 0.07 s, leaves it
    within 0.30 s, or the jump button is released during the take-off; otherwise it coasts (keeps its momentum).
    The air-steer acceleration is `rate_f × (c_forward − s)` along the motion and `rate_side × c_side` across it.
  - Attack after 0.35 s in the air smashes (no speed of its own). The power button with the dive power-up after
    0.35 s dives: a 0.333 s hover, then 1556.3 units/s down until frame 100, then gravity.
- **Context button.**
  - On the ground: a hiding place within reach, then a pipe, then flattening against a wall ahead.
  - In the air: hooks, perch points, rails and ropes, then pipes.
  - Ledges are grabbed automatically while falling, inside the 60 × 70 box around the hands, from the wall's open
    side and facing the wall.
- **Attacks.**
  - Attack on the ground is a sweep (moving or standing); rush is its own button (550 for 1 s).
  - The smash landing raises `SmashLanded` with its 0.25 s window; the dive landing raises `DiveLanded` with its 300
    and 500 radii.
- **Damage.**
  - With a charm left every kind uses one and flashes the player for 2 s; water and pits then put the player back
    (`RespawnRequested` after the reaction). Crushing gives a knock-back.
  - Otherwise a life is lost, giving `Dead`, then `RespawnRequested`; −1 lives is `GameOver`. Moving backward
    faster than 250 gives the backward death, turned away from the motion by `clamp(−1 + 0.004·speed, 0, 1)`.
  - Entering a `Water` feature is water damage.
- **Power-ups.** The reward-mask bits are checked: dive 0x0012, rolling ball 0x0004, slow motion 0x1008, mine
  0x0040, speed-up 0x0080, decoy 0x0200, stun 0x8000.
  - Slow motion and speed-up set `time_scale` (0.25, and 1.5 at full strength).
  - Decoy and mine raise `SpawnPowerObject`.
  - The rolling ball launches at 850 (along the motion from 150, else the facing) and lasts while the button is
    held, at least 0.25 s.
  - Stun raises `PowerUsed`.
- **Animations.** `AnimOids` maps the states to lookup-list OIDs (player-moves.md 8.2):
  - run 130/131, jumps 140, 141, 142, 144, 145, hang 139, swing 344, pipe 155/156, rail 283 to 285, flatten
    159/165, duck 170, barrel 181/182, sweeps 243/251, rush 261, smash 263, dive 440, ball 282;
  - reactions with a charm by kind 233, 227, 228, 229, 230, 231, 233 (then 226 or 232); deaths 236, backward 234,
    water 235, crush 238, pit 237;
  - idle is the run blend at u = 0 (325 is the idle state machine, not a segment).

### Player tests

- **Unit tests** (`player::tests`):
  - the standing jump: 839.8 units/s, apex 155 at 0.3691 s, landing at 0.7383 s; with h = 0 (to test the fall
    drag): 1055.8, apex 245 at 0.4641 s, landing at 0.9293 s (0.9282 s without the drag), and at 60 frames/s;
  - the turn: 3.33 rad/s after the first frame of a 90° turn from rest, about 0.18 s in all;
  - the swing jump: apex 140.0 at 0.3508 s;
  - run speeds at u = 0, 0.25, 0.5, 0.75, 0.9 and 1, and the steady state at full deflection;
  - the rush: 550 units in 1 s;
  - the boosted second press (once per jump, 182 and 682 vectors, refused just before landing) and the smash;
  - coast and short steering;
  - ledge grab, hang position, no shimmy, ledge jump;
  - pipes (limits and reversal), rails (reversal), aimed perch landing, hooks;
  - flatten (and its hidden rule), duck, hiding places;
  - damage, charms (water uses one), flashing, death, backward death, respawn;
  - power-up bits, time scale, decoy, the dive's hover and plunge, the rolling ball's launch and hold;
  - animation choices.
- **Disc tests** (`tests/player_disc.rs`):
  - every [X] constant read from the executable equals the spec, and a modified executable is refused;
  - root-motion speeds: 38.3, 638.4, rush 550, barrel 72.6 and 416.7, flatten two at 120 and four at 135;
  - Sly spawned in Paris: settles on the ground, runs 1.5 s (about 445 units), and jumps and lands, with the
    animation driver writing finite transforms on the player's rig every frame.

## Camera (`engine::camera`)

The follow camera of camera.md. `FollowCamera` is the camera itself and is independent of the object model;
`CameraSystem` puts it behind the game's camera seam (`game::systems::CameraSystem`).

### Constants (`camera::consts`)

`CameraConstants::SPEC` holds the spec's values. `from_executable(&exe)` reads the [X] ones after the hash check,
and the disc test checks that what it reads equals `SPEC`.

- **Default framing** (0x275980):
  - distance −450;
  - offset coefficients (0, 1, 0), (0, 1, 0) and (125, 1, 0), unlimited;
  - focus at screen (0, −0.1);
  - adjust, follow and lock-on factors 1;
  - focus scale (1, 1, 1).

  The block's layout: the distance; coefficient vectors at +16, +32 and +48; (min, max) pairs from +64; the screen
  position at +88; the factors at +96; the focus scale at +112.
- **Springs and smoothing:**
  - eye spring (15, −8) (0x261920), over-damped: 88.6 % of a step after 1 s;
  - focus offset springs (8, −6) toward a move's focus point, (5, −3.5) back to zero, −10 decay when clamped;
    look-ahead spring (5, −3.5) toward 100 ahead [R];
  - base height smoothing (1000, 10, 1.0) (0x269BC0), lock (0.5, 0, 0.5) (0x275A58), follow (4, 0, 0.25)
    (0x275A80), recentre (2.5, 0, 0.35, 15) (0x275A90), field of view (0.5, 0, 0.1, 10) (0x2618B8), view clearing
    (2500, 0, 0.25) (0x261958).
- **Focus:** ground height kept 350 to 50 below the origin, base height 125 above it and kept 325 below to 75 above
  the origin; hook − 300; cut − 75.
- **Orbit:** 2.5 rad/s and 15 rad/s²; the stick factor (1, −2, −2) limited to (1, 2) (0x275A40, 0x275A50); the ramp
  after a manual orbit (0, 1, 0) (0x275A70); lock speeds 600 and 200 flattened; idle pan after 0.75 s still and 10 s
  without a manual orbit.
- **Look-around:** eye 70 above the anchor, −25 near and −150 far along the look axis (0x261C20, 0x261C10).
- **View clearing:** the record (250, 1500, 0) (0x26194C: sphere radius and extra speed), search box 250
  (0x261970), recheck 0.5 s, cut after 2 s; probes 50 sideways, 75 up, 50 down; aspect 1.2987 for the clearance
  radius.
- **Fade time:** 0.1 s.
- **Lens defaults:** field of view 1.0, near 100, far 20000, fog (500, 20000, 0.5).
- **Panels:** field of view 0.5, distance 400.

`CameraTunables` holds the values the spec leaves open. Every one is a guess to check against the game:

| Tunable | Default |
|---|---:|
| Look-around turn rate | 2 rad/s |
| Look-around pitch limit | ±1.2 |
| Distance that ends a manual framing | 400 |
| Conversation-view offset (forward, left, up) | (300, −150, 120) |
| Speaker head height for panels | 80 |

The focus clamps of camera.md 4.1 are measured from the player's origin, 90 above the ground contact
(`CameraConstants::origin_height`, player-moves.md 1.1). Standing, neither clamp acts.

- The follow factor eases to 0 while `Subject::follow_off` is set. `CameraSystem` sets it in the air and in the
  casual hiding kind; the world state does not carry the boost, hook, damage and sidestep states.
- On a cut, the nearest clear point around the origin comes from rays of 500 to the sample sphere of 0x261928
  (1, 5, 10, 12, 15, 12, 10, 5, 1 pans at tilts −90° to 90°, stored as floats). The framed eye is taken when the
  line from that point is clear, else the angle search around the focus.
- Option 884 is −1 none, 0 forced out, 1 forced in.

### Framing and zones (`camera::framing`, `camera::zone`)

- **Frames.** A `Frame` has an origin and forward, left and up axes (the camera basis of transforms.md 7). Each
  focus component is mapped by an `OffsetFn` (`clamp(c0 + c1·x + c2·x², min, max)`), then the eye steps the
  distance along forward. `Frame::reversed` gives reverse framing.
- **Cylindrical zones** (866) map (angle, radius, height) around the frame's up axis. Their angle follows the
  focus only beyond the slack angle (858).
- **Zone frames.** `ZoneFrame::Fixed` comes from the stored transform (path kind −1). `ZoneFrame::Path` is a
  polyline; the frame sits at the point closest to the focus, forward along the path.
- **`zones_from_stream`** reads every in-world camera zone: the transform, the convex volume, the path, and options
  851, 852, 854, 856 and 858 to 886 except 883 (853, 855 and 857 have no stream encoding, so the limits stay
  unlimited): pan times and strengths (873 to 876), cut on entry (877), field of view (878), no clearing (879),
  follow target, speed and limits (880 to 882), state override (884: 1 in, 2 out, 0 none) and the eye spring
  factors (885, 886). The zone's transform is applied to its path and fixed frame.
- **`select_zone`** picks the highest priority (870) among the zones containing the player (or forced in by 884).
  In the air, another zone takes over only with 871, and the current zone is kept rather than dropped.

### Fade groups (`camera::fade`)

- `fade_sources` lists every in-world solid with camera-fade groups and a node map: 83 on the disc, matching
  collision.md.
- `FadeSource::faded_meshes(transform, eye, focus)` pushes the segment through the solid's collision tree and
  collects the meshes of every reached node's group: the group's first entry and the following −1 entries.
- `FadeTracker` keeps each mesh faded for 0.1 s after it last blocked the view.

### The follow camera (`camera::follow`)

`FollowCamera::update(real_dt, &CameraInput, &Subject, &CameraScene) -> CameraFrame`:

- `CameraInput`: left and right sticks, the right-stick press, horizontal invert.
- `Subject`: the player's feet, velocity, facing, airborne; the ground contact height, a hook's height, a move's
  focus point, flattened, hidden, clamped, and a follow target's position.
- `CameraScene`: zones, the collision world, fade sources with their transforms, picture-panel speakers, letterbox.

It runs these steps (camera.md 4 to 7):

1. **Zone selection**; entering a zone with 877 cuts.
2. **Focus.** The ground height G (the contact's while standing, kept in the air, hook − 300, origin − 75 on a cut),
   clamped; the base point at the feet's x, y with its height eased toward G + 125; the look-ahead adjustment
   springing toward 100 ahead (none when hidden); the nose `B + adjust × 861`; the zone's focus scale in the zone
   frame; the focus offset (toward a move's focus point, back to zero, or decaying).
3. **Follow factor** eases toward the zone's (smoothing (4, 0, 0.25)).
4. **Manual orbit.** Target speed `x·|x|·2.5`, with the speed changing by at most 15 rad/s². It switches to manual
   framing until the player moves 400 units on.
5. **Automatic orbit** (manual framing only): −(sideways speed / distance), capped at 2.5, × follow factor ×
   `clamp(1 − 2y − 2y², 1, 2)` × `clamp(t, 0, 1)` since the last manual orbit. The sideways motion is the velocity
   or the motion toward the follow target (880 to 882), across the view or along it (872).
6. **Recentre** on the right-stick press: toward the player's facing, with the recentre smoothing.
7. **Framing.** The eye is kept in cylindrical form (θ, ρ, z) around the focus.
   - In a zone (unless in manual framing and the zone allows manual): the framed eye in the zone frame (reversed
     when allowed and the player faces against it) gives ρ and z; θ eases toward the framed angle with
     (0.5·L·w, 0, 0.5), L = min(1, speed / 600 (200 flattened)) × 863, raised to the pan ramps (868 with
     873/874, 869 with 875/876), w = c²·(3 − |c|) on a path (1 on a fixed frame, 3 hidden).
   - Otherwise ρ and z from the zone's or default framing at the orbit heading (450 and 125 by default).
8. **Eye spring** on ρ and z: `v ← (1 − 8·s₈₈₆·dt)·v + 15·s₈₈₅·(target − x)·dt`, then `x ← x + v·dt`.
9. **View clearing** (unless 879):
   - the squish: the camera meshes within 250 of the previous eye give planes; each asks for a velocity along its
     normal of at least the speed the smoothing (2500, 0, 0.25) gives from its distance to the clearance radius
     `tan(fov/2)·√(1.2987² + 1)·near + 1` (90.5 by default); the smallest non-negative combination of the normals
     that satisfies them (coordinate ascent on the weights) is added, capped at the eye's speed + 1500;
   - the cut when blocked: from 0.5 s after the zone became active, five probe points around the player (origin,
     ±50 sideways, 75 up, 50 down) are tested against the matching points on the near-clip plane; after 2 s with
     none clear the camera cuts to the framed eye if its line is clear, else the smallest clear turn around the focus
     in 16° steps, else the longest clear line; then out of surfaces within the clearance radius.
10. **Near-clip push:** `eye = F + e·(|e| + near)/|e|`; `raw_eye` keeps the position before it (the near-plane
    centre).
11. **Aiming.** The view is aimed so the focus appears at the framing's screen position, (0, −0.1) by default.
12. **Fades** along the eye–focus segment; picture panels; letterbox; the field of view eased toward the zone's
    (878) or the lens's.

**Modes.** `set_mode` cuts on a change of mode:

| Mode | View |
|---|---|
| `Follow` | the steps above |
| `Look { yaw, pitch, zoom }` | eye 70 above the anchor, −25 to −150 along the look axis; the left stick turns the view |
| `Animation { placement }` | a camera placement's world transform: forward, left and up columns |
| `Aligned { anchor, offset }` | the conversation view and scripted alignments |
| `Manual { yaw, pitch }` | a free camera |

`cut_to(eye, target)` snaps the camera to a new place.

**`CameraFrame`** holds:

- eye, target and up;
- field of view, near, far, fog and fog colour;
- the focus;
- the faded meshes (`FadedMesh { record, mesh }`);
- the letterbox flag;
- `panels: [Option<PanelView>; 2]`. Each panel looks at the speaker's head from the panel distance in front of
  it, with the panel field of view, screen position and light;
- whether this frame was a cut;
- the active zone's record.

It also gives:

- `view_matrix()`: column-major, right-handed Y-up; game space is converted with (x, z, −y) as in transforms.md 7.1;
- `projection(aspect, zero_to_one)`: depth 0..1 for deko3d, −1..1 for OpenGL;
- `project(point, aspect)`: normalised device coordinates.

### Behind the game seam (`camera::system::CameraSystem`)

It implements `game::systems::CameraSystem`:

- **`configure(world)`:**
  - lens and fog from world options 529 to 535;
  - the zones and fade sources from `world.level`;
  - a camera collision world built from the level's solids where they were placed, with the player's own solids
    disabled.
- **`update(world, input, real_dt)`:**
  - follows `world.player_target()`, with velocity, airborne state, the base point's height (the ground) and the
    hiding mode (flattened: focus point 50 above the origin; barrel: hidden) from `world.shared.player`;
  - recentres on `Buttons::RSTICK`;
  - while a dialogue holds the controls (`shared.dialogue` with `controls_locked`), switches to the conversation
    view at the dialogue object's transform;
  - takes the picture-panel speakers from `shared.panels` and the letterbox from `shared.screen.letterbox`;
  - returns the `CameraView`.
- **`cut(eye, target)`** snaps the camera.
- **`frame()`** gives the whole last `CameraFrame` (fades, panels, letterbox, fog).
- **`set_mode(Mode::Animation { .. })`** is for cutscene placements (animation events 12 and 13 enable and disable
  one).

**Requests to the game side (done):**

1. Pass `frame().faded` and `frame().panels` to the renderer. The seam's `update` returns only `CameraView`, so a
   trait method or a downcast is needed.
2. Call `set_mode` from events 12 and 13 with the placement's world transform each frame.
3. Moving solids are not in the camera's collision world yet: share the physics system's collision world when it
   exists (`CollisionQueries::collision_world`).

### Camera tests

- **Unit tests** (`camera::tests` and the module tests):
  - the settled default framing: focus 100 ahead and 125 above the ground, framed offset (−450, 0, 125), eye at
    focus + (−546.4, 0, 151.8) with near 100 and (−517.5, 0, 143.7) with near 70, the focus at screen y −0.1; the
    ground height kept in the air; the clearance radii 90.5 and 71.3;
  - the orbit: 0.25 rad/s after 1/60 s, 2.5 rad/s after 0.1667 s, x·|x| response, turning in the follow mode;
  - the eye spring: 88.6 after 1 s, within 1 % from frame 114, no overshoot; a zone with 885 = 0.5 is slower; in
    the camera a 100-unit move is followed without overshoot;
  - zone framing (with the near-clip push), path and cylindrical frames with slack, reverse framing, zone priority
    and air takeover;
  - view clearing: the eye keeps the clearance radius from a wall, and a camera behind a wall cuts back;
  - look-around offsets, cutscene placement, panels, letterbox, field-of-view easing;
  - fade groups through a tree, the fade timer;
  - projection.
- **Disc tests** (`tests/camera_disc.rs`):
  - the constants from the executable equal the spec's;
  - 442 camera zones on the disc, all read (209 with a path);
  - 83 fade sources;
  - Sly followed through a scripted run in Paris (idle, run, orbit, recentre, jump): the focus stays on screen in
    all 302 frames, and the eye stays within 2000 units of the focus;
  - `CameraSystem` behind the seam on the game's `World` for Paris: lens from the world options (1.1, 70, 60000), and
    the focus holds still without a player.

## Sly 2 characters (`engine::gang`)

Written from `specs/sly2/characters.md`, `gadgets.md`, `physics.md`, `clock-and-loop.md`, `camera.md` and
`animation.md`, with Sly 2 data from `formats-sly2`. The Sly 1 controller (`engine::player`) is unchanged.

### Modules

| Module | Contents |
|---|---|
| `gang` | `Who` (`Sly`, `Murray`, `Bentley`; `index()` 0, 1, 2 for the per-character arrays; `target_id()`), `Gang` |
| `gang::character` | `Character` (one state machine per character), `GangEvent`, `Shared`, `Combo`, the state numbers in `sly`, `murray`, `bentley` |
| `gang::consts` | `GangConstants` (`SPEC`, `from_executable` after the SHA-256 check), `spec` ([R] values), `GangTunables` |
| `gang::items` | `ItemTable` (43 records of eight `s32` at `0x2BCD00`), `Item`, `ButtonUse`, `Owned`, `Slots`, `Power`, `id` |
| `gang::health` | `Health` (40, difficulty scaling, 2 s flash), `Hit`, `Reaction`, the hit `kind` bits |
| `gang::throw` | `ThrowAim` (range, elevation `0.15 + 0.55 × far`, 29-step prediction, Long Toss), `Flight`, `turn_toward` |
| `gang::input` | `GangInput`, `button` bits, `PressBuffer` ("pressed within t", consumed by the action), `Target`, `TargetKind`, `GangWorld` |
| `anim::xform` | `XformRig`: Sly 2 transform lists with kinds, mirror pairs and two-bone chains |

### How the object model drives it

Each frame the object model fills a `GangWorld` and calls `Gang::update(dt, &input, &world)`. The `GangWorld` holds:

- the collision world, with the characters' own solids off;
- the nearby `Target`s: guards with their rank and "launched" flag, liftable or attackable objects, interactables;
- the job's difficulty help, and whether the safe house is the player.

The current player reads the pad and the others get an idle input. Each character sees the other two as `Target`s
(`TargetKind::Character`). The result is a list of `(Who, GangEvent)`: state changes, jumps and landings, punches
with their target, flop landings (with `inferno` when Raging Inferno Flop is owned), lifts and throws with the launch
velocity, crossbow swings with the high-clip weight, darts with their end point, gadgets summoned, triggered,
started, ended or refused (sound 46), and hits taken. Effects on other objects (damage to guards, bombs, the camera)
belong to the object model.

- Hits come in through `Character::hit(&consts, &hit, &world, &mut events)`.
- Scripts use `Character::hold(level)` (Murray's held state) and `Character::enter_timed()` (Bentley's 2 s state).
- `Gang::switch_to(who)` changes the player unless the character is absent, carried or thrown.
- When Murray lifts Bentley, Bentley rides 150 units above Murray. A throw launches him with the throw's velocity,
  and he becomes the player when he lands. If Murray lets go another way (a hit), Bentley drops off with
  `spec::BENTLEY_DROP` (400 back, 600 up).

The characters move with `physics::character::move_and_slide` (capsules) under gravity −2275 (−1750 × 1.3), and
fall after 0.1 s off the floor. Animation selection and root motion stay with the caller: `XformRig::root_velocity`
gives a clip's transform-1 velocity.

### States

- **Sly** (`sly::*`): stand 0, run 1 (R1), air 2, hook 3, damage 4, fight 5 (a guard within reach without R1),
  interact 7, sneak 10, pickpocket 11 (Circle behind a robbable guard), sneak-run 12, air attacks 13 and 14,
  throw aim 23, Stealth Slide 24. Ground choices in order: jump, fight stance, binocucom (R3), pickpocket, interact.
  Shadow Power moves at 0.75 of the run speed.
- **Murray** (`murray::*`): stand 0, run 1, air 2, Thunder Flop 3, lift 4, carry 5, held by a script 6, stance 9
  (a guard, object or Bentley ahead), walk 10, punch 11, charge 12, interact 13, damage 14, throw aim 15,
  Turnbuckle 21.
  - Punches: Square starts the first pair (clips 0, 1) and Triangle the second (2, 3), the first punch chosen by
    the target's side. The other punch of the pair is queued within 0.25 s, up to five punches. The stick steers
    with blend `0.5 + 0.5 × deflection`, and ends the combo after half the punch.
  - Circle grabs a launched guard within 300 horizontally and 100 to 500 above; Square within 400 and 0 to 600
    above juggles it (clip 4).
  - The flop: Square or Triangle in the air within 0.15 s, when he will not land within 0.1 s. A 0.2 s wind-up
    with no motion, then −500 down, steering toward `0.75 × 515 × stick` with gain 2.5.
  - Held by a script, the level drops by one after 1 s without a press.
- **Bentley** (`bentley::*`): stand 0, move 1, air 2, interact 3, damage 4, near 5, holding a gadget 6, walk 7,
  carried 8, throw aim 12, timed 14.
  - Square swings the crossbow at the attackable target nearest the point 175 ahead, within 1.5 rad of the heading
    (3.15 standing, turning to face it). The target's height from −50 to 50 blends the low and high clips.
  - R1 fires a dart along the screen centre from depth 10 to 40,000, stopping at the first solid. Darts last 4 s;
    reload 2 s.
  - Bombs (7, 9, 11, 13): a press summons and holds, releasing aims, Square or Cross throws, and the next press
    triggers a thrown 7 or 11.
  - Hover Pack, in the air only: 300 × stick across, −100 down, facing the motion above 200, 0.5 to 3 s, draining
    1 per second.

### Shared rules

- **Health**: 40 per character.
  - A change is scaled by `(1 − h)·1 + h·0.5` for help `h`, and halved again for a shrunk hitter.
  - A hit of the capacity or more is fatal unless it is a touch. The 2 s flash takes no damage.
  - The reaction by kind: Murray's knock-aside (1000, ±900, 950 in his frame), thrown (600 away, 800 up),
    knock-back along the hit's vector, crush (0, 0, −1000), stun, heal.
- **Gadgets**: slots L1, L2, R2 (bits 4, 1, 2 in the executable).
  - Only owned items with button use 2 or 1, owned by that character, go on a button.
  - A press within 0.15 s uses the item. It is refused when the state does not allow it or power is short. It is
    paid at once, except items that pay at their animation event or when thrown.
  - Hold items run while held: Thief Reflexes (time scale 0.25, 2 per second), Time Rush (1.35, 1 per second),
    Shadow Power (3 per second).
  - Power starts at 30, maximum 100, and drains in whole units.
- **Throw aim**: range and flight time ×1.5 and ×1.35 with Long Toss. The stick moves the distance (800 per second)
  and the heading (2 rad/s); the body turns at 3 rad/s. Square or Cross throws; Circle or Triangle cancels.

### Sly 2 transform rigs (`anim::xform`)

`XformRig::from_list(&formats_sly2::transforms::TransformList)` builds:

- the nodes: parent, kind, ID and loaded local matrix;
- the mirror pairs;
- the two-bone chains: upper, lower and end bones, effector, handle and target, with the lengths, bind bend and
  bend axis measured at load.

Methods:

- `reset()` returns to the loaded pose.
- `set_vector(i, v)` writes by kind: 1 translation, 6 rotation `Rz · Ry · Rx`, 12 the scale diagonal.
- `apply_clip(&clip, object, t)` evaluates a `formats_sly2::anim::Clip`'s channels for one object: vector channels
  by kind, scalar channels as chain swivels.
- `update(&root)` computes the world matrices, then the mirror pairs, then the chains.
- `chain_goal(k)` and `chain_effector(k)` expose the solve.

`formats-sly2` has its own posing (`formats_sly2::rig::Rig`) for its disc tests; the disc test here checks that the
two agree.

### Tunables (`gang::consts::GangTunables`)

Per-character arrays are `[Sly, Murray, Bentley]`.

| Field | Default | Note |
|---|---|---|
| `radius`, `height` | 30/90, 45/80, 35/60 | capsules |
| `turn_rate` | 12 rad/s | |
| `dead_zone` | 0.15 | |
| `reach` | 200 | targets for fight, stance, lift, interact, punches |
| `throw_range`, `throw_time` | 200 to 1500, 1.5 s | before Long Toss |
| `aim_speed` | 800 per second | |
| `flop_windup`, `hurt_time` | 0.2, 0.8 s | |
| `combo_clip_time` | 0.5 s | Sly 3 combo attack length (its clips' lengths are not given) |
| `fishing_range`, `map_directions` | 200, 8 | Sly 3: the Fishing Pole tip's distance ahead of Bentley; the treasure map's heading steps |
| `rocket_factor`, `ball` | 1.5; 800, 2.0/s | Sly 3: Rocket Boots' speed factor; Be The Ball's rolling speed and easing |

The move clips' lengths are `consts::clip` (characters.md 9.1): Sly's pickpocket 1.0 s (clip 280), air attacks
0.6667 and 2.0 s (190, 191); Murray's punches 0.6667 × 4 and 0.9167 s, picking up 1.0 s (452), Thunder Flop 1.6667 s
(417), Berserker clips 440 to 443, Turnbuckle 0.6667 s (509); Bentley's swing blends 0.5 s, Size Destabilizer
0.3333 s (341), Adrenaline 0.1667 s (588).

Also `physics::SolverSettings::sly2()` (at rest below 5 units/s and 0.05 rad/s, after one still frame) and
`physics::body::SLY2_GRAVITY` (−1750).

### Collision for the gang

`formats-sly2` exposes Sly 2 collision parts as `formats::collision` meshes and trees (`Solid::collision`). The
hub disc test builds its `CollisionWorld` from them: one body per collision part of every in-world solid, at the
solid's world transform from `World::load_sly2`, without the characters and what they hold.

### Gaits and jumps (characters.md 1.1 and 1.2)

`consts::gait` holds the gaits measured on the nightclub hub as (root travel, length) per clip. Their speed is
`Gait::speed(u)` = `((1 − u)·d_slow + u·d_fast) / ((1 − u)·T_slow + u·T_fast)`:

| Character, state | Speed |
|---|---|
| Sly run (1) | 640 × 1.3 (0x2BF3D4) = 832 |
| Sly move (12, and the sneak 10) | 225 to 510 by the stick (Shadow Power: the weight × 0.75) |
| Murray run (1), walk (10), carrying (5) | 800; 225 to 510; 125 to 516.7 (800 with Atlas Strength and R1) |
| Bentley move (1), walk (7), timed (14) | 750; 165 to 510; 1,500 |
| Sly's combat dodge | 1,500 for 1/3 s |

**Jumps by kind:**

- 1, 7 and 15 bring the base point to 215 above the last floor (989.1 standing).
- 6, Sly's double jump (Cross in the air, once per jump), peaks 315 above the first jump's floor.
- 13, 14 and 9 bring the base point to 500, 250 and 1,200.
- 2, 3 and 18 launch at 700, 800 and 900.

Air steering aims at 515 × stick. Murray's flop is refused after jump kinds 0, 11, 15 and 16.

**Drops:**

- When Murray drops what he carries (a hit), it gets (−400, 0, 600) in his frame (`GangEvent::Dropped`).
- Bentley, when a toss ends without a throw (a hit or a cancel), flies at 400 along the direction from Murray to
  him (+X at the same spot) and 600 up, then becomes the player when he lands.

Adrenaline Burst is Bentley's state 14 for 2 s at 1,500, his heading following the stick (gadgets.md 5.1).

**Move items** (gadgets.md 5.1 and 5.2; the constants are in `GangConstants`):

- **Feral Pounce:** jump kind 14 with air-steering × 1.75. With the stick under 0.2 it adds a push of
  515 × 1.75.
- **Mega Jump:** jump kind 9, steering × 1.5, Sly at 0.75 of his size until he lands.
- **Paraglide:** in the glider's frame the forward speed eases to 300 to 800, the sink to −200 to −400 and the
  sideways speed to 0, all by the stick's forward part. The easings are (800, 0, 0.1) and (5000, 0, 0.1). The
  acceleration is capped at 8,000, and the easing never pulls downward. It turns at a rate blended from 5 (stick
  back) to 12 rad/s (stick forward) × s, with s = stick magnitude × sin(stick heading − body heading); headings run
  counter-clockwise, so a stick to the left turns Sly left (gadgets.md 5.4).
- **Combat Dodge:** circles the target at 1,500 with the radius eased toward 250 by (500, 0, 0.5). The side comes
  from the stick, else the last side. It ends with the clip, the velocity halved. The target
  (`Character::dodge_target`) is chosen again every frame from all active guards with no distance limit: the lowest
  d − c·(500 + 0.5·d), with c = clamp(0.5 − 0.5·k, 0, 1) and k the stick's deflection times the cosine between the
  stick and the direction from the guard to Sly (gadgets.md 5.4). With no guard the clip plays without circling.
- **Knockout Dive** (Sly state 9): a rush at the target at 720/s for 0.917 s.
- **Berserker Charge:** 700/s for 0.66 s, then the stop at 250/s for 1 s; a wall ends it.
- **Diablo Fire Slam** (Murray state 8): standing still for 0.8333 s (`spec::SLAM_TIME`: its own clip, key 507, is
  missing, so it plays the carried slam, clip 492). **Lightning Spin:** 1.0 s (clip 348, `spec::LIGHTNING_SPIN_TIME`).
- **Juggle strike** (Sly state 16, `Character::juggle_strike`): 1.1667 s (clip 196), or 1.3333 s with Silent
  Obliteration (clip 198) (`spec::JUGGLE_TIME`). With Silent Obliteration Sly homes in, the distance eased toward 200
  with (1000, 0, 0.5) at half the frame step.

`gadgets::blast_of(item)` gives a bomb's blast:

| Bomb | Radius | Filter | State code |
|---|---:|---:|---:|
| Trigger | 300 | 256 | −20 |
| Snooze | 300 | 524,288 | −60 |
| Reduction | 600 | 16,777,216 | −5 |
| Rage | 600 | 33,554,432 | −5 |

The two-bone chains follow animation.md 6. A degenerate bone gives angle 0 and the axis (1, 0, 0). Parallel bones
take the perpendicular axis built from u's smallest component (`anim::xform::perpendicular_axis`). Scale channels
(kind 12) keep the off-diagonal entries.

### Gadgets (`gang::gadgets`)

From specs/sly2/gadgets.md, with the assignment screen of safehouse-and-shop.md 6.

- **Effects.** `effect::of(item)` classifies every item:
  - `Held` (12, 25, 29, 40, 42);
  - `Pocket` (26, 37, 38, 41);
  - `ThrownTrigger` (7, 11);
  - `Summoned` (9, 13);
  - `Overlay` (15, 34, 35);
  - `Activation` (8, 14, 21, 23);
  - `Jump(kind)` (30: 14, 31: 9);
  - `Move` (10, 16, 19, 20, 22, 24, 33, 39);
  - `Passive` (17, 18, 27, 28, 32, 36).

  `effect::pay_at` is `Start`, `Event` (23, 14, 21, 15, 34, 35, 8) or `Throw` (pocket items). `effect::world` gives
  the activated items' `WorldEffect`: `Smoke { resume: 2 }`, `Freeze { duration: 6 }`, `Frighten`, `Shrink`.
- **Use rules in the characters.** A press within 0.15 s of a slot button:
  - triggers a thrown 7 or 11 that is out;
  - removes an applied overlay;
  - or else is refused (`ItemRefused { item, reason }`): `State` when the state forbids it, `Power` when power is
    below the cost, `Pool` when a pooled item has fewer than 2 free copies (`GangWorld::pools`).

  State limits: the dodge only in combat; Thief Reflexes and Time Rush exclude each other and the hurt state; the
  Hover Pack only in the air; Diablo Fire Slam only while carrying.

  An item that pays at its event waits in `Character::pending` until the animation calls
  `Character::activation_event` (or `GangTunables::activation_time` passes). Then it pays and raises
  `ItemActivated`, `Overlay { on }` and `World(effect)`.

  Pocket items aim with the throw aim and pay on the throw. With Juggernaut Throw (17) owned, a throw by Murray also
  raises `Explosive(id)`.
- **Passives in the characters.**
  - Paraglide (27): R1 in the air, at least 0.3 s after landing (`SpecialJump(19)`, a slow fall).
  - Atlas Strength (18): Murray jumps (`SpecialJump(15)`) and runs while carrying.
  - Long Toss (36): see the throw aim.
- **Special jumps and moves.**
  - Feral Pounce and Mega Jump raise `SpecialJump(14)` and `SpecialJump(9)`; the Hover Pack raises `SpecialJump(17)`.
  - The Combat Dodge is a sidestep (state 6).
  - Adrenaline Burst is Bentley's sprint.
  - Knockout Dive, Lightning Spin, Berserker Charge and Diablo Fire Slam raise `Move(item)`.

  The move times are the clip lengths of gadgets.md 5.3; the pounce's forward speed is the spec's 515 × 1.75.
- **Objects for the object model.**
  - `Carryable` is the carryable's state machine, states 0 to 5. Each change gives goal 109 + state. Contacts are
    ignored 0.25 s after state 0. The Alarm Clock and TOM are used on landing; the Trigger Bomb and the Health
    Extractor are kept out until triggered; a used object fades for 0.25 s, then returns to its pool.
  - `blast_rays()` gives the blast's 92 rays (11 rings), and `blast_push` its push: `(1 − u²) × 100`, up to 1000
    along the ray.
  - `Capture` is the Health Extractor's capture: velocity × 0.5 + 400 up, springs 15 and 2.5, scale
    `1.2 − (t/3)²`, done after 3 s, search radius 400, held pose within 100.
  - `tom_target` finds TOM's guard (height counted twice, 32 searched).
  - `take_medicine` gives 10 health and 20 power, and is refused when both are full.
- **The Select assignment screen.** `grid_from_executable(exe, who)` reads the grids (Sly 18 cells at 0x2C2DE8,
  Bentley 9 at 0x2C3028, Murray 8 at 0x2C3148). `Assignment::open`, `step(dir)` and `assign(slot)` work as follows:
  - the stick moves to the nearest owned cell within 750 along and 40 across, else within 200 across;
  - L1, L2 or R2 put the item on that slot;
  - the screen answers `Assigned`, `NotOwned` or `NotAssignable`.
- `GadgetConstants::from_executable` reads the [D] values of gadgets.md 7 and 8.

### Pickpocketing (`gang::pocket`)

From specs/sly2/pickpocket-and-pickups.md 2 and guards.md 8. This is the API the guard behaviours call:

1. When a giver resets: `Pocket::fill(&row, &mut rng, task_done)`.
   - `row` comes from `PocketTable::from_executable(exe, GUARD_TABLE_VA)` (class 58) or `OTHER_TABLE_VA`, through
     `.row(episode)`.
   - A prize kept from the last fill stays, unless `task_done(kind)` says its task is done.
   - Otherwise a prize is drawn by the six weights, with the row's chance.
   - The coins drawn in the row's range are split over 2 or 3 entries (each at least 1).
   - The prize goes last; at most 8 entries in all.
2. Each frame for the marker: `Pocket::marker(alerted, current_is_sly)` gives `Glint` (the next entry is a prize),
   `Plain` (only coins) or `None`.
3. When Sly's pickpocket move (`GangEvent::Pickpocket(id)`) reaches its animation event: `Pocket::take()` gives the
   next `Entry` (`Coins(n)` or `Prize(kind)`).
4. `give(entry, giver, sly_centre, &GiveConstants, &mut rng)` gives the launches. Each copy goes to a point ±1.5 rad
   from the line to the giver, 100 to 150 out and 75 below the centre, with a flight of 0.75 to 1.2 s under −2250.
   Copies leave 0.05 s apart; coins are collected by themselves 0.15 s × their order after the throw.
5. On collection: `Purse::receive(entry)` adds the coins, or 1 to a loot record (kind 1114 + i is record i of
   `LootTable`). It returns false for a task item, whose task the mission system completes.

`GiveConstants::from_executable` reads 0x2C0CE0, 0x2C0CF0, and the gravity as the z of the vector at 0x2C0D60.
`LootTable::from_executable` reads the 64 records at 0x2C3288.

### The Sly 3 crew (`gang::sly3`)

From specs/sly3/characters.md, with gameplay-overview.md 3 and 4, transforms.md and animation.md, and Sly 3 data from
`formats-sly3`.

- **Members.** `Member`: Sly 52, Murray 53, Bentley 54, the Guru 55, Panda King 56, Penelope 57, Dimitri 59 and
  Carmelita 60 (`class`, `from_class`, `core`, `recruit`, `target_id`).
- **The crew.** `Crew { gang, recruits }`.
  - The core three run on `gang::Character` (Sly 2's mechanics, characters.md 5).
  - The recruits are placed and listed to the others as targets. Their moves and health are still open
    (characters.md 8), so `switch_to` and `enable` refuse them.
- **Arrival** (characters.md 2):
  - `find_arrival(checkpoints, id)` matches a checkpoint by its OID or by the ids its scripts added (default 424,
    `DEFAULT_ARRIVAL`).
  - `choose_player` takes the saved player, else Sly, Bentley, Murray, the first present.
  - `Crew::arrive` places the chosen player at the checkpoint. The others stay out until a task adds them with
    `Crew::enable`.
- **The executable** (characters.md 3):
  - `Sly3ItemTable` reads the 67 records of 60 bytes at 0x343098 (owners 24 Sly, 25 Bentley, 26 Murray);
    `to_item_table` gives the gang's form for slots and costs.
  - `LOOKUP_TABLES` and `read_lookup` read the per-character lookup tables.
  - `constants_from_executable` reads the constants found at their Sly 3 addresses (Murray's knock-aside, the
    crush, the dart depths, the costume gravity); the others are Sly 2's until found. `sly3::shared()` uses Sly 2's
    constants and an empty item table.
- **Gadgets** (specs/sly3/gadgets.md):
  - Each Sly 3 item runs with a behaviour (`sly3::behaviour_of`, stored in `ItemTable::extra`): the number of the
    Sly 2 item whose rules carry over (for example the Smoke Bomb 25 → 23, Temporal Lock 22 → 14, now Murray's; the
    Alarm Clock 8 → 26 and the Rage Bomb 15 → 37, now Bentley's), or one of `items::behaviour` for the new items.
  - The gang's item code works on behaviours; costs, ownership and drains go through the item's own number
    (`ItemTable::actual`, `Shared::has`).
  - The held items (button use 1, items 32 to 36) drain field +16 per second (3, 2, 3, 5 on items 32 to 35; the
    Treasure Map, 36, has 0 and is held without cost), accumulated by the clock; the item stops once power is 0 or
    less. Field +12 is the entry cost (1 on items 32 to 35). The Fishing Pole (7) is a press item with no cost. The
    item record's field +36 is the usage text (`Sly3Item::usage`).
  - `sly3::slot_bits` reads the slot table at 0x349040.
  - `gadget_class` names the gadget object classes 86, 113, 114, 115, 119 and 124.
  - `Owned` holds 128 flags (Sly 3 has 67 items).
- **The core three's Sly 3 moves** (characters.md 5; the unspecified numbers are `GangTunables`):
  - Sly:
    - the cane combo: Triangle presses (up to 3), then Square, Cross or Circle give `ComboFinisher { kind, level }`
      when that upgrade (items 41 to 49) is owned; the first spin level always is. Each next press must come within
      `combo_window`, 0.35 s (the combo definitions, class 148, set it; option 1668 would override it, but no record
      uses it). The combo attack is Sly's state 23 (generic 9);
    - Shadow Power 1 and 2 make him invisible (`Invisible(level)`) while held;
    - Rocket Boots multiply his speed by `rocket_factor`;
    - the three disguises toggle (`Disguise`);
    - the Treasure Map shows the trail (`TreasureMap`).
  - Bentley: no crossbow swing (the wheelchair); the Fishing Pole takes a pocket entry from a robbable guard within
    `fishing_range` (`Pickpocket`); the Grapple-Cam raises `GrappleCam`; the Hover Pack is passive, deployed by Cross
    while jumping.
  - Murray: Be The Ball toggles rolling (`Rolling`) at `ball[0]`, × `ball[1]` with Square held. Temporal Lock is his.
- **Pickpocketing:** the Sly 2 API with Sly 3's tables.
  - `sly3::pocket_table` reads the pocket tables (guards 0x346DC8, others 0x346BA8; a zero row, then episodes 3 to 7
    as rows 1 to 5). The row is episode − 2, an episode of −1 counting as 3; the training world (2) and Kaine Island
    (8) get the empty rows 0 and 6 (`behavior::sly3::sly3_pocket_row`).
  - `sly3::loot_table` reads the loot table at 0x349070.
  - `Purse::receive_from(entry, sly3::FIRST_PRIZE_KIND)` (kind 1255 + i is loot record i; pocket prizes 1258 to
    1284).
  - `sly3::shared_from_executable` builds the gang's shared rules.
- **Rigs.** `XformRig` takes Sly 3 lists unchanged. `apply_sly3_clip` and `root_velocity_sly3` apply
  `formats_sly3::anim::ClipData`, the scalar track winning when a channel has both (animation.md 2).
  `apply_channels` is the form both games share.

Disc tests (`tests/gang3_disc.rs`, `SLYNX_DISC_SLY3`):

- worlds with each member, over all worlds: Sly 36, Bentley 34, Murray 31, Guru 9, Panda King 6, Penelope 8,
  Dimitri 6, Carmelita 10;
- the item table: records 8, 25, 30 and 41, the held items' second costs and the owners;
- the nine lookup tables' counts and first records; the constants of characters.md 3.3; a changed executable is
  refused;
- clip counts per group (characters.md 4: clips with data driving the character's tree): Sly 342, Murray 250,
  Bentley 206 in i_trainer, v_ext and v_apt; Guru 93, Panda King 102, Penelope 115, Dimitri 83, Carmelita 134 in
  m_ext;
- v_ext's 35 checkpoints include OID 424 in `v_ext_rig`, the default arrival;
- the usage texts at field +36 (gadgets.md 1); the slot table (4, 1, 2); the two pocket tables' episode rows and
  prize IDs; the loot table (records 0 and 63, prize kinds, the zero-value copies) and the pickups' gravity −2250;
  the gadget objects over all worlds (86: 34, 124: 34, 119: 38, 113: 68, 114: 34, 115: 34); the held items, and the
  owners that moved (Alarm Clock to Bentley, Temporal Lock to Murray);
- Sly in v_apt (class 52, object 24): 146 transforms, transform 1 at (0, 0, 75), 4 chains, mirror 6 → 73. All 350
  clips in his tree are posed at four times each: finite, the mirror copied, and every chain whose goal is in reach
  on its goal;
- the core three, dropped on open floor near Sly in v_apt, land, run, jump and land; Carmelita cannot be switched
  to.

### Gang tests

- **Unit tests**, on a flat floor:
  - Sly: speeds (832 run, 510 and 320 move) and jump height (peak 215), the double jump (315), air attacks,
    pickpocket from behind, the fight stance; the gait table;
  - Murray: the five-punch combo by side; the flop (wind-up, −500, steering limit, inferno); lift, carry, throw,
    Long Toss and cancel; grabbing a launched guard; the script hold;
  - Bentley: crossbow angles and height blend; darts, reload and life; the timed state;
  - gadgets: hold drains and time scale, refusal, the Hover Pack, bombs (summon, throw, trigger);
  - damage reaction and recovery; Murray throwing Bentley, who becomes the player; switching;
  - health, items and throw have their own unit tests;
  - `physics`: the Sly 2 fall (875 units at 1750 units/s; 1137.5 for characters), the rebound ratio 0.45 to 0.55
    and the one-frame rest.
- **Disc tests** (`tests/gang_disc.rs`, `SLYNX_DISC_SLY2`):
  - gadgets and pockets:
    - `GadgetConstants` and `GiveConstants` equal the spec's (gravity (0, 0, −2250) at 0x2C0D60);
    - both pocket tables: episode 1's rows as given, row 0 empty, rows 2/3, 4/5 and 6/7 equal, prizes all loot
      kinds;
    - the loot table: values of records 0 to 5 and 62, records 30 to 35 worth 0, the sum 15,425, kinds 1114 + i;
    - the three grids: cell counts, Sly's first cell, `assignable` exactly when the button use is not −1, Long Toss
      on Bentley's grid;
    - every item's effect against its button use and usage text;
  - the executable's constants equal `GangConstants::SPEC`; item records 0 to 4, 6, 7, 9, 10, 12, 27, 29, 32, 36,
    41 and 42; slot bits (4, 1, 2); a changed executable is refused;
  - the hub's three characters: Sly 53/7 (139 transforms, 4 chains, mirror 10 to 73), Bentley 55/8 (76, 2, 75 to
    52), Murray 54/9 (145, 2, 11 to 84);
  - Sly's run clip on `XformRig`: root translation (16.0044, 0, 75) at 0.025 s, (160, 0, 75) at 0.25 s and
    (320, 0, 75) at 0.5 s; root velocity 640; swivel −0.21512; the mirror; chains in reach reach their goals; every
    transform within 0.5 of `formats_sly2::rig::Rig`;
  - the gang in the hub, on the hub's own collision: all three characters are drawn (Murray and Bentley parked near
    x = 40,000).
    - Sly is drawn perched over a gap in the collision, so the three start on the nearest collision floor within 300
      units below him. There they land and each runs for 1 s (Sly 252, Murray 504, Bentley 397 units).
    - After standing again, each jumps and lands, and Murray's flop lands.

## Tests

- **Unit tests** (`cargo test -p engine`) use synthetic data built in the tests:
  - curve evaluation, with the animation.md 9 vectors written as keys;
  - playback in every repeat mode, event order, pauses and the playback events;
  - priorities, blends, the rig, skinning and poses;
  - the geometric primitives (rays, closest points, sphere and capsule sweeps checked against each other);
  - the hierarchy, a step-shaped mesh and a two-body world;
  - convex volumes and the material formulas.
- **Disc tests** (`tests/disc.rs`) run when `SLYNX_DISC_SLY1` is set and check:
  - the four curve vectors of animation.md 9 on the stored keys;
  - every segment of the disc parses: 13,176 segments; 140,413 curves (pose curves excluded) with 3 non-increasing;
    tangent-kind counts; 7,013 root-motion channels; option counts;
  - the player lookup list: in all 41 worlds with the player, every required OID is below the player, and Paris has
    92 segments;
  - skinned vertices near their bones in Paris: 5,204 vertices;
  - the player's rig against the stream's world transforms, its skins, an animation applied and skinned;
  - The Hideout's world-mesh vectors and tree structure;
  - every solid parses, with the disc-wide counts;
  - the convex-edge rule;
  - the material tables, and refusal of another executable;
  - 63 of 75 volumes contain their own origin, and far points are outside;
  - the collision tree against the hierarchy;
  - Paris world queries against brute force.

## Object model and game loop

The rest of the crate is the runtime: the objects of a loaded world (`engine::object`, `engine::world`), their
per-class behaviour (`engine::behavior`), and the game loop with its systems (`engine::game`). It is written from
`object-stream.md`, `classes/`, `transforms.md`, `options-by-class.md`, `clock-and-loop.md`, `gameplay-overview.md`,
`scripts.md`, `pickups-and-progress.md`, `interactables.md` and `level-flow.md`. The game application that drives it
is `crates/game` (below).

### Objects (`engine::object`, `engine::world`)

- **Storage.** `World` keeps one column (`Vec`) per property, indexed by slot: class, OID, record, script module,
  parent and sibling links, local and world transforms, flags, options, behaviour. A `Handle` is a slot index and a
  generation; `World::slot(handle)` refuses a stale handle. Loaded objects are created in stream order, so slot order
  is load order. Destroyed slots (spawned objects) are reused with the next generation.
- **What becomes an object.** Every record in the world hierarchy: all 126 classes on the disc, with the layout of
  `object::class::class_info`. Stock templates (top-level OIDs 12 to 42) are not created; `World::spawn_template`
  copies one at run time. Records owned by state machines, explosion groups, warps and exits stay data of their
  owner.
- **Proxies** (classes/proxy.md). A proxy's sources, full or back-referenced, are created under the proxy's parent
  with the proxy's transform composed in (the re-parenting of proxy.md 2). A back-reference makes a fresh copy of the
  source's subtree, nested back-references included, without the source's own proxy's child list (proxy.md 3). The
  child list of a single-source proxy goes under the object it placed. Overrides are applied to their target after
  the sources; an override entry also attaches its script module to the target, the earlier module staying as the
  outer environment (scripts.md 11.3). The proxy stays as an anchor: `World::placed_by(proxy)` lists what it placed,
  and lookups "below" a proxy search what it placed.
- **Options** (`object::options`). Each object's option list is decoded by the option's encoding into an
  `OptionStore` that keeps every occurrence in order: `get` reads the last one, `all` reads "add" options. Actions
  (typed argument lists) run instead of being stored; option 2 takes the object out of the world.
  `World::set_option` is the one path for scripts and lists: it stores the value, applies the generic placed-object
  effects (21 and 22 kinematic motion, 119 real clock, 125 hidden) and tells the behaviour.
- **Identity across reloads.** `World::persist_key` is the record index, plus a load-order serial for clones; it
  names "this coin" in the progress, so a collected pickup stays collected.
- **Hierarchy and order.** Children keep the order they were added. The world keeps a preorder of the tree
  (`World::order`), rebuilt when the tree changes: it is the update order (parent before children, siblings in load
  order), and every subtree is a contiguous range of it.
- **Transforms** (transforms.md). Local transforms are what behaviours, scripts and animation write (`set_local`,
  `set_position`, `set_rotation`, `set_world_transform`); they mark the subtree dirty. `World::update_transforms`
  recomputes only dirty subtrees: the parent chain, the ancestor rotation and position override (extension bits 3
  and 2), degenerate matrices replaced by the identity at load, and the "not placed" sentinel. The same pass sets
  `ACTIVE` (in the world with every ancestor) and `VISIBLE` (active, not hidden, placed) and lists the drawables
  that changed.
- **Lookups** (`world::lookup`): `find_oid` (operation 103, whole world or below a context), `find_all_oid`,
  `find_nearest` and `find_all_nearest` (the meeting-chain rule of object-stream.md 7.4), `find_class`,
  `find_ancestor`, `children_of_class`, and the raw `slots_with_oid` and `slots_of_class` indexes. Operation 103 on
  a proxy's OID gives the first source it placed, and a source whose proxy has the same OID is found only through the
  proxy (scripts.md 11.4).
- **Queues.** Objects talk to the rest of the game through the world: `World::raise` (script events),
  `World::request` (`Request`: load a world by identity or by name and warp, the default exit, respawn, game over,
  rumble, dialogue, autosave, movie), `World::anim` (`AnimCommand`), `World::physics` (`PhysicsCommand`) and
  `World::play_sound`. Systems and behaviours talk to objects with `World::notify` (`Notice`: touched, hit, hang,
  released, impact, attack, blast), read by the behaviour on its next update; contact notices are sent every frame
  the contact lasts.

### The loop (`engine::game`)

`Game::new(world, progress, systems, config)` starts a world (gameplay-overview.md 3): it builds and runs the script
modules, attaches objects to their modules, raises event 0 for every object in load order, runs each behaviour's
post-load step (auto-start state machines, pickups already collected, key exits), starts option-23 and auto-play
animations, and computes the first transforms.

`Game::frame(&input, measured_seconds)` runs one frame; `Game::step_fixed(&input)` is the fixed 1/60 s step of the
test harness. The order (clock-and-loop.md 4):

1. game state: the sprint countdown;
2. the clocks (`game::clock`): the measured interval is clamped to [1/60, 1/30] s; the game step is that times
   enabled × debug × power-up × rate (0 while paused); the real step is the clamped step, or the game step when that
   is at least 1/60 s;
3. the real-clock pass: objects with option 119 update with the real step;
4. if game time advanced: the player system decides (camera-relative input, the time scale applied to the clock's
   power); kinematic movers move; every ticking object updates in tree
   order, each followed by its event 1 (frame time) when its module defines one; animations advance (also while
   paused, for real-clock playbacks); physics steps; the scripts' scheduled calls run;
5. transforms are brought up to date; the camera updates (every frame, real step); the audio frame is built;
6. finished animation playbacks are retired and reported to their owners;
7. the render scene lists the drawables that changed, and skinning matrices, particles and screen effects are
   computed; the player's events are applied to the progress and dialogue requests routed.

Script events raised by an object are dispatched right after that object's step, and while a script runs, the host
hands newly raised events to the interpreter at once (`ScriptHost::poll_event`), so every event is handled before the
next object updates. A chain of more than 100,000 events in one flush is cut off and counted.

`Game::arrive(warp, map_wipe)` is level-flow.md 3: it picks the warp (OID, then alias 159, then 158 after a map wipe,
then default 157), places the player, hides the warp's objects, plays its animations, raises event 2 on it, blocks the
exits until the player leaves them, and cuts the camera behind the warp (turned by option 160).

`Game::state_hash()` hashes the clocks, the progress, the random state and every object's position and flags
(FNV-1a), for the harness's determinism checks. Randomness comes only from the game's `Rng`, seeded by
`GameConfig::seed`.

### Systems (`game::systems`)

| Trait | Called | Used by the game |
|---|---|---|
| `PlayerSystem` (`load`, `decide`, `place`, `after_physics`) | `decide` first in a game frame, with the input; `after_physics` with the collision queries | `SlyPlayer` (`game::player_bridge`, over `engine::player`) |
| `AnimationSystem` (`load`, `command`, `advance`, `retire`, `skin`, `root_motion`, `is_playing`) | commands as objects queue them; `advance` each frame; `retire` at the end | `SegmentPlayer` (`game::anim_bridge`); `InstantAnimation` in tests |
| `PhysicsSystem` (`load`, `step`, `queries`, `exclude`) | after animation, game frames only | `PhysicsBridge` (`game::physics_bridge`, over `engine::physics` and `engine::collision`) |
| `CollisionQueries` (`point_in_solid`, `sphere_touches`, `sweep_sphere`, `overlap_sphere`, `line_clear`, `ground`, `collision_world`, `object_of_body`) | by behaviours, scripts, the player and the camera; given by `PhysicsSystem::queries` | the physics bridge's collision world; `NoCollision` answers "not available" and callers fall back (sphere radius, origin distance) |
| `CameraSystem` (`update`, `cut`, `configure`, `extras`, `set_placement`) | every frame, last, on the real clock, with the collision queries | `engine::camera::system::CameraSystem`; `FreeCamera` (`game::camera`) when there is no player or with `follow=0` |

`Systems::for_game(materials)` is what the game uses; `Systems::null()` is the cheapest loop, for tests. A system is
replaced by assigning its box.

- **Physics and collision** (`PhysicsBridge`). Every object with a solid or conveyor layout gets a collision body at
  its world transform with its material (option 128); clones share their source's shape. Bodies follow their objects
  each frame (one refit). Behaviours make objects dynamic with `World::physics` (`PhysicsCommand::Dynamic`,
  `Impulse`, `Release`): released pieces, thrown bombs. Every contact of the step is sent as `Notice::Touched` to both
  objects for every frame it lasts, hard ones as `Notice::Impact`; events 6 and 7 are raised when a pair starts and
  stops touching.
- **The player** (`SlyPlayer`). At load the class-7 object (OID 5) becomes `World::player`; its context (pipes,
  rails, perches, hooks, baskets, water, ledges from the collision world) and its animations go to `engine::player`
  (`PlayerWorld`, `PlayerAnimator`). Each frame the controller reads camera-relative input, moves through the
  collision world, places the object and writes its bones; damage from the world (`World::hurt_player`) goes to
  `Player::damage`; attacks become `Notice::Attack` on what they reach; `hidden_from` answers the guards; the
  controller's events (charm used, life lost, respawn, game over) are applied to the progress by the game.
- **The camera.** It gets the physics collision world (solids where they are now) with each update; animation events
  12 and 13 hand it a cutscene placement's world transform (`set_placement`, `Mode::Animation`); `extras()` gives
  the meshes it fades and the dialogue picture panels (`CameraExtras`, in `RenderScene::camera_extras`).

**`SegmentPlayer`** (`game::anim_bridge`) drives `engine::anim::Animator` on world objects. It parses segments on
first use, resolves the segment's root and channel targets with the nearest-OID lookup (animation.md 4.3), writes
local transforms, raises script events 9, 10 and 11, and handles the animation events: 0 (sound), 6 (dialogue), 12
and 13 (camera placement), 25 and 26 (wipe kind, warp OID and a 32-byte level name: a world request), 27 (default
exit), 40, 46 and 47 (animation.md), 48 to 51, 57 and 58. It gives the renderer skinning matrices
(`engine::anim::Skin` over world slots) and the guards their root motion.

### Output for the renderer and the sound engine

- `RenderScene` (`game::render_scene`): the camera (`CameraView`; option 529 is the vertical field of view), the
  clear colour (528), fog (532 to 535), the drawables that changed this frame, the game time for texture animations,
  the skinning matrices (`SkinFrame`: per skinned drawable, a range of mesh-space matrices), the camera's extras, the
  live particles, and the screen effects (`World::shared.screen`: darkening eased toward options 540 and 541,
  lightning tints, letterbox bars).
- `AudioFrame` (`game::audio_frame`): the listener, every object's ambient sound (options 101 to 109), the one-shot
  sounds raised this frame, the music intensity the guards ask for, voice lines, and the music factor of the pause
  screen.

### Scripts (`game::script_host`)

`WorldHost` implements `script::ScriptHost` over the world. Script objects are handles packed into an `ObjectId` (20
bits of slot, 12 of generation); the world object (operation 108) reads and writes the world option list. Option
values are converted by the option's encoding; an unset option reads as its encoding's zero. The options no list
stores (28 to 39, 127, 205, 844, 848, 849, 850, 880, 981, 1084; options-by-class.md 1.1) are live values answered by
the object model or the behaviour (`ClassBehavior::get_option`), and an option without a known encoding falls back to
the value's own type. The object references 205, 844, 848, 880 and 1084 are stored as the object's OID and read back
as the object (the nearest with that OID), or as the null object while unset or cleared; mass (127) is a float that
takes integer writes. The 117 action IDs of scripts.md 11.5 are run as actions. Modules attach to objects only
through override entries; an earlier module stays as the outer environment (scripts.md 11.3): the game attaches
with `Interpreter::attach_with_outer(object, module, outer)` after `load`, and counts refusals in
`Game::outer_refused` (none on the disc).
Operation 103 on a proxy's OID gives the first source it placed; a source whose proxy has the same OID is found only
through the proxy (11.4). Operation 112 uses `CollisionQueries::overlap_sphere`. `Game::script_error_kinds` counts
the error values by kind.

### Shared world state (`world::shared`)

`World::shared` holds what several behaviours and systems read: the player's state and hiding, damage events
(`World::hurt`, `hurt_player`, `take_damage_events`), the guards' alert groups and the music intensity, screen effects
and darkening, the playing dialogue's subtitle and panels, the voice line, the frame input, the camera's eye and
heading (camera-relative controls), the hiding basket, the particle pool, the player's vitals, the time scale, the
player's events and the cutscene camera placement.

Notices (`World::notify`) are stamped with the frame they were sent in (`World::begin_notice_frame`): a notice sent
during the update pass or by physics waits for the receiver's next update even when the receiver already updated
this frame; older unread notices are dropped.

### Behaviours (`engine::behavior`)

Every object has a `Behavior`: an enum with one boxed variant per class behaviour, and `None` for data-only objects
(static scenery, markers, lights, bones, segments). Only objects whose behaviour ticks, or whose module defines event
1, are in the world's ticking list. A behaviour is taken out of the world while it runs and gets a `Cx` (world,
handle, step, game time, progress, collision queries, animation system, random numbers).

| Behaviour | Classes | Spec |
|---|---|---|
| `Pickup` | 86 charm, 87 life token, 88 coin, 89 key, 90 sprint hourglass | pickups-and-progress.md 2, 3, 5 |
| `Bottle` | 46 clue bottle: spins 0.8 to 1.2 rad/s with a random sign, pulled in from 300 units with reward bit 0x20000, collected on contact | pickups-and-progress.md 4.1 |
| `Button` | 40: lists 251 to 256 and 264; off-animations from 252 and 268; goals only on a running machine | interactables.md 1 |
| `TriggerVolume` | 41, 42: lists 267, 268, 275, 276; event 12 only with option 277, on a change | interactables.md 2 |
| `Exit`, `Warp`, `Checkpoint` | 125, 116, 56 | level-flow.md 2 to 4 |
| `Machine` | 136, over `script::machine`; option 516 never starts a stopped machine | scripts.md 9 |
| `Guard` | 9 flashlight, 10 proximity, 11 to 13, 15, 17 to 19, 74, 75 (14 companions are script-driven) | guards.md |
| `Alarm`, `Sensor` | 47; 49 lasers, 50 searchlights, 51 pressure plates | alarms-sensors.md |
| `Breakable`, `Piece` | 33 to 38; 39 | interactables.md 4, pickups-and-progress.md 3.3, 3.4 |
| `Hazard`, `Water`, `Barrier`, `Conveyor`, `Basket`, `Bomb` | 54, 55; 31; 52; 57; 58; 59 to 63 | interactables.md 5 to 9 |
| `Dialogue` | 101 | dialogue-and-voice.md |
| `Emitter`, `Explosion`, `Lightning`, `Sky` | 80; 120 to 122; 145; 84 (lights, 81, are renderer data) | effects.md |
| `Vault`, `LockGroup` | 23; 93 | pickups-and-progress.md 4.2, level-flow.md 7.1 |

- **Guards.** Each guard belongs to an alert group (class 137, option 694, or its own). Flashlight guards see the
  player in their light cone with line of sight; proximity guards by range, angle and leash (options 844, 848, 849,
  850), latched once seen. Hidden players are not seen (`Player::hidden_from`). The group's level (patrol, attack,
  search, dead) drives the 17 states of guards.md 3; states play the animation bound to them (656), guards move by
  root motion and attack (`hurt_player`) when 6.2 allows; a hit stuns or knocks them out, a knocked-out guard stays
  defeated and drops its reward. Alerted groups trip their alarms and raise the music intensity.
- **Alarms and sensors.** Sensors (option 290) cycle through switching on, on, tripped, switching off and off; lasers
  test their beams (from the curve shapes), searchlights their zap cycle (focus 0.25 s, zap 1 s, unfocus 0.75 s
  unless options 317 to 319 say otherwise), plates their weight. A detection trips the alarm (detect mode) or, after
  it went off, hurts the player (damage mode). Breaking all alarm boxes (289) disables it for good. The alarm's own
  goals are the fixed 538 (ready), 543 (triggered) and 544 (disabled); option 297 sets other machines' goals.
- **Breakables** count hits (event 20) to option 227, then release their pieces to physics, free what they hold, set
  off their explosion and drop coins (233, or the adaptive difficulty when negative: `game::difficulty`, tables read
  from the executable). A broken breakable stays broken.
- **Effects.** Particles are simulated on the CPU into `World::shared.particles`; emitters add them continuously,
  explosions in bursts (and hurt and push what is near, `Notice::Blast`). Lightning tints the screen; skies follow
  the camera.
- **Dialogue** plays one at a time; crucial ones (969) cannot be cut short; auto ones (968) start on load. Talk lines
  end when the subtitle is revealed, the voice line done, the minimum time passed and the button (if any) pressed.
- **Vault.** Hints by progress before all bottles are collected; then three dials (hundreds, tens, units) and the
  combination of option 1046, the placement's value over the template's (Paris: 937).

**Adding a class behaviour:**

1. Write a struct with the class's state and implement `ClassBehavior` for it: `ticks`, `post_load`, `update`,
   `option_changed`, `action`, `animation_finished`, `get_option`, each with a do-nothing default.
2. Add a boxed variant to `Behavior` and to the `dispatch!` macro in `behavior/mod.rs`.
3. Return it from `Behavior::for_class` for its class IDs.
4. Test it on a synthetic level (`world::tests::fixture::Fixture` builds records, options, proxies, volumes and exit
   descriptors) with `Systems::null()`.

`Behavior::Custom(Box<dyn ClassBehavior>)` takes a behaviour that has no variant of its own yet.

### Sly 2 behaviours (`behavior::sly2`)

Attached to Sly 2 worlds by `Game::new` (`behavior::sly2::attach`; the Sly 2 loader attaches none). First, guards
and swarming creatures (classes 58 and 57, specs/sly2/guards.md), `Sly2Guard`:

- **Sight** (3): eye 80 above the guard; the view volume (option 315's object) is taken as its box (the guard
  volume's extents) until volume meshes are read; options 312 (bits 0 and 1), 313 and 314.
- **Hearing** (4): `World::shared.noises` (`behavior::sly2::Noise`, `make_noise` with the fixed templates 706 to 712
  or a class-117 template); a noise reaches by its height range and a radius growing from 15 % over min(lifetime,
  1) s; a guard takes, while free, the best noise by seen, kind, age and distance (kind at least option 323; alarms
  only about the player).
- **Jobs**: patrol (stands at its post: routes, option 273, are not followed yet), investigate (walks to the noise,
  looks 5.5 to 6.5 s; swarmers search after 2 s), pursue (near / far ranges 292 / 293; an alarm noise kind 4 at most
  once a second; 5 s without the target ends it), attacks (option 233's action objects: distance, height, angle
  ranges, wait after; a failed choice retries after 0.25 s), return home.
- **Hits** (7): hit points (254, default 40); a hit unseen within 0.2 s knocks out at once unless option 333;
  option 232 ignores hits; knocked out, it spills half (rounded up) of its pocket's coins
  (`World::shared.coin_drops`).
- **Pockets** (8): `gang::Pocket`, filled from the guards' row of the executable's table when the guard starts
  (`World::shared.pockets`, set by the session); `Notice::Pickpocket` takes the next entry
  (`World::shared.picked`).
- **Strikes on the gang** (guards.md 6.3, characters.md 2.1): a pursuing guard's strike goes to
  `World::shared.gang_hits` with the collision part's contact kind and value: guards 10 of kind 0x200 (knock-back;
  part tag 13056, value −10, `GUARD_STRIKE`), swarmers 3 (`SWARMER_STRIKE`; −5 and −10 per species once the
  collision parts' contact fields are read). The attack clips' part on / off events (15 / 16) wait for the Sly 2
  clips; the strike lands while pursuing in reach.
- Bosses have no phase table: their fights are staged by tasks and scripts through options 254 / 255 (hit points),
  256 to 258 (block), events 23 / 24 (knocked out), the busy flag 231 / 1350 and escape records 1340; the guard
  behaviour keeps those options and events, and the job system and scripts drive the rest.
- Placeholders (marked): walk and chase speeds (Sly 2 guards move by root motion, not read yet), turn rate, the hit
  strength of the gang's attacks on guards, the reach of area attacks and of coin collection.
- Option 233 called by a script adds an attack object to the guard.
- **Noise templates** (class 117, `NoiseTemplate`): option 1095 makes a noise at a position for an owner and gives
  back an object (`World::action_result`, returned by the script host; the Sly 2 host makes it a run-time noise
  object).
- Option 26 called by a script on a Sly 2 object takes it out of the world (our reading of "removed at start").
- Script action arguments name Sly 2 objects by their `s32` IDs.
- **Lasers, touch pads, detectors** (`behavior::sly2_sensors`, alarms-sensors.md 2 to 4): laser beams (class 85, option
  455) between two ends moving along curves (modes 8 loop and 16 back and forth with eased turns) or fixed at
  objects; a character touching a beam takes 10 units of the stun kind (`World::shared.gang_hits`, applied by the
  gang). Touch pads (class 83): contact pads pressed while a character stands on them, hit pads (1, 2 toggling,
  re-arm option 419); events 5 and 6. Detectors (class 16): the targets of option 349 seen through option 345's
  volume; event 28 (target, old, new) on every change.
- **Pickups** (`behavior::sly2_pickups`, pickpocket-and-pickups.md 3 to 6): coins, task items, medicine, loot items
  and clue bottles collected within their radii of the played character's centre; coins and loot go to the purse
  (`World::shared.picked`), task items to `World::shared.task_items`, medicine to the gang (taken unless health and
  power are full), bottles set bit *n* of the level's bottle record and stay gone.
- **Reactive props and trigger volumes** (`behavior::sly2_props`, interactables.md 3 and 4): props build their
  event lists from options 1025 to 1045 and run them on hits (adds, removes, explosions, drops of 2 to 3 coins for
  the played character's hits, script event 4 values, task requests; noise 706); after the last event script event
  5 and task event 0x11. Trigger volumes (24, 25) use the box of their collision points: enter (event 5, task event
  0x1A, the task requests of option 440), hold, leave (event 6, task event 0x1B), tracking (event 9).
- **Volume shapes**: a Sly 2 volume's collision points give its box (`sly2_props::volume_box`); guards and detectors
  use it for their view volumes, the spec's guard box when it cannot be read.
- **For the job system** (coordination with the Sly 2 host's owner): task events go to `Sly2Level::task_events`
  (0x11 prop done, 0x1A / 0x1B volume entered / left); task-state requests (task, state) to
  `World::shared.task_requests` (applied by `game::sly2_jobs`); collected task items to `World::shared.task_items`.
- **Sequence starts** (`behavior::sly2_commands`, dialogue-and-voice.md 2.1): the sequence objects take script calls
  613 (start with a starter), 614 (start with the current character) and 615 (stop); trigger volumes start the
  sequence of option 441 with the entering object, interaction spots (class 35, option 1070) with the character
  using them (`GangEvent::Interact`), guard command lists signal alarms (command 15). All go to
  `World::shared.sequence_requests`, which the Sly 2 session hands to its sequence player. Type-4 animation events
  and the safe house's and vault's starts wait for the Sly 2 clips and those systems.
- **Guard command lists** (class 113, options-by-class.md 2.2): options 595 to 612 append commands in stream order
  (at load and from scripts); `sly2_commands::run_commands(world, guard, list)` makes a guard work through one
  (context, look at, wait, go to at the given speed, follow a path or a ladder, pipe or rail along its curve (6, 13;
  start to end with direction flag 1, facing along it), labels and jumps, sequence alarms; messages, clips and
  sub-jobs pass until their systems exist). Commands 1 and 2 give random run and pause times: an idle guard
  alternates between running its list and standing still.
- **Generic actions** on Sly 2 objects (options-by-class.md 2.1): 20 set the parent (keep flags 3 and 15 keep the
  world placement), 24 move to another object's place, 25 add to the world, 44 / 45 show / hide meshes by mesh object
  ID (`World::shared.hidden_meshes`). Not yet: 64 texture scroll, 66 / 73 / 74 / 106 / 368 (texture-animation and
  clip / state-machine playbacks: no Sly 2 animation system in the session).
- **Music** (class 152, `sly2_world::SoundManager`): action 146 queues the world's music stream *n*
  (`World::shared.music_requests`); the Sly 2 session puts it on the sound engine's music stack
  (`audio::sly2::MusicQueue`). No action is known yet for the queue's stop.
- **Thrown objects** (79, 81, 82; `sly2_world::Thrown`, interactables.md 8): dynamic in the physics bridge
  (`PhysicsBridge::from_collision`, built from the gang's collision world); on its first contact it hits characters
  within 384 and objects within 383, pushes with 500, makes noise 709, raises event 5 and leaves the world; kind 1
  sticks where it lands. The flight radius (20) is a marked placeholder.
- **Effects, barriers, water, barrels** (`behavior::sly2_world`, effects.md 1, interactables.md 5 to 7):
  - continuous emitters (class 29) with the Sly 2 option numbers into the particle pool, within their activation
    radius (option 13, default 3,000), stopped for good by option 479;
  - explosion groups (31) set off all their parts (32, 33) on a blast (props, scripts); a part bursts its count (555)
    and plays its sound (589);
  - barriers (86): `barrier_blocks_characters` (mode 0: the lists collide; mode 1: the lists pass) decides whether
    the gang's collision includes them;
  - water (89): its collision parts are sensors (left out of the gang's collision); a character falling in faster
    than 100 splashes (sound 19, explosion 874); a contact value of −1000 or less is a lethal hit;
  - hiding barrels (88): a hit makes noise 706, knocks it over and raises event 11 (getting in, carrying and
    hiding belong to the gang).
  **For the Sly 3 behaviours** (aliases): barriers (`barrier_blocks_characters`), water (`Water`, `water_box`) and the
  barrel's hit reaction (`Barrel`) use only the option numbers listed; so do `SoundManager` (146) and `Thrown`.
  Use points and perches: interactables.md names no Sly 2 class for them (class 35 interaction spots start
  sequences, option 1070; climbing surfaces belong to the gang), so there is nothing to alias yet.
- **Patrol routes**: a guard's option 273 names a command list (guards.md 5); a guard on patrol runs it (with its run
  and pause times), and starts it again when it ends.
- **Sub-sequences**: a sequence's option 636 (sequence, starter or −1, no-wait flag) starts another sequence, and
  waits for it to end unless the flag is set (`game::sly2::talk::SeqCommand::Start`).
- Not done yet: lights (class 4, the renderer's `stage` lighting), skies (97); sequence starts from type-4 animation events, the safe house and the vault (waiting for
  the Sly 2 clips, state machines and job system).

### Progress (`game::progress`)

`Progress` outlives worlds: lives (5 at the start, at most 99), charms (0 to 2), coins with the 100-coin rule, keys
per episode, the per-level records (visited, key, vault, sprint and boss flags; bottle and checkpoint bits; collected
pickups; remembered buttons; defeated guards, broken breakables, played dialogues; the adaptive difficulty level; the
best sprint time), vaults and episode flags, the respawn warp, the sprint countdown, and the completion percentage of
pickups-and-progress.md 6 (114 tasks on the disc).

### Sly 2 worlds (`world::sly2`)

`World::load_sly2(bytes, &option_table, Sly2Options { row, info, max_depth, builtins })` loads a decompressed Sly 2
world into the same columns as a Sly 1 world. It is written from `specs/sly2`: object-stream.md, world-file.md,
transforms.md and classes/placement.md. What only Sly 2 has is in `World::sly2` (`Sly2Level`):

- the parsed header and package bodies;
- each object's `s32` ID, package, copy, transform list and bone attachment (`Sly2Object`);
- the actions of the option lists;
- the load counters (`LoadStats`).

```rust
let mut disc = formats_sly2::Disc::open_path(path)?;
let bytes = disc.read_world(row)?;
let opts = Sly2Options { row: Some(row as i16), builtins: BuiltinTable::from_executable(&disc.executable).ok(), ..Default::default() };
let world = World::load_sly2(bytes, &disc.options, opts)?;
let config = GameConfig { option_map: OptionMap::from_sly2_executable(&disc.executable).ok(), ..Default::default() };
let game = Game::new(world, Progress::new_game(), Systems::null(), config);
```

- **Packages.** Packages with the keep flag are created where they are stored, in package order. Every other
  package is a template, created once per placement (classes 6 and 7) under the placement object, up to `max_depth`
  (8) placements deep. Each package instance is a **copy** with its own serial.
- **Records.** Every record of a created list becomes an object under the object whose body holds it. That includes
  solid children, copy extras, explosion-group parts, and the lists of tracks and state machines (clips are
  objects).
- **Level rows** (object-stream.md 2.3). A record whose conditions exclude the row is left out with its subtree.
  Classes 132 and 133 are created anyway, out of the world.
- **Clone kinds.** A kind −1 record is skipped when its ID already exists in the same copy. A kind −2 record is
  skipped when its ID exists in any copy of the same package.
- **Options.** Options are decoded with the executable's option table (`world::sly2::options`). Value types follow
  object-stream.md 1.2 (the spec's working meanings). Typed argument lists have lists flattened, and the `u16` lists
  of type `0x16` are `Args` of `Arg::Short(0x16, _)`.
  - Values go to the object's `OptionStore`. Actions go to `Sly2Level::actions`.
  - Option 26 takes the object out of the world.
  - A fresh placement's kind-5 block applies to the placement object.
  - A copy's patches apply to the placement object (targets 5 and 6), or to the first object of that copy with the
    target ID.
- **Transforms.** The reference rules of transforms.md 1: −1 is the copy root (the placement matrix, or the
  identity), `n` the object's own transform, `0x1000 | n` the enclosing solid's transform, and `0x2000` the owner
  (the explosion group for its parts, the copy placement for its extras).
  - A solid's object transform is its transform 0. Its whole list is kept relative to it (`Sly2Level::pose_of`).
  - Locals are the loaded world matrices made relative to the parent object, so `update_transforms` gives back the
    loaded matrices.
  - An object hung from `0x1000 | n` records the bone and its matrix in the bone's frame (`Sly2Object::bone`).
    `World::sly2_follow_bone(h, &bone)` moves it with an animated bone.
- **Columns.** The class column holds the record's class and the OID column its ID when the ID fits in 16 bits; −1
  otherwise. 82,570 objects over the 43 worlds have wider IDs; `World::sly2_id`, `slots_with_sly2_id` and
  `find_sly2_id` (same copy first) use the full ID.
  - The record column indexes `Sly2Level::record` (a path into the package bodies), and `World::sly2_record` returns
    the record.
  - The module column is `Program::package_module(record package, script index)`, at every depth.
  - Persist keys are (copy serial << 32) | the record's header offset.
- **Not done yet.**
  - No behaviours are attached, so Sly 2 objects are data.
  - No drawables are registered; the renderer draws Sly 2 from `formats_sly2::scene`.
  - Sly 1 option meanings are not applied: generic option effects, option 2, option-23 animations and the script
    host's live options are skipped for a Sly 2 world.
  - `LevelData` holds the world bytes with an empty Sly 1 layout and stream, so a Sly 2 world needs
    `Systems::null()` (or Sly 2 systems) until the physics, animation and camera bridges read `World::sly2`.
- **Scripts.** `Game::new` builds the program with `world::sly2::program` (every package's script block) and gives
  the interpreter the executable's built-in and option tables (`world::sly2::configure_interpreter`). Objects attach
  to their module and get event 0 as in Sly 1.
- **ID index.** `Sly2Level` (and `Sly3Level`) keep the slots of each object ID, built at load. A destroyed object
  leaves the index and its slot's ID is cleared, so a run-time object reusing the slot has none (hook in
  `World::destroy`). `World::find_sly2_id`, `find_sly3_id` and `sly2_id_slots`/`sly3_id_slots` read the index
  without allocating; before, every lookup scanned all slots.
- **Script host** (`game::script_host::sly2`, used when `World::sly2` is set):
  - The world object is class 1.
  - Options are read and written by the executable's option table. A never-set option reads as its value type's
    zero: false, 0, 0.0, a zero vector or range, or an empty list. Writes have no Sly 1 side effect, and a write of
    an error value stores nothing.
  - An unset option with value type `0x12`, or with flag `0x1` and arguments, reads as a method. The table marks
    neither as an action, but the scripts call them. Calls go to `World::run_action` and are counted in
    `World::unhandled_actions`.
  - Searches (operations 112, 113 and 115) take the full `s32` ID through the script crate's wide-ID hook. They look
    below the context first, then in its copy, then in the whole world. A placement's ID names its copy's root
    (scripts.md 5): a search that matches a placement record finds the first object of the copy it placed instead.
  - Game operations, from scripts.md 4.2:
    - 97 is the level row and 99 its episode.
    - 100 to 102 are the current job, the current task and the checkpoint task, which the job system keeps in
      `Sly2Level::job`. Until it sets them they are the null object.
    - 103 is the difficulty help (`Sly2Level::difficulty_help`, 0 to 1) and 104 is always 0.
    - 114 maps a list of IDs to the objects found, in order, with the null object where nothing is found.
    - 118 is the sound manager (class 152) and 137 the HUD prompt (class 154), made on first use. 118 reaches the
      host through the player hook, because the script crate runs it as Sly 1's operation 107.
    - 121, 123 and 124 find nothing.
    - 131 reads a saved progress value from the job system's progress block, by its path of name symbols (the
      symbols are the save names' IDs, with an index for an array). A missing path, or no job system, gives nil.
    - 138 and 139 read and set the owned-item flags (`powerUps/mppukfAcquired` in the block). Setting one queues
      task event 59 in `Sly2Level::task_events` for the job system.
  - Options of value type `0x2000` + c hold the ID of an object of class c (for example 1243 a job's first task,
    1264 a task's job, 1259 an exit). Reading one gives the object, searched from the reading object, or the null
    object.
  - With the job system attached, the host answers task options 1282 (state) and 1267 (play count) and runs the
    methods 1280 and 1281 (task requests), 1238 and 1239 (end or restart a job), 786 (fire an exit) and 778 (warp);
    see "Sly 2 jobs and progression" below.
  - Option 1095 on a noise template (class 117) is run by the template's behaviour (`behavior::sly2`), which makes
    the noise. The host then returns a new noise object: a class-147 object with no record (147 is the class whose
    option map holds script option 959, the investigation point 1098), kept in
    `Sly2Level::noise_objects` and destroyed on a later call once its noise has expired. Setting its option 1098
    (the investigation point) moves the noise there (our choice).
  - Custom script properties declared by option lists are given to the interpreter before any script runs
    (`world::sly2::custom_properties` and `seed_custom_properties`): stream options 3 (boolean), 4 (integer), 5
    (float) and 8 (an object ID, kept as the integer the scripts search for), each with the property ID as a
    tag-21 argument. A placement's go to its copy's root. Without them the aim managers' searches by properties
    1314 to 1317 found nothing and called the undefined procedure 1533 (`Unbound(1533)`, 2 to 48 per world in
    the smoke route).
  - Over the 90-frame runs of all 43 worlds no error value is the host's (`ScriptError::Host`). 49 error values
    remain (4,329 before the noise class and the custom-property seeding), from handlers that recurse until the
    game's state changes (`TooDeep`) and values the scripts get from actions no behaviour runs yet.

Hooks in shared files, each marked `Sly 2 hook`:

- `world/mod.rs`: `pub mod sly2`, the `World::sly2` field, and the two option guards;
- `game/mod.rs`: the program and the interpreter tables come from `world::sly2`, the declared custom properties
  are seeded (`Sly 2 script hook`), and option 23 is skipped;
- `game/script_host.rs`: `mod sly2`; `class_of`, `get_option`, `set_option`, `player` and the new
  `find_object_id`, `find_objects_id` and `game_operation` go to the Sly 2 host; `call_action` hands the job
  system's methods and option 1095's result to it; the live options are skipped.
- `crates/script` (`host.rs`, `builtins.rs`, marked `Sly 2 wide-ID hook`): `ScriptHost::find_object_id` and
  `find_objects_id` take an `s32` ID (defaults use the 16-bit searches), and the Sly 2 operations 112, 113 and 115
  call them.

#### Sly 2 world tests

- Unit tests: value types, the `u16` list, and typed arguments with a list and an unknown tag.
- Disc tests (`tests/sly2_worlds.rs`, `SLYNX_DISC_SLY2`):
  - All 43 worlds load at their level row: 280,356 objects in 2.6 s.
  - The objects per class, the creation order, the IDs and the world matrices after `update_transforms` equal an
    independent walk of `formats-sly2`'s records and placements. The matrices agree within 2e-3, translations
    relative to their size.
  - Every record column resolves to its record, every module equals `Program::package_module`, and the persist keys
    are unique.
  - No option fails to decode, no placement is cut by the depth limit, and no loaded placement names a missing
    package: the 378 that do (classes/placement.md 1.3) are all excluded by their own level conditions.
  - Each world runs 90 fixed frames with `Systems::null()` twice without scripts and twice with them. The state
    hashes match, and 1 to 1,876 objects per world are attached to modules.
  - No error value comes from the host. The test prints the actions no behaviour runs yet, by count: 44 (338),
    74 (123), 233 (104), 66 (98), 20 (83), 45 (77), 106 (77), 479 (54), 73 (51), 146 (43), 368 (26), 64 (21),
    25 (18), 77 (14), 24 (7), 595, 606, 1126, 1128 and 1130 (6 each), 1043 (5) and 1422 (4).

### Sly 2 jobs and progression (`game::sly2_jobs`)

Written from `specs/sly2/missions.md`, `level-flow.md`, `saving.md` and `safehouse-and-shop.md`. The system is
attached to a loaded Sly 2 world (`sly2_jobs::attach`) and lives in `Sly2Level::jobs`; without it a Sly 2 world runs
as before.

- **Graph** (`graph`). Every world of an episode carries the whole episode's jobs (class 132) and tasks (class 133).
  The single options come from the option store, the typed lists from `Sly2Level::actions`: object switches 1287,
  state-machine goals 1288, the job-complete screen 1258, the ending and closing events 1236 and 1237, and the
  jobs' load-time switches 1255. The
  prerequisite and successor lists and the internal names are read after the option list. Each save key (options
  1260 to 1263 and 1295 to 1299, paths of save name IDs) is identified by the last name of its path
  (`focus_count`, `complete_count`, `is_complete`, `state`, `from_memcard`, `time_played`, `suck_value`).
  - On the disc the job keys are 1260 time played, 1261 difficulty help, 1262 play count and 1263 completion. This
    differs from the order missions.md 2 gives, so the names decide.
- **Progress.** The live block is `save::sly2::Progress`, and it is the only saved state. Task states, play and done
  counts, job play counts, completion, timers and help are written into it as they change, along with
  `widCurrent`, `episodeCurrent`, `episodeBest`, total and per-episode play time, and the episode's
  `percent_complete`.
  - The episode's group is the `episode` group's child chosen by the level table's episode byte, for example
    `episode/nightclub` for episode 1.
  - The current job, current task and checkpoint task are not saved. A world change carries them with the job timer
    and help (`Carry`, returned by `detach` and given to `attach`).
- **Restore** (missions.md 7). The saved states are read, and a restarted task that was available without its
  prerequisites goes back to locked. Locked tasks whose prerequisites are done become available, repeated until
  nothing changes. Every task's switches are then applied for its state, and every job's load-time switches (1255)
  for the job's state (0 not begun, 1 current, 2 complete; our choice). The current job, task and checkpoint task
  come from the carry; after a load the task with option 1273 that was restored as available becomes current
  again, and its job with it.
  - A queued task is requested forced. A new game queues task 1583, which lies past the attract-mode tasks (our
    choice).
- **Task rules** (missions.md 4):
  - requests go one step up or from 1 to 3, a forced request skips the checks, and a closed task never changes;
  - a refusal sends task event 0x27 to the job;
  - done or closed raises the prerequisites, and done tasks of another job close;
  - successors are re-evaluated, and option 1272 makes a successor current at once;
  - switches are limited to their row, and groups 4 and 5 are "became" and "stopped being" current;
  - the marker (1279) is present only while its task is available;
  - the percentage counts closed tasks among those with successors (1 to 99 unless none or all);
  - outside a job, a done task whose successors all belong to jobs asks for an autosave.
- **Jobs** (missions.md 5):
  - a job begins when its task becomes current;
  - the timer runs only while the controller was touched in the last 10 s;
  - on completion the screen is reported, the last task closes (unless 1242), the job ends, an autosave is asked,
    and the next task is requested, else the job's exit fires;
  - ending saves the timer and help, shifts `suck_history`, and resets the hub respawn point to 393;
  - a restart at the checkpoint makes the checkpoint task current and asks for its world at its point.
- **Help** (missions.md 6): +0.1 on task events 0x10 and 0x1D, −0.2 the first time a task of the job is done,
  clamped to 0 to 1. During a job the value is the job's own, otherwise the mean of `suck_history`.
  `Sly2Level::difficulty_help` follows it, as does `Sly2Level::job` for operations 100 to 102.
- **Events.**
  - Task events from operation 139, the props and volumes (`Sly2Level::task_events`) and the job system are
    broadcast as script event 17 (event, object) to the current job's available tasks, then the current job.
  - A task that becomes current gets script event 19 and one that stops being current gets 20. A state change gets
    22 before and 21 after. A job gets 19 when it begins and 20 when it ends.
  - Requests from props and volumes (`World::shared.task_requests`) are applied every frame.
- **Script methods**, as script ID → stream ID:
  - task 1104 → 1280 requests a state `(state, by)`, and 1105 → 1281 does the same forced;
  - task 1284 raises script event 23 on the task, 1290 to 1293 force state 0 to 3, and 1294 makes the task
    current;
  - job 1067 → 1238 counts a failure (help +0.1; the job goes on), 1068 → 1239 restarts at the checkpoint (every
    job's handler for event 16 calls it), 1240 ends the job, and 1250 resets its tasks' play counts.
- **Worlds** (`flow`, level-flow.md 5 to 7):
  - Exits (class 26) take their destination and movies from the file records that end their body.
  - An exit fires when the player enters its volume box after having left it (`behavior::sly2_props::volume_box`).
    Task 1278, job 1259 and the scripts' 786 also fire exits.
  - Firing applies the exit's (task, state) pairs (788) and asks for a load: row, point (779, default 393), wipe
    (780) and up to 4 movies.
  - `arrive` finds the last class-42 point answering to the ID (its own, or 393, 394 or its argument by options 790
    to 792) and places the player there. It then applies the point's lists: 793 a state-machine goal (object,
    machine, goal, instant; the field order is our inference), 794 a task-state request, 795 a move of an object to
    a target. It resets the exits, sends script event 5 to the point, and broadcasts task events 26 and 5.
  - Warp points (class 41) move the player with their method 778.
- **Safe house** (`shop`):
  - the Buy page lists open, unowned items 7 to 42; buying pays the price, sets the flag and sends task event 59;
  - the Sell page lists loot held with a value, and selling one adds its value;
  - coins are capped at 9,999;
  - the job help lists open jobs with a check mark when the last task is done, and up to 4 goals for a job.
- **Notices:** the "job complete" screen and the autosave are reported as `Notice`s; the session shows them
  (`crates/game/src/sly2/screens.rs`, `ui::sly2`). State-machine goals go to `game::sly2_anim` through
  `Jobs::machine_goals`.
- **Session** (`crates/game/src/sly2/progress.rs`):
  - every loaded row gets the system attached with the carried block and arrives at the pending point;
  - a load the system asks for is performed after the frame;
  - `new_game` (row 0, point 393, task 1583), `continue_game` (the saved episode's hub, or the menu world at 100 %)
    and `save_game` go through `save::sly2::Sly2Store`.

Hooks, each marked `Sly 2 hook`:

- `engine/Cargo.toml` depends on `save`;
- `game/mod.rs` declares `pub mod sly2_jobs` and runs `sly2_jobs::step` after the scripts when a system is attached;
- `world/sly2/mod.rs` adds the `Sly2Level::jobs` field;
- `script_host.rs` adds the `call_action` pre-hook;
- `crates/game/src/sly2/mod.rs` adds `mod progress`, the pending state, attaching after `load_row`, and following
  load requests in `update`;
- `crates/save/src/lib.rs` declares `pub mod sly2`.

Tests:

- unit tests (`sly2_jobs::tests`) of the restore, the request steps, a job from beginning to completion with the
  help, the next job, close events, a restart at the checkpoint, the carry over a world change, option 1273 after a
  load, and the script methods;
- the disc test `tests/sly2_jobs.rs` (`SLYNX_DISC_SLY2`):
  - the tree is 5,461 records, layout 20,632, checksum `0x2767B4`;
  - a new game in row 0 makes task 1583 current in job 1671 (`m0_intro`), with operations 100 and 101 filled;
  - a task request completes the task, opening its successor and counting it done;
  - coins and an item are set, the progress is saved to a slot and loaded into a fresh world with every state back
    (the current task only through option 1273);
  - exit 1733 of row 3 asks for row 2 at point 393, and the hub is entered there with `widCurrent` 2 and
    `episodeCurrent` 1;
  - in the hub, buying (refused, then paid) and selling, episode play time, and no host errors in any of the runs.

### Sly 2 state machines (`anim::sly2_machine`, `game::sly2_anim`)

Written from `specs/sly2/classes/animation.md` 2, `animation.md` 8, `options-by-class.md` 2 (options 73, 74, 106,
368) and `dialogue-and-voice.md` 4, with Sly 1's run rules (`../sly1/scripts.md` 9).

- **Record** (`anim::sly2_machine::MachineDef`). A class-111 body is read after its member list: the states (state
  ID, object; −1 and 32 mean none), then the transitions (from, to, flags, an unused word, a probability, a clip).
  A transition whose clip is not a member is dropped. All 2,603 machines on the disc read exactly to their option
  list: 16,127 states and 28,766 transitions are kept.
- **Run** (`MachineRun`). A goal different from the state starts the first step of a shortest route (two-way
  transitions walked backwards). Among equal first steps the run picks by probability, or the last one when all
  are 0. A step lasts its clip's duration; one without a clip is taken at once; with no route the goal is cleared.
  A goal set "instantly" jumps there. The transition groups (flag bits 2 to 12) are not used yet.
- **Playbacks are objects** (`game::sly2_anim`, in `Sly2Level::anim`). Applying a machine (368) or a clip (106)
  makes a run-time object of class 136 or 134, the classes the option results name (`0x2088`, `0x2086`). Option
  74 on a target finds a machine's playback by the machine's ID, and 73 a clip's.
  - A machine with no playback on the target gets one at its first state when it is looked for (our choice: the
    scripts set a goal on what 74 returns at once).
  - Machine playback options: 372 goal (the scripts' 478), 375 state, 376 goal read back, 371 target, 377 speed, 379
    jump to a state (our inference), 380 remove.
  - Clip playback options: 116 clip, 117 target, 118 time, 119 speed, 124 remove. Flag 8 of 106 loops the clip
    (our inference).
  - The option-123 method on clip playbacks (721 calls in the 90-frame runs) has no meaning yet and stays an
    unhandled action.
- **Goals from elsewhere:**
  - the job system's goals (task option 1288, arrival option 793) go through `Jobs::machine_goals`;
  - voice lines' timed records reach the speaker (the session's `cues.rs` from the talk's `fired` list): targets 0,
    1, 2, 4 and 5 set goals on its machines 1548, 1549, 1550, 1551 and 1552 (5 is the mouth), and target 3 sets
    `Sly2Anim::eyes`.
- **For the `engine::anim` owner:** the clips are not applied to the targets' transforms yet.
  `Sly2Anim::playing()` lists (target, clip object, local time) every frame for the animation system to evaluate
  (`formats_sly2::anim::Clip` on the target's transform list), and `Sly2Anim::eyes` holds each speaker's eye
  closure. State changes are kept in `Sly2Anim::entered`; no script event is raised for them yet.
- **Hooks** (each marked): `anim/mod.rs` (`pub mod sly2_machine`), `game/mod.rs` (`pub mod sly2_anim` and its
  frame step after the job system's), `world/sly2` (the `Sly2Level::anim` field), the Sly 2 host (playback
  options and methods), and `crates/game/src/sly2/talk.rs` (the `fired` list).
- **Tests:** unit tests of routes, choices and a run; the disc test `tests/sly2_anim.rs`, which checks that every
  machine reads to its options and that the hub's safe-house machine 27339 walks to its computer state 10. Sly, Bentley
  and Murray each have all five speaker machines, and take a mouth goal and an eye value.
- With playbacks answering options 74 and 368, and nil and "no value" stored as no object and as nothing, the
  90-frame runs of all 43 worlds go from 4,334 script errors to 49.

### Sly 2 job screens (`ui::sly2`, `crates/game/src/sly2/screens.rs`)

Written from `specs/sly2/user-interface.md` 2, 5.4 and 6.3.

- Screens go through the show, idle and hide phases on the real clock. The label lengths are in the screens'
  pictures, which are not drawn yet; 0.6 s and 0.3 s stand in for them (the prompt's labels).
- **Job complete:** shows each part of the job's screen (option 1258) for 2.5 s or until Cross (our choice), then
  hides. The game pauses meanwhile (the clock's rate is set to 0).
- **Autosave:** at an autosave notice the session stores the timer and help and calls the platform's saver. The
  Switch program saves to slot 1 through `Sly2Store`. The saving screen (1492) shows during the save and for 1 s
  after.
- **Job help** (1248) opens with + (Start) during play, because the pause menu that offers it is not built (our
  choice).
  - Page 1 lists the open jobs with a check mark; Up and Down move, and Right or Cross opens a job.
  - Page 2 lists its goals; Left goes back.
  - Circle or + closes it, and the game pauses while it shows.
- **Drawing:** `Sly2Session::screens_draw_list` gives panels with the world's strings (through the talk module's
  `string`). The Sly 2 program has no 2D layer yet, so nothing of it reaches the screen on the Switch, and no Sly 2
  font is read.

### Sly 3 worlds (`world::sly3`)

`World::load_sly3(bytes, &option_table, Sly3Options { slot, context, info, max_depth, multipliers })` loads a
decompressed Sly 3 world into the same columns, built like `world::sly2` from `specs/sly3` (world-file.md 6,
object-stream.md 2.3, transforms.md, classes/placement.md). What only Sly 3 has is in `World::sly3` (`Sly3Level`).
Sly 2's hooks are shared, not forked: the `World::sly3` slot sits beside `World::sly2`, and `World::sly1_meanings()`
(false for Sly 2 and Sly 3 worlds) replaces the `sly2.is_none()` checks before Sly 1 option meanings (generic
option effects, option 2, option-23 animations, live script options). Options reuse Sly 2's `Decoded` and
`args_of` with Sly 3's value sizes (`world::sly3::options`).

```rust
let mut disc = formats_sly3::Disc::open_path(path)?;
let bytes = disc.read_world(slot)?;
let opts = Sly3Options { slot: Some(slot as i16), multipliers: disc.tangent_multipliers, ..Default::default() };
let world = World::load_sly3(bytes, &disc.options, opts)?;
let game = Game::new(world, Progress::new_game(), Systems::null(), GameConfig::default());
```

- **Groups.** Shared groups are created where they are stored. Others are created once per class-8 placement
  naming their group id, under the placement, up to `max_depth` (8). Mode-2 placements (spawners) create only the
  placement object.
- **Placements.** A copy placement's extra objects are created with the copy. A fresh placement's data is skipped.
  Placements past a group's copy count after option set 0 are counted (`LoadStats::over_copy_count`).
- **Selection.** Mode-1 records whose world filter or conditions fail in the context (default: the slot and option
  set 0's condition) are left out with their subtree. Modes 2 and 3 skip IDs already in the group or copy.
- **Options.** Values go to `OptionStore`, actions to `Sly3Level::actions`, and option 27 removes the object.
  - A bind applies to the placement (target 7), the copy's first object (8), or the copy's object with the target ID.
  - Grouper objects start out of the world.
- **Transforms and columns.** These work as for Sly 2.
  - The per-class index is widened to 256 classes.
  - Effect components are not objects.
  - The module column is the program-wide module the record's script index names in its own group (the group's
    first module plus the index, as `script::Program::from_sly3_bodies` numbers them; −1 for none). The index is
    also kept in `Sly3Object::script`.
  - Scripts (specs/sly3/scripts.md): `world::sly2::program` builds a Sly 3 world's program from its group bodies,
    and `configure_interpreter` passes the option table with `set_sly3_tables` (no built-in table: the spec's argument
    rules equal it). The game gives the option map read with `OptionMap::from_sly3_executable`. Hooks marked
    `Sly 3 hook` / `Sly 3 script hook`: `world/sly2/mod.rs` (`program`, `configure_interpreter`) and
    `world/sly3/load.rs` (`first_module`, `module_of`).
  - The Sly 3 host's searches treat a placement (class 8) as its copy's root, as Sly 2's do (assumed for Sly 3;
    `script_host/sly3.rs`, `sly3_copy_root`).

Tests: `tests/sly3_worlds.rs` (skips without `SLYNX_DISC_SLY3`) loads all 39 worlds. It checks per-class counts, IDs,
records and world matrices against an independent walk of `formats-sly3`'s records. It also checks persist keys and
groupers, and runs 90 deterministic frames with and without scripts (the executable's option map). With scripts,
14,792 objects in the 39 worlds are attached to modules, and no world has a script error, the host's or any other
(before the placement rule, 810: three worlds setting option 133 on placements). No keyword, form or operation is
unknown.

### Sly 3 behaviours (`behavior::sly3`)

Sly 3 runs Sly 2's behaviour engine for its guards, sensors, props and pickups, so `behavior::sly3::attach` (called
from `Game::new` after the Sly 2 attach, one marked hook) gives Sly 3 objects the Sly 2 behaviours unchanged. Before
a behaviour is made, the options whose meaning carries over are copied from their Sly 3 IDs to the Sly 2 IDs the
behaviour reads (the Sly 2 IDs are cleared first; `sly3::aliases`):

| Sly 3 class | Sly 2 behaviour | Options (Sly 3 → Sly 2) |
|---|---|---|
| 58 guards and swarmers | `Sly2Guard` | 937 → 254 hit points, 265 → 233, 309 → 292 and 310 → 293 (ranges), 329 to 332 → 312 to 315 (sight), 344 → 327, 349 → 333 |
| 96 lasers | `Sly2Laser` | 508 → 455 |
| 15 detectors | `Sly2Detector` | 395 → 345 |
| 93 contact triggers | `Sly2Pad` | 459 → 413, 468 → 419 |
| 111 reactive props | `Sly2Prop` | 1125 → 1025, 1128 to 1143 → 1030 to 1045 |
| 21 trigger shapes | `Sly2Volume` | 478 → 443, 491 → 441; lists 480 → 432, 483 → 433, 489 → 440 |
| 103 coin, 104 task item, 105 medicine, 106 pocket prize | `Sly2Pickup` (classes 91 to 94) | 964 → 902, 970 → 907 |
| 97 barriers | `sly3::interactables::Barrier` (contacts) and Sly 2's `barrier_blocks_characters` | 513 → 458; lists 510 → 456, 512 → 457 |
| 101 water | `Sly2Water` (its box through a marked hook, `interactables::solid_box`) | 930 → 874 [I] |
| 139 noise templates | `sly3::devices::NoiseTemplate` (method 1189) | 1184 to 1188 → 1090 to 1094 |

Typed lists are kept by the Sly 3 loader as the level's actions (`interactables::lists` reads them); `attach` adds
them as values of the Sly 2 options (`sly3::list_aliases`).

Sly 3's own behaviours:

- **Bouncers** (94, `devices::Bouncer`): contact → state 1; after more than 0.05 s state 2 launches what touched
  it toward option 496's object or the local point (0, 0, 300), on the arc of `devices::ballistic` (α = θ + 0.75
  (π/2 − θ); v = (d / cos α) √(|g| / (2 (d tan α − h)))), then plays clip 497. Launches go to
  `World::shared.launches` (a marked field) for the game to apply: a character's jump kind 3
  (`gang::Character` velocity), a guard's own launch.
- **Safes** (80, `devices::Safe`) hold their tumblers (option 1610, through the tumblers' placements; at most 6).
  The combination (1606) fills the wanted positions from the last tumbler backwards. States 0 idle, 1 scrambling,
  2 in use (method 1605), 3 leaving; in use, stick left / right picks a dial (sound 54 past the ends), down / up
  turns it (state 0 / 1, waiting until it rests), Circle confirms (sound 62 when wrong), Triangle leaves; with 1608
  the safe opens as soon as the dials match. Methods 1611 and 1612 reset and snap. `devices::Tumbler` is the
  ten-position dial (states 1853 + position).
- **Use points** (33, `interactables::UsePoint`): radius 1163, object 1164, animations 1170; tracks the playable
  character within the radius.
- **Data only:** treasure maps (153: radius 1901, screen 1902, pace 1903, markers 1904, strings 1905) and the
  compass (125: needle 1894).
- Answers: detector option 390 links its spotlight gun; option 937 is the maximum hit points, 1,000 and above
  effectively unkillable (`UNKILLABLE_HIT_POINTS`); option 469 is probably a state-machine goal on trigger.

Marked hooks in the Sly 2 behaviours: the playable classes (52 to 54 on Sly 3 worlds, `character_classes`), the
pocket row (`pocket_row`), and `World::find_sly2_id` falling back to the Sly 3 IDs. Guards fill their pockets from
`World::shared.pockets`, which the game sets from `gang::sly3::pocket_table`.

Not attached: carry-use points 34, treasure stands 35, hiding spots 25 (shapes; the Sly 2 barrel's hit reaction does
not fit them), set pieces 37, hooks 38, and the marker factories 127.

**Combo definitions** (`gang::combo`): class 148 children of a character (938 under Sly). `ComboSet::from_world`
reads them (button 1659 as pad bits, priority 1667, window 1668 else 0.35 s, clips 1673 / 1676 / 1679, speed factors
1680 / 1681, abilities 1683, the rest raw); `Runner` is off, attack, attack while moving, follow-up after a hit,
recovery. With definitions set (`Character::combo_defs`), Sly's cane presses start or continue the runner, he is in
`sly::COMBO` (Sly 3's state 23), `ComboClip` names the clip and `Character::combo_hit` plays the follow-up.

Disc tests (`tests/behavior_sly3_disc.rs`): the per-class record and world counts of the specs over the 39 worlds
(class 93 occurs in 15 worlds against the spec's 14; the 24 records match), the attached behaviours and the guards'
hit points (40, 70, 100, and 1,000 and 100,000,000 on a few nested templates), 90 deterministic frames with
behaviours on every world, and the combo definitions, barriers, use points, trigger lists, safes and maps.

### Vehicles and minigames (`behavior::vehicles`)

Driven machines and minigames of Sly 2 and Sly 3 (specs/sly2/vehicles-and-minigames.md, specs/sly3/vehicles-and-minigames.md),
attached by `behavior::vehicles::attach` after the Sly 2 and Sly 3 attaches (one marked `Vehicles hook` in
`Game::new`) to objects with no behaviour yet:

| Module | Sly 2 | Sly 3 |
|---|---|---|
| `turret` | 69 | 68 |
| `hack` (ship, enemies; docks by class) | 72, 73 (75) | 74, 75 (77) |
| `car` | 77 combat car (108 ammunition) | 81 truck, 85 remote cars |
| `craft` | | 82 biplanes, 83 raft, 84 jolly boat, 87 swimmer, 88 ships |
| `dance` (scoring, director) | 121 | |
| `boss` (hit points 254/255, block 256 to 258, busy 231/1350) | 59 to 67 | |

**For the game owner and the Sly 3 behaviours owner:**
- A machine reads the pad only while `World::shared.driving` names it (new field, `Vehicles hook`). The sessions set
  it with `Sly2Session::drive(Some(class))` / `Sly3Session::drive(Some(class))` (the first active object of the
  class) and clear it with `drive(None)`. While it is set, the gang players (`game::sly2::gang`,
  `game::sly3::crew`, marked hooks) get an idle pad. Jobs that put the player in a machine should call `drive`.
- The camera does not follow the driven machine yet, and the turrets' camera (stack priority 9) is not taken.
- The Sly 3 safe (80) stays `behavior::sly3::devices::Safe`; the lock (79) has no rules in the spec; Sly 2's class-50
  machines (RC chopper, tank) are flown by their scripts.
- Working assumptions are marked [I] in each module (smoothing records and drags not in the spec, damage per hit,
  swim speed, ship throttle).

Disc test (`crates/game/tests/vehicles_disc.rs`): each machine type, driven through the session for 120 frames,
moves when active, deterministically. Active and moving: the truck (o_quarry), the raft (h_sewer), the swimmer
(p_dive). Held until their jobs start (inactive at load): the hack ships, turrets, biplanes and the jolly boat; no
class-88 ship is loaded in p_at_sea. **Open:** the combat car (row 33) and the remote car (c_ext) are active with
their behaviour attached but do not move yet; the test does not require it for those two.

Answers to apply after the release freeze (7.2.2, 7.4.2, 7.5.2): the biplanes' computer flight and shots, the
swimmer's strokes and states 3 to 5, the ships' per-state sailing records (1954 to 1961) and their throttle polar.

### Sly 2 clips and skinning (`anim::sly2`)

These cover points 4 to 6 of `docs/formats-sly2.md` ("What `engine::anim` would need for Sly 2"). Points 1 to 3 are
`anim::xform`.

- `ClipPlayer::start(&clip, time, speed, Wrap, &mut fired)` starts a playback. The caller chooses the wrap mode,
  since Sly 2 clips do not store one: `Once`, `Loop`, `PingPong` or `Hold`.
  - `advance(&clip, dt, &mut fired)` fires timed events as their frame is crossed, in either direction.
  - A type-5 event jumps to its frame at its speed. This gives the explicit loop of animation.md 7.
  - Starting fires the apply list, and `retract` fires the retract list.
  - Fired events are reported as `Fired::{Apply, Timed, Retract}`. No Sly 1 event meaning is applied.
- `SkinPalette::new(&mesh.bones, mesh_bind, |object, reference| bind_world)` gives one matrix per bone,
  `W_bone · W_bone,bind⁻¹ · W_mesh,bind`.
  - `matrices(&mesh_now, |bone| world_now, &mut out)` computes them each frame. A bone that does not resolve uses
    the mesh transform.
  - `blend(&matrices, &weights, vertex)` mixes a vertex's slots at `w / 255`.
  - The weights come from `formats_sly2::geometry::Group::weights(data, bones)` (`SkinWeights`).

## The game application (`crates/game`)

`crates/game` is SlyNX itself: platform, engine, renderer, sound and frontend.

- `game::session::Session<B, R>`: the disc, the running `Game`, the renderer, the sound and the frontend.
  `load_world(index, warp, wipe)` reads, parses and starts a world, uploads it and arrives at the warp.
  `update(&input, seconds)` runs a frame (the pause menu stops the game clock), updates the renderer (changed
  instances, faded meshes, bone matrices, fog, darkening, the UI layer), feeds the sound engine and handles the
  world's requests: another world through a wipe (by descriptor identity or by name and warp), the default exit, a
  respawn at the last checkpoint, game over, a movie. `start_title()` begins the start sequence from Splash (the
  start-up logo movie, then the title with the last slot loaded); `continue_save()` is Start with a save loaded.
  Loading is synchronous behind the black screen. Dialogue picture panels (`CameraExtras::panels`) are drawn with
  `Renderer::set_panels`, 4:3 rectangles 0.3 of the screen high centred at their screen position (the size is a
  placeholder); the iris wipe (kind 1) is described below; the other kinds fade. In worlds without Sly the free
  camera stands in for the player (the world's
  *probe*). `SessionOptions` turns off wipes, the frontend, the follow camera, and the probe's triggering and
  collecting.
- `game::level`: one renderer instance per drawn submesh of every drawable. Meshes, textures and materials come from
  the world's `formats::scene::Scene`; materials follow textures.md 4 (unlit). Submeshes of mesh sets with bones are
  added with their influences (`add_skinned_mesh`); each frame their instances get the drawable's bone matrices
  (`LevelGpu::skin`, from `RenderScene::skins`) and a culling sphere grown to cover the pose. `LevelGpu::fade` hides
  the meshes the camera fades.
- `game::frontend`: the HUD from the progress (with the auto-save icon), the pause menu with its `PauseContext`, the
  title menu and the slot lists (new game, load, save, erase), subtitles, fade wipes, lightning tints, letterbox bars
  and the movie card, drawn with the `ui` crate's draw lists through `game::ui_layer` (the renderer's `ui2d` layer).
- `game::saving` (docs/save.md): the progress in the `save` crate's slots. `Session::set_saves(files)` takes any
  `SlotFiles` (`save::SaveStore` over the platform's file system: the PC's user folder, `SLYNX_USER_DIR`, or
  `sdmc:/switch/SlyNX/user/` on the Switch). Each frame `Saves::track` brings the slot state up to date: new keys,
  vaults, sprint results (`LevelRecord::best_elapsed`) and bosses go through the save session's events, which decide
  the auto-saves; bottles, checkpoints, difficulty, lives, charms, coins and play time are copied. Loading a world
  other than the one last saved auto-saves. `saving::restore` turns a slot back into a `Progress`. The title loads
  the last slot used. The first auto-save of a new game in a slot (or of a loaded slot that never accepted it) waits
  for the auto-save notice: the session calls `Menus::before_autosave()`, which opens prompt 20, and writes the save
  when `Action::Answer { prompt: 20, .. }` arrives. Placed pickups, broken breakables, defeated guards and played
  dialogues are not in the slot format and last only while the game runs.
- `game::movie`: `Request::Movie(identity)` through the `video` crate. `Session::set_movie_source(opener)` takes a
  function that opens the disc image again, so a movie streams from its own reader. While a movie plays the world
  gets no frames and no input; + skips it; the game resumes when it ends. Without a source, a movie table, the
  movie, or with a read or decode error, the request is dropped. With a sound engine the soundtrack goes to an
  `audio::pcm` stream (resampled to 48 kHz by the player) and the frames follow its clock (frames mixed); without
  one, or once the sound has run out, the real clock. Each new frame goes to the renderer's video layer
  (`set_video_frame`, `VideoLayer::show` letterboxed with `VideoLayer::fit`), with black bars drawn around it.
- Rendering also: the player's projected shadow (`level::player_projector` from options 43 to 48 and the shadow
  shader's alpha, its origin moved with the player every frame; the player's instances are casters, receivers by
  the mesh flags 0x01 and 0x02); the iris wipe (kind 1) as `render::iris::IrisShape` from the world's screen mask
  (`game::iris`, the five triangle sets coloured by shaders 1166 to 1170; none is an opening), centred on the
  player's screen position and scaled from `2 × width / 70` to `4 / 70` pixels per mask unit by
  `iris::iris_placement(player, u, target)` for the closing amount `u` (`IrisShape::set(centre, scale, u)`); dialogue picture panels (`set_panels`; the panel size and placement from HUD elements 26
  and 27 are with the renderer owner).
- `game::sly2`: `Sly2Session` on the player's Sly 2 disc. `load_row(row)` loads a world with `World::load_sly2` at
  its level row, runs it in a `Game` with its scripts (the executable's option map and built-in table) and draws its
  `formats_sly2::scene` build as static instances (`sly2::level`). The gang (`sly2::gang::GangPlayer`, a
  `PlayerSystem`): Sly, Murray and Bentley start beside Sly's drawn position and move through the world's collision
  (`collision_from_world`: each active solid's collision parts from `formats-sly2`, the characters' own left out;
  the drawn geometry when a world has none); each member's position goes into its world object and `World::player`
  is the played one; the d-pad switches (left Sly, up Murray, right Bentley) until the safe house and job tasks
  offer it. The follow camera (`engine::camera` with the spec's constants) follows the played character. Guards are
  the gang's targets; the gang's punches, crossbow swings, air attacks, flops and darts become `Notice::Attack` on
  the guards they reach, a pickpocket `Notice::Pickpocket`; guard attacks become hits on the played character; what
  is taken from pockets, and coins spilled by knocked-out guards, go to the gang's `Purse`. The characters are drawn
  by `sly2::skin`: their rigid meshes (body type 0; types 1 and 2 hold only hulls or streamed data) as renderer
  meshes with their bone weights (`add_skinned_mesh`), posed every frame by `set_bones` from the palette
  (`engine::anim::sly2::SkinPalette`) of the character's current transforms; meshes without bones follow their
  transform. The scene build's static draws of the characters are left out. Each part's bones are resolved once
  when placed, and a part is posed again only when its character's matrix or visibility changed (without Sly 2
  clips the bones move only with the character): the Sly 2 smoke route runs at 0.29 to 0.73 ms per frame at the 95th
  percentile on the null backend (19 to 60 ms before, from a world search per bone per frame).
- `game::sly2::talk` (dialogue-and-voice.md): `Sly2Session::set_sound(engine, image)` gives the session a sound
  engine and the disc image again (`audio::sly2::Sly2Disc`); each loaded world's sound is installed
  (`WorldAudio::install`). `Sly2Session::start_sequence(h)` runs a sequence object's (class 114) dialogue commands in
  stream order, each waiting until done: play a line (`Talk::play`, voice and subtitle, quiet lines without),
  wait for the line, clear the subtitle, wait for a button (B, the gang's Cross); the other commands pass at once
  until their systems exist. `Sly2Session::subtitle()` gives the revealed text and speaker colour. What starts a
  sequence (a script, an event, another sequence) is not specified yet.
- `game::sly3` (first stage, new files plus the marked `Sly 3 hook` lines in `lib.rs` and `Cargo.toml`):
  `Sly3Session` on the player's Sly 3 disc. `load_slot(slot)` loads a world with `World::load_sly3` at its slot
  and runs it in a `Game` with its scripts (the executable's option map, specs/sly3/scripts.md).
  - It draws the `formats_sly3::scene` build statically with `sly2::level::upload_static`. Passes are mapped to
    the uploader's rule by `sly3::upload_pass`: 2 to 4 opaque, 9 blended, the rest not drawn yet.
  - It uses a fresh free camera above the middle of the drawn meshes. `Game::new` configures the first camera's
    lens from the world options by their Sly 1 meanings, which would be wrong for a Sly 3 world.
  - `with_sound(engine, audio_disc)` installs each world's bank and stream (`audio::sly3::WorldAudio`) and feeds the
    listener every frame.
  - `tests/disc_sly3.rs` checks that all 39 worlds load and render on the null backend, each one deterministic over
    60 frames, and that v_apt runs deterministically with sound.
  - Stage 2: the crew (`game::sly3::crew::CrewPlayer`, `engine::gang::sly3::Crew`).
    - **Arrival** (characters.md 2, level-flow.md 5): it matches the arrival id (`Sly3Session::arrival_id`, default
      424) to a checkpoint by OID. Checkpoints are class 40 and the classes derived from it
      (`sly3::checkpoint_classes`, from the class table). If no OID matches, a point answers by alias: option 843
      for 424, 844 for 425. Option 848 then places characters at markers. Without either it uses the world's first
      checkpoint; without checkpoints, the records' placements.
    - **Placement:** the current player is Sly, else Bentley, else Murray, placed at the checkpoint. The other core
      members stand beside it, where the spec removes them, so the d-pad can switch (left Sly, up Murray, right
      Bentley).
    - **Collision:** the in-world solids' `formats_sly3` collision parts, at their objects' matrices. Trees use
      the 32-bit `WideCollisionTree` (`TriMesh::new_wide`); three level objects' trees pass 65,535 nodes.
    - **Known gap (v_apt):** walking from the arrival point, Sly falls through the floor.
      - The drawn floor there (the level object, z −40 to −33) has no collision surface under it in any solid.
      - The level object's 1,356-triangle collision covers that ground only at z 1,236 to 1,458.
      - So this is missing collision, not a ledge. Where Sly 3's floor collision there comes from is a spec
        question.
    - **Camera:** the follow camera (`follow`, default on). With `follow` off, the free camera flies by itself
      (`FreeCamera::follow` false); before, it tracked the played character.
    - **Tests:** `tests/disc_sly3.rs` checks the arrival in v_ext (checkpoint 424) and v_apt (its first
      checkpoint), and walks the crew 4 s in each world with a jump and three switches, deterministically.
  - Stage 3: level flow (`game::sly3::flow`, level-flow.md) and the safe house (`game::sly3::safehouse`,
    safehouse-and-shop.md). It mirrors the Sly 2 job system's split: data read from the records, then run by the
    session.
    - **Episodes:** `first_worlds` reads the table at `0x2EDFB0`, and `episode_start` applies the exceptions:
      - China before its recon goes to `c_intro`;
      - the pirates between the treasure and the heist go to `p_at_sea`;
      - the menus before the opening go to `m_ext` with task 1798;
      - episode 8 starts task 1841.
      `Sly3Session::start_episode` loads the world at 424, or at the task's checkpoint (option 1400) when the
      world holds it.
    - **Exits** (class 24): `formats_sly3::stream::exit_body` reads the trigger volume, the destination and the
      movies. Options 834 (arrival point), 835 (wipe), 836 and 839 (delays) come from the data; the disc uses 834
      on 12 exits and 835 on all but 7.
      - `Flow` runs the walk-in states: 0 inside since the start, 1 outside, 3 armed, 4 fired, 5 load requested.
      - The game counts as paused in states 4 and 5.
      - `Flow::fire` (`Sly3Session::fire_exit`) is honoured only when armed and takes command `0x68`'s point.
      - The session performs the requested load at the top of the next frame and keeps it in `last_load`, with the
        movies' identities; movies are not played.
    - **Exit sources:** `exit_sources` lists job option 1381, task option 1406 and command `0x68` (exit, point)
      for the job and task systems and the command runner, which do not exist for Sly 3 yet.
    - **Restart:** `Sly3Session::restart` reloads the stored checkpoint with wipe 875. Its out/in pair comes from
      `WipeOrder`, a shuffled order of the three in which the pair just used never comes first.
    - **Safe house** (class 69):
      - The selectable states come from option 1518; on the disc the list is 25, 27, 24, 26.
      - Actions come from option 1517, on the object and its placement.
      - Left and right step through the states, wrapping. Cross fires the shown state's action.
      - The computer (27) opens the shop: the item table's records for this episode and earlier, not owned.
      - A character's state takes that character out. The take-out script (2061) needs the command runner, so the
        session switches to the character instead.
      - `p_at_sea`'s safe house is placed by a conditional placement (filter 32, condition 17) that the fresh load
        context leaves out.
    - **Guards' pockets:** each load sets `World::shared.pockets` from `gang::sly3::pocket_table` (the guard
      table) and `shared.episode` from the slot's episode, before `Game::new` starts the behaviours.
    - **Tests** (`tests/disc_sly3_flow.rs`):
      - the spec's per-world exits, 424 and 843 columns;
      - 81 exits, 25 jobs, tasks 1825 to 1840, and 63 command pairs, each naming a point of its destination;
      - each episode started and an exit taken to its point;
      - restarts with the three wipe pairs;
      - the safe house and the shop: Venice's 6 items for 1,700 coins, 34 for 23,700 in all;
      - the save tree.
    - **Spec differences found:**
      - exits do carry options 834 and 835 (now corrected in the spec);
      - option 1518 lists 26 as well;
      - 9 worlds hold one or two more arrival or warp points than the section 8 table (printed by the test).
  - For the game owner: the same `Game::new` lens rule applies to Sly 2 worlds (`Sly2Session` keeps the configured
    camera).
- `game::sound`: the audio frame on `audio::SoundEngine`: banks and music per world, the listener, ambient sounds,
  one-shots, music intensity and voice lines.
- `game::settings`: the disc location (Switch: `sdmc:/switch/SlyNX/sly1.iso`, the `disc` line of
  `sdmc:/switch/SlyNX/user/game.cfg`, or `romfs:/sly1.iso`), the first world (`world`; without it the Switch build
  starts at the title) and `triggers`.
- PC: `game::pc::registry()` registers the harness apps `sly1` (OpenGL, captures) and `sly1-cpu` (null renderer).
  Options: `world=NAME`, `disc=PATH`, `triggers=0`, `collect=0`, `overlay=1`, `camera=x,y,z,tx,ty,tz`, `wipes=0`,
  `hud=0`, `start=title`, `follow=0`.

```sh
cargo run --release -p game --example slynx_game -- run --app sly1 --opt world=Paris --frames 600 \
    --capture 599 --out DIR --check-determinism       # headless, deterministic, captures and perf
cargo run --release -p game --example game_pc -- [disc.iso] [world | --title]   # in a window
cargo run --release -p game --example world_bench -- [disc.iso] [world | --all] [frames]
crates/game/tools/build-switch.sh [--debug]          # dist/slynx.nro (see switch-build.md)
```

Sly is played with player 1's controls (keyboard or gamepad); + opens the pause menu and skips movies, − (or Esc on
the PC) quits. In worlds
without Sly the free camera flies (left stick, right stick to look) and stands in for him.

### Object model and loop tests

- Unit tests on synthetic levels: hierarchy and preorder; proxies, overrides and module attachment; lookups by scope
  and through proxies; options and flags; movers; templates and stale handles; the clocks; progress rules; pickups
  and bottles; exits, buttons, trigger volumes, checkpoints and state machines; guards (detection, groups, attacks,
  stun and knock-out); alarms and each sensor kind; breakables and rewards; hazards, bombs and blasts, conveyors,
  baskets; dialogues; emitters, explosions, lightning and darkening; the vault; notices across frames; pause and the
  frame-step clamp; determinism; the audio frame; lens and fog.
- Disc tests (`tests/world_disc.rs`, `SLYNX_DISC_SLY1`): transforms.md 8; for all 46 worlds, one object per placement
  of the stream for every class checked, the 31 stock templates and one world mesh, and the disc totals of clue
  bottles (595), coins (950), charms (43), life tokens (55), keys (28), hourglasses (22), buttons (232), trigger
  volumes (461 and 87), volumes (98), breakables (1,728), pieces (1,609), exits (114) and checkpoints (51); every
  world's scripts, player, physics and loop run 90 frames, twice, with identical state hashes; no world refuses an
  outer module, and no write or read of options 127, 205, 844, 848, 880 or 1084 fails.
- `crates/game/tests/disc.rs`: every world uploads and runs on the null renderer; Paris runs deterministically for 240
  frames; a default-exit request moves from Splash to Paris through the wipe with the progress carried over; a
  movie plays, holds the world, decodes frames on time and is skipped (an unknown movie, or no movie source, is
  dropped); a new game in a slot auto-saves on loading Paris and on a key, and a fresh session's title continues
  it with the key and coins. `crates/game/tests/disc_sly2.rs` (`SLYNX_DISC_SLY2`): the Sly 2 hub loads, draws and
  runs 120 frames deterministically, and every Sly 2 world loads on the null renderer.

### Performance

Engine update per frame (`world_bench`, release, PC, with the player, physics and collision, guards, sensors and
effects; 600 to 1,200 fixed frames): Paris 0.039 ms mean and 0.065 ms at the 99th percentile (2,252 objects, 252
ticking); the busiest world, A Rocky Start, 0.155 ms mean and 0.31 ms at the 99th percentile (3,673 objects, 368
ticking); Boneyard Casino 0.086 ms; most worlds 0.02 to 0.06 ms. Starting a world (objects, behaviours, scripts,
collision and physics bodies, post-load) takes 0.2 to 33 ms, 75 ms for The Swamp's Dark Center. The harness's
`sly1-cpu` run of Paris (600 frames): update 0.069 ms, renderer build 0.049 ms, whole frame 0.15 ms mean, and a
second run matches.

### Open questions

1. **Actions without a meaning.** Script action calls no behaviour implements have no effect and are counted in
   `World::unhandled_actions` (not script errors). On the disc, by option (calls in two 90-frame runs of all 46
   worlds): 4 (264; classes 2, 3, 54, 99), 9 (104; one argument; classes 2, 3, 33, 46, 47, 56, 59, 89), 90 (2), 117
   (2), 118 (192; class 2, one argument), 126 (12; class 3), 228 (6; breakables), 282 (4; class 20), 606 (2; class
   78), 613 (54; hazards), 663 (6; flashlight guards, one argument), 722 (180; class 62), 738 (2; the player), 888
   (10; class 71), 913 / 914 (58 / 308; class 140), 1087 / 1088 (60 / 30; vehicles), 1156 (56; class 19). Implemented
   from the specs: 311 / 312 and 315 / 316 (lasers on and off, beams extended and retracted over the given time),
   362 to 364 (emitters paused for a time, forever, resumed), 657 (a guard state's animation).
2. **Options read as nothing.** Script reads of options with no encoding that nothing set return `()` (the reads
   with a meaning are live: 28 to 39, 188 a segment's duration, 547 the world's adaptive difficulty, 981). On the
   disc: world 547 is now live; still `()`: 916 (class 140), 1103 and 7 (class 123), 514 (state machines), 624
   (bottles), 686 / 688 (proximity guards), 1167 (class 13), 176 (class 126), 185 / 186 (segments), 229
   (breakables), 8, 266, 283, 12, 11, 152, 151, 1078, 6, and world options 556, 563, 555, 557. Their kinds and
   unset values are needed.
3. **Remaining script errors** (not host errors): with scripts.md 11, items 15 to 18 applied (wrong kinds of
   arguments read as 0, zero vectors or the identity; calls of nothing are no-ops), the smoke route has none (it
   had 60, mostly `NotCallable` and operations 7, 115, 101, 2 and 0 rejecting `()`), and the 90-frame runs of all
   46 worlds have 2, both `TooDeep` in Murray's Big Gamble (a handler that recurses until the game's state
   changes).
4. **Saves.** The dialogue flags and script values of a slot are not filled (their mapping to dialogue anchors is
   not described). Start skipping a movie is our choice.
5. **Placeholders** left (marked in the code): the clue bottles' touch radius without option 133. Replaced from the
   spec answers: the walkers' turning (physics.md 5.1: F = 25·r, T = 0.25/√r), the emitters' distance (the
   drawable's maximum draw distance × max(1, k / field of view), unparented emitters always, groups as one),
   hazards (no reach, no repeat timer: the player's immunity), sensors (beams against the player's mesh,
   searchlights the origin, plates real contacts; the probe as a sphere of 60 centred 15 up), the vault (250 from
   its marker, OID 892), the attacks (the four effect shapes and the aiming rule, player-moves.md 6), pickups (the
   collect-radius sphere against the player's shape).
6. **Bomb actions.** Script actions on bombs (detonate, light) are not described; bombs go off from hits and blasts.
7. **Sly 2 and Sly 3 world options.** Their lens and fog stay at the defaults: the Sly 1 option IDs are applied only
   to Sly 1 worlds (`World::sly1_meanings`) in `Game::new`, the free camera and the follow camera.
