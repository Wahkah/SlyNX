---
id: sly2-behaviour-dialogue-and-voice
title: Sly 2 dialogue, subtitles and voice (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316, all 43 world files; executable SCUS_973.16 (SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73)
    used_for: >-
      every class-114 sequence object walked (3,871 bodies, each ending at its declared size) with its option list
      decoded in order; every line command checked against the world's voice-line list and string table; the
      option-table entries of the dialogue commands (argument counts) and the talk-target table read from the
      executable; per-world counts and every test vector below
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/scpt.cpp (sequence commands, waits), sound.cpp (line
      playback, subtitle reveal, speaker colours), soundglue.cpp (talk targets, eyes), binoc.cpp, vault.cpp,
      photo.cpp, spliceglue.cpp, loaders.cpp
    used_for: >-
      which option each sequence command is, what the commands and the line player do, the reveal rule and the
      speaker colours; behaviour marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/dialog
    used_for: confirmation that no dialogue object class with stream data exists in Sly 2 (only an untrigger routine)
  - kind: disc
    what: >-
      2026-11-18: every option value at every depth of all 43 worlds (options 441 and 1070 checked against the
      class-114 objects), every clip's type-4 events, every script module (calls of script IDs 235, 638 and 530),
      option-table entries 441, 613 to 615 and 1070 read from SCUS_973.16 after the hash check
    used_for: section 2.1
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release scpt.cpp, scpt.h, trigger.cpp, interact.cpp, asegevent.cpp, po.cpp,
      vault.cpp; sly2-decomp @ 33e2bab9 symbol map
    used_for: how each hook starts a sequence [R]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/scptai.cpp, ai.cpp, aijobs.cpp (guard command loaders and runner), scpt.cpp (sequence commands)
    used_for: meanings of guard commands 1, 2, 6 and 13 and of the start-sub-sequence command (2026-11-18)
  - kind: executable
    what: SCUS_973.16 (SHA-256 checked), option table at 0x002AC880, entry 636 (routine 0x1070E0, 3 arguments)
    used_for: identifying option 636 as the start-sub-sequence command (2026-11-18)
  - kind: disc
    what: re-walk of all 43 worlds (121,153 bodies), raw argument bytes of options 596, 597, 601, 606 and 636 (2026-11-18)
    used_for: argument types and value counts of those commands
reviewed_by: pending
date: 2026-11-21
---

# Sly 2 dialogue, subtitles and voice

Differences from [../sly1/dialogue-and-voice.md](../sly1/dialogue-and-voice.md). Tags: **[D]** disc data, **[O]**
observable, **[R]** reference behaviour to be confirmed, **[I]** inferred. Classes and options are those of
[object-stream.md](object-stream.md); voice lines and their files are in [audio.md](audio.md) 4.

## 1. What changed from Sly 1

| Item | Sly 1 | Sly 2 |
|---|---|---|
| Where a conversation is stored | dialogue objects (class 101) with an event list | **sequence objects (class 114)**: an option list read as an ordered list of commands (section 3) [D] [R] |
| Speakers | speaker objects (class 102) with colours and options | no speaker objects; a voice line names its speaker by **class ID**, and the speaker is the world's first object of that class [D for the field, R for the lookup] |
| Voice file of a line | a `desc32` inside the talk event | the world's voice-line list, keyed by **line number** (audio.md 4) [D] |
| Subtitle text | inline in the talk event | the world string table, **string ID = line number** (section 4) [D] |
| Lip sync and eyes | talk-event fields | the voice line's timed records (audio.md 4) driving state machines on the speaker [D] [R] |
| Conversation kinds (radio, briefing, confrontation) | options 971/972 | none; a binocucom radio talk is opened by its own command (section 6) [D] [R] |
| Played flags, equivalents in other levels | per dialogue | none in the data; scripts decide when a sequence runs [I] |

There is no Sly 2 class with a dialogue body: the only dialogue routine the decompilation has is one that
"untriggers" a dialogue, and no class's stream layout carries talk events [R] [D].

## 2. Sequence objects (class 114)

A class-114 object's body is an option list only (classes/simple-layouts.md) [D]. Its options are not settings: the
game builds a **command list** from them, one command per option, **in stream order**, when the object loads [R].
The commands cover camera moves, actor moves and animations, waits, pausing, and the dialogue commands of
section 3. Something else starts the sequence (section 2.1); it then runs its commands one after another, each
waiting until done [R].

On the disc: 3,871 sequence objects (3,545 at package level, 326 nested), 50 to 169 per world [D]. 1,054 distinct
line numbers are played by sequences, with 5,262 line commands over all worlds [D].

### 2.1 What starts a sequence

Nothing starts a sequence by itself: a sequence runs only when something starts it [R]. A start names a **starter
object**, which the sequence's commands use as their default target [R]. The ways in, with their counts on the disc:

| Start | How it is written | On the disc [D] |
|---|---|---|
| **Script call: start with an object** (starting a sequence that is already running does nothing: `0x001DFD68`, 2026-11-21 [D]) | option 613 (script ID 235 in the class map), called with one argument, the starter: `((op111 sequence 235) starter)`; with no argument there is no starter | 1,357 calls: 810 with an object literal, 75 with a computed object, 437 without |
| **Script call: start with the current character** | option 614 (script ID 638), no arguments; the starter is the character the player controls now | 2,363 calls |
| **Script call: stop** | option 615 (script ID 530), no arguments; calls the sequence stop `0x001DFEB8`, which stops every run of the sequence and retracts the clips they wait on (2026-11-21, [animation.md](animation.md) 2) [D]. Command 631 in a sequence stops another sequence the same way [R] | 103 calls |
| **Trigger volume** | a class-24 volume whose option 441 names the sequence; started when an object the volume accepts enters it, with that object as the starter [R] | 40 volumes, all naming class-114 objects |
| **Interaction spot** | a class-35 spot whose option 1070 names the sequence; started when a character uses the spot, with that character as the starter [R] | 82 spots, all naming class-114 objects |
| **Animation event** | a timed event of type 4 in a clip (animation.md 2) whose object ID names a sequence; started when the clip crosses the event's frame, with no starter [R] | 168 timed type-4 events name class-114 objects (the other 252 type-4 events, all in retract lists, name clips and play them) |
| **Another sequence** | **option 636**, the start-sub-sequence command: arguments (sequence, starter object or −1, no-wait flag). With −1 the running sequence's current subject is passed on as the starter. Flag 0: the parent waits until the sub-sequence ends, and stopping the parent also stops it; flag 1: the parent goes on at once [R] | 708 uses, all in class-114 lists; starter −1 in 679, an object in 29; flag 0 in 473, 1 in 235 [D] |
| **Safe-house action** | a Cross action of the safe house (option 1338, safehouse-and-shop.md) whose target is a sequence: started with no starter, or with the character chosen [R] | one per character and hub (placement blocks) |
| **Vault** | the vault's state change starts the sequence set for that state, with the vault's actor as starter [R] | (vault options 1226, 1227, 1229; section 5) |

A guard command list can also **signal** a running sequence (class-113 command 15, option 612:
options-by-class.md 2.2): it raises a numbered alarm the sequence can wait for; it does not start it [R]. All 71
such commands on the disc name class-114 objects [D].

A port should therefore start a sequence from these hooks rather than offering a manual start [I]: the script calls
(by far the most), trigger volumes, interaction spots and animation events.

## 3. Dialogue commands

Each command is a stream option with a typed argument list ([object-stream.md](object-stream.md) 1). The option's
40-byte entry in the executable's option table (virtual address `0x002AC880`, entry = 0x2AC880 + 40 × ID) holds, at
offset 8, the address of the routine that appends the command; the argument counts below are the entry's own
(offset 24) and match the data [D]. Argument tags as in object-stream.md 1: tag 16 is a 2-byte integer, tag 6 a
4-byte object ID, tag `0x105D` a 4-byte enumeration, tag 0 a 1-byte flag, tag 1 an `f32`.

| Option | Arguments (tags) | Uses | What it does |
|---:|---|---:|---|
| 695 | line number (16), quiet (`0x105D`), wait flag (0) | 5,262 | **Play a voice line.** Stops the line that is playing, starts this one. Quiet = 1 (662 uses) plays it **without a subtitle**; 0 (4,600) shows the subtitle [D for the values, R for the meaning] |
| 708 | time (1) | 1,054 | **Wait for the line**: done when the sequence's last line has ended; a non-negative time ends the wait after that many seconds of the line instead. Every use on the disc has −1.0 (wait for the end) [D] [R] |
| 692 | button mask (`0x103C`), pad (6) | 26 | **Wait for a button**, showing its prompt in the subtitle box. Masks on the disc: 16 (Triangle, 6 uses), 32 (Circle, 6), 64 (Cross, 6), 128 (Square, 6), 2048 (Start, 2); pad −1 = any [D for the values; button names from the PS2 pad bit order, R that the prompt shows] |
| 696 | none | 429 | **Clear the subtitle** [R] |
| 685 | viewer (6), other speaker (6), window (6), flag (0) | 405 | **Open the binocucom for a radio talk** (section 6) [R] |
| 686 | flag (0) | 405 | **Close the binocucom**; the subtitle is cleared with it [R] |
| 679 | object (6), shader animation (6), amount (1) | 195 | **Close an object's eyes** by the amount (0 open, 1 shut); −1 for the animation means every eye animation of the object and its children. Disc: object 7 (191), object 9 (4), amount 0 in all [D] [R] |
| 697, 698 | object (6), effect kind (`0x1066`), flag (0) | 1,702, 1,443 | **Hold / release** a talk effect on an object (up to 4 held per running sequence); kinds on the disc include 32, 50, 54, 62, 63 [D] [R]; the kinds' meaning is open |

Other options of class 114 (85 distinct option IDs from 622 to 716 occur: camera blocks, moves, animations, pauses, timers, music) are
behaviour of the scripts spec and the camera spec; they are listed here only by their range [D].

## 4. Playing a line and its subtitle

- **One line at a time** [R]. Starting a line stops the previous one: the previous line's remaining timed records
  fire at once, and the speaker's mouth target is set to the closed goal 1,553 [R].
- **Voice.** The line number selects the world's voice-line entry (audio.md 4): its VAG file, headset byte and
  duration [D]. Every one of the 5,262 line commands on the disc finds an entry in its own world's list [D].
- **Speaker.** The entry's speaker field is a class ID (53 Sly, 54 Murray, 55 Bentley, 59 to 65 other characters,
  −1 none; counts in audio.md 4) [D]. The speaker is the world's first object of that class (or of a class that
  extends it) [R]. While the line plays, the timed records drive five targets on the speaker: targets 0, 1, 2, 4, 5
  set goals on the speaker's state machines whose object IDs are, in order, **1548, 1549, 1550, 1551 and 1552**
  (target 5 is the mouth), and target 3 sets how far the speaker's eyes are closed [R]. The six IDs (1548, 1549,
  1550, −1, 1551, 1552) are a table in the executable at virtual address **`0x002C3C40`** [D].
- **Subtitle text.** The text is the world string whose ID equals the line number [D: all 5,262 line commands have
  such a string in their world, and so do all 7,970 voice-list entries (1,691 distinct line numbers)]. Quiet lines show none.
- **Reveal** [R]. The text appears letter by letter over the line's duration minus 0.5 s: after `t` seconds of the
  line, the first `n = ⌊t / (duration − 0.5) × length⌋` characters show, followed by a `|` cursor; once the whole text
  shows, the cursor blinks at 4 Hz (shown when `⌊4 t_game⌋` is odd). The cursor string `|` is at virtual address
  `0x002A97F0` [D]. Sly 1 used a characters-per-second rate instead (../sly1/dialogue-and-voice.md 4).
- **Colour by speaker** [R] (RGBA bytes; alpha `7F`, i.e. fully opaque on the 0x80 scale):

  | Speaker class | Colour (R, G, B) |
  |---|---|
  | 53 (Sly) | `33 66 99` (blue) |
  | 55 (Bentley) | `33 99 33` (green) |
  | 54 (Murray) | `99 33 66` (wine) |
  | 63 | `99 33 33` (red) |
  | 60 | `99 33 99` (magenta) |
  | any other | `66 66 66` (grey) |

  The colour bytes `33`, `66`, `99` and `7F` occur as immediates in the executable's speaker-colour routine
  [D for their presence, R for the mapping].
- **Subtitle box.** Shown while the game is paused by a talk; the button prompt of command 692 appears in it [R].

## 5. Other places that start lines

| Source | What starts the line | Data |
|---|---|---|
| Scripts | object methods with stream option IDs 137 (start a line: line number, quiet), 138 (stop the line) and 139 (wait for the line) on every object | option-table entries 137 to 139 point at those routines (offset 16) [D]; script usage counts not established |
| Vault (class 78) | options 1226 (line when the vault scene starts, subtitled), 1227 (line for the clue-bottle count, quiet), 1229 (line when it opens, quiet) | [R] option meanings |
| Photo jobs | each photo target can name a line, played when its picture is taken | [R] |

The voice lists hold more lines than sequences play (for example 156 entries and 119 line commands in world row 3)
[D]; the rest are played by these other sources [I].

## 6. Binocucom radio talk

Command 685 opens the binocucom view (camera.md 6) in its talk mode: the **viewer** is the character looking
through it, the **other speaker** the one on the radio, and the **window** a screen object (−1 for the default) [R].
Arguments on the disc [D]:

| Viewer | Other | Window | Uses |
|---:|---:|---:|---:|
| 7 | 8 | −1 | 271 |
| 9 | 8 | −1 | 53 |
| 8 | 7 | −1 | 43 |
| 8 | 9 | −1 | 15 |
| 9 | 7 | −1 | 14 |
| 8 | −1 | 2385 | 9 |

Object IDs 7, 8 and 9 are the gang: 7 Sly, 8 Bentley, 9 Murray (the same codes the item table uses for owners,
characters.md 6) [I]. A radio talk is then a run of line commands (section 3), each followed by a wait, and ends with
command 686 [D].

## 7. Triggering, pausing and skipping

- A sequence starts from one of the hooks of section 2.1 (script calls, trigger volumes, interaction spots, animation events, another sequence, safe-house actions, vaults) [D] [R].
- A talk sequence usually pauses the game and the player's control for its length and resumes it at the end
  (a pause command with on/off) [R].
- **Skipping.** Nothing in the data marks a sequence as skippable [D]. Whether the original lets the player skip a
  talk, and with which button, is open. (The reference port adds its own skip; it is not part of the game.)

## 8. Test vectors

Sequences per world (all / with at least one line command) and line commands [D]:

| Row | Sequences | With lines | Line commands | Voice entries |
|---:|---:|---:|---:|---:|
| 0 | 57 | 28 | 59 | 61 |
| 2 | 98 | 44 | 119 | 193 |
| 3 | 82 | 44 | 119 | 156 |
| 12 | 110 | 43 | 129 | 169 |
| 17 | 102 | 49 | 156 | 275 |
| 32 | 169 | 58 | 152 | 262 |
| 38 | 84 | 28 | 89 | 149 |
| All 42 rows with worlds | 3,871 | — | 5,262 | — |

Sequence object 1714 in world row 3 (`f_nightclub_cellar`), package level, body at decompressed offset
`0x722FC5` [D]. Its command options in order: 689, **685** (viewer 7, other 8, window −1), **695** (line 1299),
689, **695** (1300), **695** (1301), 689, **695** (1302), 689, **695** (1303), 660, **686**. All five lines are
subtitled (quiet = 0). In that world:

| Line | Speaker class | Subtitle length | Voice duration (s) | Timed records |
|---:|---:|---:|---:|---:|
| 1299 | 55 | 101 characters | 7.425 | 65 |
| 1300 | 53 | 29 characters ("How am I supposed to do that?") | 1.280 | 19 |
| 1301 | 55 | 124 characters | 8.726 | 70 |

Line 1300's bytes in the sequence: option ID `b7 02` (695), then tag `10 00` value `14 05`, tag `5d 10` value `00 00 00
00`, tag `00 00` value `00` [D]. Its reveal shows 22 of 29 characters after 0.6 s (⌊0.6 / 0.78 × 29⌋) [R rule].

Executable: talk-target table at `0x002C3C40` reads 1548, 1549, 1550, −1, 1551, 1552 [D].

## 9. Open questions

- Whether and how the original lets the player skip a talk.
- The talk-effect kinds of commands 697/698 and the meaning of the "wait" flag on command 695.
- How often scripts use the start/stop/wait line methods (options 137 to 139), and which lines the vault and photo
  sources name.
- Commentary: no developer-commentary table like Sly 1's was looked for; whether Sly 2 has one is open.
- All [R] behaviour needs checking in play.
