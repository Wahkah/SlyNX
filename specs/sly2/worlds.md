---
id: sly2-worlds
title: Sly 2 per-world test vectors for the object stream and geometry
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; the 43 decompressed worlds (decompressed as in world-file.md)
    used_for: every number below, from a scratch walker that reaches the end of every package section and parses every solid body to the byte
  - kind: reference-project
    what: Games/Sly2-RE-notes.md and tools/sly2_world.py, sly2_geom.py (earlier research)
    used_for: the starting point of the scratch walker
  - kind: disc
    what: same disc and worlds, every-depth body walk re-run on 2026-11-14
    used_for: section 5 (121,153 bodies by context and class), the row-32 copy-extra solids and the corrected totals
date: 2026-11-13
---

# Sly 2 per-world test vectors

Counterpart of [../sly1/worlds.md](../sly1/worlds.md). A port's own walker should reproduce these numbers.

## 1. Definitions

- **Row**: the world's row in the level table (disc.md, world-file.md); row 18 is empty [D].
- **Id**: the world's identity from the level table [D].
- **Level package**: the name of the **last** package of the world, which is the level itself [D] (the earlier
  packages are prefabs used by placements).
- **Size**: decompressed bytes.
- **Packages**: package sections in the world.
- **Records**: object records in all packages' object lists, including the extra objects of copy placements
  (object-stream.md 2.4), not counting objects nested in solid bodies.
- **Placements**: placement records (classes 6 and 7) in those lists, including nested copy placements.
- **Solids**: solid bodies (classes/solid-object.md) at every depth: top-level, nested in solid bodies, and the
  extra objects of copy placements, including copy placements nested in solid bodies. Row 32 (`c_sawmill_ext`) is
  the only world with solids among copy extras: 6 (3 in top-level copy placements, 3 in a copy placement nested in a
  solid) [D].
- **Meshes**, **groups**: meshes (conditional ones included, whatever the world's row) and vertex groups
  (geometry.md) in all solid bodies of all packages.
- **Triangles**: triangles decoded from all strip lists of those meshes (geometry.md 8), degenerate ones dropped,
  each mesh counted once (copies and placements not multiplied).
- **Tail**: bytes after the last package section that are never read (world-file.md).

## 2. All worlds [D]

| Row | Id | Level package | Size | Packages | Records | Placements | Solids | Meshes | Groups | Triangles | Tail |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 0x05D | jb_intro | 17420982 | 239 | 1201 | 296 | 441 | 2218 | 3798 | 134454 | 934 |
| 1 | 0x039 | dvd_menu | 4084599 | 3 | 43 | 2 | 2 | 2 | 3 | 57 | 567 |
| 2 | 0x041 | f_nightclub_exterior | 18746337 | 287 | 2116 | 618 | 453 | 2415 | 4338 | 132161 | 2177 |
| 3 | 0x03B | f_nightclub_cellar | 11402859 | 172 | 884 | 230 | 255 | 1532 | 2441 | 71772 | 1995 |
| 4 | 0x03E | f_nightclub_disco | 13392193 | 213 | 1047 | 308 | 306 | 1708 | 2915 | 91015 | 2145 |
| 5 | 0x044 | f_nightclub_heist | 9073508 | 133 | 872 | 189 | 222 | 1111 | 1786 | 48816 | 1988 |
| 6 | 0x047 | f_nightclub_moulinrouge | 10018371 | 161 | 751 | 181 | 241 | 1211 | 2028 | 61163 | 2003 |
| 7 | 0x049 | f_nightclub_waterpump | 8588023 | 182 | 595 | 84 | 207 | 958 | 1667 | 46566 | 2007 |
| 8 | 0x052 | i_palace_ext | 19256085 | 243 | 2213 | 530 | 388 | 2048 | 3958 | 137172 | 1893 |
| 9 | 0x055 | i_palace_int | 13362757 | 194 | 1054 | 326 | 252 | 1379 | 2471 | 74235 | 1765 |
| 10 | 0x04F | i_palace_basement | 10469368 | 195 | 797 | 142 | 233 | 1195 | 2116 | 54894 | 1928 |
| 11 | 0x04C | i_palace_ballroom | 11149894 | 162 | 685 | 163 | 212 | 1120 | 2307 | 57200 | 1846 |
| 12 | 0x057 | i_temple_ext | 20134199 | 287 | 2122 | 337 | 453 | 2061 | 4107 | 125281 | 1847 |
| 13 | 0x05A | i_temple_int | 13035526 | 227 | 1075 | 255 | 327 | 1391 | 2460 | 74995 | 1878 |
| 14 | 0x082 | p_prison_ext | 18935089 | 284 | 2611 | 529 | 462 | 2105 | 3697 | 111692 | 1921 |
| 15 | 0x085 | p_prison_int | 12092778 | 197 | 1282 | 392 | 305 | 1431 | 2349 | 62030 | 1834 |
| 16 | 0x088 | p_prison_vault | 6576240 | 85 | 412 | 16 | 124 | 582 | 1090 | 32152 | 2000 |
| 17 | 0x073 | p_castle_ext | 18503588 | 251 | 2493 | 664 | 437 | 2172 | 3888 | 117899 | 2468 |
| 18 | — | (no world in this row) | | | | | | | | | |
| 19 | 0x07C | p_castle_waterrails | 7307212 | 127 | 560 | 100 | 142 | 686 | 1241 | 34335 | 2092 |
| 20 | 0x076 | p_castle_guardbreak | 9196806 | 139 | 564 | 73 | 172 | 874 | 1656 | 49565 | 2294 |
| 21 | 0x07F | p_castle_wolftomb | 7959118 | 125 | 590 | 80 | 180 | 748 | 1359 | 40193 | 2126 |
| 22 | 0x079 | p_castle_heist | 12512703 | 206 | 918 | 202 | 330 | 1534 | 2737 | 74170 | 2207 |
| 23 | 0x06D | p_castle_ewoktrainer | 7694241 | 118 | 450 | 30 | 152 | 656 | 1251 | 37393 | 2097 |
| 24 | 0x06A | p_castle_ewoktomb | 7747669 | 127 | 496 | 40 | 155 | 663 | 1254 | 36817 | 2101 |
| 25 | 0x067 | p_castle_ewokhall | 7465847 | 116 | 474 | 39 | 161 | 683 | 1247 | 35945 | 2103 |
| 26 | 0x070 | p_castle_ewokwater | 7207764 | 126 | 509 | 50 | 151 | 650 | 1193 | 33235 | 2100 |
| 27 | 0x036 | c_train_ext | 16163212 | 242 | 1930 | 352 | 357 | 2055 | 3620 | 101070 | 1660 |
| 28 | 0x033 | c_train_cabins | 9013974 | 157 | 631 | 176 | 217 | 965 | 1665 | 42683 | 1766 |
| 29 | 0x02D | c_train_a | 11149598 | 191 | 721 | 190 | 267 | 1174 | 2138 | 61959 | 1710 |
| 30 | 0x02F | c_train_b | 11607104 | 207 | 742 | 181 | 278 | 1231 | 2270 | 65063 | 1632 |
| 31 | 0x031 | c_train_c | 10878580 | 179 | 665 | 173 | 230 | 1112 | 2060 | 59397 | 1620 |
| 32 | 0x024 | c_sawmill_ext | 17949071 | 281 | 2169 | 428 | 427 | 2232 | 3818 | 108389 | 2351 |
| 33 | 0x02A | c_sawmill_mulch | 8295069 | 146 | 771 | 132 | 198 | 873 | 1456 | 39890 | 2125 |
| 34 | 0x021 | c_sawmill_burn | 8790553 | 147 | 745 | 105 | 192 | 940 | 1569 | 43716 | 2201 |
| 35 | 0x027 | c_sawmill_lighthouse | 8766035 | 172 | 873 | 131 | 198 | 938 | 1597 | 42053 | 2163 |
| 36 | 0x01B | c_sawmill_bearcave | 6546990 | 94 | 539 | 30 | 138 | 631 | 1124 | 30743 | 2110 |
| 37 | 0x01E | c_sawmill_boss | 8314854 | 145 | 624 | 31 | 167 | 822 | 1449 | 38160 | 2150 |
| 38 | 0x00F | a_blimp_ext | 17101556 | 326 | 1780 | 426 | 477 | 2314 | 3725 | 108087 | 1620 |
| 39 | 0x012 | a_blimp_int | 11093784 | 214 | 718 | 135 | 294 | 1351 | 2312 | 64824 | 1624 |
| 40 | 0x00C | a_blimp_engine_room_murray | 8623225 | 158 | 560 | 106 | 205 | 961 | 1627 | 45753 | 1625 |
| 41 | 0x006 | a_blimp_engine_room_bentley | 7663411 | 120 | 465 | 38 | 180 | 739 | 1291 | 39184 | 1603 |
| 42 | 0x009 | a_blimp_engine_room_jt | 7595016 | 112 | 465 | 60 | 182 | 754 | 1364 | 41427 | 1768 |
| 43 | 0x003 | a_blimp_boss_final | 16036375 | 263 | 885 | 221 | 480 | 1769 | 3528 | 114024 | 1623 |

Totals: 42,097 records, 11,250 solids, 53,994 meshes, 2,821,629 triangles [D].

## 3. Level-package class histograms [D]

| Row | Level package | Package start, end | Records | Classes (count) |
|---:|---|---|---:|---|
| 0 | `jb_intro` | `0x8FE580`, `0x109CF10` | 303 | 6 × 243, 10 × 3, 13 × 7, 41 × 4, 48 × 31, 49 × 1, 86 × 8, 89 × 2, 109 × 3, 115 × 1 |
| 1 | `dvd_menu` | `0x76E30`, `0x3E5140` | 11 | 6 × 1, 26 × 8, 48 × 1, 131 × 1 |
| 2 | `f_nightclub_exterior` | `0xBCDE00`, `0x11E0360` | 804 | 3 × 20, 4 × 239, 6 × 482, 11 × 6, 12 × 6, 13 × 18, 41 × 1, 43 × 27, 49 × 1, 89 × 2, 115 × 2 |
| 3 | `f_nightclub_cellar` | `0x7A2120`, `0xADF6A0` | 289 | 4 × 91, 6 × 186, 13 × 1, 43 × 10, 49 × 1 |

The first package of rows 0 and 2 is `jb_intro_vz` and `f_nightclub_exterior_vz`; of row 1, `abandon_icon` [D].

## 4. Detailed vectors

- World row 3: object-stream.md 6, classes/solid-object.md 4, geometry.md 10, transforms.md 5.

## 5. Whole-disc layout check [D]

Re-run on 2026-11-14 against the layouts in [object-stream.md](object-stream.md) and [classes/](classes/): a
scratch walker that implements those layouts read **every object body once, at every depth**, and checked that each
body ends exactly at its declared size. A placement counts as one body; its copy extras are counted separately.
Conditional records (record kind > 0) are parsed with their class's layout as well.

| Where the body is | Bodies |
|---|---:|
| package object lists (top level) | 41,889 |
| nested in solid bodies, at every depth (nested solids each counted) | 54,091 |
| part lists of explosion groups (class 31) | 24,890 |
| extra objects of top-level copy placements | 208 |
| extra objects of copy placements nested in solid bodies | 75 |
| **all bodies that end exactly at their declared size** | **121,153** |
| bodies that do not | 0 |

Per class, the three classes whose every-depth total differs from the class table of object-stream.md 3 (whose
columns cover only package lists with top-level copy extras, and solid bodies) [D]:

| Class | Top level | Nested in solids | Explosion-group lists | Top-level copy extras | Extras of nested copies | Total |
|---:|---:|---:|---:|---:|---:|---:|
| 6 | 8,546 | 8,057 | 2,595 | 5 | 56 | 19,259 |
| 13 | 258 | 92 | 0 | 3 | 3 | 356 |
| 3 | 1,676 | 2,746 | 0 | 193 | 4 | 4,619 |

An earlier figure of 114,199 bodies counted differently: a solid's nested solids were not counted individually, and
placements nested in solid bodies were counted without reading their extra objects. Every body it counted also
parses in the walk above.

What "field-exact" covers, precisely: every field of every class layout is read in order, including all counted
arrays (counted arrays of fixed-size records are stepped over by count × record size, not decoded value by value).
Three parts are still stepped over by their own size field rather than parsed:

- the option blocks of fresh placements and the patches of copy placements (`classes/placement.md` 1.1, 1.2), whose
  sizes are given in the data;
- the trailing data block of a fresh placement (S − 2 bytes), whose content is not yet described.

## 6. Open questions

- The content of the fresh-placement data block (section 5).
