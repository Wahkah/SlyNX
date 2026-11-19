---
id: sly2-gameplay-overview
title: Sly 2 runtime systems, overview and roadmap
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds walked package by package
    used_for: >-
      which classes occur in which worlds (hubs, jobs, the menu world), counts of mission records per episode, the
      purchasable-item table and the loot table read from the executable; every statement marked [D]
  - kind: observation
    what: the game as played, general knowledge of its mechanics
    used_for: the plain-English descriptions marked [O]; to be re-checked in an emulator by the behaviour specs
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release, files update, clock, activate, gang, task, pocket, gadget,
      shop (read for what the game does, not how it is written)
    used_for: frame order, clock factors, activation, character switching, jobs and tasks, pickpocketing, gadgets,
      the shop; marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), module list only
    used_for: confirmation of which subsystems exist; no facts taken from it alone
date: 2026-11-14
---

# Sly 2 runtime systems: overview and roadmap

The Sly 2 counterpart of [`../sly1/gameplay-overview.md`](../sly1/gameplay-overview.md). It says what runs in a
world in plain words and plans the behaviour specs. Tags as in the Sly 1 overview: **[D]** disc data, **[O]**
observable, **[R]** reference behaviour to be confirmed, **[I]** inferred. Class IDs are those of
[object-stream.md](object-stream.md) section 3; world rows are those of [disc.md](disc.md) section 6.

## 1. Clocks

- Same frame step as Sly 1: the measured time clamped to 1/60 s … 1/30 s [R]. The first frame after a world loads
  always takes 1/30 s [R].
- Game time advances by the frame step times the product of nine independent rate factors (slow motion, pauses,
  effects); a separate real-time clock advances by the frame step times its own factor and keeps running while
  game time is paused [R]. Sly 1 had one global scale; the several factors are new [R].
- Animation key times are in 1/60 s frames ([animation.md](animation.md)); clip durations are in seconds [D].

## 2. Order of one frame (differences from Sly 1)

Sly 1's order ([`../sly1/gameplay-overview.md`](../sly1/gameplay-overview.md) section 2) holds, with these
changes [R]:

1. Script events that were deferred from the previous frame run first, then again after the object updates and
   after physics (three points per frame).
2. **Activation.** Before the updates, the game wakes or puts to sleep whole subtrees of objects by distance from
   the player (radius-activated roots). Only active objects update [R].
3. **Update list.** Objects are updated from one flat list in a fixed order, not by walking the tree; each entry has
   flags. While game time is paused, only entries flagged to run on the real clock update (some scripts, user
   interface, real-clock animations) [R].
4. After the updates: physics passes, deferred events, shader animation, then music, camera, and the user-interface
   screens (classes 126 to 131), every frame even when paused [R].

## 3. Life of a world

1. **Load** the world file: header sections, then each package in turn ([world-file.md](world-file.md)), then the
   packages' placements clone prefabs into the live world ([classes/placement.md](classes/placement.md)) [D] [R].
   Objects carrying option 26 leave the world at start [R] (6,502 uses [D]).
2. **Level conditions.** Records, meshes and textures listing level-table rows exist only in those rows' worlds
   ([object-stream.md](object-stream.md) 2.3, [textures.md](textures.md) 2.5) [D].
3. **Post-load**: the task manager applies the saved state of every job and task to the world's objects (section 6)
   before play starts [R].
4. **Play** frame by frame (section 2).
5. **Leave** through a warp volume (class 26, in every world [D]) or a task that loads another world; progress is
   kept in the save across world loads [R].

## 4. Classes with run-time behaviour

"Where" uses [D] counts from walking every package (top-level records of all packages).

| System | Classes | Where [D] | What it does |
|---|---|---|---|
| Playable characters | 53 Sly, 55 Bentley, 54 Murray | one of each in every world except the menu world (row 1) | the thief, the planner (gadgets, darts, bombs), the strongman (carry, throw) [O] |
| Other characters and bosses | 59 to 68 (names `npc_dimitri`, `neela`, `npc_rajan`, `npc_jean_bison`, `npc_carmelita`, `npc_boss_contessa`, `npc_arpeggio`, `npc_bear`, `npc_bear_cub`, `dancer`) | in the worlds of their episode | story characters, boss fights [O] |
| Guards | 58 (`guard`), 57 swarming creatures | 58: in 28 worlds; 57: in 34 | patrol, perceive, chase, attack; carry a pocket of loot (section 8) [O] [R] |
| Safe house | 70 (`safe_house`) | exactly one in each of the 8 episode hubs (rows 2, 8, 12, 14, 17, 27, 32, 38), none elsewhere | the team's base in each hub: character switching, the shop, the job board (section 7) [O] [R] |
| Missions | 132 jobs, 133 tasks | in every world; each world carries its whole episode's set (section 6) | the job and task graph, its states and their effects on the world [R] |
| Machines and vehicles | 50 (names `powerchop_player`, `rc_chopper_root`, `TANK_DRIVEABLE_ROOT`, `attract_mode_player`), 77 (tank) | 50: in 12 worlds (rows 0, 8, 14, 17, 29, 30, 32 to 37); 77: row 33 only | player-driven machines in job worlds [O] [D for names] |
| Hacking games | 72 to 75 (names `hack_…`) | the same 5 worlds for all four: rows 14, 15, 22, 32, 38 (prison hub and interior, castle heist, sawmill and blimp hubs) | Bentley's computer-hacking minigames [O] |
| Vault | 78 (`vault`) | 8 job worlds (rows 4, 10, 13, 16, 20, 28, 34, 42) | the treasure vault of a heist [O] |
| Pickups | 91 coin, 92 key, 93 medicine, 94 pickpocket item, 95 clue, 96 minigame critter | 91 and 93: every world; 95: in all 8 hubs only (rows 2, 8, 12, 14, 17, 27, 32, 38) | coins, health, keys, clue bottles, loot [O] [D] |
| Gadget objects | 100 gadget objects, 101 mine, 102 trap, 103 loot, 104 effects buffer, 108 ammo | 100 to 102: every world | objects created by the characters' gadgets and the loot shown when picked [R] |
| Throwables and projectiles | 79, 81 to 83 | 81: every world | thrown and fired objects, missiles [O] |
| Interactables | 83 touch pads, 85 lasers, 86 barriers, 88 hiding barrels, 98 reactive props, 99 pocket dispensers | widely | pads, laser beams, barriers, breakables and switchable props [O]; class roles [D] per interactables.md and alarms-sensors.md |
| Animation and logic | 109 clips, 111 state machines, 112 to 124 settings and logic objects | widely | as Sly 1 plus new logic objects ([animation.md](animation.md), scripts spec) |
| Markers, paths, volumes | 3, 4, 10 to 25, 27, 35 to 47 | widely | patrol curves, navigation networks, trigger volumes, points (the classes/ specs) |
| Effects | 29 emitters, 31 explosion groups, 32, 33 explosion parts, 97 sky dome | 29, 31, 33: every world | particles, explosions, sky [D] |
| Warps | 26 | every world | doors between hub and jobs; destinations name worlds [D] |
| User interface | 126 to 131 screen movies | 126 and 130 in every world | menus, the HUD, the safe-house screens [D] [R] |

Hub-only classes [D]: 7, 38, 67, 69, 70, 95, 96, 106, 117, 119, 123. Job-only classes [D]: 18, 25, 32, 46, 65,
68, 77, 78, 108, 121. In every world except the menu world [D]: 2, 3, 6, 20, 26, 29, 31, 33, 42, 48, 49, 53, 54,
55, 81, 91, 93, 100, 101, 102, 113, 114, 126, 130, 132, 133.

## 5. The three playable characters and switching

- All three character objects are loaded in every playable world [D]; only one is controlled at a time and the
  others are hidden or follow scripted placements [R] [O].
- **Who plays.** In a hub, the player chooses the character at the safe house; a job fixes its character, and a
  task records which character it uses; that choice is stored in the save and restored with the task [O] [R].
- **Shared progress, separate kits.** Coins, loot and job state are shared; each character has its own three
  gadget slots and its own moves (section 8) [R].
- **Abilities by character** [O]: Sly sneaks, climbs, pickpockets, uses the cane; Bentley uses darts, bombs and
  hacking, and can be carried by Murray; Murray fights, carries and throws objects and guards.

## 6. Hubs, jobs and tasks

- Each episode has a hub world (the open area with the safe house) and job worlds; the level table flags the hubs
  ([disc.md](disc.md) 6) [D]. Nine hub rows, including the intro row 0, and 33 job and boss rows [D].
- **Jobs and tasks.** A job (class 132) is one heist of the episode; its tasks (class 133) form a graph where each
  task has prerequisites and successors [R]. Every world of an episode carries the whole episode's mission records
  [D]:

  | Episode | Class-132 records | Class-133 records |
  |---|---:|---:|
  | 0 intro | 1 | 16 |
  | 1 nightclub | 11 | 99 |
  | 2 palace | 12 | 93 |
  | 3 temple | 10 | 82 |
  | 4 prison | 9 | 110 |
  | 5 castle | 8 | 102 |
  | 6 train | 8 | 72 |
  | 7 sawmill | 10 | 122 |
  | 8 blimp | 9 | 99 |

- **Task states** [R]: none, locked, available, done, done and closed. The states live in the save. On load and on
  every change, each task adds or removes its groups of objects and sets state-machine goals in the world, so one
  world file serves every stage of the episode. A task can have a time limit; a job begins when its first task is
  reached and is complete when its last task is done; it can be restarted from its checkpoint.
- **Difficulty help** [R]: a per-job value between 0 and 1 rises by 0.1 on each death or job restart and falls by
  0.2 the first time a task is done; damage to the player is scaled by `1 − value / 2`, and drops lean on it.

## 7. The safe house

- One safe-house object per episode hub [D]. From its computer the player opens the **shop** [R]. The **gadget
  assignment** screen opens with Select during play, anywhere, and the **job help** from the pause menu [R]
  (safehouse-and-shop.md).
- **Shop** [R]: buys gadgets and moves with coins and sells loot back for coins. The item table is in the
  executable [D]:

  | Item table | Virtual address `0x002BCD00`, 43 records of 32 bytes |
  |---|---|
  | +0 `s32` | button use: 2 press, 1 hold, −1 not on a button |
  | +4 `s32` | owner: 7, 8 or 9 (20, 8 and 8 records); 7 = Sly, 8 = Bentley, 9 = Murray [R] |
  | +8 `s32` | price in coins (0 to 2,100; 0 for the 12 moves that are not sold) |
  | +12 `s32` | gadget-power cost per use (0 to 30) |
  | +16 `s32` | episode from which it is offered (1 to 8; −1 not sold) |
  | +20, +24 `s32` | string IDs of its name and description (129 to 202) |

  Records 0 to 6 are placeholders. Check value: record 7 = (2, 8, 200, 0, 1, 129, 130) [D].
- **Loot table** [D]: virtual address `0x002C3288`, 64 records of 12 bytes: `s32` value in coins (0 to 803), then
  two `s32` string IDs. The first values are 11, 19, 27, 17, 40, 60.
- Both tables are read from the player's executable after checking its SHA-256.

## 8. Gadgets, pickpocketing, guards, alarms

- **Gadgets** [R]: each character has three slots on L1, L2 and R2, saved per character; using a gadget spends
  gadget power from a meter whose maximum is 100. Gadget-made objects are classes 100 to 102 and 108 (section 4).
- **Pickpocketing** [O] [R]: guards and other givers carry a pocket of up to 8 item entries; Sly sneaks up behind
  and takes one with a hand animation (the take happens at an animation event); the item is shown and added to the
  loot (sold at the safe house) or to coins.
- **Guards** [O]: patrol, notice by sight and sound, chase and attack; Murray can lift and throw them; knocked-out
  guards drop coins. The detailed rules follow Sly 1's guard spec where unchanged (roadmap).
- **Alarms and lasers** [O]: laser beams (class 85) and guards' alarm noises raise the alert; class 83 touch pads and
  some volumes switch them (alarms-sensors.md).
- **Vehicles and machines** [O]: driven machines (class 50: RC chopper, tank and others), turrets, and
  hacking minigames (classes 72 to 75).

## 9. Roadmap: behaviour specs

"Sly 1 carry-over" says how much of the Sly 1 spec applies: **same** (cite it, no new spec text needed beyond a
pointer), **delta** (write only the differences), **new**.

| Spec | Scope | Depends on | Sly 1 carry-over |
|---|---|---|---|
| clock-and-loop.md | the rate factors, activation, the update list, frame order | — | delta of `../sly1/clock-and-loop.md` |
| scripts.md | script format and built-ins, events | world-file.md 4.1 | delta of `../sly1/scripts.md` (written separately) |
| physics.md | contacts, solver, collision parts | classes/solid-object.md | delta of `../sly1/physics.md` |
| characters.md | the three characters' moves, health, switching, carry and throw | animation.md, physics.md | new (Sly's moves: delta of `../sly1/player-moves.md`) |
| camera.md | follow camera, zones, cutscene placements, binocucom | classes/paths-and-volumes.md | delta of `../sly1/camera.md` |
| missions.md | jobs, tasks, states, world effects, time limits, restarts, difficulty help | classes/effects-logic-ui.md 3 | new |
| safehouse-and-shop.md | safe house, shop, loot, gadget assignment, job help | missions.md | new |
| gadgets.md | each item of the item table: use, cost, effect | characters.md | new |
| guards.md | patrol, perception, alert, chase, attack, pockets, knock-outs, swarms | missions.md | delta of `../sly1/guards.md` |
| pickpocket-and-pickups.md | pockets, coins, medicine, keys, clue bottles, loot | guards.md | delta of `../sly1/pickups-and-progress.md` |
| alarms-sensors.md | alarms, lasers, buttons | guards.md | delta of `../sly1/alarms-sensors.md` |
| interactables.md | buttons, barrels, switchable props, dispensers, throwables | scripts.md | delta of `../sly1/interactables.md` |
| dialogue-and-voice.md | dialogue, subtitles, voice lines | audio.md | delta of `../sly1/dialogue-and-voice.md` |
| effects.md | emitters, explosion groups, sky dome, lights | renderer | delta of `../sly1/effects.md` |
| level-flow.md | warps, hubs, job entry and exit, checkpoints, wipes, game start and end | missions.md | delta of `../sly1/level-flow.md` |
| vehicles-and-minigames.md | driven machines, turrets, hacking games, bosses | characters.md | new (bosses: delta of `../sly1/minigames.md`) |
| user-interface.md | screen movies (classes 126 to 131), HUD, menus | — | new |
| saving.md | the save's progress tree (characters, gadgets, loot, jobs) and memory-card files | missions.md | delta of the saving part of `../sly1/pickups-and-progress.md` |
| options-by-class.md | meanings of the options each class uses | object-stream.md 1 | delta of `../sly1/options-by-class.md` |

The Sly 1 specs that apply unchanged as written: none in full; the closest are `../sly1/effects.md` (emitter
behaviour) and `../sly1/dialogue-and-voice.md` (talk events), pending a check of their option numbers [I].

## 10. Open questions

1. Whether character choice is free in hubs or only at the safe house [O to confirm].
2. (Resolved: owner codes 7 Sly, 8 Bentley, 9 Murray, confirmed by item names; characters.md 6.) The use of the 12 unsold move records.
3. (Resolved: class 95 is the clue bottle; each hub holds its episode's 30 bottles. pickpocket-and-pickups.md.)
4. Which job-only classes (18, 25, 46, 65, 68, 121) belong to which job types.
5. Whether the activation radius is per object (an option) or global.
