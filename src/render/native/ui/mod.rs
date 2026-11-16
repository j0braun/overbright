//! Draws front-end quads ([`crate::frontend::Quad`]) over whatever is in
//! the target: blended by each texture's entry byte (alpha, additive,
//! destination alpha), in the order given, the 640×480 canvas
//! stretched over the whole target, x and y scaled apart (**verified**:
//! the game maps the cursor to the canvas as (x − left) · 640 / client
//! width and (y − top) · 480 / client height, 0x5BF760, and a projected
//! screen point to it as x / width · 640, y / height · 480, 0x4EC34C; so
//! on a wide window the menus and HUD stretch, as in the game). That is
//! `display::AspectMode::Stretched`; the clone's default (agreed
//! deviation) keeps every element at its 4:3 size, centred or moved to
//! its screen edge by its `display::Place`, see [`UiRenderer::record`].

use crate::frontend::{Assets, CANVAS, Quad};
use crate::render::{Gpu, bind_texture, make_texture_layout_plain, upload_rgba, upload_texture};

mod batch;
#[cfg(feature = "dev-harness")]
pub mod geom_dump;
mod layout;
mod renderer;
pub use layout::*;
pub use renderer::*;

#[cfg(test)]
mod palette_tests {
    #[test]
    fn hud_palette_range() {
        super::set_hud_palette(2, 1..5);
        assert_eq!((0..7).map(super::hud_palette_of).collect::<Vec<_>>(), [0, 2, 2, 2, 2, 0, 0]);
        super::set_hud_palette(0, 1..5);
        assert_eq!(super::hud_palette_of(2), 0);
    }
}
