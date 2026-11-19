---
id: sly3-behaviour-dialogue-and-voice
title: Sly 3 dialogue lines, subtitles and talk effects - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      the dialogue commands of the command scripts; the world string tables (subtitle text by line number); the
      line-list order rule read from the code at 0x22B398 and the speaker colours read from the code at 0x27EB50 [X]
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/scptrun3.cpp, game/scptgame3.cpp, sound.cpp,
      soundglue.cpp
    used_for: >-
      the argument order of commands 0x52, 0x53, 0x55, 0x56; what starting a line does (subtitle, quiet flag, source,
      fall-off, volume); marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 dialogue and voice

Read [../sly2/dialogue-and-voice.md](../sly2/dialogue-and-voice.md) first. In Sly 3 lines are started by the command
scripts (class 135, [scripts.md](scripts.md) 7) rather than Sly 2's sequence objects. Tags as in
[guards.md](guards.md).

## 1. Command 0x52: play a line

Arguments, in order [R] (checked against the 12,729 uses on the disc [D]):

| # | Type | Meaning |
|---:|---|---|
| 1 | `s8` n | number of lines in the list (1 to 20) |
| 2 … n+1 | `s32` | the **line list**: dialogue line numbers |
| n+2 | `s8` | **quiet** flag |
| n+3 | object ID | **sound source** (−1: none) |
| n+4, n+5 | `s32` | fall-off start and end distance |
| n+6 | `f32` | volume |

The command's own object ID (when not −1) names the speaking character (the runner's target character) [R]. With the
wait flag the runner stays on the command until the line has finished (the "line playing" test of the first entry)
[R].

### 1.1 Which entry of the list plays [X]

The game keeps, with each command, the list in a running order and a count of lines played since the last shuffle.
Each time the command runs, before the line starts [X: the routine at `0x22B398`, called by the 0x52 handler]:

- a list of one line always plays that line;
- otherwise, while the count is below n, the list is **rotated by one** (the first entry moves to the end) and the
  count goes up by one;
- when the count reaches n, the list is **shuffled at random** and the count is reset to 0. The shuffle visits the
  positions i = 0, 1, …, n − 1 in order and swaps position i with a random position j in **0 … n − 1 − i**,
  inclusive (`0x0011E8F0(lo, hi)` returns lo + r mod (hi − lo + 1)). That is not a uniform shuffle, and j can lie
  before, at or after i. Then, if the line that played last (the first entry before the shuffle) is at the front,
  it is swapped with the last entry, so the **same line never plays twice in a row**;
- the line that plays is the list's **first entry** after that step.

So a fresh command (count 0) plays its second entry first, then the third, and so on round the list, and after a full
round the order is reshuffled. A two-line list therefore always alternates: rotation alternates, and after a
shuffle the no-repeat swap puts the other line first. The order and count belong to the command and last while the world is loaded [I for
the lifetime]. 712 commands on the disc have lists of 2 to 20 lines [D, from the implementation's count].

### 1.2 Subtitle

Starting a line ends the line already playing, then [R]:

- unless the **quiet** flag is set, the subtitle shows the world string whose **ID equals the line number** (for
  example line 1587: "There are some wires crossing the canal, …" in the Venice worlds' string table [D]), typed out
  over the line's length as in Sly 2; the quiet flag starts the line **without a subtitle** (and without the
  "a line started" notice that other systems hear);
- the subtitle's colour is set by the **class of the line's speaker** [X: `0x27EB50`]:

  | Speaker class | R, G, B, A |
  |---|---|
  | 52 Sly | 51, 102, 153, 127 |
  | 53 Murray | 153, 51, 102, 127 |
  | 54 Bentley | 51, 153, 51, 127 |
  | 60 Carmelita | 153, 51, 51, 127 |
  | anything else | 102, 102, 102, 127 |

  (alpha on the 0x80 = 1.0 scale). The speaker of a line comes from the line's entry in the sound data, as in Sly 2
  [R].

### 1.3 Source, fall-off and volume

- **Source** (argument n+3): when it names an object, the line is played as a positional sound from that object's
  position, updated as it moves; with −1 it is not positional (played at full level, centred) [R].
- **Fall-off** (n+4, n+5): used only when the end distance is above 0. A start at or beyond the end is treated as
  end = start + 1. Within the start distance the line is at full level; beyond the end it is silent; between, the
  level falls as for any positional sound ([../sly2/audio.md](../sly2/audio.md)) [R]. On the disc the commonest pair
  is 300 to 3,000 [D].
- **Volume** (n+6) multiplies the line's level (1.0 on most) [R].

## 2. Command 0x53: stop a line

Arguments: `s8` n, then n `s32` line numbers [R]. The playing line is stopped **if it is one of the listed lines**; with
an empty list (n = 0) whatever line is playing stops [R]. 49 uses [D].

## 3. Command 0x54: clear the subtitle [R]

Removes the subtitle without stopping the line (2,011 uses [D]).

## 4. Commands 0x55 and 0x56: hold and release talk effects

Both act on the runner's **target character** (it must be a playable-character-family object) [R]:

- **0x55** holds that character's talk effects: arguments (`s16` kind, `s8` flag). A runner holds at most 4
  characters, and the same character only once; holding asks the character to keep the effect of the given kind
  (with the flag passed on) while the line plays [R].
- **0x56** releases it, with the same two arguments [R]. A runner that ends releases whatever it still holds [R].

*Interpretation:* the kind selects the talking animation (mouth or head movement) and the flag whether it also
faces the listener [I]. 4,222 and 3,998 uses [D]; the argument values are open.

## 5. Other line commands

`0x5F` waits until no line plays (1,169 uses); conditions 1894 and 1895 test for a playing line ([scripts.md](scripts.md)
7.1) [R]. Binocucom talk (commands `0x46` to `0x4D`) follows Sly 2 section 6 [R].

## 6. Open questions

1. The meaning of the 0x55 / 0x56 kind and flag values.
2. Whether the order and count of 1.1 survive a checkpoint reload.
3. The fall-off curve between start and end (shared with the audio spec).
