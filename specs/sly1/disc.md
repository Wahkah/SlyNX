---
id: sly1-formats-disc
title: Sly 1 (USA) disc layout and world list
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly Cooper and the Thievius Raccoonus, USA, SCUS-97198, ISO image 2,802,319,360 bytes,
      SHA-1 347b54b4a0d8b2c2e6c18cdbde5f1cabcc45b5da,
      SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a
    used_for: >-
      ISO9660 directory, SYSTEM.CNF, executable program header, the 46-entry world table (decoded and
      checked entry by entry), sector ranges, every hash and size in this document
  - kind: executable
    what: SCUS_971.98 from that disc (LBA 276, 1,712,328 bytes, SHA-1 57dc305db57932ad3f1122966cdb695d2e62a47a)
    used_for: location and layout of the world table and the other descriptor tables (read as data, not disassembled)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), its file extractor (ISO detector and level extraction service)
    used_for: >-
      pointer to the world table address and the XOR pairing of its words; every value was then re-read and
      checked against the disc (all 46 entries decode to in-image sectors and decompress to world files that
      parse to their last byte)
  - kind: reference-project
    what: SlyTools @ 63987211 (2024-11-14), unpacker sources and 010 templates
    used_for: confirmation that the WAC/WAL archive pair belongs to other releases, not to this disc
  - kind: reference-project
    what: sly-rs @ 5777345d (2026-11-05), docs/research/disc-survey.md
    used_for: cross-check of the image and executable SHA-1 values (they match)
  - kind: public-doc
    what: ECMA-119 (ISO 9660)
    used_for: volume descriptor and directory record layout
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 (USA) disc layout and world list

This document tells a reader of the player's disc image where the game's files are, how the world (level) files
are found, and what they are called. The format of a world file once it is read is in
[world-file.md](world-file.md).

Conventions used in every Sly 1 spec: all multi-byte integers and floats are **little-endian**; `u8/u16/u32` are
unsigned, `s8/s16/s32` signed two's complement, `f32` is an IEEE-754 single. A **sector** is 2048 bytes; "LBA n"
means byte offset `n * 2048` in the image.

## 1. Which disc

| Item | Value |
|---|---|
| Serial | SCUS-97198 (NTSC-U) |
| Image size | 2,802,319,360 bytes = 1,368,320 sectors |
| Image SHA-1 | `347b54b4a0d8b2c2e6c18cdbde5f1cabcc45b5da` |
| Image SHA-256 | `b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a` |
| Volume identifier (PVD) | `SLY` padded with spaces |
| System identifier (PVD) | `PLAYSTATION` padded with spaces |
| Volume creation date (PVD) | `2002082412000000` |
| Boot executable | `SCUS_971.98`, SHA-1 `57dc305db57932ad3f1122966cdb695d2e62a47a`, SHA-256 `d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c` |

A tool should identify the disc by the executable's hash (cheap: 1.7 MB) and refuse unknown executables. The PAL and
Japanese releases use different executables and different table addresses; they are out of scope here.

## 2. ISO 9660, as far as this disc needs it

Only the parts of ISO 9660 used below are listed. The image is plain 2048-byte-sector "Mode 1 / user data" (no
2352-byte raw sectors).

1. The **primary volume descriptor** is at LBA 16. Byte 0 is `1`, bytes 1..5 are `CD001`, byte 6 is `1`.
2. The **root directory record** is the 34-byte record at offset 156 of that sector.
3. A **directory record** (offsets from the start of the record):

   | Offset | Size | Type | Meaning |
   |---:|---:|---|---|
   | 0 | 1 | u8 | Record length in bytes (0 = no more records in this sector; continue at the next sector boundary) |
   | 2 | 4 | u32 | First sector (LBA) of the extent (little-endian copy; bytes 6..9 hold a big-endian copy) |
   | 10 | 4 | u32 | Data length in bytes (little-endian copy) |
   | 25 | 1 | u8 | Flags; bit 1 (`0x02`) = directory |
   | 32 | 1 | u8 | Name length |
   | 33 | n | ASCII | Name; a single byte `0x00` is "." and `0x01` is ".."; file names end in `;1` |

4. The root directory's extent is read in full and walked record by record.

### File list

The file system holds only boot files. None of the game's data is in a named file.

| Name | LBA | Size (bytes) |
|---|---:|---:|
| `SYSTEM.CNF;1` | 275 | 51 |
| `SCUS_971.98;1` | 276 | 1,712,328 |
| `IOPRP243.IMG;1` | 1113 | 253,201 |
| `SIO2MAN.IRX;1` | 1237 | 6,553 |
| `PADMAN.IRX;1` | 1241 | 44,917 |
| `MCMAN.IRX;1` | 1263 | 90,805 |
| `MCSERV.IRX;1` | 1308 | 7,353 |
| `LIBSD.IRX;1` | 1312 | 25,397 |
| `989SND.IRX;1` | 1325 | 154,600 |

`SYSTEM.CNF` contains three CRLF-terminated lines: `BOOT2=cdrom0:\SCUS_971.98;1`, `VER=1.00`, `VMODE=NTSC`. The
`BOOT2` value names the executable.

Everything else on the disc (worlds, sound, music, movies, about 2.6 GB) lives in sectors that no directory record
points at. The game finds them through tables of **file descriptors** stored inside the executable (section 4).

## 3. No archive index on this disc

Other releases of Sly 1 (prototype and PlayStation 3 builds) keep an index file (`SLY.WAC`) and a data file
(`SLY.WAL`). The retail USA PS2 disc has neither file: its executable carries the index itself. A tool for this
disc must not look for a WAC/WAL pair.

## 4. Reading data out of the executable

The executable is a 32-bit little-endian MIPS ELF. It has one loadable program header:

| Field | Value |
|---|---|
| File offset of segment | `0x1000` |
| Virtual address of segment | `0x00100000` |
| Size in file | 1,553,044 bytes |
| Size in memory | 5,424,772 bytes |

A virtual address `va` inside the file part of that segment is at **file offset `va - 0x100000 + 0x1000`**, and at
image offset `276 * 2048 + (va - 0x100000 + 0x1000)`. A tool should take the offset and address from the program
header rather than hard-coding them, then check they equal the values above.

Strings referenced by pointer are NUL-terminated ASCII.

### 4.1 The file descriptor (32 bytes)

Every pointer from the game to disc data has this shape. The words are stored scrambled in pairs; a value is
recovered by XOR of two words:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 4 | u32 | `A0` |
| 4 | 4 | u32 | `A1` |
| 8 | 4 | u32 | `A2` |
| 12 | 4 | u32 | unknown (not needed to locate data) |
| 16 | 4 | u32 | unknown (not needed to locate data) |
| 20 | 4 | u32 | `B0` |
| 24 | 4 | u32 | `B2` |
| 28 | 4 | u32 | `B1` |

- **Start sector** = `A0 XOR B0` (an LBA in the image).
- **Size in bytes** = `A1 XOR B1`.
- **Identity** = `A2 XOR B2` (a small number naming the item; see 4.2).

A descriptor whose 32 bytes are all zero means "none". Note the order: the size partner is the **last** word and
the identity partner the one before it.

The same 32-byte descriptor appears inside world files (music track, and references from one world to another).

### 4.2 The world table

| Item | Value |
|---|---|
| Virtual address | `0x00247AF0` (file offset `0x148AF0`) |
| Entry size | 44 bytes |
| Entry count | 46 |

Each entry:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 32 | descriptor | Where the **compressed world file** is (section 4.1). Its identity is the world's **search key**: the number other worlds use to refer to this one |
| 32 | 4 | u32 | **World id**: high byte = episode, low byte = level within the episode |
| 36 | 4 | u32 | Virtual address of the world's display name (NUL-terminated ASCII) |
| 40 | 4 | u32 | Task bitmask for the level (values seen: `0x0`, `0x2`, `0xE`); not needed for loading |

To read world *i*: decode the descriptor, read `size` bytes starting at byte `sector * 2048` of the image, and
decompress them as described in [world-file.md](world-file.md) section 2. The compressed data does not fill its last
sector; ignore everything past `size`. The 46 compressed worlds lie between LBA 768,999 and LBA 875,147 and do not
overlap.

Episode numbers (high byte of the world id): 0 = intro and hub, 1 = Tide of Terror, 2 = Sunset Snake Eyes,
3 = Vicious Voodoo, 4 = Fire in the Sky, 5 = The Cold Heart of Hate.

### 4.3 Other descriptor tables (not needed for phase 1)

These use the 32-byte descriptor of section 4.1 with a 32-byte stride. They are listed so later specs can build on
them; their contents are not specified here.

| Virtual address | Count | Contents (first entry decoded) |
|---|---:|---|
| `0x00244F40` | 7 | Sound banks (first: sector `0xBD9A3`, size `0xBBA90`, identity `0x3D`) |
| `0x00248490` | 17 | Movies (first: sector `0xF6A14`, size `0xAB4004`, identity `0x344`) |
| `0x002482D8` | 6 | Ambient sounds (first: sector `0x3AA4B`, size `0x23A0`, identity `0x23F`) |

## 5. World list and test vectors

How a world is named: there is no file name on the disc. A tool should name the extracted world by its display
name from the table (sanitised for the host file system: `Paris, France` contains a comma) or by its world id. The
display names below are the strings in the executable.

Columns: index in the table; world id; display name; search key (descriptor identity); start LBA; compressed size;
decompressed size; SHA-256 and CRC-32 of the **decompressed** world file.

| # | Id | Display name | Key | LBA | Compressed | Decompressed | SHA-256 (decompressed) | CRC-32 |
|---:|---|---|---:|---:|---:|---:|---|---|
| 0 | 0x002 | Splash | 21 | 820372 | 150849 | 430858 | `b157fc0374bea45c135bdb4c34d6b4f1796857c55620d6ec5b07356d8cd48533` | `3b259645` |
| 1 | 0x003 | Paris, France | 10 | 820446 | 4815585 | 7492972 | `6af6fdf31a2b8a18ab7386fca1a70569d68b37bc5abf36a462d45d1d07aa206d` | `6abd5333` |
| 2 | 0x004 | The Hideout | 9 | 863795 | 3075874 | 5549509 | `b0aa149d4488632f08bf777cb643140daba86ec8bb02404579e6985bf8858d5f` | `540c17b0` |
| 3 | 0x100 | A Stealthy Approach | 37 | 867214 | 4911835 | 7352657 | `842e81e34e9b2e15fae81d42ef46201cc200c82020fcd1d5a9bf68ad6c9b631a` | `1426c16a` |
| 4 | 0x101 | Prowling the Grounds | 38 | 782700 | 4541475 | 6879520 | `d79d2e1344c4839fe762c8f718eab83a772da473e2b75e16b67ce293fd364485` | `e8a1cec4` |
| 5 | 0x102 | High Class Heist | 33 | 841343 | 3955734 | 6286161 | `04168b1cfa1454326cb3447aecece10f208e24228e8883103310b0bf5df97742` | `f725557c` |
| 6 | 0x103 | Into the Machine | 36 | 817484 | 5407057 | 8532872 | `0f559e95aced36fd45fac71a49364c17d8874cd1174985a9bd115b9b1b69244f` | `222b9c6e` |
| 7 | 0x104 | A Cunning Disguise | 32 | 802576 | 3639823 | 5989256 | `a902f2dae4a7ee89657ead218b05c82916c52e71309c70cd789c9508458a30ae` | `ae76a0ee` |
| 8 | 0x105 | The Fire Down Below | 31 | 789463 | 3776534 | 6066850 | `ea2f25132826c3b7d3c8b4cf1e6aebaec7e7899ced5a53cd6e5208cfdc86de9a` | `0063d2bc` |
| 9 | 0x106 | Treasure in the Depths | 39 | 871687 | 831054 | 1635707 | `fcc5af00bd1534fdfc454d86537590d397889a900d31e02604142c7e0d89b893` | `a5efb4d0` |
| 10 | 0x107 | The Gunboat Graveyard | 40 | 828524 | 4097321 | 6632964 | `f7642fbadf1186641e21cafaa0238533c64b3be436cfbb13058088e92645fc68` | `e6e4af47` |
| 11 | 0x108 | The Eye of the Storm | 34 | 798818 | 2573755 | 4444661 | `b5ceee218e1651ad591bf5307bad132cfb55deae50003bfabd303dc8b4a5d6c2` | `9379352d` |
| 12 | 0x200 | A Rocky Start | 12 | 768999 | 5660852 | 8422373 | `1ae754f3f14bce7b4ddbb4546f7ac53e00d4dcec1543cac41c708968a85f3597` | `eb1ba1f1` |
| 13 | 0x201 | Muggshot's Turf | 15 | 843508 | 5649845 | 8401201 | `bb611a5fa26db78647cbcdd3b15aad48696f57832351c153bc31de0b0a7d80ed` | `c3ee43e6` |
| 14 | 0x202 | Boneyard Casino | 14 | 846455 | 4606930 | 7281823 | `83295a300b6ebaf4c4a476e63718365bd9a5a25a1ed2e4b7ae2d92a863a2e264` | `1dcdf3a9` |
| 15 | 0x203 | Murray's Big Gamble | 18 | 839261 | 3777341 | 6095806 | `f115fad89ba288f1457448d44970519caa9d05635cff79e1f1767d5ae5974156` | `4c6dd7d6` |
| 16 | 0x204 | At the Dog Track | 19 | 848933 | 4062238 | 6423515 | `4f40af4c8cbbded272e01656a0e0665314690e59d4de9bb5079d3a252296f76e` | `98346ff5` |
| 17 | 0x205 | Two to Tango | 16 | 872970 | 4458281 | 7213928 | `c8629d25615b29d608ff6b44910ee10a0e578dca7322e28ed78fe11af92bfcf9` | `d6e0add1` |
| 18 | 0x206 | Straight to the Top | 17 | 851146 | 4269520 | 7018656 | `52daa199c1dba6aa59844fe60892417113ca9d3db10fd0362c2ad69dd278721a` | `c261b207` |
| 19 | 0x207 | Back Alley Heist | 20 | 791547 | 3991670 | 6537371 | `ab71ac653b37a871428cda3822b19b1115cfef51bc5cb1646fb2b3d6f6c62dd8` | `5d63fd0d` |
| 20 | 0x208 | Last Call | 13 | 785496 | 3418438 | 5833305 | `1ad057f28fc0750042d2265c0b5241992b445a47fad1cf4d4cbfc9454a438f4e` | `df089ce9` |
| 21 | 0x300 | The Dread Swamp Path | 41 | 812070 | 4913776 | 7408414 | `d87793483ac39f94c29d47d63e8b562d568512d85fb6d96af8ed906e38af4720` | `388129c5` |
| 22 | 0x301 | The Swamp's Dark Center | 45 | 855233 | 6062815 | 8669578 | `0debd1f546d6169d5cad06a39309f9138bb7bcf40f9ee4b9ce5c15ec3dd448e9` | `0ce39db4` |
| 23 | 0x302 | The Lair of the Beast | 49 | 830751 | 4992186 | 7342888 | `90a354b64cd4b1e0d028af685376da7a396028485361e880cfd120844f504f6f` | `e11728af` |
| 24 | 0x303 | A Grave Undertaking | 44 | 836281 | 5599352 | 8267679 | `59e3cb20529ef4a3d991719f92c508319a9a4e16d80398b075940c445aad619d` | `fec49525` |
| 25 | 0x304 | Piranha Lake | 47 | 869846 | 2693873 | 4412370 | `0c48218208e33ce0a1b99b3d334d3a4a0088fad9bc03cc205db871189dc16df0` | `eb6feae6` |
| 26 | 0x305 | Descent into Danger | 48 | 793737 | 4842152 | 7364808 | `7d34301630d4347a866e423da62d4b846c12c843fc0e076752935370d9f04959` | `64279a03` |
| 27 | 0x306 | A Ghastly Voyage | 46 | 809756 | 4250610 | 6358243 | `c75a2ec969ceba48c325638b546032a7b7205511889a9095e66b44f50fb7883a` | `a9c96938` |
| 28 | 0x307 | Down Home Cooking | 43 | 806290 | 1655168 | 2992183 | `c41d179a0676432cdbca584f9e9c62500d86e68f336b0265648af16dbafd1dff` | `a4a14a46` |
| 29 | 0x308 | A Deadly Dance | 42 | 776987 | 4332120 | 6730220 | `a1cc6a33c7ed19b21c0dfd529b10f161ee86ce9b30c5085a0aa1cee065920754` | `80c1dfbd` |
| 30 | 0x400 | A Perilous Ascent | 22 | 858416 | 5741519 | 8649197 | `72dc0f51058a6e90c1fb7645791252b4931579454bf76a5e88862fb9e271ee3f` | `0b2c157f` |
| 31 | 0x401 | Inside the Stronghold | 25 | 814708 | 5233729 | 7823136 | `af2681c765bc516fc26c8e4305971ca9a3e05854a82e5be6c2a93054a44098e0` | `9d8bf8d0` |
| 32 | 0x402 | Flaming Temple of Flame | 27 | 779342 | 5637892 | 8353995 | `95ae672c4ca717079cea7b6a047d6b3667618b8ba2c8d60f013f716b9da7e7a6` | `9c4da5f2` |
| 33 | 0x403 | The Unseen Foe | 23 | 807331 | 4489144 | 7114744 | `8517342f1a1c4a8cd3b4f8e0c11a256548a3eb5c734de1e0016eb09294d5d6c3` | `537a49f8` |
| 34 | 0x404 | The King of the Hill | 28 | 861459 | 4289782 | 6812186 | `a88bf70d84128438a53e97d3cf8bf2128ebfce2bade44f5cf363eceebf460448` | `4ea5cc33` |
| 35 | 0x405 | Rapid Fire Assault | 30 | 796324 | 4661786 | 7128580 | `6628a9b0965e30a63cf7cf0643d9fa7572f14f419847e3162f09ddf85067f5b1` | `b7069c14` |
| 36 | 0x406 | Duel by the Dragon | 26 | 772152 | 5455554 | 8368262 | `dc4cddba86014873bca769a0ac0b7acc5ef5e45817afed6d06b27661e4965924` | `3855a8a4` |
| 37 | 0x407 | A Desperate Race | 29 | 833411 | 3193383 | 5158148 | `9f1ad0dca017b947b1f00ff251206cbae722db2a9aeb9193491950ac59ecce4b` | `38898bfd` |
| 38 | 0x408 | Flame Fu! | 24 | 804583 | 2294582 | 4060090 | `a731252d29be5d37d01ac440065d27cd35292eec730ff3c8190d2c40cf692786` | `b399f8e0` |
| 39 | 0x500 | A Hazardous Path | 8 | 800320 | 3384801 | 5533754 | `d6d54c7c4b7c64854d4e870d5b95127eed1c1fa019947f9be8867f7b3262dbf9` | `3ff3c4e0` |
| 40 | 0x502 | Burning Rubber | 7 | 835200 | 1722553 | 2875149 | `27d958e4a57c36dbb1bf47baa43fab6fe0f0a20506cabf617c5313e9cf4ab498` | `77868229` |
| 41 | 0x503 | A Daring Rescue | 6 | 775036 | 2729839 | 4641973 | `7370665d1d7ac03371b145ccd2081246dbfa8b1b15793d146bd307921cca8f12` | `4a980434` |
| 42 | 0x504 | Bentley Comes Through | 2 | 872327 | 739769 | 1806501 | `ec8a4b66167d547bfe04e1782c7375eddb8be2ff58df90bb762e2b0a9c592cc1` | `aa6ce18c` |
| 43 | 0x505 | A Temporary Truce | 5 | 853471 | 3127706 | 5344108 | `3405b4f82d203ceefeb16e9d66394c9eb52e1b210659d56b1e1c8cd069c26e66` | `72871659` |
| 44 | 0x506 | Sinking Peril | 4 | 865520 | 2976103 | 5278748 | `950544975bd54acd43ef0ec9a25e6cf2a084232d71f2018b0b28a7114ac16c98` | `694eb27b` |
| 45 | 0x508 | A Strange Reunion | 3 | 787397 | 3618750 | 6264094 | `3a0eed1625146a5c05b68b0898748ac67e2acac3f1fca04a3f93caf476e451a8` | `496f3de9` |

World ids `0x501` and `0x507` do not exist in the table. Index 0 ("Splash") is the start-up world shown before the
title; index 2 ("The Hideout") is the hub. Index 1 is the Paris prologue.

### Extra checks on the raw (compressed) bytes

| # | SHA-256 of the `size` compressed bytes |
|---:|---|
| 0 | `d446f62e5784f9a724644465a2042b9deeca81e2fd1eca373677538165025d3c` |
| 1 | `d061cd1f3a86dfb926503e3a5825dc9adda5afbf35be8f96a40b9e40026ca17c` |
| 2 | `497541391d6eedbac71609b82b8c933f0df97efc99c963d88aed553481b26e0d` |

### Worked example: world table entry 0

Raw words of entry 0 (at file offset `0x148AF0` of the executable), in order:
`fdc02d54 0ab32899 6bd9b0b8 3c8fb5b8 02ef0f96 fdcca9c0 6bd9b0ad 0ab165d8 00000002 0024a2d0 00000000`.

- Sector = `fdc02d54 XOR fdcca9c0` = `000c8494` = 820372.
- Size = `0ab32899 XOR 0ab165d8` = `00024d41` = 150849.
- Search key = `6bd9b0b8 XOR 6bd9b0ad` = `15` = 21.
- World id `0x002`; name pointer `0x0024A2D0` points at the string `Splash`; task mask 0.

## 6. Open questions

- The meaning of descriptor words 3 and 4 (offsets 12 and 16) is unknown. They are not needed to read data.
- The task bitmask values are noted but not decoded here (gameplay, not loading).
