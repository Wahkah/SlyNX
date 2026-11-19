//! Sly 2 state machines on the player's own Sly 2 (USA) disc (`SLYNX_DISC_SLY2`); skipped when it is not set.
//! Every state machine of every world reads to the start of its option list; in the nightclub hub the safe house's
//! machine walks to its computer state, the characters' mouth machines take a voice line's goals, and a task's goal
//! from the job system reaches its machine.

use std::sync::OnceLock;

use engine::anim::sly2_machine::MachineDef;
use engine::game::progress::Progress;
use engine::game::sly2_anim::{self, SPEAKER_MACHINES};
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly2::Sly2Options;
use engine::world::{World, WorldInfo};
use formats_sly2::Disc;
use formats_sly2::exe::OptionTable;
use formats_sly2::stream::{Record, visit};
use harness::golden::{Game as Title, require_disc};
use script::{BuiltinTable, OptionMap};

struct DiscData {
    options: OptionTable,
    map: OptionMap,
    builtins: BuiltinTable,
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly2, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let rows: Vec<usize> = d.world_rows().collect();
        let worlds = rows.into_iter().map(|r| (r, d.rows[r].world_id(), d.read_world(r).expect("world"))).collect();
        DiscData {
            options: d.options.clone(),
            map: OptionMap::from_sly2_executable(&d.executable).expect("option map"),
            builtins: BuiltinTable::from_executable(&d.executable).expect("built-ins"),
            worlds,
        }
    }))
}

fn game(d: &DiscData, row: usize) -> Game {
    let (_, id, bytes) = d.worlds.iter().find(|w| w.0 == row).expect("row");
    let info = WorldInfo { index: row as u32, id: *id, task_mask: 0, identity: *id };
    let opts =
        Sly2Options { row: Some(row as i16), info, builtins: Some(d.builtins.clone()), ..Sly2Options::default() };
    let w = World::load_sly2(bytes.clone(), &d.options, opts).expect("world loads");
    Game::new(
        w,
        Progress::new_game(),
        Systems::null(),
        GameConfig { seed: 5, option_map: Some(d.map.clone()), no_scripts: false },
    )
}

#[test]
fn every_machine_reads_to_its_options() {
    let Some(d) = disc("every_machine_reads_to_its_options") else { return };
    let (mut machines, mut states, mut transitions) = (0, 0, 0);
    for (row, _, bytes) in &d.worlds {
        let w = formats_sly2::World::parse(bytes, &d.options).expect("world parses");
        for b in &w.bodies {
            visit(&b.objects, &mut |r: &Record| {
                if r.class != 111 {
                    return;
                }
                let (m, end) =
                    MachineDef::read_with_end(bytes, r).unwrap_or_else(|| panic!("row {row}: machine {}", r.id));
                assert_eq!(Some(end), r.options().map(|o| o.start), "row {row}: machine {} ends at its options", r.id);
                assert!(m.transitions.iter().all(|t| t.object == -1 || m.members.contains(&t.object)));
                machines += 1;
                states += m.states.len();
                transitions += m.transitions.len();
            });
        }
    }
    eprintln!("{machines} state machines, {states} states, {transitions} transitions kept");
    assert_eq!(machines, 2603, "classes/animation.md 4");
}

#[test]
fn machines_walk_to_their_goals_in_the_hub() {
    let Some(d) = disc("machines_walk_to_their_goals_in_the_hub") else { return };
    let mut g = game(d, 2);
    let input = FrameInput::default();
    // The safe house (class 70, object 17239) and its machine (option 1337, object 27339; safehouse-and-shop.md 1).
    let house = g.world.slots_of_class(70).iter().find_map(|&s| g.world.handle(s)).expect("safe house");
    assert_eq!(g.world.sly2_id(house), Some(17239));
    let machine = g.world.options(house).and_then(|o| o.i32(1337)).expect("option 1337");
    assert_eq!(machine, 27339);
    let p = sly2_anim::with(&mut g.world, |a, w| a.find_machine(w, house, machine)).flatten().expect("a playback");
    let state = |g: &Game| g.world.sly2.as_deref().unwrap().anim.machines[&p.index()].run.state;
    eprintln!("safe house machine starts in state {}", state(&g));
    sly2_anim::with(&mut g.world, |a, _| a.goal(p, 10, false));
    for _ in 0..600 {
        g.step_fixed(&input);
        if state(&g) == 10 {
            break;
        }
    }
    assert_eq!(state(&g), 10, "the computer state is reached");
    // The characters' speaker machines: a mouth goal (target 5) from a voice line.
    for class in [53i16, 54, 55] {
        let Some(speaker) = sly2_anim::speaker(&g.world, class) else { continue };
        let found: Vec<bool> = SPEAKER_MACHINES
            .iter()
            .map(|&m| m >= 0 && sly2_anim::with(&mut g.world, |a, w| a.find_machine(w, speaker, m)).flatten().is_some())
            .collect();
        eprintln!("class {class}: speaker machines {found:?}");
        assert!(found[5], "class {class} has a mouth machine");
        sly2_anim::with(&mut g.world, |a, w| a.cue(w, speaker, 5, Some(5372), None));
        sly2_anim::with(&mut g.world, |a, w| a.cue(w, speaker, 3, None, Some(0.5)));
        let eyes = g.world.sly2.as_deref().unwrap().anim.eyes.get(&speaker.index()).copied();
        assert_eq!(eyes, Some(0.5));
    }
    for _ in 0..30 {
        g.step_fixed(&input);
    }
    let a = &g.world.sly2.as_deref().unwrap().anim;
    eprintln!(
        "{} machine playbacks, {} clip playbacks, {} playing",
        a.machines.len(),
        a.clips.len(),
        a.playing(&g.world).len()
    );
    assert!(!a.playing(&g.world).is_empty(), "something plays");
}
