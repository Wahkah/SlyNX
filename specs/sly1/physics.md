---
id: sly1-physics
title: Sly 1 physics and contact resolution
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 worlds
    used_for: which solids are fixed; per-class option usage (gravity, mass, buoyancy, springs) from options-by-class.md
  - kind: executable
    what: SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      the terminal-fall rule (the code that reads 0x263E78 and 0x263E7C adds its term to the accumulated
      acceleration, which already holds gravity); water damping coefficients, walker leg smoothing, turn slowdown,
      default smoothing records, read at the addresses given and confirmed to be referenced by code
  - kind: observation
    what: the game as played
    used_for: the overall feel (heavy, quick falls; objects come to rest; breakables scatter)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files update, solve, so, step, jt, jump, sw, alo, act, bez, util, water, mark, coin, actbank, spaprops
    used_for: >-
      gravity values, frame order of the solver passes and their thresholds, restitution, rest detection, water
      damping, smoothing law, drive modes, the walker leg; [R]
date: 2026-11-14
---

# Physics and contact resolution

Tags: [D] disc data, [X] read from the executable at the given virtual address (file offset = address − 0x100000 +
0x1000; the port checks the executable's SHA-256 in disc.md first and refuses on a mismatch), [O] observable, [R]
reference behaviour to confirm, [I] inferred. Units are game units
(about 1 cm) and seconds; Z is up (transforms.md 7). Collision geometry, materials and the floor/wall rule are in
collision.md.

## 1. What moves

- **Fixed solids** (physics flag bit 0; only the static world mesh on the disc) never move and are only collided
  against. [D]
- **Solid objects** (the solid layout) have mass, a moment of inertia and a centre of mass (classes/solid-object.md),
  and move under velocity, angular velocity, gravity, contacts, springs and animation. [D] [R]
- **Placed objects without collision** follow their parents, animation and scripts only. [R]
- A solid's children that are solids move with it as one rigid group (the "root" of the group takes part in the
  solver). [R]

## 2. Gravity

| Who | Acceleration (z) | Source |
|---|---|---|
| world default | −1500 units/s² | [R] |
| every solid, at creation | the world value, unless its option 129 sets its own vector, or option 138 turns gravity off | [R] [D for option use] |
| the player character | −1750 units/s², then multiplied by a gravity ratio of 1.3: **−2275 units/s²** | [R] |
| dropped pickups | −2250 units/s² | [R] |

**Gravity ratio** (actors): changing the ratio r scales the actor's gravity vector by r_new / r_old and its mass by
r_old / r_new, so heavier gravity comes with lighter mass and the contact forces stay balanced. [R] Several move
speeds scale with √r (player-moves.md). Option 711 sets the ratio on an actor. [R]

**Terminal fall** (player only, in the jump and fall states except while thrown): when the vertical velocity v_z is
below v_t = −750·√r = **−855.1 units/s** (r = 1.3), an extra upward acceleration of 5·(v_t − v_z) is added to the
frame's acceleration. Gravity stays in that sum: the term is added on top of it, not instead of it. [X] (the code
that reads −750 at 0x263E78 and 5 at 0x263E7C adds its term to the accumulated acceleration of the frame, which
already holds gravity) [R]

So −855.1 is the speed at which the drag starts, not the speed the fall settles at. Below v_t the vertical
acceleration is g + 5·(v_t − v_z), which is zero at

  v_∞ = v_t + g / 5 = −855.1 − 455.0 = **−1310.1 units/s**.

Between the two the speed relaxes toward v_∞ with a time constant of 0.2 s:
v_z(τ) = v_∞ + (v_t − v_∞)·e^(−5τ), τ measured from the moment v_z passes v_t. The literal reading of the formula
is the right one; a rule that drops gravity below v_t (and settles at −855.1) does not match the game.

**Water** (option 134 buoyancy, default 0.5 for actors [X 0x274B20]; option 135 viscosity, default 1.0): each value
is the solid's own value multiplied by the water object's (both default 1.0 on the water [R]). For a submerged
fraction f (0 to 1) of the solid, per second: [R]

- **Buoyancy**: an acceleration of −buoyancy × f × gravity vector (upward), applied only if the solid has gravity.
  It also turns the solid upright: an angular acceleration of 3 × buoyancy × f × θ about the axis that brings the
  solid's up axis to its rest orientation tilted onto the water surface, θ being the angle between the two.
- **Viscosity** damps the velocity relative to the water's current. Split the relative velocity Δv = v_current − v
  into its part along the water surface normal n and its tangential part. Each part pulls the velocity toward the
  current at a rate (per second) of
  - normal part: 3 × viscosity × s_n, where s_n = 1, except that s_n = 0.5 + 0.5 f when the solid is only partly
    under (f < 1) and is moving up out of the water relative to the current (Δv·n ≤ 0);
  - tangential part: 3 × viscosity × f.

  So a ← a + rate × Δv_part. The angular velocity is damped toward zero the same way with rate 0.75 × viscosity ×
  f about the surface normal and 0.75 × viscosity about the axes in the surface. Each rate is capped at 1/dt so
  one frame never reverses the motion. The coefficients 3 and 0.75 are stored as (0, 3, 0) at **0x26A850** and
  (0, 0.75, 0) at **0x26A860** [X]: three `f32` per record, read as a quadratic in |Δv| (c₀/|Δv| + c₁ + c₂|Δv|) of
  which only the linear term is non-zero.

For the player, deep water is a hazard instead (player-moves.md 7). [O]

## 3. One physics step

### 3.1 Integration

All solids and placed objects with a velocity use exact constant-acceleration integration over the frame step dt:
the frame's acceleration a (gravity, water, springs, the player's own terms) is summed once at the start of the
frame from the state at that moment, then [R]

  x ← x + v·dt + ½·a·dt², v ← v + a·dt.

Rotation: ω ← ω + α·dt, then the orientation turns by the average (ω_old + ω_new)·dt/2 and is re-orthonormalised.
A crate dropped from rest therefore falls exactly 750 units in 1 s whatever the frame rate; integrating velocity
first (762.5 units at 60 frames/s) is not what the game does.

### 3.2 Frame order

Per game frame (game clock running), after objects and animations have updated (clock-and-loop.md 4): [R]

1. Remember every moving solid's position (the previous position).
2. **Contacts from the start-of-frame positions.** Contacts are real touches only: pairs of collision meshes that
   touch or interpenetrate at the positions the frame starts with. There are no speculative contacts. A resting
   object is normally embedded a little (between 0 and 2 units, see step 5), which is what keeps its contacts alive
   from frame to frame.
3. **Integrate and solve pivots.** For each contact group that is not at rest: integrate as in 3.1 (gravity
   included), recompute the contact points and the relative velocities at the new positions, then solve the
   **pivot pass** at those contacts: the target change of normal relative velocity is the full approach speed, so
   an approaching contact ends with zero normal speed (never pulled together), with friction. Then each body's
   pivot points are updated.
4. **Contacts again** at the moved positions. This is where a falling object first meets the ground: it was not
   touching at the start of the frame, so the pivot pass ignored it, and now it is embedded by up to |v|·dt.
5. **Penetration**: if any contact of a group is more than **2 units** deep, the group's contacts are pushed apart
   by **depth − 1** each, leaving every corrected contact 1 unit deep [R] (for an equality constraint, if more than 0.1
   off, by the full error). The earlier reading "depth + 1" was wrong in sign: the push never separates the bodies
   completely, so the contact survives to the next frame as a pivot contact. Up to 2 units of overlap are left
   alone.
6. **Impacts**: if any contact of a group approaches faster than **5 units/s** (equality constraints: faster than 5
   in either direction), every contact of the group gets an impulse whose normal velocity change is
   approach × (1 + e × k), k drawn per contact from [0.9, 1.1], e the elasticity of the two materials (collision.md
   6); the separation speed is e × k × approach. Friction applies. The approach speed used is the one after this
   frame's integration (gravity included) and after the pivot pass, so a body at rest on its pivots shows no
   approach and never triggers an impact, while a newly touching body does.
7. **Rest**: an object whose velocity and angular velocity are zero, and whose children's are too, counts as static
   after two consecutive frames and is no longer integrated or solved until something moves it.
8. **Impact reactions**: each solid with a recorded impact reacts (damage, breaking, sounds). Then touch events
   (scripts.md 6, events 6 and 7) are raised for solids that asked for them.

Answers to the classification question: contacts are classified after this frame's acceleration, not before it;
there are no speculative contacts; impacts happen only at real (touching or embedded) contacts. A port that uses
speculative contacts for robustness must still reproduce the visible result: a landing object ends its landing
frame up to 2 units embedded (or 1 unit after a push-out) and bounces with e × k of its approach.

The constraint solver itself (an iterative solve over all contacts of a group with friction cones) belongs to a
follow-up spec.

## 4. Restoring springs, smoothing and drive modes

### 4.1 Options

Objects can be pulled back toward their original position and rotation, or eased toward animation goals: [R]

| Option | Meaning | On the disc [D] |
|---|---|---|
| 54, 60 | restore position, restore rotation (on/off; on means drive mode 1) | 121, 133 uses, value 1 |
| 55, 61 | drive mode used to restore position, rotation | 747, 831 uses, always 3 |
| 56, 58 | position spring strength, damping (scale factors of the defaults) | yes |
| 57 | position spring strength as three coefficients of distance | 4 uses |
| 63, 65 | rotation spring strength, damping | yes |
| 66 | rotation damping as three coefficients | 2 uses |
| 67, 69 | position, rotation smoothing **scale** r (see 4.3) | 465, 782 uses; 0.25 to 3 |
| 68, 70 | position, rotation smoothing as a full four-value record | never |
| 71, 72 | position, rotation rate-of-change limit as a ratio (4.3) | never |
| 73, 74 | default drive mode for animated position, rotation | 41 uses (1, 3), 38 uses (3) |
| 76 | drive mode of the object's look-at turning (rotation only; 3 there means "smooth, never lock") | 73 uses (1, 2, 3) |
| 62 | priority for those drives | 6 uses |

### 4.2 Springs

A spring with strength k and damping c applies an acceleration k·(goal − x) + c·(v_goal − v) each frame (c is
negative for damping in the camera's notation, positive as a rate in the object options); a strength or damping
given as three coefficients (c₀, c₁, c₂) of the distance d means a factor c₀/d + c₁ + c₂·d. The damping factor is
capped at 1/dt. The spring's acceleration is added to the frame's other accelerations and integrated as in 3.1.
The camera's own springs are integrated velocity first, then position (camera.md 4). [R]

### 4.3 Smoothing

**Three-value smoothing** (fast speed F, slow speed S, fast time T; "smoothing parameters" of scripts.md 2): the
speed toward the target depends only on the remaining distance d:

  speed(d) = F when d ≥ (F + S)·T / 2, otherwise speed(d) = √(S² + 2·((F − S)/T)·d),

so over the final stretch the speed falls linearly in time from F to S and the value arrives at speed S. Within a
step the motion is integrated exactly along this profile (constant speed F, then the constant deceleration
(F − S)/T), and the value never overshoots. The law has **no memory**: if the target moves, the approach restarts
from the new remaining distance, not from the current speed. [R] (An earlier sentence here said "from the current
speed"; that was wrong for the three-value form.)

**Four-value smoothing** (F, S, T, A): the same law, plus a limit A on how fast the speed may change (units/s²,
rad/s² for angles). The current rate of change is kept between frames. If the three-value law asks for a new speed
more than A·dt away from the current one, the speed changes by exactly ±A·dt instead and the value moves with that
constant acceleration over the step (x ← x + v·dt ± ½·A·dt²). The camera's field-of-view smoothing (0.5, 0, 0.1, 10)
[X 0x2618B8] therefore accelerates its change of field of view by at most 10 rad/s². [R]

**Options 67 and 69** scale default records [R] [X for the defaults]:

| Option | Fast F | Slow S | Fast time T | Rate limit A | Default record |
|---|---|---|---|---|---|
| 67 (position) value r | 1500·r units/s | 0 | 0.5 s | 4500·r units/s² | (1500, 0, 0.5, 4500) at **0x260EB0** |
| 69 (rotation) value r | 15·r rad/s | 0 | 0.5 s | 45·r rad/s² | (15, 0, 0.5, 45) at **0x260EC0** |

Options 71 and 72 (not used on the disc) would set A = value × (F − S) / T.

**Pose weights.** When an object switches to a pose set (a named combination of pose weights), each pose weight
eases toward its new value with three-value smoothing (3, 1, 0.1) in weight units per second [X 0x260E60] [R]: 3 per
second, slowing over the last 0.2 to 1 per second. Weights driven directly by an animation segment's pose channels
are set, not eased.

### 4.4 Drive modes

Codes of options 55, 61, 73, 74 and 76 (only 1, 2 and 3 occur on the disc; the others are set by the game at run
time) [R]:

| Code | Name here | Behaviour |
|---:|---|---|
| 0 | none | the drive does not move the object |
| 1 | spring | 4.2 toward the goal (position and velocity goals) |
| 2 | follow | position (rotation) and velocity (angular velocity) are set to the goal's every frame: no easing. This is also the default for an animated object that is not free to move under physics |
| 3 | smooth | 4.3 three-value smoothing along the straight line to the goal, with the goal's own speed toward the object added to F and S, and the goal's sideways velocity kept; on arrival the drive becomes code 7 |
| 4 | curve | a cubic Hermite curve from the current position and velocity to the goal position and velocity, timed to arrive at the goal's match time; after that time the position is the goal's plus its velocity × the overshoot. Used by the player's aimed jumps |
| 5 | physics | the drive leaves the object to the physics step (3.1): it moves with its own velocity and accelerations, which the object's behaviour sets |
| 6 | smooth, forced | like 3, but each frame it covers at least distance × dt / (time left to the match time + dt), so it arrives in time |
| 7 | locked | like 2 (what 3 and 6 become on arrival) |
| 8 | spring, locked | the drive does not move the object |
| 9 | smooth, never lock | like 3 without switching to 7 |

The default drive for an animated object with no option: smooth (3) if the object is a solid free to move,
otherwise follow (2). [R]

## 5. The player and contacts

- The player's friction against a pivot is limited to m · |g| · dt · (a factor), and against an impact to
  m · |g| · (a factor) (factors in the player code, open question 2). [R]
- A contact counts as ground when its normal's z exceeds 0.7 (collision.md 7). [R]

### 5.1 Walking characters (the leg)

The player and the other walking characters are physics bodies (their collision mesh is a convex shape) plus one
extra **base point**: a point **h** units below the lowest point of the collision mesh, where h is option 709
(default 35; 0 to 80 on the disc; the player uses the default). [D] [R] For the player the lowest point is 55
below his origin, so standing he floats with his origin 90 units above the ground contact (player-moves.md 1.1).
It acts like a springy leg:

- **Ground contact**: a contact of the base point whose normal has z ≥ 0.7. That contact has elasticity 0 and
  friction 1.5 [X 0x274B74]. Other contacts of a walking character have elasticity 0 and friction 0.
- **Step height**: because the body rests on the base point, the body itself floats h units above the ground. An
  obstacle up to about h high is met by the leg, not the body, and the leg compresses over it: the character steps
  up without stopping. There is no separate step-up rule. Taller obstacles meet the body and block it.
- **Turning** (guards and other walkers; the player has his own rule, player-moves.md 2): the facing turns toward
  the target heading. With F = 25·r rad/s and T = 0.25/√r s [X (25, 0, 0.25) at 0x274B80], r the spin ratio
  (option 710, default 1): the wanted turning speed is F·min(1, angle/θ_s), θ_s = F·T/2 = 3.125·√r rad (linear in
  the remaining angle inside θ_s, using the angle predicted half a frame ahead); the turning speed moves toward it
  with an angular acceleration of at most F/T = 100·r^1.5 rad/s² (reaching it within the frame when closer than
  that); a speed above F is damped at a rate of 2.5 per second. Example, r = 1: a 90° turn from rest accelerates at
  100 rad/s² toward 25·(1.571/3.125) = 12.6 rad/s and is within 5° of the target after about 0.41 s. [R]
- **Leg length**: at a ground contact, the leg's length is driven by two smoothings: it extends back toward full
  length with (3000·r, 0, 2.0) [X 0x274B40] and compresses toward zero length with (3000·r, 0, 0.25) [X 0x274B50]
  (r the gravity ratio); the leg's speed is the larger of the contact's own approach and the compressing speed,
  then moved toward the extending speed by at most dt·(3000·r)/0.25 per frame. If the leg would be more than 2
  units into the ground, it is shortened at once. (A leg that cannot reach is not a contact.)
- **Snap**: there is none. If the ground falls away by more than the leg can reach, there is no base contact and
  the character falls (jump state, falling).
- **Walking velocity**: the character's target velocity (from its run speed and heading) is projected onto the
  ground contact plane (the part along the normal removed) and given to the contact as the velocity the ground
  should impart, scaled by a turn factor u_run = clamp(1 − |Δθ|, 0, 1), Δθ the angle in radians between the
  character's facing and its target heading [X (1, −1, 0) at 0x274B30]. So on a slope the character keeps the
  same speed along the ground plane apart from the projection (cos of the slope angle along the motion), and it
  slows while turning.
- **Slopes**: ground steeper than 45.6° (normal z < 0.7) is not ground: no leg, the body slides on it with friction
  0 and falls.

## 6. Test vectors

| Case | Result |
|---|---|
| a crate (world gravity) dropped from rest falls for 1 s | falls 750 units, speed 1500 units/s, at any frame rate (3.1) |
| the player falling from rest (continuous) | z(t) = −1137.5 t² until t = 0.3759 s (−855.1 units/s, 160.7 units down); then v → −1310.1 with time constant 0.2 s: −1065.5 units/s and 281.2 units down at 0.5 s; −1290.1 units/s and 891.4 units at 1.0 s; −1310.0 at 2.0 s; 99 % of the way from −855.1 to −1310.1 at 1.30 s |
| the same at 60 frames/s (acceleration taken at the frame start) | after 61 frames (1.0167 s): z −916.0, v −1294.1 |
| a pickup tossed upward at 500 units/s | apex after 0.222 s, 55.6 units up |
| three-value smoothing (1500, 0, 0.5) from d = 1000 | the final stretch is 375 units; 625 units at 1500 units/s (0.4167 s), then 0.5 s of deceleration; arrives after 0.9167 s at speed 0 |
| water: a crate at rest, fully submerged (f = 1), buoyancy 0.5, viscosity 1, world gravity | upward acceleration 750 units/s² from buoyancy; net −750; the velocity is damped at 3 per second toward the current |

## 7. Open questions

1. The solver's iteration counts and friction model in detail (follow-up spec).
2. The player's friction-limit factors (constants in the executable; their addresses are not yet located).
3. The exact rotation path of drive mode 3 (the reference behaviour is clear for position; for rotation it eases
   the angle of the rotation that takes the current orientation to the goal, [I]).
