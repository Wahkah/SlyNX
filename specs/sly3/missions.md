---
id: sly3-behaviour-missions
title: Sly 3 jobs and tasks - records, options, state rules, events, checkpoints, save keys - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), every class-168 and class-169 record of all 39 worlds (440 + 5,519 bodies) with their
      option values (untruncated survey); executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      option counts and values; the option table at 0x32CA80 (value types, fields, setters); the "task becomes
      current" routine at 0x188658; the player-control routine at 0x1F4B00 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/task3.cpp, scptrun3.cpp, game/gen (class and option correspondence tables)
    used_for: the Sly 2 to Sly 3 correspondence (rows scored 80 or more) and the statement that Sly 3 runs Sly 2's task rules; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 jobs and tasks

Read [../sly2/missions.md](../sly2/missions.md) first: the task graph, states (−1 unknown, 0 locked, 1 available,
2 done, 3 closed; switch groups 4 "became current" and 5 "stopped being current"), requests, propagation, job
begin / complete / end, failure and restart, the difficulty help and the script events carry over [R]. This
document gives Sly 3's classes, options and data, and what the executable confirms. Tags as in [guards.md](guards.md).

## 1. Records [D]

- Jobs are class **168** (options and a name, e.g. `m1_ball`); tasks are class **169** (options, a prerequisite list,
  a successor list, a name, e.g. `t0_ascend_complete`; [classes/simple-layouts.md](classes/simple-layouts.md) 5).
- Every world of an episode carries the whole episode's jobs and tasks, in its macro group. 440 job and 5,519 task
  bodies on the disc; distinct: **76 jobs, 965 tasks**, the same as the save tree ([saving.md](saving.md) 2).
- Per episode: see [gameplay-overview.md](gameplay-overview.md) 5.

## 2. Job options (class 168)

"Kind" is the value type and field from the option table [X]; "Sly 2" is the reference's correspondence [R] unless
marked otherwise.

| Option | Kind [X] | Sly 2 | Meaning | On the disc [D] |
|---:|---|---:|---|---|
| 1364 | task (object of class 169) at +0x54 | 1243 | **first task** | all 440 |
| 1365 | task at +0x58 | 1244 | **last task** | all 440 |
| 1370, 1371 | 16-bit string IDs at +0x60, +0x64 | 1245, 1246 | display name and description | 329, 357 |
| 1361 | bool at +0x6C | 1242 | **keep open**: the job does not close its last task and does not autosave on completion | 5 jobs, all 1 |
| 1381 | exit (object of class 24) at +0xC0 | 1259 | exit fired when the job ends with no next task ([level-flow.md](level-flow.md) 4.4) | 155 |
| 1379 | method, 6 arguments | 1256 | a state-machine goal applied when the job begins and ends | 34, e.g. (5, 69222, 69135, 13758, 65535, 0) |
| 1387 to 1390 | save paths at +0xE0, +0xE8, +0xD0, +0xD8 | 1260 to 1263 | save keys `time_played`, `suck_value`, `focus_count`, `is_complete` (by the path's last name ID: 88, 89, 86, 87) | all 440 |
| 1366 | task at +0x5C | — | the job's **resume task**: when the previous job completes, the job that follows it supplies this task as the forced checkpoint (section 4.1) [X]. Only on Kaine Island's jobs (tasks 1841 to 1843) | 40 |
| 1372 | screen object (class 158) at +0x78 | — | a screen shown for the job, e.g. 121565 `m2_shaman_info` [D]. *Interpretation:* the job's info card, the counterpart of Sly 2's "job complete" screen (1258) [I] | 22 |
| 1385 | item-table record at +0xF8 | — | an **item granted when the job completes** (38, 39, 40: the three disguises, and others) [X: section 4.1] | 21 |
| 1386 | task at +0xFC | — | a task paired with 1385 (3451, 2353, 3885, …); its use is open | 21 |
| 1362 | bool at +0x70 | — | **the job ignores the player's defeat**. The job's task-event handler (`0x00186730`) reacts to event **12** (the player's defeat, [level-flow.md](level-flow.md) 7) only when 1362 is 0. With 0 it calls `0x00186EC0`, or `0x00186FE8` when the job is not the current one; the latter requests the checkpoint reload (`0x00189F38`) and runs the job's failure path (`0x001878E0`) [X]. With 1 the job does nothing, so a scripted sequence handles the defeat [I] | 30, all 1 |
| 1363 | bool at +0x74 | — | open; the job's own methods also set and clear this field at run time (`0x0018B700`, `0x0018B780`) | 4 |
| 1367 | value type `0x108A` at +0x40 | — | **completion mode**: 2 = on completion the first task goes back to state 0, so the job can be played again; otherwise (1) the last task is closed unless 1361 is set [X: section 4.1] | 62: 1 ×37, 2 ×25 |
| 1368 | method, 1 argument (`0x001860D8`) | — | adds a number to a list of at most 8 kept with the job (count +0x44, list +0x48) [X]. The values (21, 26, 11, …) are task-event numbers. *Interpretation:* Sly 2's "events that count a failure or end the job" (1236 / 1237) [I] | 37 |
| 1369 | int at +0x4C | — | open (14, 20, 19) | 17 |
| 1380 | value type `0x1065` at +0xB0 | — | **the job's picture on the job screen**: a single bit (1, 2, 4, … 128) that `0x00186930` turns into a **part ID of the job screen object** (game state +0x433C), played by `0x00262450` and tested by `0x00260758` [X]. In the normal layout, bits 1 to 128 give parts 1086, 1088, … 1100 (1102 for any other value). A second layout gives 1068 … 1082 (default 1084), and a third, mode 2, gives 1104 to 1108. The layout comes from field +80 of the object at `0x004BC244`; outside it the job's own value is used (`0x00186900`). *Interpretation:* the eight pictures are the episode's job icons; the layouts belong to the single- and two-player screens [I] | 418: 1 ×152, 4 ×63, 2 ×55, … |
| 7 | method, 2 arguments | 7 | (1852, two floats), on 5 jobs | 5 |

Not found in Sly 3: Sly 2's job object switch (1255) and the two times of 1257 (the job timer thresholds); the
job's ending and closing events are task events as in Sly 2 [R].

## 3. Task options (class 169)

| Option | Kind [X] | Sly 2 | Meaning | On the disc [D] |
|---:|---|---:|---|---|
| 1391 | job (object of class 168) at +0x6C | 1264 | the task's job | 5,137 |
| 1392 | 16-bit string ID at +0x4C | 1265 | on-screen objective | 1,103 |
| 1398 | bool at +0x5C | **1271** | while available, the task already **counts as done** for its successors' prerequisites | 45, all 1 |
| 1399 | bool at +0x60 | **1272** | the task **becomes current as soon as it becomes available** | 3,010 (1 ×2,871, 0 ×139) |
| 1400 | arrival point (class 40) at +0xA8 | 1274 | **checkpoint** | 2,136 ([level-flow.md](level-flow.md) 5.3) |
| 1401 | 16-bit world slot at +0xAC | 1275 | the world the task starts in (15 ×296, 23 ×272, 3 ×220, …) | 2,136 |
| 1402 | method, 2 arguments | 1276 | the **character** the task starts with, and a second value (always −1): (24 Sly ×952, 25 Bentley ×385, 26 Murray ×306, …) | 2,136 |
| 1406 | exit at +0xC8 | 1278 | exit fired when the task becomes current | 104 |
| 1407 | object of class **120** at +0x70 | 1279 | an object present only while the task is available: the **mission waypoint** (`mission_waypoint`, `mission_interior_waypoint`, `mission_start_jt`) | 321 |
| 1408 | command script (class 135) at +0xCC | — | a script **started when the task becomes current** [X, section 4] | 3,563 |
| 1405 | bool at +0xC4 | — | the task is **done as soon as it becomes current** [X, section 4] | 514, all 1 |
| 1416 | method, 5 arguments | 1287 | **object switch**: Sly 2's four arguments (object, states list, owner, world) plus a fifth value (0) | 5,391 |
| 1417 | method, 7 arguments | 1288 | **state-machine goal**: Sly 2's six arguments plus a seventh (0) | 2,513 |
| 1423 to 1427 | save paths at +0xD0 … +0xF0 | 1295 to 1299 | save keys `focus_count`, `complete_count`, `is_complete`, `state`, `from_memcard` (name IDs 86, 166, 87, 167, 168) | all 5,519 |
| 1403 | float pair at +0xB8 | — | (900, 1,800) on 8 tasks. *Interpretation:* two times in seconds, like Sly 2's job times [I] | 8 |
| 1404 | int at +0xC0 | — | a world slot (15 ×32, 14 ×9, 23 ×5) [I] | 46 |

No task carries Sly 2's "resume as current after loading" (1273): no Sly 3 option corresponds [R], and none of the
unexplained options has its pattern [I].

### 3.1 Corrections to earlier working readings

| Earlier reading | Correct |
|---|---|
| 1398 = becomes current when available, 1399 = resume after loading | **1398 = counts as done** (Sly 2 1271), **1399 = becomes current when available** (Sly 2 1272). The values agree: 1399 is set on most tasks, as 1272 was in Sly 2 |
| "counts as done" not found | it is 1398 |
| job keep-open flag not found | it is 1361 |
| 1407 names a placement (class 8) | 1407 names a **class-120** object (the mission waypoint) |
| 1402 = (24, −1) a character | correct; it is a 2-argument method, not a plain object option |

### 3.2 Script methods of tasks and jobs [X]

Scripts reach these through the class option map (`0x002ED518`; [scripts.md](scripts.md) 5). **A script names the
script option ID** (left column), not the stream option. Each entry has the script-method flag (grf bit 0), a
handler and an argument count. Every handler checks that the receiver is of the right kind, calls the routine
named below, and returns nothing.

**Tasks** (class 169):

| Script ID | Stream option | Arguments | Handler → routine | What it does | Sly 2 (stream / script ID) |
|---:|---:|---:|---|---|---|
| **1309** | 1409 | 2: state, requester | `0x0010E090` → `0x00189078` | **requests a state** through the request queue (`0x00189240`, kind 0: checked). As in Sly 2, −1 or 4 makes the task current and 5 stops it being current [R] | 1280 / 1104 |
| **1310** | 1410 | 2: state, requester | `0x0010E138` → `0x001890B0` | the same request, **forced** (`0x00189240`, kind 1: no checks) | 1281 / 1105 |
| **1311** | 1413 | 1: integer | `0x0010E1E0` → `0x00189DA0` | raises script event 23 on the task with the integer | 1284 / 1106 |
| **1314** | 1418 | 0 | `0x0010E460` → `0x00189710` | **forces state 0**, then re-chooses the current job (`0x00189640`) | 1290 / 1110 |
| **1315** | 1419 | 0 | `0x0010E4D8` → `0x001897C8` | forces state 1, then re-chooses the current job | 1291 / 1111 |
| **1316** | 1420 | 0 | `0x0010E550` → `0x00189880` | forces state 2, then re-chooses the current job | 1292 / 1112 |
| **1317** | 1421 | 0 | `0x0010E5C8` → `0x00189938` | forces state 3, then re-chooses the current job | 1293 / 1113 |
| **1318** | 1422 | 0 | `0x0010E640` → `0x00188540` | **makes the task current** | 1294 / 1114 |

- 1315 and 1316 carry grf `0x4001` (one extra flag); the others carry `0x1`.
- The four force routines are the same code: each runs the forced request (`0x001890B0`) with its constant (0, 1,
  2, 3), then `0x00189640`.
- Read-only task properties (grf `0x80`, value at an offset): script IDs 1292 (stream 1423, +0xD0), 1319 (1424,
  +0xD8), 1293 (1425, +0xE0), 1320 (1426, +0xE8) and 1321 (1427, +0xF0). They are Sly 2's state, play count, done,
  current and state-string reads [I by the layout].
- Other script-visible task entries: 1302 (stream 1402, a 2-argument stream method; grf `0x401`), 1313 (1416, the
  object switch), 578 (1417, the state-machine goal). The stored fields are reachable through their script IDs, for
  example 1307 → 1407 (+0x70, the waypoint) and 1295 → 1391 (+0x6C, the job).

**Jobs** (class 168), the script methods (meanings by Sly 2's order [I]; handlers [X]):

| Script ID | Stream option | Arguments | Handler → routine | Sly 2 counterpart [I] |
|---:|---:|---:|---|---|
| 1259 | 1355 | 1 | `0x0010DB10` → `0x00186E80` | 1238 count a failure |
| 1260 | 1356 | 2 | `0x0010DB98` → `0x00186EA0` | 1239 restart at the checkpoint |
| 1261 | 1357 | 2 | `0x0010DC40` → `0x00186EC0` | 1240 end the job; also the defeat path of 1362, section 2 |
| 1262 | 1358 | 3 | `0x0010DCE8` → `0x00186EE0` | (no direct Sly 2 match) |
| 1263 | 1359 | 0 | `0x0010DDA0` → `0x00187040` | 1250 reset the play counts |

Sly 3's job methods take arguments where Sly 2's took none. Their meanings are open (section 7).

**For the Venice job start:** a script that starts `t1_recon_intro` must call script ID **1309** (request, usually
state 4 or −1 to make it current, or 2) or **1318** (make current) on task 2086, or the forced forms. A call that
names Sly 2's IDs (1104 to 1114, or the stream IDs 1280 to 1294) reaches nothing in Sly 3. Script IDs 1280 and
1281 exist on tasks, but they are the stored fields +0xFC (state) and +0x100 (stream 1411, 1412). So an engine that
maps 1280 / 1281 by Sly 2's meaning writes the state field directly instead of making a request.

## 4. A task becoming current [X]

The routine at `0x00188658` (Sly 2's "become current") does, in this order:

1. Applies switch group 4 ("became current") through `0x00185DB0`.
2. Shows the task's objective prompt when it has one (field +0x4C).
3. Raises script event 22 on the task (`0x188780` onward).
4. If 1408 names a command script, starts it (`0x00226010`, with player 0's controlled object as its context [I]).
5. If **1405** is set, requests state **2** (done) at once (`0x00189078(task, 2, 0)`).
6. If 1406 names an exit, fires it.

The rest (previous task stops being current, play count, checkpoint remembered, job begins) is Sly 2's [R].

### 4.1 A job completing [X]

The routine at `0x00187270`:

1. If the job's 1367 is 2, the **first task goes back to state 0** (`0x00189B08(first task, 0)`). Otherwise, unless
   1361 is set, the last task is closed (state 3).
2. If 1385 names an item, the item is **granted** (`0x0021F5D0(item, 1)`: `0x0021F510` with the value 1, then a refresh).
   *Interpretation:* that sets the item's owned flag [I].
3. Among the last task's successors, it finds one whose job has a resume task (1366). If there is one, that task's
   ID and world are written to `oidCheckpointForce` and `widCheckpointForce` ([saving.md](saving.md) 3). Starting
   episode 8 then resumes there ([level-flow.md](level-flow.md) 3).

### 4.1a World start: restoring tasks and the "current when available" rule [X]

Every world start runs `0x00184B60`, called at `0x0025C8C0` in the world's start-up (`0x0025C2A8`). This includes a level load with
progress restored and a reload at a checkpoint. In order:

1. **Restore.** Every task is restored from the save (`0x00188CB0`), prerequisites first (the visit order from
   `0x0018A900`). For the call, the task's mode (+0xFC) is set to 1 and +0x100 to 0, and its job's +0xF0 / +0xF4
   to 1 / 0. They are put back afterwards. The restore calls the state setter directly (`0x0018A3E8`), so it does
   **not** back-fill prerequisites (below).
2. **The saved keys.** The game-state keys for the job (+0x1498) and the active task (+0x149C) are looked up. A job
   found is re-begun (`0x00186248`), and a task found is **made current** (`0x00188540`), both in mode 1 (the
   same mode values as step 1).
   Both keys are then reset to −1.
3. **"Current when available" (1399, field +0x60).** The request queue (`0x0036C6E8`) is held and its pending
   requests run (`0x00189160`, `0x00189180`). Then the game scans the **available-task list** (`0x00479498`, linked
   through +0x78). It keeps the **last** task with 1399 set that is **not the current task** (`0x00188A20`, current
   task at `0x004794C8`). If it finds one, it releases the queue and issues a **forced request for state 4**, make
   current (`0x001890B0(task, 4, 0)`). It repeats step 3 while the queue (`0x0036C6F0`) still holds requests. When
   no task qualifies, it stops.
4. A forced state-4 request (`0x00189500`) makes the task current through `0x00188540`. If the task is still
   locked (state 0) and its prerequisites hold (`0x00189008`), it first sets it available (`0x00189B08(task, 1)`).

**The available-task list holds every available task of the world, from every job and from none** [X]. The
state setter (`0x0018A3E8`) appends a task to `0x00479498` when its new state is 1 (`0x0011AF60`) and removes
it when it leaves state 1 (`0x0011B028`). Nothing else adds to the list, and no job test is made. The broadcast
of 4.2 filters by job when it delivers; **this pass does not**: it compares only 1399 and "not current".
So if a task of another job (for example an `m1_recon` task) is available with 1399 set at the world start,
the game makes it current too. A wrong result on a new game in Venice therefore points to that task's state:
it should not be available yet, because its prerequisites are not done. Limiting the pass to the current job
is not the game's rule.

**Jobs and tasks of other worlds are loaded too** (correction) [X]. The object loader (`0x00151D80`) skips a
mode-1 record whose world filter (`0x00151C98`, against the current world at `0x0047989C`) or conditions
(`0x00151D10`) do not match, **except classes 168 and 169**. A job or task of another world is created and loaded
anyway, with a marker set: a task gets +0x40 = 1, a job +0x50 = 1 (`0x00151F68` to `0x00151FB4`). A task with
+0x40 set fails the "live" test (`0x001880F0`), so it receives no task events. Every job and task of the episode's
macro group is therefore present in every world of the episode.

**Prerequisites and the restore** [X]:

- Prerequisites are **list A** of the task record and successors are **list B**
  ([classes/simple-layouts.md](classes/simple-layouts.md) 5). At post-load (`0x001881F0`) each list's OIDs are
  looked up (`0x0017E1B8`), and the counts (+0x84, +0x90) become the number found. Because other worlds' tasks are
  loaded, only an OID missing from the whole file is dropped.
- **Restore (`0x00188CB0`):** it reads the saved state (key at +0xE8) and the saved `from_memcard` flag (key +0xF0).
  - If the flag is set, the task has a job and the saved state is 1 or 2, the per-job counters are reset and the
    state becomes 1 when every prerequisite is at least 2, else 0.
  - Otherwise a saved state of 0 becomes **1 (available) only when the task has no prerequisites at all**.
  - The result goes to the state setter (`0x0018A3E8`), which appends the task to the available-task list.
  - A task never written to the save reads 0, so at a new game **only tasks with an empty list A open**.
- **Visit order** (`0x0018A900`, `0x00188B08`). The order starts at every root (no prerequisites) and walks down
  the successors. A task joins the order once all its prerequisites have been visited. Each visit records:
  - +0x104, the visit count;
  - +0x108, the **depth**: the largest depth arriving over any path (each step adds 1);
  - +0x10C, the **job-change depth**: the largest value arriving over any path. A step adds 1 when it goes from a
    task with a job to a task of a different job.

  Both are kept as separate maxima (`slt` / `movz` / `movn` at `0x00188B48` to `0x00188B5C`), so equal paths change
  nothing. The +0x10C kept need not come from the deepest path. A task's successors are visited only after its
  last prerequisite, with the task's final maxima.

  The order is then sorted (`0x0018A890`) by +0x10C, then the job's OID (−1 for none), then +0x108, then the task's
  OID. The restore appends to the available list in this order.
- **The 1399 pick** is "the last in the available list with 1399 set that is not current", with no job,
  episode or job-key filter. That is safe because at a new game the list holds only the roots.
- **New game in v_ext (world 3)** [D, simulated from the decoded records]. 150 tasks are loaded (all of `v_macro`).
  The only root is **2064 `t1_intro`** (no job, no 1399; successors 2083 `t1_intro_outside_safehouse` and 2408).
  So the available list holds only 2064. The 1399 pass picks nothing, and the job and task keys are −1
  ([saving.md](saving.md) 3), so **no task is current after the world start**. The episode's macro module then
  plays the intro and advances 2064, which opens 2083 (Sly 2's episode-intro rule [R]).
- Before the first world load, the game initialisation (`0x00184968`) sets the job, task and checkpoint keys to −1.
  Starting episode 3 chooses no task, so the world loads at point 424 ([level-flow.md](level-flow.md) 3) [X].
- **2091 in v_interpol and 2123 in v_ext** keep their prerequisites, 2089 and 2116 (both present, loaded as
  other-world tasks). Each opens through the ordinary propagation when its prerequisite is done, and not at load.
  The earlier "in-world prerequisites only" reading was wrong.

**What advances `t1_intro` (2064): the Venice intro script 2065, started by the task's own module** [D] [X]:

- **The handler** is in v_macro module **8** (at `0xB7D855` in `03_v_ext`), which **task 2064 itself** names (its
  record's module index is 8). Decoded [D]:
  `(define s24 (lambda (s1240) (if (op7 (op117 s1240 525) 1) ((op117 (op118 2065 obj0) 247) obj0))))`.
  - Correction: an earlier count skipped modules whose `begin` holds several `define`s, which put it three too low.
    Walking the block in order gives 144 modules: module 4 is 2058's (an `s20` handler), module 5 is 2059's
    (defines s1587 and s0, then the safe house 15065), module 8 is 2064's, module 9 is 2065's.
  - It handles **script event 24**. The task's state setter (`0x0018A3E8`) raises event 24 on the task after each
    state change of a live task (vtable slot 3, which first runs the object's own module). It also raises it
    through slot 4 (`0x0013BE38`), which forwards it to the objects linked at +0x8 [X].
  - s1240 is the task. Its script ID 525 is stream option 1415 (+0x44, the state), so the condition is **"the new
    state is 1 (available)"**. The action is script method 247 on command script **2065**, which starts it (stream
    option 747, handler `0x00108918`).
- **When it fires.** On a new game the restore makes 2064 available (state 0 → 1) in mode 1. Mode 1 still counts
  as live (`0x001880F0` blocks only mode 3 and other-world tasks), so event 24 is raised and the intro starts at
  the world start.
- **What 2065 does** [D, from its 120 commands]:
  - a pause (`0x1F` (1, 3));
  - the intro scene: Bentley (runner 24) and Murray (runner 25) are added and walked through it, guard commands
    `0x62`, animations `0x16` / `0x30`, camera and state-machine goals;
  - a button prompt (`0x4E` 2048) on its own runner (15067);
  - command `0x12` 2062 (the screen fade, scripts.md 7.3);
  - **`0x1E` (2064, 3)**, a checked request 1 → 3 that closes `t1_intro`;
  - command `0x12` 424, which activates object 424, the default arrival point (`v_ext_rig`);
  - finally the cast is released and removed, and object 15152 is added back.
- **The effect.** Closing 2064 opens its successors **2083** `t1_intro_outside_safehouse` and **2408**
  `t1_cops_robbers_sentinel`, whose prerequisites are now done. 2064 has no prerequisites, so nothing is
  back-filled. After that, script 2061's requests for 2083 can succeed.
- **Expected new game in v_ext:** 2064 available → event 24 → 2065 runs the intro → 2064 closed → 2083 and 2408
  available. **Option 1399 defaults to 0**: the task's load-time setup (`0x00187E90`) does not write +0x60, so a task without the option keeps 0 [X; I that the object starts zeroed]. 2083 does not list 1399, so it does **not** become current by the 1399 rule. The setup does set the state (+0x44), +0x4C, +0xA8 to +0xB4, +0xC0 and the pending state (+0xF8) to −1, and the mode (+0xFC) to 0.
- A second module also names 2064: in `v_ext_rig`, module `0xAF50B0` handles event 5,
  `(if (op117 (op118 2064 obj0) 1278) …)`, using task script ID 1278 (stream 1395, read by `0x00188A38`). It then
  calls the game object's method 983 with (0, safe house 15065). That is the safe-house arrival, not the intro.

**`t1_recon_tumble_lock` (2115): done when the cell lock receives event 5** [D] [X]:

- Task 2115 (`m1_recon`; prerequisite 2112, successor 2116) names v_macro module 21, decoded:
  `(define s20 (lambda (s1240 s1579 s1580) (case s1579 ((29) (if ((op117 s1580 32) 138191) ((op117 s1240 1309) 2 s1240))) ((18) ((op117 (op117 s1240 1295) 1260) 1563 2101)))))`.
  - Event 20 is the task event of 4.2: (code, object). It reaches 2115 while it is available, live, and of the
    current job.
  - **Code 29:** script method 32 (stream option 18, `0x001008D8` → `0x001C8768`) asks whether the object is
    138191 itself or a copy named 138191. If so, the task **requests its own state 2** (script ID 1309, 3.2),
    which opens 2116 `t1_recon_escape`.
  - **Code 18:** it calls the job's (1295 → 2085) method 1260 with (1563, 2101): a job method (3.2) [I: the
    restart / failure path].
- Task event 29 with object X is broadcast whenever **X receives script event 5**. The shared event handler
  (`0x001C86B8`, 4.2) does this for every class that uses it, the lock (class 79) included.
- **The chain:**
  1. 2115's switches add use point 138208 and trigger 138189.
  2. Script 2113 makes Sly use 138208.
  3. The point's module starts lock 138191 with the user, and player 0 controls the lock.
  4. When the lock gets event 5, 2115 is requested done.
- **Partly settled** (the lock minigame is in [interactables.md](interactables.md) 5): `lock_sequences` object 113853 passes an event 5 on to its owner, the lock, through script method 12 (raise event 5). What raises event 5 on 113853 is still open. Earlier notes: The lock's own code (`0x001C9B00` to `0x001CB400`) raises no
  event 5. On entering state n it runs the command script at +0x19C + 4n with the user (`0x001CAC30` →
  `0x00225EE8`). No script in v_interpol uses `0x12` on 138191, and `v_cell_lock` holds no command scripts.
  The likely sources are the lock's completion state or the control hand-back. The lock's states run 0 to 5:
  losing control below 4 resets to 0, and losing control at 4 or 5 goes to state 5 (`0x001CAAC0`). State 3 runs
  the solve effects (`0x001CAD10`, `0x001CB008`). Tracing which of these raises event 5 is still open.
- The trigger 138189 runs script 2098, which is a guard's dialogue loop, not the completion.

**The task mode, event 24 and requests during the restore** [X]:

- **Script ID 1280 on a task is the mode.** It is stream option 1411, value type `0x1048` at +0xFC (3.2). The
  values are:
  - **0** in live play;
  - **1** while the restore sets the task, and in step 2 of the world start;
  - 3 for silent changes (no events, `0x001880F0`).

  The implementer's reading is right: `(s1280 = 1)` means "being restored".
- **Event 24 during the restore.**
  - Each task starts at state −1 (load-time setup `0x00187E90`). The restore calls the state setter
    (`0x0018A3E8`) for every task in the world, once.
  - The setter returns at once when the new state equals the current one (`0x0018A408`), so no event fires on
    an unchanged state.
  - Otherwise it raises event 42 (before) and event 25 on the task, sets the state, then raises event 41 and
    **event 24**. All of this is gated only by the live test: in this world (+0x40 = 0) and mode ≠ 3.
  - So during the restore **every in-world task receives event 24 once, whatever state it gets, 0 included**,
    because −1 → 0 is a change. Other-world tasks (+0x40 = 1) receive nothing.
- **Requests made by those handlers are deferred until step 3** (`0x00189240`).
  - A request is queued as {kind, task, state, mode, requester}, and the **mode recorded is the task's mode at
    the time of the request** (+0xFC).
  - It runs at once only when the queue's run flag (`0x0036C6E8`) is 1 and it is the only entry.
  - The world start-up (`0x0025C2A8`) resets the flag to 0 (`0x00189118`, at `0x0025C850`) before the task
    world start (`0x00184B60`, at `0x0025C8C0`). So requests made during the restore and step 2 wait.
  - At step 3 the flag is set to 1 and the queue runs in order (`0x00189180` → `0x00189300` for checked requests,
    `0x00189500` for forced ones). Each request runs with its recorded mode, so 1.
  - Then the 1399 pass runs. After the world start the flag stays 1, so live requests run immediately (or after
    the one in progress).
- **For 2136:** restored as available in mode 1, its handler (`state = 1` and `s1280 = 1`) requests state 4.
  The request is queued, then runs at the start of step 3, in mode 1, and makes the task current
  (`0x00188540`). The 1399 pass that follows takes the last available task with 1399 set that is not current,
  so it can still move "current" to another task.
- 2138's `(state = 3) and (s1280 = 0)` therefore fires only on a live close, not when a closed task is restored.

**Starting command scripts from modules** [X]:

- Script ID **247** on a command script (class 135) is stream option 747, handler `0x00108918`. It starts the
  script **with a context object**, its one argument (`0x00225EE8(script, context)`).
- Script ID **733** is stream option 748, handler `0x001089A0`, with no argument. It starts the script with
  its default context (`0x00226010`, the same routine a task's 1408 script uses, section 4).
- **Starting a script** (`0x00225EE8(script, context)`) stores the context at +0x58 and then **raises script event 5
  on the script** (vtable slot 3, `0x00225F14`). It then starts its runners. The default start (`0x00226010`) uses
  **player 0's controlled object** (`0x001F4AD0`) as the context.
  - So every command script gets event 5 when it starts. Through the shared handler (4.2) that also broadcasts
    task event 29 with the script.
  - Script ID **740** on class 135 is stream option 756, the object at **+0x58**, i.e. **the context the script
    was started with**.
  - Take-out script 2061's `(op117 itself 740)` is therefore the character it was started for. Its event-5 handler
    runs at start, stores that character, and hands it control through method 983.

**Task 2268 `t1_follow_listen_to_lion_boss`** [D]:

- 2268's module (v_macro) answers task event 29 from object **139626** by starting script **2269** (ID 733).
- **139626 is in v_ext.** It is a **path (class 10)** nested inside the ferris-wheel object 139617 (group
  `v_ferris_wheel`, record at `0x007210EC`, 180 bytes: a transform and curve points, no options). A loader that
  skips nested records under class-47 objects misses it.
- Script 2269 is the scene where the lion boss talks at the ferris wheel. It:
  - holds the player (`0x1F`) and sets the context to the wheel;
  - plays camera keys `0x45` and turns the characters to face points;
  - gives the lion boss lists 2271 and 2270;
  - plays dialogues 1354 and 1355;
  - drives the wheel's state machine (`0x19` 139617 / 139618 → 93932);
  - places the player on 139627 (`0x22`) and removes 93224 and 93167.
- **Load condition 14** [D]. The ferris wheel (group `v_ferris_wheel`, root 139617) is a mode-1 record with
  condition 14. v_ext's option sets are 22, 19, 20 and 23, and **condition 14 is an extra condition (option 1029)
  of sets 22 and 19**:
  - 22 adds 14 and 17;
  - 19 adds 14 and 16;
  - 20 adds 15 and 16;
  - 23 adds 15 and 17.

  Table B gives condition 12 to every m1_follow task (2237 to 2266). 12 counts as none, so the world loads the
  first set, **set 22**, and the wheel loads ([conditions.md](conditions.md) 3.1) [X]. Condition 23 (sets 23 / 20, extra 15) belongs to later tasks (1826, 2141 …), and 20 to 1811 and
  table A's 2409.

  An engine that does not add a set's 1029 conditions to the match, or that picks set 20 or 23, misses the wheel.
  The choice is made at world load. No code was found that re-evaluates conditions during play [I]. So a later
  task that needs another set relies on a reload, for example an exit or checkpoint.
- **Open question:** what raises script event 5 on path 139626. The code-29 broadcast follows from any event 5
  on it (4.2). No script uses `0x12` on 139626, 139617 or 139637 in v_ext, and the path carries no options.
  *Interpretation:* a character's path movement raises it on reaching or entering the path, the listening spot
  on the wheel [I, not traced].

**Crew members removed by scripts** [D] [X]:

- In v_ext the crew records are placements in `v_ext_rig`: 24 Sly (class 52 body in `jt`), 25 Bentley, 26 Murray.
- The Venice intro (script 2065) ends by removing 24 and 25 and adding **15152**, a class-33 use point in
  `v_ext_rig`, the safe-house door. The rig's arrival module then gives player 0's control to the **safe house**
  15065 with the game object's method 983 (`0x0010A9D8` → `0x001F4B00`).
- So after the intro the player is the safe house, and Sly is out of the world on purpose. A crew member comes
  back when the player leaves the safe house with a character (the safe house's own choice, Sly 2 rules [R]), or
  when a later script adds him (`0x38`). The game does not skip crew members on a take-out.

**Simuldoors (v_gauntlet, tasks 2289, 2293, 2318): a two-button timing test** [D] [X]:

- **Starting it.** Trigger 140793 (for 2289; 140795 for 2293) is a class-21 trigger present while its task is
  available (1416). Its module (v_gauntlet_gauntlet modules 11 / 10) starts script 2292 with method 247. Per the
  implementer's reading, the triggers make the task current and leaving them resets an unfinished attempt.
- **The task script 2290** (2289's 1408):
  1. dialogue 1278, then starts **2291** and waits for it (`0x17` with the wait flag);
  2. dialogue 1287;
  3. opens the door: state machine 4935 on 139703 → 13759 (`0x19`). 2289's 1417 entries give the same door state
     13758 in states 0 / 1 and 13759 in states 2 / 3;
  4. walks Bentley (25) through the door (`0x33`, `0x34` and `0x30` toward 139704 to 139706);
  5. requests **2289 → 2** (`0x1E`). That completion is the script's last step.
- **The test, script 2291** (module 71 of v_macro):
  - **Module state.** t1 and t2 are times. The window starts at 0.25 plus a term from the player's adaptive
    difficulty: the module computes `(op0 0.25 (op3 (op107) 2.0))` (built-in 107 = the adaptive-difficulty value).
    `success` is false at start (event 5).
  - Its event-4 hooks, raised by the script's `0x15` commands, are:
    - 50517: reset t1 and t2;
    - 139707: set the calling runner's context to a marker chosen by the current task (2289 → 139711,
      2293 → 139712, 2318 → 139713);
    - 50526: t1 = now (built-in 101);
    - 139710: t2 = now;
    - 50521: success = |t1 − t2| compared with the window, then window × 1.25. It returns success and is used as
      the `if` condition.
  - Custom method 2095 returns `success`.
  - **Runner A** (Bentley's side), after hint 1293 for 20 s (`0x60`):
    1. hook 50517, then countdown dialogues 1288 to 1290;
    2. hook 139707;
    3. Bentley's interaction object 36937 (`0x29`);
    4. **hook 50526 (t1)**, and press button 139708 (`v_button_gauntlet`, state machine 50519 → 50520);
    5. wait for the dialogue;
    6. **if 50521 succeeds**, rendezvous of runners 24 and 25 at 50522 and finish; otherwise a fail line
       (1279 to 1286) and repeat.
  - **Runner B** (Sly's side): loops on "wait for **alarm 139709**", then **hook 139710 (t2)**.
- **Who sets alarm 139709.** The buttons placed for Sly carry it. Placements 145224, 145562 and 145571 (in
  `v_gauntlet_gauntlet`) set custom properties **2199 = 2291** (the script) and **2200 = 139709** (the alarm)
  with option 8. The button templates' module (in `v_button_signal` and `v_button_gauntlet`) handles **event 4**:
  it reads its owner's (+0x10) 2199 and 2200 and calls the script's ID **735** with the alarm. ID 735 is stream
  option 751, handler `0x00108A90` → `0x002261A0`, which **sets that alarm on the script**.
  - So Sly pressing his button at the marker sets alarm 139709.
  - **Open:** what raises event 4 on the button (the press interaction and its prompt) was not traced. The
    button-press animation's hook is the likely source [I].
- **Script ID 741 on a command script** (stream option 757, accessors `0x002262F0` / `0x00226320`) reads and
  writes **the context object (+0x1C) of the runner that is calling the module**. The script's current runner is
  at +0x54, and the call only applies while that runner is executing a `0x15` hook. So 139707's `(op116 s1240
  741 marker)` makes Bentley's runner act relative to the marker.
- **Commands:**
  - `0x60` (string, seconds) shows a hint (`0x0021E870`); `0x61` clears it (`0x0021E930`).
  - `0x33` is a character's movement step along a path (it starts AI job kind 243); `0x34` is a step toward an
    object's aim point (job kind 244).

**Waits and rendezvous** [X, the commands' done-checks; jump table at `0x00326840`, by kind − 12]:

- **`0x0E` (alarm, clear)** passes when the alarm is set (`0x00226270`). When its second argument is non-zero it
  **clears the alarm** as it passes (`0x002261F0`). So a loop that waits with a non-zero second argument waits
  again for the next setting.
- **`0x0D` (runners, ID)** is a rendezvous. Each named runner other than this one must currently be at a `0x0D`
  whose **ID word is the same** (2291's 50522). When all the others are there (count − 1), each is marked
  released (+0x108) and every one continues. The trailing word is therefore the **meeting ID**, which tells
  rendezvous points apart.

**Bomb doors (v_gauntlet, task 2302 `t1_gauntlet_bomb_door_1`): blow the door open with one of Bentley's bombs**
[D] [X]:

- The door is copy 140958 (template `v_int_door_reinforced`, class 47), placed by placement 140800. The
  placement's option-8 properties are:
  - 2731 = the task;
  - 2728 = 145149 and 2729 = 145147, two class-145 objects of `v_gauntlet_pz`, the AI path-zone nodes on either
    side. *Interpretation:* they let the AI route through once the door is open [I].
- **The door's module handles script event 12 (a hit), with values (door, hit record):**
  `(if (and task-found (op12 (op117 hit 556) door) ((op117 (op117 hit 276) 3) 114)) (begin door state machine
  4935 → 13759 ; task 1309 → 2)))`.
  - The hit record s1253 is an internal class-196 record, not a world object. Its script ID 556 (stream option
    1646, +0x1C) is the object hit; its ID 276 (stream option 1644, +0x14) is the hitting object.
  - Script ID 3 (stream option 2, `0x00100228` → `0x0013B500`) is "derives from class n".
  - So the three tests are: the task exists; the hit's target is this door; **the hitter is a class-114
    object, one of Bentley's bombs** (gadgets.md 2).
  - Then the door opens (state machine 4935 → 13759) and the task requests state 2. Task 2302's own 1417 entries
    keep the door at 13758 in states 0 / 1 and 13759 in states 2 / 3.
- **Script 2303** (2302's 1408, run when it becomes current; 2302 has 1399):
  - Sly walks to 139722 and turns to Bentley (`0x77` = turn a character to face an object);
  - state machine 5099 on 139723 to 139725 → 1765 (the markers);
  - Bentley runs `0x76`, then seven `0x36` steps through 139726 to 139732 (`0x36` = move a character to an
    object, or where it stands; the second argument 0 or 1 chooses), and the characters face each other again.
  - The scene ends there, and play goes on.
- **How it is played** [I for the input, from gadgets.md]: the player, as Bentley, uses a bomb gadget (class 114,
  on one of the L1 / L2 / R2 slots) by the door. The bomb's explosion hits the door. The hit raises event 12 on the
  door with a hit record whose hitter is the bomb, so the door opens and 2302 is done.
- `0x76` (`wbbbb`, 1,331 uses) is still not established.

**Laser rooms (v_gauntlet, task 2314 `t1_gauntlet_lasers_2`)** [D] [X for the IDs]:

- **Becoming current.** Trigger 140810 (present in state 1 through 2314's 1416) carries option 489 (2314, −1).
  So Sly (class 52, its counted class) entering it **requests 2314 make-current**.
- **Script 2315** (2314's 1408):
  1. takes instances of placements 139751 and 139752 (`0x6F`), two guards (class 58, template 56250, removed at
     the start);
  2. gives each the command list **2316**, a single command 740, kind 13 "**use an object**": object 140811
     (class 3, an anchor point in `v_gauntlet_gauntlet`), flags (1, 1). Each guard goes to that point and uses it
     (Sly 2's use command [R]);
  3. **adds trigger 139753** (`0x38`).
- **What finishes 2314: trigger 139753, once no guard is left inside it** (correction) [D] [X].
  - The trigger counts **class 58 (guards)** (option 483 = 58) and keeps its member set (option 478 = 1).
  - **Event 9 on a trigger means "the set of members changed"** (`0x0015D510`). Each frame, with the set kept, the
    trigger collects the members inside (up to 64, `0x0015D2F0`), sorts them, and compares them with the stored
    set (+0x23C count, +0x240 array). On any difference it stores the new set and raises event 9. It also raises
    event 9 with an empty set when it leaves the world (`0x0015CD50`). So event 9 covers both entering and
    leaving.
  - On a trigger, script ID 43 is stream option 28, the **in-world flag** (byte +0x2F bit 0, `0x002BE5E0`). ID 586
    is stream option 479, **the current member set** (`0x0015D6A0`, from +0x23C / +0x240). Built-in 25 is "is the
    empty list".
  - The module, `(if (and in-world (empty? members)) (begin (start 2317) (remove self)))`, therefore fires when
    **the last guard leaves the trigger** (or is taken out). Event 18 also removes the trigger.
  - In play the two guards walk to anchor 140811 inside 139753 and stay there. **The player has to get both
    guards out of the zone**, by knocking them out or luring them away, to open the way.
  - A knocked-out guard leaves the zone because its knock-out clip removes it from the world with an animation
    event of kind 8 ([guards.md](guards.md) 4b).
- **Script 2317:**
  1. removes **139754**, a class-47 object of `v_gauntlet_rig` that 2314's 1416 keeps in states 0 and 1. It is the
     laser barrier [I];
  2. Bentley (25), under `0x76` (25, 0, …) … `0x76` (25, 1, …, 16), uses 139755 and walks 139756 → 139757
     (`0x33` / `0x34`, speed 40);
  3. requests **2314 → 2** (`0x1E`).
- **`0x6F` / `0x70`** (handlers `0x00228FB8` / `0x00229040`) take or release a run-time **instance of a
  placement** (class 8, found by OID). `0x6F` takes the current instance (flag 1) or a new one, through
  `0x001FBF08` / `0x001FBE30` or `0x001FBEC0` / `0x001FC090` / `0x001FC0E0`. `0x70` releases it. The instance
  is the placement's template copy, placed at the placement. Flag 2 locks its row [R: the reference's
  description; the instance counts were not traced].
- **Guard command 740** (kind 13, guards.md 4a) makes the guard go to the object and use it, holding the list until
  the use ends [R].

**Control at the bomb door (2302)** [D]: script 2303 contains no `0x13` and no game-object method 983. It only
walks Sly (24) and Bentley (25), turns them to face each other (`0x77`), and steps Bentley with `0x36`, between
`0x76` (25, 0, −1, −1, 0) and `0x76` (25, 1, −1, −1, 16). So **2303 does not hand the player Bentley**.
*Interpretation:* Bentley is an AI partner here, and `0x76` toggles a scripted mode on him [I]. **Open
question:** how a class-114 bomb comes to hit the door in play (Bentley's AI, or the player switching). Neither
was traced. Nor was the gadget-slot pad path (gadgets.md).

**Brawl spawners and related methods (v_gauntlet, 2322 / 2327)** [X, with names from the reference's function
map; partial]:

- **Script ID 825** exists only on placements (class 8): stream option 1282, handler `0x0010D2B8` →
  `0x001FBE30` (the reference's PloAcquireProxyObject). It **takes a new instance of the placement** and returns
  it. That is the script-side form of command `0x6F`.
- **Script ID 35** (stream option 21, two arguments, handler `0x00100958` → `0x001C7A00` → `0x001C7A30`)
  **re-parents an object** under a new parent: it changes +0x10 and updates the parent's child lists and the
  world's root list (`0x00479278`). The spawner uses it to hang the new instance under its spawn point.
- **Built-in 113** postpones an object's updates (scripts.md 4). The spawner's "timer" is the per-frame
  **event 1** (frame time) that the object's module defines, as in Sly 1 / 2. Built-in 113 delays the next
  updates; it does not raise an event of its own [R].
- **Commands:**
  - `0x1D` (six floats) is a **time-scale ramp** (`0x0022A430`; scripts.md 7.2);
  - `0x43` **plays a sound** through `0x0025DB38` (with a pitch offset);
  - `0x44` (sound) **stops** it (`0x0025DCA8`, the reference's StopAsegaSound).
- **Class 163 (`jt`'s screen object 139786):**
  - script ID 1149 (stream option 1215) calls the screen object's vtable slot 56;
  - 1150 (stream option 1216) calls slot 54, the **hide** slot (+0x1B0, scripts.md 7.3);
  - 1153 (stream option 1220, two arguments) calls `0x00261580`, a screen-object setter.
  - The exact meaning of slot 56 is not settled [I: show or reset].
- **Script ID 424 on a guard** (class 58, stream option 297, one argument) returns an integer computed from a
  vector field of the guard (`0x002533C0` → `0x00253190`). Its meaning is not settled.
- **Crew option 938** (stream option 936, value type `0x102A`, read through an accessor; Sly 2's 1111 in the
  reference's option map) is an enum on the playable characters. **Whether 2 means "down", and how guards' strikes
  damage the crew in Sly 3, are open questions.** Sly 2's health rules (Sly 2 characters.md 2) are the reference
  until traced.

**Option-set choice at a world load** ([conditions.md](conditions.md) 3.1) [X]. The choice is made in this
order: the current job's table-A entry, then the checkpoint task's table-B entry, then the **last table-B entry
whose task has a saved state above 0** (available counts). 12 means none, and with no match the first set loads.
For m1_turf_war: 1826 `t1_chalktalk2` is current (state 1) when v_ext loads, so condition 23 is chosen and set
23 loads.

**Turf-war battles (m1_turf_war, job 2139, battles 2151 / 2155 / 2159 / 2163): Carmelita against waves**
[D: job 2139's module, v_macro module 28 in v_ext; I where marked]:

- **Set-up.** Each battle task's script (2152 …) removes Sly (`0x39` 24) and gives player 0 control (`0x13`) of
  a **Carmelita** copy taken from pool placement 139252 (class 60, a guard-family playable character). Then it
  loops, sending the hook 139266, until **alarm 139255** is set. Each battle module's hook 139345 calls the job's
  custom method **2653 with the battle number** (1 to 4). Hook 139266 calls **2658**, the wave tick, every loop.
- **Methods the job registers** (built-in 114, on its event 0):

  | Method | What it does |
  |---|---|
  | **2653** (n) | **start battle n**: take Carmelita from pool 139252 (1207 / 1209) and the boss from 139253, set up the battle's row of the wave table, add 93224 when the row asks, and run the job's 286 |
  | **2654** | end the battle: release every enemy, return the boss and Carmelita, remove 93224, run 287 |
  | **2658** | **the wave tick** (below) |
  | **2659** (enemy, hit) | an enemy's damage handler (below) |
  | 2655 to 2657, 2660, 2661 | follow-target set-ups, the list of living enemies, and a voice line |

- **The wave table** (s6068), one row per battle: (boss flag, list of wave sizes, list of spawn choices, the
  most enemies alive at once, (two anchor objects)). For example, battle 1 is
  `(0 (1) ((139222 139223 139224 139225)) 1 (139226 139227))`. Each spawn choice is (pool, spawn point, path
  in, path out).
- **The wave tick, 2658:**
  1. each living enemy's state machine runs (s6101), and enemies that have left are dropped;
  2. while fewer enemies have spawned than the row has entries, and fewer are alive than the row's limit, it
     picks a spawn choice that no living enemy uses and spawns an enemy (s6117). The enemy is taken from the
     pool and given the custom properties 2665 to 2675 and 2406 (its spawn point, added). It is placed at the
     spawn point's anchor (139257 / 139258), given velocity 0 and the command list 2146;
  3. **when no enemy is left and Carmelita's health (940) is above 0, it sets alarm 139255 on the current
     task's script** (`(op117 (op117 (op105) 1308) 735) 139255`). That ends the battle, and the task's script
     completes it.
- **Enemy states** (custom property **2665**, the previous state in 2666, a timer in 2667):
  - −1 spawning;
  - **0** entering along the "in" path (2669), then 1 or 2;
  - **1** holding;
  - **2** attacking, after a random 3 to 5 s, using attack 139259 or 139260 by the spawn row's kind (2668), at
    random (35 %) for kind 3;
  - **3** defeated: given command list 2147 and leaving along the "out" path.

  2664 is the enemy's **current hit-reaction effect** (a playback, stopped and cleared when it ends, method 249).
- **Enemy damage, 2659:** only while the enemy is not in state 3 and its target is alive.
  - Hits from the boss's projectiles do no damage to enemies.
  - A class-89 projectile **from Carmelita** subtracts the hit's amount (124) from the enemy's 940. At 0 or
    below the enemy goes to state 3.
  - Otherwise it plays reaction 139261 and sets its look by remaining health (state machine 139264: above 66 %
    13788, above 33 % 139262, else 139263).
  - Voice lines (1690 to 1709, 1665 to 1671) are chosen at random, by whether battles 2151 or 2159 are
    available.
- **How a hit reaches 2659** [D]. The enemies come from pools whose placements bind override modules 0 and 2
  of `v_ext_turf_war`. Each handles **script event 12** (a hit) as
  `(begin ((op141 (quote s12)) self hit) (if (op12 self (op117 hit 556)) ((op117 (op118 2139 obj0) 2659) self hit)))`:
  it runs the enemy's own hit handler first, then, when the hit's target (556) is this enemy, it calls **job
  2139's method 2659** with (enemy, hit). Command list 2146 is a single 743 (play clip 139264, flags 1), the
  enemy's idle.
- **Carmelita's shots** [D] [X]. Her playable projectile templates are the groups `carmelita_missile_playable`
  (8464, **class 91**) and `carmelita_missile_playable_charged` (8446, **class 92**); `carmelita_missile_fast` /
  `_slow` (8438 / 8400, class 91) are the AI Carmelita's.
  - Her mode-2 spawn points 8398, 8436, 8444 and 6676 place these. 6676, named by her option 1545, is the
    playable shot [I: which point places which template was not read].
  - **Classes 89 to 92 form one family**: the class table (`0x003409E0`, records of 20 bytes, tested by
    `0x0013BEC8`) gives class 89 the range 89 to 92. So her class-91 and class-92 shots pass 2659's "derives
    from 89" test. The class-89 groups `rock_throw_ammo`, `knife_ammo` and `ground_flower_throw_ammo` are other
    throwables, not her pistol.
  - The fire button, rate, speed, damage per shot (the hit's 124) and any auto-aim were **not traced**.
- **Carmelita herself** [I]: her copy is a class-60 character with option 937 = 600 (its maximum health,
  guards.md). Her controls, her movement and what happens when her health reaches 0 were **not traced**.
- **Damage and the script IDs used** [X, from the class option map]:
  - On a projectile (classes 89 to 92), **552** is stream option 442, the object at **+0x1E8**: the **shooter**.
    It is set by the launcher through the setter `0x0014D1F0`, so a port must store the firing character there.
    **44** is stream option 29 (`0x00100C00` → `0x001C8F20`, the reference's FLoInParentChain), "is in the
    parent chain of".
  - So 2659 tests "the shooter belongs to the boss" (the hit's 124 is set to 0) and "the shooter belongs to
    Carmelita" (the voice lines). **The damage itself is the hit's 124, which the projectile's hit supplies, and it
    is subtracted in either case.** A shot whose 552 is empty still deals its 124. It only misses the
    voice-line branch.
  - On characters and guards, **939** is stream option 937 (the maximum health, at +0x170) and **940** is stream
    option 938 (the current health, at +0x16C, clamped to 0 … maximum by its setter `0x001F43D0`).
    *Interpretation:* the maximum is option 937 (100 on the enemies, 600 on Carmelita), and the current value is
    refilled to it when the actor resets, as on taking a pool instance [I: the refill was not found. An enemy that
    starts at 940 = 0 is a port error, since 2659 then puts it straight into state 3].
  - On a guard, 461 / 462 / 463 are stream options 334 / 335 / 336: **give a command list** (0x198540,
    TellGuardCommands), **give the second kind / stop** (0x1985A8), and **is it running its commands**
    (0x1985F0, FGuardRunsCommands).
  - On a placement, **1207** takes an instance (`0x001FBF08`), **1209** locks its row (`0x001FC060`), **1210**
    unlocks the row (`0x001FC090`, LockProxyRowOf), and **1211** gives the instance back (`0x001FC0E0`).
  - **33** (stream option 19, value type `0x2008`, a getter `0x001FC118`) returns the object's **placement row
    object**, the copy's anchor. The wave code uses `(op117 enemy 33)` to reach the per-spawn custom properties
    2664 to 2675.
- **Between battles** [D]. Script 2142 keeps a runner that sends hook 139266 (the wave tick) forever. Between
  battles the tick spawns nothing, because its spawn count has reached the finished row's length (2653 resets it
  for the next battle). Its "no enemies left" step then sets alarm 139255 on the **current task's** script, which
  between battles is 2142's own task, and 2142 does not wait on 139255. So the stray alarm does nothing. When the
  next battle starts, its first tick spawns before it tests for an empty field, so the battle does not end at once.
- **What ends 2142:** command lists given to the guards set its alarms. List **2144**, given to Carmelita's copy
  139252, is:
  1. go to 140657 at 40;
  2. wait 0.1 s and play clip 8919;
  3. **set alarm 139312 on 2142**;
  4. walk 140658 → 140659, wait 0.3 s;
  5. **set alarm 139313 on 2142**.

  Lists 2148 to 2150, given to the boss 139253, presumably set 139302 and 139308 the same way [I: not decoded]. The script's runners wait on
  these and then finish.
- **`0x1F (1, 3)`** holds the game for the player (hold on, kind mask 3); `0x1F (0, 3)` releases it. It is
  paired in 2333 [I for the meaning of the mask bits].
- **Still not traced:** Carmelita's controls, movement, clips and death.

**m1_ball (job 2168, v_ext): Murray launched as a ball at tanks** [D: job 2168's module, v_macro module 36;
I where marked]:

- **World and character:** all its tasks list world 3 (v_ext) and character 26 (Murray). 2171 `t1_ball_intro`
  (waypoint 140670, checkpoint 139406) gives player 0 Murray. Its successors are 2173 and the six tank tasks
  2178 to 2183.
- **What makes 2173 `first_launch` current: Murray's first launch.** Group **`murray_ball_launcher`** has a clip
  (163095, class 130) whose module (module 1) handles **event 2** (the clip playing):
  - it **launches Murray straight up: his velocity becomes (0, 0, 5,000)**;
  - if the launcher's custom property **2760** names the current job, it calls the job's method **2677**.

  2677 sets the job's launch state to 1 and requests **2173 → 4 (make current)** if 2173 is available. So the
  first launch from a launcher pad makes 2173 current. 2173's 1417 sets the state machine 139436 on 139370 to
  15890 while it is available.
- **2175** (2173's successor) is made current by its own module (v_macro module 39). On task event 94, if the
  job's **2679** is true, it requests 2175 → 4. 2679 is "launched recently": less than 0.25 s since the last
  launch, or Murray's object 565 in the world. *Interpretation:* code 94 is a ball impact [I].
- **Tanks:** the job's table pairs tanks with tasks: (139371 2178), (139372 2179), (139373 2180),
  (139374 2181), (139375 2182) and (139376 2183). Method **2678 (tank)** destroys one:
  1. it sets the tank's state machine 113916 to 139402 (destroyed);
  2. it removes the tank's effect 6097;
  3. it requests the tank's task → **2**;
  4. it sets the launch state to 1 and runs a refresh.

  The tank tasks also carry 1405 (done on becoming current). What calls 2678 on a tank, presumably the ball
  hitting it, was not found [I].
- **Failure:** the job's task-event handler calls its 1260 with (1028, 2170), the restart / failure path, in
  three cases:
  - code 29 from object 6310;
  - code 24;
  - codes 15 / 16 that hit Murray outside the launch window or with hit kinds 65,536 / 131,072.
- **On becoming current** (event 22), the job:
  - sets object 5756's 310 to 2.0;
  - schedules its update every 0.017 s (built-in 111);
  - removes the listed objects 139377 to 139394, calls 2186 on 62490 … 139401, and starts script 2169.

  Event 23 undoes this.
- **The ball's controls and physics** (Murray's ball form) were not traced. The update (s1823) only shakes the
  camera every 5 s while launched (Murray's 819) and drops the launch state when the window ends.
- **Turf-war frame order** [I]: a battle task becomes current inside a request. Its 1408 script starts at once
  (section 4), and the script's first hook (139345 → 2653) runs on that script's first step, in the same update,
  before the script reaches its wave-tick loop. A wave tick therefore cannot run between the task becoming current
  and 2653.
- **2142's main runner** finishes after alarm 139313 (Carmelita's list 2144, above). The `0x1F (1, 3)` hold is
  released by a matching `0x1F (0, 3)` or by the script ending (a script's holds are released when it stops)
  [I: the release-on-stop rule was not traced in Sly 3].

**m1_run (job 2187, v_ext, Sly): release two blimp anchors** [D; I where marked]:

- **World and character:** the tasks list world 3 (v_ext) and character 24 (Sly).
- **"Destroy target" means using a blimp anchor.** Placements 139451 (at 8099, 11005, 126) and 139452 (at
  −8180, 7743, 354) in `v_ext_run` place copies of `v_blimp_anchor` (object 16255, class 35, a use point;
  interactables.md 5). Each copy's bind sets **option 1170** (use-point task request): **(2194, −1)** on 139451
  and **(2204, −1)** on 139452. So **using the anchor makes the task current**. Waypoints 140674 (2194) and
  140680 (2204) mark them, through the tasks' 1416.
- **2194 `destroy_target_1`**, script 2197:
  - hint 1643 is cleared and the player is held;
  - camera, then a slow-motion ramp (`0x1D`);
  - guards 139440 and 139441 get lists 2195 / 2196;
  - Sly animates (`0x30`, `0x36`);
  - **the anchor's state machine 5099 on 139451 and on 139393 goes to 1765** (released);
  - camera jolt, line 1626, cameras;
  - **2194 → 2**.
- **2198 `get_tnt_2_intro`** (1399, checkpoint 140677): script 2199 plays line 1627 and closes it.
- **2200 `get_tnt_2`**: its 1416 brings in waypoint 140678 and **trigger 140679**, which names 2200. Its
  option-489 request completes it on entry [I: the trigger's request was not decoded].
- **2201 `destroy_target_2_intro`** (1399): script 2202.
- **2204 `destroy_target_2`**: using anchor 139452 makes it current. Script 2205 stops 2203, holds the player,
  plays line 1633 and the cameras, sends **anchor 139452's 5099 → 1765**, and drives 139394's state machine
  139483 to 139484. Then Sly (24) is retired from the scene and walks to 139487. The script continues with
  cameras and line 1634 [the closing request was not read: I].
- **2206 `gauntlet_intro`** (1399, checkpoint 139487) follows, with script 2207.

**Clearing the job and task keys** [X]:

- The keys are written only by the current-job setter (`0x0018AA48`) and the current-task setter (`0x0018AAB8`):
  game +0x1498 and +0x149C hold the OID, or −1 for none (saving.md 3).
- **A task that stops being current clears the task key.** `0x00188928` (called from `0x00188620`, "stop being
  current if current") calls the setter with none.
- In live play (mode 0) the state setter (`0x0018A3E8`) calls `0x00188620` whenever a task's new state is **0
  or 3**. So **closing the current task**, as when the job's last task 2327 closes, clears +0x149C to −1. In mode
  1 (restoring) this follow-up is skipped.
- **A job that ends clears the job key.** The job end (`0x00186298` → `0x00186610`) calls the job setter with
  none.
- **Make-current does not test the state.** `0x00188540` returns early only when the task is already current.
  The world start's step 2 relies on the keys having been cleared, and does not refuse a closed task.
- So a build that still holds 2327 in the key after the exit is missing the "state 0 or 3 → stop being current"
  follow-up (or the job-end clear). The exit itself does not touch the keys.

**m1_canal_chase** (tasks 2331 to 2350) [D] [I where marked]:

| Task | World (1401) | Character (1402) | Script (1408) | What happens / what completes it |
|---|---|---|---|---|
| 2331 `t1_canal_chase_intro` (waypoint 140821, checkpoint 139815) | 3 v_ext | 25 Bentley | 2332 | opens the binocucom on Bentley at 24 (`0x46`), plays lines 1080 to 1086 and the boat set-up, then requests 2331 → 2 and **fires exit 2047 to v_canal** (point 424) |
| 1804 `minigame_intro` (1399) | 4 v_canal | 25 | 2333 | requests 1804 → 2 at once, starts 2336 (the minigame) and 2337 (a hook loop every 0.2 s), and plays the intro cameras (line 1087 the first time, test 1888) |
| 2334 `overheat` | — | — | 2335 | line 1088 the first time (1888), then 2334 → 2. *Interpretation:* a one-time tutorial line, made current when the gun first overheats [I] |
| 2341, 2343, 2345 `enemy_1` to `enemy_3` | — | — | 2342, 2344, 2346 | each starts 2340 (line 1089 or 1090, chosen by hook conditions 139859 / 139860) and then requests its task → 2. *Interpretation:* each is made current when that enemy boat is sunk [I] |
| 2347 `minigame_complete` (1399; prerequisites 2341, 2343, 2345) | — | — | 2348 | a slow-motion ramp (`0x1D`), cameras, line 1091, activates checkpoint 2349 (`0x12`), restores the time scale, requests 2347 → 2 and **fires exit 2045 to point 51592** |
| 2350 `complete` (1399) | 3 v_ext | 25 | 2351 | the closing scene in v_ext (Bentley removed and re-added, lines 1075 and 1076); successor 1826 |

- **The minigame (script 2336)** [D]:
  - it gives **player 0 control of object 2003** (`0x13` (2003, 0)). 2003 is a class-49 controller in group
    `v_boat_player`;
  - it starts the boat-art scripts (50657, 139819) and an animation (139836, flags 8);
  - it shows hint 1092 until alarms 139838 and 139839 are both set.
- **The boat** is object 139806 (class 47, `v_boat_player` module 0). Its state is kept in the controller 2003:
  **939 = maximum hull and 940 = current hull**. On each start (event 17) the hull is refilled to the maximum.
  - **Taking damage** (event 12, a hit on the boat, when the hull is above 0 and the player is not held):
    - the camera shakes (built-in 138, (10, 0.5));
    - the hull loses the hit's amount (the hit record's 124), or all of it if the hit is fatal (125);
    - dropping below 25 % starts the warning object 150018;
    - at 0, effect 37190 plays, the boat removes itself, and Sly (24) and Bentley (25) get method 130 with (256, 0,
      #t, boat). *Interpretation:* the fail and retry [I];
    - otherwise the hit reaction 150009 starts.
  - **Firing:** the boat takes an instance of projectile placement **150052** (`v_boat_player_missile`, class
    89) with script ID 825. It calls 2749 / 2748 on the art script 139819 for the heat counter [I: the overheat
    gauge]. The rest of the firing code was not decoded here.
- **Enemy boats** (`v_boat_enemy`, class-111 body 150654, module 0) [D]:
  - health 100 (s2415 / s2416, settable through custom method 1941);
  - on event 19 (hit by a projectile), when the projectile is class 89 its damage (549) is subtracted, and the
    health bar (screen object, 1153) is updated;
  - at 0 the enemy's crew object is re-parented and hidden, and 150681 (the sinking / defeat object) is placed
    and started (41, 544).
- **The boat's frame update** (`v_boat_player` module 0, event 1, decoded) [D]. The pad is the controller's
  object 943 (s6506). Built-in calls on it: 1079 (button held), 1078 (button pressed within 0.1 s), 1072 (stick
  value), 1074 (stick magnitude). The meaning of the masks is from the pad layout [I].
  - **Forward motion is not the player's.** The boat sits under an owner (+0x10, s1665) that carries it along
    the canal. The script only sets the boat's own velocity (script ID 194) to (0, lateral, vertical).
  - **Vertical:** vertical = clamp(owner z − boat z, −1,500, 1,500), so the boat follows the owner's height.
  - **Steering (lateral):**
    - when |stick| > 0.2, lateral −= stick × 1,000 / k, where k = 4 normally and 12 in the air (state machine
      150013 not in 152160), then the result is clamped to ±1,000. The lean state is 1 or 2 by the stick's
      sign;
    - otherwise lateral × 0.89 each frame;
    - lateral resets to 0 when it changes sign.
  - The two wake effects 150043 / 150038 follow the lateral speed (none below 50).
  - **Jump:** button mask `0x40` pressed sets state machine 150013 to 15663 (lean state 3 in the air) and sets
    alarm 139839 on the hint script. Triggers 139802, 139805 and 150109 (modules 1 to 3 of
    `v_canal_canal_chase`) force the same jump on any newly entering member (ramps).
  - The animation rate (state machine 150011) follows 1 + stick + (distance to the owner's goal / 400), clamped;
    the art script 50657 gets the lean state (2750).
  - **Heat gauge:** screen object 150062, value 1 when overheated, else a lerp by heat / 100 (part 892).
- **Firing and heat** (same module) [D]:
  - while button mask `0x80` or `0x08` is held, the time is past the next-shot time, and the gun is not
    overheated, the boat **fires**:
    - it takes a missile instance of placement 150052 with ID 825;
    - it lifts it by 2 when not in the jump state, sets its damage (549) to 1, and re-parents it to the world;
    - it gives it velocity (16,000, 0, 0) in its own frame;
    - the next shot is allowed 0.1 s later, so 10 shots a second;
  - **heat** rises by 3 per shot (at most 100). At **100** the gun is **overheated** and stops firing;
  - while the button is not held, heat falls by 0.75 per frame. **Overheat ends below 80**;
  - at heat ≥ 60, script 139807 runs (a small camera shake, built-in 138 (0, 0.01)); otherwise it is stopped;
  - the first shot sets alarm 139838 on the hint script (the tutorial). The art script's 2749 / 2748 mark a
    shot and a reload [I].
- **What makes the side tasks current** [D]:
  - **Overheat 2334:** job 2329's module (v_macro module 76) answers task event 29. When the object is **script
    139807**, which starts when heat reaches 60 (and a script start raises event 5, scripts.md 7.4), and 2334 is
    available, it requests **2334 → 4 (make current)**. 2334's script then plays line 1088 the first time and
    closes it.
  - When the object is the **boat 139806** (its event 5 is raised when the hull reaches 0), the job calls its
    method 1260 with (−1, 2338): the failure path [I].
  - **Enemies 2341 / 2343 / 2345:** modules 7, 6 and 5 of `v_canal_canal_chase` are override modules, attached
    to the enemy copies through their placements. On event 5 they call the inherited handler (built-in 141) and
    then **request their enemy task → 4**. Each enemy task's script (2340, then a request for 2) closes it at
    once. *Interpretation:* an enemy boat gets event 5 when it is defeated [I: the raise was not found in the
    enemy module itself]. Lines 1089 and 1090 are chosen by how many of the three enemy tasks are still at
    state 1 (2340's module: 0 left or 1 left).
- **Enemy boats:** health 100 and damage per player missile 1, so 100 hits; the defeat object 150681 is placed
  at 150682 (v_boat_enemy module, above).
- **The owner that carries the boat** [D]:
  - The boat's owner is **139840** (class 47, `v_canal_rig`). Script 2336 applies the clip **139836** with flags
    8 (`0x16`).
  - That clip is **104.63 s** long and moves two objects along keyed position tracks: 139840 and 149995. Each
    closes on itself: the track starts and ends at (65, −11,815, 0), a loop round the canal.
  - Sampled over 1,046 steps the loop is **314,065 units** long, run at a **steady ≈ 3,000 units/s**
    (10th to 90th percentile 2,980 to 3,022). There is no deliberate speed-up or slow-down.
  - The track keys only position; heading follows the motion [I]. 149995 runs alongside, about 1,560 units
    away. *Interpretation:* the chase camera's or the pacer's carrier [I].
- **Enemy boats** (`v_boat_enemy`, class-111 body 150654) [D, partial]:
  - the enemy keeps a crew member taken from a pool (option 2737's object, script ID 1207 to take, 1209 to place)
    through custom method 1954. It poses it with command list **150691**, a single 743 (play clip 56517,
    flags 8), and sets its options 69, 951, 435, 449, 421 and 482. Method 2735 returns the crew member;
  - **defeat:** at health 0 the crew member is re-parented and hidden, and object **150681** (class 89, a
    projectile body) is moved to anchor 150682, added and started (544). *Interpretation:* the sinking
    explosion [I]. Below half health, and on ordinary hits, the hit flag 1110 = 1 is set.
- **Placements** [D] (correction). The enemy boats are copies of `v_boat_enemy` placed by **139842, 139845 and
  139848** in `v_canal_canal_chase`. Their binds attach override modules 5, 6 and 7 and set 2736 (2330, the
  health-bar screen object), 2738 (139853, 139852, 139851, the bar's part) and 2737 (150689, 150687, 150685).
  Those last three are placements inside `v_boat_enemy` of **`ecto_swarmer` guards (class 58), the crews**,
  each bound with an option-8 property. The copies are removed at the start (option 27 on the placement); which
  script adds them was not found.
- **Still open:** the camera during the chase, the enemies' own movement, how the crew shoots at the player and how much of the hull (939 / 940)
  a hit takes, and what raises event 5 on a defeated enemy, which the override modules 5 to 7 need.

**Back-filling prerequisites: on every request, not on the restore** [X]:

- Every state change made by a request goes through `0x00189B08`. That routine first collects the affected tasks
  (`0x001899F0`), then sets each to its pending state (+0xF8) through `0x0018A670`. The callers are:
  - ordinary requests (`0x00189300`);
  - forced requests (`0x00189500`, used by the task methods 1314 to 1317 and by state −1 / 4 forcing);
  - the job routines `0x00187118` and `0x00187270`;
  - `0x0018A0D0`.
- The restore (`0x00188CB0`) and step 2 of the world start call the state setter directly, without back-fill.
- **The collector**, for a new state n. Each prerequisite p that has no pending state yet and whose state is below
  the target is raised to the target, and the same is done for p's own prerequisites:

| New state n | Target for each prerequisite p |
|---|---|
| 1 or 2 | p has no job: 3 (closed). p has the **same** job as the task: 2 (done). p has another job: 3, unless that job's 1361 (keep open) is set, then 2 |
| 3 | 3 |
| 0, 4, 5 | none |

- **Ordinary (checked) requests** (`0x00189300`) allow only these moves; anything else is refused and announced
  (`0x00188868`):
  - 0 → 1, only when the prerequisites hold (`0x00189008`);
  - 1 → 2 and 1 → 3;
  - 2 → 3.

  A closed task (3) never changes. A request for 3 on a task that has a job is turned into a request for 2. State
  −1 means 4 (make current), and 5 means stop being current.
- **Command `0x1E` is a checked request, not a forced one.** Its handler (`0x0022776C`) passes its one (task,
  state) pair to `0x00185908`, which issues `0x00189078`, an ordinary request. Use-point pairs (option 1170) take the
  same path [X]. Exit and arrival pairs (options 841 and 847) are expected to as well [I: through the same pair
  routine, not traced].
- **Script 2061's (2418, 3) at a new game** [D]. Task 2418 `t1_title_after_chalktalk3` has no job and prerequisite
  1827 `t1_chalktalk3`, which in turn has prerequisites 2405, 2166, 2184 and 2221. At a new game 2418 is locked
  (0), so 0 → 3 is **refused**, and nothing is closed or back-filled. Later, when 2418 is available, closing it
  closes 1827 and the tasks before it. That is the game's rule: turning off back-fill is not needed once 0x1E is
  treated as a checked request.

**So yes:** after a load, or on arriving at a checkpoint (which is a world start), an available task with 1399 set
becomes current even though it did not just change to available. For `t1_recon_cell_dialog` (2091) in
v_interpol: once 2089 is done, 2091 is restored as available, and step 3 makes it current.

During play, outside the world start, the checkpoint tasks are applied as ordinary requests
(`0x00185908`, per pair (task, state)). A task reaching state 1 that way goes through the normal "becomes
available" rule.

An engine whose rules make a task current only on the change to available must add step 3 to its world-start or
restore pass. The four rules: scan the available tasks; take the last one with 1399 set that is not current; make
it current with a forced request; repeat while requests are pending.

### 4.2 Mission waypoints (class 120) [X]

The object that task option 1407 names (for example object 121540) is a **mission waypoint**, class 120 (Sly 2's
job marker). Walking into it is what matters: the waypoint reads no button and shows no prompt of its own.

**Options** (field offsets in the object) [X]; counts over the disc [D]:

| Option | Field | Default (set at load, `0x00281AD0`) | Meaning |
|---:|---|---|---|
| 1486 | +0x130 | 0 | **kind**: 0 a job's start waypoint (counts only while no job runs), 1 a waypoint used during a job, 2 a waypoint that also counts only while no job runs (Sly 2's vault marker kind [R]), −1 a fall-back for command `0x69` (below). Disc: 1 ×50, 0 ×16, −1 ×9 |
| 1483 | +0x134 | 49 | the class the player's controlled object must derive from (51 on 15 waypoints) |
| 1484 | +0x138, +0x13C | 300, 500 | the **enter radius** and the **leave radius** (on the disc: (150, 400) twice) |
| 1488 | +0x148 | | the anchor object (class 3) |
| 1489, 1490, 1491 | +0x14C to +0x154 | | the three drawn parts: icon, line, base (class 47) |
| 1487 | +0x198 | | a flag (2 waypoints) |
| 1492, 1493 | +0x19C, +0x1A0 | 255, 255 | two masks; +0x1A0 is the mask that command `0x69` tests (below) |
| 1494 | | | a method, (1, 4500) twice |
| 1495 | +0x1AC | 5000 | a distance (1000 once) |

**Each frame** (the update at `0x00281C50`) [X]:

1. Take player 0's controlled object (`0x001F4AD0`). Unless it derives from the class in +0x134, the player is not
   in.
2. The player is in when its distance to the waypoint is below the enter radius, or below the leave radius once it
   is already in (the flag at +0x144). This gives hysteresis.
3. Kinds 0 and 2 never count as in while a job is running (game state +0x4044).
4. A class-51 player is checked further: one field (+0x33C) must be −1 and one of its methods must not answer 2.
   *Interpretation:* the player must not be busy, for example carrying or in a move [I].
5. When the in/out state changes, the waypoint stores it and raises **script event 5** (entering) or **6**
   (leaving) on itself.

**What the event does** [X]: the shared event handler (vtable slot 3, `0x001C86B8`) first passes the event to the
object's own script, then **broadcasts a task event**: code **29** on entering and **30** on leaving, with the
waypoint as the object (`0x001854C8` on the game state at `0x0036C700`). These are Sly 2's arrival events 0x1A and
0x1B.

**Who receives the broadcast** [X: `0x001854C8`], in this order:

1. `0x001855F0` (code 12 only; the defeat path).
2. The level values (`0x00171AA0`, which reacts only to codes 12, 16, 32 and 41).
3. Every task in the available-task list (`0x00479498`, linked through +0x78; 4.1a) whose job (+0x6C, option 1391) **equals the
   current job** (`0x004794A4`; both may be none). The task must also pass `0x001880F0`: its +0x40 is 0 and its
   state is not 3. Such a task gets **script event 20** with the values (code, object) (`0x00189DF8`). This is
   Sly 2's event 17.
4. Every object in the **task-event listener list** (`0x00479B98`) gets the same script event 20 with (code, object)
   (`0x001772D8`). Only **class 147**, the macro-package objects of the `*_macro` groups, joins this list: its
   enter-world method (`0x00177220`, vtable `0x357200` slot 11) appends it (`0x0011AF60`), and its leave-world
   method (`0x00177258`, slot 12) removes it (`0x0011B028`).
5. The current job, if there is one (`0x00186730`).
6. The sequence at `0x004BC230` (`0x002849A8`).

So, with no job current, a task that belongs to a job (such as `t1_recon_intro`) does **not** hear code 29. The job
is started by a **macro-package module** that listens to every task event [X for the delivery; the module's body is
data to decompile].

**Test vector: `t1_recon_intro` in v_ext** [D] (world 3, `03_v_ext`):

- Waypoint object 35785 (`mission_start_jt`, class 120) is placed by placement 121540 in group `v_ext_recon`.
- Task **2086** (class 169, group `v_macro`, world filter 3, no module of its own) holds 121540 (option 1407).
- The listeners in v_ext are the two class-147 objects of `v_macro`: **2058** (module index **4**, no filter) and
  **2228** (module index **57**, world filter 3).
- Their handlers for event 20 must test code 29 and the waypoint, then make task 2086 current (Sly 2's macro
  modules do it with a task-state request). Making the job's first task current begins the job (Sly 2's rule
  [R]).
- To answer exactly what they call, decompile v_macro modules 4 and 57 of v_ext. With a script interpreter, deliver
  event 20 (29, the waypoint) to objects 2058 and 2228.

The waypoint's own script is a second possible handler. It receives events 5 and 6 before the broadcast. Object
35785 names **module 0 of its group `mission_start_jt`**, and placement 121540 names none [D: the records' module
indices]. So decompile that module too.

**Custom properties of waypoints: option 8, 1990 to 1992, and 1485** [X] [D]:

- **Option 8** on class 120 is a load-time method with two arguments: (property ID, value). Its handler
  (`0x00100610`) calls `0x0013B750(object, ID, value)`, which stores a **custom property** on the object. IDs from
  1,816 up live in the object's property store ([scripts.md](scripts.md) 5), where scripts read them back. A
  loader that skips option 8 leaves these properties missing.
- Values across the disc on class-120 bodies: (1843, 14259) ×43, (1990, −1) ×16, (1992, −1) ×15, (1991, 24) ×7.
- **Test vector, v_ext** (decompressed `03_v_ext`):
  - Waypoint body 35785 (`mission_start_jt`) carries three option-8 entries, near `0x139156` to `0x139176`:
    1990 = −1, 1992 = −1, 1991 = 24. The entry bytes are `08 00 15 00 c7 07 06 00 18 00 00 00`: option 8, a
    `u16`-tagged ID, then an `s32`-tagged value.
  - Placement 121540 (`v_ext_recon`) carries one entry, `08 00 15 00 c6 07 06 00 26 08 00 00`: **1990 = 2086**,
    the task `t1_recon_intro`. Placement options apply to the placed copy (Sly 2's rule [R]).
  - So the loaded waypoint should have 1990 = 2086, 1991 = 24 and 1992 = −1.
- Meanings, as the enter handler uses them:
  - **1990** is the task this waypoint starts.
  - **1991** is the character allowed to start it. 24 is Sly's owner code (25 Bentley, 26 Murray, as in the item
    table), compared with the player's option 30 [I: that option 30 returns this code].
  - **1992** is an override object whose method 247 runs instead of the normal path. −1 means none, so the normal
    path runs.
- **1485** (script ID 1364, field +0x140, an object of class 169) has **no stream value** on any disc object. No
  write to +0x140 was found in the waypoint code (`0x00281000` to `0x00283000`) or the task code (`0x00184000` to
  `0x0018C000`), and the load-time setup does not set it. It therefore stays empty (reads as none / −1) unless a
  script writes it through script ID 1364 (not searched). A handler that reads it must cope with none, for example
  by using the task in 1990 [I].

**Game-object checks used by waypoint handlers** [X]. The game object is class 1 in the class option map. Script
option IDs 984 and 1000 are script methods with one integer argument, the player index n:

| Script ID | Stream option | Handler → routine | Returns |
|---:|---:|---|---|
| **984** | 986 | `0x0010AA80` → `0x001F4AD0` | **player n's controlled object** (field +0x110C of player record n; records of 4,544 bytes from `0x0036E740`); value type `0x2031`, an object |
| **1000** | 1001 | `0x0010ADB8` → `0x001A9478` | **whether player n is held**: true when player n's hold count (record +0x1180) is above 0. n = 0 reads player 0, n = 1 reads player 1, any other value adds both |

- The hold count goes up by one for each hold the game takes (`0x001A9630`, the pause and hold used by script
  command `0x1F` and by menus) and down by one when the hold is released (`0x001A96C8`).
- **When the player can start a job:** 1000(0) is **false** (count 0: no script, cutscene or menu holds player 0),
  and 984(0) is the character player 0 controls (in the Venice hub, Sly), not none.
- A handler that requires 1000(0) false refuses while any hold is still open. In an engine that is usually a pause
  or hold taken by a script and never released [I].

## 5. Command-script commands used by missions [R] [X where noted]

Argument letters as in [classes/animation.md](classes/animation.md) 3.

| Command | Arguments | What it does |
|---|---|---|
| `0x1E` | `wb`: task, state | sets a task's state as a checkpoint does (the checkpoint-task path, Sly 2 4) [R] |
| `0x13` | `wb`: object, player index | **gives player n's control to the object** [X: `0x001F4B00`]: player slot n (4,544-byte records from `0x0036E740`) has its controlled object at +0x110C. The old object is told it lost control, the new one that it gained it, and if the new one is a playable character (class-51 family), it becomes that player's current character |
| `0x68` | `ww`: exit, point | fires an exit at an arrival point ([level-flow.md](level-flow.md) 4.4) |
| `0x11`, `0x17` | `w`: script | stop, start another command script |
| `0x0E`, `0x0F`, `0x10` | | wait for, set, clear an alarm (script-to-script signals) |
| `0x12` | `w`: object | **activates one object**; what that means depends on the object's class ([scripts.md](scripts.md) 7.3) [X] |
| `0x6F`, `0x70` | `wb` | take, release an instance of a placement |
| `0x38`, `0x39` | `wf`: object, fade time | add an object to the world, remove it |
| `0x19` | `wwwb`: object, state machine, goal, snap | drive a state machine to a goal |

The full command list and the runner's rules are in [scripts.md](scripts.md) 7.

## 6. Saving [D]

Each job and task is saved under its own name in the save tree, with the keys of sections 2 and 3
([saving.md](saving.md) 2). Restoring follows Sly 2 [R]; the episode-8 forced checkpoint is in
[saving.md](saving.md) 3.

## 7. Open questions

1. The job options still open (1363, 1369, 1386, 7), the readings of 1372, 1403 and 1404, and the arguments of the job methods 1355 to 1359 (3.2).
2. The fifth argument of 1416 and the seventh of 1417 (always 0 on the commonest values).
3. Whether Sly 3 has a resume-after-loading rule at all.
4. Every [R] rule needs checking in play.
