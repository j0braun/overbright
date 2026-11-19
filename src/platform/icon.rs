//! The windows' icon (game, launcher, map editor): the same drawing as the
//! exes' icon resource and the Linux `overbright.png` (build_support/icon_art.rs,
//! compiled into all of them): the launcher's lime ring on a dark tile. Windows shows
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
    use crate::launcher::draw::{Rgb, THEME_A};

    #[test]
    fn icon_is_opaque_tile_with_clear_corners() {
        let n = super::SIZE;
        let px = super::rgba(n);
        assert_eq!(px.len(), (n * n * 4) as usize);
        // The rounded tile's corner is clear, its middle opaque.
        assert_eq!(px[3], 0);
        let mid = (((n / 2) * n + n / 2) * 4) as usize;
        assert_eq!(px[mid + 3], 255);
        assert!(super::window_icon().is_some());
    }

    fn at(px: &[[u8; 4]], n: u32, x: u32, y: u32) -> [u8; 4] {
        px[(y * n + x) as usize]
    }

    /// The mark is the launcher's: its lime ring, the ring width about 0.32 of
    /// the outer radius (never under 2 px), on the launcher's page colours; at
    /// every icon size the ring's top rows are solid lime (crisp: within 8 of it, the outermost row's antialiasing), the centre is
    /// the page.
    #[test]
    fn icon_is_the_launchers_lime_ring() {
        let Rgb(lr, lg, lb) = THEME_A.lime;
        let near_lime =
            |p: [u8; 4]| p[3] == 255 && [(p[0], lr), (p[1], lg), (p[2], lb)].iter().all(|&(a, b)| a.abs_diff(b) <= 8);
        for n in [16u32, 24, 32, 48, 64, 128, 256] {
            let px = super::art::draw(n);
            let (outer, inner) = super::art::ring_px(n);
            let w = outer - inner;
            assert!(w >= 2.0, "{n}: ring {w} px");
            assert!((w / outer - 0.32).abs() < 0.1, "{n}: ring {w} of {outer}");
            let c = n / 2;
            // Every row of the ring straight above the centre is lime.
            for k in 0..w as u32 {
                let y = c - outer as u32 + k;
                assert!(near_lime(at(&px, n, c, y)), "{n}: row {y}: {:?}", at(&px, n, c, y));
                assert!(near_lime(at(&px, n, c - 1, y)), "{n}: row {y}");
            }
            // Just outside and inside: the dark page (no glow, no streak).
            for p in [at(&px, n, c, c - outer as u32 - 1), at(&px, n, c, c - inner as u32), at(&px, n, c, c)] {
                assert!(p[0].max(p[1]).max(p[2]) <= 0x12 && p[3] == 255, "{n}: {p:?}");
            }
            let Rgb(tr, tg, tb) = THEME_A.page_top;
            let Rgb(br, bg, bb) = THEME_A.page_bottom;
            let mid = at(&px, n, c, c);
            for (m, (t, b)) in [mid[0], mid[1], mid[2]].into_iter().zip([(tr, br), (tg, bg), (tb, bb)]) {
                assert!(m >= b.min(t) && m <= b.max(t), "{n}: centre {mid:?}");
            }
        }
    }
}
