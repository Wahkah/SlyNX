// SlyNX Switch platform shim: debug draw, one position + colour pipeline on top of the platform display.
//
// Each frame Rust hands over a clear colour and a triangle list. The shim copies the vertices into a per-frame slice
// of GPU-visible memory, records a short command list into a per-frame slice of command memory, submits it to the
// display's queue and presents. A fence per in-flight frame stops the CPU from overwriting memory the GPU is still
// reading. This is also a worked example of a renderer using the display handles.
#include "slynx_shim.h"

#include <string.h>

#include <switch.h>

#define FRAMES_IN_FLIGHT 2
#define CMD_BYTES_PER_FRAME (16u * 1024u)

// Shader binaries compiled by uam and embedded by slynx_shaders.S.
extern const uint8_t slynx_color_vsh[], slynx_color_vsh_end[];
extern const uint8_t slynx_color_fsh[], slynx_color_fsh_end[];

static struct {
    bool ready;
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

static DkMemBlock make_block(DkDevice device, uint32_t size, uint32_t flags) {
    DkMemBlockMaker m;
    dkMemBlockMakerDefaults(&m, device, align_up(size, DK_MEMBLOCK_ALIGNMENT));
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

static void destroy_all(void) {
    if (g.cmdbuf) dkCmdBufDestroy(g.cmdbuf);
    if (g.vtx_mem) dkMemBlockDestroy(g.vtx_mem);
    if (g.cmd_mem) dkMemBlockDestroy(g.cmd_mem);
    if (g.code_mem) dkMemBlockDestroy(g.code_mem);
    memset(&g, 0, sizeof(g));
}

int slynx_dd_init(uint32_t max_vertices) {
    if (g.ready) return SLYNX_ERR_ALREADY_INIT;
    const SlynxDisplay* d = slynx_display_get();
    if (!d) return SLYNX_ERR_NOT_INIT;
    if (max_vertices == 0 || max_vertices > (64u * 1024u * 1024u) / sizeof(SlynxVertex)) return SLYNX_ERR_BAD_ARGUMENT;
    memset(&g, 0, sizeof(g));

    // Shaders. Code memory needs DK_SHADER_CODE_UNUSABLE_SIZE spare bytes after the last shader.
    uint32_t vsh_size = (uint32_t)(slynx_color_vsh_end - slynx_color_vsh);
    uint32_t fsh_size = (uint32_t)(slynx_color_fsh_end - slynx_color_fsh);
    uint32_t code_size = align_up(vsh_size, DK_SHADER_CODE_ALIGNMENT) + align_up(fsh_size, DK_SHADER_CODE_ALIGNMENT) +
                         DK_SHADER_CODE_UNUSABLE_SIZE;
    g.code_mem = make_block(d->device, code_size,
                            DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached | DkMemBlockFlags_Code);
    if (!g.code_mem) goto oom;
    uint32_t code_off = 0;
    load_shader(&g.vsh, slynx_color_vsh, slynx_color_vsh_end, &code_off);
    load_shader(&g.fsh, slynx_color_fsh, slynx_color_fsh_end, &code_off);

    // Command memory: one slice per in-flight frame.
    g.cmd_mem = make_block(d->device, FRAMES_IN_FLIGHT * CMD_BYTES_PER_FRAME,
                           DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuCached);
    if (!g.cmd_mem) goto oom;
    DkCmdBufMaker cbm;
    dkCmdBufMakerDefaults(&cbm, d->device);
    g.cmdbuf = dkCmdBufCreate(&cbm);
    if (!g.cmdbuf) {
        destroy_all();
        return SLYNX_ERR_DEVICE;
    }

    // Vertex memory: one slice per in-flight frame, uncached on both sides so CPU writes are visible to the GPU
    // without cache maintenance. Fine for a few hundred KiB per frame.
    g.max_vertices = max_vertices;
    g.vtx_bytes_per_frame = align_up(max_vertices * (uint32_t)sizeof(SlynxVertex), 256);
    g.vtx_mem = make_block(d->device, FRAMES_IN_FLIGHT * g.vtx_bytes_per_frame,
                           DkMemBlockFlags_CpuUncached | DkMemBlockFlags_GpuUncached);
    if (!g.vtx_mem) goto oom;

    if (slynx_display_add_exit_hook(slynx_dd_exit) != SLYNX_OK) {
        destroy_all();
        return SLYNX_ERR_OUT_OF_MEMORY;
    }
    g.ready = true;
    return SLYNX_OK;

oom:
    destroy_all();
    return SLYNX_ERR_OUT_OF_MEMORY;
}

int slynx_dd_frame(const float clear_rgba[4], const SlynxVertex* vertices, uint32_t count) {
    const SlynxDisplay* d = slynx_display_get();
    if (!g.ready || !d) return SLYNX_ERR_NOT_INIT;
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

    int image = slynx_display_acquire();
    if (image < 0) return SLYNX_ERR_NOT_INIT;

    DkImageView view;
    dkImageViewDefaults(&view, &d->images[image]);
    dkCmdBufBindRenderTarget(g.cmdbuf, &view, NULL);

    DkViewport vp = {0.0f, 0.0f, (float)d->width, (float)d->height, 0.0f, 1.0f};
    DkScissor sc = {0, 0, d->width, d->height};
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
    dkQueueSubmitCommands(d->queue, list);
    slynx_display_present(image);

    g.fence_pending[slice] = true;
    g.frame++;
    return SLYNX_OK;
}

void slynx_dd_exit(void) {
    slynx_display_remove_exit_hook(slynx_dd_exit);
    if (!g.ready) return;
    slynx_display_wait_idle();
    destroy_all();
}
