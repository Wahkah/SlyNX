# The `audio` crate

`crates/audio` plays Sly 1's sound: effect banks driven through their steps, the adaptive music, dialogue lines and
ambient loops, positioned sounds, and the mixer the platform calls on its audio thread. It is written from
[`specs/sly1/audio.md`](../specs/sly1/audio.md) and public SPU documentation (ADPCM block format, envelope
registers).

- `#![no_std]` with `alloc`, no third-party crates. Depends on `formats` (disc access, SHA-256) and `platform-api`
  (the `AudioRender` trait).
- Feature `std` (default) only turns on `formats/std` (opening an image by path). The Switch build uses
  `--no-default-features`.
- `unsafe` is confined to the single-producer/single-consumer queue in `queue.rs`.

## Quick start

```rust
use std::sync::Arc;
use audio::{create, Config, Emitter, Listener, INTENSITY_ALARM};
use audio::disc::{bank_sound_counts, read_bank, read_music, world_audio};

let (mut sound, renderer) = create(Config::default());
platform.start_audio(Box::new(renderer))?;            // the renderer now lives on the audio thread

// Loading (game or loader thread side): bank 0 always, plus the world's bank, then the world's music.
sound.set_bank_counts(bank_sound_counts(&mut disc)?);  // numbers the sound IDs
sound.load_bank(0, Some(Arc::new(read_bank(&mut disc, 0)?)));
let world = disc.read_world(index)?;
let (music, bank) = world_audio(&world)?;              // music descriptor, sound-bank index (-1 = none)
if bank > 0 { sound.load_bank(bank as usize, Some(Arc::new(read_bank(&mut disc, bank as usize)?))); }
if let Some(m) = music { sound.play_music(Arc::new(read_music(&mut disc, &m)?)); }

// Every frame:
sound.set_listener(Listener { position: cam_pos, velocity: cam_vel, left: cam_left });
let h = sound.play(549, Emitter::at(guard_pos, 2000.0, 300.0));
sound.set_emitter(h, Emitter::at(new_guard_pos, 2000.0, 300.0));
let alarm = sound.request_intensity(INTENSITY_ALARM);
sound.update(dt);
```

`cargo run --release -p audio --example render_paris -- <disc.iso> <folder outside the repo>` renders the Paris
music (calm, with an alarm, hiding) and a few effects to WAV files. `cargo run --release -p audio --example
mixer_bench` times the mixer with synthetic data.

## API

### Game thread: `SoundEngine`

| Method | What it does |
|---|---|
| `create(Config) -> (SoundEngine, AudioRenderer)` | Makes both halves. `Config`: voices per pool (24 effects, 24 music, 8 streams), queue length, random seed, `legacy_music_tuning` |
| `set_bank_counts([u16; 7])`, `resolve(id)` | Sound-ID numbering: bank `b` starts at the sum of the counts of banks `0..b` (audio.md 3.6) |
| `load_bank(slot, Option<Arc<EffectBank>>)`, `bank(slot)` | Puts a bank in a slot or empties it. Sounds from the old bank stop at once |
| `play(sound_id, Emitter) -> SoundHandle` | Starts a sound. `SoundHandle::NONE` when its bank is not loaded or a one-shot sound starts out of range; a continuous sound out of range starts silently |
| `play_dialogue(Arc<VagSound>, Emitter)` | A dialogue line in mix group 2; while it plays, groups 0, 1 and 3 drop to half volume |
| `play_stream(Arc<VagSound>, group, looped, Emitter)` | Any VAG file (ambient loops: `looped = true`) |
| `stop(handle)`, `stop_all()` | The sound's steps end and its voices release; the handle ends when they are silent |
| `is_playing(handle)`, `playing_count()` | As of the last `update` |
| `set_emitter(handle, Emitter)`, `set_listener(Listener)` | Positions; the mix follows on the next `update` |
| `play_music(Arc<MusicFile>)`, `play_music_at(.., segment)`, `stop_music(unload)` | Starts the world's music at segment 0 (or another) |
| `set_music_register(r, v)` | Scripts can set music registers |
| `request_intensity(level) -> IntensityRequest`, `remove_intensity(req)`, `intensity()` | The highest active request wins (0 with none). `INTENSITY_ALARM` = 107, `INTENSITY_HIDING` = -20 |
| `add_intermittent(Intermittent)`, `clear_intermittent()` | Options 549/550/551: a sound ID, a volume range and an interval range in seconds |
| `set_music_volume(f)` | Option 525 (0 for option 524 "music off") |
| `set_group_volume(group, v)`, `set_reverb(kind, depth)`, `set_global_register(r, v)`, `set_paused(bool)` | Mix-group masters (`audio::group::{EFFECTS, MUSIC, DIALOGUE, EFFECTS2}`), option 552, registers voice steps may read, silence on `Suspended` |
| `update(dt)` | Once per frame: collects finished handles and data returned by the renderer, moves the intensity, fires intermittent sounds, sends the new mix of every positioned sound |

`Emitter { position: Option<Vec3>, velocity, start_distance, full_distance, doppler, volume, pitch, pan }`;
`Emitter::global()` is an unpositioned sound at full volume, `Emitter::at(pos, start, full)` a positioned one.
`Listener { position, velocity, left }`, `left` being the camera's left axis. Positions are game units, game
space.

### Audio thread: `AudioRenderer`

Implements `platform_api::AudioRender`; `render(&mut self, out: &mut [f32])` also works directly (tests, offline
rendering). Output is interleaved stereo at 48 kHz, soft-clipped above 0.8 so dense mixes do not clip hard.

### Data and lower layers

| Module | Main items |
|---|---|
| `adpcm` | `decode_block`, `decode_to_end` (one pass to the end flag: what the spec hashes), `sample_len`, `Cursor` (block-by-block playback that follows the loop point; `Cursor::looping` for streams) |
| `vag` | `VagHeader`, `VagSound` (whole file in memory; decoded while it plays) |
| `bank` | `EffectBank::parse`, `SoundRecord`, `Step`/`StepKind`, `VoiceParams` (also the music key regions), `ValueSource`, `bank_sound_count` |
| `music` | `MusicFile::parse`, `Instrument`, `TrackRecord`, `Segment`, `EventCursor`/`Event`, `Command`/`commands`/`decode_command` |
| `envelope` | `Envelope`, `AdsrWords`, `Phase` |
| `effects` | `Instance`, `run` (one tick of a sound's steps) |
| `sequencer` | `Sequencer` (players, registers, macros, channel groups), `controller_pan` |
| `mixer` | `Mixer`, `Voice`, `VoiceStart`, `Pool`, `Source`, `Owner`, `SampleData`, `Reverb`, `pan_gains`; `TICK_RATE` = 240, `TICK_FRAMES` = 200 |
| `spatial` | `attenuation`, `position_pan`, `doppler_semitones`, `mix_for`, `Mix` |
| `disc` | `bank_descriptors`, `read_bank`, `bank_sound_counts`, `world_audio`, `read_music`, `read_vag` (handles the size-0 descriptor), `ambient_descriptors`, `prompt_descriptor`, `commentary_table` |
| `queue` | `channel` → `Producer`/`Consumer` (bounded SPSC ring) |

## PCM stream input (`audio::pcm`)

For movie soundtracks and other decoded sound: interleaved `i16` at 48 kHz (the output rate, no resampling), fed
from the game thread.

```rust
use audio::pcm::{pcm_stream, PCM_RATE};

let (mut writer, reader) = pcm_stream(PCM_RATE as usize / 5, 2); // 200 ms ring, stereo; allocated here
sound.attach_pcm(Some(reader));                                    // the renderer mixes it from the next block

// Every frame, on the game thread:
let taken = writer.write(&decoded[pos..]);                         // whole frames that fit; keep the rest
pos += taken * 2;
let audio_clock = writer.played_frames() as f64 / PCM_RATE as f64; // seconds mixed so far: sync video to this
let queued = writer.latency();                                     // seconds queued ahead of the clock
// At the end: writer.finish(); on a seek: writer.clear(); to stop: sound.attach_pcm(None).
```

| Item | What it does |
|---|---|
| `pcm_stream(capacity_frames, channels) -> (PcmWriter, PcmReader)` | Makes a bounded ring (1 or 2 channels). Allocation happens here, on the calling thread |
| `SoundEngine::attach_pcm(Option<PcmReader>)` | Hands the reader to the renderer, or detaches it. One stream at a time; the old reader comes back to the game thread to be dropped |
| `PcmWriter::write(&[i16]) -> usize` | Appends whole frames that fit and returns how many; never blocks |
| `free_frames()`, `buffered_frames()`, `capacity_frames()` | Ring state |
| `played_frames()` | The audio clock: frames mixed so far. It stops while paused or while the ring is dry |
| `latency()` | Buffered frames in seconds. The platform's own render block (8 ms on PC) comes on top |
| `underruns()` | Times the ring ran dry while open; silence is played for the gap and the clock does not advance |
| `finish()`, `drained()`, `clear()`, `set_gain(f)` | End of stream (dry is then not an underrun), all mixed, drop the queue (seek), output gain (1.0) |

On the audio thread the reader copies samples and moves atomic indices only: no locks, no allocation, no freeing.
The stream is mixed after the mix groups (dialogue ducking and music volume do not apply), before the soft clip. A
mono stream plays on both sides. Resuming from `set_paused(false)` takes effect from the render block after the one
that reads the command, as for every other sound.

## Threading model

```
game thread                                   audio thread (Switch: core 2)
SoundEngine ── Command queue (SPSC, 1024) ──▶ AudioRenderer::render
            ◀── Event queue (SPSC, 2048) ───  (Ended(handle), Garbage(Arc ...))
```

- **All parsing, loading and allocation happen on the game side.** Banks, music and VAG files cross the queue as
  `Arc`s. The renderer allocates nothing after `create` (voice pool, instance table, sequencer state, reverb
  lines and queues are sized up front) and takes no locks.
- **The renderer never frees memory.** When it replaces or finishes with an `Arc` (bank swap, music change, stream
  end) it sends it back as `Event::Garbage`, and `SoundEngine::update` drops it. If the event queue is full it
  keeps up to 32 events and retries; beyond that it leaks rather than free on the audio thread (only if `update`
  stops being called).
- **Commands never block the game.** If the command queue is full, `SoundEngine` keeps the overflow in order and
  sends it on the next call.
- **Timing.** The renderer runs a 240 Hz scheduler tick every 200 output frames, independent of the platform's
  buffer size (384 frames on Switch, 512 on PC). Each tick: drain commands, run each sound's steps, run the
  sequencer, retire finished streams, then set every voice's gain and pitch for the next 200 frames (gains ramp
  linearly across the tick, so no zipper noise). Step delays count these ticks.
- **Music timing is exact.** Each segment player adds `10^6 x ticks-per-quarter` per tick and spends
  `tempo x 240` per MIDI tick, all in integers, so there is no drift; events land on the 240 Hz grid. A wait is
  paid with one division, however long it is.
- **Work per tick is bounded, whatever the data says.**
  - Each sound instance runs at most once per tick with a budget of 64 steps, shared by every child it starts
    in that tick. A zero-delay loop that starts children stops when the 64-slot instance table is full.
  - Each segment player has a budget of 4,096 per tick, charged for every event and every adaptive command
    (inside macros and skipped blocks too). Running out stops that player.
  - Tempos are clamped to 10,000..10,000,000 µs per quarter and ticks per quarter to 1..9,600 when parsed;
    music registers saturate at 127.

## How a sound plays

- **Effect voices**: rate `48000 x 2^((60 - |centre| + fine/128 + bend + game pitch + Doppler)/12)`. Amplitude
  `(sound volume/127 x voice volume/127 x game gain)^2 x group master x dialogue duck`; pan = sound pan + voice
  pan + position pan, then left `(1 - sin p)/2`, right `(1 + sin p)/2`. Voices of mix groups 0 and 3 feed the
  reverb. Negative voice volumes and pans read sound registers (-1..-4), a random value (-5) or global registers.
- **Steps**: voice (1), child sound (5), nop (20), loop start/end (21/22, loops repeat until the sound is
  stopped), stop (24), random choice (25, never the same choice twice in a row per step, shared by every instance),
  random delay (26), random bend (27), add bend (29), random register (31), wait for voices (38). A sound ends when
  its steps are done and its voices are silent; child sounds share the parent's handle.
- **Music**: a note plays every region of the channel's instrument whose range holds it, at
  `48000 x 2^((note - |centre| + fine/128 + bend)/12)`. Amplitude `(track x instrument x region x velocity x
  channel volume, each /127)^2 x music master`; channel volume (CC 7), pan (CC 10, 64..127 → 0..90°, 0..63 →
  270..359°), program change and pitch bend are followed. Note-off is the `0xDn` channel-pressure message (or a
  zero-velocity note on). At most 24 music voices; a new note takes the lowest-priority voice, oldest on ties, if
  its priority is not lower. Below intensity level 0 the music volume scales from 1 down to 2/3 at -20.
- **Adaptive commands**: all of audio.md 5.3 (tests, stop/start/jump to segments, macros, channel groups,
  registers). `0F`/`10` channel groups gate channels by intensity or a register: a closed channel starts no notes
  and its sounding notes are released (layers come in at their next note). A player started by `12`/`0E` inherits
  the starter's active channel group; `11`/`0D` keep the channel settings and release the old segment's notes.
- **Distance, pan, Doppler**: the audio.md 3.6 rules, computed on the game side in `update` and sent as a `Mix`
  only when it changes.
- **Streams**: VAG ADPCM decoded while playing, resampled from 22,050 or 44,100 Hz with the same interpolator.
- **Resampling**: linear interpolation on 32.32 fixed-point positions; ADPCM is decoded 28 samples at a time
  inside the voice, so memory is the compressed data only.
- **Envelope**: the SPU ADSR rules at 48,000 steps per second.
- **Reverb**: a small Schroeder reverb (four damped combs, two all-passes) on a mono send, kinds 1..5 mapped to
  room sizes, depth 0..32767 to the wet level.

## Not yet implemented

- **LFO steps (kind 4)** are parsed (kept raw) but do nothing: their layout is unconfirmed (audio.md 7).
- **Instance limits** (0 in every bank) and the sound flag bit 2 are ignored; voice-step flag bit 0 (set on 469
  voice steps) has no known meaning and is ignored.
- **Binocular zoom distance compression** and the camera-specific distance rules: the listener is a plain point.
- **Dialogue streaming from disc in chunks**: a line is loaded whole (as compressed ADPCM) and decoded while it
  plays. Subtitles and line order belong to the dialogue spec.
- **Reverb presets** are approximations, not the SPU2's.
- **Commentary** entries are read (`commentary_table`) but nothing plays them yet.
- Interpolation is linear (the SPU's 4-tap Gaussian interpolation is not reproduced).

## Tests

- `cargo test -p audio`: unit tests with synthetic data (ADPCM blocks and cursors, VAG, envelope timing, bank and
  music parsing, events with running status and pressure note-offs, adaptive command parsing, step timing and
  loops, random choices, the sequencer's exact timing, blocks, macros, channel groups, voice stealing, ramps,
  reverb, the queue across threads, the engine end to end, intensity, streams, queue overflow) and a pinned
  SHA-256 of a mixed output (`engine::tests::mixed_output_hash`).
- `tests/disc.rs` (skips without `SLYNX_DISC_SLY1`): all six PCM hashes of audio.md 6, the per-bank LBA / size /
  sound / step counts, the sound-ID bases, every step kind count, every voice and key-region sample decoding to an
  end flag, the Paris and Hideout structure checks, the ambient, prompt and commentary tables, every segment of
  every music file walked, the adaptive command counts, note-on/note-off balance, a 10 s Paris render and a pinned
  hash of bank 0 sound 0 mixed for one second.

## Performance (release, PC, one thread)

| What | Time |
|---|---|
| Mixer, 32 looping voices with reverb, per 10 ms block (480 frames) | about 85 µs (0.85 % of real time) |
| Mixer, 8 / 48 voices, per 10 ms block | about 17 / 150 µs |
| Paris music, 60 s through the whole engine | about 75 ms (0.12 % of real time) |

Measured in WSL with `mixer_bench` and `render_paris`; Switch numbers to come.

## Sly 2

Written from `specs/sly2/audio.md`, `movies.md`, `disc.md` and `world-file.md`. Sly 1 behaviour is unchanged
(every Sly 1 test vector and the pinned mixer hashes still pass).

### Reading the disc (`audio::sly2`)

`formats` has no Sly 2 support yet, so the crate carries the little it needs:

- `Sly2Disc::open` / `open_path`: reads `SYSTEM.CNF` and the executable, refuses any executable but SHA-256
  `a511a94b…`, maps its two ELF segments, decodes the 29-record file table, decompresses the boot world (record 24,
  with the Sly 1 decompressor from `formats`) and reads the 44-row level table.
- `FileRecord::decode`: the 64-byte record, sector `(w0 ^ w13) + 0xCA0`, size `w1 ^ w15`, identity `w2 ^ w14`;
  size 0 is empty.
- `WorldHeader::parse(world)`: package list, the sound section, save variables, level table; `level_name()`.
- `read_world(row)`, `read_bank`, `read_vpk`, `read_vag`, `read` for any record.

### The world's sound section (`WorldSound`)

- **Bank and sound IDs.** The bank record and a flag byte per sound: 2 means continuous (`continuous(j)`). Then the
  sound-ID space and the sound ID of each bank sound; `sound_map()` turns that into ID → bank sound.
- **Resident stream.** The resident stream record and its channel numbers (`resident`, `resident_channels`).
- **Voice lines and music.**
  - The language tracks, then the voice lines.
  - The music stream files by music number (`music`, `music_stream(n)`).

### Effect banks, version 3

`EffectBank::parse` reads both versions (`EffectBank::version`). A version-3 step is 8 bytes:
- the kind is in the top byte of word 0;
- the low 24 bits are an offset into the step-data area (kinds 1, 4, 5, 9) or three signed bytes `a0 a1 a2`;
- word 1 is the delay.

| Kind | Parsed as | Played |
|---|---|---|
| 25 | `RandomChoice { a0, a1, last: a2 }` | random group, never the same twice in a row |
| 26 | `RandomDelay(24 bits)` | extra delay 0..n |
| 27 | `RandomBend(a0)` | random bend, ±`a0` % of the range |
| 29 | `AddBendScaled(a0)` | bend += `a0` / 127 of the range |
| 30 | `SetRegister { a0, a1 }` | set a register (negative: a global one, −1 = global 0) |
| 31 | `RandomRegister { a0, a1, a2 }` | random register value, low..high (negative register: global) |
| 39 | `GroupCycle { a0, a1, pass: a2 }` | the groups **in turn**, one per pass; the pass counter is shared by every instance |
| 9 | `VoiceLegacy(voice data)` | as a voice step (its centre note is non-negative; same pitch formula) |
| 23 | `LoopBreak` | jump past the next loop end of the sound (no effect without one) |
| 43 | `StopMarker` | normal playback ends here; `stop(handle)` runs the steps after it instead of ending the sound |

- `SoundEngine::set_sound_map(Some(map))` makes `play(id)` use the world's ID map (bank slot 0).
- `set_sound_flags(Some(flags))` treats flag 2 as continuous, in addition to the sound record's bit. This matters
  for the out-of-range rule.

### Streams

`VpkStream::parse` splits the 8 KiB interleave into one contiguous ADPCM buffer per channel. The mixer decodes
each channel while playing and loops it.

- **Playing a stream.** `play_vpk(stream, group, looped, emitter)` plays every channel at its stored volume and
  pan.
- **Fades.** `fade_out(handle, seconds)` fades a stream, then stops it.
- **Layers.** `set_layers(handle, gains, fade)` sets the gain of each channel group.
- **Voice pool.** It defaults to 16 voices. A stream that cannot get a voice for every channel is refused.
- **Resident stream (distance layers).**
  - `play_resident(stream, channel_numbers)` plays it looped in the effects group.
  - `add_falloff(Falloff { channel, position, start, end, volume })`, `set_falloff_position` and
    `remove_falloff` follow the ambient objects (option 100).
  - Each update gives a channel the loudest of its fall-offs: `volume` within `start`, silent beyond `end`.
  - With exactly one fall-off, the channel pans toward it with the Sly 1 position-pan rule. With none, it plays at
    its stored volume.
- **Music streams (a stack of 8).**
  - `push_music(stream)` fades the current top out over 1 s and plays the new one looped in the music group.
  - `pop_music()` fades the top out and restarts the one below. `music_top()` gives the handle.
- **Music layers** (`sly2::music`), computed in `update`:
  - **Groups 0, 1, 2** are sneak, normal and alert. `music_mood()` gives the alert and sneak counters and the
    character's `CharacterMusicState`. The chosen group's weight rises (sneak: fast 3, slow 0, ease 0.25 s;
    others: fast 1, slow 0, ease 0.25 s). The other two are scaled so the three squares add up to 1.
  - **Groups 3 and 4** fade with the distance from the listener to the playable characters (`set_characters`):
    full within 2,000, silent beyond 5,000. Group 3 uses the nearer of characters 0 and 1, group 4 character 2.
  - **Groups 5 to 7** play at full volume.
  - The smoothing curve is an approximation: the speed is the remaining distance over the ease time, clamped to
    the slow and fast speeds.

### Voice lines and movie soundtracks

- **Voice lines.** They are VAG files (version 0). `Sly2Disc::read_vag` reads them and `play_dialogue` plays them.
  - `VoiceTrack::headset`: 0 goes to the USB headset when the headset output is on (`to_headset()`).
  - `VoiceLine::records` are `TimedRecord { delay (1/60 s), target, value }`. The value is
    `CueValue::Eyes(f32)` for target 3 and `CueValue::Goal(i32)` otherwise; target 5 is the mouth.
  - `LineCues` fires the records as the line plays (`advance(dt, out)`). `stop(out)` fires the rest at once, then
    the closed-mouth goal (`CLOSED_MOUTH_GOAL`, 1,553, the spec's open value).
- **Movie soundtracks.** `MovieAudio` decodes a movie's audio stream: format 16 ADPCM for Sly 2, format 1 PCM for
  Sly 1.

### Sly 2 tests

`tests/sly2_disc.rs` skips without `SLYNX_DISC_SLY2`. It checks:
- **Test vectors.** Every vector of sly2/audio.md and the movie-audio vector of movies.md:
  - the row-3 bank;
  - the resident-stream channels;
  - voice line 1285;
  - the `0x808` soundtrack, demultiplexed with the `video` crate as a dev-dependency.
- **Banks.**
  - Counts: sounds, steps and samples per bank, the step-kind totals, and the extra sounds per bank.
  - Flags against the continuous bit: 654, 88 and 388.
  - Inline arguments:
    - delays under 10,000;
    - bend values 100 × 8,277 and 127 × 2,130, down to −106;
    - the six add-bend values;
    - 13 × "register 0 = 100";
    - 248 of 249 registers with low ≤ high;
    - 64 kind-9 steps with non-negative centres;
    - 572 group cycles that fit;
    - the stop marker 3 steps before the end of its sound.
- **Streams.** The resident streams (36, 32 kHz, groups 0..3 at pan 0, one channel number each), the 64 music
  files (rates and the four layouts) and the 122 empty records.
- **Voice lines.** 7,970 voice references, the speaker counts, the headset counts, the timed-record target counts
  per line number and the mouth values 5,372..5,378.
- **Disc structure.** The file table, the level table, the boot-world hash and the sound-section boundaries.

### Sly 2 spec questions

1. **Kind-1 centres.** sly2/audio.md 2 says every kind-1 step has a negative centre note. 101 kind-1 steps hold
   +120 (for example row 0, sound 103, step 3).
2. **Timed-record target counts** (57,042 mouth and so on) count each line number once (1,691 lines), not every
   reference (7,970). The spec could say so.
3. **Still open:** the fade time when music streams change (1 s here), the smoothing curve, which characters
   groups 3 and 4 follow, the goals of targets 0, 1, 2 and 4, and the closed-mouth goal.

## Music by number (script action 146; Sly 2 and Sly 3)

`audio::sly2::MusicQueue` is the scripts' way into the music stack (sly2/audio.md 3.2, options-by-class.md 2.1).
Sly 3 plays music the same way and uses the same type.

```rust
use audio::sly2::MusicQueue;

let mut music = MusicQueue::new();                        // one per loaded world
// Script action 146: queue music stream `n`; the handle goes back to the script.
let h = music.queue(&mut sound_engine, n, |n| ad.read_music_stream(world.sound(), n))?;   // Sly2Disc
let h = music.queue(&mut sound_engine, n, |n| ad3.read_music_stream(&world3.sound, n))?;  // Sly3Disc
music.stop(&mut sound_engine);                            // stop the top; the one below restarts
```

- **Queue:** the stream is read on first use, then kept for the world. The queue pushes it on the engine's stack,
  which fades out the current top and keeps up to 8. A number the world's sound section does not name gives
  `SoundHandle::NONE`.
- **Mixing:** the layers are mixed by `SoundEngine::music_mood` and `set_characters`, as before.
- **Disc I/O:** reading happens on the calling thread, so the game may want to call `queue` from its loading
  step.
- **New:** `Sly2Disc::read_music_stream`, matching the Sly 3 reader.
- **Not hooked up yet:** action 146 itself (the game owner).

## Sly 2 in the game (for the Sly 2 session)

What `crates/game/src/sly2` needs to play a world's sound and its dialogue. It is all in `audio::sly2`, read from
the audio crate's own disc reader (`Sly2Disc::open_path` on the same image).

```rust
use audio::sly2::{Sly2Disc, WorldAudio, Talk, TalkCommand, TimedRecord};

let mut ad = Sly2Disc::open_path(path)?;
let world: WorldAudio = ad.load_world_audio(row)?;    // sound section, strings, bank, stream
world.install(&mut sound_engine);                     // bank in slot 0, sound-ID map, flags, the stream as resident

// A sequence object (class 114): each option of its list, in order, may be a dialogue command.
if let Some(cmd) = TalkCommand::decode(option_id, value_bytes) { /* PlayLine, WaitLine, ClearSubtitle, ... */ }

// PlayLine { line, quiet, .. }:
let voice = ad.read_line(&world, line)?.map(Arc::new);                       // the line's VAG (first track)
talk.play(&mut sound_engine, world.line(line).unwrap(), voice, world.subtitle(line), quiet, &mut cues);
// Every frame:
talk.update(dt, &mut cues);            // cues: TimedRecord { target 0..5, Goal(id) / Eyes(f) } for the speaker
let sub = talk.subtitle(game_time);    // Subtitle { text (revealed, with the | cursor), speaker, colour }
talk.wait_done(-1.0);                  // command 708
```

- `Talk` plays one line at a time. Starting a line stops the previous one: its remaining cues fire, then the
  mouth closes (goal 1553).
- The subtitle shows `⌊t / (duration − 0.5) × length⌋` characters and a `|` cursor. Once the whole text shows, the
  cursor blinks at 4 Hz. Quiet lines have no subtitle.
- Subtitle colours come from `talk::speaker_colour`.
- `Sly2Disc::talk_targets()` reads the executable's state-machine IDs for cue targets 0 to 5 (1548, 1549, 1550, −1,
  1551, 1552).
- The speaker is the world's first object of the line's speaker class (the engine's lookup; spec section 4).

Tests: `tests/sly2_dialogue.rs` (skips without `SLYNX_DISC_SLY2`). It checks:
- all 5,262 play-line commands over 3,871 sequence objects, each with its voice entry and subtitle;
- all 7,970 voice entries with a string;
- the per-command counts and the binocucom argument table;
- the per-world table;
- the nightclub-cellar sequence 1714 with its line bytes, durations, record counts and the subtitle of line 1300,
  played through `Talk` (audible, revealed 22 of 29 after 0.6 s, the cursor blink, all 19 cues);
- every world's sound installing and speaking a line.

Disc findings for the spec side: row 1 holds no sequence objects (the spec's "50 to 169 per world" counts 42 rows).

## Sly 3

Written from `specs/sly3/audio.md`, `movies.md`, `disc.md` and `world-file.md`. Sly 3's sound is Sly 2's format, so
`audio::sly3` only adds the disc and world readers. Sly 1 and Sly 2 behaviour is unchanged, and their tests pass.

- **Disc.** `Sly3Disc::open` / `open_path` reads `SYSTEM.CNF` and the executable, and refuses any executable other
  than SHA-256 `ce817858…`. It maps the two ELF segments.
  - It reads the three 40-slot world tables (names `0x2EDE48`, episodes `0x2EDF10`, file records `0x2EDFD8`) and
    the 23-record movie table (`0x3179F8`).
  - The data in `SLY3.WAD` is read by sector, as for Sly 2.
  - `read_world(slot)`, `read_bank`, `read_vpk`, `read_vag` and `read` work as on Sly 2.
- **World header.** `sly3::WorldHeader::parse` walks the key tables, the 88-byte group list (with body sizes) and
  the save variables to the sound section, which is Sly 2's `WorldSound`.
- **Playback.** Everything plays through the Sly 2 paths:
  - version-3 banks and the sound map;
  - the world's stream as the resident stream, at 44.1 kHz;
  - the music stack;
  - voice lines and timed records;
  - `MovieAudio` for the soundtracks, including the 48 kHz long movie `0x151`.
- **Loading a world's sound.** `Sly3Disc::load_world_audio(slot)` returns a `WorldAudio`: the sound section, the
  effect bank and the world stream. `WorldAudio::install(&mut engine)` puts the bank in slot 0, sets the sound-ID map
  and flag bytes, and starts the stream as the resident stream. `read_music_stream`, `read_voice_line` and
  `music_numbers` read the rest of the section. Flag value 1 (new in Sly 3) is passed through and plays as an
  ordinary sound.
- **`VPK ` change, shared with Sly 2.** Sly 3 stream files are not padded to whole interleave rounds. The last
  round is shorter, and `VpkStream::parse` now splits it evenly between the channels. Sly 2's files are fully padded
  and decode as before.

- **Dialogue** (`audio::sly3::talk`, dialogue-and-voice.md and scripts.md 7.2). `DialogueCommand::decode(id, args)`
  reads the command-script dialogue commands: `0x52` play a line (line list, quiet, source, fall-off, volume),
  `0x53` stop (with an `s32` list), `0x54` clear the subtitle, `0x55` / `0x56` hold and release talk effects, `0x5F`
  wait.
  - Lines play through Sly 2's `Talk` (`play_text`, or `play_text_at` with an emitter; Sly 3 strings are UTF-16).
    `WorldHeader` and `WorldAudio` carry the world's strings (`subtitle(n)`) and `line(n)`.
  - `formats_sly3::stream::command_script` gives a script's commands with their argument bytes.
  - **Which line plays** (1.1): `LineOrder`, one per command, kept while the world is loaded. A single line always
    plays. Otherwise each run rotates the list by one (so a fresh command plays its second entry first) until a full
    round, then shuffles it (each position from the second on swapped with a random earlier one), resets the count,
    and swaps the line that played last to the end if the shuffle put it first. The same line never plays twice
    in a row.
  - **Subtitle** (1.2): the string whose ID is the line number, none for a quiet line. `talk::subtitle` gives
    `Talk`'s subtitle with Sly 3's colours by speaker class (`speaker_colour`: 52, 53, 54, 60, other).
  - **Source and fall-off** (1.3): `line_emitter` plays a line at its source object with full level within the
    start distance and silence beyond the end (a start at or past the end reads as end = start + 1). Without a
    source, or with an end of 0 or less, the line is unpositioned. The volume multiplies the level. The caller moves
    the emitter with the object (`SoundEngine::set_emitter` on `Talk::handle`).
  - **Stop** (2): `stops(list, playing)`: the listed lines, or any line for an empty list.
  - **Talk effects** (4): `TalkHolds`, per runner: at most 4 characters, each once; `end` releases what is left.
  - The command runner that calls these (with the source object's position and the game's random numbers) is not
    written yet.

### Sly 3 tests

`tests/sly3_disc.rs` skips without `SLYNX_DISC_SLY3`. It checks:
- **Disc and worlds.**
  - The world list: all 39 rows, with sector, sizes and the SHA-256 of each decompressed world.
  - The menu world's compressed hash and first bytes.
  - The header-section boundaries of dvd_menu, i_trainer and v_apt, and v_apt's group entries.
- **Sound sections.**
  - The identities of the bank and stream.
  - Flags: 0 × 12,182, 2 × 898, 1 × 448.
  - Sound-map ranges and voice-entry counts.
  - Speaker counts.
  - The 7 worlds without a stream.
  - Stream channel counts (2 × 18, 3 × 6, 4 × 8), all at 44.1 kHz.
  - 152 other stream files (250 empty records), by channel count.
  - 3,654 voice files, all VAG version 0 at 22,050 Hz.
- **Vectors.** The menu bank, the i_trainer bank and stream, and the v_ext stream.
- **Playback.** Every world's sound installs and renders 1 s with its stream and every mapped bank sound. Every
  world is audible, and a replay with a fresh bank gives the same samples.
- **Dialogue** (`tests/sly3_dialogue.rs`).
  - All top-level dialogue commands decode, with the spec's counts: 12,729 play, 49 stop, 2,011 clear, 4,222 and
    3,998 hold and release, 1,169 wait. The 548 nested scripts add 23 play commands, 10 holds and 4 releases.
  - All 17,046 listed lines have a voice entry and a string.
  - The line order on the disc's 712 lists of several lines: three rounds each, only listed lines, the second
    entry first, never the same line twice in a row.
  - v_apt script 2088 plays line 1587 (quiet, audible).
- **Movies.** The 23-record movie table (21 distinct). For every movie: its file hash, end code, audio rate and
  `SSbd` size. The whole `0x151` soundtrack is decoded (746.48 s at 48 kHz).

### Sly 3 spec questions

5. **Bank state:** random-choice steps keep their last choice in the bank shared by every instance (Sly 1 rule). A
   replay is therefore only bit-identical with a freshly parsed bank. Is the "last choice" per bank or per playing
   sound in Sly 3?

1. **Stream lengths:** only 9 of the 32 world streams are 3,024,016 bytes per channel (120.0 s). The others run
   from 2,199,600 to 5,498,592 bytes; the test pins the whole distribution.
2. **Stream padding:** the files are not padded to whole 8 KiB rounds as Sly 2 section 3 says. The last round is
   shorter, split evenly between the channels. Please confirm the split rule.
3. **Voice entries:** 8 voice entries have an empty file record.
4. **Still open from the spec:** the flag value 1, the voice-entry byte (headset in Sly 2), and the speaker
   classes 56 and 62 to 67.
5. **Dialogue** (dialogue-and-voice.md):
   - 1.1 says each position is swapped "with a random earlier one". That is read as strictly earlier, so a two-line
     list always alternates. Should a position be able to stay where it is?
   - 1.3 does not say what a source with a fall-off end of 0 or less does. It is played unpositioned here.

## Spec questions and observations

audio.md now carries findings 1 to 12 below (the block reading of `04`/`05`, the child-step layout, the `00`
count, segment padding, empty sounds and instruments, the choice overrun, voice flags, the fine-tuning sign, the
instant release); the crate follows the corrected spec. They are kept here as the record of what was observed:

1. **`04`/`05` are blocks, not "else".** Every use follows a test, as that test's body: Paris segment 1 has
   `16 01 00 04 02 12 13 01 01 05`, "if register 1 = 0 { if intensity < 18 { register 1 = 1 } }". A failed test
   skips the whole block. With the "else" reading, register 1 could never leave 0. The crate implements blocks.
2. **Child-sound step (kind 5) layout**: the sound index is at data offset 12, not 0. Offset 0 holds a volume
   (127, 100, 90, 75, or -1 / -5 register references) and offset 4 a pan (0 or -5). Implemented that way.
3. **Command counts**: every count in audio.md 5.3 matches the disc (top-level commands, all 45 files, walking each
   segment to its end-of-segment event) except `00`: 6,687 on the disc, 6,707 in the spec.
4. **Music identities**: 45 music descriptors, 38 identities, 37 distinct files: "Piranha Lake" (`0x67`) and "Down
   Home Cooking" (`0x62`) are byte-identical under different identities.
5. **Segment padding**: every segment's events end with `FF 2F 00` followed by zero padding; segment 0 has no
   end-of-segment event at all (its padding would parse as a data byte without a running status).
6. **Sounds without steps**: bank 6 sounds 70..79 have 0 steps and a first-step offset of -40.
7. **Empty instruments** (0 key regions) have a region offset that points before the region table.
8. **Random-choice overrun**: bank 5 sounds 28 and 29 have a 5-way choice with only 4 steps after it; the crate
   stops at the sound's end.
9. **Voice flags** are not all 0 in the effects: 469 of 1,429 voice steps have bit 0 set.
10. **Fine-tuning sign**: positive. Centre -86, fine 66 gives 11,016 Hz, within 0.1 % of 11,025 Hz (the spec's
    "about 11.6 kHz" looks like a typo for 11.0).
11. **`0x9FC0` release**: its release shift is 0, an instant release, not "medium" as audio.md 3.4 says.
12. **Commentary**: 12 of the 33 entries have a duration of 0.0.
13. Open, needs listening or the reference side: the step delay unit (1/240 s assumed), the intensity fall rate
    ("every three sound updates" is implemented as every three `update` calls, so it depends on the frame rate),
    whether channel-group activation is per segment, the default value of sound and global registers (0 and 127
    here), the music 44.1/48 kHz rule (`Config::legacy_music_tuning`, off), whether the square law applies to
    music and dialogue as it does to effects (applied here), and the reverb presets.
