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
}

/// The glyphs drawn for `btn` in `set`, left to right.
pub fn glyphs(set: GlyphSet, btn: Btn) -> &'static [Glyph] {
    use Btn as B;
    use Glyph as G;
    match set {
        GlyphSet::Keys => match btn {
            B::Confirm => &[G::Key("Enter")],
            B::Back => &[G::Key("Esc")],
            B::X => &[G::Key("Space")],
            B::Y => &[G::Key("R")],
            B::Pages => &[G::Key("Q"), G::Key("E")],
            B::Move => &[G::KeyArrow(Dir::Up), G::KeyArrow(Dir::Down)],
            B::Change => &[G::KeyArrow(Dir::Left), G::KeyArrow(Dir::Right)],
        },
        GlyphSet::PlayStation => match btn {
            B::Confirm => &[G::Symbol(Symbol::Cross)],
            B::Back => &[G::Symbol(Symbol::Circle)],
            B::X => &[G::Symbol(Symbol::Square)],
            B::Y => &[G::Symbol(Symbol::Triangle)],
            B::Pages => &[G::Bumper("L1"), G::Bumper("R1")],
            B::Move => &[G::Dpad],
            B::Change => &[G::PadArrow(Dir::Left), G::PadArrow(Dir::Right)],
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
