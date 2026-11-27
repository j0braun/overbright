// The record type sits with the engine's tables (`exe_tables::records`).
pub use crate::exe_tables::records::ShiftRec;


/// The 16 shift records (fields +8..+0x5C less the volumes; read from the
/// EXE).
pub static SHIFT_RECS: crate::exe_tables::Table<[ShiftRec; 16]> = crate::exe_tables::Table::new(|t| &t.shift_recs);

/// The launch records (0x7F3248 + 20 · n, n = engine record +0x18 for the
/// player, 0x45CA80; AI cars take 0x7F3284 or 0x7F32C0 at random):
/// {f32 the second ramp's top, rpm added, launch ms, settle ms, lift ms}.
pub static LAUNCHES: crate::exe_tables::Table<[(f32, i32, i32, i32, i32); 12]> = crate::exe_tables::Table::new(|t| &t.launches);

/// Engine record +0x18 (the launch record) for entries 0..42.
pub static LAUNCH_INDEX: crate::exe_tables::Table<[u8; 43]> = crate::exe_tables::Table::new(|t| &t.launch_index);
