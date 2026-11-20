//! The Sly 2 session's menus beyond the pause menu (`ui::sly2`; safehouse-and-shop.md 3 and 6, user-interface.md
//! 6.2):
//!
//! - **Gadget assignment** (Select during play, objects 885 to 887): the current character's grid from the
//!   executable (`engine::gang::grid_from_executable`) run by `engine::gang::Assignment`; the stick or d-pad moves to
//!   the nearest owned cell, L1, L2 and R2 (the gang's L, ZL and R) put the selected item on that slot, Select or
//!   Circle closes. On the Switch, − is Select.
//! - **Shop** (object 888): the Buy page lists the open items not owned (`sly2_jobs::shop::buy_list`), the Sell page
//!   the loot held; a purchase pays, sets the owned flag in the progress block and gives the item to the gang. The
//!   game opens it at the safe house's computer, which SlyNX does not play yet; until then the pause menu offers it
//!   in a hub [working choice].
//! - **Options** (the pause menu's Options): Music Volume, Effects Volume and Vibrations, kept in the progress
//!   block's `flags` group (`uMusicVolume`, `uEffectsVolume`, `fNoVibration`); the music volume goes to the sound
//!   engine. A block whose two volumes are both 0 has never had them set and reads the defaults (full, vibration on)
//!   [working choice].

use alloc::string::String;
use alloc::vec::Vec;

use engine::game::sly2_jobs::{self, shop};
use engine::game::systems::FrameInput;
use engine::gang::Who;
use engine::gang::gadgets::{AssignMessage, Assignment, GridCell};
use formats::iso::ReadAt;
use platform_api::Buttons;
use render::Backend;
use save::sly2::SaveValue;
use ui::nav::MenuInput;
use ui::sly2::grid::{Cell, GridScreen, GridView};
use ui::sly2::options::{OptionsAction, OptionsScreen, Values};
use ui::sly2::shop::{Page, Row, ShopAction, ShopScreen};

use super::Sly2Session;

/// The grids' screen objects: Sly, Bentley, Murray (user-interface.md 6.3).
pub const GRID_SCREENS: [i32; 3] = [885, 886, 887];
/// The shop's screen object.
pub const SHOP_SCREEN: i32 = 888;

/// The open menu.
#[derive(Default)]
pub struct Menus {
    /// The executable's grids: Sly, Bentley, Murray.
    pub grids: [Vec<GridCell>; 3],
    pub loot: Option<engine::gang::LootTable>,
    pub assignment: Option<(Assignment, GridScreen)>,
    pub shop: Option<ShopScreen>,
    pub options: Option<OptionsScreen>,
    /// What the grid view shows of the gang (it lives behind the player system): owned items and the slots.
    pub owned: engine::gang::Owned,
    pub slots: [Option<u8>; 3],
    /// The grids' art from their screens (icons, the title's place), once the fonts are read.
    pub art: [Option<super::layer::GridArt>; 3],
}

impl Menus {
    /// Whether a menu holds the game.
    pub fn open(&self) -> bool {
        self.assignment.is_some() || self.shop.is_some() || self.options.is_some()
    }
}

fn who_index(w: Who) -> usize {
    match w {
        Who::Sly => 0,
        Who::Bentley => 1,
        Who::Murray => 2,
    }
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// Which menu is open: the gadget grid, the shop (with its rows), the options page.
    pub fn menus_open(&self) -> (bool, Option<usize>, bool) {
        (self.menus.assignment.is_some(), self.menus.shop.as_ref().map(|s| s.rows.len()), self.menus.options.is_some())
    }

    fn string_or(&self, id: i32, fallback: &str) -> String {
        self.sound.as_ref().and_then(|s| s.string(id as i16)).unwrap_or_else(|| String::from(fallback))
    }

    /// The menus' frame; returns whether a menu is open (the pause menu and the job screens wait meanwhile).
    pub(super) fn update_menus(&mut self, input: &FrameInput, menu: &MenuInput, circle: bool, dt: f32) -> bool {
        let pressed = |b: Buttons| input.pressed & b.0 != 0;
        // Select opens the assignment screen during play.
        if !self.menus.open() && !self.pause.is_open() && pressed(Buttons::MINUS) {
            self.open_assignment();
            return self.menus.open();
        }
        if self.menus.assignment.is_some() {
            self.step_assignment(input, menu, circle || pressed(Buttons::MINUS), dt);
        } else if self.menus.shop.is_some() {
            self.step_shop(menu, circle, dt);
        } else if let Some(o) = self.menus.options.as_mut() {
            match o.update(dt, menu, circle) {
                OptionsAction::Changed(v) => self.store_options(v),
                OptionsAction::Closed => self.menus.options = None,
                OptionsAction::None => {}
            }
        }
        self.menus.open()
    }

    fn open_assignment(&mut self) {
        let Some(g) = self.game.as_mut() else { return };
        let Some(gang) = g.systems.player.gang_mut() else { return };
        let who = gang.player;
        let cells = self.menus.grids[who_index(who)].clone();
        if cells.is_empty() {
            return;
        }
        let a = Assignment::open(who, cells, &gang.shared.owned);
        self.menus.assignment = Some((a, GridScreen::open(GRID_SCREENS[who_index(who)])));
    }

    fn step_assignment(&mut self, input: &FrameInput, menu: &MenuInput, close: bool, dt: f32) {
        let Some((mut a, mut screen)) = self.menus.assignment.take() else { return };
        screen.update(dt);
        if close {
            screen.screen.hide();
        }
        let pressed = |b: Buttons| input.pressed & b.0 != 0;
        let mut message = None;
        if let Some(g) = self.game.as_mut()
            && let Some(gang) = g.systems.player.gang_mut()
            && matches!(screen.screen.phase, ui::sly2::Phase::Idle(_))
        {
            // A d-pad press, or the stick past halfway (y down on the screen).
            let mut dir = [0.0f32, 0.0];
            for (b, d) in [
                (Buttons::LEFT, [-1.0, 0.0]),
                (Buttons::RIGHT, [1.0, 0.0]),
                (Buttons::UP, [0.0, -1.0]),
                (Buttons::DOWN, [0.0, 1.0]),
            ] {
                if pressed(b) {
                    dir = d;
                }
            }
            let _ = menu;
            a.step(dir, &gang.shared.owned);
            for (b, slot) in [(Buttons::L, 0usize), (Buttons::ZL, 1), (Buttons::R, 2)] {
                if pressed(b) {
                    let (items, owned) = (gang.shared.items.clone(), gang.shared.owned);
                    if let Some(c) = gang.get_mut(a.who) {
                        message = match a.assign(slot, &mut c.slots, &items, &owned) {
                            AssignMessage::Assigned => None,
                            // The world's strings 50 and 51.
                            AssignMessage::NotOwned => Some((50, "Not owned")),
                            AssignMessage::NotAssignable => Some((51, "Can't be put on a button")),
                        };
                    }
                }
            }
        }
        if let Some((id, fallback)) = message {
            screen.say(self.string_or(id, fallback));
        }
        if screen.screen.visible() {
            self.menus.assignment = Some((a, screen));
        }
    }

    /// The assignment screen's view.
    pub(super) fn grid_view(&self) -> Option<GridView> {
        let (a, _) = self.menus.assignment.as_ref()?;
        let name = |item: i32| -> String {
            let id = self.items.as_ref().and_then(|t| t.get(item as u8)).map_or(-1, |i| i.name);
            self.string_or(id, "")
        };
        let slots: [Option<String>; 3] = self.menus.slots.map(|s| s.map(|i| name(i as i32)));
        let art = self.menus.art[who_index(a.who)].as_ref();
        let cells = a
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| Cell {
                position: c.position,
                owned: self.menus.owned.has(c.item as u8),
                assignable: c.assignable(),
                name: name(c.item),
                slot: self.menus.slots.iter().position(|s| *s == Some(c.item as u8)),
                icon: art.and_then(|g| g.icons.get(i).copied().flatten()),
            })
            .collect();
        Some(GridView {
            title: self.string_or(3151, "Gadget Grid"),
            title_at: art.and_then(|g| g.title),
            cells,
            selected: a.selected,
            slots,
        })
    }

    /// Copies what the grid view needs from the gang (it lives behind the player system).
    pub(super) fn refresh_gang_view(&mut self) {
        let Some(g) = self.game.as_mut() else { return };
        let Some(gang) = g.systems.player.gang_mut() else { return };
        self.menus.owned = gang.shared.owned;
        let who = self.menus.assignment.as_ref().map_or(gang.player, |(a, _)| a.who);
        self.menus.slots = gang.get_mut(who).map_or([None; 3], |c| c.slots.0);
    }

    /// Opens the shop on its Buy page.
    pub fn open_shop(&mut self) {
        let rows = self.shop_rows(Page::Buy);
        let coins = self.progress().map_or(0, shop::coins);
        self.menus.shop = Some(ShopScreen::open(SHOP_SCREEN, rows, coins, true));
    }

    fn shop_rows(&self, page: Page) -> Vec<Row> {
        let (Some(p), Some(items)) = (self.progress(), self.items.as_ref()) else { return Vec::new() };
        match page {
            Page::Buy => shop::buy_list(p, items)
                .into_iter()
                .filter_map(|n| items.get(n).map(|i| (n, i.name, i.price)))
                .map(|(n, name, price)| Row { id: n as usize, text: self.string_or(name, "?"), amount: price })
                .collect(),
            Page::Sell => match self.menus.loot.as_ref() {
                Some(t) => shop::sell_list(p, t)
                    .into_iter()
                    .map(|k| Row {
                        id: k,
                        text: alloc::format!("{} x{}", self.string_or(t.records[k].name, "?"), shop::loot(p, k)),
                        amount: t.records[k].value,
                    })
                    .collect(),
                None => Vec::new(),
            },
        }
    }

    fn step_shop(&mut self, menu: &MenuInput, circle: bool, dt: f32) {
        let Some(mut s) = self.menus.shop.take() else { return };
        let action = s.update(dt, menu, circle);
        let page = s.page;
        match action {
            ShopAction::Buy(n) => {
                let items = self.items.clone();
                let mut refused = false;
                if let (Some(g), Some(items)) = (self.game.as_mut(), items.as_ref()) {
                    let ok = sly2_jobs::with(&mut g.world, |j, _| shop::buy(j, items, n as u8)).unwrap_or(Ok(()));
                    match ok {
                        Ok(()) => {
                            if let Some(gang) = g.systems.player.gang_mut() {
                                gang.shared.owned.add(n as u8);
                            }
                        }
                        Err(_) => refused = true,
                    }
                }
                if refused {
                    s.say(String::from("Not enough coins"));
                }
                let coins = self.progress().map_or(0, shop::coins);
                s.set_rows(self.shop_rows(page), coins);
            }
            ShopAction::Sell(k) => {
                let loot = self.menus.loot.clone();
                if let (Some(g), Some(t)) = (self.game.as_mut(), loot.as_ref()) {
                    sly2_jobs::with(&mut g.world, |j, _| shop::sell(&mut j.progress, t, k));
                }
                let coins = self.progress().map_or(0, shop::coins);
                s.set_rows(self.shop_rows(page), coins);
            }
            ShopAction::Page(p) => {
                let coins = self.progress().map_or(0, shop::coins);
                s.set_rows(self.shop_rows(p), coins);
            }
            ShopAction::Closed => return,
            ShopAction::None => {}
        }
        self.menus.shop = Some(s);
    }

    /// The stored options.
    pub(super) fn stored_options(&self) -> Values {
        let Some(p) = self.progress() else { return Values::default() };
        let (m, e) = (p.float(&["flags", "uMusicVolume"]), p.float(&["flags", "uEffectsVolume"]));
        if m == 0.0 && e == 0.0 {
            return Values::default();
        }
        Values { music: m, effects: e, vibration: p.int(&["flags", "fNoVibration"]) == 0 }
    }

    /// Opens the options page.
    pub fn open_options(&mut self) {
        self.menus.options = Some(OptionsScreen::open(self.stored_options()));
    }

    fn store_options(&mut self, v: Values) {
        if let Some(g) = self.game.as_mut() {
            sly2_jobs::with(&mut g.world, |j, _| {
                let p = &mut j.progress;
                p.set_named(&["flags", "uMusicVolume"], None, SaveValue::Float(v.music));
                p.set_named(&["flags", "uEffectsVolume"], None, SaveValue::Float(v.effects));
                p.set_named(&["flags", "fNoVibration"], None, SaveValue::Flag(!v.vibration));
            });
        }
        if let Some(s) = self.sound.as_mut() {
            s.engine.set_music_volume(v.music);
        }
    }
}
