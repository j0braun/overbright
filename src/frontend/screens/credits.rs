use crate::frontend::fe_ids;
use super::*;

/// Credits.fng (screen 0x4B8800): the lines of CREDITS\UK_ENGLISH.TXT
/// (UTF-16; "CREDITS\%s%s.TXT" of a prefix and the language name,
/// 0x4B8808: in English NA_ for region 0, UK_ for a European region
/// (0x570A50); the European release's region 0x7FA73C is 6, so UK_,
/// a US install's ([`crate::region`]) NA_ENGLISH.TXT). The Event Handler's script sends 0xE6E946B8
/// at 320 ms; each 0xE6E946B8 clones CreditsArea (0xE6093486) with the
/// next line (0x511360; after the last, from the first again, 0x511340)
/// and runs RollCredit (0x587E5F53, 5200 ms) on it, whose own 0xE6E946B8
/// at 300 ms brings the next line and 0x29161540 at 5200 ms removes it.
/// Back goes to UI_OptionsMain (0x78FA3C).
pub struct Credits {
    pub lines: Vec<String>,
    pub opened: f32,
}

/// The English credits file in `region` (0x4B8808).
pub fn credits_file(region: u32) -> &'static str {
    if region == 0 { "CREDITS/NA_ENGLISH.TXT" } else { "CREDITS/UK_ENGLISH.TXT" }
}

/// The credits file of a text language (0x4B8808, `CREDITS\%s%s.TXT`: the
/// region's prefix and the upper-case language name; the prefix is for
/// English only, docs/modding/asset-format/text.md §3.4).
pub fn credits_file_for(region: u32, language: &str) -> String {
    if language.eq_ignore_ascii_case(crate::game_assets::DEFAULT_LANGUAGE) {
        credits_file(region).to_string()
    } else {
        format!("CREDITS/{}.TXT", language.to_ascii_uppercase())
    }
}

pub const AREA: u32 = fe_ids::AREA;
pub const ROLL: u32 = fe_ids::ROLL;
pub const FIRST: f32 = 320.0;
pub const EVERY: f32 = 300.0;
pub const ROLL_LENGTH: f32 = 5200.0;

impl Credits {
    pub fn load(root: &std::path::Path, opened: f32) -> Credits {
        let file = credits_file_for(crate::region::get(), crate::game_assets::text_language());
        let bytes = crate::game_fs::read(root, &file).unwrap_or_default();
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let text = String::from_utf16_lossy(&units);
        let lines =
            text.trim_start_matches('\u{FEFF}').split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        Credits { lines, opened }
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        use credits::*;
        let Some(package) = assets.package("Credits") else { return Vec::new() };
        let t = now - self.opened;
        // The events' times are the package's (`FE_RATE`): a line every
        // 300 ms of package time, 250 ms real.
        let ft = t * crate::frontend::FE_RATE;
        let mut state = ScreenState::default();
        footer(&mut state, package, "Back");
        state.hidden.insert(AREA);
        state.hidden.insert(fe_ids::PLAYER_NAME);
        state.hidden.insert(fe_ids::UI_MAIN_CASH);
        let mut quads = layout(assets, package, &state, t);
        if self.lines.is_empty() || ft < FIRST {
            return quads;
        }
        // Only the clone of CreditsArea: every other object hidden.
        let mut line = ScreenState::default();
        line.hidden = package.objects.iter().map(|o| o.hash).filter(|&h| h != AREA).collect();
        let last = ((ft - FIRST) / EVERY) as usize;
        let first = ((ft - FIRST - ROLL_LENGTH) / EVERY).ceil().max(0.0) as usize;
        for k in first..=last {
            let start = (FIRST + EVERY * k as f32) / crate::frontend::FE_RATE;
            line.text.insert(AREA, self.lines[k % self.lines.len()].clone());
            line.scripts.insert(AREA, (ROLL, start));
            quads.extend(layout(assets, package, &line, t));
        }
        quads
    }
}

#[cfg(test)]
mod region_tests {
    #[test]
    fn credits_file_by_language() {
        assert_eq!(super::credits_file_for(crate::region::EUROPE, "English"), "CREDITS/UK_ENGLISH.TXT");
        assert_eq!(super::credits_file_for(crate::region::US, "english"), "CREDITS/NA_ENGLISH.TXT");
        // No region prefix in the other languages.
        assert_eq!(super::credits_file_for(crate::region::US, "French"), "CREDITS/FRENCH.TXT");
        assert_eq!(super::credits_file_for(crate::region::EUROPE, "German"), "CREDITS/GERMAN.TXT");
    }

    /// 0x4B8808's English prefix: NA_ for region 0, UK_ for a European one.
    #[test]
    fn credits_file_by_region() {
        assert_eq!(super::credits_file(crate::region::US), "CREDITS/NA_ENGLISH.TXT");
        assert_eq!(super::credits_file(crate::region::EUROPE), "CREDITS/UK_ENGLISH.TXT");
    }
}
