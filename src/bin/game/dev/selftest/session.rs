//! The selftest's session reset and its guards for the process-wide
//! switches it sets: each guard puts its
//! switch back when dropped, so a step that fails with `?` leaves no
//! switch set for the steps after it. On success each is dropped where
//! the step restored the switch by hand before.
use crate::*;
use std::sync::atomic::Ordering;

impl Game {
    /// The game left and started again: only the files are kept (the
    /// drive, the scenes, the D-Day state, the GPS and the profile go,
    /// and no profile is current).
    pub(crate) fn forget_session(&mut self) {
        self.drop_drive();
        self.scenes.shop = None;
        self.shop.visit = None;
        self.career_world.pending_explore = None;
        self.sim.paused_race = None;
        self.career_world.dday = Default::default();
        self.career_world.dday_prompt = None;
        self.career_world.career_scene_played = false;
        self.career_world.gps = None;
        self.profile = None;
        profiles::clear_current();
    }
}

/// `career::UNLOCK_ALL` set for a while; the old value back on drop.
pub(crate) struct UnlockAll(bool);

impl UnlockAll {
    pub(crate) fn set(on: bool) -> Self {
        Self(career::UNLOCK_ALL.swap(on, Ordering::Relaxed))
    }
}

impl Drop for UnlockAll {
    fn drop(&mut self) {
        career::UNLOCK_ALL.store(self.0, Ordering::Relaxed);
    }
}

/// Enhancements applied for a while (`Enhancements::apply`, process-wide);
/// the settings' own applied again on drop.
pub(crate) struct EnhApplied(nfsu2::qol::Enhancements);

impl EnhApplied {
    /// `enh` applied; `settings` (the game's own) applied on drop.
    pub(crate) fn apply(enh: nfsu2::qol::Enhancements, settings: nfsu2::qol::Enhancements) -> Self {
        enh.apply();
        Self(settings)
    }
}

impl Drop for EnhApplied {
    fn drop(&mut self) {
        self.0.apply();
    }
}

/// The headless audio capture (`AUDIO_CAPTURE`) on, an empty buffer; off
/// on drop.
pub(crate) struct AudioCapture;

impl AudioCapture {
    pub(crate) fn start() -> Self {
        *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
        Self
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        *AUDIO_CAPTURE.lock().unwrap() = None;
    }
}
