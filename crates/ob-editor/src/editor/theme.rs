//! The editor's look: the launcher's look-A tokens
//! ([`crate::launch::theme::THEME_A`]) mapped onto egui, and its type,
//! Sofia Sans Semi Condensed (SIL OFL 1.1, src/launcher/fonts/OFL.txt):
//! upright faces for reading text, italic ones for titles and buttons. Lime
//! marks titles, focus, selection and the main action; errors and warnings
//! keep their red and amber.

use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, FontFamily};
use egui::{Color32, CornerRadius, FontId, Shadow, Stroke, TextStyle, Visuals};

use crate::launch::theme::{Rgb, Rgba, THEME_A, Theme};

macro_rules! face {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/launcher/fonts/SofiaSansSemiCondensed-",
            $name,
            ".ttf"
        ))
    };
}

/// The upright 600 face (values, status).
pub const SEMI: &str = "semi";
/// The italic 600 face (buttons, labels, tabs).
pub const LABEL: &str = "label";
/// The italic 700 face (titles).
pub const TITLE: &str = "title";
/// The italic 800 face (the main action).
pub const PLAY: &str = "play";

/// The theme's tokens.
pub fn tokens() -> &'static Theme {
    &THEME_A
}

pub fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A token with alpha, composited over the page (egui's fills are opaque
/// where the launcher's slab lets the page through).
pub fn over_page(c: Rgba) -> Color32 {
    rgb(c.over(tokens().page))
}

/// Over another opaque colour.
pub fn over(c: Rgba, under: Rgb) -> Color32 {
    rgb(c.over(under))
}

pub fn lime() -> Color32 {
    rgb(tokens().lime)
}
pub fn ink() -> Color32 {
    rgb(tokens().ink)
}
pub fn dim() -> Color32 {
    rgb(tokens().dim)
}
pub fn error() -> Color32 {
    rgb(tokens().err)
}
pub fn warn() -> Color32 {
    rgb(tokens().warn)
}

/// The panels' fill: the smoked slab over the page.
pub fn panel() -> Color32 {
    over_page(tokens().slab)
}

/// The fonts: the launcher's faces only (egui's own default fonts are not
/// built in: their licences are not all GPL-compatible). Monospace text
/// takes the upright face too. The faces are subset to Latin: the editor's
/// words use no other characters ([`USED_CHARS`]).
pub fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::empty();
    for (name, bytes) in [
        ("body", &face!("Body")[..]),
        (SEMI, &face!("BodySemi")[..]),
        (LABEL, &face!("LabelItalic")[..]),
        (TITLE, &face!("TitleItalic")[..]),
        (PLAY, &face!("PlayItalic")[..]),
    ] {
        f.font_data.insert(name.to_string(), Arc::new(FontData::from_static(bytes)));
        // The upright face behind each, for any glyph an italic cut lacks.
        let list = if name == "body" { vec![name.to_string()] } else { vec![name.to_string(), "body".to_string()] };
        f.families.insert(FontFamily::Name(name.into()), list);
    }
    f.families.insert(FontFamily::Proportional, vec!["body".into()]);
    f.families.insert(FontFamily::Monospace, vec!["body".into()]);
    f
}

/// The characters beyond ASCII the editor's own words use; the faces must
/// have them (a test checks).
pub const USED_CHARS: &str = "…—·°";

/// A font of the named face (`SEMI`, `LABEL`, `TITLE`, `PLAY`).
pub fn font(face: &str, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(face.into()))
}

/// The look as egui visuals.
pub fn visuals() -> Visuals {
    let t = tokens();
    let mut v = Visuals::dark();
    let page = rgb(t.page);
    let panel = panel();
    v.dark_mode = true;
    v.override_text_color = None;
    v.weak_text_color = Some(rgb(t.dim));
    v.hyperlink_color = rgb(t.lime);
    v.faint_bg_color = over(t.row_rule, t.slab.over(t.page));
    v.extreme_bg_color = over(t.field, t.slab.over(t.page));
    v.text_edit_bg_color = Some(over(t.field, t.slab.over(t.page)));
    v.code_bg_color = page;
    v.warn_fg_color = rgb(t.warn);
    v.error_fg_color = rgb(t.err);
    v.panel_fill = panel;
    v.window_fill = rgb(t.dialog);
    v.window_stroke = Stroke::new(1.0, over(t.dialog_edge, t.dialog));
    v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(160) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(140) };
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(4);
    v.selection.bg_fill = over(Rgba(t.lime.0, t.lime.1, t.lime.2, 64), t.slab.over(t.page));
    v.selection.stroke = Stroke::new(1.0, rgb(t.lime));

    let base = t.slab.over(t.page);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = panel;
    w.noninteractive.weak_bg_fill = panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, over(t.row_rule, base));
    w.noninteractive.fg_stroke = Stroke::new(1.0, rgb(t.ink));
    w.inactive.bg_fill = over(t.button_fill, base);
    w.inactive.weak_bg_fill = over(t.button_fill, base);
    w.inactive.bg_stroke = Stroke::new(1.0, over(t.field_edge, base));
    w.inactive.fg_stroke = Stroke::new(1.0, rgb(t.ink));
    w.hovered.bg_fill = over(t.hover, t.button_fill.over(base));
    w.hovered.weak_bg_fill = over(t.hover, t.button_fill.over(base));
    w.hovered.bg_stroke = Stroke::new(1.0, rgb(t.outline));
    w.hovered.fg_stroke = Stroke::new(1.5, rgb(t.ink));
    w.active.bg_fill = over(t.pressed, t.button_fill.over(base));
    w.active.weak_bg_fill = over(t.pressed, t.button_fill.over(base));
    w.active.bg_stroke = Stroke::new(1.5, rgb(t.lime));
    w.active.fg_stroke = Stroke::new(2.0, rgb(t.ink));
    w.open.bg_fill = over(t.pressed, base);
    w.open.weak_bg_fill = over(t.pressed, base);
    w.open.bg_stroke = Stroke::new(1.0, rgb(t.lime));
    w.open.fg_stroke = Stroke::new(1.0, rgb(t.ink));
    for wv in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        wv.corner_radius = CornerRadius::same(4);
    }
    v
}

/// Look A on a context: visuals, faces, text sizes.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(egui::Theme::Dark, visuals());
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, font(LABEL, 15.0)),
            (TextStyle::Heading, font(TITLE, 20.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 5.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
    });
}

/// The main action's button (Test race): the light pill with dark italic
/// words, as the launcher's Play.
pub fn main_action(text: &str) -> egui::Button<'static> {
    let t = tokens();
    egui::Button::new(egui::RichText::new(text.to_string()).font(font(PLAY, 15.0)).color(rgb(t.on_play)))
        .fill(rgb(t.play))
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(12))
}

/// A title in lime italics.
pub fn title(text: &str) -> egui::RichText {
    egui::RichText::new(text.to_string()).font(font(TITLE, 17.0)).color(lime())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_look_a() {
        assert_eq!(rgb(tokens().page), Color32::from_rgb(0x05, 0x06, 0x05));
        assert_eq!(lime(), Color32::from_rgb(0xc6, 0xf0, 0x87));
        assert_eq!(ink(), Color32::from_rgb(0xee, 0xf0, 0xe8));
        let v = visuals();
        assert_eq!(v.selection.stroke.color, lime());
        assert_eq!(v.error_fg_color, error());
    }

    #[test]
    fn the_faces_have_every_character_used() {
        use ab_glyph::{Font, FontRef};
        let f = fonts();
        for (name, data) in &f.font_data {
            let font = FontRef::try_from_slice((*data.font).as_ref()).unwrap();
            for c in USED_CHARS.chars().chain(' '..='~') {
                assert_ne!(font.glyph_id(c).0, 0, "{name} lacks {c:?}");
            }
        }
    }

    #[test]
    fn fonts_have_the_launcher_faces_first() {
        let f = fonts();
        assert_eq!(f.families[&FontFamily::Proportional], vec!["body".to_string()]);
        assert_eq!(f.font_data.len(), 5, "no egui default fonts");
        for n in [SEMI, LABEL, TITLE, PLAY] {
            assert_eq!(f.families[&FontFamily::Name(n.into())][0], n);
        }
    }
}
