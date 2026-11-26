---
id: sly2-behaviour-guards
title: Sly 2 guards and swarming creatures - placement, perception, alert, chase, attack, pockets, knock-out
side: reference
status: draft
sources:
  - kind: executable
    what: SCUS_973.16 (SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73), option-table entries 47, 362, 364, 365 at 0x2AC880 + 40 × ID and their routines; the knock-back routine at 0x1A16C8 (hit-kind tests at 0x1A170C and 0x1A1714) and its floats at 0x2BFACC to 0x2BFAD4 (2026-11-18)
    used_for: what option 364 stores; contact kind 0x8000
  - kind: disc
    what: all 43 worlds (2026-11-18), every option-364 value on class-120 objects and every clip it names (length, events); the scripts of rows 28 and 37 that read option 255 (script ID 381); Jean Bison's module and the sawmill boss job's module
    used_for: the action-to-clip table, Bison's fight rules
  - kind: reference-project
    what: sly2-decomp src/new_100008 (routines 0x104408, 0x1044B0), src/rwm-steprun (0x20AF38, 0x20AF88), src/step-rwm (animation-reference bind, start and done routines), src/aseg (label search); SafeHouse Sly2-Release damage.cpp, damage.h, pcguard.cpp
    used_for: the animation-reference rule and the 0x8000 reaction, checked against the executable where noted
  - kind: disc
    what: Sly 2 - Band of Thieves (USA) SCUS-97316, image 2,884,141,056 bytes; all 43 decompressed worlds
    used_for: >-
      where guard and swarmer templates sit (packages and their names), how many are placed per world (placements
      and spawner points), every option value on the class-57 and class-58 templates, the action objects (class 120)
      and noise templates (class 117) of two worlds, the vision volumes' extents
  - kind: executable
    what: SCUS_973.16, SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73
    used_for: confirmation of the constants and the pocket tables at the virtual addresses in section 10 (read as data)
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/guard.h, guardstart.cpp, detect.cpp, detect.h, ainoise.cpp,
      ainoise.h, aipursuit.cpp, aipursuit.h, aihit.cpp, aihit.h, pocket.cpp, pocket.h
    used_for: what the guards do, defaults, and which option sets which property; re-expressed here; marked [R]
  - kind: reference-project
    what: Sly2-RE-notes.md (spawns section) and tools/sly2_spawns.py, re-run on the disc for section 1
    used_for: the count of guards and swarmers each world creates at start
  - kind: disc
    what: SCUS-97316, all 43 worlds re-walked 2026-11-18 (every collision part of classes 56-78 and their nested solids; every clip's part-switch events 15/16 in the guard, swarmer and boss packages; the characters' lookup slots resolved in the nightclub hub)
    used_for: melee contact kinds and values, hit windows, boss attack parts, clip per slot [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release (alo.cpp collision-part loader, so.cpp contact dispatch, asegevent.cpp events 13-16, aihit.cpp, guardstart.cpp boss options, sly.cpp and gang.cpp clip slots)
    used_for: what the collision-part fields mean, how hits are delivered, which move uses which slot [R]
  - kind: executable
    what: SCUS_973.16 (SHA-256 a511a94b…6a73), noise template table (709: 2,500, 0.35 s), clone-pool lease test 0x001CA8F4 (2026-11-21)
    used_for: section 4.2, noises from idle projectiles [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/ainoise.cpp (own-noise test), projectile.cpp (update, contacts, end)
    used_for: section 4.2, which noise is "its own" and when a projectile makes noise [R]
reviewed_by: pending
date: 2026-11-21
---

# Sly 2 guards and swarming creatures

Delta of [../sly1/guards.md](../sly1/guards.md). Sly 1's guards were two hand-written state machines (flashlight
and proximity guards). In Sly 2 every guard, swarming creature, boss and story character of classes 56 to 68 runs
one shared behaviour engine: a **stack of jobs** (patrol, investigate, pursue, act, hit, …) driven by a
**detector** (sight), **noises** (hearing and alarms) and **action objects** (attacks). Only the job rules and
numbers that matter for guards and swarmers are given here; bosses belong to the boss specs. Notation and tags as
in [gameplay-overview.md](gameplay-overview.md). Option numbers are stream option IDs
([object-stream.md](object-stream.md)).

## 1. Classes and where they are

| Class | Role | Templates [D] | Name strings of the templates (game data) [D] |
|---:|---|---|---|
| 58 | guard | 28, one per world in 28 worlds | `guard` (package names per episode: `npc_pig_guard`, `npc_boar_guard`, `npc_rhino_guard`, `npc_vulture_guard`, `npc_moose_guard`, `npc_pelican_guard`) |
| 57 | swarming creature and pop-up enemy | 66 in 34 worlds | `npc_swarmer_rat`, `npc_swarmer_ratso`, `npc_swarmer_monkey`, `npc_swarmer_goat`, `npc_swarmer_wolf`, `npc_swarmer_goose`, `npc_swarmer_pelican`, `npc_miniswarmer_spider`, `npc_miniswarmer_chopper`, `npc_surprise_cobra`, `npc_surprise_tentacle`, `npc_popup_mole`, `small_swarmer` |
| 59 to 68 | bosses and story characters (same engine) | | see [object-stream.md](object-stream.md) 4 |

The species is the package, not the class: every guard is class 58 with the same options (section 9); the pig,
boar, rhino, vulture, moose and pelican guards differ only in their meshes, animations and the few options marked
in section 9 [D].

Templates sit in no-clone packages ("the level's cast", one per job); the world creates the live guards by copy
placements, fresh placements and, in hubs, spawners (32 spawners, all in hubs) [D] [R]
([classes/placement.md](classes/placement.md)). Guards placed at start, per world [D] (re-counted with the spawn
walk; hubs count spawner points up to their slot limits):

| Row | World | Guards (species) | Swarmers and pop-ups |
|---:|---|---:|---|
| 0 | `jb_intro` | 7 pig | 0 |
| 2 | `f_nightclub_exterior` | 1 boar | 0 |
| 3 | `f_nightclub_cellar` | 4 boar | 8 rats |
| 4 | `f_nightclub_disco` | 4 boar | 4 rats (`ratso`) |
| 6 | `f_nightclub_moulinrouge` | 6 boar | 0 |
| 7 | `f_nightclub_waterpump` | 1 boar | 1 rat |
| 8 | `i_palace_ext` | 12 rhino | 13 cobras, 7 goats, 7 monkeys |
| 9 | `i_palace_int` | 8 rhino | 14 cobras, 4 goats, 3 monkeys |
| 10 | `i_palace_basement` | 2 rhino | 2 cobras, 2 goats, 4 monkeys |
| 11 | `i_palace_ballroom` | 2 rhino | 0 |
| 12 | `i_temple_ext` | 19 rhino | 7 cobras, 13 goats, 14 monkeys |
| 13 | `i_temple_int` | 8 rhino | 9 cobras, 4 monkeys |
| 14 | `p_prison_ext` | 18 vulture | 12 goats, 13 wolves |
| 15 | `p_prison_int` | 1 vulture | 3 wolves |
| 17 | `p_castle_ext` | 11 vulture | 8 goats, 8 wolves |
| 19 to 26 | castle jobs | 0 or 2 vulture | 2 to 10 wolves or tentacles |
| 27 | `c_train_ext` | 19 moose | 6 goats, 7 geese |
| 28 to 31 | train jobs | 4, 7, 8, 3 moose | 2 to 7 goats or geese |
| 32 | `c_sawmill_ext` | 20 moose | 12 goats, 13 geese |
| 33, 34 | `c_sawmill_mulch`, `c_sawmill_burn` | 12, 4 moose | 0 |
| 38 | `a_blimp_ext` | 22 pelican | 21 pelicans |
| 39 | `a_blimp_int` | 4 pelican | 0 |

All other worlds start with no guards. More are created later by scripts, spawners and job starts [R].

## 2. The behaviour engine (what replaces Sly 1 sections 2 and 3)

Each guard has a **stack of jobs**; the top job runs each frame and may push a child job [R]. The jobs a guard uses:

| Job (our name) | What it does |
|---|---|
| patrol | follows its route (option 273: a command list; the regions it may enter, option 301), with stand and run times drawn from option 274 / 275 ranges [R] |
| investigate | walks to a noise it accepted (section 4), looks, searches, then gives up |
| pursue | chases a seen target, surrounds it, picks attacks and shots (section 6) |
| act | plays one attack (an action object) at the target |
| shoot | plays the guard's gun animation at the target |
| hit / knocked out | reactions to being hit (section 7) |
| held | carried and thrown by Murray ([characters.md](characters.md)) |
| wait while paused | stands still while the game pauses the guards (briefings, menus) |

A **state mask** of up to 64 numbered flags tells which animation set fits: each job sets its flags, and the guard
plays the animation entry (options 243, 244, 245, 246 to 248) whose flag set best covers the mask [R] [D for the
options: 683 entries of option 243 on the 28 guard templates]. Animations move the guard by their root motion
([animation.md](animation.md)).

A guard's **home** object (option 327) bounds how far it wanders when investigating (section 4.2) [R].

## 3. Sight: the detector

Each guard owns a **detector** that watches up to 8 objects (the players are added when the guard resets) and keeps
for each one a state: **0 unseen, 1 seen, 2 lost** [R].

- **Eye**: 80 units above the guard's position [R] (unit vector (0, 0, 1) at `0x2A2DC0` [D], scale 80 [R]).
- **View volume**: an object of class 22 named by option 315, a closed mesh in the guard's rig; a second volume
  (option 316) can further limit it [R] [D]. A target is inside when its point lies inside the mesh (the volume's
  collision tree). The 27 guard templates that set option 315 all name the same rig object; its mesh (30 positions)
  spans, in the volume's own frame, x −116 … 934, y −661 … 661, z −341 … 315, and the volume sits 166 units ahead of
  its parent [D] (world rows 3 and 8). In practice: a wedge reaching about 1,100 units ahead, about 1,320 wide and
  about 650 tall.
- **Seen** (state 1) when all of [R]:
  1. the target is in the world, not knocked out, and not flagged unseen by a script (an actor option);
  2. no smoke cloud volume (class 23 in the world) cuts the eye-to-target line;
  3. it is inside the view volume (and the second volume, when set);
  4. it is not hidden (3.1), unless option 312 bit 0 is set;
  5. nothing solid lies on the line from the eye to the target, unless option 312 bit 1 is set. Sensors, objects
     marked see-through and objects under the detector's own root never block; with option 312 bit 2 only level
     geometry blocks.
  Touching the guard's body also counts as seen [R].
- **Lost** (state 2): the target was seen and is now outside the view volume (but still inside the second volume, if
  any) [R]. Leaving the second volume resets to 0.
- Option 313 makes the detector always see; option 314 makes it never see [R]. On the disc: 314 = 1 on the
  `jb_intro` guard template only; 312 = 0 on rat and pelican swarmers [D].
- A change of state is passed to the guard; a change to "seen" also tells the jobs and tasks (missions.md) [R].

### 3.1 When the player counts as hidden

[R] By the player's current interaction kind: hidden while hanging from certain spots, always while inside a hiding
object, while performing two particular hidden moves; and for wall-hugging and cover, hidden when the player faces
away from the detector (the side flag of the interaction can flip this). Interactions on paths flagged "visible"
and on surfaces flagged "visible" never hide. Exact interaction kinds belong to [characters.md](characters.md).

## 4. Hearing and alarms: noises

A **noise** is a short-lived point of interest: a position, an owner, a kind, and a template that says how far it
carries [R].

### 4.1 Templates

Noise templates are class-117 objects (options 1090 perception: 0 heard within a radius, 1 seen through the view
volume; 1091 radius; 1092 height range; 1093 lifetime in seconds, −1 for permanent; 1094 kind) [D] [R]. Defaults
when an option is absent: radius 1,500, height range −200 … 500, lifetime 0.25 s, kind 0, heard [D] (`0x2BDCD0`,
`0x2BDCD8`; lifetime [R]).

Seven fixed templates exist in every world, by object ID [D] (overrides at `0x2BDD00`):

| Object ID | Radius | Lifetime | Other | Used for [R] |
|---:|---:|---:|---|---|
| 706 | 1,500 | 0.25 s | | ordinary noises: breaking objects, pickups, hiding, Murray's moves, a guard landing |
| 707 | 1,500 | permanent | seen, not heard | a visible thing a guard should look at (a dropped item, an emptied pedestal) |
| 708 | 750 | 0.25 s | | |
| 709 | 2,500 | 0.35 s | | thrown objects and projectiles landing (kind 1) |
| 710 | 4,000 | 0.5 s | | a guard's alarm about its target (kind 4) |
| 711 | 1,500 | permanent | kind 3 | |
| 712 | 1,000 | 0.25 s | | damage effects |

Worlds add their own templates (world row 3 has, for example, radius 300 / 0.1 s, radius 3,000 / 2, 3 and 5 s, and
radius 4,000 / 1 s kind 4) [D].

**Reach.** A heard noise reaches a point when its height difference is inside the height range and its horizontal
distance is within the noise's radius, which grows over the first min(lifetime, 1 s) of its life:
`radius × (0.15 + 1.1·u − 0.25·u²)`, `u` = age / min(lifetime, 1), capped at 1 (so it starts at 15 % and ends at
100 %) [D] (coefficients at `0x2BDCF0`, cap at `0x2BDCE0`) [R for the rule].

### 4.2 Which noise a guard takes, and investigating

A guard considers a noise only while it is free (not already hit, held, pursuing or in a few scripted states), when
the noise's kind is at least the guard's minimum (option 323, default −1), when it did not make the noise itself,
and, for kind-4 alarms, when the alarm's target is something its own detector watches [R]. Among acceptable noises
it prefers: seen over heard, then higher kind, then newer, then nearer [R]. One named character hears sonic darts at
double range [R].

Investigating [R]:
1. Alerted pose, then walks toward the noise (for an alarm about the guard itself, to where it was raised; else to
   where the alarm points).
2. Looks for a time drawn between 5.5 and 6.5 s, then gives up.
3. Swarming creatures (class 57) instead start searching around after 2 s (`0x2BE5A0` = 2.0 [D]): four waypoints
   300–400 units from a point 350 behind the noise, chosen on a turning fan.
4. A guard with a home object (option 327) and a walking gait only goes if the noise is no more than 50 units
   farther than its home.

**"Not its own" and projectiles** (added 2026-11-21) [R unless marked]:

- **What counts as its own.** A guard skips a noise only when the noise's owner is the guard's own root object. A
  noise owned by anything else is taken, even an object of the guard's own package or copy.
- **A projectile's noise.** A projectile (classes 79 to 82) makes a noise only when it **ends**, as one of its final
  actions. That noise is the fixed template 709 (kind 1, radius 2,500, 0.35 s [D]), placed at the impact and owned by
  the thrower: the projectile's launcher, or the launcher's parent for a class-112-derived launcher, or else the
  projectile itself. Unless the guard itself threw it, the guard will investigate.
- **When a projectile ends:**
  - its end time passes;
  - its speed falls below option 392 (+0x158, 0 by default, so never at rest);
  - a contact is accepted;
  - for kind 0 (option 385), any contact at all.

  A copy freshly taken from the pool has no end time and its contacts switched on. So an **idle** projectile left in
  the world makes no noise until something touches it, but a kind-0 one touching the ground, or the guard holding it,
  ends at once and makes the noise.
- **In the game, a character's bullet is not in the world until fired.** Its placement in the character's package is
  fixed (option 1178 = 1, classes/placement.md 2.1 [D]), and its stored template is a pool object. An engine that
  puts the template, or a copy, in the world at load gets exactly the problem of a kind-1 noise beside the character,
  who then investigates and drops his command list [I].

### 4.3 Alarms

While pursuing a target, a guard raises an **alarm noise** about it at its own position (kind 4, template 710 unless
the guard names another) at most once per second (`0x2C3DB8` = 1.0 [D]); a smoke bomb can hold all alarms back until
a deadline [R]. Other guards whose detectors watch the same target take the alarm and come (4.2).

## 5. Patrol

- Route: option 273 names a command list (curves, waits, go-tos) [R]. Stand times (option 275) and run times (option
  274) are drawn from their ranges; a guard whose wait began while the game paused for a briefing returns to its
  patrol at once [R].
- On the disc, patrol data come mostly from the placement blocks of fresh placements (route 273, region 301, carried
  keys 308 and 309) ([classes/placement.md](classes/placement.md)) [D].
- **Following a curve does not set the height** (added 2026-11-26) [R]. Command 601's path step (job `0xC0`) takes
  the nearest point of the curve, then steps a fixed distance along it (forward or back, clamped or wrapped). Each
  step is handed to an ordinary "go to" job as a target. The guard or creature walks there with its own step physics:
  gravity, and a floor found by its probe points (physics.md).
  - Its height comes from the floor under it, not from the curve's z.
  - Over a void it has no floor, so it falls.
  - Swarming creatures (class 57) are guards (class 56) and move the same way.
  - A creature that keeps the curve's height and walks over voids is missing the step physics, not a curve rule.

## 6. Pursuit, attacks and shots

When the detector reports the player seen, the guard starts pursuing [R]. All guards pursuing one target share a
**target record**: when it was last confirmed and by what (seen, lost but near, attacking, shooting, hit) [R].

- **Lost**: a target record not refreshed for **5 s** ends the pursuit; the guard plays its confused animation and
  returns to patrol [D] (`0x2C3DB4` = 5.0) [R for the rule].
- **Distances** (horizontal, between guard and target, both in the same navigation region): closer than the **near
  range** (option 292) → surround and attack; farther than the **far range** (option 293) or in another region →
  chase along the navigation network, re-planning when the target moves more than 500 units (`0x2BE614`) [D] [R].
  Defaults 800 / 1,200 [R]; **the 27 non-tutorial guard templates set 700 / 1,000** [D]. Swarming goats use 350 / 400
  and choppers 50 / 100 [D].
- **Turn-taking**: unless option 320 is set, only one guard of a target strikes at a time; unless option 321 is set,
  a guard attacks only while on screen (inside the camera's view) and within an angle of the target's reference
  heading [R]. The 27 non-tutorial guard templates set both 320 and 321 to 1 [D].

### 6.1 Attacks: action objects

Each attack is an **action object** (class 120; option 233 of the guard lists up to 8 of them by object ID) with
these options [R] [D for presence]: 353 horizontal distance range, 354 height range, 355 wait after the attack
(range, seconds), 356 angle within which the target must be (degrees), 358 may run alongside another response, 359
needs a clear line, 361 (flag), 363 enabled, 364 the animation entry it plays. Defaults [R]: 0 … 300, −5,000 …
5,000, 0.2 … 0.5 s, 10° — the same numbers as Sly 1's attacks
([../sly1/guards.md](../sly1/guards.md) 6.2). The angle test scales with distance by 0.005 per unit as in Sly 1
(`0x2BE600` = (0, 0.005, 0)) [D]. A failed choice is retried after 0.25 s (`0x2BE610`) [D].

The boar guards of world row 3 list four actions [D]:

| Action OID | Distance | Height | Angle | Wait after | Flags |
|---:|---|---|---:|---|---|
| 19643 | 0 … 400 | −150 … 200 | 360° | 0 | |
| 125099 | 0 … 1,150 | −175 … 175 | 60° | 0 | clear line, 361 |
| 125095 | 0 … 5,000 | −450 … 450 | 40° | 0.2 … 1.2 s | alongside other responses, clear line, 361 |
| 125097 | 0 … 5,000 | −450 … 450 | 40° | 0.2 … 1.2 s | clear line, 361 |

### 6.2 Shots

Guards whose gait allows shooting fire at a target [R] when it is between the **shot range** (option 298; default
300 … 1,200, which every swarmer template that sets it also uses [D]) and within ±45° of the guard's heading and
±75° of its pitch, the line to the target is clear, and either the target is climbing, hanging or on a pipe, or the
guard has been surrounding it for 3 s [R]. A player already being hit or held is not shot [R].

### 6.3 Melee damage: how a guard's strike hurts (executable and data findings)

**How a strike is delivered.** A guard does not send damage from its attack code. Its body carries a **collision
part** whose last fields ([classes/solid-object.md](classes/solid-object.md) 2, row 7) are a **contact kind** (a hit
kind bit set, `u32`) and a **contact value** (`s16`) [D for the fields, R for their meaning]. While the part touches the
player, the contact is sent with that kind and value, and the player's counter loses the value
([characters.md](characters.md) 2) [R]. The part is switched on and off by the attack clip's **events 15 (part on) and
16 (part off)**, which name the part by its tag; each attack clip also has an untimed event 16 in its stop list, so the
part goes off if the strike is cut short [D for the events, R for the effect].

**Every guard (class 58, all six species)** has exactly one such part [D]: tag **13056**, contact kind **`0x200`**
(knock-back), value **−10**, on a hand or weapon transform, radius 267 to 323. Its windows [D]:

| Species package | Clip | Length | Part on from … to |
|---|---:|---:|---|
| boar, rhino, moose, pelican | 422 | 1.5 s | 0.75 … 1.0 s |
| vulture | 422 | 1.5 s | 0.75 … 0.9167 s |
| boar | 13179 | 0.75 s | 0.3333 … 0.5 s |
| rhino, moose | 19761 | 0.75 s | 0.4167 (rhino) or 0.3333 (moose) … 0.5833 / 0.5 s |
| vulture | 19761 | 0.6667 s | 0.3333 … 0.5 s |
| pelican | 19761 | 0.75 s | 0.4167 … 0.5333 s |

The pig guard's package switches no part (the tutorial pig never strikes) [D].

**What one strike costs** [I from the values above and characters.md 2]: 10 health units at no difficulty help and 5
at full help (the counter holds 40, so 4 to 8 strikes); a shrunk guard's strike is halved again [R].

**Swarmers (class 57)** carry several parts [D]: values **−3** (chopper, spider, goose, monkey, pelican, rat, wolf
bites), **−5** (goose) and **−10**, with kinds `0x200` (knock-back), `0x400` (crush), `0x800` (stun), `0x1000`
(second stun) and `0x8000` (cobra and tentacle; a kind not in characters.md 2's table, meaning open). Their windows
come from the same events; for example the wolf's clip 59492 (2.25 s) bites three times, at 0.8 … 0.9167,
1.3 … 1.4167 and 1.8 … 1.9167 s [D].

### 6.4 Which animation each action plays (option 364) (2026-11-18 findings)

**What option 364 holds.** Option 364 is a method entry with two integer arguments in the executable's option
table (value type 0x12, flags 0x401, 2 arguments; routines `0x104408` and `0x1044B0`) [D]. It stores an **animation
reference**: the first value is the object ID of the animation, the second a start index [D for the fields, R for
their use]. When the guard binds its actions, the ID is looked up from the action's own package and the reference
becomes one of [R]:

| Named object | Becomes |
|---|---|
| a clip (class 109) | played as an ordinary clip from its start at speed 1 |
| a clip with a type-11 event of value 34 | a jump-driven clip |
| a state machine (class 111) | the state machine is applied; a start index of −1 becomes 0 |
| anything else, or not found | no animation: the action is skipped |

The action counts as finished when its clip (or state machine) has ended [R].

**On the disc** [D]: 393 option-364 entries on 382 distinct (world, action) records; the second value is **0 in all
393**. 371 name a clip of the same package, none of which has the type-11 value-34 event, so all play as ordinary
clips; one (Dimitri, row 5) names state machine 122437; the remaining 10 are action objects whose ID is shared with
another package's action and resolve there in the same way.

**Actions of the guards (class 58)** [D] (action object → clip, length; clips with events 15/16 are the melee strikes
of 6.3, the others fire at their event 12 [I]):

| Package | Actions |
|---|---|
| boar | 19643 → 13179 (0.75 s, strike); 125095 → 125103 (0.5 s); 125097 → 125105 (1.3333 s); 125099 → 125117 (1.3333 s) |
| rhino | 19637 → 37834, 19639 → 37836, 19641 → 37838 (each 1.3333 s); 19643 → 19761 (0.75 s, strike) |
| moose | 19637 → 37834 (1.3333 s), 19639 → 37836 (0.5 s), 19641 → 37838 (1.3333 s), 19643 → 19761 (0.75 s, strike) |
| pelican | 19637 → 19757 (1.3333 s), 19639 → 19759 (1.0 s), 19641 → 19763 (1.3333 s), 19643 → 19761 (0.75 s, strike) |
| vulture | 19639 → 19759 (0.75 s), 19641 → 19763 (1.0 s), 19643 → 19761 (0.6667 s, strike) |

The non-strike clips carry an event 12 (an `s32`: 10837, 20479 or −1) between 0.2833 and 0.5833 s with events 7/8
around it, and switch no damage part [D]; reading event 12 as the moment a shot or throw leaves is an inference [I].

**Other packages** [D]: swarming goat (10 actions, clips 45130 to 45183, 1.0 to 2.6667 s), goose (32510, 32512,
32514), pelican swarmer (19939 twice, 19941, 19943), rat (125345, 17901, 125347), wolf (59490, 59494, 59492, 59488),
spider mini (13179, 0.5 s), cobra and tentacle (65388, 1.6667 s), bear (17901, 34485, both 1.3333 s), Carmelita
(13212 0.5 s, 13173 1.3333 s), Contessa (65626, 65628, 65630), Rajan (95637, 95613, 417), Dimitri (122431, 19763,
state machine 122437), Jean Bison (19643 → 32389, 2.0 s; 32373 → 32387, 0.6667 s; vehicles-and-minigames.md 7.5).

## 7. Being hit and knocked out

- **Hit points**: option 254 sets the starting value. **Every guard template sets 40** [D]. Swarmers: chopper 15,
  spider 10, cobra and tentacle 5, mole 0, goat 30, goose 60, monkey, pelican, rat, ratso and wolf 40 [D]. Each hit
  subtracts its strength; the hit point count never goes below 0; at 0 the guard is knocked out [R]. How strong each
  of the gang's attacks is: [characters.md](characters.md).
- A hit the guard did not see coming (it has not seen the attacker in the last 0.2 s, `0x2C3DAC` [D]) is a
  surprise and can knock out at once (Sly's sneak attack) unless option 333 is set (set on spider swarmers) [R] [D].
- Option 232 makes a guard ignore hit damage [R].
- **Knocked out** [R]: it plays its knock-out animation, the tasks hear of it (task event 24 if the current player
  did it, else 23, [missions.md](missions.md)), its detector forgets everything, and it **spills coins**: a set
  count if its field says so, otherwise **half (rounded up) of the coins still in its pocket**, at a point on its
  body.
- Held, carried and thrown guards: [characters.md](characters.md) (Murray).

## 8. Pockets (what a guard carries)

Every prize giver (guards and some other characters) carries a **pocket** of up to 8 entries, filled from the
episode's pocket table when the guard resets [R]; Sly takes entries one at a time by pickpocketing
([characters.md](characters.md); [pickpocket-and-pickups.md](pickpocket-and-pickups.md)).

The tables are in the executable [D], one 60-byte record per episode (record 0 unused):

| Table | Virtual address | Used for |
|---|---|---|
| class-58 guards | `0x002C3968` | guards |
| other givers | `0x002C3748` | everyone else |

Record: `s32` fewest coins, `s32` most coins, `f32` chance of a prize, then six (`s32` weight, `s32` prize ID).

| | Guards [D] | Other givers [D] |
|---|---|---|
| Coins | 6 … 12 | 4 … 8 |
| Prize chance | 0.4 | 0.2 |
| Prize weights | 30, 30, 15, 15, 5, 5 | same |
| Prize IDs, episode 1 | 1116, 1115, 1122, 1121, 1119, 1118 | 1114, 1115, 1120, 1121, 1117, 1118 |

Episodes 2 and 3 share a row, as do 4 and 5, and 6 and 7 [D]. The prize IDs (1114 to 1143) identify loot items;
their meaning is in [pickpocket-and-pickups.md](pickpocket-and-pickups.md).

Filling [R]: the coin count is drawn from the range and split over 2 or 3 entries of roughly equal size (each at
least 1); with the given chance one prize is drawn by weight and added as a last entry. A prize carried over from the
previous fill stays unless its task is done. The pocket object (option 306) shows the glint over a robbable guard.

## 9. Option values on the templates (test vectors)

Guard templates (class 58) [D]. The intro world's pig template (row 0) is a tutorial guard: it sets only options
8, 33, 35, 59, 60, 62, 226, 243, 254, 281 = 0 and 314 = 1 (never sees). The other 27 templates all carry the values
below; option 254 is on all 28.

| Option | Value on the 27 templates | Meaning [R] |
|---:|---|---|
| 254 | 40 | hit points |
| 292 / 293 | 700 / 1,000 | near / far pursuit range |
| 315 | object 13065 | view volume (section 3) |
| 320, 321 | 1, 1 | strike without turn-taking; attack off screen |
| 327, 331, 304, 305, 306 | objects 13059, 8698, 12718, 12716, 12787 | home, grab point for Murray, two helpers, pocket object |
| 332 | 1 | (launch reaction flag) |
| 226 | object 19755 (27 templates), 19853 (the pig) | |
| 233 | 3 to 4 action objects | attacks (section 6.1) |
| 59 | 0.5 (23 templates), 1.0 (all 5 boar templates) | |
| 35 | gravity (0, 0, −3000) on boar and pelican templates; (0, 0, −1750) on the pig | ([physics.md](physics.md)) |

Example: world row 3 (`f_nightclub_cellar`), package `npc_boar_guard`, guard object 11539: options in stream order
62 = 1, 33 = 0, 35 = (0, 0, −3000), 59 = 1.0, 49 = 65662, 254 = 40, then 5 × option 53, 24 × option 243, 2 × 247,
2 × 248, then 315, 327, 331, 332 = 1, 304, 305, 306, 226, 2 × option 8, 292 = 700.0, 293 = 1,000.0, 320 = 1, 321 = 1,
4 × option 233, 2 × option 26 (removed at start: the template itself) [D].

Swarmer templates (class 57) [D]: hit points as in section 7; option 298 = 300 … 1,200 on goat, goose, monkey,
pelican, rat, ratso and wolf; options 296 / 297 = (100, 150) / (50, 200) on spiders and (450, 600) / (400, 650) on
wolves (circling distances [I]); view volume object 9091 (a different rig object per package; in row 3 it spans
x −232 … 776, y −667 … 679, z −258 … 266 around a point 122 ahead and 89 up) [D]; cobras and tentacles also name a
second, much larger volume (object 13106: ±1,096 × ±341 × ±949 in world row 8) [D] — they lie in wait and strike
when the player passes [O] [I].

## 10. Constants in the executable

SCUS_973.16 (SHA-256 above); first load segment: file offset = virtual address − 0x100000 + 0x80. Values read [D]:

| Address | Value | Meaning [R] |
|---|---|---|
| `0x2A2DC0` | (0, 0, 1) | eye direction (× 80) |
| `0x2BDCD0` | 1500.0 | default noise radius |
| `0x2BDCD8` | (−200, 500) | default noise height range |
| `0x2BDCE0` | 1.0 | cap of the radius growth time |
| `0x2BDCF0` | (0.15, 1.1, −0.25) | radius growth polynomial |
| `0x2BDD00` | −1, 750, 2500, 0.35, 4000, 0.5, 1000 | fixed-template overrides (section 4.1) |
| `0x2BE5A0` | 2.0 | swarmers' search start |
| `0x2BE600` | (0, 0.005, 0) | attack angle scale by distance |
| `0x2BE610` | 0.25, 500.0 | attack retry; chase re-plan distance |
| `0x2BE620` | 5.0 | (hit-reaction timer) |
| `0x2BE748` | 200.0 | surround re-plan distance |
| `0x2C3748`, `0x2C3968` | pocket tables | section 8 |
| `0x2C3D08` | 1000.0, 250.0 | "seen nearby" distance and height |
| `0x2C3DAC` | 0.2 | surprise window |
| `0x2C3DB0` | 2.0 | (awareness wait) |
| `0x2C3DB4` | 5.0 | target lost after |
| `0x2C3DB8` | 1.0 | alarm repeat |

Values not found as data (probably immediates in code, [R] only): eye height 80, investigation wait 5.5 … 6.5 s,
search distances 200 … 1,200 / 300 … 400 / 350, action defaults, shot angles ±45° / ±75°, the 3 s shot delay.

## 11. Open questions

- (Resolved: strike damage, section 6.3; the animation each action plays, section 6.4.) What the non-strike actions' event 12 launches.
- Options 49, 53, 59, 62, 226, 304, 305, 332 and 361: roles not established.
- Which option sets a guard's own alarm template.
- Spawner timing and which templates a hub spawner uses when (missions.md).
- The prize IDs 1114 to 1143.
- Every [R] statement needs checking in the running game; the boss characters (59 to 68) are left to the boss specs.
