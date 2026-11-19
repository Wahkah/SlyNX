---
id: sly2-formats-movies
title: Sly 2 movies (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      all 23 distinct movies demultiplexed in full (packs, video sequence headers, picture counts, audio headers
      and sizes); audio interleave measured by left/right correlation; every file record in every world scanned
      for references to the movie files; all hashes and durations below
  - kind: disc
    what: same image, re-check 2026-11-17 with picture start codes counted in the demultiplexed video stream only
    used_for: confirmation of every Pictures value and the audio/picture comparison (0x80B differs by 1.38 s)
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (src/mpeg); ProjectCane @ f77825d6, File Extractor GUI/Services/Sly1AudioCodec.cs (demultiplexer, Sly 1)
    used_for: confirmation that the movie player reads the same container as Sly 1; no fact here rests on them alone
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 movies

Read [`../sly1/movies.md`](../sly1/movies.md) first. Sly 2 uses the same container (MPEG-2 program stream, 16 KiB
packs, video in stream `0xE0`, audio in private stream 1 behind a 4-byte sub-header, `SShd`/`SSbd` audio headers)
[D]. The differences:

| | Sly 1 | Sly 2 |
|---|---|---|
| Where the list is | Executable table, 17 records | Executable file table, `disc.md` 5: 24 movie records, 23 distinct [D] |
| Audio sub-header | `FF A0 00 00` | `FF A1 00 00` (still sub-stream `0xFF`; remove the 4 bytes) [D] |
| Audio format (`SShd` format field) | 1: 16-bit PCM | **16 (`0x10`): SPU ADPCM** ([`../sly1/audio.md`](../sly1/audio.md) 2.1) [D] |
| Audio rate | 48,000 Hz | **44,100 Hz** in 21 movies, 48,000 Hz in 2 (identities `0x7D4`, `0x801`) [D] |
| Channels | 2 | 2 [D] |
| Interleave field | `0x200` bytes | **`0x20` = 32 bytes**: the audio data alternates 32 bytes (two ADPCM blocks) of left and 32 bytes of right [D] |
| Video size | 640 × 480 | 640 × 480, except identity `0x7D9`: **720 × 480** [D] |
| Bit-rate field | 5 Mbit/s | 4.5 Mbit/s in 19 movies, 5.0 in 3, 5.5 in 1 (`0x801`) [D] |

Unchanged on every movie [D]: MPEG-2 Main Profile @ Main Level (`0x48`), aspect code 2 (4:3), frame-rate code 4
(29.97 frames per second), VBV field 112, `progressive_sequence` 0, 4:2:0, PES packets about 4 KiB, a system header
in the first pack, an end code after the last pack.

**Decoding the audio.** Take the bytes after the `SSbd` header (its size field gives the length), split them into
alternating 32-byte left and right pieces, and decode each channel's bytes as one continuous SPU ADPCM stream
(28 samples per 16-byte block; no end flags are used). Each channel gives `size / 2 / 16 × 28` samples; the
duration matches the video picture count (counted in the demultiplexed video stream, at 29.97/s) to within 0.07 s in
22 of the 23 movies; in `0x80B` the audio is 1.38 s longer than the pictures (135.98 s against 134.60 s) [D]. The 32-byte interleave was confirmed by the
correlation of the two decoded channels on movie `0x7E8` (0.75 at 32 bytes; below 0.15 at every other power of two
from 16 to 4096) [D].

## Where the movies play

Worlds point at the movies with ordinary 64-byte file records ([`disc.md`](disc.md) 4); every reference found
names the same sector as the executable table (there are no second copies, unlike Sly 1) [D]. Rows are level-table
rows ([`disc.md`](disc.md) 6):

| Identity | Referenced from |
|---|---|
| `0x7D9`, `0x7DE`, `0x7E3`, `0x7E7`, `0x7EC`, `0x7F1`, `0x7F6`, `0x7FA` | row 1 (menu world) only, eight records in a row [D] |
| `0x7DA` | rows 2–7 (episode 1) [D] |
| `0x7DF` | rows 8–11 (episode 2) [D] |
| `0x7E4` | rows 12–13 (episode 3) [D] |
| `0x7E8` | rows 14–16 (episode 4) [D] |
| `0x7ED` | rows 17, 19–26 (episode 5) [D] |
| `0x7F2` | rows 27–31 (episode 6) [D] |
| `0x7F7` | rows 32–37 (episode 7) [D] |
| `0x7FB`, `0x7D5` | rows 38–43 (episode 8) [D] |
| `0x7D4`, `0x801`, `0x802`, `0x808`, `0x80A`, `0x80B` | no world; started by the executable [D] |

*Interpretation [I]:* the menu world's eight are the episode openings shown in a movie menu, each episode's own
record is the movie it plays (opening or ending), and the executable-only movies are logos, the long credits
(`0x801`, 578.7 s) and short inserts. Which screen plays which is not determined from the data.

## Test vectors

| Table # | Identity | Start sector | Bytes | Size (W × H) | Bit rate | Pictures | Audio rate | Audio bytes | Video s | SHA-256 |
|---:|---|---:|---:|---|---:|---:|---:|---:|---:|---|
| 0 | `0x7D9` | 1298436 | 40452100 | 720 × 480 | 4.5 M | 2584 | 44100 | 4347136 | 86.22 | `83c36d4fbc8f550f428bf5f4e6687e03ddadf663e63ba7e09b7f48618d9eb75e` |
| 1 | `0x7DA` | 58380 | 21381124 | 640 × 480 | 4.5 M | 1310 | 44100 | 2203904 | 43.71 | `6da0bbff55e685ceb6ef58ebf4e81fb5d479edbe795b9c196ffaf2fce5db742e` |
| 2 | `0x7DE` | 1326206 | 42369028 | 640 × 480 | 4.5 M | 2598 | 44100 | 4370688 | 86.69 | `bb475cf6b993cd004e6c878a1f83ced8bcaf13829e57bd1ca2cb28582c31c476` |
| 3 | `0x7DF` | 92343 | 20103172 | 640 × 480 | 4.5 M | 1232 | 44100 | 2072640 | 41.11 | `ca59234ac8e5ba55a35cf22e1004b83680c36b8e80cbc6f0f77a577dc311fb41` |
| 4 | `0x7E3` | 124137 | 38912004 | 640 × 480 | 4.5 M | 2383 | 44100 | 4008960 | 79.51 | `153955fa809cf6be3d528ef756ec3602b51cb24a1331fe583f9b698fb1470217` |
| 5 | `0x7E4` | 220097 | 33865732 | 640 × 480 | 4.5 M | 2095 | 44100 | 3524480 | 69.90 | `5137f4aacb653254ac4878f7719547389829f8208a8ed49371c391b98e26325f` |
| 6 | `0x7E7` | 260612 | 46399492 | 640 × 480 | 4.5 M | 2878 | 44100 | 4841728 | 96.03 | `024fbf9d421ca3c6886059a42672fac4c69d92f95d4a69fc1f606810ca8051c4` |
| 7 | `0x7E8` | 36306 | 25542660 | 640 × 480 | 4.5 M | 1566 | 44100 | 2634560 | 52.25 | `94042801e89a7c9ac416684f94080e650a1eb9809f56e336194cd74586d97f54` |
| 8 | `0x7EC` | 1268530 | 35274756 | 640 × 480 | 4.5 M | 2171 | 44100 | 3652352 | 72.44 | `81a6aa8efe1499ee7b1946a6ac1fddc5c4fad3c5728a69d8fc8cdaa601c9a154` |
| 9 | `0x7ED` | 1285755 | 25968644 | 640 × 480 | 4.5 M | 1609 | 44100 | 2706880 | 53.69 | `aa588930150820e2ede6ea78f24205d9539579e69a781356f0c8ccb6b8ac0372` |
| 10 | `0x7F1` | 102160 | 45006852 | 640 × 480 | 4.5 M | 2784 | 44100 | 4683584 | 92.89 | `eca6a167b45e1ae9f8d0c91fd0b59c96c72989bd7d7a468d6ad90564f5304e4b` |
| 11 | `0x7F2` | 961247 | 24248324 | 640 × 480 | 4.5 M | 1484 | 44100 | 2496576 | 49.52 | `d898351ea1a23174cf95fd6465bc4bbb5d15fa13ace39085bd926be47afc951f` |
| 12 | `0x7F6` | 878801 | 37683204 | 640 × 480 | 4.5 M | 2306 | 44100 | 3879424 | 76.94 | `facc3d665af5256163f6721db637cd9282e33720935bb56b3553447c7a645cf7` |
| 13 | `0x7F7` | 236634 | 28672004 | 640 × 480 | 4.5 M | 1888 | 44100 | 3176256 | 63.00 | `289d83130d96cdf02c674b7d500316b7fd381f0b32b700bda6491abd49f9f8e3` |
| 14 | `0x7FA` | 159131 | 36126724 | 640 × 480 | 4.5 M | 2275 | 44100 | 3827264 | 75.91 | `cd017f138b7c312172dbd7d91b4fcbe06f4bd363802c614d999874a1aba5521a` |
| 15 | `0x7FB` | 1346895 | 85835780 | 640 × 480 | 4.5 M | 5261 | 44100 | 8850624 | 175.54 | `c3637c1c50990cabd9d3ad10f4b4d558fd0231aab9bba221297be55e06a1c21b` |
| 16 | `0x80B` | 897202 | 61325316 | 640 × 480 | 4.5 M | 4034 | 44100 | 6853312 | 134.60 | `dade0dc290f36619bc89d1954428050f1635c2475fd36b866d5e2b4b6b2cec27` |
| 17 | `0x802` | 204695 | 9666564 | 640 × 480 | 4.5 M | 600 | 44100 | 1009088 | 20.02 | `48821aaaf6654f67a679f31e2c2864c64d565c914864df715b73c75670be5cfb` |
| 18 | `0x7D4` | 29785 | 13352964 | 640 × 480 | 5.0 M | 927 | 48000 | 1696896 | 30.93 | `cad16e34205bb8610c1b9f3132121b7aeac8352920ec7b50d24076efde0894d7` |
| 19 | `0x801` | 973088 | 345489412 | 640 × 480 | 5.5 M | 17345 | 48000 | 31748288 | 578.74 | `7e9942caed0d028bc9ceb7a93ffdfead44a99d0e37c5d980e6bba9f41f664866` |
| 20 | `0x7D5` | 1141785 | 56918020 | 640 × 480 | 4.5 M | 4016 | 44100 | 6756160 | 134.00 | `c92cdd47ada24a9fab38b342e817c0755444a6f1c1dccb0c1db938588c708e01` |
| 23, 27 | `0x808` | 959782 | 2998276 | 640 × 480 | 5.0 M | 132 | 44100 | 222144 | 4.40 | `c1b6125e5d24a49ddf517cb1165099110914377a3b43bd6c661dcd68ef6d2963` |
| 28 | `0x80A` | 1388808 | 6275076 | 640 × 480 | 5.0 M | 277 | 44100 | 466112 | 9.24 | `9352a412169cba61ceab8df4b882fabfb6738f987c6f063b41ec1aa5fcea4a7e` |

"Start sector" includes the `0xCA0` base; "Audio bytes" is the `SSbd` size. Decoded audio of `0x808`: 194,376
samples per channel; left and right are identical in this movie; SHA-256 of the left channel as little-endian `s16`:
`9b4584d0fc117ec50e1761adca5cfdbd7b29246648590fa7c1a7145a9434c908` [D]. First audio payload of `0x808`:
`FF A1 00 00 53 53 68 64 18 00 00 00 10 00 00 00 44 AC 00 00 02 00 00 00 20 00 00 00` [D].

## Open questions

- Which screen starts each movie the worlds do not reference, and what the movie menu offers.
- Whether the menu world's eight records and each episode's record are the same movie (opening) or a pair
  (opening and ending); the pairs `0x7D9`/`0x7DA`, `0x7DE`/`0x7DF` and so on suggest a pair per episode [I].
