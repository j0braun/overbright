//! The game's render-side state, owned by the game (not by the
//! simulation session): the drive's world renderer, the cars' renderers by
//! the sim's car id, the kept region's renderer, the scene crowd's
//! renderer and the front end's garage scene. The sim keeps only ids
//! ([`crate::sim::car_id::DrawId`]).
use crate::sim::car_id::DrawId;
use crate::*;

/// A renderer per sim id, created where the frame first draws the id (as
/// the `Option` it was in the sim type) and dropped once the id is gone
/// ([`Keyed::sweep`]), or where the game drops it by hand.
pub(crate) struct Keyed<T> {
    map: std::collections::BTreeMap<u64, (std::sync::Weak<u64>, Option<T>)>,
}

impl<T> Default for Keyed<T> {
    fn default() -> Self {
        Keyed { map: Default::default() }
    }
}

impl<T> Keyed<T> {
    /// The id's slot (the `Option<renderer>` the sim type held).
    pub(crate) fn slot(&mut self, id: &DrawId) -> &mut Option<T> {
        &mut self.map.entry(id.key()).or_insert_with(|| (id.watch(), None)).1
    }

    pub(crate) fn get(&self, id: &DrawId) -> Option<&T> {
        self.map.get(&id.key()).and_then(|s| s.1.as_ref())
    }

    /// By the id's key (`DrawId::key`).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn get_key(&self, key: u64) -> Option<&T> {
        self.map.get(&key).and_then(|s| s.1.as_ref())
    }

    /// Whether the id's key has a renderer.
    pub(crate) fn has_key(&self, key: u64) -> bool {
        self.map.get(&key).is_some_and(|s| s.1.is_some())
    }

    pub(crate) fn get_mut(&mut self, id: &DrawId) -> Option<&mut T> {
        self.map.get_mut(&id.key()).and_then(|s| s.1.as_mut())
    }

    /// `slot = Some(r)`: the old one (if any) is dropped after the new one
    /// is in place, as the assignment to the sim type's field did.
    pub(crate) fn set(&mut self, id: &DrawId, r: T) {
        let old = self.slot(id).replace(r);
        drop(old);
    }

    /// The id's renderer dropped now (its car is dropped here).
    pub(crate) fn remove(&mut self, id: &DrawId) {
        self.map.remove(&id.key());
    }

    /// The renderers whose ids are gone, dropped (oldest id first).
    pub(crate) fn sweep(&mut self) {
        self.map.retain(|_, (alive, _)| alive.strong_count() > 0);
    }
}

/// The game's render-side state.
#[derive(Default)]
pub(crate) struct RenderState {
    /// The drive's world renderer (was `Drive.renderer`); there only while
    /// a drive is (`sim.drive`).
    pub(crate) world: Option<WorldRenderer>,
    /// The cars' renderers by car id: the player's (was
    /// `Drive.car_renderer`), the rivals', traffic's, encounter racers' and
    /// the shop scene's companion's (were their `renderer` fields).
    pub(crate) cars: Keyed<CarRenderer>,
    /// The race start scene's characters' renderer (was
    /// `SceneCrowd.renderer`), by the crowd's id.
    pub(crate) crowds: Keyed<nfsu2::render::characters::CharacterRenderer>,
    /// The kept region's renderer with the device it is on (was the third
    /// of `Loads.resident`); there only while `loads.resident` is.
    pub(crate) resident: Option<(wgpu::Device, WorldRenderer)>,
    /// The kept region's Animated Water (`Water::for_new_drive`), on the
    /// device of `resident`'s renderer: the next drive on that world takes
    /// it instead of building it again.
    pub(crate) resident_water: Option<nfsu2::render::water::Water>,
    /// What a kept world made ready ahead leaves for its load besides the
    /// world and its renderer (`loads::kept::KeptExtras`), between taking
    /// the world and building the drive.
    pub(crate) kept_extras: Option<crate::loads::kept::KeptExtras>,
    /// The garage scene under the front-end menus (was `GarageFe.scene`):
    /// its platform renderer and cars with the scene they are drawn in.
    pub(crate) garage: Option<GarageScene>,
    /// Photo mode's grade (was `Photo.grade`), by the photo's id.
    pub(crate) photo_grades: Keyed<nfsu2::render::photo_grade::PhotoGrade>,
    /// Car renderers built ahead of the frame their car first shows in
    /// (`render::cars_ahead`).
    pub(crate) cars_ahead: super::cars_ahead::CarsAhead,
    /// The streak buffer's commits by tick (`render::streak`).
    pub(crate) streak: super::streak::StreakState,
    /// What the drive's programs were last made for (`loads::warm`): at
    /// its load, or since when a Graphics row changed.
    pub(crate) warmed: Option<crate::loads::warm::WarmedFor>,
}

impl RenderState {
    /// The renderers of sim objects dropped since the last sweep.
    pub(crate) fn sweep(&mut self) {
        self.cars.sweep();
        self.crowds.sweep();
        self.photo_grades.sweep();
    }
}
