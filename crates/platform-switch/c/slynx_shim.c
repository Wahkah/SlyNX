// SlyNX Switch platform shim: libnx applet/input and a minimal deko3d renderer.
//
// The renderer owns one device, one queue, a double-buffered swapchain and one pipeline (position + colour).
// Each frame Rust hands over a clear colour and a triangle list; the shim copies the vertices into a per-frame
// slice of GPU-visible memory, records a short command list into a per-frame slice of command memory, submits it
// and presents. A fence per in-flight frame stops the CPU from overwriting memory the GPU is still reading.
#include "slynx_shim.h"

#include <stdlib.h>
#include <string.h>

#include <switch.h>
#include <deko3d.h>

#define FB_COUNT 2
#define FRAMES_IN_FLIGHT 2
#define FB_WIDTH 1280
#define FB_HEIGHT 720
#define CMD_BYTES_PER_FRAME (16u * 1024u)

// Shader binaries compiled by uam and embedded by slynx_shaders.S.
extern const uint8_t slynx_color_vsh[], slynx_color_vsh_end[];
extern const uint8_t slynx_color_fsh[], slynx_color_fsh_end[];

static PadState s_pad;

static struct {
    bool ready;
    DkDevice device;
    DkQueue queue;
    DkMemBlock fb_mem;
    DkImage fb[FB_COUNT];
    DkSwapchain swapchain;
    DkMemBlock code_mem;
    DkShader vsh, fsh;
    DkMemBlock cmd_mem;
    DkCmdBuf cmdbuf;
    DkMemBlock vtx_mem;
    uint32_t max_vertices;
    uint32_t vtx_bytes_per_frame;
    DkFence fence[FRAMES_IN_FLIGHT];
    bool fence_pending[FRAMES_IN_FLIGHT];
    uint32_t frame;
} g;

static uint32_t align_up(uint32_t v, uint32_t a) { return (v + a - 1) & ~(a - 1); }

// ---------------------------------------------------------------------------------------------------------------
// Applet, input, time, debug

void slynx_input_init(void) {
    padConfigureInput(1, HidNpadStyleSet_NpadStandard);
    padInitializeDefault(&s_pad);
}

bool slynx_applet_main_loop(void) { return appletMainLoop(); }

void slynx_input_poll(uint64_t* down, uint64_t* held) {
    padUpdate(&s_pad);
    if (down) *down = padGetButtonsDown(&s_pad);
    if (held) *held = padGetButtons(&s_pad);
}

uint64_t slynx_ticks_ns(void) { return armTicksToNs(armGetSystemTick()); }

void slynx_debug_log(const char* msg, size_t len) {
    if (msg && len) svcOutputDebugString(msg, len);
}

_Noreturn void slynx_abort(void) { abort(); }

// ---------------------------------------------------------------------------------------------------------------
// Graphics

uint32_t slynx_gfx_width(void) { return FB_WIDTH; }
uint32_t slynx_gfx_height(void) { return FB_HEIGHT; }

static DkMemBlock make_block(uint32_t size, uint32_t flags) {
    DkMemBlockMaker m;
    dkMemBlockMakerDefaults(&m, g.device, align_up(size, DK_MEMBLOCK_ALIGNMENT));
    m.flags = flags;
    return dkMemBlockCreate(&m);
}

// Copies one embedded shader into code memory at *offset and initialises it.
static void load_shader(DkShader* shader, const uint8_t* begin, const uint8_t* end, uint32_t* offset) {
    uint32_t size = (uint32_t)(end - begin);
    memcpy((uint8_t*)dkMemBlockGetCpuAddr(g.code_mem) + *offset, begin, size);
    DkShaderMaker sm;
    dkShaderMakerDefaults(&sm, g.code_mem, *offset);
    dkShaderInitialize(shader, &sm);
    *offset = align_up(*offset + size, DK_SHADER_CODE_ALIGNMENT);
}

int slynx_gfx_init(uint32_t max_vertices) {
    if (g.ready) return SLYNX_ERR_ALREADY_INIT;
    if (max_vertices == 0 || max_vertices > (64u * 1024u * 1024u) / sizeof(SlynxVertex)) return SLYNX_ERR_BAD_ARGUMENT;
    memset(&g, 0, sizeof(g));

    DkDeviceMaker dm;
    dkDeviceMakerDefaults(&dm);
    dm.flags = DkDeviceFlags_DepthZeroToOne | DkDeviceFlags_OriginUpperLeft;
    g.device = dkDeviceCreate(&dm);

    // Framebuffers and swapchain.
    DkImageLayoutMaker lm;
    dkImageLayoutMakerDefaults(&lm, g.device);
    lm.flags = DkImageFlags_UsageRender | DkImageFlags_UsagePresent | DkImageFlags_HwCompression;
    lm.format = DkImageFormat_RGBA8_Unorm;
    lm.dimensions[0] = FB_WIDTH;
    lm.dimensions[1] = FB_HEIGHT;
    DkImageLayout layout;
    dkImageLayoutInitialize(&layout, &lm);
    uint32_t fb_size = align_up((uint32_t)dkImageLayoutGetSize(&layout), dkImageLayoutGetAlignment(&layout));
    g.fb_mem = make_block(FB_COUNT * fb_size, DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image);
    const DkImage* images[FB_COUNT];
    for (int i = 0; i < FB_COUNT; i++) {
        dkImageInitialize(&g.fb[i], &layout, g.fb_mem, i * fb_size);
        images[i] = &g.fb[i];
    }
    DkSwapchainMaker scm;
    dkSwapchainMakerDefaults(&scm, g.device, nwindowGetDefault(), images, FB_COUNT);
    g.swapchain = dkSwapchainCreate(&scm);

    // Shaders. Code memory needs DK_SHADER_CODE_UNUSABLE_SIZE spare bytes after the last shader.
    uint32_t vsh_size = (uint32_t)(slynx_color_vsh_end - slynx_color_vsh);
    uint32_t fsh_size = (uint32_t)(slynx_color_fsh_end - slynx_color_fsh);
    uint32_t code_size = align_up(vsh_size, DK_SHADER_CODE_ALIGNMENT) + align_up(fsh_size, DK_SHADER_CODE_ALIGNMENT) +
                         DK_SHADER_CODE_UNUSABLE_SIZE;
    g.code_mem = make_block(code_size, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached | DkMemBlockFlags_Code);
    uint32_t code_off = 0;
    load_shader(&g.vsh, slynx_color_vsh, slynx_color_vsh_end, &code_off);
    load_shader(&g.fsh, slynx_color_fsh, slynx_color_fsh_end, &code_off);

    // Command memory: one slice per in-flight frame.
    g.cmd_mem = make_block(FRAMES_IN_FLIGHT * CMD_BYTES_PER_FRAME, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    DkCmdBufMaker cbm;
    dkCmdBufMakerDefaults(&cbm, g.device);
    g.cmdbuf = dkCmdBufCreate(&cbm);

    // Vertex memory: one slice per in-flight frame. Uncached on both sides, so CPU writes are visible to the GPU
    // without cache maintenance. Fine for a few hundred KiB per frame; a real renderer will stage differently.
    g.max_vertices = max_vertices;
    g.vtx_bytes_per_frame = align_up(max_vertices * (uint32_t)sizeof(SlynxVertex), 256);
    g.vtx_mem = make_block(FRAMES_IN_FLIGHT * g.vtx_bytes_per_frame, DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuUncached);

    DkQueueMaker qm;
    dkQueueMakerDefaults(&qm, g.device);
    qm.flags = DkQueueFlags_Graphics;
    g.queue = dkQueueCreate(&qm);

    g.ready = true;
    return SLYNX_OK;
}

int slynx_gfx_frame(const float clear_rgba[4], const SlynxVertex* vertices, uint32_t count) {
    if (!g.ready) return SLYNX_ERR_NOT_INIT;
    if (!clear_rgba || (count && !vertices)) return SLYNX_ERR_BAD_ARGUMENT;
    if (count > g.max_vertices) count = g.max_vertices;
    count -= count % 3;  // whole triangles only

    uint32_t slice = g.frame % FRAMES_IN_FLIGHT;
    if (g.fence_pending[slice]) {
        dkFenceWait(&g.fence[slice], -1);
        g.fence_pending[slice] = false;
    }

    uint32_t vtx_off = slice * g.vtx_bytes_per_frame;
    if (count) memcpy((uint8_t*)dkMemBlockGetCpuAddr(g.vtx_mem) + vtx_off, vertices, count * sizeof(SlynxVertex));

    dkCmdBufClear(g.cmdbuf);
    dkCmdBufAddMemory(g.cmdbuf, g.cmd_mem, slice * CMD_BYTES_PER_FRAME, CMD_BYTES_PER_FRAME);

    int image = dkQueueAcquireImage(g.queue, g.swapchain);

    DkImageView view;
    dkImageViewDefaults(&view, &g.fb[image]);
    dkCmdBufBindRenderTarget(g.cmdbuf, &view, NULL);

    DkViewport vp = {0.0f, 0.0f, (float)FB_WIDTH, (float)FB_HEIGHT, 0.0f, 1.0f};
    DkScissor sc = {0, 0, FB_WIDTH, FB_HEIGHT};
    dkCmdBufSetViewports(g.cmdbuf, 0, &vp, 1);
    dkCmdBufSetScissors(g.cmdbuf, 0, &sc, 1);
    dkCmdBufClearColorFloat(g.cmdbuf, 0, DkColorMask_RGBA, clear_rgba[0], clear_rgba[1], clear_rgba[2], clear_rgba[3]);

    if (count) {
        const DkShader* shaders[] = {&g.vsh, &g.fsh};
        dkCmdBufBindShaders(g.cmdbuf, DkStageFlag_GraphicsMask, shaders, 2);

        DkRasterizerState rs;
        dkRasterizerStateDefaults(&rs);
        rs.cullMode = DkFace_None;
        DkColorState cs;
        dkColorStateDefaults(&cs);
        DkColorWriteState cws;
        dkColorWriteStateDefaults(&cws);
        dkCmdBufBindRasterizerState(g.cmdbuf, &rs);
        dkCmdBufBindColorState(g.cmdbuf, &cs);
        dkCmdBufBindColorWriteState(g.cmdbuf, &cws);

        DkVtxAttribState attribs[2];
        memset(attribs, 0, sizeof(attribs));
        attribs[0].bufferId = 0;
        attribs[0].offset = offsetof(SlynxVertex, x);
        attribs[0].size = DkVtxAttribSize_2x32;
        attribs[0].type = DkVtxAttribType_Float;
        attribs[1].bufferId = 0;
        attribs[1].offset = offsetof(SlynxVertex, r);
        attribs[1].size = DkVtxAttribSize_4x32;
        attribs[1].type = DkVtxAttribType_Float;
        DkVtxBufferState buf = {sizeof(SlynxVertex), 0};
        dkCmdBufBindVtxAttribState(g.cmdbuf, attribs, 2);
        dkCmdBufBindVtxBufferState(g.cmdbuf, &buf, 1);
        dkCmdBufBindVtxBuffer(g.cmdbuf, 0, dkMemBlockGetGpuAddr(g.vtx_mem) + vtx_off, count * sizeof(SlynxVertex));
        dkCmdBufDraw(g.cmdbuf, DkPrimitive_Triangles, count, 1, 0, 0);
    }

    dkCmdBufSignalFence(g.cmdbuf, &g.fence[slice], false);
    DkCmdList list = dkCmdBufFinishList(g.cmdbuf);
    dkQueueSubmitCommands(g.queue, list);
    dkQueuePresentImage(g.queue, g.swapchain, image);

    g.fence_pending[slice] = true;
    g.frame++;
    return SLYNX_OK;
}

void slynx_gfx_exit(void) {
    if (!g.ready) return;
    dkQueueWaitIdle(g.queue);
    dkQueueDestroy(g.queue);
    dkCmdBufDestroy(g.cmdbuf);
    dkMemBlockDestroy(g.vtx_mem);
    dkMemBlockDestroy(g.cmd_mem);
    dkMemBlockDestroy(g.code_mem);
    dkSwapchainDestroy(g.swapchain);
    dkMemBlockDestroy(g.fb_mem);
    dkDeviceDestroy(g.device);
    memset(&g, 0, sizeof(g));
}
