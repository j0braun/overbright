//! The parts columns' and track strips' icon scroller items.

use glam::Vec2;

use crate::frontend::Quad;
use crate::frontend::widgets::icon_menu::UNSELECTED_SCALE;

/// An IconScrollerMenu's items (0x52ED90 stacks them along the strip by
/// their template's size plus `gap`; 0x52EEB0 places them): the selected
/// one sits on the master at full size, the others at 0.8 of it (kept to
/// their leading edge, centred across), none outside the `extent` around
/// the master, and fading over 0.8 of an icon at the ends (0x50D8D0).
pub(crate) fn scroller_quads(
    package: &crate::frontend::feng::Package,
    master: u32,
    template: u32,
    icons: &[u32],
    selected: usize,
    across: bool,
    gap: f32,
    extent: f32,
) -> Vec<Quad> {
    let (Some(m), Some(t)) = (package.object(master), package.object(template)) else { return Vec::new() };
    let centre = Vec2::new(m.attributes.position().x, m.attributes.position().y) + crate::frontend::CANVAS * 0.5;
    let size = Vec2::new(t.attributes.size().x, t.attributes.size().y);
    let pitch = if across { size.x } else { size.y } + gap;
    let axis = if across { Vec2::X } else { Vec2::Y };
    let mut color = t.attributes.color();
    // A dialog's DIM over the screen fades the clones as their template.
    color[3] *= crate::frontend::dim_alpha(package, t);
    let uv = t.attributes.uv();
    let mut quads = Vec::new();
    if across {
        // The strip's box for the wheel (its extent, **inferred** as on
        // the icon menus).
        let name = package.name.strip_suffix(".fng").unwrap_or(&package.name);
        let half = Vec2::new(extent * 0.5, size.y * 0.5);
        crate::frontend::mouse::record_rect(
            name,
            crate::frontend::mouse::STRIP,
            centre - half,
            centre + half,
            Default::default(),
        );
    }
    for (i, &icon) in icons.iter().enumerate() {
        let along = (i as f32 - selected as f32) * pitch;
        if along.abs() > extent * 0.5 {
            continue;
        }
        let length = if across { size.x } else { size.y };
        let ramp = length * UNSELECTED_SCALE;
        let to_end = extent * 0.5 - along.abs();
        let alpha = (to_end / ramp).clamp(0.0, 1.0);
        let (half, c) = if i == selected {
            // The mouse: the current item's icon (the strip's, or the
            // parts column's).
            let name = package.name.strip_suffix(".fng").unwrap_or(&package.name);
            let id =
                if across { crate::frontend::mouse::STRIP_SELECTED } else { crate::frontend::mouse::COLUMN_SELECTED };
            crate::frontend::mouse::record_rect(
                name,
                id,
                centre + axis * along - size * 0.5,
                centre + axis * along + size * 0.5,
                Default::default(),
            );
            (size * 0.5, centre + axis * along)
        } else {
            // Shrunk about its own centre, as the icon menus' (Wine, the
            // finished-career save's Body Shop: the side icons 25 px right of an
            // edge-anchored shrink, the column's dots 3 px lower).
            let shrink = size * UNSELECTED_SCALE;
            (shrink * 0.5, centre + axis * along)
        };
        quads.push(Quad {
            texture: Some(icon),
            rect: [c - half, c + half],
            uv: [Vec2::new(uv[0], uv[1]), Vec2::new(uv[2], uv[3])],
            color: [color[0], color[1], color[2], color[3] * alpha],
            z: t.attributes.position().z,
            angle: 0.0,
            place: Default::default(),
        });
    }
    quads
}
