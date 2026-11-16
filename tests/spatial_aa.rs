use nfsu2::{qol::{self, Enhancements}, frontend::options::{self, Category, RowId, Settings, Value}};

#[test]
fn extreme_spatial_aa_and_native_override() {
    qol::set_gpu_caps(16384, wgpu::DeviceType::DiscreteGpu);
    let mut e = Enhancements::default();
    e.parse("enh.post_aa", "10");
    e.apply();
    assert_eq!(qol::scene_size((3840, 2160), 1), (15360, 8640));
    let mut s = Settings::default();
    s.custom[14] = 3;
    s.enh = e;
    let native = options::rows(Category::CustomDisplay, &s, 0, false).into_iter().find(|r| r.id == RowId::Custom(14)).unwrap();
    assert!(!native.enabled);
    assert!(matches!(native.value, Some(Value::Owned(v)) if v.contains("Disabled") && v.contains("Enhancements")));
    options::change(RowId::Custom(14), &mut s, 0, false, false);
    assert_eq!(s.custom[14], 3, "disabled row preserves saved native AA");
    assert!(e.parse("enh.aa_msaa", "4"));
    assert!(e.parse("enh.aa_filter", "4"));
    e.apply();
    assert_eq!(qol::msaa_override(), Some(4));
    let p = nfsu2::render::enhance::Plan::of(&nfsu2::render::RenderOptions::in_force());
    assert!(p.fxaa && p.smaa);
    let mut saved = String::new();
    e.write(&mut saved);
    let mut loaded = Enhancements::default();
    for line in saved.lines() {
        let (k,v) = line.split_once(' ').unwrap();
        assert!(loaded.parse(k,v));
    }
    assert_eq!(e, loaded);
    // Every explicit MSAA/filter combination survives saving and selects
    // the requested passes. Unsupported counts fall back without invalid pipelines.
    for preset in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        for msaa in [0, 1, 2, 4, 8, 16] {
            for filter in 0..=4 {
                e.parse("enh.post_aa", &preset.to_string());
                e.parse("enh.aa_msaa", &msaa.to_string());
                e.parse("enh.aa_filter", &filter.to_string());
                e.apply();
                if msaa != 0 { assert_eq!(qol::msaa_override(), Some(msaa)); }
                let plan = nfsu2::render::enhance::Plan::of(&nfsu2::render::RenderOptions::in_force());
                let expected = match filter { 0 => (preset == 1, preset == 2), 1 => (false,false), 2 => (true,false), 3 => (false,true), _ => (true,true) };
                assert_eq!((plan.fxaa, plan.smaa), expected);
                let mut text = String::new(); e.write(&mut text);
                let mut reloaded = Enhancements::default();
                for line in text.lines() { let (k,v) = line.split_once(' ').unwrap(); reloaded.parse(k,v); }
                assert_eq!(reloaded, e);
            }
        }
    }
    qol::set_msaa_support(1 | (1 << 2)); // 1x and 4x only.
    assert!(!qol::msaa_offered(2));
    assert!(qol::msaa_offered(4));
    assert_eq!(qol::supported_msaa(8), 4);
    s.enh.post_aa = 7;
    s.enh.aa_msaa = 1;
    options::change(RowId::AaMsaa, &mut s, 0, true, false);
    assert_eq!(s.enh.aa_msaa, 4);
    s.enh.post_aa = 10;
    s.enh.aa_msaa = 8;
    qol::set_output_size((3840,2160));
    qol::set_gpu_caps(8192, wgpu::DeviceType::DiscreteGpu);
    let row = options::rows(Category::GraphicsGroup(options::GraphicsGroup::Image), &s, 0, false).into_iter().find(|r| r.id == RowId::PostAa).unwrap();
    assert!(matches!(row.value, Some(Value::Owned(v)) if v.contains("limited")));
    qol::set_output_size((0,0));
    qol::set_msaa_support(31);
    s.enh = Enhancements::default();
    assert!(options::rows(Category::CustomDisplay, &s, 0, false).into_iter().find(|r| r.id == RowId::Custom(14)).unwrap().enabled);
    s.enh.apply();
    assert_eq!(qol::msaa_override(), None);
}
