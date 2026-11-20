//! Sly 2's HUD (user-interface.md 5): the character gauge, the coin counter, the prompt and the subtitle, laid out
//! by SlyNX (the screens' own pictures are not drawn yet; their strings and timings are the game's).
//!
//! - **Gauge** (object 875): the current character's health and, when it owns an item with a power cost, gadget
//!   power. A rise shows at once; a fall eases toward the new value (the smoothing record (100, 0, 0.5) at
//!   `0x2C3248` [R]; here an exponential approach with 0.5 s [working choice]). Hidden when no character is current.
//! - **Coins** (object 877, string 719 `%d`): shown when the count changes, hidden after 5 s idle (option 1125).
//! - **Prompt** (object 881): one string for a set time, or until ended when the time is negative; a new prompt while
//!   one shows waits in one slot and follows it; asking for the shown string only updates its time. The time counts
//!   only while the prompt is visible.
//! - **Subtitle**: the talk's text in its speaker's colour, at the bottom.

use alloc::string::String;
use alloc::vec::Vec;

use super::font::FontSheet;
use super::jobs::{Phase, Screen};
use crate::draw::{DrawList, Rect};

/// Seconds a counter stays idle before hiding (option 1125 on objects 877 to 879).
pub const COUNTER_IDLE: f32 = 5.0;
/// Time constant of a falling gauge [working choice].
pub const GAUGE_EASE: f32 = 0.5;
/// Text sizes [working choice].
pub const TEXT_SIZE: f32 = 22.0;
pub const SUBTITLE_SIZE: f32 = 20.0;

/// What the HUD shows this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HudValues {
    /// The current character (0 Sly, 1 Bentley, 2 Murray), none when the current object is not one of them.
    pub who: Option<u8>,
    /// Health (value, maximum).
    pub health: (f32, f32),
    /// Gadget power (value, maximum), when the character owns an item with a power cost.
    pub power: Option<(f32, f32)>,
    /// Coins owned.
    pub coins: i32,
}

/// A prompt: its text and seconds left (negative: until ended).
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub id: i32,
    pub text: String,
    pub time: f32,
}

/// The HUD.
#[derive(Clone, Debug, PartialEq)]
pub struct Hud {
    pub values: HudValues,
    /// The gauge's shown health and power (eased).
    shown: (f32, f32),
    pub coins: Screen,
    last_coins: Option<i32>,
    pub prompt: Option<Prompt>,
    queued: Option<Prompt>,
    pub prompt_screen: Screen,
}

impl Default for Hud {
    fn default() -> Self {
        let mut coins = Screen::new(877);
        coins.idle_limit = Some(COUNTER_IDLE);
        Hud {
            values: HudValues::default(),
            shown: (0.0, 0.0),
            coins,
            last_coins: None,
            prompt: None,
            queued: None,
            prompt_screen: Screen::new(881),
        }
    }
}

fn ease(shown: f32, target: f32, dt: f32) -> f32 {
    if target >= shown {
        target
    } else {
        let k = 1.0 - libm_exp(-dt / GAUGE_EASE);
        let v = shown + (target - shown) * k;
        if (v - target).abs() < 1e-4 { target } else { v }
    }
}

/// `e^x` for small negative x without `std` (a few series terms are plenty for a fade).
fn libm_exp(x: f32) -> f32 {
    let x = x.max(-10.0);
    // e^x = (e^(x/8))^8, with e^(x/8) from five series terms.
    let y = x / 8.0;
    let mut e = 1.0 + y * (1.0 + y / 2.0 * (1.0 + y / 3.0 * (1.0 + y / 4.0 * (1.0 + y / 5.0))));
    for _ in 0..3 {
        e *= e;
    }
    e
}

impl Hud {
    /// Shows a prompt (operation 137's option 979): a string for `time` seconds (negative: until ended).
    pub fn prompt(&mut self, id: i32, text: String, time: f32) {
        match &mut self.prompt {
            Some(p) if p.id == id => p.time = time,
            Some(_) => self.queued = Some(Prompt { id, text, time }),
            None => {
                self.prompt = Some(Prompt { id, text, time });
                self.prompt_screen = Screen::new(881);
                self.prompt_screen.show();
            }
        }
    }

    /// Ends the prompt showing.
    pub fn end_prompt(&mut self) {
        self.prompt_screen.hide();
    }

    /// Advances on the real clock with this frame's values.
    pub fn update(&mut self, dt: f32, values: HudValues) {
        if self.values.who != values.who {
            self.shown = (values.health.0, values.power.map_or(0.0, |p| p.0));
        }
        self.values = values;
        self.shown.0 = ease(self.shown.0, values.health.0, dt);
        self.shown.1 = ease(self.shown.1, values.power.map_or(0.0, |p| p.0), dt);
        if self.last_coins != Some(values.coins) {
            if self.last_coins.is_some() {
                self.coins.phase = Phase::Showing(0.0);
            }
            self.last_coins = Some(values.coins);
        }
        self.coins.update(dt);
        // The prompt's time counts while it is visible; a waiting one follows.
        if let Some(p) = self.prompt.as_mut()
            && matches!(self.prompt_screen.phase, Phase::Idle(_))
            && p.time >= 0.0
        {
            p.time -= dt;
            if p.time <= 0.0 {
                self.prompt_screen.hide();
            }
        }
        self.prompt_screen.update(dt);
        if self.prompt.is_some() && !self.prompt_screen.visible() {
            self.prompt = self.queued.take();
            if self.prompt.is_some() {
                self.prompt_screen.show();
            }
        }
    }

    /// Draws the HUD and `subtitle` (text, RGBA colour on the 0x80 alpha scale).
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, subtitle: Option<(&str, [u8; 4])>) {
        let cw = list.canvas.width;
        let ch = list.canvas.height;
        let bytes = |s: &str| -> Vec<u8> { s.chars().map(|c| c as u32 as u8).collect() };
        // Gauge.
        if self.values.who.is_some() {
            let (x, y, w) = (32.0, 28.0, 150.0);
            list.rect(Rect::new(x - 4.0, y - 4.0, w + 8.0, 36.0), [0.0, 0.0, 0.0, 0.45]);
            let hp = if self.values.health.1 > 0.0 { self.shown.0 / self.values.health.1 } else { 0.0 };
            list.rect(Rect::new(x, y, w * hp.clamp(0.0, 1.0), 12.0), [0.9, 0.2, 0.15, 0.95]);
            list.frame(Rect::new(x, y, w, 12.0), 1.5, [1.0, 1.0, 1.0, 0.8]);
            if let Some((_, max)) = self.values.power {
                let pw = if max > 0.0 { self.shown.1 / max } else { 0.0 };
                list.rect(Rect::new(x, y + 16.0, w * pw.clamp(0.0, 1.0), 8.0), [0.25, 0.55, 1.0, 0.95]);
                list.frame(Rect::new(x, y + 16.0, w, 8.0), 1.5, [1.0, 1.0, 1.0, 0.8]);
            }
        }
        // Coins.
        if self.coins.visible()
            && let Some(f) = font
        {
            let a = self.coins.coverage();
            let text = alloc::format!("{}", self.values.coins);
            let w = f.measure(text.as_bytes(), TEXT_SIZE);
            let r = Rect::new(cw - w - 52.0, 26.0, w + 20.0, f.line_height(TEXT_SIZE) + 6.0);
            list.rect(r, [0.0, 0.0, 0.0, 0.45 * a]);
            list.rect(Rect::new(r.x - 16.0, r.y + 6.0, 12.0, 12.0), [1.0, 0.85, 0.2, a]);
            f.draw(list, text.as_bytes(), r.x + 10.0, r.y + 3.0, TEXT_SIZE, [[1.0, 1.0, 1.0, a]; 2], None);
        }
        // Prompt.
        if let (Some(p), Some(f)) = (&self.prompt, font)
            && self.prompt_screen.visible()
            && !p.text.is_empty()
        {
            let a = self.prompt_screen.coverage();
            let w = (cw * 0.6).min(420.0);
            let x = (cw - w) / 2.0;
            let text = bytes(&p.text);
            let mut probe = DrawList::default();
            let (_, h) = f.draw(&mut probe, &text, 0.0, 0.0, TEXT_SIZE, [[1.0; 4]; 2], Some(w - 24.0));
            let r = Rect::new(x, ch * 0.62, w, h + 16.0);
            list.rect(r, [0.05, 0.05, 0.15, 0.7 * a]);
            list.frame(r, 2.0, [0.9, 0.8, 0.3, a]);
            f.draw(
                list,
                &text,
                x + 12.0,
                r.y + 8.0,
                TEXT_SIZE,
                [[1.0, 1.0, 1.0, a], [1.0, 0.85, 0.3, a]],
                Some(w - 24.0),
            );
        }
        // Subtitle.
        if let (Some((s, c)), Some(f)) = (subtitle, font) {
            let col = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, (c[3] as f32 / 128.0).min(1.0)];
            let w = cw * 0.8;
            let x = (cw - w) / 2.0;
            let text = bytes(s);
            let mut probe = DrawList::default();
            let (_, h) = f.draw(&mut probe, &text, 0.0, 0.0, SUBTITLE_SIZE, [[1.0; 4]; 2], Some(w));
            let y = ch - h - 28.0;
            list.rect(Rect::new(x - 8.0, y - 4.0, w + 16.0, h + 8.0), [0.0, 0.0, 0.0, 0.4]);
            f.draw(list, &text, x, y, SUBTITLE_SIZE, [col, col], Some(w));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_eases_down_and_jumps_up() {
        let mut h = Hud::default();
        let v = |hp| HudValues { who: Some(0), health: (hp, 40.0), power: None, coins: 0 };
        h.update(0.016, v(40.0));
        h.update(0.016, v(20.0));
        assert!(h.shown.0 > 20.0 && h.shown.0 < 40.0);
        for _ in 0..900 {
            h.update(0.016, v(20.0));
        }
        assert_eq!(h.shown.0, 20.0);
        h.update(0.016, v(30.0));
        assert_eq!(h.shown.0, 30.0);
    }

    #[test]
    fn coins_show_on_change_and_hide_after_5_s() {
        let mut h = Hud::default();
        let v = |c| HudValues { coins: c, ..HudValues::default() };
        h.update(0.1, v(5));
        assert!(!h.coins.visible(), "the first value does not show");
        h.update(0.1, v(6));
        assert!(h.coins.visible());
        for _ in 0..70 {
            h.update(0.1, v(6));
        }
        assert!(!h.coins.visible());
    }

    #[test]
    fn prompts_queue_in_one_slot() {
        let mut h = Hud::default();
        h.prompt(810, "a".into(), 1.0);
        h.prompt(811, "b".into(), 1.0);
        h.prompt(810, "a".into(), 2.0);
        assert_eq!(h.prompt.as_ref().unwrap().time, 2.0);
        for _ in 0..40 {
            h.update(0.1, HudValues::default());
        }
        assert_eq!(h.prompt.as_ref().map(|p| p.id), Some(811), "the queued prompt follows");
    }
}
