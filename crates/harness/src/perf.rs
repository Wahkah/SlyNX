//! Performance measurement: per-frame stage timers and counters, summary statistics, reports and baselines.
//!
//! During a run each frame gets a [`FrameProbe`]. The harness times the `update`, `render` and `capture` stages and
//! the whole `frame` (update plus render); the app adds its own stages (`probe.scope("render.cull")`) and counters
//! (`probe.count(DRAW_CALLS, n)`). A [`PerfRecorder`] keeps every frame's numbers and turns them into a
//! [`PerfReport`]: mean, median, 95th and 99th percentile, best and worst frame, per stage and per counter. Reports
//! are written as text and JSON and compared against a stored baseline with [`compare_reports`].
//!
//! Stages may nest and are recorded flat, by name, so nested stage times are not additive. Stage names are free-form;
//! use `parent.child` for sub-stages by convention.

use std::cell::Cell;
use std::fmt::Write as _;
use std::rc::Rc;
use std::time::Instant;

use crate::json::{JsonError, Value};

/// Stage name for the whole frame (update plus render), timed by the harness.
pub const STAGE_FRAME: &str = "frame";
/// Stage name for [`App::update`](crate::App::update), timed by the harness.
pub const STAGE_UPDATE: &str = "update";
/// Stage name for [`App::render`](crate::App::render), timed by the harness.
pub const STAGE_RENDER: &str = "render";
/// Stage name for [`App::capture`](crate::App::capture), timed by the harness on capture frames only. Not part of
/// `frame`.
pub const STAGE_CAPTURE: &str = "capture";
/// Counter name for draw calls submitted by the renderer in a frame.
pub const DRAW_CALLS: &str = "draw_calls";
/// Counter name for triangles submitted by the renderer in a frame.
pub const TRIANGLES: &str = "triangles";

/// Where timings come from: the real monotonic clock, or a manual clock for tests.
#[derive(Clone, Debug)]
pub enum Clock {
    /// `std::time::Instant`, measured from when the clock was created.
    Real(Instant),
    /// A shared counter in nanoseconds, advanced by hand.
    Manual(Rc<Cell<u64>>),
}

impl Default for Clock {
    fn default() -> Self {
        Clock::real()
    }
}

impl Clock {
    /// The real clock.
    pub fn real() -> Self {
        Clock::Real(Instant::now())
    }

    /// A manual clock at 0 and the handle that advances it.
    pub fn manual() -> (Self, ManualClock) {
        let cell = Rc::new(Cell::new(0));
        (Clock::Manual(cell.clone()), ManualClock(cell))
    }

    /// Current time in nanoseconds.
    pub fn now_ns(&self) -> u64 {
        match self {
            Clock::Real(origin) => origin.elapsed().as_nanos() as u64,
            Clock::Manual(c) => c.get(),
        }
    }
}

/// The handle of a manual [`Clock`].
#[derive(Clone, Debug)]
pub struct ManualClock(Rc<Cell<u64>>);

impl ManualClock {
    /// Moves the clock forward.
    pub fn advance_ns(&self, ns: u64) {
        self.0.set(self.0.get() + ns);
    }

    /// Moves the clock forward by milliseconds.
    pub fn advance_ms(&self, ms: f64) {
        self.advance_ns((ms * 1e6).round() as u64);
    }
}

/// One frame's stage times (nanoseconds) and counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub frame: u64,
    pub stages: Vec<(String, u64)>,
    pub counters: Vec<(String, u64)>,
}

fn add_named(list: &mut Vec<(String, u64)>, name: &str, v: u64) {
    match list.iter_mut().find(|(n, _)| n == name) {
        Some((_, total)) => *total += v,
        None => list.push((name.to_string(), v)),
    }
}

/// A started stage, from [`FrameProbe::start`]. Pass it to [`FrameProbe::stop`].
#[must_use = "a started stage records nothing until it is stopped"]
#[derive(Debug)]
pub struct StageToken {
    name: String,
    start: u64,
}

/// Collects one frame's stage times and counters.
#[derive(Debug)]
pub struct FrameProbe {
    clock: Clock,
    sample: FrameSample,
}

impl FrameProbe {
    /// A probe for `frame` reading `clock`.
    pub fn new(clock: Clock, frame: u64) -> Self {
        FrameProbe { clock, sample: FrameSample { frame, ..Default::default() } }
    }

    /// Current time from the probe's clock, in nanoseconds.
    pub fn now_ns(&self) -> u64 {
        self.clock.now_ns()
    }

    /// Starts timing a stage.
    pub fn start(&self, name: &str) -> StageToken {
        StageToken { name: name.to_string(), start: self.clock.now_ns() }
    }

    /// Stops a stage and adds its time. A stage timed more than once in a frame accumulates.
    pub fn stop(&mut self, token: StageToken) {
        let elapsed = self.clock.now_ns().saturating_sub(token.start);
        add_named(&mut self.sample.stages, &token.name, elapsed);
    }

    /// Times a closure as a stage. The closure gets the probe back, so stages can nest.
    pub fn time<R>(&mut self, name: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        let token = self.start(name);
        let r = f(self);
        self.stop(token);
        r
    }

    /// Times until the returned guard is dropped. The guard derefs to the probe, so stages can nest:
    ///
    /// ```
    /// # use harness::perf::{Clock, FrameProbe};
    /// let mut probe = FrameProbe::new(Clock::real(), 0);
    /// {
    ///     let mut render = probe.scope("render");
    ///     let _cull = render.scope("render.cull");
    /// }
    /// assert!(probe.sample().stages.len() == 2);
    /// ```
    pub fn scope(&mut self, name: &str) -> ScopedStage<'_> {
        let token = Some(self.start(name));
        ScopedStage { probe: self, token }
    }

    /// Adds a time measured elsewhere (a GPU timer query, for instance).
    pub fn add_time_ns(&mut self, name: &str, ns: u64) {
        add_named(&mut self.sample.stages, name, ns);
    }

    /// Adds to a counter (draw calls, triangles, objects updated...).
    pub fn count(&mut self, name: &str, n: u64) {
        add_named(&mut self.sample.counters, name, n);
    }

    /// The numbers collected so far.
    pub fn sample(&self) -> &FrameSample {
        &self.sample
    }

    /// Ends the frame and returns its numbers.
    pub fn finish(self) -> FrameSample {
        self.sample
    }
}

/// A stage being timed until this guard is dropped. See [`FrameProbe::scope`].
pub struct ScopedStage<'a> {
    probe: &'a mut FrameProbe,
    token: Option<StageToken>,
}

impl std::ops::Deref for ScopedStage<'_> {
    type Target = FrameProbe;
    fn deref(&self) -> &FrameProbe {
        self.probe
    }
}

impl std::ops::DerefMut for ScopedStage<'_> {
    fn deref_mut(&mut self) -> &mut FrameProbe {
        self.probe
    }
}

impl Drop for ScopedStage<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            self.probe.stop(t);
        }
    }
}

/// Percentile of sorted data by linear interpolation between closest ranks (`p` in `[0, 100]`). This is the common
/// "type 7" definition (the default of spreadsheets and NumPy). Returns `NaN` for empty data.
pub fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    match sorted.len() {
        0 => f64::NAN,
        1 => sorted[0],
        n => {
            let rank = (p.clamp(0.0, 100.0) / 100.0) * (n - 1) as f64;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
        }
    }
}

/// Summary statistics of one series.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
    /// The frame with the largest value (the first one, on ties).
    pub worst_frame: u64,
}

impl Stats {
    /// Statistics of `(frame, value)` pairs. `None` when empty.
    pub fn from_samples(samples: &[(u64, f64)]) -> Option<Stats> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<f64> = samples.iter().map(|s| s.1).collect();
        sorted.sort_by(f64::total_cmp);
        let mut worst = samples[0];
        for s in samples {
            if s.1 > worst.1 {
                worst = *s;
            }
        }
        Some(Stats {
            count: samples.len(),
            mean: sorted.iter().sum::<f64>() / sorted.len() as f64,
            median: percentile_sorted(&sorted, 50.0),
            p95: percentile_sorted(&sorted, 95.0),
            p99: percentile_sorted(&sorted, 99.0),
            min: sorted[0],
            max: sorted[sorted.len() - 1],
            worst_frame: worst.0,
        })
    }

    /// A metric by name: `mean`, `median`, `p95`, `p99`, `min` or `max`.
    pub fn metric(&self, name: &str) -> Option<f64> {
        Some(match name {
            "mean" => self.mean,
            "median" => self.median,
            "p95" => self.p95,
            "p99" => self.p99,
            "min" => self.min,
            "max" => self.max,
            _ => return None,
        })
    }

    /// The statistics rounded to six decimals, so reports stay readable and survive a JSON round trip.
    fn rounded(self) -> Stats {
        Stats {
            mean: round6(self.mean),
            median: round6(self.median),
            p95: round6(self.p95),
            p99: round6(self.p99),
            min: round6(self.min),
            max: round6(self.max),
            ..self
        }
    }

    fn to_json(&self) -> Value {
        let n = |v: f64| Value::Number(v);
        Value::Object(vec![
            ("count".into(), n(self.count as f64)),
            ("mean".into(), n(self.mean)),
            ("median".into(), n(self.median)),
            ("p95".into(), n(self.p95)),
            ("p99".into(), n(self.p99)),
            ("min".into(), n(self.min)),
            ("max".into(), n(self.max)),
            ("worst_frame".into(), n(self.worst_frame as f64)),
        ])
    }

    fn from_json(v: &Value) -> Result<Stats, String> {
        let f = |k: &str| v.get(k).and_then(Value::as_f64).ok_or_else(|| format!("missing number `{k}`"));
        Ok(Stats {
            count: f("count")? as usize,
            mean: f("mean")?,
            median: f("median")?,
            p95: f("p95")?,
            p99: f("p99")?,
            min: f("min")?,
            max: f("max")?,
            worst_frame: f("worst_frame")? as u64,
        })
    }
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Keeps every frame's sample for a run.
#[derive(Clone, Debug, Default)]
pub struct PerfRecorder {
    samples: Vec<FrameSample>,
}

impl PerfRecorder {
    /// An empty recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a frame.
    pub fn push(&mut self, sample: FrameSample) {
        self.samples.push(sample);
    }

    /// Every frame recorded.
    pub fn samples(&self) -> &[FrameSample] {
        &self.samples
    }

    /// Summarises the run, leaving out frames before `warmup` (loading and first-frame costs).
    pub fn report(&self, app: &str, seed: u64, warmup: u64) -> PerfReport {
        let kept: Vec<&FrameSample> = self.samples.iter().filter(|s| s.frame >= warmup).collect();
        let series = |pick: fn(&FrameSample) -> &Vec<(String, u64)>, scale: f64| {
            let mut names: Vec<String> = Vec::new();
            for s in &kept {
                for (n, _) in pick(s) {
                    if !names.contains(n) {
                        names.push(n.clone());
                    }
                }
            }
            names
                .into_iter()
                .filter_map(|name| {
                    let data: Vec<(u64, f64)> = kept
                        .iter()
                        .filter_map(|s| {
                            pick(s).iter().find(|(n, _)| *n == name).map(|(_, v)| (s.frame, *v as f64 * scale))
                        })
                        .collect();
                    Stats::from_samples(&data).map(|st| (name, st.rounded()))
                })
                .collect::<Vec<_>>()
        };
        let frame_ms = kept
            .iter()
            .map(|s| s.stages.iter().find(|(n, _)| n == STAGE_FRAME).map_or(f64::NAN, |(_, v)| round6(*v as f64 / 1e6)))
            .collect();
        PerfReport {
            app: app.to_string(),
            seed,
            frames: self.samples.len() as u64,
            warmup,
            stages_ms: series(|s| &s.stages, 1e-6),
            counters: series(|s| &s.counters, 1.0),
            frame_ms,
        }
    }
}

/// A run's performance summary. Stage times are in milliseconds.
#[derive(Clone, Debug, PartialEq)]
pub struct PerfReport {
    pub app: String,
    pub seed: u64,
    /// Frames run, including warm-up.
    pub frames: u64,
    /// Frames left out of the statistics at the start.
    pub warmup: u64,
    pub stages_ms: Vec<(String, Stats)>,
    pub counters: Vec<(String, Stats)>,
    /// The `frame` stage of each frame after warm-up, in milliseconds.
    pub frame_ms: Vec<f64>,
}

/// The value of `format` in report JSON.
pub const REPORT_FORMAT: &str = "slynx-perf";
/// The report JSON version written by this harness.
pub const REPORT_VERSION: u32 = 1;

impl PerfReport {
    /// Statistics of a stage.
    pub fn stage(&self, name: &str) -> Option<&Stats> {
        self.stages_ms.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }

    /// Statistics of a counter.
    pub fn counter(&self, name: &str) -> Option<&Stats> {
        self.counters.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }

    /// The report as JSON.
    pub fn to_json(&self) -> String {
        let group =
            |list: &[(String, Stats)]| Value::Object(list.iter().map(|(n, s)| (n.clone(), s.to_json())).collect());
        Value::Object(vec![
            ("format".into(), Value::String(REPORT_FORMAT.into())),
            ("version".into(), Value::Number(REPORT_VERSION as f64)),
            ("app".into(), Value::String(self.app.clone())),
            // Seeds can exceed 2^53, so they are written as strings.
            ("seed".into(), Value::String(self.seed.to_string())),
            ("frames".into(), Value::Number(self.frames as f64)),
            ("warmup".into(), Value::Number(self.warmup as f64)),
            ("stages_ms".into(), group(&self.stages_ms)),
            ("counters".into(), group(&self.counters)),
            ("frame_ms".into(), Value::Array(self.frame_ms.iter().map(|v| Value::Number(*v)).collect())),
        ])
        .to_pretty()
    }

    /// Reads a report written by [`PerfReport::to_json`].
    pub fn from_json(text: &str) -> Result<PerfReport, String> {
        let v = Value::parse(text).map_err(|e: JsonError| e.to_string())?;
        if v.get("format").and_then(Value::as_str) != Some(REPORT_FORMAT) {
            return Err(format!("not a {REPORT_FORMAT} report"));
        }
        let version = v.get("version").and_then(Value::as_f64).unwrap_or(0.0);
        if version != REPORT_VERSION as f64 {
            return Err(format!("unsupported report version {version}"));
        }
        let group = |key: &str| -> Result<Vec<(String, Stats)>, String> {
            let obj = v.get(key).and_then(Value::as_object).ok_or_else(|| format!("missing `{key}`"))?;
            obj.iter()
                .map(|(n, s)| Stats::from_json(s).map(|st| (n.clone(), st)).map_err(|e| format!("{key}.{n}: {e}")))
                .collect()
        };
        Ok(PerfReport {
            app: v.get("app").and_then(Value::as_str).unwrap_or("").to_string(),
            seed: v.get("seed").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0),
            frames: v.get("frames").and_then(Value::as_f64).unwrap_or(0.0) as u64,
            warmup: v.get("warmup").and_then(Value::as_f64).unwrap_or(0.0) as u64,
            stages_ms: group("stages_ms")?,
            counters: group("counters")?,
            frame_ms: v
                .get("frame_ms")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
                .unwrap_or_default(),
        })
    }

    /// The report as a text table.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "app {}  seed {}  frames {} (statistics from frame {})",
            self.app, self.seed, self.frames, self.warmup
        );
        let header = |out: &mut String, title: &str| {
            let _ = writeln!(
                out,
                "{title:<24} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8}",
                "mean", "median", "p95", "p99", "worst", "@frame"
            );
        };
        let row = |out: &mut String, name: &str, s: &Stats, prec: usize| {
            let _ = writeln!(
                out,
                "{name:<24} {:>10.prec$} {:>10.prec$} {:>10.prec$} {:>10.prec$} {:>10.prec$} {:>8}",
                s.mean, s.median, s.p95, s.p99, s.max, s.worst_frame
            );
        };
        if !self.stages_ms.is_empty() {
            header(&mut out, "stage (ms)");
            for (n, s) in &self.stages_ms {
                row(&mut out, n, s, 3);
            }
        }
        if !self.counters.is_empty() {
            header(&mut out, "counter");
            for (n, s) in &self.counters {
                row(&mut out, n, s, 1);
            }
        }
        if let Some(f) = self.stage(STAGE_FRAME) {
            let over = self.frame_ms.iter().filter(|&&v| v > 1000.0 / 60.0).count();
            let _ = writeln!(
                out,
                "frames over the 16.667 ms budget: {over} of {}; median frame rate {:.1} fps",
                self.frame_ms.len(),
                if f.median > 0.0 { 1000.0 / f.median } else { f64::INFINITY }
            );
        }
        out
    }
}

/// Rules for [`compare_reports`].
#[derive(Clone, Debug, PartialEq)]
pub struct RegressionRules {
    /// A metric that grows by more than this percentage is a regression (shrinks by more: an improvement).
    pub threshold_pct: f64,
    /// Stage changes smaller than this (milliseconds) are noise and never flagged.
    pub min_delta_ms: f64,
    /// Which statistics to compare, by [`Stats::metric`] name.
    pub metrics: Vec<String>,
}

impl Default for RegressionRules {
    fn default() -> Self {
        RegressionRules { threshold_pct: 10.0, min_delta_ms: 0.05, metrics: vec!["median".into(), "p95".into()] }
    }
}

/// The verdict for one metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Regression,
    Improvement,
    /// In the baseline but not in the current report, or the other way round.
    Missing,
}

/// One compared metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// `stage` or `counter`.
    pub kind: &'static str,
    pub name: String,
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    /// Change relative to the baseline, in percent (`NaN` when the baseline is 0 or a side is missing).
    pub change_pct: f64,
    pub verdict: Verdict,
}

/// The result of comparing a report against a baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineComparison {
    pub findings: Vec<Finding>,
}

impl BaselineComparison {
    /// The regressions found.
    pub fn regressions(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.verdict == Verdict::Regression)
    }

    /// Whether anything regressed.
    pub fn has_regressions(&self) -> bool {
        self.regressions().next().is_some()
    }

    /// The comparison as a text table.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<8} {:<24} {:<7} {:>12} {:>12} {:>9}  verdict",
            "kind", "name", "metric", "baseline", "current", "change"
        );
        for f in &self.findings {
            let change = if f.change_pct.is_nan() { "n/a".to_string() } else { format!("{:+.1}%", f.change_pct) };
            let verdict = match f.verdict {
                Verdict::Ok => "ok",
                Verdict::Regression => "REGRESSION",
                Verdict::Improvement => "improved",
                Verdict::Missing => "missing",
            };
            let _ = writeln!(
                out,
                "{:<8} {:<24} {:<7} {:>12.3} {:>12.3} {:>9}  {verdict}",
                f.kind, f.name, f.metric, f.baseline, f.current, change
            );
        }
        let n = self.regressions().count();
        let _ = writeln!(out, "{}", if n == 0 { "no regressions".to_string() } else { format!("{n} regression(s)") });
        out
    }

    /// The comparison as JSON.
    pub fn to_json(&self) -> String {
        let findings = self
            .findings
            .iter()
            .map(|f| {
                Value::Object(vec![
                    ("kind".into(), Value::String(f.kind.into())),
                    ("name".into(), Value::String(f.name.clone())),
                    ("metric".into(), Value::String(f.metric.clone())),
                    ("baseline".into(), Value::Number(f.baseline)),
                    ("current".into(), Value::Number(f.current)),
                    ("change_pct".into(), Value::Number(f.change_pct)),
                    ("verdict".into(), Value::String(format!("{:?}", f.verdict).to_lowercase())),
                ])
            })
            .collect();
        Value::Object(vec![
            ("regressions".into(), Value::Number(self.regressions().count() as f64)),
            ("findings".into(), Value::Array(findings)),
        ])
        .to_pretty()
    }
}

/// Compares `current` against `baseline` stage by stage and counter by counter.
pub fn compare_reports(current: &PerfReport, baseline: &PerfReport, rules: &RegressionRules) -> BaselineComparison {
    let mut findings = Vec::new();
    let mut group = |kind: &'static str, cur: &[(String, Stats)], base: &[(String, Stats)], min_delta: f64| {
        for (name, b) in base {
            let c = cur.iter().find(|(n, _)| n == name).map(|(_, s)| s);
            for metric in &rules.metrics {
                let Some(bv) = b.metric(metric) else { continue };
                let Some(c) = c else {
                    findings.push(Finding {
                        kind,
                        name: name.clone(),
                        metric: metric.clone(),
                        baseline: bv,
                        current: f64::NAN,
                        change_pct: f64::NAN,
                        verdict: Verdict::Missing,
                    });
                    continue;
                };
                let cv = c.metric(metric).unwrap_or(f64::NAN);
                let delta = cv - bv;
                let change_pct = if bv != 0.0 { delta / bv * 100.0 } else { f64::NAN };
                let big = delta.abs() > min_delta;
                let verdict = if bv == 0.0 {
                    if cv > min_delta { Verdict::Regression } else { Verdict::Ok }
                } else if big && change_pct > rules.threshold_pct {
                    Verdict::Regression
                } else if big && change_pct < -rules.threshold_pct {
                    Verdict::Improvement
                } else {
                    Verdict::Ok
                };
                findings.push(Finding {
                    kind,
                    name: name.clone(),
                    metric: metric.clone(),
                    baseline: bv,
                    current: cv,
                    change_pct,
                    verdict,
                });
            }
        }
        for (name, _) in cur {
            if !base.iter().any(|(n, _)| n == name) {
                findings.push(Finding {
                    kind,
                    name: name.clone(),
                    metric: "-".into(),
                    baseline: f64::NAN,
                    current: f64::NAN,
                    change_pct: f64::NAN,
                    verdict: Verdict::Missing,
                });
            }
        }
    };
    group("stage", &current.stages_ms, &baseline.stages_ms, rules.min_delta_ms);
    group("counter", &current.counters, &baseline.counters, 0.0);
    BaselineComparison { findings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_match_known_values() {
        let data: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(percentile_sorted(&data, 0.0), 1.0);
        assert_eq!(percentile_sorted(&data, 100.0), 10.0);
        assert!((percentile_sorted(&data, 50.0) - 5.5).abs() < 1e-12);
        assert!((percentile_sorted(&data, 95.0) - 9.55).abs() < 1e-12);
        assert!((percentile_sorted(&data, 99.0) - 9.91).abs() < 1e-12);
        assert!((percentile_sorted(&data, 25.0) - 3.25).abs() < 1e-12);
        assert_eq!(percentile_sorted(&[7.0], 99.0), 7.0);
        assert!(percentile_sorted(&[], 50.0).is_nan());
        let odd = [1.0, 2.0, 100.0];
        assert_eq!(percentile_sorted(&odd, 50.0), 2.0);
    }

    #[test]
    fn stats_of_a_series() {
        // 1..=100 in a shuffled frame order; frame 37 holds the worst value.
        let mut samples: Vec<(u64, f64)> = (0..100).map(|i| (i, ((i * 37) % 100 + 1) as f64)).collect();
        samples.swap(0, 5);
        let s = Stats::from_samples(&samples).unwrap();
        assert_eq!(s.count, 100);
        assert!((s.mean - 50.5).abs() < 1e-12);
        assert!((s.median - 50.5).abs() < 1e-12);
        assert!((s.p95 - 95.05).abs() < 1e-9);
        assert!((s.p99 - 99.01).abs() < 1e-9);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 100.0);
        let worst = samples.iter().find(|x| x.1 == 100.0).unwrap().0;
        assert_eq!(s.worst_frame, worst);
        assert!(Stats::from_samples(&[]).is_none());
    }

    #[test]
    fn probe_times_nested_stages_with_manual_clock() {
        let (clock, hand) = Clock::manual();
        let mut p = FrameProbe::new(clock, 3);
        {
            let mut r = p.scope("render");
            hand.advance_ms(1.0);
            {
                let mut c = r.scope("render.cull");
                hand.advance_ms(0.5);
                c.count(DRAW_CALLS, 10);
            }
            r.count(DRAW_CALLS, 5);
        }
        p.time("update", |_| hand.advance_ms(2.0));
        let t = p.start("update");
        hand.advance_ms(1.0);
        p.stop(t);
        p.add_time_ns("gpu", 123);
        p.count(TRIANGLES, 7);
        let s = p.finish();
        assert_eq!(s.frame, 3);
        let get = |n: &str| s.stages.iter().find(|x| x.0 == n).unwrap().1;
        assert_eq!(get("render"), 1_500_000);
        assert_eq!(get("render.cull"), 500_000);
        assert_eq!(get("update"), 3_000_000);
        assert_eq!(get("gpu"), 123);
        assert_eq!(s.counters, vec![(DRAW_CALLS.to_string(), 15), (TRIANGLES.to_string(), 7)]);
    }

    fn synthetic_report(frame_ms: impl Fn(u64) -> f64, draws: u64) -> PerfReport {
        let mut rec = PerfRecorder::new();
        for f in 0..120 {
            let mut s = FrameSample { frame: f, ..Default::default() };
            s.stages.push((STAGE_FRAME.into(), (frame_ms(f) * 1e6) as u64));
            s.stages.push((STAGE_UPDATE.into(), 1_000_000));
            if f % 30 == 0 {
                s.stages.push((STAGE_CAPTURE.into(), 4_000_000));
            }
            s.counters.push((DRAW_CALLS.into(), draws));
            rec.push(s);
        }
        rec.report("stub", 9, 10)
    }

    #[test]
    fn report_warmup_and_json_round_trip() {
        let r = synthetic_report(|f| if f < 10 { 100.0 } else { 5.0 + (f % 3) as f64 }, 40);
        assert_eq!(r.frames, 120);
        assert_eq!(r.frame_ms.len(), 110);
        let frame = r.stage(STAGE_FRAME).unwrap();
        assert_eq!(frame.max, 7.0, "warm-up frames are left out");
        assert_eq!(r.stage(STAGE_CAPTURE).unwrap().count, 3);
        assert_eq!(r.counter(DRAW_CALLS).unwrap().median, 40.0);
        let back = PerfReport::from_json(&r.to_json()).unwrap();
        assert_eq!(back, r);
        assert!(r.to_text().contains("frames over the 16.667 ms budget: 0 of 110"));
        assert!(PerfReport::from_json("{\"format\":\"other\"}").is_err());
    }

    #[test]
    fn baseline_flags_regressions_and_improvements() {
        let base = synthetic_report(|_| 10.0, 100);
        let same = compare_reports(&base, &base, &RegressionRules::default());
        assert!(!same.has_regressions());
        assert!(same.findings.iter().all(|f| f.verdict == Verdict::Ok));

        let slower = synthetic_report(|_| 11.5, 100);
        let c = compare_reports(&slower, &base, &RegressionRules::default());
        let regs: Vec<_> = c.regressions().map(|f| (f.name.as_str(), f.metric.as_str())).collect();
        assert_eq!(regs, vec![("frame", "median"), ("frame", "p95")]);
        assert!(c.to_text().contains("2 regression(s)"));
        assert!(Value::parse(&c.to_json()).is_ok());

        let slightly = synthetic_report(|_| 10.9, 100);
        assert!(!compare_reports(&slightly, &base, &RegressionRules::default()).has_regressions());

        let faster_more_draws = synthetic_report(|_| 8.0, 120);
        let c = compare_reports(&faster_more_draws, &base, &RegressionRules::default());
        assert!(c.findings.iter().any(|f| f.name == "frame" && f.verdict == Verdict::Improvement));
        assert!(c.regressions().all(|f| f.kind == "counter" && f.name == DRAW_CALLS));
        assert!(c.has_regressions());
    }

    #[test]
    fn tiny_stage_changes_are_noise() {
        let mut base = synthetic_report(|_| 0.010, 1);
        let cur = synthetic_report(|_| 0.020, 1);
        assert!(!compare_reports(&cur, &base, &RegressionRules::default()).has_regressions());
        base.stages_ms.retain(|(n, _)| n != STAGE_UPDATE);
        let c = compare_reports(&cur, &base, &RegressionRules::default());
        assert!(c.findings.iter().any(|f| f.name == STAGE_UPDATE && f.verdict == Verdict::Missing));
    }
}
