//! The options page of the pause menu (Sly 2 user-interface.md 6.2: the options table at `0x2BFEC8`; Sly 3's pause
//! and options screen 1056 has the same volume texts): Music Volume (103) and Effects Volume (104) are sliders moved
//! with Left and Right, Vibrations (72) turns on and off with Cross or Left/Right. Triangle or Circle goes back. The
//! session saves the values in the progress block's `flags` group and applies what it can.

use super::font::FontSheet;
use super::jobs::{Phase, Screen};
use crate::draw::{DrawList, Rect};
use crate::nav::{Dir, MenuInput, Repeat};

/// Slider step per press [working choice].
pub const STEP: f32 = 0.1;
/// Text size [working choice].
pub const SIZE: f32 = 18.0;

/// The values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Values {
    /// 0 to 1.
    pub music: f32,
    pub effects: f32,
    pub vibration: bool,
}

impl Default for Values {
    fn default() -> Self {
        Values { music: 1.0, effects: 1.0, vibration: true }
    }
}

/// What the page asks of the session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionsAction {
    None,
    /// A value changed: store and apply it.
    Changed(Values),
    Closed,
}

/// The rows' string IDs (Sly 2): music, effects, vibration.
pub const STRINGS: [i16; 3] = [103, 104, 72];

/// The options page.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionsScreen {
    pub values: Values,
    pub cursor: usize,
    pub screen: Screen,
    repeat: Repeat,
}

impl OptionsScreen {
    /// Opens with the stored values.
    pub fn open(values: Values) -> OptionsScreen {
        let mut screen = Screen::new(880);
        screen.show();
        OptionsScreen { values, cursor: 0, screen, repeat: Repeat::default() }
    }

    pub fn visible(&self) -> bool {
        self.screen.visible()
    }

    /// Advances on the real clock. `circle`: Circle went down.
    pub fn update(&mut self, dt: f32, input: &MenuInput, circle: bool) -> OptionsAction {
        self.screen.update(dt);
        if !matches!(self.screen.phase, Phase::Idle(_)) {
            return if self.screen.visible() { OptionsAction::None } else { OptionsAction::Closed };
        }
        if input.back || circle {
            self.screen.hide();
            return OptionsAction::None;
        }
        let before = self.values;
        match self.repeat.update(input, dt) {
            Some(Dir::Up) if self.cursor > 0 => self.cursor -= 1,
            Some(Dir::Down) if self.cursor < 2 => self.cursor += 1,
            Some(d @ (Dir::Left | Dir::Right)) => {
                let by = if d == Dir::Right { STEP } else { -STEP };
                match self.cursor {
                    0 => self.values.music = (self.values.music + by).clamp(0.0, 1.0),
                    1 => self.values.effects = (self.values.effects + by).clamp(0.0, 1.0),
                    _ => self.values.vibration = !self.values.vibration,
                }
            }
            _ => {}
        }
        if input.accept && self.cursor == 2 {
            self.values.vibration = !self.values.vibration;
        }
        if self.values != before { OptionsAction::Changed(self.values) } else { OptionsAction::None }
    }

    /// Draws the page; `labels` are the rows' texts and the on/off words.
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, labels: [&[u8]; 5]) {
        if !self.screen.visible() {
            return;
        }
        let a = self.screen.coverage();
        let (cw, ch) = (list.canvas.width, list.canvas.height);
        let panel = Rect::new(cw * 0.22, ch * 0.3, cw * 0.56, ch * 0.4);
        list.rect(Rect::new(0.0, 0.0, cw, ch), [0.0, 0.0, 0.0, 0.35 * a]);
        list.rect(panel, [0.05, 0.08, 0.2, 0.92 * a]);
        list.frame(panel, 3.0, [0.9, 0.75, 0.25, a]);
        let row = 40.0;
        for i in 0..3 {
            let y = panel.y + 24.0 + row * i as f32;
            if i == self.cursor {
                list.rect(Rect::new(panel.x + 10.0, y - 4.0, panel.w - 20.0, row - 6.0), [0.9, 0.75, 0.25, 0.3 * a]);
            }
            if let Some(f) = font {
                f.draw(list, labels[i], panel.x + 20.0, y, SIZE, [[1.0, 1.0, 1.0, a]; 2], None);
            }
            let bar = Rect::new(panel.x + panel.w * 0.55, y + 6.0, panel.w * 0.38, 10.0);
            match i {
                0 | 1 => {
                    let v = if i == 0 { self.values.music } else { self.values.effects };
                    list.rect(bar, [0.2, 0.2, 0.3, a]);
                    list.rect(Rect::new(bar.x, bar.y, bar.w * v, bar.h), [0.3, 0.8, 1.0, a]);
                }
                _ => {
                    if let Some(f) = font {
                        let word = if self.values.vibration { labels[3] } else { labels[4] };
                        f.draw(list, word, bar.x, y, SIZE, [[1.0, 0.85, 0.3, a]; 2], None);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliders_and_toggle() {
        let mut o = OptionsScreen::open(Values::default());
        for _ in 0..60 {
            o.update(1.0 / 60.0, &MenuInput::default(), false);
        }
        let left = MenuInput { dpad: [false, false, true, false], ..MenuInput::default() };
        assert!(matches!(o.update(0.016, &left, false), OptionsAction::Changed(v) if (v.music - 0.9).abs() < 1e-6));
        o.update(0.5, &MenuInput::default(), false);
        let down = MenuInput { dpad: [false, true, false, false], ..MenuInput::default() };
        o.update(0.016, &down, false);
        o.update(0.5, &MenuInput::default(), false);
        o.update(0.016, &down, false);
        assert_eq!(o.cursor, 2);
        assert!(matches!(
            o.update(0.016, &MenuInput { accept: true, ..MenuInput::default() }, false),
            OptionsAction::Changed(v) if !v.vibration
        ));
        o.update(0.016, &MenuInput::default(), true);
        let mut last = OptionsAction::None;
        for _ in 0..40 {
            last = o.update(1.0 / 60.0, &MenuInput::default(), false);
        }
        assert_eq!(last, OptionsAction::Closed);
    }
}
