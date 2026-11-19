//! Sly 2's job screens (`specs/sly2/user-interface.md` 2, 5.4 and 6.3, `missions.md` 5, `saving.md` 3): the "job
//! complete" screen, the memory-card activity screen shown while the game saves, and the job help.
//!
//! - **Screens** ([`Screen`]) follow user-interface.md 2: showing (the show label), idle, hiding (the hide label),
//!   gone; they run on the real clock, so they animate while the game is paused. The label lengths live in the
//!   screens' pictures, which SlyNX does not draw yet: [`SHOW_TIME`] and [`HIDE_TIME`] stand in for them (the
//!   prompt's labels, user-interface.md 7) [working choice].
//! - **Job complete** (missions.md 5): the game pauses and shows the job's screen object (option 1258) part by
//!   part, then it is done. Each part stays [`PART_TIME`] seconds or until Cross [working choice: the parts are
//!   pictures whose own lengths are not read].
//! - **Saving** (object 1492): shown while a save runs and hidden 1 s after it ends.
//! - **Job help** (object 1248, from the pause menu, [`super::pause`]; safehouse-and-shop.md 7): page 1 lists the
//!   episode's open jobs (up to 10, a check mark when the job's last task is done); page 2 a job's goals (up to 4:
//!   the done tasks and the current one). Up and Down move, Right or Cross opens a job, Left goes back, Circle or
//!   Start closes. It pauses the game.
//!
//! The model is drawn into a [`DrawList`] as panels with the texts the session gives by string ID (`text`), in the
//! Sly 2 font sheet when there is one; the screens' own vector pictures are not drawn.

use alloc::string::String;
use alloc::vec::Vec;

use super::font::FontSheet;
use crate::draw::{DrawList, Rect};
use crate::nav::{Dir, MenuInput, Repeat};

/// Seconds of a screen's show and hide labels [working choice].
pub const SHOW_TIME: f32 = 0.6;
pub const HIDE_TIME: f32 = 0.3;
/// Seconds each part of the job-complete screen stays [working choice].
pub const PART_TIME: f32 = 2.5;
/// The saving screen hides this long after the save ends.
pub const SAVE_LINGER: f32 = 1.0;
/// Most rows on the job help's pages.
pub const JOB_ROWS: usize = 10;
pub const GOAL_ROWS: usize = 4;
/// Screen objects.
pub const SAVING_SCREEN: i32 = 1492;
pub const JOB_HELP_SCREEN: i32 = 1248;
/// String 312, "Jobs".
pub const JOBS_STRING: i16 = 312;

/// Where a screen is (user-interface.md 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Gone (or never shown).
    Gone,
    /// Playing its show label, for this long so far.
    Showing(f32),
    /// On screen, for this long.
    Idle(f32),
    /// Playing its hide label.
    Hiding(f32),
}

/// A screen object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// The object.
    pub object: i32,
    pub phase: Phase,
    /// Hides itself after this long idle.
    pub idle_limit: Option<f32>,
}

impl Screen {
    /// A gone screen.
    pub const fn new(object: i32) -> Screen {
        Screen { object, phase: Phase::Gone, idle_limit: None }
    }

    /// Shows it (again).
    pub fn show(&mut self) {
        if !matches!(self.phase, Phase::Showing(_) | Phase::Idle(_)) {
            self.phase = Phase::Showing(0.0);
        }
    }

    /// Hides it.
    pub fn hide(&mut self) {
        if matches!(self.phase, Phase::Showing(_) | Phase::Idle(_)) {
            self.phase = Phase::Hiding(0.0);
        }
    }

    /// Whether it is on screen at all.
    pub fn visible(&self) -> bool {
        self.phase != Phase::Gone
    }

    /// How much shows, 0 to 1 (the show and hide labels as fades).
    pub fn coverage(&self) -> f32 {
        match self.phase {
            Phase::Gone => 0.0,
            Phase::Showing(t) => (t / SHOW_TIME).min(1.0),
            Phase::Idle(_) => 1.0,
            Phase::Hiding(t) => 1.0 - (t / HIDE_TIME).min(1.0),
        }
    }

    /// Advances on the real clock.
    pub fn update(&mut self, dt: f32) {
        self.phase = match self.phase {
            Phase::Showing(t) if t + dt >= SHOW_TIME => Phase::Idle(0.0),
            Phase::Showing(t) => Phase::Showing(t + dt),
            Phase::Idle(t) if self.idle_limit.is_some_and(|l| t + dt >= l) => Phase::Hiding(0.0),
            Phase::Idle(t) => Phase::Idle(t + dt),
            Phase::Hiding(t) if t + dt >= HIDE_TIME => Phase::Gone,
            Phase::Hiding(t) => Phase::Hiding(t + dt),
            Phase::Gone => Phase::Gone,
        };
    }
}

/// The job-complete screen in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct JobComplete {
    /// The job's display name.
    pub title: i16,
    /// The screen object and its parts (option 1258).
    pub parts: Vec<i32>,
    /// The part showing.
    pub part: usize,
    /// Seconds in the part.
    pub time: f32,
    pub screen: Screen,
}

/// A row of the job help's first page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobRow {
    /// The job's object ID and display name.
    pub job: i32,
    pub title: i16,
    /// Its last task is done.
    pub done: bool,
}

/// The job help's page.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    /// Page 1: the jobs.
    Jobs,
    /// Page 2: a job's goals (objective string, done).
    Goals { job: usize, goals: Vec<(i16, bool)> },
}

/// The job help.
#[derive(Clone, Debug, PartialEq)]
pub struct JobHelp {
    pub rows: Vec<JobRow>,
    pub cursor: usize,
    pub page: Page,
    pub screen: Screen,
    repeat: Repeat,
}

/// What the job help asks of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpAction {
    /// Nothing.
    None,
    /// Page 2 of this job is wanted: call [`JobScreens::show_goals`].
    Goals(i32),
    /// Closed.
    Closed,
}

/// The job screens.
#[derive(Clone, Debug, PartialEq)]
pub struct JobScreens {
    pub complete: Option<JobComplete>,
    pub saving: Screen,
    /// Seconds since the save ended (`None` while saving or idle).
    save_ended: Option<f32>,
    pub help: Option<JobHelp>,
}

impl Default for JobScreens {
    fn default() -> Self {
        JobScreens { complete: None, saving: Screen::new(SAVING_SCREEN), save_ended: None, help: None }
    }
}

impl JobScreens {
    /// A job completed: show its screen (`screen`: the object and its two parts; none: only the title).
    pub fn job_complete(&mut self, title: i16, screen: Option<[i32; 3]>) {
        let parts = screen.map_or_else(Vec::new, |s| s[1..].to_vec());
        let mut sc = Screen::new(screen.map_or(-1, |s| s[0]));
        sc.show();
        self.complete = Some(JobComplete { title, parts, part: 0, time: 0.0, screen: sc });
    }

    /// A save began.
    pub fn save_started(&mut self) {
        self.saving.show();
        self.save_ended = None;
    }

    /// The save ended.
    pub fn save_finished(&mut self) {
        self.save_ended = Some(0.0);
    }

    /// Opens the job help on page 1.
    pub fn open_help(&mut self, rows: Vec<JobRow>) {
        let mut screen = Screen::new(JOB_HELP_SCREEN);
        screen.show();
        let rows = rows.into_iter().take(JOB_ROWS).collect();
        self.help = Some(JobHelp { rows, cursor: 0, page: Page::Jobs, screen, repeat: Repeat::default() });
    }

    /// Shows page 2 for the job at the cursor with its goals.
    pub fn show_goals(&mut self, goals: Vec<(i16, bool)>) {
        if let Some(h) = self.help.as_mut() {
            h.page = Page::Goals { job: h.cursor, goals: goals.into_iter().take(GOAL_ROWS).collect() };
        }
    }

    /// Whether the screens pause the game (job complete, job help).
    pub fn pauses(&self) -> bool {
        self.complete.is_some() || self.help.as_ref().is_some_and(|h| h.screen.visible())
    }

    /// Advances the screens on the real clock with this frame's input. `circle`: Circle went down (closing the
    /// job help, as Start does). Returns what the job help asks.
    pub fn update(&mut self, dt: f32, input: &MenuInput, circle: bool) -> HelpAction {
        // Job complete: part after part, Cross skips.
        if let Some(c) = self.complete.as_mut() {
            c.screen.update(dt);
            c.time += dt;
            if c.time >= PART_TIME || input.accept {
                if c.part + 1 < c.parts.len().max(1) {
                    c.part += 1;
                    c.time = 0.0;
                } else {
                    c.screen.hide();
                }
            }
            if !c.screen.visible() {
                self.complete = None;
            }
        }
        // Saving: hidden 1 s after the save ends.
        self.saving.update(dt);
        if let Some(t) = self.save_ended.as_mut() {
            *t += dt;
            if *t >= SAVE_LINGER {
                self.saving.hide();
                self.save_ended = None;
            }
        }
        let Some(h) = self.help.as_mut() else { return HelpAction::None };
        h.screen.update(dt);
        if !h.screen.visible() {
            self.help = None;
            return HelpAction::Closed;
        }
        if matches!(h.screen.phase, Phase::Hiding(_)) {
            return HelpAction::None;
        }
        if circle || input.start {
            h.screen.hide();
            return HelpAction::None;
        }
        let step = h.repeat.update(input, dt);
        match (&h.page, step) {
            (Page::Jobs, Some(Dir::Up)) if h.cursor > 0 => h.cursor -= 1,
            (Page::Jobs, Some(Dir::Down)) if h.cursor + 1 < h.rows.len() => h.cursor += 1,
            (Page::Jobs, Some(Dir::Right)) => {
                return h.rows.get(h.cursor).map_or(HelpAction::None, |r| HelpAction::Goals(r.job));
            }
            (Page::Jobs, None) if input.accept => {
                return h.rows.get(h.cursor).map_or(HelpAction::None, |r| HelpAction::Goals(r.job));
            }
            (Page::Goals { .. }, Some(Dir::Left)) => h.page = Page::Jobs,
            _ => {}
        }
        HelpAction::None
    }

    /// Draws the screens: panels, and the texts `text(string ID)` gives (in `fonts` when there are some).
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, text: &dyn Fn(i16) -> Option<String>) {
        let cw = list.canvas.width;
        let ch = list.canvas.height;
        let put = |list: &mut DrawList, s: Option<String>, x: f32, y: f32, alpha: f32| {
            if let (Some(f), Some(s)) = (font, s) {
                let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
                f.draw(list, &bytes, x, y, 22.0, [[1.0, 1.0, 1.0, alpha], [1.0, 0.85, 0.3, alpha]], None);
            }
        };
        if let Some(c) = &self.complete {
            let a = c.screen.coverage();
            let r = Rect::new(cw * 0.15, ch * 0.35, cw * 0.7, ch * 0.3);
            list.rect(r, [0.0, 0.0, 0.0, 0.7 * a]);
            list.frame(r, 3.0, [1.0, 0.85, 0.2, a]);
            put(list, text(c.title), r.x + 24.0, r.y + 24.0, a);
            // One mark per part, the current one lit.
            for i in 0..c.parts.len().max(1) {
                let lit = if i == c.part { 1.0 } else { 0.35 };
                list.rect(
                    Rect::new(r.x + 24.0 + 20.0 * i as f32, r.bottom() - 28.0, 12.0, 12.0),
                    [1.0, 0.85, 0.2, lit * a],
                );
            }
        }
        if self.saving.visible() {
            let a = self.saving.coverage();
            let r = Rect::new(cw - 96.0, ch - 80.0, 56.0, 40.0);
            list.rect(r, [0.1, 0.1, 0.3, 0.8 * a]);
            list.frame(r, 2.0, [0.9, 0.9, 1.0, a]);
        }
        if let Some(h) = &self.help {
            let a = h.screen.coverage();
            let panel = Rect::new(cw * 0.1, ch * 0.1, cw * 0.8, ch * 0.8);
            list.rect(panel, [0.02, 0.05, 0.12, 0.85 * a]);
            list.frame(panel, 3.0, [0.4, 0.7, 1.0, a]);
            put(list, text(JOBS_STRING), panel.x + 20.0, panel.y + 16.0, a);
            let row_h = 28.0;
            let top = panel.y + 56.0;
            match &h.page {
                Page::Jobs => {
                    for (i, r) in h.rows.iter().enumerate() {
                        let y = top + row_h * i as f32;
                        if i == h.cursor {
                            list.rect(
                                Rect::new(panel.x + 12.0, y - 2.0, panel.w - 24.0, row_h - 4.0),
                                [0.3, 0.5, 0.9, 0.5 * a],
                            );
                        }
                        let mark = if r.done { [0.3, 1.0, 0.4, a] } else { [0.5, 0.5, 0.5, 0.6 * a] };
                        list.rect(Rect::new(panel.x + 20.0, y + 4.0, 14.0, 14.0), mark);
                        put(list, text(r.title), panel.x + 44.0, y, a);
                    }
                }
                Page::Goals { job, goals } => {
                    if let Some(r) = h.rows.get(*job) {
                        put(list, text(r.title), panel.x + 20.0, top, a);
                    }
                    for (i, (s, done)) in goals.iter().enumerate() {
                        let y = top + row_h * (i + 1) as f32 + 8.0;
                        let mark = if *done { [0.3, 1.0, 0.4, a] } else { [1.0, 0.85, 0.2, a] };
                        list.rect(Rect::new(panel.x + 20.0, y + 4.0, 14.0, 14.0), mark);
                        put(list, text(*s), panel.x + 44.0, y, a);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn tick(s: &mut JobScreens, secs: f32, input: MenuInput) -> HelpAction {
        let mut last = HelpAction::None;
        let mut t = 0.0;
        while t < secs {
            let a = s.update(1.0 / 60.0, &input, false);
            if a != HelpAction::None {
                last = a;
            }
            t += 1.0 / 60.0;
        }
        last
    }

    #[test]
    fn job_complete_shows_its_parts_then_ends() {
        let mut s = JobScreens::default();
        s.job_complete(323, Some([1750, 17425, 17426]));
        assert!(s.pauses());
        tick(&mut s, PART_TIME + 0.1, MenuInput::default());
        assert_eq!(s.complete.as_ref().unwrap().part, 1);
        // Cross ends the last part; the screen hides and the pause ends.
        s.update(0.01, &MenuInput { accept: true, ..Default::default() }, false);
        tick(&mut s, HIDE_TIME + 0.1, MenuInput::default());
        assert!(s.complete.is_none() && !s.pauses());
    }

    #[test]
    fn saving_lingers_one_second() {
        let mut s = JobScreens::default();
        s.save_started();
        tick(&mut s, 2.0, MenuInput::default());
        assert!(s.saving.visible());
        s.save_finished();
        tick(&mut s, SAVE_LINGER - 0.1, MenuInput::default());
        assert!(matches!(s.saving.phase, Phase::Idle(_)));
        tick(&mut s, 0.2 + HIDE_TIME, MenuInput::default());
        assert!(!s.saving.visible());
    }

    #[test]
    fn job_help_pages() {
        let mut s = JobScreens::default();
        let row = |job, done| JobRow { job, title: 323, done };
        s.open_help(vec![row(1760, true), row(1771, false)]);
        tick(&mut s, SHOW_TIME + 0.05, MenuInput::default());
        s.update(0.01, &MenuInput { dpad: [false, true, false, false], ..Default::default() }, false);
        assert_eq!(s.help.as_ref().unwrap().cursor, 1);
        let a = s.update(0.5, &MenuInput { accept: true, ..Default::default() }, false);
        assert_eq!(a, HelpAction::Goals(1771));
        s.show_goals(vec![(325, true), (326, false)]);
        assert!(matches!(s.help.as_ref().unwrap().page, Page::Goals { job: 1, .. }));
        s.update(0.5, &MenuInput::default(), false);
        s.update(0.01, &MenuInput { dpad: [false, false, true, false], ..Default::default() }, false);
        assert_eq!(s.help.as_ref().unwrap().page, Page::Jobs);
        s.update(0.01, &MenuInput::default(), true);
        assert_eq!(tick(&mut s, HIDE_TIME + 0.1, MenuInput::default()), HelpAction::Closed);
        assert!(s.help.is_none());
        let mut list = DrawList::new(Default::default());
        s.open_help(vec![row(1, false)]);
        s.job_complete(1, None);
        s.save_started();
        tick(&mut s, 0.3, MenuInput::default());
        s.draw(&mut list, None, &|_| None);
        assert!(!list.quads.is_empty());
    }
}
