//! Packs the player's own Sly 3 disc image and checks the result, like `disc.rs` (Sly 1) and `disc_sly2.rs`. Runs
//! when `SLYNX_DISC_SLY3` names the image and a SlyNX program ELF is available (`SLYNX_PACK_PROGRAM`, or the viewer or
//! platform demo ELF under `$CARGO_TARGET_DIR`, default `~/slynx-target`); otherwise it prints why and passes. The
//! package goes to a temporary folder outside the repository (`SLYNX_PACK_TMP`) and is deleted afterwards.

use formats::iso::ReadAt;
use formats_sly3::Disc;
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

/// A window of a file, as a disc image source.
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

#[test]
fn pack_sly3() {
    let Some(iso) = std::env::var_os("SLYNX_DISC_SLY3").map(PathBuf::from) else {
        eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
        return;
    };
    let Some(program) = program() else {
        eprintln!("skipped: no SlyNX program ELF (set SLYNX_PACK_PROGRAM or run tools/build-switch.sh)");
        return;
    };
    match identify_path(&iso).unwrap() {
        Identity::Supported { game, .. } => assert_eq!(game.id, "sly3-usa"),
        other => panic!("not the supported Sly 3 disc: {other}"),
    }
    let tmp = std::env::var_os("SLYNX_PACK_TMP").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let out = tmp.join(format!("slynx-pack-sly3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let report = pack(&PackOptions::new(&iso, &out, &program), |_, _| {}).unwrap();
    let iso_len = std::fs::metadata(&iso).unwrap().len();
    assert_eq!(report.layout, Layout::RomFs, "3.55 GB fits under the 4 GiB limit");
    assert_eq!(report.nro, out.join("switch/SlyNX/slynx-sly3.nro"));
    assert!(report.nro_size <= FAT32_MAX_FILE && report.nro_size > iso_len);
    let info = inspect_nro(&report.nro).unwrap();
    assert_eq!((info.nacp.title.as_str(), info.nacp.author.as_str()), ("SlyNX 3", "Wahkah"));
    assert_eq!(info.romfs_files, vec!["sly3.iso".to_string(), "slynx/package.txt".to_string()]);
    let romfs = info.romfs.as_ref().unwrap();
    let span = romfs.lookup("sly3.iso").unwrap();
    assert_eq!(span.size, iso_len);
    // The manifest names the game and the image.
    let m = romfs.lookup("slynx/package.txt").unwrap();
    let mut f = File::open(&report.nro).unwrap();
    let mut text = vec![0u8; m.size as usize];
    f.seek(SeekFrom::Start(info.romfs_offset + m.offset)).unwrap();
    f.read_exact(&mut text).unwrap();
    let text = String::from_utf8(text).unwrap();
    assert!(text.contains("game=sly3-usa\n") && text.contains("image=romfs:/sly3.iso\n"), "{text}");
    // The packed image opens through the Sly 3 disc API: executable hash and the world table.
    let base = info.romfs_offset + span.offset;
    let packed = Disc::open(Window { file: File::open(&report.nro).unwrap(), base, len: span.size }).unwrap();
    assert_eq!(packed.world_slots().count(), 39);
    std::fs::remove_dir_all(&out).unwrap();
}
