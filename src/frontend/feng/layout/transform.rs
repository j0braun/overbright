//! Object transforms and placement records for [`super::layout`].

use glam::{Mat4, Quat, Vec3};

use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::package_run::PackageRun;
use crate::frontend::feng::script::object_attributes;
use crate::frontend::feng::state::ScreenState;

/// An object's own transform (0x51B0B0): its size scales the unit
/// (an image is the quad ±0.5, a string is in font pixels), a rotation
/// turns it about the pivot, then it moves to its position. The game
/// multiplies row vectors, v · S · T(−pivot) · R · T(pivot) · T(position);
/// the same here for column vectors. The pivot only matters with a
/// rotation (the game skips both when the quaternion is (0, 0, 0, 1)).
pub(crate) fn local_matrix(size: Vec3, pivot: Vec3, rotation: Quat, position: Vec3) -> Mat4 {
    let mut m = Mat4::from_translation(position);
    if rotation != Quat::IDENTITY {
        m = m * Mat4::from_translation(pivot) * Mat4::from_quat(rotation.normalize()) * Mat4::from_translation(-pivot);
    }
    m * Mat4::from_scale(size)
}

/// Whether the package or the screen's code hides `o`: the runtime
/// hidden bit 0 (set by 0x50CA00 down a group, cleared by 0x50CA50; the
/// renderer skips such objects, 0x51C670) and the screen's own hides.
/// 0x10000000 is not a hide bit (the button list's flag, verified): an
/// object with it draws unless its screen hid it.
///
/// Every package's init (0x54E4B0, "Init Package %s") runs the visitor
/// 0x51C870 (vtable 0x79AB94) over its objects: one flagged 0x40 gets the
/// hidden bit through 0x50CA00 (itself and, for a group, everything
/// under it; flags |= 0x400001) and loses 0x10000000 (**verified**,
/// 2026-10-29). On PC that drops the console footers — the button glyphs,
/// "Select", and the screens' own player name / cash group — which
/// UI_PC_Help_Bar stands in for. A screen's code can show one again
/// (`state.shown`).
pub(crate) fn hidden(o: &feng::Object, state: &ScreenState) -> bool {
    state.hidden.contains(&o.hash) || (o.flags & 0x41 != 0 && !state.shown.contains(&o.hash))
}

/// [`layout`]'s attributes, each worked out on first use.
pub(crate) struct Attrs<'a> {
    pub(crate) package: &'a Package,
    pub(crate) state: &'a ScreenState,
    pub(crate) run: &'a PackageRun,
    pub(crate) now: f32,
    pub(crate) cache: std::cell::RefCell<Vec<Option<feng::Attributes>>>,
}

impl Attrs<'_> {
    pub(crate) fn get(&self, i: usize) -> feng::Attributes {
        if let Some(a) = self.cache.borrow()[i] {
            return a;
        }
        let a = object_attributes(self.package, self.state, self.run, i, self.now);
        self.cache.borrow_mut()[i] = Some(a);
        a
    }
}

/// One object's place in the tree: its world transform, colour (0..255
/// integers), whether it or a group above it is hidden, and its draw
/// bucket.
#[derive(Clone, Copy)]
pub(crate) struct Placed {
    pub(crate) world: Mat4,
    pub(crate) color: [i32; 4],
    pub(crate) hidden: bool,
    pub(crate) z: f32,
    /// The matrix 0x52C5E0 hands a group's children for their queue
    /// keys: like `world` but without any size (see `key_matrix`).
    pub(crate) key: Mat4,
    /// 0x51A970's queue key: the pivot plus position through the
    /// parent's `key`, z.
    pub(crate) queue_z: f32,
    /// The top-level object it hangs from (itself at the top): the
    /// element whose [`Place`] it shares.
    pub(crate) root: usize,
}

/// The matrix 0x52C5E0 builds for a group's children's queue keys
/// (0x52C649 → 0x504A50 turns the quaternion into a matrix, 0x52C68E →
/// 0x504B50 puts −pivot through it, the translation row is that plus
/// pivot plus position, then · the parent's, 0x5187A0): the rotation
/// about the pivot and the move, but **no size** — unlike the draw
/// transform (`local_matrix`), so a group with size z 0 does not flatten
/// its children's keys to 0.
pub(crate) fn key_matrix(pivot: Vec3, rotation: Quat, position: Vec3) -> Mat4 {
    Mat4::from_translation(position + pivot) * Mat4::from_quat(rotation.normalize()) * Mat4::from_translation(-pivot)
}
