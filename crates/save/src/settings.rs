//! The port's own options (save-format.md 5): kept in their own file so they survive slot changes. The original's
//! settings bits (vibration, sound, music, inverts) stay in the slot ([`crate::model::Settings`]).
//!
//! Stored in the container of [`crate::codec`] with magic `SLYNXCF\0`, one section (tag 1) of entries `u16 key,
//! u16 length, value`. Entries with keys this build does not know are kept and written back unchanged, so options
//! added by a newer SlyNX survive a round trip through an older one.

use crate::codec::{self, DecodeError, GAME_SLY1, Reader, SETTINGS_MAGIC, Writer};
use alloc::vec::Vec;

pub const SETTINGS_VERSION: u16 = 1;
const TAG_ENTRIES: u16 = 1;

const KEY_DEADZONE_INNER: u16 = 1;
const KEY_DEADZONE_OUTER: u16 = 2;
const KEY_MASTER_VOLUME: u16 = 3;
const KEY_MUSIC_VOLUME: u16 = 4;
const KEY_EFFECTS_VOLUME: u16 = 5;
const KEY_SHOW_FPS: u16 = 6;
const KEY_LAST_SLOT: u16 = 7;
const KEY_CONFIRM_BUTTON: u16 = 8;

/// Which face button confirms in menus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConfirmButton {
    /// The right face button (A), the Switch's convention.
    #[default]
    Right,
    /// The bottom face button (B), the original controller's position for "accept".
    Bottom,
}

/// The port's options.
#[derive(Clone, Debug, PartialEq)]
pub struct PortSettings {
    /// Stick deadzone, 0..=1 (inner < outer).
    pub deadzone_inner: f32,
    pub deadzone_outer: f32,
    /// Volumes, 0..=1.
    pub master_volume: f32,
    pub music_volume: f32,
    pub effects_volume: f32,
    pub show_fps: bool,
    /// The slot used last (0..3), offered first in the load menu.
    pub last_slot: Option<u8>,
    pub confirm_button: ConfirmButton,
    /// Entries written by a newer SlyNX, kept as they are.
    pub unknown: Vec<(u16, Vec<u8>)>,
}

impl Default for PortSettings {
    fn default() -> Self {
        PortSettings {
            deadzone_inner: 0.15,
            deadzone_outer: 0.95,
            master_volume: 1.0,
            music_volume: 1.0,
            effects_volume: 1.0,
            show_fps: false,
            last_slot: None,
            confirm_button: ConfirmButton::Right,
            unknown: Vec::new(),
        }
    }
}

fn unit(v: f32, default: f32) -> f32 {
    if v.is_nan() { default } else { v.clamp(0.0, 1.0) }
}

impl PortSettings {
    /// Brings every value into range.
    pub fn clamp(&mut self) {
        let d = PortSettings::default();
        self.deadzone_inner = unit(self.deadzone_inner, d.deadzone_inner);
        self.deadzone_outer = unit(self.deadzone_outer, d.deadzone_outer);
        if self.deadzone_inner >= self.deadzone_outer {
            self.deadzone_inner = d.deadzone_inner;
            self.deadzone_outer = d.deadzone_outer;
        }
        self.master_volume = unit(self.master_volume, 1.0);
        self.music_volume = unit(self.music_volume, 1.0);
        self.effects_volume = unit(self.effects_volume, 1.0);
        if self.last_slot.is_some_and(|s| s >= 3) {
            self.last_slot = None;
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.section(TAG_ENTRIES, |w| {
            let mut entry = |key: u16, value: &[u8]| {
                w.u16(key);
                w.u16(value.len() as u16);
                w.buf.extend_from_slice(value);
            };
            entry(KEY_DEADZONE_INNER, &self.deadzone_inner.to_le_bytes());
            entry(KEY_DEADZONE_OUTER, &self.deadzone_outer.to_le_bytes());
            entry(KEY_MASTER_VOLUME, &self.master_volume.to_le_bytes());
            entry(KEY_MUSIC_VOLUME, &self.music_volume.to_le_bytes());
            entry(KEY_EFFECTS_VOLUME, &self.effects_volume.to_le_bytes());
            entry(KEY_SHOW_FPS, &[self.show_fps as u8]);
            entry(KEY_LAST_SLOT, &[self.last_slot.unwrap_or(0xFF)]);
            entry(KEY_CONFIRM_BUTTON, &[matches!(self.confirm_button, ConfirmButton::Bottom) as u8]);
            for (k, v) in &self.unknown {
                entry(*k, v);
            }
        });
        codec::seal(SETTINGS_MAGIC, SETTINGS_VERSION, 1, GAME_SLY1, &w.buf)
    }

    /// Decodes a settings file. Values of the wrong size are ignored (the default stays); the result is clamped.
    pub fn decode(data: &[u8]) -> Result<PortSettings, DecodeError> {
        let opened = codec::open(SETTINGS_MAGIC, SETTINGS_VERSION, GAME_SLY1, data)?;
        let mut s = PortSettings::default();
        for section in codec::sections(opened.payload) {
            let (tag, body) = section?;
            if tag != TAG_ENTRIES {
                continue;
            }
            let mut r = Reader::new(body);
            let mut pos = 0usize;
            while let (Some(key), Some(len)) = (r.u16(), r.u16()) {
                let start = pos + 4;
                let Some(value) = body.get(start..start + len as usize) else { return Err(DecodeError::Truncated) };
                pos = start + len as usize;
                r = Reader::new(&body[pos..]);
                let f = || value.try_into().ok().map(f32::from_le_bytes);
                let b = || (value.len() == 1).then(|| value[0]);
                match key {
                    KEY_DEADZONE_INNER => s.deadzone_inner = f().unwrap_or(s.deadzone_inner),
                    KEY_DEADZONE_OUTER => s.deadzone_outer = f().unwrap_or(s.deadzone_outer),
                    KEY_MASTER_VOLUME => s.master_volume = f().unwrap_or(s.master_volume),
                    KEY_MUSIC_VOLUME => s.music_volume = f().unwrap_or(s.music_volume),
                    KEY_EFFECTS_VOLUME => s.effects_volume = f().unwrap_or(s.effects_volume),
                    KEY_SHOW_FPS => s.show_fps = b().is_some_and(|v| v != 0),
                    KEY_LAST_SLOT => s.last_slot = b().filter(|&v| v != 0xFF),
                    KEY_CONFIRM_BUTTON => {
                        s.confirm_button = if b() == Some(1) { ConfirmButton::Bottom } else { ConfirmButton::Right }
                    }
                    _ => s.unknown.push((key, value.to_vec())),
                }
            }
        }
        s.clamp();
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unknown_entries() {
        let mut s = PortSettings {
            deadzone_inner: 0.2,
            deadzone_outer: 0.9,
            master_volume: 0.5,
            music_volume: 0.25,
            effects_volume: 0.75,
            show_fps: true,
            last_slot: Some(2),
            confirm_button: ConfirmButton::Bottom,
            unknown: alloc::vec![(500, alloc::vec![1, 2, 3])],
        };
        assert_eq!(PortSettings::decode(&s.encode()).unwrap(), s);
        s.unknown.clear();
        assert_eq!(PortSettings::decode(&s.encode()).unwrap(), s);
        assert_eq!(PortSettings::decode(&PortSettings::default().encode()).unwrap(), PortSettings::default());
    }

    #[test]
    fn clamping_and_damage() {
        let s = PortSettings {
            deadzone_inner: 0.99,
            deadzone_outer: 0.5,
            master_volume: 4.0,
            music_volume: f32::NAN,
            last_slot: Some(9),
            ..PortSettings::default()
        };
        let d = PortSettings::decode(&s.encode()).unwrap();
        assert_eq!((d.deadzone_inner, d.deadzone_outer), (0.15, 0.95));
        assert_eq!((d.master_volume, d.music_volume, d.last_slot), (1.0, 1.0, None));
        let mut f = s.encode();
        let n = f.len();
        f[n - 2] ^= 1;
        assert_eq!(PortSettings::decode(&f).err(), Some(DecodeError::Checksum));
    }
}
