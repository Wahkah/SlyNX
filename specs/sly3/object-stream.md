---
id: sly3-formats-object-stream
title: Sly 3 world file - the object stream
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: option-table file offset and entries 26/27 with their load-time handlers compared against Sly 2; class-157 note
  - kind: disc
    what: SCUS-97464 image; executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338; all 39 worlds
    used_for: >-
      every object record of every world (125,693 at every depth) parsed field by field with the layouts of classes/,
      the option table and class table read from the executable, the counts and test vectors below
  - kind: reference-project
    what: >-
      earlier research (Games/Sly3-RE-notes.md, tools/sly3_world.py, sly3_geom.py, sly3_anim.py, 2026-11); SafeHouse @
      c70803ce (2026-11-12), Sly3-Release object, placement, effect, script and screen-object loaders
    used_for: first reading of the record framing and of each class's read order; every layout re-checked on the disc
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 world file: the object stream

Read [../sly2/object-stream.md](../sly2/object-stream.md) first. Sly 3 keeps Sly 2's object records with a byte size
on every body, so any record can be skipped; the record header gains world filters, and the classes are renumbered.
Notation as in the Sly 2 spec; tags as in [disc.md](disc.md).

- **Transform reference** (`s16`): as Sly 2 ([transforms.md](transforms.md) 1).
- **Header** (used by many layouts, "header" below): `s16` parent transform reference, a 48-byte matrix (three axis
  images and a translation, [transforms.md](transforms.md) 1), `u32` zone mask. As Sly 2's transform header.

## 1. Option lists

**Same shape as Sly 2** ([../sly2/object-stream.md](../sly2/object-stream.md) 1): `s16` option ID and value,
repeated, ended by a negative ID; each value's encoding comes from the executable's option table. [D]

### 1.1 The option table (executable)

| Item | Value |
|---|---|
| Virtual address | `0x0032CA80` (file offset = address − `0x100000` + `0x1000`) |
| Entries | 2,044 of 40 bytes, entry *n* for option ID *n* |
| Entry +0 | `u32` **value type** |
| Entry +4 | `u32` flags; bit `0x400` set: the value is a **typed argument list** |
| Entry +24 | `s32` argument count for typed argument lists |

[X] Read from the player's executable after the SHA-256 check ([disc.md](disc.md) 1).

### 1.2 Value sizes

| Value type | Bytes |
|---|---|
| 0 | 1 |
| 1, 5, 6, 13 | 4 |
| 10 | 8 |
| 2, 4, 9, 11 | 12 |
| 3, 12 | 16 |
| 22 | a wide string: `u16` count, then 2 bytes per character |
| 8, 18, and 24 or above | 4 if the type's bits 12..15 are 1 or 2 (an enumeration or a reference), else nothing more |
| others (7, 14 to 17, 19 to 21, 23) | 2 |

Typed arguments use Sly 2's tag format (1.3 there): `u16` tag (bit 15 = a list, then `u8` count), then each value
sized by the tag: 0 → 1 byte; 1, 5, 6 → 4; 2, 9 → 12; 10 → 8; 7, 14 to 17, 19 to 21, 23 → 2; tags `0x1000` to
`0x1FFF` → 4. [D: every option list of every world parses with these sizes]

### 1.3 Options with a load-time effect

Option **27** (a typed argument list with no arguments) removes the object from the world when the level starts
(Sly 2's option 26). A static render leaves these objects out. [R]
[X] Evidence: entries 26 and 27 have the same value type and flags, so the choice rests on their load-time
handlers. Entry 27's handler (`0x100B08`) has the same instruction sequence as Sly 2's remove-at-start handler
(Sly 2 entry 26, `0x100B10`): a direct call where the object's flag is set. Entry 26's handler (`0x100A88`) instead
matches Sly 2's entry 25 handler (`0x100A90`), which makes an indirect call through the object's class. So entry
27 is remove-at-start and entry 26 is not.

Option 1032 in an option set ([world-file.md](world-file.md) 3.7) sets a group's copy count: arguments (group id,
count). [R]

## 2. Object lists and records

### 2.1 Object list

`s16` count, then that many records. [D]

### 2.2 Object record

| Field | Type | Notes |
|---|---|---|
| mode | `s8` | 0, 1, 2 or 3 (2.3) |
| filter count `f`, condition count `c` | `u8`, `u8` | only when mode = 1 |
| world filter | `f` × `s16` | world slots ([world-file.md](world-file.md) 6) |
| conditions | `c` × `s32` | condition ids ([world-file.md](world-file.md) 6) |
| class ID | `s16` | section 3 |
| object ID | `s32` | |
| script module | `s16` | index into the group's script block; −1 none |
| body size | `s32` | |
| body | | exactly `body size` bytes [D] |

**Difference from Sly 2:** one mode byte replaces Sly 2's record kind and level-row count; filtered records carry
both a world filter and a condition list. [D]

### 2.3 Modes

Top-level records over the 39 worlds: mode 0 (27,779), 1 (16,554), 2 (1,685), 3 (203). [D]

- **0**: an ordinary object. [R]
- **1**: loaded only when its world filter and its conditions both match ([world-file.md](world-file.md) 6); otherwise
  its body is skipped. On the disc 15,115 such records carry only a filter, 1,362 only conditions, 77 both. [D]
- **2 and 3**: objects of a group that is copied (Sly 2's kinds −2 and −1): skipped if an object with the same ID
  already exists in the shared set (2) or in the current copy (3), otherwise loaded into it. [R]

### 2.4 Where object lists appear

| Context | Section |
|---|---|
| End of each group body | [world-file.md](world-file.md) 4 |
| End of every solid body (children) | [classes/solid-object.md](classes/solid-object.md) |
| Inside placements | [classes/placement.md](classes/placement.md) |
| Zones and groupers | [classes/paths-and-volumes.md](classes/paths-and-volumes.md) |
| State machines | [classes/animation.md](classes/animation.md) |
| Effects (components with the same header) | [classes/effects-logic-ui.md](classes/effects-logic-ui.md) |

### 2.5 Walking a whole world

Read the header sections and each group body ([world-file.md](world-file.md)); for each record read the header and
skip the body by its size. To use an object, parse its body with its class's layout (section 3). All 39 worlds walk
this way, and every one of the 125,693 bodies at every depth parses field by field to exactly its size, except one
body of class 157 ([classes/effects-logic-ui.md](classes/effects-logic-ui.md) 6). [D]

## 3. Class table

### 3.1 The class table (executable)

249 entries of 20 bytes at virtual address `0x003409E0` [X]: `u32` address of the base class's entry (0 for the
root), `s32` class ID, `s32` last derived class ID, `u32` object size, `u32` constructor address. The base pointers
give the derivation used by the rule "a class derived from class 8 is a placement". [X] Only class 8 itself occurs as
a placement on the disc. [D]

### 3.2 Classes on the disc

Layouts ([classes/](classes/)): **solid** and **solid + name** ([classes/solid-object.md](classes/solid-object.md));
**placement** ([classes/placement.md](classes/placement.md)); **options**, **header + options**, **header + options +
name**, **options + name**, **task** ([classes/simple-layouts.md](classes/simple-layouts.md)); **volume**, **shape**,
**shape + keys**, **path**, **mesh zone**, **zone**, **grouper** ([classes/paths-and-volumes.md](classes/paths-and-volumes.md));
**emitter**, **effect**, **key list**, **stream list**, **pair list**, **screen object**
([classes/effects-logic-ui.md](classes/effects-logic-ui.md)); **clip**, **state machine**, **command script**
([classes/animation.md](classes/animation.md)).

"Top" counts records in group object lists; "nested" counts records at every other depth (children of solids, in
placements, zones, groupers, state machines); "worlds" is the number of worlds where the class occurs; the groups
are the commonest owners (names from the data, not meanings). [D]

| Class | Layout | Top | Nested | Worlds | Commonest groups |
|---|---|---:|---:|---:|---|
| 2 (0x02) | options | 251 | 0 | 39 | `h_macro`, `c_macro`, `o_macro` |
| 3 (0x03) | header + options | 2,638 | 2,856 | 39 | `p_boat_hero`, `murray`, `bentley` |
| 4 (0x04) | header + options | 2,166 | 182 | 39 | `o_ext`, `c_ext`, `m_gauntlet` |
| 5 (0x05) | header + options | 60 | 0 | 39 | `c_macro`, `h_hangar_c_hangar_defense`, `p_at_sea_multi` |
| 6 (0x06) | header + options | 0 | 270 | 37 | `jt`, `bentley`, `murray` |
| 7 (0x07) | volume | 25 | 0 | 11 | `m_ext_vz`, `p_dive_vz`, `v_gauntlet_vis_zones` |
| 8 (0x08) | placement | 8,121 | 11,061 | 39 | `bentley`, `jt`, `h_hack` |
| 9 (0x09) | path | 2,288 | 585 | 37 | `o_ext_pz`, `p_ext_pz`, `h_ext_pz` |
| 10 (0x0A) | path | 50 | 69 | 20 | `npc_giant_carmelita`, `p_ext`, `h_breakable_windmill` |
| 11 (0x0B) | path | 11 | 3 | 7 | `h_ext`, `m_gauntlet`, `v_gauntlet` |
| 12 (0x0C) | path | 90 | 96 | 22 | `v_ext`, `p_ext`, `m_ext` |
| 13 (0x0D) | path | 20 | 202 | 33 | `crate_small`, `barrel`, `chair` |
| 14 (0x0E) | zone | 258 | 366 | 36 | `jt`, `murray`, `npc_giant_carmelita` |
| 15 (0x0F) | header + options | 0 | 5 | 5 | `spotlight_shooter`, `spotlight_shooter_thick` |
| 16 (0x10) | mesh zone | 573 | 39 | 26 | `o_ext_pz`, `v_ext_pz`, `h_ext_pz` |
| 17 (0x11) | header + options | 5 | 134 | 31 | `carmelita`, `barrel`, `npc_dingo` |
| 18 (0x12) | header + options | 65 | 0 | 35 | `adjust_hand_bentley`, `adjust_hand_jt`, `adjust_hand_murray` |
| 19 (0x13) | shape | 118 | 347 | 36 | `carmelita`, `npc_dingo`, `c_ext_rccar` |
| 20 (0x14) | shape | 0 | 36 | 36 | `ninja_ball` |
| 21 (0x15) | shape | 348 | 184 | 34 | `m_ext_jbintro_escape`, `h_ext_heist_rowboat`, `p_ext_love` |
| 23 (0x17) | shape | 0 | 150 | 36 | `jt`, `murray`, `penelope` |
| 24 (0x18) | shape + keys | 503 | 0 | 38 | `c_macro`, `o_macro`, `h_macro` |
| 25 (0x19) | shape | 16 | 87 | 18 | `o_minecart_hide`, `hide_table`, `c_crawl_tree` |
| 27 (0x1B) | emitter | 20 | 523 | 37 | `jt`, `shaman`, `carmelita_missile_playable` |
| 29 (0x1D) | grouper | 12 | 100 | 37 | `guard_trap`, `npc_dingo`, `carmelita_missile_playable` |
| 30 (0x1E) | emitter | 0 | 2 | 1 | `c_intro_recon` |
| 31 (0x1F) | emitter | 4 | 438 | 38 | `guard_trap`, `carmelita_missile_playable`, `carmelita_missile_playable_charged` |
| 33 (0x21) | header + options | 48 | 38 | 26 | `m_gauntlet_gauntlet`, `p_boat_hero`, `deathtrap_button` |
| 34 (0x22) | header + options | 0 | 142 | 34 | `crate_small`, `barrel`, `npc_dingo` |
| 35 (0x23) | header + options | 3 | 6 | 5 | `v_blimp_anchor`, `v_ext_run`, `booty_return_site` |
| 37 (0x25) | header + options | 42 | 104 | 22 | `c_dragon_giant`, `c_rail_arch`, `o_cave_active_grinder` |
| 38 (0x26) | header + options | 3 | 39 | 20 | `wall_hook`, `hook`, `npc_doctor_m` |
| 39 (0x27) | header + options + name | 203 | 5 | 28 | `c_ext`, `v_gauntlet`, `p_ext` |
| 40 (0x28) | header + options + name | 305 | 7 | 38 | `c_intro_recon`, `m_gauntlet_gauntlet`, `h_ext_rig` |
| 41 (0x29) | header + options | 143 | 876 | 38 | `jt`, `bentley`, `guard_trap` |
| 42 (0x2A) | header + options | 6 | 128 | 37 | `jt`, `bentley`, `murray` |
| 43 (0x2B) | header + options | 12 | 1 | 4 | `v_ext_follow`, `i_trainer_spycam`, `c_ext_get_job` |
| 44 (0x2C) | header + options | 0 | 3 | 3 | `v_lion_boss` |
| 45 (0x2D) | effect | 3,766 | 3,406 | 39 | `jt`, `murray`, `bentley` |
| 46 (0x2E) | header + options | 0 | 1 | 1 | `npc_wolf` |
| 47 (0x2F) | solid | 2,284 | 5,794 | 39 | `jt`, `murray`, `bentley` |
| 48 (0x30) | solid | 68 | 0 | 38 | `i_trainer`, `v_ext_heist`, `v_ext_follow` |
| 49 (0x31) | solid + name | 16 | 4 | 18 | `o_macro`, `m_macro`, `o_claw_machine` |
| 50 (0x32) | solid + name | 5 | 0 | 1 | `panda_king_torso`, `dimitri_torso`, `shaman_torso` |
| 52 (0x34) | solid + name | 36 | 0 | 36 | `jt` |
| 53 (0x35) | solid + name | 31 | 0 | 31 | `murray` |
| 54 (0x36) | solid + name | 34 | 0 | 34 | `bentley` |
| 55 (0x37) | solid + name | 9 | 0 | 9 | `shaman` |
| 56 (0x38) | solid + name | 6 | 0 | 6 | `panda_king` |
| 57 (0x39) | solid + name | 8 | 0 | 8 | `penelope` |
| 58 (0x3A) | solid + name | 65 | 0 | 28 | `npc_dingo`, `ecto_swarmer`, `npc_kangaroo` |
| 59 (0x3B) | solid + name | 6 | 0 | 6 | `dimitri` |
| 60 (0x3C) | solid + name | 10 | 0 | 10 | `carmelita` |
| 61 (0x3D) | solid + name | 2 | 0 | 2 | `npc_wolf`, `npc_hopping_vampire` |
| 62 (0x3E) | solid + name | 3 | 0 | 3 | `v_lion_boss` |
| 63 (0x3F) | solid + name | 3 | 0 | 3 | `npc_baron` |
| 64 (0x40) | solid + name | 3 | 0 | 3 | `muggshot` |
| 65 (0x41) | solid + name | 3 | 0 | 3 | `npc_prince` |
| 66 (0x42) | solid + name | 2 | 0 | 2 | `npc_le_fwee` |
| 67 (0x43) | solid + name | 4 | 0 | 4 | `npc_doctor_m` |
| 68 (0x44) | solid + name | 0 | 1 | 1 | `c_turret` |
| 69 (0x45) | solid + name | 6 | 0 | 6 | `safe_house` |
| 70 (0x46) | solid + name | 5 | 0 | 5 | `v_macro` |
| 71 (0x47) | solid + name | 5 | 0 | 2 | `v_apt_disguise`, `h_hangar_b_roster` |
| 72 (0x48) | solid + name | 33 | 0 | 25 | `c_macro`, `h_macro`, `v_macro` |
| 73 (0x49) | solid + name | 11 | 2 | 4 | `hack_color_lock`, `hack_gravity_well`, `hack_shield_prince` |
| 74 (0x4A) | solid + name | 4 | 0 | 4 | `hack_ship`, `hack_ship_multi` |
| 75 (0x4B) | solid + name | 22 | 0 | 4 | `hack_turret`, `hack_enemy`, `hack_gunship` |
| 76 (0x4C) | solid + name | 16 | 0 | 4 | `hack_bullet`, `hack_ship_bullet`, `hack_black_hole` |
| 77 (0x4D) | solid + name | 3 | 0 | 3 | `hack_lock` |
| 78 (0x4E) | solid + name | 0 | 10 | 3 | `h_ext_hack`, `c_hall_a_webcam`, `c_hall_b_stealing_thiefnet` |
| 79 (0x4F) | solid + name | 4 | 0 | 4 | `o_lock`, `v_cell_lock`, `h_truck_lock` |
| 80 (0x50) | solid + name | 0 | 3 | 3 | `h_wall_safe`, `m_vault_controls` |
| 81 (0x51) | solid + name | 1 | 0 | 1 | `o_truck_player` |
| 82 (0x52) | solid + name | 5 | 0 | 3 | `h_biplane_hero_multi`, `h_biplane_enemy`, `m_biplane_enemy` |
| 83 (0x53) | solid + name | 2 | 0 | 2 | `h_raft` |
| 84 (0x54) | solid + name | 1 | 0 | 1 | `p_jolly_boat` |
| 85 (0x55) | solid + name | 7 | 0 | 5 | `c_rccar`, `m_van_suv`, `m_robot_car` |
| 86 (0x56) | solid + name | 34 | 0 | 34 | `bentley_webcam` |
| 87 (0x57) | solid + name | 2 | 0 | 2 | `dmitri_swimmer` |
| 88 (0x58) | solid + name | 9 | 0 | 4 | `p_boat_hero`, `p_boat_galleon_medium`, `p_boat_deaths_head` |
| 89 (0x59) | solid | 55 | 57 | 34 | `bentley_webcam`, `rock_throw_ammo`, `panda_king` |
| 90 (0x5A) | solid | 16 | 0 | 13 | `chi_ammo`, `ape_missile`, `doberman_grenade` |
| 91 (0x5B) | solid | 154 | 0 | 36 | `bullet_webcam`, `bentley_dart`, `bullet` |
| 92 (0x5C) | solid | 69 | 0 | 16 | `carmelita_missile_playable_charged`, `panda_king_ammo_weak`, `panda_king_ammo_strong` |
| 93 (0x5D) | solid | 20 | 4 | 15 | `wrench_ammo`, `hack_color_key`, `o_cave_murray` |
| 94 (0x5E) | solid | 17 | 2 | 11 | `bouncer`, `c_bouncer_tarp`, `o_bouncer` |
| 96 (0x60) | solid | 42 | 42 | 10 | `m_gauntlet`, `v_gauntlet_rig`, `m_ext_rig` |
| 97 (0x61) | solid | 153 | 295 | 38 | `jt`, `p_at_sea`, `bentley_webcam` |
| 98 (0x62) | solid | 31 | 0 | 1 | `p_at_sea` |
| 101 (0x65) | solid | 36 | 3 | 17 | `o_ext`, `o_arena`, `h_ext` |
| 103 (0x67) | solid | 39 | 0 | 39 | `coin` |
| 104 (0x68) | solid | 4 | 0 | 4 | `i_prize_trainer`, `h_hanky`, `key` |
| 105 (0x69) | solid | 39 | 0 | 39 | `medicine` |
| 106 (0x6A) | solid | 27 | 0 | 27 | `pickpocket_item` |
| 107 (0x6B) | solid | 2 | 0 | 2 | `h_biplane_ammo_crate` |
| 108 (0x6C) | solid | 1 | 0 | 1 | `p_treasure_at_sea` |
| 109 (0x6D) | solid | 1 | 0 | 1 | `mini_bat` |
| 110 (0x6E) | solid | 39 | 0 | 21 | `o_sky_dome`, `h_skydome`, `c_skydome` |
| 111 (0x6F) | solid | 375 | 40 | 36 | `crate_small`, `barrel`, `npc_dingo` |
| 112 (0x70) | solid | 27 | 0 | 27 | `pickpocket_item_dispenser` |
| 113 (0x71) | solid | 68 | 0 | 34 | `bentley_webcam_tossable`, `alarm_clock` |
| 114 (0x72) | solid | 34 | 0 | 34 | `bentley_mine` |
| 115 (0x73) | solid | 34 | 0 | 34 | `guard_trap` |
| 116 (0x74) | solid | 39 | 0 | 39 | `booty` |
| 117 (0x75) | solid | 4 | 0 | 4 | `standard_buffer_effects` |
| 118 (0x76) | solid | 0 | 848 | 37 | `jt`, `bentley`, `murray` |
| 119 (0x77) | solid | 2 | 36 | 34 | `bentley_webcam`, `rc_chopper`, `h_ext_heist_rowboat` |
| 120 (0x78) | solid | 71 | 0 | 35 | `mission_waypoint`, `mission_interior_waypoint`, `mission_start_jt` |
| 121 (0x79) | solid | 4 | 0 | 4 | `vault_tumbler` |
| 122 (0x7A) | solid | 0 | 90 | 37 | `jt`, `npc_dingo`, `ecto_swarmer` |
| 123 (0x7B) | solid | 4 | 0 | 2 | `h_beacon_carmelita`, `h_beacon` |
| 124 (0x7C) | solid | 0 | 34 | 34 | `bentley_webcam` |
| 125 (0x7D) | solid | 36 | 0 | 36 | `compass` |
| 126 (0x7E) | solid | 1 | 0 | 1 | `npc_shark` |
| 127 (0x7F) | solid | 12 | 0 | 6 | `treasure_map_dot_factory`, `treasure_map_x_factory` |
| 128 (0x80) | solid | 32 | 0 | 31 | `shaman_possession_glow`, `c_fb_memory_haze` |
| 129 (0x81) | solid | 0 | 3 | 3 | `hack_ship_multi`, `m_laser_zap`, `m_boss_boss` |
| 130 (0x82) | clip | 174 | 42,717 | 39 | `jt`, `murray`, `bentley` |
| 132 (0x84) | state machine | 14 | 2,832 | 39 | `jt`, `murray`, `bentley` |
| 133 (0x85) | options | 91 | 102 | 36 | `bentley_webcam`, `bentley`, `aim_manager_guard_weapon` |
| 134 (0x86) | options | 2,304 | 72 | 38 | `o_macro`, `v_macro`, `m_macro` |
| 135 (0x87) | command script | 5,863 | 548 | 39 | `c_macro`, `h_macro`, `o_macro` |
| 136 (0x88) | options | 34 | 49 | 36 | `jt`, `v_gauntlet`, `v_ext_follow` |
| 137 (0x89) | options | 18 | 1 | 9 | `p_dive`, `m_underwater`, `p_ext` |
| 138 (0x8A) | options | 563 | 300 | 38 | `h_macro`, `c_macro`, `o_macro` |
| 139 (0x8B) | options | 7 | 484 | 39 | `bentley_dart`, `booty`, `ninja_ball` |
| 141 (0x8D) | options | 40 | 0 | 9 | `v_ext_disguise`, `p_ext_love`, `v_ext_rig` |
| 142 (0x8E) | options | 0 | 500 | 31 | `npc_dingo`, `carmelita`, `npc_doctor_m` |
| 143 (0x8F) | key list | 114 | 0 | 38 | `h_macro`, `c_macro`, `o_macro` |
| 144 (0x90) | options | 5 | 0 | 5 | `v_macro` |
| 145 (0x91) | options | 1,934 | 0 | 10 | `o_ext_pz`, `v_ext_pz`, `p_ext_pz` |
| 147 (0x93) | options | 79 | 6 | 38 | `h_macro`, `c_macro`, `v_macro` |
| 148 (0x94) | options | 0 | 1,180 | 37 | `jt`, `penelope`, `bentley` |
| 149 (0x95) | options | 0 | 50 | 23 | `npc_doctor_m`, `ecto_swarmer`, `npc_kangaroo` |
| 150 (0x96) | options | 0 | 101 | 37 | `jt`, `bentley`, `murray` |
| 151 (0x97) | stream list | 2 | 36 | 37 | `jt`, `all_wayback` |
| 152 (0x98) | options | 1 | 0 | 1 | `c_ext_rccar` |
| 153 (0x99) | options | 6 | 0 | 6 | `v_ext_treasure_map`, `o_ext_treasure_map`, `h_ext_treasure_map` |
| 154 (0x9A) | options | 1 | 0 | 1 | `p_at_sea_rig` |
| 156 (0x9C) | options | 44 | 0 | 35 | `v_ext_rig`, `o_ext_rig`, `h_ext_rig` |
| 157 (0x9D) | pair list | 15 | 0 | 1 | `all_wayback` |
| 158 (0x9E) | screen object | 1,405 | 584 | 39 | `stock_swf`, `jt`, `stock_objects` |
| 159 (0x9F) | screen object | 92 | 0 | 33 | `h_macro`, `o_macro`, `c_macro` |
| 160 (0xA0) | screen object | 45 | 0 | 24 | `h_macro`, `m_macro`, `p_macro` |
| 161 (0xA1) | screen object | 39 | 0 | 39 | `stock_swf` |
| 162 (0xA2) | screen object | 1 | 0 | 1 | `episode_chooser` |
| 163 (0xA3) | screen object | 27 | 147 | 38 | `jt`, `bentley`, `murray` |
| 164 (0xA4) | screen object | 13 | 0 | 13 | `h_macro`, `p_macro`, `v_ext_cops_robbers` |
| 165 (0xA5) | screen object | 8 | 0 | 8 | `h_macro` |
| 166 (0xA6) | screen object | 2 | 4 | 4 | `p_boat_hero`, `p_at_sea_multi` |
| 167 (0xA7) | screen object | 39 | 0 | 39 | `stock_swf` |
| 168 (0xA8) | options + name | 440 | 0 | 38 | `h_macro`, `c_macro`, `o_macro` |
| 169 (0xA9) | task | 5,519 | 0 | 38 | `c_macro`, `h_macro`, `o_macro` |
| 170 (0xAA) | options + name | 853 | 0 | 38 | `h_macro`, `o_macro`, `c_macro` |

Totals: 46,221 top-level records and 79,472 nested ones. [D] Every class with a solid layout carries drawable
geometry ([geometry.md](geometry.md)). Name strings of the "solid + name" classes include `jt` (class 52, Sly),
`murray` (53), `bentley` (54), `shaman` (55, the Guru), `panda_king` (56), `penelope` (57), `guard` (58),
`dimitri` (59), `carmelita` (60). [D]

Sly 2 → Sly 3 class numbers for the main families [D for the layouts, R for the pairing]: placement 6 → 8; solid 48
→ 47; Sly 53 → 52; Murray 54 → 53; Bentley 55 → 54; guard 58 → 58; clip 109 → 130; state machine 111 → 132.

## 4. Object identity

As Sly 2: object IDs are `s32` and not unique across groups; references are by ID within the owning group or copy.
The script module indexes the group's script block. [R]

## 5. Differences from Sly 2 at a glance

| Item | Sly 2 | Sly 3 |
|---|---|---|
| Record header | kind and level-row list | mode byte, world filter and condition list |
| Option table | `0x2AC880` | `0x32CA80`, 2,044 entries; more value types (3, 4, 11, 12, 22, 24 and up) |
| Remove-at-start option | 26 | 27 |
| Class numbers | 2 to 133 | 2 to 170, renumbered |
| Placement | packages by set ID, patch targets 5 and 6 | groups by id, bind targets 7 and 8 |
| New families | | effects with components, screen objects with element sets, command scripts, tasks with lists |

## 6. Test vectors

- The option table starts at file offset `0x22DA80` (virtual address `0x32CA80` − `0x100000` + `0x1000`). Entry 27 is
  at file offset `0x22DA80 + 27 × 40 = 0x22DEB8` and starts `12000000 01440000 080b1000 00000000 800b1000` (value
  type 18, flags `0x4401`, load-time handler `0x100B08`). Entry 26, at `0x22DE90`, has the same type and flags but
  handler `0x100A88`: `12000000 01440000 880a1000 00000000 880a1000`. [X]
- Class table entry 0 (file offset `0x2419E0`): `00000000 00000000 f9000000 10000000 00000000`; entry 1: base
  `0x3409E0`, class 1, last 2, size `0x5000`. [X]
- World slot 5 (v_apt), level group `v_apt`: 198 top-level records (class 8 × 160, 4 × 35, 48 × 1, 136 × 1,
  5 × 1). Its first record, at `0x6C4C86`, starts `00 30 00 01 00 00 00 ff ff 65 22 06 00`: mode 0, class 48,
  object ID 1, no script module, body size 402,021. [D]
- The same group's first placement (object 139,880) names group id 149,054 (`v_alarm_light`), mode −1 (a copy),
  parent −1, zone mask 255, no binds, body 66 bytes. [D]

## 7. Open questions

- The layout of the one class-157 body that does not follow the pair-list layout (mode 2; see classes/effects-logic-ui.md 5 for what is known).
- The roles of most classes (the table gives owners, not meanings).
- The exact rule for mode-1 records whose filter fails but that a script later needs.
