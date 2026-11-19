//! The Switch entry point (`dist/slynx-video-bench.nro`): runs the benchmark twice, at the default CPU clock and
//! with the CPU boost, writing `sdmc:/switch/SlyNX/user/video-bench.log` after every movie and each line to the
//! debug log. No graphics: the screen stays as the Homebrew Menu left it. Press + to stop early.

use alloc::string::String;

use platform_switch::platform_api::{Buttons, FileSystem, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};

use crate::{LOG_FILE, Settings, run};

/// Called by libnx's startup code once the runtime is up.
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    debug_log("SlyNX video-bench: start\n");
    let mut platform = match SwitchPlatform::new(SwitchConfig::default()) {
        Ok(p) => p,
        Err(e) => panic!("platform: {e}"),
    };
    let fs = platform.fs();
    let settings = Settings::load(&fs);
    let mut text = String::new();
    let platform = core::cell::RefCell::new(&mut platform);
    let now = || platform.borrow().now_ns();
    let mut stop = false;
    for boost in [false, true] {
        platform.borrow_mut().set_cpu_boost(boost);
        let mut log = |line: &str| {
            debug_log(line);
            debug_log("\n");
            text.push_str(line);
            text.push('\n');
            // Written after every line, so a run cut short still leaves its results.
            let _ = fs.write_user(LOG_FILE, text.as_bytes());
        };
        let mut keep_going = || {
            let mut p = platform.borrow_mut();
            if !p.pump() || p.input().p1().pressed.0 & Buttons::PLUS.0 != 0 {
                stop = true;
            }
            !stop
        };
        run(&fs, &settings, boost, &now, &mut log, &mut keep_going);
        if stop {
            break;
        }
    }
    platform.borrow_mut().set_cpu_boost(false);
    debug_log("SlyNX video-bench: done\n");
    0
}
