---
id: sly1-formats-movies
title: Sly 1 movies (full-motion video)
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a); executable SCUS_971.98
    used_for: >-
      movie table and title strings in the executable; all 17 movies and all 13 movie references in the worlds
      demultiplexed in full (pack and packet structure, MPEG-2 sequence, GOP and picture headers, picture counts,
      audio headers and sizes); every hash and duration below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), File Extractor GUI/Services/Sly1AudioCodec.cs (demultiplexer) and Sly1MediaExtractionService.cs; Sly1-Release/fmv.cpp
    used_for: pointer to the movie table and the audio header tags; every fact re-measured on the disc
  - kind: public-doc
    what: ISO/IEC 13818-1 (MPEG-2 systems, program stream) and ISO/IEC 13818-2 (MPEG-2 video)
    used_for: pack, PES and video header parsing
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 movies

**Summary: every movie is an MPEG-2 program stream (a Sony "PSS" file) holding MPEG-2 Main Profile @ Main Level
video, 640 × 480, 29.97 frames per second, and uncompressed 16-bit stereo PCM audio at 48 kHz in a private stream.**

## 1. Where the movies are

### 1.1 Movie table (executable)

17 file descriptors (`desc32`, [disc.md](disc.md) 4.1) at virtual address `0x00248490`, 32 bytes apart. A movie is
named by its descriptor **identity**.

### 1.2 Title table (executable)

16 pointers at virtual address `0x002699C0` to NUL-terminated titles. Title *i* belongs to movie table entry
*i* + 1 (entry 0 has no title: it is the start-up logo movie). The titles are the game's own strings.

| Table entry | Identity | Title (game string) | Duration (s) | Played from |
|---:|---|---|---:|---|
| 0 | `0x344` | (start-up logo) | 13.65 | world 1 data (copy at another sector) and the start of the game |
| 1 | `0x340` | The Secret Police File | 108.7 | world 1 "Paris, France" |
| 2 | `0x341` | The Tokyo Police File | 110.1 | world 2 "The Hideout" (dialogue object) |
| 3 | `0x33C` | The Making Of Sly Cooper | 430.4 | world 2 (dialogue object) |
| 4 | `0x33F` | Commercials & Outtakes | 274.8 | not referenced by any world |
| 5 | `0x345` | Tide of Terror | 61.9 | world 2 |
| 6 | `0x346` | Raleigh Hung out to Dry | 45.9 | world 11 "The Eye of the Storm" |
| 7 | `0x33D` | Sunset Snake Eyes | 90.8 | world 2 |
| 8 | `0x33E` | Muggshot Busted | 46.9 | world 20 "Last Call" |
| 9 | `0x347` | Vicious Voodoo | 56.9 | world 2 |
| 10 | `0x348` | Mz Ruby Vexed | 30.7 | world 29 "A Deadly Dance" |
| 11 | `0x342` | Fire in the Sky | 75.6 | world 2 |
| 12 | `0x343` | The Panda King on Ice | 51.6 | world 38 "Flame Fu!" |
| 13 | `0x336` | The Cold Heart of Hate | 36.3 | world 2 |
| 14 | `0x33A` | Ten Seconds | 108.2 | not referenced by any world |
| 15 | `0x33B` | The Thievius Raccoonus | 48.5 | not referenced by any world |
| 16 | `0x338` | Credits | 114.0 | not referenced by any world |

World numbers are indices in the world table ([disc.md](disc.md) 5). Worlds reference movies with a `desc32` in an
animation event (event kind 51, 11 references) or a dialogue object (event kind 7, 2 references) (see
[object-stream.md](object-stream.md)). Those descriptors point at **second copies** of the movies at other sectors;
each copy is byte-identical to the table's file with the same identity. The four unreferenced movies, and the
logo at start-up, are started by the executable (title screen, menus and the ending; *which screen plays which is
not determined from the data*). The titles are what the game's movie-viewing menu shows.

## 2. Container

An MPEG-2 **program stream** (ISO/IEC 13818-1). Facts measured on all movies:

- The file is a sequence of **packs of exactly 16,384 bytes**, each starting with an MPEG-2 pack header
  (`00 00 01 BA`, 14 bytes, no stuffing), and ending with the end code `00 00 01 B9` at the end of the file.
- The first pack carries a system header (`00 00 01 BB`). A few padding packets (`00 00 01 BE`) occur.
- **Video**: PES packets of stream id `0xE0` (about 4 KiB each), with PTS (and DTS on some packets).
- **Audio**: PES packets of **private stream 1** (`0xBD`). After the PES header, every audio payload starts with
  the 4 bytes `FF A0 00 00` (sub-stream id `0xFF`), which are not audio and must be removed. Concatenating the
  remainders gives the audio stream (2.2).

Standard tools recognise the video stream directly. They do not recognise sub-stream `0xFF`, so a converter must
extract the audio itself as below.

### 2.1 Video

| Parameter | Value (all movies) |
|---|---|
| Codec | MPEG-2 video (ISO/IEC 13818-2), Main Profile @ Main Level (`profile_and_level` `0x48`) |
| Frame size | 640 × 480 |
| Aspect ratio code | 2 (4:3 display) |
| Frame rate code | 4 (30000/1001 = 29.97 frames per second) |
| Bit rate field | 12,500 (× 400 bit/s = 5 Mbit/s maximum) |
| VBV buffer size field | 112 |
| Chroma | 4:2:0 |
| Sequence | `progressive_sequence` = 0; frame pictures; top field first |
| GOP | closed; one I picture per 15 (I : P : B about 1 : 4 : 10, e.g. 28 : 109 : 272 in the logo movie) |

### 2.2 Audio

The extracted audio stream starts with two headers:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | ASCII | `SShd` |
| 4 | 4 | u32 | Header size: `0x18` |
| 8 | 4 | u32 | Format: **1 = 16-bit signed PCM, little-endian** (every movie) |
| 12 | 4 | u32 | Sample rate: **48,000** |
| 16 | 4 | u32 | Channels: **2** |
| 20 | 4 | u32 | Interleave: **0x200** (512 bytes) |
| 24 | 8 | | `FF` × 8 |
| 32 | 4 | ASCII | `SSbd` |
| 36 | 4 | u32 | Size of the audio data in bytes |
| 40 | | | Audio data |

The data alternates **512-byte blocks of left and right channel** (L, R, L, R, …), each block being 256 samples
of that channel. To get ordinary interleaved stereo, take sample *k* of each pair of blocks from the left block and
the right block in turn. Duration = data size / (48,000 × 2 × 2) seconds; it matches the video picture count / 29.97
to within 0.1 s (0.65 s for identity `0x343`).

## 3. Playback behaviour

- Movies fill a 4:3 picture (on a wider screen, pillar-box to keep 4:3). Video is interlaced-coded: a converter
  should deinterlace (or keep field order top-first).
- The movie's own audio plays instead of music and effects, which are paused while a movie runs (*observed in the
  reference port; confirm in the game*).
- The start-up logo movie plays once when the game starts.

## 4. Converting on the player's PC

A converter can feed the video elementary stream (stream `0xE0`) to any MPEG-2 decoder and write the PCM described in
2.2 as a 48 kHz stereo WAV, then encode both to the target format. Example parameters for a standard tool:
video input is MPEG-2 in a program stream at 29.97 fps, top field first; audio is raw `s16le`, 48,000 Hz, 2
channels, after the de-blocking of 2.2.

## 5. Test vectors

| Identity | Table LBA | Size (bytes) | Packs | Pictures | Audio data (bytes) | Video elementary stream (bytes) | SHA-256 of the file |
|---|---|---:|---:|---:|---:|---:|---|
| `0x344` | `0xF6A14` | 11,223,044 | 685 | 409 | 2,621,440 | 8,521,535 | `3fb688e6833fd27311e2530ed5e6bb9dff47792fcf84708b5294327c84568b41` |
| `0x340` | `0x4FFF` | 89,325,572 | 5,452 | 3,259 | 20,855,808 | 67,955,913 | `ba48dd230268519f222c597ed967b89b2412d1391074ea11575e14f0fb36a9b4` |
| `0x341` | `0x3D3A9` | 90,456,068 | 5,521 | 3,300 | 21,122,048 | 68,810,930 | `8224bc26ac498cf3a45e5548cb1575f4ac0e7e3e96c22b728813abcb1aa94bf3` |
| `0x33C` | `0x10678E` | 353,697,796 | 21,588 | 12,900 | 82,642,944 | 269,022,833 | `ce1e8842f9ac907bffe459f30036e425e6c82e9c44fa107c95184b1e5ad390da` |
| `0x33F` | `0x17588` | 225,787,908 | 13,781 | 8,235 | 52,757,504 | 171,726,249 | `d78255226ac699b77eac4e7324f31f6ef372987fd0cc5a43c89e2097e57a0996` |
| `0x345` | `0x67BF1` | 50,872,324 | 3,105 | 1,856 | 11,878,400 | 38,697,515 | `3200cda7efec117a1abca8127e555c4b20ac078cfdaea9c2842f34736b16e3d5` |
| `0x346` | `0xD9EEC` | 37,715,972 | 2,302 | 1,376 | 8,806,400 | 28,687,515 | `457fb6b0a33be3781219cf434e202e59256b2de1dc7394bd315ce6d5a867ed9f` |
| `0x33D` | `0x799DF` | 74,596,356 | 4,553 | 2,721 | 17,415,168 | 56,736,368 | `963f00f75f161a6ae6f021e6dc927c52d22dfa7183db9946e6b082ae58ff9eaf` |
| `0x33E` | `0x5CA8C` | 38,502,404 | 2,350 | 1,405 | 8,985,600 | 29,292,289 | `5ff11b9d307652acbfbec612966172d2a78707e081e16d618073177ee831df49` |
| `0x347` | `0xF0F6C` | 46,710,788 | 2,851 | 1,703 | 10,931,200 | 35,506,825 | `113df4f4de3f19cd56cb5510121d62b93d1a953dbf25c3d8d8a4c31e647d24fd` |
| `0x348` | `0x32431` | 25,214,980 | 1,539 | 920 | 5,888,000 | 19,178,012 | `f63cc2f54be1f04a0423010335c33b6e6a1903798536e05b436c07a78733c82b` |
| `0x342` | `0xE98F3` | 62,111,748 | 3,791 | 2,266 | 14,502,912 | 47,247,723 | `ce102b637e4128436dc25255635efa551c6396f727c166304815c024fc8a4605` |
| `0x343` | `0x100C92` | 42,270,724 | 2,580 | 1,546 | 9,779,200 | 32,232,724 | `aa7b8b010543b412b93788cc1a0742b224d53569a2c99c10606a6fde038805af` |
| `0x336` | `0xD5B7D` | 29,868,036 | 1,823 | 1,089 | 6,977,536 | 22,702,367 | `edeaff63f77715039285e69b4c8b9aef6a1de6d0e9893c2d31a5c8730f4adc05` |
| `0x33A` | `0x13D429` | 88,932,356 | 5,428 | 3,243 | 20,783,104 | 67,622,243 | `1738dfa6d07fd19e5ad71e5d79abd020bfb6d82acc542db2e5c7816bfd07a2d4` |
| `0x33B` | `0x617F0` | 39,878,660 | 2,434 | 1,455 | 9,312,256 | 30,334,998 | `1dd9973dfe5e8816dc444ca440c7c1ccccbfd86c2e988566bb4f72edb74c797d` |
| `0x338` | `0x513F6` | 93,700,100 | 5,719 | 3,417 | 21,899,264 | 71,250,871 | `fd70a97b96f42c661bd80d9792dbd9e67c72c4696f36530d018c6cdd18572b58` |

Second copies referenced from worlds (same bytes as above): `0x340` at `0x6EDB0`, `0x344` at `0xC8F03`, `0x347` at
`0xF89FC`, `0x345` at `0x1147F`, `0x342` at `0x132E51`, `0x33D` at `0x82FA2`, `0x336` at `0x148568`, `0x33C` at
`0x90D55`, `0x341` at `0xDE6DD`, `0x346` at `0x8C2BE`, `0x33E` at `0x4899E`, `0x348` at `0x4DD03`, `0x343` at
`0x3547A`.

Byte checks on the logo movie (`0x344`): first pack header `00 00 01 BA 44 00 04 00 04 01 00 FF 53 F8`; first audio
payload begins `FF A0 00 00 53 53 68 64 18 00 00 00 01 00 00 00 80 BB 00 00 02 00 00 00 00 02 00 00`.

## 6. Open questions

- Which screens start the four movies no world references, and when the start-up logo plays.
- Whether music and effects are paused or only muted during movies.
