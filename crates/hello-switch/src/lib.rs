//! SlyNX Switch smoke test.
//!
//! Shows a slowly shifting blue clear, a spinning colour triangle and "SlyNX" in big block letters that bob, with a
//! status line underneath. Press + to return to the Homebrew Menu.
//!
//! The scene is built by plain Rust in [`scene`] (tested on a PC with `cargo test -p hello-switch`); only the
//! entry point below is Switch-specific.
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub mod font;
pub mod scene;

#[cfg(target_os = "horizon")]
mod entry {
    use crate::scene::Scene;
    use alloc::vec::Vec;
    use platform_switch::{Buttons, Gfx, Pad, applet_main_loop, debug_log, ticks_ns};

    /// Called by libnx's startup code once the runtime is up.
    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
        debug_log("SlyNX hello-switch: start\n");

        let mut gfx = match Gfx::new(Scene::MAX_VERTICES as u32) {
            Ok(g) => g,
            Err(e) => panic!("{e}"),
        };
        let mut pad = Pad::new().expect("pad already taken");
        let (w, h) = gfx.size();
        let scene = Scene::new(w as f32, h as f32);
        let mut vertices: Vec<platform_switch::Vertex> = Vec::with_capacity(Scene::MAX_VERTICES);
        let start = ticks_ns();

        while applet_main_loop() {
            let input = pad.poll();
            if input.down.contains(Buttons::PLUS) {
                break;
            }
            let t = (ticks_ns() - start) as f32 * 1e-9;
            vertices.clear();
            let clear = scene.build(t, &mut vertices);
            if let Err(e) = gfx.frame(clear, &vertices) {
                panic!("{e}");
            }
        }

        drop(gfx);
        debug_log("SlyNX hello-switch: exit\n");
        0
    }
}
