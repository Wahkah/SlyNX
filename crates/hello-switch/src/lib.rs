//! SlyNX platform demo.
//!
//! Shows every player's controller live (buttons, sticks after the deadzone, controller kind), plays a tone on A, B,
//! X or Y, reads a RomFS file on the I/O core, counts launches in a save file, toggles CPU boost on ZR and reports
//! the display mode, applet events and frame rate. Press + to exit.
//!
//! The demo ([`demo`]) and the frame ([`scene`]) are plain Rust written against `platform_api::Platform`. The Switch
//! entry point is below; the PC one is `examples/hello_pc.rs` (`cargo run -p hello-switch --example hello_pc`).
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub mod demo;
pub mod font;
pub mod scene;
pub mod tone;

#[cfg(target_os = "horizon")]
mod entry {
    use crate::demo::Demo;
    use crate::scene::Scene;
    use alloc::vec::Vec;
    use platform_switch::platform_api::{DebugDraw, Platform, Vertex};
    use platform_switch::{SwitchConfig, SwitchDebugDraw, SwitchPlatform, debug_log};

    /// Called by libnx's startup code once the runtime is up.
    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
        debug_log("SlyNX hello-switch: start\n");

        let mut platform = match SwitchPlatform::new(SwitchConfig::default()) {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        let mut draw = match SwitchDebugDraw::new(&platform, Scene::MAX_VERTICES as u32) {
            Ok(d) => d,
            Err(e) => panic!("{e}"),
        };
        let mut demo = Demo::new(&mut platform);
        let mut vertices: Vec<Vertex> = Vec::with_capacity(Scene::MAX_VERTICES);

        while platform.pump() {
            vertices.clear();
            let Some(clear) = demo.frame(&mut platform, &mut vertices) else { break };
            if let Err(e) = draw.frame(clear, &vertices) {
                panic!("{e}");
            }
        }

        drop(draw);
        drop(platform);
        debug_log("SlyNX hello-switch: exit\n");
        0
    }
}
