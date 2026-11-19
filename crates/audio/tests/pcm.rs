//! The PCM stream input through the engine: attached, mixed, paused, detached.

use audio::pcm::{PCM_RATE, pcm_stream};
use audio::{Config, create};

#[test]
fn pcm_stream_plays_through_the_renderer() {
    let (mut engine, mut renderer) = create(Config::default());
    let (mut w, r) = pcm_stream(PCM_RATE as usize / 5, 2);
    engine.attach_pcm(Some(r));
    // 100 ms of a constant stereo frame.
    let frames: Vec<i16> = (0..4800).flat_map(|_| [8192i16, -8192]).collect();
    assert_eq!(w.write(&frames), 4800);
    assert!((w.latency() - 0.1).abs() < 1e-6);
    let mut out = vec![0.0f32; 800 * 2];
    engine.update(1.0 / 60.0);
    renderer.render(&mut out);
    assert_eq!((out[0], out[1]), (0.25, -0.25));
    assert_eq!(w.played_frames(), 800);
    assert_eq!(w.buffered_frames(), 4000);
    // Paused: nothing is consumed and the clock holds.
    engine.set_paused(true);
    engine.update(1.0 / 60.0);
    renderer.render(&mut out);
    assert_eq!(w.played_frames(), 800);
    assert!(out.iter().all(|&s| s == 0.0));
    // Resuming takes effect from the block after the one that reads the command (the engine's pause rule).
    engine.set_paused(false);
    engine.update(1.0 / 60.0);
    renderer.render(&mut out);
    renderer.render(&mut out);
    assert_eq!(w.played_frames(), 1600);
    // Detached: the renderer stops reading it.
    engine.attach_pcm(None);
    engine.update(1.0 / 60.0);
    renderer.render(&mut out);
    assert_eq!(w.played_frames(), 1600);
    assert!(out.iter().all(|&s| s == 0.0));
    assert_eq!(w.buffered_frames(), 3200);
    engine.update(1.0 / 60.0);
    assert_eq!(w.underruns(), 0);
}
