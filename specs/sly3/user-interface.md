---
id: sly3-behaviour-user-interface
title: Sly 3 user interface - screen objects, streamed screens, options, game-supplied strings - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; the six streamed screen files of v_ext decompressed and parsed;
      executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      every class-158 to 167 body (1,671) with its header, options and file record; the text elements of the shared
      screens (string IDs and game-supplied string IDs) resolved against the world string table; option-table entries
      1210 to 1243 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/screen3.h, screen3.cpp, transition.cpp, game/gen (class and option correspondence tables)
    used_for: >-
      the part (label) numbers, the meanings of the game-supplied strings, the Sly 2 to Sly 3 option correspondence
      (rows scored 80 or more), and the statement that Sly 3 runs Sly 2's screen runtime; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 user interface

Read [../sly2/user-interface.md](../sly2/user-interface.md) first: the screen runtime (labels, states, drawing,
text escapes) carries over [R]. Layouts are in [classes/effects-logic-ui.md](classes/effects-logic-ui.md) 6. Tags as
in [guards.md](guards.md).

## 1. The screen objects on the disc [D]

### 1.1 Counts

Classes 158 to 167 (Sly 2's 126 to 135): 1,671 bodies at every depth.

| Class | Bodies | Where |
|---:|---:|---|
| 158 | 1,405 | 936 in `stock_swf` (24 per world), 156 in `stock_objects` (4 per world), the rest in the episode macro groups (`c_macro` 88, `h_macro` 80, `o_macro` 63, `v_macro` 20, `p_macro` 16, `m_macro` 15) and a few job groups |
| 159 | 92 | episode macro groups |
| 160 | 45 | episode macro groups |
| 161 | 39 | one per world, object 1039 in `stock_swf` |
| 162 | 1 | |
| 163 | 27 | episode macro groups and `h_hack` |
| 164, 165 | 13, 8 | `h_macro`, `p_macro` |
| 166 | 2 | |
| 167 | 39 | one per world, object 1048 in `stock_swf` |

**`stock_swf` holds the same 26 screen objects in every world** (24 of class 158, 1039 of class 161, 1048 of class
167) and `stock_objects` four more (1061, 9935, 9937, 9939). They are the interface proper; the macro groups hold
each episode's own screens.

### 1.2 Header values

- Frame time: 1/60 s on 1,568 bodies, 1/30 s on 55, 1/12 s on 9, the largest `f32` on 39 (object 1061, the font
  holder, which never animates).
- Canvas: 640 × 480 on 1,593; 1,024 × 1,024 on 39 (object 9939, a picture used as a texture); 0 × 0 on 39 (1061).
  The word after the size is 1 on 1,180 bodies and 0 on 491 (meaning open, as in Sly 2).

### 1.3 Options [X] [D]

| Option | Sly 2 [R] | Kind [X] | Values [D] | Meaning |
|---:|---:|---|---|---|
| 1210 | 1122 | bool at +0x5D | always 0 | open |
| 1211 | 1123 | value type `0x1062` at +0x94 | 2 to 38 (34 ×274, 6 ×175, 7 ×118, 10 ×117) | draw **priority**, higher on top [R] |
| 1212 | 1124 | bool at +0x9C | 1 ×406, 0 ×156 | the game **pauses** while the screen is shown [R] |
| 1213 | — | bool at +0xA0 | 1 ×236 | new; open |
| 1214 | 1125 | float at +0x7C | 5.0 ×117, 1.5 ×13 | seconds in the idle label before hiding [R] |
| 1225 to 1234 | 1133 to 1142 | 16-bit at +0xBC + 4 × i | string IDs | the screen's ten **arguments** (texts shown by game-supplied strings 834 to 843) [R] |
| 1236 | — | float at +0xF0 | 100.0 ×60 | open |
| 1237 to 1241 | 1143 to 1147 | 16-bit at +0x120 … +0x130 | string IDs 5363 to 5367, on 1039 | the camera screen's five messages [I] |
| 1242, 1243 | 1150, 1151 | method, one argument | object IDs | open |

## 2. Streamed screens [D]

Six objects per world (234 bodies) end in a file record instead of their element data: 1056, 1062, 1063, 1064,
1065 and 1389, all in `stock_swf`. Each world has its own copies. The file is compressed like a world file and
decompresses to `ZRB.`, a `u16` version **7** (Sly 2: 6), then the element set and group tree of an inline body
(the six v_ext files parse to their end with that layout).

| Object | Screen (by its strings) | v_ext file: sector, compressed → decompressed |
|---:|---|---|
| 1056 | pause and options menu (strings 120 "Effects Volume", 119 "Music Volume", 78 "Center Screen...", 61 "Current Game Status", 60 "Data Access", …; 39 text elements) | 805,184: 61,559 → 145,469 |
| 1062, 1063, 1064 | the three gadget grids (string 5337 "Gadget Grid") | 825,792: 30,827 → 67,773; 825,824: 30,512 → 67,057; 805,152: 20,512 → 45,424 |
| 1065 | the shop (663 "Connecting to ThiefNet…", 5551 "Buy Gadgets", 662 "Available Gadgets") | 805,216: 10,808 → 25,703 |
| 1389 | job help (4686 "Jobs", 4687 "more jobs", 4684 "Goals", 5589 "Complete!", 5590 "Not complete") | 825,856: 13,282 → 39,576 |

The shop file has **no sell page**: none of its 20 text elements is a selling text (Sly 2's file had "Sell Loot")
[D]. 1062 is Sly's grid, 1063 Bentley's and 1064 Murray's ([gadgets.md](gadgets.md) 2) [X].

### 2.1 Screen sets in Sly 2's form [D: from the implementation's parse; partly re-checked]

The element sets of Sly 3 screens map onto Sly 2's layout ([../sly2/user-interface.md](../sly2/user-interface.md) 3)
with these differences:

- A **group reference** is an item of kind 4: an `s32`, then the group (Sly 2: a `u16` sprite number).
- Each **key placement** is:
  - the element;
  - 4 bytes: slot, an unknown byte, a flag, and a colour-pair index, where 127 means none (Sly 2: an `s16` with −1
    for none);
  - an `s32`;
  - the matrix.
- The **trailing bytes** of a key are the slots that the key empties before its placements apply, as in Sly 2.
- The **idle marker** is **4869**.

Check made here [D]: 4869 is not an immediate value anywhere in the executable, so it is data, not a fixed label in
code. In the decompressed v_ext world, the 32-bit value 4869 occurs 31 times inside class-158 screen objects, and
the 16-bit value 32 times. The rest of the layout was not re-parsed here. It rests on the implementation, which
parses the screens to their last byte.

## 3. Shared inline screens [D] [R]

Text elements of kind 3 name either a world string (text kind 0) or a **game-supplied string** (text kind 1: an ID
the game fills in at draw time). The shared screens, identified by their strings:

| Object | Class | Strings [D] | Role |
|---:|---:|---|---|
| 1048 | 167 | game string 812 | coin counter: 812 is the coins shown [R]; idle limit 5 s |
| 1049 | 158 | game string 813 | a second counter [I]; idle limit 5 s |
| 1053 | 158 | game string 814 | HUD timer, "minutes:seconds" left [R]; idle limit 5 s |
| 1057 | 158 | game strings 815 to 822 | open (eight game-supplied values) |
| 1054 | 158 | game strings 998, 999 | open |
| 1042 | 158 | game string 826 | open |
| 1039 | 161 | game strings 826 to 834; arguments 5342 ("R: take picture / right analog stick: zoom"), 4683 "Help", 4655 "Exit" | the camera (photo) screen [I] |
| 1058 | 158 | game string 846 | the **prompt** (846 = the prompt's text) [R] |
| 1060 | 158 | game strings 844, 845 | the **subtitle** (844 = the subtitle text, 845 its buttons) [R] ([dialogue-and-voice.md](dialogue-and-voice.md)) |
| 1066 | 158 | 4803 "COMPLETE!", 4804 "JOB", 489 "Press S to Replay Challenge…", 490; game strings 1110 to 1112 | job complete |
| 1067 | 158 | 4801 "Job Failed!", 659 "Press X to try again."; game string 851 | job failed; 851 is the job's failure line [R] |
| 1466 | 158 | game strings 847, 848 | the memory-card dialog (message and its OK / Yes-No buttons) [R] |
| 1467 | 158 | 56 "The disc was removed…" | disc error |
| 1701 | 158 | game string 1702 | open |
| 9904 | 158 | game strings 852, 853 | open |
| 9912 | 158 | 5586 "X take 1 pace", 5587 "O next clue/dig"; game string 834 | treasure-map steps [I] ([pickpocket-and-pickups.md](pickpocket-and-pickups.md)) |
| 875 | 158 | no texts (17,871 bytes) | the restart wipe ([level-flow.md](level-flow.md) 7) |
| 874, 1909, 9925 | 158 | no texts | open |

Game strings 834 to 843 show the screen's arguments (option 1225 + i) [R]. The button-icon escapes (`&2X&.`,
`&2S&.`, …) are Sly 2's [D: they occur in the strings].

## 4. How a screen is shown [R]

As Sly 2 section 2, with Sly 3's label numbers: **886 show, 887 idle, 888 hide** (Sly 2's `0x30F` to `0x311` plus
103); **891** is the idle used by a screen without 887. The restart wipe's pairs (880, 881), (882, 883), (884, 885)
are labels of the same kind ([level-flow.md](level-flow.md) 7).

## 5. Test vectors [D]

- Object 1065 in v_ext: class 158, 95-byte body, options 1211 = 34, 1212 = 1, 1210 = 0; file record: sector 805,216,
  10,808 bytes, identity 165; decompressed 25,703 bytes, starting `ZRB.` and version 7.
- Object 1039 in v_ext: class 161, 19,208 bytes, 43 elements; option 1225 = `0x14DE` (5342).
- Object 1048 in v_ext: class 167, 8,991 bytes, 9 elements; options 1211 = 10, 1214 = 5.0.

## 6. Open questions

1. The roles of 874, 1042, 1054, 1057, 1701, 1909, 9904, 9925, and of classes 159, 160, 162 to 166.
2. (Settled: the grids and their cell tables, [gadgets.md](gadgets.md) 2.)
3. Options 1210, 1213, 1236, 1242, 1243.
4. The game-supplied strings not listed in section 3 (the table has 178 entries [R]).
5. The HUD gauges and the menus' behaviour beyond Sly 2's.
