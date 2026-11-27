//! Blur / depth-of-field tests that name the menus' option table (the top layer's tests).

use super::*;

/// The Graphics rows: Customize Display's values first, the clone's
/// tiers above them (stored apart, the native value held on).
#[test]
fn graphics_rows_share_the_native_switches() {
    use crate::frontend::graphics_table::row;
    use crate::frontend::options::{RowId, Settings};
    let mut s = Settings::default();
    let r = row(RowId::MotionBlur).unwrap();
    assert_eq!(r.tiers.len(), 4);
    (r.set)(&mut s, 0);
    assert_eq!((s.custom[11], s.enh.motion_blur), (0, 0));
    (r.set)(&mut s, 1);
    assert_eq!((s.custom[11], s.enh.motion_blur, (r.get)(&s)), (1, 0, 1));
    (r.set)(&mut s, 3);
    assert_eq!((s.enh.motion_blur, (r.get)(&s)), (2, 3));
    (r.set)(&mut s, 2);
    assert_eq!((s.custom[11], s.enh.motion_blur, (r.get)(&s)), (1, 1, 2));
    let r = row(RowId::DepthOfField).unwrap();
    (r.set)(&mut s, 2);
    assert_eq!((s.enh.depth_of_field, (r.get)(&s)), (1, 2));
    (r.set)(&mut s, 0);
    assert_eq!((s.custom[13], s.enh.depth_of_field), (0, 0));
    // Tile and streak sizes scale with the scene.
    assert_eq!(max_blur((1920, 1080)), 43.2);
    assert!(tile_size((3840, 2160)) as f32 >= max_blur((3840, 2160)) * 0.5);
}
