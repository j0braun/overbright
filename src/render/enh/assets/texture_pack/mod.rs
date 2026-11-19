//! Optional external PNG replacements; original materials remain authoritative.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

mod cache;
mod decode;
mod roots;
pub(in crate::render) use cache::*;
pub(in crate::render) use decode::*;
pub(in crate::render) use roots::*;

pub(in crate::render) struct Replacement {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// None: RGBA8 levels (PNG); Dxt1/Dxt5: block-compressed levels from a
    /// DDS (the upscaled world pack, `crate::upscale`).
    pub format: Option<crate::formats::texture::PixelFormat>,
    pub bytes: Vec<u8>,
}

/// Select by renderer, so identical hashes cannot leak between UI, world and cars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::render) enum Domain {
    Ui,
    World,
    Car,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated loading-screen folder is a UI folder only, and
    /// only while HD UI Art is in force (here: not, so native).
    #[test]
    fn generated_ui_folder_needs_hd_ui_art() {
        assert_eq!(generated_ui(Domain::World), None);
        assert_eq!(generated_ui(Domain::Car), None);
        if !crate::hd_ui_pack::active() {
            assert_eq!(generated_ui(Domain::Ui), None);
        }
    }

    /// The VRAM cap drops a generated DDS's top levels and keeps the
    /// rest of the chain byte for byte; a PNG is never cut.
    /// The pack fingerprint the arrays' cache is checked against
    /// changes when a file a slot would use is added, rewritten or removed,
    /// or when the size limit changes, and not for files no slot uses.
    #[test]
    fn fingerprint_follows_the_files_slots_use() {
        const CHILD: &str = "NFSU2_TP1B_FINGERPRINT_CHILD";
        let Some(dir) = crate::render::debug_env::test_child(CHILD) else {
            let p = Pack::new();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "render::enh::assets::texture_pack::tests::fingerprint_follows_the_files_slots_use"])
                .env(CHILD, &p.0)
                .env("NFSU2_WORLD_TEXTURE_PACK", &p.0)
                .env_remove("NFSU2_CAR_TEXTURE_PACK")
                .env_remove("NFSU2_TEXTURE_PACK")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        };
        crate::hd_ui_pack::initialize(false);
        let p = PathBuf::from(dir);
        let slots = [(0xABCD_EF12u32, "road"), (0x1234_5678, "wall")];
        let key = || fingerprint(Domain::World, &slots, 4096).unwrap();
        assert!(fingerprint(Domain::Car, &slots, 4096).is_none(), "no car pack");
        let empty = key();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).save(p.join("ABCDEF12.png")).unwrap();
        let one = key();
        assert_ne!(one, empty, "a file added");
        assert_eq!(key(), one, "stable");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).save(p.join("0BADF00D.png")).unwrap();
        assert_eq!(key(), one, "a file no slot uses");
        std::fs::write(p.join("wall.png"), b"x").unwrap();
        let two = key();
        assert_ne!(two, one, "a name fallback added");
        image::RgbaImage::from_pixel(8, 4, image::Rgba([1, 2, 3, 255])).save(p.join("ABCDEF12.png")).unwrap();
        assert_ne!(key(), two, "a file rewritten");
        assert_ne!(fingerprint(Domain::World, &slots, 2048).unwrap(), key(), "another size limit");
        std::fs::remove_file(p.join("ABCDEF12.png")).unwrap();
        std::fs::remove_file(p.join("wall.png")).unwrap();
        assert_eq!(key(), empty, "removed again");
    }

    #[test]
    fn several_folders_first_match_wins() {
        let (a, b) = (Pack::new(), Pack::new());
        a.png("ABCDEF12.png", 4, 4, [1, 2, 3, 255]);
        b.png("ABCDEF12.png", 8, 8, [9, 9, 9, 255]);
        b.png("12345678.png", 2, 2, [7, 7, 7, 255]);
        let roots = vec![a.0.clone(), b.0.clone()];
        let (root, r) = load_first(&roots, 0xABCD_EF12, "", 4096).unwrap();
        assert_eq!((root, r.width), (&a.0, 4), "the first folder wins");
        let (root, r) = load_first(&roots, 0x1234_5678, "", 4096).unwrap();
        assert_eq!((root, r.width), (&b.0, 2), "a later folder fills in");
        assert!(load_first(&roots, 0x0BAD_F00D, "", 4096).is_none());
        // A PATH-style list; a missing folder is skipped.
        let list = std::env::join_paths([a.0.clone(), PathBuf::from("/nonexistent-tp1"), b.0.clone()]).unwrap();
        // SAFETY: a variable only this test reads.
        unsafe { std::env::set_var("NFSU2_TP1_TEST_PACKS", &list) };
        assert_eq!(explicit_roots("NFSU2_TP1_TEST_PACKS", crate::render::debug_env::nfsu2_tp1_test_packs()), roots);
    }

    #[test]
    fn drop_levels_keeps_the_lower_chain() {
        use crate::formats::texture::PixelFormat;
        let size = |w: u32, h: u32| PixelFormat::Dxt1.level_size(w, h);
        let total: usize = (0..9).map(|m| size((256u32 >> m).max(1), (128u32 >> m).max(1))).sum();
        let bytes: Vec<u8> = (0..total).map(|i| i as u8).collect();
        let r = Replacement { width: 256, height: 128, mips: 9, format: Some(PixelFormat::Dxt1), bytes: bytes.clone() };
        let d = drop_levels(r, 1);
        assert_eq!((d.width, d.height, d.mips), (128, 64, 8));
        assert_eq!(d.bytes, bytes[size(256, 128)..]);
        let png = Replacement { width: 64, height: 64, mips: 7, format: None, bytes: vec![0; 10] };
        assert_eq!(drop_levels(png, 1).width, 64);
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Pack(PathBuf);
    impl Pack {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "nfsu2-pack-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn png(&self, name: &str, w: u32, h: u32, color: [u8; 4]) {
            image::RgbaImage::from_pixel(w, h, image::Rgba(color)).save(self.0.join(name)).unwrap();
        }
    }
    impl Drop for Pack {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn hash_wins_and_non_square_mips_reach_one_texel() {
        let p = Pack::new();
        p.png("ABCDEF12.png", 8, 2, [20, 40, 60, 128]);
        p.png("road.png", 4, 4, [255; 4]);
        let r = load(&p.0, 0xabcdef12, "road", 4096).unwrap();
        assert_eq!((r.width, r.height, r.mips), (8, 2, 4));
        assert_eq!(r.bytes.len(), (16 + 4 + 2 + 1) * 4);
        assert!(r.bytes.chunks_exact(4).all(|p| p == [20, 40, 60, 128]));
    }
    #[test]
    fn transparent_colors_do_not_bleed_into_mips() {
        let p = Pack::new();
        let rgba = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 0, 255, 255]).unwrap();
        rgba.save(p.0.join("edge.png")).unwrap();
        let r = load(&p.0, 1, "edge", 4096).unwrap();
        assert_eq!(&r.bytes[..8], rgba.as_raw());
        assert_eq!(&r.bytes[8..], &[0, 0, 255, 128]);
    }

    #[test]
    fn generated_dds_loads_with_its_mips_and_a_png_overrides_it() {
        let p = Pack::new();
        let rgba: Vec<u8> = (0..16 * 8).flat_map(|i| [(i * 2) as u8, 100, 200, 255]).collect();
        let up = crate::upscale::Upscaled { width: 16, height: 8, rgba, scale: 1 };
        let e = crate::upscale::encode(&up);
        std::fs::write(p.0.join("0000ABCD.dds"), crate::upscale::dds_bytes(&e)).unwrap();
        let r = load(&p.0, 0xabcd, "sign", 4096).unwrap();
        assert_eq!((r.width, r.height, r.mips, r.format), (16, 8, 5, Some(crate::formats::texture::PixelFormat::Dxt1)));
        assert_eq!(r.bytes, e.data);
        // Over the GPU's limit, truncated or not a DDS: the original is used.
        assert!(load(&p.0, 0xabcd, "sign", 8).is_none());
        let bytes = crate::upscale::dds_bytes(&e);
        std::fs::write(p.0.join("0000ABCE.dds"), &bytes[..bytes.len() - 1]).unwrap();
        assert!(load(&p.0, 0xabce, "x", 4096).is_none());
        std::fs::write(p.0.join("0000ABCF.dds"), b"DDS not really").unwrap();
        assert!(load(&p.0, 0xabcf, "x", 4096).is_none());
        // A hand-made PNG of the same hash wins.
        p.png("0000ABCD.png", 4, 4, [1, 2, 3, 255]);
        let r = load(&p.0, 0xabcd, "sign", 4096).unwrap();
        assert_eq!((r.width, r.format), (4, None));
    }

    #[test]
    fn world_layer_uses_bc_as_is_or_decodes_with_mips() {
        const CHILD: &str = "NFSU2_DDS_PACK_TEST_CHILD";
        if crate::render::debug_env::test_child(CHILD).is_none() {
            let p = Pack::new();
            let up = crate::upscale::Upscaled { width: 8, height: 4, rgba: [90u8, 80, 70, 255].repeat(32), scale: 4 };
            std::fs::write(p.0.join("ABCDEF12.dds"), crate::upscale::dds_bytes(&crate::upscale::encode(&up))).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "render::enh::assets::texture_pack::tests::world_layer_uses_bc_as_is_or_decodes_with_mips",
                ])
                .env(CHILD, "1")
                .env("NFSU2_WORLD_TEXTURE_PACK", &p.0)
                .env_remove("NFSU2_CAR_TEXTURE_PACK")
                .env_remove("NFSU2_TEXTURE_PACK")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        crate::hd_ui_pack::initialize(false);
        use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
        let t = Texture {
            name: "road".into(),
            hash: 0xabcdef12,
            width: 2,
            height: 1,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: vec![255; 8].into(),
            palette: vec![],
            alpha_test: false,
            blend: Blend::None,
            rendering_order: 0,
            late_buckets: false,
            scroll: Scroll::default(),
        };
        let (key, bytes) = crate::render::texture_layer_with_pack(Domain::World, Some(&t), true, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Bc1RgbaUnorm, 8, 4, 4));
        assert_eq!(bytes.len(), 16 + 8 + 8 + 8);
        let (key, bytes) = crate::render::texture_layer_with_pack(Domain::World, Some(&t), false, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Rgba8Unorm, 8, 4, 4));
        assert_eq!(bytes.len(), (32 + 8 + 2 + 1) * 4);
        assert!(bytes.chunks_exact(4).all(|p| p[3] == 255 && (p[0] as i32 - 90).abs() <= 8));
        // Not a world texture: the car domain has no pack here.
        let (key, _) = crate::render::texture_layer_with_pack(Domain::Car, Some(&t), true, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Rgba8Unorm, 2, 1, 1));
    }

    /// An Overbright pack (docs/modding/texture-packs.md) in the data folder reaches
    /// the world layer: its PNG compiled to BC7, used as is with BC and
    /// decoded with its mips without; the car domain has nothing.
    #[test]
    fn overbright_pack_reaches_the_world_layer_as_bc7() {
        const CHILD: &str = "NFSU2_OBPACK_TEST_CHILD";
        if crate::render::debug_env::test_child(CHILD).is_none() {
            let (data, cache) = (Pack::new(), Pack::new());
            let dir = data.0.join("packs/roads/world");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.parent().unwrap().join("pack.toml"), "format = 1\nname = \"Roads\"\n").unwrap();
            image::RgbaImage::from_pixel(8, 4, image::Rgba([90, 80, 70, 255])).save(dir.join("road.png")).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "render::enh::assets::texture_pack::tests::overbright_pack_reaches_the_world_layer_as_bc7"])
                .env(CHILD, "1")
                .env("NFSU2_DATA_DIR", &data.0)
                .env("NFSU2_CACHE_DIR", &cache.0)
                .env_remove("NFSU2_WORLD_TEXTURE_PACK")
                .env_remove("NFSU2_CAR_TEXTURE_PACK")
                .env_remove("NFSU2_TEXTURE_PACK")
                .status()
                .unwrap();
            assert!(status.success());
            assert!(cache.0.join("packs/roads/world").join(format!("{:08X}.dds", crate::formats::hash::bin_hash("road"))).is_file());
            return;
        }
        crate::hd_ui_pack::initialize(false);
        use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
        let t = Texture {
            name: "road".into(),
            hash: crate::formats::hash::bin_hash("road"),
            width: 2,
            height: 1,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: vec![255; 8].into(),
            palette: vec![],
            alpha_test: false,
            blend: Blend::None,
            rendering_order: 0,
            late_buckets: false,
            scroll: Scroll::default(),
        };
        let (key, bytes) = crate::render::texture_layer_with_pack(Domain::World, Some(&t), true, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Bc7RgbaUnorm, 8, 4, 4));
        assert_eq!(bytes.len(), (2 + 1 + 1 + 1) * 16);
        let (key, bytes) = crate::render::texture_layer_with_pack(Domain::World, Some(&t), false, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Rgba8Unorm, 8, 4, 4));
        assert!(bytes.chunks_exact(4).all(|p| p[3] == 255 && (p[0] as i32 - 90).abs() <= 4));
        let (key, _) = crate::render::texture_layer_with_pack(Domain::Car, Some(&t), true, 4096);
        assert_eq!(key, (wgpu::TextureFormat::Rgba8Unorm, 2, 1, 1));
    }

    #[test]
    fn safe_name_fallback_and_invalid_files() {
        let p = Pack::new();
        p.png("road.png", 3, 5, [10, 20, 30, 255]);
        assert!(load(&p.0, 1, "road", 5).is_some());
        assert!(load(&p.0, 1, "road", 4).is_none());
        assert!(load(&p.0, 1, "../road", 4096).is_none());
        assert!(load(&p.0, 1, "missing", 4096).is_none());
        std::fs::write(p.0.join("00000001.png"), b"invalid png").unwrap();
        assert!(load(&p.0, 1, "missing", 4096).is_none());
        assert!(load(&p.0, 1, "road", 4096).is_some());
    }
    #[test]
    fn world_pack_works_with_hd_ui_disabled() {
        const CHILD: &str = "NFSU2_WORLD_PACK_TEST_CHILD";
        if crate::render::debug_env::test_child(CHILD).is_none() {
            let world = Pack::new();
            world.png("ABCDEF12.png", 8, 2, [20, 40, 60, 128]);
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "render::enh::assets::texture_pack::tests::world_pack_works_with_hd_ui_disabled"])
                .env(CHILD, "1")
                .env("NFSU2_WORLD_TEXTURE_PACK", &world.0)
                .env_remove("NFSU2_CAR_TEXTURE_PACK")
                .env_remove("NFSU2_TEXTURE_PACK")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        crate::hd_ui_pack::initialize(false);
        assert!(!crate::hd_ui_pack::active());
        use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
        let t = Texture {
            name: "road".into(),
            hash: 0xabcdef12,
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: vec![255; 16].into(),
            palette: vec![],
            alpha_test: true,
            blend: Blend::Additive,
            rendering_order: 5,
            late_buckets: true,
            scroll: Scroll::default(),
        };
        let runtime = crate::render::prepared::arrays_with_pack(&[Some(&t)], false, 256, 4096);
        assert_eq!(runtime.arrays[0].key, (wgpu::TextureFormat::Rgba8Unorm, 8, 2, 4));
        assert_eq!(&runtime.arrays[0].levels[0].data[..4], &[20, 40, 60, 128]);
        assert_eq!((t.width, t.height), (2, 2));
    }

    #[test]
    fn scoped_packs_isolate_hashes_and_restore_native_without_ui_or_maps() {
        const CHILD: &str = "NFSU2_SCOPED_PACK_TEST_CHILD";
        let Ok(mode) = crate::render::debug_env::test_child_mode(CHILD) else {
            let world = Pack::new();
            let car = Pack::new();
            let broken = Pack::new();
            world.png("ABCDEF12.png", 8, 2, [20, 40, 60, 128]);
            car.png("ABCDEF12.png", 4, 8, [80, 100, 120, 64]);
            std::fs::write(broken.0.join("ABCDEF12.png"), b"invalid PNG").unwrap();
            for mode in ["both", "world", "car", "none", "empty", "missing", "invalid"] {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child.args(["--exact", "render::enh::assets::texture_pack::tests::scoped_packs_isolate_hashes_and_restore_native_without_ui_or_maps"])
                    .env(CHILD, mode).env_remove("NFSU2_TEXTURE_PACK")
                    .env_remove("NFSU2_WORLD_TEXTURE_PACK").env_remove("NFSU2_CAR_TEXTURE_PACK");
                if mode == "both" || mode == "world" {
                    child.env("NFSU2_WORLD_TEXTURE_PACK", &world.0);
                }
                if mode == "both" || mode == "car" {
                    child.env("NFSU2_CAR_TEXTURE_PACK", &car.0);
                }
                if mode == "empty" {
                    child.env("NFSU2_WORLD_TEXTURE_PACK", "").env("NFSU2_CAR_TEXTURE_PACK", "");
                }
                if mode == "missing" {
                    child.env("NFSU2_WORLD_TEXTURE_PACK", world.0.join("absent"));
                }
                if mode == "invalid" {
                    child.env("NFSU2_WORLD_TEXTURE_PACK", &broken.0);
                }
                assert!(child.status().unwrap().success(), "scoped pack case {mode}");
            }
            return;
        };
        crate::hd_ui_pack::initialize(false);
        use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
        let t = Texture {
            name: "road".into(),
            hash: 0xabcdef12,
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: vec![255; 16].into(),
            palette: vec![],
            alpha_test: true,
            blend: Blend::Additive,
            rendering_order: 5,
            late_buckets: true,
            scroll: Scroll { kind: 1, speed: [0.5, 0.25], ..Default::default() },
        };
        // Repeated cross-domain lookups of the same hash exercise the cache key.
        for domain in [Domain::World, Domain::Car, Domain::Ui, Domain::Car, Domain::World] {
            let (shape, bytes) = crate::render::texture_layer_with_pack(domain, Some(&t), false, 4096);
            let (expected, pixel) = match domain {
                Domain::World if mode == "world" || mode == "both" => ((8, 2, 4), [20, 40, 60, 128]),
                Domain::Car if mode == "car" || mode == "both" => ((4, 8, 4), [80, 100, 120, 64]),
                _ => ((2, 2, 1), [255; 4]),
            };
            assert_eq!((shape.1, shape.2, shape.3), expected, "{mode} {domain:?}");
            assert_eq!(&bytes[..4], &pixel, "{mode} {domain:?}");
            let (capped, _) = crate::render::texture_layer_with_pack(domain, Some(&t), false, 2);
            assert_eq!((capped.1, capped.2), (2, 2));
        }
        assert_eq!(
            enabled(Domain::World),
            matches!(mode.as_str(), "world" | "both" | "invalid"),
            "car-only pack must not bypass native world-array cache"
        );
        assert!(!enabled(Domain::Ui));
        assert!(!crate::hd_ui_pack::active());
        assert!(!crate::hd_ui_pack::installed());
        assert_eq!((t.width, t.height, t.mip_count), (2, 2, 1));
        assert_eq!(t.blend, Blend::Additive);
        assert!(t.alpha_test && t.late_buckets);
        assert_eq!(t.scroll.speed, [0.5, 0.25]);
        assert_eq!(t.rendering_order, 5);
        if mode == "both" && crate::render::debug_env::nfsu2_pack_test_gpu() {
            let gpu = pollster::block_on(crate::render::Gpu::new(
                None,
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
            ))
            .unwrap();
            for domain in [Domain::World, Domain::Car, Domain::Ui] {
                let _view = crate::render::upload_texture_for(&gpu, &t, false, domain);
            }
            gpu.queue.submit([]);
        }
    }

    #[test]
    fn runtime_arrays_use_pack_without_changing_native_arrays_or_metadata() {
        const CHILD: &str = "NFSU2_PACK_TEST_CHILD";
        if crate::render::debug_env::test_child(CHILD).is_none() {
            let p = Pack::new();
            p.png("ABCDEF12.png", 8, 2, [20, 40, 60, 128]);
            for empty_scoped in [false, true] {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child.args(["--exact", "render::enh::assets::texture_pack::tests::runtime_arrays_use_pack_without_changing_native_arrays_or_metadata"])
                    .env(CHILD, "1").env("NFSU2_TEXTURE_PACK", &p.0)
                    .env_remove("NFSU2_WORLD_TEXTURE_PACK").env_remove("NFSU2_CAR_TEXTURE_PACK");
                if empty_scoped {
                    child.env("NFSU2_WORLD_TEXTURE_PACK", "").env("NFSU2_CAR_TEXTURE_PACK", "");
                }
                assert!(child.status().unwrap().success());
            }
            return;
        }
        use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
        let t = Texture {
            name: "road".into(),
            hash: 0xabcdef12,
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: vec![255; 16].into(),
            palette: vec![],
            alpha_test: true,
            blend: Blend::Additive,
            rendering_order: 5,
            late_buckets: true,
            scroll: Scroll { kind: 1, speed: [0.5, 0.25], ..Default::default() },
        };
        let sources = [Some(&t), None];
        let native = crate::render::prepared::arrays(&sources, false, 256);
        let runtime = crate::render::prepared::arrays_with_pack(&sources, false, 256, 4096);
        let a = &runtime.arrays[runtime.slots[0].0 as usize];
        assert_eq!(a.key, (wgpu::TextureFormat::Rgba8Unorm, 8, 2, 4));
        assert_eq!(a.levels.len(), 4);
        assert_eq!(&a.levels[0].data[..4], &[20, 40, 60, 128]);
        assert_eq!(native.arrays[native.slots[0].0 as usize].key, (wgpu::TextureFormat::Rgba8Unorm, 2, 2, 1));
        let capped = crate::render::prepared::arrays_with_pack(&sources, false, 256, 4);
        assert_eq!(capped.arrays[capped.slots[0].0 as usize].key, native.arrays[native.slots[0].0 as usize].key);
        assert_eq!((t.width, t.height, t.mip_count), (2, 2, 1));
        assert!(t.alpha_test && t.late_buckets);
        assert_eq!(t.blend, Blend::Additive);
        assert_eq!(t.scroll.speed, [0.5, 0.25]);
        assert_eq!(t.rendering_order, 5);
        // Opt-in GPU smoke check, leaving normal unit tests adapter-independent.
        if crate::render::debug_env::nfsu2_pack_test_gpu() {
            let gpu = pollster::block_on(crate::render::Gpu::new(
                None,
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
            ))
            .unwrap();
            let _view = crate::render::upload_texture(&gpu, &t, false);
            let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor::default());
            let _arrays = crate::render::TextureArrays::new(&gpu, &sampler, &runtime);
            gpu.queue.submit([]);
        }
    }

    /// A texture mod the launcher keeps (a synthetic one-texture package
    /// imported through the launcher's code path) reaches the loader with
    /// no variable: exactly its texture is replaced, in the world only;
    /// switched off, the original again.
    #[test]
    fn launcher_texture_mods_reach_the_loader() {
        use crate::texture_mods::tests as tm;
        let home = tm::scratch("loader");
        crate::platform::dirs::set_thread_data_home(home.clone());
        let road = tm::texture("ROAD_LOADER", 4, 4, [90, 90, 90, 255]);
        let wall = tm::texture("WALL_LOADER", 4, 4, [30, 60, 90, 255]);
        let game = vec![
            crate::tpf_import::Source {
                file: "TRACKS/L4RA.BUN".into(),
                domain: crate::tpf_import::Domain::World,
                tex: road.clone(),
            },
            crate::tpf_import::Source {
                file: "TRACKS/L4RA.BUN".into(),
                domain: crate::tpf_import::Domain::World,
                tex: wall.clone(),
            },
        ];
        let key = crate::tpf_import::texmod_hash(&road).unwrap();
        let bytes = tm::zip(&[(&format!("SPEED2.EXE_0x{key:08X}.png"), tm::png(8, 8))], "One road");
        let pkg: crate::texture_mods::Package = crate::formats::tpf::read(&bytes).unwrap().into();
        let packages = crate::texture_mods::packages_dir();
        let (info, _) =
            crate::texture_mods::install(&pkg, std::path::Path::new("one.tpf"), &game, &packages, &|_, _| {}).unwrap();
        let id = info.unwrap().id;
        let shape =
            |d, t: &crate::formats::texture::Texture| crate::render::texture_layer_with_pack(d, Some(t), false, 4096).0;
        let (road_native, wall_native, road_car) =
            (shape(Domain::World, &road), shape(Domain::World, &wall), shape(Domain::Car, &road));
        assert_eq!((road_native.1, road_native.2), (4, 4), "not listed: native");
        crate::texture_mods::save(&[crate::texture_mods::Entry { id: id.clone(), on: true }]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(550));
        assert_eq!((shape(Domain::World, &road).1, shape(Domain::World, &road).2), (8, 8), "the mod's texture");
        assert_eq!(shape(Domain::World, &wall), wall_native, "every other texture native");
        assert_eq!(shape(Domain::Car, &road), road_car, "a world package leaves cars alone");
        crate::texture_mods::save(&[crate::texture_mods::Entry { id, on: false }]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(550));
        assert_eq!(shape(Domain::World, &road), road_native, "off: native again");
        let _ = std::fs::remove_dir_all(home);
    }
}
