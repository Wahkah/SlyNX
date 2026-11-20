---
id: sly1-behaviour-guards
title: Sly 1 guards and enemies - patrol, perception, alert, chase, attack, knock-out
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: >-
      which guard classes occur where and how many are placed (proxy use counts); every option value set on guard
      templates and in proxy overrides; guard-group membership; the state numbers carried by typed option arguments
  - kind: executable
    what: SCUS_971.98, SHA-256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      confirmation of the sight test points, the attack-angle scale, the search and timer ranges at the addresses
      given in section 9 (read as data)
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/stepguard.cpp, smartguard.cpp, smartguard.h, gomer.cpp,
      ub.cpp, mb.cpp, murray.cpp, puffer.cpp, crusher.cpp, ruby.cpp, flash.cpp, spaprops.cpp
    used_for: >-
      the behaviour of guards and guard groups, default values, and which option ID sets which property; read for
      what the game does, re-expressed here; values the disc confirms are tagged [D]
  - kind: observation
    what: general knowledge of the game as played
    used_for: statements tagged [O]
reviewed_by: pending
date: 2026-11-18
---

# Guards and enemies

Tags: **[D]** disc data, **[O]** observable in play, **[R]** reference behaviour to be confirmed, **[I]** inferred.
Class IDs and option numbers are those of [object-stream.md](object-stream.md) and
[option-encodings.md](option-encodings.md). Distances are game units, angles radians unless stated, times seconds.
The player character is OID 5 in every world [D]. "Horizontal" means in the X-Y plane (Z is up).

## 1. Guard classes and where they are

Each guard kind is written once per world as a template and placed many times through proxies
([classes/proxy.md](classes/proxy.md)); the placed count is the template's proxy use count [D].

| Class | Plain name (ours) | Worlds | Placed | Notes |
|---:|---|---|---:|---|
| 9 | flashlight guard | 17 worlds (4, 7, 10, 13–15, 17, 19, 21, 22, 26, 30–34, 36) | 83 | Sees with a light cone: a flash volume, class 96 [D] [R] |
| 10 | proximity guard | 31 worlds | 193 | Sees by distance and facing angle; stays inside a leash area [D] [R] |
| 11 | world-11 enemy | world 11 "The Eye of the Storm" | 1 | Proximity rules with a full circle of view; detects only while a scripted counter is non-zero [D] [R] |
| 12 | world-20 boss | world 20 "Last Call" | 1 | Alerted only while its state machine is at goal OID 1082, 1084 or 1086 [D] [R] |
| 13 | world-38 boss | world 38 "Flame Fu!" | 1 | Alert flag set by scripts [D] [R] |
| 14 | companion character | worlds 15, 16, 34, 37 | 4 | Allied character driven by scripts; not a threat [D] [O] |
| 15 | world-25 enemy | world 25 "Piranha Lake" | 12 | [D] |
| 17, 18 | minigame enemies | world 28 "Down Home Cooking" | 1 each | See minigames.md [D] |
| 19 | world-29 boss | world 29 "A Deadly Dance" | 1 | Rhythm boss; see minigames.md [D] |
| 74, 75 | final boss and its part | world 45 "A Strange Reunion" | 1 each | Part has a hit counter of 16 (option 1148) [D] |
| 137 | guard group | worlds 14, 21, 22, 30 | 5 groups | Lists member guards by OID (option 694; 21 links on the disc) [D] |
| 138 | path zone | 27 worlds | 98 | Walkable navigation area for chasing and searching [D] |

Sections 2 to 7 describe classes 9 and 10, which make up 276 of the 302 placed enemies. Section 8 covers the rest.

## 2. Guard groups and the alert level

Every guard belongs to exactly one **group**: the class-137 record that names it, or else a group of its own made
at load [R]. Guards in a group share one alert level [R]:

| Group level | Meaning |
|---|---|
| patrol | nobody has seen the player |
| attack | at least one member sees the player now |
| search | the player was seen and then lost |
| dead | the group was removed (every member dead) |

Rules, re-evaluated whenever a member asks (at least once per frame while members are active) [R]:

- **patrol → attack** when any member detects the player (sections 4, 5). If every member is a flashlight guard,
  sound 63 plays (audio.md: start distance 3000, full volume 300) [R].
- **attack → search** when no member detects the player.
- **search → attack** when any member detects the player again.
- **search → patrol** when the search is abandoned: every member reports "done searching", or any member reports
  "give up now" (5.4).
- On entering **attack**: the music intensity request 107 is raised (audio.md 5.4), every alarm linked to the group
  is triggered (alarms-sensors.md), and each member is given a time to stop patrolling: now if it sees the player
  itself, otherwise after a random 0 to 1.5 s [R] [D for the range, section 9].
- On leaving **attack**: the intensity request is removed and each member gets a "lost the player" delay, random 0 to
  0.3 s [R] [D].
- On entering **search**: members are sent to search points (5.3). On leaving it, each member gets a give-up delay,
  random 0 to 4 s [R] [D].
- On entering **patrol**: linked alarms return to armed. On entering **dead**: linked alarms are disabled [R].

The last position where a member saw the player is remembered per group [R].

## 3. One guard's states

A guard is always in one of these states. The numbers are data: scripts and options name states by them (options
662, 663, 664, 703; for example option 664 binds physics to states 11 and 16, option 703 binds a flashlight target
to state 6 or 2) [D].

| No. | State (our name) | What the guard does |
|---:|---|---|
| 0 | decide | transient: picks the next state from the group level |
| 1 | patrol | follows its patrol animation (root motion drives position and heading) |
| 2 | stand at post | no patrol animation, or patrol paused: turns back to its original heading and idles |
| 3 | at waypoint | patrol reached a waypoint object (class 98); waits there until the waypoint releases it |
| 4 | return to patrol | walks back to its patrol route (to the closest point, or the point it left, option 676) |
| 5 | go to search point | walks to its assigned search point |
| 6 | search on the spot | at the search point, faces the given direction and plays its look-around animation |
| 7 | spot | noticed the player: plays the "spotted" animation, then chases |
| 8 | lost the player | plays a confused animation, then decides again |
| 9 | give up search | plays a shrug animation, then returns to patrol |
| 10 | taunt | after hurting the player: plays a taunt, then decides again |
| 11 | dying | knocked out; stays down |
| 12 | chase | runs toward the player along a path (section 6) |
| 13 | wait while chasing | close enough or no path; faces the player |
| 14 | attack | plays an attack animation |
| 15 | (unused by the generic rules) | — |
| 16 | stunned | took a non-fatal hit; plays a stun animation, then decides again |

Each state may have its own animation (option 656 and the call option 657 bind an animation segment, class 131,
to a state number; 192 bindings on class 9 templates and 367 on class 10 templates [D]). A state with no animation
bound is skipped where the table below says so.

**Transitions** [R] (checked every frame; "anim done" means the state's animation reached its end):

| From | Group level | Goes to |
|---|---|---|
| 0 decide | patrol / search / attack | 4 / 5 / 7 |
| 1 patrol | patrol | 2 if the patrol animation ended or is paused |
| 2 stand at post | patrol | 1 if a patrol animation exists and runs |
| 1, 2, 3, 4 | search | 5, once the stop-patrol time has passed |
| 1, 2, 3, 4 | attack | 7, once the stop-patrol time has passed |
| 4 return | patrol | 1 on reaching the route |
| 5 go search | patrol / search / attack | 9 / 6 on arrival / 7 |
| 6 search spot | patrol | 9 after the give-up delay |
| 6 search spot | search | 5 if its path became invalid |
| 6 search spot | attack | 7 |
| 7 spot | any | 12 when anim done |
| 8, 10, 16 | any | 0 when anim done |
| 9 give up | patrol / search / attack | 4 when anim done / 5 / 7 after stop-patrol time |
| 12 chase | patrol or search | 8 after the lost-the-player delay |
| 12 chase | attack | 13 when anim done; 14 if an attack is possible (6.2); 13 on reaching the goal if a wait animation exists |
| 13 wait | patrol or search | 8 after the lost-the-player delay |
| 13 wait | attack | 14 if an attack is possible; 12 if the path became invalid |
| 14 attack | patrol or search | 8 after the lost-the-player delay |
| 14 attack | attack | 12 when anim done |
| 11 dying | — | stays |

Before these rules, from any state except 11 and 16: if a taunt animation is bound and the taunt time has come, go
to 10 [R].

## 4. Perception: flashlight guards (class 9)

### 4.1 The light cone

A flashlight guard owns a **flash volume** (class 96; a convex volume, collision.md 5) named by option 701; the
volume is attached to the guard and aimed by the game [D] [R]. 65 flash volumes exist on the disc, all in the 17
worlds with flashlight guards [D]. The volume is scaled smoothly toward a target scale [R]:

| Group level | Target scale | Aimed at |
|---|---|---|
| patrol | 1.0 | a point 1000 units straight ahead of the guard, or a per-state target (option 703) |
| search | 1.3 | as patrol |
| attack | 1.3 | the player (unless the guard's look is locked, option 669) |
| dead | 0.1 | ahead |

Scaling follows a smoothing with fast rate 1.0, slow rate 0.2 and 0.1 s ramp [R]. A point is **in the light** when,
after dividing its position in the volume's local space by the current scale, it is inside the convex volume [R].

### 4.2 Detection test

Each check, in order [R]:

1. Not while the game has taken control away from the player (cutscenes), not with option 705 set ("never
   detect"), never while the guard is dying (11) or stunned (16).
2. Option 704 set: always detects.
3. The player's test position is the player's base contact point (or 70 units below its origin when airborne).
4. **Close range or light**: the player is detected only if it is inside the light, or within the near range —
   horizontal distance ≤ near distance (option 696, default 300) **and** height difference ≤ near height
   (option 697, default 300).
5. **Hiding**: unless detection flag bit 0 (option 706) is set or the group is already at attack level, a hidden
   player is not seen (4.3).
6. Detection flag bit 1 (option 706): detected without a line-of-sight test.
7. **Line of sight**: from the guard's head (or its origin) to four test points on the player, in the player's
   local frame; detected if **any** of the four segments is clear of solid objects (ignoring the player, decoys,
   and up to 4 barrier objects listed by option 708) [R]:

   | Group level | Test points (x, y, z) |
   |---|---|
   | patrol or search | (0, 0, −20), (0, 0, 70), (0, −30, 30), (0, 30, 30) |
   | attack | (0, 0, −80), (0, 0, 100), (0, −70, 40), (0, 70, 40) |

   [D] Both sets are in the executable (section 9).

Values on the disc [D]: near distance (696) 230 on 5 templates and 600 on 1; in proxy overrides 200, 220, 130, 0,
400, 230, 250; near height (697) 200 in 7 overrides; options 704 and 705 set on 7 placed guards each (scripted
guards).

### 4.3 When the player counts as hidden

[R], used by both guard kinds and the searchlights:

- Flattened against a wall: hidden from a guard on the wall side of the player (the guard is behind the player's
  facing direction), unless the hiding spot is marked detectable.
- Ducking in cover: hidden from a guard in front of the player, unless the spot is marked detectable.
- In a hiding barrel (or similar) while standing still: hidden.
- A fourth hiding mode counts as hidden only while a particular vault power-up is owned (power-up mask
  `0x12000`).

### 4.4 Sneaking music

While the player is within the sneak area of a flashlight guard — a volume named by option 700, or else horizontal
distance < 2000 (option 698) and height difference < 300 (option 699) — the guard raises the music intensity
request −20 (quiet sneaking music), and removes it when the player leaves or the guard dies [R].

## 5. Perception: proximity guards (class 10)

### 5.1 Leash

A proximity guard gives up on the player outside its **leash** [R]:

- a volume (option 843, an OID; set in 34 proxy overrides [D]), or else
- distance from the guard's starting position > abandon distance (option 841, default 2000) or height difference
  > abandon height (option 842, default 300).

Disc values [D]: 841 = 4200, 1600, 1500, 2000, 3000, 2200 on templates; 20000 (26) and 80000 (9) in overrides;
842 = 400 or 500 on templates; 4000 (26) and 80000 (9) in overrides. A player in the "grabbed/held" state is never
outside the leash [R].

### 5.2 Detection

Once detected, the guard stays alerted (latched) until the player leaves the leash [R]. Otherwise [R]:

1. Inside the detect volume (option 847, an OID; set in 17 proxy overrides [D]) counts as seen, skipping 2.
2. Else with distance `d`, detection range `(dmin, dmax)` (option 845, default (1000, 1500)) and view-angle range
   `(amin, amax)` (option 846, default 45° to 180°):
   - `d ≥ dmax`: not seen;
   - `d < dmin`: seen if the player is within `amax` of the guard's heading;
   - between: the allowed angle shrinks linearly from `amax` at `dmin` to `amin` at `dmax`.
3. A hidden player (4.3) is not seen.

Disc values of option 845 [D]: on templates (1200, 1500) 12, (1300, 1500) 9, (2000, 2100) 8, (4000, 4000) 5,
(1500, 1600) 4, (1300, 1400) 4; in overrides (1, 1) on 26 placed guards (effectively blind until scripted),
(700, 800) 10, (500, 600) 7, (1100, 1200) 6.

Proximity guards have no line-of-sight test [R].

### 5.3 Search points

On entering search, the group's guards are given points [R]:

- First, search markers listed for the group (option 695; none on the disc [D]): each guard in turn takes the
  unused marker nearest the last seen position, and faces the marker's X axis.
- Remaining guards are spread on a circle around the last seen position: with `n` members, angles step by `2π/n`
  starting from the direction of the player's last velocity, each jittered by a random fraction −0.2 to +0.2 of
  the step; radius `n × 700 / (2π)` times a random 0.8 to 1.2 [R] [D for 700, the jitter ranges].

### 5.4 Ending a search

A flashlight guard searching on the spot reports "done" after it has looked around at least once and 5 s have
passed in that state; dying guards report "still busy". Proximity guards report "still busy" by default [R].

## 6. Chasing and attacking

### 6.1 Moving

- Guards move by root motion of their state animations (animation.md 4.2) and steer toward a goal [R].
- With a path zone (option 652; 132 overrides set it [D]) the guard plans a route of up to 16 points through the
  zone's walkable triangles (classes/paths-and-maps.md) to the goal; without one it heads straight [R].
- Gravity scale: option 711 (default 1.5; 2.0 to 8.0 on the disc [D]). Turn rate scale: option 710 (0.4 to 2.0
  [D]). The turn rate itself: physics.md 5.1 ("Turning"): at most 25·r rad/s, accelerating at up to 100·r^1.5
  rad/s², r = option 710 (default 1) [X 0x274B80] [R]. Contact friction: option 691 (default 1.25; 2.0 to 5.0 on the disc [D]).
- Option 709 lifts the guard's contact point (0 to 80 [D]).

### 6.2 Attacks

A guard may attack when all hold [R]:

- the player is not in a state that cannot be hurt (hidden, already being hurt);
- the distance `d` is inside the attack range (option 653, default 0 to 300);
- the angle `a` between the guard's heading and the player, scaled by `min(1, 0.005 × d)`, is at most the maximum
  attack angle (option 655 in **degrees**; default 10°) [D for 0.005, section 9];
- the attack cooldown has passed: after every attack a new cooldown is drawn from the attack interval (option 654,
  default 0.2 to 0.5 s);
- a flashlight guard must also detect the player at that moment (4.2);
- a guard with a ranged weapon (option 682) must have its ammunition loaded.

Disc values [D]: attack range (653) on templates (0, 10000) 14, (0, 1500) 6, (0, 500) 5, (100, 700) 5, (0, 2000) 3, (0, 700) 3, (0, 350) 3,
(1700, 2000) 3; in overrides (0, 80000) 12, (0, 1100) 7, (0, 300) 6. Attack interval (654) (1.0, 1.5) on 6
flashlight templates, (0, 0) on most others, (4, 4) and (10, 10) in overrides. Attack angle (655) 7° (12), 180° (6),
20°, 360° [D].

What an attack does to the player: option 661 is a method with two arguments, (effect object, damage kind). Each
occurrence adds one attack effect to the guard; a guard keeps at most 4 [R]. The kinds are the player's damage
kinds ([player-moves.md](player-moves.md) 7), and the disc uses **0 blunt, 2 fire, 4 crush and 6 thrown** [D for
the values, R for the names]. When the effect object touches the player [R]:

- **0 blunt**, or **6 thrown** from a guard that is not flying: the player is thrown. The velocity is the effect
  object's motion at the contact point × 1.25 (elasticity 0.25 + 1), shortened to at most 1,200 units/s, with its
  upward part raised to at least 600 units/s.
- **6 thrown** from a flying guard: the throw reaction with zero velocity.
- **Which guards are flying**: it is a per-template flag, not a class. Option **673** (a byte) sets it [R for the
  meaning]. On the disc only six **class-10** (proximity guard) templates set it, all to 1, all in Vicious Voodoo
  worlds [D]: OID 1612 in world 21 (The Dread Swamp Path), 1993 in 23 (The Lair of the Beast), 1601 and 1729 in 24
  (A Grave Undertaking), 1772 in 27 (A Ghastly Voyage) and 1429 in 28 (Down Home Cooking). Every other guard,
  including all flashlight guards, is not flying. The same flag also gives flying guards a tilted drive while moving
  [R].
- **2 fire**, **4 crush**: the damage of that kind with no throw (crush sets the player's velocity to (0, 0, −1,000),
  player-moves.md 7).
- The effect objects are removed after the hit (one shot).
- Non-player objects that touch the effect are pushed by its motion instead.

The constants 0.25, 600 and 1,200 are from the reference; they are not stored as data words in the executable
(probably immediate values in code).

### 6.3 Taunting

When a guard's group hurts the player, guards that are chasing, waiting or attacking schedule a taunt [R]: the
nearest guard after the minimum of the taunt range (option 689, default 0 to 0.8 s; (0, 0) on the disc [D]), the
others staggered across the range by distance rank.

## 7. Being hit

- A hit from the player's attacks (cane, bomb, crush) knocks the guard out [R]. A guard with a remaining **stun
  count** (option 665) is stunned instead and the count drops by one; the default count is 0, so one hit knocks a
  guard out [R]. Disc values [D]: 3 (world-11 enemy, world-29 boss), 4 (world-38 boss), 80000 (world-20 boss,
  effectively unkillable by hits), 2 and 3 in overrides.
- Option 692 makes a guard ignore all damage (2 placed guards [D]).
- The guard turns toward where the hit came from, scaled by the fling factor (option 684: 0 to 0.8 on the disc,
  default 1.0) [R] [D].
- A death animation may be chosen per kind of hit (option 662). An explosion object can fire on the hit (option
  681) [R].
- **After death**: the guard drops coins (option 678 count; −1 default lets the difficulty system decide; 0 on all 23
  records that set it [D]) at an offset (option 679) [R]; it stays dead after the player respawns at a checkpoint
  [R]. Automatic fading of the body (option 685) is set **off on every template that sets it** (classes 9 to 13, 15, 19) [D]; when on, the
  body fades after its death animation plus option 659 (2.0 s where set [D]).
- Its patrol waypoints are released so other guards can use them [R].

## 8. Other enemies

| Class | What differs [R] |
|---|---|
| 11 | Proximity rules with view angle 0 to 360° and abandon height 30000; detects only while its jump counter is non-zero (set by scripts) |
| 12 | Detects only while its state machine's current goal is OID 1082, 1084 or 1086; stun count 80000 [D] |
| 13 | Detection is a flag set by scripts; stun count 4 [D] |
| 14 | Never fades, no fling; follows scripts as an ally |
| 15 | Twelve placed in world 25 [D]; buoyancy 2 and viscosity 3 (options 134, 135 [D]) |
| 17, 18 | Part of the world-28 minigame: a spawner (class 104) keeps a kill goal of 50 within 90 s, respawning up to 2 at a time [R]; see minigames.md |
| 19 | Rhythm boss: beats of 60 frames, 8 beats per measure, measures of 8 s [R]; see minigames.md |
| 74, 75 | Final boss; its part takes 16 hits (option 1148 [D]) |

## 9. Constants in the executable

All in SCUS_971.98 (SHA-256 above); file offset = virtual address − 0xFF000. Check these values before use [D]:

| Virtual address | Contents |
|---|---|
| `0x00274620` | Attack-level sight test points, 4 × (x, y, z, pad): (0, 0, −80), (0, 0, 100), (0, −70, 40), (0, 70, 40) |
| `0x00274660` | Patrol-level sight test points: (0, 0, −20), (0, 0, 70), (0, −30, 30), (0, 30, 30) |
| `0x002746A0` | Flashlight aim point ahead of the guard: (1000, 0, 0) |
| `0x00274CC0` | Attack-angle distance scale as a quadratic (0, 0.005, 0) |
| `0x00274D08` | Give-up delay range (0, 4) |
| `0x00274D10` | Stop-patrol ("spot") delay range (0, 1.5) |
| `0x00274D18` | Lost-the-player delay range (0, 0.3) |
| `0x00274D20` | Search circle spacing 700 |
| `0x00274D28` | Search angle jitter (−0.2, 0.2) |
| `0x00274D30` | Search radius jitter (0.8, 1.2) |

Defaults marked [R] elsewhere (300, 2000, 1000 to 1500, 45° to 180°, 10°, 0 to 300, 0.2 to 0.5 s) were not found
as data and are probably immediate values in code.

## 10. Test vectors

- Placed counts per world, flashlight guards (class 9) [D]: world 4: 3, 7: 3, 10: 2, 13: 4, 14: 9, 15: 6, 17: 2,
  19: 3, 21: 7, 22: 6, 26: 5, 30: 11, 31: 5, 32: 4, 33: 5, 34: 7, 36: 1 (total 83).
- Proximity guards (class 10) [D]: world 3: 5, 4: 1, 5: 2, 6: 11, 7: 1, 8: 6, 10: 5, 12: 13, 13: 4, 14: 1, 15: 14,
  17: 2, 18: 8, 19: 5, 21: 13, 22: 1, 23: 17, 24: 14, 26: 4, 27: 7, 28: 1, 30: 7, 31: 1, 32: 7, 33: 6, 34: 13,
  35: 8, 36: 2, 40: 4, 42: 1, 43: 9 (total 193).
- World 4 "Prowling the Grounds", flashlight-guard template OID 1555 [D]: options 281 = 1, 659 = 2.0, 110 = 41,
  91 = 14, 703 = (state 6, target OID 1580), 701 = 1594 (its flash volume), 41 = 125, 42 = 300, 653 = (0, 10000),
  710 = 0.6, 684 = 0.5, 681 = 1565, 685 = 0.
- World 3 "A Stealthy Approach", proximity-guard template OID 1520 [D]: 845 = (1200, 1500), 684 = 0.3, 681 = 1525,
  41 = 200, 42 = 300; a second copy has 653 = (1700, 2000), 684 = 0.4.
- World 14 "Boneyard Casino" guard groups [D]: OID 1578 lists guards 2178, 2177, 2176, 2174; OID 1579 lists 2056,
  2052.

## 11. Open questions

- Chase and patrol speeds come from animations; per-animation speeds belong to animation.md and player-moves.md.
- The meaning of options 41, 42 (shadow radius and depth), 91 and 110 (what the player's attacks may do to the
  object) is for options-by-class.md.
- Whether a guard alone in its group ever triggers alarms without option 291 (unused on the disc [D]); scripts may
  link them.
- (Settled: option 661's damage kinds, section 6.2; the throw constants are [R].)
- All [R] rules need checking against play, especially the detection ranges and the hidden rules.
