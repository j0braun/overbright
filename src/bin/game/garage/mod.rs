pub(crate) mod ahead;
pub(crate) mod draw;
pub(crate) mod request;
pub(crate) mod rig;
pub(crate) mod scene;
pub(crate) mod snapshot;

use crate::*;

/// GarageMain under the front end, its view and car swing state (were
/// `Game` fields).
pub(crate) struct GarageFe {
    // The garage scene (GarageMain.fng) under the front-end menus
    // is `RenderState.garage`.
    /// GarageMain's view while it is up: its platform scene, the last
    /// view request applied (see `garage_view_request`) and the view.
    pub(crate) view: Option<(String, Option<GarageViewRequest>, nfsu2::garage::View)>,
    /// The last scene a front-end screen showed, kept under the dialogs
    /// it opens (`garage_request`).
    pub(crate) last: Option<GarageRequest>,
    /// The request the last `garage_update` made (`garage_request`).
    pub(crate) request: Option<GarageRequest>,
    /// The screen GarageMain stood under last frame (`GarageViewPhase`),
    /// for the screens' returns to the base view (0x4C2140).
    pub(crate) phase: GarageViewPhase,
    /// The car lot's selection last frame (index, count, car).
    pub(crate) lot: Option<(usize, usize, screens::CarChoice)>,
    /// The lot's last Left / Right time (s; UI_CareerCarSelect +0xB8,
    /// UI_CareerCarLot +0xE0, UI_QRCarSelect +0xC4) until its car is asked
    /// for 0.3 s later (`fe_rules::CRIB_SWING_DELAY`).
    pub(crate) crib: Option<f32>,
    /// The same step's real-time clock 0x8651A8 (1/4000 s): the screens
    /// stamp it and their tick tests it.
    pub(crate) crib_real: u64,
    /// The car the platform shows (GarageMain's loaded FE car), and the
    /// one asked for while a swing still turns (the loader's queued ride
    /// +0x23D0 / +0x2CD8, the last one asked wins).
    pub(crate) crib_shown: Option<screens::CarChoice>,
    pub(crate) lot_queued: Option<screens::CarChoice>,
    /// The FE car type GarageMain's loader last loaded (its +0x10, the
    /// car type the loader's 0x4D80C3 compares; `None` when a fresh
    /// GarageMain has loaded none: the constructor's +0x2CE0 = 1).
    pub(crate) type_shown: Option<Option<String>>,
    /// The hood, trunk and doors whose 0x61F830 sound has played.
    pub(crate) parts_sounded: [bool; 4],
    /// UI_IcePartsOverlay's Open/Close Hood / Doors (BUTTON1 on the Split
    /// Hoods or Doors list, 0x568D40 → 0x521050): the channels (0 hood, 2
    /// and 3 the doors) it holds open, with the list they were opened in.
    pub(crate) toggled: Option<(u8, Option<usize>, [bool; 4])>,
    /// GarageMain +0x148: the last car step was Left (its swing turns
    /// that way and plays LEFT on CARSEL; 0 from the constructor).
    pub(crate) left: bool,
}

impl Default for GarageFe {
    fn default() -> GarageFe {
        GarageFe {
            view: None,
            last: None,
            request: None,
            phase: GarageViewPhase::Other,
            lot: None,
            crib: None,
            crib_real: 0,
            crib_shown: None,
            type_shown: None,
            lot_queued: None,
            parts_sounded: [false; 4],
            toggled: None,
            left: false,
        }
    }
}
