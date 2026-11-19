---
id: sly1-behaviour-effects
title: Sly 1 visual effects - particles, explosions, flashes, lights, sky, shadows, darkening
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files; executable SCUS_971.98 (SHA-256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c)
    used_for: >-
      every emitter (1,321), explosion object (2,687), light (2,254), sky (40), flash volume (65), lightning flash
      (77) and dynamic shadow (41) walked, including proxy-placed ones; option statistics per class; emitter
      curves, origin meshes and colour lists; darkening events; every test vector below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/emitter.cpp, emitter.h, explo.cpp, expls.cpp, explg.cpp, flash.cpp, thndflash.cpp, light.cpp, sky.cpp, shadow.cpp, dysh.cpp, update.cpp, util.cpp, spaprops.cpp (option table entries 41 to 51, 348 to 504, 540, 541, 608, 1132 to 1139)
    used_for: >-
      meaning of the options and their defaults, the emission modes, particle motion, the lighting terms, the
      darkening and scale smoothing rates; behaviour marked [R]
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 visual effects

Tags: **[D]** disc data, **[O]** observable, **[R]** reference behaviour to be confirmed, **[I]** inferred. Classes,
options and layouts are those of [object-stream.md](object-stream.md), [option-encodings.md](option-encodings.md),
[classes/placed-object.md](classes/placed-object.md) and [classes/shared-structures.md](classes/shared-structures.md).

Notation: a **range** option (encoding `p`) is two `f32` (minimum, maximum); a value "drawn from a range" is uniform
between them. A **quadratic curve** option (three `f32` plus one unused `f32`, encoding 16 bytes) gives a value
`g0 + g1·u + g2·u²` over a particle's normalised age `u` (0 at birth, 1 at death). [R]

## 1. Particle emitters (class 80)

1,321 emitters in 45 worlds. [D]

### 1.1 Stream data

After the placed part [D]:

- **Origin curve**: absent (curve kind −1) in 1,226 emitters; a polyline (kind 0) in 7; a cubic curve (kind 1)
  in 88. Particles start along it when the origin kind is 2. [D] [R]
- **Origin mesh** when the object's own origin kind (option 366) is 4: points, triangles (three `u16` corners and a
  `f32` area each) and a centre point. Particles start on the triangles, chosen in proportion to their area; each
  point's normal is the normalised sum of the normals of the triangles that use it. 86 emitters (4 to 101 points,
  2 to 104 triangles over all emitters and explosion emitters). [D] [R]
- **Colour list** (up to 32 kept): `u8` "ranges" flag, then packed RGBA colours (R, G, B, A bytes; A on the
  0x80 = opaque scale). 863 emitters have one or more colours (most 1 or 2, up to 12); the flag is set in 19
  lists. Each sprite takes a colour from the list (with the flag: a random blend between neighbouring entries). [D]
  [R]

### 1.2 Emission

| Option | Meaning | Default [R] | On the disc [D] |
|---:|---|---|---|
| 351 | Emission rate range, particles per second (re-drawn every 1 s) | 10, 10 | 1,314 emitters; (30, 30) ×153, (20, 20) ×144, (80, 80) ×104 |
| 352 | Constant population: keep this many live particles | 0 | 9 (400 or 700) |
| 353 | Pause probability per burst | 0 | 10 (0.75 or 1.0) |
| 354 | Pause length range, s | — | 10 |
| 350 | Emission mode (below) | chosen automatically | 6, all mode 2 |
| 349 | Pause forever after the first burst | off | 2 |
| 348 | Total particle budget (−1 = unlimited) | −1 | — |
| 355 | Enabled | on | — |
| 356 | Rate is per unit of box volume (box origins) | off | 2 |
| 361 | Group: emitters sharing a group OID emit together as one | none | 39 |
| 362, 363, 364 | Script calls: pause for a time, pause forever, resume | | |

Modes [R]:

- **Continuous** (default when 352, 353 and 349 are unset): emits `rate × dt` particles per frame, carrying the
  fraction.
- **Constant population** (default when 352 is set): tops the live count up to option 352.
- **Burst** (mode 2): once per 1/60 s tick, emits `rate` particles at once, then with probability 353 pauses for a
  time drawn from 354, and draws a new rate from 351.
- **Legacy burst** (default when 353 or 349 is set): like continuous, but at a 60× rate and with the pause rule.

An emitter attached to a parent emits only while the camera is within its maximum draw distance (drawable block,
[geometry.md](geometry.md) 1.1) times the camera's draw-distance factor max(1, k / field of view), k the world's
camera draw-distance ratio (1 unless a script changes it) — so normally exactly the drawable's own distance, the
same distance at which it is drawn. The test is the straight distance from the emitter's world position to the
camera eye, checked every frame. An emitter with no parent always emits. Emitters in a group (option 361) emit
when any member qualifies (a member with no parent always does). [R]

### 1.3 Where particles start (origin)

| Option | Meaning | On the disc [D] |
|---:|---|---|
| 366 | Origin kind: 0 point, 1 box, 2 along the origin curve, 3 on a skeleton (option 412 adds bones), 4 on the origin mesh | point 720 (default), box 420, curve 95, mesh 86 |
| 367, 368, 369 | Origin point; box corner 1 (368) and box corner 2 (369) | 420 boxes; e.g. (−5, −6.498, −7.014) to (−3.286, 6.498, 7.021) on 172 |
| 371 | Direction kind: 0 along a fixed vector (372), 1 radial from the origin, 2 curve tangent, 3 curve normal, 4 toward the screen, 5 mesh normal | 0 ×144, 5 ×47, 1 ×41 |
| 372 | Fixed direction vector | (1, 0, 0) default; 540 set, (1, 0, 0) ×361, (0, 0, −1) ×94 |
| 375 | Start offset range along the direction | — |
| 376 | Which frame velocities are in: 0 relative to the emitter, 1 relative with spin, 2 not relative (world after launch), 3 world | 2 ×604 |
| 357, 370 | Reference object and shape object OIDs | 19, — |

### 1.4 Motion

Each particle gets a speed drawn from 377 along its direction, tilted away from it by an angle drawn from 384 and
spread around it in rings of option 385 particles (default 200), with a random share 386 (default 1). It then
moves under constant acceleration 379 and linear damping 380: `dv/dt = a + damping × v`, so a negative damping
slows it at that rate per second; with curl 381 the velocity also turns about axis 382 at that many radians per
second. [R]

| Option | Meaning | Default [R] | On the disc [D] |
|---:|---|---|---|
| 377 | Speed range | 750, 750 | all 1,321; (0, 200) ×123, (30, 50) ×103 |
| 378 | Vertical speed factor | 1 | 13 |
| 379 | Acceleration (gravity) | (0, 0, −1500) | 1,291; (0, 0, 0) ×357, (0, 0, 100) ×109, (0, 0, 200) ×101 |
| 380 | Damping, per second | −1 | 1,302; −1 ×312, −0.4 ×272, −2 ×212 (range −7.4 to 3) |
| 381 | Curl, radians per second | 0 | 317 (2 to 6) |
| 384 | Tilt range, radians | 0, 0 | 1,294 |
| 387 | Lifetime, s | 1 | 1,284; 0.1 to 10, 1.1 ×226 |
| 389 | Birth delay range, s | 0, 0 | 383 |
| 388 | Skip range | 0, 0 | 18 |

### 1.5 Drawing

Most particles are camera-facing sprites. [R] [D for counts]

| Option | Meaning | Default [R] | On the disc [D] |
|---:|---|---|---|
| 398 | Shader OID ([textures.md](textures.md)) | — | all 1,321 |
| 399 | Size curve over age | (1, 0, 0) | 1,280; (83, 7, 0) ×95, (5, 10, 0) ×95 |
| 390 | Alpha curve over age | (1, 0, −1) | 1,313; (1, 0, −1) ×114, (0, 2.5, −3) ×89 |
| 401 | Colour curve | — | 10 |
| 400 | Texture-frame curve | — | none |
| 403 | Seconds per cycle through the shader's animation frames | — | 854; 0.7 ×187, 1.0 ×145 |
| 404, 411 | Random first frame; random roll | off | 14, 14 |
| 405 | Motion style (2 in all 12 uses) | — | 12 |
| 407 | Orientation style: 1 or 2 | — | 158 |
| 408 | Stretch factor while flying | — | 101 (1, 3, 6, 11) |
| 410 | Spin range, radians per second | — | 57 |
| 391, 392 | Particle type 1 = small **objects** instead of sprites, of the given object type | sprites | 39 |

Alpha and size are clamped at 0. [I]

## 2. Explosions (classes 120, 121, 122)

- An **explosion group** (class 120, 267 on the disc) holds a list of explosion objects. Setting it off sets off
  every member. [D for counts] [R]
- An **explosion emitter** (class 122, 2,407; and class 121, 13) emits one burst of particles when set off, using
  the explosion option set (421 to 476, the counterparts of 366 to 411) and these: [R] [D]

| Option | Meaning | On the disc [D] |
|---:|---|---|
| 440 | Particle count range (count drawn per detonation) | 2,396; (1, 1) ×327, (10, 10) ×209, (15, 15) ×158 |
| 444 | Delay before it fires, s | 4: 0.1, 0.2, 1.0 |
| 445 | Grouped | 308 |
| 451 | Lifetime, s | all; 0.1 to 8 |
| 433 | Speed range | all; (400, 550) ×232 |
| 435 | Tilt range | 2,215; (0, 3.14) ×592 |
| 454 | Particle type (1 = objects) | 1,377; 1 ×1,209 |
| 477, 478, 479, 480 | Sound: sound ID ([audio.md](audio.md) 3.6), start distance, full-volume distance, volume | 386 sounds, IDs 34 to 742 (44 ×105); distances 1000 / 200 typical; volume 2 typical |
| 483, 484 | Secondary explosion OID and rate | 16 |
| 485 to 487 | Debris radius, elasticity, friction | 68, 64, 53 |

Explosions are set off by script calls (options 481, 482, 489, 490), by breakables (their option 240 names an
explosion OID), by bombs and missiles. Stock explosion groups at OIDs 12 to 42 are cloned at run time
(object-stream.md 7.1). [R] [D]

## 3. Flash volumes (class 96)

65 on the disc. [D] A flash volume is a visible beam (its mesh) and a convex volume (the stream data) together:
the light beam of a guard's flashlight. Its scale eases toward a target (fast rate 1 per second, slow rate 0.2,
easing time 0.1 s): it shrinks to 0.1 when its guard is knocked out. A point is lit by the beam when, divided by the
current scale, it lies inside the convex volume ([collision.md](collision.md) 5); a scale below 0.0001 never lights
anything. Detection rules are in guards.md. [R]

## 4. Lightning flashes (class 145)

77 on the disc, in 41 worlds. [D] A lightning flash fills the whole screen with a colour that changes over time when
a script starts it (option 1132): [R]

- from the start colour (1133) to the peak colour (1134) over the fade-in time (1136);
- holds the peak for the hold time (1138);
- from the peak to the end colour (1135) over the fade-out time (1137); then stops.

Option 1139 chooses whether it is drawn behind the world (0) or in front (1); every flash on the disc is 1. Flashes
are not drawn while the binocular view or a radio dialogue is up, or outside play. [R] [D]

Values on the disc [D] (colours as R, G, B, A bytes; alpha on the 0x80 = opaque scale [I]):

| Use | Start | Peak | End | Fade in / hold / fade out (s) |
|---|---|---|---|---|
| 41 flashes | (255, 255, 255, 128) | (255, 255, 255, 51) | (255, 255, 255, 1) | 0.08 / 0 / 0.2 |
| 22 flashes | (100, 100, 255, 1) | (200, 200, 255, 88) | (255, 255, 255, 1) | 0.04 / 0.02 / 0.08 |

## 5. Lights (class 81)

2,254 lights in 44 worlds. [D] Lights feed the lighting of kind-0 materials ([textures.md](textures.md) 4). [R]

| Option | Meaning | Default [R] | On the disc [D] |
|---:|---|---|---|
| 491 | Kind: 0 directional, 1 point, 2 frustum, 3 spot | 0 | 1: 2,202; 0: 50; 3: 2 |
| 492 | Highlight colour as hue (degrees), saturation (0 to 255), value (0 to 255) | — | all; (0, 0, 0) ×426, (30, 200, 250) ×46 |
| 493 | Mid-tone strength (0 to 255 scale; up to 511 seen) | 0 | 200 ×604, 100 ×339 |
| 494 | Shadow strength | 0 | 0 ×2,209, 160 ×41 |
| 495, 496, 497 | Highlight, mid-tone and shadow angles, degrees | 180, 240, 180 | e.g. 0 / 200 / 0 |
| 502 | Fall-off distance range: full strength at the minimum, none at the maximum | 200, 2000 | 2,204; (300, 700) ×45 |
| 503, 504 | Spot cone and hot-spot angles, degrees | 60, — | 2 spots: 33.3, 18.8 |
| 500 | Lights moving objects | — | 2 |
| 501 | Does not light moving objects | off | 765 |

Each lighting term (highlight, mid-tone, shadow) grows linearly in `f(θ) = cos(θ/2) + cos³(θ/2)`, θ the angle
between the surface normal and the direction to the light: zero at the term's angle, its full strength (÷ 255) when
facing the light; the shadow term is mirrored (from 360° − shadow angle to 360°). The fall-off follows a
reciprocal curve through (minimum, 1) and (maximum, 0). [R]

## 6. Sky (class 84)

40 sky objects in 39 worlds. [D] A sky moves with the camera every frame: its position becomes the camera's in X and
Y, and in Z too when option 608 is set (9 skies). It is never frozen. [R] [D]

## 7. Shadows

- **Blob shadows**: options 41 (radius) and 42 (depth) give a simple round shadow below an object; option 41 is
  set 574 times across 28 classes (for example on class 10 guards: radius 200 on 18 of 41, depth 300 on 35 of 41). [D] [R]
- **Projected shadows**: options 43 (cast a shadow), 44 (shadow shader OID), 45 and 46 (near and far radius), 47 and
  48 (near and far casting distance, defaults 100 and 400), cast straight down (−Z). Set on every player character
  (class 7, 41 worlds): shader 347, radius 150 near / 10 far, casting distance 0 to 500. [D] [R]
- **Dynamic shadow** (class 97, one per player world, 41): renders the character's silhouette into a 64 × 64
  texture (the 32-bit bitmaps of [textures.md](textures.md) 1) that the projected shadow then uses. [D] [R]

## 8. World darkening

A world-wide brightness factor multiplies every drawn mesh except meshes whose mesh flags byte has bit `0x04` set
([geometry.md](geometry.md) 3). [R]

- Animation event 57 sets it at once; event 58 sets a target it eases toward (options 540 and 541 do the same from
  scripts). [R] On the disc only event 58 occurs: value 0.3 (41 times), 1.0 (41), 0.6 (4). [D]
- Easing toward a darker target runs at 2.0 per second, toward a brighter one at 1.0 per second, slowing linearly
  to 0 over the last 0.1 s of travel. [R]

## 9. Test vectors

| Item | Value |
|---|---|
| Objects by class [D] | emitters 1,321; explosion groups 267; explosion emitters 13 (class 121) and 2,407 (class 122); flash volumes 65; lightning flashes 77; lights 2,254; skies 40; dynamic shadows 41 |
| First emitter in "Paris, France" (OID 1387) [D] | options: 351 (25, 50); 384 (1.4, 1.7); 377 (130, 150); 380 −1; 387 1.1; 403 1.8; 389 (0, 0.3); 379 (0, 0, 20); 390 (0, 3, −3); 399 (13, 0, 0); 372 (1, 0, 0); 376 2; 398 shader 2922; no curve; one colour `23 D9 FF 80`, flag 0 |
| First lightning flash in "Paris, France" (OID 1807) [D] | start `FF FF FF 80`, peak `FF FF FF 33`, end `FF FF FF 01`, fade in 0.08, hold 0, fade out 0.2, drawn in front |
| Emitter curves [D] | none 1,226, polyline 7, cubic 88 (class 80); none 2,345, polyline 25, cubic 37 (class 122) |
| Origin meshes [D] | 86 on class 80, 199 on class 122 (285 in total) |

## 10. Open questions

- The exact sprite orientation and motion styles (options 405, 407, 409) and the object-particle types (392).
- How the colour list and colour curve combine.
- The full lighting formula for kind-0 materials (textures.md 4), which blends their three palettes (rendering spec).
- Whether lightning alpha is on the 0x80 or 0xFF scale.
