/// A car farther than this from the player is placed again (0x417BB0
/// through 0x409840: 0x5F0FC0 against 0x78429C); a placement point nearer
/// than this to the player is refused (0x418110 through 0x409840).
pub const DESPAWN: f32 = 250.0;

/// clone deviation, docs/pop-in.md: at World Detail Ultra / Extreme
/// with LOD Fade a traffic car is drawn fully up to this far from the
/// player and fades out (screen-door) to nothing at [`DESPAWN`], where the
/// native placer moves it; it is placed at [`DESPAWN`] or farther, so it
/// fades in as it comes nearer instead of appearing.
pub const RING_FADE_FROM: f32 = 200.0;

/// The share of a traffic car drawn at `d` metres from the player (1 up to
/// [`RING_FADE_FROM`], 0 from [`DESPAWN`]).
pub fn ring_presence(d: f32) -> f32 {
    ((DESPAWN - d) / (DESPAWN - RING_FADE_FROM)).clamp(0.0, 1.0)
}
