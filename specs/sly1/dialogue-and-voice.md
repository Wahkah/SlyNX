---
id: sly1-behaviour-dialogue-and-voice
title: Sly 1 dialogue, subtitles and voice
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files; executable SCUS_971.98 (SHA-256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c)
    used_for: >-
      every dialogue (822) and speaker (97) object walked, including those placed through proxies; event-kind,
      talk-parameter and text-length statistics; option values on dialogues and speakers; every test vector below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/dialog.cpp, dialog.h, speaker.cpp, binoc.cpp (subtitle reveal), sound.cpp (voice streaming), spaprops.cpp (option table entries 966 to 999)
    used_for: >-
      meaning of the event kinds and talk fields, the state rules, the reveal speed, the option meanings and
      defaults; behaviour marked [R]
  - kind: observation
    what: the game as played (general knowledge of its presentation)
    used_for: statements marked [O]
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 dialogue, subtitles and voice

Tags: **[D]** confirmed from the disc data, **[O]** observable by playing, **[R]** reference behaviour to be
confirmed, **[I]** inferred. Class IDs, options and events are those of [object-stream.md](object-stream.md),
[option-encodings.md](option-encodings.md) and [classes/placed-object.md](classes/placed-object.md) 5.2. Voice
files are VAG files ([audio.md](audio.md) 2.2); movies are in [movies.md](movies.md).

## 1. Objects

- **Dialogue** (class 101): a placed object followed by a list of up to 127 events. 822 on the disc, in 45 worlds
  (none in "Splash"), almost all placed through proxies. [D]
- **Speaker** (class 102): a placed object standing for a character who talks: a face shown in an on-screen
  picture panel during radio and briefing dialogues. 97 on the disc. [D] [O]

A dialogue is drawn hidden; it is only a script of events. [R]

## 2. Dialogue kinds

A dialogue has one of three kinds, set by a script option on it: [R]

| Kind | Set by | On the disc | What it is |
|---:|---|---:|---|
| 0 (default) | — | 35 | **Radio**: plays in the binocular-radio overlay; the player keeps control. [O] [R] |
| 1 | option 971 (no value) | 764 | **Briefing / instruction**: tutorial and mission text shown in the overlay. [R] |
| 2 | option 972 (no value) | 23 | **Confrontation**: an in-world conversation; the player's controls are locked, the camera switches to a conversation view, and effects and music drop to 0.6 of their volume if option 978 is set (4 dialogues). [R] [D for the counts] |

## 3. Events

Events run in order. Payloads are in classes/placed-object.md 5.2; their meaning: [R] unless marked

| Kind | Count on disc [D] | Meaning |
|---:|---:|---|
| 0 | 1,055 | **Load voice line**: start loading the VAG file named by the `desc32`. The dialogue waits until the line is loaded and playing. |
| 1 | 338 | **Left picture panel**: show speaker OID (first `s16`) in the left panel, optionally replacing the panel's picture with object OID (second `s16`); −1 = none |
| 2 | 63 | **Right picture panel**: same for the right panel |
| 3 | 1,076 | **Talk**: one subtitle line (3.1) |
| 4 | 111 | **Call script**: run the script label whose OID is the `s16` (scripts.md) |
| 5 | 17 | **Letterbox on**: show the black cinema bars |
| 6 | 15 | **Letterbox off** |
| 7 | 2 | **Play movie**: queue the movie named by the `desc32` ([movies.md](movies.md); both in "The Hideout") |

Every dialogue on the disc has at least one voice line and one talk event. [D] Event counts per dialogue: 2 events
(479 dialogues), 3 (268), and up to 28. [D]

### 3.1 Talk event

| Field | Type | Meaning | On the disc [D] |
|---|---|---|---|
| zoom | `f32` | Binocular zoom for this line; −1 = leave as is. When ≥ 0, the view zooms to `zoom + 1` and the line waits until the zoom is reached [R] | −1 in 972 lines; else 0 to 100 |
| minimum time | `f32` | Seconds the line stays at least; −1 = no minimum [R] | −1 in 549 lines; others 10 to 30 typical |
| wait for button | `u8` | Non-zero: the line waits for a button press [R] | 1 in 34 lines |
| button | `u8` | Which input ends the line (3.2) | 1 or 14 mostly |
| speaker | `s16` | Speaker OID; the panel showing that speaker animates as talking and the other as listening; −1 = none [R] | set in 580 lines |
| lip animation | `s16` | Animation OID for the mouth; −1 = the speaker's own talking animation (OID 1354 under the speaker) [R] | set in 555 lines |
| focus | `s16` | Object the binocular view turns to; −1 = none | set in 99 lines |
| anchor | `s16` | Point OID that anchors the camera for this line; −1 = the dialogue's default anchor (option 967) [R] | set in 20 lines |
| text | `str` | Subtitle, 0 to 386 bytes, empty in 56 lines; mean 128 | |

A line ends when **all** of these have happened [R]: the voice line has finished; the subtitle has been fully
revealed; the zoom has been reached; the minimum time has passed; the panels have opened; the required button has
been pressed (if any); the lip animation has ended (when explicit). Then the next event runs.

### 3.2 Buttons for "wait for button"

Values used [R]: 0 and 1 = the confirm button (Cross on PS2); 2 Square; 3 Circle; 4 Triangle; 5 L1; 6 L2; 7 R1;
8 R2; 9 left stick pushed more than 0.8; 10 right stick more than 0.8; 11 left stick X more than 0.8; 12 left
stick Y more than 0.8; 13 the player performs a double jump; 14 the level's vault has been opened. While waiting,
the overlay shows the button's symbol. On the disc: 1 (11 lines), 14 (21), 9 (1), 10 (1). [D]

## 4. Subtitles

- The text appears in the overlay one character at a time. **Reveal speed** = (characters + 10) / minimum time
  characters per second when the minimum time is positive (or the lip animation's length, when one is given
  explicitly), otherwise **15 characters per second**. [R]
- Colour: the speaker's text colour (option 988, packed RGBA where 128 = full intensity per channel); without a
  speaker, white at half alpha. [R] Values on the disc: (0, 82, 126, 128) on 40 speakers, (20, 110, 30, 128) on 40,
  (213, 62, 76, 128) on 10, (213, 78, 44, 128) on 7. [D]
- Text is stored in the USA build as 8-bit text in the talk event. [D]

## 5. Speakers

Speaker options (all on the disc as placed-object options) [D for counts and values; meanings R]:

| Option | Meaning | Default [R] | On the disc [D] |
|---:|---|---|---|
| 988 | Subtitle colour | white | all 97 |
| 989 | Panel camera field of view, radians | 0.5 | — |
| 990 | Panel camera distance | 400 | 57: 350 (39), 800, 900, 1500, 1700, 2000 |
| 991, 992 | Panel position on screen (fractions of width, height) | 0.5, 0.5 | 95 × x: 0.4 (48), 0.6 (40), 0.3 (7); 7 × y: 1.2 |
| 993 | Panel light offset | (50, 0, 0) | (−80, 0, 300) ×48, (300, 0, 0) ×40, (500, −70, 30) ×7, (200, 0, 300) ×2 |
| 995, 996 | Panel position in confrontations | 0.3, 0.5 | 3 each |
| 997 | The speaker's idle state machine (OID) | none | all 97, OIDs 1378 to 2630 |

When a panel shows a speaker, the speaker's idle state machine is sent to goal state OID 844. [R]

## 6. When dialogues play

States: enabled → triggered → playing → done; plus "unable to call" and "unavailable" for radio dialogues already
played or blocked. [R]

- **Trigger** by script (option 974; 975 untriggers) or automatically once on load when option 968 is set and no
  screen wipe is running. [R]
- A triggered dialogue starts when the player character is in a state that allows it; if it cannot start within
  **5 s**, it returns to enabled. [R]
- Only one dialogue plays at a time. A new triggered dialogue cuts short the playing one unless the playing one is
  **crucial** (option 969; set on 1 dialogue). [R] [D]
- **Played** flag: set when a dialogue finishes or is cut short; stored per level with the save data, keyed by the
  anchor OID (option 976, default the dialogue's own OID). A played confrontation never plays again; a played radio
  dialogue becomes "unable to call". Up to 4 **equivalent** dialogues in other levels (option 973: level key and
  OID) count as played too. [R]
- Dialogues do not start, and waiting ones reset, while a timed-challenge screen, the commentary prompt, commentary
  audio, or an outgoing screen wipe is active. [R]
- Ending: when the last event has run, the overlay closes; remaining script-call and letterbox events are still
  carried out if a dialogue is cut short. [R]

**Skipping.** Option 970 marks a dialogue as not skippable. [R] How the original game lets the player skip lines
is not specified here (*to be observed*). The vault-combination dialogue must never be skippable: it is the only
place the vault code is told. [R] [O]

## 7. Voice playback

- One voice line plays at a time, streamed from disc as a mono VAG at its own sample rate (22,050 or 44,100 Hz),
  in the dialogue mix group (audio.md 3.5). [D] [R]
- While a line plays, music is ducked (audio.md 3.5). [R]
- Radio and briefing voices are not positioned; confrontation voices may be. [R]

## 8. Commentary

The developer commentary (audio.md 2.3: 33 entries, one per level id, each a VAG and a duration) is played from the
level's commentary prompt; while it plays, dialogues are held back (section 6). [R] [D for the table]

## 9. Test vectors

| Item | Value |
|---|---|
| Dialogues per world (index: count) [D] | 1: 42, 2: 4, 3: 39, 4: 11, 5: 34, 6: 36, 7: 34, 8: 35, 9: 4, 10: 33, 11: 2, 12: 34, 13: 4, 14: 34, 15: 1, 16: 3, 17: 35, 18: 35, 19: 32, 20: 5, 21: 35, 22: 5, 23: 34, 24: 34, 25: 1, 26: 35, 27: 2, 28: 1, 29: 3, 30: 34, 31: 4, 32: 33, 33: 34, 34: 1, 35: 2, 36: 34, 37: 4, 38: 2, 39: 7, 40: 4, 41: 5, 42: 12, 43: 3, 44: 3, 45: 33; 0: none |
| First dialogue of "Paris, France" | OID 1371; events: left panel (speaker 1372, no replacement); load voice (sector `0x14D2D9`, 187,232 bytes, identity `0x24B`); talk (zoom −1, minimum time −1, wait for button 1, button 14, speaker 1372, lip animation 4440, focus −1, anchor −1, text 187 bytes beginning "Way to go, Sly.") |
| Movie dialogues | "The Hideout", OIDs 830 and 828, event kinds 2, 5, 0, 3, 7, 6 |

## 10. Open questions

- How the original lets the player skip a line or a whole dialogue.
- Exact panel layout and camera numbers for the picture panels (options 989 to 999) on screen.
- The "binocucom" overlay's own drawing (camera and overlay specs).
