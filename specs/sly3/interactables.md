---
id: sly3-behaviour-interactables
title: Sly 3 breakables, barriers, water, use points and trigger shapes - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: records of the interactable classes (every depth) and their option values
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/breakable.cpp, barrier.cpp, water.cpp, interact.cpp, trigger.cpp, hide.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 interactables

Read [../sly2/interactables.md](../sly2/interactables.md) first; behaviour carries over [R]. Contact triggers and
bouncers (Sly 2's pads) are in [alarms-sensors.md](alarms-sensors.md). Tags as in [guards.md](guards.md).

## 1. Classes [D]

| Sly 3 class | Sly 2 class | Role | Records (top + nested), worlds |
|---:|---:|---|---|
| 111 | 98 | reactive props and breakables (`crate_small`, `barrel`, …) | 375 + 40, 36 |
| 97 | 86 | barriers (block listed classes) | 153 + 295, 38 |
| 101 | 89 | water | 36 + 3, 17 |
| 21 | 24 | trigger shapes | 348 + 184, 34 |
| 22 | 25 | trigger shapes (second kind) | none on the disc |
| 33 | 35 | use points (interact here) | 48 + 38, 26 |
| 34 | 36 | carry-use points | 142 nested, 34 |
| 35 | 37 | treasure stands | 3 + 6, 5 |
| 25 | 27 | crawl hooks and hiding spots (`o_minecart_hide`, `hide_table`, `c_crawl_tree`) | 16 + 87, 18 |
| 37 | 39 [R] | moving set pieces with an attach point (`c_dragon_giant`, `c_rail_arch`, `o_cave_active_grinder`) [D for the groups] | 42 + 104, 22 |
| 38 | 40 [R] | hooks (`wall_hook`, `hook`, also on `npc_doctor_m`) | 3 + 39, 20 |

Sly 2's class 38 counterpart (Sly 3 class 36) does not occur on the disc [D].

## 2. Reactive props and breakables (class 111)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 1132 | 625 | 36 | 4748 ×363; 33402 ×45; 18119 ×20 | 1034 |
| 1128 | 595 | 36 | list (-1) ×569; list (1766) ×8; list (49748) ×4 | 1030 |
| 1140 | 364 | 36 | list (-1) ×253; list (18972) ×11; list (30549) ×7 | 1042 |
| 53 | 354 | 35 | list (33620350, 2) ×195; list (67108863, 0) ×64; list (33554814, 2) ×20 | 53 |
| 13 | 324 | 36 | 4000 ×167; 5000 ×49; 6000 ×30 | 13 |
| 1141 | 301 | 32 | list (17341) ×41; list (17318) ×35; list (27515) ×11 | 1043 |
| 1125 | 171 | 30 | list (0, 0) ×58; list (58, 1) ×48; list (0, 1) ×17 | 1025 |
| 1135 | 165 | 31 | list (-1) ×162; list (11) ×3 | 1037 |
| 1142 | 158 | 29 | list (24483) ×21; list (18115) ×19; list (30541) ×8 | 1044 |
| 1138 | 144 | 32 | list (17318) ×36; list (30543) ×8; list (18818) ×7 | 1040 |
| 1137 | 82 | 16 | 94731 ×5; 33030 ×4; 44545 ×4 | 1039 |
| 1139 | 65 | 22 | list (18115) ×23; list (30541) ×8; list (153857) ×3 | 1041 |
| 1130 | 64 | 25 | list (-1) ×54; list (2) ×4; list (3) ×3 | — |
| 50 | 57 | 22 | 85 ×49; 50 ×8 | 50 |
| 1131 | 55 | 22 | 19308 ×21; 18809 ×16; 18811 ×7 | 1033 |
| 49 | 49 | 20 | object/id ×49 | 49 |
| 1134 | 32 | 15 | list (9) ×32 | 1036 |
| 8 | 29 | 12 | list (2147, -1) ×4; list (2494, -1) ×3; list (2618, 122231) ×2 | 8 |
| 1136 | 28 | 10 | 14581 ×11; -1 ×8; 50139 ×4 | 1038 |
| 63 | 24 | 17 | 2 ×24 | — |
| 1133 | 21 | 13 | list (0) ×21 | 1035 |
| 27 | 17 | 13 | list () ×17 | 26 |
| 3 | 12 | 4 | list (2249, 0) ×7; list (2619, 0) ×2; list (2515, 1) ×1 | 3 |
| 1129 | 9 | 5 | list () ×9 | 1031 |

1128 to 1143 are Sly 2's 1030 to 1045: the event list, hit reactions, pieces and rewards of Sly 2 section 3 [R];
13 is the activation radius (4,000 on 167, 5,000 on 49) [D].

## 3. Barriers (class 97)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 512 | 502 | 38 | list (58) ×149; list (50) ×85; list (88) ×59 | 456 |
| 27 | 231 | 38 | list () ×231 | 26 |
| 53 | 84 | 35 | list (67108863, 2) ×34; list (64520193, 0) ×34; list (256, 2) ×8 | 53 |
| 510 | 50 | 18 | list (24) ×16; list (68698) ×8; list (50623) ×5 | — |
| 513 | 48 | 20 | 1 ×33; 0 ×15 | 458 |
| 65 | 10 | 5 | 1 ×10 | 63 |
| 33 | 7 | 6 | 0 ×7 | 33 |
| 39 | 6 | 4 | 1 ×6 | 39 |
| 91 | 6 | 6 | 1 ×6 | — |
| 13 | 4 | 2 | 200000 ×4 | 13 |
| 35 | 4 | 4 | (0,0,-800) ×4 | 35 |
| 82 | 4 | 4 | 100 ×4 | 80 |
| 34 | 3 | 2 | 0 ×3 | 34 |
| 63 | 2 | 2 | 2 ×2 | — |

512 is Sly 2's 456: a class that the barrier stops, one occurrence per class (58 guards on 149, 50, 88, …) [D] [R];
513 (Sly 2 458) a flag; 510 names objects [D].

## 4. Water (class 101)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 930 | 29 | 14 | 21014 ×24; object/id ×1; object/id ×1 | — |
| 932 | 27 | 13 | 21010 ×27 | — |
| 931 | 23 | 12 | 20996 ×22; object/id ×1 | — |
| 929 | 18 | 1 | list (99853, 0000c8430000000000000000) ×1; list (99851, 0000fa430000000000000000) ×1; list (99849, 0000fa430000000000000000) ×1 | — |
| 926 | 5 | 3 | 10 ×4; 2 ×1 | — |
| 925 | 4 | 2 | 8192 ×4 | 877 |
| 935 | 2 | 1 | 51385 ×1; 51384 ×1 | — |
| 13 | 1 | 1 | 5500 ×1 | 13 |
| 27 | 1 | 1 | list () ×1 | 26 |
| 927 | 1 | 1 | 1 ×1 | — |

Sly 3's water options are new numbers (925 is Sly 2's 877 [R]); 929 lists current points (an object and a vector,
18 entries in one world) [D]; 930 to 932 name the splash and spray effects [I]. Behaviour: Sly 2 section 7 [R].

## 5. Use points (class 33)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 1163 | 62 | 24 | 50 ×21; 100 ×12; 150 ×11 | 1069 |
| 1164 | 47 | 18 | 2060 ×9; 62463 ×8; 44936 ×6 | 1070 |
| 1170 | 26 | 10 | list (2405, 4) ×1; list (2111, -1) ×1; list (2105, -1) ×1 | 1076 |
| 1178 | 23 | 11 | 1 ×8; 7 ×8; 4 ×3 | — |
| 1168 | 18 | 11 | 3300 ×14; 3400 ×2; 3500 ×2 | 1074 |
| 1175 | 13 | 7 | 0 ×10; -1 ×3 | — |
| 27 | 12 | 9 | list () ×12 | 26 |
| 1167 | 4 | 4 | 25 ×3; 29 ×1 | 1073 |
| 1162 | 1 | 1 | 24 ×1 | — |

1163 (Sly 2 1069) is the use radius (50, 100, 150) [R] [D for the values]; 1164 (1070) the object used; 1168 (1074)
3,300 to 3,500 [D]; 1170 (1076) is **not** an animation list; see below.

**Reach and eligibility** [X]:

- The use point's load-time setup (`0x001A1D70`) sets the reach (+0x120, option 1163) to **200** when the option is
  absent. It also sets +0xF8 to 1250.
- **Correction.** The routine `0x001A21B0`, which subtracts reach − 100 (G = 250 at `0x00345700`), is vtable slot
  61 of **class 34**, a class derived from 33. Class 33's own slot 61 (`0x001A0830`) does something else. The
  slot's only caller (`0x0013CC90`) is a notification loop that clears a flag bit (8) and calls slot 61 on each
  listed object; it is not a distance search. So for a class-33 point such as 138208, this score code does not
  apply [X].
- In the use point code (`0x001A1000` to `0x001A3000`) nothing else reads the reach field +0x120. The routine that
  picks the point when the player presses use was not found. The character's use action is a vtable entry
  (`0x0014EB40`, entry `0x0035399C`) that ends by calling the use routine. So **how the distance is measured, and
  whether there is a height window, is still open** [X for the paths checked].
- The use routine is class 33's vtable slot 55 (offset 440), the same slot command `0x12` calls. The mask test
  `0x001A2198` is called only from a script method handler (`0x0010C3F8`), so it is a script-side "can use" test,
  not part of a built-in search.
- **Open question:** the routine that collects use-button candidates (distance, height window, mask) was not
  found in this pass.
- The use test (`0x001A2198`) also requires that the point's mask (+0xF0, set to 4 at load by `0x001A21E0`, on
  the disc option 1178) shares a bit with the user's mask (+0x164).
- **Use = event 5.** The use routine raises script event 5 on the point (step 4 of the list below), and the point's
  module handles it.

**Use point 138208 (v_interpol)** [D] [X]:

- It names module 0 of its group `v_interpol_recon`, decoded:
  `(define s5 (lambda (s1240) ((op117 (op118 138191 obj0) 247) (op117 s1240 555))))`. Script ID 555 on class 33
  is stream option 1162, the field +0x11C where the use routine stores **the user**.
- So on event 5 it calls method 247 of object **138191** with the user. 138191 is the **cell lock**: class 79,
  group `v_cell_lock`, placed by placement 138191.
- Class 79's method 247 (stream option 1600, handler `0x0010F9F8` → `0x001CAF30`) **starts the lock minigame**. It
  stores the user at +0x1F0, puts the lock in state 1 (`0x001CAC30`), and gives **player 0's control to the lock**
  (`0x001F4B00(game, 0, lock)`).
- Raising event 5 on the point alone does nothing useful when +0x11C is empty. The user must be stored first,
  which the use routine `0x001A20F8` does.
- In the disc's scene, command script 2113 makes Sly use the point. It runs command `0x12` 138208, which activates
  a class-33 object with the run's character ([scripts.md](scripts.md) 7.3).

**The cell lock (class 79): a tumbler-lock minigame** [X] [D for the v_interpol objects]:

- **Parts** (lookup table `0x00345860`, 9 records bound at post-load by `0x001C8970`):
  - meshes 1866 and 1867 (+0x190, +0x194);
  - state machine 1872 (+0x198), whose instance is kept at +0x1C0;
  - command scripts 1868, 1869, 1870 and 1871 (flag `0x500`) for states 1 to 4 (+0x1A0 to +0x1AC);
  - two more parts, 1873 and 1874 (+0x254, +0x258).
- In v_interpol the scripts live in group `lock_sequences`:
  - 1868 (state 1): plays a camera / animation;
  - 1869 (state 2): quit;
  - 1870 (state 3): starts the solving view, then waits about 5,000 s, i.e. for the rest of the minigame;
  - 1871 (state 4): solved. It sets the character's interaction object to 5929 for 0.5 s, waits 0.4 s, then runs
    command `0x15` −1, a hook on the script itself.
- **Start** (script method 247, `0x001CAF30`, called by use point 138208's module):
  - stores the user at +0x1F0;
  - sets state 1;
  - gives player 0's control to the lock.
- **States** (+0x1B4; the update `0x001C9D70` steps them every frame). On entering a state the lock runs that
  state's script with the user (`0x001CAC30` → `0x00225EE8`), after stopping the previous state's script.

| State | Meaning | Leaves when |
|---:|---|---|
| −1 / 0 | idle | the start method sets 1 |
| 1 | intro | its script ends (`0x00226190`) → 3 |
| 3 | **playing** | quit: if quitting is allowed (+0x1D0, 1 at load) and the controller's button `0x10` is pressed (`0x001A7A80(player, 0, 0x10)`; *Triangle* [I]) → 2. **Solved:** when no tumbler is moving (+0x1C4 = 0), or one has been moving for more than 0.4 s (+0x1D4), and **all three tumblers** (+0x20C, +0x210, +0x214) have values 5 or 6 (`0x001CAFC8`) → 4. Otherwise the tumbler update runs (`0x001C9F00`) |
| 2 | quit | its script ends → control goes back to the user (`0x001F4B00(game, player +0x130, user +0x1F0)`) |
| 4 | **solved** | its script ends → control goes back to the user, as for 2 |
| 5 | done | — |

- Losing control (vtable slot 83, `0x001CAAC0`) sets state 0 from states below 4, and state 5 from 4 or 5.
- **The dial** (update `0x001CA400`, from `0x001CA544`) [X]:
  - Each frame the stick's angle, atan2 of the input pair at the lock's +0x134 record (+0 x, +4 y), is compared
    with the previous stick angle (+0x1F8).
  - A change smaller than 0.1 rad counts as 0; a larger one is reduced by 0.1. The result is added to +0x1F8.
  - The change per second is smoothed into the dial's angular speed (+0x1FC; smoothing parameters at
    `0x00323820`). The dial angle (+0x200) advances by speed × the frame step. The previous dial angle is kept at
    +0x204.
  - **Turning the stick round turns the dial.**
- **Gates** (`0x001CAF70`, set at load with (85, 20, 75)). Tumbler i's gate angle (+0x1E4 + 4i) is
  2 × n × 0.0314159 (`0x3D00ADFD`): the dial is **100 marks**, and the v_interpol combination is **85, 20, 75**.
  Two tolerances are used: 0.5027 rad (`0x3F00ADFD`, 8 marks) and 0.2513 rad (`0x3E80ADFD`, 4 marks).
- **Tumbler states** (+0x20C + 4i, values −1 to 6; one step per frame per tumbler, `0x001CA078`, jump table
  `0x003237F0` by value + 1) [X for the mechanism; I for the reading]:
  - The direction sign comes from `0x001CABB8`: +1 or −1 from the tumbler's value and the parity of its index.
    So the tumblers are set by turning in **alternating directions**, as on a combination dial.
  - The current and previous dial angles are compared with the signed gate.
  - Crossing the gate ± 8 marks moves the tumbler to its next stage: values 3 and 4, then 5 or 6.
  - Values 5 and 6 are **set** (state-machine goal 1876 or 1877).
  - Passing a stage the wrong way, or moving on while the tumbler two places on (i + 2) is still unset, gives
    value 2 and resets through `0x001CAD10(lock, 2)`.
  - Setting tumbler i to 5 or 6 also moves the next tumbler (i + 1) to stage 2 or 1 (`0x001CAE80(lock, i + 1, …)`).
  - `0x001CAE80` writes the value and sets the visual goals: 1877 for 0 or 5, 1876 for 6, 1875 for 5 and 6 on the
    third tumbler.
- **Solved** when all three values are 5 or 6 and no tumbler is part-way (+0x1C4 = 0, or more than 0.4 s since
  it moved). Then state 4 starts.
- The exact per-value thresholds of `0x001CA078` (which side of the gate, ± 4 or ± 8 marks, for each value) are
  in the jump table cases `0x001CA108` to `0x001CA360`. This summary does not list them case by case [I].
- **Completion and the task** [D for the module; I for the last link]:
  - The lock has no code that raises script event 5 (`0x001C9B00` to `0x001CB400`), and script 1871's `0x15`
    raises event 4 on the script itself, which has no module.
  - Object 113853 (class 47, `lock_sequences` module 0) handles event 5 as
    `(define s5 (lambda (s1240) ((op117 (op117 s1240 34) 12))))`. It takes its script ID 34 (stream option 20,
    the object at +0x10) and calls method 12 on it. Method 12 is stream option 11, handler `0x001007E8` →
    `0x0013BC80`: **raise event 5**. So event 5 on 113853 is passed on to that object, which is the lock it
    belongs to [I]. The shared handler then broadcasts task event 29 with the lock, and task 2115 requests done
    (missions.md 4.1a).
  - **113853 really is part of the lock** [D]. The lock's body (138191, `v_cell_lock`) holds a nested class-8
    record that places 113853 inside it, at `0x0018F568` to `0x0018F5AE`. Its options are:
    - option 8 (2496, 2114): script 2114, a camera key that then waits for alarm 50516;
    - option 3 (2498, 1) and option 3 (2499, 0).

    So the copy's owner (+0x10) is the lock.
  - 113853's module also defines two custom methods (script IDs 2495 and 1869, through `op114`). 2495 starts
    script 2114 and sets the camera; 1869 undoes it. Neither ID appears in the executable or in another v_interpol
    module.
  - **Checked, and none of them raises event 5 on 113853** [D] [X]:
    - State machine 1872 has states 159, 163, 164, 165, 132 and 166. Its clip events are kind 0 (sounds 490, 215,
      753, 811) and kinds 13 / 14 (show or hide a mesh group of 113837, 113839, 138200, 138193).
    - State machine 1874 has states 1875 to 1877, with clips 140909 and 140910 (mesh groups only).
    - The lock code, including states 4 and 5 and the control hand-back.
    - The solved script 1871.
  - **Open question:** what raises event 5 on 113853, and so on the lock. A port that needs the job to advance
    can raise event 5 on the lock when it reaches state 4 (solved). That gives the task its code-29 event [I: a
    stand-in, not the game's traced path]. It is probably the end of the solved sequence (the state
    machine 1872 reaching its last state, or the solved script) [I].

**Option 1170: a task request when the point is used** [X]. It is a load-time method with two arguments, (task,
state). Its handler (`0x0010C530` → `0x001A20C0`) appends the pair to the use point's list (count +0x16C, pairs
from +0x170, at most 4; `0x001858D0`), so one point can carry up to four pairs. When the point is **used**
(`0x001A20F8`), the game does the following in order:

1. stores the user at +0x11C;
2. runs `0x0025D3F8` with the point's +0x128 / +0x12C;
3. applies the pairs through `0x00185908`. Each pair's task is found by OID and gets an ordinary (checked) request
   for the state (`0x00189078`), with the point as the requester;
4. raises script event 5 on the point;
5. starts the point's command script (option 1164, +0x124) with the user, when one is set (`0x00225EE8`).

The use routine is called from the character interaction code at `0x0014ECB4` (in `0x0014EB40`) and from
`0x001A22BC`, and it sits in a vtable at `0x0035AB74`. *Interpretation:* `0x0014EB40` is the player's use-button
interaction with the point [I].

State −1 in a request means "make current", as in Sly 2 [R]. So use point 138235's (2105, −1) asks for **task 2105
`t1_recon_go_to_key` to become current** when Sly uses the point. That task's 1405 then marks it done at once
(missions.md 4). Disc values include (2405, 4), (2111, −1) and (2105, −1).

## 6. Other classes

**Trigger shapes (class 21)**, 348 top-level and 184 nested bodies. Options (after the 54-byte header) [D], with value
types [X]. The correspondence to Sly 2's trigger-volume options 432 to 443 (Sly 2
[interactables.md](../sly2/interactables.md) 5) is by shape and values [I], except 478 [R]:

| Option | Kind [X] | Bodies | Values [D] | Sly 2 | Meaning |
|---:|---|---:|---|---:|---|
| 27 | method, no arguments | 79 | — | 26 | removed at start |
| 478 | bool at +0x244 | 22 | 1 | 443 [R] | tracking: keep the ordered set of members inside |
| 480 | method, one argument (repeats) | 136 | object IDs: 24 (Sly) ×45, others | 432 | an object that counts |
| 483 | method, one argument (repeats) | 144 | class IDs: 52 (Sly) ×49, 58 (guards) ×32, 60 ×13, 53 ×13 | 433 | a class that counts |
| 486 | float | 7 | 0, 1.0, 15.0 | 434 | hold time [I] |
| 487 | bool | 77 | 1 | 436 or 438 | open |
| 488 | method, 4 arguments | 7 | (flag, object, object, object) | 439 | state-machine goals on enter / leave |
| 489 | method, 2 arguments | 108 | (task, state), e.g. (4644, 4) | 440 | task-state requests on enter |
| 490 | method, 3 arguments | 74 | (ID, object, −1) | — | open |
| 491 | command script (class 135) at +0x70 | 37 | script IDs | 441 | script started on enter |

Behaviour: Sly 2 section 5 [R]. Hiding spots (25), class 37 and the hooks (38): layouts in
[classes/paths-and-volumes.md](classes/paths-and-volumes.md); behaviour Sly 2 sections 4 and 5 [R]. Thrown objects
use the carry class 113 ([gadgets.md](gadgets.md)).

## 7. Open questions

1. Options 1130, 1178, 1175 and the water options 926, 927, 935.
2. Whether class 22 exists only in code.
3. Every [R] rule needs checking in play.
