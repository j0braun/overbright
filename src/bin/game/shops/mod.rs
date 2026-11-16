pub(crate) mod graphics;
pub(crate) mod performance;
pub(crate) mod preview;
pub(crate) mod body;
pub(crate) mod graphics_impl;
pub(crate) mod custom_hud;
pub(crate) mod specialties;
pub(crate) mod performance_impl;
pub(crate) mod rims;
pub(crate) mod session;

use crate::*;

/// The shops' state: the graphics setup being bought, the performance
/// purchase in progress, the shop visited (were `Game` fields).
pub(crate) struct ShopState {
    /// UI_IcePartsOverlay +0x116: the Nitrous Purge list's Test N2O Purge
    /// is on (0xC519BFC0 toggles it, its release 0xD871B0A4 clears it).
    pub(crate) purge_test: bool,
    /// The graphics shop's pending setup (0x8389C8), bought by Purchase
    /// Setup.
    pub(crate) setup: Setup,
    /// The vinyl layer being changed (0x8389A8) and the vinyl whose
    /// colours UI_Paint is choosing (0x839BB8).
    pub(crate) vinyl_layer: u8,
    pub(crate) vinyl: Option<u32>,
    /// UI_Paint's chosen vinyl colours not yet in the setup (screen
    /// +0x160, {slot, part}; 0x53C170 replaces a slot's).
    pub(crate) vinyl_pending: Vec<(u8, u32)>,
    /// UI_DecalMain's node (screen +0x154).
    pub(crate) decal_node: u32,
    /// What the open dialog's buttons do to the graphics setup.
    pub(crate) dialog_steps: Vec<Option<SetupStep>>,
    /// UI_BuyPerformanceParts while the performance package screen is up.
    pub(crate) perf_parts: Option<screens::PerfParts>,
    /// A performance purchase waiting on its Yes / No.
    pub(crate) pending_perf: Option<PerfPending>,
    /// The brand slots 0x8389B4… (one per package part, 0 at boot): the
    /// brand screen's Select writes its rows' brands there in row order
    /// (0x521F00); buying a part takes slot 0 (0x521B10), a package slot i
    /// for its part i (0x54FE60), and each slot read is set to 0xFFFFFFFF
    /// after. A purchase without the brand screen reads what is left
    /// (Wine: an unbranded part bought after a branded one appends
    /// 0xFFFFFFFF to the car's brand list, one bought first nothing).
    pub(crate) perf_brand_slots: [u32; 4],
    /// UI_QRCarSelect's transmission question is up (0x497D50) for this
    /// Quick Race (None: Free Run).
    pub(crate) ask_gearbox: Option<Option<usize>>,
    /// The performance category being shopped.
    pub(crate) perf_category: u8,
    /// Customizing from the garage (stock and owned parts only, 0x50F0D0
    /// mode 2) rather than in a shop (mode 6).
    pub(crate) garage_parts: bool,
    /// The shop being visited (the career's 0x86344C), by its hash
    /// (shop record +0x38).
    pub(crate) visit: Option<u32>,
    /// 0x836350: the shop being entered plays its movie first (0x499640).
    pub(crate) movie: bool,
    /// UI_GenericParts_Browser's category locks (item +0x22) as its
    /// strip was built: by the constructor (0x55E830 → 0x54FAF0 at
    /// 0x55EA8A → 0x545B60) and on leaving carbon fibre (0x567830), never
    /// on coming back from a category or after a purchase. None: the next
    /// strip is built afresh (`Game::new_parts_browser`).
    pub(crate) strip_locks: std::cell::RefCell<Option<Vec<bool>>>,
}

impl Default for ShopState {
    fn default() -> ShopState {
        ShopState {
            purge_test: false,
            setup: Setup::default(),
            vinyl_layer: 0x40,
            vinyl_pending: Vec::new(),
            vinyl: None,
            decal_node: 0,
            dialog_steps: Vec::new(),
            perf_parts: None,
            pending_perf: None,
            perf_brand_slots: [0; 4],
            ask_gearbox: None,
            perf_category: 3,
            garage_parts: false,
            visit: None,
            movie: false,
            strip_locks: Default::default(),
        }
    }
}
