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
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 above, checked 2026-11-20), machine code of 0x00147F40, 0x00148300, 0x0014DC58, 0x0014E078,
      0x0014E148, 0x001492F0
    used_for: the camera's zone mask and position used by activation, and the trigger volume's activation kind (section 3.1) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/activate.cpp, loaders.cpp, alo.cpp, brx.cpp, lo.h, trigger.cpp, game.cpp
    used_for: where an object's zone mask is loaded and how it goes down a tree; the trigger volume's update (section 3.1) [R]
reviewed_by: pending
date: 2026-11-26
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

### 3.1 The zone mask in detail (added 2026-11-20)

- **The camera's side.** The camera's zone mask is the mask of the visibility volume that currently contains the
  camera's **eye**: camera record +0x14, set from the volume's second mask. It is recomputed whenever the camera's
  matrices are rebuilt, every frame, for any view, cutscene camera objects included. It is kept unchanged while the
  eye is in no volume [D]. [camera.md](camera.md) section 7.1.1 has the details.
- **The test.** Both activation tests read that mask and the camera position directly from the camera record:
  the per-frame test `0x00147F40` and the immediate test for a newly added root `0x00148300` [D].
  - For kinds 0 and 3, a root whose own mask (object +0x2D) shares no bit with the camera's mask is inactive. The
    radius is not looked at [D].
  - Kind 3 measures its radius from the player's position, or from the camera's when there is no player [D].
  - Kind 3's zone test still uses the camera's mask, never a mask for where the player stands [D].
- **The object's side.**
  - For a placed object, the mask is the `u32` that follows its transform in its header (the load flags).
  - For a solid object, it is the matching field of its own header [R].
  - Setting a mask passes it down to every child that lacks option 32 ("keeps its own mask"), and ORs it into every
    ancestor [R].
  - Masks seen on the disc are single bits (1 to 0x80), small sets such as 0x7 and 0x30, and 0xFF. 0xFF is the
    default when nothing sets one [D values; R default].
- **Radius.** Without option 13 the radius is 1e10, so a kind-0 or kind-3 root without it is gated by the mask
  alone [R].
- **Trigger volumes (class 24) are not exempt.**
  - They take kind 0 in their init (`0x001492F0`) [D].
  - They test their watched objects (option 432, else the player) only in their own update, which runs only while
    they are active [R].
  - So a trigger fires only while the camera's eye is inside a visibility volume whose mask shares a bit with the
    trigger's own mask. Where the watched object stands does not matter.
- **Row 0 example.** [camera.md](camera.md) section 7.2.1 has row 0's trigger volumes 150620 (mask 0x7) and 150658
  (mask 0x1), which are dormant while the cutscene eye sits in a 0x20 volume.

### 3.2 What keeps an object active outside the camera's zones (added 2026-11-26)

Running a command list does **not** by itself keep a character active. Solids (classes 48 to 108, apart from the
exceptions below) start with kind 0, so a guard or escort whose zone mask shares no bit with the camera's is out of the update list like anything
else [R]. Activation kind 1 (always active, neither the mask nor the radius tested) comes only from these [D for the
setter `0x001A6CA8` and its callers; R for each meaning]:

- **The class at init.** Classes 4, 15, 16, 28, 87, 104, 115, 116, 121 and 125 start at kind 1. Classes 11–13, 18,
  19, 27, 34–39 and 88 start at kind 3 (the player's radius). The others start at kind 0 or 2 [R, from the
  reference's table].
- **The player character.** Sly's post-load sets kind 1 (`0x00188924`).
- **A running sequence.** Starting a sequence sets the sequence object to kind 1 and stopping it to kind 2
  (`0x001DFE34` / `0x001DFF58`). This covers the sequence only, not the characters it commands.
- **Guard locks.** A guard is kind 1 while any of its AI jobs holds a lock, otherwise kind 0 (`0x0020B438` /
  `0x0020B478`). Locks come from:
  - the command-list section between commands 607 and 608 (options-by-class.md 2.2);
  - certain moves (an animation event 0x27);
  - crossing a link between regions;
  - hit reactions and pursuit.
- **Scripts.** Option 14 (the activation kind: getter `0x001A6C90`, setter `0x001A6CA8`) is script-settable.
- **Leased copies get the placement's mask.** When a placement leases a copy, each object of the copy is given the
  placement's zone mask (`0x001CB4E0`: the placement's +0x2D, through the mask setter `0x001A6D28`), unless the object
  keeps its own mask (option 32). It also gets the placement's activation radius (`0x001A6C48`: +0x38, option 13)
  [D]. The record's own mask in the package is therefore not what counts for a leased character.
  - **Example (row 2):** Neyla's placement 8436 (`npc_boss_neela`, fresh, top-level at (4112.8, −7827.9, 2799.6)) has
    mask **0x2**, while her package root 8438 has 0x1 [D].
  - Intro sequence 1855's shot puts the eye at (5276.6, −6899.9, 1324.7), inside visibility volume 28595 (mask 0x2).
    Neyla stands in the same volume [D].
  - So with the placement's mask she is active, and she runs list 124079 up to its command 612, which sends signal
    124080 to 1855. With the root's 0x1 she would never update and 1855 would wait for ever [I for the stall].

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
2. Which option holds a root's zone mask, and which classes use activation kinds 1, 2 and 3. Partly answered
   2026-11-20 (3.1): the mask is a header field, not an option, and trigger volumes use kind 0. The full class
   list per kind is still open.
3. The end-of-frame passes after the camera are not identified.
