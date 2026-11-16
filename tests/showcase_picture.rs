use nfsu2::frontend::screens::{ShowcasePreview, ShowcaseKey, ShowcaseMode, showcase};
use nfsu2::frontend::screens::Key;

#[test]
fn native_crop_keeps_fixed_pixel_offsets_at_each_output_size() {
    for (size, expected) in [
        ((640,480), ((150,20),(333,430))),
        ((1920,1080), ((470,50),(973,970))),
        ((2560,1080), ((630,50),(1293,970))),
        ((3840,2160), ((950,104),(1933,1942))),
        // x87 helper truncates the scaled extent first; integer half-size
        // truncation follows independently of the framebuffer centre.
        ((641,481), ((150,20),(333,430))),
        ((643,483), ((151,20),(333,432))),
        ((1919,1079), ((470,50),(971,968))),
    ] {
        assert_eq!(showcase::capture_rect(size), Some(expected));
    }
    assert!(showcase::capture_rect((0,480)).is_none());
    assert!(showcase::capture_rect((1,1)).is_none());
}

#[test]
fn showcase_save_waits_five_update_ticks_and_locks_inputs() {
    let mut v = ShowcasePreview::new(42, false, 0.0);
    assert_eq!(v.key(Key::Accept, 0.0), ShowcaseKey::Stream);
    assert_eq!(v.key(Key::Back, 0.0), ShowcaseKey::None);
    assert!(!v.picture && v.save_ticks.is_none(), "retake is not a save");
    assert_eq!(v.key(Key::Accept, 0.0), ShowcaseKey::Stream);
    assert_eq!(v.key(Key::Accept, 0.0), ShowcaseKey::Save);
    assert!(v.save_ticks.is_none(), "question alone must not arm capture");
    v.save_ticks = Some(5);
    for left in (0..5).rev() {
        assert_eq!(v.key(Key::Back, 0.0), ShowcaseKey::None);
        assert_eq!(v.key(Key::Accept, 0.0), ShowcaseKey::None);
        v.save_tick();
        assert_eq!(v.save_ticks, Some(left));
        assert_eq!(v.mode, ShowcaseMode::Picture);
    }
    v.save_tick();
    assert_eq!(v.save_ticks, Some(0), "capture stays pending without a rendered frame");
}

#[test]
fn captured_pixels_preserve_scene_orientation_and_roundtrip_native_container() {
    let size = (640u32,480u32);
    let pixels:Vec<u8> = (0..size.1).flat_map(|y| (0..size.0).flat_map(move |x| [x as u8,y as u8,(x^y) as u8,0])).collect();
    let (rgba,w,h) = showcase::capture_rgba(&pixels,size,false).unwrap();
    assert_eq!((w,h),(333,430));
    assert_eq!(&rgba[..4], &[150,20,130,255]);
    let last = (((449*640)+482)*4) as usize;
    assert_eq!(&rgba[rgba.len()-4..rgba.len()-1], &pixels[last..last+3]);
    let mut bgra=pixels.clone();
    for p in bgra.chunks_exact_mut(4) { p.swap(0,2); }
    assert_eq!(showcase::capture_rgba(&bgra,size,true).unwrap().0, rgba);
    let file=nfsu2::formats::native_save::picture(&rgba,w,h);
    let saved=nfsu2::formats::native_save::read_picture(&file).unwrap();
    assert_eq!(saved.len(),512*512*4);
    assert!(saved.chunks_exact(4).all(|p|p[3]==255));
    assert!(showcase::capture_rgba(&pixels[..100],size,false).is_none());
}
