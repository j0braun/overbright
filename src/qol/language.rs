//! The text languages the engine can show: the default, a language's file
//! and what the engine can do with it. Pure, so the settings layer can ask
//! (re-exported by [`crate::game_assets`]).

/// The language whose text file the game loads when nothing else is
/// chosen (`LANGUAGES/English.bin`).
pub const DEFAULT_LANGUAGE: &str = "English";

/// The text file of a language: strings (chunk 0x39000) and fonts (chunk
/// 0x30201).
pub fn text_file_of(language: &str) -> String {
    format!("LANGUAGES/{language}.bin")
}

/// Whether the engine can show a language's file well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    /// No, and why (a player's words).
    No(&'static str),
}

/// What the engine can do with the language file `name` (any case).
///
/// The one-byte languages read through the string table's character map
/// (`frontend::feng::strings`): **inferred** to work for every file that
/// has one, since the engine reads them as `English.bin`. Spanish and
/// Italian were tried and start in their language; the others were not. The rest
/// cannot be shown well today (docs/modding/asset-format/text.md §3.3,
/// §10).
pub fn language_support(name: &str) -> Support {
    // prov: inferred game_assets.language_support
    match name.to_ascii_lowercase().as_str() {
        "english" | "french" | "german" | "italian" | "spanish" | "dutch" | "swedish" | "danish" => Support::Yes,
        "korean" => Support::No("its character map is not decoded"),
        "japanese" | "chinese" | "thailand" => Support::No("its text table is empty"),
        _ => Support::No("Overbright does not know this language file"),
    }
}

/// Whether `name` can be a language name in a path (letters and digits).
pub fn valid_language_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.bytes().all(|b| b.is_ascii_alphanumeric())
}
