//! The AA preset must actually raise scene resolution, survive saving,
//! and restore the independent render scale when switched off.
use nfsu2::qol::{self, Enhancements};

#[test]
fn ssaa_presets_render_more_pixels_and_restore_settings() {
    qol::set_gpu_caps(8192, wgpu::DeviceType::DiscreteGpu);
    let mut e = Enhancements { render_scale: 75, ..Default::default() };
    assert!(e.parse("enh.post_aa", "7"));
    e.apply();
    assert_eq!(qol::scene_size((1920, 1080), 1), (3840, 2160), "SSAA 4x must render four times the pixels, independently of Render Scale");
    assert_eq!(qol::msaa_override(), Some(1), "SSAA must not also enable native MSAA");
    let mut saved = String::new();
    e.write(&mut saved);
    let mut loaded = Enhancements::default();
    for line in saved.lines() {
        let (key, value) = line.split_once(' ').unwrap();
        assert!(loaded.parse(key, value));
    }
    assert_eq!(loaded, e);
    assert!(qol::aa_offered(7, (1920, 1080)));
    assert!(qol::aa_offered(7, (3840, 2160)), "explicit SSAA 4x must be available at 4K on a capable discrete GPU");
    assert!(qol::aa_offered(6, (3840, 2160)));
    assert_eq!(qol::scene_size((3840, 2160), 1), (7680, 4320));
    assert!((qol::effective_scale(200, (3840, 2160), 1) - std::f64::consts::SQRT_2).abs() < 1e-9, "legacy Render Scale keeps its previous budget");
    qol::set_gpu_caps(8192, wgpu::DeviceType::IntegratedGpu);
    assert_eq!(qol::scene_size((3840, 2160), 1), (3840, 2160));
    assert!(!qol::aa_offered(7, (3840, 2160)));
    qol::set_gpu_caps(1920, wgpu::DeviceType::DiscreteGpu);
    assert_eq!(qol::scene_size((1920, 1080), 1), (1920, 1080));
    assert!(!qol::aa_offered(6, (1920, 1080)));
    qol::set_gpu_caps(8192, wgpu::DeviceType::Cpu);
    assert_eq!(qol::scene_size((1920, 1080), 1), (1920, 1080));
    qol::set_gpu_caps(8192, wgpu::DeviceType::DiscreteGpu);
    loaded.parse("enh.post_aa", "6");
    loaded.apply();
    assert_eq!(qol::scene_size((1920, 1080), 1), (2715, 1527));
    loaded.parse("enh.post_aa", "0");
    loaded.apply();
    assert_eq!(qol::scene_size((1920, 1080), 1), (1440, 810));
    Enhancements::default().apply();
    assert_eq!(qol::scene_size((1920, 1080), 1), (1920, 1080));
    assert_eq!(qol::msaa_override(), None);
    // Borderless fallback/resizing must use the actual output, not the
    // requested resolution retained in the options file.
    use nfsu2::frontend::options::{self, Category, RowId, Settings, Value};
    let mut settings = Settings::default();
    settings.resolution = (1920, 1080);
    settings.enh.post_aa = 7;
    qol::set_output_size((7680, 4320));
    let aa = options::rows(Category::GraphicsGroup(options::GraphicsGroup::Image), &settings, 0, false).into_iter().find(|r| r.id == RowId::PostAa).unwrap();
    assert!(matches!(aa.value, Some(Value::Owned(v)) if v.contains("limited")));
    settings.enh.post_aa = 5;
    options::change(RowId::PostAa, &mut settings, 0, true, false);
    // TAA (11) follows SSAA 16x on the row.
    assert_eq!(settings.enh.post_aa, nfsu2::qol::TAA, "unavailable supersampling is skipped at actual 8K output");
    qol::set_output_size((1920, 1080));
    settings.enh.post_aa = 5;
    options::change(RowId::PostAa, &mut settings, 0, true, false);
    assert_eq!(settings.enh.post_aa, 6);
    qol::set_output_size((0, 0));
}
