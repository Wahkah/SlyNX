---
id: sly1-ui-and-menus
title: Sly 1 title screen, menus, hideout and map screens
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      worlds; executable SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      the menu, prompt and response text tables and other strings read from the executable at the addresses given;
      the font tables of all 46 worlds (glyph sets, sizes, bitmap and palette hashes, decoded pictures); the prompt
      table's layout flags; the movie-title pointer table
  - kind: observation
    what: the game as played
    used_for: screen flow, what each menu shows
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files ui, prompt, resp.h, font, title, hubsel, hubblot, wm, Input, blot, fmv
    used_for: rules that build the pause and options menus, navigation repeat timing, the text markup rules; [R]
date: 2026-11-14
---

# Title screen, menus, hideout and map screens

Tags: [D] disc data, [X] read from the executable at the given virtual address (file offset = address − 0x100000 +
0x1000; before reading any table by address the port checks the executable's SHA-256 against disc.md and refuses on
a mismatch), [O] observable, [R] reference behaviour, [I] inferred.

## 1. Text and fonts

### 1.1 Fonts

Each world's material tables hold its fonts (textures.md 2.6, which also gives the record and glyph layout). Fonts 0
to 3 are in every world; a fifth font (font 4) is in **13 worlds**, not only the hideout. [D]

| Font | Bitmap | Cell width, space width, line height (px) | Scale | Glyphs | Use |
|---:|---|---|---:|---|---|
| 0 | 512 × 512, 4-bit | 25, 10, 41 | 1.0 | 133: printable ASCII and accented letters | all running text |
| 1 | 256 × 128, 8-bit | 32, 32, 32 | 2.0 | 19: `! ( ) + < > L O R S T X ^ _ a b l r v` | controller button pictures |
| 2 | 512 × 128, 8-bit | 39, 21, 62 | 1.0 | 24: `% , - . / 0-9 : = ? B H L T h i` | large counter numerals and icons |
| 3 | 512 × 64, 8-bit | 39, 21, 62 | 1.0 | 7: `! C K k l v w` | counter icons (coin, key, vault) |
| 4 | varies (below) | 39, 21, 62 | 1.0 or 2.0 | varies | hideout pictures, minigame counters |

Fonts 0, 1 and 2 are identical in all 46 worlds (same glyphs, same bitmap and palette bytes). [D]

**Font 3 has six pictures**, one per part of the game, with the same seven glyph codes and positions [D]: one for
the splash, Paris and the hideout; one for each episode's worlds (worlds 3 to 11, 12 to 20, 21 to 29, 30 to 38, 39
to 45). What changes is the art: the coin picture `C` shows the episode's emblem; the key and vault pictures are
alike. The port takes font 3 from the world being played, never from another world.

**Font 4** [D]:

| Worlds | Bitmap | Scale | Glyphs | Pictures |
|---|---|---:|---|---|
| 2 The Hideout | 512 × 256 | 1.0 | 21: `_`, `a` to `t` | hideout screen pictures |
| 11, 20, 29, 45 (boss fights) | 512 × 128 | 1.0 | `r s t u v z` (20 has no `v`) | `r`…`v` the boss's face in five states (seen in world 11; the others by their letters [I]), `z` the frame of the health bar (356 wide) |
| 38 Flame Fu! | 512 × 128 | 1.0 | `r s t u v w z` | as above, one more state |
| 16, 37 (races) | 512 × 64 | 1.0 | `F N V` | `F` lap flag, `N` boost bottle, `V` the van |
| 25, 40 | 512 × 64 | 1.0 | `F N` / `N V` | the same letters, presumably the same pictures [I] |
| 9, 28 | 512 × 64 | 1.0 | `V` / `N` | one picture each, by its letter presumably the van and the boost bottle [I] |
| 42 Bentley Comes Through | 256 × 64 | 2.0 | `b d m s` | `d` a bomb-timer sign, `s` Bentley, `b` and `m` the full and empty parts of a bar |

**Picture meanings** (from the decoded bitmaps; the pictures are art, the letters are fixed by the data) [D]:

- **Font 1**: `X` Cross, `O` Circle, `S` Square, `T` Triangle, `(` left stick, `)` right stick, `L` L1, `R` R1,
  `l` L2, `r` R2, `<` `>` `^` `v` the direction pad's left, right, up and down arms, `+` the whole direction pad,
  `_` Select, `!` Start, `a` Sly's mask (blue: the save icon), `b` the mask in orange.
- **Font 2**: digits and `% , - . / : =` as text (`-` is a small ring and `=` a soft dark spot, used as decoration);
  `?` a clue bottle, `L` Sly's head (lives), `H` an hourglass, `h` the hourglass without its backdrop, `i` its
  outline, `T` a stopwatch, `B` Bentley.
- **Font 3**: `C` coin, `K` key, `k` key without backdrop, `l` key outline, `!` a vault bursting open, `v` the
  vault without burst, `w` the vault outline.

### 1.2 Text markup

Strings mix text and markup [R]:

| Sequence | Effect |
|---|---|
| `&` digit | switch to a font: `&0` font 0, `&2` font 1 (buttons), `&1` font 2 (numerals), `&3` font 3 (icons), `&4` font 4; the switched font is scaled to the current line height |
| `&.` | back to the base font and colour |
| `^` d d | multiply the text scale by d.d (for example `^15` = 1.5) until the matching `^.` |
| `~` RRGGBB | colour (hex); halved to the console's 0 to 128 colour range, so `~ffffff` is full white |
| `~.` | back to the base colour |
| `&&`, `^^`, `~~` | the literal character |
| `\n` | new line |

A malformed sequence prints its first character and resets font and colour. Example: `&2X&. : New Game` shows the
Cross button picture followed by ": New Game".

## 2. Text tables in the executable

| What | Address | Layout |
|---|---|---|
| Prompt table | `0x269D50` | 33 records of 24 bytes: `u32` address of the title text, `f32` title text scale, `f32` response text scale, `s32` **layout** (1: responses stacked vertically, one per line, chosen with up/down; 0: responses side by side on one line, chosen with left/right), `s32` response count, `u32` address of the list of response IDs (`s32` each) [X] [R for the layout meaning] |
| Response text table | `0x26A068` | 27 `u32` text addresses, one per response ID 0 to 26; IDs whose text depends on a setting point to buffers the game fills at run time [X] |
| Two-state response texts | `0x26A0D8` onward | pairs of `u32` text addresses: quit (New Game / Empty … "Quit Game", "Save and Quit"), vibration, music, speakers, the three invert options [X] |
| Movie titles | pointer table `0x2699C0`: 16 `u32` addresses, in the order of movie table entries 1 to 16 (movies.md); the strings lie at `0x24B438` to `0x24B578` (the last, "Credits", starts at `0x24B578`) | null-terminated strings [X] |
| Level names | world table, disc.md 4.2 | [X] |

Prompts (index: title, responses) [X]: 0 "Paused" (8 responses, built at run time, section 4); 1 "Game Over. Try
again?" (Yes, No); 2 "Really Quit?" (Yes, No); 3 to 10 memory-card messages; 11 "Save Game", 16 "New Game", 17 "Load
Game" (slot 1, slot 2, slot 3, Back); 12, 19 saving and erasing notices; 13 "Game Saved" (Continue); 14 "Do you want
to overwrite this game?", 18 "Do you want to erase this game?" (Yes, No); 20 the auto-save notice (Continue); 21 card
removed; 22 disc removed; 23 "Options" (9 responses, built at run time); 24 the centre-screen help text; 25, 26 "Try
again?"; 27 to 30 Master Thief Sprint results; 31 "Controls" (three invert options, Back); 32 insert-card notice.

Response IDs [X]: 0 Yes, 1 No, 2 Continue, 3 Return to Game, 4 View Map, 5 Options..., 6 New Game..., 7 Load Game...,
8 Save Game..., 9 Quit (two states), 10 to 12 slots, 13 Vibrations (two states), 14 Center Screen..., 15 Music (two
states), 16 Speakers (two states), 17 Back, 18 Restart Race, 19 Restart Sprint, 20 Restart Level, 21 Exit Level, 22
Exit to Hideout, 23 Controls..., 24 to 26 invert options (two states each).

Layout flag [X]: 21 prompts have 1 (0, 3, 5, 6, 8 to 13, 15 to 17, 19 to 24, 31, 32: the menus, slot lists and
notices); the 12 with 0 are the yes/no questions (1, 2, 4, 7, 14, 18, 25, 26, 28 to 30) and the sprint result 27.
Yes/no answers therefore sit side by side.

**Movie titles** [X]: the 16 strings are not interleaved name lists; they are the movie menu's titles in movie
order: four bonus movies (The Secret Police File, The Tokyo Police File, The Making Of Sly Cooper, Commercials &
Outtakes), then per episode its introduction and its boss-defeat movie (Tide of Terror / Raleigh Hung out to Dry,
Sunset Snake Eyes / Muggshot Busted, Vicious Voodoo / Mz Ruby Vexed, Fire in the Sky / The Panda King on Ice, The
Cold Heart of Hate / Ten Seconds), then The Thievius Raccoonus and Credits. The introduction titles are also the
episode names, which is why the hideout can show entries 5, 7, 9, 11 and 13 (pointer-table slots 4, 6, 8, 10, 12)
as episode names.

Slot description format: `%d%% - %s - %d:%02d` (completion percent, the world being played, play time in hours and
minutes) or "Empty" [X] [R for the fields].

## 3. Screen flow

[O] [R]

1. **Start-up**: the logo and legal screens, then the attract loop (the title world with "Sly Cooper / and the
   Thievius Raccoonus" and "Press START button for New Game" or, with a save loaded, "Press START button to Resume
   Game"; "Press SELECT button for Menu" opens the options menu). Idle on the title plays the attract movie.
2. **New game** starts the prologue world; **Load game** resumes the saved world and level (save-format.md).
3. **Playing**: Start opens the pause menu (the game clock eases to 0, clock-and-loop.md 3).
4. **Hideout**: between levels the player is in the hideout world with the episode select (section 5).
5. **Map screen**: "View Map" from the pause menu, and the level select inside an episode (section 6).
6. **Wipes**: every world change goes through a screen wipe (level-flow.md).

The game's user-interface states: splash, attract, hideout, playing, pausing, paused, unpausing, map, wiping. [R]

## 4. Pause and options menus

**Pause menu** (title "Paused"), entries in this order, each only when its rule holds [R]:

| Entry | Shown when |
|---|---|
| Return to Game | always |
| Restart Race | in the two race levels, during the race |
| Restart Sprint | a Master Thief Sprint is running |
| Restart Level | in three levels only: Piranha Lake, Down Home Cooking, Burning Rubber |
| View Map | the level has a map |
| Options... | always |
| Exit Level, or Exit to Hideout | not in the prologue; "Exit to Hideout" in the final episode, in the hideout itself, and in an episode's approach level before it has been completed |
| Quit Game / Save and Quit | always ("Save and Quit" when a save slot is in use) |

**Options menu** (title "Options"): New Game... (only from the title screen), Load Game..., Save Game... (not from the
title screen), Vibrations On/Off, Center Screen..., Music On/Off, Speakers Stereo/Mono, Controls..., Back. [R] [X for
the texts; the exact retail order is built at run time and should be confirmed by observation]

**Controls**: Binoc-u-com "Up = Look Down/Up", Turrets "Up = Aim Down/Up", Jet Pack "Up = Fly Down/Up", Back. [X]

**Center Screen**: moves the picture with the direction pad; Triangle cancels, Cross accepts. [X] The port may omit
or replace it.

**Save, load, new**: a list of the three slots and Back; Cross on a slot saves, loads or starts; on an occupied slot
"Do you want to overwrite this game?"; Square erases ("Do you want to erase this game?"). [X] [R]

## 5. Hideout episode select (class 105)

[R] [O]

- Left/right (or up/down) moves between episodes; it stops at the first and at the last unlocked episode, and wraps
  around only once the final episode is unlocked.
- Cross on an episode already visited enters it (its map screen); on the newest, not yet visited episode it plays
  its introduction, with the hint text "Press &2X&. to Start Mission!" [X `0x24A7B8`].
- A short voice hint plays for the selected episode; the first time a new episode is unlocked a fixed voice line
  plays once (setting flag 2, save-format.md).
- The totals panel shows the selected episode's progress (keys, vaults, sprints); on the prologue it shows the name
  "Bentley" [X `0x24A7A8`], on an unvisited episode "Murray" [X `0x24A7B0`] (the character who briefs it).

## 6. Map screen (class 103)

[R] [O] The map shows the episode's levels as positions on a picture; the cursor moves between them with the stick or
direction pad; locked levels cannot be selected; each level shows its completed goals (key, vault, sprint) and the
episode's totals; Cross enters the selected level with a wipe. The detailed layout comes from the map object's data
in the hideout and the episode hubs.

## 7. Navigation rules

- Move: direction pad, or the left stick pushed past 0.8 of its range. Holding repeats: the first repeat after
  0.3333 s, each next interval two thirds of the previous, never below 0.1 s; holding the stick past 0.25 of its
  range keeps the repeat going. [R]
- Cross accepts, Triangle backs out where a menu allows it, Start closes the pause menu. [O] [X for the help text]
- Menus slide in and out (0.25 s each by default, hud.md 2). [R]
- Sounds: a cursor move and an accept/back sound play (sound IDs to be taken from the effects bank; open question 2).

## 8. Open questions

1. (Settled: section 1.1 gives every picture of fonts 1 to 3.)
2. Menu sound IDs.
3. The map screen's data layout (class 103 options and its picture objects).
4. The retail order of the options menu (built at run time).
