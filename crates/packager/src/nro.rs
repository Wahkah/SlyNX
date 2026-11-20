//! NRO: the Switch homebrew executable format, written without devkitPro.
//!
//! An `.nro` is the program image followed by an asset section:
//!
//! - **Image.** The three loadable segments of the linked ELF (code, read-only data, data) laid out at their
//!   virtual addresses, each padded to a 4 KiB page. libnx's startup code reserves the first 0x80 bytes of the code:
//!   bytes 0..0x10 stay as linked (a branch to the entry point, the offset of the `MOD0` header, and padding); bytes
//!   0x10..0x80 receive the NRO header (libnx `NroHeader`): magic `NRO0`, version 0, image size, flags 0, the three
//!   segments as (offset, size), the zero-initialised size after the data segment (`bss`), a reserved word, the
//!   32-byte build ID (the ELF's GNU build-id note, zero-padded) and 32 reserved bytes.
//! - **Assets** (libnx `NroAssetHeader`), right after the image: magic `ASET`, version 0, then (offset, size) of the
//!   icon (JPEG), the NACP and the RomFS, offsets relative to the asset header. The parts follow the header
//!   back to back, in that order.
//!
//! The image can also be taken from an existing `.nro` (its asset section is replaced).

use std::io::{self, Write};

pub const PAGE: u64 = 0x1000;
pub const NRO_MAGIC: &[u8; 4] = b"NRO0";
pub const ASSET_MAGIC: &[u8; 4] = b"ASET";
pub const ASSET_HEADER_SIZE: u64 = 0x38;
/// Bytes at the start of the code reserved for `NroStart` + `NroHeader`.
pub const HEADER_END: usize = 0x80;
/// The largest file a FAT32 SD card can hold.
pub const FAT32_MAX_FILE: u64 = u32::MAX as u64;

fn bad(why: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.into())
}

fn align(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// `n` bytes at `o`, or "truncated" (also when `o + n` overflows).
fn bytes_at(b: &[u8], o: usize, n: usize) -> io::Result<&[u8]> {
    o.checked_add(n).and_then(|e| b.get(o..e)).ok_or_else(|| bad("truncated"))
}
fn u16_at(b: &[u8], o: usize) -> io::Result<u16> {
    Ok(u16::from_le_bytes(bytes_at(b, o, 2)?.try_into().unwrap()))
}
fn u32_at(b: &[u8], o: usize) -> io::Result<u32> {
    Ok(u32::from_le_bytes(bytes_at(b, o, 4)?.try_into().unwrap()))
}
fn u64_at(b: &[u8], o: usize) -> io::Result<u64> {
    Ok(u64::from_le_bytes(bytes_at(b, o, 8)?.try_into().unwrap()))
}

#[derive(Clone, Copy, Debug)]
struct Phdr {
    kind: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    filesz: u64,
    memsz: u64,
}

const PT_LOAD: u32 = 1;
const SHT_NOTE: u32 = 7;
const NT_GNU_BUILD_ID: u32 = 3;

/// The NRO program image (code, read-only data, data, with the NRO header in place).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NroImage {
    pub bytes: Vec<u8>,
}

/// Parsed fields of an NRO header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NroHeader {
    pub size: u32,
    pub segments: [(u32, u32); 3],
    pub bss: u32,
    pub build_id: [u8; 0x20],
    pub mod0_offset: u32,
}

impl NroImage {
    /// Converts a linked AArch64 ELF (as produced by devkitA64 with libnx's `switch.specs`).
    pub fn from_elf(elf: &[u8]) -> io::Result<NroImage> {
        if elf.get(0..4) != Some(b"\x7fELF") {
            return Err(bad("not an ELF file"));
        }
        if elf.get(4..6) != Some(&[2, 1]) {
            return Err(bad("not a 64-bit little-endian ELF"));
        }
        if u16_at(elf, 0x12)? != 183 {
            return Err(bad("not an AArch64 ELF"));
        }
        let (phoff, phentsize, phnum) = (u64_at(elf, 0x20)? as usize, u16_at(elf, 0x36)? as usize, u16_at(elf, 0x38)?);
        let mut loads = Vec::new();
        for i in 0..phnum as usize {
            let o = i.checked_mul(phentsize).and_then(|x| x.checked_add(phoff)).ok_or_else(|| bad("truncated"))?;
            let p = Phdr {
                kind: u32_at(elf, o)?,
                flags: u32_at(elf, o + 4)?,
                offset: u64_at(elf, o + 8)?,
                vaddr: u64_at(elf, o + 0x10)?,
                filesz: u64_at(elf, o + 0x20)?,
                memsz: u64_at(elf, o + 0x28)?,
            };
            if p.kind == PT_LOAD {
                loads.push(p);
            }
        }
        if loads.len() != 3 {
            return Err(bad(format!("expected 3 loadable segments (code, rodata, data), found {}", loads.len())));
        }
        // Flags: X = 1, W = 2, R = 4.
        let want = [(5, "code (R+X)"), (4, "read-only data (R)"), (6, "data (R+W)")];
        let mut expect_vaddr = 0u64;
        for (p, (flags, what)) in loads.iter().zip(want) {
            if p.flags & 7 != flags {
                return Err(bad(format!("segment at {:#x} is not {what}", p.vaddr)));
            }
            if p.vaddr != expect_vaddr {
                return Err(bad(format!("segment {what} at {:#x}, expected {expect_vaddr:#x}", p.vaddr)));
            }
            if p.offset.checked_add(p.filesz).is_none_or(|e| e > elf.len() as u64) {
                return Err(bad(format!("segment {what} runs past the end of the file")));
            }
            expect_vaddr = align(p.vaddr + p.filesz, PAGE);
        }
        let (text, ro, data) = (loads[0], loads[1], loads[2]);
        if text.filesz < HEADER_END as u64 {
            return Err(bad("code segment too small for the NRO header"));
        }
        let size = expect_vaddr;
        if size > u32::MAX as u64 {
            return Err(bad("program image over 4 GiB"));
        }
        let mut img = vec![0u8; size as usize];
        for p in &loads {
            let src = &elf[p.offset as usize..(p.offset + p.filesz) as usize];
            img[p.vaddr as usize..(p.vaddr + p.filesz) as usize].copy_from_slice(src);
        }
        let bss_end = data.vaddr.checked_add(data.memsz.max(data.filesz)).filter(|&e| e <= u32::MAX as u64);
        let Some(bss_end) = bss_end else { return Err(bad("data segment's memory size runs past 4 GiB")) };
        let bss = align(bss_end, PAGE) - size;
        let seg = |p: Phdr| (p.vaddr as u32, align(p.filesz, PAGE) as u32);
        let header = NroHeader {
            size: size as u32,
            segments: [seg(text), seg(ro), seg(data)],
            bss: bss as u32,
            build_id: build_id(elf).unwrap_or([0; 0x20]),
            mod0_offset: u32_at(&img, 4)?,
        };
        write_header(&mut img, &header);
        let image = NroImage { bytes: img };
        image.check_mod0()?;
        Ok(image)
    }

    /// Takes the image of an existing `.nro`, dropping its assets.
    pub fn from_nro(nro: &[u8]) -> io::Result<NroImage> {
        let h = parse_header(nro)?;
        let image = NroImage { bytes: nro[..h.size as usize].to_vec() };
        image.check_mod0()?;
        Ok(image)
    }

    /// Reads either an ELF or an `.nro`.
    pub fn from_program(bytes: &[u8]) -> io::Result<NroImage> {
        if bytes.starts_with(b"\x7fELF") { NroImage::from_elf(bytes) } else { NroImage::from_nro(bytes) }
    }

    pub fn header(&self) -> io::Result<NroHeader> {
        parse_header(&self.bytes)
    }

    fn check_mod0(&self) -> io::Result<()> {
        let mod0 = u32_at(&self.bytes, 4)? as usize;
        if self.bytes.get(mod0..mod0 + 4) != Some(b"MOD0") {
            return Err(bad("no MOD0 header where the image says (is this a libnx program?)"));
        }
        Ok(())
    }
}

fn write_header(img: &mut [u8], h: &NroHeader) {
    let mut w = Vec::with_capacity(0x70);
    w.extend_from_slice(NRO_MAGIC);
    w.extend_from_slice(&0u32.to_le_bytes());
    w.extend_from_slice(&h.size.to_le_bytes());
    w.extend_from_slice(&0u32.to_le_bytes());
    for (off, size) in h.segments {
        w.extend_from_slice(&off.to_le_bytes());
        w.extend_from_slice(&size.to_le_bytes());
    }
    w.extend_from_slice(&h.bss.to_le_bytes());
    w.extend_from_slice(&0u32.to_le_bytes());
    w.extend_from_slice(&h.build_id);
    w.resize(0x70, 0);
    img[0x10..HEADER_END].copy_from_slice(&w);
}

/// Parses and checks the NRO header at the start of `nro`.
pub fn parse_header(nro: &[u8]) -> io::Result<NroHeader> {
    if nro.get(0x10..0x14) != Some(NRO_MAGIC) {
        return Err(bad("no NRO0 magic at 0x10"));
    }
    let size = u32_at(nro, 0x18)?;
    if size as usize > nro.len() {
        return Err(bad("image size is larger than the file"));
    }
    let mut segments = [(0u32, 0u32); 3];
    let mut expect = 0u32;
    for (i, s) in segments.iter_mut().enumerate() {
        *s = (u32_at(nro, 0x20 + i * 8)?, u32_at(nro, 0x24 + i * 8)?);
        if s.0 != expect || s.1 == 0 || s.0 % PAGE as u32 != 0 || s.1 % PAGE as u32 != 0 {
            return Err(bad(format!("segment {i} at {:#x}+{:#x} is not contiguous and page aligned", s.0, s.1)));
        }
        expect = s.0 + s.1;
    }
    if expect != size {
        return Err(bad("segments do not add up to the image size"));
    }
    let mut build_id = [0u8; 0x20];
    build_id.copy_from_slice(bytes_at(nro, 0x40, 0x20)?);
    let mod0_offset = u32_at(nro, 4)?;
    Ok(NroHeader { size, segments, bss: u32_at(nro, 0x38)?, build_id, mod0_offset })
}

/// The GNU build ID from the ELF's note sections, if any.
fn build_id(elf: &[u8]) -> Option<[u8; 0x20]> {
    let shoff = u64_at(elf, 0x28).ok()? as usize;
    let shentsize = u16_at(elf, 0x3A).ok()? as usize;
    let shnum = u16_at(elf, 0x3C).ok()? as usize;
    for i in 0..shnum {
        let o = i.checked_mul(shentsize)?.checked_add(shoff)?;
        let sh = bytes_at(elf, o, 0x28).ok()?;
        if u32_at(sh, 4).ok()? != SHT_NOTE {
            continue;
        }
        let (off, size) = (u64_at(sh, 0x18).ok()? as usize, u64_at(sh, 0x20).ok()? as usize);
        let notes = elf.get(off..off.checked_add(size)?)?;
        let mut p = 0;
        while p + 12 <= notes.len() {
            let namesz = u32_at(notes, p).ok()? as usize;
            let descsz = u32_at(notes, p + 4).ok()? as usize;
            let kind = u32_at(notes, p + 8).ok()?;
            let name_at = p + 12;
            let desc_at = name_at.checked_add(namesz.div_ceil(4).checked_mul(4)?)?;
            let next = desc_at.checked_add(descsz.div_ceil(4).checked_mul(4)?)?;
            if kind == NT_GNU_BUILD_ID && bytes_at(notes, name_at, namesz).ok()? == b"GNU\0" {
                let desc = bytes_at(notes, desc_at, descsz).ok()?;
                let mut id = [0u8; 0x20];
                let n = desc.len().min(0x20);
                id[..n].copy_from_slice(&desc[..n]);
                return Some(id);
            }
            p = next;
        }
    }
    None
}

/// Sizes of the asset parts, to compute the file layout before writing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetLayout {
    pub icon: (u64, u64),
    pub nacp: (u64, u64),
    pub romfs: (u64, u64),
}

impl AssetLayout {
    pub fn new(icon_len: u64, nacp_len: u64, romfs_len: u64) -> AssetLayout {
        let icon = (ASSET_HEADER_SIZE, icon_len);
        let nacp = (icon.0 + icon.1, nacp_len);
        let romfs = (nacp.0 + nacp.1, romfs_len);
        AssetLayout { icon, nacp, romfs }
    }

    /// Bytes from the asset header to the end of the file.
    pub fn len(&self) -> u64 {
        self.romfs.0.saturating_add(self.romfs.1)
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn header_bytes(&self) -> [u8; ASSET_HEADER_SIZE as usize] {
        let mut h = [0u8; ASSET_HEADER_SIZE as usize];
        h[0..4].copy_from_slice(ASSET_MAGIC);
        for (i, (off, size)) in [self.icon, self.nacp, self.romfs].into_iter().enumerate() {
            h[8 + i * 16..16 + i * 16].copy_from_slice(&off.to_le_bytes());
            h[16 + i * 16..24 + i * 16].copy_from_slice(&size.to_le_bytes());
        }
        h
    }

    pub fn parse(h: &[u8]) -> io::Result<AssetLayout> {
        if h.get(0..4) != Some(ASSET_MAGIC) {
            return Err(bad("no ASET header after the image"));
        }
        if u32_at(h, 4)? != 0 {
            return Err(bad("unknown asset header version"));
        }
        let part = |i: usize| -> io::Result<(u64, u64)> {
            let p = (u64_at(h, 8 + i * 16)?, u64_at(h, 16 + i * 16)?);
            if p.0.checked_add(p.1).is_none() {
                return Err(bad("asset part runs past the largest file size"));
            }
            Ok(p)
        };
        Ok(AssetLayout { icon: part(0)?, nacp: part(1)?, romfs: part(2)? })
    }
}

/// Writes image, asset header, icon and NACP; the caller then writes exactly `romfs_len` bytes of RomFS.
pub fn write_head(
    w: &mut impl Write,
    image: &NroImage,
    icon: &[u8],
    nacp: &[u8],
    romfs_len: u64,
) -> io::Result<AssetLayout> {
    let layout = AssetLayout::new(icon.len() as u64, nacp.len() as u64, romfs_len);
    w.write_all(&image.bytes)?;
    w.write_all(&layout.header_bytes())?;
    w.write_all(icon)?;
    w.write_all(nacp)?;
    Ok(layout)
}

/// Total file size of an `.nro` with these parts.
pub fn file_size(image: &NroImage, icon_len: u64, nacp_len: u64, romfs_len: u64) -> u64 {
    image.bytes.len() as u64 + AssetLayout::new(icon_len, nacp_len, romfs_len).len()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small synthetic ELF shaped like a libnx program: three segments, a branch + MOD0 offset at the start of
    /// the code, a MOD0 header in the code, a GNU build-id note.
    pub(crate) fn synthetic_elf() -> Vec<u8> {
        let mut elf = vec![0u8; 0x8000];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[6] = 1;
        elf[0x12..0x14].copy_from_slice(&183u16.to_le_bytes());
        elf[0x20..0x28].copy_from_slice(&0x40u64.to_le_bytes()); // phoff
        elf[0x36..0x38].copy_from_slice(&0x38u16.to_le_bytes());
        elf[0x38..0x3A].copy_from_slice(&3u16.to_le_bytes());
        // (flags, file offset, vaddr, filesz, memsz)
        let segs = [
            (5u32, 0x1000u64, 0u64, 0x1234u64, 0x1234u64),
            (4, 0x3000, 0x2000, 0x100, 0x100),
            (6, 0x4000, 0x3000, 0x80, 0x2500),
        ];
        for (i, (flags, off, va, fsz, msz)) in segs.into_iter().enumerate() {
            let o = 0x40 + i * 0x38;
            elf[o..o + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
            elf[o + 4..o + 8].copy_from_slice(&flags.to_le_bytes());
            elf[o + 8..o + 0x10].copy_from_slice(&off.to_le_bytes());
            elf[o + 0x10..o + 0x18].copy_from_slice(&va.to_le_bytes());
            elf[o + 0x20..o + 0x28].copy_from_slice(&fsz.to_le_bytes());
            elf[o + 0x28..o + 0x30].copy_from_slice(&msz.to_le_bytes());
            for k in 0..fsz as usize {
                elf[off as usize + k] = (k as u8).wrapping_mul(3).wrapping_add(i as u8 + 1);
            }
        }
        // Code start: branch, MOD0 offset 0x100, "HOMEBREW", then the reserved header area.
        elf[0x1000..0x1004].copy_from_slice(&0x1400_0020u32.to_le_bytes());
        elf[0x1004..0x1008].copy_from_slice(&0x100u32.to_le_bytes());
        elf[0x1008..0x1010].copy_from_slice(b"HOMEBREW");
        elf[0x1100..0x1104].copy_from_slice(b"MOD0");
        // One SHT_NOTE section holding the build ID, at file offset 0x5000.
        let note_off = 0x5000usize;
        let desc: Vec<u8> = (1..=20).collect();
        elf[note_off..note_off + 4].copy_from_slice(&4u32.to_le_bytes());
        elf[note_off + 4..note_off + 8].copy_from_slice(&20u32.to_le_bytes());
        elf[note_off + 8..note_off + 12].copy_from_slice(&NT_GNU_BUILD_ID.to_le_bytes());
        elf[note_off + 12..note_off + 16].copy_from_slice(b"GNU\0");
        elf[note_off + 16..note_off + 36].copy_from_slice(&desc);
        let shoff = 0x6000usize;
        elf[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        elf[0x3A..0x3C].copy_from_slice(&0x40u16.to_le_bytes());
        elf[0x3C..0x3E].copy_from_slice(&2u16.to_le_bytes());
        let s = shoff + 0x40; // section 1 (section 0 is the null section)
        elf[s + 4..s + 8].copy_from_slice(&SHT_NOTE.to_le_bytes());
        elf[s + 0x18..s + 0x20].copy_from_slice(&(note_off as u64).to_le_bytes());
        elf[s + 0x20..s + 0x28].copy_from_slice(&36u64.to_le_bytes());
        elf
    }

    #[test]
    fn elf_to_image() {
        let elf = synthetic_elf();
        let img = NroImage::from_elf(&elf).unwrap();
        let h = img.header().unwrap();
        assert_eq!(h.segments, [(0, 0x2000), (0x2000, 0x1000), (0x3000, 0x1000)]);
        assert_eq!(h.size, 0x4000);
        assert_eq!(img.bytes.len(), 0x4000);
        // Data memsz 0x2500 from 0x3000 ends at 0x5500, page end 0x6000; the image ends at 0x4000.
        assert_eq!(h.bss, 0x2000);
        assert_eq!(&h.build_id[..20], &(1..=20).collect::<Vec<u8>>()[..]);
        assert!(h.build_id[20..].iter().all(|&b| b == 0));
        assert_eq!(h.mod0_offset, 0x100);
        // NroStart kept from the ELF; segment bytes copied at their addresses; padding zero.
        assert_eq!(&img.bytes[0..4], &elf[0x1000..0x1004]);
        assert_eq!(&img.bytes[8..0x10], b"HOMEBREW");
        assert_eq!(&img.bytes[0x80..0x1234], &elf[0x1080..0x2234]);
        assert!(img.bytes[0x1234..0x2000].iter().all(|&b| b == 0));
        assert_eq!(&img.bytes[0x2000..0x2100], &elf[0x3000..0x3100]);
        assert_eq!(&img.bytes[0x3000..0x3080], &elf[0x4000..0x4080]);
        // Round trip through from_nro.
        assert_eq!(NroImage::from_nro(&img.bytes).unwrap(), img);
    }

    #[test]
    fn rejects_bad_elves() {
        let mut elf = synthetic_elf();
        elf[0x1100] = b'X'; // MOD0 gone
        assert!(NroImage::from_elf(&elf).is_err());
        let mut elf = synthetic_elf();
        elf[0x38] = 2; // two segments
        assert!(NroImage::from_elf(&elf).is_err());
        let mut elf = synthetic_elf();
        elf[0x40 + 0x38 + 4] = 6; // rodata writable
        assert!(NroImage::from_elf(&elf).is_err());
        assert!(NroImage::from_elf(b"not an elf").is_err());
    }

    #[test]
    fn damaged_programs_are_refused_without_panicking() {
        // Every truncation of a good ELF, and of its image as an .nro.
        let elf = synthetic_elf();
        let img = NroImage::from_elf(&elf).unwrap().bytes;
        for n in (0..elf.len()).step_by(97).chain([4, 5, 6]) {
            let _ = NroImage::from_elf(&elf[..n]);
        }
        for n in (0..img.len()).step_by(61).chain([0x14, 0x20, 0x41]) {
            let _ = NroImage::from_nro(&img[..n]);
        }
        // A program-header offset and count that overflow, a data memory size past 4 GiB, a section-header
        // offset that overflows: refused (or, for the build ID, ignored).
        let mut e = synthetic_elf();
        e[0x20..0x28].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(NroImage::from_elf(&e).is_err());
        let mut e = synthetic_elf();
        let data_memsz = 0x40 + 2 * 0x38 + 0x28;
        e[data_memsz..data_memsz + 8].copy_from_slice(&(u64::MAX - 0x100).to_le_bytes());
        assert!(NroImage::from_elf(&e).is_err());
        let mut e = synthetic_elf();
        e[0x28..0x30].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(NroImage::from_elf(&e).unwrap().header().unwrap().build_id, [0; 0x20]);
        // An asset header whose part overflows.
        let mut h = AssetLayout::new(1, 2, 3).header_bytes();
        h[16 + 2 * 16..24 + 2 * 16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(AssetLayout::parse(&h).is_err());
    }

    #[test]
    fn assets() {
        let img = NroImage::from_elf(&synthetic_elf()).unwrap();
        let mut out = Vec::new();
        let layout = write_head(&mut out, &img, b"\xff\xd8icon", &[7u8; 0x4000], 0x270).unwrap();
        assert_eq!(layout.icon, (0x38, 6));
        assert_eq!(layout.nacp, (0x3E, 0x4000));
        assert_eq!(layout.romfs, (0x403E, 0x270));
        assert_eq!(out.len() as u64 + 0x270, file_size(&img, 6, 0x4000, 0x270));
        let parsed = AssetLayout::parse(&out[img.bytes.len()..]).unwrap();
        assert_eq!(parsed, layout);
    }
}
