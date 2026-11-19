//! Queuing music streams by number (sly2/audio.md 3.2; script action 146, options-by-class.md 2.1): the game's
//! script interface to the music stack. Sly 3 plays music the same way (sly3/audio.md: "as Sly 2") and uses this
//! type with its own disc reader.
//!
//! A script asks for music number `n`; the world's sound section names the stream file for it
//! ([`crate::sly2::WorldSound::music_stream`]). [`MusicQueue::queue`] reads the stream once per world (later calls
//! reuse it), pushes it on the engine's music stack (the current top fades out; up to 8 kept) and returns the
//! handle the script gets back. [`MusicQueue::stop`] pops the top, restarting the one below.
//!
//! Reading a stream is disc I/O on the calling thread. The caller passes the reader, so the same queue works
//! for [`crate::sly2::Sly2Disc::read_music_stream`] and [`crate::sly3::disc::Sly3Disc::read_music_stream`].

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::engine::{SoundEngine, SoundHandle};
use crate::sly2::vpk::VpkStream;

/// The music streams one world has queued, by number.
#[derive(Clone, Debug, Default)]
pub struct MusicQueue {
    streams: Vec<(i16, Option<Arc<VpkStream>>)>,
}

impl MusicQueue {
    /// An empty queue (one per loaded world).
    pub fn new() -> MusicQueue {
        MusicQueue::default()
    }

    /// Queues music number `n`: the stream read with `read` (once per number) and pushed on the music stack.
    /// Returns its handle, or [`SoundHandle::NONE`] when the world has no stream with that number.
    pub fn queue<E>(
        &mut self,
        engine: &mut SoundEngine,
        n: i16,
        read: impl FnOnce(i16) -> Result<Option<VpkStream>, E>,
    ) -> Result<SoundHandle, E> {
        let stream = match self.streams.iter().find(|s| s.0 == n) {
            Some(s) => s.1.clone(),
            None => {
                let s = read(n)?.map(Arc::new);
                self.streams.push((n, s.clone()));
                s
            }
        };
        Ok(match stream {
            Some(s) => engine.push_music(s),
            None => SoundHandle::NONE,
        })
    }

    /// Stops the top music stream (it fades out) and restarts the one below it.
    pub fn stop(&mut self, engine: &mut SoundEngine) {
        engine.pop_music();
    }

    /// The numbers read so far (and whether each named a stream).
    pub fn numbers(&self) -> impl Iterator<Item = (i16, bool)> + '_ {
        self.streams.iter().map(|s| (s.0, s.1.is_some()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream() -> VpkStream {
        VpkStream::parse(&crate::sly2::vpk::tests::file(2, 64, 64, 32_000)).unwrap()
    }

    #[test]
    fn queue_reads_once_and_pushes() {
        let (mut e, _r) = crate::create(crate::Config::default());
        let mut q = MusicQueue::new();
        let mut reads = 0;
        let mut read = |n: i16| -> Result<Option<VpkStream>, ()> {
            reads += 1;
            Ok((n == 13).then(stream))
        };
        let a = q.queue(&mut e, 13, &mut read).unwrap();
        assert_ne!(a, SoundHandle::NONE);
        assert_eq!(e.music_top(), a);
        assert_eq!(q.queue(&mut e, 99, &mut read).unwrap(), SoundHandle::NONE, "no stream 99");
        let b = q.queue(&mut e, 13, &mut read).unwrap();
        assert_ne!(b, a);
        assert_eq!(e.music_top(), b);
        drop(read);
        assert_eq!(reads, 2, "stream 13 is read once");
        q.stop(&mut e);
        assert_ne!(e.music_top(), SoundHandle::NONE, "the one below restarts");
        assert_eq!(q.numbers().collect::<Vec<_>>(), [(13, true), (99, false)]);
    }
}
