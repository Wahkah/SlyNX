//! The safe-house computer (safehouse-and-shop.md 3 to 6) and the job help (section 7), on the progress block:
//! coins `loot/ccoin`, loot counts `loot/mplootkc`, owned items `powerUps/mppukfAcquired`, the best episode
//! `episodeBest`.

use alloc::vec::Vec;

use save::sly2::{Progress, SaveValue};

use super::{Jobs, event, state};
use crate::gang::{ItemTable, LootTable};

/// The coin counter's limit [reference only, safehouse-and-shop.md 4].
pub const COIN_CAP: i32 = 9999;
/// Item records the shop can list (7 to 42).
pub const SHOP_ITEMS: core::ops::RangeInclusive<u8> = 7..=42;

/// The coins.
pub fn coins(p: &Progress) -> i32 {
    p.int(&["loot", "ccoin"])
}

/// Sets the coins (kept within 0 and the cap).
pub fn set_coins(p: &mut Progress, v: i32) {
    p.set_named(&["loot", "ccoin"], None, SaveValue::Int(v.clamp(0, COIN_CAP)));
}

/// Whether item `n` is owned.
pub fn owned(p: &Progress, n: i32) -> bool {
    n >= 0 && p.flag_at(&["powerUps", "mppukfAcquired"], n as usize)
}

/// Sets item `n`'s owned flag.
pub fn set_owned(p: &mut Progress, n: i32, on: bool) -> bool {
    n >= 0 && p.set_named(&["powerUps", "mppukfAcquired"], Some(n as usize), SaveValue::Flag(on))
}

/// The count held of loot kind `k`.
pub fn loot(p: &Progress, k: usize) -> i32 {
    p.get_named(&["loot", "mplootkc"], Some(k)).map_or(0, SaveValue::as_i32)
}

/// Adds `by` to loot kind `k`.
pub fn add_loot(p: &mut Progress, k: usize, by: i32) -> bool {
    let v = loot(p, k) + by;
    p.set_named(&["loot", "mplootkc"], Some(k), SaveValue::Int(v.max(0)))
}

/// The Buy page: the items that are open (episode not −1 and at most `episodeBest`) and not owned, in table order.
pub fn buy_list(p: &Progress, items: &ItemTable) -> Vec<u8> {
    let best = p.int(&["episodeBest"]);
    SHOP_ITEMS
        .filter(|&n| items.get(n).is_some_and(|i| i.episode != -1 && i.episode <= best) && !owned(p, n as i32))
        .collect()
}

/// The Sell page: the loot kinds held (count > 0) that are worth coins.
pub fn sell_list(p: &Progress, table: &LootTable) -> Vec<usize> {
    (0..table.records.len()).filter(|&k| loot(p, k) > 0 && table.records[k].value > 0).collect()
}

/// Why a purchase did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopError {
    /// The item is not on the Buy page.
    NotOffered,
    /// Not enough coins (a message shows for 5 s).
    NotEnoughCoins,
    /// No such loot held.
    NothingToSell,
}

/// Buys item `n` (after the confirmation): the price is paid, the item becomes owned and the job system gets task
/// event 59 (the characters' script event 26 is the session's).
pub fn buy(jobs: &mut Jobs, items: &ItemTable, n: u8) -> Result<(), ShopError> {
    if !buy_list(&jobs.progress, items).contains(&n) {
        return Err(ShopError::NotOffered);
    }
    let price = items.get(n).map_or(0, |i| i.price);
    let c = coins(&jobs.progress);
    if c < price {
        return Err(ShopError::NotEnoughCoins);
    }
    set_coins(&mut jobs.progress, c - price);
    set_owned(&mut jobs.progress, n as i32, true);
    jobs.task_event(event::ITEM_OWNED, None);
    Ok(())
}

/// Sells one of loot kind `k`: its count drops by one and its value is added to the coins.
pub fn sell(p: &mut Progress, table: &LootTable, k: usize) -> Result<(), ShopError> {
    if !sell_list(p, table).contains(&k) {
        return Err(ShopError::NothingToSell);
    }
    add_loot(p, k, -1);
    set_coins(p, coins(p) + table.records[k].value);
    Ok(())
}

/// A row of the job help's first page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobRow {
    /// The job's object ID and display name string.
    pub job: i32,
    pub title: i16,
    /// Its last task is done (the check mark).
    pub done: bool,
}

/// The job help's first page: the episode's open jobs (a task of theirs is available or done), at most 10.
pub fn job_help(jobs: &Jobs) -> Vec<JobRow> {
    let g = &jobs.graph;
    (0..g.jobs.len())
        .filter(|&j| g.tasks_of(j).any(|t| jobs.states[t] >= state::AVAILABLE))
        .map(|j| JobRow {
            job: g.jobs[j].id,
            title: g.jobs[j].title,
            done: g.jobs[j].last.is_some_and(|l| jobs.states[l] >= state::DONE),
        })
        .take(10)
        .collect()
}

/// The job help's second page for job `job`: its done tasks and the current one, with their goal texts (tasks
/// without one are left out), at most 4: (objective string, done).
pub fn job_goals(jobs: &Jobs, job: i32) -> Vec<(i16, bool)> {
    let g = &jobs.graph;
    let Some(&j) = g.job_ids.get(&job) else { return Vec::new() };
    g.tasks_of(j)
        .filter(|&t| jobs.states[t] >= state::DONE || jobs.current == Some(t))
        .filter_map(|t| g.tasks[t].objective.map(|o| (o, jobs.states[t] >= state::DONE)))
        .take(4)
        .collect()
}
