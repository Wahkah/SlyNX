# Testing

The project is tested on PC first. The `harness` crate (`crates/harness`) drives the game, or any other program that
implements its `App` trait, in a deterministic, headless mode. It captures frames as PNG images, compares them with
reference images, and records per-frame performance. It uses no third-party crates; the PNG encoder and decoder are
its own.

## Quick reference

```sh
cargo test --workspace                 # every unit test; disc and golden tests skip when not configured
cargo clippy --all-targets
cargo fmt --check

cargo run --release -p harness -- list
cargo run --release -p harness -- run --app pattern --frames 600 --capture 120,599 --out /tmp/run1 \
    --width 1280 --height 720 --warmup 10 --check-determinism
cargo run --release -p harness -- compare /tmp/run1/frame_000599.png /tmp/run2/frame_000599.png --diff /tmp/diff.png
cargo run --release -p harness -- perf /tmp/run1/perf.json --baseline ~/slynx-baselines/pattern.json
```

Run performance measurements from a `--release` build. Debug numbers are not comparable with anything.

Exit codes for every command: `0` passed, `1` a check failed (images differ, a performance regression, a run that
is not deterministic), `2` bad usage or an I/O error.

## Environment variables

| Variable | Used by | Meaning |
|---|---|---|
| `SLYNX_GOLDEN_DIR` | golden tests | Folder of reference images. **Must be outside the repository**; the harness refuses a folder inside the checkout. Unset: golden tests skip. |
| `SLYNX_GOLDEN_UPDATE` | golden tests | `1` (or `true`): write the actual images as the new goldens instead of comparing. |
| `SLYNX_TEST_OUT` | golden tests | Where mismatch artefacts go (`<name>.actual.png`, `<name>.diff.png`). Default: `<SLYNX_GOLDEN_DIR>/_failures`. Must also be outside the repository. |
| `SLYNX_DISC_SLY1`, `SLYNX_DISC_SLY2`, `SLYNX_DISC_SLY3` | tests that read a game | Path to the player's own disc image (or a folder the converter made from it). Unset or missing path: the test skips. |

A test that skips still passes, and prints `skipped: <test>: <reason>` on stderr. Use `cargo test -- --nocapture` to
see which tests skipped.

## Golden images: the rules

Reference images of the game are made from the game's data, so they are treated like the game's data:

1. **Goldens are never committed.** Not in the repository, not in a branch, not in an issue or pull request
   attachment. The release audit blocks images it does not recognise, but the rule comes first.
2. **They live outside the checkout**, in the folder named by `SLYNX_GOLDEN_DIR` (for example `~/slynx-goldens`).
   The harness checks this and fails the test if the folder is inside the SlyNX repository.
3. **Each developer makes their own** from their own disc, with `SLYNX_GOLDEN_UPDATE=1`, from a build that is known
   to be right (or from captures of the original game, cropped and scaled to match). Goldens are not shared through
   the repository.
4. **Updating a golden is a deliberate act.** Check the diff image first, and say in the change description which
   goldens changed and why.
5. Performance baselines (`perf.json`) contain no game data and may be kept anywhere, but keep them next to the
   goldens so a machine's baselines stay together. Baselines are per machine: never compare numbers across machines.

Golden names are relative paths of letters, digits, `_`, `-`, `.` and `/`, without `..`; the file is
`<SLYNX_GOLDEN_DIR>/<name>.png`. Suggested layout: `sly1/<level>/<shot>`, and `harness/...` for the harness's own
tests.

Writing a golden test:

```rust
use harness::golden::assert_golden;
use harness::{RunConfig, Tolerance, run};

#[test]
fn village_start_view() {
    let Some(disc) = harness::golden::require_disc(harness::golden::Game::Sly1, "village_start_view") else { return };
    let mut app = make_game_app(&disc);                      // the game's App
    let config = RunConfig { frames: 120, captures: vec![119], seed: 1, ..Default::default() };
    let result = run(&mut app, &config).unwrap();
    assert_golden("sly1/village/start", result.capture(119).unwrap(), &Tolerance::gpu_lenient(1280, 720));
}
```

`assert_golden` skips when `SLYNX_GOLDEN_DIR` is not set, writes the golden in update mode, and otherwise panics with
the comparison and the paths of the actual and diff images when they do not match. A golden that does not exist yet
is a failure (not a skip) once `SLYNX_GOLDEN_DIR` is set.

## Deterministic runs

`harness::run(app, &RunConfig)` runs an app for `frames` frames:

- fixed timestep of 1/60 s (`FIXED_DT`); time is `frame * dt`, never the wall clock;
- one random number generator (`harness::Rng`, xoshiro256** seeded through SplitMix64) seeded with `seed`;
- input from an input script (below), or none;
- after each frame's update, the app's state hash (if it provides one) is recorded;
- on the frames listed in `captures`, the rendered frame is read back as an image.

The run's *fingerprint* hashes every state hash and every captured image. Two runs with the same app build, seed and
script must have the same fingerprint; `harness run --check-determinism` (or `runner::check_determinism` in a test)
runs twice and reports the first frame where they differ.

`harness run` writes to `--out`: `frame_NNNNNN.png` per capture, `hashes.txt` (state hash per frame and the
fingerprint), `perf.json` and `perf.txt`.

### The `App` trait

```rust
pub trait App {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError>;  // one fixed step
    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError>;  // draw (offscreen when headless)
    fn capture(&mut self) -> Result<Image, AppError>;                          // last frame, RGBA8, top row first
    fn state_hash(&self) -> Option<u64> { None }                                // for determinism checks
    fn shutdown(&mut self) {}
}
```

`FrameContext` carries `frame`, `dt`, `input: &InputState`, `rng: &mut Rng`, `probe: &mut FrameProbe` (timers and
counters) and `capture_requested`. Apps are created from an `AppConfig` (`width`, `height`, `seed`, `headless`, and
`--opt key=value` options) through an `AppRegistry`.

The determinism contract an app must keep:

- time only from `ctx.frame` and `ctx.dt`;
- randomness only from `ctx.rng` (or generators seeded from it or from `AppConfig::seed`);
- input only from `ctx.input`;
- nothing that affects the simulation or the image may depend on thread timing, hash-map iteration order or
  uninitialised memory;
- `render` must not change simulation state (a run that renders only captured frames must give the same images);
- state hashes use `harness::hash::Fnv64`, not `std`'s hasher, so they stay stable across Rust versions.

The built-in `pattern` app is a small software-rendered stub that follows these rules; the harness's own tests use it.

### Input scripts

Plain text, one command per line, `#` comments. Each line starts with a frame `N` or a half-open range `N..M`:

```text
0..60    hold right          # held on frames 0 to 59
30       tap a               # pressed on frame 30 only
45       down zr             # held until released
90       up zr
100      stick left 0.5 -1   # x right, y up, each in [-1, 1]
120..180 stick right 1 0     # back to 0 0 at frame 180
```

Buttons: `a b x y l r zl zr plus minus up down left right lstick rstick sl sr` (`start`/`select` for
`plus`/`minus`), case-insensitive. Same-frame commands apply in file order. The button bits match the platform
layer's `Buttons` layout. `InputScript::from_states` turns a recorded sequence of input states back into a script,
so a played session can be saved and replayed.

## Image comparison

`harness compare A.png B.png` compares a reference `A` with an actual image `B`:

| Option | Meaning |
|---|---|
| `--tolerance D` | a channel differing by at most `D` counts as equal (default 0) |
| `--max-diff-pixels N` | up to `N` pixels may differ beyond the tolerance (default 0) |
| `--min-psnr DB` | also require a PSNR of at least `DB` |
| `--ignore-alpha` | leave alpha out |
| `--diff OUT.png` | write a diff image: the reference dimmed to grey, differences beyond the tolerance in red (brighter = larger), changes within it in yellow |
| `--json` | print the report as JSON |

The report gives the size, the changed pixels, the pixels beyond the tolerance, the maximum delta per channel, MSE
and PSNR (`10 log10(255^2 / MSE)`, infinite when identical) and the first differing pixel. `Tolerance::exact()`
requires identical images; `Tolerance::gpu_lenient(w, h)` (channels within 2, 0.1 % of pixels, 40 dB, alpha
ignored) suits renders from different GPUs or drivers.

The PNG decoder reads every standard PNG (all colour types, bit depths 1 to 16, palettes, `tRNS`, Adam7
interlacing), so captures from other tools (emulator screenshots, for instance) can be compared directly. The
encoder writes 8-bit RGB or RGBA.

## Performance reports

Each frame gets a `FrameProbe`. The harness times `update`, `render`, `frame` (update plus render) and, on captured
frames only, `capture` (not part of `frame`). The app adds its own:

```rust
fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
    let mut cull = ctx.probe.scope("render.cull");         // timed until dropped; scopes nest
    /* ... */
    drop(cull);
    ctx.probe.count(harness::perf::DRAW_CALLS, stats.api_draw_calls as u64);
    ctx.probe.count(harness::perf::TRIANGLES, stats.triangles);
    if let Some(gpu) = stats.gpu_ns { ctx.probe.add_time_ns("render.gpu", gpu); }
    Ok(())
}
```

Stages are recorded flat by name (use `parent.child`), so nested times are not additive. A stage timed twice in a
frame accumulates. Counters add up within a frame.

The report (`perf.json`, `perf.txt`) gives, per stage (milliseconds) and per counter: mean, median, 95th and 99th
percentile, minimum, maximum and the frame of the maximum. Percentiles interpolate linearly between the closest
ranks (the usual "type 7" definition). `--warmup N` leaves the first `N` frames out. The text report also counts
frames over the 16.667 ms budget.

`harness perf current.json --baseline base.json` compares two reports. By default a stage or counter whose median or
95th percentile grew by more than 10 % is a regression (exit code 1), one that shrank by more than 10 % is an
improvement, and stage changes under 0.05 ms are ignored as noise. Options: `--threshold PCT`, `--min-delta-ms MS`,
`--metrics median,p95,p99,mean,max,min`, `--json`. A stage or counter present on only one side is reported as
`missing`.

To make a baseline, run the same command on a known-good build and keep its `perf.json`. Compare only runs made on the
same machine, with the same build profile, resolution and script.

## Using the harness with the game

The `harness` binary only knows its built-in apps. The game's PC binary (or a small `slynx-harness` binary next to
it) registers the game's apps and hands over to the same command-line front end:

```rust
fn main() {
    let mut apps = harness::apps::AppRegistry::builtin();
    apps.register("sly1", "Sly 1 from SLYNX_DISC_SLY1; options: level=NAME", |cfg| Ok(Box::new(Sly1App::new(cfg)?)));
    std::process::exit(harness::cli::main_with(&apps));
}
```

### Needed from the platform layer and the renderer

For the harness to drive the real game these pieces are needed outside `crates/harness`:

**Platform layer (PC)**

1. **A headless mode:** create the graphics device and an offscreen render target of a given size without opening a
   window (or with a hidden window, if the API needs one).
2. **Input injection:** a way to feed the game an input state per frame instead of reading devices. The harness's
   `InputState` uses the same button bits as the platform `Buttons` type (`buttons`, `pressed`, `released`, and
   sticks with `x` right and `y` up in `[-1, 1]`), so the conversion is a copy. Player 1 only, for now.
3. **No wall-clock pacing in harness mode:** no vsync, no frame limiter, no sleeping; the harness calls `update` and
   `render` as fast as it can, and time comes from the frame number.
4. **File access to the disc** through the path in `SLYNX_DISC_SLY1` (and later 2 and 3).
5. **Threads:** if the game uses worker threads, the results that affect the simulation must not depend on their
   timing (join in a fixed order, merge results in a fixed order).

**Renderer**

1. **Frame readback:** after `render`, wait for the GPU and read the colour target back as RGBA8, top row first (flip
   OpenGL's bottom-up rows; `Image::flip_vertical` does this), at the configured size. This is `App::capture`.
2. **Counters per frame:** the renderer's `FrameStats` already has what the report needs. Feed `api_draw_calls` to
   `DRAW_CALLS`, `triangles` to `TRIANGLES`, and optionally `draw_commands`, `batches`, `visible` as counters, and
   `cpu_build_ns`, `cpu_submit_ns`, `gpu_ns` as stage times (`render.build`, `render.submit`, `render.gpu`) with
   `FrameProbe::add_time_ns`.
3. **Deterministic output:** the same scene and camera give the same pixels on the same machine and driver (no
   time-based effects, no uninitialised targets; clear every target each frame).
4. **A fixed render size** independent of any window, so captures match goldens of a given resolution.

## On the Switch

The PC harness is how changes are proven until the end-of-project Switch pass. On the Switch, the FPS overlay and the
owner's screenshot loop confirm performance; the same input scripts and seeds should be used there once the platform
layer can play them back, so PC and Switch runs are comparable.

## Smoke runs

`cargo run --release -p game --example smoke -- <disc.iso> [--game sly1|sly2|sly3] [--seconds N] [--worlds a,b]
[--once]` plays a fixed route twice and compares every frame's state hash (`crates/game/tests/smoke_run/mod.rs`;
`tests/smoke.rs` runs a short Sly 1 route). The Sly 1 route, the input patterns and the checks are `game::smoke`, which
the Switch smoke build (`dist/slynx-smoke.nro`, docs/switch-build.md) runs on hardware:

- **Sly 1**: the title, a new game through Splash into Paris, then the Hideout and each episode's first level.
- **Sly 2** (`SLY2_ROUTE`, level rows): the episode 1 hub, then the first job world after each episode's hub (rows
  3, 9, 13, 15, 19, 28, 33, 39).
- **Sly 3** (`SLY3_ROUTE`, world slots): v_apt, v_ext, then the hubs o_ext, h_ext, c_ext, p_ext and m_ext.

Each stage holds forward with jumps, attacks and turns; Sly 2 and 3 stages are loaded directly and also switch
character on the d-pad every 4 s. A stage fails when its world is not reached or does not load, it panics (caught;
the route goes on), its state never changes, its host errors grow in its second half, the two runs differ, or (release
builds) its 95th-percentile frame is over 16.7 ms. Each line gives the frame times, the update share's 95th
percentile, the load time, and the script and host error counts with the commonest kinds.

## Sly 2 and Sly 3 game sessions on PC

`slynx_game` also registers `sly2` and `sly3` (OpenGL, offscreen when headless): the game sessions the Switch NROs
run, with captures, for checking lighting, characters and HUD.

```sh
cargo run --release -p game --example slynx_game -- run --app sly2 --opt world=3 --frames 90 --capture 80 --out DIR
cargo run --release -p game --example slynx_game -- run --app sly3 --opt world=v_apt --frames 90 --capture 80 --out DIR
```

`sly2`: `world=ROW` or a world id (`0x3B`), default the hub; `disc=PATH`, default `SLYNX_DISC_SLY2`. `sly3`:
`world=NAME` or `slotN`, default v_apt; `disc=PATH`, default `SLYNX_DISC_SLY3`.

