//! The `harness` command-line tool.
//!
//! ```text
//! harness run --app <name> --frames N [--capture K[,K...]]... --out DIR [options]
//! harness compare A.png B.png [--tolerance D] [--max-diff-pixels N] [--min-psnr DB] [--ignore-alpha] [--diff OUT.png] [--json]
//! harness perf REPORT.json [--baseline BASE.json] [--threshold PCT] [--min-delta-ms MS] [--metrics m1,m2] [--json]
//! harness list
//! ```
//!
//! Exit codes: `0` success (images match, no regression), `1` a check failed (images differ, regression, run not
//! deterministic), `2` usage or I/O error.
//!
//! A binary that links the game calls [`main_with`] with a registry holding the game's apps, so the same commands
//! drive the real game.

use std::io::Write;
use std::path::PathBuf;

use crate::app::AppConfig;
use crate::apps::AppRegistry;
use crate::compare::{Tolerance, compare_images, diff_image};
use crate::input::InputScript;
use crate::perf::{PerfReport, RegressionRules, compare_reports};
use crate::png;
use crate::runner::{RenderMode, RunConfig, capture_file_name, first_divergence, run};

/// Exit code: everything passed.
pub const EXIT_OK: i32 = 0;
/// Exit code: a check failed.
pub const EXIT_FAIL: i32 = 1;
/// Exit code: bad usage or an I/O error.
pub const EXIT_ERROR: i32 = 2;

const USAGE: &str = "\
usage:
  harness run --app NAME --out DIR [--frames N] [--capture K[,K...]]... [--seed S] [--input SCRIPT]
              [--width W] [--height H] [--warmup N] [--opt KEY=VALUE]... [--captures-only] [--check-determinism]
  harness compare A.png B.png [--tolerance D] [--max-diff-pixels N] [--min-psnr DB] [--ignore-alpha]
              [--diff OUT.png] [--json]
  harness perf REPORT.json [--baseline BASE.json] [--threshold PCT] [--min-delta-ms MS] [--metrics m1,m2] [--json]
  harness list

exit codes: 0 ok, 1 check failed (images differ, regression, not deterministic), 2 usage or I/O error
";

/// Runs the tool with the process's arguments and the given apps; returns the exit code.
pub fn main_with(registry: &AppRegistry) -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    run_cli(&args, registry, &mut stdout.lock(), &mut stderr.lock())
}

/// Runs the tool with explicit arguments (without the program name) and output streams.
pub fn run_cli(args: &[String], registry: &AppRegistry, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let result = match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..], registry, out),
        Some("compare") => cmd_compare(&args[1..], out),
        Some("perf") => cmd_perf(&args[1..], out),
        Some("list") => {
            for (name, desc) in registry.list() {
                let _ = writeln!(out, "{name:<16} {desc}");
            }
            Ok(EXIT_OK)
        }
        Some("help" | "-h" | "--help") => {
            let _ = write!(out, "{USAGE}");
            Ok(EXIT_OK)
        }
        Some(other) => Err(format!("unknown command `{other}`\n{USAGE}")),
        None => Err(USAGE.to_string()),
    };
    match result {
        Ok(code) => code,
        Err(msg) => {
            let _ = writeln!(err, "harness: {msg}");
            EXIT_ERROR
        }
    }
}

/// Parsed arguments: positionals and `--name value` / `--name=value` / `--flag` options, in order.
struct Args {
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(args: &[String], flags: &[&str], valued: &[&str]) -> Result<Args, String> {
        let mut positional = Vec::new();
        let mut options = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let (name, inline) = match name.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (name, None),
                };
                if flags.contains(&name) {
                    if inline.is_some() {
                        return Err(format!("--{name} takes no value"));
                    }
                    options.push((name.to_string(), None));
                } else if valued.contains(&name) {
                    let v = match inline {
                        Some(v) => v,
                        None => it.next().cloned().ok_or_else(|| format!("--{name} needs a value"))?,
                    };
                    options.push((name.to_string(), Some(v)));
                } else {
                    return Err(format!("unknown option --{name}"));
                }
            } else {
                positional.push(a.clone());
            }
        }
        Ok(Args { positional, options })
    }

    fn flag(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }

    fn all(&self, name: &str) -> impl Iterator<Item = &str> {
        self.options.iter().filter(move |(n, _)| n == name).filter_map(|(_, v)| v.as_deref())
    }

    fn last(&self, name: &str) -> Option<&str> {
        self.all(name).last()
    }

    fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.last(name) {
            Some(v) => v.parse().map_err(|_| format!("--{name}: bad value `{v}`")),
            None => Ok(default),
        }
    }
}

fn cmd_run(raw: &[String], registry: &AppRegistry, out: &mut dyn Write) -> Result<i32, String> {
    let a = Args::parse(
        raw,
        &["captures-only", "check-determinism"],
        &["app", "frames", "capture", "out", "seed", "input", "width", "height", "warmup", "opt"],
    )?;
    if !a.positional.is_empty() {
        return Err(format!("run: unexpected argument `{}`", a.positional[0]));
    }
    let app_name = a.last("app").ok_or("run: --app is required")?;
    let out_dir = PathBuf::from(a.last("out").ok_or("run: --out is required")?);
    let frames: u64 = a.number("frames", 60)?;
    let mut captures = Vec::new();
    for list in a.all("capture") {
        for k in list.split(',').filter(|s| !s.is_empty()) {
            captures.push(k.trim().parse::<u64>().map_err(|_| format!("--capture: bad frame `{k}`"))?);
        }
    }
    captures.sort_unstable();
    captures.dedup();
    let script = match a.last("input") {
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            InputScript::parse(&text).map_err(|e| format!("{p}: {e}"))?
        }
        None => InputScript::empty(),
    };
    let mut options = Vec::new();
    for kv in a.all("opt") {
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("--opt: expected KEY=VALUE, got `{kv}`"))?;
        options.push((k.to_string(), v.to_string()));
    }
    let seed: u64 = a.number("seed", 0)?;
    let app_config = AppConfig {
        width: a.number("width", AppConfig::default().width)?,
        height: a.number("height", AppConfig::default().height)?,
        seed,
        headless: true,
        options,
    };
    let config = RunConfig {
        frames,
        captures,
        seed,
        script,
        warmup: a.number("warmup", 0)?,
        render: if a.flag("captures-only") { RenderMode::CaptureFramesOnly } else { RenderMode::EveryFrame },
        ..Default::default()
    };

    let mut app = registry.create(app_name, &app_config).map_err(|e| e.to_string())?;
    let result = run(app.as_mut(), &config).map_err(|e| e.to_string())?;
    drop(app);
    result.write_to(&out_dir, app_name)?;
    let _ = writeln!(out, "ran {app_name} for {frames} frames (seed {seed}); outputs in {}", out_dir.display());
    for (f, _) in &result.captures {
        let _ = writeln!(out, "captured {}", out_dir.join(capture_file_name(*f)).display());
    }
    let _ = writeln!(out, "fingerprint {:016x}", result.fingerprint());
    let _ = write!(out, "{}", result.perf_report(app_name).to_text());

    if a.flag("check-determinism") {
        let mut again = registry.create(app_name, &app_config).map_err(|e| e.to_string())?;
        let second = run(again.as_mut(), &config).map_err(|e| e.to_string())?;
        if let Some(d) = first_divergence(&result, &second) {
            let _ = writeln!(out, "NOT DETERMINISTIC: {d}");
            return Ok(EXIT_FAIL);
        }
        let _ = writeln!(out, "deterministic: a second run matched");
    }
    Ok(EXIT_OK)
}

fn cmd_compare(raw: &[String], out: &mut dyn Write) -> Result<i32, String> {
    let a = Args::parse(raw, &["ignore-alpha", "json"], &["tolerance", "max-diff-pixels", "min-psnr", "diff"])?;
    let [pa, pb] = a.positional.as_slice() else {
        return Err("compare: expected two PNG files".into());
    };
    let ia = png::read_file(pa).map_err(|e| format!("{pa}: {e}"))?;
    let ib = png::read_file(pb).map_err(|e| format!("{pb}: {e}"))?;
    let tol = Tolerance {
        max_channel_delta: a.number("tolerance", 0)?,
        max_diff_pixels: a.number("max-diff-pixels", 0)?,
        min_psnr: a
            .last("min-psnr")
            .map(|v| v.parse().map_err(|_| format!("--min-psnr: bad value `{v}`")))
            .transpose()?,
        ignore_alpha: a.flag("ignore-alpha"),
    };
    let report = compare_images(&ia, &ib, &tol);
    if let Some(d) = a.last("diff") {
        png::write_file(d, &diff_image(&ia, &ib, &tol)).map_err(|e| format!("{d}: {e}"))?;
    }
    let _ = write!(out, "{}", if a.flag("json") { report.to_json() } else { report.to_text() });
    Ok(if report.passed { EXIT_OK } else { EXIT_FAIL })
}

fn cmd_perf(raw: &[String], out: &mut dyn Write) -> Result<i32, String> {
    let a = Args::parse(raw, &["json"], &["baseline", "threshold", "min-delta-ms", "metrics"])?;
    let [path] = a.positional.as_slice() else {
        return Err("perf: expected one report file".into());
    };
    let load = |p: &str| -> Result<PerfReport, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        PerfReport::from_json(&text).map_err(|e| format!("{p}: {e}"))
    };
    let report = load(path)?;
    let Some(base_path) = a.last("baseline") else {
        let _ = write!(out, "{}", if a.flag("json") { report.to_json() } else { report.to_text() });
        return Ok(EXIT_OK);
    };
    let baseline = load(base_path)?;
    let defaults = RegressionRules::default();
    let rules = RegressionRules {
        threshold_pct: a.number("threshold", defaults.threshold_pct)?,
        min_delta_ms: a.number("min-delta-ms", defaults.min_delta_ms)?,
        metrics: match a.last("metrics") {
            Some(m) => m.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            None => defaults.metrics,
        },
    };
    for m in &rules.metrics {
        if !["mean", "median", "p95", "p99", "min", "max"].contains(&m.as_str()) {
            return Err(format!("--metrics: unknown metric `{m}`"));
        }
    }
    let cmp = compare_reports(&report, &baseline, &rules);
    if a.flag("json") {
        let _ = write!(out, "{}", cmp.to_json());
    } else {
        let _ = write!(out, "{}", report.to_text());
        let _ = writeln!(out, "\nagainst baseline {base_path} (threshold {}%):", rules.threshold_pct);
        let _ = write!(out, "{}", cmp.to_text());
    }
    Ok(if cmp.has_regressions() { EXIT_FAIL } else { EXIT_OK })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use std::path::Path;

    fn cli(args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = run_cli(&args, &AppRegistry::builtin(), &mut o, &mut e);
        (code, String::from_utf8(o).unwrap(), String::from_utf8(e).unwrap())
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("slynx-harness-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn s(p: &Path) -> &str {
        p.to_str().unwrap()
    }

    #[test]
    fn usage_errors() {
        assert_eq!(cli(&[]).0, EXIT_ERROR);
        assert_eq!(cli(&["frobnicate"]).0, EXIT_ERROR);
        assert_eq!(cli(&["help"]).0, EXIT_OK);
        let (code, out, _) = cli(&["list"]);
        assert_eq!(code, EXIT_OK);
        assert!(out.contains("pattern"));
        assert_eq!(cli(&["run", "--frames", "3"]).0, EXIT_ERROR, "missing --app");
        assert_eq!(cli(&["run", "--app", "pattern", "--out", "x", "--bogus"]).0, EXIT_ERROR);
        assert_eq!(cli(&["run", "--app", "pattern", "--out"]).0, EXIT_ERROR);
        assert_eq!(cli(&["compare", "one.png"]).0, EXIT_ERROR);
        let (code, _, err) = cli(&["run", "--app", "nope", "--out", "x"]);
        assert_eq!(code, EXIT_ERROR);
        assert!(err.contains("unknown app"));
    }

    #[test]
    fn run_compare_and_perf_end_to_end() {
        let dir = temp("e2e");
        let script = dir.join("input.txt");
        std::fs::write(&script, "0..30 hold right\n5 tap a\n").unwrap();
        let run_args = |out: &Path, seed: &str| {
            vec![
                "run".to_string(),
                "--app".into(),
                "pattern".into(),
                "--frames=40".into(),
                "--capture".into(),
                "10,39".into(),
                "--capture".into(),
                "20".into(),
                "--out".into(),
                out.to_str().unwrap().into(),
                "--seed".into(),
                seed.into(),
                "--input".into(),
                script.to_str().unwrap().into(),
                "--width".into(),
                "40".into(),
                "--height".into(),
                "24".into(),
                "--opt".into(),
                "work=10".into(),
                "--check-determinism".into(),
            ]
        };
        let a = dir.join("a");
        let b = dir.join("b");
        let c = dir.join("c");
        let run = |args: Vec<String>| {
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let code = run_cli(&args, &AppRegistry::builtin(), &mut o, &mut e);
            (code, String::from_utf8(o).unwrap(), String::from_utf8(e).unwrap())
        };
        let (code, out, err) = run(run_args(&a, "7"));
        assert_eq!(code, EXIT_OK, "{out}{err}");
        assert!(out.contains("deterministic: a second run matched"));
        for f in [10, 20, 39] {
            assert!(a.join(capture_file_name(f)).exists());
        }
        assert_eq!(run(run_args(&b, "7")).0, EXIT_OK);
        assert_eq!(run(run_args(&c, "8")).0, EXIT_OK);

        let fa = a.join("frame_000039.png");
        let fb = b.join("frame_000039.png");
        let fc = c.join("frame_000039.png");
        let (code, out, _) = cli(&["compare", s(&fa), s(&fb)]);
        assert_eq!(code, EXIT_OK, "{out}");
        assert!(out.contains("PASS"));
        let diff = dir.join("diff.png");
        let (code, out, _) = cli(&["compare", s(&fa), s(&fc), "--diff", s(&diff), "--json"]);
        assert_eq!(code, EXIT_FAIL, "different seeds give different sparkles: {out}");
        assert!(out.contains("\"passed\": false"));
        assert_eq!(png::read_file(&diff).unwrap().width(), 40);
        let (code, _, _) = cli(&["compare", s(&fa), s(&fc), "--tolerance", "255", "--ignore-alpha"]);
        assert_eq!(code, EXIT_OK);
        let (code, _, err) = cli(&["compare", s(&fa), s(&dir.join("missing.png"))]);
        assert_eq!(code, EXIT_ERROR);
        assert!(err.contains("missing.png"));

        let ra = a.join("perf.json");
        let (code, out, _) = cli(&["perf", s(&ra)]);
        assert_eq!(code, EXIT_OK);
        assert!(out.contains("frame"));
        // A baseline that claims everything used to be much faster makes the report a regression.
        let mut fast = PerfReport::from_json(&std::fs::read_to_string(&ra).unwrap()).unwrap();
        for (_, st) in &mut fast.stages_ms {
            for v in [&mut st.mean, &mut st.median, &mut st.p95, &mut st.p99, &mut st.min, &mut st.max] {
                *v = 0.0;
            }
        }
        let base = dir.join("base.json");
        std::fs::write(&base, fast.to_json()).unwrap();
        let (code, out, _) = cli(&["perf", s(&ra), "--baseline", s(&base), "--min-delta-ms", "-1"]);
        assert_eq!(code, EXIT_FAIL, "{out}");
        assert!(out.contains("REGRESSION"));
        let (code, _, _) = cli(&["perf", s(&ra), "--baseline", s(&ra), "--json"]);
        assert_eq!(code, EXIT_OK);
        assert_eq!(cli(&["perf", s(&ra), "--baseline", s(&ra), "--metrics", "p50"]).0, EXIT_ERROR);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compare_reads_foreign_pngs() {
        // An RGBA image the decoder must handle like any other.
        let dir = temp("foreign");
        let mut img = Image::new(3, 2, [1, 2, 3, 4]);
        img.set(2, 1, [9, 9, 9, 9]);
        let p = dir.join("x.png");
        png::write_file(&p, &img).unwrap();
        assert_eq!(cli(&["compare", s(&p), s(&p)]).0, EXIT_OK);
        let _ = std::fs::remove_dir_all(dir);
    }
}
