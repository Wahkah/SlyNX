// SlyNX Switch platform shim: the deko3d device, graphics queue and swapchain.
//
// The platform owns these objects and lends them to the renderer (and to the debug draw in slynx_debugdraw.c).
// Images are allocated at 1920x1080 once; handheld output crops to the top-left 1280x720, so nothing has to be
// recreated when the console is docked or undocked.
#include "slynx_shim.h"

#include <stdio.h>
#include <string.h>

#include <switch.h>

static SlynxDisplay g;
static bool g_ready;

// GPU users to shut down before the device goes, run newest first by slynx_display_exit.
static void (*g_exit_hooks[SLYNX_DISPLAY_MAX_EXIT_HOOKS])(void);
static uint32_t g_exit_hook_count;

// Transitional fallback until the renderer registers its exit function as a hook: a weak reference that resolves
// to the renderer's exit function when its C code is linked into the program and to NULL otherwise. Calling it
// after the hook ran is harmless (it is idempotent).
extern void slynx_render_exit(void) __attribute__((weak));

static uint32_t align_up(uint32_t v, uint32_t a) { return (v + a - 1) & ~(a - 1); }

static void on_deko_debug(void* user, const char* context, DkResult result, const char* message) {
    (void)user;
    char line[256];
    int n = snprintf(line, sizeof(line), "deko3d: %s: result %d: %s\n", context ? context : "?", (int)result,
                     message ? message : "");
    if (n > 0) slynx_debug_log(line, (size_t)(n < (int)sizeof(line) ? n : (int)sizeof(line) - 1));
}

// Destroys whatever exists, in reverse creation order. Safe on a partly built display.
static void destroy_all(void) {
    if (g.queue) {
        dkQueueWaitIdle(g.queue);
        dkQueueDestroy(g.queue);
    }
    if (g.swapchain) dkSwapchainDestroy(g.swapchain);
    if (g.image_mem) dkMemBlockDestroy(g.image_mem);
    if (g.device) dkDeviceDestroy(g.device);
    memset(&g, 0, sizeof(g));
    g_ready = false;
}

static void apply_mode(bool docked) {
    g.docked = docked;
    g.width = docked ? 1920 : 1280;
    g.height = docked ? 1080 : 720;
    // A zero crop shows the whole image.
    if (docked)
        dkSwapchainSetCrop(g.swapchain, 0, 0, 0, 0);
    else
        dkSwapchainSetCrop(g.swapchain, 0, 0, (int32_t)g.width, (int32_t)g.height);
}

int slynx_display_init(uint32_t device_flags, uint32_t image_count, bool docked) {
    if (g_ready) return SLYNX_ERR_ALREADY_INIT;
    if (image_count < 2 || image_count > SLYNX_DISPLAY_MAX_IMAGES) return SLYNX_ERR_BAD_ARGUMENT;
    memset(&g, 0, sizeof(g));

    DkDeviceMaker dm;
    dkDeviceMakerDefaults(&dm);
    dm.flags = device_flags;
    dm.cbDebug = on_deko_debug;
    g.device = dkDeviceCreate(&dm);
    if (!g.device) return SLYNX_ERR_DEVICE;

    DkImageLayoutMaker lm;
    dkImageLayoutMakerDefaults(&lm, g.device);
    lm.flags = DkImageFlags_UsageRender | DkImageFlags_UsagePresent | DkImageFlags_HwCompression;
    lm.format = DkImageFormat_RGBA8_Unorm;
    lm.dimensions[0] = SLYNX_FB_WIDTH;
    lm.dimensions[1] = SLYNX_FB_HEIGHT;
    DkImageLayout layout;
    dkImageLayoutInitialize(&layout, &lm);
    uint32_t image_size = align_up((uint32_t)dkImageLayoutGetSize(&layout), dkImageLayoutGetAlignment(&layout));

    DkMemBlockMaker mm;
    dkMemBlockMakerDefaults(&mm, g.device, align_up(image_count * image_size, DK_MEMBLOCK_ALIGNMENT));
    mm.flags = DkMemBlockFlags_GpuCached | DkMemBlockFlags_Image;
    g.image_mem = dkMemBlockCreate(&mm);
    if (!g.image_mem) {
        destroy_all();
        return SLYNX_ERR_OUT_OF_MEMORY;
    }

    const DkImage* images[SLYNX_DISPLAY_MAX_IMAGES];
    for (uint32_t i = 0; i < image_count; i++) {
        dkImageInitialize(&g.images[i], &layout, g.image_mem, i * image_size);
        images[i] = &g.images[i];
    }
    DkSwapchainMaker sm;
    dkSwapchainMakerDefaults(&sm, g.device, nwindowGetDefault(), images, image_count);
    g.swapchain = dkSwapchainCreate(&sm);
    if (!g.swapchain) {
        destroy_all();
        return SLYNX_ERR_DEVICE;
    }

    DkQueueMaker qm;
    dkQueueMakerDefaults(&qm, g.device);
    qm.flags = DkQueueFlags_Graphics;
    g.queue = dkQueueCreate(&qm);
    if (!g.queue) {
        destroy_all();
        return SLYNX_ERR_DEVICE;
    }

    g.image_count = image_count;
    g.fb_width = SLYNX_FB_WIDTH;
    g.fb_height = SLYNX_FB_HEIGHT;
    g.format = DkImageFormat_RGBA8_Unorm;
    apply_mode(docked);
    g_ready = true;
    return SLYNX_OK;
}

const SlynxDisplay* slynx_display_get(void) { return g_ready ? &g : NULL; }

void slynx_display_set_docked(bool docked) {
    if (g_ready && docked != g.docked) apply_mode(docked);
}

int slynx_display_acquire(void) { return g_ready ? dkQueueAcquireImage(g.queue, g.swapchain) : -1; }

void slynx_display_present(int index) {
    if (g_ready && index >= 0 && (uint32_t)index < g.image_count) dkQueuePresentImage(g.queue, g.swapchain, index);
}

void slynx_display_wait_idle(void) {
    if (g_ready) dkQueueWaitIdle(g.queue);
}

int slynx_display_add_exit_hook(void (*hook)(void)) {
    if (!hook) return SLYNX_ERR_BAD_ARGUMENT;
    for (uint32_t i = 0; i < g_exit_hook_count; i++)
        if (g_exit_hooks[i] == hook) return SLYNX_OK;
    if (g_exit_hook_count == SLYNX_DISPLAY_MAX_EXIT_HOOKS) return SLYNX_ERR_OUT_OF_MEMORY;
    g_exit_hooks[g_exit_hook_count++] = hook;
    return SLYNX_OK;
}

void slynx_display_remove_exit_hook(void (*hook)(void)) {
    for (uint32_t i = 0; i < g_exit_hook_count; i++) {
        if (g_exit_hooks[i] == hook) {
            for (uint32_t j = i + 1; j < g_exit_hook_count; j++) g_exit_hooks[j - 1] = g_exit_hooks[j];
            g_exit_hooks[--g_exit_hook_count] = NULL;
            return;
        }
    }
}

void slynx_display_exit(void) {
    // Each hook is popped before it runs, so a hook that removes itself (the usual exit function) is harmless.
    while (g_exit_hook_count) {
        void (*hook)(void) = g_exit_hooks[--g_exit_hook_count];
        g_exit_hooks[g_exit_hook_count] = NULL;
        hook();
    }
    if (slynx_render_exit) slynx_render_exit();
    if (g_ready) destroy_all();
}

bool slynx_display_handles(SlynxDisplayHandles* out) {
    if (!g_ready || !out) return false;
    out->device = g.device;
    out->queue = g.queue;
    out->swapchain = g.swapchain;
    out->images = &g.images[0];
    out->image_count = g.image_count;
    out->image_stride = (uint32_t)sizeof(DkImage);
    out->fb_width = g.fb_width;
    out->fb_height = g.fb_height;
    out->width = g.width;
    out->height = g.height;
    out->format = (uint32_t)g.format;
    out->docked = g.docked;
    return true;
}
