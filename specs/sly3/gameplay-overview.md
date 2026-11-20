---
id: sly3-gameplay-overview
title: Sly 3 runtime systems, overview and roadmap
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3: Honor Among Thieves, USA, SCUS-97464; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338; all 39 worlds walked group by group
    used_for: which classes occur in which worlds, mission and task record counts per episode, every statement marked [D]
  - kind: observation
    what: the game as played, general knowledge of its mechanics
    used_for: the plain-English descriptions marked [O]; to be re-checked in an emulator by the behaviour specs
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, 2026-11); SafeHouse @ c70803ce (2026-11-12), Sly3-Release (read for what the game does)
    used_for: load-time rules (conditions, option sets, placements, deferred clips); marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 runtime systems: overview and roadmap

The Sly 3 counterpart of [../sly2/gameplay-overview.md](../sly2/gameplay-overview.md). It says what runs in a world in
plain words and plans the behaviour specs. Sly 3 is built on Sly 2's engine: unless a section says otherwise, the
Sly 2 overview applies. Tags as there. Class IDs are those of [object-stream.md](object-stream.md) 3.2; world slots
those of [disc.md](disc.md) 5.1.

## 1. Clocks and the frame

As Sly 2 sections 1 and 2 (frame step clamped to 1/60 … 1/30 s, several game-time rate factors, a real-time clock,
activation by distance, one flat update list, deferred script events). [R, unchanged as far as the loaders show] No
Sly 3-specific change is known yet; the clock spec should confirm against the executable.

## 2. Life of a world

1. **Load** the world file ([world-file.md](world-file.md)): header sections, then each group body. [D]
2. **Option set and conditions.** With no job or task in progress the game applies option set 0: its id becomes the
   world's condition, its options set group copy counts and add conditions. A saved job or task state selects another
   set. [R] Records, meshes, palettes, bitmaps and colour tables whose world filter or conditions fail are skipped
   ([world-file.md](world-file.md) 6). [D for the fields, R for the rule]
3. **Placements** copy groups into the world ([classes/placement.md](classes/placement.md)); option 27 removes
   objects at start. [R]
4. **Deferred clips** (13 of Sly's per world) arrive from another source after loading ([animation.md](animation.md)
   4). [R]
5. **Play**, then **leave** through an exit (class 24 shapes carry world records) or a task that loads another
   world. [D for the records, R]

## 3. Classes with run-time behaviour

"Where" counts top-level and placement records per world. [D]

| System | Classes | Where [D] | What it does |
|---|---|---|---|
| Playable gang | 52 Sly, 53 Murray, 54 Bentley | Sly in 36 worlds, Bentley 34, Murray 31; none in the menu world or the hacking world (slot 22) | the thief, the strongman, the planner [O] |
| New gang members | 55 the Guru (`shaman`), 56 Panda King, 57 Penelope, 59 Dimitri, 60 Carmelita | Guru 9 worlds, Panda King 6, Penelope 8, Dimitri 6, Carmelita 10 | the recruits, each playable in their episode's jobs [O] |
| Guards and creatures | 58 (names vary: dingo, swarmers, kangaroo…), 61 to 67 bosses and story characters | 58 in 28 worlds | patrol, perceive, chase, attack, carry loot [O] |
| Safe house | 69 (`safe_house`) | 6 worlds: v_ext, o_ext, h_ext, c_ext, p_ext, p_at_sea | the gang's base in each hub [O] |
| Missions | 168, 169, 170 | in every world of an episode, the whole episode's set (section 5) | jobs, tasks and related records [R] |
| Hacking games | 73 to 77 (`hack_…`) | h_ext, h_hack, c_hall_a, c_hall_b | Bentley's hacking minigame [O] |
| Vehicles and machines | 79, 81 (truck), 82 (biplanes), 83 (raft), 84 (jolly boat), 85 (RC car, van), 87 (Dimitri swimming), 88 (ships) | in their jobs (82: h_ext, h_dogfight, m_ext; 88: the pirate worlds) | driving, flying and sailing jobs [O] |
| Pickups | 103 coin, 104 key, 105 medicine, 106 pickpocket item, 112 pickpocket dispenser, 116 loot | 103, 105, 116 in every world; 106 and 112 in 27 | coins, health, loot, pickpocketing [O] |
| Vault | 121 (`vault_tumbler`) | v_apt, h_hangar_b, c_ext, m_ext | the episode's vault combination [O] |
| Navigation aids | 125 compass, 127 treasure-map markers | 125: 36 worlds; 127: the five hubs with a treasure map | [O] |
| Sky | 110 | 21 worlds | sky domes [D] |
| Animation and logic | 130 clips, 132 state machines, 135 command scripts | every world | as Sly 2, plus command scripts (Sly 2's sequences) |
| Effects | 45 effects, 27/30/31 emitters | every world | particles and explosions |
| Paths and zones | 9 to 16, 19 to 25, 29 | widely | patrol paths, navigation meshes, trigger and exit shapes, zones |
| User interface | 158 to 167 screen objects | 158, 161, 167 in every world | HUD, menus, the thief-net screens [R] |

Classes in every playable world [D]: 2, 4, 5, 8, 24, 40, 45, 47, 48, 103, 105, 116, 134, 135, 138, 147, 158, 161,
167, 168, 169.

## 4. The gang

- The three core characters exist in most worlds; one is controlled at a time. [D] [O] The new recruits (Guru,
  Panda King, Penelope, Dimitri, Carmelita) appear in their episodes' jobs and later. [D]
- Moves and gadgets follow Sly 2's scheme (per-character gadget slots, shared coins and loot). [O]
- Character bodies can differ per episode through world-filtered meshes ([geometry.md](geometry.md) 2.1). [D]

## 5. Episodes, hubs, jobs and tasks

- Six episodes after the prologue, each with a hub named `…_ext` (v, o, h, c, p, m). [D]
- Each world carries its whole episode's mission records [D]:

  | Episode | Class 168 | Class 169 | Class 170 |
  |---|---:|---:|---:|
  | 2 prologue (i_trainer) | 7 | 95 | 0 |
  | 3 Venice | 11 | 150 | 21 |
  | 4 Outback | 11 | 129 | 23 |
  | 5 Holland | 14 | 144 | 22 |
  | 6 China | 12 | 179 | 18 |
  | 7 pirates | 12 | 153 | 19 |
  | 8 Kaine Island | 9 | 115 | 18 |

  The menu world carries 101 class-170 records. [D] *Interpretation:* 168 jobs, 169 tasks (each with two id lists:
  [classes/simple-layouts.md](classes/simple-layouts.md) 5), 170 further mission records (named). [I]
- Job and task states select option sets and conditions at load (section 2), so one world file serves every stage of
  the episode. [R]

## 6. Roadmap: behaviour specs

"Carry-over" says how much of the Sly 2 plan applies (Sly 2 overview section 9): **same**, **delta** or **new**.

| Spec | Scope | Depends on | Carry-over | Status |
|---|---|---|---|---|
| clock-and-loop.md | frame order, rate factors, activation | — | same as Sly 2 (confirm) | written |
| scripts.md | script cells (renumbered kinds), built-ins, option map, command scripts (class 135) | world-file.md 4.1, classes/animation.md 3 | delta of Sly 2 | written |
| conditions.md | option sets, conditions, world filters, job and task states | world-file.md 3.7, 6 | new | written |
| physics.md | contacts, collision parts (two new fields) | classes/solid-object.md | delta of Sly 2 | written |
| characters.md | Sly, Murray, Bentley, and the five recruits: moves, health, switching | animation.md | delta of Sly 2 (recruits new) | written |
| camera.md | follow camera, zones, cutscenes | classes/paths-and-volumes.md | delta of Sly 2 | written |
| missions.md | jobs, tasks (169's lists), class 170, the thief-net screens | conditions.md | delta of Sly 2 | written |
| safehouse-and-shop.md | safe house, shop, recruits' gadgets | missions.md | delta of Sly 2 | written |
| guards.md | patrols, perception, swarms, loot | missions.md | delta of Sly 2 | written |
| pickpocket-and-pickups.md | coins, medicine, keys, loot, pickpocketing, vaults, treasure maps, compass | guards.md | delta of Sly 2 | written |
| vehicles-and-minigames.md | trucks, biplanes, ships, raft, RC car, hacking, bosses | characters.md | delta of Sly 2 (biplanes and ships new) | written |
| effects.md | effects with components (class 45), emitters, sky | renderer | new layout, Sly 2 behaviour | written |
| rendering.md | colour grading, colour tables, passes, outlines | textures.md, geometry.md | delta of Sly 2 | written |
| dialogue-and-voice.md | voice entries, speakers | audio.md | delta of Sly 2 | written |
| user-interface.md | screen objects (classes 158 to 167), external screen files | classes/effects-logic-ui.md 6 | delta of Sly 2 | written |
| level-flow.md | exits (class 24), hubs, job entry and exit | missions.md | delta of Sly 2 | written |
| saving.md | progress variables, job and task states | missions.md | delta of Sly 2 | written |
| options-by-class.md | option meanings for the renumbered classes | object-stream.md 1 | delta of Sly 2 | written |
| alarms-sensors.md | lasers, detectors, contact triggers, bouncers, alarms | guards.md | delta of Sly 2 | written |
| gadgets.md | item usage, slots, power and drains, the gang's gadget objects | characters.md | delta of Sly 2 | written |
| interactables.md | breakables, barriers, water, use points, trigger shapes, hiding spots | — | delta of Sly 2 | written |

## 7. Open questions

1. Where deferred clips come from.
2. The roles of classes 170 and 143 (key lists in macro groups). Class 170 is probably the challenges: the menu
   world's 101 records match the 101 `w…` challenge entries of the save tree ([saving.md](saving.md) 2) [I].
3. (Answered in [characters.md](characters.md) 2 and 6: the safe house chooses among the core three; recruits are
   played only in their jobs.)
