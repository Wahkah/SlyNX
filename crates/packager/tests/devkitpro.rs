//! Byte-for-byte comparison with devkitPro's `nacptool` and `elf2nro`, when they are installed (`DEVKITPRO`, default
//! `/opt/devkitpro`). The NRO test also needs a linked libnx program: `SLYNX_TEST_ELF`, or the platform demo's ELF
//! from `tools/build-switch.sh` under `$CARGO_TARGET_DIR` (default `~/slynx-target`). Missing tools or ELF: the
//! test prints why and passes.

use packager::icon::default_icon;
use packager::nacp::Nacp;
use packager::nro::{NroImage, write_head};
use packager::romfs::RomfsBuilder;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tool(name: &str) -> Option<PathBuf> {
    let root = std::env::var_os("DEVKITPRO").map(PathBuf::from).unwrap_or_else(|| "/opt/devkitpro".into());
    let p = root.join("tools/bin").join(name);
    p.is_file().then_some(p)
}

fn test_elf() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SLYNX_TEST_ELF") {
        return Some(p.into());
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("slynx-target")))?;
    let p = target.join("switch-build/release/slynx-hello/slynx-hello.elf");
    p.is_file().then_some(p)
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("slynx-pack-dkp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y).or((a.len() != b.len()).then(|| a.len().min(b.len())))
}

#[test]
fn nacp_matches_nacptool() {
    let Some(nacptool) = tool("nacptool") else {
        eprintln!("skipped: nacptool not installed");
        return;
    };
    let dir = scratch("nacp");
    for (title, author, version) in
        [("SlyNX 1", "Wahkah", "0.0.1"), ("A much longer title, with spaces", "x", "10.20.30")]
    {
        let out = dir.join("t.nacp");
        let status = Command::new(&nacptool).args(["--create", title, author, version]).arg(&out).output().unwrap();
        assert!(status.status.success());
        let theirs = std::fs::read(&out).unwrap();
        let ours = Nacp::new(title, author, version).to_bytes();
        assert_eq!(first_difference(&ours, &theirs), None, "NACP differs from nacptool's for {title:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

fn write_tree(root: &Path) {
    // Enough files for more than 19 hash buckets, nested folders, names of every length modulo 4, an empty file.
    for i in 0..30 {
        let dir = match i % 5 {
            0 => root.to_path_buf(),
            1 => root.join("data"),
            2 => root.join("data/levels"),
            3 => root.join("Zeta"),
            _ => root.join("a/b/c"),
        };
        std::fs::create_dir_all(&dir).unwrap();
        let name = format!("{}{}.bin", "f".repeat(i % 4 + 1), i);
        let data: Vec<u8> = (0..(i * 997) % 5000).map(|k| (k as u32 * 13 + i as u32) as u8).collect();
        std::fs::write(dir.join(name), data).unwrap();
    }
    std::fs::write(root.join("empty.txt"), b"").unwrap();
}

#[test]
fn nro_matches_elf2nro() {
    let (Some(elf2nro), Some(elf)) = (tool("elf2nro"), test_elf()) else {
        eprintln!("skipped: elf2nro or a test ELF is missing (set SLYNX_TEST_ELF or run tools/build-switch.sh)");
        return;
    };
    let dir = scratch("nro");
    let romfs_dir = dir.join("romfs");
    write_tree(&romfs_dir);
    let icon = default_icon(1);
    let nacp = Nacp::new("SlyNX 1", "Wahkah", "0.0.1").to_bytes();
    std::fs::write(dir.join("icon.jpg"), &icon).unwrap();
    std::fs::write(dir.join("control.nacp"), &nacp).unwrap();

    let reference = dir.join("ref.nro");
    let out = Command::new(&elf2nro)
        .arg(&elf)
        .arg(&reference)
        .arg(format!("--icon={}", dir.join("icon.jpg").display()))
        .arg(format!("--nacp={}", dir.join("control.nacp").display()))
        .arg(format!("--romfsdir={}", romfs_dir.display()))
        .output()
        .unwrap();
    assert!(out.status.success(), "elf2nro failed: {}", String::from_utf8_lossy(&out.stderr));
    let theirs = std::fs::read(&reference).unwrap();

    let image = NroImage::from_elf(&std::fs::read(&elf).unwrap()).unwrap();
    let mut b = RomfsBuilder::new();
    b.add_dir(&romfs_dir).unwrap();
    let romfs = b.build();
    let mut ours = Vec::new();
    write_head(&mut ours, &image, &icon, &nacp, romfs.size()).unwrap();
    romfs.write_to(&mut ours, |_, _| {}).unwrap();

    if std::env::var_os("SLYNX_KEEP_NRO").is_some() {
        std::fs::write(dir.join("ours.nro"), &ours).unwrap();
        eprintln!("kept {}", dir.display());
    }
    if let Some(at) = first_difference(&ours, &theirs) {
        let part = if at < image.bytes.len() { "program image" } else { "assets" };
        panic!("our .nro differs from elf2nro's at {at:#x} ({part}); sizes {:#x} vs {:#x}", ours.len(), theirs.len());
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
