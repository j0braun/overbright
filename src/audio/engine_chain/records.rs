// The record types sit with the engine's tables (`exe_tables::records`).
pub use crate::exe_tables::records::{Rec80, Rec84};

/// The engine records (0x7F0DE0, 0x4C bytes): accel GIN (+0x0C), hybrid flag (+0x10),
/// decel record (+0x14), rpm max / min (+0x1C / +0x20), layer volumes (+0x24 / +0x28),
/// +0xD0 normaliser (+0x2C), A / B gains at t = 1 (+0x30 / +0x34) and t = 0 (+0x38 / +0x3C),
/// fixed gains (+0x40 / +0x44), minimum cutoff (+0x48).
pub static ENGINES: crate::exe_tables::Table<[Rec80; 43]> = crate::exe_tables::Table::new(|t| &t.engines);

/// The decel records (0x7F1AA8, 0x34 bytes): the _DCL GIN (+0), layer-A volume off
/// throttle (+4), decel volume (+8), trapezoid rpm ends (+0xC / +0x10), its ramp
/// fractions (+0x14 / +0x18), |+0xD0| normaliser (+0x1C), A / B gains at t2 = 1
/// (+0x24 / +0x28) and 0 (+0x2C / +0x30).
pub static DECELS: crate::exe_tables::Table<[Rec84; 37]> = crate::exe_tables::Table::new(|t| &t.decels);
