#![cfg(not(target_os = "horizon"))]
//! The save store on a real disk, through the PC platform's file system (temporary file, then replace).

use platform_api::{FileSystem, Root};
use platform_pc::PcFs;
use save::model::Settings;
use save::{LoadError, PortSettings, SaveStore, Session, Slot};
use std::path::PathBuf;

#[test]
fn slots_and_settings_on_disk() {
    let base = std::env::temp_dir().join(format!("slynx-save-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let fs = PcFs::new(base.join("romfs"), base.join("sd"), base.join("user"));
    let store = SaveStore::new(fs.clone());
    let slot = Slot::new(0).unwrap();

    let mut session = Session::new_game(Some(slot), Settings(Settings::MUSIC_OFF));
    session.acknowledge_autosave_notice();
    let req = session.on_world_loaded(1, 0).expect("auto-save on a new level");
    store.commit(&mut session, req).unwrap();
    let req = session.on_key_collected(1, 0).expect("auto-save on a key");
    store.commit(&mut session, req).unwrap();

    let user: PathBuf = base.join("user/saves/sly1");
    assert!(user.join("slot1.sav").is_file());
    assert!(user.join("slot1.bak").is_file(), "the first save became the backup");
    assert!(!user.join("slot1.sav.tmp").exists(), "no temporary file left behind");

    let loaded = store.load(slot).unwrap();
    assert_eq!(loaded.state, session.state);
    assert!(!loaded.state.settings.music());

    // Damage the slot on disk: the backup (before the key) loads.
    let path = user.join("slot1.sav");
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len();
    bytes[n - 10] ^= 0xFF;
    std::fs::write(&path, bytes).unwrap();
    let l = store.load(slot).unwrap();
    assert!(l.from_backup);
    assert_eq!(l.state.episodes[1].keys, 0);

    store.erase(slot).unwrap();
    assert_eq!(store.load(slot), Err(LoadError::Empty));
    assert!(!fs.exists(Root::User, "saves/sly1/slot1.bak"));

    let settings = PortSettings { music_volume: 0.5, ..PortSettings::default() };
    store.save_settings(&settings).unwrap();
    assert_eq!(store.load_settings(), settings);

    std::fs::remove_dir_all(&base).unwrap();
}
