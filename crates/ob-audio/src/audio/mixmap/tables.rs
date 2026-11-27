/// 0x7EE048..=0x7EE7EC (602 dwords): the linear level of 0x467FF0 at mB % 602 (index 601 + r).
pub(super) static DB_TO_LIN: crate::exe_tables::Table<[i32; 602]> = crate::exe_tables::Table::new(|t| &t.db_to_lin);
/// 0x7EE7F0 (512 dwords): the mB of a linear mantissa (0x468050).
pub(super) static LIN_TO_DB: crate::exe_tables::Table<[i32; 512]> = crate::exe_tables::Table::new(|t| &t.lin_to_db);
/// 0x7EA628 (192 × {u16 angle, f32 slope}): the 16-bit asin table of 0x43C490.
pub(super) static ASIN: crate::exe_tables::Table<[(u16, f32)]> = crate::exe_tables::Table::new(|t| &t.asin[..192]);
