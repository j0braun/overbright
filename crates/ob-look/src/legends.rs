//! The button legends' data: a button's role on a screen, the glyph sets
//! (Xbox-style pads, the Steam Deck, PlayStation-style pads, keyboard keys)
//! and the glyphs each set shows for a role, picked by the input the player
//! used last. The launcher draws them (its `legends`); Studio's screens show
//! the same keys and buttons.

/// The device the player used last (the legends' glyph set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum InputKind {
    Mouse,
    #[default]
    Keyboard,
    Xbox,
    PlayStation,
    Deck,
}

impl InputKind {
    /// A pad of any kind.
    pub fn is_pad(self) -> bool {
        matches!(self, InputKind::Xbox | InputKind::PlayStation | InputKind::Deck)
    }

    /// Its number (a stored last input).
    pub fn to_u8(self) -> u8 {
        match self {
            InputKind::Mouse => 0,
            InputKind::Keyboard => 1,
            InputKind::Xbox => 2,
            InputKind::PlayStation => 3,
            InputKind::Deck => 4,
        }
    }

    /// The kind numbered `v` (anything else: the mouse).
    pub fn from_u8(v: u8) -> InputKind {
        match v {
            1 => InputKind::Keyboard,
            2 => InputKind::Xbox,
            3 => InputKind::PlayStation,
            4 => InputKind::Deck,
            _ => InputKind::Mouse,
        }
    }
}

/// Which way a circled arrow or chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// A button's role (the glyph follows the input kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    /// A / Enter.
    Confirm,
    /// B / Esc.
    Back,
    /// X / Space.
    X,
    /// Y / R.
    Y,
    /// LB RB / Q E.
    Pages,
    /// D-pad / arrows.
    Move,
    /// ◀ ▶ / ← →.
    Change,
    /// LT RT / F6: the next or previous area of a window with several (Studio).
    Area,
    /// The Menu / Options button / F5: start the main action (Studio: test).
    Start,
    /// View / Create / Alt+←: back to the last place.
    LastPlace,
    /// The sticks / right mouse button and WASD: move and look in a 3D view.
    Sticks,
    /// The Deck's back grips L4 R4 / Ctrl+Z Ctrl+Y: undo and redo.
    DeckGrips,
}

impl Btn {
    /// Every role, in the order above.
    pub const ALL: [Btn; 12] = [
        Btn::Confirm,
        Btn::Back,
        Btn::X,
        Btn::Y,
        Btn::Pages,
        Btn::Move,
        Btn::Change,
        Btn::Area,
        Btn::Start,
        Btn::LastPlace,
        Btn::Sticks,
        Btn::DeckGrips,
    ];
}

/// Which program's keys a legend shows for the keyboard (the pad's buttons are the same in every program).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum KeySet {
    /// The launcher: single letters (Q E pages, Space, R).
    #[default]
    Launcher,
    /// Studio and the map editor, where letters are tools and typing: Ctrl+PgUp/PgDn, Shift+F10, Ctrl+K.
    Studio,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub btn: Btn,
    pub label: String,
}

pub fn action(btn: Btn, label: &str) -> Action {
    Action { btn, label: label.into() }
}

/// The glyph family for an input kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlyphSet {
    Xbox,
    Deck,
    PlayStation,
    Keys,
}

impl GlyphSet {
    pub fn for_input(kind: InputKind) -> GlyphSet {
        match kind {
            InputKind::Xbox => GlyphSet::Xbox,
            InputKind::Deck => GlyphSet::Deck,
            InputKind::PlayStation => GlyphSet::PlayStation,
            InputKind::Keyboard | InputKind::Mouse => GlyphSet::Keys,
        }
    }

    pub fn is_pad(self) -> bool {
        self != GlyphSet::Keys
    }
}

/// The four PlayStation-style face symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symbol {
    Cross,
    Circle,
    Square,
    Triangle,
}

/// One drawn glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A filled disc with a letter (A, B, X, Y).
    Face(&'static str),
    /// A filled disc with a plain shape.
    Symbol(Symbol),
    /// A filled disc with a triangle (the D-pad's left / right).
    PadArrow(Dir),
    /// A filled disc with a plus (the D-pad).
    Dpad,
    /// The View button: a filled disc with two overlapping frames.
    View,
    /// A PlayStation-style Create button: a disc with a small capsule and
    /// three short strokes over it.
    Create,
    /// An outlined shoulder button with its name (LB, R1).
    Bumper(&'static str),
    /// An outlined keycap with its name (Enter, Esc, Q).
    Key(&'static str),
    /// An outlined keycap with an arrow.
    KeyArrow(Dir),
    /// An outlined trigger with its name (LT, R2), drawn as a shoulder button.
    Trigger(&'static str),
    /// The Menu / Options button: a filled disc with three short lines.
    Start,
    /// An outlined stick with its name (LS, RS), drawn as a shoulder button.
    Stick(&'static str),
}

/// The glyphs drawn for `btn` in `set` with the launcher's keys, left to right.
pub fn glyphs(set: GlyphSet, btn: Btn) -> &'static [Glyph] {
    glyphs_in(set, KeySet::Launcher, btn)
}

/// The glyphs drawn for `btn` in `set` with `keys`' keyboard keys, left to right. Empty: the device has no such
/// button (only the Deck has back grips).
pub fn glyphs_in(set: GlyphSet, keys: KeySet, btn: Btn) -> &'static [Glyph] {
    use Btn as B;
    use Glyph as G;
    // The roles the launcher does not use, and Studio's keys for the shared ones.
    let studio_key = |btn: Btn| -> Option<&'static [Glyph]> {
        Some(match btn {
            B::X => &[G::Key("Shift+F10")],
            B::Y => &[G::Key("Ctrl+K")],
            B::Pages => &[G::Key("Ctrl+PgUp"), G::Key("Ctrl+PgDn")],
            _ => return None,
        })
    };
    let pad_extra = |btn: Btn, set: GlyphSet| -> Option<&'static [Glyph]> {
        Some(match (btn, set) {
            (B::Area, GlyphSet::PlayStation | GlyphSet::Deck) => &[G::Trigger("L2"), G::Trigger("R2")],
            (B::Area, _) => &[G::Trigger("LT"), G::Trigger("RT")],
            (B::Start, _) => &[G::Start],
            (B::LastPlace, GlyphSet::PlayStation) => &[G::Create],
            (B::LastPlace, _) => &[G::View],
            (B::Sticks, _) => &[G::Stick("LS"), G::Stick("RS")],
            (B::DeckGrips, GlyphSet::Deck) => &[G::Bumper("L4"), G::Bumper("R4")],
            (B::DeckGrips, _) => &[],
            _ => return None,
        })
    };
    match set {
        GlyphSet::Keys => match btn {
            B::Area => &[G::Key("F6")],
            B::Start => &[G::Key("F5")],
            B::LastPlace => &[G::Key("Alt+←")],
            B::Sticks => &[G::Key("RMB"), G::Key("WASD")],
            B::DeckGrips => &[G::Key("Ctrl+Z"), G::Key("Ctrl+Y")],
            _ if keys == KeySet::Studio && studio_key(btn).is_some() => studio_key(btn).unwrap_or(&[]),
            B::Confirm => &[G::Key("Enter")],
            B::Back => &[G::Key("Esc")],
            B::X => &[G::Key("Space")],
            B::Y => &[G::Key("R")],
            B::Pages => &[G::Key("Q"), G::Key("E")],
            B::Move => &[G::KeyArrow(Dir::Up), G::KeyArrow(Dir::Down)],
            B::Change => &[G::KeyArrow(Dir::Left), G::KeyArrow(Dir::Right)],
        },
        _ if pad_extra(btn, set).is_some() => pad_extra(btn, set).unwrap_or(&[]),
        GlyphSet::PlayStation => match btn {
            B::Confirm => &[G::Symbol(Symbol::Cross)],
            B::Back => &[G::Symbol(Symbol::Circle)],
            B::X => &[G::Symbol(Symbol::Square)],
            B::Y => &[G::Symbol(Symbol::Triangle)],
            B::Pages => &[G::Bumper("L1"), G::Bumper("R1")],
            B::Move => &[G::Dpad],
            B::Change => &[G::PadArrow(Dir::Left), G::PadArrow(Dir::Right)],
            _ => &[],
        },
        GlyphSet::Xbox | GlyphSet::Deck => match btn {
            B::Confirm => &[G::Face("A")],
            B::Back => &[G::Face("B")],
            B::X => &[G::Face("X")],
            B::Y => &[G::Face("Y")],
            B::Pages if set == GlyphSet::Deck => &[G::Bumper("L1"), G::Bumper("R1")],
            B::Pages => &[G::Bumper("LB"), G::Bumper("RB")],
            B::Move => &[G::Dpad],
            B::Change => &[G::PadArrow(Dir::Left), G::PadArrow(Dir::Right)],
            _ => &[],
        },
    }
}

/// The glyph of the button held while Overbright starts to come back to
/// the launcher (Settings' "To come back here, hold …" note).
pub fn hold_glyph(set: GlyphSet) -> Glyph {
    match set {
        GlyphSet::Keys => Glyph::Key("Shift"),
        GlyphSet::PlayStation => Glyph::Create,
        GlyphSet::Xbox | GlyphSet::Deck => Glyph::View,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETS: [GlyphSet; 4] = [GlyphSet::Xbox, GlyphSet::Deck, GlyphSet::PlayStation, GlyphSet::Keys];

    #[test]
    fn launcher_keys_are_unchanged() {
        assert_eq!(glyphs(GlyphSet::Keys, Btn::Pages), [Glyph::Key("Q"), Glyph::Key("E")]);
        assert_eq!(glyphs(GlyphSet::Keys, Btn::X), [Glyph::Key("Space")]);
        assert_eq!(glyphs(GlyphSet::Keys, Btn::Y), [Glyph::Key("R")]);
    }

    #[test]
    fn studio_keys_avoid_letters() {
        assert_eq!(
            glyphs_in(GlyphSet::Keys, KeySet::Studio, Btn::Pages),
            [Glyph::Key("Ctrl+PgUp"), Glyph::Key("Ctrl+PgDn")]
        );
        assert_eq!(glyphs_in(GlyphSet::Keys, KeySet::Studio, Btn::X), [Glyph::Key("Shift+F10")]);
        assert_eq!(glyphs_in(GlyphSet::Keys, KeySet::Studio, Btn::Y), [Glyph::Key("Ctrl+K")]);
        for b in Btn::ALL {
            for g in glyphs_in(GlyphSet::Keys, KeySet::Studio, b) {
                if let Glyph::Key(k) = g {
                    assert!(k.len() > 1 || !k.chars().all(|c| c.is_ascii_alphabetic()), "{b:?}: a single letter {k}");
                }
            }
        }
    }

    #[test]
    fn pad_buttons_are_the_same_in_every_program() {
        for set in SETS.into_iter().filter(|s| s.is_pad()) {
            for b in Btn::ALL {
                assert_eq!(glyphs_in(set, KeySet::Launcher, b), glyphs_in(set, KeySet::Studio, b), "{set:?} {b:?}");
            }
        }
    }

    #[test]
    fn every_role_has_glyphs_except_grips_off_the_deck() {
        for set in SETS {
            for b in Btn::ALL {
                let g = glyphs_in(set, KeySet::Studio, b);
                let none_ok = b == Btn::DeckGrips && matches!(set, GlyphSet::Xbox | GlyphSet::PlayStation);
                assert_eq!(g.is_empty(), none_ok, "{set:?} {b:?}");
            }
        }
        assert_eq!(glyphs_in(GlyphSet::Xbox, KeySet::Studio, Btn::Area), [Glyph::Trigger("LT"), Glyph::Trigger("RT")]);
        assert_eq!(
            glyphs_in(GlyphSet::Deck, KeySet::Studio, Btn::DeckGrips),
            [Glyph::Bumper("L4"), Glyph::Bumper("R4")]
        );
    }
}
