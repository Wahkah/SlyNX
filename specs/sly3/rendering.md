---
id: sly3-rendering
title: Sly 3 rendering - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      option set 0 of every world (fog, fog colour, world shadow colour, colour-grading and tint options, the packed
      colour of option 1024); the option descriptors at 0x32CA80 for those options and for the light options; the
      code sites that read the condition set's colour field; the shader register values (textures.md 4)
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/shd.cpp, lightsw.cpp, light.cpp, sky.cpp, game/cond3.h, game/cond3.cpp
    used_for: >-
      the passes each shader kind builds in Sly 3, the light fields, the world shader colour and fog fields of the
      condition set, the sky placement; marked [R]. That port carries Sly 2's renderer with Sly 3's numbers and does
      not draw kinds 2 and 3
  - kind: reference-project
    what: earlier research in this workspace (Games/Sly3-RE-notes.md, tools/sly3_world.py, 2026-11)
    used_for: the world walker used for the surveys; the observation that vertices without colour-table entries are lit at run time
  - kind: public-doc
    what: PS2 GS documentation (TEST, ALPHA, TEX0 MODULATE, FOG, FOGCOL)
    used_for: reading the register values
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 rendering: differences from Sly 2

Read [../sly2/rendering.md](../sly2/rendering.md) first; everything there holds for Sly 3 unless this file says
otherwise. Tags: [D] disc data, [X] executable data at the given address (after the SHA-256 check of
[disc.md](disc.md) 1), [R] reference behaviour, [I] inferred. Pass numbers are Sly 3's (Sly 2's minus one,
[textures.md](textures.md) 4).

## 0. Answers in brief

| # | Question | Answer |
|---:|---|---|
| 1 | Lighting; palette slots of kinds 1 and 7 | As Sly 2 (two colours A and B per vertex; kind 1 = slot 1 × A, then slot 0 × B added; kind 7 with its extra first pass). New: most static meshes carry the **pre-baked A and B** in their colour tables, so the static bake runs only for vertices without them (section 2). [R] [D for the tables] |
| 2 | Grey filter | Sly 2's colour adjustment is still off. What Sly 3 adds is the **colour grading** of palettes (textures.md 5), with per-world options; a desaturation step exists but is off in retail (section 3). [R] [D for the options] |
| 3 | Texture frames | As Sly 2: the shader's animation record decides, through records of classes 175 to 180 (Sly 2's 139 to 144) (section 4). [D] [I for the class correspondence] |
| 4 | Passes; sky | Sly 2's list minus one (section 5); pass 15 (kind 5) is new. The sky (class 110) is moved to the camera's position for each camera that draws it, height kept unless its flag says otherwise. [R] |
| 5 | Kinds 2, 3, 6; flags `0x08`, `0x02` | As Sly 2 (kind 2 hulls in passes 6 and 7, kind 3 volumes in 10 and 14, kind 6 in pass 1). In Sly 3 kind 2's depth test is chosen by flag `0x04` (section 5). [R] |
| 6 | Material colour scale | As Sly 2: RGB 0xFF = 1.0, alpha onto the 0x80 scale. [R] |
| 7 | Clear colour, fog, darkening | Fog and the world shadow colour are **options of the world's option set** (1010 to 1017; section 6), not world options. Option 1024 is the **clear colour** (section 6) [X]. No darkening option. [D] [R] |
| 8 | Stray objects | No far copies like Sly 2's intro hub; the large thing out of place is the hacking game's stage parked 25,000 units up in the Holland worlds (section 7). [D] [R] |
| 9 | Colours above 0x80 | As Sly 2: the GS colours are the computed or pre-baked A and B on the 0x80 scale and brighten the texel up to about 2×. [R] |
| 10 | Opaque alpha cut-off | There is **no discard cut-off**: every fragment of an opaque pass is blended; alpha ≥ `0x7E` (0.984, 251/255 after decoding) only decides which fragments **write depth** (textures.md 4). [D] [R] |

## 1. Unchanged from Sly 2

GS conventions (modulate with 0x80 = 1.0, the blend formula, "greater" as nearer), the light model and the A/B
split (../sly2/rendering.md 2), the passes of kinds 0, 1, 4, 5, 6 and 7 (3.1 to 3.3; for kind 7 the slot count is
decided by flag `0x10` instead of `0x20`, textures.md 4), the fog formula, the material colour scale and the colour
headroom (8). [R, from the Sly 3 register lists the reference reads at `0x235128` [X]]

Kind 5 (44 shaders, pass 15, new in Sly 3) builds the same one-pass list as kinds 4 and 6: the alpha test never
passes and depth is tested "always", so it draws colour only, without depth. [R]

## 2. Lighting

### 2.1 Lights

Light objects use options 159 to 166 [X: descriptors at `0x32CA80 + 40 × id`] [R]:

| Option | Sly 2's | Meaning |
|---:|---:|---|
| 159 | 151 | kind (0 directional, 1 point) |
| 160 | 152 | colour as HSV |
| 161 | 153 | strength |
| 162 | 154 | cone angle |
| 163 | 155 | point-light fall-off range |
| 164 | 157 | not offered to moving objects' four-light selection |
| 165 | 158 | not used for the static bake |
| 166 | 156 | weight for the direction of cast shadows, now a **float** (0 = not used) |

The evaluation is Sly 2's (../sly2/rendering.md 2.1), including the default light. [R]

**The light class is class 4** (Sly 2's class 4; the reference's correspondence scores it 98) [R]. On the disc:
2,348 class-4 records in all 39 worlds. Every one carries 159 to 162, 2,158 carry 163, 2,342 carry 164, 2,368
occurrences of 165 appear, and 28 carry 166 [D]. In the option survey, no other class carries any of 159 to 166
[D]. "A record with option 160 is a light" therefore picks out exactly the class-4 records on this disc. The sound
rule is the class: a light is a class-4 object, or one derived from it. A light that relied on the class defaults
for its colour (no option 160) would be missed by the option test, although the disc has none. Option 160's
descriptor (value type 2, a vector; setter `0x001C54F0`) [X] is used only by the light class.

### 2.2 The world shadow colour

`W = (RGB(HSV) × scale / 255, ambient / 255)` as in Sly 2, from the world's option set: **1015** HSV (hue 0..360,
saturation and value 0..255), **1017** scale, **1016** ambient (Sly 2's amount) [D for the options, R for the
formula]. Values per world: section 6.

### 2.3 Pre-baked colours (colour tables)

A strip mesh may carry colour tables ([geometry.md](geometry.md) 4.2): per vertex a colour A, a colour B (15-bit,
5 bits per channel, the low 3 bits zero after expanding) and an amount byte. These are the static bake's results
stored on the disc: Sly 2's bake produces exactly such colours (quantised to 5 bits per channel, on the 0x80 scale)
[I]. So:

- a strip list whose flags have bit `0x20` takes A, B and the amount from the matching table (the last table whose
  condition matches; geometry.md 4.2) and is **not** baked at run time; the amount is B's alpha (Sly 2's β) [I];
- other static vertices are baked at load from the lights as in Sly 2 (2.3 there) [R];
- moving objects are lit per frame by their four strongest lights as in Sly 2 [R].

Because the tables are chosen by condition, a world's lighting can change with the job or task state without any
light changing. [R]

## 3. Colour grading, tint and the grey filter

- Palettes with flag bit 0 are graded at load ([textures.md](textures.md) 5) with lightness `a`, contrast `b` and
  saturation `c`. Their defaults are 1.5, 1.2, 1.4. **Options 1025, 1026 and 1027** of the option set override them
  [X: descriptors writing fields `0x38`, `0x3C`, `0x40` of the condition set]. On the disc [D]:
  - c_ext sets (1, 1, 1): **no grading** in China's hub;
  - c_flashback sets (1.0, 1.1, 0.24): saturation 0.24, the near-grey look of the flashback.

  (textures.md 5 said no option set changes them; corrected there.)
- **Option 1028** (a `vec3`, field `0x50`) is a per-channel adjustment of the same grading; only p_dive sets it,
  (−15, −10, 10) [D]. *Meaning:* added to the graded R, G, B on the 0..255 scale, giving the underwater cast [I].
- **Option 1031** (field `0x8C`, p_dive 0.8) is a desaturation amount used only when a global switch is on; that
  switch is off in retail, so it has no effect [R].
- Sly 2's colour adjustment of shader and light colours stays off [R].

## 4. Texture frames and UV animation

As ../sly2/rendering.md 4, with these Sly 3 differences [D]:

- the shader's animation reference is an `s8` index into the set's animation records (textures.md 2.4);
- records are a `u16` class index and **36** bytes (Sly 2: 32);
- classes 175 to 180 with counts 178, 42, 5, 193, 99, 28. In order they correspond to Sly 2's 139 (loop), 140
  (ping-pong), 141 (random frame), 142 (eyes), 143 (UV scroll) and 144 (UV circle) [I: same order, comparable
  counts]; the first 32 bytes are read as in Sly 2 and the four extra bytes are of unknown use.

## 5. Passes

| Pass | Sly 2 pass | What [D for the kinds, R for the role] |
|---:|---:|---|
| 0 | 1 | sky (kind 4), no depth test or writes |
| 1 | 2 | background layers (kind 6) |
| 2 | 3 | static opaque scenery (kinds 0, 1) |
| 3 | 4 | moving opaque objects |
| 4 | 5 | cel scenery (kind 7) |
| 5 | 6 | faded moving objects (run time) |
| 6, 7 | 7, 8 | kind-2 shadow hulls (6: effect outlines and indexed bodies; 7: 18 indexed) |
| 8 | 9 | ink outlines (outline bodies) |
| 9 | 10 | translucent, sorted far to near |
| 10, 14 | 11, 13 | kind-3 projected volumes |
| 15 | — | kind 5 (44 shaders): colour only, no depth |

Kind 2 in Sly 3: two untextured passes as in Sly 2; the depth test of the counting passes depends on flag `0x04`
instead of `0x08` [R, from the reference's note on the Sly 3 register list]. Kind 3: as Sly 2 (flag `0x02` = the
volume also draws its texture). [R]

**Sky** (class 110, 21 worlds): for every camera that draws it, the sky's meshes are drawn with the object moved to
the camera's position, its own height kept unless the object's "follow height" flag is set (Sly 2 moved the object
itself each frame; the result on screen is the same) [R].

## 6. Fog, clear colour and darkening

Sly 3 keeps these in the **option set** of the world's current condition ([world-file.md](world-file.md) 3.7), not
in the world option list [D]:

| Option | Field | Meaning | Default when no set gives it [R] |
|---:|---|---|---|
| 1010 | `0x14` | fog start | 5,500 |
| 1011 | `0x18` | fog end | 60,000 |
| 1014 | `0x1C` | fog maximum | 0.4 |
| 1012 | `0x20` | fog colour, packed RGBA (R in the low byte) | `80965515` (R 0x15, G 0x55, B 0x96) |
| 1013 | `0x20` | fog colour given as HSV (hue, saturation 0..255, value 0..255), converted to RGBA | — |
| 1015, 1016, 1017 | `0x60`, `0x74`, `0x70` | world shadow colour (2.2) | (210, 150, 100), 35, 60 |
| 1024 | `0x34` | **clear colour** (packed RGBA): see below [X] | not established; black assumed [I] |
| 1025 to 1028, 1031 | `0x38` … `0x8C` | colour grading (section 3) | |

**Option 1024 is the clear colour** [X]. The option-set fields live in a block at `0x0036BC10`. The fog colour
(+0x20) is at `0x0036BC30`, read by the shader set-up code (`0x00235128`, five reads), and 1024's field +0x34 is at
`0x0036BC44`. The world-draw routine `0x0015A600` copies that word near its start, before any scene drawing, and
passes it with the screen rectangle (`0x0036E680`) to `0x00195808`. That routine emits a **full-screen sprite of
that colour**: the PS2 way to clear, opaque when the alpha byte's top bit is set (0x80). The second user,
`0x001689A8`, does the same for another view. So a world with 1024 starts each frame from that colour instead of
black. Values: black in the menu, grey (100, 100, 100) in the caves and the vault, light grey (200, 200, 200) at
sea, dark blue (R 30, G 40, B 100) in the China halls (section 6 table).

The fog formula and the black fog colour of additive passes are Sly 2's. [R] (The fog fields are named by
analogy with Sly 2's camera fields; the reference marks the start/end/maximum reading as a guess that the values
support: start < end everywhere, maximum 0.18 to 0.8.) There is no darkening option. [D]

Option set 0 of every world [D] (fog start / end / maximum; fog colour as given; shadow HSV, scale, ambient;
option 1024):

| World | Fog | Fog colour | Shadow colour | 1024 |
|---|---|---|---|---|
| 0 dvd_menu | 1,300 / 6,000 / 0.3 | HSV (45, 200, 100) | (230, 260, 250), 95, 0 | `00000080` |
| 2 i_trainer | 3,000 / 60,000 / 0.3 | HSV (210, 106, 152) | (220, 200, 200), 120, 15 | — |
| 3 v_ext, 4 v_canal | 3,000 / 60,000 / 0.3 | HSV (82, 100, 75) | (195, 255, 210), 140, 10 | — |
| 5 v_apt, 7 v_interpol | 2,000 / 6,000 / 0.2 | `80648237` | (220, 180, 170), 140, 10 / 0 | — |
| 6 v_gauntlet | 1,000 / 10,000 / 0.8 | HSV (96, 99, 69) | (200, 180, 180), 115, 0 | — |
| 8 o_ext | 3,000 / 60,000 / 0.45 | HSV (320, 55, 45) | (250, 150, 125), 135, 30 | — |
| 11 o_cave_a, 14 o_cave_murray | 1,800 or 1,500 / 20,000 / 0.6 | `80000000` (black) | (228, 200, 140), 145, 25 | `80646464` |
| 15 h_ext | 5,500 / 60,000 / 0.6 | HSV (195, 165, 75) | (245, 155, 165), 155, 30 | — |
| 23 c_ext | 1,500 / 10,000 / 0.2 | HSV (333, 99, 100) | (268, 100, 50), 200, 3 | — |
| 24 c_intro, 28 c_hall_a, 29 c_hall_b, 30 c_tilt_hall | 3,200 or 4,500 / 5,500 to 60,000 / 0.3 | `80937136` | varies | `8064281e` |
| 31 p_ext, 32 p_at_sea | 2,500 or 7,000 / 60,000 / 0.7 or 0.5 | HSV (23, 65, 25 or 35) | (266, 100, 50), 180 or 80, 45 or 65 | `80c8c8c8` |
| 35 m_ext | 5,500 / 60,000 / 0.4 | HSV (210, 220, 150) | (210, 150, 100), 60, 35 | — |
| 37 m_vault, 39 m_boss | 1,800 / 8,000 / 0.3 | `80000000` | (230, 230, 130), 130, 20 | `80646464` |

Option 1024 occurs in 12 worlds: dvd_menu (`80000000`); o_cave_a, o_cave_murray, m_vault, m_boss (`80646464`);
h_hangar_c, p_ext, p_at_sea (`80c8c8c8`); c_intro, c_hall_a, c_hall_b, c_tilt_hall (`8064281e`). [D]

(Packed colours are written here with alpha first, as a little-endian word read as `0xAABBGGRR`: `80648237` is
R 0x37, G 0x82, B 0x64. m_ext's values equal the reference's defaults, which were read from the game's memory in
that world, and the HSV reading of 1013 is confirmed by it: HSV (210, 220, 150) gives R 21, G 85, B 150 =
`0x15, 0x55, 0x96`.)

## 7. Geometry out of the way

Sly 3 has no far mesh copies like Sly 2's intro hub. In the Holland worlds the hacking minigame's stage (the hack
classes 73 to 78: arena, gunship, beads) is parked about 25,000 units above the level; it is real geometry that is
only active during the minigame, and the game does not draw it otherwise (its objects are inactive) [R] [D for the
position]. Some character bodies are hidden at load with no world filter and shown by game state [D]. A port should
draw only active objects.

## 8. Test vectors [D]

- Option set 0, v_apt: 1010 = 2,000, 1011 = 6,000, 1014 = 0.2, 1012 = `80648237`, 1015 = (220, 180, 170), 1017 = 140,
  1016 = 10. So `W = (RGB(220°, 180/255, 170/255) × 140/255, 10/255)`.
- c_flashback: 1025 = 1.0, 1026 = 1.1, 1027 = 0.24; c_ext: 1025 = 1026 = 1027 = 1.0.
- Descriptor of option 1024: type `0x0D` (packed colour), field `0x34`; option 1012: type `0x0D`, setter
  `0x1691B0`; option 1013: setter `0x1691E0` [X].
- Shader register values: textures.md 4.

## 9. Open questions

1. The value of the clear colour when a world's option set does not give 1024 (no write to its field was found
   besides the option).
2. Options 1018 to 1023 (fields `0x90` to `0xC0`, with a shared setter; only 1021 = 256 occurs, in c_hall_b) and 1030
   (field `0x88`).
3. The meaning of the colour-table amount byte and the exact match of the tables to Sly 2's bake (2.3); a frame
   comparison settles both.
4. Every [R] rule needs a frame comparison against the game in an emulator, as for Sly 2.
