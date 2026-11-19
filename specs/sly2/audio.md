---
id: sly2-formats-audio
title: Sly 2 sound (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      the sound section of every world parsed; all 43 effect banks (43 distinct files by SHA-256) parsed through
      their steps (every voice-step sample decoded to an end flag); every step's inline argument checked against
      its sound; the resident stream of every world and the 64 music stream files read (channel layouts); all
      7,970 voice-line references and their timed records checked; option 100 of every ambient-sound object
      (class 43) matched against its world's resident-stream channel numbers; every test vector below
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9, src/new_1fbf20 (world sound-section loader) and src/sound
    used_for: field order of the world sound section and which list length the game uses; meanings re-checked on the disc
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/snd989.cpp, sound.cpp, sound.h, soundglue.cpp, aichase.cpp,
      spliceglue.cpp; Sly-2-Recomp @ b692aa3e, games/scus_97316_sly2_band_of_thieves/native/iop/snd989/
      (snd989_seqev.cpp, snd989_seq.cpp)
    used_for: >-
      what each step kind does with its inline argument, the per-sound flag, the voice-line headset byte and timed
      records, the resident stream's distance layers and the music layer weights; marked [R] and re-checked
      against the data where the data can show it
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 sound

Read [`../sly1/audio.md`](../sly1/audio.md) first. Unchanged in Sly 2 [D]:

- SPU ADPCM blocks and their decoding (Sly 1 2.1).
- The effect-bank file envelope: file kind 3, two sections, `SBlk` description section, sample section (Sly 1 3.1).
- Sound records of 12 bytes (Sly 1 3.2, "sound record").
- The voice-step data of 24 bytes (Sly 1 3.2, "voice step data"): priority, volume, centre note (negated), fine
  tuning, pan, note range, bend ranges, two envelope words, flags, sample offset; envelopes, pitch and the pan law
  (Sly 1 3.3 to 3.5).

What changed:

| | Sly 1 | Sly 2 |
|---|---|---|
| Effect banks | 7 global banks; bank 0 always plus one per world | **one bank file per world**, named by the world's sound section [D] |
| `SBlk` version | 1 | **3**, with a different step encoding (section 2) [D] |
| Sound IDs | global numbering over the 7 banks | a per-world map from game sound IDs to the world bank's sounds (section 1) [D] |
| Music | sequenced (instrument bank and MIDI-like segments) | **streamed**: multi-channel ADPCM stream files in `VPK ` format, a resident stream per world plus music streams chosen by scripts (section 3) [D] |
| Dialogue | VAG files named by object data | VAG files named by a voice-line table in the world's sound section (section 4) [D] |

## 1. The world's sound section

Layout: [`world-file.md`](world-file.md) 3.2. Meanings [D] unless marked:

| Field | Meaning |
|---|---|
| First file record | The world's **effect bank**. Its identity is always the world's identity − 1 (43 of 43) |
| Count `b`, `b` flag bytes | One entry per bank sound, in bank order. Flag values 0 (12,229) or 2 (742). 2 marks a **continuous** (looping) sound; when the bank loads, the game also sets 2 on every sound the bank itself reports as looping [R]. On the disc 654 of the 742 flagged sounds have the continuous bit of their sound record set (Sly 1 3.2) and 88 do not; 388 sounds with that bit set carry flag 0 [D] |
| The next `s16` (`n`) | Size of the sound-ID map: game sound IDs run from 0 to `n − 1` (290 to 690 per world) |
| `b` × `s16` | **Sound IDs**: entry `j` is the game sound ID played by bank sound `j`. All distinct and below `n` in every world. IDs not listed map to no sound |
| Count `m`, stream record, `m` × `s16` | The world's **resident stream** (section 3.1) and one **channel number** per channel of it; `m` equals the stream's channel count in every world. The stream's identity is the world's identity + 1 (36 worlds; 7 have none) |
| Track count `t`, `t` bytes | Voice tracks (languages). `t` is 1 and the byte 0 in every USA world; the game uses the track whose byte matches its language setting [R] |
| Entries | **Voice lines** (section 4) |
| Records: `s16` number, file record | **Music stream files** by music number (section 3.2). 64 distinct files; 122 records are empty (a record whose size decodes to 0) |

A bank can hold up to three more sounds than `b` (over the 43 banks: 25 have none, 13 one, 4 two, 1 three); those are never played [D].

## 2. Effect bank version 3

The description section (offsets from its start, `SBlk` at 0):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 4 | 4 | u32 | Version `3` |
| 22 | 2 | u16 | Sound count |
| 24 | 2 | u16 | **Step count** |
| 26 | 2 | u16 | Number of **distinct samples** (equals the distinct sample offsets of the voice steps in all 43 banks) |
| 28 | 4 | u32 | Offset of the sound table (`0x40`) |
| 32 | 4 | u32 | Offset of the **step table** (= `0x40` + 12 × sound count) |
| 36 | 4 | u32 | `0x5040` (as Sly 1) |
| 40 | 4 | u32 | Sample-section size (repeated at 44) |
| 52 | 4 | u32 | Offset of the **step-data area** (= step table + 8 × step count) |

**Step** (8 bytes, replacing Sly 1's 40-byte step):

| Bits | Meaning |
|---|---|
| word 0, bits 31..24 | Step kind (same numbering as Sly 1 3.2) |
| word 0, bits 23..0 | For kinds 1, 4, 5 and 9: offset of the kind's data in the step-data area (24-byte voice data for 1 and 9; 32-byte oscillator data for 4; the Sly 1 child layout for 5). For the other kinds: an **inline argument**, read as three signed bytes `a0` (bits 7..0), `a1` (bits 15..8) and `a2` (bits 23..16) unless stated otherwise |
| word 1 | Delay before the step (as Sly 1) |

A sound's "first step" field is a byte offset into the step table (8 bytes per step); a sound with no steps holds −8.
Step kinds over all banks: 1 (65,493), 27 (10,968), 25 (10,734), 26 (5,525), 4 (5,022), 20 (882), 39 (596), 21 and
22 (466 each), 31 (249), 5 (77), 24 (75), 9 (64), 29 (45), 30 (13), 23 (1), 43 (1) [D].

**Inline arguments and the new kinds** ([R] for the action; [D] for the value checks):

| Kind | Argument | Action | Disc check |
|---:|---|---|---|
| 25 | `a0` choices, `a1` steps per choice, `a2` the last choice (0 on the disc; run-time state) | random choice, never the same choice twice in a row (Sly 1 3.2) | 10,707 of 10,734 fit inside their sound; the rest overrun its end, as in Sly 1 |
| 26 | the whole 24 bits: a maximum `n` | extra delay of a random 0 to `n` (inclusive) | all 5,525 below 10,000 |
| 27 | `a0`: signed percent | random pitch bend: a random value in −32,768..32,767 scaled by `a0` / 100 | `a0` = 100 (8,277), 127 (2,130), 75, 70, … down to −106 |
| 29 | `a0`: signed amount | add 32,767 × `a0` / 127 to the pitch bend | 50, 70, 127, −128, −100, −70 |
| 30 (new) | `a0` register, `a1` value | set a sound register | all 13: register 0 = 100 |
| 31 | `a0` register, `a1` low, `a2` high | set the register to a random value from low to high (a negative register is a global one, Sly 1 3.2) | 248 of 249 have low ≤ high |
| 39 (new) | `a0` groups, `a1` steps per group, `a2` a pass counter (0 on the disc) | **plays the groups in turn**, not at random: each pass plays group `a2`, then `a2` advances and wraps to 0 after `a0` passes | 572 of 596 fit their sound |
| 9 (new) | data offset | a voice step with the same 24-byte data as kind 1. All 64 kind-9 steps on the disc have a **non-negative** centre note (the older 44.1 kHz tuning convention, Sly 1 5.2); every kind-1 step's is negative | all 64 samples decode to an end flag |
| 23 (new) | none | jump forward to the next loop-end step (leave a loop) | 1 step, inside a loop with no later loop end: it has no effect there |
| 43 (new) | none | marks the steps that run when the sound is **stopped**: in normal playback the sound ends here; a stop request runs the steps after it | 1 step, 3 steps before the end of its sound |

## 3. Stream files (`VPK `)

Music and other long sounds are streamed from files with this 2048-byte header [D]:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | ` KPV` as stored (the u32 `0x56504B20`) |
| 4 | 4 | u32 | ADPCM bytes **per channel** |
| 8 | 4 | u32 | Offset of the data (`0x800`) |
| 12 | 4 | u32 | **Interleave**: `0x2000` (8 KiB) |
| 16 | 4 | u32 | Sample rate: 32,000 (every resident stream; 14 music files) or 44,100 (50 music files) |
| 20 | 4 | u32 | Channel count `c` (2 to 5) |
| 24 | 4 × `c` | | Per channel: `u8` volume (127), `u8` group number, `u16` pan in degrees (0 centre, 270 left, 90 right) |

The data is `c` channels in turn, 8 KiB of each: channel 0, channel 1, …, channel `c − 1`, then channel 0 again.
The file is padded so every channel has a whole number of 8 KiB blocks. Each channel is one continuous ADPCM stream
of the stated length, with no end or loop flags; the stream is played from the start again when it ends [I].

### 3.1 The resident stream (one per world)

2 to 4 channels, all mono at pan 0 with group numbers 0, 1, 2, 3; 32,000 Hz; exactly 120.00 s per channel in the
files checked [D]. It starts with the world and loops [R]. Each channel is a **distance layer**:

- The sound section gives each channel a **channel number** (section 1) [D].
- Ambient-sound objects (class 43) carry option 100, a list of typed records (channel number, start distance `s0`,
  end distance `s1`, volume `g`) [D]. All 101 such records on the disc name one of their own world's channel
  numbers [D]. Example values: (39, 6,000, 10,000, 1.0), (28, 500, 4,000, 1.0), (46, 900, 7,000, 1.1) [D].
- Each record adds a distance fall-off to its channel, measured from the object to the listener: full volume `g`
  within `s0`, silent beyond `s1`, linear in between [R]. A channel's volume is the loudest of its fall-offs; when
  exactly one of its objects is alive the channel pans toward it, otherwise it stays centred [R]. A channel with no
  fall-off plays at its stored volume [I].

### 3.2 Music streams (by music number)

Scripts start and stop music by music number. The game keeps a **stack of up to 8** music streams: starting one
fades out the current top, and stopping the top restarts the one below it [R]. Music streams loop [R].

Channel layouts of the 64 files, by (group, pan) [D]:

| Files | Channels | Layout |
|---:|---:|---|
| 40 | 5 | (0, 0), (1, 270), (1, 90), (2, 270), (2, 90): a mono layer and two stereo layers |
| 22 | 2 | (5, 270), (5, 90): one stereo layer |
| 1 | 5 | (1, 270), (1, 90), (2, 0), (3, 0), (4, 0): row 2's music number 33, identity `0x76E`, 44,100 Hz |
| 1 | 4 | (3, 270), (3, 90), (4, 270), (4, 90): row 12's music number 21, identity `0x794`, 44,100 Hz |

**Layer weights** [R]. Groups 0, 1 and 2 of the top music stream are three versions of the piece: 0 for
sneaking, 1 normal, 2 alert. Each frame the game picks one:

1. **Alert (group 2)** while an alert counter is above zero. A guard chasing the player raises it while the chase
   lasts; a script command raises and lowers it too.
2. Otherwise **sneaking (group 0)** while a sneak counter (raised and lowered by scripts) is above zero.
3. Otherwise by the current player character's state: group 0 while it is in a stealth or interaction state,
   group 2 in one other state, group 1 otherwise (the exact state list is not specified here).

The chosen group's weight moves toward 1 with the speed-limited smoothing used elsewhere in the game (fast speed,
slow speed, ease time): sneak (3, 0, 0.25 s), normal and alert (1, 0, 0.25 s). The other two groups are then
scaled together so that the three weights' squares add up to 1 (an equal-power cross-fade). Groups 3 and 4 fade by
distance to the three playable characters, each with full volume within 2,000 units and silence beyond 5,000 (two
characters on group 3, one on group 4) [R]. Group 5 (the two-channel files) is not weighted [I].

This replaces the provisional "tension" reading of the earlier draft: Sly 1's intensity level does not drive Sly 2's
layers.

## 4. Voice lines

Each entry of the world sound section's voice-line list [D]:

| Field | Meaning |
|---|---|
| `s16` | **Line number** (1,691 distinct over the disc; a number always names the same file) |
| `s16` | The speaking character's class ID (55 Bentley 3,651 lines, 53 Sly 1,608, 54 Murray 836, 63 463, 62 424, 60 281, 64 260, 65 99, 59 97, 61 75; −1 for none 176) [D for the counts, I for the reading] |
| per track: file record, `s8`, `f32` | The VAG file, a **headset byte**, and the **duration** in seconds (matches the decoded length). Headset byte values: −1 (4,867), 0 (2,879), 1 (112), 2 (76), 3 (36) [D]. When the game's headset output is on, a line whose byte is 0 goes to the USB headset; any other value keeps it on the speakers [R] |
| `s32 k`, `k` × 8 bytes | `k` **timed records** (below) |

**Timed record** (8 bytes) [D for the layout, R for the meaning]:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 2 | s16 | Delay after the previous record, in 1/60 s |
| 2 | 2 | s16 | Target 0 to 5 (−1 none) |
| 4 | 4 | s32 / f32 | Value: an integer goal for targets 0, 1, 2, 4 and 5; a float for target 3 |

While the line plays, each record fires when the line's play time reaches it. Target 3 sets how far the speaker's
eyes are closed (0 to 1; disc values 0.0, 0.5, 1.0, 0.7, …). The other targets set a goal on one of up to five state
machines bound to the speaker for the line; target 5 is the mouth (lip sync). When the line stops early, the
remaining records fire at once and the mouth is set to a closed goal [R]. Disc counts by target: 5 (57,042; values
5,372 to 5,378, the mouth shapes), 1 (3,294), 4 (3,036), 0 (1,624), 2 (1,398), 3 (308) [D].

The VAG files are as Sly 1 2.2 except the version field is 0 instead of `0x20` [D]. All 7,970 references are
22,050 Hz mono [D].

## 5. Test vectors

| Item | Location | Facts | Samples | PCM SHA-256 (little-endian `s16`) |
|---|---|---|---:|---|
| World row 3 bank, sound 0, first voice step | Bank at sector 380,864, 1,330,700 bytes, identity 58; sample section at `0xFC8C` | sound: volume 75, group 3, 8 steps, all kind 1; voice data `006eaa42 00000000 0000ff80 c09f0000 c0f80400 00000000`, sample offset 325,824 | 15,316 | `7aecc0b63cebc53d3dbdf68891e8ff96e5e006ea467ab77b4c3721d889dd12c5` |
| World row 3 resident stream, channel 0 | Sector 381,600, 8,783,872 bytes, identity 60; 4 channels × 2,194,304 bytes, 32,000 Hz | first samples 4096, 4864, 3536, 4339 | 3,840,032 | `6726ba3a83a1144a66f0f60dd751a6b909f7e6843d3e0a04b42fe9a91780ce83` |
| same, channel 1 | | | 3,840,032 | `26d66eda76398c343e4c4094ff4d22661c4818987585ae3eb2ce8dab0aeedea0` |
| same, channel 2 | | | 3,840,032 | `89e0779c61928dd895b3c7bdeb0d1dbc8af38acf4e5023c6d58448b1f3d3f4b5` |
| same, channel 3 | | | 3,840,032 | `a9f5b67d4f91d6aa3656b45ab07ec6de10ba8d7ea9500756a790f5427a7448eb` |
| Voice line 1285 (row 3) | Sector 1,253,673, 123,968 bytes, identity 439; speaker class 55; duration 9.835 s | 22,050 Hz | 216,860 | `e04a3943d7b08e82a774afe8fe9792d1f706d4dea320a01a26403acc17b7505c` |

Bank header bytes of row 3's bank (first 24 bytes of the file):
`03000000 02000000 18000000 74fc0000 8cfc0000 80511300`. Step counts per bank (row: sounds, steps, samples): 0: 296,
2490, 351; 3: 309, 2510, 387; 43: 290, 2445, 343 [D].

## 6. Open questions

- Which goals targets 0, 1, 2 and 4 of the timed records set (they depend on the speaker's state machines), and the
  closed-mouth goal (the reference port uses 1,553) [R].
- The exact player-character states that select the sneak and alert music layers.
- Which three characters the group-3 and group-4 fall-offs follow, in which order.
- Kind 23's behaviour where a loop end does follow (no such case on the disc).
- Effect behaviour (distance, Doppler, reverb) is assumed unchanged from Sly 1 3.6 to 3.7; not checked.
