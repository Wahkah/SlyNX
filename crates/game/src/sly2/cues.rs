//! Voice lines drive their speaker (dialogue-and-voice.md 4, audio.md 4): each timed record fired by the line
//! playing sets a goal on one of the speaker's state machines (targets 0, 1, 2, 4, 5; 5 is the mouth) or how far its
//! eyes are closed (target 3), through `engine::game::sly2_anim`. The speaker is the world's first object of the
//! line's speaker class.

use audio::sly2::CueValue;
use engine::game::sly2_anim;
use formats::iso::ReadAt;
use render::Backend;

use super::Sly2Session;

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// Applies the timed records the sound fired since the last frame.
    pub(super) fn apply_voice_cues(&mut self) {
        let (Some(sound), Some(g)) = (self.sound.as_mut(), self.game.as_mut()) else { return };
        for (class, rec) in sound.fired.drain(..) {
            let Some(speaker) = sly2_anim::speaker(&g.world, class) else { continue };
            let (goal, eyes) = match rec.value {
                CueValue::Goal(v) => (Some(v), None),
                CueValue::Eyes(v) => (None, Some(v)),
            };
            sly2_anim::with(&mut g.world, |a, w| a.cue(w, speaker, rec.target, goal, eyes));
        }
    }
}
