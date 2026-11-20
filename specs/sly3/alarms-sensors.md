---
id: sly3-behaviour-alarms-sensors
title: Sly 3 lasers, detectors and contact triggers - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: laser, detector, contact-trigger and bouncer records and their option values (every record, every depth)
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/laser.cpp, laserrender.cpp, detect.cpp, pad.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 lasers, detectors and contact triggers

Read [../sly2/alarms-sensors.md](../sly2/alarms-sensors.md) first. As in Sly 2 there is **no alarm object**: an alarm
is a guard's noise about its target, plus the script alarms of command scripts ([scripts.md](scripts.md) 7) [R].
Tags and the option-correspondence column as in [guards.md](guards.md).

## 1. Classes [D]

| Sly 3 class | Sly 2 class | Role | Records |
|---:|---:|---|---|
| 96 | 85 | laser beams | 84 records in 10 worlds: m_gauntlet 31, c_hall_a 4, v_apt 2, c_tilt_hall 2, o_cave_a, h_hangar_a, m_boss (top-level groups; the rest nested) |
| 15 | 16 | stand-alone detectors (searchlights) | 5, nested, in the groups `spotlight_shooter` and `spotlight_shooter_thick` |
| 93 | 83 | contact triggers (Sly 2's touch pads) | 24 in 15 worlds (20 top-level, 4 nested); in Sly 3 they are the triggers of pickups, keys, mines and ammo (groups `wrench_ammo`, `claw_ammo`, `hack_color_key`, `h_spike_mine`, `p_sea_mine`, `p_dive_fish`, …) |
| 94 | — | bouncers (Sly 2's bounce pads were touch pads with a script) | 19 in 11 worlds (`bouncer`, `c_bouncer_tarp`, `o_bouncer`) |

## 2. Lasers (class 96)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 508 | 342 | 10 | list (17841, 0, 0, 0, 16, 17839, 1.0, 0, 0, 16) ×18; list (34537, 0, 0, 0, 8, 34535, 0, 0, 0, 8) ×15; list (34537, 1.0, 0, 0, 8, 34535, 1.0, 0, 0, 8) ×15 | 455 |
| 501 | 87 | 10 | list (51) ×66; list (50) ×13; list (58) ×8 | — |
| 503 | 84 | 10 | 21358 ×81; 5777 ×2; 5775 ×1 | 450 |
| 505 | 78 | 9 | 5769 ×78 | 452 |
| 506 | 63 | 10 | 5763 ×48; 5769 ×15 | 453 |
| 13 | 45 | 4 | 4000 ×30; 6000 ×12; 3000 ×1 | 13 |
| 504 | 17 | 6 | 5783 ×17 | 451 |
| 500 | 4 | 1 | 14265 ×4 | — |
| 27 | 2 | 1 | list () ×2 | 26 |
| 44 | 1 | 1 | list (17834) ×1 | 44 |
| 507 | 1 | 1 | 0 ×1 | 454 |

**508 is the beam** (Sly 2's 455), one occurrence per beam, the same 10 arguments (two anchors, each with start
fraction, speed, period and motion mode) [D for the occurrences, R for the layout]: 342 beams in all. 501 (a short
value, `0x33` on 66) and 503, 505, 506, 504 (object references, Sly 2's 450 to 453) set the laser's look and parts
[R]. Behaviour: Sly 2 section 2 [R].

## 3. Detectors (class 15)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 390 | 5 | 5 | 77588 ×5 | — |
| 395 | 5 | 5 | 36099 ×5 | 345 |
| 396 | 5 | 5 | 36099 ×5 | 346 |

395 and 396 are the view volumes (Sly 2's 345, 346). Sly 2's watched-target option (349) does not occur.
**390** is a reference to an object of class 47 stored at detector field +0xD0 (value type `0x202F`) [X]. All five
detectors name object 77588, which is the class-47 body of the `spotlight_shooter` / `spotlight_shooter_thick`
groups [D]. So 390 ties the detector to the spotlight gun that it aims, not to a watched target. *Interpretation:*
the detector reports what it sees to that shooter, which then fires [I]. The classes it watches are not set by any
option on the disc; they come from the class defaults [I]. Behaviour: Sly 2
section 4 (a script event on every state change) [R].

## 4. Contact triggers (class 93)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 459 | 21 | 14 | 0 ×21 | 413 |
| 464 | 16 | 10 | list (51) ×6; list (47) ×3; list (73) ×3 | — |
| 468 | 13 | 9 | 0.25 ×7; 0 ×5; 1 ×1 | — |
| 34 | 12 | 9 | 0 ×12 | 34 |
| 33 | 11 | 8 | 0 ×6; 4 ×5 | 33 |
| 13 | 9 | 5 | 10000 ×3; 4000 ×3; 7000 ×2 | 13 |
| 8 | 5 | 4 | list (2310, 76124) ×3; list (2168, -1) ×1; list (2169, -1) ×1 | 8 |
| 35 | 5 | 5 | (0,0,-1700) ×5 | 35 |
| 37 | 5 | 3 | 1 ×5 | 37 |
| 472 | 5 | 5 | 96531 ×3; 25264 ×2 | — |
| 53 | 4 | 2 | list (67108863, 2) ×2; list (46170112, 0) ×2 | 53 |
| 31 | 3 | 2 | (0,0,0.125) ×3 | 30 |
| 27 | 2 | 2 | list () ×2 | 26 |
| 32 | 1 | 1 | 1 ×1 | 32 |
| 61 | 1 | 1 | 1 ×1 | 59 |
| 86 | 1 | 1 | 1 ×1 | — |
| 460 | 1 | 1 | af01 ×1 | — |
| 469 | 1 | 1 | list (1, 113648, 17975, 13758) ×1 | — |

459 is the press kind (Sly 2's 413: 0, by contact, on all 21) and 468 the re-arm time (419; 0.25 on 7) [D] [R]. In
Sly 3 these objects carry the pickup-like props (ammo crates, keys, mines, collectibles): touching one raises its
script events 5 and 6 as in Sly 2 section 3 [R].

## 5. Bouncers (class 94)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 13 | 19 | 11 | 6000 ×19 | 13 |
| 497 | 18 | 11 | 30862 ×18 | — |
| 63 | 5 | 4 | 2 ×5 | — |
| 65 | 3 | 3 | 1 ×3 | 63 |
| 467 | 1 | 1 | list (19200) ×1 | — |
| 469 | 1 | 1 | list (1, 30844, 153894, 162883) ×1 | — |

Groups [D]: `bouncer`, `v_bouncer`, `o_bouncer`, `c_bouncer_tarp`, `h_bouncer_cheese`, `h_catapult`, `p_catapult`.
**497** has value type `0x2082`, a reference to an object of class 130 (an animation clip), at field +0x25C [X]. The
value 30862 on 18 bodies is a clip inside the bouncer's own body, not a separate object [D: no top-level object has
that ID]. So 497 is the bounce animation, not a launch target. This corrects the earlier reading. 469 is a method
with four arguments, (1, 30844, 153894, 162883) on `p_catapult` [X] [D]. 30844 is the catapult's own placement ID.
*Interpretation:* 469 is a state-machine goal on launch, the same shape as Sly 2's option 424 [I].

**Launch** [X]. Class 94 derives from 93 (the contact trigger; object size 624). Its own options and fields:

| Option | Field | Kind | Default (start-up code `0x0015D6D8`) | On the disc |
|---:|---|---|---|---|
| 496 | +0x258 | object of class 2 (value type `0x2002`) | none | never |
| 497 | +0x25C | animation clip (class 130) | none | 18 bodies |
| 499 | +0x264 | float | **0.75** | never |
| 498 | +0x268 | float | **1.0** | never |

- **States** (field +0x250, its start time at +0x254): a contact puts the bouncer in state 1. The update
  (`0x0015D810`) moves state 1 to state 2 once more than **0.05 s** have passed, and state 2 back to 0 on the next
  update. Entering state 2 launches (`0x0015D908`).
- **Target point T**: if 496 names an object, its position; otherwise the bouncer's local point **(0, 0, 300)**
  (the vector at `0x00320870`) carried into the world by the bouncer's transform (`0x001C9428`). That is 300 units
  along the bouncer's up axis.
- **Who is launched**: the object recorded as touching the bouncer (field +0x144).
  - A character of the playable family (class-51 test) goes through `0x00153F68` with (T, 0.75, 1.0). The start
    point is the character's position raised by one of its own fields (+0x268 of the character). Its jump state is
    set to kind 3, with the computed velocity (+0x370) and the 1.0 factor (+0x350).
  - The current player, when its field +0x1520 is 22, takes a variant of the same path (open).
  - A guard (class 58) receives (T, 0.75, 1.0) through one of its own methods.
- **The velocity** (`0x00212480`, from start S to T under the character's gravity g, with arc factor a = 0.75, first passed through `0x002A35F8`):
  1. θ = the elevation angle of T − S above the horizontal (atan2 of the height difference h over the horizontal
     distance d), clamped to at least 0.
  2. The launch angle is α = θ + a × (π/2 − θ): three quarters of the way from the straight line toward vertical.
  3. The speed v makes a projectile leaving at α pass through T: v = (d / cos α) × √(|g| / (2 (d tan α − h))).
  4. The velocity is v along α in the vertical plane through S and T. When d tan α − h ≤ 0 there is no solution
     and a fixed vector (`0x00317560`) is returned.

  With the default target straight above (d = 0), α = π/2 and the launch is a vertical jump that just reaches
  300 units.
- Afterwards the bouncer plays its clip (497) through `0x00137588`.

So the launch is a ballistic arc aimed at a point. On the disc that point is always 300 units above the bouncer,
because no bouncer sets 496.

- **The 1.0 factor** (option 498) [X]. It is stored in the character's jump block (character +0x320) at offset
  +0x30, next to the jump kind 3 (+0x3C) and the velocity (+0x50). The jump update (`0x001B88E0`) passes it to the
  arc integration (`0x001B8570`) as a scale for every jump kind except 12, which uses a timed curve instead. With 1.0
  the arc is integrated unchanged. *Interpretation:* a time or speed scale of the launched arc [I].
- **Option 469** has the same place and shape in the contact trigger's options (460 to 472) as 488 in the trigger
  shape's (478 to 491): a 4-argument method, followed by a 2-argument (470 ↔ 489), a 3-argument (471 ↔ 490) and a
  script reference (472 ↔ 491). 488 corresponds to Sly 2's state-machine goals on enter or leave (439), so 469 is
  read the same way [I]. Its one use, (1, 30844, 153894, 162883) on `p_catapult`, is "on trigger (1), in object 30844 (the
  catapult's placement), drive state machine 153894 to goal 162883". That is exactly Sly 2's (trigger or release,
  root object, state machine, goal) layout; the trigger shapes' 488 has the same form, for example (1, 77637, −1,
  26092) with −1 meaning any state machine. (Earlier versions showed only three arguments: the survey tool cut
  lists at three entries.)

## 6. Alarms

As Sly 2 section 5 [R]: guards raise alarm noises about their targets; command scripts keep numbered alarms
(commands `0x0E`, `0x0F`, `0x10` and conditions 1891 / 1892, [scripts.md](scripts.md) 7); music and task events
follow the guards' alert.

## 7. Open questions

1. Laser options 501, 503 to 507; contact-trigger options 464, 472; the classes a detector watches.
2. The exact effect of the jump factor in the arc integration; confirmation of option 469's reading.
3. Every [R] rule needs checking in play.
