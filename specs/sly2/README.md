---
id: sly2-formats-index
title: Sly 2 format specs - index and reading order
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316, image 2,884,141,056 bytes; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73
    used_for: the documents listed below; this index adds no facts of its own
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 format specs

Specs for reading Sly 2: Band of Thieves (USA, SCUS-97316) from the player's own disc image. They build on the Sly 1
specs in [`../sly1/`](../sly1/README.md): where Sly 2 stores something the same way, the Sly 2 spec points at the Sly 1
section and states only the differences. Read the Sly 1 conventions first.

## Reading order

| # | Document | What it covers |
|---:|---|---|
| 1 | [disc.md](disc.md) | File system, the 64-byte file record and its sector base, the executable's file table (movies and boot worlds), the level table carried in every world, world names, per-world sizes and hashes |
| 2 | [world-file.md](world-file.md) | Compression (as Sly 1), the world header sections, the per-package body (scripts, materials, texture data, graph, object list), the unread tail |
| 3 | [textures.md](textures.md) | Palette and bitmap headers, storage kinds and level conditions, shaders and texture records, palettes per frame by shader kind, decoding |
| 4 | [object-stream.md](object-stream.md) and [classes/](classes/) | Object records with their size and level conditions, the option encoding, the class table, placements, solid and simple layouts |
| 5 | [geometry.md](geometry.md) | Mesh sets with slots, per-level variants and copies; rigid, skinned, streamed and indexed bodies; strips; bone tables and weights; outline edges |
| 6 | [transforms.md](transforms.md) | Per-object transform lists, parent references, matrix convention, skin binding |
| 7 | [worlds.md](worlds.md) | Per-world test vectors: sizes, packages, records, meshes, triangles, class histograms |
| 8 | [animation.md](animation.md) | Differences from Sly 1 animation: clip storage, curves, transform kinds, root motion, chains and mirrors, looping; how each playable character's clips are looked up |
| 9 | [audio.md](audio.md) | Differences from Sly 1 sound: the world's sound section, effect bank version 3, stream files, voice lines |
| 10 | [movies.md](movies.md) | Differences from Sly 1 movies: the movie files, their parameters and where they play |
| 11 | [scripts.md](scripts.md) | Differences from Sly 1 scripts: cell kinds, special forms, the built-in table, the per-class script option map, events |
| 12 | [gameplay-overview.md](gameplay-overview.md) | Runtime systems: frame order, classes with behaviour, the three characters, hubs, jobs and tasks, safe house, gadgets, pickpocketing; roadmap of behaviour specs |
| 13 | [clock-and-loop.md](clock-and-loop.md) | Differences from Sly 1: nine rate factors, activation kinds and radii, the update list, frame order |
| 14 | [physics.md](physics.md) | Differences from Sly 1: collision parts, gravity values, two-pass solver, coming to rest, contact defaults, constraints |
| 15 | [characters.md](characters.md) | Sly, Murray and Bentley: health and damage, states, moves, gadgets and the item table, throwing, switching |
| 16 | [camera.md](camera.md) | Differences from Sly 1: camera zones, the view stack, framing, lens and fog, follow constants, binocucom view, cutscene cameras |
| 17 | [missions.md](missions.md) | Jobs and tasks: records, the task graph, states, world effects, job timer, failure and restart, difficulty help, job names |
| 18 | [safehouse-and-shop.md](safehouse-and-shop.md) | The safe house, the shop and prices, coins, loot table, owned items and gadget slots, gadget grids, job help |
| 19 | [gadgets.md](gadgets.md) | Every item of the item table: what it does and its numbers, gadget slots and power, carried gadget objects, the blast |
| 20 | [guards.md](guards.md) | Differences from Sly 1: guard and swarmer classes, the shared enemy engine, sight, hearing and alarms, pursuit and attacks, hit points, pockets |
| 21 | [pickpocket-and-pickups.md](pickpocket-and-pickups.md) | Pickpocketing, pocket filling and prizes (loot kinds), coins, medicine, task items, clue bottles, vaults, treasure, drops |
| 22 | [alarms-sensors.md](alarms-sensors.md) | Differences from Sly 1: guard alarm noises instead of alarm objects, lasers and beam motion, touch pads, stand-alone detectors |
| 23 | [interactables.md](interactables.md) | Touch pads, reactive props, trigger volumes, hiding barrels, barriers, water, thrown objects and contact damage |
| 24 | [dialogue-and-voice.md](dialogue-and-voice.md) | Differences from Sly 1: conversations as command lists in sequence objects, line commands, subtitles, binocucom talk, lip sync and eyes |
| 25 | [effects.md](effects.md) | Differences from Sly 1: emitters (29, 32, 33), explosion groups (31), lights (class 4), world shadow colour, sky, shadows |
| 26 | [level-flow.md](level-flow.md) | Differences from Sly 1: game start, episode menu, exits (26), arrival points (42), warp points (41), loading, death, wipes, the ending |
| 27 | [vehicles-and-minigames.md](vehicles-and-minigames.md) | Driven machines, turrets, hacking, dance, combat car, bosses: controls, goals, hit points, win and lose rules |
| 28 | [user-interface.md](user-interface.md) | Screen-movie objects (126 to 131), streamed screens, drawing, text and button icons, HUD gauge and counters, menus |
| 29 | [saving.md](saving.md) | The save-variable tree, value kinds, what is saved when, the load check, restore order, memory-card files |
| 30 | [options-by-class.md](options-by-class.md) | How options apply, meanings gathered from the other specs, the options each class uses on the disc with counts |
| 31 | [rendering.md](rendering.md) | Lighting and how shader kinds combine their palettes, texture frames, the passes and their draw state, shadows and projected volumes, colour scales, fog |

## Conventions

As in the Sly 1 specs: little-endian, packed, `vec3` = 3 × `f32`; statements tagged [D] disc data, [O] observable,
[R] reference behaviour, [I] inferred. Tables that exist only in the executable are given by virtual address and
read from the player's executable after checking its SHA-256.

## Status

All documents are drafts awaiting review. Each ends with its open questions.
