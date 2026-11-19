---
id: sly1-behaviour-alarms-sensors
title: Sly 1 alarms and sensors - lasers, searchlights, pressure plates, alarm boxes
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: where each class occurs, how many are placed (proxy use counts), every option value on alarm and
      sensor templates and in proxy overrides
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/alarm.cpp, sensor.cpp, sensor.h, stepguard.cpp, spaprops.cpp
    used_for: >-
      the behaviour of alarms and sensors, the sensor state cycle, timers and sounds, and which option ID sets which
      property; read for what the game does and re-expressed here
  - kind: observation
    what: general knowledge of the game as played
    used_for: statements tagged [O]
reviewed_by: pending
date: 2026-11-18
---

# Alarms and sensors

Tags as in [gameplay-overview.md](gameplay-overview.md): **[D]** disc, **[O]** observed, **[R]** reference
behaviour to confirm, **[I]** inferred. Class IDs: [object-stream.md](object-stream.md) section 6. Guards:
[guards.md](guards.md). Sound IDs and distances: [audio.md](audio.md) 3.6. The player is OID 5 [D].

## 1. What exists

| Class | Plain name (ours) | Records / worlds | Placed | Where |
|---:|---|---|---:|---|
| 47 | alarm | 17 / 17 | 37 (through proxies) | worlds 1, 3, 5–8, 10, 12, 14, 19, 24, 26, 32–34, 41, 45 [D] |
| 34 | alarm box (breakable) | 17 / 17 | 17 | same 17 worlds as the alarms [D] |
| 49 | laser sensor | 11 / 11 | 106 | worlds 1 (5), 5 (22), 6 (3), 12 (16), 14 (12), 24 (2), 26 (1), 32 (8), 33 (22), 41 (3), 45 (12) [D] |
| 50 | searchlight sensor | 38 / 11 | 59 | worlds 3 (6), 5 (6), 7 (4), 8 (5), 10 (9), 14 (2), 19 (4), 26 (10), 32 (2), 33 (4), 41 (7) [D] |
| 51 | pressure plate | 22 / 2 | 22 (some records are written inside proxies) | worlds 12 and 14 [D] |

## 2. Alarms (class 47)

### 2.1 States and links

An alarm is **disabled**, **armed** or **triggered** [R]. It is linked to [R] [D]:

- up to 4 **alarm boxes** (option 289, an OID; one per alarm template on the disc [D]);
- up to 16 **sensors** (option 290, an OID per link; set in 195 proxy overrides, 176 distinct sensors [D]);
- up to 6 guards (option 291; **not used on the disc** [D]); guard groups link alarms on their own (guards.md 2);
- up to 8 **state-machine goals** to set on trigger or on reset (option 297, typed arguments: on-trigger flag, root
  OID, state-machine OID, goal OID; 11 overrides [D]).

Options 131 and 132 (= 1 on every alarm [D]) lock the alarm's body in place.

### 2.2 What each state does

[R]:

- **Armed**: its linked sensors are switched on in "detect only" mode (section 3), unless a sensor has its own
  starting state (option 306). Its own animation goes to the "ready" pose (state-machine goal OID 538).
  The goal IDs 538 (ready), 543 and 544 are **constants in the executable**: the alarm's state routine (`0x00123888`)
  passes them as immediate values to the state-machine goal calls (`0x001B64D8`, `0x001B6628`) [X]. So every alarm in
  every world must hold its animation states under these IDs.
- **Triggered**:
  - siren sound 21 at the alarm (start distance 8000, full volume 5000) unless the alarm is silent (option 292);
  - music intensity request 107 (audio.md 5.4);
  - every linked sensor is told that the alarm went off, and switches to **damage mode** (3.2);
  - linked state-machine goals with the on-trigger flag are applied;
  - script event 2 on the alarm (scripts.md 6);
  - the game's alarm counter increases (used by progress and ranking; pickups-and-progress.md);
  - animation goal OID 543.
- **Leaving triggered**: siren stops, the intensity request is removed, goals without the on-trigger flag are
  applied.
- **Disabled**: linked sensors are switched off; animation goal OID 544; script event 3 the first time, event 22
  when the disabled state is restored from a checkpoint; the disabled state is remembered across checkpoints.

### 2.3 How state changes happen

[R]:

- A linked sensor that detects its target triggers the alarm (3.1).
- A guard group reaching "attack" triggers its alarms; returning to "patrol" re-arms them; a dead group disables
  them (guards.md 2).
- **Reset timer**: a triggered alarm with a non-zero reset time (option 293) returns to armed after that many
  seconds. No alarm on the disc sets it [D], so alarms stay triggered until something else changes them [I].
- **Alarm boxes**: when every linked alarm box has been broken, the alarm is disabled for good.
- If the alarm is triggered when the player celebrates (collecting a key), its alarm boxes break by themselves.
- Script calls: trigger (option 296), set goal (297).

## 3. Sensors in general

### 3.1 Who triggers a sensor

[R], same rules for every sensor kind:

1. Option 303 ("anything triggers") set: any object.
2. Objects or classes on the ignore lists (options 300 OIDs, 302 class IDs) never trigger.
3. Objects on the trigger list (299 OIDs) or of a class on the class list (301) trigger.

On the disc [D]: every laser and pressure plate lists the player (299 = 5); every searchlight ignores barriers
(302 = class 52); 4 proxy overrides add ignored objects (300).

**Player size used by each sensor** [R]: there is no separate player radius. Lasers intersect their beam segments
with the player's collision mesh (the current state's shape; standing, from 55 below to 90 above his origin,
player-moves.md 1.1). Searchlights test a single point, the player's origin, against the beam volume, and the line
from the light to that point. Pressure plates use real contacts with the player's mesh (physics.md 3.2).

### 3.2 The sensor cycle

Every sensor runs this cycle; each kind adds its own detection test (sections 4 to 6) [R]:

| State | What happens | Leaves when |
|---|---|---|
| detect: switching on | detection runs (lasers) or not (others) | after the switch-on time (option 304) → detect: on |
| detect: on | detection runs | target detected → detect: tripped |
| detect: tripped | the linked alarm is triggered; script event 2 on the sensor | next update → detect: switching off |
| detect: switching off | | after the switch-off time (option 305) → off (or → damage: switching on, if the sensor is now in damage mode) |
| damage: switching on | | after the switch-on time → damage: on |
| damage: on | detection runs; detecting the target hurts it | — (kind-specific, below) |
| damage: switching off | | after the damage switch-off time → off |
| off | | kind-specific (searchlights and plates switch back on) |

- A sensor switched on by an alarm starts in **detect** mode. When its alarm goes off, the sensor is put into
  **damage mode**: from then on it hurts the player instead of raising the alarm [R].
- If a sensor is switched off within 0.2 s of its alarm being disabled, it stays off for good [R].
- A sensor that has just hurt the player pauses its own animation until the player recovers [R].

Timers on the disc [D]: lasers 304 = 1.0, 305 = 1.0 (one override 1.6); searchlights 305 = 0.4.

## 4. Laser sensors (class 49)

- A laser has one or more **beams**, each following a curve shape among its children (classes/paths-and-maps.md)
  [R]. A beam is tested segment by segment from its first point, up to its current drawn length.
- **Kind** (option 313): 0 moving, 1 fixed (set to 1 in 51 overrides [D]). For fixed lasers whose only trigger is the
  player, and only while the beam segment is on screen, the test is against the player alone [R].
- **Rate**: each laser tests 30 times per second; the tests of all active lasers are spread evenly over 1/30 s [R].
- A beam segment that hits a triggering object first (closer than anything else) trips the laser. The player is
  ignored while hidden, and while in the casual-hiding pose with the power-up mask `0x12000` owned [R].
- **Damage mode**: the hit is fire damage to the player; sparks and scorch emitters at the hit point run while the
  beam touches something [R].
- Sound 26 loops while the laser is on (start distance 1000, full volume 100, volume 0.4); it fades out over the
  switch-off time when the laser turns off [R].
- Scripts can extend and retract the beams (options 315, 316) and switch lasers on and off (311, 312) [R].
- Once off, a laser stays off until a script switches it on again [R].

## 5. Searchlight sensors (class 50)

- The searchlight's beam is the sensor's own collision mesh, used as a volume: the player is in the beam when its
  position is inside that mesh's tree (collision.md 2) [R].
- It turns to look at the player: every searchlight on the disc has look-at options 73, 74, 76 = 3, ignore distance
  77 = 3000, pan and tilt functions 78, 80 = (0, 1, 0), tilt limits 81 = (−3.14, 3.14) [D]. Meaning: options-by-class.md.
- **Detection** [R]: the player is not invulnerable to fire, is inside the beam, is not hidden from it (the same
  hiding rules as guards.md 4.3, judged from the searchlight's position), and the line from the searchlight to the
  player is clear (ignoring the player, barriers, hidden objects).
- **Damage mode** (after its alarm went off) [R]: on detection the light **focuses** for a focus time (option 317),
  then **zaps** the player with fire damage (sound 27, start distance 3000, full volume 300) for a zap time (318),
  then **unfocuses** for an unfocus time (319), then watches again. If the player becomes invulnerable while it
  focuses, it unfocuses at once. No searchlight sets 317 to 319 in its options on the disc [D]; scripts may [I]. The
  defaults are **0.25 s focus, 1.0 s zap, 0.75 s unfocus** [R: the reference's current option table, which lists
  each option's default; an older reference build had 0]. They could not be read from the executable directly,
  because the option table is filled by start-up code ([option-encodings.md](option-encodings.md)).
- A searchlight that is off switches itself back on (in its current mode), unless it was switched off with its
  alarm (3.2) [R].

## 6. Pressure plates (class 51)

- A plate detects a triggering object (the player) standing on it, by contact [R].
- **Damage mode**: contact is electric damage to the player; other objects get a damaging push [R].
- **Timed cycle**: with "stay on" time `T_on` (option 335) ≥ 0, the plate switches off after `T_on` s and back on
  after the "stay off" time (option 336) [R]. Disc values [D]: 335 = 2.0 on 16 records and −1 (stay on) on 6;
  336 = 2.0 on all 22. 8 records start off (option 306 = 8, the "off" state).
- When its alarm goes off, a plate without a timed cycle restarts in damage mode [R].
- The plate's look follows its state through a looping texture animation (textures.md 2.4), using frame ranges set
  by options [D] [R]:

  | State | Frames (options) | Seconds per loop (option) | Values on all 22 records [D] |
  |---|---|---|---|
  | detect | 321–322 | 323 | frames 0 to 7, 1.0 s |
  | damage | 325–326 | 327 | frames 8 to 13, 0.75 s |
  | off | 329–330 | 331 | frame 14 |
  | switching off (flashing) | 333 | 334 (rate) | frame 14 at 8 |

  Options 324, 328, 332 (= 1, 0, 0 [D]) select the playback style of each range [R].

## 7. Alarm boxes (class 34)

A breakable object (interactables.md) that reports to its alarm when broken [R]. On the disc every alarm box has
option 221 = 2.0 (break threshold), 230 = 1 (ignore its own broken pieces), 231 = 0, four exclusion links (212) and
one fixed-part link (214) [D]. Breaking all of an alarm's boxes disables the alarm (2.3).

## 8. Test vectors

- World 5 "High Class Heist": laser template OID 1590 with options 130 = 1, 299 = 5, 304 = 1.0, 305 = 1.0, placed 22
  times; alarm template OID 1608 with 289 = 1609 (its alarm box), 131 = 1, 132 = 1, placed 4 times [D].
- World 3 "A Stealthy Approach": searchlight template OID 1639 with 302 = 52, 305 = 0.4, 130 = 1, look-at options as
  in section 5; placed 6 times [D].
- World 14 "Boneyard Casino": pressure plate OID 1647 with 299 = 5, 321 = 0, 322 = 7, 323 = 1.0, 324 = 1, 325 = 8,
  326 = 13, 327 = 0.75, 328 = 0, 329 = 14, 330 = 14, 331 = 0.0, 332 = 0, 333 = 14, 334 = 8.0, 335 = −1.0,
  336 = 2.0 [D].

## 9. Open questions

- Searchlight focus, zap and unfocus times: confirm the reference's defaults (0.25, 1.0, 0.75 s) in play.
- What "damage mode" does to non-player objects on lasers.
- (Settled: goals 538, 543 and 544 are constants in the executable, so every world uses the same IDs; see section 2.)
- How the alarm counter feeds the end-of-level ranking.
