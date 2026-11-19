---
id: sly1-formats-index
title: Sly 1 format specs - index and reading order
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a)
    used_for: the documents listed below; this index adds no facts of its own
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 format specs

Specs for reading Sly Cooper and the Thievius Raccoonus (USA, SCUS-97198) from the player's own disc image. Phase 1
goal: load one world and draw its static geometry with textures and object transforms.

## Reading order

| # | Document | What it covers |
|---:|---|---|
| 1 | [disc.md](disc.md) | ISO 9660 basics for this disc, the file list, the scrambled file descriptors in the executable, the table of 46 worlds with names, sectors, sizes and hashes |
| 2 | [world-file.md](world-file.md) | The world-file compression (byte by byte) and the top-level layout: header, scripts, options, material tables, object stream, texture payload |
| 3 | [object-stream.md](object-stream.md) and [classes/](classes/) | The per-class layout of objects, the option-list encoding and its size table |
| 4 | [textures.md](textures.md) | Palette, bitmap, shader and font tables; decoding 4-bit, 8-bit and 32-bit bitmaps to RGBA8 (index permutation, alpha 0x80 = 1.0, row order); how shaders pick bitmaps and palettes |
| 5 | [geometry.md](geometry.md) | The drawable block of an object, mesh sets, meshes, instances, submeshes (positions, normals, colours, UVs, bone weights, morph data, triangle strips), and how transforms place them in the world |
| 6 | [option-encodings.md](option-encodings.md) | The value size of every option ID, needed to step over option lists |
| 7 | [transforms.md](transforms.md) | Matrix convention, the parent chain, proxies, degenerate and mirrored matrices, axes, units and conversion to a Y-up GPU space |
| 8 | [worlds.md](worlds.md) | Per-world test vectors for the object-stream walk (record counts, depth, drawables, class histograms) |
| 9 | [collision.md](collision.md) | Meaning of the collision data: surface planes, convex edges, the collision tree, camera meshes and camera-fade groups, convex volumes, surface materials and their friction and elasticity tables |
| 10 | [audio.md](audio.md) | SPU ADPCM and VAG files, effect banks, sound IDs, envelopes, pitch, mixing and distance rules, dialogue, sequenced music with adaptive commands and intensity |
| 11 | [movies.md](movies.md) | The 17 movies: MPEG-2 program streams with 640 x 480 MPEG-2 video and 48 kHz PCM audio, where each plays, conversion parameters |
| 12 | [animation.md](animation.md) | Animation curves and their evaluation, channels, playback and events, run-time blends, how characters find their animations, skinning and poses |
| 13 | [scripts.md](scripts.md) | The world scripts: values, evaluation, special forms, events raised by objects, built-in operations, script options, state machines |
| 14 | [gameplay-overview.md](gameplay-overview.md) | Map of the runtime systems, frame order, life of a world, the player's moves, roadmap of behaviour specs |
| 15 | [clock-and-loop.md](clock-and-loop.md) | Frame-step measurement and clamp, game and real clocks, time-scale factors, frame order |
| 16 | [physics.md](physics.md) | Gravity values, terminal speed, buoyancy, the two solver passes and their thresholds, restitution, rest, springs and smoothing |
| 17 | [player-moves.md](player-moves.md) | The player's parameters, run speeds from root motion, jumps, climbing, wall moves, attacks, damage, power-ups |
| 18 | [camera.md](camera.md) | Lens and fog, camera modes, camera zones and default framing, springs, orbit input, look mode, keeping the view clear |
| 19 | [options-by-class.md](options-by-class.md) | How options apply, meanings of the commonly needed options, which options each class uses on the disc |
| 20 | [guards.md](guards.md) | Guard kinds and groups, the shared alert level, per-guard states, sight and leash rules, hidden-player rules, attacks, stuns and knock-outs |
| 21 | [alarms-sensors.md](alarms-sensors.md) | Alarms and alarm boxes, lasers, searchlights, pressure plates: detection, triggering, damage mode, switching off |
| 22 | [pickups-and-progress.md](pickups-and-progress.md) | Coins, charms, lives, keys, clue bottles, vaults, timed runs, adaptive difficulty drops, level flags, completion percentage, saving |
| 23 | [interactables.md](interactables.md) | Buttons, trigger volumes, breakables and pieces, hazards and water, bombs and mines, hiding baskets, barriers, conveyors |
| 24 | [dialogue-and-voice.md](dialogue-and-voice.md) | Dialogue kinds and events, talk lines and subtitles, speakers, voice playback, triggering, commentary |
| 25 | [effects.md](effects.md) | Particle emitters, explosions, flashlight beams, lightning, lights, sky, shadows, world darkening |
| 26 | [level-flow.md](level-flow.md) | Game structure, exits and warps, checkpoints and death, wipes, game start, episode unlocking, job doors, world map, ending |
| 27 | [minigames.md](minigames.md) | Boss, vehicle, turret, race and rhythm levels: goals, timers, health and win/lose rules |
| 28 | [ui-and-menus.md](ui-and-menus.md) | Fonts and text markup, menu and prompt text tables in the executable, screen flow, pause and options menus, hideout episode select, map screen, navigation |
| 29 | [hud.md](hud.md) | Display elements and their placement table, slide timings, counters, timer, prompts, auto-save icon, binocucom overlay, damage feedback |
| 30 | [save-format.md](save-format.md) | State that must persist, when the game saves, restore rules, slot list, guidance for SlyNX's own save file, optional import of original saves |
| 31 | [rendering.md](rendering.md) | Colour scales, three-palette lighting and light coefficients, pass list and draw order, backdrops and overlays, fog per world, outlines, billboards, blending, texture animation, blob and projected shadows |

## Path to a first rendered world

1. Find the executable through `SYSTEM.CNF`, check its hash, read world table entry *i* (disc.md 4).
2. Read and decompress the world; check its SHA-256 (world-file.md 2, disc.md 5).
3. Walk the top-level sections; read the material tables (world-file.md 4, textures.md 2).
4. Walk the object stream ([object-stream.md](object-stream.md)). For each object with a drawable block, record its transform
   and mesh set (geometry.md 1-3), and compose world transforms (geometry.md 6, transforms.md). Check the walk against worlds.md.
5. After the stream, align to 16 to get the texture base T; decode the bitmaps that shaders use (textures.md 3).
6. Build vertices from strip entries, triangles from the strip rule, bind each submesh's shader (geometry.md 3.3, 5).

## Conventions shared by all Sly 1 specs

- Little-endian; `u8/u16/u32`, `s8/s16/s32`, `f32` (IEEE-754 single); `vec3` = 3 x `f32`.
- Fields are packed with no alignment unless a spec says otherwise.
- Offsets in world-file tables are from the start of the decompressed world file, except texture offsets, which
  are from the texture base T.
- Test vectors come from the disc identified in [disc.md](disc.md). Tests that use them read the player's own image
  at run time; no game data is stored in the repository.

## Status

All documents are drafts awaiting review. Each ends with its open questions.
