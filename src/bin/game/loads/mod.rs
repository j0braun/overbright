pub(crate) mod cache;
pub(crate) mod loading_screen;
pub(crate) mod preload;
pub(crate) mod drive_in;
pub(crate) mod kept;
pub(crate) mod preload_impl;
pub(crate) mod read_ahead;
pub(crate) mod warm;

use crate::*;
use crate::loads::preload::RivalKey;

/// A lost race's ending companion (the AI winner riding along,
/// `SceneCar::from_rival`: a copy of its car with an id of its own, made at
/// the race's end) has its renderer built on a thread once the player has
/// finished behind it: the winner's car loaded again from its key (the same
/// load as the race's set-up made; the renderer reads its parts, solids,
/// textures and light materials, which nothing in a race changes) and its
/// renderer built after it, as the ending's first frame would build it.
/// The ending offers it under the companion's id when it is done and was
/// built for that rival; otherwise the frame builds it in place as before.
pub(crate) struct CompanionAhead {
    /// The rival (index into the race's rivals) it was built for.
    pub(crate) rival: usize,
    pub(crate) job: std::thread::JoinHandle<()>,
    pub(crate) renderer: crate::loads::preload::CarRendererAhead,
}

/// The loads in progress and ahead, and the world kept between drives
/// (were `Game` fields). Every field starts empty (`Default`).
#[derive(Default)]
pub(crate) struct Loads {
    /// PC_Loading over the track load in progress (`screens::Loading`).
    pub(crate) loading: Option<screens::Loading>,
    /// PC_Loading over the game start (see [`BootLoading`]).
    pub(crate) boot_loading: Option<BootLoading>,
    /// The main menu's garage scene made during the title screen
    /// (`garage::ahead`).
    pub(crate) garage_ahead: Option<crate::garage::ahead::GarageAhead>,
    /// The scenes made ahead that a garage tick took (a check aid).
    pub(crate) garage_ahead_taken: u32,
    /// Track loads that showed it (selftest).
    pub(crate) loads_shown: u32,
    /// Track loads (free roam, races, test runs), shown or not.
    pub(crate) track_loads: u32,
    /// The last drive's world (its renderer, with the device it is on, is
    /// `RenderState.resident`), kept when the drive was dropped for a
    /// car change: the next load of that region takes it instead of
    /// reading the track again.
    pub(crate) resident: Option<(String, World)>,
    /// `resident` (with its renderer and water) being made ready for its
    /// next drive in the menus ([`kept::KeptAhead`]).
    pub(crate) kept_ahead: Option<kept::KeptAhead>,
    /// The menus' loads ahead ([`Preload`]).
    pub(crate) preload: Option<Preload>,
    /// A drive's programs being made ahead in the menus ([`warm::ProgramsAhead`]).
    pub(crate) programs: Option<warm::ProgramsAhead>,
    /// The engaged race's rivals loading ahead ([`RivalsAhead`]).
    pub(crate) rivals_ahead: Option<RivalsAhead>,
    /// The last race set-up's rivals: the cars it took from
    /// [`RivalsAhead`] and the cars it asked for (`line_up_rivals`).
    pub(crate) rivals_taken: (usize, usize),
    /// The race's rivals' car loads in rival order (`line_up_rivals`), for
    /// a lost race's ending companion ([`CompanionAhead`]).
    pub(crate) race_rivals: Vec<RivalKey>,
    /// The rivals' cars of a race raced again (Restart, Next Race), each
    /// with the key of the load that made it, for the next set-up
    /// (`line_up_rivals`), which takes a car for the same key instead of
    /// loading it again; how many the last set-up took.
    pub(crate) raced_cars: Vec<(RivalKey, Car)>,
    pub(crate) rivals_kept: usize,
    /// A lost race's ending companion's renderer being built during the
    /// race-over hold ([`CompanionAhead`]).
    pub(crate) companion_ahead: Option<CompanionAhead>,
    /// Free roam's encounter racers loading ahead ([`EncountersAhead`]),
    /// and the frames the menus going on to free roam have been up.
    pub(crate) encounters_ahead: Option<EncountersAhead>,
    /// The last free-roam set-up's encounter racers: the cars it took
    /// from [`EncountersAhead`] and the cars it asked for.
    pub(crate) encounters_taken: (usize, usize),
    pub(crate) free_roam_menu: Option<std::mem::Discriminant<Menu>>,
    pub(crate) free_roam_menu_secs: f32,
    /// The waits and the drives' frame times for the log ([`crate::shell::waits::Waits`]).
    pub(crate) waits: crate::shell::waits::Waits,
    /// The files after the first frame are queued for reading ahead
    /// (`read_ahead::Game::read_ahead_after_start`).
    pub(crate) read_ahead_queued: bool,
    /// The last tick's menus went on to free roam (the screens after them
    /// do too until a drive or a race runs).
    pub(crate) to_free_roam: bool,
    /// The profile's cars whose part map is made ahead, and the thread
    /// making it ([`Game::preload_owned_bits`]).
    pub(crate) owned_bits: Option<(Vec<String>, Option<std::thread::JoinHandle<()>>)>,
}
