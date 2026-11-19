---
id: sly3-formats-disc
title: Sly 3 (USA) disc layout, file records and world list
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3: Honor Among Thieves, USA, SCUS-97464, ISO image 3,553,755,136 bytes; executable SCUS_974.64 (LBA 271,
      3,200,076 bytes, SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338)
    used_for: >-
      ISO9660 tree, SYSTEM.CNF, program headers, the executable's world tables and movie table (every record decoded
      and the target sectors inspected), every world read from the disc, decompressed and walked to its end; every
      hash and size below
  - kind: reference-project
    what: >-
      earlier research in this workspace (Games/Sly3-RE-notes.md; tools/sly3_extract.py, tools/sly3_world.py,
      2026-11); SafeHouse @ c70803ce (2026-11-12), Sly3-Release loaders
    used_for: first reading of the world-table addresses and the record scramble; all re-checked on the disc
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 (USA) disc layout, file records and world list

Conventions as in [../sly2/disc.md](../sly2/disc.md) and [../sly1/disc.md](../sly1/disc.md) (little-endian,
2048-byte sectors, LBA). Tags: **[D]** checked on the disc, **[X]** read from the executable at the given virtual
address, **[R]** from a reference, **[I]** inferred, **[O]** observable in play. Every table read from the executable
is read from the player's own copy after checking its SHA-256 (section 1); the port refuses on a mismatch.

## 1. Which disc

| Item | Value |
|---|---|
| Serial | SCUS-97464 (NTSC-U) [D] |
| Image size | 3,553,755,136 bytes = 1,735,232 sectors (the PVD's volume space size) [D] |
| Volume identifier (PVD) | blank [D] |
| System identifier (PVD) | `PLAYSTATION` [D] |
| Volume creation date (PVD) | `2005082817072100` [D] |
| Boot executable | `SCUS_974.64`, SHA-256 `ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338`, SHA-1 `5941b7877e6bf5dfb8acb2d04a3c5059367f385b` [D] |

## 2. File system

ISO 9660 as in Sly 1 and Sly 2. Root directory [D]:

| Name | LBA | Size (bytes) |
|---|---:|---:|
| `SYSTEM.CNF;1` | 270 | 57 |
| `SCUS_974.64;1` | 271 | 3,200,076 |
| `IOPRP300.IMG;1` | 1,834 | 275,345 |
| `SLY3.WAD;1` | 3,232 | 3,492,583,424 |

`SYSTEM.CNF` holds `BOOT2 = cdrom0:\SCUS_974.64;1`, `VER = 1.00`, `VMODE = NTSC` (CRLF lines). [D]

**Difference from Sly 2:** the game data is one **named** file, `SLY3.WAD`, starting at LBA 3,232 (`0xCA0`), the same
sector base the file records use (section 4). Sly 2 kept its data in unnamed sectors from the same base and padded
the disc with a dummy file. The WAD ends at sector 1,708,595; the 26,637 sectors after it belong to no file. [D]
A reader can ignore the WAD's name and read sectors directly, as for Sly 2. [I]

## 3. The executable

A 32-bit little-endian MIPS ELF with two loadable segments [D]:

| Segment | File offset | Virtual address | Size in file |
|---|---|---|---:|
| 1 | `0x1000` | `0x00100000` | `0x26ADD8` |
| 2 | `0x26C000` | `0x004F3000` | `0x73FC0` |

A virtual address inside segment 1 is at file offset `va − 0x100000 + 0x1000` (Sly 2: `+ 0x80`). Read the mapping
from the program headers. [D]

## 4. File record (64 bytes)

**Identical to Sly 2** ([../sly2/disc.md](../sly2/disc.md) section 4): start sector `(w0 XOR w13) + 0xCA0`, size
`w1 XOR w15`, identity `w2 XOR w14`; the other ten words are filler. A record whose size decodes to 0 is empty. [D]

## 5. Tables in the executable

### 5.1 World tables

Three parallel tables of 40 entries, one per world slot [X]:

| Virtual address | Entry | Meaning |
|---|---|---|
| `0x002EDE48` | `u32` pointer to a NUL-terminated name | the world's name |
| `0x002EDF10` | `s32` | the world's **episode** (1 menus, 2 prologue, 3 to 8 the six episodes) |
| `0x002EDFD8` | 64-byte file record (section 4) | the compressed world file; the identity is the **world id** |

**Difference from Sly 2:** the world list lives in the executable, not in a level table copied into every world
file. [D] Slot 1 (`sampler_menu`) has an empty record (size 0) and no world file. [X] The other 39 records point at
compressed world files. [D]

*Interpretation:* the menu world (slot 0, `dvd_menu`) is the one loaded at boot. [I]

### 5.2 Movie table

23 file records at virtual address `0x003179F8`, 64 bytes apart [X]. Every one points at an MPEG-2 program stream
(first bytes `00 00 01 BA`) [D]. Records 19 and 21 are the same, and so are 20 and 22, so there are 21 distinct
movies. Record 5 starts at sector 3,232, the WAD's first sector. [D] Details: [movies.md](movies.md).

| # | Start sector | Size | Identity |
|---:|---:|---:|---|
| 0 | 444753 | 77496324 | 0x14F |
| 1 | 482594 | 44302340 | 0x150 |
| 2 | 504227 | 510345220 | 0x151 |
| 3 | 1526531 | 61390852 | 0x130 |
| 4 | 71778 | 54394884 | 0x12F |
| 5 | 3232 | 70549508 | 0x14E |
| 6 | 1556508 | 55754756 | 0x134 |
| 7 | 387661 | 27705348 | 0x135 |
| 8 | 1583733 | 49987588 | 0x139 |
| 9 | 332545 | 30507012 | 0x13A |
| 10 | 1608142 | 63995908 | 0x13E |
| 11 | 253157 | 35815428 | 0x13F |
| 12 | 1639391 | 39616516 | 0x144 |
| 13 | 177776 | 34652164 | 0x145 |
| 14 | 161495 | 33341444 | 0x146 |
| 15 | 1658736 | 45088772 | 0x14A |
| 16 | 98339 | 28377092 | 0x14B |
| 17 | 1680753 | 49741828 | 0x14C |
| 18 | 37681 | 69828612 | 0x14D |
| 19 | 1705042 | 1376260 | 0x152 |
| 20 | 1705715 | 5832708 | 0x153 |
| 21 | 1705042 | 1376260 | 0x152 |
| 22 | 1705715 | 5832708 | 0x153 |

The 64 bytes after the last record do not decode to a sensible record. [X]

## 6. World list and test vectors

Columns: slot, name, episode, world id, start sector, compressed size, decompressed size, SHA-256 of the
decompressed world, group count and shared-group count ([world-file.md](world-file.md) 3.1), top-level object
records ([object-stream.md](object-stream.md)), top-level placement records, shaders, unread tail bytes. [D]

| Slot | Name | Ep. | Id | Sector | Compressed | Decompressed | SHA-256 (decompressed) | Groups | Shared | Objects | Placements | Shaders | Tail |
|---:|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|
| 0 | dvd_menu | 1 | 133 | 753440 | 1906746 | 4083477 | `48718637c810dd5daf7ac1aceca1aefde6dcebe3a385407c7b060d6698eb520b` | 15 | 4 | 172 | 6 | 59 | 1797 |
| 1 | sampler_menu | 1 | — | — | 0 | — | — | — | — | — | — | — | — |
| 2 | i_trainer | 2 | 157 | 775552 | 5547748 | 10092456 | `067fbb0f46879c687cd4da6f5e6735f53a3659fbd67ede9dbe223f0738cbe8a2` | 189 | 12 | 561 | 59 | 850 | 2184 |
| 3 | v_ext | 3 | 239 | 797472 | 13425318 | 25364277 | `60d7615df6e7c0a2ad1b9955b41097220eca19d0071bd04e8e55641b32c60200` | 339 | 19 | 3110 | 630 | 2065 | 5429 |
| 4 | v_canal | 3 | 237 | 826112 | 9203376 | 18690203 | `38fff196a20f6c083eaa0a510c80d371bec15641d7f8c34ae3cb8fa23d9051b9` | 187 | 6 | 970 | 143 | 1008 | 5387 |
| 5 | v_apt | 3 | 234 | 835648 | 4424331 | 8320220 | `4b8beaf0de21ccf5a3694b9feced105a6dd0a970bcd09be5f1adeccb05f3f017` | 130 | 5 | 958 | 186 | 645 | 5436 |
| 6 | v_gauntlet | 3 | 242 | 851808 | 8481498 | 14971275 | `0aebdfec19f143abfcad24d088312969c3f3aa99a1655fcb6d555340617b3349` | 235 | 8 | 1601 | 356 | 1108 | 5451 |
| 7 | v_interpol | 3 | 245 | 872448 | 3496739 | 6650373 | `ae9d224a2932976857c9846421d5bb58464c5697006082220f766197ad876c71` | 115 | 5 | 802 | 62 | 502 | 5237 |
| 8 | o_ext | 4 | 201 | 901280 | 11982028 | 19997607 | `cd0f69420512c9d0a7a32ea11e48c8c0ced8b70ac97a7c3c73792d998b0343da` | 281 | 17 | 2919 | 440 | 1715 | 4391 |
| 9 | o_quarry | 4 | 204 | 973024 | 7523243 | 13517957 | `426b3fabb9f11d4d4120987aaff275d9a420df8348d1af77506d174d95d36e6d` | 207 | 8 | 877 | 41 | 971 | 4373 |
| 10 | o_arena | 4 | 187 | 931104 | 5686984 | 10119876 | `9bd6e162324ed2a4a04ad948a476cf99eda233187208ecf44669e467a3ea69d6` | 180 | 5 | 891 | 109 | 795 | 4340 |
| 11 | o_cave_a | 4 | 192 | 942560 | 5911606 | 10194832 | `c7da26626c96a32f47d7d56b18626041a227d56dc5e0d25d734ab2ee9ef66cdb` | 175 | 6 | 865 | 97 | 707 | 4448 |
| 12 | o_cave_b | 4 | 195 | 957888 | 5589231 | 9700906 | `b8fd802e22942a2e285135f52177c3b0744e0716bf0f3fa4e7f816db85363e5b` | 157 | 6 | 729 | 33 | 598 | 4346 |
| 13 | o_bar | 4 | 189 | 991136 | 5915836 | 10338986 | `43bee44d7f4ad12289a66c6aebc526a9e3126cb00e14ebbae94c8d59ae59d834` | 178 | 6 | 880 | 64 | 799 | 4378 |
| 14 | o_cave_murray | 4 | 198 | 1008256 | 6849642 | 11477335 | `210deb65b97c61243562a4235ffbe105ca2533cfb20d30c95cc1688cdfad352b` | 168 | 6 | 807 | 77 | 729 | 4391 |
| 15 | h_ext | 5 | 138 | 1035072 | 12650409 | 22409036 | `00985a4e4c47c0458017ff6230c60b8952089916286b48dfa27be6dc5475114b` | 347 | 19 | 2553 | 451 | 1998 | 5964 |
| 16 | h_hotel | 5 | 151 | 1064128 | 6328218 | 11479334 | `26b44695ed24bd9fc0c2b3f77ce8795ef58c20fc5ca39e3c26db938066c4193b` | 174 | 9 | 995 | 214 | 894 | 5926 |
| 17 | h_hangar_a | 5 | 143 | 1082848 | 3539566 | 6785858 | `66bf458a2d2507cb23d80e4d2b43baa28a7f4c0c150767a9fbdc6d71eb83171d` | 115 | 5 | 654 | 41 | 516 | 5954 |
| 18 | h_hangar_b | 5 | 145 | 1094176 | 4609797 | 8678941 | `887f47c62b27356350d38c2cc2cd1cda879038790bcb10a2576f4b90a652cef3` | 136 | 5 | 723 | 64 | 677 | 5981 |
| 19 | h_hangar_c | 5 | 148 | 1112352 | 5638470 | 10570744 | `635454478fd84d4703b47f2524c701aaede659da3c8610dad0bb2e412a539850` | 176 | 7 | 885 | 74 | 833 | 5928 |
| 20 | h_sewer | 5 | 154 | 1127264 | 5503683 | 10925099 | `8eb1874a88309ae233b3d7deae73a7f7011057c93ea78cd5972f7872859e52ce` | 117 | 5 | 1021 | 255 | 480 | 6043 |
| 21 | h_dogfight | 5 | 135 | 1142464 | 8579908 | 14859910 | `5562dce629dbd18a3388a5423994b6c3ec648ccb6cc7043dc3d85ffd82d75c97` | 227 | 9 | 1011 | 195 | 1075 | 6006 |
| 22 | h_hack | 5 | 141 | 1160512 | 1482561 | 3295153 | `56a7db7a7387c02f330a1eafdbe05d39d1155b47bb928e082cac9e0f58d19d29` | 32 | 3 | 528 | 13 | 150 | 5793 |
| 23 | c_ext | 6 | 3 | 1180000 | 12375275 | 21378964 | `5446969784d08c255ce6802e91160252938dbfa38d6ac55752b2a6d9e50fb25e` | 324 | 21 | 2860 | 536 | 1698 | 4820 |
| 24 | c_intro | 6 | 17 | 1210880 | 6675411 | 11848300 | `22b2d1bdaa6caffabf99c50474c8779acb6a94f0ebe9cbe785d050877efc9d1c` | 181 | 7 | 1059 | 213 | 888 | 4780 |
| 25 | c_flashback | 6 | 6 | 1231648 | 4519103 | 8319343 | `331d7e44ce0ca41b024ce5367453715e2c3be8f89d7461d4e861b254556ad4fc` | 118 | 6 | 864 | 195 | 514 | 4799 |
| 26 | c_forest | 6 | 9 | 1246464 | 4984308 | 9285172 | `f0a0c5bc179ea54ad7e1346cb8d924e695acecdc4ec3cbc4645caafb0451f5bc` | 156 | 8 | 1365 | 564 | 583 | 4756 |
| 27 | c_apt | 6 | 1 | 1306752 | 2059187 | 4070050 | `9d14c37b374ed92c13a6a7d6868003aee03e2983fc71ac6ecce299191975921c` | 84 | 4 | 639 | 20 | 359 | 4786 |
| 28 | c_hall_a | 6 | 12 | 1260608 | 6662452 | 11788186 | `588c822e48f2f6f71f4a5d278714491718a40a1cec2d84be19f5270223f86158` | 193 | 9 | 952 | 178 | 974 | 4794 |
| 29 | c_hall_b | 6 | 15 | 1275904 | 6081676 | 11097099 | `94044243d224abb647cd8434440ae263dd6c66de561d0bb3d1aa1bff301731af` | 221 | 12 | 1037 | 190 | 1005 | 4795 |
| 30 | c_tilt_hall | 6 | 20 | 1292256 | 5101147 | 9219638 | `6460a44d41110b26d24a26478fe40ca39e734971eaae8853de1b485d265317c7` | 128 | 5 | 887 | 119 | 596 | 4806 |
| 31 | p_ext | 7 | 213 | 1325024 | 12876900 | 21858684 | `f4827e156eff9d45045e9999c712c6040cbcc1f3249ef1a76112bd389d5dc700` | 317 | 18 | 2595 | 456 | 1995 | 6396 |
| 32 | p_at_sea | 7 | 207 | 1362880 | 11471010 | 20755500 | `a575b68af7d62edd1880ba03e2d40776dee3dcfb9c54d9db449184f178d1cbcd` | 228 | 12 | 1356 | 300 | 1174 | 6732 |
| 33 | p_dive | 7 | 210 | 1383552 | 7144182 | 12437854 | `0d538699825fa6819cd49f5438e7bff52f6d69c2a5e07cb4dea4dc0310588764` | 129 | 5 | 1367 | 570 | 672 | 6430 |
| 34 | p_island_map | 7 | 216 | 1396512 | 8666781 | 14547783 | `e56cb4243c12f3261b84ac21ed5ed62becaf1669fe74f0b0bc79a51575aad40c` | 205 | 7 | 1241 | 133 | 1006 | 6439 |
| 35 | m_ext | 8 | 175 | 1428480 | 17612657 | 30254415 | `870a08e90588f40924cfff973db3a4f2c997f6e15fb47cc418d5cc032f7eb53c` | 401 | 17 | 1929 | 373 | 2263 | 3983 |
| 36 | m_underwater | 8 | 181 | 1459520 | 5765765 | 10273746 | `ae8ddffb493d61222f3f287fc9ae252a78aade3c3c343cbb9c964713fbc00f54` | 111 | 6 | 759 | 101 | 490 | 3826 |
| 37 | m_vault | 8 | 184 | 1470368 | 6253725 | 11031837 | `33a692d2d88e8c17b959fde00a552b35ade2ceb27c0f0db8933d836b223bb2e6` | 183 | 8 | 793 | 75 | 725 | 3933 |
| 38 | m_gauntlet | 8 | 178 | 1487968 | 7095487 | 11432056 | `d6e8b8cadf73644025e5d370b7e62c3d82e778fc4f0f60803487624364e00974` | 114 | 6 | 1299 | 454 | 775 | 3800 |
| 39 | m_boss | 8 | 172 | 1503904 | 5438399 | 9864023 | `9d89303d77c62ac34b6062b47c80863026b2610ac1c64879171d8d44dc592160` | 178 | 6 | 707 | 34 | 764 | 3991 |

Totals: 39 worlds, 7,151 groups, 46,221 top-level object records, 8,121 top-level placement records. [D] The menu
world's 1,906,746 compressed bytes have SHA-256 `a27ff71985596e0cf9864c1d9c8d80a6de2614fe50723381e85067eaa96f8b45`
and start `ff 00 00 0f 00 46 4b 24 58 ff 61 6c 6c 5f 77 61` (the start of the first group name, `FK$Xall_wa…`, inside
the compressed stream). [D]

Name prefixes follow the episodes: `i_` prologue, `v_` Venice, `o_` Outback, `h_` Holland, `c_` China, `p_` pirates,
`m_` the last episode (Kaine Island). [D]

## 7. Open questions

- Which slot the game loads first (presumably `dvd_menu`).
- What the empty slot 1 (`sampler_menu`) was for (a demo-disc build ?).
