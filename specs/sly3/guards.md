---
id: sly3-behaviour-guards
title: Sly 3 guards and swarming creatures - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: where guard-family templates sit and their option values (every record, every depth); the pocket tables at the addresses given
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/guard.cpp, guard3.cpp, guardstart.cpp, detect.cpp, ainoise.cpp, aipursuit.cpp, aihit.cpp, pocket.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 guards and swarming creatures

Read [../sly2/guards.md](../sly2/guards.md) first: Sly 3 keeps Sly 2's behaviour engine (job stack, detector, noises,
action objects, pockets) [R]. This file gives what differs and the Sly 3 numbers. Tags: [D] disc, [X] executable
data at an address (after the SHA-256 check of [disc.md](disc.md) 1), [R] reference behaviour, [I] inferred. Option
numbers are Sly 3 stream option IDs ([object-stream.md](object-stream.md) 1); the "Sly 2 option" column gives the
option of ../sly2/guards.md whose meaning carries over [R].

## 1. Classes and templates

| Sly 3 class | Sly 2 class | Role | Records [D] |
|---:|---:|---|---|
| 58 | 56 (guards) and 57 (swarmers) | **every guard and swarming creature**: Sly 3 has no separate swarmer class | 65 templates in 28 worlds |
| 59, 60 | 59, 63 | Dimitri and Carmelita (the guard family's loader; [characters.md](characters.md)) | 6, 10 |
| 61 to 67 | 61 to 67 | bosses and story characters on the same engine | 2 to 4 each |

Templates by group (the species is the group, as in Sly 2) [D]:

| Episode | Groups (worlds) |
|---|---|
| prologue, Venice | `doberman_guard` (i_trainer, v_ext, v_gauntlet), `ecto_swarmer` (6 worlds), `endo_swarmer` (v_ext, v_gauntlet), `npc_ape` (v_ext, v_interpol, o_quarry, m_ext) |
| Outback | `npc_dingo` (10 worlds, also the pirate worlds), `npc_kangaroo` (6), `npc_mask` (4), `o_croc`, `o_scorpion` |
| Holland | `npc_ram_guard`, `npc_pig_swarmer` (3), `npc_bunny_swarmer` (2), `npc_belgium`, `npc_iceland` (h_hotel) |
| China | `npc_tiger_guard` (3), `npc_monkey_swarmer`, `squat_swarmer` (also m_vault), `npc_small_dragon` (2), `flashback_panda_king` (c_flashback, c_apt), `c_forest_dragon`, `c_underground_zombies` |
| pirates | `npc_hound_guard` (2), `npc_croc` |
| Kaine Island | `npc_elephant_guard` (2), `npc_gator_crab` (2), `npc_mutant_shark` |

How the live guards are made from the templates (placements, spawners) is Sly 2's rule [R]
([classes/placement.md](classes/placement.md)).

## 2. Options on the templates

All class-58 records, at every depth [D] (values: the three commonest; "list" = a typed argument list, shown as its
first words read as integers):

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 267 | 1698 | 28 | list (1, 451, 0) ×58; list (17, 1998, 0) ×57; list (18, 8888, 0) ×53 | 243 |
| 53 | 307 | 26 | list (13107200, 2) ×38; list (2097152, 10) ×38; list (327752, 32) ×36 | 53 |
| 271 | 280 | 22 | list (262144, 24874, 0) ×47; list (1024, 24874, 0) ×46; list (8192, 8810, 0) ×36 | 248 |
| 8 | 75 | 26 | list (1823, 13792) ×43; list (2057, -1) ×17; list (1823, 42034) ×4 | 8 |
| 269 | 70 | 20 | list (1024, 8830, 0) ×37; list (2048, 8808, 0) ×31; list (32768, 25221, 0) ×2 | — |
| 372 | 68 | 20 | list (0, 3, 1, 1, 1, 67108863) ×28; list (1, 3, 1, 1, 1, 67108863) ×28; list (0, 2, 1, 1, 1, 67108863) ×6 | — |
| 33 | 67 | 28 | 0 ×67 | 33 |
| 255 | 66 | 28 | 9444 ×37; 36101 ×15; object/id ×4 | 226 |
| 937 | 65 | 28 | 40 ×42; 100 ×12; 70 ×4 | — |
| 348 | 64 | 27 | 1 ×64 | 332 |
| 61 | 63 | 28 | 0.5 ×52; 1 ×10; 0.001 ×1 | 59 |
| 64 | 59 | 27 | 1 ×59 | — |
| 270 | 59 | 22 | list (262144, 24872, 0) ×48; list (65536, 24868, 0) ×8; list (65536, 145005, 0) ×3 | 247 |
| 35 | 55 | 24 | (0,0,-3000) ×55 | 35 |
| 264 | 55 | 27 | 0 ×39; 1 ×12; 2 ×4 | — |
| 332 | 53 | 24 | 9143 ×37; 8701 ×11; 36099 ×4 | 315 |
| 277 | 52 | 25 | 8891 ×52 | 259 |
| 344 | 51 | 23 | 8695 ×41; 46672 ×10 | 327 |
| 319 | 49 | 23 | 8377 ×49 | 304 |
| 320 | 49 | 23 | 8375 ×49 | 305 |
| 347 | 49 | 22 | 17119 ×49 | 331 |
| 62 | 48 | 25 | 1 ×48 | 60 |
| 321 | 47 | 23 | 8479 ×47 | 306 |
| 367 | 47 | 22 | 24818 ×47 | — |
| 368 | 47 | 22 | 0 ×46; 3 ×1 | — |
| 329 | 43 | 21 | 0 ×43 | 312 |
| 315 | 37 | 20 | (300,1200) ×37 | 298 |
| 268 | 29 | 20 | list (13, 8940, 8938, 8944, 8942) ×27; list (13, 8940, 25227, 8944, 8942) ×2 | 244 |
| 337 | 24 | 16 | 1 ×24 | 320 |
| 309 | 21 | 15 | 700 ×11; 900 ×6; 100 ×3 | 293 |
| 310 | 21 | 15 | 1000 ×15; 101 ×3; 1100 ×2 | — |
| 32 | 20 | 6 | 1 ×20 | 32 |
| 49 | 17 | 10 | object/id ×13; 4 ×4 | 49 |
| 265 | 16 | 6 | list (71860) ×2; list (68008) ×2; list (68006) ×2 | 233 |
| 338 | 16 | 14 | 1 ×16 | 321 |
| 298 | 8 | 8 | 0 ×8 | 280 |

Meanings that the values confirm [D] [I]:

- **937 = maximum hit points** [X]. Its setter `0x001F43F8` stores the value at actor +368 (the maximum), then
  clamps the current hit points (+364) to the range 0 … maximum. Setting the option does not refill the current value.
  The power meter is the next pair (+372 current, +376 maximum; [gadgets.md](gadgets.md) 2). Values on class 58: 40
  on 42 templates, 100 on 12, 70 on 4, and **99,999,999** on the 4 `npc_mask` templates [D]. Other classes [D]:
  1,000 on `flashback_panda_king` (class 58, nested), `v_lion_boss` (62) and `npc_le_fwee` (66); 10,000 on
  `npc_wolf` (61); 1,250 Muggshot (64); 600 Carmelita (60); 360 the Panda King (59); 60 on class 53. No code
  treats a large value specially [I: not searched]. 99,999,999 is out of reach of ordinary hits, so those
  characters cannot be beaten by damage and are ended by script [I]. The bosses' 1,000 is a large pool, with
  scripted phases deciding the fight [I].
- **309 / 310 = near / far pursuit range**: 700 / 1,000 on most guards, as Sly 2's 292 / 293 (the reference's table
  pairs 309 with 293; the values say 309 is the near range) [D] [I].
- **332 = view volume** (Sly 2 315), 333 the second volume (316); 344 the home object (327); 319, 320, 321 the two
  helpers and the pocket object (304, 305, 306); 347 Murray's grab point (331); 329 the sight flags (312), 330 / 331
  always / never sees (313 / 314); 337 / 338 strike without turn-taking / attack off screen (320 / 321); 315 the shot
  range, 300 … 1,200 as in Sly 2 (298); 313 / 314 the circling distances (296 / 297); 349 no surprise knock-out
  (333). [R for the pairing, D for the values]
- **267** is the animation table (Sly 2 243); 268, 270 and 271 are the other animation entries (244, 247, 248) [R],
  and 269 has the same list shape, probably Sly 2's 246 [I].
- 35 = gravity (0, 0, −3000) on 55 templates, as Sly 2's boar and pelican guards. [D]
- 372, 367, 368, 376 to 379, 948: new options without a Sly 2 counterpart; meanings open.

## 3. Pockets [X]

Same record layout as Sly 2 (`s32` fewest coins, `s32` most coins, `f32` prize chance, six `s32` weight and `s32`
prize ID pairs; 60 bytes), with a zero record first:

| Table | Virtual address | Records 1 to 5 |
|---|---|---|
| other givers | `0x346BA8` | 4 … 8 coins, chance 0.2, weights 30, 30, 15, 15, 5, 5 |
| guards | `0x346DC8` | 6 … 12 coins, chance 0.4, same weights |

Prize IDs are 1258 to 1284 (Sly 2: 1114 to 1143). Record 1 of the guard table holds prizes 1263, 1262, 1260, 1259,
1266, 1265; of the other table 1261, 1262, 1258, 1259, 1264, 1265. **Row = episode − 2** [X: the routine at `0x001F1A30` adds 60 × the current episode to the table's address minus
120; an episode of −1 counts as 3]: Venice (3) uses record 1 … the pirates (7) record 5; the training world (2) and
Kaine Island (8) land on the empty records 0 and 6 (no coins, no prize). Givers whose pocket kind is 1 or 2 use the
guard table, all others the first table [X]. The filling rule is Sly 2's [R].

## 4. Everything else

Sight (the detector), hiding, noises and their templates, alarms, patrol, pursuit, action objects, shots, hits and
knock-out follow Sly 2 sections 3 to 7 [R]. **Noise templates are class 139** [D] [X]. The reference's correspondence pairs Sly 2's class 117 with Sly 3's
140 (medium confidence), but no class-140 record exists on the disc. Class 139 (491 records in all 39 worlds, 7
top-level, 484 nested in groups such as `bentley_dart`, `booty` and `ninja_ball`) carries the template options
1184 to 1188. Those options write fields +0x44 to +0x54, as the template class's fields do [X]. Read in order
against Sly 2's 1090 to 1094 (Sly 2 [guards.md](../sly2/guards.md) 4.3) [I from the order and the values]:

| Option | Kind [X] | Values [D] | Meaning (Sly 2 option) |
|---:|---|---|---|
| 1184 | | 0 ×454 | perception: 0 heard within a radius (1090) |
| 1185 | float at +0x44 | 1,000 ×129, 1,500 ×108, 300 ×107 | radius (1091) |
| 1186 | float pair at +0x48 | (−250, 250) ×88, (−340, 300) ×36, (−2,000, 2,000) ×35 | height range (1092) |
| 1187 | float at +0x50 | 0 ×124, 0.25 ×107, 0.1 ×107 | lifetime in seconds (1093) |
| 1188 | value type `0x103E` at +0x54 | 2 ×214, 3 ×129, 4 ×40 | kind (1094) |

Option **1189** is the "make a noise here" method (two arguments; it returns an object of the run-time class
183, value type `0x20B7`; wrapper `0x0010C5D8`) [X], Sly 2's 1095 [R]. The executable constants of Sly 2 section 10 and the fixed noise
template IDs were not re-located in Sly 3 (open question).

## 4a. Guard command lists (class 134) and command `0x62` [X] [D]

**Command `0x62`** (object, list), handler `0x00228A8C`, calls `0x00198540`, the reference's TellGuardCommands. It
takes or creates the guard's AI job of kind 200 (`0x002387A8` on the AI at +0x29C), and sends it message
{5, list} (`0x0019A2D0`). If the command's flag bit `0x100` is set, the runner keeps the guard and the list at
+0xFC / +0x100. `0x63` is the second kind (`0x001985A8`); its list may be −1.

**A command list is a class-134 object** (Sly 2's class 113; 2,376 bodies). Each stream option appends one command
when the object loads. The handlers (`0x00107E38` to `0x00108870`) call builders that append these kinds [X]:

| Option | Kind | Arguments | Sly 2 command (options-by-class 2.2) [R] |
|---:|---:|---|---|
| 729 | 0 | object | set the context object |
| 730 | 1 | range | how long the list runs before a pause |
| 731 | 2 | range | how long the guard then stands |
| 732 | 3 | object, number | look at the object |
| 733 | 4 | object | send a message to the object |
| 734 | 5 | seconds, object | wait (Sly 3 adds an object) |
| 735 | 6 | path, direction, face object, value, flag | follow a path (builder `0x001973B8`) |
| 736 | 7 | object, speed, …, flag | go to the object at the speed (disc: 40) |
| 737 | 8 | object | play the clip |
| 738 / 739 | 9 / 10 | number | label / jump to the label |
| 740 | 13 | object, flag, value | use an object (a ladder, rail or similar) |
| 741 / 742 | 11 / 12 | — | lock / unlock |
| **743** | **14** | clip, flags | play an animation and wait for its end |
| **746** | **15** | script, alarm | **set the numbered alarm on a script** |
| 744 / 745 | 17 / 18 | one value | new in Sly 3 (not established) |

Commands 5, 6, 7, 13 and 14 hold the list until they are done (time, arrival, use, animation) [R]. Pathing,
speeds and the arrival test are Sly 2's guard movement [R]; they are not re-traced here.

**The lion in m1_follow** [D]. Script 2243 gives lion 139555 the list **2244**, then waits on alarm 139577
(`0x0E`). List 2244 is:

1. 743 (140742, 0): play clip 140742;
2. 735 (140743, 1, −1, −1, 0): walk path 140743 forwards, facing along it;
3. **746 (2243, 139577): set alarm 139577 on script 2243**;
4. 743 (451, 0): play clip 451, the idle.

So **the list itself sets the alarm when the lion reaches the path's end**. Script 2243 then goes on: it starts
2246 and requests task 2242 → 2.

## 4b. Knocked-out guards leave the world [D] [R] [X]

A knocked-out guard **removes itself from the world at the end of its knock-out clip**, through an animation event
rather than a timer or a flag:

- Animation event **kind 8**, "remove an object", has the payload (object ID, fade time). The reference's dispatch
  is `0x00138270`, which calls `0x0025D740`, the same add / remove routine that command `0x39` uses. Kind 7 adds.
  The object is found **near the clip's owner**, so a pooled guard removes **its own instance**, not the first
  copy with that ID [R]. With a fade time above 0 the guard fades out and is then removed; with 0 it is removed at
  once.
- **Disc check, v_gauntlet** (`ecto_swarmer`, template 56250). Clips that end in removing 56250 itself, with the
  event time and fade:

  | Clip | Length | Event time | Fade |
  |---:|---:|---:|---:|
  | 56542 | 1.0 s | end | 0 |
  | 56544 | 2.33 s | 0.92 s | 0.4 |
  | 56546 | 1.5 s | 1.17 s | 0.3 |
  | 56550 | 1.75 s | 1.42 s | 0.5 |
  | 56552 | 1.0 s | 0.92 s | 0 |
  | 56554 | 2.0 s | 1.67 s | 0.6 |
  | 56556 | 1.0 s | 1.0 s | 0 |
  | 51423 | 1.58 s | 0.42 s | 0.4 |
  | 25217 | 1.0 s | end | 0 |
  | 9170 | 2.33 s | 0.92 s | 0.4 |

  These are the knock-out and defeat clips [I for which clip plays for which hit].
- **Triggers then lose the guard.** A trigger counts only members that are in the world (`0x001C8610`: byte +0x2F
  bit 0, and the object on the world's root list `0x00479278`), and the member scan (`0x0015D2F0`) also tests the
  shape. So when the guard leaves the world, the next scan drops it, and event 9 follows. The scan itself has no
  knocked-out test: a knocked-out guard **still counts until its clip removes it**.
- A port must run the clip's kind-8 event on the guard's own instance. A guard that stays in the world after its
  knock-out keeps triggers such as v_gauntlet's 139753 occupied (missions.md 4.1a, lasers 2).

## 5. Test vectors [D]

- 65 class-58 records in 28 worlds; v_ext holds four templates (`doberman_guard`, `endo_swarmer`, `ecto_swarmer`,
  `npc_ape`).
- Option 937 values: 40 ×42, 100 ×12, 70 ×4.
- `0x346E04` (guard pocket record 1): (6, 12, 0.4, 30, 1263, 30, 1262, 15, 1260, 15, 1259, 5, 1266, 5, 1265).

## 6. Open questions

1. The meanings of the new options (372, 367, 368, 376 to 379, 948) and the roles of 264 and 255.
2. The noise templates' class, fixed IDs and constants in Sly 3.
3. Which pocket record each world uses.
4. Every [R] rule needs checking in the running game.
