//! The Sly 3 script host (`game::script_host::sly3`) on every world of the player's own Sly 3 disc
//! (`SLYNX_DISC_SLY3`); skipped when it is not set. The Sly 3 interpreter does not evaluate yet, so the host is
//! driven directly: classes, option reads and writes by the executable's table, searches by full ID, the fixed objects
//! and the game operations. None of them gives a host error.

use std::sync::OnceLock;

use engine::game::rng::Rng;
use engine::game::script_host::WorldHost;
use engine::game::systems::NoCollision;
use engine::object::{OptionValue, WORLD_SCRIPT_ID};
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats_sly2::exe::OptionTable;
use formats_sly3::Disc;
use harness::golden::{Game as Title, require_disc};
use script::{ScriptHost, Value};

struct DiscData {
    options: OptionTable,
    multipliers: [f32; 3],
    /// (slot, world id, bytes)
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly3, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let slots: Vec<usize> = d.world_slots().collect();
        let worlds = slots
            .into_iter()
            .map(|s| {
                let id = d.worlds[s].world_id();
                (s, id, d.read_world(s).expect("read world"))
            })
            .collect();
        DiscData { options: d.options.clone(), multipliers: d.tangent_multipliers, worlds }
    }))
}

#[test]
fn host_answers_every_world() {
    let Some(d) = disc("host_answers_every_world") else { return };
    assert_eq!(d.worlds.len(), 39);
    for (slot, id, bytes) in &d.worlds {
        let info = WorldInfo { index: *slot as u32, id: *id, task_mask: 0, identity: *id };
        let opts = Sly3Options { slot: Some(*slot as i16), info, multipliers: d.multipliers, ..Sly3Options::default() };
        let mut w = World::load_sly3(bytes.clone(), &d.options, opts).expect("world loads");
        let objects = w.len();
        let mut rng = Rng::new(7);
        let mut h = WorldHost {
            world: &mut w,
            rng: &mut rng,
            time: 0.0,
            collision: &NoCollision,
            errors: 0,
            kinds: Vec::new(),
        };

        // The world object is class 1.
        assert_eq!(h.class_of(WORLD_SCRIPT_ID), Some(1));
        // Game operations.
        assert!(matches!(h.game_operation(103, &[]), Value::Int(s) if s == *slot as i32));
        assert!(matches!(h.game_operation(108, &[]), Value::Int(0)));
        assert!(matches!(h.game_operation(109, &[]), Value::Int(_)));
        assert!(matches!(h.game_operation(107, &[]), Value::Float(_)));
        // The fixed objects: made once, with their classes, reused afterwards.
        let p = h.player().expect("player object");
        assert_eq!(h.player(), Some(p));
        assert_eq!(h.class_of(p), Some(194));
        let c = h.camera().expect("camera object");
        assert_eq!(h.class_of(c), Some(198));
        for (op, class) in [(126, 155), (145, 158)] {
            let Value::Object(Some(o)) = h.game_operation(op, &[]) else { panic!("operation {op}") };
            assert_eq!(h.class_of(o), Some(class));
        }
        // Pickpocket items.
        assert!(matches!(h.game_operation(146, &[Value::Int(5)]), Value::Bool(false)));
        h.game_operation(147, &[Value::Int(5), Value::Bool(true)]);
        assert!(matches!(h.game_operation(146, &[Value::Int(5)]), Value::Bool(true)));
        // Searches by full ID, and operation 120 over a list of IDs.
        let first = h.world.handles().find(|&x| h.world.sly3_id(x).is_some_and(|i| i > i16::MAX as i32));
        if let Some(x) = first {
            let id = h.world.sly3_id(x).unwrap();
            let found = h.find_object_id(id, None, false).expect("found by its full ID");
            assert_eq!(h.world.sly3_id(engine::game::script_host::handle_of(h.world, found).unwrap()), Some(id));
            let list = h.game_operation(120, &[Value::list([Value::Int(id), Value::Int(-12345)]), Value::Object(None)]);
            assert_eq!(list.to_vec().len(), 1);
        }
        // Options: an unset option reads as its type's zero (or a method); a write reads back.
        let obj = h.world.handles().find(|&x| h.world.sly3_id(x).is_some()).unwrap().to_script();
        for option in 0..64u16 {
            assert!(!matches!(h.get_option(obj, option), Value::Error(_)), "option {option}");
        }
        let float_option = (0..2000u16).find(|&o| {
            let l = h.world.sly3.as_deref().unwrap();
            l.table.get(o as i16).is_some_and(|e| e.value_type == 1 && !e.is_typed() && e.flags & 1 == 0)
        });
        if let Some(o) = float_option {
            h.set_option(obj, o, &Value::Float(2.5)).unwrap();
            assert!(matches!(h.get_option(obj, o), Value::Float(v) if v == 2.5));
            let hd = engine::game::script_host::handle_of(h.world, obj).unwrap();
            assert_eq!(h.world.options(hd).unwrap().get(o), Some(&OptionValue::Float(2.5)));
        }
        assert_eq!(h.errors, 0);
        assert!(!matches!(h.game_operation(110, &[]), Value::Error(_)));
        // Four fixed objects were added.
        assert_eq!(w.len(), objects + 4);
    }
}
