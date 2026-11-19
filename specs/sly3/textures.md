---
id: sly3-formats-textures
title: Sly 3 textures, palettes and shaders
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: alpha rounding (shader 22 hashes with rounding down and with rounding to nearest)
  - kind: disc
    what: SCUS-97464 image; executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338; all 39 worlds
    used_for: >-
      the material tables of all 7,151 groups walked (both copies), every value survey and count below, the
      palettes-per-frame rule checked on every shader, the test vectors (decoded with and without the colour grading)
  - kind: reference-project
    what: >-
      earlier research (Games/Sly3-RE-notes.md, tools/sly3_world.py and tools/sly3_textures.py, 2026-11); SafeHouse @
      c70803ce (2026-11-12), Sly3-Release shader loader and its colour-grading port
    used_for: first reading of the record layouts, which copy the game keeps, the colour-grading rule and its defaults
  - kind: executable
    what: SCUS_974.64 (hash checked), the shader register-packet builder at 0x00235200..0x00235C80, GS TEST and ALPHA values
    used_for: the alpha-test table of section 4
  - kind: public-doc
    what: Sony GS User's Manual, TEST and ALPHA registers
    used_for: decoding the register values
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 textures, palettes and shaders

Read [../sly2/textures.md](../sly2/textures.md) first. Terms as there (palette, bitmap, shader, texture record,
texture data). Tags as in [disc.md](disc.md). The material tables sit in each group body
([world-file.md](world-file.md) 4, item 5) and address that group's texture data block (item 6).

## 1. What changed from Sly 2

- Every group stores its palette and bitmap tables **twice** ("copy 1" then "copy 0"), each table prefixed with its
  byte size. The game uses copy 1. [D for the layout, R for the choice] Both copies decode.
- Palette and bitmap headers have both a **world filter** and a **condition list** ([world-file.md](world-file.md) 6),
  replacing Sly 2's level-row conditions. [D]
- Palette colours are stored at **half intensity** and pass through a **colour grading** at load (section 5). [D for
  the values, R for the rule]
- The animation-record list is counted by a `u8` and its records are 36 bytes (Sly 2: `u16`, 32 bytes); the shader's
  animation reference is an `s8` (Sly 2: `s16`). [D]
- Render-pass numbers are Sly 2's **minus one** for the kinds both games share (section 4). [D]
- Pixel decoding is unchanged (Sly 1 section 3). [D]

## 2. Material tables

In this order [D]:

| Order | Table |
|---:|---|
| 1 | Palettes, copy 1 |
| 2 | Bitmaps, copy 1 |
| 3 | Palettes, copy 0 |
| 4 | Bitmaps, copy 0 |
| 5 | Animation records (2.3) |
| 6 | Shaders (2.4) |

Tables 1 to 4 each start with an `s32` **byte size** (counting from after the size field) and a `u16` count; every
table ends exactly at its size. [D]

### 2.1 Palettes

Per palette: a 16-byte header, the filter and condition lists, then the colours if they are inline. [D]

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Palette id |
| 4 | 1 | u8 | **Storage**: 1 = colours in the texture data block (69,666 per copy); 3 = inline (80); 0 = inline (2) |
| 5 | 1 | u8 | (meaning not needed) |
| 6 | 1 | s8 | World-filter count `f` |
| 7 | 1 | s8 | Condition count `c` |
| 8 | 4 | u32 | Flags: bit 0 = colour grading applies (all but 227 palettes); bits 7..15 = **colour count** (1 to 256); bits 16 up = size class 1, 2 or 4 (as Sly 2's) |
| 12 | 4 | s32 | Offset of the colours in the texture data block (storage 1) |

Then `f` × `s16` world slots, `c` × `s32` conditions, then for storage other than 1 the colours (`count` × 4 bytes,
R, G, B, A, alpha 0..0x80). [D] 6,518 palettes per copy carry a filter or condition. [D]

### 2.2 Bitmaps

Per bitmap: a 24-byte header, the filter and condition lists, then the texels if inline. [D]

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Bitmap id |
| 4 | 1 | u8 | **Storage**: 1 = in the texture data block (36,333 per copy); 3 = inline (80; sky domes); 0 = inline (1) |
| 5 | 1 | u8 | (meaning not needed) |
| 6 | 1 | s8 | World-filter count |
| 7 | 1 | s8 | Condition count |
| 8 | 2 | u16 | Width |
| 10 | 2 | u16 | Height |
| 12 | 1 | u8 | Pixel format: `0x14` 4-bit (33,565 in copy 1), `0x13` 8-bit (2,849). No other value [D] |
| 13 | 1 | u8 | GS buffer width; not needed |
| 14 | 2 | u16 | GS block count; not needed |
| 16 | 4 | u32 | Texel byte count |
| 20 | 4 | s32 | Offset in the texture data block (storage 1) |

Then the filters, conditions and (inline) texels as for palettes. 3,312 bitmaps per copy carry a filter or
condition. [D] The pixel format always agrees with the palette size (more than 16 colours ⇒ 8-bit). [D]

### 2.3 Animation records

`u8` count, then per record a `u16` class index (`0xFFFF` = empty slot, no more bytes) followed by 36 bytes of
settings. [D] 545 records on the disc; class indices 175 to 180 (175: 178 records, 176: 42, 177: 5, 178: 193,
179: 99, 180: 28). [D] *Interpretation:* texture-animation controllers (Sly 2 2.3, Sly 1 rendering 8). [I]

### 2.4 Shaders

`u16` count, then per shader a 24-byte header, an `s8` animation index (into 2.3, −1 none; 545 shaders use one), and
its texture records. [D]

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | s32 | Shader id |
| 4 | 1 | u8 | **Kind** (section 4) |
| 5 | 1 | u8 | Flags (Sly 2 2.4's bits; section 4) |
| 6 | 1 | u8 | 1 (3 on the 74 kind-4 shaders) |
| 7 | 1 | u8 | 0 |
| 8 | 4 | 4 × u8 | **Material colour** R, G, B, A (`ff ff ff ff` on textured shaders); RGB on a 255 scale, alpha on the 0x80 scale [R] |
| 12 | 4 | 4 × u8 | Second colour |
| 16 | 4 | 4 × u8 | Third colour |
| 20 | 2 | 2 × u8 | Two parameters (non-zero on 5,471 shaders) |
| 22 | 1 | u8 | **Render pass** (section 4) |
| 23 | 1 | u8 | Texture record count: **always 1** |

**Texture record** (as Sly 2): 8 bytes (`s32` id, 2 bytes, `u8` frame count `f`, `u8` palette count `p`), then `f` ×
`u16` bitmap indices and `p` × `u16` palette indices (`0xFFFF` = none). Indices point into **copy 1**'s tables. [D]

## 3. Texture data block

As Sly 2 section 3: storage-1 palettes and bitmaps live in the group's texture data block at their offsets. [D]
Decoding is Sly 1's (linear rows, 4-bit low nibble first, 8-bit through the CSM1 index swap) except for the
alpha rounding: alpha is `min(255, floor(a × 255 / 128))`, **rounded down** in integer arithmetic, not rounded to
nearest as in ../sly1/textures.md 3. [D] The two rules differ only for some alpha values below 0x80; the section 6
hashes use rounding down (shader 22, whose palette has partial alpha, hashes to `45779e69…` rounded down and to
`50ee463b…` rounded to nearest; shaders 0 and 24 hash the same either way). [D] 171,944 decodes (every frame of every shader with every palette, both copies) end
without error. [D]

## 4. Shader kinds, palettes per frame and passes

| Kind | Shaders | Palettes per frame | Passes (count) | Sly 2 passes of the same kind |
|---:|---:|---|---|---|
| 0 | 3,527 | 1 | 9 (2,081), 2 (1,446) | 10, 3 |
| 1 | 29,941 | 2 | 2 (16,148), 9 (13,793) | 3, 10 |
| 2 | 1,060 | 1 | 6 (1,054), 7 (6) | 7, 8 |
| 3 | 565 | 1 | 10 (489), 14 (76) | 11, 13 |
| 4 | 74 | 1 | 0 | 1 |
| 5 | 44 | 1 | 15 | — (no kind-5 shader in Sly 2) |
| 6 | 309 | 1 | 1 | 2 |
| 7 | 135 | 2 if flag `0x10` is set (125), else 3 (10) | 4 | 5 |

[D] (Sly 2 decided kind 7's count by flag `0x20`; Sly 3 by `0x10`.) Frame `i` uses bitmap `bitmaps[i]` and palettes
`palettes[k·i .. k·i + k − 1]`. [D] For kind 1, slot 0 is the coloured palette and slot 1 a grey copy, as in
Sly 2. [D for the spot checks, R]

Pass meanings follow Sly 2's minus one [R]: 0 sky (kind 4), 2 opaque (most scenery), 9 alpha-blended (foliage,
glass, effects), 6 kind-2 blob shadows, 10 and 14 kind-3 projected volumes, 4 kind 7, 1 kind 6, 15 kind 5.
**Flag bit 0** selects additive blending (source × source alpha + destination) instead of ordinary blending, as in
Sly 2. [R] For a first render: passes 0 (sky), 2 and 3 opaque with the depth-write threshold below, 9 blended. [I]

**Alpha test (the "cut-off").** The game sets the GS alpha test per shader when it builds the shader's register
packet (routine at virtual address `0x00235200`–`0x00235C80` in SCUS_974.64, hash checked) [D for the register
values read there]:

| Shaders | GS `TEST` value | Meaning |
|---|---|---|
| kind 0, flag `0x20` clear | `0x717EB` | alpha test on: pass if alpha ≥ `0x7E`; on failure write colour only (no depth); depth test "greater" (the PS2's nearer), depth write on |
| kind 0, flag `0x20` set | `0x717EA` | alpha test off |
| kinds 4, 5, 6 (the same packet builder as kind 0 [R]) | `0x317E1` (`0x317E0` with flag `0x20`) | test "never" with colour-only failure, depth test "always": colour only, no depth write (with flag `0x20` the test is off and depth is written) |
| kind 1, first pass (palette slot 1) | `0x717EB` (`0x717EA` with flag `0x20`) | as kind 0 |
| kind 1, second pass (slot 0, added) | `0x51001` | colour only at equal or nearer depth |
| kind 7, depth pre-pass | `0x5000D` | pass if alpha > 0, write depth |

So there is **no discard cut-off**: every fragment is blended into the colour buffer with the shader's blend
(normal, or additive with flag bit 0); the threshold only decides which fragments **write depth**. The reference is
`0x7E` on the PS2 alpha scale, applied to the fragment's final alpha (texture alpha × vertex alpha, both 0x80 = 1.0):
depth is written where alpha ≥ 0x7E / 0x80, about 0.984, which is 251 / 255 after the RGBA8 conversion of section 3.
[D for the values, R for the GS semantics from the public GS manual] A port that cannot keep colour and depth
writes apart can approximate it as the renderer's two-step draw: blend everything, and write depth only where alpha
≥ 251/255 [I]. A plain alpha-test discard at 0.5 is not what the game does [D].

## 5. Colour grading

Stored palette colours are about half intensity (the largest channel rarely exceeds 0x80). [D] When the palette's
flag bit 0 is set, each colour is converted at load [R]:

1. Convert R, G, B (each / 255) to hue, lightness L and saturation S (the usual HLS model).
2. `L' = clamp((L × a − a/4) × b + a/4, 0, 1)` and `S' = clamp(S × c, 0, 1)`, with **a = 1.5** (lightness), **b = 1.2**
   (contrast), **c = 1.4** (saturation). Hue is unchanged.
3. Convert back to RGB, round each channel `× 255 + 0.5`, keep alpha.

The three numbers are fields of the world's option set, set by options 1025, 1026 and 1027 [X]. Two worlds change
them in option set 0 [D]: c_ext (1, 1, 1: no grading) and c_flashback (1.0, 1.1, 0.24); every other world uses the
defaults above. (An earlier version said no set changes them.) Option 1028 adds a per-channel adjustment (p_dive
only). Details: [rendering.md](rendering.md) 3.

## 6. Test vectors

World slot 5 (v_apt), level group `v_apt` (body at `0x61D5B0`; texture data at `0x61FF00`, 675,200 bytes; copy 1:
126 palettes, 64 bitmaps; 88 shaders). "Raw" decodes the stored colours; "graded" applies section 5. RGBA hashes are
SHA-256 of `width × height × 4` bytes, rows in storage order, frame 0, palette slot 0. [D]

| Shader | Kind / pass | Shader header | Bitmap header | Palette header | Raw first RGBA | Graded first RGBA | Raw SHA-256 | Graded SHA-256 |
|---:|---|---|---|---|---|---|---|---|
| 0 | 1 / 2 | `4128000001020100ffffffffffffffff0000000000000201` | (62) `432800000102000010001000140201008000000020010800` | (114) `22300000010200008107010020550000` | `23292cff 262b2fff` | `2c373cff 313a42ff` | `5011f422a92a23fddb499ddf812227a5e1dfabcdbbe45149844aa7c7e0600413` | `e698f82e3d6c97dd1f93b2846e3f773f33423082a659329321dc1f09a8fc66d8` |
| 2 | 1 / 2 | `0e1d020001020100ffffffffffffffff0000000000000201` | (8) `101d020001020000800080001302400000400000a0e90400` | (105) `861e02000102000001800400e03e0000` | `6a5331ff 6a5331ff` | `b6843aff b6843aff` | `b19bbc8404a9893251c2d6f9b90c67b0c581fe68e6fe950a2f5e4b351da2a0e8` | `efb29efbf8d463f951f8e93c29a70edd99af802077e63821a4f568f860b36a43` |
| 7 | 0 / 2 | `411d020000020100ffffffffffffffff0000000000000201` | (57) `c265010001020000200020001402020000020000a0fb0700` | (109) `de6701000102000001080100e04e0000` | `5d3107ff 643b09ff` | `8e4500ff 9e5700ff` | `52f18ab43306a8444cde1297928b45fa48f155dbf9ab5fcb9866573b0c14a9d9` | `ec5d5d6b07fa36f3bd15545dbc84983fd094775c6c7c21880c18129940141741` |
| 22 | 1 / 9 | `d93a020001020100ffffffffffffffff0000000082020901` | (7) `db3a020001010000800080001302400000400000a0a90400` | (103) `003f02000101000001800400e0360000` | `00000000 00000000` | `00000000 00000000` | `45779e69bbab7a1b950732b6919607c53278ca990558b681707ac2fff35f2110` | `df378fc01d2a8aa453a88b6730b6b06da1e9f7b7d59e0f9ea9ee8b6bcb78e03c` |
| 24 | 1 / 2 | `ed3a020001020100dcdcdcffdcdcdcff0000000000000201` | (63) `e207000001020000080008001402010020000000a0010800` | (0) `e4070000010200008100010000000000` | `807f76ff 807f76ff` | `cdcdc7ff cdcdc7ff` | `08844f180db8bce2edd946f5805adc4946bbac285c7a773fc0faf25c57d368de` | `20ac5c68a770c5a49e8a47c391ff34cb08df9c99a760f31e5ae5a0e2f69072eb` |

Shader 24 uses the same 8 × 8 grey placeholder as Sly 2 (one colour `80 7f 76 80`): its raw hash equals Sly 2's
`08844f18…` (../sly2/textures.md 5). Shader 22 carries the parameter bytes `82 02` at offset 20. [D]

Totals (copy 1) over the 39 worlds: 69,748 palettes, 36,414 bitmaps, 35,655 shaders, 545 animation records. [D]

## 7. Open questions

- Bytes 5 of the palette and bitmap headers, the shader's second and third colours and its two parameter bytes.
- The animation records' settings (36 bytes) per class.
- Why copy 0 exists (a lower-detail set for another memory mode ?) and whether it ever differs in content from copy 1.
- (Answered in [rendering.md](rendering.md) 2.3: kinds 1 and 7 combine their slots as in Sly 2, with A and B taken
  from the colour tables where a strip list uses them.)
