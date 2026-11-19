---
id: sly2-behaviour-effects
title: Sly 2 visual effects - differences from Sly 1
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      every option list of every body of classes 4, 29, 31, 32, 33 and 97 at every depth (an every-depth walk of
      all 121,153 bodies, 0 failures), the world option lists of all 43 worlds; the option table entries 13,
      151 to 158, 469 to 593 and 914 to 916 read as data (value type, flags, record offset, record selector);
      counts, values and test vectors below
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/emitter.cpp, emitter.h, light.h, lightsw.cpp, lightsw.h, sky.cpp
    used_for: >-
      which internal record each option writes and what that record field does (the defaults, the emission,
      origin, velocity and drawing rules, the light formula, the sky rule); marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/new_2584a8, src/new_258658, src/dzg-emitter
    used_for: cross-check of the emitter and explosion-group loaders (layouts in classes/effects-logic-ui.md)
reviewed_by: pending
date: 2026-11-17
---

# Sly 2 visual effects

A delta of [../sly1/effects.md](../sly1/effects.md). The particle system behaves as in Sly 1 (same defaults, same
emission modes, same motion equation); what changed is the class numbers, the **option numbers**, the origin kinds,
lights (now ordinary placed objects with their own options) and the sky. Byte layouts are in
[classes/effects-logic-ui.md](classes/effects-logic-ui.md) (emitters, explosion groups) and
[classes/simple-layouts.md](classes/simple-layouts.md) (lights). Statements are tagged [D] disc data, [R] reference
behaviour, [I] inferred, as in the Sly 1 specs.

## 0. Class map

| Sly 1 | Sly 2 | Bodies on the disc, every depth [D] |
|---|---|---:|
| particle emitter, class 80 | **class 29** (continuous emitters, packages named `pe_…`) | 6,230 in 42 worlds |
| explosion group, class 120 | **class 31** (packages named `explode_…`) | 1,958 in 42 worlds |
| explosion emitters, classes 121 and 122 | **class 33** (8,381) and **class 32** (52) | 42 worlds |
| light, class 81 | **class 4** (transform header and options) | 2,855 in 42 worlds |
| sky, class 84 | **class 97** (solid layout) | 19 in 18 worlds |
| flash volume (96), lightning (145) | no class with that role identified [I] | — |

The object-stream class table ([object-stream.md](object-stream.md) 3) describes class 4 as "placed marker with
settings"; every class-4 body on the disc carries the light options 151 and 152, so class 4 is the **light** [D].

## 1. Emitters (classes 29, 32, 33)

### 1.1 How options reach the emitter [D] [R]

Each emitter option's entry in the executable's option table (virtual address `0x002AC880`, 40-byte entries,
read after the hash check) names a **record selector** and a **field offset**. The selectors used by the emitter
options pick one of: the emitter's own record (emission), its particle-launch record (origin, velocity, motion,
lifetime), a sprite record, an object-particle record, the origin record, and a sound record. Classes 32 and 33
use a second, parallel set of option numbers that write the same records [D: same offsets and value types]. The
tables below give, for each option, its Sly 1 counterpart; where the counterpart is listed, the meaning, default
and behaviour are those of the Sly 1 spec section named [R], and only the number differs.

### 1.2 Emission (continuous emitters, class 29)

| Option | Value | Sly 1 counterpart ([../sly1/effects.md](../sly1/effects.md) 1.2) | On the disc [D] |
|---:|---|---|---|
| 472 | range | 351 emission rate (default 10, 10) | 6,228; (30, 35) ×369, (10, 10) ×340, (20, 20) ×319 |
| 473 | float | 352 constant population | 5 (200 ×3, 120 ×2) |
| 474 | float | 353 pause probability | 198, all 1.0 |
| 475 | range | 354 pause length | 198; (0.6, 1.0) ×84, (0.3, 0.3) ×45 |
| 471 | enumeration | 350 emission mode | 203; 2 ×198, 1 ×5 |
| 470 | flag | 349 pause after the first burst | — |
| 469 | integer | 348 total particle budget | 3 (4 ×2, −1 ×1) |
| 476 | flag | 356 rate per unit of box volume | — |
| 477 | object ID | 361 emit together with this group | 1,347 |
| 13 | float | (activation and cull radius of any object) | 4,914; 3,000 ×3,186, 4,000 ×714, 2,000 ×528 |

### 1.3 Origin: changed

| Option | Value | Meaning | On the disc [D] |
|---:|---|---|---|
| 483 | enumeration | **Origin kind**: 0 a point (default), 1 a box, 2 on another object's surface, 3 on the origin mesh. Sly 1's curve and skeleton kinds are gone; the mesh kind moved from 4 to 3 [R for the kind names, D that kind 3 is exactly the bodies with a mesh] | 3,458; box 3,212, surface 156, mesh 84 |
| 484 | `vec3` | the point | — |
| 485, 486 | `vec3` each | the two box corners | 3,212 each; (−15, −15, −15) to (15, 15, 15) ×300 |
| 487 | object ID | the object whose surface emits | 242 |
| 488 | object ID | the source object the emitter follows | 8 |
| 490 | enumeration | **direction kind**, as Sly 1 371 (5 = along the mesh normal) | 168; 1 ×126, 5 ×42 |
| 491 | `vec3` | fixed direction (Sly 1 372) | 4,502; (1, 0, 0) ×2,221, (0, 0, 1) ×1,884 |
| 494 | range | start offset along the direction (Sly 1 375) | 506 |
| 495 | enumeration | velocity frame (Sly 1 376: 0 relative, 1 relative with spin, 2 not relative after launch, 3 world) | 5,591; 3 ×3,344, 0 ×1,460, 2 ×787 |

The origin mesh (classes 29 and 33 only) is in the body after the options when the origin kind is 3 (class 29)
or the direction kind is 5 (class 33) ([classes/effects-logic-ui.md](classes/effects-logic-ui.md) 1) [D].
Particles start on its triangles as in Sly 1 1.1 [R].

### 1.4 Launch and motion

| Option | Value | Sly 1 counterpart ([../sly1/effects.md](../sly1/effects.md) 1.4) | On the disc [D] |
|---:|---|---|---|
| 496 | range | 377 speed (default 750, 750) | 6,188; (0, 0) ×1,379, (20, 30) ×353 |
| 497 | float | 378 vertical speed factor (default 1) | 4,657; 1.0 ×2,788, 0 ×1,265 |
| 498 | `vec3` | 379 acceleration (default (0, 0, −1500)) | 6,272; (0, 0, 0) ×2,773, (0, 0, 100) ×447 |
| 499 | float | 380 damping per second (default −1) | 6,201; −1 ×2,598, 0 ×1,050, −2 ×590 |
| 500 | float | 381 curl, radians per second | 480; 2 ×140, 3 ×127 |
| 501 | `vec3` | 382 curl axis (default (0, 0, 1)) | 42, all (0, 0, 1) |
| 502 | range | 384 tilt range, radians | 5,974; (0, 3) ×1,148, (0, 3.14) ×1,126 |
| 503 | integer | 385 particles per ring (default 200) | — |
| 504 | float | 386 random share (default 1) | 2,461 |
| 505 | float | 387 lifetime, s (default 1) | all 6,230; 0.5 ×888, 0.3 ×844 |
| 506 | range | head-start: each particle is moved along its velocity by a time drawn from this range at birth [R] | 248 |
| 507 | range | 388 pre-age: the particle is born up to this many seconds in the past [R] | 1,683; (−0.2, 0.2) ×318 |
| 508 | `vec3` curve | 390 alpha over age (default (1, 0, −1)) | 6,200; (1, 0, −1) ×556, (0, 1.5, −1.5) ×548 |

The motion law (`dv/dt = a + damping × v`, curl about the axis) and the ring spread are Sly 1 1.4 [R].

### 1.5 Drawing: sprites and object particles

Each emitter selects a **particle kind** (option 482, setter; 1 on 269 class-29 emitters) and, inside it, a
**payload** (sprites: option 523, 2 ×3,234, 1 ×365; objects: option 509, 3 ×252, 8 ×10, 2 ×4) [D]. The fields
below are written only into the block that matches the current kind and payload [R].

| Option | Value | Meaning | Sly 1 counterpart ([../sly1/effects.md](../sly1/effects.md) 1.5) | On the disc [D] |
|---:|---|---|---|---|
| 526 | object ID | shader of the sprites | 398 | 5,961 |
| 529 | `vec3` curve | size over age | 399 | 5,964 |
| 528 | float | seconds per cycle through the shader's frames | 403 | 2,604; 0.2 ×688, 0.5 ×575, 10 ×443 |
| 524 | enumeration | update style of the sprites [R] | 405 | 46, all 2 |
| 525 | flag | (sprite flag) | — | 2,556; 1 ×2,409 |
| 527 | object ID | (reference object) | — | 46 |
| 530, 531, 532 | enumeration, flag, range | payload 2: orientation style, random roll, spin range [R] | 407, 411, 410 | 1,594, 2,203, 3,231 |
| 533 | float | payload 1: stretch factor while flying | 408 | 365; 5 ×135, 10 ×126 |
| 510 to 516 | | object particles: kind, the object to clone (511), two more object IDs, a float, a range, a size curve | 391, 392 | 37 to 269 each |
| 517, 518 / 519 / 520 to 522 | | object-particle payloads 2 (object ID and a float), 3 (a byte), 8 (three floats) | — | 4 / — / — |

Colours from the body's colour list are used as in Sly 1 1.1, at most 32; each colour channel is halved
(`(c + 1) >> 1`) before use, putting the 0..255 bytes on the 0..128 scale of the drawing hardware [R]. Without a
colour list the shader's colour is used [R]. A shader with flag bit 0 set draws its particles additively [R]. The
number of animation frames used is the shader's frame count rounded up to a power of two, at most 16 [R].

### 1.6 Explosion parts (classes 32 and 33)

Explosion parts carry the same fields under the second set of option numbers [D: same record offsets and value
types in the option table]. The counterparts:

| Class-33 option | Same as class-29 option | Class-33 option | Same as |
|---:|---|---:|---|
| 536 | 483 origin kind | 558 | 498 acceleration |
| 537, 538, 539, 540 | 484, 485, 486, 487 | 559 | 499 damping |
| 541 | 488 source object | 560, 561 | 500, 501 curl |
| 542 | 490 direction kind | 562 | 505 lifetime |
| 543, 544, 545 | 491 to 493 direction vectors | 563 | 507 pre-age |
| 546 | 494 start offset | 564 | 508 alpha curve |
| 547 | 495 velocity frame | 565 to 571 | 510 to 516 object particles |
| 548, 549 | 496, 497 speed, vertical factor | 572 to 577 | 517 to 522 object payloads |
| 550, 551, 552, 553 | 502, 503, 504, 506 | 578 to 584 | 523 to 529 sprites |
| 554, 557 | 509, 482 payload and kind | 585 to 588 | 530 to 533 |

Explosion-only options:

| Option | Value | Meaning | On the disc [D] |
|---:|---|---|---|
| 555 | range | particle count per detonation (Sly 1 440) | 8,233; (1, 1) ×1,143, (30, 30) ×346, (8, 10) ×331 |
| 556 | flag | (Sly 1 445, grouped [I]) | 294, all 1 |
| 589 | `s16` | **sound** ID played at the detonation ([audio.md](audio.md)) | 2,832; 151 ×169, 147 ×142, 112 ×128 |
| 590 | float | sound start distance (default 3,000) | 2,779; 3,000 ×2,039, 5,000 ×194 |
| 591 | float | sound full-volume distance (default 300) | 2,779; 300 ×2,089, 500 ×150 |
| 592 | float | sound volume (default 1) | 2,045; 1.0 ×1,510, 0.8 ×152 |
| 593 | float | sound pitch offset [I] | 2,423; 0 ×1,141, −0.2 ×155, 0.5 ×137 |

The sound defaults (no sound, 300, 3,000, 1) are those the reference sets when an explosion first uses its sound
record [R]; that 590 is the start distance and 591 the full-volume distance follows from start > full, as in Sly 1
[I].

### 1.7 Explosion groups (class 31)

A group has no options of its own (0 option values on all 1,958 bodies) [D]; its object list holds the parts. Setting
off a group sets off every part; a part draws its count from 555 and fires one burst, then its sound [R]. Groups
are set off by scripts, breakables, bombs and gadgets, as in Sly 1 2 [R]. The parts' parent reference `0x2000`
places them relative to the group ([transforms.md](transforms.md) 1) [D].

## 2. Lights (class 4)

| Option | Value | Meaning [R] | Default [R] | On the disc [D] |
|---:|---|---|---|---|
| 151 | enumeration | kind: 0 directional, 1 point | 0 | all 2,855; point 2,688, directional 167 |
| 152 | `vec3` | colour as hue 0..360, saturation 0..255, value 0..255 | — | all 2,855 |
| 153 | float | strength, 0..255 | 0 | all; 60 ×282, 100 ×239, 150 ×226 |
| 154 | float | cone angle, degrees (at least 30 is used) | 240 | all; 240 ×2,059, 220 ×313 |
| 155 | range | point light fall-off: full at the first distance, none at the second | 200, 2,000 | 2,688; (200, 600) ×141, (200, 700) ×124 |
| 156 | flag | counts when choosing the direction of cast shadows | on | 121, all 0 |
| 157 | flag | not offered to moving objects' four-light selection | off | 2,843; on ×545 |
| 158 | flag | not used for the static light bake | off | 2,885; on ×168 |

How a light is evaluated [R]: its colour is converted from HSV to RGB and divided by the larger of its value and
strength; a direction-dependent factor is a straight line in `c + c³` (with `c = cos(θ/2)`) through 0 at the cone
angle and `strength / 255` at 0°, as in Sly 1 5; a point light also scales by a straight line in distance from 1 at
the fall-off start to 0 at its end. A directional light's direction is its transform's first axis. When a world has
no bakeable light, a default directional light is used: direction normalise(−0.2, 0.3, −1), strength 255, angle
240, colour black.

Two uses [R]: a **static bake** colours every vertex of static meshes once from every bakeable light (lights with
158 off); **moving objects** (mesh flag 1) are lit each frame by the four lights whose contribution at the object's
centre is strongest, chosen among lights with 157 off. Each produces the two colours a lit shader blends between
(the shadow palette and the lit palette, [textures.md](textures.md) 4).

### 2.1 World shadow colour

Three world options set the colour of the unlit side [D for the options, R for the formula]:

| Option | Value | Meaning | Default [R] |
|---:|---|---|---|
| 914 | `vec3` | hue, saturation, value (0..360, 0..255, 0..255) | (180, 255, 255) |
| 916 | float | scale, 0..255 | 90 |
| 915 | float | amount (alpha), 0..255 | 0 |

The colour is RGB(HSV) × 916 / 255 with alpha 915 / 255. All 43 worlds set all three (row 18 is empty) [D];
915 is 0 in 30 worlds and 10, 20 or 30 in the other 13.

## 3. Sky (class 97)

A sky object is always active and updated every frame [R]. Each frame it is moved to the camera's horizontal
position; its height stays its own unless a per-object flag makes it follow the camera's height too [R] (the option
that sets the flag is not identified). It is drawn first, without depth writes ([textures.md](textures.md) 4, render
pass 1) [R]. 19 sky objects in 18 worlds [D]; option 30 (a `vec3`, (0, 0, 0)) occurs on 3 [D].

## 4. Shadows

Blob shadows are meshes with kind-2 shaders; larger shadows and light shafts are kind-3 projected volumes (render
passes 11 and 13) ([textures.md](textures.md) 4, [geometry.md](geometry.md)) [D for the shader kinds, R for the
roles]. The direction of an object's cast shadow comes from the strongest of its chosen lights that have option 156
on [R].

## 5. Test vectors [D]

Row 3 (`f_nightclub_cellar`):

- **Class-29 emitter, object 3937** (top level): 471 = 2, 474 = 1.0, 475 = (0.6, 1.0), 472 = (1, 2), 502 = (0, 0.3),
  497 = 1, 496 = (300, 400), 499 = −2, 507 = (−0.6, 0.6), 505 = 2.5, 498 = (0, 0, 300), 508 = (0, 4, −4),
  529 = (15, 200, −200), 523 = 2, 532 = (−0.2, 0.2), 531 = 0, 500 = 3, 491 = (1, 0, 0), 495 = 0, 13 = 4000,
  526 = 4972.
- **Class-33 explosion part, object 130056**: 566 = 130050, 557 = 1, 554 = 5, 565 = 1, 555 = (5, 7),
  548 = (700, 1000), 550 = (−0.7, 0.7), 562 = 2, 559 = −10, 558 = (0, 0, −750), 543 = (0, 0, 1),
  570 = (1.5, 2), 13 = 2000, sound 589 = 148, 590 = 3000, 591 = 300, 592 = 1, 593 = 0.
- **Light, object 7430** (nested): kind 0, colour (50, 150, 0), strength 175, angle 240, 157 off, 158 on.
- **World shadow colour**: 914 = (260, 180, 180), 916 = 110, 915 = 10.

Row 0 world shadow colour: (250, 200, 120), 120, 30.

Per-world body counts (rows 0, 2, 3, 4, 5, 6): class 29: 122, 209, 122, 144, 115, 120; class 33: 292, 332, 242,
286, 189, 205; class 4: 122, 253, 96, 84, 96, 85.

## 6. Open questions

- The meaning of options 525, 527, 556, the object-particle fields 510 to 522 and the payload numbers.
- Whether classes 32 and 33 differ in behaviour (they share every option).
- Which option holds the sky's follow-height flag; what option 30 does on the sky.
- Whether Sly 2 has screen flashes, lightning or world darkening, and where (no dedicated class found).
- The sound pitch reading of option 593.
- All [R] statements need checking in play.
