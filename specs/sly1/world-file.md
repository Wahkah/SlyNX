---
id: sly1-formats-world-file
title: Sly 1 world file - compression and outer layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: >-
      decompression checked on all 46 worlds (output hashes in disc.md); every world parsed from byte 0 through
      the object stream to the texture payload with a throwaway checker; section offsets, counts and the
      relation between header word 0 and the texture payload measured on every world
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), file extractor (level decompressor) and its native world-file walker
    used_for: >-
      order of the top-level sections and the shape of the script table; decompression rule; all re-checked
      against the disc as above
  - kind: reference-project
    what: SlyTools @ 63987211 (2024-11-14), decompressor source
    used_for: confirmation that the window is a true 8 KiB ring (two independent decoders give identical output on four worlds)
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 world file: compression and outer layout

A **world file** holds one level: its scripts, materials, every object (with the object's geometry inline), and at
the end the texel and palette data. [disc.md](disc.md) says where the compressed bytes are on the disc. This
document covers how to decompress them and how the decompressed file is laid out at the top level. The per-class
contents of the object stream are in [object-stream.md](object-stream.md) (per-class layouts in [classes/](classes/)); textures and
geometry are in [textures.md](textures.md) and [geometry.md](geometry.md).

Conventions: little-endian; `vec3` = three `f32` (12 bytes); offsets are from the start of the **decompressed**
file. Unless a section says otherwise, **fields are packed with no padding and no alignment**: an `f32` can start
at an odd offset. Read multi-byte values byte-wise (or with unaligned loads). There are exactly two alignment points
in the whole file (sections 4.9 and, inside geometry, the vertex arrays of [geometry.md](geometry.md)); both are
relative to the start of the decompressed file.

A length-prefixed **string** is `u16 n` followed by `n` bytes of text with no terminator.

## 1. Overview

| Order | Section | Spec |
|---:|---|---|
| 1 | Header (8 bytes) | 4.1 |
| 2 | Prolog: debug flag, sound bank, name-table count | 4.2 |
| 3 | Script table | 4.3 |
| 4 | World option list | 4.4 |
| 5 | Two descriptors and a string | 4.5 |
| 6 | Material tables: palettes, bitmaps, shaders, fonts | 4.6, [textures.md](textures.md) |
| 7 | Object stream (all objects; geometry is inside them) | 4.7, [object-stream.md](object-stream.md), [geometry.md](geometry.md) |
| 8 | Zero padding to a multiple of 16 | 4.8 |
| 9 | Texture payload: texels and palette entries, to end of file | 4.9, [textures.md](textures.md) |

So the material **tables** (which bitmap, which palette, which shader) come **before** the object stream, the
geometry is **inside** the object stream, and the **pixel data** comes **after** it. A reader cannot find the
texture payload without walking the whole object stream, because the stream has no length field.

## 2. Compression

The bytes on the disc are an LZ77-style stream with an 8 KiB window. There is no header, no stored decompressed
size and no checksum. Decompression continues until the input (exactly `size` bytes from the descriptor) runs out.

State:

- a **window** of 8192 bytes, all zero at the start;
- a **write position** `w` in the window, starting at 0. After each output byte is stored at `w`, `w` becomes
  `(w + 1) mod 8192`. So `w` is always (number of bytes output so far) mod 8192.

Algorithm:

1. Read one **flag byte**. Its eight bits describe the next eight items, **least significant bit first**.
2. For each bit, in order bit 0 to bit 7. Stop at once if the input is exhausted:
   - **Bit = 1: literal.** Read one byte, output it.
   - **Bit = 0: copy.** Read two bytes `lo`, `hi` and form `t = lo + 256 * hi`.
     - Source position `p = t AND 0x1FFF` (13 bits). **This is an absolute position in the window, not a distance
       back from `w`.**
     - Length `n = (t >> 13) + 3` (3 to 10 bytes).
     - Repeat `n` times: take the byte at window position `p`, output it (which writes it at `w` and advances `w`),
       then `p = (p + 1) mod 8192`.
     - Because each byte is written before the next is read, a copy whose source overlaps the bytes it is writing
       repeats them (the usual LZ run behaviour). Positions never written yet read as zero.
3. Go to step 1 while input remains.

Every output byte goes both to the result and into the window. On all 46 worlds the input ends exactly at the end
of an item (never in the middle of a two-byte copy). The output length is whatever results; the sizes are listed in
[disc.md](disc.md) section 5.

### Worked example (world 0, "Splash")

Compressed bytes 0..23: `ff f0 f4 05 00 bb 76 68 3d bf 00 01 00 d3 05 00 08 00 00 f3 00 44 10 00`.

- Flag `ff`: eight literals `f0 f4 05 00 bb 76 68 3d` (output 0..7).
- Flag `bf` = binary `1011 1111`: bits 0..5 are 1, so six literals `00 01 00 d3 05 00` (output 8..13). Bit 6 is 0: a
  copy with bytes `08 00`, `t = 0x0008`, source position 8, length 3: window bytes 8, 9, 10 are `00 01 00`, output
  14..16. Bit 7 is 1: literal `00` (output 17).
- Next flag `f3`, and so on.

Decompressed bytes 0..23: `f0 f4 05 00 bb 76 68 3d 00 01 00 d3 05 00 00 01 00 00 00 44 00 00 00 00`.

## 3. Checking a decompressor

[disc.md](disc.md) section 5 gives the SHA-256 and CRC-32 of all 46 decompressed worlds. Matching all of them is
the acceptance test.

## 4. Top-level sections

### 4.1 Header

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | **Texture payload size** in bytes (see 4.9). On every world it equals the largest *bitmap offset + bitmap size* in the bitmap table |
| 4 | 4 | u32 | Probably a build timestamp (seconds since 1970-01-01 UTC). All 46 values decode to 2002-08-25, the day after the disc's creation date. *Interpretation; not needed for loading* |

### 4.2 Prolog

| Size | Type | Meaning |
|---:|---|---|
| 1 | u8 | **Debug flag.** 0 on every retail world. When non-zero, the name table below and the script table carry extra strings |
| 2 | s16 | **Sound bank** index for the level, or -1 for none. Values 0..6 seen; the bank list is the executable table in [disc.md](disc.md) 4.3 |
| 4 | u32 | **Name-table count** `n` |
| var | | Only if the debug flag is non-zero: `n - 1` length-prefixed strings. On retail worlds nothing follows the count |

### 4.3 Script table

The level's scripts are stored as nested lists. The contents belong to the scripting spec; a loader that only
renders must still be able to step over them.

| Size | Type | Meaning |
|---:|---|---|
| 4 | u32 | Script count `c` |
| 4 | u32 | Symbol count `s` (only used when the debug flag is set) |
| 4 | u32 | Extra-string count `x` (only used when the debug flag is set) |
| var | | `c` times: one **list node** (below); if the debug flag is set, three length-prefixed strings after each |
| var | | Only if the debug flag is set: `s + x` length-prefixed strings |

A **list node** starts with a tag byte `b`:

- `b = 0xFF`: empty; the node is only this byte.
- Otherwise, `k = b AND 0x7F` is the node kind and bit 7 says whether a further node follows this one.
  - If the debug flag is set, 10 bytes of debug information follow the tag byte.
  - Then a payload that depends on `k`:

    | `k` | Payload bytes |
    |---|---:|
    | 20, 21 | 4 (a 32-bit integer or float) |
    | 22 | 12 (a vector) |
    | 23 | 64 (a 4x4 matrix) |
    | 24, 26 | 12 |
    | 25 | 8 |
    | 27, 28, 29, 32 | 4 |
    | 30 | one nested list node (recursively, with this same encoding) |
    | any other value up to 36 | 0 |

  - If bit 7 of `b` was set, another list node follows (recursively). Kinds above 36 do not occur.

### 4.4 World option list

A list of (option id, value) pairs that sets properties on the world object itself. Every object in the object
stream carries the same kind of list. Its encoding, and the table that gives each option id's value size, are
specified in [object-stream.md](object-stream.md); this document only fixes its position. The list ends with the option id
`-1` (bytes `ff ff`).

### 4.5 Descriptors

| Size | Type | Meaning |
|---:|---|---|
| 32 | descriptor | An audio item. Empty (all zero) on every USA world |
| var | string | Length-prefixed string. Empty (`00 00`) on every USA world |
| 32 | descriptor | The level's **music** track ([disc.md](disc.md) 4.1). Empty on "Splash" |

### 4.6 Material tables

Five tables in a row, specified byte by byte in [textures.md](textures.md) section 2:

1. palette (CLUT) table;
2. bitmap table;
3. shader count, shader-animation count, then the shaders (each with its texture references);
4. font table.

### 4.7 Object stream

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Object count `m` |
| var | | `m` objects |

Each object starts with:

| Size | Type | Meaning |
|---:|---|---|
| 2 | s16 | Class id |
| 2 | s16 | Object id |
| 2 | s16 | A third identifier whose meaning is given in [object-stream.md](object-stream.md) (not needed to step over the object) |
| var | | Class-specific body, defined in [object-stream.md](object-stream.md) and [classes/](classes/) |

Bodies nest: many classes end with their own object count and child objects in the same format. Every class that
can be drawn contains a **mesh set** ([geometry.md](geometry.md)) at a fixed point of its body, after its transform
and option list. So the geometry is scattered through the stream, one mesh set per drawable object.

### 4.8 Padding

After the last object, zero bytes up to the next offset that is a multiple of 16 (none if already aligned). Call
that offset **T** (the texture base).

### 4.9 Texture payload

From T to the end of the file. Bitmap and palette offsets in the material tables are **relative to T**. The first
`header word 0` bytes of the payload are covered by the tables. On every world a few more bytes follow (26 to
about 750); nothing in the tables points at them. Ignore them. *Interpretation:* leftover from the build tool.

## 5. Test vectors

Section boundaries measured on three worlds (hexadecimal offsets in the decompressed file; each range is
start..end, end exclusive).

| Section | Splash (#0) | Paris, France (#1) | The Hideout (#2) |
|---|---|---|---|
| Header word 0 | `0x0005F4F0` | `0x00224410` | `0x003DEA60` |
| Header word 1 | `0x3D6876BB` | `0x3D6876B4` | `0x3D68767A` |
| Debug flag / sound bank / name count | 0 / 1 / 1491 | 0 / 1 / 4641 | 0 / 1 / 2814 |
| Script table | 0xF..0x41 | 0xF..0x3187 | 0xF..0x41 |
| Script count / symbols / extra | 1 / 68 / 0 | 68 / 221 / 9 | 1 / 68 / 0 |
| World option list | 0x41..0x49 | 0x3187..0x31C8 | 0x41..0x6D |
| Descriptors | 0x49..0x8B | 0x31C8..0x320A | 0x6D..0xAF |
| Music descriptor decodes to (sector, size, identity) | none | 0xC8E0E, 0x7A6A0, 0x55 | 0xD3411, 0x6F7A0, 0x43 |
| Material tables | 0x8B..0x14D5 | 0x320A..0xFCBC | 0xAF..0x8651 |
| Object stream | 0x14D5..0x9DFF | 0xFCBC..0x500EEA | 0x8651..0x16C2A6 |
| Texture base T | 0x9E00 | 0x500EF0 | 0x16C2B0 |
| File size | 0x6930A | 0x72556C | 0x54ADC5 |
| Unreferenced tail bytes | 26 | 620 | 181 |
| Objects at top level / all depths | 36 / 39 | 385 / 1295 | 94 / 557 |

On all 46 worlds: the debug flag is 0, the audio descriptor and the string of 4.5 are empty, and walking from
byte 0 through the object stream lands on a padding that never exceeds 15 bytes. A parser that reaches T on all 46
worlds with the header word-0 check passing has the outer layout right.

## 6. Open questions

- Header word 1 as a timestamp is an inference from the values.
- Descriptor words 3 and 4 of the music descriptor are non-zero (for example Paris: `0x01001000`, `0x60000004`) and
  their meaning is unknown.
- The tail bytes after the texture payload are not explained.
