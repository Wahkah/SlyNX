//! The packaging pipeline: disc image + SlyNX program -> `.nro` (and, if the image does not fit, the image next to
//! it on the SD card).

use crate::game::{Identity, KnownGame, identify_path};
use crate::icon::default_icon;
use crate::jpeg;
use crate::nacp::{NACP_SIZE, Nacp};
use crate::nro::{self, AssetLayout, FAT32_MAX_FILE, NroImage};
use crate::romfs::{RomfsBuilder, RomfsReader, Source};
use formats::hash::hex;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Author field of every package.
pub const AUTHOR: &str = "Wahkah";
/// Packager version, also the default version shown in the Homebrew Menu.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Folder for SlyNX on the SD card (relative to the card's root).
pub const SD_DIR: &str = "switch/SlyNX";
/// Path of the package manifest inside the RomFS.
pub const MANIFEST_PATH: &str = "slynx/package.txt";
/// Shown after every successful run.
pub const PERSONAL_USE: &str =
    "This build contains data from your own copy of the game. It is for your personal use. Do not share it.";

/// Where the disc image goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Inside the `.nro` when the result stays under the FAT32 file size limit, otherwise next to it.
    Auto,
    /// Always inside the `.nro` (fails if that would exceed 4 GiB).
    RomFs,
    /// Always next to the `.nro`, at `switch/SlyNX/<image>` on the SD card.
    Sd,
}

/// Options for [`pack`].
#[derive(Clone, Debug)]
pub struct PackOptions {
    pub iso: PathBuf,
    /// Output folder; the package is written under `<out>/switch/SlyNX/`, mirroring the SD card.
    pub out: PathBuf,
    /// The SlyNX program: a linked `.elf` or an `.nro` without game data.
    pub program: PathBuf,
    /// A 256x256 JPEG; the generated icon when `None`.
    pub icon: Option<PathBuf>,
    pub title: Option<String>,
    pub layout: Layout,
    /// NSP output with the player's own keys. Not supported yet; see [`PackError::NspNotSupported`].
    pub nsp_keys: Option<PathBuf>,
    /// Largest `.nro` to write (the FAT32 limit unless a test lowers it).
    pub max_nro_size: u64,
}

impl PackOptions {
    pub fn new(iso: impl Into<PathBuf>, out: impl Into<PathBuf>, program: impl Into<PathBuf>) -> PackOptions {
        PackOptions {
            iso: iso.into(),
            out: out.into(),
            program: program.into(),
            icon: None,
            title: None,
            layout: Layout::Auto,
            nsp_keys: None,
            max_nro_size: FAT32_MAX_FILE,
        }
    }
}

#[derive(Debug)]
pub enum PackError {
    /// The NSP path is not implemented. Keys are never read.
    NspNotSupported,
    Disc(String),
    /// A disc we know but cannot package yet, or do not know.
    UnsupportedDisc(String),
    Program(String),
    Icon(String),
    /// The image does not fit where the layout asks for it.
    TooLarge(String),
    Io(String),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::NspNotSupported => f.write_str(
                "NSP output is not supported yet. Make an .nro instead (leave out --nsp and --keys); \
                 no key file was read.",
            ),
            PackError::Disc(e) => write!(f, "disc image: {e}"),
            PackError::UnsupportedDisc(e) => write!(f, "this disc cannot be packaged: {e}"),
            PackError::Program(e) => write!(f, "SlyNX program: {e}"),
            PackError::Icon(e) => write!(f, "icon: {e}"),
            PackError::TooLarge(e) => write!(f, "{e}"),
            PackError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PackError {}

fn io_err(what: &Path) -> impl FnOnce(io::Error) -> PackError + '_ {
    move |e| PackError::Io(format!("{}: {e}", what.display()))
}

/// What [`pack`] wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackReport {
    pub game_id: &'static str,
    pub release: &'static str,
    pub nro: PathBuf,
    pub nro_size: u64,
    /// The disc image copied next to the `.nro`, for the SD layout.
    pub image_on_sd: Option<PathBuf>,
    pub layout: Layout,
    pub title: String,
}

/// The text file packed at [`MANIFEST_PATH`]: what the package holds, for the game and for support questions.
pub fn manifest(game: &KnownGame, exe_sha256: &[u8; 32], layout: Layout, image_len: u64) -> String {
    let place = match layout {
        Layout::Sd => format!("sdmc:/{SD_DIR}/{}", game.image_name),
        _ => format!("romfs:/{}", game.image_name),
    };
    format!(
        "format=1\ngame={}\nexecutable_sha256={}\nimage={place}\nimage_size={image_len}\npackager={VERSION}\n",
        game.id,
        hex(exe_sha256),
    )
}

/// Identifies the disc and packages it. `progress(done, total)` reports bytes of the disc image copied.
pub fn pack(opts: &PackOptions, progress: impl FnMut(u64, u64)) -> Result<PackReport, PackError> {
    // Refuse the NSP path before anything else, and never open the key file.
    if opts.nsp_keys.is_some() {
        return Err(PackError::NspNotSupported);
    }
    let (game, sha) = match identify_path(&opts.iso).map_err(PackError::Disc)? {
        Identity::Supported { game, executable_sha256 } => (game, executable_sha256),
        other => return Err(PackError::UnsupportedDisc(other.to_string())),
    };
    build_package(opts, game, &sha, progress)
}

/// Packages an already identified disc image (split out so tests can run without a real disc).
pub fn build_package(
    opts: &PackOptions,
    game: &'static KnownGame,
    exe_sha256: &[u8; 32],
    mut progress: impl FnMut(u64, u64),
) -> Result<PackReport, PackError> {
    if opts.nsp_keys.is_some() {
        return Err(PackError::NspNotSupported);
    }
    let program =
        fs::read(&opts.program).map_err(|e| PackError::Program(format!("{}: {e}", opts.program.display())))?;
    let image = NroImage::from_program(&program).map_err(|e| PackError::Program(e.to_string()))?;

    let icon = match &opts.icon {
        Some(path) => {
            let data = fs::read(path).map_err(|e| PackError::Icon(format!("{}: {e}", path.display())))?;
            match jpeg::dimensions(&data) {
                Some((256, 256)) => data,
                Some((w, h)) => {
                    return Err(PackError::Icon(format!("{} is {w}x{h}; it must be 256x256", path.display())));
                }
                None => return Err(PackError::Icon(format!("{} is not a JPEG", path.display()))),
            }
        }
        None => default_icon(game.number),
    };
    let title = opts.title.clone().unwrap_or_else(|| game.title.to_string());
    let nacp = Nacp::new(&title, AUTHOR, VERSION).to_bytes();

    let iso_len = fs::metadata(&opts.iso).map_err(io_err(&opts.iso))?.len();
    let romfs_with = |layout: Layout| -> io::Result<crate::romfs::RomfsImage> {
        let mut b = RomfsBuilder::new();
        b.add(MANIFEST_PATH, Source::Bytes(manifest(game, exe_sha256, layout, iso_len).into_bytes()))?;
        if layout != Layout::Sd {
            b.add(game.image_name, Source::File { path: opts.iso.clone(), len: iso_len })?;
        }
        Ok(b.build())
    };
    let size_with =
        |romfs: &crate::romfs::RomfsImage| nro::file_size(&image, icon.len() as u64, nacp.len() as u64, romfs.size());

    let inside = romfs_with(Layout::RomFs).map_err(|e| PackError::Io(e.to_string()))?;
    let fits = size_with(&inside) <= opts.max_nro_size;
    let layout = match opts.layout {
        Layout::Auto if fits => Layout::RomFs,
        Layout::Auto => Layout::Sd,
        Layout::RomFs if !fits => {
            return Err(PackError::TooLarge(format!(
                "the .nro would be {} bytes, over the {} byte limit of a FAT32 SD card; use --layout sd",
                size_with(&inside),
                opts.max_nro_size
            )));
        }
        l => l,
    };
    if layout == Layout::Sd && iso_len > FAT32_MAX_FILE {
        return Err(PackError::TooLarge(format!(
            "the disc image is {iso_len} bytes, over the FAT32 file size limit; this disc cannot be packaged yet"
        )));
    }
    let romfs =
        if layout == Layout::Sd { romfs_with(Layout::Sd).map_err(|e| PackError::Io(e.to_string()))? } else { inside };

    let dir = opts.out.join(SD_DIR);
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let nro_path = dir.join(format!("{}.nro", game.nro_name));
    let part = dir.join(format!("{}.nro.part", game.nro_name));
    {
        let file = File::create(&part).map_err(io_err(&part))?;
        let mut w = BufWriter::with_capacity(1 << 20, file);
        nro::write_head(&mut w, &image, &icon, &nacp, romfs.size()).map_err(io_err(&part))?;
        romfs.write_to(&mut w, &mut progress).map_err(io_err(&part))?;
        let file = w.into_inner().map_err(|e| PackError::Io(format!("{}: {}", part.display(), e.error())))?;
        file.sync_all().map_err(io_err(&part))?;
    }
    fs::rename(&part, &nro_path).map_err(io_err(&nro_path))?;
    let nro_size = fs::metadata(&nro_path).map_err(io_err(&nro_path))?.len();

    let image_on_sd = if layout == Layout::Sd {
        let dest = dir.join(game.image_name);
        copy_with_progress(&opts.iso, &dest, &mut progress)?;
        Some(dest)
    } else {
        // A stale image from an earlier SD-layout run would shadow the packaged one; leave it but do not create one.
        None
    };
    Ok(PackReport { game_id: game.id, release: game.release, nro: nro_path, nro_size, image_on_sd, layout, title })
}

fn copy_with_progress(src: &Path, dest: &Path, progress: &mut impl FnMut(u64, u64)) -> Result<(), PackError> {
    let part = dest.with_extension("iso.part");
    let mut input = File::open(src).map_err(io_err(src))?;
    let total = input.metadata().map_err(io_err(src))?.len();
    let mut out = BufWriter::with_capacity(1 << 20, File::create(&part).map_err(io_err(&part))?);
    let mut buf = vec![0u8; 8 << 20];
    let mut done = 0u64;
    loop {
        let n = input.read(&mut buf).map_err(io_err(src))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(io_err(&part))?;
        done += n as u64;
        progress(done, total);
    }
    let file = out.into_inner().map_err(|e| PackError::Io(e.error().to_string()))?;
    file.sync_all().map_err(io_err(&part))?;
    fs::rename(&part, dest).map_err(io_err(dest))?;
    Ok(())
}

/// What an `.nro` holds, read back for checking.
#[derive(Debug)]
pub struct NroInfo {
    pub header: nro::NroHeader,
    pub assets: AssetLayout,
    pub nacp: Nacp,
    pub icon_size: (u16, u16),
    /// Offset of the RomFS from the start of the file.
    pub romfs_offset: u64,
    pub romfs_files: Vec<String>,
    pub romfs: Option<RomfsReader>,
}

/// Reads and checks an `.nro` (header, assets, NACP, icon, RomFS metadata) without loading the RomFS data.
pub fn inspect_nro(path: &Path) -> io::Result<NroInfo> {
    let bad = |why: String| io::Error::new(io::ErrorKind::InvalidData, why);
    let mut f = File::open(path)?;
    let file_len = f.metadata()?.len();
    let mut head = vec![0u8; 0x80];
    f.read_exact(&mut head)?;
    if head.get(0x10..0x14) != Some(b"NRO0") {
        return Err(bad("no NRO0 magic at 0x10".into()));
    }
    let size = u32::from_le_bytes(head[0x18..0x1C].try_into().unwrap()) as u64;
    // The image must fit in the file, with the asset header after it, before anything is allocated for it.
    if size < 0x80 || size.saturating_add(nro::ASSET_HEADER_SIZE) > file_len {
        return Err(bad(format!("image size {size:#x} does not fit in a {file_len:#x}-byte file")));
    }
    let mut image = vec![0u8; size as usize];
    f.seek(SeekFrom::Start(0))?;
    f.read_exact(&mut image)?;
    let header = nro::parse_header(&image)?;
    let mod0 = header.mod0_offset as usize;
    if image.get(mod0..mod0 + 4) != Some(b"MOD0") {
        return Err(bad("no MOD0 header".into()));
    }
    let mut ah = vec![0u8; nro::ASSET_HEADER_SIZE as usize];
    f.read_exact(&mut ah)?;
    let assets = AssetLayout::parse(&ah)?;
    let end = size.saturating_add(assets.len());
    if end != file_len {
        return Err(bad(format!("asset section ends at {end:#x}, file is {file_len:#x} bytes")));
    }
    // Every part inside the file (a part may claim more than the layout's end).
    for (what, (off, len)) in [("icon", assets.icon), ("NACP", assets.nacp), ("RomFS", assets.romfs)] {
        if size.saturating_add(off).saturating_add(len) > file_len {
            return Err(bad(format!("{what} runs past the end of the file")));
        }
    }
    let mut read_part = |(off, len): (u64, u64)| -> io::Result<Vec<u8>> {
        let mut v = vec![0u8; len as usize];
        f.seek(SeekFrom::Start(size + off))?;
        f.read_exact(&mut v)?;
        Ok(v)
    };
    let icon = read_part(assets.icon)?;
    let icon_size = jpeg::dimensions(&icon).ok_or_else(|| bad("icon is not a JPEG".into()))?;
    let nacp_bytes = read_part(assets.nacp)?;
    if nacp_bytes.len() != NACP_SIZE {
        return Err(bad(format!("NACP is {:#x} bytes", nacp_bytes.len())));
    }
    let nacp = Nacp::parse(&nacp_bytes).expect("size checked");
    let romfs_offset = size + assets.romfs.0;
    let (romfs, romfs_files) = if assets.romfs.1 > 0 {
        let romfs_end = romfs_offset + assets.romfs.1;
        let r = RomfsReader::open(|off, len| {
            // RomFS metadata names its own offsets: keep each read inside its part.
            let end = romfs_offset.checked_add(off).and_then(|s| s.checked_add(len as u64));
            if end.is_none_or(|e| e > romfs_end) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "RomFS read past its part"));
            }
            let mut v = vec![0u8; len];
            f.seek(SeekFrom::Start(romfs_offset + off))?;
            f.read_exact(&mut v)?;
            Ok(v)
        })?;
        let files = r.list();
        (Some(r), files)
    } else {
        (None, Vec::new())
    };
    Ok(NroInfo { header, assets, nacp, icon_size, romfs_offset, romfs_files, romfs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GAMES;
    use crate::nro::tests::synthetic_elf;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("slynx-pack-unit-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A fake "disc image" (any bytes: identification is bypassed) and a synthetic program.
    fn inputs(dir: &Path) -> (PathBuf, PathBuf, Vec<u8>) {
        let iso: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 253) as u8).collect();
        fs::write(dir.join("disc.iso"), &iso).unwrap();
        fs::write(dir.join("program.elf"), synthetic_elf()).unwrap();
        (dir.join("disc.iso"), dir.join("program.elf"), iso)
    }

    fn read_span(path: &Path, off: u64, len: u64) -> Vec<u8> {
        let mut f = File::open(path).unwrap();
        f.seek(SeekFrom::Start(off)).unwrap();
        let mut v = vec![0u8; len as usize];
        f.read_exact(&mut v).unwrap();
        v
    }

    #[test]
    fn romfs_layout() {
        let dir = scratch("romfs");
        let (iso, program, data) = inputs(&dir);
        let opts = PackOptions::new(&iso, dir.join("out"), &program);
        let mut last = (0, 0);
        let r = build_package(&opts, &GAMES[0], &[1; 32], |d, t| last = (d, t)).unwrap();
        assert_eq!(r.layout, Layout::RomFs);
        assert_eq!(r.nro, dir.join("out/switch/SlyNX/slynx-sly1.nro"));
        assert_eq!(r.image_on_sd, None);
        assert!(last.0 == last.1 && last.0 >= data.len() as u64, "progress ends at the total: {last:?}");
        let info = inspect_nro(&r.nro).unwrap();
        assert_eq!(info.nacp, Nacp::new("SlyNX 1", "Wahkah", VERSION));
        assert_eq!(info.icon_size, (256, 256));
        assert_eq!(info.romfs_files, vec!["sly1.iso".to_string(), "slynx/package.txt".to_string()]);
        let romfs = info.romfs.as_ref().unwrap();
        let span = romfs.lookup("sly1.iso").unwrap();
        assert_eq!(read_span(&r.nro, info.romfs_offset + span.offset, span.size), data);
        let m = romfs.lookup(MANIFEST_PATH).unwrap();
        let text = String::from_utf8(read_span(&r.nro, info.romfs_offset + m.offset, m.size)).unwrap();
        assert!(text.contains("game=sly1-usa\n") && text.contains("image=romfs:/sly1.iso\n"), "{text}");
        assert!(!dir.join("out/switch/SlyNX/slynx-sly1.nro.part").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn inspect_refuses_forged_sizes() {
        let dir = scratch("forged");
        let (iso, program, _) = inputs(&dir);
        let opts = PackOptions::new(&iso, dir.join("out"), &program);
        let r = build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}).unwrap();
        let good = fs::read(&r.nro).unwrap();
        let size = u32::from_le_bytes(good[0x18..0x1C].try_into().unwrap()) as usize;
        let forged = dir.join("forged.nro");
        let try_with = |patch: &dyn Fn(&mut Vec<u8>)| {
            let mut b = good.clone();
            patch(&mut b);
            fs::write(&forged, &b).unwrap();
            inspect_nro(&forged)
        };
        // An image size near 4 GiB: refused before anything that size is allocated.
        assert!(try_with(&|b| b[0x18..0x1C].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes())).is_err());
        // No NRO magic; a file cut short inside the image.
        assert!(try_with(&|b| b[0x10] = b'X').is_err());
        assert!(try_with(&|b| b.truncate(0x100)).is_err());
        // A RomFS part that claims to run past the end of the file, or that overflows.
        let romfs_len = size + 16 + 2 * 16;
        assert!(try_with(&|b| b[romfs_len..romfs_len + 8].copy_from_slice(&(1u64 << 40).to_le_bytes())).is_err());
        assert!(try_with(&|b| b[romfs_len..romfs_len + 8].copy_from_slice(&u64::MAX.to_le_bytes())).is_err());
        // The untouched package still inspects.
        assert!(try_with(&|_| {}).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sd_layout_when_too_large() {
        let dir = scratch("sd");
        let (iso, program, data) = inputs(&dir);
        let mut opts = PackOptions::new(&iso, dir.join("out"), &program);
        opts.max_nro_size = 200_000; // smaller than the image: forces the image out of the .nro
        opts.title = Some("My title".into());
        let r = build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}).unwrap();
        assert_eq!(r.layout, Layout::Sd);
        let on_sd = r.image_on_sd.clone().unwrap();
        assert_eq!(on_sd, dir.join("out/switch/SlyNX/sly1.iso"));
        assert_eq!(fs::read(&on_sd).unwrap(), data);
        let info = inspect_nro(&r.nro).unwrap();
        assert_eq!(info.nacp.title, "My title");
        assert_eq!(info.romfs_files, vec!["slynx/package.txt".to_string()]);

        opts.layout = Layout::RomFs;
        assert!(matches!(build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}), Err(PackError::TooLarge(_))));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn icons_and_nsp() {
        let dir = scratch("icon");
        let (iso, program, _) = inputs(&dir);
        let mut opts = PackOptions::new(&iso, dir.join("out"), &program);
        let small = jpeg::encode(&jpeg::Rgb { width: 16, height: 16, pixels: vec![[1, 2, 3]; 256] });
        fs::write(dir.join("small.jpg"), small).unwrap();
        opts.icon = Some(dir.join("small.jpg"));
        assert!(matches!(build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}), Err(PackError::Icon(_))));
        fs::write(dir.join("own.jpg"), crate::icon::default_icon(0)).unwrap();
        opts.icon = Some(dir.join("own.jpg"));
        assert!(build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}).is_ok());

        // The NSP path is refused before anything is read: the key path does not even exist.
        opts.nsp_keys = Some(dir.join("does-not-exist.keys"));
        assert!(matches!(pack(&opts, |_, _| {}), Err(PackError::NspNotSupported)));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bad_program() {
        let dir = scratch("prog");
        let (iso, _, _) = inputs(&dir);
        fs::write(dir.join("junk.elf"), b"junk").unwrap();
        let opts = PackOptions::new(&iso, dir.join("out"), dir.join("junk.elf"));
        assert!(matches!(build_package(&opts, &GAMES[0], &[1; 32], |_, _| {}), Err(PackError::Program(_))));
        fs::remove_dir_all(&dir).unwrap();
    }
}
