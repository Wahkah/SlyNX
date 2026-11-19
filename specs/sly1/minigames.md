---
id: sly1-behaviour-minigames
title: Sly 1 minigame and boss levels
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a; executable
      SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c; all 46 decompressed worlds
    used_for: >-
      which worlds hold which vehicle, turret, rhythm, boss and helper classes (counted through proxies); which
      worlds have no player character; option values on those objects; the task masks of the world table
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release: tank, rog (remote vehicle game), turret, puffer, suv, lgn,
      jetpack, rythm, mgv, clkwork, boss, spaprops (option meanings)
    used_for: >-
      goals, win and lose conditions, timers, counts, speeds and difficulty ranges; read for behaviour and
      described in our own words; marked [R]
  - kind: observation
    what: general knowledge of the game as played
    used_for: statements marked [O]
date: 2026-11-13
---

# Minigame and boss levels

Tags as in [gameplay-overview.md](gameplay-overview.md): **[D]** disc data, **[O]** observable, **[R]** reference
behaviour to be confirmed, **[I]** inferred. World numbers are indices of the world table ([disc.md](disc.md) 5);
class numbers are those of [object-stream.md](object-stream.md) section 6. Names chosen here for the classes are
descriptive; the class number is the identifier.

## 1. Which worlds

The world table marks the minigame jobs with a task mask of `0x2` (key only, no vault and no timed run)
([level-flow.md](level-flow.md) 1). [D] With the bosses and episode 5:

| World | Name | Kind | Distinctive objects [D] | Player character (class 7) present [D] |
|---:|---|---|---|---|
| 9 | Treasure in the Depths | underwater remote-vehicle defence | 21 remote vehicle ×1; 66, 67, 68, 69 ×1 each; 95 game controller ×2 | no |
| 15 | Murray's Big Gamble | turret defence | 22 turret ×1; 14 companion ×1 | yes |
| 16 | At the Dog Track | vehicle race | 25 vehicle ×5 (one playable); 14 companion | yes |
| 25 | Piranha Lake | creature-shooting under a timer | 24 player creature ×1; 15 food creature ×1; 72 target ×1; 68 ×1 | yes |
| 27 | A Ghastly Voyage | armoured-vehicle level | 20 armoured vehicle ×1 | yes |
| 28 | Down Home Cooking | arena with spawned enemies | 17, 18 guard variants; 104 spawner ×1 | yes |
| 29 | A Deadly Dance (boss) | rhythm duel | 19 boss ×1; 28 rhythm controller ×1; 144 rhythm sequences ×4 | yes |
| 34 | The King of the Hill | turret defence | 22 turret ×1; 14 companion | yes |
| 35 | Rapid Fire Assault | armoured-vehicle level | 20 armoured vehicle ×1 | yes |
| 37 | A Desperate Race | vehicle race | 25 vehicle ×2 (one playable); 14 companion | yes |
| 39 | A Hazardous Path | gun-platform level | 26 gun platform ×1 | yes |
| 40 | Burning Rubber | vehicle chase | 25 vehicle ×1 (playable) | no |
| 42 | Bentley Comes Through | small remote vehicle collecting codes | 29 small vehicle ×1 | no |
| 43 | A Temporary Truce | turret defence | 22 turret ×1 | yes |
| 45 | A Strange Reunion (final boss) | jet-pack flight against the final boss | 27 jet pack ×1; 74 final boss ×1; 75 its damage part ×1 | yes |

The armoured vehicle (class 20) also stands in the hubs of episodes 3 and 4 (worlds 22 and 31). [D] Bosses of
episodes 1, 2 and 4 (worlds 11, 20, 38) are fought on foot with the normal moves (guards spec). [O]

Common rules [R]:

- When the player character is absent, the playable vehicle is the player object. Vehicles take charms like the
  player: a charm absorbs a hit and the vehicle flashes for 2 s; pickups are collected automatically within a radius
  (150 units for the armoured vehicle, 200 for race vehicles, 300 for turrets).
- Losing restarts the minigame through the checkpoint rules of [level-flow.md](level-flow.md) 4; every loss also
  feeds the difficulty adjustment ("easy" to "hard" ranges below move towards "easy" after failures).
- Winning a key-only level gives its key ([level-flow.md](level-flow.md) 1).

## 2. Boss health

The boss health bar, per episode [R]:

| Episode | Phases | Hits per phase | Total | Health shown full at start |
|---:|---:|---:|---:|---|
| 1 (world 11) | 4 | 1 | 4 | no (fills in) |
| 2 (world 20) | 3 | 1 | 3 | yes |
| 3 (world 29) | 4 | 1 | 4 | yes |
| 4 (world 38) | 5 | 10 | 50 | no |
| 5 (world 45) | 4 | 5 | 20 (the final boss sets 4 phases × 4 when its last stage starts) | no |

Each hit removes one; when a phase's hits run out the next phase starts with full hits; the bar shows
(remaining full phases × hits per phase + hits left) / total. [R]

## 3. World 9: remote vehicle under water

[R] unless marked.

- The player steers a remote underwater vehicle (class 21): left stick drives, right stick aims its turret; it can
  brake, descend and ascend; its guns fire continuously while firing is held.
- **Targets** (class 67) appear one at a time. Each takes up to **8** hits; the number of targets to destroy is
  **40** by default (the game's counter shows destroyed / total). **Carriers** (class 66) wander, grab a target and
  try to carry it to the exit; class 68 is the treasure container (open/closed), class 69 a revealed item to collect
  at the end.
- **Lose:** any target carried out. A dialogue plays (OID 830), the carriers celebrate, and the attempt restarts.
- **Win:** all targets destroyed. A dialogue plays (OID 829), the carriers sulk, the vehicle ascends; the level
  ends 5 s later (or 3 s after the final item is collected).
- **Difficulty** (blended from easy to hard by progress, `destroyed / total`):

  | Quantity | Easy | Hard |
  |---|---|---|
  | Targets active at once | 2 | 5 |
  | Carriers active at once | 0 | 8 |
  | Seconds between new targets | 1.0 to 2.5 | 0.1 to 0.3 |
  | Seconds between new carriers | 1.5 to 3.0 | 0.15 to 0.4 |
  | Carrier speed (units/s) | 400 | 1500 |
  | Carrier turn rate (rad/s) | 3 | 6 |

- The two game-controller objects (class 95) carry options 771 to 783 (object references and the easy/hard numbers
  above as data: for example 782 = (0.2, 0.3), 783 = (0.1, 0.2)). [D]

## 4. Turret defence (worlds 15, 34, 43)

[R] unless marked.

- The player mans a turret (class 22) while enemies attack a companion or a position. Aiming uses the binocular
  reticle: a ray from the reticle's screen point through the camera picks the target; the camera zoom limit is 10
  while manning (100 normally). Option 1041 = 2 on all three turrets (the look mode). [D]
- The turret holds up to 16 target objects. Firing launches the turret's projectile at the picked point.
- Win and lose conditions come from the level's scripts and state machines (scripts.md); [O]: the companion's
  health bar falls when enemies reach him; the level is won when the waves are cleared.

## 5. Vehicle race and chase (worlds 16, 37, 40)

[R] unless marked.

- Vehicles (class 25): front and rear wheel radius 50 by default (options 1080, 1081 override it: 26.25 to 94 seen [D]); top speed 3000 units/s; maximum suspension travel 1300; tyre grip 2.0. Option 281 marks the playable vehicle
  (world 16: one of five; world 37: one of two; world 40: the only one). [D]
- **Race:** 3 laps by default. The track has check volumes in order; passing the first one again completes a lap.
  Race progress = `((lap − 1) × 3 + checks passed) × 0.1`. Places are ranked by lap, then progress. Finishing the
  last lap first wins (race state machine OID 1068: running 1071).
- **Chase (world 40):** both sides collect items along the road; the first to **60** wins (state 1074 win, 1073
  lose).
- Opponents adjust their pace to the player's place: the difficulty updates only while the player is in the top
  three.

## 6. World 25: shooting under a timer

[R] unless marked.

- The player controls a creature (class 24) that swims on water and moves on ground (maximum push speeds 5000 and
  4000 units/s; elasticity 0.2, friction 0.1). It eats food creatures (class 15) and spits fire at targets
  (class 72); each spit uses one eaten creature; a target lights 0.25 s after the shot.
- **Win:** every target lit before the timer ends (counter: lit / total; the total counts the targets in the world).
- **Lose:** the **120 s** timer runs out (option on the creature can change it; not set on the disc [D]).

## 7. Armoured vehicle (worlds 27, 35; parked in 22, 31)

[R] unless marked.

- The player can climb into the vehicle (class 20) and out of it where allowed. Driving speed is
  `800 × deflection²` units/s along the stick direction; the body tilts up to 0.15 rad and leans up to 15 units
  with speed; the turret head turns towards the stick direction and fires (right-stick deflection above 0.7 aims;
  it fires when aimed within 0.5 rad).
- It takes damage from electric contacts (the guards' and hazards' zap). Options on all four: 41 = 60, 42 = 400
  (shadow radius and depth), 709 = 35, 710 = 4 (step settings). [D]
- Goals are scripted per level (destroy targets, reach the end). [O]

## 8. World 29: rhythm duel (boss of episode 3)

[R] unless marked.

- The boss dances to the music; the four rhythm sequences (class 144) are lists of timed button commands
  (up to 192 per sequence; options 1161 and 1162 carry them [D]). Command times are in beats: measure × 4 + beat,
  following the music's bar counter.
- The player must press the shown button within an **input window** centred on the beat:
  window = (1 − u) × 0.5 s + u × 0.8 s, where u (0 to 1) is the difficulty assistance, which grows after failures.
  So the window is 0.5 s normally and widens to 0.8 s.
- A correct press plays a success sound (one of four, per button); a miss plays a failure sound and the boss
  throws the player back (700 units back, 650 up).
- Each completed sequence is one boss hit (4 phases × 1, section 2). Script events 26, 27, 28 report sequence start,
  each command reached, and stop (scripts.md 6). [D] for the event numbers

## 9. World 39: gun platform

[R] unless marked.

- The player operates a gun platform (class 26) with two aiming modes (primary and secondary aim states). It has
  passive, active, zapped and destroyed states; a hit flashes it. Goals are scripted. [O]

## 10. World 42: small remote vehicle

[R] unless marked.

- The player drives a small vehicle (class 29) at up to **1200 units/s** (stick deflection below 0.25 does not move
  it; below 0.1 does not aim). It shoots, and collects **security codes**: the counter shows codes / **13**; a code
  is collected by touching its special contact, which explodes and disappears.
- Its lives are shown on a bar of 4. A hit within the damage radius (150 units) costs one; options 131 = 5,
  132 = 1, 133 = 50 on it. [D]
- **Win:** all 13 codes; **lose:** no lives left.

## 11. World 45: final boss

[R] unless marked.

- The player flies a jet pack (class 27): position and velocity follow the stick with smoothing; aiming covers a
  rectangle 750 × 450 units ahead.
- The final boss (class 74) moves at up to 1250 units/s in its first stage and 750 forward / 750 sideways in its
  second; it predicts the player's position 0.5 s ahead. Hitting its weak point damages it (section 2).
- Its damage part (class 75) needs **16** hits; when they are spent, the ending starts 3 s later
  ([level-flow.md](level-flow.md) 10).

## 12. Test vectors

- Distinctive class counts per world: the table of section 1 (counted through proxies). [D]
- World 9 controller options (two objects): 771 = OID 1488, 772 = OID 1468, 773 = OID 1421, 774 = OID 1460 (one
  object), 776 = 2.25 / 1.0, 777 = 2.5 / 4.5, 779 = 1.0 / 2.25 / 2.0, 780 = 2.5 / 4.5 / 4.75. [D]
- Race vehicles, option 1080 values: 60 (×2), 65, 75, 84 (×3), 26.25; option 1081: 60, 75, 80, 88, 94 (×3), 37.6. [D]

## 13. Open questions

- The win and lose rules of the turret, armoured-vehicle and gun-platform levels live in the levels' scripts; the
  script spec should list them per world.
- World 28's arena rules (spawner, guard variants 17 and 18) belong to guards.md.
- Lap counts per race: the disc sets none, so all races use 3 laps — confirm by play.
- Input-window direction in the rhythm duel (wider when easier) to be confirmed by play.
