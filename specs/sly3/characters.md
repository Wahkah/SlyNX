---
id: sly3-behaviour-characters
title: Sly 3 playable characters - the gang and the recruits
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464) image; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338; all 39 worlds walked
    used_for: >-
      which character classes occur in which world; clip counts per character group; checkpoint records; the item
      table at 0x343098 decoded record by record with names from the world string table (Venice hub); the lookup
      tables and constants marked [X], located by the code that reads them
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/transition.cpp, sly.cpp, gang.cpp, damage.cpp, game/sly3.h, game/gen/globalmap.inc
    used_for: >-
      the arrival rule (checkpoint matching, default arrival id, choice of the current player), which loader reads
      which table, Sly 2 to Sly 3 address correspondences used as first guesses; marked [R]. That port carries Sly 2's
      gang code with Sly 3 numbers; it does not implement the recruits, so it is not a source for their moves
  - kind: reference-project
    what: earlier research in this workspace (Games/Sly3-RE-notes.md and tools/sly3_world.py, sly3_anim.py, 2026-11)
    used_for: the world walker and clip reader used for the counts; the loader addresses of the recruit classes
  - kind: observation
    what: general knowledge of the game as played
    used_for: the plain-language descriptions of the recruits' roles, marked [O], to be checked in an emulator
  - kind: executable
    what: SCUS_974.64 machine code read on 2026-11-18 (item-start routines, move-state fields)
    used_for: section 9
  - kind: executable
    what: >-
      SCUS_974.64 machine code read on 2026-11-18 (Bentley's item start 0x140128, the interaction start, the guard
      hit handler 0x251B40, guard modes 0x2514D8 and 0x2517C0) and the Outback hub's Bentley group (clips 374, 377,
      693; package psys_bentley_scale)
    used_for: section 10
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release/aihit.cpp, guard.cpp, damage.cpp, asegevent.cpp
    used_for: what the guard modes do (target lists, damage scaling) and event kinds 7 and 8; marked [R] in section 10
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 playable characters

The delta of [../sly2/characters.md](../sly2/characters.md) for Sly 3. Read that first: what is not listed here is
taken as unchanged until observation says otherwise. Tags: [D] disc data, [X] executable data at the given virtual
address (file offset = address − 0x100000 + 0x1000, disc.md 3; read only after checking the executable's
SHA-256), [R] reference behaviour, [O] observation, [I] inferred. Class numbers are Sly 3's
([object-stream.md](object-stream.md) 3.2).

## 1. Who is where [D]

| Class | Character | Name string of the group | Loaded by its own actor loader |
|---:|---|---|---|
| 52 | Sly | `jt` | yes |
| 53 | Murray | `murray` | yes |
| 54 | Bentley | `bentley` | yes |
| 55 | the Guru | `shaman` | yes |
| 56 | the Panda King | `panda_king` | yes |
| 57 | Penelope | `penelope` | yes |
| 59 | Dimitri | `dimitri` | no: the guard family's loader (as classes 58 and 61 to 67) [R] |
| 60 | Carmelita | `carmelita` | no: the guard family's loader [R] |
| 87 | Dimitri swimming | | a vehicle-like class (vehicles spec) |

Worlds that contain each (records of any mode, so some are present only under a job's conditions):

| World | Characters |
|---|---|
| 2 i_trainer | Sly, Murray, Bentley |
| 3 v_ext | Sly, Murray, Bentley, Carmelita, safe house |
| 4 v_canal | Sly, Bentley, Carmelita |
| 5 v_apt, 6 v_gauntlet | Sly, Murray, Bentley |
| 7 v_interpol | Sly, Carmelita, Dimitri |
| 8 o_ext | Sly, Murray, Bentley, Guru, Carmelita, safe house |
| 9 o_quarry | Sly, Murray, Bentley, Guru, Carmelita |
| 10 to 14 (Outback interiors) | Sly, Murray, Bentley |
| 15 h_ext | Sly, Murray, Bentley, Guru, Carmelita, safe house |
| 16 h_hotel | Sly, Murray, Bentley, Dimitri |
| 18 h_hangar_b | Sly, Murray, Bentley, Guru |
| 19 h_hangar_c | Sly, Murray, Bentley, Penelope |
| 17, 20, 21 | Sly, Murray, Bentley |
| 22 h_hack | none (Bentley's hacking game) |
| 23 c_ext | all eight, safe house |
| 24 c_intro | all but Carmelita and Dimitri |
| 27 c_apt | Bentley, Panda King |
| 29 c_hall_b | Sly, Murray, Bentley, Guru, Panda King, Carmelita |
| 30 c_tilt_hall | Sly, Murray, Bentley, Penelope |
| 25, 26, 28 | Sly, Murray, Bentley |
| 31 p_ext | Sly, Murray, Bentley, Penelope, safe house |
| 32 p_at_sea | all but Carmelita, safe house |
| 33 p_dive | Sly, Bentley, Dimitri swimming |
| 34 p_island_map | Sly, Murray, Bentley, Dimitri, Penelope |
| 35 m_ext | all eight |
| 36 m_underwater | Sly, Dimitri, Dimitri swimming |
| 37 m_vault | Sly, Murray, Bentley |
| 38 m_gauntlet | Sly |
| 39 m_boss | Sly, Murray, Bentley, Carmelita |

## 2. Where characters start in a world [R] [D]

The game does **not** start a character at its own record's transform, except as a fallback:

1. Every world load is asked for an **arrival point**, an id. Exits and tasks name one; the boot, the world menu and
   a load with nothing named use the **default arrival id 424** (`0x1A8`; Sly 2 used 393). A second id, 425, is
   also used by checkpoints [R].
2. The arrival point is looked up among the **checkpoints** (class 40 and the classes derived from it): a
   checkpoint matches if its OID equals the id, or if its scripts have added the id to the ids it answers to (they
   add 424 and 425). If an exit or task also names a key object, only checkpoints inside that object count. [R]
   Example [D]: v_ext has a checkpoint with OID **424** (in group `v_ext_rig`, the safe-house checkpoint) among its
   35 checkpoints; v_apt has six (OIDs 139910, 139967, 140036, 140827, 140831, 140836) and none with OID 424, so v_apt
   is entered through a point that its exits from v_ext name.
3. **No matching checkpoint**: every playable character stays at its record's placement, which for the gang is
   usually its group's local origin. That is why loading v_apt with the default point leaves the crew at their
   local origins: in the game that load never happens without a named point. A port that loads a world directly
   should name one of the world's checkpoints (the first, or the one its exits use).
4. **The current player** is chosen when none is current: the progress value "current player object" if set (the
   save keeps it), otherwise Sly, then Bentley, then Murray (the first of those present). It is enabled and placed
   at the checkpoint's position and orientation, unless the checkpoint's own pair list places that character at
   another object. Every other playable character that is not enabled is removed from the world. [R]
5. A checkpoint's arrival handler (a script) may then hand control to another object: at a hub's default point the
   **safe house** (class 69) becomes the current player once the episode's first jobs are done, and the player picks
   a character there (section 6). [R]

## 3. Executable tables and constants

### 3.1 The item table [X]

Virtual address **`0x343098`**, **67 records of 60 bytes** (Sly 2: 43 of 32 at `0x002BCD00`). Read by the shop and
gadget code (15 code sites reference the address [D]).

| Offset | Type | Meaning |
|---:|---|---|
| +0 | `s32` | button use: 2 press, 1 hold, −1 passive (as Sly 2) |
| +4 | `s32` | owner: **24 Sly, 25 Bentley, 26 Murray** (confirmed by the names), 28 four unnamed records, −1 none |
| +8 | `s32` | price in coins |
| +12 | `s32` | gadget-power cost per use (1 on the held items 32 to 35: the entry cost) |
| +16 | `s32` | power drained per second while held (3, 2, 3, 5 on items 32 to 35; 0 elsewhere) [X: [gadgets.md](gadgets.md) 2] |
| +20 | `s32` | episode from which it is sold (3 Venice … 7 pirates; −1 not sold) |
| +24 | `s32` | 14 or 15 on sold items, −1 otherwise (which shop page or price tier [I]) |
| +28, +32 | `s32` | string ids of the name and the description (the world's string table) |
| +36 | `s32` | string ID of the usage text (286 to 301; [gadgets.md](gadgets.md) 1), −1 on the attack upgrades |
| +40 | `s32` | an object or effect id (1967 to 1985) on 18 records, else 0 |
| +44 to +56 | 4 × `s32` | two (id, count) pairs, e.g. (312, 4, 312, 4) on Smoke Bomb [meaning open] |

Records [D] (name strings as stored):

| # | Owner | Button | Price | Power | Ep. | Name |
|---:|---|---:|---:|---:|---:|---|
| 0 to 5 | — | | | | | placeholders |
| 6 | Bentley | 2 | — | 0 | — | Trigger Bomb |
| 7 | Bentley | 2 | — | 0 | — | Fishing Pole |
| 8 | Bentley | 2 | 200 | 5 | 3 | Alarm Clock |
| 9 | Bentley | 2 | 400 | 15 | 3 | Adrenaline Burst |
| 10 | Bentley | 2 | 600 | 5 | 4 | Health Extractor |
| 11 | Bentley | −1 | 300 | 0 | 4 | Hover Pack |
| 12 | Bentley | 2 | 600 | 10 | 5 | Insanity Strike |
| 13 | Bentley | 2 | 400 | 0 | 5 | Grapple-Cam |
| 14 | Bentley | 2 | 800 | 5 | 6 | Size Destabilizer |
| 15 | Bentley | 2 | 1200 | 25 | 6 | Rage Bomb |
| 16 | Bentley | 2 | 1400 | 20 | 7 | Reduction Bomb |
| 17 | Murray | 2 | — | 0 | — | Be The Ball |
| 18 | Murray | 2 | 300 | 10 | 4 | Berserker Charge |
| 19 | Murray | −1 | 400 | 0 | 4 | Juggernaut Throw |
| 20 | Murray | 2 | 600 | 15 | 5 | Guttural Roar |
| 21 | Murray | 2 | 800 | 10 | 5 | Fists of Flame |
| 22 | Murray | 2 | 1000 | 25 | 6 | Temporal Lock |
| 23 | Murray | 2 | 1200 | 20 | 6 | Raging Inferno Flop |
| 24 | Murray | 2 | 1400 | 25 | 7 | Diablo Fire Slam |
| 25 | Sly | 2 | 200 | 5 | 3 | Smoke Bomb |
| 26 | Sly | 2 | 400 | 0 | 3 | Combat Dodge |
| 27 | Sly | −1 | 300 | 0 | 4 | Paraglide |
| 28 | Sly | −1 | 400 | 0 | 5 | Silent Obliteration |
| 29 | Sly | 2 | 1000 | 0 | 6 | Feral Pounce |
| 30 | Sly | 2 | — | 34 | — | Mega Jump |
| 31 | Sly | 2 | 300 | 10 | 3 | Knockout Dive |
| 32 | Sly | 1 | 1000 | 1 | 5 | Shadow Power Level 1 |
| 33 | Sly | 1 | 1200 | 1 | 6 | Thief Reflexes |
| 34 | Sly | 1 | 1600 | 1 | 7 | Shadow Power Level 2 |
| 35 | Sly | 1 | 600 | 1 | 4 | Rocket Boots |
| 36 | Sly | 1 | — | 0 | — | Treasure Map |
| 37 | Sly | 2 | — | 20 | — | (placeholder strings `ENGLISHpowerup_shield_name`) |
| 38 to 40 | Sly | 2 | — | 0 | — | Venice Disguise, Photographer Disguise, Pirate Disguise |
| 41 to 43 | Sly | −1 | —, 300, 800 | 0 | —, 4, 5 | Spin Attack Level 1, 2, 3 |
| 44 to 46 | Sly | −1 | 400, 800, 1000 | 0 | 4, 6, 7 | Jump Attack Level 1, 2, 3 |
| 47 to 49 | Sly | −1 | 200, 600, 1000 | 0 | 3, 5, 6 | Push Attack Level 1, 2, 3 |
| 50 to 53 | 28 | −1 | — | 0 | — | no strings |
| 54 to 60, 62 to 66 | — | −1 | — | 0 | — | no strings, only the +40 to +56 fields |
| 61 | — | | | | | placeholder |

"—" price: 0 (not sold); "—" episode: −1. Check vector: record 8 = (2, 25, 200, 5, 0, 3, 14, 217, 218, 299, 0, 0, 0,
0, 0) "Alarm Clock"; record 25's last five words (1971, 312, 4, 312, 4) [D].

### 3.2 Lookup (parts) tables [X]

Each character's loader binds a table of 12-byte records: `u32` flags, `s32` OID, and a third word that is an
in-memory field offset of the original program (ignore it; the port keeps its own mapping by entry index). Sly 2
had the same scheme ([../sly2/animation.md](../sly2/animation.md) 9). Found by reading which table each loader
passes, with its count:

| Table | Address | Records | Read by |
|---|---|---:|---|
| every actor (first) | `0x342DC0` | 1 | the shared actor set-up |
| every playable actor | `0x342DD0` | 59 | the shared set-up (Sly 2: 53 at `0x002BC9F0`) |
| Sly | `0x345140` | 106 | Sly's loader |
| Sly (second) | `0x345638` | 11 | Sly's loader (Sly 2: 18 at `0x002BF1B0`) |
| Murray | `0x345FD8` | 127 | Murray's loader |
| Bentley | `0x342890` | 84 | Bentley's loader |
| the Guru | `0x349460` | 43 | the Guru's loader |
| the Panda King | `0x346650` | 49 | the Panda King's loader |
| Penelope | `0x3468A0` | 53 | Penelope's loader |

Dimitri and Carmelita use the guard family's tables (guards spec). Check values: `0x342890` starts (768, 449),
(768, 447), (768, 451); `0x345FD8` starts with the same three; `0x342DD0` starts (272, 731), (768, 729) [D].

### 3.3 Constants found [X]

Sly 2's constants that recur in Sly 3, located by value and confirmed to be read by code:

| Value | Sly 3 address | Sly 2 address | Use (characters.md of Sly 2) |
|---|---|---|---|
| (1000, 900, 950) and (1000, −900, 950) | `0x323630`, `0x323640` | `0x002BFAE0`, `0x002BFAF0` | knocked aside by Murray |
| (0, 0, −1000) | `0x323670` | `0x002BFB10` | crushed down |
| (0, 0, 10), (0, 0, 40000) | `0x31FC80`, `0x31FC90` | `0x002BC970`, `0x002BC980` | dart aim line near and far depths |
| (0, 0, −750) | `0x323560` | `0x002BFA00` | costume parts' gravity during damage |

Not found as stored data in Sly 3 (moved into code or changed): the crossbow swing point (175, 0, 0), the Thunder
Flop window (0.15, 0.1) and its (0.75, 2.5, −500), Sly's physics scale 1.6, the thrown knock-back (600, 800) [D].
Bentley swings no crossbow in Sly 3 (he attacks from his wheelchair, section 5) [O].

## 4. Clip counts [D]

"Clips of a character" counts the clip records (class 130) in the **character's group** (the group named by its
string, e.g. `jt`), at any depth (inside its tree, its state machines and proxies), that **have data** (not
deferred) **and drive at least one transform of the character's own tree**. Clips whose data is deferred, and clips
with data but no channels (event-only clips), are left out:

| Group | Clip records | Deferred | With data | No channels | **Counted** |
|---|---:|---:|---:|---:|---:|
| `jt` (Sly) | 363 | 13 | 350 | 8 | **342** |
| `murray` | 261 | 0 | 261 | 11 | **250** |
| `bentley` | 210 | 0 | 210 | 4 | **206** |
| `carmelita` | 136 | 0 | 136 | 2 | **134** |
| `doberman_guard` | 90 | 0 | 90 | 0 | 90 |
| `shaman` (Guru) | 94 | 0 | 94 | 1 | 93 |
| `panda_king` | 107 | 0 | 107 | 5 | 102 |
| `penelope` | 120 | 0 | 120 | 5 | 115 |
| `dimitri` | 83 | 0 | 83 | 0 | 83 |

The gang's counts are the same in i_trainer, v_ext and v_apt (checked); the recruits' are from m_ext, and
Carmelita's from v_ext and m_ext. So the 350 found in v_apt is the "with data" column: the 8 extra clips carry only
events. The figures in [animation.md](animation.md) 5 are the "counted" column; the word "Venice hub" there means
"any world checked" (Sly's figure was first taken in i_trainer).

## 5. The core three: what differs from Sly 2

Sources: the item table [D], the disc's strings [D], observation [O]; the moves' mechanics are Sly 2's unless
listed.

- **Sly** [D] [O]:
  - The cane attacks are a **combo system**: Triangle presses build a combo that a final button finishes. The
    item strings give the inputs: spin attack Triangle then Square, jump attack Triangle then Cross, push attack
    Triangle then Circle; levels 2 and 3 add one and two more Triangle presses first (Level 1 of the spin attack is
    owned from the start: price 0). [D]
  - New items: Rocket Boots (held), Shadow Power levels 1 and 2 (held: invisibility, then attacking while
    invisible), the Treasure Map (held, follows a trail in the hubs with a treasure map, class 127), three
    disguises (Venice, China, pirates), and an unused shield power-up. Thief Reflexes, Smoke Bomb, Combat Dodge,
    Paraglide, Silent Obliteration, Feral Pounce, Knockout Dive and Mega Jump carry over from Sly 2. [D]
  - **Combo state and timing** [X]: Sly's state field is actor +0x19E0, with states −1 to 23. Two jump tables map
    them to the actor's generic states used by the combo code: `0x003228E0` (state → generic; 25 entries for −1 to
    23) and `0x003228B0` (generic → state; 12 entries for −1 to 10). Generic 9, the combo attack, is **Sly's state
    23** and nothing else maps to it. (Generic 0 = state 0, 1 = 3, 2 = 8, 3 = 5, 4 = 14, 5 = 15, 6 = 16, 7 = 18,
    8 = 9, 10 = 6; every other state reads as 11.)
  - **Combo definitions** are objects of class 148 (1,180 records in 37 worlds) [D]. Their start-up code
    (`0x00139740`) sets the **press window to 0.35 s** (field +0x70, `0x3EB33333`) and a steering speed of 800
    (+0x74) [X]. Option 1668 would set the window; no record uses it, so 0.35 s is the time allowed for the next
    press everywhere [D]. Options: 1659 the button that continues the combo (`0x80` ×610, `0x10` ×248, `0x40`
    ×116), 1667 a priority (2 to 5), 1673 / 1676 / 1679 the clips (standing, while moving, follow-up), 1680 and
    1681 two speed factors (2.2 / 1.6, 1.6 / 1.15, …), 1683 the abilities one of which must be owned (44, 45, 46, …)
    [X for the fields; R for the meanings]. Runner states [R]: off, attack, attack while moving (an overlay),
    follow-up after a hit, recovery.
- **Bentley** [D] [O]: in a **wheelchair**: no crossbow swing; his melee items work through the chair (Insanity
  Strike, Size Destabilizer: "whacking them with your wheelchair"). New: Fishing Pole (pickpocketing at range, given
  free), Grapple-Cam (a remote camera), Rage Bomb; Alarm Clock moves to him; Hover Pack is passive (−1). Trigger
  Bomb is his free starting item. His darts keep Sly 2's aim depths (3.3). He still hacks (classes 73 to 77).
- **Murray** [D] [O]: new Be The Ball (free); Temporal Lock moves to him; Atlas Strength is gone from the table.
  His knock-aside vectors are unchanged (3.3).
- **Health and damage**: Sly 2's rules are taken as unchanged (capacity 40, flashing 2 s after a hit, scaling by the
  job's difficulty help) [R, Sly 2 code carried over by the reference]; Sly 3's own values are not yet confirmed in
  its executable (open question 2).

## 6. Switching and when recruits are played [R] [O]

- **In a hub** the player picks one of the core three at the safe house (class 69: present in v_ext, o_ext, h_ext,
  c_ext, p_ext, p_at_sea); the choice is the "current player" progress value, saved and restored (section 2). [R]
  [O]
- **No free switching in a hub** [X, partial]. No gameplay code tests a d-pad button through the game's button
  test (`0x001A7A80`): no call passes a d-pad mask (`0x1000`, `0x2000`, `0x4000`, `0x8000`). The places that hand
  player control to another character (`0x001F4B00`'s 23 callers) are:
  - tasks and scripts (command `0x13`, the game object's method 983);
  - the update shared by classes 86 and 170 (`0x002859F0`, vtable slot 18 / 72), which reads L3 `0x200` and Circle
    `0x20`;
  - minigames (the lock) and vehicles.

  The safe house's own selection path was not traced here.

  So switching outside the safe house is not the game's rule; a d-pad switch in a port is an addition [I: other
  input paths were not all traced].
- **In a job**, the task makes its character the current player (tasks set and enable a player object; the task
  spec owns the details) [R]. The recruits become the player only this way: each recruit is present only in the
  worlds of section 1, mostly under job conditions, and has no safe-house entry. **Recruits are not playable outside
  their jobs** [O]; they appear in hubs as non-player characters (bystanders and story roles) after joining [O].
- Who is played where, from the worlds and the story [O] [I, to confirm in play]:

  | Recruit | Worlds present (section 1; not all are playable there) | Role in play |
  |---|---|---|
  | the Guru | Outback (o_ext, o_quarry), later Holland and China | small and agile; his own lookup table (43 entries) and 93 clips; climbs and rides on guards to control them [O] |
  | Penelope | Holland (h_hangar_c), China (c_tilt_hall), pirates | her jobs drive the remote-control vehicles (class 85 RC car and helicopter) and use her own on-foot moves (53 entries, 115 clips) [O] |
  | the Panda King | China (c_apt, c_hall_b) | fireworks attacks and his own moves (49 entries, 102 clips) [O] |
  | Dimitri | v_interpol, h_hotel, p_at_sea, p_dive, p_island_map, m_ext, m_underwater | played mainly diving (class 87, p_dive and m_underwater); on foot a guard-family character (83 clips) [O] |
  | Carmelita | Venice (v_ext, v_canal, v_interpol), the hubs, Kaine Island (m_ext, m_boss) | a guard-family character, mostly an opponent or bystander; her playable job uses her shock pistol (shooting), late in the game [O, to confirm which world] |

- **Health of the recruits**: not in the data found so far; each has the shared actor table's parts (3.2), so the
  Sly 2 health counter applies by default [I] (open question 2).

## 7. Test vectors [D]

- Item table: record 8 above; record 41 = (−1, 24, 0, 0, 0, −1, −1, 267, 268, −1, …) "Spin Attack Level 1", record 30
  = (2, 24, 0, 34, 0, −1, −1, 227, 228, 287, 1968, 324, 4, 324, 4) "Mega Jump".
- Lookup tables: the counts of 3.2 (each loader passes exactly that count).
- Constants: `0x323630` = 1000, 900, 950; `0x323670` = 0, 0, −1000; `0x31FC90` = 0, 0, 40000.
- Clip counts: section 4.
- Arrival: v_ext has a class-40 record with OID 424 in group `v_ext_rig`.

## 8. Open questions

1. The recruits' moves in detail (states, inputs, speeds): no reference implements them; they need observation in an
   emulator and a reading of their loaders' code (the Guru `0x22C260`, Panda King `0x1E9438`, Penelope `0x1EFF30`).
2. Health: Sly 3's capacity and hit amounts, per character, read from its own executable.
3. The item table's fields +24, +40 to +56 and the owner code 28.
4. How a job hands control to a recruit and back (task records, missions spec).
5. Which lookup entry is which move, per character.

## 9. Executable findings: item states [X]

From the machine code of SCUS_974.64 (2026-11-18; details and addresses in [gadgets.md](gadgets.md) 6). The move
state is the word at actor +6624 for Sly (sub-state at +6632) and at +5408 for Murray and Bentley [X].

| Character | Item | State entered |
|---|---|---|
| Sly | Rocket Boots (35) | 19, sub-state 6 (start-up), then 7 (run) |
| Sly | Treasure Map (36) | 24, sub-state 8 |
| Sly | Disguises (38 to 40) | no state change; costume parts 1881, 1882, 1883 |
| Murray | Be The Ball (17) | 22 (ball mode at +6868) |
| Bentley | Fishing Pole (7) | 16 |
| Bentley | Grapple-Cam (13) | 14 |

Each character's item-start routine is the entry at +916 of its dispatch table: Sly `0x001AF6C0`, Murray
`0x001E25B0`, Bentley `0x00140128` [X]. Sly's animation for states 18 and up comes from his slot list at +6224 (one
word per state) [X].

## 10. Executable findings: Bentley's chair melee (items 12 and 14)

From the machine code of SCUS_974.64 (hash checked) and Bentley's group in the Outback hub (`o_ext`), 2026-11-18.
Mode numbers below are Sly 3's own; the reference port carries Sly 2's numbering for the same modes.

**Starting the item** [X]. Bentley's item-start routine (`0x00140128`) dispatches items 4 to 16 through the table at
`0x0031FF20`. Items 12 (Insanity Strike) and 14 (Size Destabilizer) share one branch: it records the item number as
the pending action (actor +2556), takes the clip bound to the item from Bentley's parts and starts an interaction of
kind 11 with no target. When the interaction begins, the clip is applied to Bentley (start 0, speed 1) and the item
number is kept as the playing action (+5116) until the clip is retracted; then it is cleared to −1 [X]. Both items
are press items on a gadget slot with power costs 10 and 5 (section 3.1) [D].

**The clips** [D]. Bentley's lookup table (`0x342890`, section 3.2) binds them: record 81 (OID 377) to item 12 and
record 80 (OID 374) to item 14 (record 82, OID 693, is a third clip of the same family).

| Item | Clip OID | Length | Channels | Events |
|---|---:|---:|---:|---|
| Insanity Strike (12) | 377 | 0.3333 s | 0 | none |
| Size Destabilizer (14) | 374 | 0.3333 s | 0 | apply list: kind 7 on object 7027; retract list: kind 8 on 7027 |

Event kinds 7 and 8 add and remove an object by OID [R]. Object 7027 is a placement (class 8) of the package
`psys_bentley_scale` (set ID 7029), a particle effect, so the Size Destabilizer shows its scaling particles for the
clip's third of a second [D]. Neither clip moves Bentley (no channels) [D].

**What the hits do to guards** [X]. The guard hit handler (`0x00251B40`) turns two hit kinds into modes:

| Hit kind | Guard mode | Effect | Ends after |
|---|---:|---|---|
| `0x400000` | 14 | the guard's scale controller is set to 0.5 (vector at `0x003272E0`); in the reference port a shrunk guard takes 4 × the damage and deals half [R] | 10 s (`0x002517FC`) |
| `0x800000` | 12 | the guard adds guards (class 58) to its targets and attacks them [R]; a guard in mode −1 that is hit by a guard in mode 12 also enters mode 12 | 30 s (`0x00251818`) |

Mode 16 behaves as 14 and mode 15 as 12 [X]. A mode is not entered twice (the timer is not extended) and returns to
−1 when its time is up (checked at `0x002517C0`) [X]. Modes 12 and 15 are refused for class 59 (table at
`0x003498F8`); modes 14, 16 and 22 are always allowed [X]. The guard-mode routine is at `0x002514D8`.

*Interpretation* [I]: Size Destabilizer's hit is kind `0x400000` (shrink) and Insanity Strike's is `0x800000` (the
guard turns on other guards, and it spreads). The code that sends these two hit kinds was not found as an immediate
in the executable; it is probably taken from data (an option of the hitting object) and is the remaining step to
confirm this pairing, together with the reach of the hit.

## 11. Stick input [X]

Sly 3 reads the sticks with Sly 1's routine (`0x001A8930`, called for each stick by the pad update `0x001A8B88`:
bytes 38/39 the left stick, 36/37 the right). Steps 1 to 4 are identical:

1. **Bytes to axes.** Each stick gives two bytes, b_x and b_y (0 to 255). x = 2·b_x / 255 − 1 and
   y = 1 − 2·b_y / 255, so up is positive y.
2. **Waking up.** A stick reads (0, 0) until it is first seen in use: its bytes change from one frame to the next,
   or either byte leaves the window 96 … 160. From then on it is read every frame. The flag is kept per stick.
3. **Self-calibrating range.** The routine keeps, per stick, a range [lo, hi] for each of four directions d:
   (1, 0), (0, 1), (0.7, 0.7) and (0.7, −0.7) (the table at `0x00345108`). The ranges start at **[−0.75, 0.75]** when
   the pad is (re)connected (`0x001A88CC`). Each frame, for each direction:
   - p = x·d.x + y·d.y;
   - if p ≥ 0, hi = max(hi, p), else lo = min(lo, p);
   - r = p / hi (or p / lo when p < 0).

   The raw deflection is e = the largest r of the four (0 when none is positive).
4. **Dead zone and response.** u = clamp(2·(e − 0.35), 0, 1): **no response below 35 % of the calibrated range**,
   linear above it, and **full deflection from 85 %**. With the starting range this is 0.2625 to 0.6375 of the raw
   axis.
5. **Output (differs from Sly 1).**
   - When u ≥ s − 0.2 (s is a remembered value, the previous frame's u in the normal case), s becomes u, and the direction is (x, y) × u × |v|, with
     |v| = √(x² + y²). It is **not normalised** as in Sly 1, so a partly deflected stick gives a smaller vector
     than u alone [X: `0x001A8B24` … `0x001A8B50`].
   - When u falls faster than that, the remembered value s becomes s − 0.2 (never below 0) and the previous
     direction is scaled by u / s. So the direction vector fades out over a few frames when the stick is released.
   - The reported deflection is always u.

## 12. Lookup slots by state: which clip each state plays

### 12.1 Method [X] [R]

Each lookup record (3.2) ends with the field where the game stores what it found. The reference's mapping file pairs
every Sly 3 record with the Sly 2 field that holds the same thing [R]. A slot is named here only when that Sly 2
field's role is established. Walk and run were traced in the executable (12.3).

### 12.2 Slot → state → clip (object IDs) [X] [R]

| Slot (Sly 3 field) | State | Sly (`0x345140`) | Bentley (`0x342890`) | Murray (`0x345FD8`) |
|---|---|---|---|---|
| +0x1134 | **idle**: the idle state machine (Sly 2 +0x1028) | 172 (record 0) | 451 (2) | 451 (2) |
| walk gait, clips 0 and 1 (gait at +0x1470: clips at +0x1484, +0x14C4) | **full-speed move** at weight 1 (Sly 2's state-1 run; 12.3 corrects the earlier "walk" label) | 173, 174 (records 1, 2) | 449, 447 (0, 1) | 449, 447 (0, 1) |
| run gait, clips 0 and 1 (Sly +0x1C90, Bentley +0x1770, Murray +0x1680) | **stick-driven move** (the gait `0x00173068` returns; 12.3) | 307, 308 (second table `0x345638`, records 6, 7) | 535, 534 (26, 27) | 535, 534 (33, 34) |
| Murray's third gait (+0x15A0) | **carry** gait (12.3) [D] | — | — | 528, 529 (17, 18) |
| jump block +0x3E0 … +0x3F0 | **jump**: five segments, by the jump's sub-kind 0 to 4 | 200, 204, 203, 201, 202 (records 15, 19, 18, 16, 17) | 696, 700, 699, 697, 698 (33, 37, 36, 34, 35) | 696, 700, 699, 697, 698 (53, 57, 56, 54, 55) |
| fall block (Sly, Murray +0x3F4 … +0x404; Bentley +0x408, +0x420, —, +0x40C, +0x410) | **fall**: the same five sub-kinds | 208, 212, 211, 209, 210 (20, 24, 23, 21, 22) | 707, 710, —, 708, 709 (38, 42, —, 39, 40) | 701, 705, 704, 702, 703 (58, 62, 61, 59, 60) |
| Sly +0x41C, Bentley +0x424 (Sly 2 jump +0x154) | **ledge hang** | 217 (14) | 711 (43) | 706 (63) [I by the shared layout] |
| Sly +0x1EB0 / +0x1EB4 (Sly 2 +0x2000 / +0x2004) | **combat dodge**, left and right | 312, 313 (80, 81) | — | — |
| Sly +0x1A74, +0x1A80, +0x1A8C; +0x1A50 | **swing on a hook**: the swing blend's three clips; the hang state machine | 213, 214, 215; 216 (54 to 57) | — | — |
| shared table `0x342DD0` records 23 to 29 | **paraglide**: the opening clip, the five glide-blend clips, the glider's camera zone | 1727; 1728, 1729, 1730, 1732, 1731; 1733 | the same | the same |

- **Attack** is not a lookup slot: the cane, chair and Murray attacks are the combo definitions' clips (class 148,
  options 1673 standing, 1676 moving, 1679 follow-up; section 5) [X].
- **Land**: Sly 2 has no separate landing slot either; landing is the end of the fall segment and the return to the
  idle state machine [R].

The same IDs name different clips for each character: each looks them up inside its own group (Sly 2 9) [D].

### 12.3 Posing answers: directions, gait speeds, hang, punches, blends

**Jump and fall sub-kinds** [X: `0x001B8EB0`, the same rule and constants as Sly 2]. At take-off the game takes the
stick's angle minus the body's facing, normalised to −π … π. Positive means the stick points to the left of the
facing, counter-clockwise seen from above with z up. Then:

| Sub-kind | Direction | Rule | Sly jump / fall | Turn applied to the heading after the move [R] |
|---:|---|---|---|---:|
| 0 | none: the default before a direction is chosen, and every fixed launch (hang, climb, scripted) | — | 200 / 208 | 0 |
| 1 | stick to the **right** | 1.0996 ≤ \|a\| ≤ 2.0420 and a ≤ 0 | 204 / 212 | +π/2 |
| 2 | stick to the **left** | 1.0996 ≤ \|a\| ≤ 2.0420 and a > 0 | 203 / 211 | −π/2 |
| 3 | **forward** | \|a\| < 1.0996 (`0x3F8CBE4C`) | 201 / 209 | 0 |
| 4 | **backward** | \|a\| > 2.0420 (`0x4002B0B4`) | 202 / 210 | π |

The choice is made once per jump phase, for the initial jump and again for the double jump. In the fall state the
same index picks the fall clip. A missing clip falls back to index 0 [R]. The jump and fall clips have no root
travel (201 moves 0.4 units in 3.17 s) [D]. Their motion comes from the launch and gravity.

**Gait speeds** (v_apt, root travel of transform 1 over one play; Sly is group `jt`) [D]. Sly 3 keeps Sly 2's pairs
unchanged. One correction to 12.2: the gait at +0x1470 is the **full-speed** gait, played at weight u = 1 (Sly 2's
state-1 "run"). The gait that `0x00173068` returns is the **stick-driven** one (Sly 2's walk / move). Sly's
full-speed code (`0x001AD868`) sets the weight to 1 (`0x00258CF0`), takes the blended velocity (`0x00258DA0`) and
multiplies it by **1.3** [X], as Sly 2 does.

| Character | Gait | Slow clip: length, travel, speed | Fast clip | Weight | Speed |
|---|---|---|---|---|---|
| Sly | +0x1470 | 173: 1.0 s, 160, 160/s | 174: 0.5 s, 320, 640/s | 1 | 640 × 1.3 = **832** [X] |
| Sly | +0x1C90 (stick) | 307: 1.3333 s, 300.8, 225.6/s | 308: 0.6667 s, 340, 510/s | stick | 225.6 to 510 |
| Bentley | +0x1470 | 449: 1.0 s, 100, 100/s | 447: 0.3333 s, 250, 750/s | 1 [R] | **750** |
| Bentley | +0x1770 (stick) | 535: 2.0 s, 220, 110/s | 534: 0.5 s, 255, 510/s | stick | 110 to 510 |
| Murray | +0x1470 | 449: 0.3333 s, 170, 510/s | 447: 0.5 s, 400, 800/s | 1 [R] | **800** |
| Murray | +0x1680 (stick) | 535: 1.3333 s, 300, 225/s | 534: 0.5 s, 255, 510/s | stick | 225 to 510 |
| Murray | +0x15A0 | 528: 1.0 s, 125, 125/s | 529: 0.6 s, 310, 516.7/s | stick | 125 to 516.7 |

- Murray's third gait is his **carry gait**. Its clips match Sly 2's carry pair (482 / 483) exactly, which settles
  that question.
- Speed at weight u: Sly 2's footfall-weighted rule ([../sly2/characters.md](../sly2/characters.md) 1.1),
  `((1 − u)·d_slow + u·d_fast) / ((1 − u)·T_slow + u·T_fast)`.
- The playback rate is 1, unless something scales the blend. To match a crew member's actual speed v with one
  clip, play it at v / (the clip's speed).

**Ledge hang (Sly 217, Bentley 711, Murray 706)** [R]. This is the jump machine's ledge state (Sly 2's jump state
17, which enters the ledge and plays this slot). It is used when a jump or fall catches a ledge edge, and only under
player control. Crew posing never needs it. In v_apt, Sly's 217 is loaded (6.3333 s) and Murray's 706 is loaded
(1.6667 s), but Bentley's group has no 711 [D]. *Interpretation:* Bentley cannot catch ledges, and the record is
left over from the shared table layout [I].

**Murray's punches: lookup records, not combo definitions** [X] [D]. Murray has no class-148 combo records in any
world. Those exist only in the groups `jt`, `bentley`, `penelope`, `shaman` and `panda_king`. His punches keep Sly
2's scheme: five consecutive lookup records of his table `0x345FD8`, records 5 to 9, fill the fields +0x187C to
+0x188C (Sly 2's +0x17FC to +0x180C).

| Punch index | Field | Clip (v_apt) | Length | Sly 2 role [R] (Sly 2 clip) |
|---:|---|---:|---:|---|
| 0 | +0x187C | **457** | 0.6667 s | Square, first (424) |
| 1 | +0x1880 | **458** | 0.6667 s | Square, second (425) |
| 2 | +0x1884 | **468** | 0.6667 s | Triangle, first (435) |
| 3 | +0x1888 | **467** | 0.6667 s | Triangle, second (434) |
| 4 | +0x188C | **244** | 0.9167 s | the fifth, special punch (196) |

- The punch-state routine `0x001E1018` takes the punch index n (−1 none, 0 to 4). It stores n at +0x1890 and the
  game time at +0x1894, clears +0x1898 to +0x18A4, and plays the clip in field +0x187C + 4n. Index 4 takes a
  separate branch.
- The lengths match Sly 2's pattern exactly (four of 0.6667 s, one of 0.9167 s) [D].

**Swing blend (Sly 213, 214, 215, in that order)** [R: Sly 2's swing, same three-clip layout]:

- Weights: (active × speed, active × (1 − speed), 1 − active).
- active and speed are smoothed toward targets by sub-state: hanging still (0) 1 and 1; pushing (2) 1 and 0;
  free-swinging (1) 0, and speed 1 when the current speed is above 0.5, else 0. Smoothing uses (1, 0, 0.1) for
  active and (4, 0, 0.1) for speed.
- The blend's time follows the swing angle: target time = length × (clamp(angle / 1.7, −1, 1) + 1) / 2, reached at
  no more than 3 per second.
- A still pose is weights (1, 0, 0), that is 213 alone, at the middle time (0.3333 s of 0.6667 s).

**Glide blend (1728, 1729, 1730, 1732, 1731: lookup records 24 to 28, in that order)** [R: Sly 2's glide]:

- On entering the glide, the weights are (1, 0, 0, 0, 0).
- Each frame, with m the stick deflection, a = the stick angle − the heading, s = m·sin a and c = m·cos a:
  - w0 (1728, neutral) = 1 − m;
  - for m > 0, q = m / (|s| + |c|);
  - w1 (1729) = s·q when s ≥ 0 (stick left);
  - w2 (1730) = −s·q when s < 0 (stick right);
  - w3 (1732) = c·q when c > 0 (forward);
  - w4 (1731) = −c·q otherwise (back).
- The weights sum to 1.
- The opening clip is 1727 (0.25 s). All five blend clips are 2.0833 s long [D].

**Accessory records** (flag `0x20`, IDs listed in 12.4): not settled. In v_apt they are not group records, nested
objects or transform IDs of the three characters [D]. The next place to look is the characters' mesh pieces
(globs), which carry filters and conditions ([classes/solid-object.md](classes/solid-object.md)). The states that
show them are still open.

### 12.4 Not settled

- **Walk and run: settled** [X]. Each character's start-up code builds its gaits with the footstep sound 111
  (`0x00258930`): Sly at +0x1470 and +0x1C90 (`0x001AA118`), Bentley at +0x1470 and +0x1770 (`0x0013D2D0`), Murray
  at +0x1470, +0x15A0 and +0x1680 (`0x001DCA30`). A gait's two clips sit at its +0x14 and +0x54, and the lookup
  tables fill exactly those fields (12.2; Sly's run clips come from his second table, `0x345638`, which his
  post-load reads at `0x001AA2D4`). The routine at `0x00173068` returns each character's run gait (Sly +0x1C90,
  Bentley +0x1770, Murray +0x1680), which fixes which gait is the run. A run gait can be replaced while the
  character holds an object whose own gait is active (`0x00173408` on the object at +0x1108; for example a carried
  object) [X; I for the example].
- **The other slots** (Sly 106, Bentley 84, Murray 127 records) need their Sly 2 fields named first.
- **Accessory meshes.** Records with flag `0x20` are parts or meshes of the character, not looked up by object ID:
  - Sly: 254, 256, 257 to 260, 252, 253, 255;
  - Bentley: 611, 714, 715, 753 to 756;
  - Murray: 714, 715, 542 to 545, 753 to 756;
  - all three: 1710.

  Which are shown only in some states, and when, is open (12.3).
- **Collision toggles (option 812)**: each playable character carries 14 entries (index, object, collision tag),
  for example Sly (0, 8043, 14790), (1, 8031, 14791) [D]. At load the game finds each tag among the character's
  collision entries and **turns it off** [R: the handler `0x00109BD0`]. Some states switch the entries back on;
  which states is open. These are collision shapes, not meshes.
