/// How many times a canvas-spanning quad must magnify its texture (on both
/// axes) to count as a fill that reaches the screen's edges on a wide
/// screen (a dim or backing: U2_Menu_Box's 16 × 16 over the canvas is
/// ×40 and more), not a picture (PC_load.tga, movies: about ×1).
pub(super) const FILL_MAGNIFY: f32 = 4.0;
/// clip.z flags for ui.wgsl (added to the alpha-test mode 0 / 2): HD UI
/// Art pack art sampled by `sharp`, straight or premultiplied.
pub(super) const SHARP: f32 = 4.0;
pub(super) const SHARP_PREMULTIPLIED: f32 = 8.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Vertex {
    pub(super) position: [f32; 2],
    pub(super) uv: [f32; 2],
    pub(super) color: [f32; 4],
    /// Position in the quad, −1..1, and z: 1 clips to the inscribed
    /// circle, 2 alpha-tests (the texture's entry byte 0x55), 0 neither.
    pub(super) clip: [f32; 3],
}

/// What the UI pass may assume about its target's alpha.
#[derive(Clone, Copy, Debug, Default)]
pub struct Alpha {
    /// The pass before (the post passes' last) already wrote alpha 0, so
    /// destination-alpha textures need no full-screen alpha-0 quad first.
    pub zeroed: bool,
    /// The target is a swap chain image presented opaque
    /// (`CompositeAlphaMode::Opaque`): its alpha is never read after the
    /// UI, so the closing full-screen alpha-1 quad is left out. Read-backs
    /// (shots, the bench) keep it.
    pub opaque: bool,
}

/// docs/playing/modern-extras.md §4; clone deviation, opt-in: the
/// colour-vision palette (1 deuteranopia, 2 protanopia, 3 tritanopia; 0
/// native) and the quads it applies to (the in-game HUD's, by index in
/// the frame's list), set by the frame before [`UiRenderer::record`].
/// ui.wgsl remaps the HUD's reds, greens and blues (`palette`); nothing
/// else is touched.
static HUD_PALETTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn set_hud_palette(kind: u8, quads: std::ops::Range<usize>) {
    let v = if kind == 0 || quads.is_empty() {
        0
    } else {
        u64::from(kind.min(3)) << 56 | (quads.start as u64 & 0x0FFF_FFFF) << 28 | (quads.end as u64 & 0x0FFF_FFFF)
    };
    HUD_PALETTE.store(v, std::sync::atomic::Ordering::Relaxed);
}

pub(super) fn hud_palette_of(i: usize) -> u8 {
    let v = HUD_PALETTE.load(std::sync::atomic::Ordering::Relaxed);
    let (kind, start, end) = ((v >> 56) as u8, (v >> 28 & 0x0FFF_FFFF) as usize, (v & 0x0FFF_FFFF) as usize);
    if kind != 0 && (start..end).contains(&i) { kind } else { 0 }
}

/// Where the palette's pipelines start in `UiRenderer::pipelines`.
pub(super) const PALETTE_PIPELINES: usize = 6;
