---
id: sly1-worlds
title: Sly 1 per-world object-stream test vectors
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      worlds decompressed (output hashes in disc.md section 5)
    used_for: every number in this file, produced by a throwaway walker that follows object-stream.md and classes/
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder
    used_for: cross-check only (the reference walker also reaches the texture payload on every world)
date: 2026-11-13
---

# Per-world test vectors for the object stream

A loader that follows [object-stream.md](object-stream.md), [classes/](classes/), [option-encodings.md](option-encodings.md)
and [geometry.md](geometry.md) must reproduce these numbers on the player's own disc. World numbers and names are
those of the executable's world table ([disc.md](disc.md)); decompressed-file hashes are in disc.md section 5, and
the outer section offsets in [world-file.md](world-file.md) section 5.

## 1. Definitions

- **Records**: every object record read, in every context of object-stream.md 5.2 (object lists, proxy sources
  written in full, state-machine animations, explosion-group members). Warp and exit animations are not counted.
- **List-loaded**: records read through an object list (top level and child lists). This is the "all depths"
  count of world-file.md section 5.
- **Top level**: the count of the world's top-level object list.
- **Max depth**: deepest nesting, the top level being depth 0 and each object list, proxy source, state-machine
  animation or group member adding 1.
- **Drawable**: records whose layout contains a mesh set with at least one mesh that has a submesh, or at least one
  instance mesh (classes/placed-object.md section 2). Equals the "non-empty mesh sets" of geometry.md 7.1.
- **Drawable, live**: drawable records not inside a stock template (OIDs 12 to 42 at the top level). All 46 worlds
  have exactly 31 stock templates, holding 27 drawable records.
- **Proxy sources / back-references**: full sources and back-references in proxies (classes/proxy.md).
- **Drawable in clones**: for each back-reference, the number of drawable objects in one clone of the referenced
  source, summed over all back-references. One clone holds exactly what the source's subtree holds once the
  source's own body has been read completely (classes/proxy.md, section 3): the source, its child-list
  descendants, the sources placed by every proxy inside it (full sources and the clones of back-references) and the
  children of single-source proxies inside it, each counted with its own descendants. The child list of the proxy that holds the source is not part of it.
  A renderer that instantiates
  clones draws drawable-live + this many objects with models.
- **Options**: option entries in every option list read: the world list, every object's list, proxy overrides,
  and the option lists at the end of every animation segment, including the animation segments owned by warps,
  exits and state machines. Typed-argument contents are not counted as options.
- **Object stream**: from the `u16` count of the top-level list to the end of the last record (end exclusive);
  the texture base T follows after padding.

## 2. All 46 worlds

| # | World | Object stream (start..end) | Records | List-loaded | Top level | Max depth | Drawable | Drawable, live | Proxy sources | Back-references | Drawable in clones | Options |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | Splash | 0x14D5..0x9DFF | 49 | 39 | 36 | 3 | 28 | 1 | 1 | 0 | 0 | 125 |
| 1 | Paris, France | 0xFCBC..0x500EEA | 1614 | 1295 | 385 | 10 | 327 | 300 | 114 | 141 | 483 | 3761 |
| 2 | The Hideout | 0x8651..0x16C2A6 | 649 | 557 | 94 | 12 | 290 | 263 | 18 | 1 | 0 | 565 |
| 3 | A Stealthy Approach | 0xD854..0x4C2268 | 1404 | 1133 | 340 | 10 | 310 | 283 | 102 | 79 | 188 | 3197 |
| 4 | Prowling the Grounds | 0xB887..0x4469EC | 1133 | 938 | 315 | 10 | 212 | 185 | 61 | 86 | 105 | 2265 |
| 5 | High Class Heist | 0xADF0..0x3B143C | 1275 | 1009 | 273 | 10 | 271 | 244 | 115 | 162 | 398 | 2639 |
| 6 | Into the Machine | 0xDA1C..0x4ABDF7 | 1574 | 1308 | 474 | 10 | 306 | 279 | 100 | 220 | 485 | 3692 |
| 7 | A Cunning Disguise | 0xAEA6..0x360585 | 1200 | 958 | 235 | 10 | 260 | 233 | 82 | 98 | 409 | 2323 |
| 8 | The Fire Down Below | 0xB11D..0x376A9A | 1342 | 1086 | 391 | 10 | 266 | 239 | 94 | 171 | 318 | 2953 |
| 9 | Treasure in the Depths | 0x358D..0x49647 | 271 | 227 | 77 | 6 | 98 | 71 | 10 | 12 | 54 | 547 |
| 10 | The Gunboat Graveyard | 0xB45A..0x3B79EF | 1270 | 1040 | 260 | 10 | 338 | 311 | 106 | 109 | 399 | 2514 |
| 11 | The Eye of the Storm | 0x7664..0x1F8D0A | 820 | 672 | 148 | 10 | 157 | 130 | 19 | 27 | 154 | 1686 |
| 12 | A Rocky Start | 0xEC30..0x58FE20 | 1486 | 1229 | 339 | 10 | 294 | 267 | 92 | 139 | 352 | 3455 |
| 13 | Muggshot's Turf | 0xDD63..0x53682D | 1166 | 999 | 305 | 10 | 235 | 208 | 54 | 89 | 174 | 2603 |
| 14 | Boneyard Casino | 0xE8AD..0x498749 | 1528 | 1257 | 445 | 10 | 278 | 251 | 93 | 229 | 614 | 3400 |
| 15 | Murray's Big Gamble | 0xE317..0x3A2FD8 | 1259 | 1047 | 322 | 12 | 206 | 179 | 43 | 64 | 203 | 2440 |
| 16 | At the Dog Track | 0xD2F1..0x37994A | 909 | 716 | 178 | 11 | 191 | 164 | 34 | 26 | 87 | 1946 |
| 17 | Two to Tango | 0x126FD..0x4230C9 | 2070 | 1780 | 530 | 10 | 448 | 421 | 117 | 282 | 718 | 3400 |
| 18 | Straight to the Top | 0xD873..0x3E9320 | 1509 | 1262 | 426 | 10 | 284 | 257 | 93 | 230 | 596 | 2982 |
| 19 | Back Alley Heist | 0xD540..0x39460A | 1417 | 1181 | 380 | 10 | 251 | 224 | 86 | 187 | 384 | 2797 |
| 20 | Last Call | 0xA24A..0x2AE154 | 1010 | 851 | 262 | 10 | 209 | 182 | 30 | 44 | 165 | 2292 |
| 21 | The Dread Swamp Path | 0xCB2C..0x46E264 | 1486 | 1239 | 375 | 10 | 244 | 217 | 90 | 147 | 371 | 3360 |
| 22 | The Swamp's Dark Center | 0xC4AC..0x596E9F | 1226 | 1059 | 341 | 12 | 272 | 245 | 46 | 79 | 200 | 3126 |
| 23 | The Lair of the Beast | 0xC38A..0x4DA721 | 1393 | 1171 | 496 | 10 | 228 | 201 | 78 | 249 | 578 | 3197 |
| 24 | A Grave Undertaking | 0xB2C1..0x51A4CE | 1469 | 1224 | 380 | 10 | 252 | 225 | 96 | 154 | 488 | 3443 |
| 25 | Piranha Lake | 0x643E..0x27E26D | 714 | 603 | 132 | 11 | 156 | 129 | 19 | 51 | 154 | 1259 |
| 26 | Descent into Danger | 0xC3E1..0x4553F7 | 1521 | 1239 | 418 | 10 | 291 | 264 | 120 | 210 | 474 | 3888 |
| 27 | A Ghastly Voyage | 0x9884..0x34EB23 | 1182 | 1033 | 393 | 12 | 229 | 202 | 52 | 260 | 1242 | 3008 |
| 28 | Down Home Cooking | 0x53C0..0x19F236 | 725 | 605 | 115 | 10 | 167 | 140 | 27 | 8 | 16 | 1387 |
| 29 | A Deadly Dance | 0xD2D8..0x421E1C | 1217 | 1044 | 248 | 10 | 232 | 205 | 57 | 107 | 226 | 3210 |
| 30 | A Perilous Ascent | 0xF12B..0x53FB6B | 1491 | 1240 | 427 | 10 | 257 | 230 | 87 | 146 | 554 | 3167 |
| 31 | Inside the Stronghold | 0xB9DF..0x4B5AAB | 1074 | 913 | 232 | 12 | 209 | 182 | 40 | 49 | 126 | 2674 |
| 32 | Flaming Temple of Flame | 0xDBA1..0x556271 | 1533 | 1291 | 427 | 12 | 259 | 232 | 93 | 207 | 503 | 3161 |
| 33 | The Unseen Foe | 0xECAE..0x4268E1 | 1434 | 1214 | 417 | 10 | 278 | 251 | 91 | 158 | 435 | 3376 |
| 34 | The King of the Hill | 0xCA6E..0x3D47F8 | 1242 | 1050 | 310 | 12 | 199 | 172 | 31 | 40 | 199 | 2834 |
| 35 | Rapid Fire Assault | 0xBF6F..0x3FB168 | 1104 | 975 | 335 | 12 | 222 | 195 | 41 | 134 | 1045 | 2649 |
| 36 | Duel by the Dragon | 0x1049C..0x552B48 | 1978 | 1687 | 488 | 10 | 307 | 280 | 111 | 255 | 581 | 4114 |
| 37 | A Desperate Race | 0x8030..0x25D7B7 | 813 | 631 | 147 | 11 | 149 | 122 | 24 | 10 | 33 | 1671 |
| 38 | Flame Fu! | 0x7C26..0x22DAEE | 1050 | 921 | 201 | 10 | 140 | 113 | 35 | 184 | 225 | 2248 |
| 39 | A Hazardous Path | 0x93B0..0x28A64B | 1026 | 848 | 298 | 12 | 180 | 153 | 35 | 155 | 212 | 2242 |
| 40 | Burning Rubber | 0x4884..0x1462E2 | 433 | 367 | 159 | 9 | 115 | 88 | 15 | 63 | 189 | 801 |
| 41 | A Daring Rescue | 0x9400..0x2C4836 | 1105 | 895 | 211 | 10 | 247 | 220 | 56 | 42 | 89 | 2563 |
| 42 | Bentley Comes Through | 0x3768..0xEDAFA | 297 | 253 | 87 | 6 | 102 | 75 | 18 | 12 | 192 | 627 |
| 43 | A Temporary Truce | 0xA5B2..0x2B0570 | 1119 | 935 | 326 | 10 | 212 | 185 | 44 | 84 | 129 | 2651 |
| 44 | Sinking Peril | 0x775A..0x2A17DF | 1054 | 890 | 102 | 10 | 262 | 235 | 43 | 117 | 171 | 2103 |
| 45 | A Strange Reunion | 0xE5C2..0x33A4AB | 1418 | 1182 | 185 | 12 | 357 | 330 | 76 | 76 | 87 | 2923 |

Totals over the disc: 54059 records, 45093 list-loaded, 2889 proxy sources written in full, 5398 state-machine
animations, 679 explosion-group members; 126 classes occur (object-stream.md section 6).

## 3. Detailed worlds

### World 1: Paris, France (the hub)

- Records 1614 (list-loaded 1295, proxy sources 114, state-machine animations 183, explosion-group members 22); top level 385; maximum depth 10.
- Drawable 327, drawable outside stock templates 300; proxy back-references 141, adding 483 drawable cloned objects.
- Options in all object option lists, world list excluded: 3750. World option IDs in order: 528, 530, 531, 532, 533, 534, 535, 529, 524, 526, 527.
- World mesh (class 4, OID 1): record at 0x12912; 1910 meshes (1413 instance meshes), 1685 submeshes, 92667 strip entries, 50394 positions.
- First four records in stream order (any context):

| Header offset | Class | OID | Script index |
|---|---:|---:|---:|
| 0xFCBE | 134 | 2 | -1 |
| 0x12912 | 4 | 1 | -1 |
| 0x24EF2A | 83 | 1355 | -1 |
| 0x24EF61 | 87 | 1356 | -1 |

Class histogram (all records, every context):

| Class | Name | Records |
|---:|---|---:|
| 131 | animation segment | 378 |
| 83 | proxy (instancer) | 233 |
| 2 | placed object | 228 |
| 79 | skeleton bone | 104 |
| 122 | explosion emitter, simple ? | 84 |
| 81 | light | 79 |
| 39 | breakable piece | 59 |
| 3 | solid object | 47 |
| 115 | transform marker | 45 |
| 101 | dialogue | 42 |
| 126 | point | 39 |
| 94 | tail / chain segment | 33 |
| 80 | particle emitter | 31 |
| 136 | state machine | 24 |
| 99 | camera zone ? | 22 |
| 117 | target marker | 17 |
| 33 | breakable | 16 |
| 41 | trigger volume button | 15 |
| 116 | warp point | 15 |
| 42 | trigger volume variant ? | 12 |
| 120 | explosion group | 10 |
| 52 | barrier | 9 |
| 100 | jump location ? | 9 |
| 78 | camera placement | 7 |
| 110 | curve shape | 6 |
| 102 | speaker | 5 |
| 82 | skeleton helper ? | 4 |
| 40 | button | 3 |
| 55 | hazard volume ? | 3 |
| 111 | curve shape variant ? | 3 |
| 124 | rat hole | 3 |
| 37 | fragile breakable | 2 |
| 4 | static world mesh | 1 |
| 7 | player character | 1 |
| 23 | vault | 1 |
| 34 | breakable alarm ? | 1 |
| 47 | alarm | 1 |
| 49 | laser sensor | 1 |
| 56 | checkpoint | 1 |
| 60 | missile | 1 |
| 61 | missile variant ? | 1 |
| 62 | homing missile ? | 1 |
| 63 | mine | 1 |
| 65 | rat | 1 |
| 71 | jump object ? | 1 |
| 76 | decoy | 1 |
| 84 | sky | 1 |
| 86 | charm | 1 |
| 87 | life token | 1 |
| 88 | coin | 1 |
| 97 | dynamic shadow | 1 |
| 123 | volume | 1 |
| 134 | visibility map | 1 |
| 139 | reach map ? | 1 |
| 140 | object variant ? | 1 |
| 141 | object variant ? | 1 |
| 142 | screen mask | 1 |
| 143 | object variant ? | 1 |
| 145 | lightning flash ? | 1 |

### World 2: The Hideout

- Records 649 (list-loaded 557, proxy sources 18, state-machine animations 70, explosion-group members 4); top level 94; maximum depth 12.
- Drawable 290, drawable outside stock templates 263; proxy back-references 1, adding 0 drawable cloned objects.
- Options in all object option lists, world list excluded: 559. World option IDs in order: 526, 527, 549, 551, 550, 525.
- World mesh (class 4, OID 1): record at 0x8653; 68 meshes (48 instance meshes), 201 submeshes, 13607 strip entries, 7871 positions.
- First four records in stream order (any context):

| Header offset | Class | OID | Script index |
|---|---:|---:|---:|
| 0x8653 | 4 | 1 | -1 |
| 0x60D22 | 83 | 1355 | -1 |
| 0x60D59 | 87 | 1356 | -1 |
| 0x63531 | 122 | 605 | -1 |

Class histogram (all records, every context):

| Class | Name | Records |
|---:|---|---:|
| 2 | placed object | 297 |
| 131 | animation segment | 141 |
| 126 | point | 90 |
| 79 | skeleton bone | 35 |
| 83 | proxy (instancer) | 19 |
| 81 | light | 15 |
| 94 | tail / chain segment | 12 |
| 122 | explosion emitter, simple ? | 8 |
| 136 | state machine | 6 |
| 103 | world map ? | 5 |
| 106 | hub progress | 5 |
| 101 | dialogue | 4 |
| 80 | particle emitter | 2 |
| 82 | skeleton helper ? | 2 |
| 120 | explosion group | 2 |
| 4 | static world mesh | 1 |
| 78 | camera placement | 1 |
| 87 | life token | 1 |
| 102 | speaker | 1 |
| 105 | hub selector | 1 |
| 142 | screen mask | 1 |

### World 3: A Stealthy Approach

- Records 1404 (list-loaded 1133, proxy sources 102, state-machine animations 151, explosion-group members 18); top level 340; maximum depth 10.
- Drawable 310, drawable outside stock templates 283; proxy back-references 79, adding 188 drawable cloned objects.
- Options in all object option lists, world list excluded: 3177. World option IDs in order: 530, 531, 532, 534, 535, 526, 527, 549, 551, 550, 549, 551, 550, 549, 551, 550, 549, 551, 550, 538.
- World mesh (class 4, OID 1): record at 0xE0BA; 1962 meshes (1648 instance meshes), 901 submeshes, 47521 strip entries, 24870 positions.
- First four records in stream order (any context):

| Header offset | Class | OID | Script index |
|---|---:|---:|---:|
| 0xD856 | 134 | 2 | -1 |
| 0xE0BA | 4 | 1 | -1 |
| 0x220877 | 83 | 1355 | -1 |
| 0x2208AE | 87 | 1356 | -1 |

Class histogram (all records, every context):

| Class | Name | Records |
|---:|---|---:|
| 131 | animation segment | 347 |
| 2 | placed object | 227 |
| 83 | proxy (instancer) | 173 |
| 79 | skeleton bone | 81 |
| 81 | light | 71 |
| 3 | solid object | 58 |
| 122 | explosion emitter, simple ? | 54 |
| 80 | particle emitter | 52 |
| 39 | breakable piece | 47 |
| 101 | dialogue | 39 |
| 126 | point | 36 |
| 115 | transform marker | 33 |
| 136 | state machine | 24 |
| 94 | tail / chain segment | 19 |
| 116 | warp point | 13 |
| 41 | trigger volume button | 11 |
| 99 | camera zone ? | 11 |
| 33 | breakable | 10 |
| 117 | target marker | 9 |
| 120 | explosion group | 8 |
| 50 | camera sensor ? | 6 |
| 82 | skeleton helper ? | 6 |
| 55 | hazard volume ? | 4 |
| 110 | curve shape | 4 |
| 112 | pipe | 4 |
| 10 | guard variant ? | 3 |
| 31 | water | 3 |
| 78 | camera placement | 3 |
| 130 | scan point ? | 3 |
| 145 | lightning flash ? | 3 |
| 54 | hazard zone ? | 2 |
| 56 | checkpoint | 2 |
| 57 | conveyor | 2 |
| 102 | speaker | 2 |
| 111 | curve shape variant ? | 2 |
| 123 | volume | 2 |
| 125 | level exit | 2 |
| 138 | path zone (navigation mesh) | 2 |
| 4 | static world mesh | 1 |
| 7 | player character | 1 |
| 23 | vault | 1 |
| 34 | breakable alarm ? | 1 |
| 46 | clue | 1 |
| 47 | alarm | 1 |
| 52 | barrier | 1 |
| 62 | homing missile ? | 1 |
| 63 | mine | 1 |
| 64 | fly ? | 1 |
| 76 | decoy | 1 |
| 84 | sky | 1 |
| 86 | charm | 1 |
| 87 | life token | 1 |
| 88 | coin | 1 |
| 89 | key | 1 |
| 90 | gold prize ? | 1 |
| 92 | lock | 1 |
| 93 | lock group | 1 |
| 97 | dynamic shadow | 1 |
| 103 | world map ? | 1 |
| 118 | handle marker ? | 1 |
| 134 | visibility map | 1 |
| 139 | reach map ? | 1 |
| 140 | object variant ? | 1 |
| 142 | screen mask | 1 |

### World 0: Splash

- Records 49 (list-loaded 39, proxy sources 1, state-machine animations 5, explosion-group members 4); top level 36; maximum depth 3.
- Drawable 28, drawable outside stock templates 1; proxy back-references 0, adding 0 drawable cloned objects.
- Options in all object option lists, world list excluded: 124. World option IDs in order: 528.
- World mesh (class 4, OID 1): record at 0x14D7; 0 meshes (0 instance meshes), 0 submeshes, 0 strip entries, 0 positions.
- First four records in stream order (any context):

| Header offset | Class | OID | Script index |
|---|---:|---:|---:|
| 0x14D7 | 4 | 1 | -1 |
| 0x1595 | 83 | 1355 | -1 |
| 0x15CC | 87 | 1356 | -1 |
| 0x3DA5 | 122 | 605 | -1 |

Class histogram (all records, every context):

| Class | Name | Records |
|---:|---|---:|
| 2 | placed object | 27 |
| 122 | explosion emitter, simple ? | 8 |
| 131 | animation segment | 6 |
| 120 | explosion group | 2 |
| 4 | static world mesh | 1 |
| 83 | proxy (instancer) | 1 |
| 87 | life token | 1 |
| 125 | level exit | 1 |
| 136 | state machine | 1 |
| 142 | screen mask | 1 |

## 4. Transform vectors

See [transforms.md](transforms.md) section 8 for world transforms of a proxy chain in Paris and in Muggshot's Turf.
World-space bounds of the origins of the drawable, live objects (section 2 rules, Z up):

| World | Min (x, y, z) | Max (x, y, z) |
|---|---|---|
| Paris, France | (-8328.4, -4665.6, -6671.0) | (8707.7, 8445.7, 2419.0) |
| The Hideout | (-1090.4, -747.7, -485.8) | (2399.7, 586.8, 418.5) |
| A Stealthy Approach | (-15830.8, -38740.9, -1645.6) | (5999.8, 1889.4, 6126.7) |

These bounds use the stored transforms only (proxy back-references not instantiated, ancestor overrides not
applied, degenerate matrices replaced by the identity).

## 5. Clarifications

Answers to questions raised while implementing against these vectors, settled on the disc.

- **Which records a clone holds.** A clone copies the source as it stands after its body is read, so nested
  proxies inside the source have already placed their content: their full sources, their back-reference clones and
  their single-source children all sit inside the source's subtree and are copied with it. The column "Drawable in
  clones" counts all of these. Counting only child lists gives 473 in Paris; adding the full sources of nested
  proxies but not their back-reference clones gives 475; the complete rule gives 483. The same three counts are
  94, 104 and 154 for The Eye of the Storm, and 18, 33 and 33 for A Desperate Race. Total over the disc: 14,805 under the full rule.
- **The source's own proxy children are not in its clones.** The pool is made before the child list of the
  proxy that holds the source is read (classes/proxy.md section 3). The only case on the disc is in Back Alley
  Heist, where each back-referencing proxy lists its own copy of the child; counting the original's child inside
  the clones as well would give 386 there instead of 384.
- **Nested back-references.** A back-reference inside a source's subtree takes one clone from the referenced
  source's pool when the source is loaded, once. Clones of the outer source are made afterwards by copying the
  whole subtree, and so contain copies of that nested clone; they do not take further clones from any pool. This is
  why the use counts of classes/proxy.md 1.3 count proxies, not copies.
- **Options in warp and exit animations** are counted (section 1).

## 6. Open questions

- The bytes after the texture payload (26 to 747 per world) are unexplained (world-file.md 6).
- The roles of many gameplay classes (marked "?" in object-stream.md section 6) are tentative.
- Several fields are parsed but not understood: the state machine's trailing `s16`, the reach map's header values,
  the purpose of the name-table count when no names are stored.
- The option encodings of the 527 option IDs that never occur on the USA disc come from the reference only.
