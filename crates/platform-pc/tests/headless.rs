//! Headless platform: offscreen GL rendering and readback, scripted input, the fixed clock and threads. Everything
//! runs in one test function because SDL and the GL context are process-wide.
//!
//! Needs SDL2 and (on Linux) EGL with Mesa's surfaceless platform, or a display for the hidden-window fallback.

use platform_api::{
    Buttons, ControllerKind, Core, DebugDraw, Event, JoinHandle, MAX_PLAYERS, Platform, RawPad, Stick, ThreadSpec,
    Vertex,
};
use platform_pc::{PcConfig, PcPlatform};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

fn pixel(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    rgba[i..i + 4].try_into().unwrap()
}

#[test]
fn headless_platform() {
    let (w, h) = (64u32, 48u32);
    let mut p = PcPlatform::new(PcConfig::headless(w, h)).expect("headless platform");
    assert!(PcPlatform::new(PcConfig::headless(8, 8)).is_err(), "only one platform at a time");
    assert_eq!(p.name(), "pc-headless");

    let gl = p.gl();
    assert!(gl.is_offscreen());
    assert_ne!(gl.default_framebuffer(), 0);
    assert_eq!(gl.size(), (w, h));
    assert!(!gl.get_proc_address(c"glDrawElementsInstanced").is_null());
    let (version, renderer) = gl.describe();
    eprintln!("GL {version} on {renderer}");

    // Clear to red, then a green triangle in the top half only (y = -1 is the top).
    let mut draw = p.debug_draw(64).unwrap();
    let green = [0.0, 1.0, 0.0, 1.0];
    let tri = [
        Vertex { pos: [-1.0, -1.0], color: green },
        Vertex { pos: [3.0, -1.0], color: green },
        Vertex { pos: [-1.0, 0.0], color: green },
    ];
    // A partial trailing triangle is ignored.
    let mut verts = tri.to_vec();
    verts.push(Vertex::default());
    draw.frame([1.0, 0.0, 0.0, 1.0], &verts).unwrap();
    let px = p.read_pixels();
    assert_eq!(px.len(), (w * h * 4) as usize);
    assert_eq!(pixel(&px, w, 2, 2), [0, 255, 0, 255], "top-left is inside the triangle");
    assert_eq!(pixel(&px, w, w - 2, h - 2), [255, 0, 0, 255], "bottom-right is the clear colour");
    // Colours are stored as given: 0.5 is 128 (no gamma conversion).
    draw.frame([0.5, 0.25, 0.0, 1.0], &[]).unwrap();
    let c = pixel(&p.read_pixels(), w, 10, 10);
    assert!(c[0].abs_diff(128) <= 1 && c[1].abs_diff(64) <= 1 && c[2] == 0, "{c:?}");

    // Fixed clock: one step per pump.
    assert_eq!(p.now_ns(), 0);
    assert!(p.pump());
    assert!(p.pump());
    assert_eq!(p.now_ns(), 2 * 16_666_667);

    // Scripted input drives edges and connection events.
    let mut pads = [RawPad::DISCONNECTED; MAX_PLAYERS];
    pads[1] = RawPad {
        kind: ControllerKind::ProController,
        buttons: Buttons::A,
        left: Stick { x: 1.0, y: 0.0 },
        right: Stick::ZERO,
    };
    p.set_scripted_input(Some(pads));
    assert!(p.pump());
    assert_eq!(p.poll_event(), Some(Event::ControllerConnected(1)));
    assert_eq!(p.poll_event(), None);
    assert_eq!(p.input().players[1].pressed, Buttons::A);
    assert!((p.input().players[1].left.x - 1.0).abs() < 1e-6);
    assert!(p.pump());
    assert_eq!(p.input().players[1].pressed, Buttons::NONE);
    assert_eq!(p.input().players[1].held, Buttons::A);

    // Audio is off in the headless configuration.
    assert!(p.start_audio(Box::new(|_: &mut [f32]| {})).is_err());
    assert_eq!(p.audio_latency_ns(), 0);

    // Threads run and can be joined.
    let hits = Arc::new(AtomicU32::new(0));
    let h2 = hits.clone();
    let t = p
        .spawn(
            ThreadSpec::new("io", Core::Io),
            Box::new(move || {
                h2.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .unwrap();
    t.join();
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    drop(draw);
    drop(p);
    // The platform can be created again once the first is gone.
    let p = PcPlatform::new(PcConfig::headless(16, 16)).expect("second platform after drop");
    assert_eq!(p.display().width, 16);
}
