---
id: sly1-save-format
title: Sly 1 saved state, save triggers and restore rules
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a;
      executable SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: save-related strings and the slot description format read from the executable
  - kind: observation
    what: the game as played
    used_for: when the game saves, what a loaded game restores
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files game.h, game, save, prompt, transition, coin, pzo, credit, mb, ub, ruby, clkwork, hubblot, hubsel, Input
    used_for: the game-state record's fields, the auto-save triggers, the settings bits; [R]
date: 2026-11-18
---

# Saved state, save triggers and restore rules

This builds on pickups-and-progress.md sections 6 and 7, which give the memory-card files and the logical content of
a slot. SlyNX stores saves in its own format; this file says what must persist and how it is used.

Tags: [D] disc data, [X] executable data at the given address (hash check as in disc.md), [O] observable, [R]
reference behaviour, [I] inferred.

## 1. State that must persist (one slot)

[R] unless marked.

| Scope | Item | Range / type |
|---|---|---|
| slot | play time | seconds (float) |
| slot | current episode and level (where Load resumes) | episode 0 to 5, level index |
| slot | lives | −1 (game over) to 99; a new game starts with 5 |
| slot | charms | 0 to 2 |
| slot | coins | 0 to 99 |
| slot | settings flags | bit 1: the episode-unlocked voice line has played; bit 5: vibration off; bit 6: mono sound; bit 7: music off; bit 8: the auto-save notice has been acknowledged (auto-save enabled); bit 9: binocucom up = look up; bit 10: turrets inverted; bit 11: jet pack inverted |
| slot | power-up reward mask | one bit per power-up won from vaults (pickups-and-progress.md 4.2) |
| slot | unlocked movies | bit mask |
| slot | milestone flags | first bottle, half the bottles, all bottles, first vault, second vault (bits 0 to 4) |
| slot | last power-up selected | index, or none |
| per episode (6) | keys, vaults, sprints done; time spent; flags (visited, finished) | counts, seconds, bits |
| per level (9 per episode) | goal flags: visited, key, vault, sprint, boss beaten | bits 0 to 4 |
| per level | time spent; best sprint time | seconds |
| per level | adaptive difficulty value and its raw value | 0 to 1 each |
| per level | 12 "dialogue already played" flags | booleans |
| per level | 4 key/value pairs for the level's scripts | integers |
| per level | 64 clue-bottle collected bits; 8 checkpoint-reached bits | bit arrays |

Placed pickups already collected stay collected (pickups-and-progress.md 2); this follows from the bottle bits and
from per-level "collected" state kept with the checkpoints. *The exact store for collected coins, charms and keys
across a reload is to be confirmed (open question 1).*

## 2. When the game saves

- **Auto-save** (only after the player has acknowledged the auto-save notice, settings bit 8, and only when a slot is
  in use) [R] [O]:
  - collecting a level's key;
  - opening a vault;
  - finishing a Master Thief Sprint (first completion or a new best time);
  - defeating an episode boss (each of the five) and the final boss;
  - in the hideout, after the first progress dialogue;
  - on loading a different world or level than the one saved.
- **Manual**: "Save Game..." in the options menu, or "Save and Quit" in the pause menu. [X] [R]
- While saving, the auto-save icon shows (hud.md 6); the player must not switch off (a text the port may drop). [X]

## 3. Restoring a slot

[R] [O]

- **Load** puts the player at the start of the saved episode/level (its hub or approach level for the episodes), with
  the saved lives, charms and coins.
- Goal flags decide what each level contains on entry: a level whose key is collected loads without its key; opened
  vaults stay open; collected clue bottles are not placed again (their bits).
- Checkpoint bits decide where a restart after losing a life puts the player inside a level; on a fresh load the player
  starts at the level's entry.
- Settings bits are applied at once (vibration, mono, music, inverts).
- **Game over** (lives below 0) offers "Try again?" and restarts the level with the starting lives; progress already
  saved is kept.
- **New game** resets everything to the new-game values (prologue, 5 lives, no rewards, no power-up selected).

## 4. Slot list

Three slots. Each shows "Empty" or `%d%% - %s - %d:%02d`: completion percent (pickups-and-progress.md 6), the episode
or world name, and play time as hours:minutes [X `0x24C418`] [R]. Overwriting and erasing ask for confirmation
(ui-and-menus.md 4).

## 5. SlyNX save file (guidance for the implementation)

- One file per slot, holding exactly the state of section 1, with a format version; values outside the ranges of
  section 1 are clamped on load.
- A separate settings file for the port's own options (display, controls) so they survive slot changes; the original's
  settings bits (vibration, sound, music, inverts) stay in the slot to match the original's behaviour.
- Write atomically (write a new file, then replace), so a crash cannot leave a half-written slot.

## 6. Optional: importing an original memory-card save

The original's slot files are `save1` to `save3` in the directory `BASCUS-97198YAOTWTD!`, 6,656 bytes each
(pickups-and-progress.md 7.1). Each holds the record of section 1 in this order: version (18), record size, a check
word, play time; six episode records (each nine level records, then keys, vaults, sprints, time, flags); then current
episode and level, lives, charms, coins, settings flags, reward mask, unlocked movies, milestone flags, last power-up.
[R] A level record is: flags, time, best sprint time, two difficulty values, 12 dialogue flags, 4 key/value pairs,
64 bottle bits, 8 checkpoint bits. Field sizes and the check word must be confirmed from a card written by the game
before an importer is built. [I]

**Header and check word** [X]. The routine that writes a slot (`0x0018CFC0`) and the one that reads it back
(`0x0018CE90` onward) fix the record's first three words:

| Offset | Value |
|---:|---|
| 0 | version, **18** |
| 4 | record size, **6,656** |
| 8 | **check word**: the sum of the 6,644 bytes from offset 12 to the end, each read as an unsigned byte, added into a 32-bit integer (`0x00160650`) |

On loading, the slot is refused with a different error for each failure: a read error; a wrong file size (error 8);
version ≠ 18 or size ≠ 6,656 (error 28); check word ≠ the recomputed sum (error 27). So the check word is a real
checksum, a plain byte sum. A new record (`0x001605E8`) is zero-filled, then gets the version, the size and a few
starting values (offset 6,620 = 3, 6,624 = 5, 6,652 = −1, 6,616 = 6,640 = 0). *Interpretation:* 5 the starting
lives, 3 a starting count and −1 "no power-up selected" [I].

## 7. Open questions

1. Where collected placed pickups (coins, charms, life tokens) are remembered between sessions, and whether a reload
   restores them.
2. (Settled: the check word is a byte sum, section 6.)
3. The exact level a loaded game resumes in for each episode.
