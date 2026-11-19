//! The launcher's look as colour tokens ("Slab & Rail"), shared with the
//! programs that wear the same look: the launcher (`launcher::draw`), the
//! map editor and the game's free-camera panel.

/// A straight (not premultiplied) RGB colour, opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// This colour with `alpha` (0..255).
    pub const fn a(self, alpha: u8) -> Rgba {
        Rgba(self.0, self.1, self.2, alpha)
    }
    /// Opaque.
    pub const fn rgba(self) -> Rgba {
        Rgba(self.0, self.1, self.2, 255)
    }
}

/// A straight (not premultiplied) RGB colour with alpha (0 = clear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn rgb(self) -> Rgb {
        Rgb(self.0, self.1, self.2)
    }
    /// The same colour with its alpha scaled by `k` (0..1).
    pub fn fade(self, k: f32) -> Rgba {
        Rgba(self.0, self.1, self.2, (f32::from(self.3) * k.clamp(0.0, 1.0)).round() as u8)
    }
    /// Composited over the opaque `under`.
    pub fn over(self, under: Rgb) -> Rgb {
        let a = f32::from(self.3) / 255.0;
        let m = |s: u8, d: u8| (f32::from(s) * a + f32::from(d) * (1.0 - a)).round() as u8;
        Rgb(m(self.0, under.0), m(self.1, under.1), m(self.2, under.2))
    }
}

impl From<Rgb> for Rgba {
    fn from(c: Rgb) -> Rgba {
        c.rgba()
    }
}

/// The look's colours by role ("Slab & Rail": a smoked slab on a near-black
/// page, lime for titles and focus, a filled light pill for the main
/// action). Widgets and layout read their colours from here
/// ([`THEME_A`], [`theme`]). Contrast (WCAG 2, the slab composited over the darkest
/// page): `ink` 15.8:1, `dim` 11.1:1, ink in the selection bar 15.2:1,
/// the label on `play` 18.1:1, the focus ring against the page 15.5:1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The page under everything.
    pub page: Rgb,
    /// The backdrop's vertical gradient, top and bottom.
    pub page_top: Rgb,
    pub page_bottom: Rgb,
    /// The smoked slab behind the content, and its top edge's light.
    pub slab: Rgba,
    pub slab_edge: Rgba,
    /// A panel on the slab (the description panel, a log's tail).
    pub panel: Rgba,
    /// Body text.
    pub ink: Rgb,
    /// Secondary text (hints, paths, sub-lines).
    pub dim: Rgb,
    /// Titles, the current page's name, focus.
    pub lime: Rgb,
    /// The selection bar's fill (the focused row).
    pub sel: Rgb,
    pub ok: Rgb,
    pub err: Rgb,
    pub warn: Rgb,
    pub info: Rgb,
    /// The glyph on a filled ok / error / warning mark.
    pub on_mark: Rgb,
    pub on_err: Rgb,
    pub on_warn: Rgb,
    /// The rail under the page names and its dot.
    pub rail: Rgb,
    pub dot: Rgb,
    /// The main action's fill (Play and other primary buttons) and its
    /// label; `play_busy` while starting, `play_hover` / `play_pressed`.
    pub play: Rgb,
    pub play_hover: Rgb,
    pub play_pressed: Rgb,
    pub play_busy: Rgb,
    pub on_play: Rgb,
    /// A secondary button's fill; the outline of buttons, pills and key
    /// glyphs.
    pub button_fill: Rgba,
    pub outline: Rgb,
    /// Shades over a control under the pointer / held down.
    pub hover: Rgba,
    pub pressed: Rgba,
    /// A text field's well and outline.
    pub field: Rgba,
    pub field_edge: Rgba,
    /// Lines between summary rows.
    pub row_rule: Rgba,
    /// A message box's fill (its left edge takes the mark's colour).
    pub message: Rgba,
    /// A toggle: off track and outline; on track (its outline and knob
    /// take `lime`).
    pub toggle_off: Rgb,
    pub toggle_off_edge: Rgb,
    pub toggle_on: Rgb,
    /// A progress bar's track (its fill is `lime`).
    pub bar_track: Rgba,
    /// Over the page behind a dialog; the dialog's fill and edge.
    pub scrim: Rgba,
    pub dialog: Rgb,
    pub dialog_edge: Rgba,
    /// An outlined tag ("suggested").
    pub tag_edge: Rgba,
    /// A control that cannot be used: its words and outline.
    pub disabled: Rgb,
    /// An empty text field's hint.
    pub placeholder: Rgb,
    /// The backdrop's light streak (pink) and its hot end.
    pub streak: Rgb,
    pub streak_end: Rgb,
}

/// Direction A's tokens.
pub const THEME_A: Theme = Theme {
    page: Rgb(0x05, 0x06, 0x05),
    page_top: Rgb(0x08, 0x0a, 0x08),
    page_bottom: Rgb(0x04, 0x05, 0x04),
    slab: Rgba(168, 176, 160, 22),
    slab_edge: Rgba(255, 255, 255, 23),
    panel: Rgba(0, 0, 0, 102),
    ink: Rgb(0xee, 0xf0, 0xe8),
    dim: Rgb(0xc8, 0xcc, 0xc0),
    lime: Rgb(0xc6, 0xf0, 0x87),
    sel: Rgb(0x0a, 0x0b, 0x09),
    ok: Rgb(0xc6, 0xf0, 0x87),
    err: Rgb(0xff, 0x7a, 0x6b),
    warn: Rgb(0xff, 0xcb, 0x5c),
    info: Rgb(0xc8, 0xcc, 0xc0),
    on_mark: Rgb(0x07, 0x10, 0x06),
    on_err: Rgb(0x1a, 0x04, 0x02),
    on_warn: Rgb(0x1a, 0x12, 0x02),
    rail: Rgb(0x6c, 0x6f, 0x69),
    dot: Rgb(0xff, 0xff, 0xff),
    play: Rgb(0xf2, 0xf4, 0xec),
    play_hover: Rgb(0xff, 0xff, 0xfa),
    play_pressed: Rgb(0xd6, 0xd9, 0xcf),
    play_busy: Rgb(0xc9, 0xcc, 0xc2),
    on_play: Rgb(0x07, 0x08, 0x06),
    button_fill: Rgba(0, 0, 0, 140),
    outline: Rgb(0xe3, 0xe6, 0xdc),
    hover: Rgba(255, 255, 255, 18),
    pressed: Rgba(255, 255, 255, 34),
    field: Rgba(0, 0, 0, 115),
    field_edge: Rgba(255, 255, 255, 36),
    row_rule: Rgba(255, 255, 255, 15),
    message: Rgba(0, 0, 0, 107),
    toggle_off: Rgb(0x2a, 0x2d, 0x27),
    toggle_off_edge: Rgb(0x8a, 0x90, 0x83),
    toggle_on: Rgb(0x1d, 0x2a, 0x12),
    bar_track: Rgba(255, 255, 255, 31),
    scrim: Rgba(2, 3, 2, 184),
    dialog: Rgb(0x1b, 0x1d, 0x19),
    dialog_edge: Rgba(198, 240, 135, 140),
    tag_edge: Rgba(198, 240, 135, 140),
    disabled: Rgb(0x80, 0x84, 0x7c),
    placeholder: Rgb(0x96, 0x9a, 0x90),
    streak: Rgb(0xff, 0x5c, 0x82),
    streak_end: Rgb(0xff, 0xd6, 0xe0),
};
