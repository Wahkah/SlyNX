//! Packs the player's own Sly 1 disc image and checks the result. Runs when `SLYNX_DISC_SLY1` names the image and
//! a SlyNX program is available (`SLYNX_PACK_PROGRAM`, or the viewer or platform demo ELF built by
//! `tools/build-switch.sh` under `$CARGO_TARGET_DIR`, default `~/slynx-target`); otherwise it prints why and passes.
//!
//! The package is written to a temporary folder outside the repository (`SLYNX_PACK_TMP`, default the system temp
//! folder) and deleted afterwards. It needs about the disc image's size in free space there.

use formats::disc::Disc;
use formats::iso::ReadAt;
use packager::game::{Identity, identify_path};
use packager::nro::FAT32_MAX_FILE;
use packager::pack::{Layout, PackOptions, inspect_nro, pack};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

fn program() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SLYNX_PACK_PROGRAM") {
        return Some(p.into());
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("slynx-target")))?;
    ["slynx-viewer/slynx-viewer.elf", "slynx-hello/slynx-hello.elf"]
        .iter()
        .map(|p| target.join("switch-build/release").join(p))
        .find(|p| p.is_file())
}

/// A window of a file, as a disc image source for `formats`.
struct Window {
    file: File,
    base: u64,
    len: u64,
}

impl ReadAt for Window {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> formats::error::Result<()> {
        if offset + buf.len() as u64 > self.len {
            return Err(formats::error::Error::Eof {
                offset: offset as usize,
                needed: buf.len(),
                len: self.len as usize,
            });
        }
        self.file.seek(SeekFrom::Start(self.base + offset)).map_err(|e| formats::error::Error::Io(e.to_string()))?;
        self.file.read_exact(buf).map_err(|e| formats::error::Error::Io(e.to_string()))
    }

    fn image_len(&mut self) -> Option<u64> {
        Some(self.len)
    }
}

fn read(file: &mut File, at: u64, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    file.seek(SeekFrom::Start(at)).unwrap();
    file.read_exact(&mut v).unwrap();
    v
}

#[test]
fn pack_sly1() {
    let Some(iso) = std::env::var_os("SLYNX_DISC_SLY1").map(PathBuf::from) else {
        eprintln!("skipped: SLYNX_DISC_SLY1 is not set");
        return;
    };
    let Some(program) = program() else {
        eprintln!("skipped: no SlyNX program ELF (set SLYNX_PACK_PROGRAM or run tools/build-switch.sh)");
        return;
    };
    match identify_path(&iso).unwrap() {
        Identity::Supported { game, .. } => assert_eq!(game.id, "sly1-usa"),
        other => panic!("not the supported Sly 1 disc: {other}"),
    }

    let tmp = std::env::var_os("SLYNX_PACK_TMP").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let out = tmp.join(format!("slynx-pack-disc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let opts = PackOptions::new(&iso, &out, &program);
    let mut calls = 0u32;
    let report = pack(&opts, |_, _| calls += 1).unwrap();
    assert!(calls > 0);
    let iso_len = std::fs::metadata(&iso).unwrap().len();

    // The whole image fits under 4 GB, so it goes into the .nro.
    assert_eq!(report.layout, Layout::RomFs);
    assert!(report.nro_size <= FAT32_MAX_FILE && report.nro_size > iso_len);
    let info = inspect_nro(&report.nro).unwrap();
    assert_eq!(info.nacp.title, "SlyNX 1");
    assert_eq!(info.nacp.author, "Wahkah");
    assert_eq!(info.icon_size, (256, 256));
    assert_eq!(info.romfs_files, vec!["sly1.iso".to_string(), "slynx/package.txt".to_string()]);
    let span = info.romfs.as_ref().unwrap().lookup("sly1.iso").unwrap();
    assert_eq!(span.size, iso_len);

    // The packed image opens through the same disc API the game uses: executable hash, world table, a world.
    let base = info.romfs_offset + span.offset;
    let mut packed = Disc::open(Window { file: File::open(&report.nro).unwrap(), base, len: span.size }).unwrap();
    let mut original = Disc::open_path(&iso).unwrap();
    assert_eq!(packed.worlds.len(), original.worlds.len());
    let last = packed.worlds.len() - 1;
    for w in [0, last] {
        assert_eq!(packed.read_world_compressed(w).unwrap(), original.read_world_compressed(w).unwrap(), "world {w}");
    }

    // Byte checks: start, end and evenly spread windows.
    let (mut a, mut b) = (File::open(&iso).unwrap(), File::open(&report.nro).unwrap());
    let win = 1 << 20;
    let mut offsets: Vec<u64> = (0..48).map(|i| i * (iso_len - win as u64) / 47).collect();
    offsets.extend([0, iso_len - win as u64]);
    for off in offsets {
        assert_eq!(read(&mut a, off, win), read(&mut b, base + off, win), "image bytes at {off:#x}");
    }

    std::fs::remove_dir_all(&out).unwrap();
}
