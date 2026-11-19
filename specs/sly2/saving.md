---
id: sly2-behaviour-saving
title: Sly 2 saving - the progress tree and the memory-card files
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; all 43 decompressed worlds; executable SCUS_973.16 (SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73)
    used_for: >-
      the save-variable section parsed in all 43 worlds (identical table in each); record kinds, offsets, the tree
      and its paths; the card directory and file name strings, the memory-card icon file record and its bytes
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9 (2026-11-12), src/new_1d7780 (save-variable lookup, get and set, snapshot, layout
      check), src/memcard and src/memcard-memory (slot paths, slot read and write, card set-up, scan, autosave,
      newest-slot choice), nonmatching/memcard (icon description, card set-up)
    used_for: the record-kind meanings, the layout check, the card file set and sizes, when the game saves; marked [R]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release (task and save handling)
    used_for: cross-check of the restore order and the autosave points [R]
  - kind: executable
    what: SCUS_973.16 (hash checked), option table at 0x002AC880, class option map
    used_for: 2026-11-18 re-check of options 790-795 and the job and task options (stream options or script methods)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/task.cpp, frontend.cpp, transition.cpp
    used_for: 2026-11-18 answers on arrival-point lists, the new-game start, the current job and task, the episode save group; marked [R]
reviewed_by: pending
date: 2026-11-17
---

# Sly 2 saving

Delta of the saving part of [../sly1/pickups-and-progress.md](../sly1/pickups-and-progress.md). Sly 2 keeps all
progress in one **progress block**: a fixed-size byte area whose layout is described by a table carried in every
world file, the **save-variable section** ([world-file.md](world-file.md) 3.3). Each memory-card save slot is that
block plus two check words. **No real memory-card image written by the game has been examined: the card-file byte
layout below comes from the data tables and the reference and is unverified on a real card [R].**

## 1. The save-variable section

Layout ([world-file.md](world-file.md) 3.3) [D]:

| Field | Type | Notes |
|---|---|---|
| name count `n` | `u16` | 960 |
| names | `n` × (`u16` name ID, `u16` length, bytes) | ASCII, no terminator; for example `ccoin`, `afClue` |
| record count `m` | `u16` | 5,461 |
| records | `m` × 10 bytes | below |

All 43 worlds carry the **same** section: the 54,610-byte record table has SHA-256
`14924d24ebf10a04ef3b25d89c0ad79db8be8566688bbcbc586d13c4ffbeb627` in every world [D]. In world row 0 the section
runs from decompressed offset 43,668 to 120,585 [D].

### 1.1 Record (10 bytes)

| Offset | Type | Meaning |
|---:|---|---|
| 0 | `s16` | name ID (into the names above) |
| 2 | `s8` | value kind (1.2); −1 = a group with children |
| 3 | `u8` | element count: 0 = a single value, otherwise an array of that many elements |
| 4 | `u16` | byte offset of the value in the progress block (0 for groups) |
| 6 | `u16` | index of the first child record (groups only; 0 = none) |
| 8 | `u16` | index of the next sibling record (0 = none) |

Record 0 is the root group (its first child is record 1). The **last record** is a terminator group whose byte offset
field holds the size of the used part of the block: **20,632** bytes [D]. All 5,459 other records are reachable from
the root [D].

### 1.2 Value kinds

| Kind | Value | Size per element | Records on the disc [D] |
|---:|---|---|---:|
| −1 | group | — | 989 (including root and terminator) |
| 0 | integer | 4 | 2,486 single, 48 arrays |
| 2 | object ID (integer) | 4 | 1 (`oidPoCurrent`) |
| 3 | float | 4 | 173 |
| 4 | flag | single: 4 bytes (non-zero = true); array: one **bit** per element, element *i* in byte *i* / 8, bit *i* mod 8 | 1,718 single, 45 arrays |
| 9 | world ID (integer) | 4 | 1 (`widCurrent`) |

Kinds 1, 5 (vector), 6 (16-byte record), 7 (8-byte range), 8 (12-byte record) and 10 (integer) are understood by the
game but unused on the disc [R].

### 1.3 Finding a value by path

A path is a list of names, root first. Start at the root's first child; walk siblings until one has the next name;
for a group, descend to its first child and continue; the last name must be reached exactly. An **array** record
takes one more path element, a number, as the element index; it must be the path's last element [R]. A missing path
reads as "no value" [R]. Scripts and the job and task records name save entries by such paths (missions.md 7;
options 1260–1263, 1295–1299, value type 0x16 in object-stream.md 1.2) [D].

## 2. What the tree holds [D]

The top level has 886 entries:

- **Game state:** `episodeCurrent` (offset 0), `episodeBest` (4), `widCurrent` (8, the current world),
  `oidPoCurrent` (12, the current character's object ID), `dt` (16, total play time, float).
- **`suck_history`:** three floats `mission_0` to `mission_2` (20, 24, 28): the difficulty help of the last three
  jobs (missions.md 5).
- **`flags`** (the options): `lang`, `fNoVibration`, `nMixerMode`, `fAutosaveWarned`, `fReverseLookAround`,
  `fReverseAim`, `fNoShowBudDistractWarning`, `uMusicVolume`, `uEffectsVolume`, `fUseHeadset`, `dxScreenCenter`,
  `dyScreenCenter` (offsets 32 to 76).
- **`sly`, `bentley`, `murray`:** `hpCur`, `mpCur` (health and power), `oidBooty` (carried loot), `apukCur` (3
  gadget slots; safehouse-and-shop.md) — Sly's at 80 to 103, Bentley's from 104.
- **`powerUps/mppukfAcquired`:** 43 owned-item flags (one bit each) at offset 152.
- **`loot`:** `ccoin` (coins, 160) and `mplootkc` (64 loot counts, from 164).
- **`episode/<name>`** for the 9 episodes (`intro` … `blimp`): the episode's `dt` (offsets 420, 564, 980, 1260, 1404,
  1616, 2304, 2652, 3068) and, per level, `booty` (with `aoidAssembly`), `afClue` (32 clue-bottle flag bits) and
  `fVault`. Example: `episode/nightclub/level/cellar/afClue` at 700, `…/fVault` at 704.
- **One entry per job** (79, names `m<episode>_…`): `focus_count`, `is_complete`, `time_played`, `suck_value`. Example
  `m1_sat`: 3644, 3648, 3652, 3656.
- **One entry per task** (795, names `t<episode>_…`, matching the 795 distinct tasks of missions.md): `focus_count`,
  `complete_count`, `is_complete`, `state`, `from_memcard`. Example `t0_attract_mode`: 4732 to 4748.
- **Not in the tree:** the current job, the current task and the checkpoint task. None of the 960 names is such a key
  [D]; the game keeps the three in memory for the session only (missions.md 7) [R]. A port must not keep them in the
  block's unused tail [I].
- **Episode group.** The `episode/<name>` node a world writes to (its `dt` and `percent_complete`) is chosen by the
  current world's episode, the episode byte of its level-table row (disc.md 6) [R]; the nine episode nodes are the
  children of `episode` in episode order 0–8 (`intro`, `nightclub`, `palace`, `temple`, `prison`, `castle`,
  `train`, `sawmill`, `blimp`) [D]. The level row's save path starts with the same `episode/<name>` [D for the
  paths, I that the two always agree].

## 3. When the game saves and restores [R]

- **Live block.** The game keeps one progress block in memory. Job and task changes, coins, loot, items, clue bottles
  and options write into it as they happen (missions.md 6–7, safehouse-and-shop.md).
- **Writing a slot.** Saving copies the live block to the slot buffer, sets every task's `from_memcard` flag to 1 in
  the copy, and writes the slot file (section 4).
- **Autosave.** Attempted when a job completes (unless the job's option 1242 is set), when a task outside a job
  whose successors all belong to jobs is done (missions.md 6), and at other story points. It runs only when a card
  slot has been chosen, no world is loading, and the current episode is not the intro (unless forced). If the
  `flags/fAutosaveWarned` value is 0, the autosave warning screen is shown first instead.
- **Layout check on load.** A slot's two check words (section 4) must equal the layout word (20,632, the terminator's
  offset) and the layout checksum (the **sum of the bytes of the record table**, 2,582,452 = `0x2767B4` on this disc)
  [D for the values]. If either differs, the whole 21,504-byte data area is cleared to zero and the check words are
  rewritten. There is **no checksum over the saved values themselves**.
- **Empty slots.** A slot whose `dt` is 0 is treated as unused and reset, keeping only the options (`flags`). A new
  game resets the chosen slot the same way and starts from it.
- **Choosing the newest slot** (to continue): among used slots, the one whose card file has the latest modification
  time.
- **Restore order** on loading a world: episode and world, then every task's state (missions.md 7), then the current
  job, current task and checkpoint task **from session memory** (none after loading a card save, except a task
  with option 1273), then the current character (`oidPoCurrent`) with its health, power and gadget slots.

## 4. Memory-card files [R]

| Path on the card | Size (bytes) | Contents |
|---|---:|---|
| directory `/BASCUS-97316YAOTWTD!` | — | the game's save directory (name in the executable at `0x2A6BB0` [D]) |
| `BASCUS-97316YAOTWTD!` inside it | 5 | the text `Sly 2` (no terminator) |
| `icon.sys` | 964 | standard PS2 icon description: magic `PS2D` [D string], title `Sly 2` (converted to Shift-JIS), background, light directions and colours, and the icon file name `memcard_icon.ico` for the list, copy and delete views |
| `memcard_icon.ico` | 25,688 | copied unchanged from the disc: executable file-table record 25 (identity `0x7CF`; disc.md 5) [D] |
| `save1`, `save2`, `save3` | 21,512 each | one slot each (below) |

**Slot file** (21,512 = `0x5408` bytes):

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 21,504 | the progress block (`0x5400` bytes; the first 20,632 are described by the table) |
| 21,504 | 4 | layout word (`u32`) |
| 21,508 | 4 | layout checksum (`s32`, byte sum of the record table) |

A slot file is accepted only if its size is exactly 21,512 and its card attributes mark it a readable file; there
are always 3 slots. Creating the save needs free card space of 1 (2 when the root directory needs a new entry
cluster) + 5 + ⌈25,688 / 1,024⌉ + 66 = 98 to 99 one-kilobyte clusters; the card scan treats a card with fewer than
102,400 free bytes as full.

**icon.sys values** (from the reference's set-up): background colours (0, 135, 255), (0, 41, 111), (0, 41, 111),
(0, 135, 255) at alpha 0x60 transparency; light directions (0, 0.5, −0.5), (0.5, −0.4, 0.1), (−0.5, 0.5, 0.5); light
colours (0.14, 0.03, 0.48), (0.33, 0, 0.33), (0.14, 0.14, 0.38); ambient (0.5, 0.5, 0.5); line break after 32 bytes
of title.

## 5. Test vectors [D]

| Item | Value |
|---|---|
| Names / records | 960 / 5,461 |
| Record-table SHA-256 | `14924d24ebf10a04ef3b25d89c0ad79db8be8566688bbcbc586d13c4ffbeb627` |
| Layout word / checksum | 20,632 / 2,582,452 (`0x2767B4`) |
| Root record | (−1, −1, 0, 0, 1, 0) |
| `loot/ccoin` | record (62, 0, 0, 160, 0, 42) |
| `loot/mplootkc` | record (63, 0, 64, 164, 0, 0) |
| `powerUps/mppukfAcquired` | record (60, 4, 43, 152, 0, 0) |
| `sly/apukCur` | record (56, 0, 3, 92, 0, 0) |
| `episode/intro/level/exterior/afClue` | record (71, 4, 32, 488, 0, 52) |
| Tree depth (entries per depth 1–6) | 886, 4,326, 27, 44, 132, 44 |
| Memory-card icon | sector 1,169,578, 25,688 bytes, SHA-256 `708e8fd5acddece90865a3713734b39629fcbabc2d8df07445b7841036bfa08c`, first bytes `00 00 01 00 01 00 00 00 03 00 00 00` |

## 6. Open questions

- The card-file layout is unverified: write a save in an emulator and compare (sizes, the 5-byte directory file,
  icon.sys bytes, check words).
- The full list of autosave points and what `focus_count` counts.
- Whether `from_memcard` changes how a restored task behaves.
- Which value kinds the scripts' save built-ins accept (scripts.md).
