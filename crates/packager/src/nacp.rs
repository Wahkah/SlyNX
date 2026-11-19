//! NACP: the 0x4000-byte application control data the Homebrew Menu reads for an `.nro`'s title, author and
//! version.
//!
//! Field offsets follow libnx's `NacpStruct` (`switch/nacp.h`): 16 language entries of 0x300 bytes (name 0x200,
//! author 0x100) from 0, then the control fields from 0x3000. The other fields get the same values devkitPro's
//! `nacptool --create` writes for homebrew, so the Homebrew Menu treats our `.nro` like any other (the tests compare
//! the bytes when `nacptool` is installed).

/// Size of the structure.
pub const NACP_SIZE: usize = 0x4000;
/// Languages that get the title and author (American English through Russian, as `nacptool` fills them).
pub const LANGUAGES_FILLED: usize = 12;
const LANG_ENTRY: usize = 0x300;
const NAME_LEN: usize = 0x200;
const AUTHOR_LEN: usize = 0x100;
const VERSION_LEN: usize = 0x10;

const STARTUP_USER_ACCOUNT: usize = 0x3025;
const SUPPORTED_LANGUAGE_FLAG: usize = 0x302C;
const DATA_LOSS_CONFIRMATION: usize = 0x3036;
const RATING_AGE: usize = 0x3040;
const DISPLAY_VERSION: usize = 0x3060;
const USER_ACCOUNT_SAVE_DATA_SIZE: usize = 0x3080;
const USER_ACCOUNT_SAVE_DATA_JOURNAL_SIZE: usize = 0x3088;
const LOGO_TYPE: usize = 0x30F0;
const LOGO_HANDLING: usize = 0x30F1;

/// Rating ages per rating organisation (0xFF: no rating), the homebrew defaults.
const RATING_AGES: [u8; 12] = [12, 0xFF, 0xFF, 10, 0xFF, 12, 12, 12, 12, 12, 13, 13];

/// What the Homebrew Menu shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nacp {
    pub title: String,
    pub author: String,
    pub version: String,
}

/// Copies the longest prefix of `s` that fits in `max - 1` bytes (keeping a NUL) without splitting a character.
fn put_str(out: &mut [u8], at: usize, max: usize, s: &str) {
    let mut end = s.len().min(max - 1);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out[at..at + end].copy_from_slice(&s.as_bytes()[..end]);
}

impl Nacp {
    pub fn new(title: &str, author: &str, version: &str) -> Nacp {
        Nacp { title: title.into(), author: author.into(), version: version.into() }
    }

    /// The 0x4000-byte structure. Strings that do not fit are cut at a character boundary.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut d = vec![0u8; NACP_SIZE];
        for lang in 0..LANGUAGES_FILLED {
            put_str(&mut d, lang * LANG_ENTRY, NAME_LEN, &self.title);
            put_str(&mut d, lang * LANG_ENTRY + NAME_LEN, AUTHOR_LEN, &self.author);
        }
        d[STARTUP_USER_ACCOUNT] = 1;
        d[SUPPORTED_LANGUAGE_FLAG..SUPPORTED_LANGUAGE_FLAG + 4].copy_from_slice(&0x0BFFu32.to_le_bytes());
        d[DATA_LOSS_CONFIRMATION] = 1;
        d[RATING_AGE..RATING_AGE + 0x20].fill(0xFF);
        d[RATING_AGE..RATING_AGE + RATING_AGES.len()].copy_from_slice(&RATING_AGES);
        put_str(&mut d, DISPLAY_VERSION, VERSION_LEN, &self.version);
        d[USER_ACCOUNT_SAVE_DATA_SIZE..USER_ACCOUNT_SAVE_DATA_SIZE + 8].copy_from_slice(&0x03E0_0000u64.to_le_bytes());
        d[USER_ACCOUNT_SAVE_DATA_JOURNAL_SIZE..USER_ACCOUNT_SAVE_DATA_JOURNAL_SIZE + 8]
            .copy_from_slice(&0x0018_0000u64.to_le_bytes());
        d[LOGO_TYPE] = 2;
        d[LOGO_HANDLING] = 1;
        d
    }

    /// Reads title, author (first language) and version back.
    pub fn parse(d: &[u8]) -> Option<Nacp> {
        if d.len() != NACP_SIZE {
            return None;
        }
        let s = |at: usize, len: usize| {
            let f = &d[at..at + len];
            let end = f.iter().position(|&b| b == 0).unwrap_or(len);
            String::from_utf8_lossy(&f[..end]).into_owned()
        };
        Some(Nacp { title: s(0, NAME_LEN), author: s(NAME_LEN, AUTHOR_LEN), version: s(DISPLAY_VERSION, VERSION_LEN) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields() {
        let d = Nacp::new("SlyNX 1", "Wahkah", "0.1.0").to_bytes();
        assert_eq!(d.len(), NACP_SIZE);
        assert_eq!(&d[0..8], b"SlyNX 1\0");
        assert_eq!(&d[0x200..0x207], b"Wahkah\0");
        assert_eq!(&d[0x300 * 11..0x300 * 11 + 7], b"SlyNX 1");
        assert!(d[0x300 * 12..0x3000].iter().all(|&b| b == 0), "languages 12..16 stay empty");
        assert_eq!(&d[0x3060..0x3066], b"0.1.0\0");
        assert_eq!(Nacp::parse(&d), Some(Nacp::new("SlyNX 1", "Wahkah", "0.1.0")));
    }

    #[test]
    fn long_strings_are_cut_on_char_boundaries() {
        let title = "é".repeat(400); // 800 bytes
        let d = Nacp::new(&title, "a", "1.2.3.4.5.6.7.8.9").to_bytes();
        let back = Nacp::parse(&d).unwrap();
        assert_eq!(back.title.len(), 0x1FE, "255 two-byte characters fit in 0x1FF bytes");
        assert!(back.title.chars().all(|c| c == 'é'));
        assert_eq!(back.version.len(), 15);
    }
}
