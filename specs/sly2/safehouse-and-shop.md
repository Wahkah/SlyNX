---
id: sly2-behaviour-safehouse-and-shop
title: Sly 2 safe house, shop, gadget assignment, loot and job help
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      options of every safe-house object (class 70) and of the placements that add actions to it, walked in all 43
      worlds; the item table, the loot table and the three gadget-grid tables read from the executable; item and loot
      names from the worlds' string tables; the save-variable tree (names and shapes of the coin, loot, item and
      gadget-slot variables) from the world header
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release, files shop, po (safe house), gadget, pickup, marker, gamebif
    used_for: what the safe house, the shop, the gadget assignment, the loot pickup and the job help do; marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), GPL-3.0, read for facts only
    used_for: confirmation of table addresses
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 safe house, shop, gadget assignment, loot and job help

Tags: [D] disc data, [O] observable, [R] reference behaviour (to be confirmed by play), [I] inferred.
Executable tables are read from the player's executable after checking its SHA-256 (above); a virtual address
`va` is at file offset `va − 0x100000 + 0x80` ([disc.md](disc.md) 3).

Names of progress variables in back-quotes (`loot`, `ccoin`, …) are the game's own names, stored in every world's
save-variable section ([world-file.md](world-file.md) 3.3) [D].

## 1. The safe house object

- Class 70, name string `safe_house`, object ID 17239, exactly one in each of the 8 episode hubs (level-table rows 2,
  8, 12, 14, 17, 27, 32, 38) and nowhere else [D].
- Its options are the same in all 8 hubs [D]:

  | Option | Value | Meaning |
  |---:|---|---|
  | 13 | 3000.0 | activation radius ([clock-and-loop.md](clock-and-loop.md)) |
  | 1337 | object 27339 | the state machine that seats the characters and the computer [R] |
  | 1339 | list (8, 10, 7, 9) | the **selectable states**: 7 Sly, 8 Bentley, 9 Murray, 10 the computer [R for the meanings] |
  | 1338 | (10, buttons `0x40`, object 888) | an **action**: in state 10, the Cross button opens the shop screen (object 888) [R for the meaning] |
  | 8 | two records: (1367, −1), (1368, −1) | not established |

- Each hub's level package adds further actions through placement option blocks (option 1338, [D]): in state 7, 8
  or 9, Cross starts a script object that takes that character out into the hub. Script objects: 27264 (Sly),
  27266 (Bentley), 27262 (Murray) in rows 8 to 38; 141710, 141708, 141706 in row 2 [D]. Row 14 (episode 4, the
  prison) has no action for Murray [D]: in that episode Murray cannot leave the safe house [R], which matches the
  story [O].

## 2. Being in the safe house [R]

- Entering: the player walks in through the hub's door; a script makes the safe house the current "player"
  object. While it is, no character takes damage ([characters.md](characters.md) 2) [R].
- On entering: a running job that is not marked "keep" ends; every character puts back any carried loot bag and has
  health and gadget power refilled; objects made by gadgets (class 79 and its related gadget classes) are removed;
  the state machine is set to the saved current character (`oidPoCurrent`), else its first state; script event 5 is
  raised, and the job system gets event 26 [R].
- **Choosing.** Left and right step through the state machine's states, skipping those not in the selectable list;
  the list wraps around [R]. The hub's script narrows the list as the story goes: for example the nightclub hub
  starts with (10, 7), the computer and Sly, and adds Murray and Bentley as their first jobs open [R]. A script can
  set the list (at most 6 states) at any time [R].
- **Acting.** In the shown state, the button named by a matching action (option 1338) either opens the shop (target
  888) or starts the target script [R]. Leaving raises script event 6 and the job system's event 27 [R].
- Character switching happens only here in hubs ([characters.md](characters.md) 8) [R] [O].

## 3. The shop (the computer) [R]

The shop screen pauses the game (rate factor 0 at 0, [clock-and-loop.md](clock-and-loop.md)) and has two pages,
switched with Triangle:

- **Buy** lists the items of the item table ([characters.md](characters.md) 6; records 7 to 42) that are **open** and
  **not owned**, in table order. An item is open when its episode field is not −1 and the best episode reached
  (`episodeBest`) is at least that episode. Six rows are shown at a time.
- Cross on a row asks for confirmation (a question that stays until answered); a second Cross buys: the price is
  subtracted from the coins, the item's "acquired" flag is set, the job system gets event 59, and every character
  is told (script event 26). Without enough coins a message shows for 5 s instead. Circle cancels the question;
  Circle on the page closes the shop.
- **Sell** lists the loot kinds held (count > 0). Cross sells one: the count drops by 1 and the loot's value
  (section 5) is added to the coins. There is no confirmation.
- Sounds: 41 on a question and on a purchase or sale, 54 on a refused action or a message, 53 on a page change and
  on a cursor step (global sound IDs, [audio.md](audio.md)).

**Prices and opening episodes** are the item table's ([characters.md](characters.md) 6) [D]: per episode one item
for each character opens, prices from 200 (Trigger Bomb, episode 1) to 2,100 (Diablo Fire Slam, episode 8). The
24 sold items cost 24,350 coins in all (Sly 7,550, Bentley 8,200, Murray 8,600) [D]. The twelve items with price 0 and episode −1 are never offered
[D]; they are granted by clue-bottle rewards (section 6) [R].

## 4. Coins [D] [R]

- The coin count is the save variable `loot` / `ccoin`, a single integer [D]. A picked-up coin adds 1 [R]; the shop
  and loot sales change it as above.
- No limit is stored with the variable [D]; the reference reports that the counter stops at 9,999 [R], which
  needs confirming in play.

## 5. Loot [D]

- Loot table: executable virtual address `0x002C3288`, **64 records of 12 bytes**: `s32` value in coins, `s32` name
  string ID (228 to 291), `s32` a second string ID (1114 to 1177; meaning not established) [D].
- Records 30 to 35 have value 0 and repeat the names of records 6 to 8 and 3 to 5 [D]; they are never sold for coins
  [I]. The other 58 values range from 11 (record 0, "Bronze Comb") to 803 (record 62, "Golden Headdress"); the 64
  values sum to 15,425 [D].
- The count held of each kind is the save variable `loot` / `mplootkc`, an array of 64 integers indexed by record [D].
  Picking up a loot item (from a pickpocketed pocket or a loot pickup, [gameplay-overview.md](gameplay-overview.md)
  8) adds 1 to its kind's count [R]. Some loot is carried as a bag that is lost when the character is hit hard
  enough ([characters.md](characters.md) 2) and put back on entering the safe house [R].

## 6. Owned items and the gadget assignment

- Owned items: the save variable `powerUps` / `mppukfAcquired`, 43 flags indexed by item-table record [D]. Set by
  the shop, by scripts (a built-in sets or tests a flag; for example episode 5's paraglide job waits, testing every
  2 s, until item 27, "Paraglide", is bought) and by the clue-bottle reward [R].
- **Clue-bottle reward** [R]: a hub marker shows the reward once all the episode's bottles are collected, no job is
  running, the reward item is not yet owned and, where the marker names a job, that job's last task is done; taking
  it sets the item's flag. These are the twelve unsold items (section 3) [I].
- **Mega Jump** (item 31) is taken away again when a world is reloaded with a particular reload flag [R].
- **Assignment screen** [R]: opened with Select during play (not only in the safe house); it pauses the game and
  shows the current character's grid. Each character's grid is an executable table of 32-byte cells [D]:

  | Character | Virtual address | Cells |
  |---|---|---:|
  | Sly | `0x002C2DE8` | 18 |
  | Bentley | `0x002C3028` | 9 |
  | Murray | `0x002C3148` | 8 |

  Cell: `s32` item record, three `s32` screen-part IDs (open, owned, selected), `s32` −1, `f32` x, `f32` y,
  `s32` flags (bit 0: may be put on a button) [D]. Example: Sly's first cell is item 23 "Smoke Bomb" at (145, 155),
  flags 1. Items with button use −1 (Paraglide, Silent Obliteration, Juggernaut Throw, Atlas Strength, Long Toss)
  have flags 0 [D]. Bentley's grid includes item 36, Long Toss [D].
- The stick or d-pad moves to the nearest owned cell in its direction (within 750 units along the direction and 40
  across it, else the nearest within 200 across) [R]. L1, L2 and R2 put the selected item on that slot (slot
  order L1, L2, R2: button bits 4, 1, 2, an executable table at `0x002C3258`) [D for the table]; an item already on
  another slot leaves it. Messages: not owned, not assignable [R].
- The three slots per character are the save variables `sly` / `bentley` / `murray` → `apukCur`, three integers each
  (item record or −1) [D].

## 7. Job help [R]

Opened from the pause menu (not the safe house). Page 1 lists the episode's open jobs (up to 10 rows), with a check
mark when a job's last task is done; page 2 lists a job's goals (up to 4 rows: the done tasks and the current one,
using each task's goal text). It pauses the game. Right or Cross opens a job; left goes back; Circle or Start closes.

## 8. Test vectors [D]

- Safe-house option 1339 bytes (all hubs): `06 80 04 08000000 0a000000 07000000 09000000` (list tag, 4 entries).
- Option 1338 bytes on the safe house: `0600 0a000000 3c10 40000000 0600 78030000`; on row 8's placement for Sly:
  `0600 07000000 3c10 40000000 0600 806a0000`.
- Loot records 0 to 5: values 11, 19, 27, 17, 40, 60; names "Bronze Comb", "Silver Comb", "Gold Comb", "Bronze
  Ring", "Silver Ring", "Gold Ring".
- Item records 0 to 6 hold only a button-use field (−1, −1, −1, −1, 2, 2, 1) and zeros.
- Save-variable shapes (nightclub hub header): `ccoin` single integer; `mplootkc` 64 integers; `mppukfAcquired` 43
  flags; `apukCur` 3 integers.

## 9. Open questions

- The meaning of option 8's two records on the safe house.
- The coin cap (9,999 per the reference only).
- The loot table's second string ID.
- (Resolved: Long Toss's owner code 7 and its place on Bentley's grid do not conflict; see characters.md 6.)
- All [R] behaviour needs checking in play.
