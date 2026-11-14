// SlyNX Switch platform shim: the C side of the FFI boundary.
//
// This layer only talks to libnx and deko3d. All program logic lives in Rust, which calls these functions
// through `crates/platform-switch/src/sys.rs`. Keep the two in sync: every struct here is mirrored there with
// `#[repr(C)]`.
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// One vertex as the built-in colour shader expects it: position in normalised device coordinates
// (x right, y down, -1..1) and a linear RGBA colour.
typedef struct SlynxVertex {
    float x, y;
    float r, g, b, a;
} SlynxVertex;

// Result codes. 0 is success; anything else is a failure the caller should report.
enum {
    SLYNX_OK = 0,
    SLYNX_ERR_ALREADY_INIT = 1,
    SLYNX_ERR_NOT_INIT = 2,
    SLYNX_ERR_OUT_OF_MEMORY = 3,
    SLYNX_ERR_BAD_ARGUMENT = 4,
};

// Applet and input.
void slynx_input_init(void);
bool slynx_applet_main_loop(void);
void slynx_input_poll(uint64_t* down, uint64_t* held);

// Time: nanoseconds since boot from the system tick counter.
uint64_t slynx_ticks_ns(void);

// Debug output (visible with a debugger or an svcOutputDebugString logger) and abort.
void slynx_debug_log(const char* msg, size_t len);
_Noreturn void slynx_abort(void);

// Graphics: one 1280x720 swapchain, one built-in colour pipeline, triangle lists of up to `max_vertices`.
uint32_t slynx_gfx_width(void);
uint32_t slynx_gfx_height(void);
int slynx_gfx_init(uint32_t max_vertices);
int slynx_gfx_frame(const float clear_rgba[4], const SlynxVertex* vertices, uint32_t count);
void slynx_gfx_exit(void);

#ifdef __cplusplus
}
#endif
