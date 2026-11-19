---
id: sly1-option-encodings
title: Sly 1 option IDs, value encodings
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds
    used_for: >-
      the observed-ID grid; verification of the encoding of every observed ID (a walk of every world that uses
      these sizes ends exactly at the texture payload, and every typed-argument tag found is valid)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files spaprops (option descriptor table) and brx (option reader)
    used_for: >-
      a first guess at the value type of each option ID, kept here only for the 653 IDs whose sizes the disc walk
      confirms (the table is built by code in the game, not stored as data in the executable)
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (OptionSchema.generated.inc, OptionWalker)
    used_for: >-
      the disc sizes of the eight packed-colour options, which the PC port stores differently, and the typed
      argument counts
date: 2026-11-13
---

# Option IDs and their value encodings

An option list (object-stream.md, section 4) gives no value sizes: the size follows from the option ID. This file is
the table a loader needs to step over every option that occurs on the USA disc. The IDs that occur run from 2 to
1166.

The game builds its option table in code at start-up; there is no data table in the executable that a packager
could read, so the encodings are given here. Checked in the executable: the option descriptors live in an array of
32-byte records at address `0x2505D8` (file offset `0x1515D8`; scripts.md section 8 uses these addresses), and the
first 1,179 records, one per option ID 0 to 1178, are **all zero** in the file. Their contents (value type, flags,
argument counts) are written by start-up code, so the encodings cannot be read from the player's disc as data. Only IDs that occur on the disc are listed, and the size of each one
is confirmed by walking all 46 worlds. Any other ID is treated as corrupt input.

## 1. Codes

| Code | Bytes | Value |
|---|---|---|
| `b` | 1 | byte |
| `h` | 2 | `s16` |
| `f` | 4 | `f32` |
| `i` | 4 | `s32` |
| `c` | 4 | packed colour R, G, B, A (object-stream.md 4.2) |
| `p` | 8 | two `f32` |
| `v` | 12 | three `f32` |
| `q` | 16 | four `f32` |
| `0` to `6` | varies | a typed argument list of that many arguments (object-stream.md 4.3); `0` means the option ID alone, with no bytes after it |
| `.` | | does not occur on the USA disc; a stream containing it is treated as corrupt |

## 2. Encoding grid

Each line holds 64 consecutive IDs, starting at the number on the left, in four groups of 16. The character for ID
n is at line floor(n / 64), group floor((n mod 64) / 16), position n mod 16 within the group. Example: line `0`,
group 1, position 5 is ID 21, code `v`.

```
   0  ..0...........22 22222vvhf....... .........ffbhfff f...bhbhfvf.bhif
  64  .fvf.f...hh.hfvp vp......3..hfb.c .hvvfhffffp..fhh hhhh...b...b.b..
 128  hvbhhfffb.bbbb.h .h1.b........bbh f4.h..h.bbbbbf.. ..hbbifvp....b..
 192  ...........v.... ..bbhhhhbf.pffhh hbbi..bhfiv.hfff hbfhh.hff.bhhh.h
 256  .fh.bb.b4..h.f.b bbbh4b.h.b...h.. bhh......4.hh.h. ffh......hf.....
 320  .iifhiifhiifhiff fhfhhfbfhhb..bhp ffp.bhhhhh....h. vvhhv.vphpfvffv.
 384  piffppvhh.p.h.hv .v.fbhhhfhpb6b.. .fff.h.vvh.hv.vp hpfpifp.phhhfbvf
 448  f..fpvhh.pvh.hv. v.fbh.hfhpb.bhff f..hffffb..hvfff ff..bbpff.......
 512  .b..........bfff cffffffc.ff..... .....hpp2....... ............b...
 576  ....1........... ..........hvvv.. bhh...fbhfbh.h.f .ffh...f.hbhh.fb
 640  f..vhhh.....hppf 2..f422.2ib.p.h. .bfvh.iv.hhffb.b .pbfbbh.ff...h.2
 704  bbhhhfffffhhb.fp f..b.hhffffb.... ..........hhhh3. ffffff.....ffff.
 768  vbvhhhhhffvffvpp vppvffvff.vf.... ihivf.i..v...... h...h..f..b.1ff.
 832  fff..hf..ffh.p.h ...fv.v.v.ffffff fvbhbbib.ff.fbfb .vp.hffh......h.
 896  h.hffh.ff..4bh.. ......h.......pf .h..vhvvpvp.h... ..h..h..........
 960  .......h.b.002.. h.b...b.bp..c.ff fv.ffh..h.i0.vff fvvpp...........
1024  .....bfbbfffb... bh.hppi2hhb....f ...h.f.f.f....ff .ff.i...ff......
1088  ....hi...f.vpvp. .....ff.f.....11 1112433321b0.ccc fffh........i.i.
1152  .........332222. ................ ................ ................
```

Spot checks: ID 2 is `0`; ID 14 to 20 are `2`; ID 366 and 421 (emitter origin kind) are `h`; ID 528 and 535 (sky
and fog colour) are `c`; ID 529 to 534 are `f`.

The value sizes are confirmed by the disc walk. Within one size, the kind of value comes from the reference and is
a working assumption until a spec that uses the option confirms it: `f`, `i` and `c` are all 4 bytes, and `0` to
`6` are told apart by their argument tags (object-stream.md 4.3).

## 3. Which IDs occur on the USA disc

Same arrangement; `#` means the ID occurs in at least one of the 46 worlds, `.` means it does not.

```
   0  ..#...........## #########....... .........####### #...#######.####
  64  .###.#...##.#### ##......#..###.# .##########..### ####...#...#.#..
 128  #########.####.# .##.#........### ##.#..#.######.. ..#######....#..
 192  ...........#.... ..########.##### ####..#####.#### #####.###.####.#
 256  .##.##.##..#.#.# ######.#.#...#.. ###......#.##.#. ###......##.....
 320  .############### ###########..### ###.######....#. #####.#########.
 384  #########.#.#.## .#.###########.. .###.#.###.##.## #######.########
 448  #..#####.###.##. #.###.#####.#### #..######..##### ##..#####.......
 512  .#..........#### ########.##..... .....####....... ............#...
 576  ....#........... ..........####.. ###...######.#.# .###...#.####.##
 640  #..####.....#### #..####.###.#.#. .####.##.#####.# .######.##...#.#
 704  #############.## #..#.#######.... ..........#####. ######.....####.
 768  ################ #########.##.... #####.#..#...... #...#..#..#.###.
 832  ###..##..###.#.# ...##.#.#.###### ########.##.#### .##.####......#.
 896  #.####.##..###.. ......#.......## .#..#######.#... ..#..#..........
 960  .......#.#.###.. #.#...#.##..#.## ##.###..#.##.### #####...........
1024  .....########... ##.########....# ...#.#.#.#....## .##.#...##......
1088  ....##...#.####. .....##.#.....## ############.### ####........#.#.
1152  .........######. ................ ................ ................
```

653 IDs occur. Their encodings are verified by the full walks: a wrong size for any of them would derail the walk of
every world that uses it. The game defines more IDs than occur on the disc; they are left out of this spec on
purpose, since a loader for the USA disc can treat an unobserved ID as corrupt input without losing anything.

Encodings of the observed IDs: 200 `f`, 166 `h`, 97 `b`, 62 `v`, 46 `p`, 25 `i`, 7 `c`, and 50 typed argument lists
(5 with 0 arguments, 9 with 1, 21 with 2, 7 with 3, 7 with 4, 1 with 6). No `q` option occurs.

## 4. Notes

- The ID is read as `s16`; any negative ID ends the list (only −1 occurs).
- `c` options on the disc: 95, 528, 535, 988, 1133, 1134 and 1135. On the disc these are 4-byte packed colours; the PC
  port converts them to four floats, so its own table disagrees for these IDs. Use this file. Option 95 was checked
  on the disc: its 50 values are colours (28 end in an alpha byte of 0x80, 22 in 0xFF; for example `00 ff 10 80`).
- Options 526 and 527 (world ambient light) hold values on a 0 to 255 scale (object-stream.md 4.4).
- Only options 366 and 421 change the layout of what follows them (object-stream.md 4.5).
