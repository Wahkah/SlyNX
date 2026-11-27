---
id: sly2-behaviour-interactables
title: Sly 2 interactables - pads, trigger volumes, breakable props, hiding barrels, barriers, water, thrown objects
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 USA (SCUS-97316) image; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      a scratch walk of every package of all 43 worlds: records per class, the packages that hold them, live
      instances at world start (placement multiplicity), every option ID and value on classes 9, 22, 24, 25, 27, 79,
      83, 86, 88, 89, 98; ordered option lists of individual objects (test vectors); the option table entries
      (virtual address 0x002AC880, 40-byte entries) for the options cited, read from the executable after the hash
      check; the constants 200 at 0x002BC9C8 and 500 at 0x002BC9E0
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/pad.cpp, trigger.cpp, breakable.cpp, hide.cpp, barrier.cpp,
      water.cpp, projectile.cpp, damage.cpp, activate.cpp, cls.cpp; tools/sly2_classes.py and sly2_world.py
      (earlier reference-side research)
    used_for: >-
      which class does what, state rules, timings and defaults marked [R]; every option meaning was checked
      against the option table entry it names (field offset or method) and against the values found on the disc
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (GPL-3.0; facts only), the symbol map for the class loaders
    used_for: which classes share a loader layout (cross-check only)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/interact.cpp, jt.cpp (the Circle-button choice, interaction kind 8) (2026-11-26)
    used_for: class 27 as the crawl space (section 9) [R]
  - kind: disc
    what: SCUS-97316 row 4 (`1_nightclub/03_0x03e`) walked 2026-11-26 (2,965 bodies, 0 failures); crawl space 124027, volume 133043 and their modules in `f_nightclub_disco_rig`
    used_for: section 9's class-27 example [D]
reviewed_by: pending
date: 2026-11-26
---

# Sly 2 interactables

A delta of [../sly1/interactables.md](../sly1/interactables.md). Sly 2 keeps the same ideas (things that react when
touched, hit or entered) but stores them differently: buttons became **pads**, Sly 1's many breakable classes became
one **reactive prop** class driven by an event list in its options, hiding baskets became **hiding barrels**, and
hazard volumes are gone (damage comes from contact values on objects, section 8). Class IDs are those of
[object-stream.md](object-stream.md) section 3; option IDs are stream option IDs (the index into the executable's
option table, [object-stream.md](object-stream.md) 1.1). Tags as in [../sly1/gameplay-overview.md](../sly1/gameplay-overview.md).

What is **not** here: lasers and alarms ([alarms-sensors.md](alarms-sensors.md)), the pickpocket item dispenser (class
99, `pickpocket_item_dispenser`) and pickups ([pickpocket-and-pickups.md](pickpocket-and-pickups.md)), gadget objects
(classes 100 to 102, [gadgets.md](gadgets.md)), mission waypoints (class 107, [missions.md](missions.md)).

## 1. Counts

Records are object records in package object lists, top-level and nested; "live" counts every record once per
instance of its package that exists when the world starts (the package's own copy when it is kept in place, plus one
per placement in a live package, recursively) [D]. Objects that scripts create later (thrown ammo, for example) are
not live at start [D]. Records nested inside other solid bodies are not multiplied [I: a lower bound for those].

| Class | Role (ours) | Records | Live at start, all worlds | Packages (most common) [D] |
|---:|---|---:|---:|---|
| 98 | reactive prop (section 3) | 574 top + 43 nested | 4,071 | `i_palace_ext_bombbridge` 17, `i_barrel_single` 10, `c_fragile_bottle` 8, `c_chair` 8, `p_wolf_statue` 7, `p_armor` 7 |
| 83 | pad (section 2) | 68 + 24 | 240 | `c_sawmill_bearcave` 14, `p_bouncer_neela` 9, `a_bouncer_bellows` 5, `c_bouncer_tarp` 4, `i_basket_bouncer` 3, `jb_seesaw` |
| 24 | trigger volume (section 4) | 320 | 333 | `p_castle_ext_heist` 27, `jb_intro_rig` 16, `f_nightclub_cellar_rig` 13 |
| 25 | trigger volume (same rules) | 2 | — | `f_nightclub_heist_rig`, `i_temple_int_recon` |
| 86 | barrier (section 6) | 51 + 58 | 49 | `jb_intro` 8, `jb_intro_chase` 6 |
| 88 | hiding barrel (section 5) | 8 | 41 | `i_tnt_barrel` 2, `c_barrel_hide` 2, `disguise_barrel`, `disguise_barrel_static`, `p_hide_barrel_static`, `a_tnt_barrel` |
| 89 | water (section 7) | 41 + 17 | 45 | `i_palace_ext` 7, `p_castle_ext` 6, `c_train_ext` 4 |
| 79 | thrown object (section 8) | 81 + 19 | 7 | `rock_throw_ammo` 29, `bomb_throw_ammo` 11, `c_bison_bomb` 6, `pipe_bomb_throw_ammo` 5 |
| 27 | volume naming an object (section 9) | 38 | 38 | `p_prison_ext_rig` 5, `f_nightclub_cellar_rig` 4 |
| 22 | plain volume (section 9) | 13 | 13 | `f_nightclub_heist_rig` 8 |
| 9 | climbing surface (section 9) | 8 | 7 | `c_sawmill_ext` 5, `c_sawmill_bearcave` 2, `c_iceberg` 1 |

Live instances per world (rows of the level table, [disc.md](disc.md) 6) [D]:

| Row | Level package | 98 | 83 | 24 | 86 | 88 | 89 | 27 | 22 | 9 | 79 |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | `jb_intro` | 152 | 6 | 27 | 19 | 0 | 4 | 0 | 0 | 0 | 0 |
| 1 | `dvd_menu` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 2 | `f_nightclub_exterior` | 243 | 15 | 11 | 3 | 0 | 2 | 0 | 2 | 0 | 0 |
| 3 | `f_nightclub_cellar` | 179 | 0 | 13 | 0 | 0 | 0 | 4 | 0 | 0 | 0 |
| 4 | `f_nightclub_disco` | 168 | 5 | 9 | 0 | 0 | 2 | 1 | 0 | 0 | 0 |
| 5 | `f_nightclub_heist` | 81 | 4 | 2 | 0 | 0 | 0 | 1 | 8 | 0 | 0 |
| 6 | `f_nightclub_moulinrouge` | 122 | 8 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 7 | `f_nightclub_waterpump` | 57 | 0 | 2 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 8 | `i_palace_ext` | 167 | 56 | 20 | 3 | 0 | 11 | 4 | 0 | 0 | 0 |
| 9 | `i_palace_int` | 221 | 4 | 9 | 0 | 5 | 0 | 0 | 0 | 0 | 0 |
| 10 | `i_palace_basement` | 59 | 1 | 3 | 3 | 0 | 1 | 2 | 0 | 0 | 0 |
| 11 | `i_palace_ballroom` | 36 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 12 | `i_temple_ext` | 103 | 16 | 21 | 1 | 0 | 1 | 2 | 0 | 0 | 1 |
| 13 | `i_temple_int` | 103 | 1 | 17 | 1 | 0 | 1 | 0 | 0 | 0 | 0 |
| 14 | `p_prison_ext` | 133 | 17 | 32 | 4 | 24 | 1 | 5 | 0 | 0 | 1 |
| 15 | `p_prison_int` | 327 | 4 | 7 | 2 | 0 | 0 | 1 | 0 | 0 | 0 |
| 16 | `p_prison_vault` | 2 | 0 | 1 | 0 | 0 | 0 | 2 | 0 | 0 | 0 |
| 17 | `p_castle_ext` | 351 | 9 | 35 | 1 | 0 | 6 | 3 | 0 | 0 | 0 |
| 19 | `p_castle_waterrails` | 2 | 0 | 1 | 1 | 0 | 3 | 0 | 0 | 0 | 0 |
| 20 | `p_castle_guardbreak` | 19 | 0 | 4 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 21 | `p_castle_wolftomb` | 25 | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| 22 | `p_castle_heist` | 26 | 4 | 4 | 0 | 0 | 1 | 0 | 1 | 0 | 0 |
| 23 | `p_castle_ewoktrainer` | 9 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 24 | `p_castle_ewoktomb` | 7 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 25 | `p_castle_ewokhall` | 6 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 26 | `p_castle_ewokwater` | 4 | 0 | 2 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| 27 | `c_train_ext` | 151 | 24 | 20 | 6 | 0 | 4 | 0 | 1 | 0 | 1 |
| 28 | `c_train_cabins` | 120 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 29 | `c_train_a` | 102 | 0 | 10 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 30 | `c_train_b` | 92 | 0 | 10 | 1 | 0 | 0 | 3 | 0 | 0 | 0 |
| 31 | `c_train_c` | 78 | 2 | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 32 | `c_sawmill_ext` | 127 | 31 | 37 | 1 | 9 | 2 | 4 | 0 | 5 | 0 |
| 33 | `c_sawmill_mulch` | 60 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| 34 | `c_sawmill_burn` | 19 | 3 | 2 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| 35 | `c_sawmill_lighthouse` | 154 | 0 | 5 | 0 | 0 | 0 | 2 | 1 | 0 | 0 |
| 36 | `c_sawmill_bearcave` | 0 | 14 | 1 | 0 | 0 | 0 | 2 | 0 | 2 | 0 |
| 37 | `c_sawmill_boss` | 3 | 0 | 0 | 1 | 0 | 3 | 0 | 0 | 0 | 0 |
| 38 | `a_blimp_ext` | 265 | 6 | 5 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| 39 | `a_blimp_int` | 99 | 2 | 1 | 1 | 2 | 0 | 2 | 0 | 0 | 0 |
| 40 | `a_blimp_engine_room_murray` | 79 | 2 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 41 | `a_blimp_engine_room_bentley` | 19 | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 42 | `a_blimp_engine_room_jt` | 16 | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 43 | `a_blimp_boss_final` | 85 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |

Option 13 (the activation radius, [clock-and-loop.md](clock-and-loop.md)) is set on most of these: on pads 6,000
(29 records), 4,000 (15), 7,000 / 5,000 / 3,000 (3 each); on reactive props 4,000 in the examples below [D].

## 2. Pads (class 83)

Sly 1's buttons (class 40) became **pads** [D for the class, R for the role]: solid objects that react when an
object stands on them or hits them. On the disc they are mostly launch pads (packages `…_bouncer_…`, `jb_seesaw`,
`i_buoy_lillypad`, the bear-cave pads) [D].

### 2.1 Options [D: IDs, fields and values; R: meanings]

| Option | Meaning | Default | Values on the disc |
|---:|---|---|---|
| 413 | **kind**: 0 touched by a listed object; 1 hit once; 2 hit toggles on and off; 3 the player using the pad's action object (option 414) | 0 | 0 (87 records), 1 (5) |
| 417 | object IDs that count (up to 8; none = any) | — | one record: object 9 (Murray) |
| 418 | classes that count, derived classes included (up to 8; none = any) | — | class 52 and derived (the three playable characters, 53 to 55) on 77 records |
| 419 | re-arm time in seconds | 1.0 | 0.25 (59), 4.0 (6), 0.1 (3), 1.0 (3), 15.0 (1) |
| 421 | no sound on trigger | 0 | 1 on 78 records |
| 422 | (an unknown flag, field +0x1F0) | 0 | 1 (3) |
| 423 | stays triggered (does not move on to the hold state) | 0 | not used |
| 415 | animations started on trigger / release (object reference, flag) | — | 6 records |
| 424 | state-machine goals on trigger / release: (trigger or release, root object, state machine, goal) | — | 16 records |

### 2.2 States [R]

| State | Meaning |
|---:|---|
| 0 | idle |
| 1 | triggered |
| 2 | holding |
| 4 | re-arming |

- **Kind 0:** each frame the pad looks for a contact with an object that passes its lists. A contact makes it
  triggered (state 1). Contacts are ignored from an object that is in its "bounced" jump state, or that bounced off
  something less than 0.15 s ago [R].
- **Kinds 1 and 2:** a hit (an attack contact) from an object that passes the lists triggers the pad from idle. Kind 2
  toggles: a hit while triggered returns it to idle, and a hit while idle triggers it again only once the re-arm time
  has passed since the last change [R]. The cane-hit bells are kind 2 [R]; the disc has none, only kinds 0 and 1 [D].
- **Entering state 1 from 0** (the trigger actions): script event 5 on the pad; sound 2 at the pad unless option 421
  is set; the trigger list (animations of option 415, goals of option 424, task-state requests); then state 2 unless
  "stays" is set; the pad's listeners are told [R].
- **State 2** lasts the re-arm time (it is renewed while the player keeps using the pad's action object), then state
  4. From 4, kind 1 returns to idle; the others go back to state 1 silently [R]. A kind-0 pad in state 1 with nobody
  on it goes to 0, which runs the **release actions**: script event 6 and the release list [R].
- A **launch pad** is a kind-0 pad whose script (event 5) launches the toucher toward a target point stored in the
  pad's script after 0.05 s [R].

### 2.3 Launching [R]

The launch velocity from the toucher's feet to a target point, for an elevation parameter `g` (0 to 1) and the
toucher's gravity `a` (default (0, 0, −2275), [physics.md](physics.md)):

- `e = max(0, atan2(Δz, d))`, where `d` is the horizontal distance and `Δz` the height difference;
- launch angle `θ = e + (π/2 − e) · sin g`;
- `q = a_z / (2 (Δz − d tan θ))`; no launch if `q ≤ 0`;
- speed `= d · √q / cos θ`, along the horizontal direction to the target raised by `θ`.

The toucher is put in its "launched" jump state with air control toward the horizontal part of that velocity, with a
weight given by the script [R].

## 3. Reactive props (class 98)

Most props (chairs, crates, pots, statues, bottles, barrels, fence posts, control panels, the newspaper boxes) are
class 98 [D]. Sly 1's breakable classes 33 to 39 have no counterpart; what a prop does when hit is described by an
**event list** built from its options [D for the options, R for the behaviour].

### 3.1 The event list

The options are read in order and build the list; an item belongs to the event that is current when it is read [R].

| Option | Adds | Data [D] |
|---:|---|---|
| 1025 | a **trigger class**: objects of this class (or derived) that set the prop off by touching it (on/off flag) | class, flag. Values: (0, off) 159, (56 guards, on) 158, (101 mine, off) 12, (0, on) 11, (52 playable characters, on) 9 |
| 1030 | a **new event**, which becomes current | event ID: −1 on 891 of 935 |
| 1031 | the **every-event** entry (its items run with every hit) | — (7 records) |
| 1032 | the current event's **hit count** | −1 (139), 25, 3, 7, 5, 2 |
| 1039 | item: add an object to the world | object ID |
| 1042 | item: remove an object (−1: the prop itself) | object ID |
| 1040, 1043 | item: show / hide a mesh group | group ID |
| 1041, 1044 | item: turn a collision group on / off | group ID |
| 1033 | item: the **source** object for the next explosion or drop | object ID |
| 1034 | item: **explode** an explosion object (class 31 or a class with its layout) | object ID; 1,260 uses |
| 1035 | item: camera shake | 1 use |
| 1036 | item: controller rumble | not used |
| 1037 | item: **drops** (pickups) | −1 (369: the game's default drop), 0 (13: none), 1 (6) |
| 1038 | item: script event 4 on the prop, with a value | 85 uses |
| 1045 | item: request a task state (task, state) | 41 uses |

Show/hide and collision items read **before any event** apply at once, setting up the prop's intact look [R]. A prop
holds at most 32 events of at most 64 items each [R].

### 3.2 Hits [R]

- **Default contact bytes** (added 2026-11-26) [D]. A class-98 prop's init (`0x001BAB50`) sets the response byte of
  every hit kind to **8** (a hit), then kinds `0x80001` (bits 0 and 19) to 0 (calls of `0x001F7918` at `0x001BAB70` and
  `0x001BAB84`). So a prop that no script has sealed can be broken by any attack. Scripts change the bytes with script
  ID 125 (characters.md 2.3).

- A **hit** is an attack contact on the prop, or a touch by an object of an active trigger class.
- A hit runs the current event's items, then counts toward the event's hit count; when the count is reached, the next
  event becomes current. After the last event the prop raises script event 5 and tells the tasks (task event 0x11).
- An event with a hit count of −1 (or 0) never advances, and its show/hide, add/remove and collision items are not
  run: it only plays its explosions, drops and other effects on every hit (a prop that clanks after it is wrecked).
- Each hit makes **noise** from the fixed noise template 706 (radius 1,500, [guards.md](guards.md)) at the prop,
  unless a guard hit it.
- **Drops, camera shake and rumble** happen only for hits by the current player character, by certain hit kinds, or
  (drops) by gadget hits; a guard knocking a prop over drops nothing.
- Events have IDs: the special IDs 1,545 and 1,546 mean "the first event" and "past the last". Scripts can move a prop
  to any event; moving backwards undoes the items in reverse.
- A copied prop starts in its source's state at the moment the copy is made.

### 3.3 Examples [D]

The trash can of world row 2 (package `f_trash_can`, object 131450, body at `0x6D243`, 19,479 bytes):

1. Before any event: hide mesh groups `0x20193` and `0x2018F`, turn off collision groups `0x20191` and `0x2018D`.
2. Event 1: explode `0x20186`; hide group `0x20197`, collision off `0x20195`; show group `0x20193`, collision on
   `0x20191`; drop the default pickups.
3. Event 2: explode `0x20186`, `0x20181`, `0x2018B`; hide `0x20193`, collision off `0x20191`; show `0x2018F`, collision
   on `0x2018D`; drop.
4. Event 3: explode `0x20186`, `0x20181`; hit count −1 (it rattles forever).

So the can goes intact → dented → wrecked, dropping pickups at each step [I].

The money crate (row 2, package `money_crate`, object 95360, body at `0xEBE93`): one event: explode `0x200E`, remove
itself, drop the default pickups [D].

## 4. Trigger volumes (classes 24 and 25)

Sly 1's trigger volumes (classes 41, 42). The volume's shape is its collision mesh and tree
([classes/paths-and-volumes.md](classes/paths-and-volumes.md) 6) [D].

| Option | Meaning | Values on the disc [D] |
|---:|---|---|
| 432 | object IDs that count (up to 8) | 7 (Sly, 56 records), 8 (Bentley, 17), 9 (Murray, 15), others |
| 433 | classes that count | 52 playable characters (61), 53 Sly (14), 54 Murray (14), 57 (7), 56 guards (7), 70 (6) |
| 434 | hold time in seconds (−1: never) | not used: always the default 1.0 |
| 436 | no sound | not used |
| 438 | stays in state 1 | 1 (2 records) |
| 439 | state-machine goals on enter / leave | 3 records |
| 440 | task-state requests on enter (task, state; up to 4) | 77 records |
| 441 | script object started on enter | 39 records |
| 443 | **tracking**: keeps the ordered set of members inside | 1 (16 records) |
| 26 | removed at world start (scripts add it later) | 71 records |

Behaviour [R]:

- **States** 0 (empty), 1 (occupied), 2 (holding). Entering with a counting object: script event 5, the tasks' event
  0x1A, sound 2 (unless no sound), the enter list (animations, goals, task requests, script start), then state 2. After
  the hold time it returns to 1 and checks again; when nothing counting is inside it goes to 0: script event 6, the
  tasks' event 0x1B, the leave list.
- A **tracking** volume stays held while any member is inside and raises script event 9 whenever its set of members
  changes.
- Removing a volume with something inside first runs its leave actions.
- Scripts read the state, the first object found inside and the member list through the volume's script options
  528, 534 and 533 ([scripts.md](scripts.md) script option IDs, not stream option IDs).

## 5. Hiding barrels (class 88)

Sly 1's hiding baskets (class 58). Packages: `c_barrel_hide`, `disguise_barrel`, `disguise_barrel_static`,
`p_hide_barrel_static`, and the TNT barrels `i_tnt_barrel`, `a_tnt_barrel` [D]. 41 live at start, 24 of them in the
prison hub [D].

- The player gets into a barrel and moves with it on (the barrel follows its holder), unless option 828 is 0: then it
  stays put [R]. Option 828 = 0 on 4 records (the `…_static` ones) [D].
- While a character is inside, or jumping in or out, the barrel does not collide with it [R].
- A followed barrel hangs from its holder's anchor point, kept level about the vertical; free barrels are pulled level
  by a spring (constants 0, 45, 4.5) with angular damping (0, 7.5, 0.75) [R].
- **Hit while standing:** it makes noise (template 706), closes the holder's binocucom, puts the holder back on the
  ground, falls over (state 3) and raises script event 11 [R]. A hit on a hiding player is passed to the barrel
  ([characters.md](characters.md)).
- Default barrel height 300 [R].

## 6. Barriers (class 86)

Sly 1's barriers (class 52): solid objects whose collision applies only to some objects (the intro's walls Murray
walks through, the guards' fences) [R].

| Option | Meaning | Values [D] |
|---:|---|---|
| 456 | object IDs in the list | 42 uses |
| 457 | classes in the list (up to 4 count) | 52 (36), 56 (10), 57 (7), 51 (5), 54 (4), 53 (3) |
| 458 | mode | 0 (33), 1 (14) |

- **Mode 0:** objects in the lists collide with the barrier; everything else passes through [R].
- **Mode 1:** objects in the lists pass through; everything else collides as usual [R].
- A barrier is its own physics root, so its collision parts never join a parent's [R].
- Each object in contact (up to 16) is reported to the barrier's script by event 5, readable through script option
  534 [R].

## 7. Water (class 89)

| Option | Meaning | Values [D] |
|---:|---|---|
| 878 | flow velocity of the water | one use: (0, −150, 0) |
| 874, 875 | splash explosions (object IDs) | 52 / 29 uses |
| 880, 881 | wake emitter, ripple object | 53 each |
| 876 | contact value given to objects in it | 0 (1), −1000 (1) |

Behaviour [R]:

- The water's collision parts are sensors. An object with a **buoyancy** (its option 59; drag option 60) that
  overlaps it floats in proportion to how much of its height is under the surface, is dragged toward the flow, damped
  in rotation and turned upright.
- An object entering while falling faster than 100 units/s **splashes**: sound 19 and the splash explosion where it
  crossed the surface.
- Floating objects make a wake: sounds 75 to 78 by relative speed and the wake emitter.
- The contact value −1000 on one water is a lethal contact for the characters [I].

## 8. Thrown objects (class 79) and damage by contact

Class 79 is everything thrown at a target that hurts on impact: rocks, bombs, pipe bombs, the chopper's bombs [D]
(classes 81 and 82, darts, bullets and missiles, use the same rules [R]). They are created by scripts and launchers,
never live at start except 7 [D].

| Option | Meaning | Default | Values [D] |
|---:|---|---|---|
| 383 | impact radius against other objects | 200 (at `0x002BC9C8`, [D]) | 400 (18), 300 (16), 100 (9), 200, 600 (4 each), 5 (4) |
| 384 | impact radius against the playable characters | 200 | 200 (16), 0 (12), 160 (8), 300 (8), 400 (7), 100 (6) |
| 385 | flight kind: 0 ends on contact; 1 sticks to what it hits | 0 | 1 (48) |
| 386 | orientation at impact: 0 its z axis to the surface normal, 1 facing its velocity, 2 its x axis to the normal, 3 kept | 0 | 3 (19) |
| 387 | no hits at the end (a dud or a decoration) | 0 | 1 (29) |
| 388 | burst density of the impact explosion | 0 | 1.0 (68), 0.5 (4), 2.0 (1) |
| 391 | contact filter of the hits | `0x100` | `0x200` (6), `0x100` (5) |
| 394 | owner (whose burst plays at the end) | — | 90 uses |
| 395 | **contact value** of the hits | −10 | −20 (22), −60 (7), −10 (5), −5 (5), −100 (2), −500 (2) |
| 396 | noise at impact | 1 | 0 (2) |
| 398 | sticks only to some contacts | 0 | 1 (6) |

At the end of its flight [R]: every object whose collision touches a sphere of the impact radius (the player
radius for the playable characters) gets a contact message with the filter and the contact value, and a push of 500
away from the impact (500 at `0x002BC9E0`, [D]); the owner's explosion plays; the fixed noise 709 sounds at the impact
unless option 396 is 0; the object leaves the world and raises script event 5. A hit by another thrown object ends a
flight 0.25 s later.

**Contact values and damage.** Sly 2 has no hazard-volume class: damage is the negative **contact value** that an
object hands to whatever it hits or holds (thrown objects, option 395; water, option 876; guard attacks,
[guards.md](guards.md)) [D for the values, R for the role]. How a value becomes health loss is in
[characters.md](characters.md) [I: −10 is one ordinary hit].

**Falling into hazards.** A character that lands somewhere unsafe is returned to its last safe ground: by a jump when
the jump would take at most 1.25 s, otherwise by moving it there at once with a camera cut and a fade [R].

## 9. Other classes

- **Class 27** (38 records, one per level rig on average): a volume whose only data is option 1182, an object ID (all
  38), and option 1189 on 3 [D]. **It is a crawl space** (added 2026-11-26) [R unless marked]:
  - **Entering.** When the player presses Circle, the Circle-button choice goes through the tracked targets within
    250 units. A class-27 volume (or a derived class) starts **interaction kind 8, the crawl**.
  - **Event 5.** The crawl starts in state 7 (crouching in). Entering that state stops the character, plays the
    crouch-in clip and raises **script event 5** on the volume. That also broadcasts task event 26 with the volume.
  - **Crawling.** In the crawl the stick moves the character forward, back and sideways at the crawl clips' speeds.
  - **Event 6.** The crawl ends when the character is no longer covered and has room to stand: state 8, standing up.
    That state raises **script event 6** on the volume (task event 27).
  - So events 5 and 6 come from the player's own crawl, not from entering the shape or a timer.
  - Example (row 4, `f_nightclub_disco_rig`) [D]: crawl space 124027's module 9 adds volume 133043 on event 5 and
    removes it on event 6. The volume therefore exists only while Sly is crawling there (missions.md 4.3).
- **Class 22** (13 records): plain volumes with no options [D]; read by scripts as shapes (a script asks whether a point
  is inside) [R].
- **Class 9** (8 records in 3 worlds): climbing surfaces Sly climbs on (a dedicated state, [characters.md](characters.md))
  [R]; layout in [classes/paths-and-volumes.md](classes/paths-and-volumes.md) 4 [D].
- **Dispensers.** No class dispenses objects. A "dispenser" (the newspaper boxes, the TNT barrel drop) is a prop whose
  script places a fresh copy of a package when the previous copy is used up; the copy and the dispenser's script tell
  each other about themselves with script event 14 when the copy is made [R].

## 10. Test vectors [D]

- **Pad:** world row 2, package `p_bouncer_neela`, object 8448, body at `0x5CE0D3`, 6,055 bytes. Its options, in order:
  413 = 0 (touched), 418 = class 52, 419 = 0.25 s, 421 = 1 (no sound), 13 = 6,000.
- **Reactive props:** the trash can and the money crate of section 3.3.
- **Option table entries** (each 40 bytes at `0x002AC880 + 40 × ID`; word 2 is the field offset or loading method,
  word 7 a setter):

  | Option | Words 0–2 | Word 7 |
  |---:|---|---|
  | 413 | `0000101a 00000082 000002a8` | `00000000` |
  | 419 | `00000001 00000092 000001e8` | `00000000` |
  | 1030 | `00000012 00000400 0010c8c8` | `00000000` |
  | 1034 | `00000006 00000100 00000000` | `001bba90` |
  | 383 | `00000001 00000092 0000015c` | `00000000` |
  | 395 | `00000005 00000092 000001f0` | `00000000` |

- **Totals live at start:** 4,071 reactive props, 333 trigger volumes, 240 pads, 49 barriers, 45 waters, 41 hiding
  barrels.

## 11. Open questions

- Option 422 on pads, option 1189 and the role of class 27, and the "unknown flag" fields of volumes (436 to 438).
- The exact rule that decides which hits produce drops, shakes and rumble (the hit kinds involved).
- Whether contact value −10 is one unit of health, and what −1000 does (instant defeat is assumed).
- The camera-shake and rumble strengths of props.
- The launch pads' target points and elevation values live in their scripts; they are not decoded here.
- Live counts do not multiply records nested inside other solid bodies, and ignore per-level conditions on records.
- Every [R] statement needs checking in play.
