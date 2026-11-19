---
id: sly1-behaviour-level-flow
title: Sly 1 level flow - start, exits, warps, checkpoints, wipes, hubs, map, unlocking, ending
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a; executable
      SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c; all 46 decompressed worlds
    used_for: >-
      every exit (114) with its level descriptors and options, every warp (399) with its name and options, the
      animation events 25 that wipe to another world, checkpoint, screen-mask, hub-progress, hub-selector, world-map
      and lock-group objects (counted through proxies), the per-world task masks of the world table; the exit graph
      and all test vectors below
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release: xform (warps and exits), chkpnt, wipe, keyhole, game,
      hubsel, hubprog, wm, transition, pzo (lock groups), jt (key placing), spaprops (option meanings)
    used_for: >-
      what exits, warps, checkpoints, wipes and the progress rules do; timings and thresholds. Read for behaviour,
      described here in our own words; marked [R]
  - kind: observation
    what: general knowledge of the game as played
    used_for: statements marked [O]
date: 2026-11-18
---

# Level flow

How the player moves through the game: start-up, the prologue and the hideout, episodes and their hubs, jobs,
exits, warps, checkpoints and death, screen wipes, the world map, unlocking, and the ending.

Tags as in [gameplay-overview.md](gameplay-overview.md): **[D]** disc data, **[O]** observable, **[R]** reference
behaviour to be confirmed, **[I]** inferred. "World *n*" is index *n* of the world table ([disc.md](disc.md) 5);
"id `0xEll`" is the world id (episode `E`, slot `ll`). Class numbers and option numbers are those of
[object-stream.md](object-stream.md) and [option-encodings.md](option-encodings.md).

## 1. Structure of the game

- **Episodes.** The world id's high byte is the episode: 0 intro, 1 to 5 the five episodes. [D] Episode titles are
  the movie titles of [movies.md](movies.md) 1.2 (Tide of Terror, Sunset Snake Eyes, Vicious Voodoo, Fire in the
  Sky, The Cold Heart of Hate). [D]
- **Slots within an episode** (low byte of the id). [D] for the ids, [R] for the roles:

  | Slot | Role | Episodes 1 to 4 | Episode 5 |
  |---:|---|---|---|
  | 0 | Approach: the way in, ends at the hub | worlds 3, 12, 21, 30 | world 39 (first of a chain) |
  | 1 | Hub: doors to the jobs | worlds 4, 13, 22, 31 | not on disc |
  | 2 to 7 | Jobs (levels) | six per episode | ids `0x502` to `0x506` (worlds 40 to 44) |
  | 8 | Boss | worlds 11, 20, 29, 38 | world 45 (id `0x508`) |

  Ids `0x501` and `0x507` do not exist. [D]
- **Intro worlds.** Splash (id `0x002`, world 0), Paris, France (`0x003`, world 1, the prologue), The Hideout
  (`0x004`, world 2, the hub of hubs). [D]
- **Tasks.** Byte 40 of each world-table entry ([disc.md](disc.md) 4.2) is the level's task mask: bit 1 (`0x2`) a key,
  bit 2 (`0x4`) the clue-bottle vault, bit 3 (`0x8`) the timed run. [D] for the values, [R] for the meanings.
  - `0xE` (all three): the approaches and the "thief" jobs: worlds 3, 5 to 8, 10, 12, 14, 17 to 19, 21, 23, 24, 26,
    30, 32, 33, 36. [D]
  - `0x2` (key only): the minigame jobs, worlds 9, 15, 16, 25, 27, 28, 34, 35, 37 ([minigames.md](minigames.md)). [D]
  - `0x0`: intro worlds, hubs, bosses and all of episode 5. [D]
  - So each of episodes 1 to 4 has exactly **7 keys**: the approach and the six jobs. [D]

## 2. Exits (class 125)

An exit is a convex trigger volume with two level descriptors (classes/markers-and-volumes.md 6). 114 exits in 43
worlds. [D]

- **Destination.** The first descriptor's identity is the destination's search key ([disc.md](disc.md) 4.2). [D]
  The second descriptor is on every exit of the disc the descriptor of world 0 (Splash) and is not used for
  normal play. [D] [I]
- **Options** (all values seen on the disc): [D] for counts, [R] for meanings.

  | Option | Type | Meaning | Exits using it |
  |---:|---|---|---:|
  | 163 | object ID | warp in the destination world to arrive at (an alias, 444 to 453) | 63 |
  | 166 | wipe kind | 0 fade, 1 iris (keyhole), 2 world map, 3 freeze | 94 (values: 2 in 88, 0 in 4, 1 in 1, 3 in 1) |
  | 168 | flag | **default exit** of the world (the one "leave level" uses) | 34 |
  | 169 | flag | **key exit**: only present once the level's key has been collected | 17 |
  | 170 | flag | follows the default exit | 3 |
  | 171 | flag | instead of its own destination, trigger the world's default exit | 17 (exactly the key exits) |
  | 172 | flag | show the collected-totals panel while the player is near | 30 (hub doors) |
  | 173 | seconds | delay between triggering and the wipe | 20 (0.2 to 4.0 s) |
  | 2 | — | (one exit carries an empty option 2) | 2 |

- **Triggering.** The player entering the volume triggers the exit. Its animations (owned by the exit) play; the
  wipe starts after option 173's delay, or when the animations end; then the destination world loads at the
  named warp, or at its default warp. [R] If the level's timed run is in progress, a key, default or "follows
  default" exit ends the run instead of leaving. [R]
- **Key exits.** In every job with a key, one key exit (always together with option 171) appears once the key is
  taken; walking into it leaves through the default exit, back to the hub door the player came from. [D] [R]
- **Self-exits.** Approaches 3, 21 and 30 have a default exit back to themselves with a fade; 25 and 28 have a
  fade exit to themselves (a retry). [D]
- **Leaving from the pause screen** uses the default exit of the world; with none, the game restarts the current
  world; in a hub with no default exit it goes to the episode's hub at warp 445; in episode 5 and the intro it
  goes to The Hideout. [R]
- **Warps blocked.** When the player arrives through a warp, every enabled exit in the world is blocked until the
  player has left its volume (no immediate bounce back). [R]

## 3. Warps (class 116)

Named arrival points (classes/markers-and-volumes.md 5); 399 in 41 worlds. [D]

- **Choosing the warp on arrival:** the warp whose OID matches the requested one in the requested scope; else a
  warp whose alias (option 159) matches; else the default warp (option 157); while a world-map wipe is running, a
  warp with option 158 wins over the default. [R]
- **Arriving:** the player is placed at the warp's position and orientation, given the warp's velocity (option 153,
  or 154 to 156 per axis), its animations play, the objects it lists are hidden, and the camera cuts behind the
  player, turned by option 160 (radians) if set. The warp raises script event 2. [R]
- Options on the disc: 157 default (40 warps), 158 default after a map wipe (12), 159 alias (45; values 445 to 453),
  160 initial camera angle (19; values 0, ±0.5, 1.0, 0.3930), 161 state-machine goal (1). [D]
- **Hub return points.** The episode hubs hold one warp per door with aliases 446 to 453; each job's exits name the
  alias of its own door (exit option 163), so the player reappears outside the door of the job just left. [D]
- Warp names are game strings; frequent ones: `check_point1` (17), `check_point2` (12), `from_hub` (7),
  `back_of_van` (5), `mystery_machine` (5), `default` (5). [D]

## 4. Checkpoints (class 56) and death

51 checkpoints in 25 worlds, counting every proxy clone, including clones nested inside cloned sources (30 records are written in full in the stream). [D]

- A checkpoint is triggered when the player enters one of its volumes, or hits it. Triggering records it in the
  level's checkpoint bits and saves the respawn point: its warp (a child warp, or the warp named by its options)
  and the bits so far. Its state machine goes to "just triggered" (state OID 1352) or, if it was already
  triggered, to 1353; at load, an already-triggered checkpoint starts in state 1351. Its animations play and the
  controller rumbles (medium, 0.2 s) when hit. [R]
- A triggered checkpoint re-arms only after the player has moved more than **1000 units** from the volume that
  triggered it. [R] Checkpoints disappear during the timed run. [R]
- Options on every checkpoint: 91 = 14, 92 = 90.0, 93 = true (target settings), 101 = sound 129 (0x81),
  102 = 1000.0, 103 = 0.0, 130 = true. [D] (101 to 103 select the sound and its distances; [R].)
- **Death:** one life is lost. With lives below 0 the "game over" prompt appears. Otherwise the world reloads with
  an iris wipe at the last checkpoint's warp (with the checkpoint bits restored), or at the world's start if no
  checkpoint was reached. In a hub, the world reloads at the warp the player last arrived through. [R]
- **New game and game over:** 5 lives, 0 charms, checkpoints cleared. [R]

## 5. Screen wipes

[R] unless marked.

| Wipe kind | Out (to black) | In (from black) | Use |
|---|---|---|---|
| 0 fade | 0.5 s | 0.25 s | ordinary transitions |
| 1 iris ("keyhole") | 1.5 s | 1.0 s | death, retry, reload, return to the hideout after a boss |
| 2 world map | 0.5 s | handled by the map (section 8) | almost every exit (88 of 94 with a kind) [D] |
| 3 freeze | 0.5 s | 0.25 s | Splash to Paris (the only one on the disc) [D] |

- During "out" the sound fades with the picture; the new world loads while black. [R]
- **The iris shape** is the screen mask (class 142): one per world, identical in all 46 worlds [D]: 127 points,
  bounds −35 to 35 in both axes, and five triangle lists of 47, 53, 22, 22 and 62 triangles. The shape is drawn
  centred on the player's screen position (kept at least 10 pixels from the left and right edges and 16 pixels from
  the top and bottom), scaled from `2 × screen width / 70` when open to `4 / 70` when closed. [R]
  Drawing, with u the closing amount 0 → 1 [R]: the five triangle sets are drawn from set 4 down to set 0, each in
  its shader colour (classes/paths-and-maps.md 5) with its alpha multiplied by clamp(2.5 u, 0, 1); set 0's colour
  is also darkened toward black as u grows (its red, green and blue × (1 − u), its alpha moved toward 1), and the
  rest of the screen outside the shape's bounding box is filled with that same set-0 colour. From u = 0.9 the
  bounding box itself is filled too, reaching full cover at u = 1. So the wipe is a blue mask with white eyes
  shrinking onto the player over a darkening screen, not a hole showing the scene; only the death wipe follows the
  player (other iris wipes close at the screen centre). [R]
- Animation events 25 and 26 start a wipe to a world (26 waits for a button); event 27 triggers the world's
  default exit (animation.md 5.3). [D]

## 6. Start of the game

1. **Start-up.** The game wipes (fade) into world 0, Splash. [R] The start-up logo movie (identity `0x344`) plays
   once at power-on ([movies.md](movies.md)). [O] Splash shows the title screen and, when idle, the attract mode
   (animation events 55, 56). [D] for the events, [O]
2. **New game** starts in Paris (id `0x003`) with 5 lives. [R] Splash's single exit (OID 1369, default, freeze
   wipe) leads to Paris at warp 445. [D]
3. **Paris** (prologue) ends with an animation event 25 to The Hideout. [D] Paris plays the movie "The Secret Police
   File" (identity `0x340`). [D]
4. **The Hideout** has no exits; five animation events 25 lead to the five approaches (worlds 3, 12, 21, 30, 39). [D]
   It holds the episode selector (class 105, OID 1568), five hub-progress objects (class 106, OIDs 1301 to 1305, one
   per episode) and five world maps (class 103, OIDs 454 to 458). [D]

## 7. Episode selection and unlocking

[R] unless marked; to confirm by play.

- **Selector.** In The Hideout the player picks an episode. An episode already visited is entered directly. The
  first time an episode is picked, its introduction movie is unlocked, the selector's animation for that episode
  plays (which shows the movie and wipes), and the game goes to the episode's approach with a fade.
- **Which episodes can be picked.** The highest pickable episode is the largest *n* (from 5 down, but at least 2 on
  the USA disc) for which episode *n* − 2 is complete or episode *n* − 1 has been entered. So episodes 1 and 2 are
  open from the start; entering an episode, or completing the one before, opens the next. Episode 5 also needs
  episode 4 complete before the selector shows it as the current goal.
- **Episode complete** is set when the episode's boss is beaten (it unlocks the boss's movie, section 10).
- **Hub progress (class 106).** Object OID `1300 + episode` shows nine markers: markers 0 to 7 light up when that
  slot's level has its key (or, for a level without a key task, has been visited); marker 8 when the episode is
  complete. [D] for the OIDs

### 7.1 Job doors and keys (lock groups, classes 92 and 93)

- Each lock group has a number of key slots and a world-state bit. When the player uses it, one collected key is
  placed per slot; if the episode's key count is at least the number of slots, the group opens and sets its bit,
  otherwise it shows the partial state. Lock states: 558 closed, 559 open, 560 partly filled, 561 empty slot. [R]
  Confirmed in the executable [X]:
  - **Loading** (`0x00199218`): the group collects its lock objects (at most 8: the ones named by option 1000, then
    lock objects found among its children) into a list (count at +0x300). If the world state's lock bits (+0x448)
    contain the group's bit, every lock and the group show 559 (open); otherwise they show 558 (closed).
  - **Using** (`0x00170DCC`): with k = the episode's key count (world state +0x438) and n = the number of locks,
    lock i shows 561 (empty) when i ≥ k. When i < k it shows 559 if k ≥ n, else 560. The group itself goes to 559 and
    ORs its bit into +0x448 when **k ≥ n**; otherwise it shows 560.
- The bit is `2 << index`, index from option 1002 [X: the option's setter `0x00199380` stores 1 << (index + 1) at
  +0x2D0]. Lock groups on the disc (slots counted from option 1000; the
  approaches' groups also find lock objects among their children): [D]

  | World | Lock group OID | Slots in options | Index (option 1002) |
  |---|---|---:|---:|
  | 3 A Stealthy Approach | 1740 | 1 | 2 |
  | 4 Prowling the Grounds | 1608 / 1843 | 7 / 3 | 1 / 0 |
  | 12 A Rocky Start | 1554 | 1 | 0 |
  | 13 Muggshot's Turf | 1706 / 1992 | 7 / 3 | 2 / 1 |
  | 21 The Dread Swamp Path | 1843 | 1 | 0 |
  | 22 The Swamp's Dark Center | 1859 / 1905 | 7 / 3 | 2 / 1 |
  | 30 A Perilous Ascent | 1752 | 1 | 0 |
  | 31 Inside the Stronghold | 1400 / 1444 | 7 / 3 | 2 / 1 |

- **Rule:** in each hub a 3-key door opens the later jobs and a 7-key door opens the boss. This follows from the
  code above and the slot counts on the disc (3 and 7) [X] [D]. In episode 1, four jobs are open once the hub has been visited; in episodes 2 to 4,
  three. The others open with the 3-key door (episode 1: bit `0x2`; episodes 2 to 4: bit `0x4`). [R] [O]

## 8. World map (class 103)

- One map object per world in each episode (OIDs: episode 1 455, episode 2 456, episode 3 457, episode 4 454,
  episode 5 458), all five in The Hideout; option 119 = true on all 48. [D]
- The world-map wipe shows the episode map, moves a marker along the route from the current slot to the destination
  (the map's animation, played forwards or backwards), shows the collected totals, then loads the destination.
  The letterbox shows for 2 s (0.5 s for The Hideout). [R]
- From the pause screen the map lets the player pick any slot already visited, plus the jobs opened by section 7.1;
  the hub slot of episode 5 and slot 7 do not exist there. Map sounds: open 138, close 139, move 121/123. [R]

## 9. The exit graph (test vector)

All exits and world-changing animation events on the disc. [D] "OID n to w" = exit object n leads to world w;
"warp" = option 163; "map/fade/iris/freeze" = wipe kind; "anim" = number of animations owned by the exit; the last
column lists the destinations of animation events 25.

| World | Name | Exits | Event-25 wipes |
|---:|---|---|---|
| 0 | Splash | OID 1369 to 1 (default, warp 445, freeze) | - |
| 1 | Paris, France | - | 2 |
| 2 | The Hideout | - | 21, 3, 30, 12, 39 |
| 3 | A Stealthy Approach | OID 1908 to 3 (default, fade); OID 1910 to 4 (warp 445, map, 1 anim) | 2 |
| 4 | Prowling the Grounds | OID 1461 to 9 (totals, map, delay 4 s); OID 1800 to 10 (totals, map); OID 1957 to 8 (totals, map); OID 1958 to 4 (default, fade); OID 1959 to 3 (totals, warp 446, map, 1 anim); OID 1960 to 5 (totals, map); OID 1961 to 7 (totals, map); OID 1838 to 11 (warp 446, map, delay 0.6 s); OID 1962 to 6 (totals, map, delay 0.25 s, 1 anim) | - |
| 5 | High Class Heist | OID 1709 key exit; OID 1960 to 4 (default, warp 446, map); OID 1961 to 4 (warp 446, map, delay 0.5 s, 1 anim) | - |
| 6 | Into the Machine | OID 1713 key exit; OID 2192 to 4 (default, warp 452, map); OID 2193 to 4 (warp 452, map, delay 0.5 s, 1 anim) | - |
| 7 | A Cunning Disguise | OID 1375 key exit; OID 1908 to 4 (default, warp 448, map); OID 1909 to 4 (warp 448, map, delay 0.3 s, 1 anim) | - |
| 8 | The Fire Down Below | OID 1782 key exit; OID 2042 to 4 (default, warp 447, map); OID 2043 to 4 (warp 447, map, 1 anim) | - |
| 9 | Treasure in the Depths | OID 1416 to 4 (default, warp 450, map); OID 1417 to 4 (map) | - |
| 10 | The Gunboat Graveyard | OID 1570 key exit; OID 2018 to 4 (default, warp 449, map); OID 2019 to 4 (warp 449, map, delay 0.75 s, 1 anim) | - |
| 11 | The Eye of the Storm | OID 1866 to 4 (default, warp 453, map) | 2 |
| 12 | A Rocky Start | OID 2129 to 13 (default, warp 445, map, 1 anim) | 2 |
| 13 | Muggshot's Turf | OID 2108 to 19 (totals, map, delay 1 s, 1 anim); OID 2109 to 16 (totals, map, delay 1 s, 1 anim); OID 2110 to 15 (totals, warp 446, map); OID 2111 to 18 (totals, map, delay 0.75 s, 1 anim); OID 2112 to 17 (totals, map, delay 0.75 s, 1 anim); OID 2113 to 14 (totals, map, delay 0.75 s, 1 anim); OID 2114 to 20 (map, delay 0.75 s, 1 anim); OID 2115 to 12 (totals, warp 446, map, delay 0.75 s, 1 anim) | - |
| 14 | Boneyard Casino | OID 1778 key exit; OID 2228 to 13 (default, warp 448, map); OID 2229 to 13 (warp 448, map, delay 0.5 s, 1 anim) | - |
| 15 | Murray's Big Gamble | OID 1536 to 13 (follows default, warp 451, map); OID 2186 to 13 (default, warp 451, map) | - |
| 16 | At the Dog Track | OID 1495 to 13 (default, warp 452, map) | - |
| 17 | Two to Tango | OID 2284 key exit; OID 2311 to 13 (follows default, warp 450, map); OID 2487 to 13 (default, warp 450, map) | - |
| 18 | Straight to the Top | OID 1638 key exit; OID 2190 to 13 (default, warp 449, map) | - |
| 19 | Back Alley Heist | OID 1645 key exit; OID 2102 to 13 (default, warp 447, map); OID 2103 to 13 (warp 447, map, delay 0.5 s, 1 anim) | - |
| 20 | Last Call | OID 1402 to 13 (default, map) | 2 |
| 21 | The Dread Swamp Path | OID 2102 to 21 (default, fade); OID 2103 to 22 (warp 445, map, 1 anim) | 2 |
| 22 | The Swamp's Dark Center | OID 2019 to 23 (totals, map, 1 anim); OID 2020 to 26 (totals, map, 1 anim); OID 2021 to 25 (totals, map, 1 anim); OID 2022 to 27 (totals, map); OID 2023 to 24 (totals, map, 1 anim); OID 2024 to 28 (totals, warp 446, map, 1 anim); OID 1873 to 29 (map); OID 2025 to 21 (totals, warp 446, map, 1 anim) | - |
| 23 | The Lair of the Beast | OID 1785 key exit; OID 2123 to 22 (default, warp 449, map); OID 2124 to 22 (warp 449, map, 1 anim) | - |
| 24 | A Grave Undertaking | OID 1799 key exit; OID 2124 to 22 (default, warp 446, map); OID 2125 to 22 (warp 446, map, delay 0.2 s, 1 anim) | - |
| 25 | Piranha Lake | OID 1400 to 25 (warp 447, fade); OID 1612 to 22 (totals, warp 450, map); OID 1401 to 22 (default, totals, warp 450, map) | - |
| 26 | Descent into Danger | OID 1736 key exit; OID 2225 to 22 (default, warp 448, map); OID 2226 to 22 (warp 448, map, 1 anim) | - |
| 27 | A Ghastly Voyage | OID 1737 key exit; OID 1840 to 22 (default, warp 447, map); OID 2081 to 22 (warp 447, map, delay 1.5 s, 1 anim) | - |
| 28 | Down Home Cooking | OID 1550 to 28 (warp 447, fade); OID 1576 to 22 (default, warp 451, map) | - |
| 29 | A Deadly Dance | OID 2075 to 22 (default, map) | 2 |
| 30 | A Perilous Ascent | OID 2199 to 30 (default, fade); OID 2200 to 31 (warp 445, map, 1 anim) | 2 |
| 31 | Inside the Stronghold | OID 1883 to 35 (totals, map); OID 1884 to 37 (totals, map); OID 1885 to 34 (totals, warp 446, map); OID 1886 to 32 (totals, map); OID 1887 to 36 (totals, map); OID 1888 to 38 (warp 446, map); OID 1889 to 33 (totals, map); OID 1890 to 30 (totals, warp 446, map) | - |
| 32 | Flaming Temple of Flame | OID 1396 to 31 (default, warp 446, map); OID 1690 key exit; OID 1872 to 31 (warp 446, map, 1 anim) | - |
| 33 | The Unseen Foe | OID 2157 key exit; OID 2158 to 31 (warp 447, map, 1 anim); OID 2159 to 31 (default, warp 447, map) | - |
| 34 | The King of the Hill | OID 1600 to 31 (default, follows default, warp 448, map) | - |
| 35 | Rapid Fire Assault | OID 1581 key exit; OID 2114 to 31 (default, warp 449, map) | - |
| 36 | Duel by the Dragon | OID 1737 key exit; OID 2502 to 31 (default, warp 450, map) | - |
| 37 | A Desperate Race | OID 1592 to 31 (default, warp 451, map) | - |
| 38 | Flame Fu! | OID 1712 to 2 (iris); OID 1713 to 31 (default, warp 444, fade) | 2 |
| 39 | A Hazardous Path | OID 1932 to 40 (map) | - |
| 40 | Burning Rubber | OID 1683 to 41 (map) | - |
| 41 | A Daring Rescue | OID 2031 to 43 (warp 446, map, 1 anim); OID 1886 to 42 (map); OID 2042 to 40 (warp 447, map, delay 0.75 s, 1 anim) | - |
| 42 | Bentley Comes Through | OID 1370 to 41 (warp 446, map) | - |
| 43 | A Temporary Truce | OID 1997 to 44 (warp 446, map) | - |
| 44 | Sinking Peril | OID 1690 to 45 (warp 446, map, delay 0.3 s, 1 anim) | - |
| 45 | A Strange Reunion | - | - |

Reading the graph: each episode 1 to 4 runs approach → hub → jobs and boss → back to the hub; the bosses
(11, 20, 29, 38) wipe to The Hideout. [D] Episode 5 is a chain: 39 → 40 → 41 → 43 → 44 → 45, with 42 a side trip
from 41 and back. [D]

Other counts per world [D]: warps — 0, 15, 0, 13, 16, 10, 13, 6, 10, 1, 7, 2, 16, 21, 14, 10, 0, 12, 14, 20, 5, 14, 9,
11, 18, 2, 13, 7, 2, 3, 22, 11, 18, 13, 3, 7, 14, 0, 3, 3, 2, 4, 0, 2, 8, 5 (worlds 0 to 45). Checkpoints (through
proxies) — world 1: 1; 3: 2; 5, 6, 7, 8, 10, 11: 1 each; 12: 2; 14, 17, 18, 19: 1 each; 21: 2; 23, 24, 26, 27, 29: 1
each; 30: 2; 32, 33, 35: 1 each; 36: 2; 38: 1 (total 30).

Screen mask: OID 1367 in every world; the shape bytes (from the point count to the end of the fifth triangle list,
2,280 bytes) have SHA-256 `931fa9a102e01a3e43fd53f02c97789ea0806346a2aab988482ff9d75bb0d1ad` in world 1. [D]

## 10. Ending and the movie menu

[R] unless marked.

- **Bosses.** Beating a boss marks its episode complete and wipes (iris) to The Hideout, with the boss's movie
  ([movies.md](movies.md) 1.2: Raleigh Hung out to Dry, Muggshot Busted, Mz Ruby Vexed, The Panda King on Ice). [D]
  for the movie references in worlds 11, 20, 29, 38
- **Movie menu unlocking.** One bit per movie title (title index *i*, [movies.md](movies.md) 1.2). Entering an
  episode for the first time unlocks its introduction (titles 4, 6, 8, 10, 12); completing it unlocks the boss
  movie (5, 7, 9, 11). Collecting every key of the game (with every episode complete) unlocks titles 1, 13 and 15;
  every clue-bottle vault unlocks 14 and 15; every timed run unlocks 2 and 3. Title 0 (The Secret Police File) is
  seen in the prologue.
- **Final ending.** After the last boss (world 45), the ending movie plays: title 13 "Ten Seconds" for a key-complete
  game, title 14 "The Thievius Raccoonus" when every vault is open, followed by title 15 "Credits"; then the game
  returns to The Hideout with an iris wipe.
- **Completion percentage.** Count one task per non-intro world (visited), one per task bit in its mask, and one per
  episode completed; percentage = completed × 100 / total, shown as 1 to 99 unless none or all are done. With the
  disc's task masks the total is 43 non-intro worlds + 28 key tasks + 19 vault tasks + 19 timed-run tasks + 5
  episodes = **114 tasks**. [D] for the counts

## 11. Open questions

- The episode-selection rule is from the reference port (the key-door rule is settled, 7.1); check against a save
  file from the original game.
- The second level descriptor on every exit (always world 0) and the one exit option 2 have no known use.
- How the approaches' single-slot lock groups behave.
- The exact screen position and timing of the world-map route animation.
