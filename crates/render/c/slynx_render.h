// SlyNX renderer, deko3d backend: the C side of the FFI boundary.
//
// The platform layer owns the deko3d device, the graphics queue and the swapchain (crates/platform-switch,
// `slynx_display_get()` in slynx_shim.h); the renderer borrows them. Rust builds each frame's packet (instance
// records, draw commands, batches) and hands it over in one call; this layer copies it into the frame slot's GPU
// memory, records one command list, submits it and presents.
//
// Mirrored in crates/render/src/deko3d.rs with #[repr(C)]; keep the two in sync.
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SLYNX_RENDER_MAX_BATCHES 4096u

enum {
    SLYNX_RENDER_OK = 0,
    SLYNX_RENDER_ERR_ALREADY_INIT = 1,
    SLYNX_RENDER_ERR_NOT_INIT = 2,
    SLYNX_RENDER_ERR_OUT_OF_MEMORY = 3,
    SLYNX_RENDER_ERR_BAD_ARGUMENT = 4,
    SLYNX_RENDER_ERR_TOO_LARGE = 5,
    SLYNX_RENDER_ERR_NO_LEVEL = 6,
    SLYNX_RENDER_ERR_NO_DISPLAY = 7,
};

// One texture pool: layer_count layers of width x height with mip_levels levels each. layers[i] points at layer
// i's texels, all levels packed one after another (level 0 first), layer_bytes bytes. format 0 = RGBA8. clamp: 1 to
// sample with clamp-to-edge instead of repeat.
typedef struct SlynxRenderPool {
    uint32_t width, height, mip_levels, layer_count, format, layer_bytes, clamp;
    const uint8_t* const* layers;
} SlynxRenderPool;

// Same layout as DkDrawIndexedIndirectData and the Rust DrawCmd.
typedef struct SlynxRenderDraw {
    uint32_t index_count, instance_count, first_index;
    int32_t base_vertex;
    uint32_t base_instance;
} SlynxRenderDraw;

// Batch state bits (the Rust STATE_* constants in src/frame.rs).
#define SLYNX_RENDER_DEPTH_MASK 3u          // 0 less, 1 less-or-equal, 2 always (no depth test)
#define SLYNX_RENDER_DEPTH_WRITE (1u << 2)
// Blending: 0 none, 1 alpha over, 2 additive, 3 reverse alpha (src (1 - As) + dst As), 4 reverse additive
// (src + dst As), 5 by destination alpha (src Ad + dst (1 - Ad)), 6 by inverse destination alpha
// (src (1 - Ad) + dst Ad), 7 multiply (dst src, the alpha kept). The destination alpha otherwise takes the source
// alpha.
#define SLYNX_RENDER_BLEND_MASK (7u << 3)
#define SLYNX_RENDER_NO_CULL (1u << 6)
#define SLYNX_RENDER_FRONT_CW (1u << 7)     // mirrored instances: front faces are clockwise on screen
#define SLYNX_RENDER_CULL_FRONT (1u << 8)
#define SLYNX_RENDER_STENCIL_MASK (3u << 9) // 1: count volume surfaces, 2: paint where the count is not zero
#define SLYNX_RENDER_NO_COLOR (1u << 11)
#define SLYNX_RENDER_ALPHA_TEST (1u << 12)  // the alpha-tested fragment shader
#define SLYNX_RENDER_SCREEN (1u << 13)
#define SLYNX_RENDER_ALPHA_ONLY (1u << 14)  // colour writes to the alpha channel only

typedef struct SlynxRenderBatch {
    uint32_t state;  // SLYNX_RENDER_* state bits
    uint32_t pool;
    uint32_t first_draw, draw_count;
    uint32_t pass;   // the renderer's pass (for statistics)
} SlynxRenderBatch;

// One part of a frame drawn with its own frame constants into a rectangle of the target (picture panels).
#define SLYNX_RENDER_MAX_VIEWS 8u
#define SLYNX_RENDER_VIEW_CLEAR_DEPTH 1u  // clear depth and stencil inside the viewport
#define SLYNX_RENDER_VIEW_CLEAR_COLOR 2u  // clear colour inside the viewport
typedef struct SlynxRenderView {
    float uniforms[36];        // 144 bytes (std140 block "Frame")
    uint32_t viewport[4];      // x, y from the top left, width, height in pixels
    float clear_color[4];
    uint32_t clear;            // SLYNX_RENDER_VIEW_CLEAR_* bits
    uint32_t first_batch, batch_count;
    uint32_t target;           // 0 the frame's target, 1 the silhouette image (SLYNX_RENDER_SILHOUETTE_SIZE square),
                               // 2 and 3 the panel images (SLYNX_RENDER_PANEL_IMAGE_SIZE square, with depth)
} SlynxRenderView;
// The projectors' silhouette image: drawn by views with target 1, sampled at fragment texture binding 1.
#define SLYNX_RENDER_SILHOUETTE_SIZE 64u
// The picture panels' images: drawn by views with target 2 and 3, sampled at fragment texture bindings 5 and 6.
#define SLYNX_RENDER_PANEL_IMAGE_SIZE 512u

typedef struct SlynxRenderFrame {
    const void* uniforms;   // 144 bytes (std140 block "Frame")
    const void* instances;  // instance_count records of 80 bytes, in slot order
    uint32_t instance_count;
    uint32_t draw_count;
    const void* bones;      // bone_count matrices of 48 bytes (storage block 1)
    const void* morph;      // morph_count vertices of 32 bytes (storage block 2)
    uint32_t bone_count;
    uint32_t morph_count;
    const SlynxRenderDraw* draws;
    const SlynxRenderBatch* batches;
    uint32_t batch_count;
    uint32_t cull;  // 0 none, 1 back, 2 front (front faces are counter-clockwise on screen)
    float clear[4];
    // No views: every batch over the whole target with `uniforms`. Otherwise the views in order, after the clear.
    const SlynxRenderView* views;
    uint32_t view_count;
    uint32_t reserved;
    // A new movie frame to upload, or video_width 0: Y (width x height), Cb and Cr ((width + 1) / 2 x
    // (height + 1) / 2), rows top first. Sampled at fragment texture bindings 2 to 4.
    const uint8_t* video_planes[3];
    uint32_t video_width, video_height;
} SlynxRenderFrame;
#define SLYNX_RENDER_MAX_VIDEO_SIZE 2048u

typedef struct SlynxRenderStats {
    uint32_t api_draw_calls;
    uint32_t cmd_bytes_budget;  // command memory per frame slot
    uint32_t cmd_bytes_used;    // command memory this frame used, rounded up to 16 KiB chunks; above the budget when
                                // the slot ran out and more was allocated
    uint32_t reserved;
    uint64_t wait_ns;           // time blocked on the frame slot's fence
    uint64_t gpu_ns;            // GPU time of the latest finished frame (FRAME_SLOTS behind); 0 when unknown
} SlynxRenderStats;

// Creates the renderer on the platform's display (slynx_display_init must have run). max_instances bounds the
// instance slot stream and per-frame instance memory; max_draws bounds per-frame command memory. A frame may have
// at most max_draws draw commands, min(max_draws, SLYNX_RENDER_MAX_BATCHES) batches, max_bones bone matrices and
// max_morph morphed vertices.
int slynx_render_init(uint32_t max_instances, uint32_t max_draws, uint32_t max_bones, uint32_t max_morph);
// Current output size (follows docked/handheld).
void slynx_render_size(uint32_t* width, uint32_t* height);
// Replaces the level's geometry arena and texture pools. Waits for the GPU to go idle. extras holds the second
// vertex stream, 16 bytes per vertex (bones u8 x4, weights unorm16 x4, index in the mesh u16, padding); weights the
// per-instance vertex weights, 8 bytes each (storage block 3); outline_points the outline edge points, 16 bytes
// each (storage block 4).
int slynx_render_upload_level(const void* vertices, uint32_t vertex_bytes, const void* extras, const void* weights,
                              uint32_t weight_count, const void* outline_points, uint32_t outline_count,
                              const uint16_t* indices, uint32_t index_count,
                              const SlynxRenderPool* pools, uint32_t pool_count);
// Draws and presents one frame. Command memory grows past the budget when needed (see the stats). If no more can
// be allocated, the frame is still presented without the draws that did not fit, and the call returns
// SLYNX_RENDER_ERR_OUT_OF_MEMORY. A draw outside the arena or the frame's instances is SLYNX_RENDER_ERR_BAD_ARGUMENT.
int slynx_render_frame(const SlynxRenderFrame* frame, SlynxRenderStats* stats);
// Waits for the GPU and frees everything the renderer allocated (the display stays).
void slynx_render_exit(void);

#ifdef __cplusplus
}
#endif
