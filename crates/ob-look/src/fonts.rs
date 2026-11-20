//! The look's type: Sofia Sans Semi Condensed (SIL OFL 1.1, the Sofia Sans
//! Project Authors; `fonts/OFL.txt` beside this crate's sources), five static
//! instances cut from the variable fonts by tools/cut_fonts.py (subset to the
//! Latin the launcher and the editor need, GPOS pair kerning flattened into a
//! `kern` table).

macro_rules! face_file {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/SofiaSansSemiCondensed-", $name, ".ttf"))
    };
}

/// The upright 400 face (reading text).
pub static BODY: &[u8] = face_file!("Body");
/// The upright 600 face (values, status, the letters inside glyphs).
pub static BODY_SEMI: &[u8] = face_file!("BodySemi");
/// The italic 600 face (buttons, labels, tabs).
pub static LABEL_ITALIC: &[u8] = face_file!("LabelItalic");
/// The italic 700 face (titles).
pub static TITLE_ITALIC: &[u8] = face_file!("TitleItalic");
/// The italic 800 face (the main action).
pub static PLAY_ITALIC: &[u8] = face_file!("PlayItalic");

/// The licence text that goes with the faces (their own notice).
pub const LICENCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/OFL.txt"));
