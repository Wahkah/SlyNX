---
id: sly1-gameplay-interactables
title: Sly 1 buttons, trigger volumes, breakables, hazards, hiding baskets, barriers and conveyors
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds
    used_for: per-world counts (proxy clones counted), every option value quoted below, conveyor values
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files button, break, zap, water, hide, barrier, cnvo, bomb,
      mine, chkpnt, rat, spaprops (option bindings)
    used_for: behaviour, defaults and constants; marked [R]
date: 2026-11-13
---

# Interactables

Objects the player presses, enters, breaks or is hurt by. Tags as in [gameplay-overview.md](gameplay-overview.md):
**[D]** disc data, **[O]** observable, **[R]** reference behaviour to be confirmed, **[I]** inferred. Class IDs,
option numbers and event numbers are those of [object-stream.md](object-stream.md),
[option-encodings.md](option-encodings.md) and [scripts.md](scripts.md). Game units and seconds.

All of these classes use the solid layout ([classes/solid-object.md](classes/solid-object.md)) and so carry a
collision mesh and a drawable block. "Raises event n" means the object's script module gets event n
([scripts.md](scripts.md)).

## 1. Buttons (class 40)

A button is a solid object that becomes **pressed** by some kind of contact, plays its attached animations and
raises events. 232 buttons on the disc [D].

### 1.1 Kinds

Option 258 picks the kind; all 72 written button records set it [D]:

| Value | Kind | Pressed when | On the disc |
|---:|---|---|---:|
| 0 | push | the button body slides to its second stop point (its child points give the stops); "at a stop" means within 2 units [R] | 0 |
| 1 | lever | the body rotates to its second stop orientation (child transforms); within 0.1 [R] | 0 |
| 2 | contact | an allowed object touches it (any object, or only those listed by OID) [R] | 39 |
| 3 | hit | the player hits it with the cane (or any attack that reaches it) [R] | 28 |
| 4 | hang | the player hangs from the button's hook (its child hook object) [R] | 5 |

### 1.2 States and timing [R]

- States: untouched, touched, pressed, disabled, resetting.
- Untouched → touched **presses** the button: it raises event 2, plays a press sound (sound ID 2) unless option 260
  "silent" is set (52 buttons [D]), runs its "on" animations (its child animation segments, or ones added by option),
  starts its state-machine goals and dialogue (option 265), and becomes pressed. Touched → untouched raises event 3
  and runs the "off" animations.
- A pressed button returns to resetting after option 257 seconds have passed since it was pressed; a negative
  value means never. Values: −1 on 10 buttons, 4.17, 2.5 and 2.0 s on two each [D]. Resetting returns to touched
  once the body is back at its first stop (push, lever), at once for contact and hang, or to untouched for hit
  buttons.
- Option 261 restarts the animations from the start on every press (3 buttons [D]); otherwise they continue from
  where they are. Option 262 keeps the button pressed until a script resets it.
- **Off animations and goals** [R]: releasing runs the "off" animations (option 252; volumes 268). On press the
  button sets each goal record's machine (option 264; volumes 276; options-by-class.md 1.2) to its press goal, on
  release to its release goal: the root object is found by OID near the button, then the run of that machine on that
  root; a machine without a run is skipped (a record never starts a machine), and a record whose goal is −1 does
  nothing. The dialogue of option 265 is started on press and stopped on release.
- Option 263 "remembered": a pressed button stays pressed after a checkpoint restart, and its animations jump to
  their end (0 on 21 buttons, i.e. not remembered [D]). A remembered press raises event 22 instead of 2 when it is
  restored.

## 2. Trigger volumes (classes 41 and 42)

A volume (its collision mesh or a sphere) that reacts to objects **inside** it. Same states and options as buttons,
with their own option numbers: repush delay 269, silent 271, restart animations 272, remembered 273, dialogue 279
[D for the bindings' existence in data, R for meaning]. 461 class-41 and 87 class-42 volumes on the disc [D].

- Which objects count [R]: the player, unless the volume lists objects by OID (up to 8; these can be ordinary
  objects, points, transforms or the camera). An object counts when its position is inside the volume's collision
  tree, or within the sphere radius if the volume is a sphere.
- Anything inside → touched (pressed); nothing inside → untouched [R].
- Option 274 "must exit": after the repush delay the volume waits until everything has left before it can fire
  again (6 volumes [D]). Otherwise it returns to untouched after the delay.
- Option 277 "report changes": the volume raises event 12 every time the set of objects inside changes (and never
  resets itself) (7 volumes [D]).
- Repush delay values: 0 on 11 volumes, −1 on 3 [D]. Silent: 25 [D]. Dialogue on press: 51 volumes [D].
- Class 42 behaves the same; it is used where a volume tests a specific group of objects (*use not settled*) [I].
- A vault's hint dialogues are started by a trigger volume named by the vault ([pickups-and-progress.md](pickups-and-progress.md) 4.2) [R].

## 3. Other volumes (classes 123 and 124)

Convex volumes ([collision.md](collision.md) 5) with no behaviour of their own: scripts and other objects test
whether something is inside [R]. A checkpoint uses the class-123 volumes among its children as the area that
activates it (level-flow.md) [R]. Class 124 volumes are rat holes that the rats (class 65) run into [R]. 98 and 8 on
the disc [D].

## 4. Breakables (classes 33 to 39)

| Class | Kind | On the disc |
|---:|---|---:|
| 33 | breakable | 1728 |
| 34 | breakable that raises an alarm (alarms-sensors.md) | 37 |
| 35 | breakable gun ? | 12 |
| 36 | breakable variant ? | 3 |
| 37 | fragile breakable (breaks on impact) | 140 |
| 38 | electrified breakable | 18 |
| 39 | breakable piece | 1609 |

### 4.1 Breaking [R]

- A breakable absorbs attacks that reach it (cane hits, bombs, the player's rush and dive attacks; options 91 and
  115 select which), plus contacts by objects or classes it lists. A whack pushes it with the attacker's velocity
  times option 217 (default 1.0; 0.25, 0.5, 0.2 on the disc [D]).
- Each counted hit raises event 20 with the hit count. When the count reaches option 227 (hits to break, default
  1) it breaks. Values on the disc: 2 (24), 1 (11), 12 (6), 999 (5, effectively unbreakable by hits) [D].
- **Fragile breakables** (class 37) also break when an impact exceeds a force or torque limit (options 242 to 244)
  [R].
- **Breaking**: its pieces (class-39 children and other children, each marked by the breakable's rules as appearing,
  disappearing, staying or staying fixed) are released. A loose piece gets a random velocity within ±option 220
  on each axis (1000 on 17, 750 on 13, 0 on 6, 800 on 4 breakables [D]) plus its offset from the centre times a
  random factor 0.8 to 1.2 (option 219), and a random spin within ±option 221 (8 on 30 [D]). Clue bottles and
  pickups inside are set upright and freed. Then an explosion effect (option 240 names it; otherwise a standard
  smoke cloud sized by option 231 and scaled by option 232), the break sound (sound ID 42) and the coin drop of
  [pickups-and-progress.md](pickups-and-progress.md) 3.4 (option 233, at the offset of option 234).
- A broken breakable stays broken after a checkpoint restart (it is broken again silently on load) [R]. Option 216
  keeps the broken object in the world (1 [D]).
- **Pieces** (class 39) fade over option 247 seconds and disappear after option 248 seconds (1.2 on 97, 1.5 on 89,
  5.0 on 31, 2.0 on 25 pieces [D]); option 250 makes them ignore other pieces (104 [D]).

## 5. Hazards (classes 54 and 55; water, class 31)

A hazard damages the player on contact. Damage kinds (option 609; also the water's built-in kind) [R] [O]:

| Value | Kind | Class 54 | Class 55 |
|---:|---|---:|---:|
| 0 | blunt | 5 | 4 |
| 1 | electric | 10 | 7 |
| 2 | fire | 15 | 80 |
| 3 | water | — | — |
| 4 | crush | 4 | — |
| 5 | fall into a pit | — | 66 |

(Counts are records that set option 609 [D]; the default is blunt [R].)

- **Class 54** (solid hazard): damages the player when they touch its surface; also damages rats [R]. 158 on the
  disc [D].
- **Class 55** (hazard volume): damages the player when the player is inside its volume (bounding spheres overlap,
  then a real contact). A pit volume fires only while the player is falling (vertical speed ≤ 0) and only once the
  player's centre — and, for a spherical player, the whole sphere — is inside [R]. 273 on the disc [D].
- **Reach and repeat** [R]: a class-54 hazard has no reach of its own: it hurts on a real contact between its
  collision mesh and the player's (physics.md 3.2), i.e. at distance 0. A class-55 volume first compares bounding
  spheres (the volume's radius + the player's), then needs the player's origin inside the volume (or inside the
  sphere, for a spherical volume) or a real overlap of the player's mesh with the volume. There is **no repeat
  timer**: the test runs every frame, and repeated damage is prevented only by the player's own immunity rules
  (player-moves.md 7: in a damage reaction, flashing for 2 s after a charm hit, and the water/pit exceptions).
- **After damage** the player is thrown to the best of the hazard's **recovery points**: its child points and curves,
  plus objects added by OID (up to 32); the throw rises by option 208 (default 150) [R]. With option 209 the player
  is placed on the target at once instead of jumping [R]. The hazard raises event 2.
- The player's own damage handling (charm loss, lives, invulnerability while a power-up is active) is in
  player-moves.md.
- **Water** (class 31, 43 on the disc [D]): objects that enter or leave it are told so; entering downward faster
  than 100 units/s makes a splash (sound ID 34) sized by the object; spray at relative speeds above 500 [R]. Water
  carries the "water" damage kind: the player falling in loses a life [O] [R]. Buoyancy and viscosity default to 1
  [R]. Option 203 sets a current (one water on the disc: `(−175, 500, 0)` [D]).

## 6. Bombs and mines (classes 59 to 63) as hazards

- A **bomb** (class 59, 142 on the disc [D]) detonates on impact, after a delay or when primed by another
  explosion (0.1 s). Defaults [R]: detonation radius 200 (separately 200 for the player), maximum push speed 800,
  objects heavier than 5000 not pushed, delay 1 s, damage kind blunt.
- On detonation, every solid in range is pushed away from the blast (direction raised by 200 units of height),
  harder when closer and lighter, given a random spin, and hit as by an attack (breakables break); the player
  takes damage of the bomb's kind and is thrown like a hazard throw. Other non-timed bombs in range are primed
  [R].
- **Missiles** (classes 60 to 62) and the **mine** (class 63, one per world: the player's power-up, 41 [D]) use the
  same detonation; the mine is placed by the player and drifts toward targets (player-moves.md) [R].

## 7. Hiding baskets (class 58)

A basket the player can climb into and walk around in, hidden from guards [O]. 3 on the disc ("A Cunning
Disguise" 2, "A Daring Rescue" 1) [D].

- When the player gets in, the basket attaches to the player and turns with them; it raises event 2. Getting out
  detaches it (event 3) [R].
- While free it springs back upright (rotation spring and damping 3.0) [R]. The player can enter from up to 300
  units below its top [R]; the entry position is 70 units beyond the basket's flatten radius from its centre, on the
  player's side [R].
- Darts fired by guards stick in it; a limit of stuck darts can be set (option; meaning shared with guards.md) [R].

Hiding in ordinary scenery (hiding shapes and points, the player's flatten-against-wall move) belongs to
player-moves.md.

## 8. Barriers (class 52)

Invisible walls that block some objects and let others through [R]. 233 on the disc [D].

- The barrier lists up to 4 OIDs and 4 classes. Without option 346 it blocks the listed objects and lets everything
  else pass; with option 346 (set on 13 [D]) it lets only the listed ones pass [R].
- A barrier can return what hits it to a place: its single child point, or a warp named by option 347. The object is
  moved there with its velocity cleared at the next update [R]. It raises event 2 and event 29 (with the object) on
  every hit [R].

## 9. Conveyors (class 57)

A solid whose surface carries what stands on it. 51 on the disc [D].

- Belt speed defaults to 100 units/s [R]; scripts can change it. Contacts with the belt get a sideways velocity of
  the belt speed along the direction perpendicular to both the conveyor's local Y axis and the contact normal [R].
- The two floats after the solid layout ([classes/solid-object.md](classes/solid-object.md) 4) are the texture
  scroll per unit of belt speed (u, v); the conveyor's UV-scroll animation is set to −speed × each value so the
  texture moves with the belt [R]. On the disc 39 conveyors have (0, 0); the others have u = 0 and v between −0.0086
  and 0.0091 [D].

## 10. Test vectors (per world, proxy clones counted) [D]

Columns: buttons (40), trigger volumes (41), trigger volumes (42), volumes (123), rat holes (124), breakables (33),
alarm breakables (34), class 35, class 36, fragile (37), electrified (38), pieces (39), barriers (52), solid hazards
(54), hazard volumes (55), water (31), conveyors (57), baskets (58), bombs (59), missiles (60, 61, 62), mines (63).

| # | 40 | 41 | 42 | 123 | 124 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 52 | 54 | 55 | 31 | 57 | 58 | 59 | 60 | 61 | 62 | 63 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 15 | 12 | 1 | 3 | 64 | 1 | 0 | 0 | 3 | 0 | 224 | 9 | 0 | 12 | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 |
| 3 | 0 | 11 | 0 | 3 | 0 | 15 | 2 | 0 | 0 | 0 | 0 | 77 | 3 | 2 | 5 | 3 | 2 | 0 | 0 | 0 | 0 | 2 | 1 |
| 5 | 0 | 5 | 0 | 2 | 0 | 26 | 4 | 0 | 0 | 26 | 0 | 66 | 9 | 0 | 7 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| 6 | 19 | 10 | 0 | 3 | 1 | 12 | 1 | 0 | 0 | 0 | 0 | 10 | 3 | 47 | 24 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| 7 | 0 | 17 | 0 | 1 | 2 | 26 | 2 | 9 | 0 | 0 | 0 | 87 | 0 | 0 | 2 | 0 | 0 | 2 | 0 | 0 | 0 | 3 | 1 |
| 14 | 5 | 8 | 0 | 2 | 0 | 71 | 6 | 0 | 0 | 0 | 0 | 32 | 7 | 2 | 3 | 3 | 15 | 0 | 5 | 0 | 0 | 9 | 1 |
| 15 | 5 | 67 | 0 | 0 | 0 | 21 | 0 | 0 | 0 | 0 | 0 | 19 | 12 | 0 | 2 | 1 | 0 | 0 | 15 | 0 | 1 | 2 | 1 |
| 17 | 11 | 8 | 35 | 6 | 0 | 85 | 0 | 0 | 0 | 14 | 0 | 427 | 6 | 0 | 5 | 0 | 0 | 0 | 10 | 1 | 1 | 3 | 1 |
| 20 | 44 | 1 | 0 | 0 | 0 | 21 | 0 | 0 | 0 | 0 | 0 | 0 | 5 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 1 |
| 26 | 0 | 7 | 0 | 4 | 0 | 39 | 2 | 0 | 0 | 16 | 0 | 14 | 2 | 30 | 20 | 5 | 1 | 0 | 0 | 2 | 0 | 5 | 1 |
| 36 | 33 | 2 | 32 | 2 | 0 | 166 | 0 | 0 | 0 | 4 | 0 | 0 | 2 | 0 | 3 | 2 | 0 | 0 | 14 | 1 | 1 | 2 | 1 |
| 39 | 3 | 21 | 0 | 0 | 0 | 12 | 0 | 0 | 1 | 0 | 18 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 51 | 0 | 1 | 0 | 1 |
| 42 | 0 | 0 | 0 | 0 | 0 | 195 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 1 | 0 | 0 |
| **All 46** | **232** | **461** | **87** | **98** | **8** | **1728** | **37** | **12** | **3** | **140** | **18** | **1609** | **233** | **158** | **273** | **43** | **51** | **3** | **142** | **28** | **21** | **97** | **41** |

Worlds 0 ("Splash") and 2 ("The Hideout") have none of these. Other worlds' rows can be regenerated by the same walk
(object records plus the clones made by proxy back-references, classes/proxy.md).

## 11. Open questions

- The meaning of options 91 and 115 (which attacks affect an object), shared with player-moves.md.
- Classes 35, 36 and 42: what distinguishes them in play.
- Exact throw trajectory after hazard damage, and invulnerability timing (player-moves.md).
- Whether the press sound and break sound IDs (2, 42) are effect-bank IDs in the global numbering of audio.md 3.6
  (they are in the reference) and which bank they fall in.
