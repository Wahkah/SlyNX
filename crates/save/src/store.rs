//! Slot and settings files in the user folder, through the platform's [`FileSystem`].
//!
//! Files (relative to the user folder, `sdmc:/switch/SlyNX/user/` on the Switch):
//!
//! - `saves/sly1/slot1.sav` .. `slot3.sav`: the slots;
//! - `saves/sly1/slot1.bak` .. `slot3.bak`: the previous good contents of each slot;
//! - `settings.bin`: the port's options.
//!
//! Every write goes through `FileSystem::write_user`, which writes a temporary file and then replaces the old one,
//! so a crash leaves either the old or the new file. Before a slot is replaced, its current contents, if they read
//! back correctly, are copied to the `.bak` file; a slot that fails its checksum is loaded from there instead.

use crate::autosave::{SaveRequest, Session, Slot};
use crate::codec::DecodeError;
use crate::completion::{CompletionTable, SlotSummary};
use crate::model::SlotState;
use crate::settings::PortSettings;
use crate::slotfile;
use alloc::format;
use alloc::string::String;
use platform_api::{FileSystem, FsError, Root};

pub const SAVE_DIR: &str = "saves/sly1";
pub const SETTINGS_FILE: &str = "settings.bin";

/// Why a slot could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// No save in this slot.
    Empty,
    /// The slot (and its backup, if any) is damaged.
    Damaged(DecodeError),
    /// Written by a newer SlyNX that this build cannot read. The file is left alone.
    TooNew(DecodeError),
    Io(FsError),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::Empty => f.write_str("empty slot"),
            LoadError::Damaged(e) | LoadError::TooNew(e) => write!(f, "{e}"),
            LoadError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// A loaded slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub state: SlotState,
    /// The file's format version.
    pub version: u16,
    /// The slot file was damaged; this is its backup (the previous save).
    pub from_backup: bool,
}

/// The save files of one game.
#[derive(Clone, Debug)]
pub struct SaveStore<F: FileSystem> {
    fs: F,
}

pub fn slot_path(slot: Slot) -> String {
    format!("{SAVE_DIR}/slot{}.sav", slot.index() + 1)
}

pub fn backup_path(slot: Slot) -> String {
    format!("{SAVE_DIR}/slot{}.bak", slot.index() + 1)
}

enum Read {
    Missing,
    Bytes(alloc::vec::Vec<u8>),
}

impl<F: FileSystem> SaveStore<F> {
    pub fn new(fs: F) -> SaveStore<F> {
        SaveStore { fs }
    }

    fn read(&self, path: &str) -> Result<Read, FsError> {
        match self.fs.read(Root::User, path) {
            Ok(b) => Ok(Read::Bytes(b)),
            Err(FsError::NotFound) => Ok(Read::Missing),
            Err(e) => Err(e),
        }
    }

    /// Loads a slot, falling back to its backup when the slot file is damaged or missing.
    pub fn load(&self, slot: Slot) -> Result<Loaded, LoadError> {
        let main = self.read(&slot_path(slot)).map_err(LoadError::Io)?;
        let main_err = match &main {
            Read::Bytes(b) => match slotfile::decode(b) {
                Ok(d) => return Ok(Loaded { state: d.state, version: d.version, from_backup: false }),
                Err(e @ DecodeError::TooNew { .. }) => return Err(LoadError::TooNew(e)),
                Err(e) => Some(e),
            },
            Read::Missing => None,
        };
        match self.read(&backup_path(slot)).map_err(LoadError::Io)? {
            Read::Bytes(b) => match slotfile::decode(&b) {
                Ok(d) => Ok(Loaded { state: d.state, version: d.version, from_backup: true }),
                Err(e) => Err(LoadError::Damaged(main_err.unwrap_or(e))),
            },
            Read::Missing => match main_err {
                Some(e) => Err(LoadError::Damaged(e)),
                None => Err(LoadError::Empty),
            },
        }
    }

    /// Saves `state` to `slot`, keeping the previous good save as the backup.
    pub fn save(&self, slot: Slot, state: &SlotState) -> Result<(), FsError> {
        if let Read::Bytes(old) = self.read(&slot_path(slot))?
            && slotfile::decode(&old).is_ok()
        {
            self.fs.write_user(&backup_path(slot), &old)?;
        }
        self.fs.write_user(&slot_path(slot), &slotfile::encode(state))
    }

    /// Writes a session's save request and tells the session.
    pub fn commit(&self, session: &mut Session, request: SaveRequest) -> Result<(), FsError> {
        self.save(request.slot, &session.state)?;
        session.saved(&request);
        Ok(())
    }

    /// Erases a slot and its backup ("Do you want to erase this game?").
    pub fn erase(&self, slot: Slot) -> Result<(), FsError> {
        self.fs.remove_user(&slot_path(slot))?;
        self.fs.remove_user(&backup_path(slot))
    }

    /// What the slot list shows for `slot`. `world_name(episode, level)` names the world the slot resumes in.
    pub fn summary(&self, slot: Slot, table: &CompletionTable, world_name: impl Fn(u8, u8) -> String) -> SlotSummary {
        match self.load(slot) {
            Ok(l) => SlotSummary::of(&l.state, table, &world_name(l.state.episode, l.state.level)),
            Err(LoadError::Empty) => SlotSummary::Empty,
            Err(_) => SlotSummary::Unreadable,
        }
    }

    /// The three slots' summaries.
    pub fn summaries(&self, table: &CompletionTable, world_name: impl Fn(u8, u8) -> String) -> [SlotSummary; 3] {
        Slot::ALL.map(|s| self.summary(s, table, &world_name))
    }

    /// The port's options; the defaults when the file is missing or unreadable.
    pub fn load_settings(&self) -> PortSettings {
        match self.read(SETTINGS_FILE) {
            Ok(Read::Bytes(b)) => PortSettings::decode(&b).unwrap_or_default(),
            _ => PortSettings::default(),
        }
    }

    pub fn save_settings(&self, settings: &PortSettings) -> Result<(), FsError> {
        self.fs.write_user(SETTINGS_FILE, &settings.encode())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::Settings;
    use crate::slotfile::tests::busy_state;
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use platform_api::DataFile;
    use std::sync::Mutex;

    /// An in-memory user folder.
    #[derive(Clone, Default)]
    pub(crate) struct MemFs(pub Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

    pub(crate) struct MemFile(Vec<u8>);

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
        fn open(&self, root: Root, path: &str) -> Result<MemFile, FsError> {
            assert_eq!(root, Root::User);
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

    fn slot(i: u8) -> Slot {
        Slot::new(i).unwrap()
    }

    #[test]
    fn save_load_erase() {
        let fs = MemFs::default();
        let store = SaveStore::new(fs.clone());
        assert_eq!(store.load(slot(0)), Err(LoadError::Empty));
        let s = busy_state();
        store.save(slot(0), &s).unwrap();
        assert_eq!(store.load(slot(0)).unwrap(), Loaded { state: s.clone(), version: 1, from_backup: false });
        assert!(fs.exists(Root::User, "saves/sly1/slot1.sav"));
        assert!(!fs.exists(Root::User, "saves/sly1/slot1.bak"), "nothing to back up on the first save");
        assert_eq!(store.load(slot(1)), Err(LoadError::Empty));
        store.erase(slot(0)).unwrap();
        assert_eq!(store.load(slot(0)), Err(LoadError::Empty));
    }

    #[test]
    fn damaged_slot_falls_back_to_the_previous_save() {
        let fs = MemFs::default();
        let store = SaveStore::new(fs.clone());
        let older = SlotState::new_game();
        let newer = busy_state();
        store.save(slot(2), &older).unwrap();
        store.save(slot(2), &newer).unwrap();
        // Damage the slot file.
        {
            let mut files = fs.0.lock().unwrap();
            let f = files.get_mut("saves/sly1/slot3.sav").unwrap();
            let n = f.len();
            f[n / 2] ^= 0x40;
        }
        let l = store.load(slot(2)).unwrap();
        assert!(l.from_backup);
        assert_eq!(l.state, older);
        // A damaged slot is not copied over a good backup.
        store.save(slot(2), &newer).unwrap();
        assert_eq!(store.load(slot(2)).unwrap().state, newer);
        assert_eq!(store.load(slot(2)).map(|l| l.from_backup), Ok(false));
        // Both damaged: reported as damaged.
        for name in ["saves/sly1/slot3.sav", "saves/sly1/slot3.bak"] {
            fs.0.lock().unwrap().get_mut(name).unwrap()[30] ^= 1;
        }
        assert_eq!(store.load(slot(2)), Err(LoadError::Damaged(DecodeError::Checksum)));
    }

    #[test]
    fn too_new_is_not_replaced_by_the_backup() {
        let fs = MemFs::default();
        let store = SaveStore::new(fs.clone());
        store.save(slot(0), &SlotState::new_game()).unwrap();
        store.save(slot(0), &busy_state()).unwrap();
        let future = crate::codec::seal(crate::codec::SLOT_MAGIC, 9, 9, 1, &[]);
        fs.write_user("saves/sly1/slot1.sav", &future).unwrap();
        assert!(matches!(store.load(slot(0)), Err(LoadError::TooNew(_))));
    }

    #[test]
    fn summaries_and_commit() {
        let fs = MemFs::default();
        let store = SaveStore::new(fs.clone());
        let table = CompletionTable::new([(1, 0, 0xE), (1, 1, 0)], &[]);
        let mut session = Session::new_game(Some(slot(1)), Settings(0));
        session.acknowledge_autosave_notice();
        session.state.play_time = 3725.0;
        let req = session.on_world_loaded(1, 0).unwrap();
        store.commit(&mut session, req).unwrap();
        assert_eq!(session.on_world_loaded(1, 0), None, "commit records the save position");
        fs.write_user("saves/sly1/slot3.sav", b"garbage").unwrap();
        let names = |e: u8, l: u8| format!("World {e}-{l}");
        let s = store.summaries(&table, names);
        assert_eq!(s[0], SlotSummary::Empty);
        // One visit task of 5 done: 20%.
        assert_eq!(s[1].text(), "20% - World 1-0 - 1:02");
        assert_eq!(s[2], SlotSummary::Unreadable);
    }

    #[test]
    fn settings_file() {
        let fs = MemFs::default();
        let store = SaveStore::new(fs.clone());
        assert_eq!(store.load_settings(), PortSettings::default());
        let s = PortSettings { show_fps: true, last_slot: Some(1), ..PortSettings::default() };
        store.save_settings(&s).unwrap();
        assert_eq!(store.load_settings(), s);
        fs.write_user(SETTINGS_FILE, b"junk").unwrap();
        assert_eq!(store.load_settings(), PortSettings::default());
    }
}
