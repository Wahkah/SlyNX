# The `video` crate

`crates/video` plays the Sly 1, Sly 2 and Sly 3 movies: it demultiplexes the game's PSS files, decodes their MPEG-2 video and
de-blocks their PCM audio, and times frames against an audio clock. Written from ITU-T H.262 | ISO/IEC 13818-2
(video), H.222.0 | ISO/IEC 13818-1 (program streams) and `specs/sly1/movies.md`; see
[legal/MOVIE_PLAYBACK.md](legal/MOVIE_PLAYBACK.md) for why the decoder is our own.

- `#![no_std]` with `alloc`, `#![forbid(unsafe_code)]`, no third-party crates. Depends only on `formats` (without
  its `std` feature), for disc access and the movie table. Builds for `aarch64-nintendo-switch-freestanding` with
  `--no-default-features`.
- Feature `std` (default): no library code depends on it yet; the example and the `threads` feature need it.
- Feature `threads` (implies `std`): `StdThreads`, a slice-decoding runner on scoped `std` threads.
- Nothing panics on bad input: malformed streams give `video::Error` (or, inside a picture, a counted slice error).

## Quick start

```rust
use video::movies::{movie_table, Region};
use video::{MoviePlayer, PlayerStatus};

let movies = movie_table(&disc.executable)?;                    // 17 entries, titles for 1 to 16
let m = &movies[0];                                             // the start-up logo
let mut player = MoviePlayer::new(Region::of_descriptor(image_file, &m.descriptor));
let audio = player.audio_header()?.unwrap();                    // 48 kHz, stereo, 16-bit
// Audio side (feed the mixer): interleaved L, R samples.
let n = player.read_audio(&mut samples)?;
// Game side, once per frame: the clock is the audio played so far (minus the output latency).
match player.update(clock_ns)? {
    PlayerStatus::Ended => { /* back to the game */ }
    PlayerStatus::Playing => if let Some(frame) = player.frame() { /* draw it */ },
}
if skip_pressed { player.skip(); }
```

`cargo run --release -p video --example movie -- <disc.iso> list | bench [index|all] [--threads N] |
png <index> <out-dir> <frame,frame,...>` lists the movie table, times decoding, or writes frames as PNG (outside the
repository; the frames are game data).

## Modules

| Module | Main items | Source |
|---|---|---|
| `demux` | `Demuxer` (`push`, `next_packet`, `stats`), `Packet::{Video, Audio}`, `DemuxStats`, `PACK_SIZE` | H.222.0 2.5; movies.md 2 |
| `audio` | `AudioHeader::parse`, `AudioStream` (`push` gives interleaved `i16` stereo, `finish`) | movies.md 2.2 |
| `decoder` | `Decoder` (`push`, `finish`, `decode` -> `Decoded::{Frame, NeedData, End}`, `set_skip_b`, `set_parallel`, `set_threads`, `sequence`, `gop`, `stats`), `Parallel`, `StdThreads` | H.262 6, 7 |
| `headers` | `SequenceHeader`, `GopHeader`, `PictureHeader`, `PictureType`, `PictureStructure` | H.262 6.2, 6.3 |
| `vlc` | `Vlc`, the Annex B code tables, `Tables` | H.262 Annex B |
| `tables` | scan orders, default quantiser matrices, quantiser scales | H.262 7.3, 6.3.11, 7.4.2.2 |
| `idct` | `idct`, `idct_dc` | H.262 Annex A |
| `frame` | `Frame` (Y, Cb, Cr planes; `to_rgba`, `planes`, `color_matrix`), `FrameInfo`, `Planes`, `ColorMatrix`, `GLSL_YUV_TO_RGB` | |
| `player` | `MoviePlayer` (`update`, `frame`, `read_audio`, `audio_header`, `duration_ns`, `skip`, `is_finished`, `frame_time_ns`, `set_audio_enabled`, `set_threads`), `PlayerStatus` | movies.md 3 |
| `movies` | `movie_table` (`MovieEntry`: index, descriptor, title), `Region` (a byte range of the image), `TITLE_TABLE` | movies.md 1 |

## Sly 2 and Sly 3 movies

Same container as Sly 1 (specs/sly2/movies.md, specs/sly3/movies.md). What changes:

- **Lists.** Sly 2: the executable's file-table records 0 to 20, 23, 27 and 28 (`movies::SLY2_MOVIE_RECORDS`,
  `sly2_movies(&disc.files)`; 23 distinct). Sly 3: the movie table `formats_sly3::Disc::movies`
  (`sly3_movies`; 21 distinct). `Region::of_file_record` gives the bytes.
- **Audio sub-header** `FF A1 00 00`: still sub-stream `0xFF`, removed like Sly 1's.
- **Audio**: SPU ADPCM (`SShd` format 16; `audio::Adpcm`, specs/sly1/audio.md 2.1) with a **32-byte** interleave:
  each channel's bytes are one continuous ADPCM stream, 28 samples per 16-byte block. 44.1 kHz in most movies,
  48 kHz in a few. The player resamples to the platform's 48 kHz (`audio::Resampler`, linear interpolation,
  deterministic in any piece sizes; `MoviePlayer::set_output_rate`, `audio_rate`).
- **Video**: one Sly 2 movie is 720 x 480; Sly 3 runs at 30 frames per second (frame-rate code 5) except one. These
  movies use field DCT and field motion in frame pictures throughout (Sly 1's did not); both decode with no error.

Decode speed (release, one thread, PC as below, whole movies):

| Game | Movies | Frames | Frames per second | Mean ms per frame (range over movies) | 99th percentile |
|---|---:|---:|---:|---|---|
| Sly 1 | 17 | 51,100 | about 1,300 | 0.57 to 0.94 | 1.1 to 1.8 |
| Sly 2 | 23 | 63,755 | 1,477 | 0.46 to 0.91 | 1.23 to 1.65 |
| Sly 3 | 21 | 84,220 | 1,543 | 0.34 to 0.84 | 1.25 to 1.60 |

The longest movies (Sly 2's 578-second credits, Sly 3's 766-second credits) are the slowest per frame (0.91 and
0.84 ms). The Switch estimate of the Performance section applies unchanged.

## Switch decode benchmark

`crates/video-bench` measures the decoder on hardware: `dist/slynx-video-bench.nro`, a homebrew program with no
graphics that decodes the movies of the player's own discs and writes the times to the SD card.

**Build** (WSL, from the repository root; devkitPro as in [switch-build.md](switch-build.md)):

```sh
SLYNX_CRATE=video-bench SLYNX_NAME=slynx-video-bench SLYNX_TITLE="SlyNX video bench" tools/build-switch.sh
# -> dist/slynx-video-bench.nro
```

**Run** (in the final hardware pass):

1. Put the disc images on the SD card: `sdmc:/switch/SlyNX/sly1.iso`, `sly2.iso`, `sly3.iso` (any that exist are
   used; Sly 1 also follows `disc = ...` in `sdmc:/switch/SlyNX/user/game.cfg`, as the game does).
2. Optional: `sdmc:/switch/SlyNX/user/video-bench.cfg` with `sly1 = <path>`, `sly2 = <path>`, `sly3 = <path>` and
   `frames = N` (frames decoded per movie; default 900, which is 30 seconds; 0 decodes every frame).
3. Copy `dist/slynx-video-bench.nro` to `sdmc:/switch/` and start it from the Homebrew Menu. It runs the whole
   benchmark twice, at the default CPU clock and then with the CPU boost (1,785 MHz), and returns to the menu when
   done; + stops it early. With the default 900 frames over 61 movies, each pass takes about 55,000 decoded frames:
   roughly 5 to 10 minutes per pass at the estimated Switch speed.
4. Read `sdmc:/switch/SlyNX/user/video-bench.log` (rewritten after every movie, so an interrupted run keeps its
   results; the same lines go to the debug log, `svcOutputDebugString`).

**Log format:** one line per movie and CPU setting (the numbers below only illustrate the layout):

```text
sly1 movie  0 0x344 640x480 boost off: 900 frames, decode mean 5.12 ms, p50 4.90, p95 7.80, p99 11.20, max 14.30 ms,
    195 fps; read+demux 0.40 ms/frame; errors 0; within 12 ms
```

"Decode" is the time of each `decode` call that returned a frame (one thread, no slice-parallel runner);
"read+demux" is the SD reads and demultiplexing spread per frame; "within 12 ms" compares the mean with the target
of the Performance section. A `summary` line closes each pass. The same code runs on a PC in
`cargo test -p video-bench` (through `platform-pc`, discs linked into a temporary SD folder, 15 frames per movie).

## Decoder coverage

What H.262 Main Profile at Main Level, 4:2:0, needs, without scalability:

- Sequence header and extension, sequence display extension (`matrix_coefficients`), quantiser matrix extension
  (loaded matrices), GOP header, picture header and picture coding extension; user data and other extensions are
  skipped. MPEG-1 streams, D pictures and 4:2:2/4:4:4 are refused with `Error::Unsupported`.
- Slices (any number per row, starting anywhere in the row), macroblock address increments with escapes, skipped
  macroblocks in P and B pictures, all macroblock types of Tables B.2 to B.4, quantiser scale changes (linear and
  non-linear), intra-slice fields.
- Motion vectors with any `f_code` 1 to 9, predictor resets, concealment vectors (parsed; not used for
  prediction); frame and field prediction in frame pictures; field, 16 x 8 and dual-prime prediction in field
  pictures; dual prime in frame pictures; half-sample interpolation; forward, backward and bidirectional
  prediction. References outside the picture are clamped to its edge instead of failing.
- Field pictures, including a P second field predicted from the first field of the same frame.
- Intra DC with all `intra_dc_precision` values, both DCT coefficient tables (`intra_vlc_format`), escapes, zigzag and
  alternate scan, frame and field DCT, inverse quantisation with saturation and mismatch control.
- Display order: I and P frames are held until the next reference arrives; the last one is returned at the end of
  the sequence or the input. Frames carry `display_index` (counting skipped B pictures), type, temporal reference,
  `top_field_first`, `progressive_frame` and `repeat_first_field`.

The movies use only frame pictures with frame prediction and frame DCT (`frame_pred_frame_dct` set): no field
motion, field DCT, 16 x 8 or dual prime occurs on the disc (the disc test checks). Those paths are tested on
synthetic streams written by `tests/synthetic.rs`, whose expected pictures are computed from the field geometry
alone, not from the decoder's formulas.

### IDCT

Integer, separable: two passes of an 8-point transform split into even and odd halves, 13-bit coefficients
(`round(2^13 cos(k pi / 16) / 2)`) and 4 extra fractional bits between passes, rounding at each pass, output
saturated to -256..255. Each pass works on eight columns at once so it vectorises (NEON has 32-bit vector multiplies;
x86-64's SSE2 baseline does not, so PC builds run it mostly scalar). It meets the Annex A (IEEE 1180-1990)
requirements, tested with 10,000 blocks in each of the ranges -256..255, -5..5 and -300..300, both signs: peak
error at most 1, per-pixel and overall mean square error at most 0.06 and 0.02, per-pixel and overall mean error at
most 0.015 and 0.0015, and zero in, zero out. Blocks with only a DC coefficient (after mismatch control) skip the
transform.

## Output: frames and colour

`Frame` holds the three planes at the coded size (`stride` x `coded_height` luma, `chroma_stride` x
`coded_height / 2` for each chroma plane); the visible picture is the top-left `width` x `height` (640 x 480).
The movies are interlaced; frames are returned woven (both fields, top first), not deinterlaced.

- **CPU**: `Frame::to_rgba(&mut out)` converts to RGBA8, top row first, studio-range ITU-R BT.601 (or BT.709 when
  the sequence says `matrix_coefficients` 1), chroma repeated 2 x 2. Meant for tests,
  screenshots and the PNG tool.
- **GPU** (the intended path): upload `frame.planes()` as three `R8` textures (luma full size, chroma half width
  and height, linear filtering), and convert in the fragment shader with `frame.color_matrix().rows` as three
  `vec4` uniforms: `rgb = rows * (y, cb, cr, 1)`. `video::frame::GLSL_YUV_TO_RGB` is a ready-made GLSL function;
  `ColorMatrix::apply` gives the same numbers on the CPU for checking. Uploading the planes is 460,800 bytes per
  frame instead of 1,228,800 for RGBA.

## Player

`MoviePlayer<R: ReadAt>` reads the movie in 64 KiB chunks only when it needs data, so memory stays small (the
elementary-stream buffer, three decoded frames, a pending and a current frame, and the audio not yet read).

- **Clock**: `update(clock_ns)` takes the time since the movie started, normally the audio played so far (samples
  read by the mixer / 48,000, minus the output latency), so video follows audio. Frame *n* is due at
  `n * 1001 / 30000` s (`frame_time_ns`, from the sequence header's frame rate).
- **Decoding ahead**: one frame is decoded ahead and shown when due; the frame from `frame()` stays the same until a
  later one is due.
- **Falling behind**: when several frames are due in one `update`, each becomes current in turn and only the last
  is shown (`frames_dropped` counts the others); while the next frame is already due, B pictures are skipped
  without being decoded (`DecoderStats::skipped_b`), so the player catches up quickly.
- **Audio**: `read_audio` gives interleaved stereo `i16` at 48 kHz, reading the file further when its buffer runs
  low. With `set_audio_enabled(false)` audio is parsed and dropped.
- **End**: `update` returns `PlayerStatus::Ended` once the last frame has been shown for its full period; `skip()`
  ends playback at once (no more frames or audio).
- `set_threads(n)` (feature `threads`) decodes slices on `n` threads.

## Performance

Decoding only (the movie already demultiplexed), release build, one thread, x86-64 (Intel Core Ultra 9 275HX,
in WSL), all 17 movies (51,100 frames): **about 1,300 frames per second**, mean 0.57 to 0.94 ms per frame
depending on the movie, 99th percentile 1.1 to 1.8 ms (the I pictures), and isolated maxima of 2 to 7 ms (host
scheduling noise; they move between runs). Time splits roughly into bitstream and coefficient decoding (most of
it, and all of the I-picture tail), motion compensation (about 0.12 ms per frame) and the IDCT.

| Threads (`--threads`, "Commercials & Outtakes") | Mean ms/frame | 99th percentile |
|---|---|---|
| 1 | 1.04 | 1.46 |
| 2 | 0.61 | 0.94 |
| 3 | 0.45 | 0.71 |
| 4 | 0.39 | 0.63 |

**Switch (not measured yet).** One Cortex-A57 core at the default 1,020 MHz is very roughly 8 to 12 times slower
than the PC core above, which puts the mean at about 5 to 10 ms per frame and the I pictures at about 12 to 20 ms.
The budget per movie frame is 33.4 ms (29.97 fps), and the player decodes one frame ahead, so an occasional slow I
picture does not cause a visible drop; the 12 ms target holds for the mean, probably not for every I picture. If
needed on hardware:

1. CPU boost (`Platform::set_cpu_boost`, 1,785 MHz) while a movie plays: about 1.75 times faster.
2. Slice-parallel decoding on the idle cores: implement `video::Parallel` on the platform layer's threads (cores 1
   and 2; the I/O core is idle during a movie, the audio core lightly loaded) and pass it to `Decoder::set_parallel`.
   The trait only needs "run these jobs and return when all have finished". 2 jobs give about 1.7 times.
3. Skipping B pictures when late is already automatic.

Confirm with the FPS overlay and the `bench` numbers from a Switch build before tuning further.

## Tests

- Unit tests (`cargo test -p video`): every Annex B table decodes each of its codes (and the tables are prefix codes
  with the expected Kraft sums: complete where the standard's table is complete); scan orders and quantiser tables;
  the bit reader against a naive reader on random reads; the IDCT against a double-precision reference (the full
  IEEE 1180 procedure, every single-coefficient block, every DC value, extreme blocks without overflow); motion
  compensation at full and half samples, field views and clamping; the program-stream demultiplexer fed one byte
  at a time, and its errors; the audio de-blocking in pieces of any size; colour conversion (CPU against the GPU
  matrix).
- Synthetic streams (`tests/synthetic.rs`): field DCT, field prediction in frame pictures, dual prime, skipped and
  no-MC macroblocks, a non-intra residual; field pictures with the second field predicted from the first, field
  and 16 x 8 prediction; a whole synthetic movie (program stream, video, audio) through `MoviePlayer` with a stepped
  audio clock.
- Sly 2 and Sly 3 disc tests (`tests/disc_sly2.rs`, `tests/disc_sly3.rs`, with `SLYNX_DISC_SLY2` and
  `SLYNX_DISC_SLY3`; about 20 and 30 s): every distinct movie's SHA-256, pack count, picture count, sequence
  parameters (size, profile, aspect, frame rate, bit rate, VBV), audio header (ADPCM, 32-byte interleave, rate,
  size) and every audio sample decoded; the first GOP decoded on its own and the last two GOPs from the
  second-to-last GOP header, each picture giving a frame with no slice error; Sly 2's `0x808` left channel against
  the spec's SHA-256 (and identical channels); and the player on `0x808` resampling to 48 kHz and ending on time.
  Unit tests cover the ADPCM block decoder against the formula, the channel de-blocking in any piece sizes, and the
  resampler.
- Disc tests (`tests/disc.rs`, run when `SLYNX_DISC_SLY1` names the image): the movie table (identities, sectors,
  sizes, titles); every movie's SHA-256, pack count (whole 16 KiB packs plus the end code), picture count, video
  elementary-stream bytes, audio header and data bytes, sample count, and duration against the table and the
  picture count; the logo's first pack header and audio header bytes; the 13 second copies against the table's
  hashes; every movie decoded in full without a slice error, in display order, with the sequence parameters of
  movies.md 2.1, one GOP per I picture and about one I picture in 15, 28 : 109 : 272 in the logo, only the first
  GOP marked closed, and no field or dual-prime prediction;
  slice-parallel decoding giving the same frames as serial decoding; the player on the logo with a 60 Hz audio
  clock (every frame shown in order, every sample delivered, ended on time), and catching up after a 5-second jump,
  then skipping. movies.md gives no PCM hash, so none is checked. The disc tests take about 7 minutes in a debug
  build (the full decode of 51,100 frames, one thread per movie) and well under a minute in release.
- Visual check: frames of the logo, "Tide of Terror" and "The Making Of Sly Cooper" (I, P and B pictures) were
  written with the `png` tool and inspected: clean, no blocking or drift.

## Spec notes

- **Sly 3 picture counts.** specs/sly3/movies.md's "Pictures" column counts the `00 00 01 00` patterns anywhere in
  the file, audio data included: every value equals that raw count, while the video streams hold 2 to 27 % fewer
  pictures (5,434 instead of 5,561 in `0x14F`; 4,970 instead of 6,319 in `0x14E`), which the decoder confirms. The
  "Seconds" column inherits the error, and so does the note that `0x14E`'s audio is 42 s shorter than its pictures:
  it is 2.7 s longer. The disc test uses the measured counts.
- **Sly 2 `0x80B`.** Its audio is 1.38 s longer than its pictures, against movies.md's "within 0.1 s"; the spec's own
  sizes give the same.

- **Open GOPs.** movies.md 2.1 says the GOPs are closed. On the disc only the first GOP of each movie has
  `closed_gop` set; every later GOP is open (its leading B pictures, shown before its I picture, predict from the
  previous GOP's last reference), and no `broken_link` is set. The decoder follows the standard either way; the
  disc test asserts what the disc holds. movies.md 2.1 should say "first GOP closed, the others open".
- Audio and video lengths: movies.md 2.2 says they agree within 0.1 s (0.65 s for 0x343). Computed from the
  spec's own sizes and picture counts, two movies differ by more: 0x340 by 0.118 s and 0x347 by 0.110 s; the
  bound is 0.12 s. The duration column of movies.md 1.2 is within 0.1 s of the
  audio or the video length, but follows neither consistently (0x347's 56.9 s is the audio's, 0x343's 51.6 s the
  video's). The disc test checks those bounds.

## Open questions

- Which screens start the four movies no world references, and when the start-up logo plays (movies.md 6).
- Whether music and effects are paused or only muted during a movie (movies.md 6).
- Switch timing: measure `bench` on hardware, then decide on CPU boost and a `Parallel` runner.
