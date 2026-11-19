//! System events: focus, suspend, display mode, controllers, quit.

/// The current output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Render size in pixels: 1920x1080 docked, 1280x720 handheld on the Switch; the window or offscreen size on the
    /// PC.
    pub width: u32,
    pub height: u32,
    /// True when the Switch is docked (TV output). On the PC, true when the window is 1920x1080 or larger.
    pub docked: bool,
}

/// Something that happened since the previous `Platform::pump`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The system or the user asked the program to exit. `pump` returns `false` from now on.
    Quit,
    /// The program lost input focus (Switch: a system overlay or library applet is in front; PC: the window lost
    /// focus). Keep running, but consider pausing gameplay.
    FocusLost,
    /// Focus came back.
    FocusGained,
    /// The program went to the background (Switch: HOME menu or sleep; PC: window minimised). Pause the game and
    /// silence audio; the Switch may stop scheduling the program until it resumes.
    Suspended,
    /// The program is in the foreground again after [`Event::Suspended`] or after the console woke up.
    Resumed,
    /// Docked/handheld or window size changed. Render at the new size from the next frame.
    DisplayChanged(DisplayInfo),
    /// A controller connected for this player (0-based).
    ControllerConnected(u8),
    /// The player's controller disconnected.
    ControllerDisconnected(u8),
}
