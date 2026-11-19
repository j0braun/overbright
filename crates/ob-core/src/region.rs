//! The game's region (the executable's byte 0x7FA73C, read by 0x570A40;
//! 0x570A50 tells a European one: 1 or 5–9).
//!
//! The European 1.2 release stores 6 there and no code writes it. The
//! North American release's car folders differ: its CARS folder has CIVIC
//! and RSX and lacks CORSA and PEUGOT106 (the European build's lot and
//! opponent tables drop exactly CIVIC and RSX, the region-0 tables
//! 0x7F7C48 and 0x7F6DA4 drop CORSA and PEUGOT106). So the region is read
//! from the installed data: such a CARS folder is region 0, any other 6.
//!
//! What follows the region (each traced to its reader):
//! * the lot's two excluded cars (0x5022C0: 0x7F7C48 = CORSA, PEUGOT106
//!   for region 0, else 0x7F7C50 = CIVIC, RSX) and Quick Race's free cars
//!   (0x529CD0: 0x7F7C08 for region 0 with CIVIC in place of CORSA, else
//!   0x7F7C28);
//! * the random opponents' pool (0x4FEAF0: 0x7F6DA4 for region 0, else
//!   0x7F6DC0);
//! * the world's shared texture pack (0x57FC5E: GlobalTexturesNA.bin for
//!   region 0, else GlobalTexturesROW.bin);
//! * the credits file's prefix (0x4B8808: NA_ in English for region 0,
//!   UK_ for a European region);
//! * the lot's make and badge names (0x511E60: region 0 keeps every car's
//!   own; a European region shows VAUXHALL / CORSA, MX5 and COUPE);
//! * the needmybestbuy code and the AD_BESTBUY vinyl (0x575780 and the
//!   graphics shop: only in region 0).

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

/// North America.
pub const US: u32 = 0;
/// The European value the 1.2 release stores.
pub const EUROPE: u32 = 6;

static REGION: AtomicU32 = AtomicU32::new(EUROPE);

/// The region the game runs in (6 until [`init`] finds a US install).
pub fn get() -> u32 {
    REGION.load(Ordering::Relaxed)
}

/// Set the region for the whole process.
pub fn set(region: u32) {
    REGION.store(region, Ordering::Relaxed);
}

/// Whether `region` is a European one (0x570A50: 1 or 5–9).
pub fn european(region: u32) -> bool {
    region == 1 || (5..=9).contains(&region)
}

/// The region `root`'s CARS folder belongs to: 0 when CIVIC and RSX are
// prov: inferred region.detect
/// there and CORSA and PEUGOT106 are not, else 6. Names match without case.
pub fn detect(root: &Path) -> u32 {
    let has = |car: &str| crate::game_fs::find(root, &format!("CARS/{car}")).is_some_and(|p| p.is_dir());
    if has("CIVIC") && has("RSX") && !has("CORSA") && !has("PEUGOT106") { US } else { EUROPE }
}

/// Detect `root`'s region and set it ([`detect`], [`set`]).
pub fn init(root: &Path) -> u32 {
    let region = detect(root);
    if region != get() {
        log::info!("region: {region} (from the CARS folder)");
    }
    set(region);
    region
}
