//! Widescreen placement of FEng elements (clone deviation; off at 4:3, see docs/formats.md
//! "Display modes and aspect").

use glam::Vec2;

use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::state::{CANVAS, Quad, ScreenState};
use crate::frontend::screens;
use crate::render::display::Place;

/// Where an element (a package's top-level object and all under it) goes
/// on a screen that is not 4:3 (clone deviation, [`Place`]; see
/// docs/formats.md "Display modes and aspect"), from its position in the
/// 640 × 480 layout (the package's, or the one the game's code gives it:
/// the minimap's markers), modelled on ThirteenAG's WidescreenFix (its
/// HudLayout table moves the same top-level HUD objects to the edges and
/// leaves the front end a centred 4:3 box):
/// * HUD packages (`HUD_*`, `2PHud*`): by thirds of the canvas — left of
///   x 213.3 to the left edge, right of 426.7 to the right, the middle
///   third centred; vertically an element at a side goes to the top or
///   bottom when more than 30 from the middle line, a middle one only past
///   the outer sixths (y < 80 or > 400), so timers and messages stay
///   centred. The minimap's pieces and markers (resources `Minimap_*`)
///   go bottom left with the map.
/// * `Chyron_IG` (EA Trax in game): top left; `Chyron_FE`: bottom left
///   (the fix moves EA Trax left too).
/// * `UI_PC_Help_Bar` at the screen's foot (where its scripts have it
///   now) with its backing strip (RealGroup) drawn: its buttons bottom
///   right, the profile name bottom left. Without the strip (a mask with
///   0x8000 or 0x10000: the World Maps, SMS, Career Status, pause,
///   results) the buttons belong to the screen's panel and stay with it.
/// * Every other front-end package stays a centred 4:3 box.
///
/// Side elements stop at the 16:9 safe area's edges on wider screens
/// (`display::SAFE_SPREAD`); HUD ones (`HUD_*`, `2PHud*`, `Chyron_IG`,
/// [`Place::hud`]) go to the screen's edges with HUD Layout Full Width.
pub(crate) fn element_place(package: &Package, o: &feng::Object, state: &ScreenState, now_at: Vec2) -> Place {
    let at = state.position.get(&o.hash).copied().unwrap_or_else(|| o.attributes.position().truncate());
    // The header's name, "UI_PC_Help_Bar.fng".
    let name = package.name.strip_suffix(".fng").unwrap_or(&package.name);
    if name.starts_with("HUD_") || name.starts_with("2PHud") {
        let minimap = package.resources.get(o.resource as usize).is_some_and(|r| r.name.starts_with("Minimap_"));
        if minimap {
            return Place::hud(-1, 1);
        }
        let third = CANVAS.x / 6.0;
        let x = if at.x < -third {
            -1
        } else if at.x > third {
            1
        } else {
            0
        };
        let cut = if x == 0 { CANVAS.y / 3.0 } else { 30.0 };
        let y = if at.y < -cut {
            -1
        } else if at.y > cut {
            1
        } else {
            0
        };
        return Place::hud(x, y);
    }
    match name {
        "Chyron_IG" => Place::hud(-1, -1),
        "Chyron_FE" => Place::new(-1, 1),
        // The screen-level bar only: at the canvas foot (where its scripts
        // have it now) with its backing strip RealGroup drawn. A mask with
        // 0x8000 or 0x10000 hides RealGroup (0x54E6F0) and the screen's
        // own panel frames the buttons (World Maps, SMS, Career Status,
        // pause, results; the layout scripts In_Game, Pause_*,
        // Event_Results put them in it): they stay centred with it.
        "UI_PC_Help_Bar" if now_at.y > CANVAS.y / 3.0 && !state.hidden.contains(&screens::REAL_GROUP) => {
            Place::new(if now_at.x >= -100.0 { 1 } else { -1 }, 1)
        }
        _ => Place::CENTRE,
    }
}

/// A front-end element moved to a side (the help bar's name / cash group
/// and its buttons, `Chyron_FE`; not the HUD, whose parts go with their
/// element) by the thirds of the canvas each quad covers (clone deviation,
/// see docs/formats.md "Display modes and aspect"): a quad wholly in the
/// opposite outer third goes to that edge instead, and an unturned one
/// reaching from the left third into the right third stretches, its ends
/// with their edges ([`Place::stretch`]). So the help bar's backing strip
/// (RealGroup's U2_Menu_Box pieces, canvas 19..617 under the name and the
/// buttons) keeps joining the name box at the left to the buttons at the
/// right. At 4:3 nothing moves.
pub(crate) fn side_strip(package: &Package, place: Place, q: &Quad) -> Place {
    let name = &package.name;
    if place.x == 0 || q.angle != 0.0 || name.starts_with("HUD_") || name.starts_with("2PHud") {
        return place;
    }
    let (lo, hi) = (q.rect[0].x.min(q.rect[1].x), q.rect[0].x.max(q.rect[1].x));
    let (left, right) = (CANVAS.x / 3.0, CANVAS.x * 2.0 / 3.0);
    if lo < left && hi > right {
        Place { x: 0, stretch: true, ..place }
    } else if place.x < 0 && lo >= right {
        Place { x: 1, ..place }
    } else if place.x > 0 && hi <= left {
        Place { x: -1, ..place }
    } else {
        place
    }
}
