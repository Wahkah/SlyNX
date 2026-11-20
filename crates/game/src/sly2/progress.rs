//! The Sly 2 session's progress (level-flow.md 3, saving.md 3): new game, continue, saving to a slot, and moving
//! between worlds when the job system asks for a load (`engine::game::sly2_jobs`).
//!
//! The live progress block travels with the session: each loaded world gets the job system attached with it and
//! arrives at the requested arrival point; before the next load it is taken back out. Slots go through
//! `save::sly2::Sly2Store` (SlyNX's own file format with the game's slot semantics).

use alloc::string::String;

use engine::game::sly2_jobs::{self, Notice, flow};
use formats::iso::ReadAt;
use platform_api::FileSystem;
use render::Backend;
use save::Slot;
use save::sly2::{Progress, Sly2Store};

use super::Sly2Session;

/// Where the session goes next and with what.
#[derive(Clone, Debug, Default)]
pub struct Pending {
    /// The progress block to attach to the next world (none: a fresh one from that world's tree).
    pub progress: Option<Progress>,
    /// What the previous world carried over (none after a new game or a load).
    pub carry: Option<sly2_jobs::Carry>,
    /// The arrival point.
    pub point: Option<i32>,
    /// A task to request on arrival.
    pub task: Option<i32>,
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// Attaches the job system to the world just loaded and arrives at the pending point (called by `load_row`).
    pub(super) fn attach_progress(&mut self) {
        let Some(g) = self.game.as_mut() else { return };
        let pending = core::mem::take(&mut self.pending);
        let progress = match pending.progress {
            Some(p) => p,
            None => match sly2_jobs::tree_of(&g.world) {
                Some(t) => Progress::new(t),
                None => return,
            },
        };
        if sly2_jobs::attach(&mut g.world, progress, pending.carry, pending.task) {
            sly2_jobs::arrive(g, pending.point.unwrap_or(flow::DEFAULT_POINT));
        }
    }

    /// Performs a load the job system asked for (an exit, a restart): takes the progress out and loads the row.
    pub(super) fn follow_load_request(&mut self) -> Result<(), String> {
        let Some(g) = self.game.as_mut() else { return Ok(()) };
        let Some(req) = sly2_jobs::with(&mut g.world, |j, _| j.load.take()).flatten() else { return Ok(()) };
        let (progress, carry) = sly2_jobs::detach(&mut g.world).unzip();
        self.pending = Pending { progress, carry, point: Some(req.point), task: req.task };
        self.load_row(req.row.max(0) as usize)
    }

    /// The job system's notices since the last call (objectives, job completion, autosave points).
    pub fn take_notices(&mut self) -> alloc::vec::Vec<Notice> {
        self.game
            .as_mut()
            .and_then(|g| sly2_jobs::with(&mut g.world, |j, _| core::mem::take(&mut j.notices)))
            .unwrap_or_default()
    }

    /// The live progress block, if a world with the job system is running.
    pub fn progress(&self) -> Option<&Progress> {
        Some(&self.game.as_ref()?.world.sly2.as_deref()?.jobs.as_deref()?.progress)
    }

    /// A new game (level-flow.md 3): the intro hub at point 393 with task 1583 queued, from a reset block that
    /// keeps the options of `options` (a previous block) if given.
    pub fn new_game(&mut self, options: Option<Progress>) -> Result<(), String> {
        let progress = options.map(|mut p| {
            p.reset_keeping_options();
            p
        });
        self.pending =
            Pending { progress, carry: None, point: Some(flow::DEFAULT_POINT), task: Some(sly2_jobs::NEW_GAME_TASK) };
        self.load_row(sly2_jobs::BOOT_ROW as usize)
    }

    /// Continues from `slot` (level-flow.md 3): the saved episode's hub at point 393, or the menu world when that
    /// episode is complete.
    pub fn continue_game<F: FileSystem>(&mut self, store: &Sly2Store<F>, slot: Slot) -> Result<(), String> {
        let tree = self.game.as_ref().and_then(|g| sly2_jobs::tree_of(&g.world)).ok_or("no Sly 2 world loaded")?;
        let loaded = store.load(slot, &Progress::new(tree)).map_err(|e| alloc::format!("{e}"))?;
        self.continue_from(loaded.progress)
    }

    /// Continues a loaded progress block (level-flow.md 3): the saved episode's hub at point 393, or the menu world
    /// when that episode is complete.
    pub fn continue_from(&mut self, p: Progress) -> Result<(), String> {
        let episode = p.int(&["episodeCurrent"]).max(0) as u8;
        let hub = self.hub_row(episode).unwrap_or(sly2_jobs::BOOT_ROW as usize);
        let done = self.episode_percent(&p, hub) >= 100;
        let row = if done { flow::MENU_ROW as usize } else { hub };
        self.pending = Pending { progress: Some(p), carry: None, point: Some(flow::DEFAULT_POINT), task: None };
        self.load_row(row)
    }

    /// Saves the live progress to `slot`.
    pub fn save_game<F: FileSystem>(&mut self, store: &Sly2Store<F>, slot: Slot) -> Result<u64, String> {
        let g = self.game.as_mut().ok_or("no world")?;
        sly2_jobs::with(&mut g.world, |j, _| j.store_current()).ok_or("no progress")?;
        let p = self.progress().ok_or("no progress")?;
        store.save(slot, p).map_err(|e| alloc::format!("{e}"))
    }

    /// The hub row of `episode` (the level table's hub flag).
    pub(super) fn hub_row(&self, episode: u8) -> Option<usize> {
        let w = self.game.as_ref()?.world.sly2.as_deref()?;
        w.header.level_rows.iter().position(|r| r.hub && r.episode == episode)
    }

    /// The saved percent complete of the episode of `row`.
    fn episode_percent(&self, p: &Progress, row: usize) -> i32 {
        let Some(w) = self.game.as_ref().and_then(|g| g.world.sly2.as_deref()) else { return 0 };
        let Some(path) = w.header.level_rows.get(row).map(|r| r.save_path.clone()) else { return 0 };
        let Some(id) = p.tree.name_id("percent_complete") else { return 0 };
        let mut path: alloc::vec::Vec<u16> = path.into_iter().take(2).collect();
        path.push(id);
        p.get(&path, None).map_or(0, |v| v.as_i32())
    }
}
