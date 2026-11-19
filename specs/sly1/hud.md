---
id: sly1-hud
title: Sly 1 on-screen display (HUD)
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a;
      executable SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: the placement table and the display strings read from the executable at the addresses given; the fonts
  - kind: observation
    what: the game as played
    used_for: which counters appear when, the binocucom view
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files blot, ctr, timer, binoc, prompt, save, gl
    used_for: timings, counting speed, text formats, placement rules; [R]
date: 2026-11-14
---

# On-screen display (HUD)

Tags: [D] disc data, [X] read from the executable at the given virtual address (file offset = address − 0x100000 +
0x1000; the port checks the executable's SHA-256 in disc.md first and refuses on a mismatch), [O] observable, [R]
reference behaviour, [I] inferred. Screen positions are in pixels of the original frame (640 wide), y measured
**down from the top**; the port scales them to its display.

## 1. Display elements

The display is a set of elements (37 kinds, numbered 0 to 36), each a box that slides on and off screen. Element
numbers used below [R]: 0 letterbox, 1 binocucom, 3 lives, 4 clue bottles, 5 keys, 6 coins, 7 gold/sprint prize,
8 to 15 minigame counters (charge, lap, boost, place, security code, vehicle health, boss, crusher), 16 timer,
17 note (sprint text), 18 credits note, 19 title, 20 totals, 21 call, 22 map, 23 hideout panel, 24 movie menu, 25
prompt (menus), 26, 27 television panels (dialogue picture panels), 28 van computer, 29 scores, 30 logo, 31 attract,
32 credits, 33, 34 boss and player icons, 35 auto-save icon, 36 percent.

## 2. Placement and motion

**Placement table** [X `0x26C3E0`]: 37 records of 20 bytes, one per element: `f32` x hint, `f32` y hint, `s32` edge it
slides in from (0 left, 1 right, 2 top, 3 bottom, −1 none), `s32` element it is placed against (−1 none), `s32` edge
of that element it is placed against.

- x hint > 0: the box's left side is that far from the left edge; < 0: its right side is that far from the right
  edge; 0: centred. y hint > 0: its top is that far from the top; < 0: its bottom that far from the bottom; 0:
  centred. [R]
- **Placed against another element** (the last two fields): when that element is showing, the area the element is
  placed in shrinks to the far side of it. The edge field says **on which side of the element being placed the other
  element lies** [R]: 0 left → the area starts at the other's right side (the element sits to
  its right); 1 right → it ends at the other's left side (sits to its left); 2 top → it starts at the other's
  bottom (sits **below** it); 3 bottom → it ends at the other's top (sits **above** it). So the keys (5, against
  4 with edge 2) sit under the clue counter, and the note (17, against the timer 16 with edge 3) sits **above** the
  timer; both are anchored to their screen edge (y 20 from the top for the keys, y −20 from the bottom for the
  note), so they stack away from it. [X for the table] [R]
- "Showing" means appearing or visible; while the element being placed is itself disappearing, a disappearing
  other element still counts. If the other element is not showing, the element tries that element's own "placed
  against" entry, and so on down the chain (the sprint prize 7 is against the keys 5, which are against the clue
  counter 4). [R]
- Selected records [X]:

| Element | x, y | Slides from | Placed against | Where it ends up |
|---|---|---|---|---|
| 3 lives | 0, 20 | top | — | top centre |
| 4 clue bottles | −18, 20 | right | — | top right |
| 5 keys | −18, 20 | right | clue bottles (edge 2) | under the clue counter |
| 6 coins | 18, 20 | left | — | top left |
| 7 sprint prize | −18, 20 | right | keys (edge 2) | under the keys |
| 16 timer | 18, −20 | left | — | bottom left |
| 17 note | 18, −20 | left | timer (edge 3) | above the timer |
| 19 title | −18, −20 | right | — | bottom right |
| 25 prompt | 0, 0 | — | — | centred |
| 26, 27 picture panels | 1, −20 / −1, −20 | left / right | — | bottom left / bottom right |
| 35 auto-save icon | 18, −20 | bottom | — | bottom left |

**Motion** [R]: an element is hidden, appearing, visible or disappearing. Appearing takes 0.25 s and disappearing
0.25 s by default (some elements set their own); the box moves from its off-screen position (fully past the edge it
slides from) to its place along a smoothed path, and back. A visible element hides again after its visible time if
it has one.

## 3. Counters

Counters are drawn in font 2 (ui-and-menus.md 1.1) [R]:

| Counter | Text | Notes |
|---|---|---|
| lives | `L` + count | `L` is the lives picture |
| clue bottles | collected `/` total + `?` (font 2: a bottle) while incomplete, or `&3!&.` (font 3: the vault bursting open) when all are found | hidden when the level has no bottles |
| keys | count + `&3K&.` (font 3) | only in the episodes' worlds |
| coins | `&3C&.` (font 3, the episode's coin) + count | only where pickups are enabled |
| sprint prize, laps, boost, place | `count/total` + a symbol | minigames |
| percent | count + `%` | completion |

[R] [X for the format strings at `0x24CC78` to `0x24CCB8`]

- **Counting**: when the real value changes, the shown number moves toward it at up to 6000 per second of game time
  (so in practice it jumps within a frame or two); when it arrives, a "smack" animation plays. [R]
- **Visibility**: a counter appears when its value changes and stays visible 2.5 s while the game clock runs, then
  slides away. [R] All counters show while the game is paused. [O]
- Charms: the number of charms is shown by the player character (a charm picture appears briefly when one is gained or
  lost), not by a counter; after a hit the player flashes for 2 s (player-moves.md 1). [O] [R]

## 4. Timer

Shown during a Master Thief Sprint and timed minigames: `m:ss` in colour `#b2a83d` at scale 1.0, or tenths of a second
as `0.%d` below one second (format strings `^10~b2a83d%d:%02d~.^.` and `^10~b2a83d0.%d~.^.` [X `0x24CDE0`,
`0x24CDC8`]). [R]

## 5. Prompts

A prompt (menu or message, ui-and-menus.md 2) is a centred box: title text at the prompt's title scale, responses below
at its response scale, the current response highlighted; it slides in and out like the other elements. [R]

## 6. Auto-save icon

While the game writes its save, the auto-save element shows the save picture (`^15&2a&.^.`: font 1 glyph `a` at 1.5
times scale) at the bottom left; the player is told once what it means (prompt 20). [X] [R]

## 7. Binocucom view

The look-around view's overlay [R] [O]:

- a dark vignette background and a binocular-shaped outline;
- a centre reticle that follows the look direction;
- a compass strip and a zoom readout;
- markers on scan points and on objects of interest within 4000 units (up to 32 at once), and a filter mode that
  highlights one kind of object (for example clue bottles), refreshed every 0.25 s;
- the radio text typed out letter by letter with a white `|` cursor.

Positions and art of these pieces are drawn by the game, not taken from world data (except the scan points, class
130, and the objects they mark).

## 8. Damage feedback

- A hit: the player's damage reaction (player-moves.md 7), a controller rumble, the charm or lives change on screen. [O]
- Losing the last life: the "Game Over. Try again?" prompt. [X] [R]

## 9. Open questions

1. Exact pixel sizes of each counter box (from the fonts' glyph widths and the text; to be confirmed against the
   running game).
2. The smoothing curve of the slide motion.
3. Binocucom overlay layout in detail (compass and zoom positions, colours).
4. How charms are shown on screen, if at all, beyond the player's own model.
