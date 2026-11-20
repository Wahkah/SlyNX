---
id: sly2-behaviour-user-interface
title: Sly 2 user interface - screen movies, HUD, menus and text
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every class 126-131 record in every package's object list parsed to its declared size (1,803 records);
      their option lists, frame times, sizes, label numbers and text strings; the six streamed screen files read
      from the disc, decompressed and parsed to their end; the menu tables, gauge smoothing values, button-glyph
      table and format strings read from the executable after the hash check; strings looked up in the worlds'
      string tables; all test vectors below
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/hud.h, hudwidget.h, hudwidget.cpp, screenobj.h,
      screenobj.cpp, screenrt.h, screenrt.cpp, menu.h, menu.cpp, frontend.h
    used_for: >-
      what the game does with the screen objects (states, show / idle / hide labels, the gauge, the prompt, the
      counters, the timer, the menus and their availability rules, the text escapes, the screen units); marked [R]
  - kind: disc
    what: >-
      re-check 2026-11-19: every shape primitive of row 0's inline screen bodies (top level and the fonts nested in
      object 884) and of the six streamed screen files, decompressed from the disc
    used_for: >-
      the order and value ranges of the three primitive setting bytes, per-primitive colour uniformity, the
      per-vertex byte, the glyph colours of the two fonts in object 884, the text colour words of item kind 3
      (section 3)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/screenobj.h, screenobj.cpp, hud.cpp, splayer.cpp (re-read 2026-11-19)
    used_for: meaning of the first setting byte (draw layer), the colour rule and the text tint (section 3); [R]
  - kind: disc
    what: >-
      SCUS-97316, the streamed gadget-grid screens 885, 886, 887 of row 0 (decompressed and parsed to their last byte:
      45,933, 59,119 and 72,773 bytes), every sprite key of them; checked 2026-11-19
    used_for: grid labels, cell sprites and cell placements, sprite-key fields (sections 2, 3, 6.3) [D]
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…): grid cell tables at 0x2C2DE8 / 0x2C3028 / 0x2C3148, the grid opening call
      (label immediate 0x318 at 0x1DB71C), the colour-pair conversion in routine 0x213300 (0x213614 to 0x213714)
    used_for: cell record fields, the label the grid is shown with, the offset and scale rule (sections 3, 6.3) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/shop.cpp (gadget assignment), screenrt.cpp, splayer.cpp (key crossing)
    used_for: names of the cell fields, how a key's placements, released slots and colour pairs are applied [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 2 user interface

Sly 1 draws its 2-D overlays from its own code and a few world objects; Sly 2 draws almost all of them from
**screen movies**: small animated vector pictures stored as objects of classes 126 to 131 ([D] layout:
[classes/effects-logic-ui.md](classes/effects-logic-ui.md) 4). The game shows, hides and drives them by object ID
and by **label** numbers inside them. This spec says which screen does what and with which numbers. Notation and
tags as in [README.md](README.md).

## 1. The screen objects on the disc

### 1.1 Counts [D]

Records in the packages' object lists (all worlds, every package):

| Class | Records | Where |
|---:|---:|---|
| 126 | 1,465 | 1,247 in the package `swf_ui`, the rest in each episode's `<episode>_macro` package and `jb_intro_macro` |
| 127 | 41 | one per world, in the episode's `_macro` package |
| 128 | 41 | one per world, in the episode's `_macro` package |
| 129 | 212 | several per world, in the `_macro` packages |
| 130 | 43 | one per world: object 866 in `swf_ui` |
| 131 | 1 | object 144117, package `dvd_menu` (the menu world, row 1) |

Counted as bodies at every depth (object-stream.md): 1,659, 41, 41, 212, 43, 1 [D].

**The package `swf_ui` holds the same 30 screen objects in every one of the 43 worlds** (1,290 records) [D]. It is
the interface proper: HUD, menus, prompts, counters and the fonts. The `_macro` packages hold each episode's own
screens (title cards and similar) [I from their package names].

### 1.2 Frame time, size and options [D]

- Frame time: 1/60 s on 1,515 records, 1/30 s on 245, and the largest `f32` on 43 (object 884, the font holder,
  which never animates).
- Canvas size: 640 × 480 on 1,717 records; 1,024 × 1,024 on 43 (object 6414, a picture used as a texture);
  0 × 0 on 43 (object 884). The word after the size is 0 or 1 (1,341 records 1) [D]; its meaning is open.
- Options seen on screen objects, with the meaning the reference gives to the fields they set [R]:

| Option | Value type | Values on the disc [D] | Meaning |
|---:|---|---|---|
| 1122 | byte | always 0 | (open) |
| 1123 | `u32` | 2 to 22 | draw **priority** (higher on top). Gauge 3, counters 4, prompt 6, menu screens 18, card icon 21 |
| 1124 | byte | 0 or 1; 1 on the six streamed menu screens | the game **pauses** while the screen is shown (rate factor 7 of clock-and-loop.md) |
| 1125 | `f32` | 5.0 on objects 877, 878, 879 | the longest time the screen stays in its idle label before hiding itself |
| 1133 to 1142 | 16-bit | string IDs, on object 866 | the screen's ten **arguments** (string IDs shown by its text items) |
| 1143 | 16-bit | on object 866 | (open) |

### 1.3 Streamed screens [D]

258 class-126 records (6 objects × 43 worlds) end in a 64-byte file record ([disc.md](disc.md) 4) instead of their
picture data. Each world has its own copy of each file. The file is compressed exactly like a world file
([world-file.md](world-file.md) 2). Decompressed, it is:

| Size | Type | Meaning |
|---:|---|---|
| 4 | ASCII | `ZRB.` |
| 2 | `u16` | 6 (version) |
| var | | the item dictionary and the sprites, exactly as in an inline body (classes/effects-logic-ui.md 4), to the end of the file |

The game loads the file the first time the screen is shown [R].

| Object | Screen (identified by its strings) | Example file (row 0) |
|---|---|---|
| 880 | pause and options menu ("WE'LL BE RIGHT BACK!", string 309) | sector `0x53420`, 46,443 → 116,025 bytes |
| 885, 886, 887 | gadget grids of Sly, Bentley, Murray ("Gadget Grid", 3151) | `0x55760`, `0x53440`, `0x55780` |
| 888 | the shop ("Connecting to ThiefNet…", 3147; "Buy Gadgets" 3145, "Sell Loot" 3146) | `0x557A0`, 12,479 → 32,188 bytes |
| 1248 | job help ("Jobs", 312, and the job texts) | `0x53400`, 11,878 → 34,218 bytes |

## 2. How a screen is shown [R]

- A screen's sprites carry **labels** (sprite-label IDs in the data). Three labels are shared by convention
  [D: they occur on most `swf_ui` screens]: `0x30F` **show** (the screen enters), `0x310` **idle** (on screen),
  `0x311` **hide** (it leaves). A screen without them appears and disappears at once.
- A screen can also be shown with an explicit label: that label plays as the idle label, with no show label and no
  hide label (hiding is immediate) [R]. The gadget grids are shown this way, with label **792** (`0x318`, an immediate
  in the grid code at `0x1DB71C`) [D]. Their root sprite carries no `0x30F`–`0x311` labels; its labels are `790`
  (0.65 s, 12 keys), `792` (0.25 s), `791` (0.5 s, 9 keys) and the markers 5572, 16732, 16733 between them [D], so
  790 / 792 / 791 read as show / idle / hide by their order and lengths [I]; with the explicit-label call only 792
  plays [R].
- States: not loaded (streamed, before first use), stopped, idle, showing, hiding. Showing plays the show label then
  the idle label; hiding plays the hide label, then the screen is gone.
- A screen with an idle limit (option 1125) hides itself after that many seconds in idle.
- Screens are updated every frame on the **real** clock, so they keep animating while the game is paused
  ([clock-and-loop.md](clock-and-loop.md)).
- Scripts can show and hide a screen, select or reset a label on it, set a label's playback fraction or time, and set
  its ten arguments (options 1133 to 1142, also as script operations: scripts.md 4).
- Screens are drawn after the 3-D scene, in priority order (option 1123).

## 3. Drawing

- **Units.** The data's canvas is 640 × 480 [D]; the PS2 frame is 640 × 448 pixels, so the picture is scaled
  vertically by 448/480 [R]. Coordinates in shapes are signed 16-bit 12.4 fixed point (sixteenths of a pixel)
  [R; the `s16` x, y of classes/effects-logic-ui.md 4].
- **Shapes** (item kind 1). Each primitive's five header bytes are: vertex count, then three setting bytes, then
  the per-vertex-byte flag (classes/effects-logic-ui.md 4). The three setting bytes, in stored order:

  | Byte | Meaning | Values on the disc (row 0 and the six streamed files) |
  |---:|---|---|
  | 1st | **draw layer**: a primitive's depth is 100 × this value + the depth of the placement that draws it, which orders the primitives within one shape [R] | 0 to 22 in screen shapes; 0 to 4 in the font glyphs [D] |
  | 2nd | **topology**: 1 triangle fan, 2 triangle strip, 3 line strip [D for the values, R for the meaning] | 1, 2, 3 only. Every 2-vertex primitive has 3; strips have at least 4 vertices [D] |
  | 3rd | unused [R] | always 1 [D] |

  An earlier version of this section named the first byte as the topology; that was wrong.
  - Every primitive has **one colour**: all its vertices carry the same RGBA word [D] (5,391 primitives checked).
  - The per-vertex byte is used only when the primitive's flag byte is 1, and the flag is 1 only on strips [D]. A
    vertex byte of 255 marks a **no-draw vertex** (the GS "no kick" bit): the triangle that would end at that vertex
    is not drawn, and the strip otherwise carries on through it with its normal winding; two such vertices in a row
    would start a new strip, as in the mesh strips of geometry.md [R for the meaning; implementation rendering of
    both fonts confirms it]. On the disc every 255 is a single vertex, never two in a row: 4,807 in the 967 flagged
    strips (1,457 in the font glyphs, 3,350 in screens), one of them on the first or second vertex of its strip (where it has no effect); the other per-vertex bytes
    are all 0 [D]. An earlier version said a 255 "starts a new strip"; that reading tears the text font when drawn.
  - **Colour rule** [R]: the drawn colour is clamp(vertex colour × scale + offset, 0..255) per channel, where scale
    and offset are the colour pair of the placement (sprite key) that draws it; alpha 0x80 = opaque as everywhere on
    the PS2.
- **Text colour.** The text font's glyphs are stored **black**: in object 884, the first font (224 characters, the
  text font) has every glyph vertex at `0x80000000` (R = G = B = 0, alpha 0x80) or `0x00000000` [D]. They are
  tinted when drawn [R]:
  - the text item (kind 3) carries a colour word, its second `u32` (over all worlds, among text items of kinds 0 and 1: `0x7F666666`,
    grey, in 787; `0x26000000` in 721) [D];
  - the drawing offset becomes min(placement offset + the colour word's R, G, B, 255) per channel, and the alpha
    scale becomes clamp(placement scale alpha × the word's alpha / 127, 0, 1). With black glyphs and scale 1 the
    text comes out in the item's colour, its alpha scaled by the word's alpha on a 0..127 scale [R];
  - the subtitle string (0x2EF) uses the speaker's colour instead (dialogue-and-voice.md) [R];
  - the second font in object 884 is the **button-icon** font: its glyphs carry their own colours (for example
    `0x8010283D`, `0x7F333333`) [D] and are drawn after `&x` with the placement offset only, not the text colour,
    so the icons keep their own colours [R].
- **Sprite keys** (layout: classes/effects-logic-ui.md 4). A key changes the sprite's **slots** (each slot holds one
  dictionary item at a time):
  - a **placement** (50 bytes) is: `u16` item number (0 = keep the item the slot already holds), `u16` slot, `u16`
    link (0 on every grid placement), `s16` colour-pair index (−1 = none), `u8` interpolate (1 = ease the slot's
    matrix and colours toward its next placement in a later key), `u8` (0), `s32` (−1 on the grids), then the 3 × 3
    matrix, read as a 2-D affine transform: rows 1 and 2 are the axes, row 3 the translation in canvas units. It is
    the slot's transform, relative to the sprite that holds the slot [D layout and grid values; R meanings];
  - its **colour pairs** are (offset word, scale word) in that stored order. Per channel, offset = byte − 127 (so
    0x7F is no offset; one unit per step on the channel's own scale, where alpha's full scale is 0x80) and scale =
    clamp((byte − 127) / 127, −1, 1) (so 0xFF is 1 and 0x7F is 0); the executable converts them exactly so
    (routine `0x213300`, using 127.0 and 1/127) [D]. A slot with no pair uses offset 0x7F7F7F7F and scale 0xFFFFFFFF
    [R]. Nested sprites combine with their parent: scale = parent scale × own scale; offset = parent scale × own
    offset + parent offset [R];
  - its **16-bit values** are slots the key empties (releases) before its placements apply; on the grids every
    such value is below the sprite's slot count [D];
  - an item of kind 4 is a **reference**: its `u16` value n names sprite n of the same screen, which then plays
    inside the slot (in grid 885 the kind-4 items hold 1 to 28, the screen's 29 sprites being 0 to 28) [D for the
    values; R for the meaning].
  - The grid data shows the colour pairs varying only in the lowest byte of the offset word (01 … FF) with the
    lowest byte of the scale word at 0x7F; which channel that byte drives (it behaves as a fade if it is alpha) is
    not settled [I].
- **Textured quads** (kinds 6 and 7) show a texture by ID; kind 6 is blended through the screen's alpha [R].
- **Wide screens.** The game is 4:3. A port showing wider pictures should keep each screen's 4:3 canvas centred
  and extend only full-screen backgrounds [I].

## 4. Text

### 4.1 Strings

- Every visible text is a string ID into the world's **string table** (world-file.md 3.5) [D]. Text items (kind 3)
  hold the string ID in their last field when their text kind is 0 or 1 [D].
- IDs whose table entry is empty are **filled in by the game** at draw time [D for the empty entries, R for the
  rule]:

| ID | Content | Format [D: string in the executable] |
|---:|---|---|
| 719 | coins owned (progress value `loot` / `ccoin`) | `%d` |
| 720 | clue bottles found / needed in this world (30 in a hub) | `%d/%d` (`0x2A8AA8`) |
| 721 | the HUD timer: time left, never below 0 | `%d:%02d` (`0x2A8AB0`) |
| 732 | binocucom magnification by character: Sly `x%d`, Bentley `x%5.3f`, Murray `Far` below 4, `Near` below 8, else `YIKES` | `0x2A8AB8` to `0x2A8AD8` |
| 733 to 740 | photo-job texts (gameplay spec) | |
| 741 to 750 | the screen's ten arguments, as strings | |
| 751 | the current talk's subtitle ([dialogue-and-voice.md](dialogue-and-voice.md)) | |
| 752 | the buttons of the current talk's button wait, as icons | |
| 753 | the HUD prompt's string (section 5.2) | |
| 756 | the prize title card's name (loot table string, safehouse-and-shop.md) | |

IDs 751 to 753 are not empty in the table: 751 is "Use your Binocucom by pressing the R3 button…", 752 "Press the
START button for help.", 753 "To aim your sleepdart gun…" [D], and screens 872–874 and 883 show 751 and 752 as
hints. The reference's reading of 751–753 as game-filled values (subtitle, button icons, prompt) therefore holds
at most for the screens that request them that way; this is open (section 8).

### 4.2 Escapes and button icons

- `&x` switches to the second colour and the second font (the button-icon font); `&.` switches back; `\n` breaks
  the line [R]. Long lines wrap at spaces inside the text box [R].
- `&2<c>&.` shows a button icon [D: format `&2%c&.` at `0x2A9878`]. Icon letters [D: table of 16 records
  {`u32` button bit, byte letter} at `0x2A97F8`]: Cross `X`, Circle `O`, Triangle `T`, Square `S`, Start `!`,
  Select `_`, Up `^`, Down `v`, Left `<`, Right `>`, L1 `L`, L2 `l`, L3 `(`, R1 `R`, R2 `r`, R3 `)`.
  Example: string 754 "To pickpocket guards sneak up behind them and press &2O&.." [D].
- **Fonts**: the two fonts (text and button icons) are item-kind-8 sets in screen object 884 of `swf_ui` (2 sets,
  in every world) [D]. A font lists its characters (sorted) and each character's advance width; each character is
  a nested item drawn like a shape [D layout, R use]. Text size: glyphs are drawn at (text size) / 51.2 [R].
- Alignment within a text box: horizontal left, centre, right; vertical top, centre, bottom [R].

## 5. The HUD

All HUD screens are in `swf_ui` [D].

### 5.1 Character gauge (object 875)

- Shows the current character's **health** and **gadget power**. Hidden on the title screen and when the
  current object is not one of the three characters [R].
- Each character has five labels [D: labels `0x33D` to `0x34B` on object 875]: Sly `0x33D`–`0x341`, Murray
  `0x342`–`0x346`, Bentley `0x347`–`0x34B` [R for the assignment]. In order: normal, took damage, appear (on
  switching character or after being hidden), and two special poses (being carried, and one other state).
- Two fill bars are labels whose playback **fraction** is set: `0x34F` health, `0x350` power, at fraction
  1 − (value / maximum) [D labels, R rule]. Their label durations are 1.6667 s [D].
- The power bar shows only when the character owns at least one item with a power cost ([gadgets.md](gadgets.md))
  [R].
- **Easing.** A rise shows at once. A fall eases toward the new value with the smoothing record (100.0, 0.0, 0.5)
  at `0x2C3248` [D values; R use], until within 0.0001; the damage label plays while health falls [R].
- At zero health the gauge goes to its appear state and waits for health to rise again [R].

### 5.2 Prompt (object 881)

- Shows one string (ID 753 is its text) for a set time, or until ended when the time is negative [R].
- A new prompt while one is shown is queued (one slot); the shown one plays label `0x312` (swap) and the queued
  one follows. Asking for the string already shown only updates its time [R].
- The time counts only while the prompt is actually visible [R].

### 5.3 Counters and timer (objects 877, 878, 879)

- Coins (877, string 719), clue bottles (878, string 720), timer (879, string 721) [D strings; R roles].
- Each shows when its value changes and hides itself after **5.0 s** idle (option 1125) [D value; R behaviour].
- The timer is started by scripts for timed jobs with an end time; it shows "minutes:seconds" left. Its time left is
  carried over a world change [R].

### 5.4 Other HUD screens

| Object | Shows [D strings; R role] |
|---|---|
| 1324 | tutorial hints (string 754 and its pair 755) |
| 1325 | "The disc was removed…" (string 43) |
| 1492 | memory-card activity ("Woops!", 1493). Shown during a save; hidden 1 s after the save ends [R] |
| 872, 873, 874, 883 | the hints of strings 751 and 752 |
| 863, 864, 865, 869 | binocucom overlays (argument texts 741, 742; magnification 732) |
| 866 (class 130) | a screen driven by its ten arguments (options 1133–1142) [D] |
| 777, 778 | full-screen wipes (778 is the iris-like wipe used by level changes) [R] |

## 6. Menus

### 6.1 Pause menu (object 880)

- **Start** opens it during play when no other menu is open [R]; the game pauses (option 1124 = 1) [D value].
- Entries: a table of seven 32-byte records at `0x2C00E8` [D]. The first word of each is a string ID [D]; the rest
  are code addresses and flags [R]. In order: **Keep Playing** (57), **Quit Game** (70), **Exit to Episode Menu**
  (66), **Abandon Job** (107), **Options** (44), **Controls...** (67), **Job Help** (106) [D].
- Shown only when it applies [R]: Exit to Episode Menu and Job Help only inside an episode (Job Help also needs the
  job-help screen 1248 in the world); Abandon Job only while a job runs; Controls only for the three characters.
- The menu is a wheel: Up and Down step with repeat, Cross selects, Triangle goes back [R].

### 6.2 Options

Table of eleven records at `0x2BFEC8` [D]: **New Game...** (60), **Load Game...** (61), **Save Game...** (62),
**Speakers: Mono** (76; cycles through its values), **Music Volume** (103), **Effects Volume** (104),
**USB Headset (for PlayStation®2): Off** (80), **Invert Binocucom Controls: Off** (81), **Invert Turret Controls:
Off** (83), **Center Screen...** (63), **Vibrations: On** (72) [D]. Volumes are sliders moved with Left and Right [R].
The three save-slot pages (new, load, save) use tables at `0x2BFDA8`, `0x2BFE08`, `0x2BFE68`, three slots each,
"Empty" (69) when unused [D]; saving is in saving.md.

### 6.3 Shop, gadget grids, job help

- Shop: object 888; see [safehouse-and-shop.md](safehouse-and-shop.md).
- Gadget grids: objects 885 (Sly), 886 (Bentley), 887 (Murray) [I from order; D strings]; opened with **Select**
  during play [R] (safehouse-and-shop.md). Shown with label 792 (section 2).
  - **Cells.** Each cell of the executable's grid tables (safehouse-and-shop.md 6; 32 bytes: item, three label IDs,
    −1, `f32` x, `f32` y, flags) names three labels of one per-cell sprite in the screen [D]. In table order they
    are the **open** (item on sale, not owned), **owned** and **selected** looks [R for the roles]: Sly's first cell
    holds 978, 977, 979, and the screen's sprite 25 carries the labels 977 (0.3167 s), 978 (0.3333 s), 979
    (0.3333 s) with the markers between them [D]. The grid plays one of the three on each cell: selected for the
    selected item, otherwise owned, otherwise open, otherwise none [R].
  - **Where cells are drawn.** The root's label 792 places each cell sprite with a matrix of scale 0.8; Sly's grid
    rows sit at y 141, 221, 301 and columns at x 183 to 583 in steps of 80; Bentley's and Murray's at x 213, 323,
    433, 543 [D]. The executable's cell positions are the same layout shifted by (−38, +14): Sly's first cell is
    (145, 155) in the table and (183, 141) on screen; this holds for every cell except Bentley's ninth (Long Toss,
    labels 1042–1044), which the table puts at (175, 315) and the screen at (373, 301) [D]. The table positions
    are used only to choose the next cell with the stick, which depends on the cells' relative positions [R], so
    the shift has no effect; draw the cells where the screen places them [I].
- Job help: object 1248, from the pause menu [R].

### 6.4 Title

The title is a screen of the boot world's `jb_intro_macro` package (object 1490) [D for the object; R for the
role]. New Game, Load Game and Options come from the options table above [R].

## 7. Test vectors [D]

| Item | Value |
|---|---|
| `swf_ui` screen objects per world | 30 in each of the 43 worlds |
| Gauge object 875, row 3 | body at decompressed offset 332,790, 18,502 bytes; options `63 04 03 00 00 00 62 04 00 ff ff` (priority 3); 46 labels |
| Prompt object 881, row 3 | body at 317,259, 3,249 bytes; options `63 04 06 00 00 00 62 04 00 ff ff`; labels `0x30F` (0.6 s), `0x310` (0.2167 s), `0x311` (0.3 s), `0x312` (0.3 s) |
| Counters 877–879 | options `63 04 04 00 00 00 65 04 00 00 a0 40 62 04 00 ff ff` (priority 4, idle limit 5.0) |
| Streamed menu screens | options `63 04 12 00 00 00 64 04 01 62 04 00 ff ff` (priority 18, pauses) |
| Streamed file, object 880, row 0 | sector `0x53420`, 46,443 bytes compressed; 116,025 decompressed; SHA-256 `f7c309ac745d76d579a5a6291121ba8bd939c394cc5756bd533f3750ae0f8d6a`; begins `5a 52 42 2e 06 00 a2 00` (162 items) |
| Streamed file, object 888, row 0 | 12,479 → 32,188 bytes, SHA-256 `274448837197e4b1d4f686e0f5662044b060af16c80f01d9e8449229e3c7fa06` |
| Streamed file, object 1248, row 0 | 11,878 → 34,218 bytes, SHA-256 `851c91d55d0500587c03054b2f85094f08a2863cadbfba16281ab56d37ff7188` |
| Gauge smoothing at `0x2C3248` | 100.0, 0.0, 0.5 |
| Pause entry string IDs at `0x2C00E8` (stride 32) | 57, 70, 66, 107, 44, 67, 106 |
| Options entry string IDs at `0x2BFEC8` (stride 32) | 60, 61, 62, 76, 103, 104, 80, 81, 83, 63, 72 |

## 8. Open questions

- The meaning of option 1122 (always 0), option 1143 and the 0/1 word after the canvas size.
- The two special gauge states (which character states select them).
- The roles of classes 127, 128 and 129 in the `_macro` packages, and of class 131 in the menu world.
- The text item's other fields (two words and three floats) beyond the string ID.
- Which screen plays the episode title cards and the job-complete screens (missions.md names the job-complete
  screen object per job).
- How the game decides between a string's table text and a game-filled value for IDs 751 to 753 (section 4.1).
- All [R] behaviour needs checking in play.
