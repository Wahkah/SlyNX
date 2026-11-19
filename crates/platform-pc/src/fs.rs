//! Files on the PC: three folders standing in for RomFS, the SD card and the user folder.

use platform_api::fs::validate;
use platform_api::{DataFile, FileSystem, FsError, Root};
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TMP_SUFFIX: &str = ".tmp";

/// File access on the PC. Cheap to clone; usable from any thread.
#[derive(Clone, Debug)]
pub struct PcFs {
    roots: Arc<[PathBuf; 3]>,
}

fn index(root: Root) -> usize {
    match root {
        Root::RomFs => 0,
        Root::Sd => 1,
        Root::User => 2,
    }
}

fn map_err(e: std::io::Error) -> FsError {
    match e.kind() {
        ErrorKind::NotFound => FsError::NotFound,
        ErrorKind::PermissionDenied => FsError::PermissionDenied,
        ErrorKind::UnexpectedEof => FsError::UnexpectedEof,
        _ => FsError::Io(e.raw_os_error().unwrap_or(-1)),
    }
}

impl PcFs {
    pub fn new(romfs: PathBuf, sd: PathBuf, user: PathBuf) -> PcFs {
        PcFs { roots: Arc::new([romfs, sd, user]) }
    }

    /// The folder that stands in for `root`.
    pub fn root_dir(&self, root: Root) -> &Path {
        &self.roots[index(root)]
    }

    fn path(&self, root: Root, path: &str) -> Result<PathBuf, FsError> {
        validate(path)?;
        let mut p = self.roots[index(root)].clone();
        p.extend(path.split('/'));
        Ok(p)
    }
}

fn tmp_of(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(TMP_SUFFIX);
    PathBuf::from(s)
}

/// A file open for reading.
pub struct PcFile {
    file: File,
    len: u64,
}

fn open_file(p: &Path) -> Result<PcFile, FsError> {
    let file = File::open(p).map_err(map_err)?;
    let meta = file.metadata().map_err(map_err)?;
    if meta.is_dir() {
        return Err(FsError::NotFound);
    }
    Ok(PcFile { file, len: meta.len() })
}

impl FileSystem for PcFs {
    type File = PcFile;

    fn open(&self, root: Root, path: &str) -> Result<PcFile, FsError> {
        let p = self.path(root, path)?;
        match open_file(&p) {
            Err(FsError::NotFound) if root == Root::User => open_file(&tmp_of(&p)),
            r => r,
        }
    }

    fn exists(&self, root: Root, path: &str) -> bool {
        self.path(root, path).is_ok_and(|p| p.exists())
    }

    fn write_user(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        let p = self.path(Root::User, path)?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(map_err)?;
        }
        let tmp = tmp_of(&p);
        let mut f = File::create(&tmp).map_err(map_err)?;
        f.write_all(data).map_err(map_err)?;
        f.sync_all().map_err(map_err)?;
        drop(f);
        // `rename` replaces an existing file atomically on both Linux and Windows.
        std::fs::rename(&tmp, &p).map_err(map_err)
    }

    fn remove_user(&self, path: &str) -> Result<(), FsError> {
        let p = self.path(Root::User, path)?;
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

impl DataFile for PcFile {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        #[cfg(unix)]
        let r = std::os::unix::fs::FileExt::read_at(&self.file, buf, offset);
        #[cfg(windows)]
        let r = std::os::windows::fs::FileExt::seek_read(&self.file, buf, offset);
        r.map_err(map_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("slynx-pcfs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn roots_reads_and_atomic_writes() {
        let base = scratch("rw");
        let fs = PcFs::new(base.join("romfs"), base.join("sd"), base.join("user"));
        std::fs::create_dir_all(base.join("romfs/data")).unwrap();
        let big: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7) as u8).collect();
        std::fs::write(base.join("romfs/data/big.bin"), &big).unwrap();

        assert!(fs.exists(Root::RomFs, "data/big.bin"));
        assert_eq!(fs.read(Root::RomFs, "data/big.bin").unwrap(), big);
        let mut f = fs.open(Root::RomFs, "data/big.bin").unwrap();
        let mut part = [0u8; 4];
        f.read_exact_at(1000, &mut part).unwrap();
        assert_eq!(part, big[1000..1004]);
        assert_eq!(fs.open(Root::Sd, "missing").err(), Some(FsError::NotFound));
        assert_eq!(fs.open(Root::RomFs, "../escape").err(), Some(FsError::BadPath));

        fs.write_user("saves/slot1.dat", b"one").unwrap();
        fs.write_user("saves/slot1.dat", b"two").unwrap();
        assert_eq!(fs.read(Root::User, "saves/slot1.dat").unwrap(), b"two");
        assert!(!base.join("user/saves/slot1.dat.tmp").exists());

        // A crash between writing the temporary file and renaming it: the reader falls back to the new data.
        std::fs::write(base.join("user/saves/slot2.dat.tmp"), b"new").unwrap();
        assert_eq!(fs.read(Root::User, "saves/slot2.dat").unwrap(), b"new");

        fs.remove_user("saves/slot1.dat").unwrap();
        fs.remove_user("saves/slot1.dat").unwrap();
        assert!(!fs.exists(Root::User, "saves/slot1.dat"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
