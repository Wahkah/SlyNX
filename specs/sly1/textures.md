---
id: sly1-formats-textures
title: Sly 1 textures, palettes and shaders
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: >-
      table layouts walked on all 46 worlds; every in-range (bitmap, palette) pair decoded on all 46 worlds
      (counts in section 2.5; palette alpha never above 0x80); value surveys of every field; decoded
      images viewed to confirm the index permutation, the row order and the absence of block swizzling;
      all test vectors below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), native world-file walker (texture and shader tables) and runtime shader headers
    used_for: >-
      first reading of the record layouts, the three-palettes-per-frame rule and the 8-bit index permutation;
      every layout re-walked and every rule re-checked on the disc data
  - kind: public-doc
    what: Sony "GS User's Manual" (PS2 Graphics Synthesizer), pixel storage formats PSMCT32, PSMT8, PSMT4, CLUT storage mode CSM1, alpha blending
    used_for: format codes 0x00/0x13/0x14, the CSM1 entry order, the 0x80 = 1.0 alpha convention
reviewed_by: pending
date: 2026-11-14
---

# Sly 1 textures, palettes and shaders

Everything needed to turn the material tables of a world file into RGBA8 images and to know which image each
submesh uses. Positions of the tables in the file: [world-file.md](world-file.md) section 4.6; the texel data is
in the texture payload that starts at offset **T** ([world-file.md](world-file.md) 4.8). Little-endian, packed, no
alignment.

Terms used here (chosen for this spec):

- **palette**: a colour look-up table (the PS2 "CLUT");
- **bitmap**: a rectangle of texels, usually palette indices;
- **shader**: a material record. Submeshes name a shader by its index; the shader names a bitmap and palette(s);
- **zone mask**: a 32-bit visibility-zone bit set used by streaming and culling. Not needed to draw.

## 1. Pixel formats

The bitmap table stores a GS pixel-format code. Three occur:

| Code | GS name | Bits per texel | Texel meaning | Count over all 46 worlds |
|---|---|---:|---|---:|
| `0x14` | PSMT4 | 4 | Palette index 0..15 | 11,003 bitmaps |
| `0x13` | PSMT8 | 8 | Palette index 0..255 | 2,570 bitmaps |
| `0x00` | PSMCT32 | 32 | Direct colour, bytes R, G, B, A | 41 bitmaps, always 64 x 64 |

For every bitmap, `size = width * height * bits / 8` exactly. **No mip levels are stored**: the data is the base
level only. A port that wants mipmaps must generate them.

## 2. Tables

### 2.1 Palette table

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Palette count |
| 12 each | | Palette entries |

Palette entry (12 bytes):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | Zone mask |
| 4 | 2 | u16 | **Colour count** `n` (1..256) |
| 6 | 2 | u16 | GS upload size class: 1 when `n <= 16`, 2 when 17..128, 4 when 129..256. Not needed to decode |
| 8 | 4 | u32 | **Offset of the colours, relative to T** |

The colours are `n` consecutive 4-byte entries **R, G, B, A** (one byte each). Palettes are packed one after another
(`n * 4` bytes apart). Do not read `n * 4 * (size class)` bytes: that over-reads.

### 2.2 Bitmap table

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Bitmap count |
| 20 each | | Bitmap entries |

Bitmap entry (20 bytes):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 2 | u16 | Width in texels (8..1024 seen; powers of two) |
| 2 | 2 | u16 | Height in texels |
| 4 | 4 | u32 | Zone mask |
| 8 | 1 | u8 | Pixel-format code (section 1) |
| 9 | 1 | u8 | GS buffer width in units of 64 texels (2, 4, 8 or 16; 1 for the 32-bit bitmaps). Not needed to decode |
| 10 | 2 | u16 | Size in 256-byte GS blocks, rounded up (`ceil(size / 256)`). Not needed to decode |
| 12 | 4 | u32 | **Size** of the texel data in bytes |
| 16 | 4 | u32 | **Offset of the texel data, relative to T** |

Bitmap 0's data starts at offset 0 of the payload on every world, and the largest *offset + size* equals header
word 0 of the world file.

### 2.3 Shader table

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Shader count |
| 2 | u16 | Number of shaders that carry an animation record (equals the count of shaders whose animation kind is 1..8, on every world) |
| var | | Shader records |

Shader record:

| Size | Type | Meaning |
|---:|---|---|
| 1 | u8 | **Kind** (section 4) |
| 1 | u8 | Blend flags (section 4.2) |
| 2 | s16 | Object id of the shader |
| 4 | 4 x u8 | **Material colour** R, G, B, A. Almost always `ff ff ff ff` or a tint with alpha 0xE0..0xFF |
| 4 | 4 x u8 | Second colour (used by projected-volume shaders); equal to the first in 8,502 of 8,535 shaders sampled |
| 4 | u32 | Zone mask |
| 2 | u16 | Alternate object id (0xFFFF = none on every shader seen) |
| 1 | u8 | **Render pass** (section 4.1) |
| 1 | u8 | Texture record count. **Always 1** on all 46 worlds |
| var | | Animation record (2.4) |
| var | | Texture record(s) (2.5) |

### 2.4 Animation record (texture animation)

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Animation kind. 0 or 0xFFFF: no animation, **the record ends here** |
| 2 | s16 | Object id |
| 2 | u16 | Instancing flag |
| 24 | | Kind-specific parameters |

Kinds 1 to 8 occur; counts over all worlds: 1: 427, 6: 156, 2: 56, 3: 22, 7: 2, 5: 1. *Working assumption (from
the reference port, not yet checked against the game):* kinds 1, 2 and 3 step through the texture record's frames
(in order, back and forth, and at random), kinds 6 and 7 move the UVs over time; for kinds 1 and 2 the parameters are
four `f32` (minimum and maximum frame time, minimum and maximum pause) then an `s16` start frame; for kind 6 four
`f32` (U speed, V speed, U wrap, V wrap). Animation is not needed for a static frame: use frame 0 and no UV offset.

The same record appears inside meshes ([geometry.md](geometry.md), mesh flag `0x040`).

### 2.5 Texture record

| Size | Type | Meaning |
|---:|---|---|
| 2 | s16 | Object id |
| 2 | s16 | Texture flags (values seen: 0 in 23,726 records, 3, 2, 1; meaning unknown, *probably* wrap/clamp) |
| 1 | u8 | Frame count `f` (bitmap index count) |
| 1 | u8 | Palette index count `p` |
| 2 * f | u16[f] | Bitmap indices, one per animation frame |
| 2 * p | u16[p] | Palette indices |

Palette indices per frame: **three** when the shader kind is 0, so `p = 3 f`; **one** for every other
kind, so `p = f`. Frame `i` uses bitmap `bitmaps[i]` with palette `palettes[3i + k]` (kind 0, slot `k` = 0, 1, 2)
or `palettes[i]` (other kinds).

- A **32-bit** bitmap has palette index `0xFFFF` (it needs none). Seen only on kind-2 shaders.
- **Dangling references.** Some shaders hold bitmap or palette indices that are **at or past the end** of their
  table: 505 bitmap index entries and 1,900 palette index entries over all 46 worlds (373 and 1,118 distinct
  values, counted per world), in 646 shaders. A palette index of `0xFFFF` on a 32-bit bitmap is not dangling (42
  entries). The values continue the table's numbering (for example 390..404 in a table of 390 bitmaps). No submesh
  in any of the 46 worlds uses any of these 646 shaders. Treat such a reference as "no texture" and do not fail.

**Counting (bitmap, palette) pairs.** A texture record yields one pair per frame for shader kinds other than 0 and
three pairs per frame for kind 0 (section 2.5). Over all 46 worlds:

| Count | Value |
|---|---:|
| pairs in all texture records | 68,019 |
| pairs with both indices in range | 66,077 |
| plus pairs of a 32-bit bitmap with palette `0xFFFF` (decodable, no palette) | 42 |
| decodable pairs, total | 66,119 |
| distinct decodable pairs, counted per world and summed | 29,070 |

These replace an earlier figure of 29,226 that could not be reproduced. A decoder that handles every record should
decode 66,119 pairs, of which 29,070 are distinct within their world.

### 2.6 Font table

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Font count (4 or 5 per world) |
| var | | Font records |

Font record:

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Bitmap index |
| 2 | s16 | Palette index |
| 1 | u8 | Cell width (px) |
| 1 | u8 | Space width (px) |
| 1 | u8 | Line height (px): every glyph is one line height tall |
| 1 | u8 | The font's own number (0 to 4; equal to its position in the table in every world) |
| 4 | f32 | Scale (1.0 or 2.0) |
| 4 | u32 | Glyph count `g` |
| 8 × g | | Glyph records |

Glyph record: `u16` character code (16-bit; codes above 0xFF occur in font 0, for example 0x2122 and 0x2026), `s16`
x, `s16` y, `s16` width, all in pixels. (x, y) is the **top-left** corner of the glyph's cell, with y counted from
the **top of the upright picture**; the cell is `width` × line height. [D]

**Font bitmaps are stored bottom row first.** Storage row 0 is the bottom row of the upright picture, so the cell's
top row y is storage row `height − 1 − y`, and its row k (from the top) is storage row `height − 1 − y − k`. [D]
(Checked on Paris: font 0's `'` has ink in rows 6 to 12 of its cell, `.` in rows 29 to 32 and `T` in rows 7 to 32;
reading the cell top-first from storage row y gives empty cells for the same glyphs.) This is the same storage
order as every other bitmap (section 3, step 5); only the glyph coordinates are measured in the upright picture.
A port that uploads the bitmap in storage order samples glyph rows at v = (height − y − k − 0.5) / height; one that
flips the bitmap upright uses the records as they are.

Font bitmaps decode like any other bitmap (font 0 is 4-bit, the others 8-bit). Counts and pictures per world:
ui-and-menus.md 1.1.

## 3. Decoding a bitmap to RGBA8

1. **Texel order.** Texels are stored row by row, `width` texels per row, rows in storage order, with **no GS block
   swizzling** (the data is a plain linear upload). For 4-bit data, texel `2j` is the **low** nibble of byte `j`
   and texel `2j + 1` the **high** nibble.
2. **Index permutation for 8-bit bitmaps.** Palettes for 8-bit bitmaps are stored in the GS "CSM1" order. To look up
   index `i` (0..255), **swap bits 3 and 4 of `i`**, then use the result as the palette entry number. Example:
   index `0x08` reads entry `0x10`, index `0x10` reads entry `0x08`, `0x18` and `0x00..0x07` are unchanged. 4-bit
   bitmaps use their index directly. (Every 8-bit bitmap in the game has a palette of more than 16 colours and every
   4-bit one 16 or fewer.) Skipping the permutation gives visibly speckled images.
3. **Range.** On all 46 worlds every decoded index is below the palette's colour count.
4. **Alpha.** PS2 texture and palette alpha uses **0x80 as fully opaque** (GS alpha blending multiplies by
   `A / 128`). Observed palette alpha never exceeds 0x80. Convert with
   `a8 = min(255, round(a * 255 / 128))`, i.e. `(a * 255 + 64) / 128` in integer arithmetic, clamped to 255.
   R, G and B are already 0..255 and are copied unchanged. Apply the same alpha rule to the 32-bit bitmaps.
5. **Orientation.** Storage row 0 is texture coordinate v = 0, as on the GS; upload rows in storage order and use
   the mesh UVs unchanged. Viewed in an ordinary image viewer, many images (for example the title logo) appear
   upside down; that is expected and must not be "fixed" for world textures.

## 4. Shader kinds and how to draw them

Kind values and counts over all 46 worlds:

| Kind | What the data shows | Usual render pass (4.1) | Shaders |
|---:|---|---|---:|
| 0 | Three palettes per frame; most world geometry | 3, 13 | 17,795 |
| 1 | One palette per frame; vertex colours common | 3, 13 | 4,571 |
| 2 | Uses the 64 x 64 32-bit bitmaps | 3, 13 | 88 |
| 3 | | 13 | 3 |
| 4 | | 6 | 483 |
| 5 | Bitmap content filled at run time (the 41 kind-5 shaders point at placeholder bitmaps) | 3 | 41 |
| 6 | | 1 | 79 |
| 7 | | 16 | 230 |
| 8 | | 17 | 1,056 |
| 9 | | 10 | 46 |
| 10 | | 12 | 4 |

Kinds 2 to 10 are special effects; their exact use belongs to the rendering spec. For phase 1, draw kinds 0 and 1
and skip the others.

**Kind-0 palettes.** For kind 0, each frame has three palettes over the same bitmap. Slot 2 is always grey
(mean chroma about 0.003 over four worlds, against about 70 to 80 for slots 0 and 1). The game blends between the slots
with lighting (rendering spec). The reference port uses **slot 1 as
the base image**; for a first, unlit render, draw kind-0 submeshes with slot 1. In 578 of the frames sampled, all three slots are the same palette (for example
the plain-white material).

**Plain white material.** Untextured surfaces point at a tiny white bitmap (8 x 8, 4-bit, all indices 0, a
one-colour palette `ff ff ff 80`). Their colour comes from the material and vertex colours. This is ordinary data
and needs no special case.

### 4.1 Render pass

The byte orders drawing (lower values are drawn earlier; *working assumption*). Values 0 to 17 exist; the values
seen on meshes over the three test worlds and on shaders over all worlds are in the tables of geometry.md 7 and
section 4. For phase 1:

| Value | Treatment for a first render |
|---:|---|
| 3, 7 | Opaque (3 is by far the most common) |
| 4, 8 | Alpha-tested (discard texels with alpha below one half) |
| 13 | Translucent: alpha-blended, drawn after opaque geometry, sorted back to front |
| 5, 9 | Cel outlines: on the disc, every mesh with pass 5 or 9 holds only outline parts (geometry.md 3.2) and every mesh with outline parts has pass 5 or 9 (worlds 1, 2, 12, 30) |
| all others | Special passes; skip in phase 1 |

Common shader combinations (kind, pass, blend flags): (0, 3, 2) 5,220; (0, 13, 2) 866; (1, 13, 3) 629;
(1, 3, 2) 519; (1, 13, 2) 501; (8, 17, 2) 327.

### 4.2 Blend flags

*From the reference port, to be confirmed by observation:* for translucent passes, blend flag values 2 and 6 use
ordinary "source alpha over" blending, other values use additive blending (source alpha times source, plus
destination). For a first render, treat pass 3/7 as opaque, pass 4/8 as alpha-tested, pass 13 as blended.

### 4.3 Colours

*Not fully settled.* Material colours are 0..255 per channel and almost always opaque `ff`. The reference port
combines, per vertex, `material colour / 255` times `vertex colour / 255` ([geometry.md](geometry.md)) and then
multiplies by the texture. That is a reasonable first approximation for kind-1 materials. The PS2 lighting model
for kind-0 materials belongs to a later rendering spec.

## 5. Test vectors

All from the decompressed worlds. "Payload offset" is relative to T. The RGBA hash is SHA-256 of
`width * height * 4` bytes, rows in storage order, after the alpha conversion of section 3.

### 5.1 Table counts

| World | Palettes | Bitmaps | Shaders | Animated shaders | Fonts |
|---|---:|---:|---:|---:|---:|
| Splash (#0) | 63 | 61 | 44 | 2 | 4 |
| Paris, France (#1) | 905 | 390 | 858 | 16 | 4 |
| The Hideout (#2) | 768 | 335 | 469 | 4 | 5 |

Shader kinds in Splash: kind 0 x 13, 1 x 22, 2 x 1, 4 x 2, 7 x 5, 9 x 1.

### 5.2 Decoded images

| World | Bitmap | Palette | Entry fields (w, h, zone, format, GS width, GS blocks, size, payload offset) | Palette entry (zone, n, class, offset) | First index bytes | First palette bytes | First RGBA bytes | RGBA SHA-256 |
|---|---:|---:|---|---|---|---|---|---|
| Splash | 45 | 62 | 8, 8, 0x10000000, 0x14, 2, 1, 32, 389872 | 0x10000000, 1, 1, 8640 | `00000000` | `ffffff80` | `ffffffff ffffffff` | `3d6876a0146de8576eb2395a858de1213d1b92c65b779df3a331cfd5a4584546` |
| Splash | 16 | 3 | 32, 32, 0x10000000, 0x13, 2, 4, 1024, 374224 | 0x10000000, 128, 2, 3072 | `7f7f7f7f` | `00000080 40a8ff80` | `ffffff00 ffffff00` | `7c1adc669cd859a0b2033e34fffccc16a386e79a3710636d3bb8d8d7f1b69ef8` |
| Paris | 7 | 1 | 256, 128, 0x10000000, 0x13, 4, 128, 32768, 701968 | 0x10000000, 256, 4, 1024 | `ffffffff` | `819bdd04 5d81b180` | `00000000 00000000` | `200b26ff045819a9aba35abe7c05082274661d557c9f4292c9c17a1cc1f4e4d0` |
| Paris | 149 | 215 | 64, 64, 0x10000000, 0x14, 2, 8, 2048, 2046480 | 0x10000000, 16, 1, 166528 | `ffffffff` | `2f2f2f18 8f8f8f48` | `ffffff00 ffffff00` | `1daa7946ff8000c2195791564e951589ac14aa8eadef5de69dda6697efef5378` |
| Paris | 160 | 418 | 64, 64, 0x10000000, 0x14, 2, 8, 2048, 2069008 | 0x10000000, 16, 1, 179520 | `20404424` | `2e234380 45346380` | `2e2343ff 3c2d56ff` | `51d74b33d94027ab2d6a711d8264955096fe14861d51132d318cfe56910f6586` |
| Paris | 160 | 419 | same bitmap | 0x10000000, 16, 1, 179584 | `20404424` | `244e0380 386e0e80` | `244e03ff 355b0bff` | `1a95e6d55f64e547f977cd1106f5c59e73782f5d4d635de36f41a6205a4cbfdd` |
| Paris | 160 | 420 | same bitmap | 0x10000000, 16, 1, 179648 | `20404424` | `29292980 3e3e3e80` | `292929ff 333333ff` | `08f947812ca44808111ea5f80edc11a339b391d9616e72a5ff0486dfcd914394` |
| Paris | 27 | 0xFFFF | 64, 64, 0x10000000, 0x00, 1, 64, 16384, 1226256 | (none) | `000000ff 000000ff` | (none) | `000000ff 000000ff` | `62fb561c59d0cea247fc588f3311ee665375f35d8675b186e2792cb7dfcff88c` |

Notes on the vectors:

- Paris bitmap 160 is the frame of shader 120, a kind-0 shader whose texture record lists palettes 418, 419, 420
  (slots 0, 1, 2). It is the first submesh's material in Paris ([geometry.md](geometry.md) 6).
- Paris bitmap 7 with palette 1 (shader 41, kind 1) is the title logo; its centre texel (x 128, y 64) decodes to
  `010101ff`.
- Paris bitmap 149 with palette 215 is shader 15; its centre texel (x 32, y 32) decodes to `14141414`.
- The first texel of Paris bitmap 160 has index `0` (low nibble of `0x20`), so it reads palette entry 0:
  `2e 23 43 80` becomes `2e 23 43 ff`.

## 6. Open questions

- Texture flags (2.5) and blend flags (4.2) are only partly understood.
- Parameter layouts of the texture-animation kinds are from the reference port and unverified.
- The exact PS2 combination of material colour, vertex colour, lighting and the three kind-0 palettes is not
  specified here.
