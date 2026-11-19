//! The HUD, the menus, subtitles and the screen wipes (the `ui` crate), drawn through [`UiLayer`].
//!
//! - **HUD** from the progress: lives, coins, the episode's keys, the level's clue bottles.
//! - **Pause** (+): the pause menu of ui-and-menus.md 4 with its [`PauseContext`]; the game clock stops while it is
//!   open. **Title** at the start sequence.
//! - **Subtitles** of the playing dialogue, typed out as the engine reveals them.
//! - **Wipes** (level-flow.md 5): the screen goes to black over the wipe's "out" time, the next world loads, and it
//!   comes back over the "in" time. All four kinds are drawn as fades for now (the iris shape of the screen mask is
//!   not drawn yet).
//! - **Screen effects**: lightning tints and the letterbox bars.
//! - **Save slots**: the title's and the options' slot lists act through the session's saving; the auto-save icon
//!   shows after each save.
//! - **Movies**: black bars around the picture (or a title card without a video layer).

use alloc::vec::Vec;

use engine::game::progress::Progress;
use engine::world::{WorldInfo, shared::ScreenEffects};
use platform_api::{ControllerKind, PadState, Stick};
use ui::FontSet;
use ui::draw::{Canvas, DrawList, Rect};
use ui::hud::{Hud, HudValues};
use ui::menu::{Action, Menus, PauseContext};
use ui::nav::{ButtonMap, MenuInput};
use ui::tables::TextTables;
use ui::text::{Align, TextStyle, layout};

use engine::game::systems::FrameInput;

/// Wipe durations, seconds: (out, in) per kind (level-flow.md 5).
pub fn wipe_times(kind: u8) -> (f32, f32) {
    match kind {
        1 => (1.5, 1.0),
        _ => (0.5, 0.25),
    }
}

/// A screen wipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wipe {
    /// Going to black; then the pending load runs.
    Out {
        /// Elapsed.
        t: f32,
        /// Kind.
        kind: u8,
    },
    /// Coming back from black.
    In {
        /// Elapsed.
        t: f32,
        /// Kind.
        kind: u8,
    },
}

impl Wipe {
    /// Black's opacity now.
    pub fn alpha(&self) -> f32 {
        match *self {
            Wipe::Out { t, kind } => (t / wipe_times(kind).0).clamp(0.0, 1.0),
            Wipe::In { t, kind } => 1.0 - (t / wipe_times(kind).1).clamp(0.0, 1.0),
        }
    }
}

/// What the frontend asks of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontAction {
    /// Close the menus and resume.
    Resume,
    /// Title: start the game (`resume`: continue the save loaded at the title).
    Start {
        /// A save is loaded.
        resume: bool,
    },
    /// A new game in a save slot.
    NewGame(usize),
    /// Load a save slot.
    LoadGame(usize),
    /// Save to a slot.
    Save(usize),
    /// Erase a slot.
    Erase(usize),
    /// Restart the level.
    RestartLevel,
    /// Leave through the default exit.
    ExitLevel,
    /// Go to the hideout.
    ExitToHideout,
    /// The auto-save notice (prompt 20) was answered.
    AutosaveNotice,
    /// Quit (saving first when `save`).
    Quit {
        /// Save and quit.
        save: bool,
    },
}

/// The frontend.
pub struct Frontend {
    /// Text tables from the executable.
    pub tables: TextTables,
    /// The HUD.
    pub hud: Hud,
    /// The menus.
    pub menus: Menus,
    /// The current world's fonts.
    pub fonts: FontSet,
    /// The draw list of the last frame.
    pub list: DrawList,
    map: ButtonMap,
}

/// A platform pad from the engine's input (for the menu navigation).
pub fn pad_of(i: &FrameInput) -> PadState {
    PadState {
        kind: ControllerKind::Handheld,
        held: platform_api::Buttons(i.buttons),
        pressed: platform_api::Buttons(i.pressed),
        released: platform_api::Buttons(i.released),
        left: Stick { x: i.left[0], y: i.left[1] },
        right: Stick { x: i.right[0], y: i.right[1] },
    }
}

impl Frontend {
    /// A frontend with the executable's tables and HUD placements.
    pub fn new(exe: &formats::disc::Executable) -> Option<Frontend> {
        let tables = TextTables::read(exe).ok()?;
        let placements = ui::tables::read_placements(exe).ok()?;
        Some(Frontend {
            tables,
            hud: Hud::new(placements, HudValues { lives: 5, ..Default::default() }),
            menus: Menus::default(),
            fonts: FontSet::default(),
            list: DrawList::new(Canvas::for_display(1280, 720)),
            map: ButtonMap::default(),
        })
    }

    /// Whether a menu is open (the game is paused).
    pub fn menu_open(&self) -> bool {
        self.menus.screen().is_some()
    }

    /// Opens the pause menu.
    pub fn pause(&mut self, info: &WorldInfo, name: &str, progress: &Progress, slot_in_use: bool) {
        let episode = info.id >> 8;
        let ctx = PauseContext {
            world_name: name.as_bytes().to_vec(),
            in_race: false,
            sprint_running: progress.sprint.is_some(),
            has_map: (1..=4).contains(&episode),
            prologue: info.id == 0x003,
            hideout: info.id == 0x004,
            final_episode: episode == 5,
            approach_incomplete: info.id & 0xFF == 0
                && !progress.has_flag(info.id, engine::game::progress::level_flag::KEY),
            slot_in_use,
        };
        self.menus.open_pause(ctx);
    }

    /// Runs the menus for a frame.
    pub fn update_menus(&mut self, input: &FrameInput, dt: f32) -> Vec<FrontAction> {
        let mi = MenuInput::from_pad(&pad_of(input), &self.map);
        let mut out = Vec::new();
        for a in self.menus.update(&mi, dt, &self.tables) {
            let f = match a {
                Action::Resume => Some(FrontAction::Resume),
                Action::TitleStart { resume } => Some(FrontAction::Start { resume }),
                Action::NewGame(s) => Some(FrontAction::NewGame(s)),
                Action::LoadGame(s) => Some(FrontAction::LoadGame(s)),
                Action::Save(s) => Some(FrontAction::Save(s)),
                Action::EraseSlot(s) => Some(FrontAction::Erase(s)),
                Action::RestartLevel | Action::RestartSprint | Action::RestartRace => Some(FrontAction::RestartLevel),
                Action::ExitLevel => Some(FrontAction::ExitLevel),
                Action::ExitToHideout => Some(FrontAction::ExitToHideout),
                Action::Quit { save } => Some(FrontAction::Quit { save }),
                Action::Answer { prompt: 20, .. } => Some(FrontAction::AutosaveNotice),
                _ => None,
            };
            if let Some(f) = f {
                // Saving and erasing leave the menus where they are; the rest leaves them.
                if !matches!(
                    f,
                    FrontAction::Quit { .. }
                        | FrontAction::Save(_)
                        | FrontAction::Erase(_)
                        | FrontAction::AutosaveNotice
                ) {
                    self.menus.close();
                }
                out.push(f);
            }
        }
        out
    }

    /// Builds the frame's draw list.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        width: u32,
        height: u32,
        progress: &Progress,
        info: &WorldInfo,
        bottle_total: usize,
        screen: &ScreenEffects,
        subtitle: Option<&engine::behavior::dialogue::Subtitle>,
        wipe: Option<Wipe>,
        saving: bool,
        dt: f32,
    ) {
        let canvas = Canvas::for_display(width, height);
        self.list = DrawList::new(canvas);
        let level = progress.level_ref(info.id);
        let values = HudValues {
            lives: progress.lives as i64,
            coins: Some(progress.coins as i64),
            keys: ((1..=5).contains(&(info.id >> 8)))
                .then(|| progress.keys.get(&(info.id >> 8)).copied().unwrap_or(0) as i64),
            bottles: (level.map_or(0, |l| l.bottles.count_ones() as i64), bottle_total as i64),
            percent: None,
        };
        self.hud.set_values(values);
        self.hud.set_saving(saving);
        self.hud.update(dt, !self.menu_open());
        // Lightning behind the HUD.
        let full = Rect { x: 0.0, y: 0.0, w: canvas.width, h: canvas.height };
        for t in &screen.tints {
            self.list.rect(full, t.color);
        }
        if screen.letterbox {
            let bar = canvas.height * 0.12;
            self.list.rect(Rect { x: 0.0, y: 0.0, w: canvas.width, h: bar }, [0.0, 0.0, 0.0, 1.0]);
            self.list.rect(Rect { x: 0.0, y: canvas.height - bar, w: canvas.width, h: bar }, [0.0, 0.0, 0.0, 1.0]);
        }
        self.hud.draw(&self.fonts, &self.tables, &mut self.list);
        if let Some(s) = subtitle {
            let c = |x: u8| (x as f32 / 128.0).min(1.0);
            let style = TextStyle {
                color: [c(s.color[0]), c(s.color[1]), c(s.color[2]), c(s.color[3]).max(0.5)],
                align: Align::Center,
                wrap: Some(canvas.width * 0.8),
                ..Default::default()
            };
            let l = layout(&self.fonts, &s.text, &style);
            let x = (canvas.width - l.width) * 0.5;
            let y = canvas.height - l.height - 24.0;
            l.draw(&self.fonts, &mut self.list, x, y, Some(s.revealed), 1.0);
        }
        self.menus.draw(&self.fonts, &self.tables, &mut self.list);
        if let Some(w) = wipe {
            self.list.rect(full, [0.0, 0.0, 0.0, w.alpha()]);
        }
    }

    /// Black bars around a movie drawn in `rect` (target pixels), so the world does not show beside it.
    pub fn movie_bars(&mut self, width: u32, height: u32, rect: [f32; 4]) {
        let canvas = Canvas::for_display(width, height);
        self.list = DrawList::new(canvas);
        let k = canvas.width / width.max(1) as f32;
        let [x, y, w, h] = rect.map(|v| v * k);
        let black = [0.0, 0.0, 0.0, 1.0];
        let (cw, ch) = (canvas.width, canvas.height);
        for r in [
            Rect { x: 0.0, y: 0.0, w: cw, h: y },
            Rect { x: 0.0, y: y + h, w: cw, h: ch - (y + h) },
            Rect { x: 0.0, y, w: x, h },
            Rect { x: x + w, y, w: cw - (x + w), h },
        ] {
            if r.w > 0.0 && r.h > 0.0 {
                self.list.rect(r, black);
            }
        }
    }

    /// Covers the screen while a movie plays without a video layer: black, with the movie's title.
    pub fn movie_card(&mut self, width: u32, height: u32, title: Option<&str>) {
        let canvas = Canvas::for_display(width, height);
        self.list = DrawList::new(canvas);
        self.list.rect(Rect { x: 0.0, y: 0.0, w: canvas.width, h: canvas.height }, [0.0, 0.0, 0.0, 1.0]);
        if let Some(t) = title {
            let style = TextStyle { align: Align::Center, wrap: Some(canvas.width * 0.8), ..Default::default() };
            let l = layout(&self.fonts, t.as_bytes(), &style);
            l.draw(
                &self.fonts,
                &mut self.list,
                (canvas.width - l.width) * 0.5,
                (canvas.height - l.height) * 0.5,
                None,
                1.0,
            );
        }
    }
}
