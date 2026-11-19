//! Deterministic, headless runs: "run N frames, capture frames K..., record performance".

use std::fmt;
use std::path::Path;

use crate::app::{App, AppError, FIXED_DT, FrameContext};
use crate::hash::Fnv64;
use crate::image::Image;
use crate::input::InputScript;
use crate::perf::{
    Clock, FrameProbe, PerfRecorder, PerfReport, STAGE_CAPTURE, STAGE_FRAME, STAGE_RENDER, STAGE_UPDATE,
};
use crate::png;
use crate::rng::Rng;

/// When the app renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Render every frame (needed for performance numbers).
    #[default]
    EveryFrame,
    /// Render only the frames that are captured (fast simulation-only runs).
    CaptureFramesOnly,
}

/// What a run does.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// Frames to run.
    pub frames: u64,
    /// Frames to capture after rendering, each below `frames`.
    pub captures: Vec<u64>,
    /// Seed for the run's [`Rng`].
    pub seed: u64,
    /// Scripted input.
    pub script: InputScript,
    /// Frames at the start left out of the performance statistics.
    pub warmup: u64,
    pub render: RenderMode,
    /// The clock for performance numbers. Tests can pass a manual clock.
    pub clock: Clock,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig {
            frames: 60,
            captures: Vec::new(),
            seed: 0,
            script: InputScript::empty(),
            warmup: 0,
            render: RenderMode::EveryFrame,
            clock: Clock::real(),
        }
    }
}

/// A run that stopped early.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunError {
    /// The frame where it stopped (`None` for a configuration error before frame 0).
    pub frame: Option<u64>,
    pub message: String,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.frame {
            Some(fr) => write!(f, "frame {fr}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for RunError {}

/// Everything a run produced.
#[derive(Clone, Debug)]
pub struct RunResult {
    /// Frames run.
    pub frames: u64,
    /// The captured frames, in frame order.
    pub captures: Vec<(u64, Image)>,
    /// The app's state hash after each frame's update, if it provides one.
    pub state_hashes: Vec<Option<u64>>,
    /// Per-frame performance samples.
    pub perf: PerfRecorder,
    seed: u64,
    warmup: u64,
}

impl RunResult {
    /// The performance report for the run.
    pub fn perf_report(&self, app_name: &str) -> PerfReport {
        self.perf.report(app_name, self.seed, self.warmup)
    }

    /// The captured image of a frame.
    pub fn capture(&self, frame: u64) -> Option<&Image> {
        self.captures.iter().find(|(f, _)| *f == frame).map(|(_, img)| img)
    }

    /// One hash of everything deterministic about the run: every state hash and every captured image.
    pub fn fingerprint(&self) -> u64 {
        let mut h = Fnv64::new();
        for s in &self.state_hashes {
            h.write_u64(s.unwrap_or(0));
        }
        for (f, img) in &self.captures {
            h.write_u64(*f).write_u32(img.width()).write_u32(img.height()).write(img.pixels());
        }
        h.finish()
    }

    /// Writes the outputs to `dir`: `frame_NNNNNN.png` per capture, `hashes.txt` (one line per frame), and
    /// `perf.json` and `perf.txt`.
    pub fn write_to(&self, dir: &Path, app_name: &str) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (f, img) in &self.captures {
            let p = dir.join(capture_file_name(*f));
            png::write_file(&p, img).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        let mut hashes = format!("# frame state_hash (fingerprint {:016x})\n", self.fingerprint());
        for (f, s) in self.state_hashes.iter().enumerate() {
            hashes.push_str(&match s {
                Some(h) => format!("{f} {h:016x}\n"),
                None => format!("{f} -\n"),
            });
        }
        let report = self.perf_report(app_name);
        for (name, text) in [("hashes.txt", hashes), ("perf.json", report.to_json()), ("perf.txt", report.to_text())] {
            let p = dir.join(name);
            std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }
}

/// The file name used for a captured frame.
pub fn capture_file_name(frame: u64) -> String {
    format!("frame_{frame:06}.png")
}

/// Runs `app` as configured.
pub fn run(app: &mut dyn App, config: &RunConfig) -> Result<RunResult, RunError> {
    if let Some(&bad) = config.captures.iter().find(|&&k| k >= config.frames) {
        return Err(RunError {
            frame: None,
            message: format!("capture frame {bad} is not below the run length of {} frames", config.frames),
        });
    }
    let mut rng = Rng::new(config.seed);
    let mut player = config.script.player();
    let mut recorder = PerfRecorder::new();
    let mut captures = Vec::new();
    let mut state_hashes = Vec::with_capacity(config.frames as usize);
    let at = |frame: u64| move |e: AppError| RunError { frame: Some(frame), message: e.0 };

    for frame in 0..config.frames {
        let input = player.next_frame();
        let capture = config.captures.contains(&frame);
        let mut probe = FrameProbe::new(config.clock.clone(), frame);
        let whole = probe.start(STAGE_FRAME);
        let mut ctx = FrameContext {
            frame,
            dt: FIXED_DT,
            input: &input,
            rng: &mut rng,
            probe: &mut probe,
            capture_requested: capture,
        };

        let t = ctx.probe.start(STAGE_UPDATE);
        app.update(&mut ctx).map_err(at(frame))?;
        ctx.probe.stop(t);
        state_hashes.push(app.state_hash());

        if capture || config.render == RenderMode::EveryFrame {
            let t = ctx.probe.start(STAGE_RENDER);
            app.render(&mut ctx).map_err(at(frame))?;
            ctx.probe.stop(t);
        }
        probe.stop(whole);

        if capture {
            let t = probe.start(STAGE_CAPTURE);
            let img = app.capture().map_err(at(frame))?;
            probe.stop(t);
            captures.push((frame, img));
        }
        recorder.push(probe.finish());
    }
    app.shutdown();
    Ok(RunResult {
        frames: config.frames,
        captures,
        state_hashes,
        perf: recorder,
        seed: config.seed,
        warmup: config.warmup,
    })
}

/// Where two runs first disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Divergence {
    /// The state hash differs from this frame on.
    StateHash { frame: u64, first: Option<u64>, second: Option<u64> },
    /// The captured image of this frame differs.
    Capture { frame: u64 },
    /// The runs captured different frames or ran a different number of frames.
    Shape,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Divergence::StateHash { frame, first, second } => {
                write!(f, "state hash differs at frame {frame}: {first:016x?} vs {second:016x?}")
            }
            Divergence::Capture { frame } => write!(f, "captured image differs at frame {frame}"),
            Divergence::Shape => f.write_str("runs differ in length or captured frames"),
        }
    }
}

/// Compares two runs and returns where they first diverge, or `None` if they agree.
pub fn first_divergence(a: &RunResult, b: &RunResult) -> Option<Divergence> {
    if a.frames != b.frames || a.captures.len() != b.captures.len() {
        return Some(Divergence::Shape);
    }
    for (frame, (x, y)) in a.state_hashes.iter().zip(&b.state_hashes).enumerate() {
        if x != y {
            return Some(Divergence::StateHash { frame: frame as u64, first: *x, second: *y });
        }
    }
    for ((fa, ia), (fb, ib)) in a.captures.iter().zip(&b.captures) {
        if fa != fb {
            return Some(Divergence::Shape);
        }
        if ia != ib {
            return Some(Divergence::Capture { frame: *fa });
        }
    }
    None
}

/// Runs a fresh app from `make` twice with the same configuration and checks the runs agree.
pub fn check_determinism(
    mut make: impl FnMut() -> Result<Box<dyn App>, AppError>,
    config: &RunConfig,
) -> Result<RunResult, String> {
    let mut first_app = make().map_err(|e| e.to_string())?;
    let first = run(first_app.as_mut(), config).map_err(|e| e.to_string())?;
    drop(first_app);
    let mut second_app = make().map_err(|e| e.to_string())?;
    let second = run(second_app.as_mut(), config).map_err(|e| e.to_string())?;
    match first_divergence(&first, &second) {
        None => Ok(first),
        Some(d) => Err(format!("not deterministic: {d}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppConfig;
    use crate::apps::PatternApp;
    use crate::input::Button;
    use crate::perf::{DRAW_CALLS, ManualClock};

    fn pattern(seed: u64) -> Box<dyn App> {
        Box::new(PatternApp::new(&AppConfig { width: 48, height: 32, seed, ..Default::default() }))
    }

    fn script() -> InputScript {
        InputScript::parse("0..40 hold right\n10 tap a\n20..50 stick left 0.25 1\n60 down b\n").unwrap()
    }

    #[test]
    fn same_seed_and_input_give_identical_runs() {
        let config =
            RunConfig { frames: 90, captures: vec![0, 45, 89], seed: 1234, script: script(), ..Default::default() };
        let result = check_determinism(|| Ok(pattern(1234)), &config).unwrap();
        assert_eq!(result.captures.len(), 3);
        assert_eq!(result.capture(45).unwrap().width(), 48);
        assert!(result.state_hashes.iter().all(Option::is_some));
    }

    #[test]
    fn different_seed_or_input_diverges() {
        let base = RunConfig { frames: 60, captures: vec![59], seed: 1, script: script(), ..Default::default() };
        let a = run(pattern(1).as_mut(), &base).unwrap();

        let other_seed = RunConfig { seed: 2, ..base.clone() };
        let b = run(pattern(1).as_mut(), &other_seed).unwrap();
        assert!(first_divergence(&a, &b).is_some());
        assert_ne!(a.fingerprint(), b.fingerprint());

        let other_input = RunConfig { script: InputScript::parse("30 tap a").unwrap(), ..base.clone() };
        let c = run(pattern(1).as_mut(), &other_input).unwrap();
        match first_divergence(&a, &c) {
            Some(Divergence::StateHash { frame, .. }) => {
                assert_eq!(frame, 0, "input differs from frame 0 (right held)")
            }
            other => panic!("expected a state divergence, got {other:?}"),
        }
    }

    #[test]
    fn input_reaches_the_app() {
        struct Recorder(Vec<u32>);
        impl App for Recorder {
            fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
                assert_eq!(ctx.dt, FIXED_DT);
                assert!((ctx.time() - ctx.frame as f64 / 60.0).abs() < 1e-12);
                self.0.push(ctx.input.pressed);
                Ok(())
            }
            fn render(&mut self, _: &mut FrameContext<'_>) -> Result<(), AppError> {
                Ok(())
            }
            fn capture(&mut self) -> Result<Image, AppError> {
                Err("no capture".into())
            }
        }
        let mut app = Recorder(Vec::new());
        let config =
            RunConfig { frames: 5, script: InputScript::parse("2 tap a\n3 tap b").unwrap(), ..Default::default() };
        let r = run(&mut app, &config).unwrap();
        assert_eq!(app.0, vec![0, 0, Button::A.bit(), Button::B.bit(), 0]);
        assert!(r.state_hashes.iter().all(Option::is_none));

        let capture = RunConfig { captures: vec![3], ..config.clone() };
        let e = run(&mut Recorder(Vec::new()), &capture).unwrap_err();
        assert_eq!(e.frame, Some(3));
        assert_eq!(e.to_string(), "frame 3: no capture");
    }

    #[test]
    fn rejects_capture_past_the_end() {
        let config = RunConfig { frames: 10, captures: vec![10], ..Default::default() };
        let e = run(pattern(0).as_mut(), &config).unwrap_err();
        assert_eq!(e.frame, None);
    }

    #[test]
    fn perf_numbers_from_a_manual_clock() {
        struct Slow(ManualClock);
        impl App for Slow {
            fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
                self.0.advance_ms(if ctx.frame == 7 { 20.0 } else { 2.0 });
                Ok(())
            }
            fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
                let mut s = ctx.probe.scope("render.submit");
                self.0.advance_ms(3.0);
                s.count(DRAW_CALLS, 12);
                Ok(())
            }
            fn capture(&mut self) -> Result<Image, AppError> {
                self.0.advance_ms(50.0);
                Ok(Image::new(1, 1, [0; 4]))
            }
        }
        let (clock, hand) = Clock::manual();
        let mut app = Slow(hand);
        let config = RunConfig { frames: 20, captures: vec![10], warmup: 2, clock, ..Default::default() };
        let report = run(&mut app, &config).unwrap().perf_report("slow");
        let frame = report.stage(STAGE_FRAME).unwrap();
        assert_eq!(frame.count, 18);
        assert_eq!(frame.median, 5.0);
        assert_eq!(frame.max, 23.0);
        assert_eq!(frame.worst_frame, 7);
        assert_eq!(report.stage(STAGE_UPDATE).unwrap().median, 2.0);
        assert_eq!(report.stage("render.submit").unwrap().mean, 3.0);
        assert_eq!(report.stage(STAGE_CAPTURE).unwrap().count, 1);
        assert_eq!(report.stage(STAGE_CAPTURE).unwrap().max, 50.0);
        assert_eq!(report.counter(DRAW_CALLS).unwrap().median, 12.0);
    }

    #[test]
    fn capture_only_mode_skips_rendering() {
        let config =
            RunConfig { frames: 30, captures: vec![29], render: RenderMode::CaptureFramesOnly, ..Default::default() };
        let r = run(pattern(0).as_mut(), &config).unwrap();
        let rendered = r.perf.samples().iter().filter(|s| s.stages.iter().any(|(n, _)| n == STAGE_RENDER)).count();
        assert_eq!(rendered, 1);
        // The image is the same as when every frame is rendered: rendering must not change state.
        let full = run(pattern(0).as_mut(), &RunConfig { render: RenderMode::EveryFrame, ..config.clone() }).unwrap();
        assert_eq!(r.capture(29), full.capture(29));
    }

    #[test]
    fn writes_outputs() {
        let dir = std::env::temp_dir().join(format!("slynx-harness-run-{}", std::process::id()));
        let config = RunConfig { frames: 12, captures: vec![3, 11], seed: 5, ..Default::default() };
        let r = run(pattern(5).as_mut(), &config).unwrap();
        r.write_to(&dir, "pattern").unwrap();
        let img = png::read_file(dir.join("frame_000011.png")).unwrap();
        assert_eq!(&img, r.capture(11).unwrap());
        let hashes = std::fs::read_to_string(dir.join("hashes.txt")).unwrap();
        assert_eq!(hashes.lines().count(), 13);
        let report = PerfReport::from_json(&std::fs::read_to_string(dir.join("perf.json")).unwrap()).unwrap();
        assert_eq!(report.frames, 12);
        assert_eq!(report.seed, 5);
        let _ = std::fs::remove_dir_all(dir);
    }
}
