---
id: sly3-formats-worlds
title: Sly 3 per-world test vectors
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every number below, from walking each world to its tail and parsing every solid body
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 per-world test vectors

Sizes, hashes, group, object, placement, shader and tail counts per world are in [disc.md](disc.md) 6. This table
adds the geometry counts, over solid bodies at every depth (top level, nested, and inside placements' object lists),
counting every mesh whether or not its filter holds. "Outline edges" counts the 4-byte edge records of outline and
effect-outline meshes. [D]

| Slot | World | Groups | Top-level records | Solid bodies | Meshes | Filtered meshes | Triangles | Outline edges |
|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 0 | dvd_menu | 15 | 172 | 6 | 50 | 27 | 2,059 | 1,506 |
| 2 | i_trainer | 189 | 561 | 297 | 1,064 | 200 | 59,372 | 33,731 |
| 3 | v_ext | 339 | 3,110 | 543 | 3,540 | 1,411 | 153,330 | 44,334 |
| 4 | v_canal | 187 | 970 | 251 | 2,046 | 1,164 | 65,508 | 29,295 |
| 5 | v_apt | 130 | 958 | 211 | 774 | 95 | 35,701 | 22,402 |
| 6 | v_gauntlet | 235 | 1,601 | 342 | 1,366 | 202 | 75,524 | 38,691 |
| 7 | v_interpol | 115 | 802 | 148 | 645 | 131 | 27,649 | 19,682 |
| 8 | o_ext | 281 | 2,919 | 442 | 2,050 | 446 | 111,614 | 47,062 |
| 9 | o_quarry | 207 | 877 | 331 | 1,351 | 235 | 67,539 | 50,399 |
| 10 | o_arena | 180 | 891 | 243 | 973 | 207 | 48,161 | 30,486 |
| 11 | o_cave_a | 175 | 865 | 259 | 962 | 207 | 47,599 | 32,717 |
| 12 | o_cave_b | 157 | 729 | 236 | 899 | 207 | 50,558 | 32,800 |
| 13 | o_bar | 178 | 880 | 274 | 982 | 207 | 44,258 | 32,519 |
| 14 | o_cave_murray | 168 | 807 | 240 | 997 | 207 | 55,561 | 33,374 |
| 15 | h_ext | 347 | 2,553 | 621 | 2,754 | 536 | 153,195 | 54,023 |
| 16 | h_hotel | 174 | 995 | 247 | 1,181 | 221 | 60,877 | 42,295 |
| 17 | h_hangar_a | 115 | 654 | 175 | 649 | 95 | 29,966 | 22,479 |
| 18 | h_hangar_b | 136 | 723 | 221 | 821 | 117 | 38,705 | 26,076 |
| 19 | h_hangar_c | 176 | 885 | 253 | 1,151 | 165 | 53,556 | 35,203 |
| 20 | h_sewer | 117 | 1,021 | 173 | 824 | 95 | 59,395 | 22,710 |
| 21 | h_dogfight | 227 | 1,011 | 351 | 1,652 | 279 | 69,766 | 38,639 |
| 22 | h_hack | 32 | 528 | 118 | 1,492 | 27 | 17,513 | 18,978 |
| 23 | c_ext | 324 | 2,860 | 511 | 2,037 | 247 | 130,391 | 56,455 |
| 24 | c_intro | 181 | 1,059 | 285 | 1,122 | 201 | 58,914 | 40,915 |
| 25 | c_flashback | 118 | 864 | 185 | 659 | 95 | 36,073 | 24,106 |
| 26 | c_forest | 156 | 1,365 | 234 | 780 | 163 | 44,961 | 27,469 |
| 27 | c_apt | 84 | 639 | 98 | 397 | 61 | 16,496 | 14,847 |
| 28 | c_hall_a | 193 | 952 | 311 | 1,291 | 171 | 58,460 | 28,652 |
| 29 | c_hall_b | 221 | 1,037 | 345 | 1,325 | 199 | 56,889 | 40,331 |
| 30 | c_tilt_hall | 128 | 887 | 206 | 748 | 97 | 39,442 | 28,872 |
| 31 | p_ext | 317 | 2,595 | 591 | 2,305 | 334 | 149,254 | 51,305 |
| 32 | p_at_sea | 228 | 1,356 | 474 | 2,441 | 332 | 147,560 | 56,601 |
| 33 | p_dive | 129 | 1,367 | 313 | 1,172 | 54 | 78,887 | 16,497 |
| 34 | p_island_map | 205 | 1,241 | 339 | 1,453 | 250 | 75,546 | 44,088 |
| 35 | m_ext | 401 | 1,929 | 675 | 3,572 | 748 | 214,251 | 71,769 |
| 36 | m_underwater | 111 | 759 | 235 | 1,243 | 106 | 87,585 | 22,557 |
| 37 | m_vault | 183 | 793 | 240 | 1,011 | 211 | 48,795 | 42,290 |
| 38 | m_gauntlet | 114 | 1,299 | 261 | 1,251 | 31 | 74,162 | 6,464 |
| 39 | m_boss | 178 | 707 | 239 | 937 | 206 | 47,366 | 33,761 |

Totals: 7,151 groups, 46,221 top-level records, 11,524 solid bodies, 51,967 meshes (9,987 filtered), 2,692,438
triangles, 1,316,380 outline edges. Object bodies at every depth: 125,693 ([object-stream.md](object-stream.md)). [D]

A port's walker should reproduce every number in this table and in [disc.md](disc.md) 6 for each world.

## Collision test vector: the v_apt floor [D]

The floor of v_apt (world 5) has collision in the **level object itself**. That object is class 48, object ID 1, in
the group `v_apt` (group ID 148660), which no placement places, so its coordinates are world coordinates. It has
one collision mesh with **1,390 vertices and 1,356 surfaces**, transform index −1 (no transform; the positions are
used as they are). Its z range is −244 to 1,929.

In the coffee-shop area (x −3,700 to −1,300, y 2,800 to 7,100) the mesh has 302 vertices. 56 of them lie on the
floor at z = −39.4, for example:

| Index | Position |
|---:|---|
| 476 | (−2,873.5, 4,495.9, −39.4) |
| 477 | (−2,947.0, 4,361.0, −39.4) |
| 480 | (−1,974.7, 3,344.9, −39.4) |
| 483 | (−2,422.3, 4,167.0, −39.4) |

The props there stand on that floor: the coffee stations are placed at z −36, the displays at −35 and the roasters
at −39 or −40.

A walker that finds no floor collision around z −40 has misread this mesh. The likeliest cause is treating each
surface as the triangle of its three corners: surfaces are convex polygons whose outline is their edge list
([classes/solid-object.md](classes/solid-object.md) 2.1; the coffee-shop floor includes the 10-sided surface 281).
A vertex offset or the transform index are other possible misreadings. No other class or option supplies the floor. Option set 12 is chosen at load (condition 12), and
the level object is not filtered by it.
