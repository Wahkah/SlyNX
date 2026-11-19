// SlyNX Switch platform shim: the C side of the FFI boundary.
//
// This layer only talks to libnx and deko3d. All program logic lives in Rust, which calls these functions through
// `crates/platform-switch/src/sys.rs`. Keep the two in sync: every struct here is mirrored there with `#[repr(C)]`.
//
// The renderer's C code includes this header to reach the display (section "Display" below): the deko3d device,
// the graphics queue, the swapchain and its images. See docs/platform.md.
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include <deko3d.h>

#ifdef __cplusplus
extern "C" {
#endif

// Result codes for the display and debug-draw functions. 0 is success.
enum {
    SLYNX_OK = 0,
    SLYNX_ERR_ALREADY_INIT = 1,
    SLYNX_ERR_NOT_INIT = 2,
    SLYNX_ERR_OUT_OF_MEMORY = 3,
    SLYNX_ERR_BAD_ARGUMENT = 4,
    SLYNX_ERR_DEVICE = 5,  // a deko3d object could not be created
};

// ---------------------------------------------------------------------------------------------------------------
// Platform: applet, input, time, CPU boost, debug output

// Mounts RomFS (if the .nro has one), configures input for four players and hooks applet messages. Call once.
void slynx_platform_init(void);
void slynx_platform_exit(void);

typedef struct SlynxAppletState {
    uint32_t focus;   // AppletFocusState: 1 in focus, 2 out of focus (overlay/library applet), 3 background
    bool docked;      // AppletOperationMode_Console
    bool resumed;     // the console woke from sleep since the previous call
} SlynxAppletState;

// Processes applet messages. Returns false when the system asks the program to exit.
bool slynx_applet_pump(SlynxAppletState* out);

// One controller reading. Player 0 merges player 1 and the handheld controls.
typedef struct SlynxPadReading {
    uint32_t style;    // HidNpadStyleTag bits (0 when disconnected)
    uint32_t connected;
    uint64_t buttons;  // HidNpadButton bits
    int32_t lx, ly, rx, ry;  // sticks, -32767..32767, y up
} SlynxPadReading;

#define SLYNX_MAX_PLAYERS 4
void slynx_input_read(SlynxPadReading out[SLYNX_MAX_PLAYERS]);

uint64_t slynx_ticks_ns(void);           // nanoseconds since boot, monotonic
int slynx_set_cpu_boost(bool on);        // 0 or a libnx Result
void slynx_debug_log(const char* msg, size_t len);
_Noreturn void slynx_abort(void);

// ---------------------------------------------------------------------------------------------------------------
// Threads

// Starts `entry(arg)` on core `core` (0..2) with priority `prio` (0x00..0x3F, lower runs first; the main thread is
// 0x2C) and a libnx-allocated stack of `stack_size` bytes. Returns an opaque handle, or NULL with *rc_out set.
void* slynx_thread_start(void (*entry)(void*), void* arg, size_t stack_size, int prio, int core, uint32_t* rc_out);
// Waits for the thread to finish and frees the handle.
void slynx_thread_join(void* thread);
// Pins the calling thread to one core. Returns false if the kernel refused.
bool slynx_thread_pin_current(int core);
int slynx_current_core(void);

// ---------------------------------------------------------------------------------------------------------------
// Audio (audout, 48 kHz stereo PCM16)

// Opens the output and queues `buf_count` silent buffers of `frames` sample frames each. Returns 0 or a Result.
uint32_t slynx_audio_open(uint32_t frames, uint32_t buf_count);
// Returns a buffer the device has finished playing (frames * 2 int16 samples), waiting up to `timeout_ns`, or NULL.
int16_t* slynx_audio_acquire(uint64_t timeout_ns);
// Queues a buffer returned by slynx_audio_acquire. Returns 0 or a Result.
uint32_t slynx_audio_submit(int16_t* samples);
void slynx_audio_close(void);

// ---------------------------------------------------------------------------------------------------------------
// Files (newlib on top of libnx's sdmc: and romfs: devices). Error returns are errno values.

bool slynx_romfs_available(void);
int slynx_fs_open_read(const char* path, int* fd_out, uint64_t* size_out);
int64_t slynx_fs_read_at(int fd, uint64_t offset, void* buf, size_t len);  // bytes read, or -errno
void slynx_fs_close(int fd);
bool slynx_fs_exists(const char* path);
int slynx_fs_mkdirs(const char* dir);  // creates every missing folder along `dir`
// Writes `data` to `tmp_path`, then replaces `path` with it.
int slynx_fs_write_replace(const char* path, const char* tmp_path, const void* data, size_t len);
int slynx_fs_remove(const char* path);  // missing file is not an error

// ---------------------------------------------------------------------------------------------------------------
// Display: the deko3d device, graphics queue and swapchain, owned by the platform and lent to the renderer.
//
// The swapchain images are allocated at 1920x1080. Docked, the whole image is shown; handheld, only the top-left
// 1280x720 is (swapchain crop), so the handles never change when the console is docked or undocked. Render at
// `width` x `height` and set viewport and scissor to that size.

#define SLYNX_DISPLAY_MAX_IMAGES 3
#define SLYNX_FB_WIDTH 1920
#define SLYNX_FB_HEIGHT 1080

typedef struct SlynxDisplay {
    DkDevice device;
    DkQueue queue;  // graphics queue; acquire and present go through it
    DkSwapchain swapchain;
    DkMemBlock image_mem;
    DkImage images[SLYNX_DISPLAY_MAX_IMAGES];
    uint32_t image_count;
    uint32_t fb_width, fb_height;  // allocated image size (1920x1080)
    uint32_t width, height;        // current output size: 1920x1080 docked, 1280x720 handheld
    DkImageFormat format;          // DkImageFormat_RGBA8_Unorm
    bool docked;
} SlynxDisplay;

// Default device flags: depth range 0..1 and an upper-left origin.
#define SLYNX_DEFAULT_DEVICE_FLAGS (DkDeviceFlags_DepthZeroToOne | DkDeviceFlags_OriginUpperLeft)

// Creates the device, queue, images and swapchain. `image_count` is 2 or 3.
int slynx_display_init(uint32_t device_flags, uint32_t image_count, bool docked);
// The display, or NULL before init. Valid until slynx_display_exit.
const SlynxDisplay* slynx_display_get(void);
// Switches between docked (1920x1080) and handheld (1280x720) output.
void slynx_display_set_docked(bool docked);
// Acquires the next swapchain image; returns its index. Commands submitted to the queue afterwards wait for it.
int slynx_display_acquire(void);
// Presents image `index` after the commands already submitted to the queue.
void slynx_display_present(int index);
void slynx_display_wait_idle(void);
// Tears down the display. First runs the registered exit hooks, newest first, while the device still exists.
void slynx_display_exit(void);

// Exit hooks: every user of the device that owns GPU objects (memory blocks, command buffers, shaders) registers
// its idempotent exit function here once it is initialised, and removes it in that same exit function. Then
// dropping the platform first shuts the user down before the device goes, and the user's own later exit call finds
// nothing left to do. Adding a hook twice is a no-op. Returns SLYNX_ERR_OUT_OF_MEMORY when all slots are taken.
#define SLYNX_DISPLAY_MAX_EXIT_HOOKS 8
int slynx_display_add_exit_hook(void (*hook)(void));
void slynx_display_remove_exit_hook(void (*hook)(void));

// Rust-side view of the display (raw pointers only, so Rust needs no deko3d types).
typedef struct SlynxDisplayHandles {
    void* device;
    void* queue;
    void* swapchain;
    const void* images;  // SlynxDisplay.images[0]
    uint32_t image_count;
    uint32_t image_stride;  // sizeof(DkImage)
    uint32_t fb_width, fb_height;
    uint32_t width, height;
    uint32_t format;
    bool docked;
} SlynxDisplayHandles;

bool slynx_display_handles(SlynxDisplayHandles* out);

// ---------------------------------------------------------------------------------------------------------------
// Debug draw: one colour pipeline on top of the display, for demos and bring-up.

// Position in normalised device coordinates (x right, y down, -1..1) and an RGBA colour (0..1, stored to the
// RGBA8_Unorm framebuffer as is).
typedef struct SlynxVertex {
    float x, y;
    float r, g, b, a;
} SlynxVertex;

int slynx_dd_init(uint32_t max_vertices);
// Acquires an image, clears, draws `count` vertices as a triangle list at the current output size and presents.
int slynx_dd_frame(const float clear_rgba[4], const SlynxVertex* vertices, uint32_t count);
void slynx_dd_exit(void);

#ifdef __cplusplus
}
#endif
