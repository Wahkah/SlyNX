//! The Sly 3 session's 2D layer (`specs/sly3/user-interface.md`: Sly 3 runs Sly 2's screen runtime [R]), through
//! the same `ui::sly2` machinery as Sly 2's session:
//!
//! - **Fonts**: the kind-8 elements of screen object 1061 (`stock_objects`), read into `ui::sly2::font` glyphs.
//!   A Sly 3 font element is a `u8`, a `u16` count and per character a `u16` code, an `f32` advance and a nested
//!   element; a glyph's kind-1 element is an `s32`, then Sly 2's shape primitives (classes/effects-logic-ui.md 6.1).
//! - **HUD**: the coins (screen 1048's game string 812: `loot/ccoin`), the prompt (1058) and the subtitle (1060):
//!   a line's text (the world string with the line's number) shows from the script's play-line command until its
//!   clear-subtitle command or [`SUBTITLE_TIME`] seconds [working choice: Sly 3 lines do not play through the
//!   sound engine here, so their durations are not at hand].
//! - **Job screens**: job complete (1066) from the job system's notices; job help (1389) from the pause menu.
//! - **Pause menu** (1056): + (Start) during play; Keep Playing, Job Help, Abandon Job, Options, Quit Game. Each
//!   entry shows the world string whose text is the entry's (matched case-insensitively at load), else English.
//! - **Shop** (1065): the safe house's computer opens `safehouse::Shop`; the screen lists its items with their
//!   prices (no Sell page in Sly 3) and buys through [`Sly3Session::buy`].
//! - **Options**: Music Volume (string 119), Effects Volume (120) and vibration, kept in the progress block's
//!   `flags` group as for Sly 2; the music volume goes to the sound engine.
//! - **Job failed** (1067): shown when a script shows screen 1067 (command `0x1A`), with strings 4801 and 659,
//!   until the script hides it or Cross.
//! - **Subtitle colour**: the speaker's, from the class of the object the play-line command runs on
//!   (`audio::sly3::talk::speaker_colour`); white when there is none.
//!
//! The screens' own pictures (element sets and group trees, `formats_sly3::stream::Screen`) are not played, as in
//! Sly 2.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use engine::game::sly2_jobs::{Notice, shop};
use engine::game::systems::FrameInput;
use formats::iso::ReadAt;
use formats::reader::Reader;
use formats_sly2::screen::{Primitive, ShapeVertex};
use formats_sly3::stream::{Body, Record, visit};
use platform_api::Buttons;
use render::{Backend, RenderError};
use ui::draw::{Canvas, DrawList};
use ui::nav::MenuInput;
use ui::sly2::font::GlyphShape;
use ui::sly2::hud::HudValues;
use ui::sly2::options::{OptionsAction, OptionsScreen, Values};
use ui::sly2::pause::{Action, Context, Entry};
use ui::sly2::shop::{Row, ShopAction, ShopScreen};
use ui::sly2::{HelpAction, JobRow};

use super::Sly3Session;

/// The screen object holding the fonts.
pub const FONT_SCREEN: i32 = 1061;
/// Seconds a subtitle stays without a clear command [working choice].
pub const SUBTITLE_TIME: f32 = 6.0;
/// Screen objects: job failed, the shop.
pub const JOB_FAILED_SCREEN: i32 = 1067;
pub const SHOP_SCREEN: i32 = 1065;
/// No "not enough coins" string is known for Sly 3's shop: an ID that no world uses [working choice].
const NOT_ENOUGH: i16 = -1;

/// Reads a glyph's kind-1 element at `at`: `u32` kind, `s32`, `u16` primitive count, then Sly 2's primitives.
fn glyph_shape(data: &[u8], at: usize) -> Option<Vec<Primitive>> {
    let mut r = Reader::at(data, at);
    if r.u32().ok()? != 1 {
        return Some(Vec::new());
    }
    r.i32().ok()?;
    let n = r.u16().ok()? as usize;
    let mut prims = Vec::with_capacity(n);
    for _ in 0..n {
        let k = r.u8().ok()? as usize;
        let settings = [r.u8().ok()?, r.u8().ok()?, r.u8().ok()?];
        let flag = r.u8().ok()?;
        let mut vertices = Vec::with_capacity(k);
        for _ in 0..k {
            let rgba = r.array::<4>().ok()?;
            let extra = r.u8().ok()?;
            let (x, y) = (r.i16().ok()?, r.i16().ok()?);
            vertices.push(ShapeVertex { rgba, extra, x, y });
        }
        prims.push(Primitive { settings, flag, vertices });
    }
    Some(prims)
}

/// The fonts of world `fw`'s object 1061, as for Sly 2: the text font white, the icon font in its colours.
pub fn font_glyphs(fw: &formats_sly3::World<'_>) -> Option<Vec<Vec<GlyphShape>>> {
    let mut fonts: Vec<Vec<GlyphShape>> = Vec::new();
    for b in &fw.bodies {
        visit(&b.objects, &mut |r: &Record| {
            if r.id != FONT_SCREEN || !fonts.is_empty() {
                return;
            }
            let Body::Screen(s) = &r.body else { return };
            for e in s.elements.iter().filter(|e| e.kind == 8) {
                let fi = fonts.len();
                let glyphs = e
                    .nested
                    .iter()
                    .map(|g| {
                        // The advance is the f32 just before the nested element.
                        let at = g.offset as usize;
                        let advance =
                            fw.data.get(at - 4..at).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                        let prims = glyph_shape(fw.data, at).unwrap_or_default();
                        let triangles = crate::sly2::layer::glyph_triangles(&prims, fi == 0);
                        GlyphShape { code: g.index as u32, advance, triangles }
                    })
                    .collect();
                fonts.push(glyphs);
            }
        });
    }
    (!fonts.is_empty()).then_some(fonts)
}

/// The world's strings (language 0), for the screens.
pub fn strings_of(fw: &formats_sly3::World<'_>) -> BTreeMap<i16, String> {
    fw.strings.iter().filter_map(|s| s.text.first().map(|t| (s.id, t.clone()))).collect()
}

/// Text as the font's codes: typographic quotes and dashes as their plain forms, an ellipsis as three dots, other
/// characters beyond the 8-bit range as `?`.
pub fn bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2018}' | '\u{2019}' => out.push(b'\''),
            '\u{201C}' | '\u{201D}' => out.push(b'"'),
            '\u{2013}' | '\u{2014}' => out.push(b'-'),
            '\u{2026}' => out.extend_from_slice(b"..."),
            c if (c as u32) < 256 => out.push(c as u32 as u8),
            _ => out.push(b'?'),
        }
    }
    out
}

/// The menu input of a frame (Cross is B, Circle A, Triangle X, Start +).
pub(super) fn menu_input(i: &FrameInput) -> (MenuInput, bool) {
    let held = |b: Buttons| i.buttons & b.0 != 0;
    let pressed = |b: Buttons| i.pressed & b.0 != 0;
    let m = MenuInput {
        dpad: [held(Buttons::UP), held(Buttons::DOWN), held(Buttons::LEFT), held(Buttons::RIGHT)],
        stick: (i.left[0], i.left[1]),
        accept: pressed(Buttons::B),
        back: pressed(Buttons::X),
        start: pressed(Buttons::PLUS),
        ..MenuInput::default()
    };
    (m, pressed(Buttons::A))
}

impl<B: Backend, R: ReadAt> Sly3Session<B, R> {
    /// Whether a screen holds the game (the pause menu, the job help, the job-complete screen): the session gives
    /// the game no input then.
    pub fn screens_hold(&self) -> bool {
        self.ui.pause.is_open()
            || self.ui.screens.pauses()
            || self.ui.shop.is_some()
            || self.ui.options.is_some()
            || self.ui.grid.open.is_some()
    }

    /// The screens' frame (real time `dt`).
    pub(super) fn update_ui(&mut self, input: &FrameInput, dt: f32) {
        let (menu, circle) = menu_input(input);
        // Job notices.
        let notices = self.jobs.as_mut().map(|j| core::mem::take(&mut j.notices)).unwrap_or_default();
        for n in notices {
            match n {
                Notice::JobComplete { job, screen } => {
                    let title =
                        self.jobs.as_ref().and_then(|j| j.graph.job_ids.get(&job).map(|&i| j.graph.jobs[i].title));
                    self.ui.screens.job_complete(title.unwrap_or(-1), screen);
                }
                // Sly 3 hook (front end): autosaves go to the slot chosen at the title.
                Notice::Autosave if self.front.slot.is_some() => {
                    self.ui.screens.save_started();
                    if let Err(e) = self.save_to_slot() {
                        self.message = Some(e);
                    }
                    self.ui.screens.save_finished();
                }
                _ => {}
            }
        }
        // The subtitle's time.
        if let Some((_, t)) = self.ui.subtitle.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.ui.subtitle = None;
            }
        }
        let values = HudValues { coins: self.coins(), ..HudValues::default() };
        self.ui.hud.update(dt, values);
        if let Some(t) = self.ui.failed.as_mut() {
            *t += dt;
            if menu.accept && *t > 0.3 {
                self.ui.failed = None;
            }
        }
        let help_open = self.ui.screens.help.is_some();
        let other = help_open || self.ui.screens.complete.is_some();
        let others_open = other
            || self.ui.pause.is_open()
            || self.ui.shop.is_some()
            || self.ui.options.is_some()
            || self.shop.is_some();
        if !other && self.update_grid(input, circle, others_open, dt) {
            // The gadget grid has the input.
        } else if !other && self.update_menus(&menu, circle, dt) {
            // The shop or the options page has the input.
        } else if self.ui.pause.is_open() {
            if let Action::Chosen(e) = self.ui.pause.update(dt, &menu) {
                self.pause_choice(e);
            }
        } else if menu.start && !other && self.game.is_some() {
            let c = Context {
                in_episode: self.episode > 0,
                has_job_help: self.jobs.is_some(),
                job_running: self.jobs.as_ref().is_some_and(|j| j.job.is_some()),
                can_save: false,
                can_exit: false,
                has_options: true,
                can_shop: false,
            };
            self.ui.pause.open(c);
        } else {
            let menu = if help_open { menu } else { MenuInput { start: false, ..menu } };
            if let HelpAction::Goals(job) = self.ui.screens.update(dt, &menu, circle && help_open) {
                let goals = self.jobs.as_ref().map(|j| shop::job_goals(j, job)).unwrap_or_default();
                self.ui.screens.show_goals(goals);
            }
        }
        let hold = self.screens_hold();
        if let Some(g) = self.game.as_mut() {
            if hold {
                g.clock.rate = 0.0;
                self.ui.paused = true;
            } else if self.ui.paused {
                g.clock.rate = 1.0;
                self.ui.paused = false;
            }
        }
    }

    fn pause_choice(&mut self, e: Entry) {
        match e {
            Entry::JobHelp => {
                if let Some(j) = self.jobs.as_ref() {
                    let rows = shop::job_help(j)
                        .into_iter()
                        .map(|r| JobRow { job: r.job, title: r.title, done: r.done })
                        .collect();
                    self.ui.screens.open_help(rows);
                }
            }
            Entry::AbandonJob => {
                if let Some(j) = self.jobs.as_mut()
                    && let Some(job) = j.job
                {
                    j.end_job(job);
                }
            }
            Entry::QuitGame => self.ui.quit = true,
            Entry::Options => self.open_options(),
            _ => {}
        }
    }

    /// Shows a dialogue line's subtitle (the world string with the line's number), in the colour of the speaker:
    /// the class of `object`, the object the command runs on.
    pub(super) fn show_line(&mut self, line: i32, object: i32) {
        if let Some(t) = i16::try_from(line).ok().and_then(|l| self.ui.strings.get(&l)) {
            // As the font's codes (quotes and dashes in their plain forms).
            let text: String = bytes(t).into_iter().map(char::from).collect();
            let class = self.game.as_ref().and_then(|g| {
                let h = (object >= 0).then(|| g.world.find_sly3_id(object, None)).flatten()?;
                g.world.class(h)
            });
            let colour = class.map_or([0xFF, 0xFF, 0xFF, 0x80], audio::sly3::talk::speaker_colour);
            self.ui.subtitle = Some((text, SUBTITLE_TIME));
            self.ui.subtitle_colour = colour;
        }
    }

    /// Opens the options page (as the pause menu's Options does).
    pub fn open_options(&mut self) {
        self.ui.options = Some(OptionsScreen::open(self.stored_options()));
    }

    /// Which page is open: the shop (with its rows), the options, job failed.
    pub fn menus_open(&self) -> (Option<usize>, bool, bool) {
        (self.ui.shop.as_ref().map(|s| s.rows.len()), self.ui.options.is_some(), self.ui.failed.is_some())
    }

    /// A script showed or hid a screen (command `0x1A` / `0x1B`): job failed (1067) is drawn.
    pub fn screen_request(&mut self, screen: i32, on: bool) {
        if screen == JOB_FAILED_SCREEN {
            self.ui.failed = on.then_some(0.0);
        }
    }

    /// The shop and options pages' frame; returns whether one is open.
    fn update_menus(&mut self, menu: &MenuInput, circle: bool, dt: f32) -> bool {
        // The safe house's computer opened the shop: show it.
        if self.shop.is_some() && self.ui.shop.is_none() {
            let rows = self.shop_rows();
            self.ui.shop = Some(ShopScreen::open(SHOP_SCREEN, rows, self.coins(), false));
        }
        if let Some(mut s) = self.ui.shop.take() {
            match s.update(dt, menu, circle) {
                ShopAction::Buy(item) => {
                    if self.buy(item).is_err() {
                        s.say(self.text_or(NOT_ENOUGH, "Not enough coins"));
                    }
                    let rows = self.shop_rows();
                    s.set_rows(rows, self.coins());
                }
                ShopAction::Closed => {
                    self.shop = None;
                    return self.ui.options.is_some();
                }
                _ => {}
            }
            self.ui.shop = Some(s);
            return true;
        }
        if let Some(o) = self.ui.options.as_mut() {
            match o.update(dt, menu, circle) {
                OptionsAction::Changed(v) => self.store_options(v),
                OptionsAction::Closed => self.ui.options = None,
                OptionsAction::None => {}
            }
            return true;
        }
        false
    }

    fn text_or(&self, id: i16, fallback: &str) -> String {
        self.ui.strings.get(&id).cloned().unwrap_or_else(|| String::from(fallback))
    }

    fn shop_rows(&self) -> Vec<Row> {
        let Some(shop) = self.shop.as_ref() else { return Vec::new() };
        shop.items
            .iter()
            .map(|(i, it)| Row { id: *i, text: self.text_or(it.name as i16, "?"), amount: it.price })
            .collect()
    }

    /// The stored options (as for Sly 2: both volumes 0 means never set).
    fn stored_options(&self) -> Values {
        let Some(p) = self.progress() else { return Values::default() };
        let (m, e) = (p.float(&["flags", "uMusicVolume"]), p.float(&["flags", "uEffectsVolume"]));
        if m == 0.0 && e == 0.0 {
            return Values::default();
        }
        Values { music: m, effects: e, vibration: p.int(&["flags", "fNoVibration"]) == 0 }
    }

    fn store_options(&mut self, v: Values) {
        if let Some(p) = self.progress_mut() {
            p.set_named(&["flags", "uMusicVolume"], None, save::sly2::SaveValue::Float(v.music));
            p.set_named(&["flags", "uEffectsVolume"], None, save::sly2::SaveValue::Float(v.effects));
            p.set_named(&["flags", "fNoVibration"], None, save::sly2::SaveValue::Flag(!v.vibration));
        }
        if let Some(s) = self.sound.as_mut() {
            s.set_music_volume(v.music);
        }
    }

    /// The pause menu's entry texts: the world string with the entry's text (any case), else the English.
    fn pause_label(&self, e: Entry) -> Vec<u8> {
        let want = e.fallback();
        let found = self.ui.strings.values().find(|s| s.trim_end_matches("...").eq_ignore_ascii_case(want));
        bytes(found.map_or(want, |s| s.as_str()))
    }

    /// The frame's UI draw list for a display of `width` × `height` pixels.
    pub fn ui_draw_list(&self, width: u32, height: u32) -> DrawList {
        let mut list = DrawList::new(Canvas::for_display(width, height));
        let font = self.ui.font.as_ref();
        // Sly 3 hook (front end): a movie shows alone, with black around it; the title and the episode menu show
        // over the world without the HUD.
        if self.movie_playing() {
            crate::movie::movie_bars(&mut list, width, height, self.front.movies.rect());
            return list;
        }
        if let Some(m) = self.front.menu.as_ref().filter(|m| m.visible()) {
            m.draw(&mut list, font);
            return list;
        }
        let text = |id: i16| self.ui.strings.get(&id).cloned();
        let sub = self.ui.subtitle.as_ref().map(|(s, _)| (s.as_str(), self.ui.subtitle_colour));
        // The HUD hides while a full screen (gadget grid, shop, options) is open.
        if self.ui.grid.open.is_none() && self.ui.shop.is_none() && self.ui.options.is_none() {
            self.ui.hud.draw(&mut list, font, sub);
        }
        self.ui.screens.draw(&mut list, font, &text);
        if let (Some(_), Some(f)) = (self.ui.failed, font) {
            // Job failed: its two strings, centred.
            let (cw, ch) = (list.canvas.width, list.canvas.height);
            list.rect(ui::draw::Rect::new(cw * 0.2, ch * 0.38, cw * 0.6, ch * 0.24), [0.15, 0.0, 0.0, 0.85]);
            for (k, (id, fb)) in [(4801i16, "Job Failed!"), (659, "Press X to try again.")].into_iter().enumerate() {
                let t = bytes(&self.text_or(id, fb));
                let size = if k == 0 { 28.0 } else { 18.0 };
                let w = f.measure(&t, size);
                f.draw(
                    &mut list,
                    &t,
                    (cw - w) / 2.0,
                    ch * 0.41 + 44.0 * k as f32,
                    size,
                    [[1.0, 0.9, 0.8, 1.0]; 2],
                    None,
                );
            }
        }
        self.ui.pause.draw(&mut list, font, &|e| self.pause_label(e));
        if let Some(s) = self.ui.shop.as_ref() {
            let labels = [
                bytes(&self.text_or(663, "ThiefNet")),
                bytes(&self.text_or(5551, "Buy Gadgets")),
                bytes(""),
                bytes("Buy it? (X)"),
                bytes("Coins"),
            ];
            s.draw(&mut list, font, [&labels[0], &labels[1], &labels[2], &labels[3], &labels[4]]);
        }
        if let (Some((_, g)), Some(view)) = (self.ui.grid.open.as_ref(), self.grid_view()) {
            g.draw(&mut list, font, &view);
        }
        if let Some(o) = self.ui.options.as_ref() {
            let labels = [
                bytes(&self.text_or(119, "Music Volume")),
                bytes(&self.text_or(120, "Effects Volume")),
                bytes("Vibration"),
                bytes("On"),
                bytes("Off"),
            ];
            o.draw(&mut list, font, [&labels[0], &labels[1], &labels[2], &labels[3], &labels[4]]);
        }
        list
    }

    /// Draws the UI through the 2D layer (before the frame is rendered).
    pub(super) fn draw_ui(&mut self) -> Result<(), RenderError> {
        self.front.movies.present(&mut self.renderer)?; // Sly 3 hook: the movie's picture.
        let Some(mut layer) = self.ui.layer.take() else { return Ok(()) };
        let (w, h) = self.renderer.backend().target_size();
        let list = self.ui_draw_list(w, h);
        let r = layer.draw(&mut self.renderer, &list);
        self.ui.layer = Some(layer);
        r
    }

    /// The glyph sheet of the Sly 3 fonts, once a world with them has loaded.
    pub fn font_sheet(&self) -> Option<&ui::sly2::font::FontSheet> {
        self.ui.font.as_ref()
    }

    /// Whether the 2D layer exists.
    pub fn has_ui_layer(&self) -> bool {
        self.ui.layer.is_some()
    }
}

impl Sly3Ui {
    /// The UI state with the gadget grids.
    pub fn with_grid(grid: super::grid::GridState) -> Sly3Ui {
        Sly3Ui { grid, ..Sly3Ui::default() }
    }
}

/// The session's UI state.
#[derive(Default)]
pub struct Sly3Ui {
    pub font: Option<ui::sly2::font::FontSheet>,
    pub layer: Option<crate::ui_layer::UiLayer>,
    pub hud: ui::sly2::hud::Hud,
    pub pause: ui::sly2::pause::PauseMenu,
    pub screens: ui::sly2::JobScreens,
    /// The world's strings.
    pub strings: BTreeMap<i16, String>,
    /// The subtitle showing and its seconds left.
    pub subtitle: Option<(String, f32)>,
    /// Its speaker's colour (RGBA, alpha on the 0x80 scale).
    pub subtitle_colour: [u8; 4],
    /// Job failed (1067) showing, for this long.
    pub failed: Option<f32>,
    pub shop: Option<ShopScreen>,
    pub options: Option<OptionsScreen>,
    /// Sly 3 hook (gadget grids): the grids and the open assignment screen.
    pub grid: super::grid::GridState,
    paused: bool,
    /// Quit Game was chosen.
    pub quit: bool,
}
