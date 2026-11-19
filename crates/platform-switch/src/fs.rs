//! Files on the SD card (`sdmc:/`) and in the program's RomFS (`romfs:/`), through newlib and libnx's devices.

use crate::sys;
use alloc::vec::Vec;
use core::ffi::c_char;
use platform_api::fs::validate;
use platform_api::{DataFile, FileSystem, FsError, Root};

/// The writable user folder.
pub const USER_DIR: &str = "sdmc:/switch/SlyNX/user/";
const TMP_SUFFIX: &str = ".tmp";

// newlib errno values.
const EPERM: i32 = 1;
const ENOENT: i32 = 2;
const EACCES: i32 = 13;
const ENOTDIR: i32 = 20;

fn map_errno(e: i32) -> FsError {
    match e {
        ENOENT | ENOTDIR => FsError::NotFound,
        EPERM | EACCES => FsError::PermissionDenied,
        _ => FsError::Io(e),
    }
}

fn prefix(root: Root) -> Result<&'static str, FsError> {
    match root {
        Root::RomFs => {
            // SAFETY: no preconditions.
            if unsafe { sys::slynx_romfs_available() } { Ok("romfs:/") } else { Err(FsError::NoRoot) }
        }
        Root::Sd => Ok("sdmc:/"),
        Root::User => Ok(USER_DIR),
    }
}

/// A NUL-terminated device path.
struct CPath(Vec<u8>);

impl CPath {
    fn new(parts: &[&str]) -> CPath {
        let mut v = Vec::with_capacity(parts.iter().map(|p| p.len()).sum::<usize>() + 1);
        for p in parts {
            v.extend_from_slice(p.as_bytes());
        }
        v.push(0);
        CPath(v)
    }

    fn ptr(&self) -> *const c_char {
        self.0.as_ptr().cast()
    }
}

fn full(root: Root, path: &str) -> Result<CPath, FsError> {
    validate(path)?;
    Ok(CPath::new(&[prefix(root)?, path]))
}

/// A file open for reading. Closed on drop.
pub struct SwitchFile {
    fd: i32,
    len: u64,
}

fn open_path(p: &CPath) -> Result<SwitchFile, FsError> {
    let (mut fd, mut len) = (-1, 0u64);
    // SAFETY: `p` is NUL-terminated; the out pointers are live locals.
    let e = unsafe { sys::slynx_fs_open_read(p.ptr(), &mut fd, &mut len) };
    if e != 0 { Err(map_errno(e)) } else { Ok(SwitchFile { fd, len }) }
}

fn open(root: Root, path: &str) -> Result<SwitchFile, FsError> {
    let p = full(root, path)?;
    match open_path(&p) {
        // A user file replaced while the program died between "remove old" and "rename new" survives under the
        // temporary name.
        Err(FsError::NotFound) if root == Root::User => open_path(&CPath::new(&[USER_DIR, path, TMP_SUFFIX])),
        r => r,
    }
}

fn exists(root: Root, path: &str) -> bool {
    match full(root, path) {
        // SAFETY: NUL-terminated path.
        Ok(p) => unsafe { sys::slynx_fs_exists(p.ptr()) },
        Err(_) => false,
    }
}

fn write_user(path: &str, data: &[u8]) -> Result<(), FsError> {
    validate(path)?;
    let dir = match path.rfind('/') {
        Some(i) => CPath::new(&[USER_DIR, &path[..i]]),
        None => CPath::new(&[USER_DIR.trim_end_matches('/')]),
    };
    // SAFETY: NUL-terminated path.
    let e = unsafe { sys::slynx_fs_mkdirs(dir.ptr()) };
    if e != 0 {
        return Err(map_errno(e));
    }
    let target = CPath::new(&[USER_DIR, path]);
    let tmp = CPath::new(&[USER_DIR, path, TMP_SUFFIX]);
    // SAFETY: NUL-terminated paths; `data` is a live slice.
    let e = unsafe { sys::slynx_fs_write_replace(target.ptr(), tmp.ptr(), data.as_ptr().cast(), data.len()) };
    if e != 0 { Err(map_errno(e)) } else { Ok(()) }
}

fn remove_user(path: &str) -> Result<(), FsError> {
    let p = full(Root::User, path)?;
    // SAFETY: NUL-terminated path.
    let e = unsafe { sys::slynx_fs_remove(p.ptr()) };
    if e != 0 { Err(map_errno(e)) } else { Ok(()) }
}

impl DataFile for SwitchFile {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        if buf.is_empty() {
            return Ok(0);
        }
        // SAFETY: `fd` is open (closed only in Drop); `buf` is a live, writable slice of `buf.len()` bytes.
        let n = unsafe { sys::slynx_fs_read_at(self.fd, offset, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 { Err(map_errno((-n) as i32)) } else { Ok(n as usize) }
    }
}

impl Drop for SwitchFile {
    fn drop(&mut self) {
        // SAFETY: `fd` is ours and closed once.
        unsafe { sys::slynx_fs_close(self.fd) };
    }
}

/// File access on the Switch: `romfs:/`, `sdmc:/` and the user folder. A zero-size handle usable from any thread
/// while the platform exists (RomFS is unmounted when the platform is dropped).
#[derive(Clone, Copy, Debug, Default)]
pub struct SwitchFs;

impl FileSystem for SwitchFs {
    type File = SwitchFile;

    fn open(&self, root: Root, path: &str) -> Result<SwitchFile, FsError> {
        open(root, path)
    }

    fn exists(&self, root: Root, path: &str) -> bool {
        exists(root, path)
    }

    fn write_user(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        write_user(path, data)
    }

    fn remove_user(&self, path: &str) -> Result<(), FsError> {
        remove_user(path)
    }
}
