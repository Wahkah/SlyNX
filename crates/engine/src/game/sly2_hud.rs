//! What the Sly 2 HUD shows, gathered in the world for the session's UI (user-interface.md 5): the current
//! character's vitals (written by the gang each frame) and the prompts scripts ask for (operation 137's object,
//! option 979 = stream 1300: `(string, seconds)`, negative seconds until ended).

use alloc::vec::Vec;

/// Stream option of the prompt object's "show a prompt" method (script option 979 on class 154).
pub const SHOW_PROMPT: u16 = 1300;
/// Stream option of the prompt object's "end the prompt" method (script option 980) [inference from the class
/// map's neighbour].
pub const END_PROMPT: u16 = 1301;

/// The HUD's inputs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sly2Hud {
    /// The current character (0 Sly, 1 Bentley, 2 Murray), none when the current object is not one of them.
    pub who: Option<u8>,
    /// Health (value, maximum).
    pub health: (f32, f32),
    /// Gadget power (value, maximum).
    pub power: Option<(f32, f32)>,
    /// Prompts asked for since the session last took them: (string ID, seconds); seconds 0 and string −1 end the
    /// prompt.
    pub prompts: Vec<(i32, f32)>,
}
