//! The Sly 2 session's screens (`ui::sly2`; user-interface.md 5 and 6, missions.md 5, saving.md 3):
//!
//! - **Pause menu**: + (Start) opens it during play when no other screen is open; Keep Playing, Save Game (when a
//!   saver is set), Job Help (inside an episode), Abandon Job (while a job runs), Exit to Episode Menu (inside an
//!   episode; ends the job and loads the menu world, row 1) and Quit Game (ends the session).
//! - **Job screens**: a completed job shows its "job complete" screen; an autosave point (and Save Game) saves
//!   through the platform's saver ([`Sly2Session::set_saver`]) with the saving screen shown during the save and for
//!   1 s after; the job help comes from the pause menu.
//! - **HUD**: the current character's gauge (`Sly2Level::hud`, written by the gang), the coins of the progress
//!   block, the prompts scripts ask for (operation 137's option 979) with their strings.
//!
//! The pause menu, the job help and the job-complete screen pause the game: the clock's rate goes to 0 while one
//! shows and back to 1 when none does.

use alloc::boxed::Box;
use alloc::string::String;

use engine::game::sly2_jobs::{self, Notice, flow, shop};
use engine::game::systems::FrameInput;
use formats::iso::ReadAt;
use platform_api::Buttons;
use render::Backend;
use save::sly2::Progress;
use ui::nav::MenuInput;
use ui::sly2::hud::HudValues;
use ui::sly2::pause::{Action, Context, Entry};
use ui::sly2::{HelpAction, JobRow};

use super::Sly2Session;

/// Saves the live progress (the platform's slot and file system); returns the save serial or why it failed.
pub type Saver = Box<dyn FnMut(&Progress) -> Result<u64, String>>;

/// The screen object of the job help (it must be in the world for Job Help to show).
pub const JOB_HELP_SCREEN: i32 = 1248;

/// The menu input of a frame (Cross is B, Circle A, Triangle X, Start +, as the gang's buttons).
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

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// Sets how autosaves and Save Game are written.
    pub fn set_saver(&mut self, saver: Option<Saver>) {
        self.saver = saver;
    }

    /// The screens' frame (real time `dt`): notices, the HUD, the pause menu, the job screens, the pause.
    pub(super) fn update_screens(&mut self, input: &FrameInput, dt: f32) {
        let (menu, circle) = menu_input(input);
        for n in self.take_notices() {
            match n {
                Notice::JobComplete { job, screen } => {
                    let title = self.jobs_ref().and_then(|j| j.graph.job_ids.get(&job).map(|&i| j.graph.jobs[i].title));
                    self.screens.job_complete(title.unwrap_or(-1), screen);
                }
                Notice::Autosave => self.save_now(),
                _ => {}
            }
        }
        self.update_hud(dt);
        self.refresh_gang_view();
        let help_open = self.screens.help.is_some();
        let other = help_open || self.screens.complete.is_some();
        if !other && self.update_menus(input, &menu, circle, dt) {
            // The gadget grid, the shop or the options page has the input.
        } else if self.pause.is_open() {
            match self.pause.update(dt, &menu) {
                Action::Chosen(e) => self.pause_choice(e),
                Action::Closed | Action::None => {}
            }
        } else if menu.start && !other {
            let c = self.pause_context();
            self.pause.open(c);
        } else {
            let menu = if help_open { menu } else { MenuInput { start: false, ..menu } };
            if let HelpAction::Goals(job) = self.screens.update(dt, &menu, circle && help_open) {
                let goals = self.jobs_ref().map(|j| shop::job_goals(j, job)).unwrap_or_default();
                self.screens.show_goals(goals);
            }
        }
        let pause = self.screens.pauses() || self.pause.is_open() || self.menus.open();
        if let Some(g) = self.game.as_mut() {
            if pause {
                g.clock.rate = 0.0;
                self.ui_paused = true;
            } else if self.ui_paused {
                g.clock.rate = 1.0;
                self.ui_paused = false;
            }
        }
    }

    /// The HUD's frame: the vitals, the coins, the prompts asked for.
    fn update_hud(&mut self, dt: f32) {
        let Some(g) = self.game.as_mut() else { return };
        let Some(l) = g.world.sly2.as_deref_mut() else { return };
        let prompts = core::mem::take(&mut l.hud.prompts);
        let coins = l.jobs.as_deref().map_or(0, |j| shop::coins(&j.progress));
        let values = HudValues { who: l.hud.who, health: l.hud.health, power: l.hud.power, coins };
        for (id, time) in prompts {
            if id < 0 {
                self.hud.end_prompt();
            } else {
                let text = self.sound.as_ref().and_then(|s| s.string(id as i16)).unwrap_or_default();
                self.hud.prompt(id, text, time);
            }
        }
        self.hud.update(dt, values);
    }

    /// What applies in the pause menu now.
    /// Whether the current world is an episode's hub (the level table's hub flag).
    fn is_hub(&self) -> bool {
        let (Some(g), Some(c)) = (self.game.as_ref(), self.current.as_ref()) else { return false };
        g.world.sly2.as_deref().and_then(|l| l.header.level_rows.get(c.row)).is_some_and(|r| r.hub)
    }

    fn pause_context(&self) -> Context {
        let in_episode =
            self.jobs_ref().is_some_and(|j| j.episode != 0) && self.current.as_ref().is_some_and(|c| c.row != 1);
        let has_job_help = self.game.as_ref().is_some_and(|g| {
            g.world
                .slots_of_class(126)
                .iter()
                .any(|&s| g.world.handle(s).and_then(|h| g.world.sly2_id(h)) == Some(JOB_HELP_SCREEN))
        });
        Context {
            in_episode,
            has_job_help,
            job_running: self.jobs_ref().is_some_and(|j| j.job.is_some()),
            can_save: self.saver.is_some(),
            can_exit: in_episode,
            has_options: true,
            can_shop: in_episode && self.is_hub(),
        }
    }

    /// Acts on a pause-menu entry.
    fn pause_choice(&mut self, e: Entry) {
        match e {
            Entry::KeepPlaying => {}
            Entry::SaveGame => self.save_now(),
            Entry::JobHelp => {
                if let Some(j) = self.jobs_ref() {
                    let rows = shop::job_help(j)
                        .into_iter()
                        .map(|r| JobRow { job: r.job, title: r.title, done: r.done })
                        .collect();
                    self.screens.open_help(rows);
                }
            }
            Entry::AbandonJob => {
                if let Some(g) = self.game.as_mut() {
                    sly2_jobs::with(&mut g.world, |j, _| {
                        if let Some(job) = j.job {
                            j.end_job(job);
                        }
                    });
                }
            }
            Entry::ExitToEpisodeMenu => {
                // The job ends and the menu world (row 1) loads at its default point (user-interface.md 6.1).
                if let Some(g) = self.game.as_mut() {
                    sly2_jobs::with(&mut g.world, |j, _| {
                        if let Some(job) = j.job {
                            j.end_job(job);
                        }
                        j.load = Some(flow::LoadRequest {
                            row: flow::MENU_ROW,
                            point: flow::DEFAULT_POINT,
                            ..Default::default()
                        });
                    });
                }
            }
            Entry::QuitGame => self.quit = true,
            Entry::Options => self.open_options(),
            Entry::Shop => self.open_shop(),
        }
    }

    fn jobs_ref(&self) -> Option<&engine::game::sly2_jobs::Jobs> {
        self.game.as_ref()?.world.sly2.as_deref()?.jobs.as_deref()
    }

    /// A save (an autosave point or Save Game): the saving screen, the save, the screen lingering 1 s.
    fn save_now(&mut self) {
        let Some(saver) = self.saver.as_mut() else { return };
        let Some(g) = self.game.as_mut() else { return };
        let Some(p) = sly2_jobs::with(&mut g.world, |j, _| {
            j.store_current();
            j.progress.clone()
        }) else {
            return;
        };
        self.screens.save_started();
        if let Err(e) = saver(&p) {
            self.message = Some(e);
        }
        self.screens.save_finished();
    }
}
