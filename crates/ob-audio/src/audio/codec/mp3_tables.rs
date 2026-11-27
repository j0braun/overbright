//! Tables of the MPEG audio layer III decoder in SPEED2.EXE, read from
//! the exe: the Huffman trees (pointer list 0x7D0C30, one per table; each
//! a tree of i16: negative entries skip ahead when the next bit is 1,
//! leaves hold x << 4 | y), the big-value tables' linbits (0x7D0CC0) and
//! the synthesis window (0x7D1600: 17 rows of 16 values, each row twice,
//! −32768 × the standard window D[32·j + k]).

/// Table n's tree (None: table 0, 4 or 14).
pub static HUFF: crate::exe_tables::Table<[Option<Box<[i16]>>; 32]> = crate::exe_tables::Table::new(|t| &t.mp3_huff);

/// Linbits of tables 16–31.
pub static LINBITS: crate::exe_tables::Table<[u8; 16]> = crate::exe_tables::Table::new(|t| &t.mp3_linbits);

/// The synthesis window rows (0x7D1600), −32768 × D[32·j + k].
pub static WINDOW_ROWS: crate::exe_tables::Table<[[f32; 16]; 17]> = crate::exe_tables::Table::new(|t| &t.mp3_window_rows);
