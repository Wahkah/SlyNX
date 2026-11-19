//! The binocucom overlay (hud.md 7): data model and a drawing of our own design (the spec leaves the layout open).
//!
//! The game supplies the look direction, the zoom, the candidate targets (scan points, class 130, and objects of
//! interest) and a projection from world to canvas; the model keeps the markers (refreshed every 0.25 s, within
//! 4000 units, up to 32), the filter mode, and the radio text typing out with a white `|` cursor.

use alloc::vec::Vec;

use crate::draw::{DrawList, Rect, WHITE, rgba8};
use crate::font::FontSet;
use crate::hud::Slide;
use crate::text::{Align, TextStyle, Typewriter, layout};

/// Markers only for targets this close to the camera.
pub const MARKER_RANGE: f32 = 4000.0;
/// Most markers at once.
pub const MAX_MARKERS: usize = 32;
/// Seconds between marker refreshes.
pub const REFRESH: f32 = 0.25;
/// Radio text speed, characters per second.
pub const RADIO_RATE: f32 = 30.0;

/// Something the binocucom can mark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// A game-defined kind (for the filter mode).
    pub kind: u16,
    /// World position.
    pub position: [f32; 3],
    /// A scan point (always marked) rather than an object of interest.
    pub scan_point: bool,
}

/// A marker on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    /// Canvas position.
    pub at: (f32, f32),
    /// The target's kind.
    pub kind: u16,
    /// Distance from the camera.
    pub distance: f32,
    /// Highlighted by the filter.
    pub highlighted: bool,
    /// A scan point.
    pub scan_point: bool,
}

/// The binocucom view.
#[derive(Clone, Debug)]
pub struct Binocucom {
    /// Slide (fade) in and out.
    pub slide: Slide,
    /// Heading in degrees (0 = north, clockwise), for the compass.
    pub heading: f32,
    /// Zoom factor.
    pub zoom: f32,
    /// Reticle offset from the centre, canvas units (follows the look motion).
    pub reticle: (f32, f32),
    /// Highlight only this kind.
    pub filter: Option<u16>,
    /// Current markers.
    pub markers: Vec<Marker>,
    refresh: f32,
    radio: Vec<u8>,
    typing: Typewriter,
    time: f32,
}

impl Default for Binocucom {
    fn default() -> Self {
        Binocucom {
            slide: Slide::default(),
            heading: 0.0,
            zoom: 1.0,
            reticle: (0.0, 0.0),
            filter: None,
            markers: Vec::new(),
            refresh: 0.0,
            radio: Vec::new(),
            typing: Typewriter::new(RADIO_RATE),
            time: 0.0,
        }
    }
}

impl Binocucom {
    /// Opens the view.
    pub fn open(&mut self) {
        self.slide.show();
        self.refresh = 0.0;
    }

    /// Closes the view.
    pub fn close(&mut self) {
        self.slide.hide();
    }

    /// Starts a radio message typing out.
    pub fn set_radio(&mut self, text: &[u8]) {
        self.radio = text.to_vec();
        self.typing.restart();
    }

    /// The radio text.
    pub fn radio(&self) -> &[u8] {
        &self.radio
    }

    /// Advances time; every [`REFRESH`] seconds the markers are rebuilt from `targets` (with `camera` the camera
    /// position and `project` mapping a world position to the canvas, `None` when off screen or behind).
    pub fn update(
        &mut self,
        dt: f32,
        camera: [f32; 3],
        targets: &[Target],
        project: impl Fn([f32; 3]) -> Option<(f32, f32)>,
    ) {
        self.slide.update(dt, true);
        self.typing.update(dt);
        self.time += dt;
        self.refresh -= dt;
        if self.refresh > 0.0 {
            return;
        }
        self.refresh += REFRESH;
        if self.refresh <= 0.0 {
            self.refresh = REFRESH;
        }
        self.markers.clear();
        for t in targets {
            let d = [t.position[0] - camera[0], t.position[1] - camera[1], t.position[2] - camera[2]];
            let dist2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if dist2 > MARKER_RANGE * MARKER_RANGE {
                continue;
            }
            if let Some(at) = project(t.position) {
                self.markers.push(Marker {
                    at,
                    kind: t.kind,
                    distance: crate::sqrt(dist2),
                    highlighted: self.filter == Some(t.kind),
                    scan_point: t.scan_point,
                });
            }
        }
        // Scan points first, then nearest first; keep 32.
        self.markers.sort_by(|a, b| b.scan_point.cmp(&a.scan_point).then(a.distance.total_cmp(&b.distance)));
        self.markers.truncate(MAX_MARKERS);
    }

    /// Draws the overlay.
    pub fn draw(&self, fonts: &FontSet, list: &mut DrawList) {
        let a = self.slide.amount();
        if a <= 0.0 {
            return;
        }
        let c = list.canvas;
        let start = list.quads.len();
        let dark = rgba8(0, 8, 12, 235);
        let rim = rgba8(120, 200, 190, 255);
        // The opening: two overlapping circles side by side, drawn as dark bands outside it.
        let r = c.height * 0.42;
        let (cx, cy) = (c.width * 0.5, c.height * 0.5);
        let (lx, rx) = (cx - r * 0.55, cx + r * 0.55);
        let band = 2.0;
        let mut y = 0.0;
        while y < c.height {
            let yc = y + band * 0.5 - cy;
            if yc.abs() >= r {
                list.rect(Rect::new(0.0, y, c.width, band), dark);
            } else {
                let half = crate::sqrt(r * r - yc * yc);
                let (x0, x1) = (lx - half, rx + half);
                list.rect(Rect::new(0.0, y, x0, band), dark);
                list.rect(Rect::new(x1, y, c.width - x1, band), dark);
                list.rect(Rect::new(x0 - 1.0, y, 2.0, band), rim);
                list.rect(Rect::new(x1 - 1.0, y, 2.0, band), rim);
            }
            y += band;
        }
        // Reticle.
        let (rx0, ry0) = (cx + self.reticle.0, cy + self.reticle.1);
        list.rect(Rect::new(rx0 - 12.0, ry0 - 0.5, 9.0, 1.0), WHITE);
        list.rect(Rect::new(rx0 + 3.0, ry0 - 0.5, 9.0, 1.0), WHITE);
        list.rect(Rect::new(rx0 - 0.5, ry0 - 12.0, 1.0, 9.0), WHITE);
        list.rect(Rect::new(rx0 - 0.5, ry0 + 3.0, 1.0, 9.0), WHITE);
        // Markers.
        for m in &self.markers {
            let col = if m.highlighted { rgba8(255, 220, 60, 255) } else { rgba8(120, 230, 210, 220) };
            let s = if m.scan_point { 7.0 } else { 5.0 };
            list.frame(Rect::new(m.at.0 - s, m.at.1 - s, 2.0 * s, 2.0 * s), 1.0, col);
        }
        // Compass strip: a tick every 15 degrees over +-90 around the heading, letters at the quarters.
        let strip = Rect::new(cx - 150.0, 12.0, 300.0, 18.0);
        list.rect(strip, rgba8(0, 0, 0, 140));
        let small = TextStyle { align: Align::Center, ..TextStyle::default() }.scaled(0.5);
        let mut deg = crate::floor(self.heading / 15.0) * 15.0 - 90.0;
        while deg <= self.heading + 90.0 {
            let x = cx + (deg - self.heading) / 90.0 * 150.0;
            let d = deg - crate::floor(deg / 360.0) * 360.0;
            let major = d % 90.0 == 0.0;
            list.rect(Rect::new(x - 0.5, strip.y, 1.0, if major { 6.0 } else { 3.0 }), rim);
            if major {
                let name: &[u8] = match d as i32 {
                    0 => b"N",
                    90 => b"E",
                    180 => b"S",
                    _ => b"W",
                };
                let l = layout(fonts, name, &small);
                l.draw(fonts, list, x - l.width * 0.5, strip.y + 6.0, None, 1.0);
            }
            deg += 15.0;
        }
        list.rect(Rect::new(cx - 0.5, strip.y - 3.0, 1.0, strip.h + 3.0), WHITE);
        // Zoom readout.
        let zoom = crate::tables::sprintf(
            b"x%d.%d",
            &[
                crate::tables::Arg::Int(self.zoom as i64),
                crate::tables::Arg::Int(((self.zoom * 10.0) as i64 % 10).abs()),
            ],
        );
        let l = layout(fonts, &zoom, &TextStyle::default().scaled(0.6));
        l.draw(fonts, list, c.width - 60.0 - l.width, c.height - 40.0, None, 1.0);
        // Radio text, typing out, with a white cursor that blinks.
        if !self.radio.is_empty() {
            let style = TextStyle::default().scaled(0.55).wrapped(300.0);
            let l = layout(fonts, &self.radio, &style);
            let shown = self.typing.visible().min(l.glyphs.len());
            let (x, y) = (60.0, c.height - 30.0 - l.height);
            l.draw(fonts, list, x, y, Some(shown), 1.0);
            if (self.time * 2.0) as i64 % 2 == 0 || shown < l.glyphs.len() {
                let (cx2, cy2) = l.caret(shown);
                let h = l.glyphs.first().map_or(10.0, |g| g.rect.h);
                list.rect(Rect::new(x + cx2 + 1.0, y + cy2 + 1.0, 1.5, h - 2.0), WHITE);
            }
        }
        list.fade_from(start, a);
    }

    /// Glyphs of the radio text showing now.
    pub fn radio_visible(&self) -> usize {
        self.typing.visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Canvas;
    use crate::font::tests::test_set;

    #[test]
    fn markers_refresh_range_filter_and_limit() {
        let mut b = Binocucom { filter: Some(2), ..Binocucom::default() };
        b.open();
        let mut targets = alloc::vec![
            Target { kind: 1, position: [100.0, 0.0, 0.0], scan_point: false },
            Target { kind: 2, position: [50.0, 0.0, 0.0], scan_point: false },
            Target { kind: 3, position: [5000.0, 0.0, 0.0], scan_point: false },
            Target { kind: 4, position: [3000.0, 0.0, 0.0], scan_point: true },
        ];
        let project = |p: [f32; 3]| Some((p[0] / 10.0, 100.0));
        b.update(0.016, [0.0; 3], &targets, project);
        assert_eq!(b.markers.len(), 3, "out of range dropped");
        assert!(b.markers[0].scan_point);
        assert_eq!(b.markers[1].kind, 2);
        assert!(b.markers[1].highlighted && !b.markers[2].highlighted);
        // Not refreshed again before 0.25 s.
        targets.clear();
        b.update(0.1, [0.0; 3], &targets, project);
        assert_eq!(b.markers.len(), 3);
        b.update(0.2, [0.0; 3], &targets, project);
        assert!(b.markers.is_empty());
        let many: Vec<Target> =
            (0..50).map(|i| Target { kind: 0, position: [i as f32, 0.0, 0.0], scan_point: false }).collect();
        b.update(0.3, [0.0; 3], &many, project);
        assert_eq!(b.markers.len(), MAX_MARKERS);
    }

    #[test]
    fn radio_types_out_and_draws() {
        let f = test_set();
        let mut b = Binocucom::default();
        b.open();
        b.set_radio(b"HELLO THERE");
        b.update(0.1, [0.0; 3], &[], |_| None);
        assert_eq!(b.radio_visible(), 3);
        b.update(0.25, [0.0; 3], &[], |_| None);
        let mut d = DrawList::new(Canvas::default());
        b.draw(&f, &mut d);
        assert!(d.quads.len() > 100);
    }
}
