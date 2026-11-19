//! Turns the shim's controller readings into the platform's [`RawPad`]. Plain Rust, tested on the host.
//!
//! Facts used here, from libnx's `hid.h`: the low 16 `HidNpadButton` bits are A, B, X, Y, StickL, StickR, L, R, ZL,
//! ZR, Plus, Minus, Left, Up, Right, Down (the same layout as [`Buttons`]); bits 24..27 are SL and SR on the left
//! and right Joy-Con; sticks range over -0x7FFF..=0x7FFF with y up; the style tags are FullKey (Pro Controller) bit 0,
//! Handheld bit 1, JoyDual bit 2, JoyLeft bit 3, JoyRight bit 4.
#![cfg_attr(not(target_os = "horizon"), allow(dead_code))]

use platform_api::{Buttons, ControllerKind, MAX_PLAYERS, RawPad, Stick};

/// Mirrors `SlynxPadReading` in `c/slynx_shim.h`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadReading {
    pub style: u32,
    pub connected: u32,
    pub buttons: u64,
    pub lx: i32,
    pub ly: i32,
    pub rx: i32,
    pub ry: i32,
}

const _: () = assert!(core::mem::size_of::<PadReading>() == 32);

const STYLE_FULL_KEY: u32 = 1 << 0;
const STYLE_HANDHELD: u32 = 1 << 1;
const STYLE_JOY_DUAL: u32 = 1 << 2;
const STYLE_JOY_LEFT: u32 = 1 << 3;
const STYLE_JOY_RIGHT: u32 = 1 << 4;

const LEFT_SL: u64 = 1 << 24;
const LEFT_SR: u64 = 1 << 25;
const RIGHT_SL: u64 = 1 << 26;
const RIGHT_SR: u64 = 1 << 27;

const STICK_MAX: f32 = 32767.0;

pub fn kind(style: u32) -> ControllerKind {
    // Player 1 merges handheld with player 1, so prefer handheld when both report.
    if style & STYLE_HANDHELD != 0 {
        ControllerKind::Handheld
    } else if style & STYLE_FULL_KEY != 0 {
        ControllerKind::ProController
    } else if style & STYLE_JOY_DUAL != 0 {
        ControllerKind::JoyConPair
    } else if style & STYLE_JOY_LEFT != 0 {
        ControllerKind::JoyConLeft
    } else if style & STYLE_JOY_RIGHT != 0 {
        ControllerKind::JoyConRight
    } else {
        ControllerKind::Gamepad
    }
}

pub fn buttons(raw: u64) -> Buttons {
    let mut b = Buttons((raw & 0xFFFF) as u32);
    if raw & (LEFT_SL | RIGHT_SL) != 0 {
        b |= Buttons::SL;
    }
    if raw & (LEFT_SR | RIGHT_SR) != 0 {
        b |= Buttons::SR;
    }
    b
}

fn axis(v: i32) -> f32 {
    (v as f32 / STICK_MAX).clamp(-1.0, 1.0)
}

pub fn to_raw(r: &PadReading) -> RawPad {
    if r.connected == 0 {
        return RawPad::DISCONNECTED;
    }
    RawPad {
        kind: kind(r.style),
        buttons: buttons(r.buttons),
        left: Stick { x: axis(r.lx), y: axis(r.ly) },
        right: Stick { x: axis(r.rx), y: axis(r.ry) },
    }
}

pub fn to_raw_all(r: &[PadReading; MAX_PLAYERS]) -> [RawPad; MAX_PLAYERS] {
    core::array::from_fn(|i| to_raw(&r[i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_bits() {
        assert_eq!(buttons(1 << 0), Buttons::A);
        assert_eq!(buttons(1 << 10), Buttons::PLUS);
        assert_eq!(buttons(1 << 15), Buttons::DOWN);
        // Stick-as-direction bits (16..23) are dropped; the sticks carry that.
        assert_eq!(buttons(0xFF << 16), Buttons::NONE);
        assert_eq!(buttons(LEFT_SL | RIGHT_SR), Buttons::SL | Buttons::SR);
        assert_eq!(buttons(RIGHT_SL), Buttons::SL);
    }

    #[test]
    fn kinds() {
        assert_eq!(kind(STYLE_HANDHELD | STYLE_FULL_KEY), ControllerKind::Handheld);
        assert_eq!(kind(STYLE_FULL_KEY), ControllerKind::ProController);
        assert_eq!(kind(STYLE_JOY_DUAL), ControllerKind::JoyConPair);
        assert_eq!(kind(STYLE_JOY_LEFT), ControllerKind::JoyConLeft);
        assert_eq!(kind(STYLE_JOY_RIGHT), ControllerKind::JoyConRight);
    }

    #[test]
    fn readings() {
        assert_eq!(to_raw(&PadReading::default()), RawPad::DISCONNECTED);
        let r = PadReading {
            style: STYLE_FULL_KEY,
            connected: 1,
            buttons: 1 << 1,
            lx: 32767,
            ly: -32767,
            rx: 0,
            ry: 40000,
        };
        let p = to_raw(&r);
        assert_eq!(p.kind, ControllerKind::ProController);
        assert_eq!(p.buttons, Buttons::B);
        assert_eq!(p.left, Stick { x: 1.0, y: -1.0 });
        assert_eq!(p.right, Stick { x: 0.0, y: 1.0 });
    }
}
