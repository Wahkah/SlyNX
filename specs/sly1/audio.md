---
id: sly1-formats-audio
title: Sly 1 sound - sample formats, sound banks, music, dialogue and the sound driver
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a); executable SCUS_971.98
    used_for: >-
      the 7 effect banks (all records parsed, all 1,429 voice steps decoded to an end flag); all 45 per-world music
      files (37 distinct; banks, instruments and key regions parsed, every sample decoded to an end flag, every
      sequence segment walked); all 1,371 dialogue references (1,363 distinct files) and the 6 ambient and 1 prompt sound in the
      executable (VAG headers read); option values for music, reverb and intermittent sounds read from all 46
      worlds; every test vector below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/sound.cpp, sound.h; File Extractor GUI/Services/Sly1AudioCodec.cs and Sly1MediaExtractionService.cs
    used_for: >-
      first reading of the bank and music layouts, the step kinds, the adaptive-music commands, the intensity
      levels, the distance and pan rules and the pitch rules. Layouts were re-checked on the disc as listed;
      behaviour that the disc data cannot confirm is marked "unconfirmed" below
  - kind: public-doc
    what: PlayStation SPU/SPU2 hardware documentation (ADPCM block format, ADSR register layout, 48 kHz voice clock); Standard MIDI File 1.1 (variable-length quantities, channel messages, meta events)
    used_for: ADPCM decoding, envelope bits, MIDI event encoding
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 sound

All game audio except movie soundtracks ([movies.md](movies.md)). Notation as in [disc.md](disc.md): little-endian
unless a field is marked **BE** (big-endian); a `desc32` is the 32-byte scrambled file descriptor of disc.md 4.1.

## 1. What is on the disc

| Kind | Where it is found | Count | Format |
|---|---|---:|---|
| Effect banks | Executable table at virtual address `0x00244F40`, 7 descriptors (disc.md 4.3) | 7 | Bank file, section 3 |
| Music | One `desc32` per world (world-file.md 4.5, "music") | 45 worlds (37 distinct files; "Splash" has none) | Music file, section 5 |
| Dialogue lines | `desc32` in dialogue objects (event kind 0) and in animation events (kind 44) of the world files | 1,371 references to 1,363 distinct files | VAG file, section 2.2 |
| Ambient loops | Executable table at `0x002482D8`, 6 descriptors | 6 | VAG file |
| Prompt sound | Executable, one descriptor at `0x0026A118` | 1 | VAG file |
| Developer commentary | Executable table at `0x00248748`, 33 entries of 44 bytes (section 2.3) | 33 | VAG file |

A music file is stored once per world that uses it: six tracks are shared by two or three worlds and are stored at
separate sectors (for example the Paris track, identity `0x55`, also serves "A Grave Undertaking"). Copies with
the same identity are byte-identical. In addition, the tracks with identities `0x67` ("Piranha Lake") and `0x62`
("Down Home Cooking") are byte-identical files under different identities: 45 descriptors, 38 identities, 37
distinct files. Use each world's own descriptor.

### 1.1 Where the dialogue descriptors are

Every one of the 1,371 dialogue references sits in one of two places in a world's object stream [D]:

- **Dialogue objects (class 101), event kind 0** — 1,055 references. The dialogue body is the placed layout followed
  by an `s8` event count and the events ([classes/placed-object.md](classes/placed-object.md) 5.2); a kind-0 event's
  payload is the `desc32` of the voice line. Most dialogue objects are placed through proxies (class 83, often
  inside a vault, class 23): owner chains seen are proxy → vault → proxy → dialogue (600), proxy → dialogue (395),
  placed object → proxy → dialogue (43) and dialogue at top level (12).
- **Animation segments (class 131), event kind 44** — 316 references. The `desc32` is the payload of an event of
  kind 44 in one of the segment's three event lists ([animation.md](animation.md) 5.3). The segments usually hang
  under a state machine (class 136) of the companion character (class 14, 244 references), of a vault (20), or under
  the player character (class 7, 41).

A loader that keeps, for each dialogue object and each animation segment, the decoded descriptors in event order
can find every line. Example (Paris, world 1): dialogue OID 1371 (inside proxy OID 1370) has its voice descriptor at
decompressed offset `0x257DCB` (sector `0x14D2D9`, 187,232 bytes, identity `0x24B`); animation segment OID 1368 has
an event-44 descriptor at `0x257574`.

## 2. Sample data

### 2.1 SPU ADPCM

Every sample in the game (effects, music instruments, dialogue) is PlayStation SPU ADPCM. Data comes in **16-byte
blocks**, each giving **28** 16-bit samples:

| Byte | Meaning |
|---:|---|
| 0 | Low nibble: **shift** `s` (0..12 seen). High nibble: **filter** `f` (0..4) |
| 1 | **Flags**: bit 0 = end of sample, bit 1 = repeat, bit 2 = loop start |
| 2..15 | 28 four-bit codes, two per byte, **low nibble first** |

Decoding a block, with the two previous output samples `h1` (most recent) and `h2` carried across blocks (both
zero at the start of a sample):

1. Filter coefficients `(k1, k2)`: f0 = (0, 0), f1 = (60, 0), f2 = (115, −52), f3 = (98, −55), f4 = (122, −60).
   A filter value above 4 does not occur in the game.
2. For each code `c` (sign-extended from 4 bits, −8..7):
   `x = (c × 4096) >> s` (arithmetic shift), then `x = x + ((k1 × h1 + k2 × h2 + 32) >> 6)`,
   clamp `x` to −32768..32767, output `x`, then `h2 = h1`, `h1 = x`.
3. After the block: if flag bit 2 is set, the **loop point** is the first sample of this block. If bit 0 is set,
   the sample ends after this block; if bit 1 is also set, playback continues at the loop point (keeping `h1` and
   `h2` as they are, which is what the hardware does), otherwise the voice falls silent.

Every effect and instrument sample in the game ends with an end flag; none runs past its data.

### 2.2 VAG files (dialogue, ambient, prompt, commentary)

A 48-byte header followed by ADPCM blocks (2.1):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | `VAGp` |
| 4 | 4 | u32 BE | Version: `0x20` on every file |
| 12 | 4 | u32 BE | Size of the ADPCM data in bytes |
| 16 | 4 | u32 BE | **Sample rate** in Hz: 22,050 (1,337 dialogue files and all ambient and prompt files) or 44,100 (26 dialogue files) |
| 32 | 16 | ASCII | Name, NUL-padded (`dap` on the ambient and prompt files) |

All are mono. The descriptor size equals 48 + the header's data size, except one dialogue reference whose
descriptor size is 0: then read the 48-byte header first and use 48 + data size.

### 2.3 Commentary table (executable, 33 × 44 bytes)

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | World id the commentary belongs to |
| 4 | 4 | f32 | Duration in seconds |
| 8 | 32 | `desc32` | The VAG file |
| 40 | 4 | u32 | Virtual address of a NUL-terminated text (the speakers' names, one per line) |

12 of the 33 entries have a duration of 0.0 [D].

## 3. Effect banks

### 3.1 File

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | File kind: `3` |
| 4 | 4 | u32 | Section count: `2` |
| 8 | 8 | 2 × u32 | Description section: offset, size (offset `0x18` in every bank) |
| 16 | 8 | 2 × u32 | Sample section: offset, size. Offset + size = file size |

### 3.2 Description section

Offsets below are from the start of this section.

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | `SBlk` |
| 4 | 4 | u32 | Version: `1` |
| 8 | 4 | u32 | `4` in every bank |
| 12 | 8 | | Zero |
| 20 | 2 | u16 | Zero |
| 22 | 2 | u16 | **Sound count** `N` |
| 24 | 2 | u16 | **Step count** (all sounds together) |
| 26 | 2 | u16 | Close to the number of distinct samples; not needed |
| 28 | 4 | u32 | Offset of the sound table (`0x34`) |
| 32 | 4 | u32 | Offset of the step table |
| 36 | 4 | u32 | `0x5040` in every bank; not needed |

**Sound record** (12 bytes, `N` of them):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 1 | s8 | Volume, 0..127 |
| 1 | 1 | s8 | Mix group, 0..3 (3.5) |
| 2 | 2 | s16 | Pan in degrees (0 = centre) |
| 4 | 1 | s8 | Step count |
| 5 | 1 | s8 | Instance limit (0 in all banks) |
| 6 | 2 | u16 | Flags: bit 0 = **continuous** (keeps playing until stopped); bit 2 seen on one sound; other bits unused |
| 8 | 4 | u32 | Offset of the sound's first step, relative to the step table |

**Step** (40 bytes): `u32` kind, `s32` delay before the step runs (in sound-scheduler ticks; values 0..80 and a few
larger occur; *unconfirmed tick length, the reference port uses 1/240 s*), then 32 bytes of kind-specific data.
A sound's steps run in order from the first.

| Kind | Count in the game | What it does |
|---:|---:|---|
| 1 | 1,429 | **Voice**: start one sample (layout below) |
| 4 | 74 | Set up a low-frequency oscillator on volume, pan, pitch or pitch bend (*details unconfirmed*) |
| 5 | 15 | Start another sound of the same bank as a child. Data: `s16` volume at offset 0 (127, 100, 90, 75, or a negative register reference as in the voice step), `s16` pan at offset 4 (0, or −5 = random), `s16` **sound index at offset 12** |
| 20 | 28 | Nothing |
| 21 | 19 | Loop start marker |
| 22 | 19 | Loop end: jump back to the matching loop start |
| 24 | 1 | Stop: release all voices of this sound and end it |
| 25 | 218 | Random choice: pick one of the following groups of steps at random (data: `s16` number of choices, `s16` steps per choice, `s16` last choice, used to avoid repeating) |
| 26 | 122 | Random extra delay before the next step (`s32` maximum) |
| 27 | 308 | Random pitch bend (`s16` amount in percent of the bend range) |
| 29 | 2 | Add to the pitch bend |
| 31 | 23 | Set a sound register to a random value in a range (`s16` register, `s16` low, `s16` high) |
| 38 | 3 | Wait until all voices of this sound have finished |

Data irregularities a player must tolerate [D]: in bank 6, sounds 52 to 79 have no steps (some with a first-step
offset of −40); in bank 5, sounds 28 and 29 start with a 5-way random choice of one step each but only 4 steps
follow, so playback stops at the end of the sound's steps.

**Voice step data** (first 24 of the 32 bytes):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 1 | s8 | Priority |
| 1 | 1 | s8 | Volume 0..127. Negative values read a sound register (−1..−4), a random value (−5) or a global register (−6 and below) |
| 2 | 1 | s8 | **Centre note**, stored negated (all 1,429 voice steps are negative). Use its magnitude |
| 3 | 1 | u8 | **Fine tuning**, 1/128 semitone units |
| 4 | 2 | s16 | Pan in degrees; negative values read a register, as for volume |
| 6 | 1 | s8 | Lowest note (0 for effects) |
| 7 | 1 | s8 | Highest note (0 for effects) |
| 8 | 1 | s8 | Pitch-bend range down, semitones (0 means 2) |
| 9 | 1 | s8 | Pitch-bend range up, semitones (0 means 2) |
| 10 | 2 | u16 | **Envelope word 1** (3.4) |
| 12 | 2 | u16 | **Envelope word 2** (3.4) |
| 14 | 2 | u16 | Flags: bit 0 is set on 469 of the 1,429 voice steps (meaning not established); other bits 0 |
| 16 | 4 | u32 | **Sample offset** in bytes, relative to the start of the sample section |
| 20 | 4 | u32 | Zero |

The sample runs from its offset to the first block with the end flag (2.1).

### 3.3 Pitch

The sound chip plays a sample at 48,000 samples per second at "pitch 1.0". An effect voice plays its sample at
`48000 × 2^((60 − |centre| + fine / 128) / 12)` samples per second (for example centre −86, fine 66 gives 11,016 Hz,
within 0.1 % of 11,025 Hz; the fine term is **added**, the subtracting reading would give a rate that matches no
common sample rate). Pitch bend adds
`range × bend / 32768` semitones (range = down range for negative bend, up range for positive). Game code can
also add a pitch offset to a playing sound (engines, sliding) and a Doppler shift (3.6).

### 3.4 Envelope

The two envelope words use the SPU hardware ADSR layout (public documentation):

| Word | Bits | Meaning |
|---|---|---|
| 1 | 15 | Attack mode: 0 linear, 1 exponential |
| 1 | 14..10 | Attack shift |
| 1 | 9..8 | Attack step (step = 7 − value) |
| 1 | 7..4 | Decay shift (decay is exponential, decreasing) |
| 1 | 3..0 | Sustain level: the decay ends at `(value + 1) × 0x800` |
| 2 | 15 | Sustain mode: 0 linear, 1 exponential |
| 2 | 14 | Sustain direction: 0 increase, 1 decrease |
| 2 | 12..8 | Sustain shift |
| 2 | 7..6 | Sustain step (increase: 7 − value; decrease: −8 + value) |
| 2 | 5 | Release mode: 0 linear, 1 exponential |
| 2 | 4..0 | Release shift |

The envelope level runs from 0 to 0x7FFF, attack → decay → sustain (until the voice is released) → release → off,
advancing at 48,000 steps per second exactly as the SPU does. The voice's output is its sample times the level.
The common effect setting `0x80FF / 0x9FC0` is a near-instant attack, no decay, full sustain and an **instant**
release (release shift 0, linear).

### 3.5 Volume and mixing

*From the reference port; consistent with the data, not measured on hardware:*

- A voice's volume is `sound volume / 127 × voice volume / 127 × (game volume for the sound)`, and the **square** of
  that value is the amplitude (the driver applies a square-law curve).
- Pan `p` in degrees (−180..180 after wrapping; 0 centre, +90 full right) sets left and right gains
  `(1 − sin p) / 2` and `(1 + sin p) / 2`; a centred voice is at half amplitude on each side.
- Four mix groups: 0 effects, 1 music, 2 dialogue, 3 a fourth effects group. Each has its own master volume. While
  a dialogue line plays, groups 0, 1 and 3 drop to half volume.

### 3.6 Sound IDs, distance, Doppler

- **IDs.** Gameplay data names an effect by a single **sound ID** (in options such as 549, in animation events and
  in object behaviour). The IDs number all sounds of the 7 banks in table order: bank `b` starts at the sum of the
  sound counts of banks 0..b−1. With the counts on the disc (169, 65, 136, 121, 102, 98, 80) the banks start at
  0, 169, 234, 370, 491, 593, 691 and the IDs end at 770.
- **Which banks are loaded.** Bank 0 is always loaded. The world's prolog names one more bank (1..6)
  (world-file.md 4.2). A request for a sound in any other bank is ignored.
- **Distance.** A positioned sound has a **start distance** `S` and a **full-volume distance** `F` (from the
  caller). With `d` the distance from the listener (*the camera; the reference port also compresses distance along
  the view axis while zoomed in with binoculars*): `d > S` silent; `F < d ≤ S` volume × `(S − d)/(S − F)`;
  `d ≤ F` full volume. A one-shot sound that starts out of range is not started; a continuous sound keeps running
  silently and becomes audible when the listener comes in range.
- **Pan from position.** While attenuated, pan = `−0.7 × (1 − attenuation) × (direction to sound · camera's
  left axis)`, converted to degrees as `(pan + 2) × 179.5`, minus 359 when pan > 0 (*reference port*).
- **Doppler.** Sounds created with a Doppler factor shift pitch by `(⌊v × factor × 0.0225⌋ × 1524 / 741) / 128`
  semitones, `v` the speed of approach in game units per second (*reference port*).
- **Intermittent world sounds** (options 549, 550, 551: sound ID, volume range, repeat-interval range, may repeat as a
  group): the sound plays at random intervals inside the interval range, at a random volume inside its range, from
  a random direction around the listener.

### 3.7 Reverb

World option 552 sets the default reverb as two typed arguments: a kind and a depth (0..32767). On the USA disc it
occurs in three worlds: kind 3 with depth 10,000 (twice) and kind 5 with depth 8,000. Options 553 and 554 push and
pop a reverb setting from scripts. Kinds (*unconfirmed mapping to SPU2 presets*): 0 none, 1..3 studio small/medium/
large, 4 pipe, 5 room. Reverb applies to effects only, not to music or dialogue (*reference port*).

## 4. Dialogue

A dialogue line is one VAG file (2.2) streamed from the disc, played once, mono, at its own sample rate, in mix
group 2. Lines started from a dialogue object can be positioned (attenuated like 3.6). Subtitles and the order of
lines belong to the dialogue spec.

## 5. Music

Music is sequenced, not streamed: a world's music file holds an instrument bank, its samples and a set of
MIDI-like sequence segments that are switched at run time by adaptive commands and the game's **intensity**.

### 5.1 File

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | File kind: `1` |
| 4 | 4 | u32 | Section count: `3` |
| 8 | 8 | 2 × u32 | Instrument section: offset (`0x20`), size |
| 16 | 8 | 2 × u32 | Sample section: offset, size |
| 24 | 8 | 2 × u32 | Sequence section: offset, size (ends at the end of the file) |

### 5.2 Instrument section

Offsets from the start of the section.

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | `SBv2` |
| 4 | 4 | u32 | Version `2` |
| 8 | 4 | u32 | `4` |
| 12 | 4 | ASCII | Four-letter track name (for example `PARI` in Paris, `SLHQ` in the hideout) |
| 20 | 2 | u16 | Track-record count (1 in every file) |
| 22 | 2 | u16 | **Instrument count** |
| 24 | 4 | 2 × u16 | Key-region counts (not needed) |
| 28 | 4 | u32 | Offset of the track record |
| 32 | 4 | u32 | Offset of the instrument table |
| 36 | 4 | u32 | Offset of the key-region table |

**Track record** (28 bytes): `u32` 5, three copies of the track name, then `s16` volume (127), `u8` repeat count
(0 = forever), `s8` mix group (1), `s16` pan, and 6 more bytes.

**Instrument** (8 bytes): `u8` key-region count, `s8` volume, `s16` pan, `u32` offset of its first key region
(from the section start). The MIDI program number selects the instrument by index.

**Key region** (24 bytes; same layout as the voice step data of 3.2): priority, volume, centre note, fine
tuning, pan, lowest note, highest note, bend ranges, envelope words, flags, sample offset (relative to the sample
section). A region whose flags have bit 7 set has no sample and is skipped. A note plays every region of its
instrument whose note range contains it.

**Music centre note:** a negative centre note means the region is tuned for the 48 kHz chip (use the magnitude); a
non-negative one is tuned in the older 44.1 kHz convention. A note `n` plays the sample at
`48000 × 2^((n − |centre| + fine / 128) / 12)` samples per second (*unconfirmed: the reference port also scales
non-negative-centre regions by 44100 / 48000*).

### 5.3 Sequence section

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | `2` |
| 4 | 4 | u32 | `1` |
| 8 | 8 | 2 × u32 | Offset (`0x10`) and size of the segment directory |

**Segment directory** (offsets from its own start): `MMID`, `u8` version 1, 2 zero bytes, `u8` **segment count**,
4-letter track name, 4 zero bytes, then one `u32` offset per segment. Each segment ends where the next begins (the
last at the end of the section).

**Segment header** (`MID `, with a trailing space):

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | `MID ` |
| 24 | 4 | u32 | Offset of the event data, from the segment start (`0xDC` seen) |
| 32 | 4 | u32 | **Initial tempo**, microseconds per quarter note |
| 36 | 4 | u32 | **Ticks per quarter note** (480 in every file) |
| 40 | 4 | u32 | Handle (`0x03ss0000`, `ss` = segment number) |

**Events** follow Standard MIDI File track encoding: a variable-length delta time, then a message, with running
status. Differences:

- **Note off is sent as channel pressure**: a `0xDn` message's data byte is the note to release (the game's data
  has exactly as many `0xDn` as `0x9n` messages). `0x9n` with velocity 0 also releases.
- Controllers used: 7 (channel volume) and 10 (pan: value 64..127 maps to 0..+90 degrees, 0..63 to 270..359).
  Program change selects the instrument. Pitch bend uses the key region's bend ranges.
- Meta `0x51` sets tempo; meta `0x2F` (end of segment) restarts the segment from its first event (or ends it after
  the track record's repeat count, when non-zero).
- **Adaptive commands** are system-exclusive messages `F0 75 … F7`. Between `75` and `F7` is a list of commands:

  | Byte | Arguments | Meaning |
  |---|---|---|
  | `00 x` | 1 | Skip the next command unless intensity > x |
  | `01 x` | 1 | Skip the next command unless intensity = x + 1 |
  | `02 x` | 1 | Skip the next command unless intensity < x |
  | `03 s` | 1 | Stop segment s |
  | `04` | 0 | Block start: the commands up to the matching `05` form the body of the preceding test. If the test failed, the whole block is skipped; otherwise it runs |
  | `05` | 0 | Block end |
  | `06 r x` | 2 | Skip the next command unless register r < x |
  | `07 r x` | 2 | Skip the next command unless register r > x |
  | `16 r x` | 2 | Skip the next command unless register r = x |
  | `0B m … F7` | macro | Define macro m as the bytes up to the next `F7` |
  | `0C m` | 1 | Run macro m |
  | `0D r` | 1 | Start the segment numbered (register r − 1) and stop this one |
  | `0E r` | 1 | Start the segment numbered (register r − 1) |
  | `0F g b (ch lo hi)* F7` | var | Define channel group g, tested against intensity (b = 0) or register b − 1: channel `ch` plays only while the value is within lo..hi |
  | `10 g` | 1 | Activate channel group g |
  | `11 s` | 1 | Start segment s and stop this one |
  | `12 s` | 1 | Start segment s |
  | `13 r x` | 2 | Set register r to x |
  | `14 r` / `15 r` | 1 | Increment / decrement register r (clamped 0..127) |

  Command counts over all music files (each segment walked to its end-of-segment event): `00` 6,687; `11` 4,487; `16` 3,395; `12` 3,071; `0C` 2,542; `04`/`05`
  2,368 each; `10` 1,795; `13` 1,298; `02` 1,276; `0E` 1,152; `07` 609; `0D` 607; `06` 467; `03` 30; `0B` 27;
  `14` 27; `0F` 23.

**Segment ends.** Every segment except segment 0 ends its events with `FF 2F 00` followed by zero padding up to
the next segment. Segment 0 has no end-of-segment event; its padding must not be parsed (it would read as data bytes
without a running status). Instruments with zero key regions have a region offset that points before the region
table; ignore it [D].

**Starting the music.** When the world loads, segment 0 is started. In every file it only sets registers and starts
the real first segment with command `11` (for example Paris: `F0 75 13 03 00 11 07 13 01 00 F7` sets register 3 to
0, starts segment 7, sets register 1 to 0). The rest of segment 0 is padding.

**Timing.** Tick length = tempo / ticks-per-quarter microseconds. The reference port runs the sequencer at 240 Hz
and carries the rounding error forward; any exact scheduler works.

**Voices.** At most 24 music voices sound at once (one SPU core); a new note takes the voice of the lowest priority
(oldest on ties) if its own priority is not lower. *Reference port.*

### 5.4 Intensity

The game keeps an **intensity level** from −20 to 107. Game events request a level and later remove the request
(*reference port values*): an alarm or an attacking guard requests 107; hiding or sneaking requests −20. The
current level is the highest active request (0 when there are none). The music's intensity value is
`level + 20` (0..127). Rises take effect at once; falls move one step every three sound updates. While the level
is negative the music group volume scales from 1 down to 2/3 (at −20). Scripts can also set music registers directly.

World options 524 (music on/off) and 525 (music volume, 0.6 to 1.5 seen) adjust music per world.

## 6. Test vectors

PCM hashes are SHA-256 of the decoded samples as little-endian `s16`, from the first block up to and including the
block that carries the end flag, with no loop repetition.

| Item | Location | Facts | Samples | First samples | PCM SHA-256 |
|---|---|---|---:|---|---|
| Effect bank 0, sound 0 (one voice step) | Bank at LBA `0xBD9A3`, 768,656 bytes; sample section at `0x4B90`; sample offset 156,880 | volume 100, group 3; voice: priority 0, volume 110, centre −86, fine 66, envelope `0x80FF`/`0x9FC0` | 1,876 | 0, 0, 0, 0, 0, 0 | `4a52bbf6fb26807754386a303bfefce62c1e5923052ec1d88f37b90e9b6c7b61` |
| Effect bank 0, sound 1 | same bank, sample offset 170,816 | as sound 0 | 16,548 | 0 × 6 | `cad14339248374645d8c1b8a9710ffc4b02edc70ab77adfcc8f8a534fa2a9f23` |
| Ambient loop 1 | LBA `0x3AA4B`, 9,120 bytes, identity `0x23F`; header `56414770 00000020 00000000 00002370 00005622` | 22,050 Hz | 15,848 | 0 × 6 | `dd730987e69cb1b117595c7d98799f91cb990bdd3740a7e754c271fa088ce940` |
| Prompt sound | LBA `0x6ED70`, 130,944 bytes, identity `0x28D` | 22,050 Hz | 229,040 | 0 × 6 | `563b38b9b2b8f36a9f67508c507451eaf38a47c2fb9d08fcfc04df7ca4f45dc5` |
| Shortest Paris dialogue line | LBA `0x14DEA0`, 19,008 bytes, identity `0x261` | 22,050 Hz | 33,152 | 0 × 6 | `e9c1d5afe99ef6e3ee5de55dc9b3c3fa9505257040a95c6a951cb5977d92f9e0` |
| Paris music, instrument 0, key region 0 | Music file at LBA `0xC8E0E`, 501,408 bytes; sample section at `0x7A0` | instrument bytes `08 5a 00 00 d8 00 00 00` (8 regions, volume 90); region: volume 127, centre −48, notes 36..36, envelope `0x80FF`/`0x9FCB`, sample offset 0 | 3,164 | 0 × 6 | `96c6ec1762cc114d336efa18ee8e926a746832de4757a16dbb1fd30025877ed6` |

Structure checks:

- Effect banks (LBA, size, sounds, steps): 0: `0xBD9A3`, 768,656, 169, 431; 1: `0xD4BE2`, 598,240, 65, 233;
  2: `0xBEFFE`, 748,060, 136, 194; 3: `0xBFAFF`, 704,904, 121, 300; 4: `0xC3DA5`, 743,188, 102, 460;
  5: `0xC4C26`, 744,324, 98, 410; 6: `0xBC77E`, 378,980, 80, 233.
- Paris music: 17 instruments, 71 key regions (26 with looping samples), 8 segments; segment tempos start
  300,000, 545,454, 500,000 µs per quarter; track record bytes `05000000 50415249 50415249 50415249 7f000001 00000002 00000000`;
  directory header `4d4d4944 01000008 50415249 00000000`.
- The Hideout music (`SLHQ`): 17 instruments, 71 key regions, 2 segments.

## 7. Open questions

- Step delay units, the LFO step's data layout (targets 4, 5 and 6 occur; the reference port handles only 1..4),
  and step kinds' exact random rules.
- The music 44.1/48 kHz rule.
- Reverb preset mapping and levels.
- The exact boundary semantics of the adaptive register tests (`06`, `07`, `16`).
- All volume, pan, distance, Doppler and intensity rules marked "reference port" need confirmation by listening
  to the game in an emulator.
