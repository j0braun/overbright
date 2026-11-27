//! The front end: menus drawn from the game's FEng packages (see
//! docs/formats.md, "Front end").
//!
//! [`Assets`] loads what the screens need: the packages (FRONTEND/FrontB,
//! GLOBAL/GlobalB, GLOBAL/GLOBALA), their textures, the fonts and the
//! language strings. [`layout`] turns a package's objects at a moment of
//! their scripts into screen quads on the game's 640×480 canvas.

pub mod career;
pub mod map_pack;
pub mod cheats;
pub mod format;
pub mod custom_hud;
pub mod dday;
pub mod enh;
pub(crate) mod fe_ids;
pub mod feng;
pub mod graphics;
pub mod graphics_table;
pub mod hud;
pub mod mouse;
pub mod options;
pub mod profiles;
pub mod quick_race;
pub mod screens;
pub mod settings_model;
pub mod settings;
pub mod settings_page;
pub mod sms;
pub mod status;
pub mod widgets;
pub use ob_assets::frontend::assets;
pub mod font {
    pub use super::feng::font::*;
}
pub mod strings {
    pub use super::feng::strings::*;
}

pub use crate::render::display::Place;
pub use assets::{Assets, asset_files, in_game_alias};
pub use feng::dim::{DIM_SCRIPT, EVENT_HANDLER, MENUFADE_SCRIPT, MENUFADEOUT_SCRIPT, UNDIM_SCRIPT, dim_alpha, dim_alpha_of, dim_script, script_alpha, with_dim, with_dim_scripts, with_dim_undim};
#[cfg(any(test, feature = "dev-harness"))]
pub use feng::layout::drawn_texts;
pub use feng::layout::layout;
pub use feng::package_run::{History, PackageRun, run_package};
pub use feng::script::{FE_RATE, animate, attributes, host_time};
pub use feng::state::{CANVAS, HAIRLINE_UV, Quad, ScreenState};
pub use feng::text::{line_count, line_count_wrapped, string_quads, text_quads, wrap, wrap_feng};
pub use format::{bprintf_fixed, bprintf_fixed_parts, grouped, grouped_fixed};

/// An FEng colour word, 0xAARRGGBB, as RGBA floats.
pub fn argb(c: u32) -> [f32; 4] {
    let f = |s: u32| ((c >> s) & 0xFF) as f32 / 255.0;
    [f(16), f(8), f(0), f(24)]
}

#[cfg(test)]
mod native_fe_tests;

#[cfg(test)]
mod native_fe_part2_tests;

#[cfg(test)]
mod line_count_tests {
    #[test]
    fn bprintf_rounds_half_up() {
        assert_eq!(super::bprintf_fixed(8.125, 2), "8.13");
        assert_eq!(super::bprintf_fixed(-4.0, 2), "-4.00");
        assert_eq!(super::bprintf_fixed(9.995, 2), "9.99"); // 9.99499… in binary
        assert_eq!(super::bprintf_fixed(0.5, 0), "1");
        assert_eq!(super::bprintf_fixed(99.96, 1), "100.0");
        assert_eq!(super::bprintf_fixed(-0.001, 2), "-0.00");
        assert_eq!(super::grouped_fixed(4325.0, 1), "4,325.0");
        assert_eq!(super::grouped_fixed(15300.0, 1), "15,300.0");
        assert_eq!(super::grouped_fixed(300.0, 1), "300.0");
        assert_eq!(super::grouped_fixed(-1234567.25, 1), "-1,234,567.3");
    }

    /// SetLanguage's number separators (0x43CB20): English '.' and ',',
    /// French ',' and ' ', German to Danish ',' and '.'; without a
    /// language chosen (every test) English's.
    #[test]
    fn number_separators_follow_the_language() {
        use super::format::{Separators, fixed2_in, fixed3_in, grouped_fixed_in, grouped_in};
        let en = Separators::of("English");
        assert_eq!(en, Separators::ENGLISH);
        assert_eq!(super::format::separators(), en, "no test sets a language");
        assert_eq!((grouped_in(-1234567, en), grouped_fixed_in(4325.0, 1, en)), ("-1,234,567".into(), "4,325.0".into()));
        assert_eq!((fixed2_in(1234.5, en), fixed3_in(1234.5, en)), ("1,234.50".into(), "1,234.500".into()));
        let fr = Separators::of("French");
        assert_eq!((fr.decimal, fr.thousands), (',', ' '));
        assert_eq!((grouped_in(-1234567, fr), grouped_fixed_in(4325.0, 1, fr)), ("-1 234 567".into(), "4 325,0".into()));
        assert_eq!((fixed2_in(1234.5, fr), fixed3_in(1234.5, fr)), ("1 234,50".into(), "1 234,500".into()));
        for l in ["German", "Italian", "Spanish", "Dutch", "Swedish", "Danish"] {
            let s = Separators::of(l);
            assert_eq!((s.decimal, s.thousands), (',', '.'), "{l}");
            assert_eq!(grouped_fixed_in(-1234567.25, 1, s), "-1.234.567,3", "{l}");
            assert_eq!(fixed2_in(0.5, s), "0,50", "{l}");
        }
        // Under 1,000 nothing is grouped; the names match without case.
        assert_eq!(grouped_in(999, fr), "999");
        assert_eq!(Separators::of("french"), fr);
        assert_eq!(Separators::of("Korean"), en);
    }

    use super::line_count;

    #[test]
    fn breaks_count_after_a_space() {
        assert_eq!(line_count("Winner!\nYour time: 10.12"), 1);
        assert_eq!(line_count("You finished second!\nYour time: 10.12"), 2);
        assert_eq!(line_count("You were last!\nYour time: 1:02.00"), 2);
        assert_eq!(line_count("a b\nc d\ne"), 3);
        assert_eq!(line_count("one line"), 1);
    }
}

#[cfg(test)]
mod history_tests {
    /// The inline history reads as the Vec it stands for, through and
    /// past its inline room.
    #[test]
    fn history_reads_as_a_vec() {
        let mut h = super::History::one((1.0, 7));
        let mut v = vec![(1.0, 7)];
        for k in 0..9u32 {
            assert_eq!(&h[..], &v[..]);
            h.push((k as f32 * 2.5, k));
            v.push((k as f32 * 2.5, k));
        }
        assert_eq!(&h[..], &v[..]);
        assert_eq!(h.first(), v.first());
        assert_eq!(h.iter().rev().find(|e| e.0 <= 10.0), v.iter().rev().find(|e| e.0 <= 10.0));
    }
}
