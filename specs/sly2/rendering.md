---
id: sly2-rendering
title: Sly 2 rendering - lighting, shader kinds, passes, fog
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; all 43 worlds; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73
    used_for: >-
      shader survey of all 38,750 shaders (kind, flags, colours, self-illumination, animation-record links); the
      animation-record classes; vertex-colour ranges and mesh type / shader kind / pass combinations (rows 0 and 3);
      the world option lists of all 43 worlds (fog, clip, shadow colour); the far placements of the intro hub;
      the lighting constants found in the executable's vector-unit code area (addresses below); every test vector
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/shd.cpp, saa.cpp, light.cpp, lightsw.cpp, render.cpp, glob.vert, glob.frag, sky.cpp
    used_for: >-
      the drawing state each shader kind and flag produces, the lighting computation (light records, static bake,
      four-light path, the two-colour decomposition), fog, the shadow and volume passes, texture-animation
      behaviour, the pass order and sorting; marked [R]
  - kind: public-doc
    what: PS2 GS documentation (TEST, ALPHA, TEX0 texture function MODULATE, FOG and FOGCOL registers)
    used_for: decoding the drawing state into blend, alpha-test and depth formulas
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), nonmatching/new_167320/LoadGlobFromBrx.cpp (the rigid-mesh loader's colour set-up) and src/data/new_167320.cpp
    used_for: which meshes are lit and which get the unlit colour (2.3, 2.5); the bake-everything switch; marked [R]
  - kind: disc
    what: re-check 2026-11-18 of rows 0, 2 and 3 (mesh-body flag bit 0, submesh flag 0x20, shader kind, pass, vertex and material colours, sky palettes) and executable 0x2BE580
    used_for: sections 2.3 and 2.5 [D]
reviewed_by: pending
date: 2026-11-18
---

# Sly 2 rendering

How the game turns the data of [textures.md](textures.md), [geometry.md](geometry.md) and [effects.md](effects.md)
into pixels. It answers the viewer's open questions (section 0) and corrects two fields of textures.md 2.4 (bytes
20–21 and flag `0x40`; changed there).

Tags as everywhere: [D] disc data, [O] observable, [R] the reference's reconstruction of the game's drawing, to be
confirmed by comparing frames with the game in an emulator, [I] inferred.

## 0. Answers in brief

| # | Question | Answer |
|---:|---|---|
| 1 | Lighting; palette slots of kinds 1 and 7 | Each vertex gets two computed colours A and B from the lights (section 2). Kind 1 draws the grey palette (slot 1) modulated by A, then **adds** the coloured palette (slot 0) modulated by B (section 3.2). Kind 7 adds a first pass on slot 2 that only writes the frame's alpha (3.3). [R] |
| 2 | Grey filter | A colour adjustment applied to palettes, shader colours and light colours at load when a global switch is on. The switch is **off** in the retail game, so nothing changes (section 7). [R] |
| 3 | Texture frames | Chosen by the shader's animation record (the node reference after the 24-byte header), not by flag `0x40` and not by bytes 20–21, which are self-illumination (section 4). [D] [R] |
| 4 | Passes; sky | Section 5: 1 sky, 2 background, 3 static opaque, 4 moving opaque, 5 cel scenery, 7 / 8 shadow volumes and hulls, 9 ink outlines, 10 translucent, 11 / 13 projected volumes. The sky follows the camera horizontally (effects.md 3). [R] [D for the combinations] |
| 5 | Kinds 2, 3, 6; flags `0x08`, `0x02` | Kind 2 = darkening shadow hulls in a stencil phase; flag `0x08` turns off the destination-alpha mask. Kind 3 = projected volumes; flag `0x02` = the volume also draws its texture. Kind 6 = like kind 0 but without depth test or depth writes, drawn in pass 2 (sections 3, 5). [R] |
| 6 | Material colour scale | 0xFF = 1.0 for colour (multiplied in as /256), alpha halved onto the 0x80 = 1.0 scale (section 8). [R] |
| 7 | Clear colour, fog, darkening | The frame is cleared to black; fog comes from world options 166–169 (table in section 6); there is no per-world darkening option, only the shadow phase's fixed ramps. [D] [R] |
| 8 | Stray objects in the intro hub | They are 201 mesh copies of the level's own geometry object (object 1), placed 60,649 to 90,827 units west of the origin; the game simply never brings the camera near them (section 9). [D] [I] |
| 10 | Sky and background (kinds 4, 5, 6) | They are **not lit**: on static meshes every shader kind except 1 and 7 gets the unlit colour of 2.5, `A = v × m / 256` quantised to 5 bits, and no colour B. The sky in row 0 (white vertices, white material) therefore draws at about 1.94× its texels, which are authored at half brightness (palette channels at most 98–128). Reading these kinds' "colour A" as the lit colour of 2.4 was a misreading; drawing them unlit as the renderer does now is right, provided the 2.5 colour is used rather than 1.0 (section 2.5). [R] [D] |
| 9 | Vertex colours above 0x80 | The colours sent to the GS are the computed A and B, not the stored vertex colours; they reach 0xFF and **do** brighten the texel up to 2× (clamped at 255) (section 8). [R] |

## 1. GS conventions used below

- Colours that reach the GS are bytes with **0x80 = 1.0**. The texture function is modulate: for each channel
  `out = ⌊texel × colour / 128⌋`, clamped to 0..255; RGB and alpha alike. So colour 0xFF gives nearly 2× the texel.
  [public doc]
- Blending: `out = (A − B) × C / 128 + D`, with A, B, D each the source colour `Cs`, the frame colour `Cd` or 0,
  and C the source alpha `As`, the frame alpha `Ad` or a fixed value. Only RGB blends; the alpha written is `As`.
  [public doc]
- Depth: larger Z is nearer; "greater" lets a pixel through when it is nearer than the stored one. [public doc]
- Alpha test: on pass the pixel writes everything; on fail it follows the failed-pixel rule: frame only (colour
  and alpha, no depth), or colour only (RGB, no alpha, no depth). [public doc]
- Fog: each vertex carries a fog value; the GS mixes the pixel toward the fog colour by it (section 6). [public doc]

## 2. Lighting

### 2.1 Light records

Every class-4 object with option 151 = 0 (directional) or 1 (point) is a light ([effects.md](effects.md) 2). Its
options become a record [R]:

- `g = max(V, S)`, with V the value of the HSV colour (option 152, 0..255) and S the strength (option 153).
- Colour `c = (RGB(HSV), S) / g` on the 0..255 scale (so the fourth channel is `S / g`). [R]
- Angle term: with `θ` the cone angle (option 154, at least 30°), `x(d) = d + d³`, a straight line `o + k·x` is
  fitted through `x = cos(θ/2) + cos³(θ/2)` → 0 and `x = 2` → `g / 255`; the record keeps the offset `o` and the
  slope `−k` as its "angular scale". For a surface with unit normal `n` and light direction `ℓ` (from the light toward
  the surface), the angular factor is `clamp(o + (d + d³)·s, 0, 1)` with `d = n · ℓ` and `s` the angular scale; a
  surface that faces the light (`d = −1`) gets exactly `g / 255`. [R]
- Directional light: `ℓ` is the light's transform's first axis. Point light: `ℓ = normalise(p − L)` and a distance
  factor `clamp(a + b·|p − L|, 0, 1)`, the line through (near, 1) and (far, 0) of option 155. [R]
- If the world has no light usable for the static bake (option 158 off on none), a default directional light is used:
  `ℓ = normalise(−0.2, 0.3, −1)`, strength 255, angle 240, colour black. [R]

### 2.2 World shadow colour

World options 914 (HSV), 916 (scale) and 915 (amount) give `W = (RGB(HSV) × 916 / 255, 915 / 255)` on the 0..1 scale
([effects.md](effects.md) 2.1). It colours the unlit part of every lit surface (2.4). [R]

### 2.3 Two ways to light a mesh

Which meshes are lit at all is decided per submesh at load, from the mesh body's flag byte bit 0 (rigid bodies,
geometry.md) and the submesh's shader kind and flags [R]:

| Mesh body flag bit 0 | Shader kind | Submesh flag `0x20` | Colours |
|---|---|---|---|
| 1 (moving) | any | — | lit **per frame** (below) |
| 0 | 1 or 7 | clear | lit by the **static bake** (below) |
| 0 | 1 or 7 | set | taken from the mesh's pre-baked colour table (2.5), not lit |
| 0 | 0, 2, 3, 4, 5, 6 | — | the **unlit** colour of 2.5, not lit |

A global switch (executable `0x002BE580`, 0 on the disc [D]) would also bake submeshes with flag `0x20`; it is off,
so the table colours stand [R]. Bit 0 does not follow the render pass: in rows 0 and 3, kind-1 submeshes on meshes
with bit 0 set sit in passes 4 (1,696), 3 (444) and 10 (286) [D].

- **Static bake** [R]: the bit-0-clear meshes above are lit once, by **every** light whose option 158 is off,
  skipping point lights whose range does not reach the mesh's bounding sphere. Cap `κ = 1`. The result is stored
  in the vertices and recomputed only when a contributing light or the shadow colour changes. The two colours are
  quantised to 5 bits per channel (the low 3 bits cleared).
- **Per frame** [R]: meshes with body flag bit 0 set (moving, animated or skinned) are lit each frame by the **four
  strongest** lights at the object's centre (strength `g` times the distance factor at the centre, strongest first;
  lights with option 157 on are not candidates; empty slots are a black light). Cap `κ = 2`. Point-light distance
  factors are taken once at the object's centre. Light directions are moved into the object's own space; the
  vertex normal is used as stored, scaled by 127 and truncated to integers.

### 2.4 Per vertex: accumulate, then split into colours A and B

For a vertex with stored colour `v` (bytes, 0xFF = 1.0, [geometry.md](geometry.md) 4) on a submesh whose shader has
material colour `m` (textures.md 2.4) [R]:

1. **Base colour** `b`: `b_c = ⌈v_c × m_c / 256⌉` for R, G, B (per frame: `⌊(v_c m_c + 255) / 512⌋`), and
   `b_α = ⌊(v_α m_α + 255) / 510⌋`, which puts alpha on the 0x80 scale.
2. **Accumulate** `L = (L_r, L_g, L_b, L_w)`: start at `(0, 0, 0, σ + W_α)`, where `σ` is the shader's
   self-illumination (bytes 20–21 / 4096), then add `c × angular × distance` for each light. The per-frame path works
   at **twice** the scale, which is what "doubled" means [R]:
   - For each of the four lights `k`, with colour `c_k`, angular offset `o_k`, angular scale `s_k` (2.1) and, for a
     point light, distance factor `δ_k` taken once at the object's centre (`δ_k = 1` for a directional light):
     `o'_k = o_k δ_k`, `s'_k = s_k δ_k`.
   - **Constant term** `C = 2 × Σ_k c_k o'_k` (the four lights' offsets, summed, then doubled).
   - **Response term** of light `k` at a vertex: `2 c_k s'_k × min(x_k, −o'_k / s'_k)`, with
     `x_k = d_k + d_k³` and `d_k` the dot product of the light direction (moved into the object's space) with the
     vertex normal scaled by 127, truncated, then by 1/127; the cap `−o'_k / s'_k` is where the line crosses zero
     (no cap when `s'_k = 0`).
   - So `L_rgb = C + Σ_k 2 c_k s'_k min(x_k, −o'_k/s'_k)` = `Σ_k 2 c_k × max(o'_k + s'_k x_k, 0)`: twice the bake's
     angular factor, floored at 0 but **not** clamped at 1. The fourth channel gets the same sum plus `σ + W_α`.
   - The doubling matches the rest of the per-frame path: the cap `κ = 2`, `ε = 2/127`, `σ_A = 63.5` (step 3), the
     base colour `/ 512` instead of `/ 256` (step 1) and `I` on a 0..127 scale; the outputs A and B land on the same
     0x80 = 1.0 scale as the bake's.
3. **Split** with cap `κ` (1 or 2):
   - `M = max(L_r, L_g, L_b)`, `r = max(κ − M, 0)` (headroom), each of `L_r, L_g, L_b` clamped to `κ`;
   - mid-tone `μ = min(L_w, r)`, shadow `ς = r − μ`;
   - `β = (κ + ε − ς) × σ_A` with `ε = 1/127` (bake) or `2/127` (per frame) and `σ_A ≈ 127` (bake) or `63.5`
     (per frame) — the four constants are in the executable at `0x25E130`, `0x25E588`, `0x25F758`, `0x25F760`
     (values 0.0078741, 126.999, 0.0157482, 63.4995) [D for the constants' presence, R for their use];
   - **colour A**: `A_c = (L_c + W_c × ς) × I` for R, G, B, where `I` is the base colour's largest channel (bake) or
     that channel ÷ 255 × 127, truncated (per frame); `A_α` = the vertex's own alpha on the 0x80 scale;
   - **colour B**: `B_c = b_c × μ × (b_α / β)`, `B_α = β`.

   All clamped to 0..255 and truncated. [R]

In words: A carries the light's colour (and the shadow colour on the unlit side) at the brightness of the surface;
B carries the surface's own colour in proportion to the mid-tone light, with an alpha that grows as the shadow
shrinks. A light whose colour has value 0 (for example light 7430, section 10) adds only to `L_w`: it brightens the
surface's own colour without tinting it.

### 2.5 Unlit colour and pre-baked colour tables

**Unlit colour A** [R]: for a static submesh whose shader kind is not 1 or 7, at load, per vertex with stored colour
`v` and material colour `m` (0xFF = 1.0 for both):

- `A_c = ⌊(v_c m_c + 255) / 256⌋` for R, G, B, then quantised to 5 bits (the low 3 bits cleared, as the stored 16-bit
  colour expands back to 8 bits);
- `A_α = ⌊(v_α m_α + 255) / 510⌋` (the 0x80 = 1.0 scale); its top bit is the 16-bit colour's alpha bit, and the full
  value is kept separately for meshes that need it.

No light, shadow colour or self-illumination enters. The GS then draws `texel × A / 128` per channel (section 8), so
`A` near 0xF8 doubles the texel. Example (row 0, the sky dome `jb_skydome`, kind 4) [D]: vertex colours
`ff ff ff ff`, material `ff ff ff ff`, so `A_rgb = 255 → 248` and the sky draws at 248/128 ≈ 1.94× its texels; its
palettes peak at 98 (mean RGB about 57, 33, 15), and the nightclub hub's sky `f_skydome` peaks at 100 to 128, i.e.
both are authored for this doubling. Drawing these kinds with colour 1.0 would leave the sky at half brightness;
drawing them with the shadow colour, as a literal reading of the old text did, is wrong.

**Pre-baked colour tables** [R]: a static kind-1 or kind-7 submesh with flag `0x20` takes both colours from the mesh's
vertex-map table ([geometry.md](geometry.md) 4.5): a list of colour sets, each a list of 5-byte records (`u16` colour
A as 5-5-5-1 bits, `u16` colour B as 5-5-5-1, `u8` B's alpha) indexed by the source vertex, then a list giving the
set for each submesh. B's alpha replaces the computed `β`. In rows 0 and 3 this applies to 4,982 of the 6,488
static kind-1 submeshes surveyed [D].

## 3. Shader kinds and their passes

For every kind: the pass's texture is frame `f` of the texture record and palette slot `k`, i.e. palette index
`f × (palettes per frame) + k` (textures.md 4). "Colour A / B" are section 2's; "constant" is a flat colour.

### 3.1 Kinds 0, 4, 5 and 6: one pass

Texture slot 0, colour A: on static meshes (body flag bit 0 clear) this is the **unlit** colour of 2.5, not the lit
colour of 2.4; on per-frame meshes it is 2.4's colour A [R]. The state follows the shader flags (textures.md 2.4) [R]:

| Flags | Alpha test | Blend | Depth |
|---|---|---|---|
| `0x20` | never passes: every pixel writes colour by the failed-pixel rule, no blend | `Cs` | tested, never written |
| `0x10` (no `0x20`) | as below | `(Cd − Cs)·As + Cs`; with `0x01`: `Cd·As + Cs` | as below |
| `0x80` (no `0x20`, `0x10`) | none; destination-alpha mask on (pixels whose frame alpha bit 7 is clear) | `(Cs − Cd)·Ad + Cd` | tested "greater" |
| none of these | as below | `(Cs − Cd)·As + Cd`; with `0x01`: `Cs·As + Cd` (additive) | as below |

"As below": kind 0 passes the alpha test at texel-times-colour alpha ≥ `0x7E` and tests depth "greater"; kinds 4, 5
and 6 never pass the alpha test and test depth "always". Passing pixels write depth; failing pixels write colour
(and frame alpha unless flag `0x04`) but no depth. So kind 0 is an opaque cut-out whose translucent texels blend
over without hiding what is behind them, and kinds 4 and 6 draw with no depth test at all. [R]

### 3.2 Kind 1: two passes (most scenery)

| Pass | Texture slot | Colour | Alpha test / depth | Blend | Fog colour |
|---|---|---|---|---|---|
| 1 | 1 (the grey palette) | A | as kind 0 (≥ `0x7E` passes, depth "greater"; failed pixels colour only with `0x04`) | as kind 0's table, including flags `0x01` and `0x80` | the world's |
| 2 | 0 (the coloured palette) | B | never passes: colour without depth; depth tested "greater or equal" (the same surface) | `Cs·As + Cd` (additive); with flag `0x80`: `Cs·Ad + Cd`, masked by destination alpha | **black** |

[R] The result on a lit, opaque surface is therefore

`pixel = grey(t) × A / 128 + colour(t) × B × (t_α × B_α / 128) / (128 × 128)` (per channel, each term clamped),

with `t` the texel, and the second term fading out in fog because its fog colour is black. Slot 1 is pure grey on
the disc (textures.md 4), so A sets the hue of the lighting and B brings back the texture's own colour.

### 3.3 Kind 7: three (or two) passes (cel scenery, pass 5)

| Pass | Present | Texture slot | Colour | State | Blend | Fog colour |
|---|---|---|---|---|---|---|
| 1 | only when flag `0x20` is clear (3 palettes per frame) | 2 | B | point sampling; alpha test "greater than 0" (keep on fail); depth "greater or equal" | leaves the colour unchanged (`(Cs − Cs)·As + Cd`); writes the frame's **alpha** | black |
| 2 | always | 1 | A | colour only (RGB), no depth writes; depth "greater or equal" | as kind 0's table | the world's |
| 3 | always | 0 | B | as pass 2 | additive `Cs·As + Cd` | black |

[R] Pass 1 marks, in the frame's alpha channel, where the cel shading's third palette has coverage; it draws no
colour.

### 3.4 Kind 2: shadow hulls (passes 7 and 8)

Untextured, no colour of its own beyond the material colour's alpha (`0x30` to `0x36` on the disc, the darkening
strength on the 0x80 scale) [D]. Drawn inside a **shadow phase** [R]:

1. At the phase's start the game keeps the frame's alpha and resets a mark channel.
2. Every hull counts, per pixel, the hull faces in front of the scene: +1 for a face toward the camera, −1 for one
   away (a closed hull leaves 1 inside, 0 outside). Faces cut by the near plane are counted with the depth test
   "always". Without flag `0x08`, the count is masked by the destination alpha (only pixels whose frame alpha bit 7
   was clear at the start); flag `0x08` drops that mask. [R]
3. At the phase's end the marked pixels are darkened toward black by fixed ramps: 128 (pass 7 marks), 96 (pass 8
   marks), 160 (both), on the 0x80 scale; the reference reads these from the game state and finds the same values in
   every world. [R]

Pass 7 holds the streamed meshes (body type 2, whose records are edges; extruded each frame from their silhouette away from the light into a
shadow volume) and indexed hulls (body type 3); pass 8 only indexed hulls. [D for the combinations: streamed body +
kind 2 + pass 7; type 3 + kind 2 + passes 7 and 8] [R for the extrusion]

### 3.5 Kind 3: projected volumes (passes 11 and 13)

Constant colour `(second colour + 1) / 2` (from 0xFF to 0x80 scale), blended `(Cs − Cd)·As + Cd`, additive with flag
`0x01` [R]:

- **Untextured** (flag `0x02` clear): the volume fills **the scene inside it**: a mask is set where the scene is in
  front of a back face of the volume and cleared where it is in front of a front face; the remaining pixels are
  filled once with the constant colour, without depth test (the camera may be inside). [R]
- **Textured** (flag `0x02` set): the volume's front faces in front of the scene are filled once each with the
  constant colour faded by the vertex alpha, then the volume's surface is drawn again textured (slot 0, colour A,
  blended, depth "greater or equal", no depth writes). Light shafts. [R]

206 kind-3 shaders have a second colour that differs from the first [D].

## 4. Texture frames and UV animation

A shader's animation is the record named by its node reference (textures.md 2.3), **not** flag `0x40` [D]: 608
shaders have a reference, and only 18 of them have flag `0x40`. Bytes 20–21 are the self-illumination of 2.4 [R]
(textures.md corrected). Frame 0 and no UV offset is right for any shader without a record.

A record is the class index (`u16`) and 32 bytes: target shader id (`s32`, +0), flags (`u32`, +4), `u16` (+8), frame
count (`u16`, +10), four `f32` parameters (+12), then a `u16` or `s32` (+28) [R]. Behaviour by class [R]:

| Class | Count [D] | What it does | Parameters |
|---:|---:|---|---|
| 139 | 243 | loops through the frames at a random rate, pausing between loops | +12/+16 loop time range (s), +20/+24 pause range, +28 first frame |
| 140 | 14 | ping-pongs through the frames, same parameters (twice the rate) | as 139 |
| 141 | 25 | shows a random other frame after each pause | +12/+16 pause range |
| 142 | 152 | eyes: open, blink (closing and reopening through the frames), sometimes twice; scripts can force them shut | +12 blink time, +16/+20 open-time range, +24 double-blink chance; +28 the id of a second shader driven the same way |
| 143 | 166 | scrolls the UVs: offset += speed × master speed × dt, wrapped | +12/+16 U/V speed, +20/+24 U/V wrap (0: none) |
| 144 | 8 | moves the UVs on a circle: `(sin(ωt)·r + u₀, cos(ωt)·r + v₀)` | +12 ω, +16 r, +20 u₀, +24 v₀ |

UV offsets are kept in 1/4096 units, modulo 1, and added to every texture coordinate of the shader's submeshes. The
texture record's two "zero" bytes (+4) are clamp flags: bit 0 clamps U, bit 1 clamps V; otherwise the texture repeats
[R].

## 5. Render passes and their order

Each mesh's render pass ([geometry.md](geometry.md) 3.1) puts it in a list; lists are drawn in increasing pass
order. Passes 1, 2, 5, 6 and 10 are sorted **far to near** by distance from the camera; the others are drawn in
record order. An opaque mesh (pass 3 or 4) whose object is fading (object alpha below 1) is moved to pass 5 or 6 and
blended. [R]

| Pass | What it draws [D for the combinations, R for the role] |
|---:|---|
| 1 | the sky dome (kind-4 shaders, 62): no depth test, no depth writes; moved each frame to the camera's horizontal position (effects.md 3) |
| 2 | background layers (kind-6 shaders, 141): drawn after the sky, no depth test, no depth writes |
| 3 | static opaque scenery, statically lit (kinds 0 and 1) |
| 4 | moving opaque objects, lit per frame (kinds 0 and 1) |
| 5 | cel-shaded scenery (kind 7), sorted; also faded pass-3 meshes |
| 6 | faded pass-4 meshes (not stored on the disc) |
| 7 | shadow volumes and hulls (kind 2), section 3.4 |
| 8 | indexed shadow hulls (kind 2), section 3.4 |
| 9 | ink outlines (body type 1, the cel-border edges): drawn after the scenery |
| 10 | translucent meshes (kinds 0 and 1, blended), sorted far to near |
| 11 | projected volumes (kind 3), section 3.5 |
| 13 | projected volumes (kind 3), section 3.5; the untextured ("murk") kind |

The frame is cleared to black, colour and depth, before pass 1 [R].

## 6. Fog, clear colour and darkening per world

- **Fog** [R]: with start distance `s` (option 166), end `e` (167) and maximum `F` (169), a vertex at camera depth `d`
  gets fog amount `f(d) = F × (1 − s/d) / (1 − s/e)` for `d > s` (0 nearer), clamped to 0..1; it equals `F` at `e` and
  keeps rising slowly beyond. The pixel becomes `mix(colour, fog colour, f)`. Fog colour is option 168, a packed
  colour with R in the low byte (`0x8034677C` = R 0x7C, G 0x67, B 0x34); additive passes use black instead. Defaults
  when a world lacks the options: 500 / 20,000 / 0.5, colour `0x80303030` ([camera.md](camera.md) 1). Meshes may scale
  their fog by a per-mesh factor [R] (one of the unknown header floats of geometry.md 3.1 [I]).
- **Clear colour**: black for every world; no option sets it [R] [D that no world option names a clear colour].
- **Darkening**: no per-world darkening option exists [D]; shadows darken through 3.4's fixed ramps [R].

Per-world values [D] (rows without an entry use the defaults; row 18 has no world):

| Row | Near / far clip | Fog start / end / max | Fog colour | Shadow HSV, amount, scale (914 / 915 / 916) |
|---:|---|---|---|---|
| 0 | 52 / 62,000 | 3,500 / 30,000 / 0.3 | `80563989` | (250, 200, 120), 30, 120 |
| 1 | 10 / 1,000 | — | — | (230, 260, 250), 0, 95 |
| 2 | 52 / 52,000 | 3,000 / 30,000 / 0.3 | `805C3D2A` | (220, 185, 185), 0, 120 |
| 3 | 25 / 25,000 | 4,000 / 30,000 / 0.5 | `8034677C` | (260, 180, 180), 10, 110 |
| 4 | 100 / 60,000 | 4,000 / — / 0.35 | `80471E29` | (250, 180, 180), 10, 80 |
| 8 | — | 2,800 / 30,000 / 0.25 | `80B0429A` | (45, 150, 80), 20, 80 |
| 12 | 75 / 75,000 | 4,000 / 150,000 / 0.3 | `807A5D2A` | (210, 170, 160), 0, 115 |
| 14 | — | 1,500 / 30,000 / 0.35 | `800F0F25` | (215, 150, 160), 0, 105 |
| 27 | 70 / 70,000 | 3,200 / 5,500 / 0.3 | `80937136` | (223, 155, 220), 0, 130 |
| 38 | 52 / 52,000 | 6,500 / 300,000 / 0.3 | `80862C7F` | (220, 175, 160), 0, 130 |
| 43 | 52 / 100,000 | 3,000 / 30,000 / 0.3 | `80772B54` | (220, 180, 160), 0, 120 |

(The full list for all 43 worlds is in the scratch survey; the values above are representative. The world option
lists of world-file.md 3.7 hold them.)

## 7. The grey filter (colour adjustment)

At load the game can pass every palette colour, the three shader colours and every light colour through a colour
adjustment: the colour's luminance is raised to a power and limited, then converted back through HSV. It is applied
only when a global switch is on; the switch is off in the retail game (the reference reads it as 0 in the game's
memory after loading a world) [R]. A port should apply no adjustment.

## 8. Colour scales

- **Material colour** (shader bytes 8–11): 0xFF = 1.0. It multiplies the vertex colour as `v × m / 256` for R, G, B;
  its alpha multiplies the vertex alpha as `v_α × m_α / 510`, which converts to the 0x80 scale [R]. Textured
  shaders hold `ff ff ff ff`; untextured parts use a placeholder texture and take their colour from here (for
  example `7f 7f 7f ff`, `9d 12 00 ff`) [D]. Kind 2 holds black with alpha `0x30`–`0x36`, used directly as the
  darkening strength [D]. Kind 3's second colour is converted as `(c + 1) / 2` to the 0x80 scale [R].
- **Stored vertex colours** are lighting inputs on the 0xFF = 1.0 scale: in rows 0 and 3, 210,486 of 220,909 have a
  largest RGB channel of 224 or more [D]. They never reach the GS directly.
- **Colours sent to the GS** (A and B of section 2) are on the 0x80 = 1.0 scale and range up to 255, so a brightly
  lit surface shows its texture at up to 2× (clamped per channel at 255). A port must keep that headroom: compute
  `texel × colour / 128` and clamp, not clamp the colour to 1.0 first. [R] [public doc]

## 9. The intro hub's far geometry

In row 0 (the intro hub), the level package's own geometry object (class 49, object 1) places 201 mesh copies at x
from −90,827 to −60,649 (y −6,200 to 16,690, z 0 to 11,763), next to 5,072 copies around the play area [D]. They are
not from a kept prefab package and not marked for removal at level start [D]. All have unlimited draw distance [D].
The game does not hide them by any flag found in the data; they are simply far from anywhere the camera goes in that
world (the world's far clip is 62,000) [I]. A free camera will see them; the engine should draw them like any other
mesh. *Which scene they serve is open.*

## 10. Test vectors

- **Light 7430** (row 3; kind 0, HSV (50, 150, 0), strength 175, angle 240, option 158 on): `g = 175`, colour
  `c = (0, 0, 0, 1)`, angular offset `o = 0.163399`, angular scale `−0.261438`; a surface facing it gets
  `0.163399 + 2 × 0.261438 = 0.686275 = 175 / 255`. Not used by the static bake (158 on); a per-frame candidate.
- **World shadow colour, row 3**: HSV (260, 180, 180), scale 110, amount 10 → `W = (0.161205, 0.089558, 0.304498,
  0.039216)`.
- **Fog, row 3** (4,000 / 30,000 / 0.5): `f(4,000) = 0`, `f(10,000) = 0.346154`, `f(30,000) = 0.5`,
  `f(60,000) = 0.538462`; fog colour R 124, G 103, B 52.
- **Shader survey** [D]: kind/flag pairs (count): (1, `0x02`) 23,653; (1, `0x42`) 4,364; (1, `0x43`) 4,320;
  (0, `0x02`) 2,870; (2, `0x02`) 1,132; (3, `0x01`) 571; (0, `0x42`) 412; (1, `0x03`) 244; (3, `0x09`) 216;
  (2, `0x08`) 189. Self-illumination: 0 on 30,816; most common non-zero values 1,044 (1,085 shaders), 562 (831),
  1,606 (659). Bytes 6/7: (1, 0) on every kind except 4, which has (3, 0).
- **Mesh combinations, rows 0 and 3** [D] (body type, shader kind, pass: submeshes): (rigid, 1, 3) 6,246;
  (rigid, 1, 4) 1,934; (rigid, 1, 10) 734; (rigid, 0, 10) 428; (streamed, 2, 7) 294; (indexed, 2, 8) 255;
  (indexed, 2, 7) 189; (rigid, 3, 13) 178; (rigid, 0, 3) 118; (rigid, 3, 11) 84; (rigid, 0, 4) 60; (rigid, 7, 5) 36;
  (rigid, 4, 1) 8.

## 11. Open questions

1. Every [R] rule above needs a frame comparison against the game in an emulator; the two-colour split (2.4) and the
   kind-1 combination (3.2) matter most.
2. What flag `0x40` means (it changes no drawing state in the reference) and flag `0x02` on kinds other than 3.
3. The exact colour-adjustment formula of section 7 (unused in retail).
4. The per-mesh fog factor's field, and the other unknown mesh-header values.
5. What the intro hub's far geometry is for (section 9).
6. Kind-7 pass 1's alpha marks: which later pass reads them.
7. The ramp values of 3.4 are read from game memory by the reference, not from the data; where the game sets them.
