# The save system (`crates/save`)

Saving for Sly 1, written from `specs/sly1/save-format.md` and `specs/sly1/pickups-and-progress.md` sections 6 and 7.
`no_std` + `alloc`, builds for the Switch; file access goes through `platform_api::FileSystem`, so the same code runs
on the Switch and the PC.

| Module | What it does |
|---|---|
| `model` | `SlotState`: everything a slot holds, its ranges (`clamp`), and the progress and restore rules |
| `completion` | percent complete and the slot description text |
| `autosave` | `Session`: the game in progress, its slot, and when it saves |
| `slotfile`, `settings`, `codec` | SlyNX's own file formats |
| `store` | `SaveStore`: the files in the user folder |
| `sly2` | Sly 2: the save-variable tree, the progress block, and Sly 2 slots (below) |

## The state model

`SlotState` follows save-format.md 1:

| Field | Range |
|---|---|
| `play_time` | seconds, `f64`, 0 or more |
| `episode`, `level` | 0..=5, 0..=8: where Load resumes |
| `lives` | −1 (game over) ..= 99; a new game starts with 5 |
| `charms` | 0..=2 |
| `coins` | 0..=99 |
| `settings` | `Settings` bits: 1 episode-unlocked line played, 5 vibration off, 6 mono, 7 music off, 8 auto-save notice acknowledged, 9 binoc-u-com up = up, 10 turrets inverted, 11 jet pack inverted. Other bits are kept as they are |
| `reward_mask`, `unlocked_movies` | 32-bit masks |
| `milestones` | first bottle, half the bottles, all bottles, first vault, second vault (bits 0..4) |
| `last_power_up` | `None` or 0..32 |
| `episodes[6]` | keys, vaults, sprints, time, flags (visited, finished), and nine `LevelRecord`s |
| `LevelRecord` | goal flags (visited, key, vault, sprint, boss: bits 0..4), time, best sprint time, difficulty and its raw value (0..=1), 12 dialogue-played flags, 4 script key/value pairs, 64 bottle bits, 8 checkpoint bits |

`clamp()` brings a loaded slot into these ranges (save-format.md 5): counters to their limits, NaN or negative times to
0, difficulties to 0..=1, unknown flag bits cleared, an out-of-range position back to the prologue, and episode
counters to at most the number of levels with the matching goal flag.

### Progress rules (what the engine calls)

| Call | Effect |
|---|---|
| `SlotState::new_game()` | prologue, 5 lives, no rewards, no power-up selected |
| `visit(e, l)` | level and episode visited; becomes the resume point |
| `add_time(e, l, s)` | slot, episode and level time |
| `collect_key(e, l)` | key flag and episode count, once; `true` the first time |
| `open_vault(e, l, reward)` | vault flag, count, reward bit; first and second vault milestones |
| `collect_bottle(e, l, i, total)` | bottle bit; first, half and all bottle milestones; whether the level's set is complete |
| `finish_sprint(e, l, s)` | sprint flag, count on the first completion, best time; reports first completion and new best |
| `defeat_boss(e, l)` | boss flag; the episode counts as finished |
| `reach_checkpoint`, `restart_checkpoint` | checkpoint bits; a restart after losing a life uses the highest-numbered checkpoint reached (inferred: the spec gives the bits, not the choice among several) |
| `set_dialogue_played`, `dialogue_played` | the 12 per-level dialogue flags |
| `lose_life()`, `continue_after_game_over()` | lives; "Try again?" restores 5 lives and keeps saved progress |
| `add_coins(n)` | pickups-and-progress.md 3.1: past 99 the counter resets and gives a charm if fewer than 2 are held, else a life if below 99, else stays at 99 |

### Restore rules (save-format.md 3)

- **Load** resumes at the saved `episode` and `level` (which level of an episode a load resumes in is open question 3
  in the spec; the engine decides from these two values).
- **What a level contains** on entry: `level_entry(e, l, placed_bottles)` says whether the key is still placed, whether
  the vault is open, and which clue bottles to place (those not yet collected).
- **Settings** are applied at once from `state.settings` (`vibration()`, `mono()`, `music()`, the inverts).
- Collected coins, charms and life tokens across a reload are open question 1 in the spec; nothing here stores them.

## Percent complete and the slot description

`CompletionTable::new(worlds, boss_levels)` takes `(episode, level, task_mask)` for every world (the task mask is
the world table's word at offset 40, `formats::disc::WorldEntry`) and the five episode-boss levels. Tasks: a visit for
every level outside the prologue, each goal bit of the task mask, and each boss. With the disc's masks that is
43 + 19 × 3 + 9 + 5 = 114, as the spec says (a test checks it). `percent()` displays 0 when nothing is done, 100
when everything is, otherwise `floor(100 × done / total)` clamped to 1..=99.

`SlotSummary::text()` gives the slot list entry: `Empty`, or `%d%% - %s - %d:%02d` (percent, the world's name,
play time as hours:minutes), for example `37% - Paris, France - 3:07`. `SaveStore::summaries(table, world_name)`
reads all three slots. A slot file that cannot be read shows `Damaged save` (a SlyNX addition; the original had no
such case).

## When the game saves

`Session` holds the game in progress, the slot in use and where it was last saved. Progress events update the
state and return a `SaveRequest` when the game should save now (save-format.md 2):

| Event | Saves when |
|---|---|
| `on_key_collected(e, l)` | the key is new |
| `on_vault_opened(e, l, reward)` | the vault was closed |
| `on_sprint_finished(e, l, s)` | first completion or a new best time |
| `on_boss_defeated(e, l)` | always (episode bosses and the final boss) |
| `on_hideout_progress_dialogue()` | always |
| `on_world_loaded(e, l)` | the world or level differs from the one last saved (or loaded) |
| `save_game(slot)` | "Save Game...": always; the chosen slot is in use from then on |
| `save_and_quit()` | "Save and Quit": when a slot is in use |

Every automatic save needs a slot in use **and** settings bit 8 (the player acknowledged the auto-save notice:
`acknowledge_autosave_notice()`; `disable_autosave()` for "Auto-save has been disabled"). A new game starts with
the bit cleared, so the notice is shown again; a loaded slot keeps it. Progress is recorded either way.

The engine writes a request with `store.commit(&mut session, request)` (which also tells the session where it was
saved) and shows the auto-save icon meanwhile (hud.md 6).

## Files

In the user folder (`sdmc:/switch/SlyNX/user/` on the Switch; see `docs/platform.md` for the PC):

| File | Content |
|---|---|
| `saves/sly1/slot1.sav` .. `slot3.sav` | the three slots |
| `saves/sly1/slot1.bak` .. `slot3.bak` | each slot's previous good save |
| `settings.bin` | the port's own options |

Every write goes through `FileSystem::write_user` (write a temporary file, then replace), so a crash leaves the old
or the new file. Before a slot is replaced, its current file, if it reads back correctly, is copied to the `.bak`.
Loading a slot whose checksum fails uses the `.bak` instead (`Loaded::from_backup` tells the caller); a file written
by a newer SlyNX is never replaced by its backup.

### Container format

```text
offset size
 0      8  magic: "SLYNXSV\0" (slot) or "SLYNXCF\0" (settings)
 8      2  format version of the writer
10      2  oldest reader version that can read the file
12      1  game (1 = Sly 1)
13      3  zero
16      4  payload length
20      4  CRC-32 (IEEE) of bytes 0..20 and the payload
24      n  payload: sections of u16 tag, u32 length, body
```

Little-endian integers, IEEE floats. Every single-bit change anywhere in a file is detected (a test flips each bit).

**Slot, format version 1:** section 1 (play time `f64`; episode, level, lives (`i8`), charms, coins; settings,
reward mask, unlocked movies (`u32`); milestones; last power-up, `0xFF` for none), then one section 2 per episode
(index, keys, vaults, sprints, flags, time `f32`, level count, then each level as a `u16`-length-prefixed record: flags,
time, best sprint, difficulty, raw difficulty (`f32`), dialogue flags (`u16`), 4 × (`i32` key, `i32` value), bottles
(`u64`), checkpoints (`u8`)).

**Settings:** section 1 of entries `u16 key, u16 length, value`: stick deadzone inner and outer, master, music and
effects volume (`f32`), show FPS, last slot used, confirm button (right or bottom face button). Entries with unknown
keys are kept and written back unchanged.

### Versions

- A reader skips unknown sections, reads only the fields it knows from each section and record (records carry
  their length), and ignores the rest. A newer SlyNX that only **adds** fields or sections keeps "oldest reader"
  unchanged, so older builds still load its files.
- A change older readers would misread raises "oldest reader"; older builds then report the slot as written by a
  newer version (`LoadError::TooNew`) and leave it alone.
- A file from an older format loads with the missing fields at their new-game defaults, goes through
  `slotfile::upgrade` (empty for version 1: there is no older format yet), and is written in the current format on
  the next save.
- Every loaded value is clamped, so a file edited by hand cannot put the game out of range.

## Sly 2 (`sly2`)

Written from `specs/sly2/saving.md`.

- `tree::VarTree` reads a world's save-variable section: names, 10-byte records, and paths of name IDs (an array
  takes one more element, the index). It gives the layout word, which is the terminator's offset (20,632 on the
  disc), and the layout checksum, which is the byte sum of the record table (`0x2767B4`).
- `progress::Progress` is the live block (`0x5400` bytes) with typed reads and writes by path:
  - integers, object and world IDs, floats, single flags (4 bytes) and flag arrays (one bit each);
  - the new-game reset keeps only the top-level `flags` group;
  - `mark_from_memcard` sets every task's flag.
- `slot` is SlyNX's container with game byte 2 and two sections. The first holds the two check words and the
  block, with every task's `from_memcard` set in the written copy. The second holds a save serial: the newest slot
  is the one with the largest serial, standing in for the card file's modification time.
  - On load, check words that differ from the running disc's clear the block. A slot whose `dt` is 0 is unused and
    is reset, keeping the options.
  - `Sly2Store` keeps `saves/sly2/slot1.sav` to `slot3.sav` with `.bak` copies, as for Sly 1.
  - Memory-card files are not written, because their layout cannot be checked (saving.md 4).
- The engine's job system (`engine::game::sly2_jobs`) writes the block. The current job, task and checkpoint task
  are not saved, as in the game.

## Sly 3 (`sly3`)

Written from `specs/sly3/saving.md`, on the Sly 2 code (nothing is copied):

- The section has Sly 2's layout, so `sly2::VarTree` reads it. Every world carries the same one: 6,559 records,
  used size 21,764, checksum 2,978,085 (`USED_SIZE`, `CHECKSUM`).
- `sly3::progress(tree)` is a `sly2::Progress` of Sly 3's size, `0x6800` bytes (`Progress::with_size`).
- Slots are Sly 2's slot files with Sly 3's `SlotKind`: game byte 3, folder `saves/sly3`. `sly3::store(fs)` is a
  `Sly2Store` built with `with_kind`; `encode_for` and `decode_for` take the kind. The layout check, the
  unused-slot reset, `from_memcard`, the backups and the newest slot are Sly 2's.
- Helpers: `checkpoint` (`widCheckpointForce`, `oidCheckpointForce`), `coins` (`loot/ccoin`), and `owned`
  (`powerUps/mppukfAcquired`, one bit per item-table record).
- `game::sly3::save_tree(world)` reads the tree from a loaded world.
- **Not yet:** when Sly 3 saves, and the session keeping its coins and items in the block (it holds them in
  `Sly3Session::coins` and `owned` for now).
- **Tests:** a Sly 3 slot round trip that a Sly 2 reader refuses. On the disc (`game/tests/disc_sly3_flow.rs`):
  all 39 worlds carry the same tree with the spec's counts, and `loot/ccoin`, `oidCheckpointForce` and
  `widCheckpointForce` sit at offsets 172, 12 and 16.

## Importing original memory-card saves

`sly1_card` holds the original record's frame from save-format.md 6. The header is version 18 and size 6,656. The
check word is the unsigned byte sum from offset 12 to the end, in 32 bits. Loading refuses a wrong file size
(error 8), a wrong version or size (error 28), and a mismatched check word (error 27). A new record has the
starting values: 5 lives at 6,624, −1 "no power-up" at 6,652, and 3 at 6,620. Its tests cover each rule.

A full importer is not implemented: the spec still marks the field layout past the header as unconfirmed
(save-format.md 6, 7), so an importer waits for a spec taken from a card written by the game.

## Spec notes

- save-format.md 1 lists the "episode unlocked voice line played" setting as **bit 1**; ui-and-menus.md 5 calls it
  "setting flag 2". This crate uses bit 1 (`Settings::EPISODE_LINE_PLAYED`); the two specs should agree.
- The first-, half- and all-bottle milestones are set from the level being played (half or all of that level's
  bottles); the spec names the flags but not their exact condition.
- Defeating a boss marks its episode finished (episode flag 1); the spec lists the flag without saying what sets it.

## Tests

`cargo test -p save`: the model (new-game values, clamping, goals and entry rules, bottle milestones, sprints,
bosses, checkpoints, lives, coins), the 114-task total and the percent rule, the slot text, the container (CRC check
value, every single-bit flip detected, truncation, wrong game, reader versions), slot round trips (an empty and a
fully populated slot), clamping on load, damage detection, a newer writer's extra fields and sections being ignored,
an older writer's missing fields getting defaults and being saved back in the current format, the settings file
(round trip, unknown entries kept, clamping), the store (save, load, erase, backup fallback, newer files left alone,
summaries, commit) on an in-memory folder, the auto-save rules, and one test on a real disk through the PC
platform's file system (`tests/pc_files.rs`). Sly 2: tree paths, typed values and bit arrays, the new-game reset,
`from_memcard`, slot round trips, the layout check, unused slots, and the store's backups and newest slot.
