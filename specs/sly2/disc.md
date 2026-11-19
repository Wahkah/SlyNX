---
id: sly2-formats-disc
title: Sly 2 (USA) disc layout, file records and world list
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2: Band of Thieves, USA, SCUS-97316, ISO image 2,884,141,056 bytes; executable SCUS_973.16 (LBA 271,
      2,597,048 bytes, SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73)
    used_for: >-
      ISO9660 tree, SYSTEM.CNF, program headers, the executable's file table (all 29 records decoded and the
      target sectors inspected), the level table (decoded from all 43 worlds, identical in every one), every
      world read from the disc, decompressed and walked to its end; every hash and size below
  - kind: reference-project
    what: >-
      earlier research in this workspace (Sly2-RE-notes.md; tools/sly2_extract.py and tools/sly2_world.py,
      2026-11); sly2-decomp @ 33e2bab9 (2026-11-12), src/ loaders for the file record and the level table
    used_for: first reading of the record pairing, the 0xCA0 base, the table address and the level-table layout; all re-checked on the disc
  - kind: reference-project
    what: sly-rs @ 5777345d (2026-11-05), docs/research/disc-survey.md
    used_for: cross-check of the executable SHA-1 (matches)
reviewed_by: pending
date: 2026-11-13
---

# Sly 2 (USA) disc layout, file records and world list

Conventions as in [../sly1/disc.md](../sly1/disc.md) (little-endian, 2048-byte sectors, LBA). Tags: **[D]** checked
on the disc, **[R]** from a reference, **[I]** inferred, **[O]** observable in play.

## 1. Which disc

| Item | Value |
|---|---|
| Serial | SCUS-97316 (NTSC-U) [D] |
| Image size | 2,884,141,056 bytes = 1,408,272 sectors [D] |
| Volume identifier (PVD) | blank (spaces) [D] |
| System identifier (PVD) | `PLAYSTATION` [D] |
| Volume creation date (PVD) | `2004081304445000` [D] |
| Boot executable | `SCUS_973.16`, SHA-256 `a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73`, SHA-1 `de2af92a54de71e33edc8a160c31c0337a95e6f5` [D] |

A tool identifies the disc by the executable hash and refuses any other executable. [I]

## 2. File system

ISO 9660 exactly as in Sly 1 ([../sly1/disc.md](../sly1/disc.md) section 2). [D] Files:

| Name | LBA | Size (bytes) |
|---|---:|---:|
| `SYSTEM.CNF;1` | 270 | 51 |
| `SCUS_973.16;1` | 271 | 2,597,048 |
| `IOPRP300.IMG;1` | 1540 | 275,345 |
| `Z6TAIL.DUP;1` | 1,391,880 | 33,554,432 |

`SYSTEM.CNF` holds `BOOT2=cdrom0:\SCUS_973.16;1`, `VER=1.00`, `VMODE=NTSC` (CRLF lines). [D] `Z6TAIL.DUP` sits at the
end of the disc; nothing points into it. *Interpretation:* filler that moves the game data outward. [I] All game
data (worlds, sounds, movies) lies in unnamed sectors from LBA 3232 (`0xCA0`) on. [D]

## 3. The executable

A 32-bit little-endian MIPS ELF with two loadable segments: [D]

| Segment | File offset | Virtual address | Size in file |
|---|---|---|---:|
| 1 | `0x80` | `0x00100000` | `0x1DD230` |
| 2 | `0x1DD380` | `0x00472200` | `0x73C40` |

A virtual address inside segment 1 is at file offset `va − 0x100000 + 0x80`. [D] Read the mapping from the program
headers and check the hash before using any address in these specs. [I]

## 4. File record (64 bytes)

Sly 2 widens Sly 1's 32-byte descriptor ([../sly1/disc.md](../sly1/disc.md) 4.1) to 16 words. Three values are
recovered by XOR of word pairs: [D]

| Value | Formula (words numbered 0 to 15) |
|---|---|
| **Start sector** | `(w0 XOR w13) + 0xCA0` |
| **Size in bytes** (compressed size for worlds) | `w1 XOR w15` |
| **Identity** | `w2 XOR w14` |

Words 3 to 12 carry no data: in the world files they hold leftover text bytes (for example fragments of a long
string from the string table), and they differ from copy to copy of the same record while the decoded values stay
the same. [D] A record whose size decodes to 0 is empty (it then decodes to sector `0xCA0`). [D]

## 5. Executable file table

29 file records at virtual address `0x002A32A8` (file offset `0x1A3328`), 64 bytes apart. [D] Contents:

| # | Start sector | Size | Identity | First bytes say |
|---:|---:|---:|---|---|
| 0 | 1298436 | 40452100 | 0x7D9 | movie (MPEG-2 program stream) |
| 1 | 58380 | 21381124 | 0x7DA | movie (MPEG-2 program stream) |
| 2 | 1326206 | 42369028 | 0x7DE | movie (MPEG-2 program stream) |
| 3 | 92343 | 20103172 | 0x7DF | movie (MPEG-2 program stream) |
| 4 | 124137 | 38912004 | 0x7E3 | movie (MPEG-2 program stream) |
| 5 | 220097 | 33865732 | 0x7E4 | movie (MPEG-2 program stream) |
| 6 | 260612 | 46399492 | 0x7E7 | movie (MPEG-2 program stream) |
| 7 | 36306 | 25542660 | 0x7E8 | movie (MPEG-2 program stream) |
| 8 | 1268530 | 35274756 | 0x7EC | movie (MPEG-2 program stream) |
| 9 | 1285755 | 25968644 | 0x7ED | movie (MPEG-2 program stream) |
| 10 | 102160 | 45006852 | 0x7F1 | movie (MPEG-2 program stream) |
| 11 | 961247 | 24248324 | 0x7F2 | movie (MPEG-2 program stream) |
| 12 | 878801 | 37683204 | 0x7F6 | movie (MPEG-2 program stream) |
| 13 | 236634 | 28672004 | 0x7F7 | movie (MPEG-2 program stream) |
| 14 | 159131 | 36126724 | 0x7FA | movie (MPEG-2 program stream) |
| 15 | 1346895 | 85835780 | 0x7FB | movie (MPEG-2 program stream) |
| 16 | 897202 | 61325316 | 0x80B | movie (MPEG-2 program stream) |
| 17 | 204695 | 9666564 | 0x802 | movie (MPEG-2 program stream) |
| 18 | 29785 | 13352964 | 0x7D4 | movie (MPEG-2 program stream) |
| 19 | 973088 | 345489412 | 0x801 | movie (MPEG-2 program stream) |
| 20 | 1141785 | 56918020 | 0x7D5 | movie (MPEG-2 program stream) |
| 21 | 350144 | 1949759 | 0x039 | world file (compressed) |
| 22 | 350144 | 1949759 | 0x039 | world file (compressed) |
| 23 | 959782 | 2998276 | 0x808 | movie (MPEG-2 program stream) |
| 24 | 335488 | 9969830 | 0x05D | world file (compressed) |
| 25 | 1169578 | 25688 | 0x7CF | other |
| 26 | 350144 | 1949759 | 0x039 | world file (compressed) |
| 27 | 959782 | 2998276 | 0x808 | movie (MPEG-2 program stream) |
| 28 | 1388808 | 6275076 | 0x80A | movie (MPEG-2 program stream) |

- 24 records are movies (MPEG-2 program streams, `00 00 01 BA`), 23 distinct (records 23 and 27 are the same);
  their format belongs to a later movies spec. [D]
- 4 records point at two world files: identity `0x5D` (the intro hub, the boot world) and identity `0x39` (the
  menu world, three copies of the same record). [D]
- Record 25 is a 25,688-byte uncompressed **memory-card icon model** in the PS2 save-icon format: magic
  `0x00010000`, 1 shape, texture type 3, scale 1.0, 1,068 vertices of 20 bytes each from offset 20 (ending at
  25,652), then a 36-byte animation block. [D] *Interpretation:* the 3-D icon shown in the console's memory-card
  browser for Sly 2 saves. [I]
- The record after the 29th is not a file record (its words decode to nonsense). [D]

The SHA-256 of the boot world's 9,969,830 compressed bytes (record 24) is
`af84b84e273d38f2ff56736e22fb8396fcd2eca2d31e672546c132c0e015f2d2`. [D]

## 6. Level table

Unlike Sly 1, the world list is not in the executable. **Every world file carries the whole level table**
([world-file.md](world-file.md) section 3.4); its decoded contents are identical in all 43 worlds, although the
filler words of the records differ. [D] To find the worlds: read the boot world from executable record 24,
decompress it ([world-file.md](world-file.md) 2), walk its header to the level table, and read each row. [D]

Layout: [D]

| Size | Type | Meaning |
|---:|---|---|
| 4 | u32 | Row count: 44 |
| 4 | u32 | 30. The game stores this word but never reads it again (the executable has a single access to it, the store at virtual address `0x20F384`) [D] |
| var | | 44 rows |

Row:

| Size | Type | Meaning |
|---:|---|---|
| 64 | file record | The compressed world file (section 4); its identity is the **world id** |
| 1 | u8 | **Episode** 0..8 |
| 1 | u8 | **Slot**: 0 for an episode hub, otherwise a number 1..29 shared between related levels across episodes (for example 7 for every "inside" level) |
| 1 | u8 | Hub flag (1 = the episode's hub) |
| 1 | u8 | Count `n` |
| 2n | u16[n] | **Save path** of the level: `n` symbol numbers (always 4 on the disc: 64, an episode symbol 65, 74, 80, 84, 85, 87, 95, 100 or 106, then 67, then 68 for a hub or a per-level symbol) into the save-variable symbol table ([world-file.md](world-file.md) 3.3). The game appends further symbols to this path to find the level's clue-bottle and vault flags in the save data [R] |

Row 18 is empty (size 0). [D] Episodes by the data: 0 intro, 1 to 8 the eight episodes; the levels' own names
(section 7) show their settings (nightclub, palace, temple, prison, castle, train, sawmill, blimp). [D]

## 7. World list and test vectors

A world is named by its **level package name**: the name of the last package in the file
([world-file.md](world-file.md) 3.1), without its 4-byte `FK$X` prefix. [D] Columns: row, world id, episode, slot,
hub, start sector, compressed size, decompressed size, SHA-256 of the decompressed world, level package name,
package count, unread tail bytes ([world-file.md](world-file.md) 5).

| Row | Id | Ep. | Slot | Hub | Sector | Compressed | Decompressed | SHA-256 (decompressed) | Level package | Packages | Tail |
|---:|---|---:|---:|---|---:|---:|---:|---|---|---:|---:|
| 0 | 0x5D | 0 | 0 | yes | 335488 | 9969830 | 17420982 | `cd208877b2b7d54c97472f5c441e15a37666b5e6b17771c708d6107b6f030ad0` | jb_intro | 239 | 934 |
| 1 | 0x39 | 0 | 1 |  | 350144 | 1949759 | 4084599 | `d79a8350969378f7fcba310cdaa7bd394b56e4a3f08dc0236101de49b76bda3a` | dvd_menu | 3 | 567 |
| 2 | 0x41 | 1 | 0 | yes | 353856 | 10938886 | 18746337 | `f55580833b4bfb688368515a852b6cd89f76ade9e5de07f9c0eb23e4f39e3916` | f_nightclub_exterior | 287 | 2177 |
| 3 | 0x3B | 1 | 2 |  | 377568 | 6651154 | 11402859 | `ff848a0534f854b9bed22c622fa23ac3c97dddc46fddfe5462f9c8d3f5a78ce7` | f_nightclub_cellar | 172 | 1995 |
| 4 | 0x3E | 1 | 3 |  | 388768 | 7645080 | 13392193 | `e3a9baf95406b34809976401fb39d7354395b1869d35824196117d8ad8787f17` | f_nightclub_disco | 213 | 2145 |
| 5 | 0x44 | 1 | 4 |  | 417504 | 5227776 | 9073508 | `0bd66479cb53b37ae8ee7e69310f1a4ff3cd49be93030482a6097a3aed7bb8f2` | f_nightclub_heist | 133 | 1988 |
| 6 | 0x47 | 1 | 5 |  | 402976 | 5751263 | 10018371 | `ea5e7bab4e94f9e9ce561000061e6f3827b629589599ffee518a878a94e51e8b` | f_nightclub_moulinrouge | 161 | 2003 |
| 7 | 0x49 | 1 | 6 |  | 409312 | 4908636 | 8588023 | `9439ed629c7b8893fd4741323eaf1faac6446a2aa821d626b94a0ebec11564cf` | f_nightclub_waterpump | 182 | 2007 |
| 8 | 0x52 | 2 | 0 | yes | 427328 | 11689480 | 19256085 | `3ada169b8cd6752c5ff5cd0288e06c8fe893903dd1c5bda49acede77a46a934e` | i_palace_ext | 243 | 1893 |
| 9 | 0x55 | 2 | 7 |  | 444896 | 7786626 | 13362757 | `27f33ce25ac836513ed10d9b2f6e3ee5bbfe26e313766bac627a394469d975a6` | i_palace_int | 194 | 1765 |
| 10 | 0x4F | 2 | 8 |  | 453760 | 6104199 | 10469368 | `89e6a1ee37d4796c8effe1bcb09701f77f9b24b4e9ea5e55683a8931ed2256eb` | i_palace_basement | 195 | 1928 |
| 11 | 0x4C | 2 | 9 |  | 465056 | 6354785 | 11149894 | `9b5d318fe3e58ec0b83181a05c36c4aa75d24511cf28264f3d3e4f2a8a6b7732` | i_palace_ballroom | 162 | 1846 |
| 12 | 0x57 | 3 | 0 | yes | 482752 | 12361630 | 20134199 | `cad4c1b0a83825116d64db374862ed27af03ea1cfb9f609d9b6ff7da0c3eeba0` | i_temple_ext | 287 | 1847 |
| 13 | 0x5A | 3 | 7 |  | 509056 | 7747866 | 13035526 | `f861a99d10ac85aa61febbdbb0ddd8cca32eee8b7d6e604e6a89e67e9f9db23e` | i_temple_int | 227 | 1878 |
| 14 | 0x82 | 4 | 0 | yes | 521344 | 11383853 | 18935089 | `9f79920c9aeed96f3fdaee7da893c7b967df6d03c857d7d7b97f1fdd5a315f14` | p_prison_ext | 284 | 1921 |
| 15 | 0x85 | 4 | 7 |  | 540160 | 7119379 | 12092778 | `5df31efb656a940a1535e8eeea3210b870b2313a5113808a2f2543f6f2e6b710` | p_prison_int | 197 | 1834 |
| 16 | 0x88 | 4 | 10 |  | 552768 | 3737282 | 6576240 | `b027e4f710e7870ecef4eb58902736ddc6b1310c62b13dfb07b5f993cff9e0fd` | p_prison_vault | 85 | 2000 |
| 17 | 0x73 | 5 | 0 | yes | 561312 | 11111409 | 18503588 | `ce7f6a4e6951b1e0bd8febf2fafecc95fbfaf7249006815ce0048a3a4a1f087c` | p_castle_ext | 251 | 2468 |
| 18 | 0x00 | 5 | 7 |  | — | 0 | — | — | (empty slot) | — | — |
| 19 | 0x7C | 5 | 11 |  | 578848 | 4208012 | 7307212 | `ac4505419dd546ed8f41fab2c74ef4d83d812833a1dda4cb21c5697e5284bb11` | p_castle_waterrails | 127 | 2092 |
| 20 | 0x76 | 5 | 12 |  | 603648 | 5363039 | 9196806 | `6e860528ec4b2ce121a78fff21799220d74c4e51767006a592aaa33e5a85d9ab` | p_castle_guardbreak | 139 | 2294 |
| 21 | 0x7F | 5 | 13 |  | 615360 | 4588804 | 7959118 | `9c49afaaba833481291a1bfa2636f1e4af2084419234698d74fba31c37b4ce80` | p_castle_wolftomb | 125 | 2126 |
| 22 | 0x79 | 5 | 4 |  | 589920 | 7234451 | 12512703 | `9b455a3fca025568fbc08a5a261bbd76979054940cc916d0fd1a7332452666a1` | p_castle_heist | 206 | 2207 |
| 23 | 0x6D | 5 | 14 |  | 626688 | 4456544 | 7694241 | `0b5d4bd6e6e5e29d756e6be6ca7264134852e8b9409ab447aa9a106a0a035205` | p_castle_ewoktrainer | 118 | 2097 |
| 24 | 0x6A | 5 | 15 |  | 649056 | 4500407 | 7747669 | `919aecc5cb2a5135e15bf6fd7ccd7ed127daeb13adf0967f6dc54efa3639d65c` | p_castle_ewoktomb | 127 | 2101 |
| 25 | 0x67 | 5 | 16 |  | 637920 | 4273600 | 7465847 | `d6d1a6126462d4e3726f61502188b2f2eefb592aab4aa6d77a90afa72a164346` | p_castle_ewokhall | 116 | 2103 |
| 26 | 0x70 | 5 | 17 |  | 660320 | 4155295 | 7207764 | `9b413421a6a6eeb09d7806af76a91f3ceae58ebe5c7fd73a876015c4688bdd06` | p_castle_ewokwater | 126 | 2100 |
| 27 | 0x36 | 6 | 0 | yes | 671392 | 9734627 | 16163212 | `c477ad6c7e8c218fd5107a28053e4edc9a4cf27acb4f1696a0843d3cd7bb7ef7` | c_train_ext | 242 | 1660 |
| 28 | 0x33 | 6 | 18 |  | 687488 | 5117219 | 9013974 | `5629082b251d84f18a3911bc399e52b3244106be00e06af413ee127954233b28` | c_train_cabins | 157 | 1766 |
| 29 | 0x2D | 6 | 19 |  | 698912 | 6442506 | 11149598 | `06861eba9b6125dade08c124a9f8fbaeeb2fe09f7876096211b2e838950d6b82` | c_train_a | 191 | 1710 |
| 30 | 0x2F | 6 | 20 |  | 710528 | 6689122 | 11607104 | `d255ffda1540b33df6e14fc6b19607d27f5028ae593eaebf52842476a4b904c7` | c_train_b | 207 | 1632 |
| 31 | 0x31 | 6 | 21 |  | 720608 | 6260457 | 10878580 | `c3703ed27408937c08f68d731d17de27e174bb9444ea1097fcaf1bde09c96381` | c_train_c | 179 | 1620 |
| 32 | 0x24 | 7 | 0 | yes | 730464 | 10965082 | 17949071 | `e0d3defe613462827481fd720052e1e8b4ae2489064c00804cbbf41fe95589b4` | c_sawmill_ext | 281 | 2351 |
| 33 | 0x2A | 7 | 22 |  | 749312 | 4755428 | 8295069 | `83b123e33894d36b31cbad5aa674b050f898c3fffeb7f08f3dd157189fb5815a` | c_sawmill_mulch | 146 | 2125 |
| 34 | 0x21 | 7 | 23 |  | 761664 | 5074218 | 8790553 | `c6d8b0f3fe86564a2526deb4ec04894ac0169f916c54e9416e5a5863403f2d9c` | c_sawmill_burn | 147 | 2201 |
| 35 | 0x27 | 7 | 24 |  | 773120 | 4974075 | 8766035 | `1a086d732982c9736200934f88167a365b7ef1620edeb21c7603e161f3791958` | c_sawmill_lighthouse | 172 | 2163 |
| 36 | 0x1B | 7 | 25 |  | 784544 | 3715670 | 6546990 | `669bbc7415e3d95f51841f3ef67b4fc0c69d649ac2f4c5336090ab48b65d73cd` | c_sawmill_bearcave | 94 | 2110 |
| 37 | 0x1E | 7 | 26 |  | 795296 | 4752435 | 8314854 | `28455726dcc75ad8edb81ffd9faa4c601d41a02dcc04c3f314fa5fdaca6c3d28` | c_sawmill_boss | 145 | 2150 |
| 38 | 0x0F | 8 | 0 | yes | 808192 | 9793104 | 17101556 | `c8027ea06739cc335f122b0313f6e318473e83f91d4005d88d36d188e3fbe225` | a_blimp_ext | 326 | 1620 |
| 39 | 0x12 | 8 | 7 |  | 823200 | 6186356 | 11093784 | `af05255fb4a213e3ccc9bf7d1d395506a7250a639fd0a937ed5eebf44e1a9ed5` | a_blimp_int | 214 | 1624 |
| 40 | 0x0C | 8 | 27 |  | 845728 | 4904282 | 8623225 | `bc0fc66dd3ba1a8ed3387610e79eee062421b3cd0e19d21ca3046579f9488f88` | a_blimp_engine_room_murray | 158 | 1625 |
| 41 | 0x06 | 8 | 28 |  | 834976 | 4317986 | 7663411 | `3cc45e581d7e8bc2905cca8671256b2335c1c13d0f2f03a0af834045ceaeff15` | a_blimp_engine_room_bentley | 120 | 1603 |
| 42 | 0x09 | 8 | 29 |  | 856896 | 4271749 | 7595016 | `bc1a478cd769ad5fc3c81d7d5af65b856b3c392644c3b2cffbb05101b660afc8` | a_blimp_engine_room_jt | 112 | 1768 |
| 43 | 0x03 | 8 | 26 |  | 867648 | 9209356 | 16036375 | `26ca1a5566bb5189f821f0a47f3431a799c99052285e175f0d91a79dcbf360fb` | a_blimp_boss_final | 263 | 1623 |

All 43 worlds decompress with the Sly 1 algorithm and walk to their unread tail. [D]

## 8. Open questions

- Which save symbol each of the per-level path values names (the save-variable spec).
