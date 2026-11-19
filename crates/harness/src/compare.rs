//! Image comparison: exact or with tolerances, with a diff image.
//!
//! A pixel *differs* when any compared channel differs by more than [`Tolerance::max_channel_delta`]. Two images
//! *match* when they have the same size, at most [`Tolerance::max_diff_pixels`] pixels differ, and (if set) the
//! PSNR is at least [`Tolerance::min_psnr`]. PSNR is computed over the compared channels with a peak of 255:
//! `10 * log10(255^2 / MSE)`, infinite for identical images.

use std::fmt::Write as _;

use crate::image::Image;
use crate::json::Value;

/// How close two images must be to match.
#[derive(Clone, Debug, PartialEq)]
pub struct Tolerance {
    /// Largest per-channel difference that still counts as equal.
    pub max_channel_delta: u8,
    /// How many pixels may differ (beyond `max_channel_delta`) and the images still match.
    pub max_diff_pixels: u64,
    /// Lowest acceptable PSNR in dB, if any.
    pub min_psnr: Option<f64>,
    /// Leave the alpha channel out of every measure.
    pub ignore_alpha: bool,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance::exact()
    }
}

impl Tolerance {
    /// Every channel of every pixel must be equal.
    pub fn exact() -> Self {
        Tolerance { max_channel_delta: 0, max_diff_pixels: 0, min_psnr: None, ignore_alpha: false }
    }

    /// A tolerance suited to comparing renders from different GPUs or drivers: channels within 2, up to 0.1 % of
    /// pixels different, PSNR at least 40 dB, alpha ignored.
    pub fn gpu_lenient(width: u32, height: u32) -> Self {
        Tolerance {
            max_channel_delta: 2,
            max_diff_pixels: (width as u64 * height as u64) / 1000,
            min_psnr: Some(40.0),
            ignore_alpha: true,
        }
    }
}

/// The result of [`compare_images`].
#[derive(Clone, Debug, PartialEq)]
pub struct CompareReport {
    /// Size of the first (reference) image.
    pub size_a: (u32, u32),
    /// Size of the second (actual) image.
    pub size_b: (u32, u32),
    /// Largest difference per channel (R, G, B, A); alpha is 0 when ignored.
    pub max_delta: [u8; 4],
    /// Pixels where any compared channel differs at all.
    pub changed_pixels: u64,
    /// Pixels where any compared channel differs by more than the tolerance.
    pub diff_pixels: u64,
    /// Mean squared error over compared channels.
    pub mse: f64,
    /// Peak signal-to-noise ratio in dB (infinite when identical).
    pub psnr: f64,
    /// The first differing pixel (beyond the tolerance), in reading order.
    pub first_diff: Option<(u32, u32)>,
    /// Whether the images match under the tolerance.
    pub passed: bool,
}

impl CompareReport {
    /// Whether the sizes differ (then nothing else is measured).
    pub fn size_mismatch(&self) -> bool {
        self.size_a != self.size_b
    }

    /// Whether the images are bit-for-bit identical in the compared channels.
    pub fn identical(&self) -> bool {
        !self.size_mismatch() && self.changed_pixels == 0
    }

    /// One-paragraph text summary.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if self.size_mismatch() {
            let _ = writeln!(
                out,
                "size mismatch: {}x{} vs {}x{}\nFAIL",
                self.size_a.0, self.size_a.1, self.size_b.0, self.size_b.1
            );
            return out;
        }
        let total = self.size_a.0 as u64 * self.size_a.1 as u64;
        let psnr = if self.psnr.is_infinite() { "inf".to_string() } else { format!("{:.2} dB", self.psnr) };
        let _ = writeln!(out, "size {}x{} ({} pixels)", self.size_a.0, self.size_a.1, total);
        let _ = writeln!(out, "changed pixels: {} ({:.4} %)", self.changed_pixels, pct(self.changed_pixels, total));
        let _ = writeln!(out, "pixels beyond tolerance: {} ({:.4} %)", self.diff_pixels, pct(self.diff_pixels, total));
        let d = self.max_delta;
        let _ = writeln!(out, "max channel delta: R {} G {} B {} A {}", d[0], d[1], d[2], d[3]);
        let _ = writeln!(out, "MSE {:.4}, PSNR {psnr}", self.mse);
        if let Some((x, y)) = self.first_diff {
            let _ = writeln!(out, "first difference at ({x}, {y})");
        }
        let _ = writeln!(out, "{}", if self.passed { "PASS" } else { "FAIL" });
        out
    }

    /// The report as JSON. An infinite PSNR is written as `null`.
    pub fn to_json(&self) -> String {
        let n = |v: f64| Value::Number(v);
        let size = |s: (u32, u32)| Value::Array(vec![n(s.0 as f64), n(s.1 as f64)]);
        Value::Object(vec![
            ("passed".into(), Value::Bool(self.passed)),
            ("size_a".into(), size(self.size_a)),
            ("size_b".into(), size(self.size_b)),
            ("max_delta".into(), Value::Array(self.max_delta.iter().map(|&v| n(v as f64)).collect())),
            ("changed_pixels".into(), n(self.changed_pixels as f64)),
            ("diff_pixels".into(), n(self.diff_pixels as f64)),
            ("mse".into(), n(self.mse)),
            ("psnr".into(), n(self.psnr)),
            ("first_diff".into(), self.first_diff.map_or(Value::Null, size)),
        ])
        .to_pretty()
    }
}

fn pct(n: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { n as f64 * 100.0 / total as f64 }
}

/// Compares `a` (the reference) with `b` (the actual image).
pub fn compare_images(a: &Image, b: &Image, tol: &Tolerance) -> CompareReport {
    let size_a = (a.width(), a.height());
    let size_b = (b.width(), b.height());
    if size_a != size_b {
        return CompareReport {
            size_a,
            size_b,
            max_delta: [0; 4],
            changed_pixels: 0,
            diff_pixels: 0,
            mse: f64::NAN,
            psnr: f64::NAN,
            first_diff: None,
            passed: false,
        };
    }
    let channels = if tol.ignore_alpha { 3 } else { 4 };
    let mut max_delta = [0u8; 4];
    let mut changed = 0u64;
    let mut diff = 0u64;
    let mut sq_sum = 0u64;
    let mut first_diff = None;
    for (i, (pa, pb)) in a.pixels().as_chunks::<4>().0.iter().zip(b.pixels().as_chunks::<4>().0).enumerate() {
        let mut worst = 0u8;
        for c in 0..channels {
            let d = pa[c].abs_diff(pb[c]);
            max_delta[c] = max_delta[c].max(d);
            worst = worst.max(d);
            sq_sum += d as u64 * d as u64;
        }
        if worst > 0 {
            changed += 1;
        }
        if worst > tol.max_channel_delta {
            diff += 1;
            if first_diff.is_none() {
                first_diff = Some((i as u32 % a.width(), i as u32 / a.width()));
            }
        }
    }
    let samples = a.width() as u64 * a.height() as u64 * channels as u64;
    let mse = if samples == 0 { 0.0 } else { sq_sum as f64 / samples as f64 };
    let psnr = if mse == 0.0 { f64::INFINITY } else { 10.0 * (255.0f64 * 255.0 / mse).log10() };
    let passed = diff <= tol.max_diff_pixels && tol.min_psnr.is_none_or(|min| psnr >= min);
    CompareReport {
        size_a,
        size_b,
        max_delta,
        changed_pixels: changed,
        diff_pixels: diff,
        mse,
        psnr,
        first_diff,
        passed,
    }
}

/// A diff image: the reference dimmed to grey, with pixels beyond the tolerance in red (brighter for larger
/// differences) and pixels that changed but are within the tolerance in yellow. Sizes that differ give the
/// overlapping area, with the rest in magenta.
pub fn diff_image(a: &Image, b: &Image, tol: &Tolerance) -> Image {
    let w = a.width().max(b.width());
    let h = a.height().max(b.height());
    let mut out = Image::new(w, h, [255, 0, 255, 255]);
    let channels = if tol.ignore_alpha { 3 } else { 4 };
    for y in 0..a.height().min(b.height()) {
        for x in 0..a.width().min(b.width()) {
            let (pa, pb) = (a.get(x, y), b.get(x, y));
            let worst = (0..channels).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
            let c = if worst > tol.max_channel_delta {
                [(128 + worst as u16 / 2).min(255) as u8, 0, 0, 255]
            } else if worst > 0 {
                [200, 200, 0, 255]
            } else {
                let luma = (pa[0] as u32 * 77 + pa[1] as u32 * 150 + pa[2] as u32 * 29) >> 8;
                let g = (luma / 4 + 16) as u8;
                [g, g, g, 255]
            };
            out.set(x, y, c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                img.set(x, y, [(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128, 255]);
            }
        }
        img
    }

    #[test]
    fn identical_images() {
        let a = gradient(32, 16);
        let r = compare_images(&a, &a.clone(), &Tolerance::exact());
        assert!(r.passed && r.identical());
        assert_eq!(r.diff_pixels, 0);
        assert!(r.psnr.is_infinite());
        assert_eq!(r.first_diff, None);
        assert!(r.to_text().contains("PSNR inf"));
        assert!(Value::parse(&r.to_json()).unwrap().get("psnr") == Some(&Value::Null));
    }

    #[test]
    fn single_pixel_change() {
        let a = gradient(10, 10);
        let mut b = a.clone();
        let mut p = b.get(3, 4);
        p[1] = p[1].wrapping_add(10);
        b.set(3, 4, p);
        let r = compare_images(&a, &b, &Tolerance::exact());
        assert!(!r.passed);
        assert_eq!(r.changed_pixels, 1);
        assert_eq!(r.diff_pixels, 1);
        assert_eq!(r.max_delta, [0, 10, 0, 0]);
        assert_eq!(r.first_diff, Some((3, 4)));
        // MSE = 10^2 / (100 pixels * 4 channels) = 0.25; PSNR = 10 log10(65025 / 0.25).
        assert!((r.mse - 0.25).abs() < 1e-12);
        assert!((r.psnr - 10.0 * (65025.0f64 / 0.25).log10()).abs() < 1e-9);

        let one_allowed = Tolerance { max_diff_pixels: 1, ..Tolerance::exact() };
        assert!(compare_images(&a, &b, &one_allowed).passed);
        let delta_allowed = Tolerance { max_channel_delta: 10, ..Tolerance::exact() };
        let r = compare_images(&a, &b, &delta_allowed);
        assert!(r.passed && r.diff_pixels == 0 && r.changed_pixels == 1 && !r.identical());
        let psnr_strict = Tolerance { max_channel_delta: 255, min_psnr: Some(60.0), ..Tolerance::exact() };
        assert!(!compare_images(&a, &b, &psnr_strict).passed);
    }

    #[test]
    fn noise_counts_and_psnr() {
        let a = gradient(64, 64);
        let mut b = a.clone();
        let mut rng = crate::rng::Rng::new(11);
        let mut expected = 0;
        for y in 0..64 {
            for x in 0..64 {
                if rng.below(10) == 0 {
                    let mut p = b.get(x, y);
                    p[0] = p[0].saturating_add(3).min(250);
                    if p != a.get(x, y) {
                        expected += 1;
                    }
                    b.set(x, y, p);
                }
            }
        }
        let r = compare_images(&a, &b, &Tolerance::exact());
        assert_eq!(r.changed_pixels, expected);
        assert!(r.max_delta[0] <= 3 && r.max_delta[1] == 0);
        assert!(r.psnr > 40.0 && r.psnr.is_finite());
        let lenient = Tolerance { max_channel_delta: 3, ..Tolerance::exact() };
        assert!(compare_images(&a, &b, &lenient).passed);
    }

    #[test]
    fn alpha_can_be_ignored() {
        let a = Image::new(4, 4, [10, 20, 30, 255]);
        let b = Image::new(4, 4, [10, 20, 30, 0]);
        assert!(!compare_images(&a, &b, &Tolerance::exact()).passed);
        let r = compare_images(&a, &b, &Tolerance { ignore_alpha: true, ..Tolerance::exact() });
        assert!(r.passed && r.identical() && r.max_delta == [0; 4]);
    }

    #[test]
    fn size_mismatch_fails() {
        let r = compare_images(&gradient(4, 4), &gradient(4, 5), &Tolerance::gpu_lenient(4, 4));
        assert!(!r.passed && r.size_mismatch());
        assert!(r.to_text().contains("size mismatch"));
        let d = diff_image(&gradient(4, 4), &gradient(4, 5), &Tolerance::exact());
        assert_eq!((d.width(), d.height()), (4, 5));
        assert_eq!(d.get(0, 4), [255, 0, 255, 255]);
    }

    #[test]
    fn diff_image_marks_pixels() {
        let a = gradient(8, 8);
        let mut b = a.clone();
        b.set(1, 1, [255, 255, 255, 255]);
        let mut p = b.get(2, 2);
        p[2] += 1;
        b.set(2, 2, p);
        let d = diff_image(&a, &b, &Tolerance { max_channel_delta: 1, ..Tolerance::exact() });
        assert_eq!(d.get(1, 1)[1], 0);
        assert!(d.get(1, 1)[0] >= 128);
        assert_eq!(d.get(2, 2), [200, 200, 0, 255]);
        let g = d.get(5, 5);
        assert!(g[0] == g[1] && g[1] == g[2] && g[0] < 100);
    }
}
