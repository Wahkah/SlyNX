---
id: sly3-behaviour-conditions
title: Sly 3 conditions and option sets - which world state loads
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), the option-set section of all 39 worlds (both selection tables, every set and its
      options); executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: the tables, set IDs and set contents below
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release world loader
    used_for: the rule that a fresh load uses the first set; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 conditions

New in Sly 3; Sly 2 has no option sets. Layout: [world-file.md](world-file.md) 3.7. Matching rules for records,
meshes and textures: [world-file.md](world-file.md) 6. Tags as in [guards.md](guards.md).

## 1. What a condition is [D] [R]

Each world file holds 1 to 4 **option sets**, each with an ID. The set chosen at load makes its ID the world's
**condition**. Conditional records, meshes, palettes and bitmaps load only when their condition list matches
([world-file.md](world-file.md) 6). The set's options apply to the world:

- fog (1010 to 1014), shadow colour (1015 to 1017), clear colour (1024), colour grading (1025 to 1028, 1031):
  [rendering.md](rendering.md) 6;
- 1032 group copy counts (group, count): 13 sets;
- 1029 adds a further condition: 10 sets;
- 1021: 1 set.

Condition IDs on the disc: 12, 13, 14, 15, 19, 20, 22, 23, plus one odd ID (118474 in o_cave_murray). 12 is the
ID of most single-set worlds (v_apt, v_gauntlet, o_cave_a, …) [D]. *Interpretation:* each ID is a lighting / story
state of a world (for example a hub by day and by night, 14 and 15) [I].

## 2. Sets per world [D]

| World | Set IDs (first = fresh load) |
|---|---|
| dvd_menu | 22 |
| i_trainer, v_apt, v_gauntlet, v_interpol, o_cave_a, o_cave_b, h_hangar_a, h_hangar_b, h_sewer, h_hack, c_apt, c_tilt_hall, m_underwater | 12 |
| v_ext | 22, 19, 20, 23 |
| v_canal | 14, 23 |
| o_ext, o_quarry, h_ext, h_hangar_c, c_ext, c_intro, c_hall_a, p_ext | 14, 15 |
| h_dogfight | 15, 14 |
| o_arena | 15 |
| o_bar, h_hotel, c_flashback, c_forest, c_hall_b, p_dive, p_island_map, m_vault, m_gauntlet, m_boss | 14 |
| o_cave_murray | 118474 |
| p_at_sea | 22, 19 |
| m_ext | 13, 14, 15 |

## 3. The selection tables [D]

Before the sets, every world holds two tables, the same in every world of an episode:

- **Table A** (`u8` count of (`u32`, `u32`)): (an object ID, a condition). v_ext's episode: (2409, 20);
  Holland: (3323, 14); pirates: (4209, 19); the others are empty.
- **Table B** (`u8` count of (`u16` n, n × `u16`, `u32`, `u32`)): one entry per task of the episode that changes the
  condition. Each entry holds:
  - a **save path** of two names: the task's own name ID and **167 = `state`**, so the path is `t…/state`
    ([saving.md](saving.md) 2);
  - the **task's object ID**;
  - a **condition**.

  Per episode: Venice 65 entries (12 ×23, 23 ×41, 20 ×1); Outback 54 (12 ×33, 15 ×21); Holland 60 (12 ×32, 14 ×1,
  15 ×27); China 59 (12 ×32, 15 ×27); pirates 67 (12 ×59, 15 ×6, 19 ×1, 14 ×1); Kaine Island 46 (12 ×6, 14 ×11,
  15 ×29); the training world 20 (all 12).
  - Example (Kaine Island): tasks 1798 to 1803 (the prologue) → 12; tasks 1841 … → 14; tasks 1844 … → 15.

*Interpretation* (superseded by 3.1) [I]: when a world loads with progress, the game looks up the tasks of table B whose saved state
(the path) says they are reached, and takes the condition of the latest one. If the world has a set with that ID it
loads that set; otherwise it loads its first set. Table A gives the condition for an object (a job or a task)
outside the task chain. A fresh load with no job or task uses the first set [R]. The exact choice (which entry wins,
and how "reached" is tested) needs the loader's code; open question 1.

### 3.1 The selection rule [X: the option-set loader `0x001500F0`, from `0x001501DC`]

The loader reads both tables, then chooses a condition c in this order:

1. **The current job.** If the game's job key (+0x1498, the OID of the current job carried across world loads;
   saving.md 3) is not −1, table A is searched in order for that OID. The first match gives c.
2. **The checkpoint task.** If there is no match and the checkpoint-task key (+0x14A0, not the active-task key +0x149C) is not −1, table B is
   searched in order for an entry whose task OID is that task. The first match gives c.
3. **Saved progress.** Otherwise every table-B entry is read in order, using its save path `t…/state`
   (`0x002BC010`). Each entry whose saved value is **above 0** sets c to its condition, so **the last such entry
   in table order wins**. A saved state above 0 is available (1), done (2) or closed (3). Locked tasks (0), and
   tasks never written to the save, do not count.
4. A result of **12 is treated as none (−1)**.

The world then loads the option set whose ID is c. When no set has that ID (none included), it loads the **first
set**. The chosen ID becomes the world's condition (`0x0036BC20`), and the set's options are applied. Option
1029 adds the extra conditions.

**The checkpoint-task key +0x14A0** [X] is written by `0x0018AAE0`:
- when a task becomes current and remembers its checkpoint (`0x00188658`), the key becomes that task;
- by the queued-task start (`0x00189E70`, which also clears the job and task keys);
- and it is **cleared when the job that owns the checkpoint task ends** (`0x00186610`, at `0x0018665C`).

So after m1_canal_chase completes, its checkpoint task (2350) is cleared with the job. +0x14A0 is −1, and step
3 decides. Carrying 2350 into the next load instead gives condition 12 and set 22, which is wrong.

The choice is made once, at world load, from the carried keys and the saved states. On a new game no table-B
task has a saved state above 0, so v_ext takes set 22.

**Example** (v_ext, table B in disc order): the m1 tasks map to 12, 1811 to 20, and **1826 `t1_chalktalk2`**,
2141 `t1_turf_war_intro` and later tasks to 23. When v_ext loads while 1826 is current, its saved state is 1,
which counts. If no later table-B task has a non-zero state, 1826's entry is the last that counts, so c = 23 and
set 23 loads (extra conditions 15 and 17), so guard 139253 (condition 15) is there. Requiring a state of 2 or 3
picks set 22 instead and misses it.

## 4. Open questions

1. (Settled: section 3.1.)
2. Options 1021 and 1029.
3. What the condition IDs mean in play.
