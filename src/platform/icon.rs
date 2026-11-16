//! The game window's icon: the same drawing as the exe's icon
//! resource (build_support/icon_art.rs, compiled into both). Windows shows
//! it in the title bar, the taskbar and Alt+Tab; X11 window managers show
//! it too (`_NET_WM_ICON`); Wayland takes no icon from the client.

mod art {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/build_support/icon_art.rs"));
}

/// The icon's size in the window: 64×64 (Windows scales it to the small
/// and big icon sizes; the exe resource carries 16 to 256).
pub const SIZE: u32 = 64;

/// Straight RGBA, `n`×`n`, rows top to bottom.
pub fn rgba(n: u32) -> Vec<u8> {
    art::draw(n).into_iter().flatten().collect()
}

/// The window icon for winit (None only if winit refuses the buffer).
pub fn window_icon() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(rgba(SIZE), SIZE, SIZE).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_is_opaque_tile_with_clear_corners() {
        let n = super::SIZE;
        let px = super::rgba(n);
        assert_eq!(px.len(), (n * n * 4) as usize);
        // The rounded tile's corner is clear, its middle opaque and lit.
        assert_eq!(px[3], 0);
        let mid = (((n / 2) * n + n / 2) * 4) as usize;
        assert_eq!(px[mid + 3], 255);
        assert!(super::window_icon().is_some());
    }
}
