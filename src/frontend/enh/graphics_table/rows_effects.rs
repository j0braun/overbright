use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Effects.
    GraphicsRow {
        id: RowId::HdrPost,
        label: HDR_POST,
        group: GraphicsGroup::Effects,
        help: "Scene and post passes in high precision: smoother glow, no banding.",
        tiers: &[
            t("Native", "Native: 8-bit passes as the original."),
            tc("High", "High precision, glow at half size.", 1.84, 0.0),
            tc("Ultra", "High precision, glow at full size.", 2.79, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.hdr_post.min(2)),
        set: |s, i| s.enh.hdr_post = i as u8,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::LodFade,
        label: LOD_FADE,
        group: GraphicsGroup::Effects,
        help: "Scenery and car detail changes cross-fade instead of popping.",
        tiers: &[t("Off", "Off: pops as the original."), tc("On", "No visible pops while driving.", 0.1, 0.06)],
        native: None,
        get: |s| usize::from(s.enh.lod_fade),
        set: |s, i| s.enh.lod_fade = i != 0,
        offered: None,
        legacy: false,
    },
    // docs/particles-smoke.md: the native emitters, detailed smoke.
    GraphicsRow {
        id: RowId::Particles,
        label: PARTICLES,
        group: GraphicsGroup::Effects,
        help: "Tyre, purge and crash smoke as many small, soft puffs lit by the lamps and car lights; same emitters.",
        tiers: &[
            t("Original", "Original: the native smoke sprites."),
            tc(
                "High",
                "4 puffs per smoke particle, soft edges, lit, lingering (integrated GPU: ~3 ms, more in thick smoke).",
                PARTICLES_HIGH_COST_4K,
                0.0,
            ),
            tc(
                "Ultra",
                "8 smaller puffs, more detail, longer lingering clouds (integrated GPU: ~5 ms, more in thick smoke).",
                PARTICLES_ULTRA_COST_4K,
                0.0,
            ),
            tc(
                "Cinematic",
                "Volumetric smoke: thick, billowing, self-shadowed clouds lit by the street lamps, head and tail lights and underglow, stirred by the cars' wakes; steam on wet roads (cheaper than Ultra in thick smoke).",
                PARTICLES_CINEMATIC_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.particles.min(3)),
        set: |s, i| s.enh.particles = qol::PARTICLE_STEPS[i],
        offered: None,
        legacy: false,
    },
    // docs/sparks-fire.md: the native spark and flame emitters, next-gen look.
    GraphicsRow {
        id: RowId::SparksFire,
        label: SPARKS_FIRE,
        group: GraphicsGroup::Effects,
        help: "Scrape and crash sparks, nitrous and turbo flames; same emitters and timing as the original.",
        tiers: &[
            t("Original", "Original: the native spark and flame sprites."),
            tc(
                "Next-Gen",
                "Sparks thrown, bouncing and cooling as hot streaks; nitrous and turbo flames with heat haze; both light the road and the cars (integrated GPU: ~2.5 ms with flames on screen).",
                SPARKS_FIRE_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.sparks_fire.min(1)),
        set: |s, i| s.enh.sparks_fire = i as u8,
        offered: None,
        legacy: false,
    },
    // docs/motionblur-dof.md: Customize Display's Motion Blur
    // (g_MotionBlurEnable 0x870CDC: the speed blur 0x5CC470) with the
    // clone's motion-vector blur above it.
    GraphicsRow {
        id: RowId::MotionBlur,
        label: 0x822E_4E9B, // Motion Blur
        group: GraphicsGroup::Effects,
        help: "Blur from motion; the HUD, glows and the player's car stay sharp.",
        tiers: &[
            t("Off", "Original value: no speed blur."),
            t("On", "Original value, World Detail 5: the original's speed blur (strongest with nitrous)."),
            tc(
                "Camera",
                "The world blurred by the camera's motion, stronger with speed; same look at any refresh rate. Integrated GPU: ~+15 ms.",
                MOTION_CAMERA_COST_4K,
                0.0,
            ),
            tc(
                "Per-Object",
                "Camera blur plus rival and traffic cars blurred by their own motion and spinning wheels. Integrated GPU: ~+15 ms.",
                MOTION_OBJECT_COST_4K,
                0.0,
            ),
        ],
        native: Some(11),
        get: |s| custom_get(s, 11, usize::from(s.enh.motion_blur.min(2))),
        set: |s, i| s.enh.motion_blur = custom_part(s, 11, i).unwrap_or(0) as u8,
        offered: None,
        legacy: false,
    },
    // Customize Display's Depth Of Field (0x870CF0: its pass 0x5CCCE0
    // never runs behind the chase camera, not ported) with the clone's
    // Cinematic above it.
    GraphicsRow {
        id: RowId::DepthOfField,
        label: 0xA9C2_7DFD, // Depth Of Field
        group: GraphicsGroup::Effects,
        help: "Background and foreground soften around the car in cut-scenes, replays, the garage and photo mode; never while driving.",
        tiers: &[
            t("Off", "Original value."),
            t("On", "Original value, World Detail 5 (the original draws none behind the chase camera)."),
            tc(
                "Cinematic",
                "Focus on the car in cut-scenes, replays, the garage and showroom (integrated GPU: ~+10 ms there).",
                DOF_COST_4K,
                0.0,
            ),
        ],
        native: Some(13),
        get: |s| custom_get(s, 13, usize::from(s.enh.depth_of_field.min(1))),
        set: |s, i| s.enh.depth_of_field = custom_part(s, 13, i).unwrap_or(0) as u8,
        offered: None,
        legacy: false,
    },
    // docs/water.md: the fountains', pools' and streams' water.
    GraphicsRow {
        id: RowId::AnimatedWater,
        label: ANIMATED_WATER,
        group: GraphicsGroup::Effects,
        help: "Fountains, pools and streams: flowing ripples, splash rings, moving falling water, spray and mist, lit by the lamps.",
        tiers: &[
            t("Off", "Off: the original's water textures."),
            tc(
                "On",
                "Animated surfaces and falling sheets, spray and mist where the water lands; costs only with water in view (integrated GPU: ~5 ms at a fountain filling the screen).",
                WATER_COST_4K,
                0.0,
            ),
            tc(
                "Realistic",
                "As On, plus reflections of the scene, see-through water with depth and caustics, glassy lit jets, more spray and wet stone around the basins; only with water in view (integrated GPU: ~15 ms at a fountain filling the screen).",
                WATER_REALISTIC_COST_4K,
                0.0,
            ),
            tc(
                "Next-Gen",
                "As Realistic, with the jets breaking into thousands of simulated drops where they fall and land (lit by the lamps), basins with real waves from the falling water and the rain, and volumetric mist; only with water in view.",
                WATER_NEXTGEN_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.water.min(3)),
        set: |s, i| s.enh.water = i as u8,
        offered: None,
        legacy: false,
    },
    // docs/ambient-detail.md: street props, blown debris, dust, steam.
    GraphicsRow {
        id: RowId::AmbientDetail,
        label: AMBIENT_DETAIL,
        group: GraphicsGroup::Effects,
        help: "A living street: litter and small props along the kerbs, leaves and paper whirled up by passing cars, tyre dust, steam from the road grates; visual only.",
        tiers: &[
            t("Off", "Off: the original's streets."),
            tc(
                "On",
                "Cans, bottles, cups, boxes, paper and leaves along the kerbs and pavements (never on the racing line), lit by the street lamps and headlights (integrated GPU: ≈ 0).",
                AMBIENT_COST_4K,
                0.0,
            ),
            tc(
                "Next-Gen",
                "As On, plus leaves and paper blown up by passing cars and gusts that settle again, lit tyre dust on dry roads, drifting steam from road grates and drains, moths round the street lamps (integrated GPU: ~8 ms).",
                AMBIENT_NEXTGEN_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.ambient_detail.min(2)),
        set: |s, i| s.enh.ambient_detail = i as u8,
        offered: None,
        legacy: false,
    },
];
