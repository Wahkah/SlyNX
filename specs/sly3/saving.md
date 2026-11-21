---
id: sly3-behaviour-saving
title: Sly 3 saving - the save-variable tree, the progress block, memory-card names - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), the save-variable section of all 39 worlds; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      the names, records and tree of the save variables; the memory-card directory and file names in the
      executable at 0x323A00 to 0x323B10 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/card3.h, savedata.cpp
    used_for: the slot size; the statement that Sly 3 keeps Sly 2's save mechanism; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 saving

Read [../sly2/saving.md](../sly2/saving.md) first: the section layout, the 10-byte records, the value kinds, path
lookup, the live progress block, autosave and the layout check carry over [R]. Tags as in [guards.md](guards.md).

## 1. The save-variable section [D]

Layout as Sly 2 ([world-file.md](world-file.md) 3.3). All 39 worlds carry the **same** section:

| Item | Sly 2 | Sly 3 |
|---|---:|---:|
| Names | 960 | **1,201** |
| Records | 5,461 | **6,559** |
| Used size of the block (the terminator's offset field) | 20,632 | **21,764** |
| Layout checksum (sum of the record-table bytes) | 2,582,452 | **2,978,085** (`0x2D7125`) |
| Record-table SHA-256 | | `6f8dd2fd21595151534f82f90d7e7d770a68ac91ca8bcba6c7e25b8038450988` |

Every record is reachable from the root (record 0); the last record is the terminator (−1, −1, 0, 21,764, 0, 0).
Tree depth: 1,161 entries at depth 1, 5,363 at depth 2, 33 at depth 3.

Value kinds used: group (1,167 including root and terminator), integer (2,980 single, 7 arrays), object ID (10),
float (267), flag (2,121 single, 5 arrays), world ID (2).

## 2. What the tree holds [D]

| Path | Kind | Offset | Notes |
|---|---|---:|---|
| `episodeCurrent`, `episodeBest` | integer | 0, 4 | |
| `widCurrent` | world ID | 8 | |
| `oidCheckpointForce` | object ID | 12 | new: replaces Sly 2's `oidPoCurrent` |
| `widCheckpointForce` | world ID | 16 | new |
| `dt` | float | 20 | total play time |
| `suck` | integer | 24 | one value (Sly 2 kept three floats under `suck_history`) |
| `flags/…` | | 28 to 80 | `lang`, `nMixerMode`, `fAutosaveWarned`, `uMusicVolume`, `uEffectsVolume`, `fUseHeadset`, `dxScreenCenter`, `dyScreenCenter`; per controller (`joyLeft`, `joyRight`): `fNoVibration` and two 7-flag arrays `mpcamprefkfReverseHorizontal` / `…Vertical` (camera inversion per camera kind) |
| `sly`, `bentley`, `murray`, `shaman`, `panda`, `penelope` `/apukCur` | 3 integers each | 84, 96, 108, 120, 132, 144 | gadget slots; three recruits have slots too |
| `powerUps/mppukfAcquired` | 67 flags (bits) | 156 | owned items, indexed by item-table record ([characters.md](characters.md) 3.1) |
| `powerUps/fAcquiredWebcamWeapon` | flag | 168 | new |
| `loot/ccoin` | integer | 172 | coins |
| `battle/cSunk`, `battle/cCaptured` | integer | 176, 180 | ship battles [I] |
| `hack_multi/iBoard` (36 integers), `hack_multi/nHighScore` | integer | 184, 328 | the hacking mode's boards and high score [I] |
| `episode/<name>/dt`, `/config`, `/found_treasure` | float, object ID, flag | 332 to 436 | nine episodes: `MultiPlayer`, `Intro`, `Trainer`, `Venice`, `Outback`, `Holland`, `China`, `Pirate`, `Isle`; 12 bytes each |
| 76 jobs `m<n>_…` | | 440 to 1,652 | `focus_count`, `is_complete`, `time_played`, `suck_value` (16 bytes each; as Sly 2) |
| 965 tasks `t<n>_…` | | 1,656 to 20,952 | `focus_count`, `complete_count`, `is_complete`, `state`, `from_memcard` (20 bytes each; as Sly 2) |
| 101 challenges `w<n>_…` | | 20,956 to 21,760 | new: `high_score` (float), `is_complete` (flag), 8 bytes each |

Job prefixes `m0` to `m6` hold 8, 11, 11, 14, 12, 12, 8 jobs; challenge prefixes `w0` to `w6` hold 1, 18, 19, 20,
14, 16, 13. The 101 challenges match the 101 class-170 records of the menu world
([gameplay-overview.md](gameplay-overview.md) 5) [I], and the job-complete screen's "Replay Challenge" text
([user-interface.md](user-interface.md) 3).

**Gone from Sly 2** [D]: `oidPoCurrent`, the per-character `hpCur`, `mpCur` and `oidBooty`, the loot counts
`mplootkc`, and the per-level `booty`, `afClue` and `fVault` entries. Loot is not kept as counts
([pickpocket-and-pickups.md](pickpocket-and-pickups.md)); health and power are not saved.

Examples (records: name ID, kind, count, offset, first child, next sibling):

- `loot/ccoin`: (66, 0, 0, 172, 0, 0)
- `powerUps/mppukfAcquired`: (63, 4, 67, 156, 0, 39)
- `sly/apukCur`: (56, 0, 3, 84, 0, 0)
- `oidCheckpointForce`: (37, 2, 0, 12, 0, 5)
- `w1_ball/high_score`: (1134, 3, 0, 20964, 0, 6260)
- `t0_ascend_complete/state`: (167, 0, 0, 1668, 0, 470)

## 3. When the game saves and restores [R]

As Sly 2 section 3, with the paths above. The layout check compares the slot's check words with 21,764 and
2,978,085.

**Cached keys** [X]: the routine `0x0025B1B8` looks up the common paths once and keeps their handles in the game
state (`0x00475480`):

| Offset | Path |
|---:|---|
| +0x406C | `dt` |
| +0x4070 | `widCurrent` |
| +0x4074 | `episodeCurrent` |
| +0x4078 | `episodeBest` |
| +0x407C | `oidCheckpointForce` |
| +0x4080 | `widCheckpointForce` |
| +0x4084 | `loot/ccoin` |

It also caches per-world and per-episode handles (`episode/<name>`, `…/dt`, `…/found_treasure`, `battle/cSunk`, …).

**The current job and task keys are not saved** [X]. The game state's +0x1498 (the current job's OID) and +0x149C
(the current task's OID), at `0x0036DB98` / `0x0036DB9C`, have **no save-tree path**. The tree's only object-ID
entries are `oidCheckpointForce` and the nine `episode/<name>/config`.

- Setting the current job (`0x0018AA48`) and the current task (`0x0018AAB8`) write the OIDs there, or −1 for none.
- The game's initialisation (`0x00184968`) clears the 5,288-byte block and sets +0x1498, +0x149C, +0x14A0 and
  +0x14A4 to −1.
- `0x0025BDD0`, in the world-start path, keeps the two values across its own job and task reset and writes them
  back afterwards.
- So the keys carry the current job and task **across world loads within a session**. After loading a memory card
  they start at −1, and the world start's 1399 pass ([missions.md](missions.md) 4.1a) chooses the current task.

**`oidCheckpointForce` / `widCheckpointForce`** [X] hold a **task's object ID and its world**, not an arrival point:

- Three routines write the pair: `0x00164228`, `0x00186338` and `0x00187270`. They write a task's ID (its field
  +0x18) and its world (field +0xAC). `0x00187270` is the job-completion routine: it takes the **resume task
  (option 1366) of the job that follows** ([missions.md](missions.md) 4.1) [X]. `0x00186338` also clears the pair to (0, 0).
- The episode start routine (`0x00289198`, [level-flow.md](level-flow.md) 3) reads the pair when starting
  **episode 8**. If the saved task is 0 or −1 it starts task 1841; otherwise it starts the saved task in the saved
  world.
- *Interpretation:* the final episode resumes at a forced checkpoint task, because its jobs run across several
  worlds [I].

**`episode/<name>/config`** (object ID): the executable has no string `config` [X: search of the whole file]. So no
game code reads or writes it by path; only world data (scripts or task records that name save paths) can. No use was
found; its meaning stays open.

## 4. Memory-card files [X] [R]

The executable's names (`0x323A00` to `0x323B10`) [X]:

| Item | Value |
|---|---|
| directory | `/BASCUS-97464YAOTWTD!` |
| directory's name file | `/BASCUS-97464YAOTWTD!/BASCUS-97464YAOTWTD!`, holding the text `Sly 3` (string at `0x323A48`) [R for the contents] |
| icon | `/BASCUS-97464YAOTWTD!/icon.sys` (magic `PS2D`) and `/BASCUS-97464YAOTWTD!/memcard_icon.ico` |
| slots | format `/%s/%s%d` with `BASCUS-97464YAOTWTD!` and `save`: `save1` to `save3` [R for the numbering] |

Slot file: **26,632 bytes** (`0x6808`) [R]: *interpretation:* a 26,624-byte (`0x6800`) block, of which the first
21,764 bytes are described by the tree, then the layout word and the layout checksum, as in Sly 2 [I].

## 5. Open questions

1. The slot-file layout and the icon file: write a save in an emulator and compare.
2. What `episode/…/config`, `suck` and the `battle` and `hack_multi` values hold, and the exact moments the three
   routines of section 3 set or clear the forced checkpoint.
3. Where the memory-card icon comes from on the disc.
4. The autosave points of Sly 3.
