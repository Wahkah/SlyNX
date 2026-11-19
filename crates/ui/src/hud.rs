//! The on-screen display (hud.md): element placement and slide motion, the counters, the timer, the auto-save icon.

use alloc::vec::Vec;

use crate::draw::{Canvas, DrawList, Rect};
use crate::font::FontSet;
use crate::tables::{Edge, Placement, TextTables};
use crate::text::{TextStyle, layout};

/// Number of display elements.
pub const ELEMENT_COUNT: usize = 37;

/// Element numbers (hud.md 1).
pub mod element {
    #![allow(missing_docs)]
    pub const LETTERBOX: usize = 0;
    pub const BINOCUCOM: usize = 1;
    pub const LIVES: usize = 3;
    pub const BOTTLES: usize = 4;
    pub const KEYS: usize = 5;
    pub const COINS: usize = 6;
    pub const PRIZE: usize = 7;
    pub const TIMER: usize = 16;
    pub const NOTE: usize = 17;
    pub const TITLE: usize = 19;
    pub const TOTALS: usize = 20;
    pub const MAP: usize = 22;
    pub const HIDEOUT: usize = 23;
    pub const PROMPT: usize = 25;
    pub const PANEL_LEFT: usize = 26;
    pub const PANEL_RIGHT: usize = 27;
    pub const AUTOSAVE: usize = 35;
    pub const PERCENT: usize = 36;
}

/// Default slide-in and slide-out time.
pub const SLIDE_TIME: f32 = 0.25;
/// How long a counter stays after its value changes (seconds of running game clock).
pub const COUNTER_VISIBLE: f32 = 2.5;
/// Fastest a shown counter moves toward its value, per second.
pub const COUNT_RATE: f64 = 6000.0;
/// Length of the "smack" pulse when a counter arrives.
pub const SMACK_TIME: f32 = 0.25;
/// Font the counters use (numerals).
pub const COUNTER_FONT: usize = 2;

/// Where an element is in its motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Off screen.
    Hidden,
    /// Sliding in.
    Appearing,
    /// In place.
    Visible,
    /// Sliding out.
    Disappearing,
}

/// The slide motion of one element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slide {
    /// Phase.
    pub phase: Phase,
    /// Seconds into the current phase.
    pub t: f32,
    /// Time to slide in.
    pub time_in: f32,
    /// Time to slide out.
    pub time_out: f32,
    /// Seconds left before a visible element hides by itself (`None`: stays).
    pub remaining: Option<f32>,
}

impl Default for Slide {
    fn default() -> Self {
        Slide { phase: Phase::Hidden, t: 0.0, time_in: SLIDE_TIME, time_out: SLIDE_TIME, remaining: None }
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Slide {
    /// Starts showing (from where it is, if it was sliding out).
    pub fn show(&mut self) {
        match self.phase {
            Phase::Hidden => {
                self.phase = Phase::Appearing;
                self.t = 0.0;
            }
            Phase::Disappearing => {
                // Reverse from the current position.
                let a = 1.0 - (self.t / self.time_out.max(1e-6)).min(1.0);
                self.phase = Phase::Appearing;
                self.t = a * self.time_in;
            }
            Phase::Appearing | Phase::Visible => {}
        }
    }

    /// Shows and hides again after `seconds` visible.
    pub fn show_for(&mut self, seconds: f32) {
        self.show();
        self.remaining = Some(seconds);
    }

    /// Starts hiding.
    pub fn hide(&mut self) {
        match self.phase {
            Phase::Visible => {
                self.phase = Phase::Disappearing;
                self.t = 0.0;
            }
            Phase::Appearing => {
                let a = (self.t / self.time_in.max(1e-6)).min(1.0);
                self.phase = Phase::Disappearing;
                self.t = (1.0 - a) * self.time_out;
            }
            Phase::Hidden | Phase::Disappearing => {}
        }
        self.remaining = None;
    }

    /// Advances the motion; `clock_running` lets the visible time run down.
    pub fn update(&mut self, dt: f32, clock_running: bool) {
        self.t += dt;
        match self.phase {
            Phase::Appearing if self.t >= self.time_in => {
                self.phase = Phase::Visible;
                self.t = 0.0;
            }
            Phase::Disappearing if self.t >= self.time_out => {
                self.phase = Phase::Hidden;
                self.t = 0.0;
            }
            Phase::Visible => {
                if clock_running && let Some(r) = self.remaining.as_mut() {
                    *r -= dt;
                    if *r <= 0.0 {
                        self.hide();
                    }
                }
            }
            _ => {}
        }
    }

    /// How far in, 0 (off screen) to 1 (in place), smoothed.
    pub fn amount(&self) -> f32 {
        match self.phase {
            Phase::Hidden => 0.0,
            Phase::Visible => 1.0,
            Phase::Appearing => smooth(self.t / self.time_in.max(1e-6)),
            Phase::Disappearing => smooth(1.0 - self.t / self.time_out.max(1e-6)),
        }
    }

    /// Whether any of it is on screen.
    pub fn on_screen(&self) -> bool {
        self.phase != Phase::Hidden
    }
}

/// Computes each element's home rectangle from the placement table, the elements' sizes and which are showing
/// (shown elements count as visible, the others as hidden). See [`place_phases`].
pub fn place(canvas: Canvas, placements: &[Placement], sizes: &[(f32, f32)], showing: &[bool]) -> Vec<Rect> {
    let phases: Vec<Phase> = (0..sizes.len())
        .map(|i| if showing.get(i).copied().unwrap_or(false) { Phase::Visible } else { Phase::Hidden })
        .collect();
    place_phases(canvas, placements, sizes, &phases)
}

/// Whether element `other` counts as showing when placing element `this` (hud.md 2): it is appearing or visible,
/// or both are disappearing.
fn counts_as_showing(phases: &[Phase], this: usize, other: usize) -> bool {
    match phases.get(other) {
        Some(Phase::Appearing | Phase::Visible) => true,
        Some(Phase::Disappearing) => phases.get(this) == Some(&Phase::Disappearing),
        _ => false,
    }
}

/// Computes each element's home rectangle from the placement table, the elements' sizes and their motion phases
/// (hud.md 2). The edge field says on which side of the element being placed the other element lies: Left, the
/// area starts at the other's right side; Right, it ends at the other's left side; Top, it starts at the other's
/// bottom (the element sits below it); Bottom, it ends at the other's top (it sits above it). If the other element
/// is not showing, its own "placed against" entry is tried, and so on down the chain.
pub fn place_phases(canvas: Canvas, placements: &[Placement], sizes: &[(f32, f32)], phases: &[Phase]) -> Vec<Rect> {
    let n = placements.len().min(sizes.len());
    let mut out: Vec<Option<Rect>> = alloc::vec![None; n];
    fn go(
        i: usize,
        canvas: Canvas,
        p: &[Placement],
        sizes: &[(f32, f32)],
        phases: &[Phase],
        out: &mut Vec<Option<Rect>>,
        depth: usize,
    ) -> Rect {
        if let Some(r) = out[i] {
            return r;
        }
        let pl = p[i];
        let (w, h) = sizes[i];
        let mut area = Rect::new(0.0, 0.0, canvas.width, canvas.height);
        // The first showing element down the chain of "placed against" entries.
        let mut entry = (pl.against, pl.against_edge);
        let mut steps = 0;
        let found = loop {
            match entry {
                (Some(j), Some(edge)) if j < out.len() && j != i && steps < 8 => {
                    if counts_as_showing(phases, i, j) {
                        break Some((j, edge));
                    }
                    entry = (p[j].against, p[j].against_edge);
                    steps += 1;
                }
                _ => break None,
            }
        };
        if let Some((j, edge)) = found
            && depth < 8
        {
            let r = go(j, canvas, p, sizes, phases, out, depth + 1);
            let (l, t, rr, b) = (area.x, area.y, area.right(), area.bottom());
            let (l, t, rr, b) = match edge {
                Edge::Top => (l, r.bottom(), rr, b),
                Edge::Bottom => (l, t, rr, r.y),
                Edge::Left => (r.right(), t, rr, b),
                Edge::Right => (l, t, r.x, b),
            };
            area = Rect::new(l, t, (rr - l).max(0.0), (b - t).max(0.0));
        }
        let x = if pl.x > 0.0 {
            area.x + pl.x
        } else if pl.x < 0.0 {
            area.right() + pl.x - w
        } else {
            area.x + (area.w - w) * 0.5
        };
        let y = if pl.y > 0.0 {
            area.y + pl.y
        } else if pl.y < 0.0 {
            area.bottom() + pl.y - h
        } else {
            area.y + (area.h - h) * 0.5
        };
        let r = Rect::new(x, y, w, h);
        out[i] = Some(r);
        r
    }
    (0..n).map(|i| go(i, canvas, placements, sizes, phases, &mut out, 0)).collect()
}

/// Where an element is drawn at slide amount `a`: between its off-screen position past `from` and its home.
/// Elements that do not slide fade instead (the returned alpha).
pub fn slide_rect(canvas: Canvas, home: Rect, from: Option<Edge>, a: f32) -> (Rect, f32) {
    let off = match from {
        Some(Edge::Left) => home.offset(-home.right(), 0.0),
        Some(Edge::Right) => home.offset(canvas.width - home.x, 0.0),
        Some(Edge::Top) => home.offset(0.0, -home.bottom()),
        Some(Edge::Bottom) => home.offset(0.0, canvas.height - home.y),
        None => return (home, a),
    };
    let lerp = |p: f32, q: f32| p + (q - p) * a;
    (Rect::new(lerp(off.x, home.x), lerp(off.y, home.y), home.w, home.h), 1.0)
}

/// A number shown on screen that counts toward its value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Counter {
    /// The real value.
    pub value: i64,
    /// The number shown.
    pub shown: f64,
    /// Seconds left of the "smack" pulse.
    pub smack: f32,
}

impl Counter {
    /// A counter showing `v`.
    pub fn new(v: i64) -> Counter {
        Counter { value: v, shown: v as f64, smack: 0.0 }
    }

    /// Moves the shown number toward the value at up to [`COUNT_RATE`] per second.
    pub fn update(&mut self, dt: f32) {
        self.smack = (self.smack - dt).max(0.0);
        let target = self.value as f64;
        if self.shown != target {
            let step = COUNT_RATE * dt as f64;
            if (target - self.shown).abs() <= step {
                self.shown = target;
                self.smack = SMACK_TIME;
            } else {
                self.shown += if target > self.shown { step } else { -step };
            }
        }
    }

    /// The shown number, rounded toward where it came from.
    pub fn display(&self) -> i64 {
        let f = crate::floor64(self.shown) as i64;
        if self.shown <= self.value as f64 || f as f64 == self.shown { f } else { f + 1 }
    }

    /// Text scale for the smack pulse (1 at rest, up to 1.3).
    pub fn pulse(&self) -> f32 {
        if self.smack <= 0.0 {
            return 1.0;
        }
        let x = 1.0 - self.smack / SMACK_TIME;
        1.0 + 0.3 * (1.0 - (2.0 * x - 1.0).abs())
    }
}

/// The values the game hands the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HudValues {
    /// Lives.
    pub lives: i64,
    /// Coins (`None`: pickups disabled in this level).
    pub coins: Option<i64>,
    /// Keys (`None`: not an episode world).
    pub keys: Option<i64>,
    /// Clue bottles (collected, total); hidden when the total is 0.
    pub bottles: (i64, i64),
    /// Completion percent, if shown.
    pub percent: Option<i64>,
}

/// The HUD model.
#[derive(Clone, Debug)]
pub struct Hud {
    /// The placement table (37 records).
    pub placements: Vec<Placement>,
    /// Each element's motion.
    pub slides: [Slide; ELEMENT_COUNT],
    /// Lives.
    pub lives: Counter,
    /// Coins.
    pub coins: Counter,
    /// Keys.
    pub keys: Counter,
    /// Clue bottles collected.
    pub bottles: Counter,
    /// Bottle total.
    pub bottle_total: i64,
    /// Percent.
    pub percent: Counter,
    values: HudValues,
    timer: Option<f32>,
    paused: bool,
    saving: bool,
}

impl Hud {
    /// A HUD using `placements` (from [`crate::tables::read_placements`]) and starting at `values`, all hidden.
    pub fn new(placements: Vec<Placement>, values: HudValues) -> Hud {
        Hud {
            placements,
            slides: [Slide::default(); ELEMENT_COUNT],
            lives: Counter::new(values.lives),
            coins: Counter::new(values.coins.unwrap_or(0)),
            keys: Counter::new(values.keys.unwrap_or(0)),
            bottles: Counter::new(values.bottles.0),
            bottle_total: values.bottles.1,
            percent: Counter::new(values.percent.unwrap_or(0)),
            values,
            timer: None,
            paused: false,
            saving: false,
        }
    }

    /// The current values.
    pub fn values(&self) -> HudValues {
        self.values
    }

    fn enabled(&self, e: usize) -> bool {
        match e {
            element::LIVES => true,
            element::COINS => self.values.coins.is_some(),
            element::KEYS => self.values.keys.is_some(),
            element::BOTTLES => self.values.bottles.1 > 0,
            element::PERCENT => self.values.percent.is_some(),
            _ => false,
        }
    }

    /// Sets the values; a counter whose value changed shows for 2.5 s.
    pub fn set_values(&mut self, v: HudValues) {
        let old = self.values;
        self.values = v;
        self.bottle_total = v.bottles.1;
        let changed = |e: usize, c: &mut Counter, new: i64, was: i64, slides: &mut [Slide; ELEMENT_COUNT]| {
            c.value = new;
            if new != was {
                slides[e].show_for(COUNTER_VISIBLE);
            }
        };
        changed(element::LIVES, &mut self.lives, v.lives, old.lives, &mut self.slides);
        changed(element::COINS, &mut self.coins, v.coins.unwrap_or(0), old.coins.unwrap_or(0), &mut self.slides);
        changed(element::KEYS, &mut self.keys, v.keys.unwrap_or(0), old.keys.unwrap_or(0), &mut self.slides);
        changed(element::BOTTLES, &mut self.bottles, v.bottles.0, old.bottles.0, &mut self.slides);
        changed(
            element::PERCENT,
            &mut self.percent,
            v.percent.unwrap_or(0),
            old.percent.unwrap_or(0),
            &mut self.slides,
        );
        for e in [element::COINS, element::KEYS, element::BOTTLES, element::PERCENT] {
            if !self.enabled(e) {
                self.slides[e].hide();
            }
        }
    }

    /// Shows every enabled counter for 2.5 s (for example after a level starts).
    pub fn flash_counters(&mut self) {
        for e in [element::LIVES, element::COINS, element::KEYS, element::BOTTLES, element::PERCENT] {
            if self.enabled(e) {
                self.slides[e].show_for(COUNTER_VISIBLE);
            }
        }
    }

    /// Pausing shows all counters; resuming lets them time out.
    pub fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        for e in [element::LIVES, element::COINS, element::KEYS, element::BOTTLES, element::PERCENT] {
            if !self.enabled(e) {
                continue;
            }
            if paused {
                self.slides[e].show();
                self.slides[e].remaining = None;
            } else {
                self.slides[e].show_for(COUNTER_VISIBLE);
            }
        }
    }

    /// Shows the timer with `seconds` left (or elapsed), or hides it.
    pub fn set_timer(&mut self, seconds: Option<f32>) {
        match (self.timer.is_some(), seconds.is_some()) {
            (false, true) => self.slides[element::TIMER].show(),
            (true, false) => self.slides[element::TIMER].hide(),
            _ => {}
        }
        self.timer = seconds;
    }

    /// Shows or hides the auto-save icon.
    pub fn set_saving(&mut self, saving: bool) {
        if saving != self.saving {
            self.saving = saving;
            if saving {
                self.slides[element::AUTOSAVE].show();
            } else {
                self.slides[element::AUTOSAVE].hide();
            }
        }
    }

    /// Advances motion and counting by `dt` seconds of game time; `clock_running` is false while the game clock is
    /// stopped (counters then stay up).
    pub fn update(&mut self, dt: f32, clock_running: bool) {
        for s in &mut self.slides {
            s.update(dt, clock_running && !self.paused);
        }
        for c in [&mut self.lives, &mut self.coins, &mut self.keys, &mut self.bottles, &mut self.percent] {
            c.update(dt);
        }
    }

    /// The markup text of counter element `e`.
    pub fn counter_text(&self, e: usize) -> Vec<u8> {
        use crate::tables::{Arg, sprintf};
        match e {
            element::LIVES => sprintf(b"L%d", &[Arg::Int(self.lives.display())]),
            element::COINS => sprintf(b"&3C&.%d", &[Arg::Int(self.coins.display())]),
            element::KEYS => sprintf(b"%d&3K&.", &[Arg::Int(self.keys.display())]),
            element::BOTTLES => {
                let done = self.values.bottles.0 >= self.bottle_total;
                let tail: &[u8] = if done { b"&3!&." } else { b"?" };
                let mut s = sprintf(b"%d/%d", &[Arg::Int(self.bottles.display()), Arg::Int(self.bottle_total)]);
                s.extend_from_slice(tail);
                s
            }
            element::PERCENT => sprintf(b"%d%%", &[Arg::Int(self.percent.display())]),
            _ => Vec::new(),
        }
    }

    /// Draws the HUD into `list`.
    pub fn draw(&self, fonts: &FontSet, tables: &TextTables, list: &mut DrawList) {
        let canvas = list.canvas;
        let counter_style =
            TextStyle { font: COUNTER_FONT.min(fonts.fonts.len().saturating_sub(1)), ..TextStyle::default() };
        // Lay out every element that is on screen, to size it.
        let mut texts: Vec<(usize, crate::text::TextLayout, f32)> = Vec::new();
        for e in [element::LIVES, element::BOTTLES, element::KEYS, element::COINS, element::PERCENT] {
            if self.slides[e].on_screen() {
                let c = match e {
                    element::LIVES => &self.lives,
                    element::BOTTLES => &self.bottles,
                    element::KEYS => &self.keys,
                    element::COINS => &self.coins,
                    _ => &self.percent,
                };
                let l = layout(fonts, &self.counter_text(e), &counter_style);
                texts.push((e, l, c.pulse()));
            }
        }
        if self.slides[element::TIMER].on_screen() {
            let t = tables.timer_text(self.timer.unwrap_or(0.0));
            texts.push((element::TIMER, layout(fonts, &t, &counter_style), 1.0));
        }
        if self.slides[element::AUTOSAVE].on_screen() {
            texts.push((element::AUTOSAVE, layout(fonts, b"^15&2a&.^.", &TextStyle::default()), 1.0));
        }
        let mut sizes = alloc::vec![(0.0f32, 0.0f32); ELEMENT_COUNT];
        let mut phases = [Phase::Hidden; ELEMENT_COUNT];
        for (e, l, _) in &texts {
            sizes[*e] = (l.width, l.height);
            phases[*e] = self.slides[*e].phase;
        }
        if self.placements.len() < ELEMENT_COUNT {
            return;
        }
        let homes = place_phases(canvas, &self.placements, &sizes, &phases);
        for (e, l, pulse) in &texts {
            let (r, alpha) = slide_rect(canvas, homes[*e], self.placements[*e].slide_from, self.slides[*e].amount());
            // The pulse grows the text about its centre.
            let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
            if *pulse != 1.0 {
                let big = layout(fonts, &self.counter_text(*e), &counter_style.scaled(*pulse));
                let x = cx - big.width * 0.5;
                let y = cy - big.height * 0.5;
                draw_shadowed(&big, fonts, list, x, y, alpha);
            } else {
                draw_shadowed(l, fonts, list, r.x, r.y, alpha);
            }
        }
    }
}

/// Draws text with a soft dark shadow one unit down and right.
pub fn draw_shadowed(l: &crate::text::TextLayout, fonts: &FontSet, list: &mut DrawList, x: f32, y: f32, alpha: f32) {
    let from = list.quads.len();
    l.draw(fonts, list, x + 1.0, y + 1.0, None, alpha * 0.5);
    for q in &mut list.quads[from..] {
        q.color[0] = 0.0;
        q.color[1] = 0.0;
        q.color[2] = 0.0;
    }
    l.draw(fonts, list, x, y, None, alpha);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::font::tests::test_set;

    /// A placement table with the hud.md selected records filled in.
    pub(crate) fn placements() -> Vec<Placement> {
        let none = Placement { x: 0.0, y: 0.0, slide_from: None, against: None, against_edge: None };
        let mut p = alloc::vec![none; ELEMENT_COUNT];
        let mk = |x, y, s, a: i32, ae| Placement {
            x,
            y,
            slide_from: Edge::from_code(s),
            against: (a >= 0).then_some(a as usize),
            against_edge: Edge::from_code(ae),
        };
        p[3] = mk(0.0, 20.0, 2, -1, -1);
        p[4] = mk(-18.0, 20.0, 1, -1, -1);
        p[5] = mk(-18.0, 20.0, 1, 4, 2);
        p[6] = mk(18.0, 20.0, 0, -1, -1);
        p[7] = mk(-18.0, 20.0, 1, 5, 2);
        p[16] = mk(18.0, -20.0, 0, -1, -1);
        p[17] = mk(18.0, -20.0, 0, 16, 3);
        p[35] = mk(18.0, -20.0, 3, -1, -1);
        p
    }

    #[test]
    fn slide_motion() {
        let mut s = Slide::default();
        assert_eq!(s.amount(), 0.0);
        s.show_for(1.0);
        s.update(0.125, true);
        assert!((s.amount() - 0.5).abs() < 1e-6);
        s.update(0.125, true);
        assert_eq!(s.phase, Phase::Visible);
        // The visible time only runs with the clock.
        s.update(5.0, false);
        assert_eq!(s.phase, Phase::Visible);
        s.update(1.0, true);
        assert_eq!(s.phase, Phase::Disappearing);
        s.update(0.1, true);
        // Showing again reverses from where it is.
        let a = s.amount();
        s.show();
        assert!((s.amount() - a).abs() < 0.05);
        s.hide();
        s.update(0.3, true);
        assert_eq!(s.phase, Phase::Hidden);
    }

    #[test]
    fn placement_rules() {
        let c = Canvas::default();
        let p = placements();
        let mut sizes = alloc::vec![(0.0, 0.0); ELEMENT_COUNT];
        sizes[3] = (40.0, 30.0);
        sizes[4] = (60.0, 30.0);
        sizes[5] = (30.0, 30.0);
        sizes[6] = (50.0, 30.0);
        sizes[16] = (40.0, 30.0);
        let mut showing = [false; ELEMENT_COUNT];
        for e in [3, 4, 5, 6, 16] {
            showing[e] = true;
        }
        let r = place(c, &p, &sizes, &showing);
        assert_eq!(r[3], Rect::new(300.0, 20.0, 40.0, 30.0), "lives top centre");
        assert_eq!(r[4], Rect::new(640.0 - 18.0 - 60.0, 20.0, 60.0, 30.0), "bottles top right");
        assert_eq!(r[5], Rect::new(640.0 - 18.0 - 30.0, 70.0, 30.0, 30.0), "keys under the bottles");
        assert_eq!(r[6], Rect::new(18.0, 20.0, 50.0, 30.0), "coins top left");
        assert_eq!(r[16], Rect::new(18.0, 480.0 - 20.0 - 30.0, 40.0, 30.0), "timer bottom left");
        // Without the bottle counter the keys take its place.
        showing[4] = false;
        let r = place(c, &p, &sizes, &showing);
        assert_eq!(r[5].y, 20.0);
        // The note (17) sits above the timer (16, edge 3), both anchored 20 from the bottom.
        sizes[17] = (80.0, 20.0);
        showing[17] = true;
        let r = place(c, &p, &sizes, &showing);
        assert_eq!(r[17], Rect::new(18.0, 480.0 - 20.0 - 30.0 - 20.0 - 20.0, 80.0, 20.0), "note above the timer");
        assert!(r[17].bottom() <= r[16].y);
        // The sprint prize (7) is against the keys (5); without the keys it falls back to the keys' own entry,
        // the clue counter (4), and sits under it.
        sizes[7] = (30.0, 30.0);
        showing[4] = true;
        showing[5] = false;
        showing[7] = true;
        let r = place(c, &p, &sizes, &showing);
        assert_eq!(r[7].y, 20.0 + 30.0 + 20.0, "prize under the clue counter");
        showing[5] = true;
        let r = place(c, &p, &sizes, &showing);
        assert_eq!(r[7].y, r[5].bottom() + 20.0, "prize under the keys");
        // A disappearing element only counts for another that is disappearing too.
        let mut ph = [Phase::Hidden; ELEMENT_COUNT];
        ph[16] = Phase::Disappearing;
        ph[17] = Phase::Visible;
        assert_eq!(place_phases(c, &p, &sizes, &ph)[17].bottom(), 480.0 - 20.0, "timer leaving: note at the edge");
        ph[17] = Phase::Disappearing;
        assert!(place_phases(c, &p, &sizes, &ph)[17].bottom() <= 480.0 - 20.0 - 30.0, "both leaving: still above");
        // Sliding: from the right edge, fully off screen at 0.
        let (off, _) = slide_rect(c, Rect::new(500.0, 20.0, 60.0, 30.0), Some(Edge::Right), 0.0);
        assert_eq!(off.x, 640.0);
        let (top, _) = slide_rect(c, Rect::new(300.0, 20.0, 40.0, 30.0), Some(Edge::Top), 0.0);
        assert_eq!(top.bottom(), 0.0);
        let (_, a) = slide_rect(c, Rect::default(), None, 0.4);
        assert_eq!(a, 0.4);
    }

    #[test]
    fn counters_count_and_time_out() {
        let mut c = Counter::new(0);
        c.value = 100;
        c.update(1.0 / 120.0);
        assert_eq!(c.display(), 50);
        c.update(1.0 / 120.0);
        assert_eq!(c.display(), 100);
        assert!(c.pulse() == 1.0 && c.smack > 0.0);
        c.update(SMACK_TIME / 2.0);
        assert!(c.pulse() > 1.2);

        let mut h = Hud::new(placements(), HudValues { lives: 3, coins: Some(0), ..HudValues::default() });
        h.set_values(HudValues { lives: 3, coins: Some(5), ..HudValues::default() });
        assert_eq!(h.slides[element::COINS].phase, Phase::Appearing);
        assert_eq!(h.slides[element::LIVES].phase, Phase::Hidden);
        h.update(0.25, true);
        h.update(2.4, true);
        assert_eq!(h.slides[element::COINS].phase, Phase::Visible);
        h.update(0.2, true);
        assert_eq!(h.slides[element::COINS].phase, Phase::Disappearing);
        // Pausing shows every enabled counter and keeps them.
        h.set_paused(true);
        h.update(10.0, false);
        assert_eq!(h.slides[element::LIVES].phase, Phase::Visible);
        assert_eq!(h.slides[element::KEYS].phase, Phase::Hidden, "keys are not enabled here");
        assert_eq!(h.counter_text(element::COINS), b"&3C&.5");
        h.set_values(HudValues { lives: 3, coins: Some(5), bottles: (30, 30), ..HudValues::default() });
        h.update(1.0, false);
        assert_eq!(h.counter_text(element::BOTTLES), b"30/30&3!&.");
    }

    #[test]
    fn draws_counters_and_timer() {
        let f = test_set();
        let tables = TextTables {
            timer_format: b"^10~b2a83d%d:%02d~.^.".to_vec(),
            timer_tenths_format: b"0.%d".to_vec(),
            ..TextTables::default()
        };
        let mut h = Hud::new(placements(), HudValues { lives: 3, ..HudValues::default() });
        h.flash_counters();
        h.set_timer(Some(75.0));
        h.set_saving(true);
        h.update(0.3, true);
        let mut d = DrawList::new(Canvas::default());
        h.draw(&f, &tables, &mut d);
        assert!(!d.quads.is_empty());
        // Everything is on screen once the slides finish.
        for q in &d.quads {
            assert!(q.rect.x >= 0.0 && q.rect.right() <= 640.0 + 1.0 && q.rect.y >= 0.0 && q.rect.bottom() <= 481.0);
        }
        // The timer text uses its colour.
        assert!(d.quads.iter().any(|q| (q.color[0] - 89.0 / 128.0).abs() < 1e-6));
    }
}
