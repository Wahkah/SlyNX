---
id: sly2-formats-textures
title: Sly 2 textures, palettes and shaders
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 image; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      material tables of all 7,753 packages walked (71,774 palettes, 37,508 bitmaps, 38,750 shaders); the first
      frame of every shader decoded without error; value surveys of every header field; 8-bit images viewed with
      and without the index permutation; condition ids surveyed in three worlds; every test vector below
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9 (2026-11-12), src/ readers for palettes, bitmaps, the node list, shaders and texture
      records; earlier research in this workspace (Sly2-RE-notes.md, tools/sly2_textures.py)
    used_for: first reading of the header layouts and the conditional-loading rule; layouts re-checked by walking every world
reviewed_by: pending
date: 2026-11-13
---

# Sly 2 textures, palettes and shaders

Tags: [D] checked on the disc, [R] from a reference, [I] inferred. Terms (palette, bitmap, shader, zone mask) as in
[../sly1/textures.md](../sly1/textures.md). The material tables sit in each package body
([world-file.md](world-file.md) 4, item 5); the texel and palette data they address is the package's **texture
data** block (item 7).

## 1. What changed from Sly 1

- Tables are per package, not per world; texture data follows each package's tables instead of the end of the
  file. [D]
- Palette and bitmap headers are new, carry an id, a storage kind and optional **level conditions**. [D]
- A **node list** between the bitmaps and the shaders holds texture-animation objects. [D] [R]
- The shader record is 24 bytes plus a node reference; the number of palettes per frame depends on the shader kind
  differently from Sly 1. [D]
- Pixel decoding is unchanged ([../sly1/textures.md](../sly1/textures.md) section 3). [D]

## 2. Material tables

In this order: [D]

### 2.1 Palettes

`u16` count, then per palette a 16-byte header, its condition ids, and its colours if they are stored inline.

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Palette id |
| 4 | 1 | u8 | **Storage**: 1 = colours are in the texture data block; 3 = colours follow inline (62 palettes) |
| 5 | 1 | s8 | Condition count `c` (2.5) |
| 6 | 2 | | Zero |
| 8 | 2 | u16 | Colour count `n` (1 to 256) |
| 10 | 2 | u16 | Size class: 1 for `n` ≤ 16, 2 for 17..128, 4 for more (as in Sly 1); not needed |
| 12 | 4 | s32 | Offset of the colours in the texture data block (storage 1) |

Then `c` × `s16` condition ids; then, for storage 3, `n` × 4 bytes of colours. Colours are R, G, B, A with alpha
0..0x80 as in Sly 1. [D]

### 2.2 Bitmaps

`u16` count, then per bitmap a 24-byte header, its condition ids, and its texels if inline.

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Bitmap id |
| 4 | 1 | u8 | **Storage**: 1 = texels in the texture data block; 3 = texels follow inline (62 bitmaps, the sky domes) |
| 5 | 1 | s8 | Condition count `c` |
| 6 | 2 | | Zero |
| 8 | 2 | u16 | Width |
| 10 | 2 | u16 | Height |
| 12 | 1 | u8 | Pixel format: `0x14` 4-bit (32,963 bitmaps) or `0x13` 8-bit (4,545). No other format occurs |
| 13 | 1 | u8 | GS buffer width in 64-texel units; not needed |
| 14 | 2 | u16 | GS block count (usually `ceil(size / 256)`); not needed |
| 16 | 4 | s32 | Texel data size; always width × height × bits / 8 |
| 20 | 4 | s32 | Offset of the texels in the texture data block (storage 1) |

Then `c` × `s16` condition ids; then, for storage 3, the texels. [D] No mip levels are stored. [D]

The game shares inline texels between bitmaps with the same id and loads the copy only once; decoding is the same
either way. [R]

### 2.3 Node list

`u16` count, then per node `u16` class index and 32 bytes of settings. [D] The class indices are entries of the
executable's class table ([object-stream.md](object-stream.md)); the values seen are 139 to 144 (243, 14, 25, 152, 166
and 8 nodes over the disc) [D]. They are texture-animation controllers referenced by shaders: frame cycling and UV
movement ([rendering.md](rendering.md) 4) [R]. Not needed for a static frame.

### 2.4 Shaders

`u16` count, then per shader: [D]

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Shader id |
| 4 | 1 | u8 | **Kind** (section 4) |
| 5 | 1 | u8 | Flags (below) |
| 6 | 1 | u8 | 1 (3 for kind 4) |
| 7 | 1 | u8 | 0 |
| 8 | 4 | 4 × u8 | **Material colour** R, G, B, A (`ff ff ff ff` on textured shaders) |
| 12 | 4 | 4 × u8 | Second colour |
| 16 | 4 | 4 × u8 | Third colour (all zero on the disc) |
| 20 | 2 | s16 | **Self-illumination**, in 1/4096 (zero on 30,816 shaders; non-zero only on kinds 0, 1, 3 and 7; values are multiples of about 80.3, i.e. steps of 5/255) [D]; added to the lighting's mid-tone term ([rendering.md](rendering.md) 2) [R]. Earlier drafts called this "animation parameters"; that was wrong |
| 22 | 1 | u8 | **Render pass** (section 4) |
| 23 | 1 | u8 | Texture record count: **always 1** |

Then `s16` node reference (an index into 2.3, or −1) [R], then the texture records. A **texture record** is 8 bytes —
`s32` id, 2 zero bytes, `u8` frame count `f`, `u8` palette count `p` — followed by `f` × `u16` bitmap indices and
`p` × `u16` palette indices (`0xFFFF` = none). [D] Every index on the disc is in range. [D]

**Shader flags** (byte 5), as the game turns them into GS drawing state for kinds 0, 4, 5 and 6 [R] (from the
reference's shader set-up; written as blending formulas with `Cs` the shader output, `Cd` the frame buffer, `As`
source alpha on the 0x80 = 1.0 scale and `Ad` destination alpha):

| Bit | Effect |
|---|---|
| `0x20` | the alpha test never passes, so every pixel takes the failed-pixel rule: colour written without blending and without depth (bit `0x04` picks whether the frame's alpha is written) [R] |
| `0x10` (when `0x20` is clear) | blend `(Cd − Cs)·As + Cs`; with bit `0x01`: `Cd·As + Cs` |
| `0x80` (when `0x20` and `0x10` are clear) | blend by destination alpha: `(Cs − Cd)·Ad + Cd` |
| none of the above | ordinary blend `(Cs − Cd)·As + Cd`; with bit `0x01`: additive `Cs·As + Cd` |
| `0x04` | pixels that fail the alpha test write colour only (RGB, keeping the frame's alpha); without it they write colour and alpha. Failed pixels never write depth [R] |
| `0x02` | kind 3 only: the volume is textured (and keeps its texture alpha) |
| `0x08` | kind 2 only: a different depth test |
| `0x40` | not an animation flag: set on 9,284 shaders of which only 18 have an animation record, while 590 shaders without it do have one [D]; the drawing state ignores it [R]; meaning open. A shader is animated when its node reference (below) is not −1 |

For these kinds the alpha test passes at alpha ≥ `0x7E` on kind 0 and never on the other kinds (so only the
failed-pixel rule draws them), with depth tested "greater" on kind 0 and "always" otherwise [R]. The three colours
are passed through a grey filter at load when the game's global grey mode is on [R].

### 2.5 Level conditions

A palette or bitmap with condition count `c > 0` lists `c` level-table row numbers ([disc.md](disc.md) 6; values 0 to
43 seen). It is loaded only when the current level's row is in the list (or no level is current); otherwise its
inline data is skipped and it counts as absent. [R] [D for the values] 9,554 palettes and 4,866 bitmaps carry
conditions. [D] The same rule selects per-level mesh variants ([geometry.md](geometry.md)). [R]

## 3. Texture data block

A package's texture data block ([world-file.md](world-file.md) 4, item 7) holds the texels and colours of every
storage-1 bitmap and palette at the offsets in their headers. In every package the largest offset + size equals the
block size exactly; 73 packages have no storage-1 items and an empty block. [D]

**Decoding** is exactly Sly 1's ([../sly1/textures.md](../sly1/textures.md) section 3): linear rows, 4-bit low
nibble first, 8-bit indices through the bit 3 / bit 4 swap (confirmed by eye on Sly 2 images: without it they are
speckled), alpha `min(255, round(a × 255 / 128))`, storage row 0 at v = 0. [D]

## 4. Shader kinds, palettes per frame and passes

| Kind | Shaders | Palettes per frame | Render passes seen (count) |
|---:|---:|---|---|
| 0 | 3,498 | 1 | 10 (2,202), 3 (1,296) |
| 1 | 32,581 | 2 | 3 (22,949), 10 (9,632) |
| 2 | 1,435 | 1 | 7 (1,394), 8 (41) |
| 3 | 941 | 1 | 11 (674), 13 (267) |
| 4 | 62 | 1 | 1 |
| 6 | 141 | 1 | 2 |
| 7 | 92 | 2 if flag `0x20` is set (59), else 3 (33) | 5 |

[D] Frame `i` of a texture record uses bitmap `bitmaps[i]` and palettes `palettes[k·i .. k·i + k − 1]`, with `k` the
palettes per frame. [D]

**Which palette to draw.** For kind 1, slot 0 holds the coloured palette and slot 1 a grey one (every colour of slot 1 has
R = G = B in all 618 kind-1 shaders of the level packages of rows 3, 9 and 27, while every slot 0 has colour). [D] For a first, unlit render use slot 0
for every kind. [I] How the game blends the slots with lighting belongs to the rendering spec.

**Passes.** For phase 3's first render: passes 3 opaque; 10 alpha-blended; 1 the sky (kind 4, inline 8-bit
textures); others special (2, 5, 7, 8, 11, 13). [D for the counts] [I for the treatment]

## 5. Test vectors

RGBA hashes are SHA-256 of `width × height × 4` bytes after the alpha conversion, rows in storage order, using the
first frame and palette slot 0.

Row 3 world (`f_nightclub_cellar`), level package (body at `0x7A2120`; texture data at `0x7A54F0`, 866,096
bytes; 224 palettes, 115 bitmaps, 171 shaders): [D]

| Shader | Kind / pass | Shader header | Bitmap header | Palette header | First index bytes | First palette bytes | First RGBA | RGBA SHA-256 |
|---:|---|---|---|---|---|---|---|---|
| 0 | 1 / 3 | `8117020001020100ffffffffffffffff0000000000000301` | (bmp 5) `831702000100000040004000140208000008000010ed0200` | (pal 49) `2e1802000100000010000100701f0000` | `4b4b66c6` | `37232780 513f3980` | `40282dff 3f3029ff` | `283a6fb0a69f53d6926b61c5acf869b1d33b01251734c92ac4ed4f264ad5b67b` |
| 3 | 1 / 10 | `e516020001020100ffffffffffffffff0000000000000a01` | (bmp 19) `6e0902000100000040004000130210000010000090980500` | (pal 205) `b00c0200010000000001040050c20000` | `ffffffff` | `100a1a06 35334572` | `80808000 80808000` | `b43df195969edc4e9a7715fcbee48a34184f2c8caf4f2ccc6cd60ec14a85884d` |
| 5 | 0 / 10 | `85f7010000030100ffffffffffffffff0000000000000a01` | (bmp 54) `1a75000001000000200020001302040000040000b0090700` | (pal 166) `a07700000100000080000200909e0000` | `7f7f7f7f` | `7f571f02 806a392a` | `80808000 80808000` | `27daa1bd2f4a469290b1f93b7ab8cab345fc76decf499e7d4c99b92ee079445e` |
| 24 | 0 / 3 | `1ce1010000020100ffffffffffffffff0000000000000301` | (bmp 61) `6438000001000000200020001402020000020000b0d10700` | (pal 131) `953f0000010000000b00010070850000` | `0a000060` | `72633880 77715380` | `766a42ff 726338ff` | `cf70dc0943ca949aa2173ff865a75b783ba24312752a6b42f05ca8af018fddfa` |
| 68 | 2 / 8 | `c2ea0100020a010000000033000000330000000000000801` | (bmp 29) `5d0600000100000008000800140201002000000010160600` | (pal 167) `5e060000010000000100010090a00000` | `00000000` | `807f7680` | `807f76ff 807f76ff` | `08844f180db8bce2edd946f5805adc4946bbac285c7a773fc0faf25c57d368de` |

Row 0 world (`jb_intro`), level package (body at `0x8FE580`; texture data at `0x904A30`, 1,325,792 bytes; 354
palettes, 178 bitmaps, 395 shaders): [D]

| Shader | Kind / pass | Bitmap (w × h, format) | Palette | First RGBA | RGBA SHA-256 |
|---:|---|---|---|---|---|
| 1 | 0 / 3 | 173 (128 × 128, 8-bit) | 76 (256 colours) | `252357ff 252357ff` | `99b3c1bf62e0eb6443de191e47a5c8ec14f9aec985a6ed1dfad42c521615b39d` |
| 2 | 1 / 3 | 129 (64 × 128, 4-bit) | 277 (16) | `3b0407ff 3b0407ff` | `e51db1bf3364d05bb1a4ace1257f58deea591e099f81b95525736b7c34fb836d` |
| 19 | 1 / 10 | 85 (32 × 32, 8-bit) | 221 (128) | `80808000 80808000` | `13ebd8dbdf9b45f13f668d229acd616892a4c83d6f42cd6ac590e36a4f9dba59` |
| 23 | 0 / 3 | 77 (32 × 32, 4-bit) | 61 (16) | `051407ff 020502ff` | `306c08d5b7faf8d2384f9428e297b40f45ed971c16f0cf0655fd1ea38282d442` |
| 43 | 7 / 5 | 95 (64 × 64, 4-bit) | 275 (16; record lists 275, 343, 350) | `80808000 80808000` | `19de47cf28c8c86219673c73618efe5af7f5bb6330982a79e455bf2c1efe049d` |

The 8 × 8 grey placeholder (bitmap 29 in the cellar, 126 in the intro hub, one colour `80 7f 76 80`) decodes to the
same hash `08844f18…` in both worlds. [D] The intro hub's sky-dome package carries an inline 1,024 × 256 8-bit bitmap
(header `c54f02000300000000040001131000040000040000000000`, 262,144 bytes). [D]

Totals over all worlds: 71,774 palettes (71,712 storage 1, 62 storage 3), 37,508 bitmaps (37,446 / 62),
38,750 shaders, 38,750 first frames decoded without error. [D]

## 6. Open questions

- Bytes 20 and 21, the second and third colours, and the node list's contents.
- How the palette slots of kinds 1 and 7 combine with lighting.
- The meaning of the pass values other than 1, 3 and 10.
