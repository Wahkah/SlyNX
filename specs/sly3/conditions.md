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

*Interpretation* [I]: when a world loads with progress, the game looks up the tasks of table B whose saved state
(the path) says they are reached, and takes the condition of the latest one. If the world has a set with that ID it
loads that set; otherwise it loads its first set. Table A gives the condition for an object (a job or a task)
outside the task chain. A fresh load with no job or task uses the first set [R]. The exact choice (which entry wins,
and how "reached" is tested) needs the loader's code; open question 1.

## 4. Open questions

1. The exact selection rule (section 3) and what reads table A.
2. Options 1021 and 1029.
3. What the condition IDs mean in play.
