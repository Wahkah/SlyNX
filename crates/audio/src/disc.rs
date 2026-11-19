//! Where the sound data is on the player's disc (audio.md 1): the effect-bank, ambient and commentary tables and
//! the prompt sound in the executable, and each world's music descriptor and sound bank.

use alloc::string::String;
use alloc::vec::Vec;

use formats::disc::{AMBIENT_TABLE, Descriptor, Disc, Executable, SOUND_BANK_TABLE};
use formats::iso::ReadAt;
use formats::options::OptionTable;
use formats::world::WorldLayout;

use crate::Error;
use crate::bank::{EffectBank, bank_sound_count};
use crate::music::MusicFile;
use crate::vag::{HEADER_SIZE, VagHeader, VagSound};

/// Virtual address of the prompt sound's descriptor.
pub const PROMPT_VA: u32 = 0x0026_A118;
/// The developer commentary table: virtual address and entry count (44-byte entries).
pub const COMMENTARY_TABLE: (u32, usize) = (0x0024_8748, 33);
/// Size of one commentary entry.
pub const COMMENTARY_ENTRY_SIZE: usize = 44;

/// The 7 effect-bank descriptors.
pub fn bank_descriptors(exe: &Executable) -> Result<Vec<Descriptor>, Error> {
    let t = exe.descriptor_table(SOUND_BANK_TABLE.0, SOUND_BANK_TABLE.1)?;
    t.into_iter().map(|d| d.ok_or(Error::Invalid("empty sound bank descriptor"))).collect()
}

/// The 6 ambient-loop descriptors.
pub fn ambient_descriptors(exe: &Executable) -> Result<Vec<Descriptor>, Error> {
    let t = exe.descriptor_table(AMBIENT_TABLE.0, AMBIENT_TABLE.1)?;
    t.into_iter().map(|d| d.ok_or(Error::Invalid("empty ambient descriptor"))).collect()
}

/// The prompt sound's descriptor.
pub fn prompt_descriptor(exe: &Executable) -> Result<Descriptor, Error> {
    exe.descriptor_table(PROMPT_VA, 1)?.remove(0).ok_or(Error::Invalid("empty prompt descriptor"))
}

/// One developer-commentary entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Commentary {
    /// World id the commentary belongs to.
    pub world: u32,
    /// Duration in seconds.
    pub duration: f32,
    /// The VAG file.
    pub file: Option<Descriptor>,
    /// The speakers' names, one per line.
    pub speakers: String,
}

/// The 33 commentary entries.
pub fn commentary_table(exe: &Executable) -> Result<Vec<Commentary>, Error> {
    let (va, n) = COMMENTARY_TABLE;
    let raw = exe.bytes_at(va, n * COMMENTARY_ENTRY_SIZE)?;
    let mut out = Vec::with_capacity(n);
    for e in raw.as_chunks::<COMMENTARY_ENTRY_SIZE>().0 {
        let word = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
        let text = word(40);
        out.push(Commentary {
            world: word(0),
            duration: f32::from_bits(word(4)),
            file: Descriptor::decode(&e[8..40])?,
            speakers: if text == 0 { String::new() } else { exe.c_string(text)? },
        });
    }
    Ok(out)
}

/// Reads and parses effect bank `index` (0..7).
pub fn read_bank<R: ReadAt>(disc: &mut Disc<R>, index: usize) -> Result<EffectBank, Error> {
    let d = *bank_descriptors(&disc.executable)?.get(index).ok_or(Error::Invalid("bank index"))?;
    EffectBank::parse(disc.read_descriptor(&d)?)
}

/// The sound count of each bank, from each bank's first sector (numbers the sound IDs, audio.md 3.6).
pub fn bank_sound_counts<R: ReadAt>(disc: &mut Disc<R>) -> Result<[u16; 7], Error> {
    let mut counts = [0u16; 7];
    for (i, d) in bank_descriptors(&disc.executable)?.iter().enumerate().take(7) {
        let head = disc.source().read_vec(d.offset(), 2048.min(d.size as usize))?;
        counts[i] = bank_sound_count(&head)?;
    }
    Ok(counts)
}

/// The music descriptor and sound-bank index of a decompressed world file (world-file.md 4.2 and 4.5).
pub fn world_audio(world: &[u8]) -> Result<(Option<Descriptor>, i16), Error> {
    let layout = WorldLayout::parse(world, &OptionTable::sly1())?;
    Ok((layout.music, layout.sound_bank))
}

/// Reads and parses a music file.
pub fn read_music<R: ReadAt>(disc: &mut Disc<R>, d: &Descriptor) -> Result<MusicFile, Error> {
    MusicFile::parse(disc.read_descriptor(d)?)
}

/// Reads a VAG file. A descriptor size of 0 (one dialogue reference) means "read the header for the size".
pub fn read_vag<R: ReadAt>(disc: &mut Disc<R>, d: &Descriptor) -> Result<VagSound, Error> {
    let size = if d.size == 0 {
        let head = disc.source().read_vec(d.offset(), HEADER_SIZE)?;
        VagHeader::parse(&head)?.file_size()
    } else {
        d.size as usize
    };
    VagSound::new(disc.source().read_vec(d.offset(), size)?)
}
