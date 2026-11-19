// SlyNX Switch platform shim: applet messages, input, time, threads, audio output and files.
#include "slynx_shim.h"

#include <errno.h>
#include <fcntl.h>
#include <malloc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include <switch.h>

// ---------------------------------------------------------------------------------------------------------------
// Platform, applet, input, time, debug

static PadState s_pads[SLYNX_MAX_PLAYERS];
static AppletHookCookie s_hook;
static bool s_resumed;
static bool s_romfs;

static void on_applet_hook(AppletHookType hook, void* param) {
    (void)param;
    if (hook == AppletHookType_OnResume) s_resumed = true;
}

void slynx_platform_init(void) {
    s_romfs = R_SUCCEEDED(romfsInit());

    padConfigureInput(SLYNX_MAX_PLAYERS, HidNpadStyleSet_NpadStandard);
    // Player 1 also reads the Joy-Cons attached to the console, so handheld play needs no setup.
    padInitializeWithMask(&s_pads[0], (1ULL << HidNpadIdType_No1) | (1ULL << HidNpadIdType_Handheld));
    for (int i = 1; i < SLYNX_MAX_PLAYERS; i++) padInitializeWithMask(&s_pads[i], 1ULL << (HidNpadIdType_No1 + i));

    appletHook(&s_hook, on_applet_hook, NULL);
}

void slynx_platform_exit(void) {
    appletUnhook(&s_hook);
    appletSetCpuBoostMode(ApmCpuBoostMode_Normal);
    if (s_romfs) romfsExit();
    s_romfs = false;
}

bool slynx_applet_pump(SlynxAppletState* out) {
    bool running = appletMainLoop();
    if (out) {
        out->focus = (uint32_t)appletGetFocusState();
        out->docked = appletGetOperationMode() == AppletOperationMode_Console;
        out->resumed = s_resumed;
    }
    s_resumed = false;
    return running;
}

void slynx_input_read(SlynxPadReading out[SLYNX_MAX_PLAYERS]) {
    for (int i = 0; i < SLYNX_MAX_PLAYERS; i++) {
        PadState* pad = &s_pads[i];
        padUpdate(pad);
        SlynxPadReading* r = &out[i];
        memset(r, 0, sizeof(*r));
        r->connected = padIsConnected(pad);
        if (!r->connected) continue;
        r->style = padGetStyleSet(pad);
        r->buttons = padGetButtons(pad);
        HidAnalogStickState l = padGetStickPos(pad, 0);
        HidAnalogStickState rs = padGetStickPos(pad, 1);
        r->lx = l.x;
        r->ly = l.y;
        r->rx = rs.x;
        r->ry = rs.y;
    }
}

uint64_t slynx_ticks_ns(void) { return armTicksToNs(armGetSystemTick()); }

int slynx_set_cpu_boost(bool on) {
    return (int)appletSetCpuBoostMode(on ? ApmCpuBoostMode_FastLoad : ApmCpuBoostMode_Normal);
}

void slynx_debug_log(const char* msg, size_t len) {
    if (msg && len) svcOutputDebugString(msg, len);
}

_Noreturn void slynx_abort(void) { abort(); }

// ---------------------------------------------------------------------------------------------------------------
// Threads

void* slynx_thread_start(void (*entry)(void*), void* arg, size_t stack_size, int prio, int core, uint32_t* rc_out) {
    uint32_t dummy;
    if (!rc_out) rc_out = &dummy;
    *rc_out = 0;
    Thread* t = (Thread*)malloc(sizeof(Thread));
    if (!t) {
        *rc_out = MAKERESULT(Module_Libnx, LibnxError_OutOfMemory);
        return NULL;
    }
    stack_size = (stack_size + 0xFFF) & ~(size_t)0xFFF;
    Result rc = threadCreate(t, entry, arg, NULL, stack_size, prio, core);
    if (R_FAILED(rc)) {
        free(t);
        *rc_out = rc;
        return NULL;
    }
    rc = threadStart(t);
    if (R_FAILED(rc)) {
        threadClose(t);
        free(t);
        *rc_out = rc;
        return NULL;
    }
    return t;
}

void slynx_thread_join(void* thread) {
    Thread* t = (Thread*)thread;
    if (!t) return;
    threadWaitForExit(t);
    threadClose(t);
    free(t);
}

bool slynx_thread_pin_current(int core) {
    if (core < 0 || core > 2) return false;
    return R_SUCCEEDED(svcSetThreadCoreMask(CUR_THREAD_HANDLE, core, 1u << core));
}

int slynx_current_core(void) { return (int)svcGetCurrentProcessorNumber(); }

// ---------------------------------------------------------------------------------------------------------------
// Audio

#define AUDIO_MAX_BUFFERS 8

static struct {
    bool open;
    uint32_t count;
    uint32_t data_bytes;
    AudioOutBuffer bufs[AUDIO_MAX_BUFFERS];
} s_audio;

static void audio_free_buffers(void) {
    for (uint32_t i = 0; i < AUDIO_MAX_BUFFERS; i++) {
        free(s_audio.bufs[i].buffer);
        s_audio.bufs[i].buffer = NULL;
    }
}

uint32_t slynx_audio_open(uint32_t frames, uint32_t buf_count) {
    if (s_audio.open) return MAKERESULT(Module_Libnx, LibnxError_AlreadyInitialized);
    if (frames == 0 || frames > 48000 || buf_count < 2 || buf_count > AUDIO_MAX_BUFFERS)
        return MAKERESULT(Module_Libnx, LibnxError_BadInput);
    memset(&s_audio, 0, sizeof(s_audio));

    // The service maps each buffer, so both address and size must be 4 KiB aligned.
    uint32_t data_bytes = frames * 2 * (uint32_t)sizeof(int16_t);
    uint32_t cap = (data_bytes + 0xFFF) & ~0xFFFu;
    for (uint32_t i = 0; i < buf_count; i++) {
        void* mem = memalign(0x1000, cap);
        if (!mem) {
            audio_free_buffers();
            return MAKERESULT(Module_Libnx, LibnxError_OutOfMemory);
        }
        memset(mem, 0, cap);
        armDCacheFlush(mem, cap);
        s_audio.bufs[i] = (AudioOutBuffer){NULL, mem, cap, data_bytes, 0};
    }

    Result rc = audoutInitialize();
    if (R_FAILED(rc)) {
        audio_free_buffers();
        return rc;
    }
    rc = audoutStartAudioOut();
    if (R_FAILED(rc)) {
        audoutExit();
        audio_free_buffers();
        return rc;
    }
    // Prime the queue with silence so the device runs from the start and released buffers drive the mixer.
    for (uint32_t i = 0; i < buf_count; i++) audoutAppendAudioOutBuffer(&s_audio.bufs[i]);

    s_audio.count = buf_count;
    s_audio.data_bytes = data_bytes;
    s_audio.open = true;
    return 0;
}

int16_t* slynx_audio_acquire(uint64_t timeout_ns) {
    if (!s_audio.open) return NULL;
    AudioOutBuffer* released = NULL;
    u32 n = 0;
    Result rc = audoutGetReleasedAudioOutBuffer(&released, &n);
    if (R_FAILED(rc) || n == 0 || !released) {
        released = NULL;
        n = 0;
        rc = audoutWaitPlayFinish(&released, &n, timeout_ns);
        if (R_FAILED(rc) || n == 0 || !released) return NULL;
    }
    return (int16_t*)released->buffer;
}

uint32_t slynx_audio_submit(int16_t* samples) {
    if (!s_audio.open) return MAKERESULT(Module_Libnx, LibnxError_NotInitialized);
    for (uint32_t i = 0; i < s_audio.count; i++) {
        AudioOutBuffer* b = &s_audio.bufs[i];
        if (b->buffer == samples) {
            b->data_size = s_audio.data_bytes;
            armDCacheFlush(b->buffer, b->buffer_size);
            return audoutAppendAudioOutBuffer(b);
        }
    }
    return MAKERESULT(Module_Libnx, LibnxError_BadInput);
}

void slynx_audio_close(void) {
    if (!s_audio.open) return;
    audoutStopAudioOut();
    audoutExit();
    audio_free_buffers();
    memset(&s_audio, 0, sizeof(s_audio));
}

// ---------------------------------------------------------------------------------------------------------------
// Files

bool slynx_romfs_available(void) { return s_romfs; }

int slynx_fs_open_read(const char* path, int* fd_out, uint64_t* size_out) {
    if (!path || !fd_out || !size_out) return EINVAL;
    int fd = open(path, O_RDONLY);
    if (fd < 0) return errno ? errno : EIO;
    struct stat st;
    if (fstat(fd, &st) != 0) {
        int e = errno ? errno : EIO;
        close(fd);
        return e;
    }
    if (S_ISDIR(st.st_mode)) {
        close(fd);
        return EISDIR;
    }
    *fd_out = fd;
    *size_out = (uint64_t)st.st_size;
    return 0;
}

int64_t slynx_fs_read_at(int fd, uint64_t offset, void* buf, size_t len) {
    if (lseek(fd, (off_t)offset, SEEK_SET) < 0) return -(int64_t)(errno ? errno : EIO);
    ssize_t n = read(fd, buf, len);
    if (n < 0) return -(int64_t)(errno ? errno : EIO);
    return (int64_t)n;
}

void slynx_fs_close(int fd) {
    if (fd >= 0) close(fd);
}

bool slynx_fs_exists(const char* path) {
    struct stat st;
    return path && stat(path, &st) == 0;
}

int slynx_fs_mkdirs(const char* dir) {
    if (!dir) return EINVAL;
    char buf[1024];
    size_t len = strlen(dir);
    if (len >= sizeof(buf)) return ENAMETOOLONG;
    memcpy(buf, dir, len + 1);
    // Skip the device prefix ("sdmc:/"): the root always exists.
    char* p = strstr(buf, ":/");
    p = p ? p + 2 : buf;
    for (;; p++) {
        if (*p == '/' || *p == '\0') {
            char saved = *p;
            *p = '\0';
            if (p > buf && p[-1] != '/' && mkdir(buf, 0777) != 0 && errno != EEXIST) {
                int e = errno ? errno : EIO;
                if (!slynx_fs_exists(buf)) return e;
            }
            *p = saved;
            if (saved == '\0') break;
        }
    }
    return 0;
}

int slynx_fs_write_replace(const char* path, const char* tmp_path, const void* data, size_t len) {
    if (!path || !tmp_path || (len && !data)) return EINVAL;
    int fd = open(tmp_path, O_WRONLY | O_CREAT | O_TRUNC, 0666);
    if (fd < 0) return errno ? errno : EIO;
    const uint8_t* p = (const uint8_t*)data;
    size_t left = len;
    while (left) {
        ssize_t n = write(fd, p, left);
        if (n <= 0) {
            int e = n < 0 && errno ? errno : EIO;
            close(fd);
            remove(tmp_path);
            return e;
        }
        p += n;
        left -= (size_t)n;
    }
    fsync(fd);
    if (close(fd) != 0) {
        int e = errno ? errno : EIO;
        remove(tmp_path);
        return e;
    }
    // The SD card's rename does not replace an existing file, so remove the old one first. A crash between the two
    // steps leaves the new contents under tmp_path; the next write replaces it.
    if (remove(path) != 0 && errno != ENOENT) {
        int e = errno ? errno : EIO;
        remove(tmp_path);
        return e;
    }
    if (rename(tmp_path, path) != 0) return errno ? errno : EIO;
    return 0;
}

int slynx_fs_remove(const char* path) {
    if (!path) return EINVAL;
    if (remove(path) != 0 && errno != ENOENT) return errno ? errno : EIO;
    return 0;
}
