use std::path::Path;

/// The memory-card library's own texts (memcard/LOCALE_<LANGUAGE>.loc:
/// "LOCH" header, "LOCI" index, "LOCL" a count at +0x0C and the strings'
/// offsets from the LOCL tag at +0x10, UTF-16LE, NUL-terminated). English:
/// 0 "2.03.09", 1 "2.08.04", 2 Yes, 3 No, 4 Continue, 5 "PC hard disk",
/// 6 "Do you want to overwrite previously saved game?", 7 "Save
/// complete.", 8 "Unable to save %1.", 9 "Load complete.", 10 "Unable to
/// load %1.", 11 "Are you sure you want to delete %1?", 12 "Save game
/// deleted.", 13 "Unable to delete %1.".
pub mod mc_text {
    pub const OVERWRITE: usize = 6;
    pub const SAVE_DONE: usize = 7;
    pub const SAVE_FAILED: usize = 8;
    pub const LOAD_DONE: usize = 9;
    pub const LOAD_FAILED: usize = 10;
    pub const DELETE_ASK: usize = 11;
    pub const DELETED: usize = 12;
    pub const DELETE_FAILED: usize = 13;
}

pub fn parse_loc(data: &[u8]) -> Vec<String> {
    let Some(at) = data.windows(4).position(|w| w == b"LOCL") else { return Vec::new() };
    let u = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    let n = u(at + 0xC).unwrap_or(0).min(256);
    (0..n)
        .map(|i| {
            let start = at + u(at + 0x10 + 4 * i).unwrap_or(usize::MAX / 2);
            let units: Vec<u16> = data
                .get(start..)
                .unwrap_or(&[])
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&c| c != 0)
                .collect();
            String::from_utf16_lossy(&units).replace('\u{a0}', " ")
        })
        .collect()
}

/// Text `i` of the library's English table, with `%1` replaced by `arg`.
pub fn text(root: &Path, i: usize, arg: &str) -> String {
    let fallback = [
        "",
        "",
        "Yes",
        "No",
        "Continue",
        "PC hard disk",
        "Do you want to overwrite previously saved game?",
        "Save complete.",
        "Unable to save %1.",
        "Load complete.",
        "Unable to load %1.",
        "Are you sure you want to delete %1?",
        "Save game deleted.",
        "Unable to delete %1.",
    ];
    let loaded = crate::game_fs::read(root, "memcard/LOCALE_ENGLISH.loc").map(|d| parse_loc(&d)).unwrap_or_default();
    let t = loaded.get(i).cloned().unwrap_or_else(|| fallback.get(i).unwrap_or(&"").to_string());
    t.replace("%1", arg)
}
