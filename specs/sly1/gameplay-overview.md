---
id: sly1-gameplay-overview
title: Sly 1 runtime systems, overview and roadmap
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds
    used_for: >-
      which classes occur where and how often (object-stream.md section 6), which objects carry scripts, state
      machines, animations and events; the class roles marked [D]
  - kind: observation
    what: the game as played, general knowledge of its mechanics
    used_for: the plain-English descriptions marked [O]; to be re-checked frame by frame in an emulator by the
      gameplay specs
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files main, update, clock, sw, game, and the per-class
      behaviour files (read for what each class does, not how it is written)
    used_for: per-frame order, the clocks, which classes have run-time behaviour; marked [R]
date: 2026-11-13
---

# Runtime systems: overview and roadmap

A map of what runs in a world, for planning the behaviour specs of the next round. It says what each part does in
plain words, not how any program is written.

Each statement carries a tag:

- **[D]** confirmed from the disc data (counts, which objects exist, what the data references);
- **[O]** observable by playing the game;
- **[R]** read from the reference's behaviour, to be confirmed by observation;
- **[I]** inferred.

## 1. Clocks

- **Game clock.** Each displayed frame, the game measures the real elapsed time and clamps it to between 1/60 s
  and 1/30 s. That is the frame step. The game clock advances by the frame step times a global time scale, and by 0
  while the game is paused. [R] The time scale changes with slow-motion effects (a power-up, some cut-ins). [O]
- **Real clock.** A second clock always advances by the frame step, even while game time is paused or slowed.
  Animations with option 189, some faders and some screen effects run on it (animation.md 5.1). [R]
- Animation key times are in 1/60 s units and every timing in the data is in seconds or 1/60 s frames. [D] The
  original runs at 60 frames per second on an NTSC console. [O] The port targets 60 frames per second.

## 2. Order of one frame

[R], in our own words; the gameplay specs refine each step.

1. Read the controller. Update the menus and the overlay screens, then the game state (level transitions, wipes,
   timers, pending cutscenes).
2. Advance the clocks (section 1).
3. Real-clock pass: update the objects, animation playbacks and faders that run on the real clock.
4. If game time advanced:
   1. The player-controlled object reads the controller and decides its move.
   2. Every active object updates, parent before children (its own behaviour, then its children's). Inactive
      objects (asleep, out of range, frozen) are skipped. Placed objects with a module raise event 1 (scripts.md 6).
   3. Every animation playback advances (animation.md 5.2) and fires its events.
   4. Faders advance, timed explosions fire, queued callbacks run.
   5. Physics: find contacts between solid objects, solve them, update pivots, solve again with more iterations;
      mark objects that came to rest; resolve impacts (damage, breaking, sounds); raise touch events (6 and 7).
   6. Run the scripts' scheduled calls whose time is up (scripts.md 5), then queued callbacks again.
   7. Advance texture animations (shader animation records).
5. Update the camera (even when game time is paused), sounds, the on-screen counters (HUD).
6. Remove animation playbacks that finished this frame. Update the screen darkening.
7. Draw the world, then the overlay screens.

## 3. Life of a world

1. **Load** the world file (world-file.md), create every object (object-stream.md), place proxies (classes/proxy.md),
   build the script modules and run them once (scripts.md 1). [D] [R]
2. **Bind**: every character and machine looks up its parts by OID (animation.md 7); skinned meshes record their
   bind matrices (animation.md 8.1). [R]
3. **Post-load**: every object raises event 0; auto-start animations (option 179) and state machines (option 513)
   start; difficulty adjustments run (for example, pickups that would be pointless are removed). [R] [D for the
   option counts]
4. **Play** frame by frame (section 2).
5. **Leave** through a level exit (class 125, two level descriptors) or an animation event 25/26 (wipe to a level),
   with a screen wipe; the game state saves progress and loads the next world. Dying restarts at the last
   checkpoint (class 56). [O] [D for the classes and events]

## 4. Systems and the classes that carry them

Class IDs are those of object-stream.md section 6. "Data" says what in the stream drives the system.

| System | Classes | What it does | Data |
|---|---|---|---|
| Player character | 7, its parts (131 animations, 3 collision parts, 136 idle machine, 139 reach map, 94 tail, 97 shadow) | the thief: moves, jumps, climbs, sneaks, attacks, uses power-ups; takes hits; dies | animation lookup list (animation.md 7), options, the reach map for reachable ledges [D] [O] |
| Camera | 78 placements, 99 camera zones, the run-time camera manager | follows the player; zones swap in fixed or constrained views; cutscenes take placements | zone volumes and paths (classes/markers-and-volumes.md 7) [D] [O] |
| Guards and enemies | 9 to 19, 74, 75, their animations (131, 136), 126 to 130 points, 138 path zones, 50 searchlight cameras | patrol, look, chase, attack; react to noise and light; bosses | patrol points, navigation meshes, state machines, options [D] [O] |
| Alarms and sensors | 47 alarms, 49 lasers, 50 searchlights, 51 pressure plates, 34 breakable alarms | detect the player and raise an alarm that alerts guards; can be disabled | options, events 2, 3, 22 [D] |
| Collectibles | 46 clue bottles, 86 charm, 87 life token, 88 coins, 89 key, 90, 91 prizes; 23 vault | coins add to a counter (100 give a charm); clue bottles count toward the level's vault; the key finishes a level | stock templates (OIDs 12 to 42) for spawned pickups [D] [O] |
| Breakables | 33 to 39 | break when hit, scatter pieces (39), may drop pickups | piece hierarchies, event 20 [D] |
| Buttons and triggers | 40 button, 41, 42 trigger volumes, 123, 124 volumes | trigger animations and scripts when pressed or entered | attached animations (options 251 to 272), events 2, 3, 12 [D] |
| Movement aids | 110 to 113 curves (pipes, rails, ledges), 129 perch points ?, 128 jump targets ?, 100 jump locations ?, 143 jump guides ? | the context moves: climb pipes, walk rails, land on perch points, guided jumps | curves and points, the binding list [D] [O] |
| Hazards | 54, 55 hazard zones and volumes, 31 water, 59 to 63 bombs, missiles, mines | damage or kill on contact (electric, fire, water, falls) | volumes, options [D] [O] |
| Hiding | 58 hiding places | the player hides inside; guards cannot see | event 19 [D] [O] |
| Power-ups | 76 decoy, 63 mine, and animations in the player's lookup list | gadgets earned from vaults | the player's lookup list [D] [O] |
| Dialogue and voice | 101 dialogues, 102 speakers | spoken lines with subtitles, camera cuts, script calls; the radio voice in the binocucom view | dialogue events (classes/placed-object.md 5.2), voice descriptors [D] [O] |
| Binocucom | (screen effect and camera mode, no world class) | the first-person look-around view with the radio commentary | camera and overlay specs [O] [I] |
| Effects | 80 emitters, 120 to 122 explosions, 96 flash volumes, 145 lightning flashes, 81 lights, 84 sky, 97 shadows | particles, explosions, lighting, sky | emitter data, options [D] |
| Animation and scripting | 131 segments, 136 state machines, script modules | everything scripted in a level | animation.md, scripts.md [D] |
| Visibility | 134 visibility map, zone masks | draw only what a zone can see | classes/paths-and-maps.md 4 [D] |
| Level flow | 125 exits, 116 warps, 56 checkpoints, 142 screen mask | leaving, warping, respawning, the iris wipe | descriptors, animations [D] |
| Hub and menus | 105 hub selector, 106 hub progress, 103 world map | the episode hub, the map; menus and the pause screen are drawn by the game itself, not from world objects | [D] [O] |
| Minigames | 20 tank, 21 to 29 vehicles, turret, rhythm game, 65 to 69 small actors, 144 rhythm sequences | driving, shooting and rhythm levels | options, events 26 to 28 [D] [O] |
| Stock templates | OIDs 12 to 42 at top level | objects cloned at run time (pickups, effects) | object-stream.md 7.1 [D] |

## 5. The player's moves (plain English)

[O] unless marked. The per-move specs will give timings, speeds and the animation used for each (animation.md 7).

- **Ground**: run (analog speed, blended run cycle), stop, turn; sneak along walls by flattening (eight directions,
  peeking out at corners); duck walk; sidestep.
- **Air**: jump from standing, running or swinging; land; land on a perch point (a sparkle marks it; one button press
  lands exactly on it); guided jumps along jump guides.
- **Climb**: grab and climb pipes (up, down, around); walk along rails and narrow ledges with balance; hang from and
  shimmy along ledges; swing on hooks.
- **Hide**: get into hiding places and move while hidden.
- **Attack**: cane swipe forward and back (moving or standing), rush, smash, dive attack; knock guards out.
- **Damage**: a hit without a charm loses a life and restarts from a checkpoint; a charm absorbs one hit; damage
  types (blunt, electric, fire, water, crush, falling into a pit) have their own reactions.
- **Celebrate**: on collecting a key, on timed goals.
- **Power-ups** (from vaults): decoy, mine and others, each with its own animation and object.

## 6. Roadmap: behaviour specs for the next round

| Spec | Scope | Depends on |
|---|---|---|
| clock-and-loop.md | clocks, pause, slow motion, frame order (section 2) | — |
| physics.md | contacts, solver, pivots, rest detection, impacts, the easing of animated objects | collision data (classes/solid-object.md) |
| player-moves.md | every move of section 5: inputs, speeds, timings, transitions, animations used | animation.md, physics.md |
| camera.md | follow camera, camera zones, cutscene placements, binocucom view | classes/markers-and-volumes.md |
| guards.md | patrol, perception (sight, sound, light), alert, chase, attack, knock-out, per guard type | state machines, path zones |
| alarms-sensors.md | alarms, lasers, searchlights, pressure plates | guards.md |
| pickups-and-progress.md | coins, charms, life tokens, clue bottles, vaults, keys, saving | stock templates |
| interactables.md | buttons, trigger volumes, breakables, hazards, hiding places | scripts.md |
| dialogue-and-voice.md | dialogue events, subtitles, voice clips, speakers | audio specs |
| effects.md | emitters, explosions, flash volumes, lightning, darkening | renderer specs |
| level-flow.md | exits, warps, checkpoints, wipes, the hub and the world map | disc.md level table |
| minigames.md | vehicle, turret, rhythm and boss levels | most of the above |
| options-by-class.md | the meaning of the stream and script options each class uses | option-encodings.md, scripts.md 8 |

## 7. Open questions

1. Whether the 1/60 s lower clamp of the frame step is a fixed rule of the original or depends on the video mode
   (the original also shipped in a 50 Hz region).
2. Which classes are drawn every frame regardless of the visibility map (sky, the player) and how the culling
   interacts with cutscenes.
3. The guard types' roles marked "?" in object-stream.md section 6.
4. Where the binocucom view lives in the data, if anywhere.
