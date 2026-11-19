---
id: sly1-gameplay-pickups-and-progress
title: Sly 1 pickups, counters, level goals and saving
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds; executable SCUS_971.98 sha256
      d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      per-world counts of every pickup class (proxy clones included), option values on pickups and vaults, the task
      mask of the world table, the adaptive-difficulty tables and the save-file names read from the executable, the
      game's own strings quoted below
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files coin, pzo, game, credit, difficulty, save, sw,
      spaprops (option bindings)
    used_for: behaviour of pickups, vaults, clue bottles, the timed goal, the adaptive difficulty level and the game-state record; marked [R]
  - kind: reference-project
    what: sly-rs @ 5777345d (2026-11-05), docs/research/memory-card.md
    used_for: the files the game writes on a blank memory card and their sizes, as logged from the running game; marked [R]
date: 2026-11-18
---

# Pickups, counters, level goals and saving

Tags as in [gameplay-overview.md](gameplay-overview.md): **[D]** disc data, **[O]** observable, **[R]** reference
behaviour to be confirmed, **[I]** inferred. Class IDs and option numbers are those of
[object-stream.md](object-stream.md) and [option-encodings.md](option-encodings.md). Distances are game units,
times seconds.

## 1. The pickups

| Class | Pickup | On the disc (all worlds, proxy clones counted) |
|---:|---|---:|
| 88 | coin | 950 placed coins in 30 worlds [D]; many more are dropped at run time (section 3) |
| 86 | charm | 43 [D] |
| 87 | life token | 55 [D] (one with OID 1356 in every world, section 3.3) |
| 89 | key | 28 [D], one per level whose task mask has bit 1 (section 6) |
| 46 | clue bottle | 595 [D] in 19 levels (proxy clones counted, nested ones included) |
| 23 | vault | 20 [D]: one per level with a vault goal, plus the Paris prologue |
| 90 | sprint hourglass (starts the timed goal) | 22 [D] |
| 91 | race prize | 14 [D] ("At the Dog Track", "A Desperate Race", 7 each) |

Per-world counts are in section 8.

## 2. Behaviour shared by coins, charms, life tokens, keys and hourglasses

These five classes share one pickup behaviour [R]:

- **States.** Resting (placed in the level, spinning in its idle animation); falling (thrown out by a broken object,
  under gravity `(0, 0, −2250)` units/s², bouncing); lost (fading out); attracted (pulled to the player); swirling
  and sticking (charms only); removed. The pickup's animations come from a state machine that each pickup class
  carries; its state OIDs are 609 (resting), 608 (falling), 611 (lost), 610 (attracted) [R].
- **Collection.** Each frame, if the player can collect and is within 300 units: the pickup is collected when it
  is within the player's automatic-collect radius, or when its collision sphere (radius below) touches the player's
  collision mesh [R]. A cane hit on the pickup also collects it [R].
- **Bounce.** While falling, a pickup bounces off solid objects (not off characters, breakable pieces, bombs,
  flies or clue bottles): the velocity component along the surface normal is reversed and scaled by the "normal"
  factor, the tangential part scaled by the "tangential" factor. On a floor (normal z > 0.7) a last bounce of a
  fixed upward speed settles it; once the vertical speed is below 25 after that bounce it rests [R]. A falling
  pickup becomes collectable 1 s after it started falling and is removed if it has fallen for 5 s [R].
- **Glint.** When the pickup's facing crosses the camera's left/right side, a sparkle plays with the class's glint
  chance, at most once per second [R].
- **Remembered.** A placed pickup that was collected stays collected after a checkpoint restart or a reload of the
  level (it is not placed again), except in world `0x308` ("A Deadly Dance"), where charms come back [R].

Per-class constants [R]:

| Class | Collect radius | Bounce radius | Normal factor | Tangential factor | Last-bounce speed (max) | Glint chance | Lifetime when dropped |
|---|---:|---:|---:|---:|---:|---:|---|
| coin | 50 | 45 | 0.75 | 0.6 | 50 (250) | 0.25 | 8 to 10 s, then lost |
| charm | 60 | 60 | 0.5 | 0.75 | 0 (300) | 0.35 | stays |
| life token | 50 | 45 | 0.5 | 0.05 | 50 (250) | 0 | 4 to 6 s, then removed |
| key | 35 | 35 | 0.6 | 0.6 | 250 (500) | 0.75 | stays |
| hourglass | 60 | 60 | 0.6 | 0.6 | 250 (500) | 0.75 | until the sprint deadline |

"Touches the player's collision mesh" means: a sphere of the collect radius around the pickup's position
intersects the player's current collision shape (standing: OID 100, a convex shape reaching 55 below and 90 above
his origin, about 57 to each side; player-moves.md 1.1) [D for the shape] [R]. A stand-in for the player (a probe
with no mesh) should use that shape, or a sphere of radius 60 centred 15 above its origin, which covers it [I].

Lifetimes count down only while the radio-dialogue view is not open [R]. Option 985 on coins sets the lifetime
range; on the disc it is (0, 0.3) on 30 coins [D] (meaning not settled: either a scale or a separate range [I]).
Option 982 ("never reuse", section 3.3) is set on 18 coins and all 22 hourglasses [D].

## 3. Counters

### 3.1 Coins

- Collecting a coin adds 1 to the coin counter and shows the coin counter on screen [O] [R].
- When the counter passes 99: if the player has fewer than **2** charms, the counter resets to 0 and a charm flies
  in from the top left of the screen and is added; otherwise, if lives are below 99, the counter resets to 0 and a
  life is added; otherwise the counter stays at 99 [R]. (100 coins give a charm, or a life when both charms are
  held. [O])

### 3.2 Charms

- The player holds **0 to 2** charms [R]. A charm absorbs one hit that would otherwise cost a life [O] (damage rules
  belong to player-moves.md).
- Collecting a charm swirls it around the player for 1 s, then sticks for 0.25 s, then counts [R]. If the player
  already holds 2, the charm is thrown away instead (reject velocity `(−250, 0, 600)` in the frame of the player's
  second charm) [R]. When the player reaches 2 charms, or a sprint starts, every remaining placed charm is removed
  from the world (except in world `0x308`) [R].
- Starting a Master Thief Sprint sets the charm count to 0 [R].

### 3.3 Lives

- A new game starts with **5** lives; the maximum is **99** [R].
- A life token adds one life [R]. Losing a life when the count is already 0 makes it −1, which is game over
  ("Game Over. Try again?" [D, executable string]) [R].
- Dropped pickups are not created at run time: each class keeps a pool of its removed objects and of objects marked
  "reusable" (OID 1356, the life token in every world, is such a pool object [D] [I]). A drop takes the pooled
  objects of that class, preferring removed ones, then the active ones farthest from the player [R]. Up to the
  requested number are thrown out in a ring: horizontal speed 75 to 100, vertical 1000 to 1300, each starting 0.5 s
  after the previous [R].

### 3.4 Adaptive difficulty level and breakable rewards

The game measures how much the player is struggling in each level (a value 0 to 1 kept per level, plus a raw value
0 to 1) and uses it to decide what breakables drop and whether to hand out charms [R]. It is an executable table.

**Tables in the executable** (read from the player's own `SCUS_971.98`, after checking its SHA-256 above):

| Virtual address | File offset | Content |
|---|---|---|
| `0x00261D40` | `0x162D40` | Reward ranges: 4 records of 12 bytes (`f32` raw-level threshold, `s32` minimum, `s32` maximum) [D] |
| `0x00261D70`, `0x00261DD0`, `0x00261E30`, `0x00261E90` | `0x162D70` … | Four 96-byte settings records [D] |

Settings record layout (offsets in bytes; the last word is padding) [D for values, R for meaning]:

| Offset | Type | Meaning |
|---:|---|---|
| 0 | 2 × `f32` | allowed range of the level value |
| 8 | 2 × `f32` | range applied when the world loads |
| 16 | `f32` | change on death, applied at once |
| 20 | `f32` | change on reaching a new checkpoint |
| 24 | `f32` | change on reaching a checkpoint again |
| 28 | 4 × `s32` | breaks before a rich drop: minimum and maximum at level 0, minimum and maximum at level 1 |
| 44 | `f32`, `s32`, `f32`, `s32` | level threshold and charm count given on world load, twice (low, high) |
| 60 | 2 × `f32` | raw change on death, on game over |
| 68 | 2 × `f32` | raw change when a life-token reward is given, raw change per break |
| 76 | `s32`, `f32` | low-lives count and extra raw change on death at or below it |
| 84 | `s32`, pointer | number of reward ranges (4) and the address of the reward-range table |

Values on the disc: the four records differ only in the first 28 bytes and the charm counts (record at `0x261D70`:
range (0, 1), load (0, 0.6), death +0.1, new checkpoint −0.8, repeat 0, charms 1 and 2; `0x261DD0`: (0, 0.5),
(0, 0.2), +0.1, −0.4, −0.4, charms 0 and 0; `0x261E30`: (0, 0), (0, 0.2), 0, 0, 0, charms 0 and 0; `0x261E90`:
(0, 1), (0, 0), +0.1, −0.8, 0, charms 1 and 2); all share rich-break counts 4 to 6 (0 to 2 at full difficulty help),
thresholds 0.399 and 0.799, deaths +0.075, game over +0.25, life reward −0.2, per break −0.003, low lives 2 with
+0.075 [D]. Reward ranges: (0.8: 2 to 5), (0.4: 6 to 10), (0.2: 15 to 25), (0.0: 50 to 100) [D].

Selection [R]: the record at `0x261E90` in the prologue, the hub and the final world; `0x261D70` in a level whose
key is not yet collected; `0x261E30` after the key. The record at `0x261DD0` is not used by the reference port
(open question).

Behaviour [R]:

- **Breakable drops.** Option 233 (coin count) on a breakable: 0 drops nothing; a positive value drops exactly
  that many coins; a negative value lets the adaptive difficulty level decide. The plain breakable (class 33) defaults to −1,
  the other breakable classes to 0 [R]. When the adaptive difficulty level decides: a countdown of "breaks until a rich drop" (drawn
  between the record's limits, moved toward the "full difficulty help" limits as the level rises) gives 4 to 6 coins when it
  runs out and 1 to 3 otherwise; a second countdown drawn from the reward-range row matching the raw level gives a
  life token instead when it runs out (and only if the raw level is not 0). Each counted break lowers the raw level
  by 0.003. On the disc option 233 is set on 156 class-33 objects (0 × 122, 1 × 14, 2 × 4, 8 × 1, 10 × 3, 15 × 2, 20 × 1,
  25 × 1), on 8 class-37 objects (1 × 7, 0 × 1), on 2 class-36 objects (0) and in 39 proxy overrides (0) [D]; the
  other 176 class-33 breakables keep the default and drop by adaptive difficulty level.
- **Deaths and checkpoints** move the level and raw level by the record's amounts; reaching the low-lives count adds
  extra. Collecting the key resets both to 0 and forgets the checkpoints reached.
- **Charms on load.** After a restart, if the level is at or above 0.399 the player is given the low charm count,
  at or above 0.799 the high one, if they hold fewer.

## 4. Clue bottles and vaults

### 4.1 Clue bottles

- Each level with a vault has 20, 25, 30 or 40 clue bottles [D] (section 8). Bottles are numbered in load order; the
  level's record keeps one bit per bottle (up to 64) [R].
- A bottle spins about the vertical axis at a rate chosen at load: uniform in 0.8 to 1.2 radians per second, with a
  random sign, from a random start angle in −π..π. It steps through its frames; option 623 sets the frame time (0.5 on
  19 bottles [D]), multiplied at load by a random factor in 0.9 to 1.1, with a random start phase in 0 to twice the
  frame time [R].
- **Touch.** A bottle has no touch radius of its own: it is collected when the player's body touches its collision
  shape (option 133 replaces that shape by a sphere of the given radius) [R]. Driven machines also collect within
  their own radius: 150 (tank), 200 (RC car), 300 (turret) [R]. With reward bit `0x20000` (4.2), a bottle within 300
  units of the player is collected unless it moves away from the player faster than 50 units per second [R]. A bottle inside a breakable becomes free when the breakable breaks [R].
- Touching or hitting a bottle collects it, plays the bottle-count display and a sound; collecting the last one plays
  a different sound [R]. The first bottle ever collected plays a dialogue (OID 840), the first time all bottles of
  a level are collected plays another (OID 842) [R].
- A collected bottle does not come back [R].

### 4.2 Vault

- A vault (class 23) has a three-digit combination dial: three dials with digits 0 to 9 (dial states OID 912 to 921
  for digits 0 to 9) [R]. Option 1046 holds the combination as an integer.
  - **Where the combination comes from** [D]: the vault template's own option list sets 1046 to 1 in all 20 vaults.
    The real combination is a second 1046 in the option list of the **placement (class 83) that places the vault**.
    A later option overrides an earlier one ([options-by-class.md](options-by-class.md) 1), and class 83 carries
    1046 exactly 20 times. Per level:

    | Episode | Level | Combination |
    |---|---|---:|
    | Paris | Paris (prologue vault) | 937 |
    | Tide of Terror | A Stealthy Approach | 792 |
    | | High Class Heist | 436 |
    | | Into the Machine | 227 |
    | | A Cunning Disguise | 242 |
    | | The Fire Down Below | 579 |
    | | The Gunboat Graveyard | 719 |
    | Sunset Snake Eyes | A Rocky Start | 314 |
    | | Boneyard Casino | 338 |
    | | Straight to the Top | 468 |
    | | Two to Tango | 532 |
    | | Back Alley Heist | 565 |
    | Vicious Voodoo | The Dread Swamp Path | 588 |
    | | A Grave Undertaking | 128 |
    | | Descent into Danger | 571 |
    | | The Lair of the Beast | 444 |
    | Fire in the Sky | A Perilous Ascent | 578 |
    | | Flaming Temple of Flame | 932 |
    | | The Unseen Foe | 667 |
    | | Duel by the Dragon | 231 |

    (Found by scanning each world for option 1046 with its 4-byte value: every vault world shows the template's 1
    and exactly one other value. The other values that appear in the scan (255, 63, 188, 64) do not come with a
    template's 1 and are byte patterns elsewhere in the data, not option lists [I].)
  - **Checking the dials** [X: the vault update at `0x0019A6A8`, loop at `0x0019AA78`]: the combination is read from
    vault field +0x640. Dial i is right when its current state minus 912 equals ⌊combination × 10^i / 100⌋ mod 10:
    dial 0 the hundreds, dial 1 the tens, dial 2 the units.
  - **How it is shown**: Bentley speaks it in a dialogue after the bottles are collected; that dialogue is the only
    place the code is given and must not be skippable [R].
- Walking into the vault's trigger volume (option, not settled) before all bottles are collected plays a hint
  dialogue chosen by progress: under half the bottles, under 80 %, all but one, all collected; each level stores up
  to 4 dialogues per hint kind with option 1047 (kinds 0 to 4) [D] [R].
- **Use distance**: with all bottles collected, pressing the context button (Circle) within **250 units** of the
  vault's player marker (its child object OID 892, the point where the player stands to turn the dials; distance
  from the player's origin to that point) starts the vault, unless the vault set would already be complete [R].
  The vault's goad volume (a button volume, class 40) can also start it directly [R].
- With all bottles collected the player turns the dials with the direction pad or the face buttons: right or Circle
  selects/moves to the next dial, left or Square back, up or Triangle and down or Cross change the digit [R]. When all
  three dials match, the vault opens [R].
- **Opening** marks the level's vault goal done (task bit 2), counts a vault for the world, and grants the vault's
  reward: one bit of a game-wide reward mask. The rewards are the player's power-ups (player-moves.md); bits seen in
  the reference: 2 and 16, 4 and 1024, 8 and 4096, 32 (coins and clue bottles are pulled toward the player), 64,
  128, 512, 32768, 0x20000 [R]. The tutorial vault in Paris has no reward bit [R].

## 5. Master Thief Sprint (timed goal)

- In a level whose task mask has bit 3 (19 levels [D]), after the key and the vault are done, an hourglass (class 90)
  appears [R]. Its pickup deadline is world option 537 (values 8, 10 and 25 s seen in three worlds) [D] [R].
- Picking up the hourglass starts the sprint: "Beginning Master Thief Sprint!" [D, string], the charm count drops to 0,
  and a countdown starts from the world's option 538 plus 0.99 s (default 180 s; values on the disc include 180,
  135, 95 and 115 s) [D] [R]. Each alarm triggered during the sprint doubles the countdown speed
  (speed `2^n` for n alarms) [R].
- Reaching the level's exit with time left completes it the first time (task bit 3 set, the remaining time stored as
  the best time, a sprint counted for the world, auto-save) or improves the best time; running out of time fails
  [R]. Best times show as "Best time: m:ss", or tenths under 1 s [D, strings].
- Levels with the sprint done show a prompt to play the developer commentary for that level (the commentary table
  of [audio.md](audio.md) 2.3) [R].

## 6. Level and game progress

- **Per level** the game keeps a flag set [R]: bit 0 visited, bit 1 key collected, bit 2 vault opened, bit 3 sprint
  completed, bit 4 boss defeated; plus time spent, the sprint best time, the adaptive difficulty level and its raw value, 12
  "dialogue played" flags, 4 key/value pairs for scripts, 64 bottle bits and 8 checkpoint bits.
- **Task mask** (world table word at offset 40, [disc.md](disc.md) 4.2) [D]: which goals a level has, in the same bit
  positions. On the disc: `0xE` (key, vault, sprint) in 19 levels, `0x2` (key only) in 9, `0x0` in the 18 others
  (prologue, hub, bosses and the final world's levels).
- **Keys.** Collecting a key sets bit 1 once, counts a key for the world, plays the key celebration, and auto-saves
  [R]. A level whose key was collected loads without its key [R].
- **Percent complete** [R]: one task per visit of each of the 43 levels outside the prologue world, one per goal bit
  in each level's task mask, and one per finished episode boss (5). On the disc that is 43 + 19 × 3 + 9 + 5 = 114
  tasks [D]. Display: 0 when none, 100 when all, otherwise `floor(100 × done / total)` clamped to 1..99.
- **Endings** [R]: opening the last vault (all vault goals done) returns the player to the hideout and plays the movie
  "The Thievius Raccoonus" and the credits; the other completion route plays "Ten Seconds" and the credits
  ([movies.md](movies.md) 1.2).

## 7. Saving

### 7.1 Memory-card files

The game saves to the memory card in slot 1 [D, strings]:

| Item | Value |
|---|---|
| Directory | `BASCUS-97198YAOTWTD!` [D, executable string at file offset `0x14C3B0`] |
| Files written when the save directory is created | a 10-byte file with the directory's name; `icon.sys` (964 bytes); `sly.ico` (21,656 bytes); `save1`, `save2`, `save3` (6,656 bytes each); about 49 KB in all [R, sly-rs log] |
| Prompts | "Do you want to create a Sly Cooper Save File?", "Auto-save has been disabled", "Game Saved", and others [D, strings] |

Three save slots; the game auto-saves after a key, a vault, a sprint record and other milestones, and the pause menu
has "Save Game" and "Save and Quit" [D, strings] [R].

### 7.2 Contents of a save slot

Each `saveN` file holds one game-state record padded to 6,656 bytes [R]. Its logical content [R]:

| Part | Content |
|---|---|
| Header | version number (18), record size, a check word (the reference stores the record size there; whether the original computes a checksum is not settled), total play time |
| 6 world records | each: 9 level records (section 6), keys, vaults and sprints counted in the world, time spent, world flags (visited, finished) |
| Game | current world and level, lives, charms, coins, game flags (first bottle, half the bottles, all bottles, first vault, second vault), reward mask, unlocked movies, more flags, last power-up selected |

A new game starts in the prologue world with 5 lives, no reward bits and the last power-up "none" [R].

*The exact byte layout of `saveN`, `icon.sys` and `sly.ico` has not been checked against a real card: no card image
was available. A spec revision should take it from a card written by the game in an emulator.* SlyNX may use its own
save format; compatibility with original cards is optional.

## 8. Test vectors (per world, proxy clones counted) [D]

Columns: vault, clue bottles, charms, life tokens, coins, keys, hourglasses, race prizes, hiding baskets (class 58),
task mask.

| # | World | Vault | Bottles | Charms | Life tokens | Coins | Keys | Hourglass | Race prizes | Baskets | Mask |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | Splash | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 1 | Paris, France | 1 | 0 | 1 | 1 | 16 | 0 | 0 | 0 | 0 | 0 |
| 2 | The Hideout | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 3 | A Stealthy Approach | 1 | 20 | 1 | 1 | 35 | 1 | 1 | 0 | 0 | E |
| 4 | Prowling the Grounds | 0 | 0 | 2 | 5 | 5 | 0 | 0 | 0 | 0 | 0 |
| 5 | High Class Heist | 1 | 30 | 1 | 2 | 40 | 1 | 1 | 0 | 0 | E |
| 6 | Into the Machine | 1 | 30 | 2 | 2 | 63 | 1 | 1 | 0 | 0 | E |
| 7 | A Cunning Disguise | 1 | 30 | 2 | 2 | 25 | 1 | 1 | 0 | 2 | E |
| 8 | The Fire Down Below | 1 | 30 | 1 | 1 | 31 | 1 | 1 | 0 | 0 | E |
| 9 | Treasure in the Depths | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| 10 | The Gunboat Graveyard | 1 | 20 | 1 | 1 | 34 | 1 | 1 | 0 | 0 | E |
| 11 | The Eye of the Storm | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 12 | A Rocky Start | 1 | 40 | 1 | 1 | 32 | 1 | 1 | 0 | 0 | E |
| 13 | Muggshot's Turf | 0 | 0 | 1 | 1 | 3 | 0 | 0 | 0 | 0 | 0 |
| 14 | Boneyard Casino | 1 | 40 | 1 | 1 | 33 | 1 | 1 | 0 | 0 | E |
| 15 | Murray's Big Gamble | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| 16 | At the Dog Track | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 7 | 0 | 2 |
| 17 | Two to Tango | 1 | 30 | 2 | 1 | 140 | 1 | 1 | 0 | 0 | E |
| 18 | Straight to the Top | 1 | 40 | 1 | 1 | 58 | 1 | 1 | 0 | 0 | E |
| 19 | Back Alley Heist | 1 | 30 | 1 | 1 | 61 | 1 | 1 | 0 | 0 | E |
| 20 | Last Call | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 21 | The Dread Swamp Path | 1 | 20 | 1 | 1 | 54 | 1 | 1 | 0 | 0 | E |
| 22 | The Swamp's Dark Center | 0 | 0 | 1 | 1 | 5 | 0 | 0 | 0 | 0 | 0 |
| 23 | The Lair of the Beast | 1 | 30 | 2 | 1 | 87 | 1 | 1 | 0 | 0 | E |
| 24 | A Grave Undertaking | 1 | 40 | 2 | 2 | 51 | 1 | 1 | 0 | 0 | E |
| 25 | Piranha Lake | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| 26 | Descent into Danger | 1 | 40 | 1 | 1 | 35 | 1 | 1 | 0 | 0 | E |
| 27 | A Ghastly Voyage | 0 | 0 | 1 | 1 | 1 | 1 | 0 | 0 | 0 | 2 |
| 28 | Down Home Cooking | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| 29 | A Deadly Dance | 0 | 0 | 10 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| 30 | A Perilous Ascent | 1 | 30 | 1 | 1 | 10 | 1 | 1 | 0 | 0 | E |
| 31 | Inside the Stronghold | 0 | 0 | 1 | 1 | 2 | 0 | 0 | 0 | 0 | 0 |
| 32 | Flaming Temple of Flame | 1 | 25 | 1 | 1 | 76 | 1 | 1 | 0 | 0 | E |
| 33 | The Unseen Foe | 1 | 30 | 1 | 1 | 17 | 1 | 1 | 0 | 0 | E |
| 34 | The King of the Hill | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| 35 | Rapid Fire Assault | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 0 | 0 | 2 |
| 36 | Duel by the Dragon | 1 | 40 | 1 | 1 | 29 | 1 | 1 | 0 | 0 | E |
| 37 | A Desperate Race | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 7 | 0 | 2 |
| 38 | Flame Fu! | 0 | 0 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| 39 | A Hazardous Path | 0 | 0 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| 40 | Burning Rubber | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 41 | A Daring Rescue | 0 | 0 | 0 | 1 | 1 | 0 | 0 | 0 | 1 | 0 |
| 42 | Bentley Comes Through | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| 43 | A Temporary Truce | 0 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 |
| 44 | Sinking Peril | 0 | 0 | 0 | 1 | 3 | 0 | 1 | 0 | 0 | 0 |
| 45 | A Strange Reunion | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| | **Total** | **20** | **595** | **43** | **55** | **950** | **28** | **22** | **14** | **3** | |

Checks: keys (28) = levels with mask bit 1 (19 + 9) [D]; vaults outside Paris (19) = levels with mask bit 2 [D];
three worlds (35, 43, 44) hold an hourglass although their mask has no sprint bit, so it never appears there [D] [I].
Other option values: every coin, life token and hourglass carries option 91 = 6 and option 115 = 16 (the hit kinds
that affect it) [D]; every bottle option 288 = 1, option 110 = 24 [D]; charms option 91 = 14 [D].

## 9. Open questions

- (Settled: the vault combinations, section 4.2.)
- What the fourth adaptive-difficulty record (`0x261DD0`) is for.
- The exact byte layout of the save files and whether the record has a real checksum.
- Which hit kinds options 91 and 115 encode (shared with the player-moves spec).
- The exact power-up per reward bit (shared with player-moves.md).
