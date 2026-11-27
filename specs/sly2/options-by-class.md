---
id: sly2-options-by-class
title: Sly 2 options by class
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316, all 43 worlds; executable SCUS_973.16 (SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73, checked before reading)
    used_for: >-
      section 3: every option list of every object body at every depth (121,153 bodies walked, 0 failures; the
      101,654 non-placement bodies carry the lists counted), plus all 5,979 option lists inside placement records
      (4,103 copy patches and 1,876 fresh-placement blocks, each parsed option by option to exactly its stored size),
      plus the two world-level lists; value types read from the option table at virtual address 0x002AC880
  - kind: reference-project
    what: >-
      the other Sly 2 specs in this folder (physics, camera, guards, alarms-sensors, effects, missions,
      dialogue-and-voice, level-flow, user-interface, safehouse-and-shop, gadgets, pickpocket-and-pickups, audio,
      clock-and-loop, scripts)
    used_for: the meanings gathered in section 2, each keeping the tag of its source spec
  - kind: disc
    what: >-
      2026-11-18 re-check: every option value at every depth (499,496 options in 121,153 bodies, 0 failures), all
      script modules of the 43 worlds (calls of the action options counted statically), option-table entries 20,
      24, 25, 44, 45, 64, 66, 73, 74, 77, 106, 146, 233, 368, 441, 479, 595 to 615 and 1070 read from SCUS_973.16
      after the hash check
    used_for: sections 2.1 to 2.3
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9, src/new_100008 (option wrapper routines), src/gs-jsg and src/jsg-jt (command
      builders), config/matching/symbol_addrs.txt; SafeHouse @ c70803ce, Sly2-Release scptai.cpp, alo.cpp,
      launcher.cpp, brx.cpp, spliceglue.cpp, soundglue.cpp
    used_for: what the action options and the class-113 commands do [R]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/scptai.cpp, ai.cpp, aijobs.cpp (guard command loaders and runner), scpt.cpp (sequence commands)
    used_for: meanings of guard commands 1, 2, 6 and 13 and of the start-sub-sequence command (2026-11-18)
  - kind: executable
    what: SCUS_973.16 (SHA-256 checked), option table at 0x002AC880, entry 636 (routine 0x1070E0, 3 arguments)
    used_for: identifying option 636 as the start-sub-sequence command (2026-11-18)
  - kind: disc
    what: re-walk of all 43 worlds (121,153 bodies), raw argument bytes of options 596, 597, 601, 606 and 636 (2026-11-18)
    used_for: argument types and value counts of those commands
  - kind: executable
    what: SCUS_973.16 (SHA-256 checked), option table at 0x002AC880, entries 106, 116 to 125, 366 to 381; class option map at 0x00279AD0 (script IDs 214 and 236)
    used_for: section 2.4 (2026-11-19)
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9, src/new_100008 (seek, apply-clip and snap-to-goal wrappers), src/aseg-asega/SeekAsega.cpp,
      src/asega/PloRootOrAseg_file.cpp (time wrap, byte-matched), src/aseg/ApplyAseg.cpp, config/matching/symbol_addrs.txt;
      SafeHouse @ c70803ce, Sly2-Release asega.cpp, asega.h, smrun.cpp, update.cpp
    used_for: meanings in section 2.4 [R]
  - kind: disc
    what: all script modules of the 43 worlds; calls of script IDs 214 and 236 with argument counts and literal values (2026-11-19)
    used_for: call and value counts in section 2.4
reviewed_by: pending
date: 2026-11-26
---

# Sly 2 options by class

Delta of [../sly1/options-by-class.md](../sly1/options-by-class.md). Option-list encoding:
[object-stream.md](object-stream.md) 1; script option IDs and the per-class map: [scripts.md](scripts.md) 5.

Tags: [D] disc data, [R] reference behaviour to confirm, [I] inferred.

## 1. How options apply

- **Order, overwriting, actions and defaults** are as in Sly 1 section 1 [R]: options in a list are applied in
  order at load, after the class defaults; a later option of the same ID overwrites an earlier one, except options
  that add to a list.
- **Encoding.** Each stream option ID has a 40-byte entry in the executable's option table (virtual address
  `0x002AC880`), which gives its value type or typed-argument count ([object-stream.md](object-stream.md) 1.1 to
  1.3). A port reads the table from the player's executable after the hash check [D].
- **Script access.** Scripts use per-class script option IDs, mapped to stream IDs through the class map at
  `0x00279AD0`; IDs from 1,233 up are custom properties held by the object itself ([scripts.md](scripts.md) 5)
  [D] [R]. Option 225, for example, is the **gravity ratio** in the option table, not a speed scale [D].
- **Where option lists occur** [D]: the two world-level lists ([world-file.md](world-file.md)); every object body
  that has a list (solid bodies, simple layouts, clips, state machines, sequences, emitters, missions, screen
  movies); and **inside placement records**:
  - every patch of a copy placement (4,103) and every block of a fresh placement (1,876) is an option list that
    ends exactly at its stored size;
  - the first field of a fresh-placement block is a **target object ID**, as for patches: 1,486 blocks name 5 and
    327 name 6 (the placed copy's root objects), and 63 name other object IDs (for example 11539, a guard) [D]. This
    replaces the "block kind" reading of [classes/placement.md](classes/placement.md) 1.1 [I for the target
    meaning];
  - a block's class ID is the class of its target; section 3 counts these lists under that class, marked `[p…]`.
- **Removed at start.** Option 26 takes the object out of the world when the level starts
  ([object-stream.md](object-stream.md) 1.4); 9,182 bodies carry it [D].
- 737 distinct option IDs occur at every depth (693 in bodies, 44 more only in placement lists), the highest
  1,448 [D]. ([object-stream.md](object-stream.md) 1 counts 464 over a narrower set of lists.)

## 2. Meanings of the commonly needed options

Gathered from the other Sly 2 specs; each line keeps the tag of its source. Options not listed have no
established meaning yet.

| Option | Classes | Meaning | Source |
|---|---|---|---|
| 13 | most solids, markers, treasure | activation radius (4,000 in 725 uses; also 3,000, 4,500, 5,000, 6,000) [D values] [R meaning] | clock-and-loop.md |
| 26 | almost all | removed when the level starts [D] | object-stream.md 1.4 |
| 33, 34 | solids | linear and angular constraint [R] | physics.md |
| 35 | solids, characters | gravity vector; (0, 0, −1750) on the three characters [D] | physics.md |
| 225 | characters, guards | gravity ratio; 1.3 on the three characters, giving −2275 [D] | physics.md |
| 37, 39, 40, 52, 57, 61, 63, 75, 76, 80 to 82 | solids | further physics settings, with disc values | physics.md |
| 100 | 43 ambient sound | channel fall-off: channel, start distance, end distance, volume [D layout] [R meaning] | audio.md |
| 151 to 158 | 4 light | kind, colour (hue, saturation, value), strength, angle, fall-off range, shadow direction, light selection, static bake [R] | effects.md |
| 161 to 170 | world list 2 | lens and camera: 161 field of view, 162 its target, fog and clip distances | camera.md |
| 175 | 5 zone volume | volume priority | camera.md |
| 178 to 214 | 15 camera zone | zone framing and behaviour; 214 the switching mode (follow the player, forced off, forced on) | camera.md |
| 254 | 58 guard, 57 swarmer | hit points (40 on every guard template) [D] | guards.md |
| 292, 293 | 58 | near and far chase range (700 and 1,000) [D] | guards.md |
| 312 to 315 | 58, 16 | sight rules: 312 sees hidden targets, 313 always sees, 314 never sees, 315 the view volume [R] | guards.md |
| 342, 345, 346, 349 | 16 detector | flags, view volumes, watched classes (7, 9, 8) | alarms-sensors.md |
| 353 to 364 | 120 attack action | attack parameters | guards.md |
| 413, 418, 419, 421 | 83 touch pad | press kind, who can press, re-arm time, no press sound | alarms-sensors.md |
| 448, 449, 455 | 85 laser | objects and classes that trigger it; one beam per occurrence of 455 | alarms-sensors.md |
| 469 to 593 | 29, 32, 33 emitters | emission, launch and motion, drawing; explosion sound 589 to 593 | effects.md |
| 622 to 708 | 114 sequence | dialogue commands: 695 play a line, 708 wait for it, 685 and 686 binocucom talk | dialogue-and-voice.md |
| 779, 780, 788 | 26 exit | arrival point, wipe, and a flag on the intro exit | level-flow.md |
| 790 to 792 | 42 arrival point | extra IDs | level-flow.md |
| 902, 907, 1152 | 91, 94, 103 | coin ground times; pocket-item display definitions (loot kinds); treasure definitions | pickpocket-and-pickups.md |
| 914 to 916 | world list 2 | world shadow colour, alpha, scale | effects.md |
| 1102, 1108, 1113 | 100 to 102 gadget objects | gadget-object settings (1108 = −20 on mines) | gadgets.md |
| 1122 to 1143 | 126 to 131 screen movies | 1123 draw priority, 1124 pause while shown, 1125 idle limit, 1133 to 1142 string arguments | user-interface.md |
| 1242 to 1263 | 132 job | job settings: first and last task, names, exit, save keys | missions.md |
| 1264 to 1299 | 133 task | task settings: job, objective, checkpoint, world row, starting character, switches, goals, save keys | missions.md |
| 1304, 1305, 1307 | 105 (one per character, nested in its body) | 1305 = 30 on all 126 bodies [D]; read as the starting power [I] | gadgets.md |
| 1337 to 1339 | 70 safe house | state machine, Cross action, selectable states | safehouse-and-shop.md |

### 2.1 Action options (methods called by scripts)

An action option is an option whose table entry has a routine at offset 16 (the one scripts call) and an
argument count at offset 24 [D]. A script fetches it from an object with the get-option operation (op 111) and calls
the result, `((op111 object id) arguments...)` (scripts.md 4, 5). The "script ID" column is the ID the scripts use,
mapped to the option by the class map (scripts.md 5). Call counts are calls written in the scripts of all 43 worlds
(static counts) [D]; meanings come from the reference loader and the decomp's wrapper routines [R]. Rows follow the
game session's run-time call counts.

| Option | Script ID | Classes (map) | Args (entry +24) | What it does |
|---:|---:|---|---:|---|
| 595 | 622 | 113 | 1 | class-113 command list: **context** command (section 2.2) |
| 44 | 110 | 61 classes from 48 up (solid-derived) | 1 | **show** the object's meshes whose mesh object ID (geometry.md mesh header) equals the argument [R]; 726 calls, 666 with a literal ID [D] |
| 66 | 174 | same 61 classes | 1 | **find** the object's texture-animation controller (classes 139 to 144, rendering.md 3) with that object ID and return it [R]; 1 call [D] |
| 20 | 33 | 132 classes, 2 up (every placed object) | 2 | **set the parent**: attach the object to the object given (detach it with no object), with a keep-flags value: 0 = nothing kept (the object's matrix becomes relative to the new parent), 3 = keep the world position and rotation, 15 = also keep the velocities [R]; 678 calls; second argument 0 (351), 3 (254), 1 (43) [D] |
| 106 | 214 | 109, 110 (clips) | 4 | **apply the clip** to an object: (target object, start time, speed, flags); returns the new playback [R]. Common literal forms (0, 1, 0) and (0.0, 1.0, 8) [D] |
| 74 | 183 | 61 classes from 48 up | 1 | **find the playback of a state machine** (class 111) by its object ID on this object and return it [R]; 412 calls, 406 with a literal ID [D] |
| 45 | 111 | 61 classes from 48 up | 1 | **hide** the meshes with that mesh object ID (as 44) [R]; also applied at load when the option is in an object's own list [R]; 963 calls [D] |
| 479 | 560 | 29 (emitter) | 0 | **stop emitting** indefinitely; particles already out live on [R]; 47 calls [D] |
| 73 | 182 | 61 classes from 48 up | 1 | **find the playback of a clip** by the clip's object ID on this object and return it [R]; 202 calls [D] |
| 146 | 273 | 152 (sound manager) | 1 | **queue a music stream** by its number (audio.md 3) and return its handle [R]; 209 calls, stream numbers 10 to 32 (32, 13, 12 and 25 the most) [D] |
| 368 | 214 | 111 (state machine) | 3 | **apply the state machine** to an object: (target object, goal, flags); returns the playback [R] |
| 64 | 166 | 61 classes from 48 up | 2 | **set the texture-scroll master speeds** (u speed, v speed) of the object's scrolling textures [R]; 25 calls [D] |
| 25 | 39 | 132 classes, 2 up | 0 | **add the object to the world** (make it live). Entry flags `0x4401`: an object that has this option in its own stream list is added at load [R]; 2,365 calls [D] |
| 601 | 625 | 113 | 3 | class-113 command list: **follow a path** command (section 2.2) |
| 24 | 37 | 132 classes, 2 up | 1 | **move the object to another object's place** (position and heading of the given object; characters are placed as at a checkpoint) [R]; 248 calls [D] |
| 233 | 359 | 56 to 67 (guards and enemies) | 1 | add an **aim target** object ID to the guard (guards.md) [R]; 250 calls [D] |
| 77 | 194 | 61 classes from 48 up | 2 | create a **new emitter handle** on the object (two integers, for example (−1, 1)) and return it [R]; 4 calls [D] |

Value types of these entries [D]: 20, 24, 25, 64 and 479 have value type 18 (no value). 66 `0x208A`, 74 and 368
`0x2088`, 73 and 106 `0x2086`, 146 `0x2097`, 77 `0x2095`. The high part `0x20` marks an object result and the low
byte its class (0x8A = 138, 0x88 = 136, 0x86 = 134, 0x97 = 151, 0x95 = 149) [I]; the same reading gives
0x72 = 114 for options 441 and 1070 (section 2.3).

### 2.2 Class 113: guard command lists

A class-113 object is a **command list for a guard or another enemy** [R]. Its stream options 595 to 612 each
append one command, in stream order, when the object loads. A guard that is told to run the list (by a script or a
sequence) works through the commands, waiting on each until it is done [R]. There are 905 class-113 objects on the
disc [D].

| Option | Command | Arguments | What the guard does [R] |
|---:|---:|---|---|
| 595 | 0 | object | set the context object (the one later commands refer to) |
| 596 | 1 | range (min, max), seconds | **how long the guard keeps running its command list** before it pauses: a random time in the range, drawn each time the list resumes. Default when absent: forever [R]. Disc: (5, 15) ×35, (3, 7) ×15, (500, 500) ×4, seven other pairs once or twice (62 uses) [D] |
| 597 | 2 | range (min, max), seconds | **how long the guard then stands still** before it resumes the list: a random time in the range. Default when absent: 0, no pause [R]. Disc: (3, 4) ×29, (0.5, 2) ×15, (6, 8) ×6, (0, 0) ×4, five other pairs (62 uses) [D] |
| 598 | 3 | object | look at the object |
| 599 | 4 | object | send a message to the object |
| 600 | 5 | float | wait that many seconds |
| 601 | 6 | path object, direction flag (1 byte), face object | **follow a path**: the object is a curve (classes 10 to 13). Direction 1 walks it from its start toward its end, 0 the other way; a guard currently on a rail or a ladder moves along that instead, in the same direction sense. The third argument is an object to face while walking (−1: face along the path) [R]. Disc: 340 uses, direction 1 in 338 and 0 in 2, face object −1 in all 340 [D] |
| 602 | 7 | object, float, object | go to the object at that speed, facing the third object on the way (−1 for none). Disc: speeds 40.0 (349) and 50.0 (28); face object 9 (Murray) in 28 [D] |
| 603 | 8 | object | play the clip (never used on the disc [D]) |
| 604 | 9 | number | a label |
| 605 | 10 | number | jump to the label |
| 606 | 13 | object, reverse flag (1 byte) | **use an object**: the guard gets on and travels the object it names (a ladder, pipe, rail or similar context object; targets include classes 11, 13 and 19), in reverse when the flag is 1 [R]. Waits until the use ends. Disc: 482 uses, flag 1 in all of them [D]. **What "use" does depends on the target's class** (added 2026-11-26) [R]: a curve (class 10 and derived): get on it at the nearest point; class 13: sit, facing away; class 39: state bit 0x29; class 11 (ladders, pipes): climb, hands placed on it; class 19 of bar kind 3: hang. **Anything else, for example a class-3 transform marker: the marker's position is dropped onto the floor (searched from 100 above to 150 below) and the guard makes its move there**, a ballistic jump when its move kind is the jumping one (the use passes "ballistic" = 1). So `606 marker` is a hop to the marker's spot. The use keeps the guard active while it lasts (clock-and-loop.md 3.2). Neyla's lists 124079 and 123907 come down from the roofs this way |
| 607, 608 | 11, 12 | none | lock, unlock: **keep the guard active** from 607 to 608. Locking puts the guard's activation kind to 1 (always active, whatever the camera's zone mask or distance) and unlocking back to 0; nested locks are counted (`0x0020B438` / `0x0020B478`) [D for the calls, R for the counting]. Episode 1's Neyla list 123907 wraps each run of 606 uses in 607 … 608 (clock-and-loop.md 3.2) [D] |
| 611 | 14 | clip object, flags | play an animation clip and wait for it to end; flags on the disc 0 (302), 8 (181), 1 (114), 9 (54) [D] |
| 612 | 15 | sequence object, number | raise the numbered alarm on a class-114 sequence; all 71 uses name a class-114 object [D] |

Commands 5, 6, 7, 13 and 14 hold the list until they are done (time passed, arrived, use or animation over) [R]. While
the guard is idle, running the list and standing still alternate under the time ranges of commands 1 and 2 [R].

### 2.3 Options that start sequences

Exactly two options are typed as a reference to a class-114 object (value type `0x2072`, flags `0x92`, stored in
an object field) [D]: **441** on trigger volumes (class 24, field at `0x1DC`; 40 uses, every one naming a class-114
object) and **1070** on interaction spots (class 35, field at `0x114`; 82 uses, all naming class-114 objects) [D].
How they start a sequence: dialogue-and-voice.md 2.1.

Inside a sequence's own command list, **option 636** is the start-sub-sequence command (option-table entry 636,
routine `0x1070E0`, 3 arguments: sequence object, starter object or −1, no-wait flag) [D]. It occurs 708 times, all
in class-114 lists [D]; its behaviour is in dialogue-and-voice.md 2.1 [R].

**Correction to [gadgets.md](gadgets.md):** options 1304, 1305 and 1307 are carried by the 126 class-105 objects
nested in the three characters' bodies (one per character per world), not by the character records of classes 53
to 55 themselves [D].

### 2.4 Clip and state-machine playbacks (options 106, 123, 368, 379)

Applying a clip (option 106) or a state machine (option 368) creates a **playback** object: class 134 for a clip,
class 136 for a state machine (value types `0x2086` and `0x2088`, section 2.1) [D for the types, R for the
classes]. Two script IDs are shared between the two kinds, and the argument count tells them apart on the disc [D]:

| Script ID | Class of the object called | Option | Args | Calls in the scripts [D] |
|---:|---|---:|---:|---:|
| 214 | 109, 110 (clip) | 106 apply the clip | 4 | 668 |
| 214 | 111 (state machine) | 368 apply the state machine | 2 or 3 | 496 (353 with 2, 143 with 3); 13 more calls pass 1 value |
| 236 | 134 (clip playback) | 123 **seek** | 3 | 140 |
| 236 | 136 (state-machine playback) | 379 **snap to goal** | 1 | 671 |

A run-time count of "option 123" calls that includes one-value calls is therefore also counting option 379 calls
[I].

**Option 106, the flags argument** (fourth value), bit by bit [R; the bits tested are confirmed in the byte-matched
decomp's time-wrap routine and in its apply routine]:

| Bit | Value | Meaning |
|---:|---:|---|
| 0 | 1 | the clip's channels **ease** their targets toward the clip's values; clear: they **set** the targets at once |
| 1 | 2 | fire the clip's start events (those at or before the start time) when the playback is created |
| 2 | 4 | snap: on creation every channel also takes the clip's value at the start time immediately |
| 3 | 8 | **loop**: past the end (or before the start, when playing backwards) the time wraps modulo the clip's length, and the timed events start again from the first (or last) one |
| 4 | 16 | **back and forth**: the time reflects at each end and the speed changes sign |
| 5 | 32 | **hold**: keep the playback at the end instead of removing it |

With none of bits 3 to 5 set, the time is clamped at the end (or the start) and the playback is **removed** in the
frame it gets there. Bit 3 takes precedence over bit 4. Literal flags on the disc (4-value calls) [D]: 0 (327),
8 (223), 4 (64), 1 (32), 9 (11), 16 (7), 32 (4). The engine's reading "flag 8 loops the clip" is **right**; without
bits 3 to 5 the playback does not loop and also does not stay: it ends and is removed.

**Option 123 on a clip playback: seek** (entry: value type 18, 3 arguments, routine `0x101C68`) [D]. Arguments
(mode, time, speed) [R]:

- mode 0: the new time is the given time; mode 1: the current time plus the given time; mode 2: the clip's length
  plus the given time;
- the playback's speed is set to the given speed (times the clip's own speed factors);
- when the new time differs from the old one, it is wrapped by the playback's flags as above, the event cursor is
  moved to the new time **without firing** the events passed over, and every channel's target is snapped to the
  clip's value at the new time;
- the playback's target time is set to the new time.

On the disc the mode is 0 in 138 calls and 2 in 2 [D]; common (time, speed) pairs are (0, 0) (38: hold the first
frame), (0, 1) (29: restart), and fixed times with speed 0.6 or 0.2 [D].

**Option 379 on a state-machine playback: snap to goal** (entry: value type 18, 1 argument, routine `0x104B70`)
[D]. The argument is a state's object ID [R]:

- the goal becomes that state;
- the transition being played, if any, is finished at once (its clip jumps to its end);
- the machine then walks its route toward the goal with every further transition started **at its end**, so it
  arrives in the goal state within the same call, its targets posed as at the end of the last transition;
- each state passed through raises the state-change event below.

The engine's reading "379 jumps to a state" is right in effect; unlike setting the state directly, the route's
transitions are passed through and announced, not skipped [R]. The 671 calls name states such as 1546 (210) and
1545 (189) [D].

**Events a playback raises in its module** (the module of the clip or state-machine object, scripts.md 6) [R]:

| Playback | Event | When | Values after the playback |
|---|---:|---|---|
| clip | 2 | created | none |
| clip | 3 | removed | none |
| clip | 4 | a timed event of type 12 is passed | the event's integer word |
| state machine | 2 | created | none |
| state machine | 3 | removed | none |
| state machine | 8 | **the state changes**, including entering the first state at creation (from none) | old state, new state (object IDs; −1 for none) |

The state change also sends message 10 (old, new) to the playback's target object and to the playback's
listeners [R]. **The engine's working choice "events 14 and 15 on state entries" is not what the game does:**
state changes raise event 8 in the state machine's own module (79 handlers on the disc, all in modules attached to
class-111 objects, signature length 3 [D]). Events 14 and 15 belong to other objects (solids, swarms, props;
scripts.md 6).

## 3. Which options each class uses on the disc

113 classes carry options; 1,258 class/option pairs occur [D]. Each entry is `ID (bodies)`: the number of bodies
of that class whose own option list holds the ID (an ID can repeat within one list). `[pN]` adds N option lists
inside placement records whose target has that class. Class names follow [object-stream.md](object-stream.md) 3 and
the behaviour specs; "?" marks a class without an established role. Classes with bodies but no options: 23, 31,
41, 99, 104.

| Class | Options used on the disc: ID (bodies) [placement lists] |
|---|---|
| 2 settings holder | 3 [p83], 4 [p20], 5 [p61], 7 [p1], 8 [p797], 13 [p301], 26 (9) [p2695], 32 [p3] |
| 3 transform marker (rigs) | 26 (151) |
| 4 light | 26 (92), 151 (2855), 152 (2855), 153 (2855), 154 (2855), 155 (2688), 156 (121), 157 (2843), 158 (2851) |
| 5 zone volume (packages named `…_vz`) | 175 (8) |
| 6 prefab placement | 13 [p63], 26 [p231], 1178 [p1290] |
| 9 ? | 873 (7) |
| 10 curve (paths, rails) | 13 (6), 26 (7), 1208 (6), 1209 (6), 1214 (6), 1217 (6), 1219 (6) |
| 11 curve variants | 26 (2) [p2], 838 (235), 839 (61), 840 (61), 841 (20), 842 (1), 844 (3), 845 (3) [p1], 849 (2) |
| 12 curve variants | 806 (18), 808 (18), 811 (4) |
| 13 curve variants | 13 (7), 26 (55), 858 (52), 864 (9) [p9], 1208 (7), 1209 (7), 1214 (7), 1217 (7), 1219 (7) |
| 14 obstacle along a curve ? | 26 (1) |
| 15 camera zone | 26 (39), 178 (563), 179 (143), 181 (181), 183 (560), 185 (30), 186 (519), 187 (404), 188 (215), 189 (20), 190 (7), 191 (86), 192 (128), 193 (127), 194 (133), 195 (1), 196 (22), 197 (601), 198 (85), 199 (12), 200 (543), 201 (105), 203 (21), 204 (108), 205 (5), 206 (11), 207 (34), 208 (494), 209 (52), 210 (3), 211 (87), 212 (91), 214 (218) |
| 16 stand-alone detector | 342 (4) [p2], 345 (22), 346 (22), 349 (22) [p6], 350 [p10] |
| 17 navigation mesh ? | 26 (1) |
| 18 markers (20: hand-adjust points of the gang) | 215 (86) [p2], 216 (25), 217 (17), 218 (1) |
| 19 markers (20: hand-adjust points of the gang) | 1011 (62) [p1], 1012 (2), 1015 (2), 1016 (59) [p5], 1019 (17) [p1], 1020 (4) |
| 20 markers (20: hand-adjust points of the gang) | 1445 (84) |
| 21 only nested | 219 (1) |
| 22 volume (guard view volumes) | 13 (41), 26 (22) |
| 24 volumes (24, 25: volumes acting as buttons ?) | 3 (1), 13 (2), 26 (101), 432 (183), 433 (193) [p4], 434 (6), 438 (2), 439 (30), 440 (88), 441 (40), 443 (33) |
| 25 volumes (24, 25: volumes acting as buttons ?) | 26 (1) |
| 26 exit | 26 (1), 779 (51), 780 (370), 788 (1) |
| 27 volumes (24, 25: volumes acting as buttons ?) | 26 (14), 1182 (159), 1189 (116) |
| 29 emitter | 13 (4914), 26 (1797), 469 (3), 471 (203), 472 (6228) [p46], 473 (5), 474 (198), 475 (198), 477 (1347), 482 (269), 483 (3458), 485 (3212), 486 (3212), 487 (242), 488 (8), 489 (8), 490 (168), 491 (4502), 494 (506), 495 (5591) [p42], 496 (6188), 497 (4657), 498 (6230) [p42], 499 (6201), 500 (480), 501 (42), 502 (5974), 504 (2461), 505 (6230) [p4], 506 (248), 507 (1683), 508 (6200), 509 (269), 510 (37), 511 (269), 514 (2), 515 (267), 516 (243), 517 (4), 518 (4), 523 (3599), 524 (46), 525 (2556), 526 (5961), 527 (46), 528 (2604), 529 (5964) [p11], 530 (1594), 531 (2203), 532 (3231), 533 (365) |
| 32 explosion part | 536 (9), 538 (9), 539 (9), 543 (2), 547 (50), 548 (52), 549 (52), 550 (52), 552 (50), 553 (43) |
| 33 explosion part | 13 (5984), 26 (189), 536 (4102), 537 (24), 538 (3934), 539 (3934), 542 (194), 543 (7186) [p2], 546 (336), 547 (5774), 548 (8233), 549 (6254), 550 (8070), 551 (44), 552 (4627), 553 (998), 554 (3153), 555 (8233) [p33], 556 (294), 557 (3195), 558 (8225), 559 (7998), 560 (241), 561 (19), 562 (8233), 563 (3127), 564 (8122), 565 (2619), 566 (3153), 567 (72), 568 [p47], 569 (98), 570 (3061), 571 (510), 572 (158), 573 (158), 575 (14), 576 (14), 577 (14), 578 (3878), 579 (9), 580 (784), 581 (5228), 582 (9), 583 (623), 584 (5080) [p33], 585 (1058), 586 (2258), 587 (2908), 588 (970), 589 (2823) [p9], 590 (2774) [p9], 591 (2774) [p9], 592 (2042) [p9], 593 (2422) [p9] |
| 35 interaction spot | 26 (30) [p2], 1068 (1), 1069 (115), 1070 (82) [p1], 1073 (28), 1074 (51), 1076 (43) [p6], 1081 (10), 1082 (21), 1084 (15) |
| 36 only nested | 26 (71) [p9], 1069 (204), 1074 (204), 1085 (204), 1086 (204), 1087 (38), 1088 (31), 1089 (1) |
| 37 markers and volumes ? | 1074 (11), 1076 [p2], 1081 (11) [p3], 1167 (4) [p27], 1169 (4) [p2], 1171 (3) [p2], 1173 [p14] |
| 38 markers and volumes ? | 26 (12), 816 (12), 818 (12), 825 (12) |
| 39 markers and volumes ? | 26 (3), 852 (5) [p3], 853 (4) |
| 40 only nested | 870 (1) |
| 42 arrival point | 790 (28), 792 (2) |
| 43 ambient sound | 26 (303) [p1], 99 (978), 100 (102) |
| 44 cutscene camera | 26 (171), 1054 (28), 1055 (36), 1056 (178), 1057 (178), 1058 (178), 1059 (93) |
| 45 only nested | 1060 (252), 1064 (252), 1065 (252) |
| 46 markers and volumes ? | 26 (3), 1195 (54), 1196 (8), 1197 (13), 1198 (54), 1199 (34), 1200 (41), 1201 (53) [p4], 1202 (17), 1203 (14), 1205 (3), 1206 (3) |
| 47 only nested | 26 (4) |
| 48 solid object (props, level geometry pieces) | 3 (19) [p1], 4 (13) [p14], 5 (45) [p15], 8 (71) [p47], 13 (999), 26 (5735) [p2], 29 (2), 30 (211), 33 (59), 34 (42), 35 (5), 37 (43), 39 (6), 44 (3), 45 (37), 47 (14), 49 (25), 50 (25), 52 (8), 53 (5) [p52], 57 (12), 61 (58) [p1], 63 (26), 67 (9), 76 (63) [p3], 80 (18) [p2], 81 (18), 82 (3) |
| 49 level geometry (the large static mesh of each level) | 62 (115) |
| 50 driven machine | 3 (4), 4 (4), 5 (4), 8 (4), 26 (7), 33 (5), 34 (1), 53 (5) |
| 52 ? | 232 [p2] |
| 53 Sly | 32 (42), 33 (42), 35 (42), 53 (42), 59 (42), 60 (42), 225 (42), 760 (42), 771 (42), 772 (42) |
| 54 Murray | 32 (42), 33 (42), 35 (42), 45 (42), 59 (42), 60 (42), 225 (42), 226 (42), 719 (42), 720 (42), 726 (42) |
| 55 Bentley | 32 (42), 33 (42), 35 (42), 45 [p1], 47 (42), 59 (42), 60 (42), 225 (42), 226 (42), 719 (42), 720 (42), 760 (42), 775 [p1], 889 [p1] |
| 56 ? | 8 [p68], 13 [p43], 26 [p14], 53 [p5], 80 [p8], 81 [p2], 231 [p2], 232 [p6], 243 [p3], 249 [p1], 254 [p6], 255 [p6], 273 [p116], 274 [p20], 275 [p30], 276 [p6], 281 [p15], 284 [p2], 294 [p1], 304 [p1], 305 [p1], 307 [p51], 308 [p35], 309 [p37], 313 [p28], 314 [p16], 315 [p1], 316 [p6], 321 [p6], 328 [p8], 329 [p9], 332 [p2], 341 [p29] |
| 57 swarmer | 8 (58) [p18], 13 (1), 33 (66), 34 (8), 35 (59), 45 (14), 47 (14), 49 (22), 53 (59), 59 (58), 60 (58), 62 (57), 80 (13), 226 (66), 233 (7), 235 (12), 236 (6), 242 [p1], 243 (66), 244 (45), 245 (58), 246 (45), 247 (40), 248 (40), 254 (66), 257 (8), 259 (21), 273 [p3], 276 (18) [p8], 281 (8) [p2], 285 (8), 286 (8), 292 (16), 293 (16), 296 (23), 297 (23), 298 (40), 304 (39), 305 (39), 306 (39), 307 [p16], 312 (5), 313 [p8], 314 [p2], 315 (66), 316 (9), 320 (5), 324 (7), 327 (40), 328 [p3], 331 (40), 332 (45), 333 (13), 339 (13) |
| 58 guard | 5 (5) [p1], 8 (28) [p3], 33 (28), 35 (9), 45 (8), 47 (8), 49 (27), 53 (27), 59 (28), 60 (1), 62 (15), 80 [p7], 226 (28), 233 (27), 243 (28), 247 (27), 248 (27), 254 (28), 273 [p3], 274 [p1], 275 [p1], 281 (1) [p5], 292 (27), 293 (27), 301 [p1], 304 (27), 305 (27), 306 (27), 307 [p8], 308 [p5], 309 [p5], 313 [p1], 314 (1) [p2], 315 (27), 320 (27), 321 (27), 327 (27), 328 (5), 331 (27), 332 (27) |
| 59 ? | 8 (2) [p1], 26 [p1], 33 (2), 35 (2), 45 (2), 47 (2), 53 (2), 59 (2), 60 (2), 62 (2), 226 (2), 243 (2), 244 (2), 245 (2), 254 (2), 256 (2), 259 (2), 292 (2), 293 (2), 296 (2), 297 (2), 315 (2), 328 (2), 329 (2), 338 (2), 1340 [p4] |
| 60 ? | 26 [p2], 33 (9) [p1], 34 [p1], 35 (9), 44 [p3], 45 (9) [p9], 47 (9), 53 (9) [p2], 81 (9) [p1], 224 [p1], 226 (9), 231 [p1], 243 (9) [p2], 245 (9), 254 (9), 281 (9) [p2], 314 [p2], 315 (9) [p1], 316 (9), 1114 (9) [p1], 1115 (9) [p1], 1118 (9), 1119 (9), 1120 (9) |
| 61 ? | 8 (3), 33 (3), 35 (3), 53 (3), 59 (3), 60 (3), 80 (3), 226 (3), 243 (3), 244 (3), 245 (3), 246 (3), 249 (3), 254 (3), 257 (3), 259 (3), 315 (3), 320 (3), 327 (3), 331 (3), 333 (3) |
| 62 ? | 33 (6), 53 (6), 59 (6), 60 (6), 62 (6), 226 (6), 233 (6), 243 (6), 245 (6), 246 (6), 254 (6), 257 (6), 259 (6), 292 (6), 293 (6), 296 (6), 297 (6), 315 (6), 320 (6), 321 (6), 325 (6), 338 (6) |
| 63 ? | 8 (7), 33 (7) [p2], 34 [p2], 35 (7), 45 (7), 47 (7), 49 (7), 53 (7) [p2], 59 (7), 60 (7), 62 (7), 224 [p2], 226 (7), 232 (7), 243 (7), 245 (7), 254 (7), 257 (7), 259 (7), 273 [p2], 274 [p1], 275 [p1], 292 (7), 293 (7), 304 (7), 305 (7), 306 (7), 307 [p2], 309 [p3], 315 (7) [p1], 320 (7), 321 (7), 327 (7), 329 (7), 333 (7) |
| 64 ? | 8 (6) [p6], 33 (6), 35 (6), 53 (6), 59 (6), 60 (6), 62 (6), 224 [p2], 226 (6), 243 (6), 244 (6), 245 (6), 254 (6), 257 (6), 259 (6), 292 (6), 293 (6), 296 (6), 297 (6), 304 (6), 305 (6), 306 (6), 307 [p3], 313 [p2], 315 (6), 320 (6), 321 (6), 325 (6), 327 (6), 328 (6), 329 (6), 332 (6), 338 (6) |
| 65 ? | 33 (2), 35 (2), 53 (2), 59 (2), 60 (2), 226 (2), 243 (2), 254 (2), 257 (2), 281 (2), 298 (2), 315 (2), 316 (2) |
| 66 ? | 8 (3), 33 (3), 35 (3), 45 (3), 47 (3), 53 (3), 59 (3), 60 (3), 62 (3), 226 (3), 232 (3), 233 (3), 243 (3), 254 (3), 256 (3), 259 (3), 292 (3), 293 (3), 304 (3), 305 (3), 306 (3), 315 (3), 320 (3), 327 (3) |
| 67 ? | 8 (1), 33 (1), 35 (1), 53 (1), 59 (1), 60 (1), 62 (1), 226 (1), 243 (1), 245 (1), 254 (1), 257 (1), 281 (1), 328 (1), 329 (1) |
| 68 ? | 33 (1), 35 (1), 45 (1), 226 (1), 1342 (1), 1343 (1) |
| 69 turret | 5 (2) [p3], 13 (1), 26 (4), 53 (4), 1000 (6), 1001 (6), 1007 (2), 1008 (2), 1009 (4), 1010 (6) [p7] |
| 70 safe house | 8 (8) [p16], 13 (8), 1337 (8), 1338 (8) [p23], 1339 (8) |
| 72 ? | 3 (5), 33 (5), 34 (5), 53 (5), 1363 (5), 1371 (5), 1372 (5), 1373 (5), 1374 (5) |
| 73 ? | 3 (26), 5 (9), 33 (11) [p10], 34 [p10], 53 (16), 1363 (26), 1365 (10), 1366 (16), 1369 (5), 1370 (5), 1375 (20), 1376 (11), 1377 (11), 1378 [p52], 1379 [p102], 1380 (16) [p10], 1381 (9), 1382 (11), 1384 (11), 1386 [p3] |
| 74 ? | 13 (7), 33 (7), 1366 (7), 1388 (1), 1389 (2) |
| 75 ? | 3 (5), 1392 (5), 1393 [p28], 1394 (5) |
| 76 ? | 53 (22), 67 (17), 1365 (22), 1366 (22), 1397 (22), 1398 (22), 1399 (22) |
| 77 combat car | 3 (1) [p2], 4 (1) [p2], 33 (1), 34 (1), 45 (1), 53 (1), 1419 (1), 1420 (1), 1421 (1), 1423 [p2], 1424 [p2], 1426 [p2], 1428 [p2], 1429 [p1], 1434 (1) |
| 78 vault | 1441 (8) |
| 79 thrown object | 13 (37), 14 (2), 26 (22), 33 (29), 34 (29), 35 (58), 53 (5), 383 (66), 384 (60), 385 (48), 386 (19), 387 (29), 388 (73), 391 (11), 394 (90), 395 (46), 396 (2), 398 (6) |
| 80 ? | 406 [p6] |
| 81 ? | 13 (129), 39 (43), 53 (105), 383 (70) [p23], 384 (58) [p23], 386 (63), 387 (43), 388 (65), 391 (19) [p2], 394 (129), 395 (129) [p29], 396 (43), 406 (132) [p27], 409 (132) [p25] |
| 82 ? | 13 (10), 53 (7), 383 (10), 384 (10), 385 (6), 386 (3), 387 (6), 388 (10), 392 (1) [p1], 394 (8), 395 (10), 406 (8), 407 (3), 410 (2) [p1], 411 (6) |
| 83 touch pad | 8 (1), 13 (54), 26 (1), 33 (23), 34 (24), 37 (18), 45 (1), 53 (5), 61 (7), 63 (2), 413 (92), 415 (6), 417 (1), 418 (77), 419 (72) [p1], 421 (78), 422 (3), 424 (11) |
| 85 laser | 13 (71), 26 (1), 448 (3), 449 (112), 450 (121), 451 (81), 452 (105), 453 (105), 454 (6), 455 (121) |
| 86 barrier | 13 (1), 26 (21), 33 (1), 34 (1), 456 (37) [p2], 457 (71) [p1], 458 (47) [p1] |
| 88 hiding barrel | 13 (7), 33 (4), 34 (4), 45 (3), 53 (3), 61 (8), 828 (4) |
| 89 water | 13 (8), 26 (1), 67 (2), 874 (52), 875 (29), 876 (2), 878 (1), 880 (53), 881 (53) |
| 91 coin | 13 (42), 32 (42), 49 (42), 891 (42), 892 (42), 893 (42), 894 (42), 902 (42) |
| 92 task item | 13 (17), 26 [p4], 49 (17), 891 (17), 892 (17), 894 (17), 898 [p30], 901 [p45], 906 [p45] |
| 93 medicine | 13 (42), 32 (42), 49 (42), 891 (42), 892 (42), 893 (42), 894 (42) |
| 94 pocket item | 32 (38), 891 (38), 892 (38), 893 (38), 907 (38), 908 (38), 909 (38) |
| 95 clue bottle | 5 (8), 8 (8), 13 (8), 892 (8) |
| 96 ? | 13 (2), 49 (3), 50 (3), 51 (3), 53 (3), 965 (3), 966 [p2], 967 [p12], 968 (3), 969 [p2], 970 [p2], 971 (2), 972 (2) |
| 97 sky dome | 30 (3) |
| 98 reactive prop | 3 (14), 4 (3), 5 (5), 7 (1), 8 (47), 13 (570), 26 (6), 30 (1), 39 (3), 49 (33), 50 (37), 51 (10), 53 (536), 61 (12), 79 (1), 80 (3), 81 (3), 1025 (199) [p4], 1030 (617), 1031 (7), 1032 (145), 1033 (155), 1034 (604), 1035 (1), 1037 (356), 1038 (58), 1039 (104), 1040 (236), 1041 (100), 1042 (400), 1043 (274), 1044 (159), 1045 (24) |
| 100 carried gadget object | 1102 (127), 1104 [p1] |
| 101 mine | 53 (42), 1102 (42), 1108 (42) |
| 102 trap | 53 (42), 1102 (42), 1113 (42) |
| 103 treasure | 13 (41), 32 (41), 53 (41), 59 (41), 60 (41), 1152 (41), 1153 (41), 1154 (41), 1155 (41), 1161 (41), 1162 (41), 1163 (41) |
| 105 ? | 26 (126), 1304 (126), 1305 (126), 1307 (126) |
| 106 ? | 26 (13), 1311 (13) |
| 107 ? | 8 (38) [p43], 1313 (23), 1314 [p4], 1316 (55), 1317 (55), 1318 (55), 1319 (55), 1320 (55), 1322 [p7], 1323 [p8] |
| 108 ? | 30 (2), 33 (2), 34 (2), 1435 (2), 1436 (2), 1437 (2) |
| 109 animation clip | 5 (5) [p1], 107 (334), 109 (11425) [p4], 110 (53), 111 (53), 112 (11517), 113 (41) |
| 111 state machine | 370 (350) |
| 112 ? | 3 (31), 4 (31), 5 (31), 8 (66) [p120], 460 (6), 462 (3), 463 (2), 466 [p40], 467 [p40] |
| 113 guard command list | 595 (7), 596 (62), 597 (62), 598 (357), 599 (104), 600 (151), 601 (196), 602 (260), 604 (89), 605 (87), 606 (172), 607 (7), 608 (6), 611 (484), 612 (56) |
| 114 sequence (dialogue) | 622 (15), 625 (795), 626 (795), 627 (250), 628 (396), 629 (316), 630 (190), 631 (122), 632 (500), 633 (364), 634 (1216), 635 (774), 636 (583), 637 (580), 638 (652), 639 (374), 640 (6), 641 (375), 642 (2016), 643 (2239), 644 (138), 645 (117), 646 (7), 647 (1533), 648 (197), 651 (36), 654 (467), 655 (24), 656 (12), 657 (97), 658 (9), 659 (65), 660 (2308), 661 (453), 662 (565), 663 (10), 665 (29), 666 (15), 667 (20), 668 (233), 669 (54), 670 (1272), 671 (1143), 672 (20), 673 (11), 674 (3), 675 (21), 676 (183), 677 (88), 678 (87), 679 (195), 680 (139), 682 (1537), 683 (1537), 684 (540), 685 (405), 686 (405), 687 (34), 688 (39), 689 (371), 690 (34), 692 (14), 693 (38), 694 (1), 695 (1767), 696 (293), 697 (857), 698 (774), 699 (8), 700 (8), 701 (8), 702 (14), 703 (8), 704 (8), 705 (51), 706 (3), 708 (763), 709 (384), 710 (6), 711 (429), 712 (74), 713 (6), 714 (6), 715 (6), 716 (6) |
| 115 ? | 945 (48), 946 (1), 947 (46), 948 (48), 949 (48), 951 (35), 952 (35), 953 (13), 954 (13), 955 (1), 956 (1), 957 (1), 958 (1), 959 (1), 960 (1), 961 (1), 962 (1) |
| 116 ? | 26 (139), 980 (141), 981 (141), 982 (141), 983 (141), 984 (141), 985 (141), 986 (141) |
| 117 noise template | 26 (10), 1090 (437), 1091 (443), 1092 (149), 1093 (484), 1094 (435) |
| 119 ? | 26 (11), 1324 (4), 1325 (27), 1326 (11), 1327 (32), 1328 (28), 1329 (2), 1330 (1), 1331 (2), 1332 (2), 1333 (15), 1334 (1), 1335 (1), 1336 (17) |
| 120 attack action | 353 (422), 354 (379) [p1], 355 (171), 356 (271), 358 (47), 359 (96) [p1], 360 (4), 361 (78), 362 (14), 363 (28), 364 (393), 365 (29) |
| 121 ? | 1344 (1), 1345 (1), 1346 (1) |
| 123 ? | 1361 (2), 1362 (2) |
| 124 ? | 1446 (2429), 1447 (1425), 1448 (1004) |
| 126 screen movie | 26 (6), 1122 (1250), 1123 (1573), 1124 (805), 1125 (129) |
| 127 ? | 1123 (41), 1133 (41), 1135 (41) |
| 128 ? | 1123 (41), 1124 (41), 1133 (41) |
| 129 ? | 1123 (212), 1150 (116), 1151 (212) |
| 130 ? | 1122 (43), 1123 (43), 1133 (43), 1134 (43), 1135 (43), 1143 (43), 1144 (43), 1145 (43), 1146 (43), 1147 (43) |
| 131 ? | 1123 (1), 1124 (1) |
| 132 job | 3 (4), 1242 (28), 1243 (388), 1244 (388), 1245 (382), 1246 (382), 1255 (17), 1257 (31), 1258 (343), 1259 (101), 1260 (388), 1261 (388), 1262 (388), 1263 (388) |
| 133 task | 1264 (3806), 1265 (807), 1271 (187), 1272 (1180), 1273 (129), 1274 (1667), 1275 (1667), 1276 (1667), 1278 (114), 1279 (353), 1287 (2184), 1288 (529), 1295 (4080), 1296 (4080), 1297 (4080), 1298 (4080), 1299 (4080) |

World option lists [D]: the two lists together hold 161 (13 worlds), 164 (28), 165 (28), 166 (42), 167 (41),
168 (42), 169 (42), 170 (1), 913, 914, 915 and 916 (43 each), 923 (15), 934 (44 occurrences), 936 (3).

## 4. Test vectors [D]

- Most used options over all bodies: 13 (13,010 occurrences), 112 (11,685), 109 (11,425), 26 (9,185), 558
  (8,325), 548 (8,285). Options 109 (value type 1, a float) and 112 (value type 5, an integer) sit on 11,425 and
  11,517 animation clips.
- Placement option lists: 4,103 patches and 1,876 fresh blocks (1,486 targeting object 5, 327 object 6, 63 other
  objects); all 5,979 parse exactly; together they hold 7,701 options.
- Body option lists at every depth: 499,496 option occurrences in 101,654 non-placement bodies.
- The guard template's row (class 58) lists 33 (28 bodies), 254 (28), 292 and 293 (27 each): every guard template
  sets hit points, and all but the intro pig set the chase ranges.

## 5. Open questions

- The meanings of the clip options 107 and 109 to 113 and most class-specific options not listed in section 2
  (the class-113 options 595 to 612 and the action options of 2.1 are now described).
- (Resolved 2026-11-18: commands 1 and 2 are the run and pause time ranges; command 6's settings are the direction and a face object; command 13's value is a reverse flag; section 2.2.)
- What the target IDs 5 and 6 of placement blocks address exactly (both root objects of the placed copy [R]).
- The roles of the "?" classes in section 3.
