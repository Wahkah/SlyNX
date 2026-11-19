---
id: sly1-rendering
title: Sly 1 rendering - lighting, passes, fog, outlines, billboards, blending, texture animation, shadows
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: >-
      every mesh's render pass, billboard byte, mesh flags, draw-order value, fog factor and animation record walked
      on all 46 worlds; every shader's kind, blend flags, colours and pass; owning classes of each pass; world options
      526 to 535 per world; outline edge index layout checked on Paris, The Hideout and A Rocky Start; vertex and
      material colour surveys; texture-animation parameters; all [D] statements and the tables in section 11
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release: glob.vert, glob.frag, celborder.vert, celborder.frag,
      dysh.frag, render.cpp, glob.cpp, alo.cpp, light.cpp, shd.cpp, shd.h, sw.cpp, cm.cpp, sky.cpp, shadow.cpp,
      shdanim.cpp and the eight texture-animation files, spaprops.cpp (option entries 41, 42, 95, 526 to 535);
      File Extractor GUI/NativeTools/BrxRebuilder ObjectWalker.cpp and AssetWalker.cpp (colour and self-illumination
      conversion)
    used_for: the lighting formulas, pass states, sort rules, fog formula, outline test, billboard rule, animation
      behaviour; everything tagged [R]
  - kind: public-doc
    what: Sony "GS User's Manual" (PS2 Graphics Synthesizer), alpha blending and texture function
    used_for: the 0x80 = 1.0 alpha convention of the hardware
reviewed_by: pending
date: 2026-11-14
---

# Sly 1 rendering

Tags: [D] disc data, [X] executable data at an address, [O] observable in the game, [R] reference behaviour, [I]
inferred. This file builds on [textures.md](textures.md) (shader kinds 4, passes 4.1, blend flags 4.2),
[geometry.md](geometry.md) (meshes, submeshes, outline parts) and [effects.md](effects.md) (lights 5, sky 6,
shadows 7, darkening 8). Pass and kind numbers are the byte values on the disc; the names in this file are
descriptive and our own.

Which pass a mesh is drawn in comes from the **mesh's** pass byte (geometry.md 3), not the shader's: the two differ
on 6,566 of 68,242 submeshes [D], and the reference uses the mesh byte [R].

## 1. Colour scales

| Quantity | 1.0 is | Basis |
|---|---|---|
| Vertex colour R, G, B, A | 0xFF | [R]: the reference divides by 255. [D]: on Paris, The Hideout and A Rocky Start the largest channel of kind-0 vertex colours is 0xF0..0xFF in about half the vertices and 0x90..0x9F in about a third; alpha is 0xFF on 99 % of kind-0 vertices. *Unconfirmed*; see question 1 |
| Material colour (shader record, first colour) | 0xFF | [R] same. The material multiplies the vertex colour per vertex; a strip entry with no colour (index 0x7F) takes the material colour alone [R] |
| Palette alpha | 0x80 | textures.md (never above 0x80 on the disc) |
| Fog colour alpha (option 535), clear colour alpha (528) | 0x80 | [R] multiplied by 2/255 |
| Second (volume) colour of kind-4 shaders | about 0xFF | [R] the alpha is halved with rounding, `(A + 1) >> 1`, then divided by 128; RGB are divided by 255 after the same halving |
| Outline colour (option 95) | 0xFF | [R]; see question 2 |
| Light strengths and world levels (493, 494, 526, 527) | 255 | [R] divided by 255 |

**The 7f7f7f materials.** 79 kind-0 and 8 kind-1 shaders in Paris have material colour `7f 7f 7f ff` (88 + 6 in A
Rocky Start) [D]. Under the 0xFF rule they draw at half intensity. That is the reference's behaviour and the
spec's rule until observation says otherwise.

## 2. Lighting

### 2.1 Kind 1 and every kind except 0 (single palette)

Per pixel, with C the vertex colour after the material (section 1), T the texel (the frame's one palette applied
to the bitmap), K the world darkening factor (effects.md 8; K = 1 for meshes whose mesh-flags byte has bit 0x04)
and F the object's fade (section 6.3) [R]:

    rgb = C.rgb · T.rgb · K
    a   = C.a · T.a · F

Lights do not affect these materials. Projected textures (section 9.2) then multiply `rgb` when `C.a · T.a` is above
0.9.

### 2.2 Kind 0 (three palettes per frame)

Each frame has three palettes over one bitmap ([textures.md](textures.md) 4): slot 0 the **shadow image**, slot 1
the **mid-tone image**, slot 2 the **highlight image** (always grey). With T0, T1, T2 the three texels and the
vertex weights of 2.3 interpolated across the triangle (a scalar weight wS, RGB weights wM and wH) [R]:

    rgb = (T0.rgb · wS + T1.rgb ⊙ wM + T2.rgb ⊙ wH) · K
    a   = C.a · T1.a · F

(⊙ is per-channel product.) Projected textures multiply `rgb` as in 2.1.

### 2.3 Per-vertex weights

Inputs for one vertex [R]:

- Ls = option 527 / 255 (world shadow level), 50/255 when the world does not set it; Lm = option 526 / 255 (world
  mid-tone level), 0 when not set.
- E, the submesh's emission: its self-illumination byte e (geometry.md 3.1) times 0.003973, which is e/251.7.
  On the disc the byte is non-zero on 15,146 submeshes, all kind 0; the common values 30 to 180 give E = 0.12 to
  0.72 [D].
- For each light j reaching the vertex (2.4): the falloff hj (1 for directional lights) and x_j = c + c³ with
  c = N·Lj (N the unit normal, Lj the unit direction from the vertex toward the light), so x_j is in [−2, 2].
  Each light has three lines (a, b) (2.4) and an RGB highlight colour Qj.

Sums:

    σ = Ls + Σj hj · max(aS,j · x_j + bS,j, 0)          (shadow sum)
    μ = Lm + E + Σj hj · max(aM,j · x_j + bM,j, 0)      (mid-tone sum)
    η = Σj hj · max(aH,j · x_j + bH,j, 0) · Qj          (highlight sum, RGB)

Weights, with ρ = 1 − max(η.r, η.g, η.b) the brightness left over by the highlight, μ' = μ clamped to [0, ρ],
and g = (C.r + C.g + C.b) / 3:

    wH = min(η, 1) · g          (per channel)
    wM = μ' · C.rgb
    wS = clamp(min(ρ − μ', σ), 0, ∞) · g

So the highlight takes its share first, the mid-tone fills the rest up to 1, and the shadow image only fills what is
left. A vertex no light reaches shows the shadow image at weight Ls · g (when Lm + E is 0).

**Objects that move.** For an object whose meshes are flagged as moving, each light is evaluated once per object:
the direction is taken from the object's bounding-sphere centre to the light, and a point light's falloff uses
that centre's distance. Lights with option 501 set skip moving objects. Static meshes are lit per vertex and the
result can be cached until a light or a world level changes [R].

### 2.4 Light lines

From each light's options (effects.md 5) [R]. Let `q(θ) = cos(θ/2) + cos³(θ/2)`, θ in degrees, so q(0) = 2,
q(180) = 0, q(360) = −2; the x of 2.3 equals q of twice the angle between the normal and the light direction.
Each term is the straight line `y = a·x + b` through two points:

| Term | Through | Notes |
|---|---|---|
| Highlight (aH, bH) | (q(option 495), 0) and (2, V / 255) | V = third value of option 492. Q = HSV (hue = first value in degrees, saturation = second / 255, value V / 255) converted to RGB and scaled so its largest channel is 1. V = 0: no highlight |
| Mid-tone (aM, bM) | (q(option 496), 0) and (2, option 493 / 255) | |
| Shadow (aS, bS) | (q(360 − option 497), 0) and (−2, option 494 / 255) | grows as the surface turns away from the light |

If the two x values are within 0.0001 the line is flat: a = 0, b = the first y. An angle of 0 therefore turns its
term off.

**Directional light (kind 0):** the light direction is the light's local +Z axis in world space; h = 1.

**Point light (kind 1):** `h = clamp(α + β / distance, 0, 1)`, the reciprocal curve with h = 1 at the minimum and
h = 0 at the maximum of option 502: β = min · max / (max − min), α = 1 − β / min. The default range 200..2000 gives
α = −0.1111, β = 222.22.

**Spot (kind 3) and frustum (kind 2) lights:** a perspective volume along the light's direction, cone angle
option 503 (clamped to 0..175), near plane 50, far plane the falloff maximum. Inside it, h is the product of
(a) a radial term from 1 at `tan(hotspot/2) / tan(cone/2)` (square root of that ratio for spots) to 0 at the cone
edge, and (b) a linear distance term from 1 at the falloff minimum to 0 at the maximum. The reference marks its own
frustum construction as approximate. Only 2 spots and no frustum lights occur on the disc [D], so this is low
priority.

### 2.5 Test vectors (lighting)

World levels as in Paris (option 527 = 60, so Ls = 0.2353; option 526 = 0, so Lm = 0; E = 0). Light: point,
mid-tone angle 200, strength 200, highlight and shadow angles 0, default falloff 200..2000. Then aM = 0.35996,
bM = 0.06439.

| Case | Result |
|---|---|
| q: 0°, 90°, 180°, 200°, 240°, 360° | 2.0, 1.0607, 0.0, −0.1789, −0.625, −2.0 |
| Mid-tone term at N·L = 1, 0.5, 0, −1 (h = 1) | 0.78431, 0.28937, 0.06439, 0 |
| Falloff 200..2000 at distance 100, 200, 600, 1000, 2000, 3000 | 1, 1, 0.2593, 0.1111, 0, 0 |
| Falloff 300..700 (45 lights on the disc) at 300, 500, 700 | 1, 0.30, 0 |
| White vertex, no light | wS 0.2353, wM 0, wH 0 |
| White vertex facing the light, distance ≤ 200 | wS 0.2157, wM 0.7843, wH 0 (sum 1) |
| White vertex facing the light, distance 600 | wS 0.2353, wM 0.2033 |
| Vertex colour (128, 128, 128) facing the light, distance ≤ 200 | wS 0.1083, wM 0.3937 |
| Add highlight: option 492 = (30, 200, 250), angle 90 | Q = (1.0, 0.6078, 0.2157), aH 1.0437, bH −1.1070; facing the light: wH (0.9804, 0.5959, 0.2115), μ' = 0.0196, wS 0 |
| Emission byte 60 / 180 | E = 0.2384 / 0.7151 |
| Pixel from the "facing, ≤ 200" vertex with T0 = T1 = (0.5, 0.4, 0.3) | (0.5, 0.4, 0.3) · K |

## 3. Passes and draw order

### 3.1 Frame order

The frame is cleared to the world's clear colour (option 528, packed colour, alpha × 2/255; default
(16, 16, 40, 80) [R]; on the disc 00 00 00 80 in most worlds, 1D 6B 5B 80, 8C 54 2A 80 and FF AD 4B 80 in a few [D]).
Depth test is "less", depth writes on and back faces culled unless a row says otherwise [R]. Lists are drawn in
this order:

| # | Pass | What uses it [D] | Depth test / write | Blend | Order within the list [R] |
|---:|---:|---|---|---|---|
| 1 | 0 | character silhouettes for shadows (class 97) | always / off, into a separate 64 × 64 target | none | submission |
| 2 | 1, blend flag 2 | sky meshes (class 84, kind 6) | always / off | alpha over | sorted far to near |
| 3 | 1, other flags | sky meshes with blend flags 34 (9 meshes) | always / off | none | submission; **drawn before list 2** |
| 4 | 2 | portrait panels (hud.md) | normal | per entry | submission (no mesh on the disc has pass 2) |
| 5 | 3 | opaque world and objects (42,348 meshes) | less / on | none | submission |
| 6 | 4 (alpha-tested, see 3.2) | cut-outs, flags ≠ 2 | less / two-step | additive | submission |
| 7 | 4, flag 2 | cut-outs | less / two-step | alpha over | sorted far to near |
| 8 | 5 | outlines (section 5) | less-or-equal / on | alpha over | submission |
| 9 | 6 | stencil volumes: blob shadows, light pools (section 9.1) | see 9.1 | see 9.1 | submission |
| 10 | 7 | opaque, after volumes (2,020 meshes) | less / on | none | submission |
| 11 | 8 | cut-outs after volumes (3 meshes, Back Alley Heist) | as 6 and 7 | as 6 and 7 | as 6 and 7 |
| 12 | 9 | outlines after volumes | less-or-equal / on | alpha over | submission |
| 13 | 10 | masked areas, step 1 (kind 9) | less-or-equal / off | see 3.3 | submission |
| 14 | 11 | masked areas, step 2 (kinds 0 and 1, blend flags 10) | less / on, alpha test 0.1 | none | submission |
| 15 | 12 | masked areas, step 3 (kind 10) | less-or-equal / off | see 3.3 | submission |
| 16 | 13 | translucent surfaces (14,089 meshes), plus anything faded (6.3) | less / two-step | per shader (3.2) | sorted far to near |
| 17 | 14 | outlines of translucent meshes | less-or-equal / on | alpha over | submission |
| 18 | 15 | particles (effects.md 1) | less / off, no culling | per emitter | per emitter |
| 19 | 16 | screen overlay meshes (kind 7): the binocucom markers, OIDs 28 and 35 to 40, present in all 46 worlds | always / off | none | submission |
| 20 | 17 | map-screen pieces (kind 8, children of the class 103 object; 1,056 meshes in 44 worlds, 109 in The Hideout) | always / off | alpha over | submission |

Passes 10 to 12 occur only in Two to Tango, Straight to the Top, The King of the Hill and Duel by the Dragon [D];
what they show there is still to be observed (question 4).

**Sort key.** "Far to near" sorts by a key, largest first: the mesh's draw-order value `g × |g|` if the mesh has
one (geometry.md 3, flag 0x004), otherwise the squared distance from the camera to the mesh's bounding-sphere
centre. The sort is stable [R]. The draw-order values are therefore squared distances chosen by hand: the three
Paris sky meshes have 8000, 9000 and 9999, so they draw in the order 9999, 9000, 8000 [D].

### 3.2 Alpha tests and two-step drawing

- **Two-step** (passes 4, 8 and 13): each submesh is drawn twice. First with alpha test (keep pixels whose
  `vertexColour.a × S1.a`, or `× texel.a` for single-palette kinds, is at least 0.9) and depth writes on; then
  without the test and with depth writes off [R]. Both draws blend.
- **Blend per shader in pass 13**: blend flags 2 and 6 use alpha over (`src × a + dst × (1 − a)`); all other values
  are additive (`src × a + dst`) [R]. On the disc pass-13 submeshes use flags 2 (4,676), 3 (1,259), 6 (81) [D].
- In passes 4 and 8 the same split picks the list: flag 2 goes to the sorted alpha-over list, other flags to the
  additive list [R].
- Destination alpha is written with the source alpha (it matters only for passes 10 and 12).

### 3.3 Passes 10 and 12

[R] Pass 10 draws each mesh twice: first its back faces with the colour left unchanged and the destination alpha
set to the source alpha, then its front faces with alpha over. Pass 12 draws each mesh twice: first with
`src × (1 − dstAlpha) + dst × dstAlpha`, then with alpha over. Together they fill the inside of a closed volume
with a tint where the scene lies behind its back faces. [I]

### 3.4 Backdrops (pass 1)

[D] Only sky objects (class 84) use pass 1: 86 meshes in 38 worlds, all with shader kind 6, mesh flags 0x10. Each
has a draw-order value (9999, 8000, 9000, 80000, 90000 and 99990 are the common ones). The sky object follows the
camera (effects.md 6), so these meshes stay at a fixed offset from the camera. They draw first, with depth test and
depth writes off, so everything else covers them. Kind 6 is lit like kind 1 (2.1) [R]. The Paris night skyline
is the sky object OID 1550: three meshes, the first with a scrolling animation (kind 6) [D].

### 3.5 Overlays (passes 16 and 17)

Pass 16 and pass 17 meshes draw after the whole 3D scene with depth test and writes off, so they are never hidden.
Their positions come from their owners: the binocucom markers from the binocucom view (hud.md), the map-screen
pieces from the map object (ui-and-menus.md 6). Pass-17 meshes have mesh flags 0x14, so world darkening does not
touch them [D]. The reference presents the map at 4:3 with black bars on wider screens [R]. The Hideout's 109
pass-17 meshes are its wall pictures: they disappear when pass 17 is skipped [D] [I].

## 4. Fog

Per vertex, interpolated, then per pixel `colour = mix(colour, fogColour.rgb, fog)` [R]:

    z   = distance from the camera to the vertex
    t   = clamp((1/near − 1/z) / (1/near − 1/far), 0, 1)
    fog = clamp(t × cap × k × fogColour.a, 0, 1)

| Value | Source | Default [R] |
|---|---|---|
| near | option 532 | 500 |
| far | option 533 | 20000 |
| cap | option 534 | 0.5 |
| fogColour | option 535, packed colour, alpha × 2/255 | 0, 0, 0, 0 (no fog) |
| k | the mesh's fog factor (geometry.md 3, flag 0x008); 1 when absent. The reference uses 1 when the factor is below 0.001 | 1 |

Fog is reciprocal in distance (it rises fast just past `near`). Worlds without option 535 have no fog: The
Hideout, Last Call, Down Home Cooking, Flame Fu, Bentley Comes Through and Splash [D]. Fog factors on the disc: 0
(49 meshes), 0.6 (16), 0.3 (12), 1.0 (8), 0.7 (3), 0.8 (2) [D].

### 4.1 Fog per world [D]

Near / far / max / colour R G B A. "—" means not set; the default applies.

| World | Near | Far | Max | Colour |
|---|---:|---:|---:|---|
| Paris, France | 1000 | 30000 | 0.4 | 35 35 106 128 |
| A Stealthy Approach | 2500 | — | 0.4 | 80 152 132 128 |
| Prowling the Grounds | 2000 | — | 0.38 | 80 152 132 128 |
| High Class Heist | 3000 | — | 0.5 | 30 102 76 128 |
| Into the Machine | 1500 | — | 0.4 | 64 104 158 128 |
| A Cunning Disguise | 2000 | — | 0.5 | 30 102 76 128 |
| The Fire Down Below | 1000 | — | 0.45 | 144 91 70 128 |
| Treasure in the Depths | 3000 | — | 0.35 | 13 86 61 128 |
| The Gunboat Graveyard | 1350 | 8000 | 0.55 | 30 102 76 128 |
| The Eye of the Storm | 2000 | — | 0.28 | 179 162 67 128 |
| A Rocky Start | 4000 | — | 0.3 | 179 91 182 128 |
| Muggshot's Turf | 2000 | — | 0.3 | 86 64 135 128 |
| Boneyard Casino | 1300 | — | 0.3 | 109 70 182 128 |
| Murray's Big Gamble | 8000 | 13000 | 0.27 | 86 64 135 128 |
| At the Dog Track | 4000 | — | 0.3 | 86 64 135 128 |
| Two to Tango | 1800 | — | 0.3 | 86 64 135 128 |
| Straight to the Top | 1700 | — | 0.3 | 107 59 121 128 |
| Back Alley Heist | 1000 | 10000 | 0.3 | 86 64 135 128 |
| The Dread Swamp Path | 1800 | 30000 | 0.3 | 93 149 161 128 |
| The Swamp's Dark Center | 2000 | 30000 | 0.3 | 93 149 161 128 |
| The Lair of the Beast | 1500 | 30000 | 0.3 | 93 149 161 128 |
| A Grave Undertaking | 1800 | 30000 | 0.3 | 93 149 161 128 |
| Piranha Lake | 1800 | 30000 | 0.3 | 93 149 161 128 |
| Descent into Danger | 1600 | 4000 | 0.3 | 47 51 110 128 |
| A Ghastly Voyage | 1000 | 9000 | 0.4 | 93 149 161 128 |
| A Deadly Dance | 4500 | 30000 | 0.3 | 93 149 161 128 |
| A Perilous Ascent | 3500 | 300000 | 0.3 | 88 106 159 128 |
| Inside the Stronghold | 2500 | 30000 | 0.3 | 103 88 159 128 |
| Flaming Temple of Flame | 3000 | 30000 | 0.3 | 103 88 159 128 |
| The Unseen Foe | 2500 | 30000 | 0.3 | 103 88 159 128 |
| The King of the Hill | 10000 | 40000 | 0.3 | 103 88 159 128 |
| Rapid Fire Assault | 2500 | 30000 | 0.3 | 103 88 159 128 |
| Duel by the Dragon | 3200 | 50000 | 0.3 | 103 88 159 43 |
| A Desperate Race | 5500 | 100000 | 0.4 | 90 88 159 128 |
| A Hazardous Path | 4000 | — | 0.3 | 139 70 171 128 |
| Burning Rubber | 2000 | — | 0.4 | 158 70 70 128 |
| A Daring Rescue | 4000 | — | 0.3 | 65 132 157 128 |
| A Temporary Truce | 4500 | — | 0.3 | 139 70 171 128 |
| Sinking Peril | 6000 | — | 0.4 | 139 70 171 128 |
| A Strange Reunion | 4500 | — | 0.3 | 139 70 171 128 |

World light levels, options 526 / 527 by world number (worlds.md; "—" = not set, default 0 / 50) [D]: 0 —/—;
1 0/60; 2 0/45; 3 0/40; 4 0/40; 5 20/50; 6 0/50; 7 0/54; 8 0/50; 9 —/50; 10 —/60; 11 —/37; 12 —/40; 13 —/40;
14 0/50; 15 0/40; 16 15/30; 17 0/40; 18 10/40; 19 20/40; 20 10/40; 21 5/40; 22 5/40; 23 5/40; 24 5/35; 25 5/30;
26 0/25; 27 10/40; 28 10/40; 29 0/37; 30 0/30; 31 —/30; 32 —/30; 33 0/30; 34 —/40; 35 0/30; 36 0/30; 37 0/30;
38 0/30; 39 0/40; 40 10/40; 41 0/40; 42 —/—; 43 0/45; 44 0/40; 45 0/40.

**Test vectors** (Paris): z = 500, 1000, 2000, 5000, 30000, 60000 gives fog 0, 0, 0.2069, 0.3310, 0.4, 0.4.
Duel by the Dragon's colour alpha 43 scales fog by 0.3373.

## 5. Outlines (passes 5, 9, 14)

**Data** [D]: every mesh with pass 5 or 9 holds only outline parts (geometry.md 3.2). Each edge is four position
indices: **bytes 1 and 2 are the edge's end points** and bytes 0 and 3 are the far corners of the two triangles
that share it. Checked on 32,742 edges in three worlds: the end-point pairs repeat 41 times, any other pairing
repeats thousands of times, and bytes 0 and 3 are equal in 2 edges (open borders).

**Which edges draw** [R]: each frame, project the four points to the screen. The edge draws when both far corners
lie on the **same side** of the projected edge line (one adjacent triangle faces the camera and the other does not),
or when a corner lies on the line. An edge with any point behind the camera does not draw. Only silhouettes draw;
creases do not.

**How** [R]: a line between the projected end points, 4.125 pixels wide at the presentation resolution (scale with
output height; question 3). Colour: the object's option 95 when set (packed colour), else (16, 16, 16, 255) / 255.
Alpha = colour alpha × outline fade × object fade. Not darkened, not fogged. Depth test less-or-equal with depth
writes. The pixels are marked so stencil volumes (9.1) do not paint over them.

**Fade** [R]: outlines stop beyond the mesh's outline draw distance (geometry.md 3, flag 0x020). Within the last
400 units (scaled by the camera's distance ratio, option 536) the alpha falls with squared distance:
`(outer² − dist²) / (outer² − inner²)`, where `outer` = distance limit + bounding radius and
`inner = outer − 400`. Mesh draw distances (flag 0x010) fade the same way.

Outlines of a translucent or faded mesh go to pass 14, of a pass-7 or 8 mesh to pass 9, all others to pass 5 [R].

## 6. Per-mesh adjustments

### 6.1 Billboards

[D] The billboard byte is 1 on 3,982 meshes (mostly pass 13 on classes 4 and 2), 0 elsewhere. [R] For mode 1,
the mesh is rotated about its bounding-sphere centre so that:

1. its local X axis points along the camera's view axis toward the viewer (opposite the camera's forward
   direction, not toward the camera's position), by the shortest rotation from its current X axis;
2. then about that axis, so that its local Z axis (projected into the screen plane) lines up with the camera's up
   axis.

So the mesh is fully screen-aligned and does not rotate with its object.

### 6.2 View-angle fade

Meshes with flag 0x080 (geometry.md 3) carry a direction `v` and four numbers `c0..c3`. Their alpha is multiplied by
`clamp(c0 + x·(c1 + x·(c2 + x·c3)), 0, 1)` with `x = |v_world · cameraForward|` [R]. This is a glint that shows
only at some view angles.

### 6.3 Object fade

An object's alpha below 1 (fades, distance fade 5) moves its pass-3, 4, 7 and 8 meshes into pass 13 and its
outlines into pass 14, sorted like other translucent meshes [R].

## 7. Transparency summary

| Blend flags (shader) | Pass 13 | Passes 4, 8 | Pass 1 |
|---|---|---|---|
| 2 | alpha over | alpha over, sorted | alpha over, sorted |
| 6 | alpha over | additive | not sorted, no blend |
| any other (0, 1, 3, 10, 34 …) | additive | additive | not sorted, no blend |

Alpha over = `src × a + dst × (1 − a)`; additive = `src × a + dst`. Particles: additive when the emitter's
shader has blend-flag bit 0 set, otherwise alpha over drawn in two steps like 3.2 [R]. Stencil volumes: 9.1.

## 8. Texture animation

An animation record ([textures.md](textures.md) 2.4) in a shader animates that shader everywhere; one inside a
mesh (flag 0x040) animates the shader whose object id it names, for that mesh only, and takes priority [R]. The
current frame picks the bitmap and palettes of frame `n` of the texture record (clamped to the last frame); UV
offsets are added to every texture coordinate. "Random in [a, b]" is uniform.

| Kind | Motion | Parameters (24 bytes) | On the disc [D] |
|---:|---|---|---|
| 1 | Loop: frames advance at `frames / T` per second, T random in [loop min, loop max]; after a full loop, pause random in [pause min, max] and pick a new T. Starts with a pause | f32 loop min, loop max, pause min, pause max; s16 start frame; 6 bytes unused | 427 shaders, 22 meshes; e.g. 0.2..0.2 s for 3 frames, 0.7 s for 1..4 frames; start frame 3 on 18 |
| 2 | Back and forth: up through all frames then down, `2 × frames / T` per second; pause at frame 0 | f32 ×4 as kind 1; u16 start frame; 6 bytes unused | 56; 42 use 0.1..0.18 s |
| 3 | Random: after each pause (random in [min, max]) jump forward by a random 1..frames−1 frames | f32 pause min, max; 16 bytes unused | 22 shaders (0.001..0.05 s), 4 meshes |
| 4 | — | — | none |
| 5 | Blink: eyes open for random [open min, open max] s (or 0 s with the given chance, a double blink), then close and reopen through all frames in `blink` s; a second shader (by object id) follows the same frame | f32 blink, open min, open max, double-blink chance; s16 second shader id (−1 none); 6 bytes unused | 185 meshes (0.14, 1, 2, 0.1..0.4), 1 shader |
| 6 | Scroll: `u = (u + su × dt) mod wu`, `v = (v + sv × dt) mod wv` | f32 su, sv (UV per second), wu, wv; 8 bytes unused | 156 shaders, 517 meshes; e.g. (0.2, −0.1), (0, −1.667), (−1, 0); wraps 1 |
| 7 | Circle: offset = (sin(w t) × r + cu, cos(w t) × r + cv), t the game clock | f32 w (radians per second), r, cu, cv; 8 bytes unused | 2 shaders (0.071, 1), 4 meshes |
| 8 | Eye look: offset steers toward where the line from the eye's focus to the look target crosses the mesh, clamped to a box, smoothed | f32 centre u, v, u min, u max, v min, v max | 80 meshes, all (0.5, 0.5, 0.3, 0.7, 0.3, 0.7) |

Frame-based kinds do nothing when the texture has one frame. Frame timing uses the game clock (clock-and-loop.md).
Kind 8 needs the character's look target (animation.md); without it the offset is 0.

**Test vectors**: kind 6 with (0.2, −0.1, 1, 1) after 2.5 s: u offset 0.5, v offset −0.25 (the remainder keeps the
sign of the dividend). Kind 1 with 4 frames and T = 0.3: 13.33 frames per second.

## 9. Shadows

### 9.1 Blob shadows and other stencil volumes (pass 6, kind 4)

Every object with option 41 (radius) > 0 draws the shared shadow mesh (object OID 29, present in every world: one
pass-6 mesh, kind 4, blend flags 0, volume colour `00 00 00 34`) at the object's position, unrotated, scaled by
`(radius/100, radius/100, depth/100)` with depth = option 42 [R] [D].

A pass-6 mesh is a closed volume. Drawing [R]:

1. Back faces, colour writes off, depth test less: where the volume's back face is **behind** the scene, mark the
   pixel; elsewhere clear the mark.
2. Front faces, colour off: where the front face is also behind the scene, clear the mark. The marked pixels are
   those where scene surfaces lie inside the volume.
3. Back faces again with depth test off, only on marked pixels not covered by an outline: draw the shader's
   **second colour**. Blend flags 0 and 2: alpha over. Flags 1 and 3: additive (flag 3 also textured).

Blob-shadow test vector: volume alpha 0x34: (52 + 1) >> 1 = 26, 26/128 = 0.203, so the ground under the shadow
becomes 0.797 of its colour. On the disc, kind-4 blend flags: 0 (306), 1 (159), 3 (69), 2 (6) [D]. Additive
volumes such as `8a ca ff 48` are pools of light.

### 9.2 Projected textures (options 43 to 48)

A projector casts straight down (−Z) from the object (effects.md 7) [R]. Near/far radius 45/46 and casting
distance 47/48 build a perspective projection (orthographic when the two radii are equal) whose eye sits where the
cone lines meet. Each receiving pixel multiplies its colour by:

- `1 − m × a` for character silhouettes (the 64 × 64 texture drawn in list 1 with each vertex's alpha, outside the
  texture 0), or
- `1 + m × a` for fixed textures,

with `m` the texture alpha at the projected point and `a` the projector shader's material alpha.

**Range along the cast direction** [R]. Let w be a receiving point's distance from the projection's eye along −Z
(the eye is where the cone lines meet, above the object), w_near and w_far the casting distances measured from that
eye:

- w_near ≤ w ≤ w_far: the projection as built.
- **w > w_far** (farther from the caster than the far casting distance, i.e. below it): the point is projected as
  if it were on the far plane (the texture coordinates are clamped there). The shadow continues below at the size
  it has on the far plane; it does **not** fade.
- **w < w_near** (between the caster and the near casting distance, i.e. above it): clamped to the near plane in the
  same way; and if the point is **more than 50 units above the near plane** (w < w_near − 50) the effect is cut to
  nothing there. This is a cut, not a gradual fade.
- An orthographic projector (equal radii) does no clamping and no cut.

So the earlier sentence ("below the casting range it clamps; more than 50 units below it, it vanishes") had the
direction reversed: the 50-unit cut is above the near distance, toward the caster, and nothing below the far
distance fades. Receivers: meshes whose mesh-flags byte has bit
0x01 receive kind-2 projectors (shadows), bit 0x02 receive kind-3 projectors (light patterns) [R]; on the disc bit
0x01 is set on 33,027 meshes and bit 0x02 on 45,284 [D]. At most 4 projectors per submesh. Pixels with alpha 0.9
or less are not affected. The player's projector uses shader 347 (kind 2, one frame, colour `ff ff ff ff`) in
42 worlds [D].

## 10. Darkening

As effects.md 8. Darkening multiplies the final colour (before fog) of kinds 0 and 1 and of stencil volumes;
outlines and meshes with mesh-flag bit 0x04 are not darkened [R].

## 11. Data summary

Meshes by pass [D], all 46 worlds: 1: 86; 3: 42,348; 4: 1,408; 5: 2,398; 6: 580; 7: 2,020; 8: 3; 9: 875; 10: 3;
11: 254; 12: 4; 13: 14,089; 16: 322; 17: 1,056. Passes 0, 2, 14 and 15 never occur on meshes (they are produced at
run time).

Shader kind by mesh pass (submeshes) [D]: pass 1 → kind 6; 3 → 0, 1, 5; 4 → 0, 1; 6 → 4; 7 → 0, 1; 10 → 9;
11 → 0, 1 with blend flags 10; 12 → 10; 13 → 0, 1; 16 → 7; 17 → 8.

## 12. Open questions

1. Vertex and material colour scale: 0xFF = 1.0 (reference, used here) or 0x80 = 1.0 with brighter values
   overbright (the hardware's texture-modulate convention). Needs a side-by-side screenshot of a 7f7f7f material.
2. Outline colour (option 95) alpha: 28 values use 0x80, 22 use 0xFF. Under 0xFF = 1.0, half the outlines would be
   half transparent.
3. Outline width on the original: 4.125 pixels of the 448-line frame (then scale with height) or fixed pixels.
4. What passes 10 to 12 show in their four worlds.
5. Whether a mesh fog factor of 0 means "no fog" (the reference treats it as 1).
6. The silhouette alpha written into the character shadow texture, and so the darkness of the player's shadow.
   Known so far [D] [R]: the silhouette is drawn flat with each vertex's colour alpha; the projector's shader (347)
   has material alpha `0xFF`; the colour entries of the player's subtree in Paris are mostly `0xFF` (31 of 56;
   others 25, 78, 153, 154, 194). With alpha read as value / 255 the product m × a is 1 under most of the body,
   and with the hardware's 0x80 = 1.0 convention values ≥ 0x80 saturate to 1 as well, so neither scale by itself
   explains a lighter shadow: a port should not rescale the vertex alpha. If the shadow looks too dark next to the
   original, the likely cause is elsewhere (the silhouette's coverage or filtering at 64 × 64, or the receiver's
   multiply), and it needs a side-by-side capture.
7. Spot-light falloff details (2 lights on the disc).
