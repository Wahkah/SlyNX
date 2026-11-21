//! The on-screen performance overlay and its log, for hardware tests.
//!
//! [`PerfMeter`] collects per-frame numbers (frame time, the program's CPU time, the renderer's draw commands and
//! the GPU time the backend measures) as running averages for the screen, and as five-second summaries (average and
//! worst) for a log the program writes to its user folder. [`PerfMeter::toggle`] turns the screen text on and off
//! with a button combo; [`crate::Renderer::enable_perf_text`] and [`crate::Renderer::set_perf_text`] draw it in a
//! corner of the screen with the renderer's own text, so programs need no level resources for it.
//!
//! The cost when shown is one text update per frame (a few dozen instances); when hidden, nothing but the averages.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::frame::FrameStats;

/// Buttons held for the combo: L and R (`platform_api::Buttons` bits 6 and 7) ...
pub const COMBO_HELD: u32 = (1 << 6) | (1 << 7);
/// ... and ZL (bit 8) pressed.
pub const COMBO_PRESS: u32 = 1 << 8;
/// Seconds per log summary.
pub const LOG_PERIOD_S: f64 = 5.0;
/// The log keeps its last this many bytes.
pub const LOG_MAX_BYTES: usize = 64 * 1024;
/// Characters the screen text needs at most.
pub const TEXT_CHARS: usize = 128;
/// The log's path in the user folder (`sdmc:/switch/SlyNX/user/perf.log` on the Switch).
pub const LOG_PATH: &str = "perf.log";

/// Running averages (for the screen) and the current log window.
#[derive(Clone, Debug, Default)]
pub struct PerfMeter {
    /// Screen text shown.
    pub on: bool,
    frame_ms: f64,
    cpu_ms: f64,
    render_ms: f64,
    gpu_ms: Option<f64>,
    draws: u32,
    triangles: u64,
    // The log window.
    elapsed_s: f64,
    window_s: f64,
    frames: u32,
    sum_frame: f64,
    max_frame: f64,
    sum_cpu: f64,
    max_cpu: f64,
    sum_gpu: f64,
    max_gpu: f64,
    gpu_frames: u32,
    sum_draws: u64,
    log: String,
    /// A summary was added since [`PerfMeter::log_due`] last said so.
    dirty: bool,
}

/// Exponential average with weight 1/16 per frame.
fn ema(old: f64, new: f64) -> f64 {
    if old == 0.0 { new } else { old + (new - old) / 16.0 }
}

impl PerfMeter {
    pub fn new() -> PerfMeter {
        let mut m = PerfMeter::default();
        m.log.push_str(
            "# time_s frames fps frame_ms_avg frame_ms_max cpu_ms_avg cpu_ms_max gpu_ms_avg gpu_ms_max draws_avg\n",
        );
        m
    }

    /// Flips the screen text when the combo (L + R held, ZL pressed this frame) is in `held` and `pressed`
    /// (`platform_api::Buttons` bits); returns true when it flipped.
    pub fn toggle(&mut self, held: u32, pressed: u32) -> bool {
        if held & COMBO_HELD == COMBO_HELD && pressed & COMBO_PRESS != 0 {
            self.on = !self.on;
            return true;
        }
        false
    }

    /// Adds a frame: `frame_ns` since the previous one, `cpu_ns` of work in it (update and render, without waiting
    /// for the display), and the renderer's stats.
    pub fn record(&mut self, frame_ns: u64, cpu_ns: u64, s: &FrameStats) {
        let frame = frame_ns as f64 * 1e-6;
        let cpu = cpu_ns as f64 * 1e-6;
        self.frame_ms = ema(self.frame_ms, frame);
        self.cpu_ms = ema(self.cpu_ms, cpu);
        self.render_ms = ema(self.render_ms, s.cpu_ms());
        self.draws = s.draw_commands;
        self.triangles = s.triangles;
        if let Some(g) = s.gpu_ns {
            let g = g as f64 * 1e-6;
            self.gpu_ms = Some(ema(self.gpu_ms.unwrap_or(0.0), g));
            self.sum_gpu += g;
            self.max_gpu = self.max_gpu.max(g);
            self.gpu_frames += 1;
        }
        self.frames += 1;
        self.sum_frame += frame;
        self.max_frame = self.max_frame.max(frame);
        self.sum_cpu += cpu;
        self.max_cpu = self.max_cpu.max(cpu);
        self.sum_draws += s.draw_commands as u64;
        self.window_s += frame * 1e-3;
        self.elapsed_s += frame * 1e-3;
        if self.window_s >= LOG_PERIOD_S {
            self.close_window();
        }
    }

    fn close_window(&mut self) {
        let n = self.frames.max(1) as f64;
        let gpu = if self.gpu_frames > 0 {
            format!("{:.2} {:.2}", self.sum_gpu / self.gpu_frames as f64, self.max_gpu)
        } else {
            String::from("- -")
        };
        let line = format!(
            "{:.1} {} {:.1} {:.2} {:.2} {:.2} {:.2} {} {:.0}\n",
            self.elapsed_s,
            self.frames,
            self.frames as f64 / self.window_s.max(1e-6),
            self.sum_frame / n,
            self.max_frame,
            self.sum_cpu / n,
            self.max_cpu,
            gpu,
            self.sum_draws as f64 / n
        );
        self.log.push_str(&line);
        self.dirty = true;
        if self.log.len() > LOG_MAX_BYTES {
            // Keep the header and the newest lines.
            let header_end = self.log.find('\n').map_or(0, |i| i + 1);
            let cut = self.log.len() - LOG_MAX_BYTES / 2;
            let from = self.log[cut..].find('\n').map_or(self.log.len(), |i| cut + i + 1);
            let mut kept = String::from(&self.log[..header_end]);
            kept.push_str(&self.log[from..]);
            self.log = kept;
        }
        self.window_s = 0.0;
        self.frames = 0;
        (self.sum_frame, self.max_frame, self.sum_cpu, self.max_cpu) = (0.0, 0.0, 0.0, 0.0);
        (self.sum_gpu, self.max_gpu, self.gpu_frames, self.sum_draws) = (0.0, 0.0, 0, 0);
    }

    /// Whether to write the log out now: a summary was added since the last write and the overlay is on. While it
    /// is off nothing is written (no storage writes on the game thread in normal play); turning it on writes the
    /// summaries kept meanwhile.
    pub fn log_due(&mut self) -> bool {
        let due = self.dirty && self.on;
        if due {
            self.dirty = false;
        }
        due
    }

    /// The log so far: a header, then one line per five seconds.
    pub fn log(&self) -> &str {
        &self.log
    }

    /// Once per frame before rendering: follows the combo in `held` / `pressed` and puts the current text on screen
    /// (or takes it off). Needs [`crate::Renderer::enable_perf_text`].
    pub fn show<B: crate::Backend>(
        &mut self,
        r: &mut crate::Renderer<B>,
        held: u32,
        pressed: u32,
    ) -> Result<(), crate::RenderError> {
        let flipped = self.toggle(held, pressed);
        if self.on {
            let lines = self.lines();
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            r.set_perf_text(&refs)?;
        } else if flipped {
            r.set_perf_text(&[])?;
        }
        Ok(())
    }

    /// The screen text (empty while off).
    pub fn lines(&self) -> Vec<String> {
        if !self.on {
            return Vec::new();
        }
        let fps = if self.frame_ms > 0.0 { 1000.0 / self.frame_ms } else { 0.0 };
        let gpu = self.gpu_ms.map_or(String::from("-"), |g| format!("{g:.2}"));
        alloc::vec![
            format!(
                "FPS {fps:.1}  FRAME {:.2} MS  CPU {:.2} MS (RENDER {:.2})",
                self.frame_ms, self.cpu_ms, self.render_ms
            ),
            format!("GPU {gpu} MS  DRAWS {}  TRIS {}K", self.draws, self.triangles / 1000),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_toggles() {
        let mut m = PerfMeter::new();
        assert!(!m.toggle(COMBO_HELD, 0));
        assert!(!m.toggle(1 << 6, COMBO_PRESS), "R not held");
        assert!(m.toggle(COMBO_HELD | COMBO_PRESS, COMBO_PRESS) && m.on);
        assert!(m.toggle(COMBO_HELD, COMBO_PRESS) && !m.on);
        assert!(m.lines().is_empty());
    }

    #[test]
    fn averages_and_log_windows() {
        let mut m = PerfMeter::new();
        m.on = true;
        let s = FrameStats { draw_commands: 100, gpu_ns: Some(8_000_000), ..FrameStats::default() };
        // Six seconds at 60 fps: one summary line.
        for _ in 0..360 {
            m.record(16_666_667, 5_000_000, &s);
        }
        assert!(m.log_due() && !m.log_due(), "due once per summary");
        assert_eq!(m.log().lines().count(), 2);
        let line = m.log().lines().nth(1).unwrap();
        let f: Vec<&str> = line.split(' ').collect();
        assert_eq!(f[2], "60.0", "{line}");
        assert_eq!(f[7], "8.00");
        let text = m.lines();
        assert!(text[0].starts_with("FPS 60.0") && text[1].contains("DRAWS 100"), "{text:?}");
    }

    #[test]
    fn no_log_writes_while_the_overlay_is_off() {
        let mut m = PerfMeter::new();
        let s = FrameStats::default();
        for _ in 0..360 {
            m.record(16_666_667, 5_000_000, &s);
        }
        assert!(!m.log_due(), "off: nothing to write");
        m.on = true;
        assert!(m.log_due() && !m.log_due(), "on: the kept summary is written once");
    }

    #[test]
    fn the_log_stays_small() {
        let mut m = PerfMeter::new();
        let s = FrameStats::default();
        for _ in 0..200_000 {
            m.record(100_000_000, 1, &s);
        }
        assert!(m.log().len() <= LOG_MAX_BYTES);
        assert!(m.log().starts_with("# time_s"));
    }

    #[test]
    fn the_renderer_draws_the_text_with_every_level() {
        use crate::{Camera, NullBackend, Renderer, math::Vec3};
        let cam = Camera {
            eye: Vec3::new(0.0, -5.0, 0.0),
            target: Vec3::ZERO,
            up: Vec3::Z,
            fov_y: 1.0,
            near: 0.1,
            far: 100.0,
        };
        let mut r = Renderer::new(NullBackend::new(640, 448));
        r.enable_perf_text(TEXT_CHARS);
        r.commit().unwrap();
        r.set_perf_text(&["FPS 60"]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 5, "five glyphs, the space not drawn");
        // A new level brings the text back with its commit; hidden text draws nothing.
        r.clear_level();
        r.commit().unwrap();
        r.set_perf_text(&["AB"]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 2);
        r.set_perf_text(&[]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 0);
    }
}
