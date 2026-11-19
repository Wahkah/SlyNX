//! Keyboard and gamepad mapping onto the platform's buttons. Pure functions over SDL's raw state, tested here.
//!
//! Gamepads map by position, like a Switch controller: the bottom face button is B, the right one A, the left one Y,
//! the top one X. (SDL names them by Xbox position, A bottom, B right, X left, Y top; button labels are turned off
//! with `SDL_GAMECONTROLLER_USE_BUTTON_LABELS=0` so Nintendo controllers report by position too.)

use crate::ffi::*;
use platform_api::{Buttons, ControllerKind, Stick};

/// Trigger travel past which ZL/ZR count as pressed.
const TRIGGER_THRESHOLD: i16 = 16_384;

/// Keyboard bindings for player 1, documented in `docs/platform.md`.
pub const KEY_BUTTONS: [(usize, Buttons); 14] = [
    (scancode_letter(b'X'), Buttons::A),
    (scancode_letter(b'Z'), Buttons::B),
    (scancode_letter(b'V'), Buttons::X),
    (scancode_letter(b'C'), Buttons::Y),
    (scancode_letter(b'Q'), Buttons::L),
    (scancode_letter(b'E'), Buttons::R),
    (scancode_digit(1), Buttons::ZL),
    (scancode_digit(3), Buttons::ZR),
    (SCANCODE_RETURN, Buttons::PLUS),
    (SCANCODE_BACKSPACE, Buttons::MINUS),
    (SCANCODE_UP, Buttons::UP),
    (SCANCODE_DOWN, Buttons::DOWN),
    (SCANCODE_LEFT, Buttons::LEFT),
    (SCANCODE_RIGHT, Buttons::RIGHT),
];

fn key(keys: &[u8], code: usize) -> bool {
    keys.get(code).is_some_and(|&k| k != 0)
}

fn key_stick(keys: &[u8], left: u8, right: u8, up: u8, down: u8) -> Stick {
    let axis =
        |neg: u8, pos: u8| (key(keys, scancode_letter(pos)) as i32 - key(keys, scancode_letter(neg)) as i32) as f32;
    let (x, y) = (axis(left, right), axis(down, up));
    // Diagonals at full length 1, like a real stick.
    if x != 0.0 && y != 0.0 {
        Stick { x: x * core::f32::consts::FRAC_1_SQRT_2, y: y * core::f32::consts::FRAC_1_SQRT_2 }
    } else {
        Stick { x, y }
    }
}

/// Reads the keyboard: buttons, left stick (WASD) and right stick (IJKL).
pub fn keyboard(keys: &[u8]) -> (Buttons, Stick, Stick) {
    let mut b = Buttons::NONE;
    for (code, button) in KEY_BUTTONS {
        if key(keys, code) {
            b |= button;
        }
    }
    (b, key_stick(keys, b'A', b'D', b'W', b'S'), key_stick(keys, b'J', b'L', b'I', b'K'))
}

/// SDL gamepad buttons to platform buttons, by position.
pub const PAD_BUTTONS: [(i32, Buttons); 14] = [
    (CONTROLLER_BUTTON_A, Buttons::B),
    (CONTROLLER_BUTTON_B, Buttons::A),
    (CONTROLLER_BUTTON_X, Buttons::Y),
    (CONTROLLER_BUTTON_Y, Buttons::X),
    (CONTROLLER_BUTTON_BACK, Buttons::MINUS),
    (CONTROLLER_BUTTON_START, Buttons::PLUS),
    (CONTROLLER_BUTTON_LEFTSTICK, Buttons::LSTICK),
    (CONTROLLER_BUTTON_RIGHTSTICK, Buttons::RSTICK),
    (CONTROLLER_BUTTON_LEFTSHOULDER, Buttons::L),
    (CONTROLLER_BUTTON_RIGHTSHOULDER, Buttons::R),
    (CONTROLLER_BUTTON_DPAD_UP, Buttons::UP),
    (CONTROLLER_BUTTON_DPAD_DOWN, Buttons::DOWN),
    (CONTROLLER_BUTTON_DPAD_LEFT, Buttons::LEFT),
    (CONTROLLER_BUTTON_DPAD_RIGHT, Buttons::RIGHT),
];

fn axis(v: i16) -> f32 {
    (v as f32 / 32767.0).clamp(-1.0, 1.0)
}

/// Reads a gamepad through `button(sdl_button)` and `axis_of(sdl_axis)`. SDL's y axes point down; ours point up.
pub fn gamepad(button: impl Fn(i32) -> bool, axis_of: impl Fn(i32) -> i16) -> (Buttons, Stick, Stick) {
    let mut b = Buttons::NONE;
    for (sdl, ours) in PAD_BUTTONS {
        if button(sdl) {
            b |= ours;
        }
    }
    if axis_of(CONTROLLER_AXIS_TRIGGERLEFT) >= TRIGGER_THRESHOLD {
        b |= Buttons::ZL;
    }
    if axis_of(CONTROLLER_AXIS_TRIGGERRIGHT) >= TRIGGER_THRESHOLD {
        b |= Buttons::ZR;
    }
    let left = Stick { x: axis(axis_of(CONTROLLER_AXIS_LEFTX)), y: -axis(axis_of(CONTROLLER_AXIS_LEFTY)) };
    let right = Stick { x: axis(axis_of(CONTROLLER_AXIS_RIGHTX)), y: -axis(axis_of(CONTROLLER_AXIS_RIGHTY)) };
    (b, left, right)
}

pub fn kind(sdl_type: i32) -> ControllerKind {
    match sdl_type {
        CONTROLLER_TYPE_NINTENDO_SWITCH_PRO => ControllerKind::ProController,
        CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_LEFT => ControllerKind::JoyConLeft,
        CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_RIGHT => ControllerKind::JoyConRight,
        CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_PAIR => ControllerKind::JoyConPair,
        _ => ControllerKind::Gamepad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(pressed: &[usize]) -> Vec<u8> {
        let mut k = vec![0u8; 512];
        for &p in pressed {
            k[p] = 1;
        }
        k
    }

    #[test]
    fn keyboard_buttons_and_sticks() {
        let (b, l, r) = keyboard(&keys(&[scancode_letter(b'X'), SCANCODE_RETURN, scancode_letter(b'W')]));
        assert_eq!(b, Buttons::A | Buttons::PLUS);
        assert_eq!(l, Stick { x: 0.0, y: 1.0 });
        assert_eq!(r, Stick::ZERO);

        let (_, l, _) = keyboard(&keys(&[scancode_letter(b'A'), scancode_letter(b'S')]));
        assert!((l.magnitude() - 1.0).abs() < 1e-6 && l.x < 0.0 && l.y < 0.0);

        // Opposite keys cancel.
        let (_, _, r) = keyboard(&keys(&[scancode_letter(b'J'), scancode_letter(b'L')]));
        assert_eq!(r, Stick::ZERO);

        // A short keyboard array (SDL never returns one, but be safe) reads as nothing pressed.
        assert_eq!(keyboard(&[]).0, Buttons::NONE);
    }

    #[test]
    fn gamepad_by_position() {
        let (b, l, r) = gamepad(
            |btn| btn == CONTROLLER_BUTTON_A || btn == CONTROLLER_BUTTON_START,
            |ax| match ax {
                CONTROLLER_AXIS_LEFTY => -32768,
                CONTROLLER_AXIS_RIGHTX => 16384,
                CONTROLLER_AXIS_TRIGGERRIGHT => 32767,
                _ => 0,
            },
        );
        assert_eq!(b, Buttons::B | Buttons::PLUS | Buttons::ZR);
        assert_eq!(l, Stick { x: 0.0, y: 1.0 });
        assert!((r.x - 0.5).abs() < 1e-3 && r.y == 0.0);
    }

    #[test]
    fn kinds() {
        assert_eq!(kind(CONTROLLER_TYPE_NINTENDO_SWITCH_PRO), ControllerKind::ProController);
        assert_eq!(kind(CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_PAIR), ControllerKind::JoyConPair);
        assert_eq!(kind(1), ControllerKind::Gamepad);
    }
}
