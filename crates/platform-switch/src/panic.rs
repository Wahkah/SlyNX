//! Panic handler: formats the message into a fixed stack buffer (no allocation, the heap may be the problem),
//! sends it to the debug log, then aborts.

use core::fmt::Write;
use core::panic::PanicInfo;

/// A fixed-size text buffer that silently truncates.
struct StackBuf {
    buf: [u8; 512],
    len: usize,
}

impl Write for StackBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.buf.len() - self.len;
        let n = s.len().min(room);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    let mut out = StackBuf { buf: [0; 512], len: 0 };
    let _ = writeln!(out, "SlyNX panic: {info}");
    // Truncation may split a UTF-8 sequence; the log takes raw bytes, so that is harmless.
    crate::switch::debug_log_bytes(&out.buf[..out.len]);
    crate::switch::abort()
}
