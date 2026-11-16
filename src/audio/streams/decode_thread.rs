use super::*;

pub(super) fn new_buf() -> TrackBuf {
    Arc::new(Mutex::new(TrackPcm { channels: 2, rate: 32000, samples: Vec::new(), done: false }))
}

/// Start a decoder thread (unnamed, as `std::thread::spawn` makes it), or
/// with `inline` run it here and now (an offline mix: [`Streams::set_inline_decode`]).
/// Without a thread (the process at its thread limit) the track is silent
/// rather than the game stopping: the closure is dropped, and with it its
/// [`Decoding`] hold, which marks the buffer done. False then.
pub(super) fn spawn_decoder(inline: bool, f: impl FnOnce() + Send + 'static) -> bool {
    if inline {
        f();
        return true;
    }
    match std::thread::Builder::new().spawn(f) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("audio: no decoder thread ({e}): the track stays silent");
            false
        }
    }
}

/// A decoder thread's hold on its buffer: the buffer is done when the
/// thread ends, a panic included (a track whose decoder died would wait
/// for samples for good, a song's holding the music for the session).
pub(super) struct Decoding(pub(super) TrackBuf);

impl Drop for Decoding {
    fn drop(&mut self) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).done = true;
    }
}
