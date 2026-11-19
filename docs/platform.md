# The platform layer

The engine, the renderer, the harness and the demo run on one platform API with two backends:

| Crate | Where | Built on |
|---|---|---|
| `platform-api` (`crates/platform-api/`) | everywhere | `no_std` + `alloc`, no FFI: the `Platform` trait and shared types |
| `platform-switch` (`crates/platform-switch/`) | Nintendo Switch | libnx and deko3d through a small C shim (`c/`) |
| `platform-pc` (`crates/platform-pc/`) | Linux, Windows | SDL2 and OpenGL 4.3 core (EGL for headless on Linux), hand-written FFI |

Depend on the API with `platform-api = { path = "../platform-api" }` (from another crate in `crates/`).

Code that runs on both picks the backend once:

```rust
#[cfg(target_os = "horizon")]
type Host = platform_switch::SwitchPlatform;
#[cfg(not(target_os = "horizon"))]
type Host = platform_pc::PcPlatform;
```

and is otherwise generic over `P: platform_api::Platform`. Only the graphics hand-off is backend-specific, because
the renderer has one backend per graphics API anyway.

## The frame loop

```rust
let mut platform = SwitchPlatform::new(SwitchConfig::default())?;   // or PcPlatform::new(PcConfig::default())
while platform.pump() {                       // system messages + controllers; false = exit requested
    while let Some(event) = platform.poll_event() { /* see Events */ }
    let input = platform.input();             // this frame's controllers
    // update, render, present (renderer-specific, see below)
}
```

Only one platform may exist at a time. It is not `Send`; keep it on the thread that created it (the game thread).

## `Platform` at a glance

| Method | Switch | PC |
|---|---|---|
| `pump`, `poll_event` | applet messages (`appletMainLoop`), focus, operation mode, HID | SDL events, window, keyboard, gamepads |
| `input`, `set_deadzone` | 4 players: handheld, Joy-Con pair or single, Pro Controller | 4 gamepads, keyboard on player 1, or scripted input |
| `display` | 1920x1080 docked, 1280x720 handheld | window or offscreen size; "docked" at 1920x1080 or larger |
| `now_ns` | system tick counter, monotonic | `Instant`, or a fixed step per `pump` (deterministic runs) |
| `set_cpu_boost` | `appletSetCpuBoostMode(FastLoad)` | accepted, no effect |
| `fs` | `romfs:/`, `sdmc:/`, `sdmc:/switch/SlyNX/user/` | three folders (below) |
| `start_audio`, `stop_audio`, `audio_latency_ns` | audout, mixer thread on core 2, 32 ms | SDL audio device, about 21 ms |
| `spawn`, `pin_current_thread`, `current_core` | libnx threads created on their core | `std::thread`, optional pinning |

### Events

`Quit` (pump returns `false` from then on), `FocusLost` / `FocusGained` (Switch: overlay or library applet; PC:
window focus), `Suspended` / `Resumed` (Switch: HOME menu or sleep, and waking up; PC: minimise and restore),
`DisplayChanged(DisplayInfo)` (docked/undocked, window resized), `ControllerConnected(player)`,
`ControllerDisconnected(player)`. Pause the game and silence audio on `Suspended`.

### Input

`InputState::players[0..4]`, each a `PadState`: `kind` (`ControllerKind`: `None`, `Handheld`, `JoyConPair`,
`JoyConLeft`, `JoyConRight`, `ProController`, `Gamepad`, `Keyboard`), `held`, `pressed` (went down this frame),
`released`, and the two sticks after the deadzone (`x` right, `y` up, -1..1).

- **Buttons** are bits in `Buttons`: A (right face), B (bottom), X (top), Y (left), L, R, ZL, ZR, + and -, the D-pad,
  stick clicks LS and RS, and SL and SR from either Joy-Con. The low 16 bits match the Switch's HID report.
- **Player 1** also gets the console's attached Joy-Cons, so handheld play needs no setup. A single Joy-Con reports
  its buttons and stick in its upright orientation; turning it sideways is up to the game.
- **Deadzone**: radial, default inner 0.15 and outer 0.95, rescaled so movement starts smoothly at the inner edge.
  Change it with `set_deadzone`.
- **Action mapping**: `ActionMap` binds game actions (numbers 0..64) to buttons or stick directions, several
  bindings per action:

  ```rust
  const JUMP: u8 = 0;
  let mut map = ActionMap::new(0);                       // player 1
  map.bind(JUMP, Binding::Button(Buttons::B));
  map.bind(MOVE_LEFT, Binding::Stick(StickId::Left, Dir::Left));
  let actions = map.update(platform.input());            // once per frame
  if actions.pressed(JUMP) { /* ... */ }
  ```

**PC keys (player 1):** WASD left stick, IJKL right stick, arrows D-pad, X = A, Z = B, V = X, C = Y, Q = L, E = R,
1 = ZL, 3 = ZR, Enter = +, Backspace = -. Esc quits, F11 switches the window between 1280x720 and 1920x1080.
Gamepads map by position, so the bottom face button is B on every pad.

### Audio

Output is 48 kHz stereo on both. The game hands `start_audio` an `AudioRender` (a closure
`FnMut(&mut [f32]) + Send` works): it is called on the audio thread to fill each buffer with interleaved `f32`
samples in -1..1 (the buffer arrives zeroed). It must not block or do I/O; talk to it through atomics or a lock-free
queue.

- **Switch:** audout, PCM16. The audio thread (core 2, above the game thread's priority) keeps a ring of 4 buffers
  of 384 frames (8 ms) queued on the device; each time one is released it calls the renderer, converts to `i16` and
  queues it again. Worst-case latency 32 ms.
- **PC:** an SDL audio device with 512-frame buffers; SDL's audio thread calls the renderer. About 21 ms.
- Headless runs have audio off (`start_audio` returns `AudioError::Disabled`).

### Files

`platform.fs()` returns a `FileSystem` handle that is cheap to clone and `Send`, so the loader thread on core 1 can
own one. Paths are relative and `/`-separated; `..`, `.`, empty parts, `\` and `:` are refused (`FsError::BadPath`).

| `Root` | Switch | PC (default, override with env) |
|---|---|---|
| `RomFs` (read-only) | the `.nro`'s RomFS, `romfs:/` | `romfs/` (`SLYNX_ROMFS_DIR`) |
| `Sd` (read-only through this API) | `sdmc:/` | `sdmc/` (`SLYNX_SD_DIR`) |
| `User` (saves, settings) | `sdmc:/switch/SlyNX/user/` | `$XDG_DATA_HOME/slynx`, `~/.local/share/slynx` or `%APPDATA%\SlyNX` (`SLYNX_USER_DIR`) |

- `open` gives a `DataFile` with `len` and positioned reads (`read_at`, `read_exact_at`); `read` loads a whole
  file in one sequential pass. Large reads go straight to the file system (no stdio buffering on the Switch).
- `write_user` creates parent folders, writes `<path>.tmp`, then replaces `<path>`. The Switch's SD card cannot
  rename over an existing file, so the old file is removed first; if the program dies in between, `open` of the
  path falls back to the `.tmp` file. Either way a reader sees the old or the new contents, never a mix.
- If the `.nro` has no RomFS, `Root::RomFs` fails with `FsError::NoRoot`.

### Threads and cores

The Switch gives a homebrew program cores 0, 1 and 2. `Core::Game` (0) runs the game and rendering,
`Core::Io` (1) loading, `Core::Audio` (2) the mixer. `SwitchPlatform::new` pins the calling thread to core 0;
`spawn(ThreadSpec::new("loader", Core::Io).priority(Priority::Low), job)` creates a thread on its core. Dropping a
thread handle waits for the thread. On the PC pinning is off unless `PcConfig::pin_threads` (or
`SLYNX_PIN_THREADS=1`).

### CPU boost

`set_cpu_boost(true)` during loading screens raises the CPU clock (the GPU is throttled meanwhile), `false` after.
The platform turns it off when it is dropped.

## Graphics hand-off

### Switch: deko3d

The platform owns the deko3d device, one graphics queue, and a swapchain of 3 RGBA8 images allocated at 1920x1080.
Handheld output crops to the top-left 1280x720, so **the handles never change** when the console is docked or
undocked; only the render size does. Render at `display().width` x `display().height` and set viewport and scissor
to that size.

The default device flags are depth 0..1 and an upper-left origin (`SwitchConfig::device_flags` changes them).

From C (the renderer's code in `crates/render/c/`), include the shim header and read the display:

```c
#include "slynx_shim.h"   // crates/platform-switch/c; pulls in deko3d.h

const SlynxDisplay* d = slynx_display_get();   // NULL before the platform exists
DkDevice dev = d->device;
DkQueue  q   = d->queue;
// d->images[0 .. d->image_count), d->fb_width x d->fb_height (1920x1080), d->format,
// d->width x d->height: the current output size.

int slot = slynx_display_acquire();            // dkQueueAcquireImage on the display queue
/* record commands rendering into d->images[slot], submit them to d->queue */
slynx_display_present(slot);                   // dkQueuePresentImage
```

Read `width`/`height` every frame. Submit the frame's work to `d->queue`: acquire inserts the wait for the image
there, and present follows the commands already submitted. More queues (compute, transfer) can be made from
`d->device` and synchronised by the renderer.

From Rust, `SwitchPlatform::display_handles()` returns the same objects as raw pointers (`DisplayHandles`:
`device`, `queue`, `swapchain`, `images` with `image_stride`, sizes, format, `docked`), and `acquire_image()` /
`present(index)` / `gpu_wait_idle()` wrap the C calls, for passing to the renderer's C functions.

The renderer's C files are compiled and linked by `tools/build-switch.sh` next to the shim (`SLYNX_EXTRA_C`, see
`docs/switch-build.md`). Do not call `slynx_display_exit` yourself.

#### Shutdown order: display exit hooks

Every user of the device that owns GPU objects (memory blocks, command buffers, shaders, fences) must be shut down
before the device is destroyed, whichever is dropped first: the platform or the user. The platform keeps a list of
exit hooks for that:

```c
int  slynx_display_add_exit_hook(void (*hook)(void));    // at most SLYNX_DISPLAY_MAX_EXIT_HOOKS (8)
void slynx_display_remove_exit_hook(void (*hook)(void));
```

- A GPU user registers its exit function right after it initialised successfully, and its exit function removes
  the hook first thing. The exit function must be idempotent (do nothing when not initialised).
- Dropping `SwitchPlatform` calls `slynx_display_exit`, which runs the remaining hooks newest first while the device
  still exists, then destroys the queue, swapchain, images and device. A user dropped afterwards finds itself
  already shut down and does nothing.
- A user dropped before the platform runs its exit function normally, which removes the hook.
- From Rust: `SwitchPlatform::add_display_exit_hook` / `remove_display_exit_hook` (the first is `unsafe`: the
  function must meet the rules above).

`SwitchDebugDraw` uses this already. The renderer's deko3d backend does it with two lines in
`crates/render/c/slynx_render.c`:

- in `slynx_render_init`, right after `g.ready = true;` (so `slynx_render_exit` can free everything on failure):
  `if (slynx_display_add_exit_hook(slynx_render_exit) != SLYNX_OK) { slynx_render_exit(); return SLYNX_RENDER_ERR_OUT_OF_MEMORY; }`
- first line of `slynx_render_exit`: `slynx_display_remove_exit_hook(slynx_render_exit);`.

Until that change lands, `slynx_display_exit` also calls `slynx_render_exit` through a weak reference (resolved
when the renderer's C code is linked, NULL otherwise), so the renderer is shut down before the device either way.
The weak call goes once the renderer registers its hook; calling it after the hook already ran is harmless because
`slynx_render_exit` is idempotent.

### PC: OpenGL 4.3 core

`PcPlatform::gl()` returns a `GlContext`:

- `get_proc_address(c"glDrawElements")` loads GL functions (`platform_pc::get_proc_address_c` is the same as an
  `extern "C"` function pointer for C-style loaders);
- `default_framebuffer()`: the framebuffer the final image goes to. 0 with a window; an offscreen framebuffer
  (RGBA8 colour, depth 24 / stencil 8) when headless. Bind this instead of 0;
- `size()`: the drawable size; `describe()`: `GL_VERSION` and `GL_RENDERER`.

`PcPlatform::present()` swaps the window (vsync by default) or flushes when headless. GL's clip space has y up and
depth -1..1; to match the Switch conventions use `glClipControl(GL_UPPER_LEFT, GL_ZERO_TO_ONE)` (GL 4.5, or
`ARB_clip_control`; Mesa has it). The context is current on the platform's thread only.

### Headless (tests, harness, image comparison)

`PcConfig::headless(w, h)` (or `SLYNX_HEADLESS=1`): no visible window, no audio, no controllers, a fixed 60 Hz clock.
On Linux the context comes from EGL's surfaceless platform (Mesa), which needs no X or Wayland display; otherwise,
or if that fails, from a hidden SDL window. Either way rendering goes to the offscreen framebuffer and
`read_pixels()` returns it as RGBA8, rows top to bottom (call it before `present` when there is a window).
`set_scripted_input(Some(pads))` replaces the controllers for deterministic runs.

### Debug draw

Both backends have a tiny triangle renderer for demos and bring-up: `SwitchDebugDraw::new(&platform, max)` and
`platform.debug_draw(max)` on the PC, both implementing `DebugDraw::frame(clear, &vertices)` (clear, draw, present).
Vertices are `Vertex { pos: [x, y], color: [r, g, b, a] }` with x right and y down in -1..1 on both backends.
Colours go to the 8-bit framebuffer as given (0.5 stores 128; no gamma conversion). Do not use it together with the
real renderer.

## The demo

`crates/hello-switch` is written once against `Platform` and runs on both: every player's controller live, a tone
on A, B, X or Y, a RomFS file read on the I/O core, a launch counter in the user folder, CPU boost on ZR, the display
mode, applet events and the frame rate.

```sh
tools/build-switch.sh                                         # dist/slynx-hello.nro
cargo run -p hello-switch --example hello_pc                  # PC window (WSLg works)
cargo run -p hello-switch --example hello_pc -- --headless 120 out.ppm
```

## Building and testing

`platform-pc` links `libSDL2` (and `libEGL` on Linux) from the system: on Debian/Ubuntu `libsdl2-dev` and
`libegl-dev` (Mesa). On Windows put `SDL2.lib` on the linker path and `SDL2.dll` next to the program.

```sh
cargo test --workspace                     # includes the headless GL test (works with no display)
cargo clippy --workspace
SWITCH="-Z build-std=core,alloc --target aarch64-nintendo-switch-freestanding"
cargo clippy -p platform-api -p platform-switch -p hello-switch -p render -p viewer $SWITCH
cargo clippy -p render --features switch-demo $SWITCH    # the renderer demo's entry point
cargo clippy -p formats --no-default-features $SWITCH    # formats is no_std without its std feature
```
