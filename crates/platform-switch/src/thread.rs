//! Threads on libnx, each created on one core.

use crate::sys;
use alloc::boxed::Box;
use core::ffi::c_void;
use platform_api::{Core, JoinHandle, Priority, ThreadError, ThreadSpec};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A running thread. Dropping it waits for the thread to finish.
pub struct SwitchThread {
    handle: *mut c_void,
}

// SAFETY: the handle is an owned libnx `Thread`; waiting for and closing it from another thread is allowed.
unsafe impl Send for SwitchThread {}

/// Horizon priorities: lower runs first; the main thread is 0x2C.
fn priority(p: Priority) -> i32 {
    match p {
        Priority::High => 0x2B,
        Priority::Normal => 0x2C,
        Priority::Low => 0x2D,
    }
}

extern "C" fn trampoline(arg: *mut c_void) {
    // SAFETY: `arg` is the `Box<Job>` leaked by `spawn`, handed to exactly one thread.
    let job = unsafe { Box::from_raw(arg as *mut Job) };
    job();
}

pub fn spawn(spec: ThreadSpec, f: Job) -> Result<SwitchThread, ThreadError> {
    let arg = Box::into_raw(Box::new(f)) as *mut c_void;
    let mut rc = 0u32;
    // SAFETY: `trampoline` takes ownership of `arg` when the thread runs; on failure no thread exists, so we take
    // it back below.
    let handle = unsafe {
        sys::slynx_thread_start(
            trampoline,
            arg,
            spec.stack_size.max(16 * 1024),
            priority(spec.priority),
            spec.core.index() as i32,
            &mut rc,
        )
    };
    if handle.is_null() {
        // SAFETY: the thread was never started, so `arg` is still ours.
        drop(unsafe { Box::from_raw(arg as *mut Job) });
        return Err(ThreadError::Create(rc));
    }
    Ok(SwitchThread { handle })
}

pub fn pin_current(core: Core) -> bool {
    // SAFETY: no preconditions; affects only the calling thread.
    unsafe { sys::slynx_thread_pin_current(core.index() as i32) }
}

impl SwitchThread {
    fn wait(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: `handle` came from `slynx_thread_start` and is joined once (nulled after).
            unsafe { sys::slynx_thread_join(self.handle) };
            self.handle = core::ptr::null_mut();
        }
    }
}

impl JoinHandle for SwitchThread {
    fn join(mut self) {
        self.wait();
    }
}

impl Drop for SwitchThread {
    fn drop(&mut self) {
        self.wait();
    }
}
