---
id: sly3-formats-audio
title: Sly 3 sound - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every world's sound section read; every effect bank, music stream, other stream and voice file header checked; the counts below
  - kind: reference-project
    what: earlier research (tools/sly3_world.py); SafeHouse @ c70803ce, Sly3-Release sound loader
    used_for: the section's read order (the same reads as Sly 2's)
  - kind: disc
    what: re-check of 2026-11-17 (all 39 worlds' sound sections, all 39 banks, all 184 distinct stream files)
    used_for: flag value 1 against the banks' continuous bit, stream lengths and the last-round rule, the empty voice entries, the voice-entry byte by speaker
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release/sound.cpp, sound.h (sound kinds, the section loader), snd989.cpp (the random-choice step)
    used_for: the meaning of the sound kinds and where the random choice keeps its last value; marked [R]
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 sound: differences from Sly 2

Read [../sly2/audio.md](../sly2/audio.md) first. Tags as in [disc.md](disc.md).

## 1. Unchanged

- The world's sound section has Sly 2's layout field for field ([world-file.md](world-file.md) 3.4). [D]
- The effect bank is a Sly 1 bank-file envelope with an `SBlk` description of **version 3** (Sly 2 section 2); all 39
  banks. [D]
- Stream files use the ` KPV` header and 8 KiB interleave of Sly 2 section 3; only the last round's padding differs (2.2). [D]
- Voice files are VAG files with version 0, 22,050 Hz mono (3,654 distinct files, all checked). [D]

## 2. What changed

| | Sly 2 | Sly 3 |
|---|---|---|
| Music stream sample rate | 32,000 Hz | **44,100 Hz** (all 32 world streams) [D]. Lengths per channel vary: 2,199,600 to 5,498,592 bytes; only 9 of the 32 are exactly 3,024,016 bytes (120.0 s). Distribution in 2.2 [D] |
| Worlds without a music stream | 7 of 43 | 7 of 39 (dvd_menu, v_canal, o_arena, h_hangar_a, h_hack, c_apt, c_hall_b) [D] |
| Music channels | 2 to 4 | 2 (18 worlds), 3 (6), 4 (8); groups 0, 1, 2, 3 in order, volume 127, pan 0 [D] |
| Other stream files | 32,000 / 44,100 Hz, 2 or 5 channels | all 44,100 Hz; 2 channels (124 files), 5 (25), 3 (2), 7 (1); 152 distinct, 250 empty records [D] |
| Per-sound flag byte | 0 or 2 | 0 (12,182), 2 (898), **1** (448) [D]; meaning in 2.1 |
| Speaker class IDs | Sly 2 classes | Sly 3 classes: 52 Sly (7,054 entries), 54 Bentley (7,134), 53 Murray (2,570), 60 Carmelita (2,403), 59 Dimitri (1,728), 57 Penelope (1,653), 56 (479), 64 (400), 65 (344), 55 (328), 63 (328), 67 (310), 66 (280), 62 (260), −1 (2,078) [D] |

### 2.1 The per-sound flag byte

The byte is the sound's **kind** as the sound manager uses it [R]: 2 marks a looping (continuous) sound, which keeps
a handle and plays until stopped; any other value plays as a one-shot. At load, every sound whose flag is not 2 is
changed to 2 when its bank record says it is continuous (Sly 1 bank flag bit 0) [R]. Value 1 is the code the game's
own sound objects use for "one-shot" [R].

Data check [D]: all 448 sounds flagged 1 are one-shots in their bank (continuous bit clear in 448 of 448; 408 of them
are a single voice step, 34 have no steps, 6 also use LFO, random-bend or stop steps). Flag 0 covers both kinds (11,714
one-shot, 468 continuous, the latter promoted to 2 at load); flag 2 covers 781 continuous and 117 that are not
continuous in the bank (they loop by script or are kept as handles). A port plays flags 0 and 1 the same way and
treats a sound as looping when its flag is 2 or its bank record is continuous. [I]

**Random choice (step kind 25).** The "last choice" argument is kept in the bank's step record itself, so it is
shared by every playing instance of that sound in the bank (as in Sly 1), not per playing sound [R]. A replay is
bit-identical only from a freshly loaded bank. [I]

### 2.2 Stream files: lengths and the last round

The ` KPV` header and interleave are Sly 2's ([../sly2/audio.md](../sly2/audio.md) 3). The padding rule differs [D],
checked on all 184 distinct stream files (32 world streams, 152 other streams):

- All rounds but the last hold `c` blocks of 8,192 bytes, one per channel, in channel order.
- In the **last round** every channel gets an equal share: **4,096 bytes** when the channel's remaining data is at
  most 4,096 bytes (55 files), otherwise **8,192 bytes** (129 files). Each share starts with that channel's remaining
  data; the rest of the share is padding. File size = 2,048 + (rounds − 1) × 8,192 × `c` + `c` × share. [D]
- Sly 2's files always use 8,192-byte shares (fully padded rounds). [D for Sly 2's files as checked by the Sly 2
  audio spec]

World-stream lengths per channel (bytes, count): 2,199,600 (1), 2,261,760 (1), 2,266,864 (1), 2,267,888 (3),
2,269,200 (1), 2,281,648 (1), 2,290,640 (4), 3,004,432 (1), 3,024,016 (9), 3,024,032 (1), 3,024,048 (4), 3,024,080
(1), 3,024,176 (1), 3,027,040 (1), 3,031,872 (1), 5,498,592 (1). [D]

### 2.3 Voice entries

- **Empty entries** [D]: 8 entries have an empty file record (size 0). All 8 are the same line: id 2373, speaker
  class 53 (`murray`), byte −1, length 0.75 s, no timed records, one in each world of rows 15 to 22 (the Holland
  episode). A port should treat the line as silent for its 0.75 s length [I].
- **The entry byte** (Sly 2: a headset switch, 0 sends the line to a USB headset [R]) takes more values in Sly 3 [D]:
  −1 (19,194), 0 (6,036), 2 (1,267), 3 (611), 6 (96), 4 (95), 5 (50); the byte is the same in every language record
  of an entry (all worlds carry one language). By speaker class: 5 occurs only with class 67, 6 only with class 65,
  4 only with no speaker (−1) or class 60 [D]. Its Sly 3 meaning is not established; reading only 0 as "headset" and
  every other value as "speakers" matches the Sly 2 rule [I].
- **Speaker classes** are the class IDs of the object-stream spec, named by the name strings stored in their
  bodies [D]: 52 `jt` (Sly), 53 `murray`, 54 `bentley`, 55 `shaman`, 56 `panda_king`, 57 `penelope`, 59 `dimitri`,
  60 `carmelita`, 62 `v_lion_boss`, 63 `npc_baron`, 64 `muggshot`, 65 `npc_prince`, 66 `npc_le_fwee`, 67
  `npc_doctor_m`; −1 is a line with no speaker. ([object-stream.md](object-stream.md) class table.)

Identities as in Sly 2: the effect bank's identity is the world id − 1 and the music stream's the world id + 1 in
every world. [D] Sound-ID maps: entry counts 181 to 502 per world, map sizes 319 to 858; all IDs distinct and below
the map size. [D] Voice entries per world: 151 (menu) to 950. [D]

## 3. Test vectors

| Item | Facts |
|---|---|
| Menu world bank | record: sector 754,400, 580,996 bytes, identity 132; first 24 bytes `03000000 02000000 18000000 0c780000 24780000 60650800`; `SBlk` version 3, 181 sounds [D] |
| i_trainer bank | sector 778,272, 1,476,852 bytes, identity 156; 361 sounds, map size 858 [D] |
| i_trainer music | sector 781,088, 6,055,936 bytes, identity 158; ` KPV`, 3,024,016 bytes per channel, data at `0x800`, interleave `0x2000`, 44,100 Hz, 2 channels (volume 127, groups 0 and 1, pan 0) [D] |
| v_ext music | sector 809,472, 9,082,880 bytes, identity 240; 3 channels [D] |

## 4. Open questions

- The Sly 3 meaning of the voice-entry byte values 2 to 6 (2.3).
- As Sly 2: the new bank step kinds, the music-layer mixing, the 8-byte voice records.
