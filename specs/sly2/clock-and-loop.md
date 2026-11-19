---
id: sly2-clock-and-loop
title: Sly 2 clocks, rate factors, activation and frame loop (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds walked
    used_for: the two slow-motion factors read from the executable; activation-radius option values (option 13) on all
      worlds
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/clock.cpp, update.cpp, activate.cpp, gadget.cpp, menu.cpp, shop.cpp, photo.cpp, overlay.cpp, screenrt.cpp, scpt.cpp, anim.cpp, physworld.cpp
    used_for: the frame-step rule, the nine rate factors and who sets them, the activation rule, the update list and the frame order; [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), the clock and world-update functions (facts only)
    used_for: cross-check of the clock constants
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 clocks, rate factors, activation and frame loop

A delta of [`../sly1/clock-and-loop.md`](../sly1/clock-and-loop.md). Tags: [D] disc data, [O] observable,
[R] reference behaviour to confirm, [I] inferred.

## 1. Measuring a frame

Unchanged from Sly 1 section 1: the timer base is 294,912,000 ticks per second and the measured step is clamped to
[1/60 s, 1/30 s] [R]. New: the **first frame after the clock is reset** (a world starts) always measures 1/30 s [R].

## 2. Two clocks, nine rate factors

Sly 1 had one game-clock scale built from a few named factors. Sly 2 keeps **nine numbered rate factors**, all 1 at
reset (factor 3 is never changed) [R]:

| Factor | Who sets it | Values |
|---:|---|---|
| 0 | Pause by menus, the shop, the photo mode | 0 while open, 1 otherwise |
| 1 | Sly's two time gadgets while held | **0.25** (slow motion) or **1.35** (speed-up); back to 1 when released |
| 2 | (not set) | 1 |
| 3 | (never changed) | 1 |
| 4 | Debug speed | 1 in retail |
| 5 | (not set) | 1 |
| 6 | Scripts (a script can stop game time; its own update then runs on the real clock while game time is 0) | 0 or 1 |
| 7 | On-screen movie screens that pause the game (counted: the first such screen sets 0, the last to close sets 1) | 0 or 1 |
| 8 | The pause overlay; it scales the **real** clock | 0 while the overlay is up, else 1 |

The two gadget values are read from the executable after the hash check: **0.25 at virtual address `0x002BF3EC`**
and **1.35 at `0x002BF3F0`** [D].

- **Game step** = measured step × (product of all nine factors) [R]. It is 0 while any factor is 0.
- **Real step** = measured step × factor 8 [R]. Unlike Sly 1 (where the real step followed the game step when the
  game step was at least 1/60 s), it does not depend on factors 0 to 7.
- **Game time t** advances by the game step; the per-level play-time counters advance with it [R].

## 3. Activation

New in Sly 2: whole object subtrees are switched on and off by distance [R].

- Every top-level object (a **root**) has an activation kind: **0** active within its radius of the **camera**,
  **1** always active, **2** inactive until told, **3** active within its radius of the **player** [R]. Placed and
  solid objects use kind 0 unless their class sets another [R].
- The radius is option 13 [R]. Values on the disc [D]: 4,000 (725 uses), 5,000 (363), 6,000 (326), 4,500 (275),
  3,000 (133), 8,000 (125), 10,000 (92), 2,500 (63), 15,000 (51), 3,500, 7,000, 19,000 and a few others.
- A root also has a zone mask; it is active only while that mask shares a bit with the camera's current zone mask
  (and is within its radius) [R].
- Each frame, before the updates, the queued roots are tested against their distance minus radius. A root whose
  distance is more than **500** units away from the boundary is moved to the back of the queue; the queue is fully
  re-tested whenever the camera or the player has moved more than 500 units since the last full test, or the zone
  mask changed [R].
- An inactive subtree is out of the update list (section 4) and is not drawn [R].

## 4. Update list

Objects that update are kept in **one flat list** in insertion order, not walked as a tree [R]. Each entry carries
flags: update every frame, run on the real clock, take part in the physics integration (moving entry), visible [R].
Removed entries leave holes that are compacted when more than 10 % of the list is holes or the list reaches 4,096
entries [R].

## 5. Order of one frame

Replaces Sly 1 section 4 [R]:

1. Run the script events and calls deferred from the previous frame.
2. Activation (section 3).
3. **If game time is paused** (game step 0): update only the entries flagged "update" and "real clock", then advance
   the animation clips flagged to run on the real clock. Skip to step 5.
4. **Otherwise:**
   1. Read the controllers.
   2. Drop contact records older than **0.25 s**; per-frame checks of jobs, triggers and vaults.
   3. Update every entry flagged "update", in list order.
   4. Frozen-guard pass, then advance all animation clips, then expire AI noises.
   5. Run deferred script events and calls.
   6. **Physics** ([physics.md](physics.md) 3), then remove finished clips.
   7. Navigation polygons; texture animation.
   8. Run deferred script events and calls again.
5. Always (paused or not): music, camera, then the remaining end-of-frame passes and drawing.

Script events can therefore be raised at three points in a frame (start, after the updates, after physics) [R].

## 6. Test vectors

| Situation (60 fps) | Game step | Real step |
|---|---|---|
| normal play | 0.0166667 s | 0.0166667 s |
| slow-motion gadget held (factor 1 = 0.25) | 0.0041667 s | 0.0166667 s |
| speed-up gadget held (factor 1 = 1.35) | 0.0225 s | 0.0166667 s |
| shop open (factor 0 = 0) | 0 | 0.0166667 s |
| pause overlay up (factor 8 = 0) | 0 | 0 |
| first frame of a world | 0.0333333 s | 0.0333333 s |

## 7. Open questions

1. Factor 8 enters both the game product and the real step, so the pause overlay stops both clocks [R]; to confirm in
   play that menus and camera input still respond (they would then need the measured step directly).
2. Which option holds a root's zone mask, and which classes use activation kinds 1, 2 and 3.
3. The end-of-frame passes after the camera are not identified.
