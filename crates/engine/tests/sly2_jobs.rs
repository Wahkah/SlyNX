//! The Sly 2 job and progression system on the player's own Sly 2 (USA) disc (`SLYNX_DISC_SLY2`); skipped when it
//! is not set. A new game in the intro hub takes the first job, a task is completed by a task request (as props and
//! volumes send them), the progress is saved to a slot and loaded into a fresh world with the same states, and an
//! exit of an episode-1 job world leads back to the hub, which is entered at its default arrival point.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use engine::game::progress::Progress;
use engine::game::sly2_jobs::{self, flow, shop, state};
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly2::Sly2Options;
use engine::world::{World, WorldInfo};
use formats_sly2::Disc;
use formats_sly2::exe::OptionTable;
use harness::golden::{Game as Title, require_disc};
use platform_api::{DataFile, FileSystem, FsError, Root};
use save::Slot;
use save::sly2::{Progress as Block, SaveValue, Sly2Store};
use script::{BuiltinTable, OptionMap, ScriptError};

struct DiscData {
    options: OptionTable,
    map: OptionMap,
    builtins: BuiltinTable,
    items: engine::gang::ItemTable,
    loot: engine::gang::LootTable,
    worlds: BTreeMap<usize, (u32, Vec<u8>)>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly2, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let worlds = [0usize, 1, 2, 3]
            .into_iter()
            .map(|row| (row, (d.rows[row].world_id(), d.read_world(row).expect("read world"))))
            .collect();
        DiscData {
            options: d.options.clone(),
            map: OptionMap::from_sly2_executable(&d.executable).expect("option map"),
            builtins: BuiltinTable::from_executable(&d.executable).expect("built-ins"),
            items: engine::gang::ItemTable::from_executable(&d.executable).expect("item table"),
            loot: engine::gang::LootTable::from_executable(&d.executable).expect("loot table"),
            worlds,
        }
    }))
}

fn game(d: &DiscData, row: usize) -> Game {
    let (id, bytes) = &d.worlds[&row];
    let info = WorldInfo { index: row as u32, id: *id, task_mask: 0, identity: *id };
    let opts =
        Sly2Options { row: Some(row as i16), info, builtins: Some(d.builtins.clone()), ..Sly2Options::default() };
    let w = World::load_sly2(bytes.clone(), &d.options, opts).expect("world loads");
    let cfg = GameConfig { seed: 3, option_map: Some(d.map.clone()), no_scripts: false };
    Game::new(w, Progress::new_game(), Systems::null(), cfg)
}

/// Loads `row` with `progress`, attaches the job system (with a queued task, nothing carried) and arrives at
/// `point`.
fn enter(d: &DiscData, row: usize, progress: Block, queued: Option<i32>, point: i32) -> Game {
    let mut g = game(d, row);
    assert!(sly2_jobs::attach(&mut g.world, progress, None, queued));
    sly2_jobs::arrive(&mut g, point);
    g
}

fn jobs(g: &Game) -> &sly2_jobs::Jobs {
    g.world.sly2.as_deref().and_then(|l| l.jobs.as_deref()).expect("jobs attached")
}

fn run(g: &mut Game, frames: u32) {
    let input = FrameInput { left: [0.0, 0.5], ..Default::default() };
    for _ in 0..frames {
        g.step_fixed(&input);
    }
}

/// An in-memory user folder.
#[derive(Clone, Default)]
struct MemFs(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);
struct MemFile(Vec<u8>);
impl DataFile for MemFile {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        let start = (offset as usize).min(self.0.len());
        let n = buf.len().min(self.0.len() - start);
        buf[..n].copy_from_slice(&self.0[start..start + n]);
        Ok(n)
    }
}
impl FileSystem for MemFs {
    type File = MemFile;
    fn open(&self, _: Root, path: &str) -> Result<MemFile, FsError> {
        self.0.lock().unwrap().get(path).cloned().map(MemFile).ok_or(FsError::NotFound)
    }
    fn exists(&self, _: Root, path: &str) -> bool {
        self.0.lock().unwrap().contains_key(path)
    }
    fn write_user(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        self.0.lock().unwrap().insert(path.to_string(), data.to_vec());
        Ok(())
    }
    fn remove_user(&self, path: &str) -> Result<(), FsError> {
        self.0.lock().unwrap().remove(path);
        Ok(())
    }
}

#[test]
fn exits_name_their_destination_and_movies() {
    let Some(d) = disc("exits_name_their_destination_and_movies") else { return };
    // The end of the intro (level-flow.md 3): exit 150652 to the menu world (row 1, identity 0x39) with movie 0x806;
    // the menu world's exit 1329 to episode 1's hub (row 2) with its opening movie 0x7D9 (level-flow.md 4).
    let g = game(d, 0);
    let e = flow::exits(&g.world).into_iter().find(|e| e.id == 150652).expect("exit 150652");
    assert_eq!(flow::exit_records(&g.world, e.handle).map(|r| r.0), Some(0x39));
    assert_eq!((e.row, e.movies.clone()), (Some(1), vec![0x806]));
    let g = game(d, 1);
    let exits = flow::exits(&g.world);
    let e = exits.iter().find(|e| e.id == 1329).expect("exit 1329");
    assert_eq!((e.row, e.movies.clone()), (Some(2), vec![0x7D9]));
    // Every exit of both worlds names a destination row.
    for e in flow::exits(&game(d, 0).world).iter().chain(&exits) {
        assert!(e.row.is_some(), "exit {} names a row", e.id);
    }
}

#[test]
fn new_game_job_task_save_load_and_hub() {
    let Some(d) = disc("new_game_job_task_save_load_and_hub") else { return };

    // The progress tree is the same in every world (saving.md 1): 960 names, 5,461 records, layout 20,632.
    let tree = sly2_jobs::tree_of(&game(d, 0).world).expect("save-variable section");
    assert_eq!((tree.records().len(), tree.layout_word(), tree.checksum()), (5461, 20_632, 0x2767B4));
    assert_eq!(tree.var_named(&["loot", "ccoin"], None).map(|v| v.offset), Some(160));
    assert_eq!(tree.var_named(&["powerUps", "mppukfAcquired"], Some(42)).map(|v| v.offset), Some(152));

    // New game: the intro hub with task 1583 queued; its job begins (level-flow.md 3).
    let mut fresh = Block::new(tree.clone());
    fresh.reset_keeping_options();
    let mut g = enter(d, 0, fresh, Some(sly2_jobs::NEW_GAME_TASK), flow::DEFAULT_POINT);
    {
        let j = jobs(&g);
        let t = j.graph.task_ids[&1583];
        assert_eq!(j.current, Some(t), "the queued task is current");
        let job = j.job.expect("the intro job began");
        eprintln!(
            "intro: {} jobs, {} tasks; job {} ({}), task {} ({}), state {}",
            j.graph.jobs.len(),
            j.graph.tasks.len(),
            j.graph.jobs[job].id,
            j.graph.jobs[job].name,
            j.graph.tasks[t].id,
            j.graph.tasks[t].name,
            j.states[t]
        );
        assert_eq!(j.graph.tasks[t].job, Some(job));
        assert_eq!(j.states[t], state::AVAILABLE);
        assert_eq!(j.progress.int(&["widCurrent"]), 0);
        let l = g.world.sly2.as_deref().unwrap();
        assert_eq!(l.job[0], Some(j.graph.jobs[job].slot), "operation 100 gives the job");
        assert_eq!(l.job[1], Some(j.graph.tasks[t].slot), "operation 101 gives the task");
    }
    run(&mut g, 30);

    // Complete the current task by a task request (as props and volumes send them, `World::shared.task_requests`).
    let (task, task_id, succ) = {
        let j = jobs(&g);
        let t = j.current.unwrap();
        (t, j.graph.tasks[t].id, j.graph.tasks[t].succ.clone())
    };
    g.world.shared.task_requests.push((task_id, state::DONE as i32));
    run(&mut g, 2);
    let (states, current, percent) = {
        let j = jobs(&g);
        assert!(j.states[task] >= state::DONE, "the task is done");
        for &c in &succ {
            eprintln!("  successor {} ({}): state {}", j.graph.tasks[c].id, j.graph.tasks[c].name, j.states[c]);
        }
        assert!(succ.iter().any(|&c| j.states[c] >= state::AVAILABLE), "a successor opened");
        let key = j.graph.tasks[task].keys.completed.clone().expect("done-count key");
        assert_eq!(j.progress.get(&key, None), Some(SaveValue::Int(1)));
        (j.states.clone(), j.current.map(|t| j.graph.tasks[t].id), j.percent())
    };
    eprintln!("after the task: current {current:?}, percent {percent}");

    // Coins and an item, then save to slot 2 and load into a fresh world.
    sly2_jobs::with(&mut g.world, |j, _| {
        shop::set_coins(&mut j.progress, 500);
        shop::set_owned(&mut j.progress, 23, true);
    });
    run(&mut g, 10);
    let played = jobs(&g).progress.float(&["dt"]);
    assert!(played > 0.5, "play time counts ({played})");
    let (saved, carry) = sly2_jobs::detach(&mut g.world).expect("progress");
    assert_eq!(Some(carry.task), current, "a world change would carry the current task");
    let store = Sly2Store::new(MemFs::default());
    let slot = Slot::new(1).unwrap();
    store.save(slot, &saved).expect("save");
    let loaded = store.load(slot, &Block::new(tree.clone())).expect("load");
    assert!(!loaded.cleared && !loaded.unused);
    let g2 = enter(d, 0, loaded.progress, None, flow::DEFAULT_POINT);
    {
        let j = jobs(&g2);
        assert_eq!(j.states, states, "every task's state comes back");
        // The current task is not saved (saving.md); after a load only a task with option 1273 restored as
        // available is current again.
        assert!(j.current.is_none_or(|t| j.graph.tasks[t].resume && j.states[t] == state::AVAILABLE));
        assert_eq!(shop::coins(&j.progress), 500);
        assert!(shop::owned(&j.progress, 23));
        assert!(j.progress.float(&["dt"]) >= played);
    }

    // Episode 1: a job world (row 3) has an exit back to the hub (row 2); firing it asks for the hub at its point.
    let mut ep1 = Block::new(tree.clone());
    ep1.set_named(&["dt"], None, SaveValue::Float(1.0));
    let mut g3 = enter(d, 3, ep1, None, flow::DEFAULT_POINT);
    let exit = jobs(&g3).exits.iter().find(|e| e.row == Some(2)).map(|e| e.id).expect("an exit to the hub");
    sly2_jobs::with(&mut g3.world, |j, w| j.fire_exit_id(w, exit));
    let req = sly2_jobs::with(&mut g3.world, |j, _| j.load.take()).flatten().expect("a load request");
    eprintln!("exit {exit}: {req:?}");
    assert_eq!(req.row, 2);
    let (p, carry) = sly2_jobs::detach(&mut g3.world).unwrap();
    let mut hub = game(d, 2);
    assert!(sly2_jobs::attach(&mut hub.world, p, Some(carry), req.task));
    let at = sly2_jobs::arrive(&mut hub, req.point).expect("the hub's arrival point");
    {
        let j = jobs(&hub);
        assert_eq!((j.progress.int(&["widCurrent"]), j.progress.int(&["episodeCurrent"])), (2, 1));
        assert!(j.progress.int(&["episodeBest"]) >= 1);
        assert_eq!(j.respawn, req.point);
        assert!(j.exits.iter().all(|e| e.state == flow::ExitState::Inside));
        eprintln!("hub: {} jobs, {} tasks, arrival at {:?}", j.graph.jobs.len(), j.graph.tasks.len(), at.t);
        // The safe-house computer offers the episode-1 items not owned.
        let offered = shop::buy_list(&j.progress, &d.items);
        assert!(!offered.is_empty() && offered.iter().all(|&n| d.items.get(n).unwrap().episode == 1));
        // The job help lists the open jobs.
        eprintln!("job help: {:?}", shop::job_help(j));
    }
    // Buying at the computer: refused without coins, then paid; the item is owned and leaves the Buy page.
    let item = shop::buy_list(&jobs(&hub).progress, &d.items)[0];
    let price = d.items.get(item).unwrap().price;
    sly2_jobs::with(&mut hub.world, |j, _| {
        assert_eq!(shop::buy(j, &d.items, item), Err(shop::ShopError::NotEnoughCoins));
        shop::set_coins(&mut j.progress, price + 10);
        assert_eq!(shop::buy(j, &d.items, item), Ok(()));
        assert_eq!(shop::coins(&j.progress), 10);
        assert!(shop::owned(&j.progress, item as i32));
        assert!(!shop::buy_list(&j.progress, &d.items).contains(&item));
        // Selling one loot item adds its value.
        shop::add_loot(&mut j.progress, 0, 2);
        assert_eq!(shop::sell(&mut j.progress, &d.loot, 0), Ok(()));
        assert_eq!((shop::loot(&j.progress, 0), shop::coins(&j.progress)), (1, 10 + d.loot.records[0].value));
    });
    run(&mut hub, 10);
    // Play time also counts for the episode (the level row's save path names the episode's group).
    assert!(jobs(&hub).progress.float(&["episode", "nightclub", "dt"]) > 0.0);
    for g in [&g, &g2, &g3, &hub] {
        let host: Vec<_> = g.script_error_kinds.iter().filter(|k| matches!(k.0, ScriptError::Host(_))).collect();
        assert!(host.is_empty(), "host errors {host:?}");
    }
}
