---
id: sly3-formats-index
title: Sly 3 format specs - index and reading order
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 3 - Honor Among Thieves, USA, SCUS-97464, image 3,553,755,136 bytes; executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: the documents listed below; this index adds no facts of its own
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 format specs

Specs for reading Sly 3: Honor Among Thieves (USA, SCUS-97464) from the player's own disc image. Sly 3 is built on
Sly 2's engine, so these specs build on [`../sly2/`](../sly2/README.md) (and through it on [`../sly1/`](../sly1/README.md)):
where Sly 3 stores something the same way, the Sly 3 spec points at the Sly 2 section and states only the
differences.

## Reading order

| # | Document | What it covers |
|---:|---|---|
| 1 | [disc.md](disc.md) | File system (one named data file), the executable's world and movie tables, per-world sizes and hashes |
| 2 | [world-file.md](world-file.md) | Compression (as Sly 1), key tables, the group list with body sizes, UTF-16 strings, option sets, partition trees, the group body, conditions and world filters |
| 3 | [textures.md](textures.md) | Two copies of the palette and bitmap tables, filters and conditions, shaders, palettes per frame, colour grading, decoding |
| 4 | [object-stream.md](object-stream.md) and [classes/](classes/) | Records with modes, filters and conditions; the option and class tables; every class layout, field-exact on every world |
| 5 | [geometry.md](geometry.md) | Mesh sets, filtered meshes, the mesh header with bones, the four body types, colour tables, copies |
| 6 | [transforms.md](transforms.md) | Transform lists (as Sly 2), placements, skin binding |
| 7 | [worlds.md](worlds.md) | Per-world test vectors |
| 8 | [animation.md](animation.md) | Clip layout with shared key times, packed tracks, events, deferred clips |
| 9 | [audio.md](audio.md) | Differences from Sly 2 sound: 44.1 kHz streams, speakers |
| 10 | [movies.md](movies.md) | Differences from Sly 2 movies: the movie table, 30 frames per second |
| 11 | [gameplay-overview.md](gameplay-overview.md) | Runtime systems and the roadmap of behaviour specs |
| 12 | [characters.md](characters.md) | Who is in which world, where characters start, the item and lookup tables, clip counts, the gang's changes from Sly 2, the recruits |
| 13 | [rendering.md](rendering.md) | Differences from Sly 2's rendering: pre-baked colour tables, grading and tint options, fog and shadow colour in the option sets, passes, the depth-only cut-off |
| 14 | [scripts.md](scripts.md) | Differences from Sly 2 scripts: keyword numbering, the built-in table, the option map, events, first custom property, command scripts (class 135) |
| 15 | [guards.md](guards.md) | Guard and swarmer classes and templates, their options with Sly 2 equivalents, hit points, pursuit ranges, the pocket tables |
| 16 | [alarms-sensors.md](alarms-sensors.md) | Lasers, detectors, contact triggers, bouncers; alarms |
| 17 | [pickpocket-and-pickups.md](pickpocket-and-pickups.md) | Pickup classes and constants, pocket items, the loot table, treasure definitions, vault tumblers, treasure maps |
| 18 | [gadgets.md](gadgets.md) | Item usage texts, gadget slots and power, the gang's gadget objects, new Sly 3 items |
| 19 | [interactables.md](interactables.md) | Breakables, barriers, water, use points, trigger shapes, hiding spots, perches |
| 20 | [dialogue-and-voice.md](dialogue-and-voice.md) | Dialogue commands: line-list order, subtitles and speaker colours, source and fall-off, stop, talk effects |
| 21 | [level-flow.md](level-flow.md) | Episode start, exits (class 24) and what fires them, arrival and warp points, loading, restart, the wipe |
| 22 | [safehouse-and-shop.md](safehouse-and-shop.md) | The safe-house object, its states and actions, the shop screen and what it sells |
| 23 | [vehicles-and-minigames.md](vehicles-and-minigames.md) | Turret, hacking classes, locks and safes, the new driven machines: classes, worlds, option tables |
| 24 | [user-interface.md](user-interface.md) | Screen objects (classes 158 to 167), streamed screens, options, the shared screens and their game-supplied strings |
| 25 | [saving.md](saving.md) | The save-variable tree (1,201 names, 6,559 records), what it holds, memory-card names |
| 26 | [options-by-class.md](options-by-class.md) | The option table, meanings gathered from the behaviour specs, the options every class uses |

## Conventions

As in the Sly 1 and Sly 2 specs: little-endian, packed, `vec3` = 3 × `f32`; statements tagged [D] disc data, [X]
executable data at an address, [O] observable, [R] reference behaviour, [I] inferred. Tables that exist only in the
executable are given by virtual address and read from the player's executable after checking its SHA-256
(disc.md 1); the port refuses on a mismatch.

## Status

All documents are drafts awaiting review. Each ends with its open questions.
