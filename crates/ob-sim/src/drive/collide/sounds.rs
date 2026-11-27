//! The sounds a collision step posts (impacts, `audio::collision`;
//! scrape pushes, `audio::scrape`), into the queues of the caller's
//! session instead of process-wide ones (two games in one process, or
//! parallel tests, took each other's impacts before). Each queue keeps its
//! cap as before: past it the oldest drop.

use crate::audio::world::impacts::{Impact, Pending};
use crate::audio::car::pushes::{Push, Pushes};

/// A session's posted contact sounds, waiting for its audio.
#[derive(Clone, Debug, Default)]
pub struct ContactSounds {
    /// The records' first-push impacts (cap 64).
    pub impacts: Pending,
    /// The records' scrape pushes (cap 256).
    pub pushes: Pushes,
}

impl ContactSounds {
    /// Post `later` (posted after everything in `self`) in its order, each
    /// under the cap as it was posted: the queues hold what posting all of
    /// them one by one into one queue would (dropping the oldest is the
    /// same whether the posts arrive one by one or in batches).
    pub fn append(&mut self, mut later: ContactSounds) {
        for i in later.impacts.drain() {
            self.impacts.post_impact(i);
        }
        for p in later.pushes.drain() {
            self.pushes.post(p);
        }
    }

    /// Whether nothing is posted.
    pub fn is_empty(&self) -> bool {
        self.impacts.as_slice().is_empty() && self.pushes.as_slice().is_empty()
    }

    /// The impacts posted, oldest first.
    pub fn impacts(&self) -> &[Impact] {
        self.impacts.as_slice()
    }

    /// The pushes posted, oldest first.
    pub fn pushes(&self) -> &[Push] {
        self.pushes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::car::pushes::Class;
    use glam::Vec3;

    fn impact(n: usize) -> Impact {
        Impact { at: Vec3::new(n as f32, 0.0, 0.0), kind: n % 9, volume: (n % 128) as u8, material: 0, crash_camera: false, splash: false }
    }

    fn push(n: usize) -> Push {
        Push { record: n as u64, class: Class::Track, slide: n as f32, at: Vec3::ZERO }
    }

    /// A tick's posts handed on in a batch leave the queues as posting
    /// them one by one into the session's queue would, the caps (64
    /// impacts, 256 pushes, the oldest dropped) included.
    #[test]
    fn batches_keep_the_caps_and_the_order() {
        for (before, tick) in [(0, 10), (30, 50), (60, 100), (0, 300), (250, 20)] {
            let mut one = ContactSounds::default();
            let mut batched = ContactSounds::default();
            for n in 0..before {
                one.impacts.post_impact(impact(n));
                one.pushes.post(push(n));
                batched.impacts.post_impact(impact(n));
                batched.pushes.post(push(n));
            }
            let mut events = ContactSounds::default();
            for n in before..before + tick {
                one.impacts.post_impact(impact(n));
                one.pushes.post(push(n));
                events.impacts.post_impact(impact(n));
                events.pushes.post(push(n));
            }
            batched.append(events);
            let key = |s: &ContactSounds| {
                (
                    s.impacts().iter().map(|i| i.at.x as usize).collect::<Vec<_>>(),
                    s.pushes().iter().map(|p| p.record).collect::<Vec<_>>(),
                )
            };
            assert_eq!(key(&one), key(&batched), "{before} then {tick}");
            assert!(one.impacts().len() <= 64 && one.pushes().len() <= 256);
        }
    }
}
