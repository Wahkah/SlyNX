//! Threads on the PC: `std::thread`, optionally pinned to a core.

use crate::ffi;
use platform_api::{Core, JoinHandle, ThreadError, ThreadSpec};

/// A running thread. Dropping it waits for the thread to finish, like on the Switch.
pub struct PcThread(Option<std::thread::JoinHandle<()>>);

impl JoinHandle for PcThread {
    fn join(mut self) {
        if let Some(h) = self.0.take() {
            let _ = h.join();
        }
    }
}

impl Drop for PcThread {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            let _ = h.join();
        }
    }
}

pub fn spawn(spec: ThreadSpec, pin: bool, f: Box<dyn FnOnce() + Send + 'static>) -> Result<PcThread, ThreadError> {
    std::thread::Builder::new()
        .name(spec.name.to_string())
        .stack_size(spec.stack_size.max(64 * 1024))
        .spawn(move || {
            if pin {
                pin_current(spec.core);
            }
            f()
        })
        .map(|h| PcThread(Some(h)))
        .map_err(|e| ThreadError::Create(e.raw_os_error().unwrap_or(-1) as u32))
}

/// Pins the calling thread to `core` (modulo the number of CPUs). Best effort.
pub fn pin_current(core: Core) -> bool {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    let cpu = core.index() as usize % n;
    #[cfg(target_os = "linux")]
    {
        let mut mask = [0u64; 16]; // 1024 CPUs, glibc's cpu_set_t
        mask[(cpu / 64) % 16] |= 1 << (cpu % 64);
        // SAFETY: pid 0 is the calling thread; the mask is a live 128-byte buffer.
        unsafe { ffi::sched_setaffinity(0, size_of_val(&mask), mask.as_ptr()) == 0 }
    }
    #[cfg(windows)]
    {
        // SAFETY: GetCurrentThread returns a pseudo-handle for the calling thread.
        unsafe { ffi::SetThreadAffinityMask(ffi::GetCurrentThread(), 1usize << (cpu % usize::BITS as usize)) != 0 }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = cpu;
        false
    }
}

pub fn current_core() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: no preconditions.
        let c = unsafe { ffi::sched_getcpu() };
        if c >= 0 { Some(c as u32) } else { None }
    }
    #[cfg(windows)]
    {
        // SAFETY: no preconditions.
        Some(unsafe { ffi::GetCurrentProcessorNumber() })
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}
