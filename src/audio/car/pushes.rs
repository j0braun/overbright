//! The scrape sounds' events the physics posts (`drive::ContactSounds`): a push on a contact record and the
//! queue the audio drains. No playback (`scrape` plays them).

use glam::Vec3;

/// The sound object's class (0x459680).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Track,
    Cars,
}

/// One push on a contact record, as 0x4792D0 sees it.
#[derive(Clone, Copy, Debug)]
pub struct Push {
    /// The record (unique while it lives).
    pub record: u64,
    pub class: Class,
    /// Record +0x40: the sliding speed, m/s.
    pub slide: f32,
    pub at: Vec3,
}

/// The pushes posted by the physics, waiting for the audio (bounded: the
/// oldest drop): a session's own queue (`drive::ContactSounds`),
/// no longer one process-wide queue.
#[derive(Clone, Debug, Default)]
pub struct Pushes(Vec<Push>);

impl Pushes {
    /// A push on a record (0x5A9630 → 0x4792D0).
    pub fn post(&mut self, push: Push) {
        let p = &mut self.0;
        if p.len() >= 256 {
            p.remove(0);
        }
        p.push(push);
    }

    /// Take the posted pushes.
    pub fn drain(&mut self) -> Vec<Push> {
        std::mem::take(&mut self.0)
    }

    /// The pushes posted, oldest first.
    pub fn as_slice(&self) -> &[Push] {
        &self.0
    }
}
