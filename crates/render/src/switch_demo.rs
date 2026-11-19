//! Switch demo entry point (`--features switch-demo`): the procedural box grid on the deko3d backend.
//!
//! Controls: + exits; A toggles animation; X cycles the cull mode (back, front, none) to check the front-face
//! convention on hardware (with "back" the boxes must look solid, with "front" you see their insides); Y logs
//! the statistics now. Statistics are written to the debug log once a second.

use crate::deko3d::Deko3dBackend;
use crate::demo::{DemoParams, DemoScene};
use crate::{CullMode, Renderer};
use alloc::format;
use platform_switch::platform_api::{Buttons, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};

fn now() -> u64 {
    // The platform's clock is a method; the renderer wants a plain function. Ticks are global, so ask the platform
    // layer's C clock directly.
    unsafe extern "C" {
        fn slynx_ticks_ns() -> u64;
    }
    // SAFETY: reads the system tick counter; no preconditions.
    unsafe { slynx_ticks_ns() }
}

/// Called by libnx's startup code once the runtime is up.
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    debug_log("SlyNX render demo: start\n");
    let mut platform = match SwitchPlatform::new(SwitchConfig::default()) {
        Ok(p) => p,
        Err(e) => panic!("platform: {e}"),
    };
    let backend = match Deko3dBackend::new(&platform, 1 << 14, 1 << 14) {
        Ok(b) => b,
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    r.set_clock(now);
    let mut params = DemoParams::default();
    let scene = match DemoScene::build(&mut r, &params) {
        Ok(s) => s,
        Err(e) => panic!("scene: {e}"),
    };
    debug_log(&format!(
        "scene: {} instances, {} meshes, {} textures in {} pools\n",
        scene.instance_count(),
        scene.meshes,
        scene.textures,
        r.texture_pool_count()
    ));

    let start = now();
    let mut t = 0.0f32;
    let mut last = start;
    let (mut frames, mut cpu_ns, mut wait_ns, mut update_ns) = (0u64, 0u64, 0u64, 0u64);
    let mut log_at = start + 1_000_000_000;
    while platform.pump() {
        let pad = *platform.input().p1();
        if pad.pressed.any(Buttons::PLUS) {
            break;
        }
        if pad.pressed.any(Buttons::A) {
            params.animate = !params.animate;
        }
        if pad.pressed.any(Buttons::X) {
            let next = match r.cull_mode() {
                CullMode::Back => CullMode::Front,
                CullMode::Front => CullMode::None,
                CullMode::None => CullMode::Back,
            };
            r.set_cull_mode(next);
            debug_log(&format!("cull mode: {next:?}\n"));
        }
        let n = now();
        t += (n - last) as f32 * 1e-9;
        last = n;
        if params.animate
            && let Err(e) = scene.update(&mut r, t)
        {
            panic!("update: {e}");
        }
        let u = now() - n;
        let stats = match r.render(&scene.camera(t), [0.42, 0.62, 0.85, 1.0]) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        frames += 1;
        cpu_ns += stats.cpu_build_ns + stats.cpu_submit_ns;
        wait_ns += stats.wait_ns;
        update_ns += u;
        let now_ns = now();
        if now_ns >= log_at || pad.pressed.any(Buttons::Y) {
            let f = frames.max(1);
            debug_log(&format!(
                "fps {:.1} | render CPU {:.3} ms (build+submit), fence wait {:.3} ms, update {:.3} ms | {} visible, {} batches, {} draws, {} tris, cull {:?}\n",
                frames as f64 * 1e9 / (now_ns - (log_at - 1_000_000_000)).max(1) as f64,
                cpu_ns as f64 / f as f64 * 1e-6,
                wait_ns as f64 / f as f64 * 1e-6,
                update_ns as f64 / f as f64 * 1e-6,
                stats.visible,
                stats.batches,
                stats.draw_commands,
                stats.triangles,
                r.cull_mode(),
            ));
            frames = 0;
            cpu_ns = 0;
            wait_ns = 0;
            update_ns = 0;
            log_at = now_ns + 1_000_000_000;
        }
    }
    drop(r);
    drop(platform);
    debug_log("SlyNX render demo: exit\n");
    0
}
