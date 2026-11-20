//! The safe-house shop screen (Sly 2 object 888, Sly 3 object 1065; safehouse-and-shop.md 3): a Buy page (items with
//! their prices) and, in Sly 2, a Sell page (loot held with its value), switched with Triangle. Cross on a Buy row
//! asks for confirmation and a second Cross buys; Circle cancels the question, and Circle on the page closes the
//! shop. Cross on a Sell row sells one at once. A refusal shows a message for 5 s. Six rows show at a time. The game
//! pauses while the shop is open.

use alloc::string::String;
use alloc::vec::Vec;

use super::font::FontSheet;
use super::jobs::{Phase, Screen};
use crate::draw::{DrawList, Rect};
use crate::nav::{Dir, MenuInput, Repeat};

/// Rows shown at a time.
pub const ROWS: usize = 6;
/// Seconds a message stays.
pub const MESSAGE_TIME: f32 = 5.0;
/// Text size [working choice].
pub const SIZE: f32 = 18.0;

/// A row: its text and its price or value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// What the session acts on (an item record, a loot kind).
    pub id: usize,
    pub text: String,
    pub amount: i32,
}

/// The pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Buy,
    Sell,
}

/// What the screen asks of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopAction {
    None,
    /// Buy this row's item (after the confirmation).
    Buy(usize),
    /// Sell one of this row's loot.
    Sell(usize),
    /// The page changed: give its rows with [`ShopScreen::set_rows`].
    Page(Page),
    Closed,
}

/// The shop screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ShopScreen {
    pub page: Page,
    pub rows: Vec<Row>,
    pub cursor: usize,
    /// Asking to confirm the purchase of the row at the cursor.
    pub asking: bool,
    /// A message and its seconds left.
    pub message: Option<(String, f32)>,
    /// The coins shown.
    pub coins: i32,
    /// A Sell page exists (Sly 2; the Sly 3 shop has none).
    pub can_sell: bool,
    pub screen: Screen,
    repeat: Repeat,
}

impl ShopScreen {
    /// Opens on the Buy page with `rows`.
    pub fn open(object: i32, rows: Vec<Row>, coins: i32, can_sell: bool) -> ShopScreen {
        let mut screen = Screen::new(object);
        screen.show();
        ShopScreen {
            page: Page::Buy,
            rows,
            cursor: 0,
            asking: false,
            message: None,
            coins,
            can_sell,
            screen,
            repeat: Repeat::default(),
        }
    }

    /// Replaces the rows (after a purchase, a sale or a page change), keeping the cursor in range.
    pub fn set_rows(&mut self, rows: Vec<Row>, coins: i32) {
        self.rows = rows;
        self.coins = coins;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.asking = false;
    }

    /// Shows a message (not enough coins) for 5 s.
    pub fn say(&mut self, text: String) {
        self.message = Some((text, MESSAGE_TIME));
        self.asking = false;
    }

    /// Whether it is still on screen.
    pub fn visible(&self) -> bool {
        self.screen.visible()
    }

    /// Advances on the real clock. `circle`: Circle went down.
    pub fn update(&mut self, dt: f32, input: &MenuInput, circle: bool) -> ShopAction {
        self.screen.update(dt);
        if let Some((_, t)) = self.message.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
        if !matches!(self.screen.phase, Phase::Idle(_)) {
            return if self.screen.visible() { ShopAction::None } else { ShopAction::Closed };
        }
        if circle {
            if self.asking {
                self.asking = false;
            } else {
                self.screen.hide();
            }
            return ShopAction::None;
        }
        if input.back && self.can_sell && !self.asking {
            self.page = if self.page == Page::Buy { Page::Sell } else { Page::Buy };
            self.cursor = 0;
            return ShopAction::Page(self.page);
        }
        match self.repeat.update(input, dt) {
            Some(Dir::Up) if self.cursor > 0 && !self.asking => self.cursor -= 1,
            Some(Dir::Down) if self.cursor + 1 < self.rows.len() && !self.asking => self.cursor += 1,
            _ => {}
        }
        if input.accept
            && let Some(r) = self.rows.get(self.cursor)
        {
            match self.page {
                Page::Buy if !self.asking => self.asking = true,
                Page::Buy => {
                    self.asking = false;
                    return ShopAction::Buy(r.id);
                }
                Page::Sell => return ShopAction::Sell(r.id),
            }
        }
        ShopAction::None
    }

    /// Draws the shop; `labels` are (title, buy page, sell page, confirm question, coins) texts.
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, labels: [&[u8]; 5]) {
        if !self.screen.visible() {
            return;
        }
        let a = self.screen.coverage();
        let (cw, ch) = (list.canvas.width, list.canvas.height);
        let panel = Rect::new(cw * 0.12, ch * 0.12, cw * 0.76, ch * 0.76);
        list.rect(Rect::new(0.0, 0.0, cw, ch), [0.0, 0.0, 0.0, 0.4 * a]);
        list.rect(panel, [0.02, 0.1, 0.08, 0.92 * a]);
        list.frame(panel, 3.0, [0.3, 0.9, 0.5, a]);
        let Some(f) = font else { return };
        let white = [[1.0, 1.0, 1.0, a], [1.0, 0.85, 0.3, a]];
        let green = [[0.5, 1.0, 0.6, a]; 2];
        f.draw(list, labels[0], panel.x + 20.0, panel.y + 12.0, SIZE + 4.0, green, None);
        let page = if self.page == Page::Buy { labels[1] } else { labels[2] };
        f.draw(list, page, panel.x + 20.0, panel.y + 44.0, SIZE, white, None);
        let coins = [labels[4], b": ", alloc::format!("{}", self.coins).as_bytes()].concat();
        let w = f.measure(&coins, SIZE);
        f.draw(list, &coins, panel.right() - w - 20.0, panel.y + 44.0, SIZE, white, None);
        let row_h = f.line_height(SIZE) + 6.0;
        let first = self.cursor.saturating_sub(ROWS - 1);
        for (k, r) in self.rows.iter().enumerate().skip(first).take(ROWS) {
            let y = panel.y + 84.0 + row_h * (k - first) as f32;
            if k == self.cursor {
                list.rect(Rect::new(panel.x + 12.0, y - 2.0, panel.w - 24.0, row_h), [0.3, 0.9, 0.5, 0.25 * a]);
            }
            let text: Vec<u8> = r.text.chars().map(|c| c as u32 as u8).collect();
            f.draw(list, &text, panel.x + 24.0, y, SIZE, white, Some(panel.w * 0.6));
            let amount = alloc::format!("{}", r.amount);
            let w = f.measure(amount.as_bytes(), SIZE);
            f.draw(list, amount.as_bytes(), panel.right() - w - 24.0, y, SIZE, white, None);
        }
        let bottom = panel.bottom() - row_h - 12.0;
        if self.asking {
            f.draw(list, labels[3], panel.x + 24.0, bottom, SIZE, white, Some(panel.w - 48.0));
        } else if let Some((m, _)) = &self.message {
            let text: Vec<u8> = m.chars().map(|c| c as u32 as u8).collect();
            f.draw(list, &text, panel.x + 24.0, bottom, SIZE, [[1.0, 0.5, 0.4, a]; 2], Some(panel.w - 48.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn rows() -> Vec<Row> {
        vec![
            Row { id: 7, text: "Trigger Bomb".into(), amount: 200 },
            Row { id: 8, text: "Fishing Pole".into(), amount: 300 },
        ]
    }

    fn idle(s: &mut ShopScreen) {
        for _ in 0..60 {
            s.update(1.0 / 60.0, &MenuInput::default(), false);
        }
    }

    #[test]
    fn buying_asks_first() {
        let mut s = ShopScreen::open(888, rows(), 500, true);
        idle(&mut s);
        let cross = MenuInput { accept: true, ..MenuInput::default() };
        assert_eq!(s.update(0.016, &cross, false), ShopAction::None);
        assert!(s.asking);
        // Circle cancels the question, not the shop.
        s.update(0.016, &MenuInput::default(), true);
        assert!(!s.asking && s.visible());
        s.update(0.016, &cross, false);
        assert_eq!(s.update(0.016, &cross, false), ShopAction::Buy(7));
        // Triangle turns to the Sell page; Cross sells at once.
        assert_eq!(
            s.update(0.016, &MenuInput { back: true, ..MenuInput::default() }, false),
            ShopAction::Page(Page::Sell)
        );
        s.set_rows(vec![Row { id: 3, text: "Bronze Ring".into(), amount: 17 }], 300);
        assert_eq!(s.update(0.016, &cross, false), ShopAction::Sell(3));
        // Circle on the page closes the shop.
        s.update(0.016, &MenuInput::default(), true);
        let mut last = ShopAction::None;
        for _ in 0..60 {
            last = s.update(1.0 / 60.0, &MenuInput::default(), false);
        }
        assert_eq!(last, ShopAction::Closed);
    }

    #[test]
    fn messages_last_five_seconds() {
        let mut s = ShopScreen::open(1065, rows(), 0, false);
        s.say("Not enough coins".into());
        for _ in 0..290 {
            s.update(1.0 / 60.0, &MenuInput::default(), false);
        }
        assert!(s.message.is_some());
        for _ in 0..20 {
            s.update(1.0 / 60.0, &MenuInput::default(), false);
        }
        assert!(s.message.is_none());
        // No Sell page in Sly 3.
        assert_eq!(s.update(0.016, &MenuInput { back: true, ..MenuInput::default() }, false), ShopAction::None);
    }
}
