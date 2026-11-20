// SlyNX renderer, deko3d backend.
//
// Design (see docs/renderer.md):
// - one geometry arena per level: a vertex region and a 16-bit index region in one memory block, bound once;
// - a static per-instance stream 0, 1, 2, ... (divisor 1) that each draw's first instance offsets, so the vertex
//   shader finds its instance record in one storage buffer bound once per frame;
// - texture pools as 2D array images with full mip chains, addressed through one image descriptor set; a batch
//   binds one combined texture handle;
// - per frame slot (3): uniforms + instance records in GPU memory, command memory and a fence. A slot is reused only
//   after its fence signals, so the CPU never writes memory the GPU may still read.
// Per draw command the CPU records one dkCmdBufDrawIndexed and nothing else.
#include "slynx_render.h"

#include <stdlib.h>
#include <string.h>

#include <switch.h>
#include <deko3d.h>

#include "slynx_shim.h"

#define FRAME_SLOTS 3
#define UNIFORM_BYTES 256u
#define INSTANCE_BYTES 80u
#define WEIGHT_BYTES 8u
#define OUTLINE_POINT_BYTES 16u
#define UNIFORM_COPY_BYTES 144u
// Uniform blocks in a frame slot: the frame's, then one per view.
#define UNIFORM_REGION_BYTES (UNIFORM_BYTES * (1u + SLYNX_RENDER_MAX_VIEWS))
#define CMD_BYTES_PER_VIEW 512u
#define BONE_BYTES 48u
#define MORPH_BYTES 32u
#define EXTRA_STRIDE 16u
#define CMD_BASE_BYTES (64u * 1024u)
#define CMD_BYTES_PER_DRAW 64u
#define CMD_BYTES_PER_BATCH 256u
#define MAX_POOLS 1024u
#define VERTEX_STRIDE 28u
// Command memory is handed to the command buffer in chunks of this size (from the slot's region, then from extra
// blocks), so the bytes a frame used are known to within one chunk.
#define CMD_CHUNK_BYTES (16u * 1024u)
// Kept back to finish a frame (fence, end of list) when no more command memory can be had.
#define CMD_RESERVE_BYTES (64u * 1024u)
#define MAX_EXTRA_CMD_BLOCKS 8u

// Front-face convention. With the y-flipped projection the renderer uses for the upper-left origin, a triangle that
// is counter-clockwise in the world stays counter-clockwise on screen, which deko3d (like Vulkan) is expected to
// treat as front-facing. Not yet confirmed on hardware: if culling removes the visible faces, build with
// -DSLYNX_RENDER_FRONT_FACE_CW.
#ifdef SLYNX_RENDER_FRONT_FACE_CW
#define FRONT_FACE DkFrontFace_CW
#else
#define FRONT_FACE DkFrontFace_CCW
#endif

extern const uint8_t slynx_render_mesh_vsh[], slynx_render_mesh_vsh_end[];
extern const uint8_t slynx_render_mesh_fsh[], slynx_render_mesh_fsh_end[];
extern const uint8_t slynx_render_mesh_alpha_fsh[], slynx_render_mesh_alpha_fsh_end[];

// GPU timestamp reports per frame slot: two of 16 bytes (value, timestamp).
#define QUERY_SLOT_BYTES 32u

static struct {
    bool ready;
    DkDevice device;
    DkQueue queue;

    DkMemBlock code_mem;
    DkShader vsh, fsh, alpha_fsh;

    DkMemBlock depth_mem;
    DkImage depth;
    DkMemBlock silhouette_mem;
    DkImage silhouette;  // RGBA8, render target and texture (descriptor after the pools')
    DkMemBlock panel_mem;
    DkImage panel_color[2], panel_depth[2];  // RGBA8 (descriptors after the video planes') and Z24S8
    // The movie picture: three R8 planes (descriptors after the silhouette's), and per frame slot a staging region
    // the planes are copied through.
    DkMemBlock video_mem, video_stage_mem;
    DkImage video_img[3];
    uint32_t video_w, video_h, video_stage_bytes;
    bool video_desc_dirty;

    uint32_t max_instances, max_draws, max_batches, max_bones, max_morph;
    // Offsets in a frame slot: uniforms at 0, instances at UNIFORM_BYTES, then bones and morphed vertices.
    uint32_t bone_offset, morph_offset;
    DkMemBlock slot_stream_mem;  // u32 0..max_instances-1

    DkMemBlock frame_mem;  // FRAME_SLOTS x (uniforms + instances)
    uint32_t frame_slot_bytes;
    DkMemBlock cmd_mem;  // FRAME_SLOTS x command memory, then FRAME_SLOTS x a reserve
    uint32_t cmd_slot_bytes;
    DkCmdBuf cmdbuf;
    // Command memory of the frame being recorded: cursor into the slot's region, extra blocks made when the region
    // ran out (freed when the slot comes round again), and whether the reserve had to be used.
    uint32_t cmd_slot, cmd_cursor, cmd_extra_bytes;
    bool cmd_exhausted;
    DkMemBlock cmd_extra[FRAME_SLOTS][MAX_EXTRA_CMD_BLOCKS];
    DkFence fence[FRAME_SLOTS];
    bool fence_pending[FRAME_SLOTS];
    uint64_t frame;
    // GPU timestamps: per frame slot, two 16-byte reports (top of the pipe at the start, end of the pipe at the end),
    // read once the slot's fence has passed.
    DkMemBlock query_mem;
    uint64_t last_gpu_ns;

    // Level data.
    bool have_level;
    DkMemBlock arena_mem;
    uint32_t outline_offset, outline_bytes;
    uint8_t* pool_sampler;  // per pool: 0 repeat, 1 clamp-to-edge (sampler descriptor index)
    uint32_t vertex_bytes, extra_offset, weights_offset, weights_bytes, index_offset, index_bytes, vertex_count,
        index_count;
    DkMemBlock image_mem;
    DkImage* pool_images;
    uint32_t pool_count;
    DkMemBlock desc_mem;  // pool_count image descriptors, then two sampler descriptors (repeat, clamp)
    uint32_t sampler_desc_offset;
} g;

static uint32_t align_up(uint32_t v, uint32_t a) { return (v + a - 1) & ~(a - 1); }

static DkMemBlock make_block(uint32_t size, uint32_t flags) {
    DkMemBlockMaker m;
    dkMemBlockMakerDefaults(&m, g.device, align_up(size ? size : 1, DK_MEMBLOCK_ALIGNMENT));
    m.flags = flags;
    return dkMemBlockCreate(&m);
}

static void destroy_block(DkMemBlock* b) {
    if (*b) {
        dkMemBlockDestroy(*b);
        *b = NULL;
    }
}

static void load_shader(DkShader* shader, const uint8_t* begin, const uint8_t* end, uint32_t* offset) {
    uint32_t size = (uint32_t)(end - begin);
    memcpy((uint8_t*)dkMemBlockGetCpuAddr(g.code_mem) + *offset, begin, size);
    DkShaderMaker sm;
    dkShaderMakerDefaults(&sm, g.code_mem, *offset);
    dkShaderInitialize(shader, &sm);
    *offset = align_up(*offset + size, DK_SHADER_CODE_ALIGNMENT);
}

static void free_cmd_extra(uint32_t slot) {
    for (uint32_t i = 0; i < MAX_EXTRA_CMD_BLOCKS; i++) destroy_block(&g.cmd_extra[slot][i]);
}

// Called by deko3d when the command buffer needs more memory. Order: the next chunk of the slot's region; a new
// block (the budget was too small: rendering goes on and the stats say so); the slot's reserve, after which the
// frame stops recording draws.
static void cmd_add_mem(void* user, DkCmdBuf cb, size_t min_req) {
    (void)user;
    uint32_t want = align_up((uint32_t)min_req > CMD_CHUNK_BYTES ? (uint32_t)min_req : CMD_CHUNK_BYTES,
                             DK_CMDMEM_ALIGNMENT);
    if (g.cmd_cursor + want <= g.cmd_slot_bytes) {
        dkCmdBufAddMemory(cb, g.cmd_mem, g.cmd_slot * g.cmd_slot_bytes + g.cmd_cursor, want);
        g.cmd_cursor += want;
        return;
    }
    if (g.cmd_cursor < g.cmd_slot_bytes && g.cmd_slot_bytes - g.cmd_cursor >= (uint32_t)min_req) {
        // The rest of the region is smaller than a chunk but enough for this request.
        uint32_t rest = g.cmd_slot_bytes - g.cmd_cursor;
        dkCmdBufAddMemory(cb, g.cmd_mem, g.cmd_slot * g.cmd_slot_bytes + g.cmd_cursor, rest);
        g.cmd_cursor = g.cmd_slot_bytes;
        return;
    }
    g.cmd_cursor = g.cmd_slot_bytes;
    uint32_t grow = align_up(want > g.cmd_slot_bytes / 4 ? want : g.cmd_slot_bytes / 4, DK_CMDMEM_ALIGNMENT);
    for (uint32_t i = 0; i < MAX_EXTRA_CMD_BLOCKS && !g.cmd_exhausted; i++) {
        if (g.cmd_extra[g.cmd_slot][i]) continue;
        DkMemBlock b = make_block(grow, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
        if (!b) break;
        g.cmd_extra[g.cmd_slot][i] = b;
        g.cmd_extra_bytes += grow;
        dkCmdBufAddMemory(cb, b, 0, grow);
        return;
    }
    // Last resort: the reserve, enough to finish the frame. Draw recording stops (see slynx_render_frame).
    g.cmd_exhausted = true;
    uint32_t reserve = FRAME_SLOTS * g.cmd_slot_bytes + g.cmd_slot * CMD_RESERVE_BYTES;
    dkCmdBufAddMemory(cb, g.cmd_mem, reserve, CMD_RESERVE_BYTES);
}

static void free_level(void) {
    destroy_block(&g.arena_mem);
    destroy_block(&g.image_mem);
    destroy_block(&g.desc_mem);
    free(g.pool_images);
    g.pool_images = NULL;
    free(g.pool_sampler);
    g.pool_sampler = NULL;
    g.pool_count = 0;
    g.have_level = false;
}

int slynx_render_init(uint32_t max_instances, uint32_t max_draws, uint32_t max_bones, uint32_t max_morph) {
    if (g.ready) return SLYNX_RENDER_ERR_ALREADY_INIT;
    const SlynxDisplay* d = slynx_display_get();
    if (!d) return SLYNX_RENDER_ERR_NO_DISPLAY;
    if (max_instances == 0 || max_instances > (1u << 20) || max_draws == 0 || max_draws > (1u << 20) ||
        max_bones > (1u << 20) || max_morph > (1u << 20))
        return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    memset(&g, 0, sizeof(g));
    g.device = d->device;
    g.queue = d->queue;
    g.max_instances = max_instances;
    g.max_draws = max_draws;
    g.max_bones = max_bones;
    g.max_morph = max_morph;
    g.max_batches = max_draws < SLYNX_RENDER_MAX_BATCHES ? max_draws : SLYNX_RENDER_MAX_BATCHES;

    // Shaders. Code memory needs DK_SHADER_CODE_UNUSABLE_SIZE spare bytes after the last shader.
    uint32_t code_size = align_up((uint32_t)(slynx_render_mesh_vsh_end - slynx_render_mesh_vsh), DK_SHADER_CODE_ALIGNMENT) +
                         align_up((uint32_t)(slynx_render_mesh_fsh_end - slynx_render_mesh_fsh), DK_SHADER_CODE_ALIGNMENT) +
                         align_up((uint32_t)(slynx_render_mesh_alpha_fsh_end - slynx_render_mesh_alpha_fsh),
                                  DK_SHADER_CODE_ALIGNMENT) +
                         DK_SHADER_CODE_UNUSABLE_SIZE;
    g.code_mem = make_block(code_size, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached | DkMemBlockFlags_Code);
    if (!g.code_mem) return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    uint32_t off = 0;
    load_shader(&g.vsh, slynx_render_mesh_vsh, slynx_render_mesh_vsh_end, &off);
    load_shader(&g.fsh, slynx_render_mesh_fsh, slynx_render_mesh_fsh_end, &off);
    load_shader(&g.alpha_fsh, slynx_render_mesh_alpha_fsh, slynx_render_mesh_alpha_fsh_end, &off);

    // Depth buffer at the allocated framebuffer size; frames use its top-left width x height.
    DkImageLayoutMaker lm;
    dkImageLayoutMakerDefaults(&lm, g.device);
    lm.flags = DkImageFlags_UsageRender | DkImageFlags_HwCompression;
    lm.format = DkImageFormat_Z24S8;
    lm.dimensions[0] = d->fb_width;
    lm.dimensions[1] = d->fb_height;
    DkImageLayout layout;
    dkImageLayoutInitialize(&layout, &lm);
    g.depth_mem = make_block((uint32_t)dkImageLayoutGetSize(&layout), DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
    if (!g.depth_mem) goto oom;
    dkImageInitialize(&g.depth, &layout, g.depth_mem, 0);

    // The projectors' silhouette image.
    dkImageLayoutMakerDefaults(&lm, g.device);
    lm.flags = DkImageFlags_UsageRender;
    lm.format = DkImageFormat_RGBA8_Unorm;
    lm.dimensions[0] = SLYNX_RENDER_SILHOUETTE_SIZE;
    lm.dimensions[1] = SLYNX_RENDER_SILHOUETTE_SIZE;
    DkImageLayout sil_layout;
    dkImageLayoutInitialize(&sil_layout, &lm);
    g.silhouette_mem =
        make_block((uint32_t)dkImageLayoutGetSize(&sil_layout), DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
    if (!g.silhouette_mem) goto oom;
    dkImageInitialize(&g.silhouette, &sil_layout, g.silhouette_mem, 0);

    // The panel images: colour and depth, one block.
    {
        DkImageLayout pl[4];
        uint32_t offs[4], bytes = 0;
        for (uint32_t k = 0; k < 4; k++) {
            dkImageLayoutMakerDefaults(&lm, g.device);
            lm.flags = DkImageFlags_UsageRender | (k >= 2 ? DkImageFlags_HwCompression : 0);
            lm.format = k < 2 ? DkImageFormat_RGBA8_Unorm : DkImageFormat_Z24S8;
            lm.dimensions[0] = SLYNX_RENDER_PANEL_IMAGE_SIZE;
            lm.dimensions[1] = SLYNX_RENDER_PANEL_IMAGE_SIZE;
            dkImageLayoutInitialize(&pl[k], &lm);
            bytes = align_up(bytes, (uint32_t)dkImageLayoutGetAlignment(&pl[k]));
            offs[k] = bytes;
            bytes += (uint32_t)dkImageLayoutGetSize(&pl[k]);
        }
        g.panel_mem = make_block(bytes, DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
        if (!g.panel_mem) goto oom;
        for (uint32_t k = 0; k < 2; k++) {
            dkImageInitialize(&g.panel_color[k], &pl[k], g.panel_mem, offs[k]);
            dkImageInitialize(&g.panel_depth[k], &pl[2 + k], g.panel_mem, offs[2 + k]);
        }
    }

    // Instance slot stream: 0, 1, 2, ... Written once by the CPU before any GPU use.
    g.slot_stream_mem = make_block(max_instances * 4u, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (!g.slot_stream_mem) goto oom;
    uint32_t* slots = (uint32_t*)dkMemBlockGetCpuAddr(g.slot_stream_mem);
    for (uint32_t i = 0; i < max_instances; i++) slots[i] = i;

    // Per-frame data. GPU-uncached: the CPU rewrites a slot every FRAME_SLOTS frames, and uncached GPU reads need no
    // cache invalidation between frames. (Measuring a GPU-cached variant with an L2 invalidate is a follow-up.)
    // Each storage region is at least one record long, so it can always be bound.
    g.bone_offset = align_up(UNIFORM_REGION_BYTES + max_instances * INSTANCE_BYTES, 256);
    g.morph_offset = align_up(g.bone_offset + (max_bones ? max_bones : 1) * BONE_BYTES, 256);
    g.frame_slot_bytes = align_up(g.morph_offset + (max_morph ? max_morph : 1) * MORPH_BYTES, DK_UNIFORM_BUF_ALIGNMENT);
    g.frame_mem = make_block(FRAME_SLOTS * g.frame_slot_bytes, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuUncached);
    if (!g.frame_mem) goto oom;

    g.cmd_slot_bytes = align_up(CMD_BASE_BYTES + max_draws * CMD_BYTES_PER_DRAW + g.max_batches * CMD_BYTES_PER_BATCH +
                                    SLYNX_RENDER_MAX_VIEWS * CMD_BYTES_PER_VIEW,
                                DK_CMDMEM_ALIGNMENT);
    g.cmd_mem = make_block(FRAME_SLOTS * (g.cmd_slot_bytes + CMD_RESERVE_BYTES),
                           DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (!g.cmd_mem) goto oom;
    // Optional: without it, frames report no GPU time.
    g.query_mem = make_block(FRAME_SLOTS * QUERY_SLOT_BYTES, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (g.query_mem) memset(dkMemBlockGetCpuAddr(g.query_mem), 0, FRAME_SLOTS * QUERY_SLOT_BYTES);
    DkCmdBufMaker cbm;
    dkCmdBufMakerDefaults(&cbm, g.device);
    cbm.cbAddMem = cmd_add_mem;
    g.cmdbuf = dkCmdBufCreate(&cbm);

    g.ready = true;
    // The platform tears the display down when it is dropped; it calls this hook first, so the renderer never
    // outlives the device (docs/platform.md, "Shutdown order: display exit hooks").
    if (slynx_display_add_exit_hook(slynx_render_exit) != SLYNX_OK) {
        slynx_render_exit();
        return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    }
    return SLYNX_RENDER_OK;

oom:
    destroy_block(&g.query_mem);
    destroy_block(&g.cmd_mem);
    destroy_block(&g.frame_mem);
    destroy_block(&g.slot_stream_mem);
    destroy_block(&g.panel_mem);
    destroy_block(&g.silhouette_mem);
    destroy_block(&g.depth_mem);
    destroy_block(&g.code_mem);
    return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
}

void slynx_render_size(uint32_t* width, uint32_t* height) {
    const SlynxDisplay* d = slynx_display_get();
    if (width) *width = d ? d->width : 0;
    if (height) *height = d ? d->height : 0;
}

static void video_free(void) {
    destroy_block(&g.video_mem);
    destroy_block(&g.video_stage_mem);
    g.video_w = g.video_h = 0;
}

static void video_plane_size(uint32_t k, uint32_t* w, uint32_t* h) {
    *w = k ? (g.video_w + 1u) / 2u : g.video_w;
    *h = k ? (g.video_h + 1u) / 2u : g.video_h;
}

// Makes the plane images and staging memory for a width x height picture. Called outside command recording.
static int video_resize(uint32_t width, uint32_t height) {
    video_free();
    g.video_w = width;
    g.video_h = height;
    DkImageLayout layouts[3];
    uint32_t offsets[3], bytes = 0, stage = 0;
    for (uint32_t k = 0; k < 3; k++) {
        uint32_t w, h;
        video_plane_size(k, &w, &h);
        DkImageLayoutMaker lm;
        dkImageLayoutMakerDefaults(&lm, g.device);
        lm.format = DkImageFormat_R8_Unorm;
        lm.dimensions[0] = w;
        lm.dimensions[1] = h;
        dkImageLayoutInitialize(&layouts[k], &lm);
        bytes = align_up(bytes, (uint32_t)dkImageLayoutGetAlignment(&layouts[k]));
        offsets[k] = bytes;
        bytes += (uint32_t)dkImageLayoutGetSize(&layouts[k]);
        stage += align_up(w * h, 256);
    }
    g.video_stage_bytes = stage;
    g.video_mem = make_block(bytes, DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
    g.video_stage_mem = make_block(FRAME_SLOTS * stage, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (!g.video_mem || !g.video_stage_mem) {
        video_free();
        return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    }
    for (uint32_t k = 0; k < 3; k++) dkImageInitialize(&g.video_img[k], &layouts[k], g.video_mem, offsets[k]);
    g.video_desc_dirty = true;
    return SLYNX_RENDER_OK;
}

static void wait_all_slots(void) {
    for (int i = 0; i < FRAME_SLOTS; i++) {
        if (g.fence_pending[i]) {
            dkFenceWait(&g.fence[i], -1);
            g.fence_pending[i] = false;
        }
    }
    dkQueueWaitIdle(g.queue);
    for (uint32_t i = 0; i < FRAME_SLOTS; i++) free_cmd_extra(i);
}

int slynx_render_upload_level(const void* vertices, uint32_t vertex_bytes, const void* extras, const void* weights,
                              uint32_t weight_count, const void* outline_points, uint32_t outline_count,
                              const uint16_t* indices, uint32_t index_count,
                              const SlynxRenderPool* pools, uint32_t pool_count) {
    if (!g.ready) return SLYNX_RENDER_ERR_NOT_INIT;
    if ((vertex_bytes && (!vertices || !extras)) || vertex_bytes % VERTEX_STRIDE || (index_count && !indices) ||
        (weight_count && !weights) || weight_count > (1u << 28) || (outline_count && !outline_points) ||
        outline_count > (1u << 26) || (pool_count && !pools) || pool_count > MAX_POOLS)
        return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    wait_all_slots();
    free_level();

    // Arena: vertices, the second vertex stream, then indices. CPU writes before any GPU use; the barrier below
    // invalidates GPU caches in case a previous level's data at the same addresses is still cached.
    g.vertex_bytes = vertex_bytes;
    g.vertex_count = vertex_bytes / VERTEX_STRIDE;
    g.extra_offset = align_up(vertex_bytes, 256);
    // Per-instance vertex weights: at least one record, so storage block 3 can always be bound.
    g.weights_offset = align_up(g.extra_offset + g.vertex_count * EXTRA_STRIDE, 256);
    g.weights_bytes = (weight_count ? weight_count : 1) * WEIGHT_BYTES;
    // Outline edge points, likewise at least one.
    g.outline_offset = align_up(g.weights_offset + g.weights_bytes, 256);
    g.outline_bytes = (outline_count ? outline_count : 1) * OUTLINE_POINT_BYTES;
    g.index_offset = align_up(g.outline_offset + g.outline_bytes, 256);
    g.index_bytes = index_count * 2u;
    g.index_count = index_count;
    g.arena_mem = make_block(g.index_offset + g.index_bytes, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (!g.arena_mem) return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    uint8_t* arena = (uint8_t*)dkMemBlockGetCpuAddr(g.arena_mem);
    if (vertex_bytes) {
        memcpy(arena, vertices, vertex_bytes);
        memcpy(arena + g.extra_offset, extras, g.vertex_count * EXTRA_STRIDE);
    }
    if (weight_count) memcpy(arena + g.weights_offset, weights, weight_count * WEIGHT_BYTES);
    if (outline_count) memcpy(arena + g.outline_offset, outline_points, outline_count * OUTLINE_POINT_BYTES);
    if (g.index_bytes) memcpy(arena + g.index_offset, indices, g.index_bytes);

    // Texture pools: lay out the images, then stage every level and copy it in on the GPU.
    g.pool_images = (DkImage*)calloc(pool_count ? pool_count : 1, sizeof(DkImage));
    if (!g.pool_images) return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    DkImageLayout* layouts = (DkImageLayout*)calloc(pool_count ? pool_count : 1, sizeof(DkImageLayout));
    uint32_t* offsets = (uint32_t*)calloc(pool_count ? pool_count : 1, sizeof(uint32_t));
    if (!layouts || !offsets) {
        free(layouts);
        free(offsets);
        return SLYNX_RENDER_ERR_OUT_OF_MEMORY;
    }
    uint32_t image_bytes = 0, staging_bytes = 0, copies = 0;
    for (uint32_t p = 0; p < pool_count; p++) {
        const SlynxRenderPool* pool = &pools[p];
        if (pool->format != 0 || !pool->width || !pool->height || !pool->mip_levels || !pool->layer_count ||
            !pool->layers) {
            free(layouts);
            free(offsets);
            return SLYNX_RENDER_ERR_BAD_ARGUMENT;
        }
        DkImageLayoutMaker lm;
        dkImageLayoutMakerDefaults(&lm, g.device);
        lm.type = DkImageType_2DArray;
        lm.format = DkImageFormat_RGBA8_Unorm;
        lm.dimensions[0] = pool->width;
        lm.dimensions[1] = pool->height;
        lm.dimensions[2] = pool->layer_count;
        lm.mipLevels = pool->mip_levels;
        dkImageLayoutInitialize(&layouts[p], &lm);
        image_bytes = align_up(image_bytes, dkImageLayoutGetAlignment(&layouts[p]));
        offsets[p] = image_bytes;
        image_bytes += (uint32_t)dkImageLayoutGetSize(&layouts[p]);
        for (uint32_t m = 0; m < pool->mip_levels; m++) {
            uint32_t w = pool->width >> m ? pool->width >> m : 1, h = pool->height >> m ? pool->height >> m : 1;
            staging_bytes += align_up(w * h * 4u, 256) * pool->layer_count;
        }
        copies += pool->mip_levels * pool->layer_count;
    }

    int rc = SLYNX_RENDER_OK;
    DkMemBlock staging = NULL, upload_cmd_mem = NULL;
    DkCmdBuf upload = NULL;
    if (pool_count) {
        g.image_mem = make_block(image_bytes, DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
        staging = make_block(staging_bytes, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    }
    // Image descriptors: the pools, then the silhouette image, the three video planes and the two panel images.
    g.sampler_desc_offset =
        align_up((pool_count + 6u) * (uint32_t)sizeof(DkImageDescriptor), DK_SAMPLER_DESCRIPTOR_ALIGNMENT);
    g.video_desc_dirty = true;
    g.desc_mem = make_block(g.sampler_desc_offset + 2u * (uint32_t)sizeof(DkSamplerDescriptor),
                            DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    uint32_t upload_cmd_bytes = align_up(64u * 1024u + copies * 128u + pool_count * 128u, DK_CMDMEM_ALIGNMENT);
    upload_cmd_mem = make_block(upload_cmd_bytes, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if ((pool_count && (!g.image_mem || !staging)) || !g.desc_mem || !upload_cmd_mem) {
        rc = SLYNX_RENDER_ERR_OUT_OF_MEMORY;
        goto done;
    }
    DkCmdBufMaker cbm;
    dkCmdBufMakerDefaults(&cbm, g.device);
    upload = dkCmdBufCreate(&cbm);
    dkCmdBufAddMemory(upload, upload_cmd_mem, 0, upload_cmd_bytes);

    uint8_t* stage_cpu = staging ? (uint8_t*)dkMemBlockGetCpuAddr(staging) : NULL;
    DkGpuAddr stage_gpu = staging ? dkMemBlockGetGpuAddr(staging) : 0;
    uint32_t stage_off = 0;
    for (uint32_t p = 0; p < pool_count; p++) {
        const SlynxRenderPool* pool = &pools[p];
        dkImageInitialize(&g.pool_images[p], &layouts[p], g.image_mem, offsets[p]);
        for (uint32_t layer = 0; layer < pool->layer_count; layer++) {
            const uint8_t* src = pool->layers[layer];
            uint32_t src_off = 0;
            for (uint32_t m = 0; m < pool->mip_levels; m++) {
                uint32_t w = pool->width >> m ? pool->width >> m : 1, h = pool->height >> m ? pool->height >> m : 1;
                uint32_t bytes = w * h * 4u;
                if (src_off + bytes > pool->layer_bytes) {
                    rc = SLYNX_RENDER_ERR_BAD_ARGUMENT;
                    goto done;
                }
                memcpy(stage_cpu + stage_off, src + src_off, bytes);
                DkImageView view;
                dkImageViewDefaults(&view, &g.pool_images[p]);
                view.mipLevelOffset = (uint8_t)m;
                view.mipLevelCount = 1;
                DkImageRect rect = {0, 0, layer, w, h, 1};  // z selects the array layer
                DkCopyBuf cb = {stage_gpu + stage_off, 0, 0};
                dkCmdBufCopyBufferToImage(upload, &cb, &view, &rect, 0);
                src_off += bytes;
                stage_off += align_up(bytes, 256);
            }
        }
        DkImageView view;
        dkImageViewDefaults(&view, &g.pool_images[p]);
        DkImageDescriptor desc;
        dkImageDescriptorInitialize(&desc, &view, false, false);
        dkCmdBufPushData(upload, dkMemBlockGetGpuAddr(g.desc_mem) + p * sizeof(DkImageDescriptor), &desc, sizeof(desc));
    }
    {
        DkImageView view;
        dkImageViewDefaults(&view, &g.silhouette);
        DkImageDescriptor desc;
        dkImageDescriptorInitialize(&desc, &view, false, false);
        dkCmdBufPushData(upload, dkMemBlockGetGpuAddr(g.desc_mem) + pool_count * sizeof(DkImageDescriptor), &desc,
                         sizeof(desc));
        for (uint32_t k = 0; k < 2; k++) {
            dkImageViewDefaults(&view, &g.panel_color[k]);
            dkImageDescriptorInitialize(&desc, &view, false, false);
            dkCmdBufPushData(upload, dkMemBlockGetGpuAddr(g.desc_mem) + (pool_count + 4u + k) * sizeof(DkImageDescriptor),
                             &desc, sizeof(desc));
        }
    }
    // Sampler 0 repeats, sampler 1 clamps to the edge; both linear with linear mip filtering.
    for (uint32_t k = 0; k < 2; k++) {
        DkSampler sampler;
        dkSamplerDefaults(&sampler);
        sampler.minFilter = DkFilter_Linear;
        sampler.magFilter = DkFilter_Linear;
        sampler.mipFilter = DkMipFilter_Linear;
        if (k == 1) {
            sampler.wrapMode[0] = DkWrapMode_ClampToEdge;
            sampler.wrapMode[1] = DkWrapMode_ClampToEdge;
            sampler.wrapMode[2] = DkWrapMode_ClampToEdge;
        }
        DkSamplerDescriptor sdesc;
        dkSamplerDescriptorInitialize(&sdesc, &sampler);
        dkCmdBufPushData(upload, dkMemBlockGetGpuAddr(g.desc_mem) + g.sampler_desc_offset + k * sizeof(sdesc), &sdesc,
                         sizeof(sdesc));
    }
    dkCmdBufBarrier(upload, DkBarrier_Full,
                    DkInvalidateFlags_Image | DkInvalidateFlags_Shader | DkInvalidateFlags_Descriptors |
                        DkInvalidateFlags_L2Cache);
    dkQueueSubmitCommands(g.queue, dkCmdBufFinishList(upload));
    dkQueueWaitIdle(g.queue);
    g.pool_count = pool_count;
    g.pool_sampler = (uint8_t*)calloc(pool_count ? pool_count : 1, 1);
    if (!g.pool_sampler) {
        rc = SLYNX_RENDER_ERR_OUT_OF_MEMORY;
        goto done;
    }
    for (uint32_t p = 0; p < pool_count; p++) g.pool_sampler[p] = pools[p].clamp ? 1 : 0;
    g.have_level = true;

done:
    if (upload) dkCmdBufDestroy(upload);
    destroy_block(&upload_cmd_mem);
    destroy_block(&staging);
    free(layouts);
    free(offsets);
    if (rc != SLYNX_RENDER_OK) free_level();
    return rc;
}

// Shaders and fixed-function state for one batch (SLYNX_RENDER_* state bits; cull: the frame's mode).
static void bind_state(DkCmdBuf cb, uint32_t state, uint32_t cull) {
    const DkShader* shaders[] = {&g.vsh, (state & SLYNX_RENDER_ALPHA_TEST) ? &g.alpha_fsh : &g.fsh};
    dkCmdBufBindShaders(cb, DkStageFlag_GraphicsMask, shaders, 2);

    DkRasterizerState rs;
    dkRasterizerStateDefaults(&rs);
    if ((state & SLYNX_RENDER_NO_CULL) || (cull == 0 && !(state & SLYNX_RENDER_CULL_FRONT)))
        rs.cullMode = DkFace_None;
    else
        rs.cullMode = ((state & SLYNX_RENDER_CULL_FRONT) || cull == 2) ? DkFace_Front : DkFace_Back;
    // Mirrored instances reverse the winding.
    bool cw = (state & SLYNX_RENDER_FRONT_CW) != 0;
    rs.frontFace = cw ? (FRONT_FACE == DkFrontFace_CCW ? DkFrontFace_CW : DkFrontFace_CCW) : FRONT_FACE;
    dkCmdBufBindRasterizerState(cb, &rs);

    DkDepthStencilState ds;
    dkDepthStencilStateDefaults(&ds);
    uint32_t depth = state & SLYNX_RENDER_DEPTH_MASK;
    ds.depthTestEnable = depth != 2;
    ds.depthCompareOp = depth == 1 ? DkCompareOp_Lequal : DkCompareOp_Less;
    ds.depthWriteEnable = (state & SLYNX_RENDER_DEPTH_WRITE) != 0;
    uint32_t stencil = (state & SLYNX_RENDER_STENCIL_MASK) >> 9;
    if (stencil == 1) {
        // Count surfaces inside the volume: +1 where a back face is behind the scene, -1 where a front face is.
        ds.stencilTestEnable = true;
        ds.stencilFrontCompareOp = DkCompareOp_Always;
        ds.stencilBackCompareOp = DkCompareOp_Always;
        ds.stencilFrontFailOp = ds.stencilBackFailOp = DkStencilOp_Keep;
        ds.stencilFrontPassOp = ds.stencilBackPassOp = DkStencilOp_Keep;
        ds.stencilFrontDepthFailOp = DkStencilOp_DecrWrap;
        ds.stencilBackDepthFailOp = DkStencilOp_IncrWrap;
    } else if (stencil == 2) {
        // Paint where the count is not zero, once: the painted pixels are reset.
        ds.stencilTestEnable = true;
        ds.stencilFrontCompareOp = DkCompareOp_NotEqual;
        ds.stencilBackCompareOp = DkCompareOp_NotEqual;
        ds.stencilFrontFailOp = ds.stencilBackFailOp = DkStencilOp_Keep;
        ds.stencilFrontDepthFailOp = ds.stencilBackDepthFailOp = DkStencilOp_Keep;
        ds.stencilFrontPassOp = ds.stencilBackPassOp = DkStencilOp_Zero;
    }
    dkCmdBufBindDepthStencilState(cb, &ds);
    if (stencil) dkCmdBufSetStencil(cb, DkFace_FrontAndBack, 0xff, 0, 0xff);

    uint32_t blend = (state & SLYNX_RENDER_BLEND_MASK) >> 3;
    DkColorState cs;
    dkColorStateDefaults(&cs);
    dkColorStateSetBlendEnable(&cs, 0, blend != 0);
    dkCmdBufBindColorState(cb, &cs);
    if (blend) {
        DkBlendState bs;
        dkBlendStateDefaults(&bs);  // src alpha, one minus src alpha
        // The destination alpha takes the source alpha (rendering.md 3.2); multiply keeps it.
        bs.srcAlphaBlendFactor = blend == 7 ? DkBlendFactor_Zero : DkBlendFactor_One;
        bs.dstAlphaBlendFactor = blend == 7 ? DkBlendFactor_One : DkBlendFactor_Zero;
        switch (blend) {
            case 2: bs.dstColorBlendFactor = DkBlendFactor_One; break;
            case 3:
                bs.srcColorBlendFactor = DkBlendFactor_InvSrcAlpha;
                bs.dstColorBlendFactor = DkBlendFactor_SrcAlpha;
                break;
            case 4:
                bs.srcColorBlendFactor = DkBlendFactor_One;
                bs.dstColorBlendFactor = DkBlendFactor_SrcAlpha;
                break;
            case 5:
                bs.srcColorBlendFactor = DkBlendFactor_DstAlpha;
                bs.dstColorBlendFactor = DkBlendFactor_InvDstAlpha;
                break;
            case 6:
                bs.srcColorBlendFactor = DkBlendFactor_InvDstAlpha;
                bs.dstColorBlendFactor = DkBlendFactor_DstAlpha;
                break;
            case 7:
                bs.srcColorBlendFactor = DkBlendFactor_Zero;
                bs.dstColorBlendFactor = DkBlendFactor_SrcColor;
                break;
            default: break;
        }
        dkCmdBufBindBlendState(cb, 0, &bs);
    }
    DkColorWriteState cws;
    dkColorWriteStateDefaults(&cws);
    if (state & SLYNX_RENDER_NO_COLOR) dkColorWriteStateSetMask(&cws, 0, 0);
    else if (state & SLYNX_RENDER_ALPHA_ONLY) dkColorWriteStateSetMask(&cws, 0, DkColorMask_A);
    dkCmdBufBindColorWriteState(cb, &cws);
}

int slynx_render_frame(const SlynxRenderFrame* f, SlynxRenderStats* stats) {
    if (!g.ready) return SLYNX_RENDER_ERR_NOT_INIT;
    if (!f || !f->uniforms || (f->instance_count && !f->instances) || (f->draw_count && !f->draws) ||
        (f->batch_count && !f->batches))
        return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    if (f->draw_count && !g.have_level) return SLYNX_RENDER_ERR_NO_LEVEL;
    if ((f->bone_count && !f->bones) || (f->morph_count && !f->morph)) return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    if (f->instance_count > g.max_instances || f->draw_count > g.max_draws || f->batch_count > g.max_batches ||
        f->bone_count > g.max_bones || f->morph_count > g.max_morph)
        return SLYNX_RENDER_ERR_TOO_LARGE;
    for (uint32_t b = 0; b < f->batch_count; b++) {
        const SlynxRenderBatch* bt = &f->batches[b];
        if (bt->pool >= g.pool_count || bt->pass > 9 || bt->first_draw > f->draw_count ||
            bt->draw_count > f->draw_count - bt->first_draw)
            return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    }
    if (f->view_count > SLYNX_RENDER_MAX_VIEWS || (f->view_count && !f->views)) return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    for (uint32_t v = 0; v < f->view_count; v++) {
        const SlynxRenderView* vw = &f->views[v];
        if (vw->first_batch > f->batch_count || vw->batch_count > f->batch_count - vw->first_batch || vw->target > 3)
            return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    }
    // Every draw must stay inside the arena and the frame's instance records. (Indices are relative to the base
    // vertex and are not read here; the renderer checked them when the mesh was added.)
    for (uint32_t i = 0; i < f->draw_count; i++) {
        const SlynxRenderDraw* dr = &f->draws[i];
        if (dr->first_index > g.index_count || dr->index_count > g.index_count - dr->first_index ||
            dr->base_vertex < 0 || (uint32_t)dr->base_vertex >= g.vertex_count ||
            dr->base_instance > f->instance_count || dr->instance_count > f->instance_count - dr->base_instance)
            return SLYNX_RENDER_ERR_BAD_ARGUMENT;
    }
    const SlynxDisplay* d = slynx_display_get();
    if (!d) return SLYNX_RENDER_ERR_NO_DISPLAY;

    uint32_t slot = (uint32_t)(g.frame % FRAME_SLOTS);
    uint64_t wait_ns = 0;
    if (g.fence_pending[slot]) {
        uint64_t t0 = armTicksToNs(armGetSystemTick());
        dkFenceWait(&g.fence[slot], -1);
        wait_ns = armTicksToNs(armGetSystemTick()) - t0;
        g.fence_pending[slot] = false;
        // The slot's previous frame is done: its GPU time is end minus start, in GPU ticks (614.4 MHz).
        if (g.query_mem) {
            const uint64_t* q = (const uint64_t*)((const uint8_t*)dkMemBlockGetCpuAddr(g.query_mem) + slot * QUERY_SLOT_BYTES);
            uint64_t start = q[1], end = q[3];
            g.last_gpu_ns = end > start ? (end - start) * 625u / 384u : 0;
        }
    }
    // The GPU is done with this slot's commands, extra blocks included.
    free_cmd_extra(slot);

    // A new movie frame: (re)make the planes when the size changed (waiting for the GPU), then stage the planes.
    bool video_upload = false;
    if (f->video_width) {
        if (!f->video_planes[0] || !f->video_planes[1] || !f->video_planes[2] || !f->video_height ||
            f->video_width > SLYNX_RENDER_MAX_VIDEO_SIZE || f->video_height > SLYNX_RENDER_MAX_VIDEO_SIZE)
            return SLYNX_RENDER_ERR_BAD_ARGUMENT;
        if (f->video_width != g.video_w || f->video_height != g.video_h) {
            wait_all_slots();
            int rc = video_resize(f->video_width, f->video_height);
            if (rc != SLYNX_RENDER_OK) return rc;
        }
        uint8_t* stage = (uint8_t*)dkMemBlockGetCpuAddr(g.video_stage_mem) + slot * g.video_stage_bytes;
        uint32_t off = 0;
        for (uint32_t k = 0; k < 3; k++) {
            uint32_t w, h;
            video_plane_size(k, &w, &h);
            memcpy(stage + off, f->video_planes[k], w * h);
            off += align_up(w * h, 256);
        }
        video_upload = true;
    }

    // Copy the frame's data into the slot.
    uint32_t base = slot * g.frame_slot_bytes;
    uint8_t* cpu = (uint8_t*)dkMemBlockGetCpuAddr(g.frame_mem) + base;
    DkGpuAddr gpu = dkMemBlockGetGpuAddr(g.frame_mem) + base;
    memcpy(cpu, f->uniforms, UNIFORM_COPY_BYTES);
    for (uint32_t v = 0; v < f->view_count; v++)
        memcpy(cpu + UNIFORM_BYTES * (1u + v), f->views[v].uniforms, UNIFORM_COPY_BYTES);
    if (f->instance_count) memcpy(cpu + UNIFORM_REGION_BYTES, f->instances, f->instance_count * INSTANCE_BYTES);
    if (f->bone_count) memcpy(cpu + g.bone_offset, f->bones, f->bone_count * BONE_BYTES);
    if (f->morph_count) memcpy(cpu + g.morph_offset, f->morph, f->morph_count * MORPH_BYTES);

    DkCmdBuf cb = g.cmdbuf;
    dkCmdBufClear(cb);
    g.cmd_slot = slot;
    g.cmd_cursor = 0;
    g.cmd_extra_bytes = 0;
    g.cmd_exhausted = false;
    cmd_add_mem(NULL, cb, CMD_CHUNK_BYTES);
    DkGpuAddr query = g.query_mem ? dkMemBlockGetGpuAddr(g.query_mem) + slot * QUERY_SLOT_BYTES : 0;
    if (query) dkCmdBufReportCounter(cb, DkCounter_TimestampPipelineTop, query);

    bool video_sync = false;
    if (g.video_w && g.video_desc_dirty && g.desc_mem) {
        for (uint32_t k = 0; k < 3; k++) {
            DkImageView view;
            dkImageViewDefaults(&view, &g.video_img[k]);
            DkImageDescriptor desc;
            dkImageDescriptorInitialize(&desc, &view, false, false);
            dkCmdBufPushData(cb, dkMemBlockGetGpuAddr(g.desc_mem) + (g.pool_count + 1u + k) * sizeof(DkImageDescriptor),
                             &desc, sizeof(desc));
        }
        g.video_desc_dirty = false;
        video_sync = true;
    }
    if (video_upload) {
        DkGpuAddr stage = dkMemBlockGetGpuAddr(g.video_stage_mem) + slot * g.video_stage_bytes;
        uint32_t off = 0;
        for (uint32_t k = 0; k < 3; k++) {
            uint32_t w, h;
            video_plane_size(k, &w, &h);
            DkImageView view;
            dkImageViewDefaults(&view, &g.video_img[k]);
            DkImageRect rect = {0, 0, 0, w, h, 1};
            DkCopyBuf src = {stage + off, 0, 0};
            dkCmdBufCopyBufferToImage(cb, &src, &view, &rect, 0);
            off += align_up(w * h, 256);
        }
    }
    if (video_upload || video_sync)
        dkCmdBufBarrier(cb, DkBarrier_Full, DkInvalidateFlags_Image | DkInvalidateFlags_Descriptors);

    int image = slynx_display_acquire();
    DkImageView color, depth;
    dkImageViewDefaults(&color, &d->images[image]);
    dkImageViewDefaults(&depth, &g.depth);
    dkCmdBufBindRenderTarget(cb, &color, &depth);
    DkViewport vp = {0.0f, 0.0f, (float)d->width, (float)d->height, 0.0f, 1.0f};
    DkScissor sc = {0, 0, d->width, d->height};
    dkCmdBufSetViewports(cb, 0, &vp, 1);
    dkCmdBufSetScissors(cb, 0, &sc, 1);
    dkCmdBufClearColorFloat(cb, 0, DkColorMask_RGBA, f->clear[0], f->clear[1], f->clear[2], f->clear[3]);
    dkCmdBufClearDepthStencil(cb, true, 1.0f, 0xff, 0);

    uint32_t calls = 0;
    if (f->draw_count) {
        // Everything below is bound once per frame.
        DkVtxAttribState attribs[8];
        memset(attribs, 0, sizeof(attribs));
        attribs[0].bufferId = 0; attribs[0].offset = 0;  attribs[0].size = DkVtxAttribSize_3x32; attribs[0].type = DkVtxAttribType_Float;
        attribs[1].bufferId = 0; attribs[1].offset = 12; attribs[1].size = DkVtxAttribSize_2x32; attribs[1].type = DkVtxAttribType_Float;
        attribs[2].bufferId = 0; attribs[2].offset = 20; attribs[2].size = DkVtxAttribSize_4x8;  attribs[2].type = DkVtxAttribType_Snorm;
        attribs[3].bufferId = 0; attribs[3].offset = 24; attribs[3].size = DkVtxAttribSize_4x8;  attribs[3].type = DkVtxAttribType_Unorm;
        attribs[4].bufferId = 1; attribs[4].offset = 0;  attribs[4].size = DkVtxAttribSize_1x32; attribs[4].type = DkVtxAttribType_Uint;
        // The second stream: bones (u8 x4, integer), weights (unorm16 x4), index in the mesh (u16, integer).
        attribs[5].bufferId = 2; attribs[5].offset = 0;  attribs[5].size = DkVtxAttribSize_4x8;  attribs[5].type = DkVtxAttribType_Uint;
        attribs[6].bufferId = 2; attribs[6].offset = 4;  attribs[6].size = DkVtxAttribSize_4x16; attribs[6].type = DkVtxAttribType_Unorm;
        attribs[7].bufferId = 2; attribs[7].offset = 12; attribs[7].size = DkVtxAttribSize_1x16; attribs[7].type = DkVtxAttribType_Uint;
        DkVtxBufferState bufs[3] = {{VERTEX_STRIDE, 0}, {4, 1}, {EXTRA_STRIDE, 0}};
        dkCmdBufBindVtxAttribState(cb, attribs, 8);
        dkCmdBufBindVtxBufferState(cb, bufs, 3);
        DkGpuAddr arena = dkMemBlockGetGpuAddr(g.arena_mem);
        dkCmdBufBindVtxBuffer(cb, 0, arena, g.vertex_bytes);
        dkCmdBufBindVtxBuffer(cb, 1, dkMemBlockGetGpuAddr(g.slot_stream_mem), g.max_instances * 4u);
        dkCmdBufBindVtxBuffer(cb, 2, arena + g.extra_offset, g.vertex_count * EXTRA_STRIDE);
        dkCmdBufBindIdxBuffer(cb, DkIdxFormat_Uint16, arena + g.index_offset);
        dkCmdBufBindUniformBuffer(cb, DkStage_Vertex, 0, gpu, UNIFORM_BYTES);
        dkCmdBufBindStorageBuffer(cb, DkStage_Vertex, 0, gpu + UNIFORM_REGION_BYTES,
                                  f->instance_count ? f->instance_count * INSTANCE_BYTES : INSTANCE_BYTES);
        dkCmdBufBindStorageBuffer(cb, DkStage_Vertex, 1, gpu + g.bone_offset,
                                  f->bone_count ? f->bone_count * BONE_BYTES : BONE_BYTES);
        dkCmdBufBindStorageBuffer(cb, DkStage_Vertex, 2, gpu + g.morph_offset,
                                  f->morph_count ? f->morph_count * MORPH_BYTES : MORPH_BYTES);
        dkCmdBufBindStorageBuffer(cb, DkStage_Vertex, 3, arena + g.weights_offset, g.weights_bytes);
        dkCmdBufBindStorageBuffer(cb, DkStage_Vertex, 4, arena + g.outline_offset, g.outline_bytes);
        DkGpuAddr desc = dkMemBlockGetGpuAddr(g.desc_mem);
        dkCmdBufBindImageDescriptorSet(cb, desc, g.pool_count + 6u);
        for (uint32_t k = 0; k < 2; k++)
            dkCmdBufBindTexture(cb, DkStage_Fragment, 5 + k, dkMakeTextureHandle(g.pool_count + 4u + k, 1));
        dkCmdBufBindTexture(cb, DkStage_Fragment, 1, dkMakeTextureHandle(g.pool_count, 1));
        if (g.video_w)
            for (uint32_t k = 0; k < 3; k++)
                dkCmdBufBindTexture(cb, DkStage_Fragment, 2 + k, dkMakeTextureHandle(g.pool_count + 1u + k, 1));
        dkCmdBufBindSamplerDescriptorSet(cb, desc + g.sampler_desc_offset, 2);
        uint32_t cur_state = UINT32_MAX, cur_pool = UINT32_MAX;
        // Without views, one view: every batch over the whole target with the frame's uniforms.
        uint32_t views = f->view_count ? f->view_count : 1u;
        bool on_silhouette = false;  // drawing into an image (silhouette or panel)
        uint32_t current_target = 0;
        for (uint32_t v = 0; v < views && !g.cmd_exhausted; v++) {
        uint32_t first = 0, count = f->batch_count;
        if (f->view_count) {
            const SlynxRenderView* vw = &f->views[v];
            first = vw->first_batch;
            count = vw->batch_count;
            uint32_t x = vw->viewport[0], y = vw->viewport[1];
            uint32_t w = vw->viewport[2], h = vw->viewport[3];
            bool sil = vw->target == 1, panel = vw->target >= 2;
            bool offscreen = sil || panel;
            uint32_t tw = sil ? SLYNX_RENDER_SILHOUETTE_SIZE : panel ? SLYNX_RENDER_PANEL_IMAGE_SIZE : d->width;
            uint32_t th = sil ? SLYNX_RENDER_SILHOUETTE_SIZE : panel ? SLYNX_RENDER_PANEL_IMAGE_SIZE : d->height;
            if (on_silhouette && vw->target != current_target) {
                // Leaving an image: it is read from now on.
                dkCmdBufBarrier(cb, DkBarrier_Fragments, DkInvalidateFlags_Image);
            }
            if (sil) {
                DkImageView sv;
                dkImageViewDefaults(&sv, &g.silhouette);
                dkCmdBufBindRenderTarget(cb, &sv, NULL);
            } else if (panel) {
                DkImageView pc, pd;
                dkImageViewDefaults(&pc, &g.panel_color[vw->target - 2]);
                dkImageViewDefaults(&pd, &g.panel_depth[vw->target - 2]);
                dkCmdBufBindRenderTarget(cb, &pc, &pd);
            } else if (on_silhouette) {
                dkCmdBufBindRenderTarget(cb, &color, &depth);
            }
            on_silhouette = offscreen;
            current_target = vw->target;
            if (x >= tw || y >= th) continue;
            if (x + w > tw) w = tw - x;
            if (y + h > th) h = th - y;
            dkCmdBufBindUniformBuffer(cb, DkStage_Vertex, 0, gpu + UNIFORM_BYTES * (1u + v), UNIFORM_BYTES);
            DkViewport pvp = {(float)x, (float)y, (float)w, (float)h, 0.0f, 1.0f};
            DkScissor psc = {x, y, w, h};
            dkCmdBufSetViewports(cb, 0, &pvp, 1);
            dkCmdBufSetScissors(cb, 0, &psc, 1);
            // Clears are limited to the scissor rectangle.
            if (vw->clear & SLYNX_RENDER_VIEW_CLEAR_COLOR)
                dkCmdBufClearColorFloat(cb, 0, DkColorMask_RGBA, vw->clear_color[0], vw->clear_color[1],
                                        vw->clear_color[2], vw->clear_color[3]);
            if ((vw->clear & SLYNX_RENDER_VIEW_CLEAR_DEPTH) && !sil) dkCmdBufClearDepthStencil(cb, true, 1.0f, 0xff, 0);
        }
        for (uint32_t b = first; b < first + count; b++) {
            const SlynxRenderBatch* bt = &f->batches[b];
            if (!bt->draw_count) continue;
            if (bt->state != cur_state) {
                bind_state(cb, bt->state, f->cull);
                cur_state = bt->state;
            }
            if (bt->pool != cur_pool) {
                dkCmdBufBindTexture(cb, DkStage_Fragment, 0, dkMakeTextureHandle(bt->pool, g.pool_sampler[bt->pool]));
                cur_pool = bt->pool;
            }
            const SlynxRenderDraw* dr = &f->draws[bt->first_draw];
            for (uint32_t i = 0; i < bt->draw_count && !g.cmd_exhausted; i++, dr++) {
                dkCmdBufDrawIndexed(cb, DkPrimitive_Triangles, dr->index_count, dr->instance_count, dr->first_index,
                                    dr->base_vertex, dr->base_instance);
                calls++;
            }
            if (g.cmd_exhausted) break;
        }
        }
    }

    if (query) dkCmdBufReportCounter(cb, DkCounter_Timestamp, query + 16);
    dkCmdBufSignalFence(cb, &g.fence[slot], false);
    dkQueueSubmitCommands(g.queue, dkCmdBufFinishList(cb));
    slynx_display_present(image);
    g.fence_pending[slot] = true;
    g.frame++;

    if (stats) {
        stats->api_draw_calls = calls;
        stats->cmd_bytes_budget = g.cmd_slot_bytes;
        stats->cmd_bytes_used = (g.cmd_cursor < g.cmd_slot_bytes ? g.cmd_cursor : g.cmd_slot_bytes) +
                                g.cmd_extra_bytes + (g.cmd_exhausted ? CMD_RESERVE_BYTES : 0);
        stats->wait_ns = wait_ns;
        stats->gpu_ns = g.last_gpu_ns;
    }
    // The frame was presented either way; without memory for all of it, the draws that did not fit are missing.
    return g.cmd_exhausted ? SLYNX_RENDER_ERR_OUT_OF_MEMORY : SLYNX_RENDER_OK;
}

void slynx_render_exit(void) {
    slynx_display_remove_exit_hook(slynx_render_exit);
    if (!g.ready) return;
    wait_all_slots();
    free_level();
    dkCmdBufDestroy(g.cmdbuf);
    for (uint32_t i = 0; i < FRAME_SLOTS; i++) free_cmd_extra(i);
    destroy_block(&g.query_mem);
    destroy_block(&g.cmd_mem);
    destroy_block(&g.frame_mem);
    destroy_block(&g.slot_stream_mem);
    video_free();
    destroy_block(&g.panel_mem);
    destroy_block(&g.silhouette_mem);
    destroy_block(&g.depth_mem);
    destroy_block(&g.code_mem);
    memset(&g, 0, sizeof(g));
}
