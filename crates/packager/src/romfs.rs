//! RomFS: the read-only file system packed into an `.nro` (and read on the Switch through libnx's `romfs:` device).
//!
//! Layout written here (all integers little-endian):
//!
//! | Part | Where |
//! |---|---|
//! | Header: ten `u64` (header size 0x50, then offset and size of the directory hash table, directory table, file hash table and file table, then the file data offset) | 0 |
//! | File data, each file at a 16-byte aligned offset relative to the data start | [`DATA_OFFSET`] (0x200) |
//! | Directory hash table, directory table, file hash table, file table | after the data, 4-byte aligned |
//!
//! A directory entry is `parent, sibling, first child directory, first file, next in hash bucket, name length`
//! (six `u32`) followed by the name padded to 4 bytes; a file entry is `parent, sibling` (`u32`), data offset and
//! size (`u64`), `next in hash bucket, name length` (`u32`) and the padded name. "None" is `0xFFFFFFFF`. Entries are
//! referred to by their byte offset in their table; the root directory is at offset 0 with an empty name and parent
//! 0. A hash bucket holds the offset of the first entry whose [`hash`] falls in it; entries chain through "next in
//! hash bucket".
//!
//! Directories are numbered depth-first (parents before children, names sorted bytewise); files are grouped by
//! directory in that order with the root directory's files last, and their data follows the same order. This is
//! the order devkitPro's `build_romfs` and `elf2nro` produce (the tests compare the bytes when those tools are
//! installed).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Size of the header.
pub const HEADER_SIZE: u64 = 0x50;
/// Where file data starts.
pub const DATA_OFFSET: u64 = 0x200;
/// Alignment of each file's data.
pub const FILE_ALIGN: u64 = 0x10;
/// "No entry".
pub const NONE: u32 = 0xFFFF_FFFF;

/// The hash that places an entry in its table's buckets: seeded with the parent's offset, then each name byte
/// rotated in.
pub fn hash(parent: u32, name: &[u8]) -> u32 {
    let mut h = parent ^ 123_456_789;
    for &b in name {
        h = h.rotate_right(5) ^ b as u32;
    }
    h
}

/// Number of hash buckets for `entries` entries: at least 3, odd below 19, and otherwise the first count not
/// divisible by a prime up to 17.
pub fn bucket_count(entries: usize) -> usize {
    if entries < 3 {
        3
    } else if entries < 19 {
        entries | 1
    } else {
        let mut n = entries;
        while [2, 3, 5, 7, 11, 13, 17].iter().any(|&p| n.is_multiple_of(p)) {
            n += 1;
        }
        n
    }
}

fn align(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

fn pad4(len: usize) -> usize {
    len.div_ceil(4) * 4
}

/// Where a file's contents come from.
#[derive(Clone, Debug)]
pub enum Source {
    Bytes(Vec<u8>),
    /// A file on disk, copied in a streaming pass. Its size is taken when it is added and checked again when it is
    /// copied.
    File {
        path: PathBuf,
        len: u64,
    },
}

impl Source {
    pub fn len(&self) -> u64 {
        match self {
            Source::Bytes(b) => b.len() as u64,
            Source::File { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn file(path: impl Into<PathBuf>) -> io::Result<Source> {
        let path = path.into();
        let len = std::fs::metadata(&path)?.len();
        Ok(Source::File { path, len })
    }
}

#[derive(Debug, Default)]
struct DirNode {
    name: String,
    dirs: Vec<DirNode>,
    files: Vec<(String, Source)>,
}

/// Collects files for a RomFS image.
#[derive(Debug, Default)]
pub struct RomfsBuilder {
    root: DirNode,
}

fn check_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) || name.len() > 0x300 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("bad RomFS name {name:?}")));
    }
    Ok(())
}

impl RomfsBuilder {
    pub fn new() -> RomfsBuilder {
        RomfsBuilder::default()
    }

    /// Adds a file at `path` (`/`-separated, relative), creating its folders.
    pub fn add(&mut self, path: &str, source: Source) -> io::Result<()> {
        let parts: Vec<&str> = path.split('/').collect();
        for p in &parts {
            check_name(p)?;
        }
        let (file, dirs) = parts.split_last().expect("split always yields one part");
        let mut node = &mut self.root;
        for d in dirs {
            if node.files.iter().any(|(n, _)| n == d) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{d} is a file")));
            }
            let i = match node.dirs.iter().position(|x| x.name == *d) {
                Some(i) => i,
                None => {
                    node.dirs.push(DirNode { name: d.to_string(), ..DirNode::default() });
                    node.dirs.len() - 1
                }
            };
            node = &mut node.dirs[i];
        }
        if node.files.iter().any(|(n, _)| n == file) || node.dirs.iter().any(|d| d.name == *file) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{path} added twice")));
        }
        node.files.push((file.to_string(), source));
        Ok(())
    }

    /// Adds every file under `dir`, recursively.
    pub fn add_dir(&mut self, dir: &Path) -> io::Result<()> {
        fn walk(b: &mut RomfsBuilder, base: &Path, dir: &Path) -> io::Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    walk(b, base, &path)?;
                } else {
                    let rel = path.strip_prefix(base).expect("inside base");
                    let rel: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into()).collect();
                    b.add(&rel.join("/"), Source::file(&path)?)?;
                }
            }
            Ok(())
        }
        walk(self, dir, dir)
    }

    /// Lays the image out.
    pub fn build(mut self) -> RomfsImage {
        sort(&mut self.root);
        RomfsImage::layout(self.root)
    }
}

fn sort(node: &mut DirNode) {
    node.dirs.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    node.files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for d in &mut node.dirs {
        sort(d);
    }
}

struct DirRec {
    parent: u32,
    sibling: u32,
    child_dir: u32,
    child_file: u32,
    name: String,
}

struct FileRec {
    parent: u32,
    sibling: u32,
    data_off: u64,
    name: String,
    source: Source,
}

/// A laid-out RomFS image, ready to be written.
pub struct RomfsImage {
    dirs: Vec<(u32, DirRec)>,
    files: Vec<(u32, FileRec)>,
    data_size: u64,
    header: [u64; 10],
}

impl RomfsImage {
    fn layout(root: DirNode) -> RomfsImage {
        // Flatten depth-first, parents before children: (name, parent index, child directory indices, files).
        type Flat = (String, usize, Vec<usize>, Vec<(String, Source)>);
        fn flatten(node: DirNode, parent: usize, flat: &mut Vec<Flat>) {
            let me = flat.len();
            flat.push((node.name, parent, Vec::new(), node.files));
            if me != 0 {
                flat[parent].2.push(me);
            }
            for child in node.dirs {
                flatten(child, me, flat);
            }
        }
        let mut flat: Vec<Flat> = Vec::new();
        flatten(root, 0, &mut flat);
        let mut dirs: Vec<(u32, DirRec)> = Vec::new();
        let mut dir_off = 0u32;
        // Directory offsets.
        let mut offsets = Vec::with_capacity(flat.len());
        for (name, ..) in &flat {
            offsets.push(dir_off);
            dir_off += (0x18 + pad4(name.len())) as u32;
        }
        // Files: grouped by directory in directory order, the root's files last.
        let mut file_off = 0u32;
        let mut data = 0u64;
        let mut files: Vec<(u32, FileRec)> = Vec::new();
        let mut first_file = vec![NONE; flat.len()];
        let order = (1..flat.len()).chain([0]);
        for i in order {
            let list = std::mem::take(&mut flat[i].3);
            let n = list.len();
            for (k, (name, source)) in list.into_iter().enumerate() {
                if k == 0 {
                    first_file[i] = file_off;
                }
                let size = (0x20 + pad4(name.len())) as u32;
                data = align(data, FILE_ALIGN);
                let rec = FileRec {
                    parent: offsets[i],
                    sibling: if k + 1 < n { file_off + size } else { NONE },
                    data_off: data,
                    name,
                    source,
                };
                data += rec.source.len();
                files.push((file_off, rec));
                file_off += size;
            }
        }
        for (i, (name, parent, children, _)) in flat.iter().enumerate() {
            let sibling = if i == 0 {
                NONE
            } else {
                let siblings = &flat[*parent].2;
                let pos = siblings.iter().position(|&c| c == i).unwrap();
                siblings.get(pos + 1).map_or(NONE, |&s| offsets[s])
            };
            dirs.push((
                offsets[i],
                DirRec {
                    parent: offsets[*parent],
                    sibling,
                    child_dir: children.first().map_or(NONE, |&c| offsets[c]),
                    child_file: first_file[i],
                    name: name.clone(),
                },
            ));
        }

        let dir_hash_size = (bucket_count(dirs.len()) * 4) as u64;
        let file_hash_size = (bucket_count(files.len()) * 4) as u64;
        let dir_table_size = dir_off as u64;
        let file_table_size = file_off as u64;
        let dir_hash_off = align(DATA_OFFSET + data, 4);
        let dir_table_off = dir_hash_off + dir_hash_size;
        let file_hash_off = dir_table_off + dir_table_size;
        let file_table_off = file_hash_off + file_hash_size;
        let header = [
            HEADER_SIZE,
            dir_hash_off,
            dir_hash_size,
            dir_table_off,
            dir_table_size,
            file_hash_off,
            file_hash_size,
            file_table_off,
            file_table_size,
            DATA_OFFSET,
        ];
        RomfsImage { dirs, files, data_size: data, header }
    }

    /// Total size in bytes.
    pub fn size(&self) -> u64 {
        self.header[7] + self.header[8]
    }

    /// Bytes of file data (without metadata).
    pub fn data_size(&self) -> u64 {
        self.data_size
    }

    /// Number of files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn metadata(&self) -> Vec<u8> {
        let [_, dh_off, dh_size, _, dt_size, _, fh_size, _, ft_size, _] = self.header;
        let mut out = Vec::with_capacity((self.size() - dh_off) as usize);
        let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());

        // Hash chains: each bucket points at its first entry; later entries in the same bucket chain through
        // `next`. Entries are inserted in table order and prepended, as the reference tools do.
        let dir_buckets = (dh_size / 4) as usize;
        let mut dir_table = vec![NONE; dir_buckets];
        let mut dir_next = Vec::with_capacity(self.dirs.len());
        for (off, d) in &self.dirs {
            let b = hash(d.parent, d.name.as_bytes()) as usize % dir_buckets;
            dir_next.push(dir_table[b]);
            dir_table[b] = *off;
        }
        let file_buckets = (fh_size / 4) as usize;
        let mut file_table = vec![NONE; file_buckets];
        let mut file_next = Vec::with_capacity(self.files.len());
        for (off, f) in &self.files {
            let b = hash(f.parent, f.name.as_bytes()) as usize % file_buckets;
            file_next.push(file_table[b]);
            file_table[b] = *off;
        }

        for v in &dir_table {
            put(&mut out, *v);
        }
        let start = out.len();
        for ((_, d), next) in self.dirs.iter().zip(&dir_next) {
            for v in [d.parent, d.sibling, d.child_dir, d.child_file, *next, d.name.len() as u32] {
                put(&mut out, v);
            }
            out.extend_from_slice(d.name.as_bytes());
            out.resize(out.len() + pad4(d.name.len()) - d.name.len(), 0);
        }
        debug_assert_eq!((out.len() - start) as u64, dt_size);
        for v in &file_table {
            put(&mut out, *v);
        }
        let start = out.len();
        for ((_, f), next) in self.files.iter().zip(&file_next) {
            put(&mut out, f.parent);
            put(&mut out, f.sibling);
            out.extend_from_slice(&f.data_off.to_le_bytes());
            out.extend_from_slice(&f.source.len().to_le_bytes());
            put(&mut out, *next);
            put(&mut out, f.name.len() as u32);
            out.extend_from_slice(f.name.as_bytes());
            out.resize(out.len() + pad4(f.name.len()) - f.name.len(), 0);
        }
        debug_assert_eq!((out.len() - start) as u64, ft_size);
        out
    }

    /// Writes the image. `progress(done, total)` is called as file data is copied.
    pub fn write_to(&self, w: &mut impl Write, mut progress: impl FnMut(u64, u64)) -> io::Result<()> {
        let mut head = Vec::with_capacity(DATA_OFFSET as usize);
        for v in self.header {
            head.extend_from_slice(&v.to_le_bytes());
        }
        head.resize(DATA_OFFSET as usize, 0);
        w.write_all(&head)?;

        let mut pos = 0u64;
        let mut buf = vec![0u8; 8 << 20];
        for (_, f) in &self.files {
            if f.data_off > pos {
                w.write_all(&vec![0u8; (f.data_off - pos) as usize])?;
                pos = f.data_off;
            }
            match &f.source {
                Source::Bytes(b) => {
                    w.write_all(b)?;
                    pos += b.len() as u64;
                }
                Source::File { path, len } => {
                    let mut file = File::open(path)?;
                    if file.metadata()?.len() != *len {
                        return Err(io::Error::other(format!("{} changed size while packing", path.display())));
                    }
                    let mut left = *len;
                    while left > 0 {
                        let n = (buf.len() as u64).min(left) as usize;
                        file.read_exact(&mut buf[..n])?;
                        w.write_all(&buf[..n])?;
                        left -= n as u64;
                        pos += n as u64;
                        progress(pos, self.data_size);
                    }
                }
            }
        }
        let meta_start = self.header[1] - DATA_OFFSET;
        w.write_all(&vec![0u8; (meta_start - pos) as usize])?;
        w.write_all(&self.metadata())?;
        Ok(())
    }

    /// The whole image in memory (for small images and tests).
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        let mut v = Vec::with_capacity(self.size() as usize);
        self.write_to(&mut v, |_, _| {})?;
        Ok(v)
    }
}

/// Reads a RomFS image's metadata the way the console does (hash lookups), for checking output.
#[derive(Debug)]
pub struct RomfsReader {
    header: [u64; 10],
    meta: Vec<u8>,
    meta_base: u64,
}

fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], off: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(off..off + 8)?.try_into().ok()?))
}

/// A file found by [`RomfsReader::lookup`]: its data's offset from the start of the RomFS image, and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSpan {
    pub offset: u64,
    pub size: u64,
}

impl RomfsReader {
    /// Reads the header and metadata through `read(offset, len)`, which reads from the start of the image.
    pub fn open(mut read: impl FnMut(u64, usize) -> io::Result<Vec<u8>>) -> io::Result<RomfsReader> {
        let bad = |why: &str| io::Error::new(io::ErrorKind::InvalidData, format!("RomFS: {why}"));
        let h = read(0, HEADER_SIZE as usize)?;
        let mut header = [0u64; 10];
        for (i, v) in header.iter_mut().enumerate() {
            *v = u64_at(&h, i * 8).ok_or_else(|| bad("short header"))?;
        }
        if header[0] != HEADER_SIZE {
            return Err(bad("header size is not 0x50"));
        }
        let meta_base = header[1];
        let end = header[7] + header[8];
        if header[3] != header[1] + header[2]
            || header[5] != header[3] + header[4]
            || header[7] != header[5] + header[6]
        {
            return Err(bad("tables are not contiguous"));
        }
        if end < meta_base || end - meta_base > 256 << 20 {
            return Err(bad("metadata size out of range"));
        }
        let meta = read(meta_base, (end - meta_base) as usize)?;
        Ok(RomfsReader { header, meta, meta_base })
    }

    fn table(&self, i: usize) -> &[u8] {
        let off = (self.header[i] - self.meta_base) as usize;
        &self.meta[off..off + self.header[i + 1] as usize]
    }

    fn entry_name(table: &[u8], off: usize, fixed: usize) -> Option<&[u8]> {
        let len = u32_at(table, off + fixed - 4)? as usize;
        table.get(off + fixed..off + fixed + len)
    }

    fn find_dir(&self, parent: u32, name: &str) -> Option<u32> {
        let (hashes, table) = (self.table(1), self.table(3));
        let buckets = hashes.len() / 4;
        let mut cur = u32_at(hashes, (hash(parent, name.as_bytes()) as usize % buckets) * 4)?;
        while cur != NONE {
            let off = cur as usize;
            if u32_at(table, off)? == parent && Self::entry_name(table, off, 0x18)? == name.as_bytes() {
                return Some(cur);
            }
            cur = u32_at(table, off + 0x10)?;
        }
        None
    }

    /// Finds `path` (`/`-separated) by hash lookup, as libnx's `romfs:` device does.
    pub fn lookup(&self, path: &str) -> Option<FileSpan> {
        let parts: Vec<&str> = path.split('/').collect();
        let (file, dirs) = parts.split_last()?;
        let mut parent = 0u32;
        for d in dirs {
            parent = self.find_dir(parent, d)?;
        }
        let (hashes, table) = (self.table(5), self.table(7));
        let buckets = hashes.len() / 4;
        let mut cur = u32_at(hashes, (hash(parent, file.as_bytes()) as usize % buckets) * 4)?;
        while cur != NONE {
            let off = cur as usize;
            if u32_at(table, off)? == parent && Self::entry_name(table, off, 0x20)? == file.as_bytes() {
                let data = u64_at(table, off + 8)?;
                let size = u64_at(table, off + 16)?;
                return Some(FileSpan { offset: self.header[9] + data, size });
            }
            cur = u32_at(table, off + 0x18)?;
        }
        None
    }

    /// Every file path, walking the child and sibling links from the root.
    pub fn list(&self) -> Vec<String> {
        let (dirs, files) = (self.table(3), self.table(7));
        let mut out = Vec::new();
        let mut stack = vec![(0u32, String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let off = dir as usize;
            let mut f = u32_at(dirs, off + 0xC).unwrap_or(NONE);
            while f != NONE {
                let name = Self::entry_name(files, f as usize, 0x20).unwrap_or_default();
                out.push(format!("{prefix}{}", String::from_utf8_lossy(name)));
                f = u32_at(files, f as usize + 4).unwrap_or(NONE);
            }
            let mut d = u32_at(dirs, off + 8).unwrap_or(NONE);
            while d != NONE {
                let name = Self::entry_name(dirs, d as usize, 0x18).unwrap_or_default();
                stack.push((d, format!("{prefix}{}/", String::from_utf8_lossy(name))));
                d = u32_at(dirs, d as usize + 4).unwrap_or(NONE);
            }
        }
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reader(img: &[u8]) -> RomfsReader {
        RomfsReader::open(|off, len| Ok(img[off as usize..off as usize + len].to_vec())).unwrap()
    }

    #[test]
    fn bucket_counts() {
        assert_eq!(bucket_count(0), 3);
        assert_eq!(bucket_count(1), 3);
        assert_eq!(bucket_count(4), 5);
        assert_eq!(bucket_count(18), 19);
        assert_eq!(bucket_count(19), 19);
        assert_eq!(bucket_count(20), 23);
        assert_eq!(bucket_count(289), 293); // 289 = 17 * 17, 290 even, 291 = 3 * 97, 292 even
    }

    #[test]
    fn one_file_layout() {
        let mut b = RomfsBuilder::new();
        b.add("hello.txt", Source::Bytes(b"Hello from RomFS\n".to_vec())).unwrap();
        let img = b.build().to_vec().unwrap();
        let h: Vec<u64> = (0..10).map(|i| u64_at(&img, i * 8).unwrap()).collect();
        // Data at 0x200 (0x11 bytes), metadata from 0x214: 3 directory buckets, the root (0x18), 3 file buckets,
        // one file entry (0x20 + "hello.txt" padded to 12).
        assert_eq!(h, [0x50, 0x214, 0xC, 0x220, 0x18, 0x238, 0xC, 0x244, 0x2C, 0x200]);
        assert_eq!(img.len(), 0x270);
        assert_eq!(&img[0x200..0x211], b"Hello from RomFS\n");
        let r = reader(&img);
        assert_eq!(r.lookup("hello.txt"), Some(FileSpan { offset: 0x200, size: 0x11 }));
        assert_eq!(r.lookup("nope.txt"), None);
    }

    #[test]
    fn tree_round_trip() {
        let mut b = RomfsBuilder::new();
        let mut want = Vec::new();
        for i in 0..40 {
            let path = match i % 4 {
                0 => format!("f{i}.bin"),
                1 => format!("a/f{i}.bin"),
                2 => format!("a/deep/er/f{i}.bin"),
                _ => format!("b{}/f{i}.bin", i % 3),
            };
            let data: Vec<u8> = (0..i * 37 + 1).map(|k| (k * 7 + i) as u8).collect();
            b.add(&path, Source::Bytes(data.clone())).unwrap();
            want.push((path, data));
        }
        assert!(b.add("a/deep", Source::Bytes(vec![])).is_err(), "file over a directory");
        assert!(b.add("f0.bin", Source::Bytes(vec![])).is_err(), "duplicate");
        assert!(b.add("x/../y", Source::Bytes(vec![])).is_err(), "dot-dot");
        let image = b.build();
        let img = image.to_vec().unwrap();
        assert_eq!(img.len() as u64, image.size());
        let r = reader(&img);
        for (path, data) in &want {
            let span = r.lookup(path).unwrap_or_else(|| panic!("{path} missing"));
            assert_eq!(span.offset % FILE_ALIGN, 0);
            assert_eq!(&img[span.offset as usize..(span.offset + span.size) as usize], &data[..], "{path}");
        }
        let mut paths: Vec<String> = want.iter().map(|w| w.0.clone()).collect();
        paths.sort();
        assert_eq!(r.list(), paths);
    }

    #[test]
    fn streams_files_from_disk() {
        let dir = std::env::temp_dir().join(format!("slynx-romfs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let big: Vec<u8> = (0..9_000_001u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.join("sub/big.bin"), &big).unwrap();
        std::fs::write(dir.join("small.txt"), b"x").unwrap();
        let mut b = RomfsBuilder::new();
        b.add_dir(&dir).unwrap();
        let image = b.build();
        let mut calls = 0;
        let mut img = Vec::new();
        image
            .write_to(&mut img, |done, total| {
                calls += 1;
                assert!(done <= total + FILE_ALIGN);
            })
            .unwrap();
        assert!(calls >= 2);
        let r = reader(&img);
        let s = r.lookup("sub/big.bin").unwrap();
        assert_eq!(&img[s.offset as usize..(s.offset + s.size) as usize], &big[..]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
