//! What the renderers draw, noted for the texture browser (tools::texed) while its panel is open. The state
//! lives here, in the render layer, so the renderers do not name the tool above them.

use crate::formats::texture::Texture;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static RECORDING: AtomicBool = AtomicBool::new(false);

/// The panel opens or closes: the renderers note what they draw, or stop.
pub fn set_recording(on: bool) {
    RECORDING.store(on, Ordering::Relaxed);
}

/// The renderers note what they draw (only while the panel is open).
pub fn recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

#[derive(Default)]
struct Noted {
    world_slots: Vec<usize>,
    /// (car, texture)
    textures: Vec<(bool, Texture)>,
}

fn noted() -> &'static Mutex<Noted> {
    static N: OnceLock<Mutex<Noted>> = OnceLock::new();
    N.get_or_init(Default::default)
}

/// The world renderer's main view drew these texture slots.
pub fn note_world_slots(slots: impl Iterator<Item = usize>) {
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    n.world_slots.extend(slots);
    n.world_slots.sort_unstable();
    n.world_slots.dedup();
}

/// A renderer drew these textures (`car`: the cars').
pub fn note_textures<'a>(car: bool, textures: impl Iterator<Item = &'a Texture>) {
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    for t in textures {
        if !n.textures.iter().any(|(c, x)| *c == car && x.hash == t.hash) {
            n.textures.push((car, t.clone()));
        }
    }
}

/// What was noted since the last call: the world's slots and the other textures.
pub fn take_noted() -> (Vec<usize>, Vec<(bool, Texture)>) {
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    (std::mem::take(&mut n.world_slots), std::mem::take(&mut n.textures))
}
