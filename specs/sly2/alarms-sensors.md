---
id: sly2-behaviour-alarms-sensors
title: Sly 2 alarms and sensors (delta of Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; all 43 worlds; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73
    used_for: >-
      class counts and option values of lasers (class 85), touch pads (class 83) and stand-alone detectors (class 16),
      from a walk that reads every object body of every world to its declared size (121,153 bodies at every depth, 0 failures; worlds.md 5) and
      logs every option list; executable constants read after the hash check
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/laser.cpp, pad.cpp, trigger.cpp, detect.cpp, ainoise.cpp, aijobs.cpp, damage.cpp, scpt.cpp, gadget.cpp
    used_for: behaviour of beams, pads, stand-alone detectors and alarms; marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/alarm-button (touch-pad loader and set-up)
    used_for: confirmation that the touch pad's body is the solid layout plus run-time state only
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 alarms and sensors

A delta of [../sly1/alarms-sensors.md](../sly1/alarms-sensors.md). Tags as in
[gameplay-overview.md](gameplay-overview.md): [D] disc data, [O] observable, [R] reference behaviour, [I] inferred.
Class IDs are those of [object-stream.md](object-stream.md) section 3; option numbers are stream option IDs.

## 1. What changed from Sly 1

| Sly 1 | Sly 2 |
|---|---|
| Alarm objects (class 47) with states, sirens and links (Sly 1 section 2) | **No alarm object.** An alarm is a *noise* raised by a guard about its target ([guards.md](guards.md) 4.3), plus numbered alarm flags that script objects keep for their own command lists (section 5) [D for the absence of an alarm class, R for the rest] |
| Alarm boxes (class 34, Sly 1 section 7) | none [D] |
| Laser sensors (class 49): beams along curve shapes, switched on and off | **Laser beams (class 85)**: up to 64 beams per object, each between two moving end points (section 2) [D] [R] |
| Searchlight sensors (class 50) | **Stand-alone detectors (class 16)**: a view volume watching the three playable characters; what happens when they see someone is up to the level's scripts (section 4) [D] [R] |
| Pressure plates (class 51) | **Touch pads (class 83)**: plates, bells and bounce pads (section 3) [D] [R] |
| Alarm raises music intensity 107 (Sly 1 2.2) | The alert music layer plays while a guard chases a target or a script raises the alert counter ([audio.md](audio.md), music layers) [R] |

What sets off an alarm in Sly 2 is always a guard (or a scripted guard-like object) that sees the player: lasers and
pads hurt or trigger, and scripts decide whether that also calls guards [R] [I].

## 2. Laser beams (class 85)

Solid layout [D]. On the disc: 102 distinct objects (by world and object ID) in 15 worlds (rows 0, 3, 4, 7, 10, 13,
15, 29, 30, 31, 32, 34, 38, 42, 43; most in row 13: 21) [D].

### 2.1 Beams

Each beam is one occurrence of option **455** on the object, a typed argument list of 10 values [D]:

| Argument | Type | Meaning |
|---:|---|---|
| 1 | object reference | first end point's anchor |
| 2, 3, 4 | `f32` × 3 | first end: start fraction (0..1), speed, period |
| 5 | `s32` | first end: motion mode |
| 6 | object reference | second end point's anchor |
| 7, 8, 9 | `f32` × 3 | second end: start fraction, speed, period |
| 10 | `s32` | second end: motion mode |

On the disc: 390 beams; beams per object 1 (39 objects), 3 (16), 5 (17), up to 16 (4) [D]; motion modes **16**
(570 ends) and **8** (210 ends) [D]. A beam beyond the 64th of an object is ignored [R].

**End points** [R]:

- If the anchor is a curve (classes 10 to 14), the end moves along it. The speed and period are related by the curve
  length *L*: a non-zero period sets speed = *L* / period; otherwise a non-zero speed sets period = *L* / speed. A
  negative speed runs the curve backwards. If both are 0, the end stays at start fraction × *L* along the curve;
  that is the common case on the disc (for example 137 ends at fraction 0 and 73 at fraction 1 with speed and
  period 0; periods that occur include 6.0, 3.0, 2.0 and 0.9 s) [D].
- Mode 8: the end runs from one end of the curve to the other in one period and jumps back (a loop). Mode 0 (not on
  the disc): the same with a stationary pause record. Mode 16: back and forth with eased turns at both ends: the
  turn uses a radius *r* = 100 units (executable `0x2C3610` = 100.0 [D]) when *L* ≥ 300, else *r* = 0.2 *L* [R];
  the time is split so that each turn takes (2*r* × period) / (*L* + 2*r*), the straight runs are at constant speed
  *v* = (*L* − 2*r*) / straight time, and the turns accelerate at *v*² / (2*r*). Any other mode value is treated as 8.
- If the anchor is a transform marker or a solid object, the end is fixed at that object's position. With no anchor,
  the end is at the laser object's centre.
- An object-wide speed multiplier (a float on the object, default 1) scales every end's progress [R]; option 454
  (4.0 on 6 objects [D]) is a candidate *[I]*.

### 2.2 What a beam touches

- The beam is a segment between its two ends. Each frame it is tested against solid objects; the nearest hit cuts
  the drawn beam at the hit point [R].
- Which objects count: option **449** lists classes (up to 7; on the disc class 51 in 112 entries and class 88,
  barrels, in 8) and option **448** lists objects (up to 7; 3 entries) [D]. The playable characters belong to the
  class-51 family [R]. Objects that are not listed only block the beam [R].
- A listed object touching a beam receives a **hit of 10 units with the stun hit kind** ([characters.md](characters.md)
  damage and reactions: kind `0x1000`, "stunned, stops dead") [R]. With the gang's capacity of 40, four touches
  defeat a character at no difficulty help; the difficulty help scales the 10 down to 5 ([missions.md](missions.md))
  [R].
- A beam also cuts across the two supporting triangles under a walking character's feet, so jumping over a low
  beam is the only way past it [R].
- Options **450, 451, 452, 453** each name an object of the laser's own package (for example `0x2C21`, `0x2C31`,
  `0x2C29`, `0x2C23` in 103 of the lasers) [D]; *probably the beam's sound and effect objects [I]*.

## 3. Touch pads (class 83)

Solid layout; no fields beyond it [D]. On the disc: 87 distinct objects in 30 worlds (most in row 36: 14) [D]. A
touch pad has two states, *idle* (0) and *pressed* (1) [R].

| Option | Meaning | Disc values [D] |
|---:|---|---|
| 413 | how it is pressed: 0 by contact with an object of its lists; 1 by a hit; 2 a hit toggles it, at most once per re-arm time; 3 by the player's state | 0 on most, 1 on 5 |
| 417 | an object that may press it (up to 8) | 1 entry |
| 418 | a class that may press it (up to 8) | class 52 (the playable characters' family) on 77 |
| 419 | re-arm time, seconds (default 1) | 0.25 (59), 4.0 (6), 0.1 (3), 1.0 (3), 15 (1) |
| 421 | no press sound | 1 on 78 |
| 422, 424, 415 | flags and object links of the press actions | 3, 8, 6 entries |
| 423 | stays pressed | not used |
| 13 | activation radius ([clock-and-loop.md](clock-and-loop.md)) | 6,000 (29), 4,000 (15), others |

Behaviour [R]:

- **Contact pads (kind 0):** while a listed object stands on the pad it is pressed; when nothing listed touches it
  any more it returns to idle. A character that has just bounced off it (within 0.15 s of a bounce jump) or is in
  a falling state does not count.
- **Hit pads (kinds 1, 2):** a hit from a listed object (a cane strike) presses it; kind 2 releases on the next hit,
  no sooner than the re-arm time after the last change (bells, switches).
- **Pressing** raises script event 5 on the pad, plays press sound 2 unless option 421 is set, applies the pad's
  "pressed" animations and sends the pad's owner message 6. **Releasing** raises script event 6, applies the
  "released" animations and sends message 7.
- **Bounce pads** are touch pads whose script, on event 5, launches the toucher toward a target point on a
  ballistic arc after 0.05 s (the intro has two) [R].

## 4. Stand-alone detectors (class 16)

Header plus options; only nested (inside other objects' bodies) [D]. On the disc: 21 distinct objects in 13 worlds
(rows 6, 8, 12, 13, 14, 15, 17, 27, 29, 33, 34, 35, 38) [D]. They use the same detector as the guards
([guards.md](guards.md) 3), without a guard:

| Option | Meaning | Disc values [D] |
|---:|---|---|
| 345 | view volume (a class-22 volume) | e.g. object `0x2176` on 11 |
| 346 | second view volume | usually the same object |
| 349 | a watched target (repeatable) | objects 7, 9 and 8 (Sly, Murray, Bentley) on every detector |
| 342 | detector flags | 8 (3), 12 (1) |
| 343, 344 | always sees / never sees | not set on the disc |

Behaviour [R]:

- Each frame every watched target gets a state (unseen, seen, lost) by the guard rules of [guards.md](guards.md) 3
  (inside the view volume, not hidden, not behind a smoke cloud, a clear line from the eye). The detector's own
  object tree never blocks its sight (a searchlight's lamp does not hide what it looks at).
- Every state change raises script event 28 on the detector with three arguments: the target, the old state and
  the new state. The level's script decides what follows (raise an alarm, fail the task, call guards) [R] [I].
- A target that a script marks as unseen (an actor option) is never seen, for example a character riding in a
  bucket [R].

## 5. Alarms

- **Guard alarms:** a guard pursuing a target raises an alarm noise at its own position at most once per second
  ([guards.md](guards.md) 4.3; repeat time 1.0 at `0x2C3DB8` [D]). Other guards whose detectors watch the same target
  hear it and come [R]. The **Smoke Bomb** removes the alarms already raised by the guards it catches and holds back
  new ones until 2.0 s later (executable `0x2BCC94` = 2.0 [D]; [gadgets.md](gadgets.md)) [R].
- **Proximity escorts:** a guard running a "keep near the player" script job has three states (player near, player
  away, alarm). Entering the alarm state sends task event 51 with the guard, so the job's tasks can react
  ([missions.md](missions.md)) [R].
- **Script alarm flags:** a script object keeps up to 8 numbered alarm flags; its command lists set, test and clear
  them (one command kind sets a flag on a nearby script object) [R]. They are bookkeeping for the level's scripts,
  not a sensor.
- **Seen by a guard's detector:** when a guard's detector first sees a target, the tasks receive event 19 with the
  guard [R].
- **Music:** the alert layer wins while any guard chases a target or a script holds the alert counter up
  ([audio.md](audio.md)) [R].

There is no siren sound and no automatic alarm reset object in the data [D for the absence of such classes].

## 6. Test vectors [D]

- Laser counts per world (distinct objects): row 0: 2, 3: 7, 4: 4, 7: 2, 10: 6, 13: 21, 15: 10, 29: 3, 30: 4,
  31: 9, 32: 9, 34: 6, 38: 2, 42: 5, 43: 12.
- Laser object 26782 in row 0, first beam: anchors `0x24B38` and `0x24B36`; first end fraction 0, speed 0, period
  2.0, mode 16; second end 0, 0, 0, mode 16.
- Touch-pad counts per world: row 0: 7, 2: 5, 4: 2, 5: 1, 6: 3, 8: 6, 9: 1, 10: 1, 11: 1, 12: 3, 13: 1, 14: 4,
  15: 3, 17: 3, 20: 1, 21: 3, 22: 3, 25: 1, 27: 4, 30: 1, 31: 1, 32: 6, 34: 2, 36: 14, 38: 3, 39: 2, 40: 1, 41: 1,
  42: 1, 43: 2.
- Touch pad 147544 in row 0: options 33 = 1, 34 = 3, 37 = 1, 413 = 0, 418 = class 52, 419 = 0.25, 421 = 1.
- Stand-alone detector 27033 in row 6: options 345 = 346 = object `0x2176`; 349 = 7, 9, 8.
- Executable: `0x2C3610` = 100.0 (beam turn radius), `0x2BCC94` = 2.0 (smoke-bomb alarm hold), `0x2C3DB8` = 1.0
  (guard alarm repeat).

## 7. Open questions

- The meaning of laser options 450 to 454, and where the speed multiplier is stored.
- What each level's scripts do on detector event 28 (alarm, task failure); needs the script spec and play.
- Touch-pad options 415, 422 and 424, and the player-state kind (3).
- The flags of option 342 (values 8 and 12).
- Whether lasers also make a noise for guards when touched.
- All [R] behaviour needs checking in play.
