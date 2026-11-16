//! The options: the settings the game keeps in the profile (the options
//! block at profile +0x24, 0x83AA04 for the first profile; 0x5116E0 sets
//! its defaults from the profile constructor 0x5344B0) and the UI_Options
//! screen that edits them (screen class 0x4CF750, rows by 0x4B8470).
//!
//! UI_OptionsMain's options set the category 0x83AA04 (0x49A620…): Audio
//! 0, Display 1, Gameplay 2 and Player 3 open UI_Options.fng; Controls 4
//! opens UI_Options_PC_Controller.fng, EA GAMES TRAX 5 EA_Trax_Jukebox.fng
//! and Credits 8 Credits.fng (0x4B6080).

use glam::Vec2;

use super::screens::Key;
use super::{Assets, Quad, ScreenState, layout};
use crate::formats::hash::fe_hash;

mod category;
mod change;
mod labels;
mod rows;
mod screen;

pub use super::enh::enh_rows::*;
pub use super::screens::brand_select::*;
pub use super::settings::*;
pub(crate) use super::widgets::slider_list::*;
use super::{feng, graphics_table, screens, settings_model, settings_page};
pub use category::*;
pub use change::*;
pub use labels::*;
pub use rows::*;
pub use screen::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// The Display group holds HDR Output and its calibration; without
    /// an HDR signal (headless, SDR displays) every row is greyed, HDR
    /// Output reads "Not Supported" and nothing steps.
    #[test]
    fn hdr_rows_are_greyed_without_an_hdr_signal() {
        assert!(crate::qol::hdr_output_signal().is_none(), "no test sets a signal");
        let mut s = Settings::default();
        let list = rows(Category::GraphicsGroup(GraphicsGroup::Display), &s, 0, false);
        assert_eq!(
            list.iter().map(|r| r.id).collect::<Vec<_>>(),
            [RowId::HdrOutput, RowId::HdrPeak, RowId::HdrPaper, RowId::HdrGamma]
        );
        assert!(list.iter().all(|r| !r.enabled));
        assert!(matches!(&list[0].value, Some(Value::Owned(v)) if v == "Not Supported"));
        for id in [RowId::HdrOutput, RowId::HdrPeak, RowId::HdrPaper, RowId::HdrGamma] {
            change(id, &mut s, 0, true, false);
        }
        assert!(s.enh.is_native() && !s.enh.hdr_output);
        // Each row has a detail line; the presets leave the display rows.
        assert!(
            detail(Category::GraphicsGroup(GraphicsGroup::Display), RowId::HdrOutput, &s).unwrap().contains("HDR10")
        );
        s.enh.hdr_output = true;
        s.enh.hdr_peak = 1400;
        crate::qol::Preset::Ultra.apply(&mut s.enh);
        assert!(s.enh.hdr_output && s.enh.hdr_peak == 1400);
        crate::qol::Preset::Original.apply(&mut s.enh);
        assert!(s.enh.hdr_output);
    }

    /// The calibration tiers write the values they read back.
    #[test]
    fn hdr_calibration_tiers_round_trip() {
        let mut s = Settings::default();
        for id in [RowId::HdrPeak, RowId::HdrPaper, RowId::HdrGamma] {
            let r = super::super::graphics_table::row(id).unwrap();
            for i in 0..r.tiers.len() {
                (r.set)(&mut s, i);
                assert_eq!((r.get)(&s), i, "{id:?} tier {i}");
            }
        }
    }

    #[test]
    fn graphics_page_steps_and_restores() {
        use super::super::graphics_table::{ROWS, group_rows};
        let mut s = Settings::default();
        let native = s.clone();
        let before = s.deviations().len();
        // The page: the preset, the seven groups, Restore Native.
        let ids: Vec<RowId> = rows(Category::Graphics, &s, 0, false).iter().map(|r| r.id).collect();
        let mut want = vec![RowId::GraphicsPreset];
        want.extend(GRAPHICS_GROUPS.iter().map(|g| RowId::GraphicsGroup(*g)));
        want.push(RowId::RestoreNative);
        assert_eq!(ids, want);
        // The hub and its Advanced page list every settings group once (the Graphics page is no longer linked).
        assert!(super::super::settings_page::GROUPS.iter().all(|g| {
            [Category::Enhancements, Category::Advanced]
                .iter()
                .flat_map(|&c| rows(c, &s, 0, false))
                .filter(|r| r.id == RowId::UxGroup(*g) && r.kind == Kind::Button)
                .count()
                == 1
        }));
        assert_eq!(rows(Category::Graphics, &s, 0, false)[0].value, Some(Value::Text("Original")));
        // Row order: one row per concept, by group.
        let group_ids = |s: &Settings| {
            GRAPHICS_GROUPS
                .iter()
                .flat_map(|g| rows(Category::GraphicsGroup(*g), s, 0, false))
                .map(|r| r.id)
                .collect::<Vec<_>>()
        };
        let shown = group_ids(&s);
        assert_eq!(
            shown,
            [
                RowId::PostAa,
                RowId::AaMsaa,
                RowId::AaFilter,
                RowId::Sharpen,
                RowId::TextureFiltering,
                RowId::TexturePack,
                RowId::GenerateTexturePack,
                RowId::ModernLighting,
                RowId::ModernStrength,
                RowId::Ssao,
                RowId::ShadowQuality,
                RowId::DynamicLamps,
                RowId::LampShadows,
                RowId::ContactShadows,
                RowId::HeadlightBeams,
                RowId::LightProbes,
                RowId::SurfaceDetail,
                RowId::VolumetricFog,
                RowId::Sky,
                RowId::EnhancedReflections,
                RowId::RoadReflection,
                RowId::WetRoads,
                RowId::Ssr,
                RowId::EnhancedPaint,
                RowId::EnhancedGlass,
                RowId::CarReflection,
                RowId::ReflectionRate,
                RowId::CarDetail,
                RowId::SmoothCars,
                RowId::HdrPost,
                RowId::LodFade,
                RowId::Particles,
                RowId::SparksFire,
                RowId::MotionBlur,
                RowId::DepthOfField,
                RowId::AnimatedWater,
                RowId::AmbientDetail,
                RowId::WetCar,
                RowId::RainDetail,
                RowId::Spray,
                RowId::WetInteraction,
                RowId::LensDrops,
                RowId::RenderScale,
                RowId::Upscale,
                RowId::DetailDistance,
                RowId::HdrOutput,
                RowId::HdrPeak,
                RowId::HdrPaper,
                RowId::HdrGamma,
            ]
        );
        // Each row in exactly one group; the removed row only while on.
        let mut sorted = shown.clone();
        sorted.sort_by_key(|id| format!("{id:?}"));
        sorted.dedup();
        assert_eq!(sorted.len(), shown.len());
        assert!(!shown.contains(&RowId::ReversedZ));
        // Every row's value is a tier name; at native, the tier is the
        // original's value (World Detail 5: each shared row at its top).
        for r in ROWS {
            let i = (r.get)(&s);
            if r.native.is_some() {
                assert_eq!(i, r.native_top(), "{:?} native tier", r.id);
            }
            assert!(r.tiers.iter().all(|t| !t.name.is_empty() && !t.help.is_empty() || t.name == "On"), "{:?}", r.id);
            if r.native.is_none() {
                // Lighting Strength keeps its default (Bold) while the
                // Lighting row is Original (greyed, no effect).
                assert_eq!(
                    i,
                    match r.id {
                        RowId::RenderScale => 4,
                        RowId::ModernStrength => 4,
                        _ => 0,
                    },
                    "{:?}",
                    r.id
                );
                assert!(r.tiers[i].cost.is_none(), "{:?} native costs nothing", r.id);
            }
            // Tier names within a row are distinct.
            let mut names: Vec<&str> = r.tiers.iter().map(|t| t.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), r.tiers.len(), "{:?}", r.id);
        }
        // Labels: distinct on each clone page, and every clone label known.
        for c in [Category::Enhancements, Category::Advanced, Category::Graphics, Category::HdUi]
            .into_iter()
            .chain(GRAPHICS_GROUPS.map(Category::GraphicsGroup))
            .chain(super::super::settings_page::GROUPS.map(Category::Settings))
        {
            let mut labels: Vec<u32> = rows(c, &s, 0, false).iter().map(|r| r.label).collect();
            let n = labels.len();
            labels.sort_unstable();
            labels.dedup();
            assert_eq!(labels.len(), n);
        }
        let mut consts = [
            ENHANCEMENTS,
            FOV_OFFSET,
            HUD_SCALE,
            FRAME_CAP,
            DURABLE_SAVES,
            SAVE_BACKUPS,
            CRASH_REPORTS,
            RESTORE_NATIVE,
            NATIVE,
            GRAPHICS,
            RENDER_SCALE,
            POST_AA,
            AA_MSAA,
            AA_FILTER,
            SHARPEN,
            ROAD_REFLECTION,
            CAR_REFLECTION,
            DETAIL_DISTANCE,
            CAR_DETAIL,
            HD_UI,
            HD_UI_PAGE,
            HD_UI_PACK_STATUS,
            HD_UI_ACTIVATION_STATUS,
            RELOAD_PACKS,
            LOD_FADE,
            KEEP_LOADED,
            TEXTURE_FILTERING,
            REVERSED_Z,
            SHADOW_QUALITY,
            REFLECTION_RATE,
            SSAO,
            ENHANCED_REFLECTIONS,
            HDR_POST,
            ENHANCED_PAINT,
            GRAPHICS_PRESET,
            GROUP_IMAGE,
            GROUP_LIGHTING,
            GROUP_REFLECTIONS,
            GROUP_CARS,
            GROUP_EFFECTS,
            GROUP_PERFORMANCE,
            CUSTOM_PRESET,
            LISTS_OPEN_INSTALLED,
            OUTRUN_VARIETY,
            WET_ROADS,
            SSR,
            HUD_LAYOUT,
            HUD_CENTERED,
            HUD_FULL_WIDTH,
            DYNAMIC_LAMPS,
            LAMP_SHADOWS,
            CONTACT_SHADOWS,
            FIX_MODELS,
            UPSCALE,
            VOLUMETRIC_FOG,
            SKY,
            PARTICLES,
            SPARKS_FIRE,
            GROUP_DISPLAY,
            HDR_OUTPUT,
            HDR_PEAK,
            HDR_PAPER,
            HDR_GAMMA,
            PHOTO_MODE_ROW,
            PHOTO_MODE,
            GROUP_WEATHER,
            WET_CAR,
            RAIN_DETAIL,
            SPRAY,
            LENS_DROPS,
            WET_INTERACTION,
            SMOOTH_CARS,
            TRAFFIC_DENSITY,
            SKIP_MOVIES,
            SKIP_TITLE,
            LOW_LATENCY,
            REPLAY_CAMERAS,
            HUD_PALETTE,
            HUD_MINIMAP,
            HUD_SPEEDO,
            HUD_INFO,
            REDUCED_FLASHING,
            POWER_SAVING,
            MARK_SPOT,
            AMBIENT_DETAIL,
        ];
        consts.sort_unstable();
        assert!(consts.windows(2).all(|w| w[0] != w[1]));
        assert!(consts.iter().all(|c| clone_label(*c).is_some()));
        // Each row's detail line names what it does.
        for id in &shown {
            let d = detail(Category::GraphicsGroup(GraphicsGroup::Image), *id, &s).unwrap();
            assert!(d.len() > 20, "{id:?}: {d}");
        }
        // Tier stepping: Right once from native on every row.
        for id in &shown {
            change(*id, &mut s, 0, true, false);
        }
        // Render Scale wraps 100 % → 50 %; the shared rows wrap from the
        // original's top to their first enhanced tier.
        assert_eq!(
            (s.enh.render_scale, s.enh.post_aa, s.enh.sharpen, s.enh.road_reflection, s.enh.car_reflection),
            (50, 1, 25, 1, 256)
        );
        // World Detail 5 sets World Detail 2 of the card's 3: Right gives
        // the original's 3 first.
        assert_eq!((s.enh.detail_distance, s.custom[6], s.enh.car_detail, s.enh.shadows), (100, 3, 200, 2));
        assert!(
            s.enh.lod_fade
                && !s.enh.keep_loaded
                && s.enh.enhanced_reflections
                && s.enh.enhanced_paint
                && s.enh.wet_roads
        );
        assert_eq!((s.enh.ssao, s.enh.hdr_post, s.enh.ssr, s.enh.aa_msaa, s.enh.aa_filter), (25, 1, 1, 1, 1));
        // Texture Filtering and Update Rate have no tier above the
        // original's: they wrap to its lowest value.
        assert_eq!((s.custom[20], s.custom[0]), (0, 0));
        assert_eq!(rows(Category::Graphics, &s, 0, false)[0].value, Some(Value::Label(CUSTOM_PRESET)));
        // A cost shows in the detail line of a costly tier.
        assert!(
            detail(Category::GraphicsGroup(GraphicsGroup::Lighting), RowId::Ssao, &s).unwrap().contains("Cost: "),
            "{:?}",
            detail(Category::GraphicsGroup(GraphicsGroup::Lighting), RowId::Ssao, &s)
        );
        let image = rows(Category::GraphicsGroup(GraphicsGroup::Image), &s, 0, false);
        // Upscaling, stepped to Quality with the rest, runs TAA in
        // place of the row's FXAA and says so (greyed, overridden).
        assert_eq!(s.enh.upscale, 1);
        assert_eq!(image[0].value, Some(Value::Owned("Overridden by Upscaling (TAA)".into())));
        assert!(!image[0].enabled);
        assert!(s.deviations().len() > before);
        // An FSR 2 mode greys the Anti-Aliasing row and says why
        // (SSAA / MSAA / filters are off under it).
        let mut f = s.clone();
        f.enh.post_aa = 6;
        f.enh.upscale = 8;
        let image = rows(Category::GraphicsGroup(GraphicsGroup::Image), &f, 0, false);
        assert_eq!(image[0].value, Some(Value::Owned("Overridden by FSR 2".into())));
        assert!(!image[0].enabled);
        // Every tier of each row in turn, both ways, back to the start.
        for r in ROWS.iter().filter(|r| !r.legacy) {
            let mut t = native.clone();
            let start = (r.get)(&t);
            let mut seen = vec![start];
            loop {
                change(r.id, &mut t, 0, true, false);
                let i = (r.get)(&t);
                if i == start {
                    break;
                }
                seen.push(i);
                assert!(seen.len() <= r.tiers.len(), "{:?} loops", r.id);
            }
            let offered = (0..r.tiers.len()).filter(|&i| r.is_offered(&native, i)).count();
            assert_eq!(seen.len(), offered, "{:?} visits every offered tier", r.id);
            change(r.id, &mut t, 0, false, false);
            assert_eq!((r.get)(&t), *seen.last().unwrap(), "{:?} Left undoes Right", r.id);
        }
        // Upscaling sets the scene's size over the Render Scale: off for
        // the Render Scale checks below.
        let mut up = s.clone();
        assert!(
            !rows(Category::GraphicsGroup(GraphicsGroup::Performance), &up, 0, false)
                .into_iter()
                .find(|r| r.id == RowId::RenderScale)
                .unwrap()
                .enabled
        );
        change(RowId::RenderScale, &mut up, 0, true, false);
        assert_eq!(up.enh.render_scale, s.enh.render_scale, "Render Scale held under Upscaling");
        s.enh.upscale = 0;
        // Anti-Aliasing offers SSAA without changing the saved Render Scale.
        let mut aa = Vec::new();
        for _ in 0..12 {
            aa.push(s.enh.post_aa);
            change(RowId::PostAa, &mut s, 0, true, false);
        }
        // TAA (11) after SSAA 16x.
        assert_eq!(aa, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0]);
        s.enh.post_aa = 7;
        let scale_row = |s: &Settings| {
            rows(Category::GraphicsGroup(GraphicsGroup::Performance), s, 0, false)
                .into_iter()
                .find(|r| r.id == RowId::RenderScale)
                .unwrap()
        };
        assert!(!scale_row(&s).enabled);
        let mut t = s.clone();
        change(RowId::RenderScale, &mut t, 0, true, false);
        assert_eq!(t.enh.render_scale, 50, "SSAA preserves the independent scale setting");
        s.enh.post_aa = 0;
        // Render Scale shows the rendered height.
        s.resolution = (2560, 1440);
        s.enh.render_scale = 75;
        assert_eq!(scale_row(&s).value, Some(Value::Owned("75% (1080p)".into())));
        // A saved Render Scale above 100 % still steps back into range.
        s.enh.render_scale = 150;
        change(RowId::RenderScale, &mut s, 0, false, false);
        assert_eq!(s.enh.render_scale, 85);
        s.restore_native();
        assert!(s.enh.is_native() && s.deviations().is_empty());
        let _ = group_rows(GraphicsGroup::Image, &s);
    }

    /// One concept, one setting: a Graphics row that shares a
    /// Customize Display row steps through the original's values (same
    /// storage) and then the clone's tiers, and the Customize Display page
    /// shows and changes the same setting.
    #[test]
    fn shared_rows_extend_the_original_values() {
        use super::super::graphics_table::row;
        let mut s = Settings::default();
        s.custom = custom_presets(5);
        // Road Reflection Detail: Off, Low, Medium, High (custom 7 = 0–3),
        // then Very High / Ultra (enh.road_reflection 1 / 2).
        let r = row(RowId::RoadReflection).unwrap();
        let mut seen = Vec::new();
        for _ in 0..6 {
            change(RowId::RoadReflection, &mut s, 0, true, false);
            seen.push((s.custom[7], s.enh.road_reflection, r.tiers[(r.get)(&s)].name));
        }
        assert_eq!(
            seen,
            [(3, 1, "Very High"), (3, 2, "Ultra"), (0, 0, "Off"), (1, 0, "Low"), (2, 0, "Medium"), (3, 0, "High")]
        );
        // The Customize Display row while an enhanced tier is on: any change
        // goes back to the original's top.
        change(RowId::CarReflection, &mut s, 0, true, false);
        assert_eq!((s.custom[1], s.enh.car_reflection), (1, 256));
        let cd = rows(Category::CustomDisplay, &s, 0, false);
        assert_eq!(cd.iter().find(|r| r.id == RowId::Custom(1)).unwrap().value, Some(Value::Owned("Very High".into())));
        change(RowId::Custom(1), &mut s, 0, true, false);
        assert_eq!((s.custom[1], s.enh.car_reflection), (1, 128));
        change(RowId::Custom(1), &mut s, 0, true, false);
        assert_eq!(s.custom[1], 0, "then the original's toggle");
        // An enhanced tier keeps the original's value at its top when the
        // World Detail slider lowers the custom values.
        change(RowId::ShadowQuality, &mut s, 0, true, false);
        assert_eq!((s.custom[2], s.enh.shadows), (2, 2));
        s.detail = 2;
        change(RowId::Detail, &mut s, 0, false, false);
        assert_eq!(s.detail, 1);
        assert_eq!((s.custom[2], s.enh.shadows), (2, 2));
        // The removed duplicates (enh.anisotropy, enh.reflection_rate) load
        // as the original rows' values they duplicated.
        let mut t = Settings::default();
        t.custom = custom_presets(3);
        assert!(t.enh.parse("enh.anisotropy", "4") && t.enh.parse("enh.reflection_rate", "1"));
        super::super::graphics_table::normalize(&mut t);
        assert_eq!((t.custom[20], t.custom[0], t.enh.anisotropy, t.enh.reflection_rate), (2, 3, 0, 0));
        assert!(t.enh.is_native());
        // The old Keep All Loaded row is World Detail's top tier: a saved
        // enh.keep_loaded 1 (or 200 %) loads as Ultra (200 % + the ring).
        for (k, v) in [("enh.keep_loaded", "1"), ("enh.detail_distance", "200")] {
            let mut u = Settings::default();
            assert!(u.enh.parse(k, v));
            super::super::graphics_table::normalize(&mut u);
            assert_eq!((u.enh.detail_distance, u.enh.keep_loaded, u.custom[6]), (200, true, 2), "{k}");
            let r = row(RowId::DetailDistance).unwrap();
            assert_eq!(r.tiers[(r.get)(&u)].name, "Ultra");
        }
        assert!(super::super::graphics_table::row(RowId::KeepLoaded).is_none(), "no separate row");
        // At native, normalize changes nothing.
        for d in 1..=5 {
            let mut n = Settings::default();
            n.custom = custom_presets(d);
            let before = n.clone();
            super::super::graphics_table::normalize(&mut n);
            assert_eq!(n, before, "detail {d}");
        }
    }

    /// An enhanced tier or preset raises a lower Customize Display
    /// value to World Detail 5's; Restore Native and the Original preset
    /// give the player's own value back, also after options.txt is saved
    /// and loaded in between.
    #[test]
    fn texture_pack_rows_are_greyed_until_generated() {
        // Tests run without a generated pack (no data home pack) and
        // without NFSU2_WORLD_TEXTURE_PACK in the options' process view.
        if std::env::var_os("NFSU2_WORLD_TEXTURE_PACK").is_some()
            || crate::upscale_pack::generated()
            || crate::upscale_pack::imported()
        {
            return;
        }
        let s = Settings::default();
        let image = rows(Category::GraphicsGroup(GraphicsGroup::Image), &s, 0, false);
        let at = image.iter().position(|r| r.id == RowId::TexturePack).unwrap();
        assert!(!image[at].enabled);
        assert_eq!(image[at].value, Some(Value::Owned("Not Generated".into())));
        assert_eq!(image[at + 1].id, RowId::GenerateTexturePack);
        assert!(image[at + 1].enabled && image[at + 1].kind == Kind::Button);
        assert!(
            detail(Category::GraphicsGroup(GraphicsGroup::Image), RowId::GenerateTexturePack, &s)
                .unwrap()
                .contains("own world and car textures")
        );
        assert_eq!(clone_label(TEXTURE_PACK), Some("World Texture Pack"));
        // Off is native: nothing written, presets keep the player's choice.
        let mut e = crate::qol::Enhancements::default();
        let mut text = String::new();
        e.write(&mut text);
        assert!(!text.contains("texture_pack"));
        e.texture_pack = 1;
        crate::qol::Preset::Ultra.apply(&mut e);
        assert_eq!(e.texture_pack, 1);
        crate::qol::Preset::Original.apply(&mut e);
        assert_eq!(e.texture_pack, 1);
    }

    #[test]
    fn restore_native_and_the_original_preset_keep_the_controller_layout() {
        use crate::pad::extras::{LAYOUT_AUTO, LAYOUT_MODERN, LAYOUT_ORIGINAL};
        for layout in [LAYOUT_AUTO, LAYOUT_MODERN, LAYOUT_ORIGINAL] {
            let mut s = Settings::default();
            s.enh.pad.layout = layout;
            s.enh.pad.rumble = 70;
            s.restore_native();
            assert_eq!(
                (s.enh.pad.layout, s.enh.pad.rumble),
                (layout, 0),
                "Restore Native: the layout stays, the feel options go native"
            );
            s.enh.ssao = 50;
            crate::qol::Preset::Original.apply(&mut s.enh);
            assert_eq!(s.enh.pad.layout, layout, "the Original preset leaves the layout");
        }
        assert_eq!(Settings::default().enh.pad.layout, LAYOUT_AUTO);
    }

    #[test]
    fn restore_native_sets_the_detail_levels_native_display() {
        use crate::qol::Preset;
        // Review 2026-11-06: Restore Native / the Original preset give the
        // original's Customize Display values for the World Detail level.
        for d in 1..=5u8 {
            let mut s = Settings::default();
            s.detail = d;
            s.custom = custom_presets(1);
            s.custom[2] = 0; // stale low values (a player report)
            s.custom_raised[2] = Some(0);
            for _ in 0..3 {
                change(RowId::GraphicsPreset, &mut s, 0, true, false);
            }
            assert_eq!(s.enh.preset(), Some(Preset::Ultra));
            let mut t = Settings::from_text(&s.options_text());
            t.restore_native();
            assert_eq!(t.custom, custom_presets(d), "detail {d}");
            assert_eq!(t.custom_raised, [None; 22]);
            assert!(!t.options_text().contains("enh.raised"));
            // The Original preset does the same.
            for _ in 0..8 {
                change(RowId::GraphicsPreset, &mut s, 0, true, false);
                if s.enh.preset() == Some(Preset::Original)
                    && !super::super::settings_page::preset_value(&s).starts_with("60")
                {
                    break;
                }
            }
            assert_eq!((s.enh.preset(), s.custom), (Some(Preset::Original), custom_presets(d)), "detail {d}");
        }
        // Detail 5 has the car shadows on (the report: "native has no shadows").
        assert_eq!(custom_presets(5)[2], 2);
        assert!(!Settings::default().options_text().contains("enh.raised"));
    }

    /// The presets: Original is native; each one round-trips through
    /// options.txt and is recognised; the preset row cycles them and
    /// leaves the QoL values alone.
    #[test]
    fn graphics_presets_cycle_and_round_trip() {
        use crate::qol::{Enhancements, PRESETS, Preset};
        let mut s = Settings::default();
        s.enh.fov_offset = 10;
        s.enh.hd_ui = true;
        assert_eq!(s.enh.preset(), Some(Preset::Original));
        let mut seen = Vec::new();
        for _ in 0..4 {
            change(RowId::GraphicsPreset, &mut s, 0, true, false);
            seen.push(s.enh.preset().unwrap());
            assert_eq!((s.enh.fov_offset, s.enh.hd_ui), (10, true), "QoL values kept");
        }
        // Balanced, High, Ultra, Extreme (then the 60 FPS entry, Original).
        assert_eq!(seen, [Preset::Clean, Preset::Modern, Preset::Ultra, Preset::Extreme]);
        change(RowId::GraphicsPreset, &mut s, 0, false, false);
        assert_eq!(s.enh.preset(), Some(Preset::Ultra));
        // Presets only use rows the group pages list, and each is a
        // superset of the one before (more deviations).
        let mut last = 0;
        for p in PRESETS {
            let mut e = Enhancements::default();
            p.apply(&mut e);
            assert_eq!(e.preset(), Some(p));
            assert!(!e.reversed_z && e.anisotropy == 0 && e.reflection_rate == 0, "{p:?} sets a removed row");
            // Every preset value is a tier of its row.
            let mut t = Settings { enh: e, custom: custom_presets(5), ..Settings::default() };
            let before = t.clone();
            super::super::graphics_table::normalize(&mut t);
            assert_eq!(t, before, "{p:?} values are tiers at World Detail 5");
            for r in super::super::graphics_table::ROWS {
                let i = (r.get)(&t);
                (r.set)(&mut t, i);
                assert_eq!(t, before, "{p:?} {:?} tier value stable", r.id);
            }
            let n = e.deviations().len();
            assert!(n >= last, "{p:?}");
            last = n;
            let mut text = String::new();
            e.write(&mut text);
            let mut back = Enhancements::default();
            for line in text.lines() {
                let (k, v) = line.split_once(' ').unwrap();
                assert!(back.parse(k, v));
            }
            assert_eq!(back, e, "{p:?} round trip");
        }
        // A changed row makes it Custom; from Custom, Right gives Original.
        s.enh.sharpen = 25;
        assert_eq!(s.enh.preset(), None);
        change(RowId::GraphicsPreset, &mut s, 0, true, false);
        assert_eq!(s.enh.preset(), Some(Preset::Original));
        assert_eq!(s.enh.sharpen, 0);
    }

    /// A removed row (audit) that a saved options.txt has on is listed
    /// in its group until switched off, and options.txt keeps its key.
    #[test]
    fn removed_rows_show_while_on_and_switch_off() {
        let mut s = Settings::default();
        assert!(s.enh.parse("enh.reversed_z", "1"));
        let g = GraphicsGroup::Performance;
        let mut o = OptionsScreen::new(Category::GraphicsGroup(g), &s, false, 0.0);
        let list = rows(Category::GraphicsGroup(g), &s, 0, false);
        o.selected = list.iter().position(|r| r.id == RowId::ReversedZ).expect("listed while on");
        assert_eq!(o.key(Key::Right, &mut s, false, true, 0.0), OptionsKey::Changed);
        assert!(!rows(Category::GraphicsGroup(g), &s, 0, false).iter().any(|r| r.id == RowId::ReversedZ));
        assert!(o.selected < rows(Category::GraphicsGroup(g), &s, 0, false).len());
        assert!(s.enh.is_native());
    }

    /// Lists Open on Installed: an Enhancements (QoL) row after Crash
    /// Reports, Off first, toggled by Left / Right, reset by Restore Native.
    #[test]
    fn lists_open_installed_row_toggles_and_restores() {
        let mut s = Settings::default();
        let before = s.deviations();
        let ids: Vec<RowId> = qol_rows(&s).iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            [
                RowId::FovOffset,
                RowId::HudScale,
                RowId::HudLayout,
                RowId::FrameCap,
                RowId::DurableSaves,
                RowId::SaveBackups,
                RowId::CrashReports,
                RowId::MarkSpot,
                RowId::ListsOpenInstalled,
                RowId::OutrunVariety,
                RowId::FixModels,
                RowId::TrafficDensity,
                RowId::PhotoMode,
                RowId::SkipMovies,
                RowId::SkipTitle,
                RowId::LowLatency,
                RowId::PowerSaving,
                RowId::ReplayCameras,
                RowId::HudPalette,
                RowId::HudMinimap,
                RowId::HudSpeedo,
                RowId::HudInfo,
                RowId::ReducedFlashing,
                RowId::PerfOverlay,
                RowId::OverlayPosition,
                RowId::OverlayScale,
                RowId::OverlayLog,
                RowId::ReloadPacks,
                RowId::PadLayout,
                RowId::PadDeadzone,
                RowId::PadTriggerDeadzone,
                RowId::PadSaturation,
                RowId::PadSteerCurve,
                RowId::PadRumble,
                RowId::PadPauseOnDisconnect,
                RowId::PadSafeBind
            ]
        );
        let value = |s: &Settings| qol_rows(s).into_iter().find(|r| r.id == RowId::ListsOpenInstalled).unwrap().value;
        assert_eq!(value(&s), on_off(false));
        change(RowId::ListsOpenInstalled, &mut s, 0, true, false);
        assert!(s.enh.lists_open_installed);
        assert_eq!(value(&s), on_off(true));
        assert_eq!(s.deviations().last().map(String::as_str), Some("lists open on installed"));
        assert_eq!(s.deviations().len(), before.len() + 1);
        change(RowId::ListsOpenInstalled, &mut s, 0, false, false);
        assert!(!s.enh.lists_open_installed);
        s.enh.lists_open_installed = true;
        s.restore_native();
        assert!(s.enh.is_native() && s.deviations().is_empty());
        assert_eq!(clone_label(LISTS_OPEN_INSTALLED), Some("Lists Open on Installed"));
    }

    /// Outrun Variety: an Enhancements (QoL) row after Lists Open on
    /// Installed, Off first, toggled by Left / Right, reset by Restore
    /// Native.
    #[test]
    fn outrun_variety_row_toggles_and_restores() {
        let mut s = Settings::default();
        let value = |s: &Settings| qol_rows(s).into_iter().find(|r| r.id == RowId::OutrunVariety).unwrap().value;
        assert_eq!(value(&s), on_off(false));
        change(RowId::OutrunVariety, &mut s, 0, true, false);
        assert!(s.enh.outrun_variety);
        assert_eq!(value(&s), on_off(true));
        assert_eq!(s.deviations().last().map(String::as_str), Some("outrun variety"));
        change(RowId::OutrunVariety, &mut s, 0, false, false);
        assert!(!s.enh.outrun_variety);
        s.enh.outrun_variety = true;
        s.restore_native();
        assert!(s.enh.is_native() && s.deviations().is_empty());
        assert_eq!(clone_label(OUTRUN_VARIETY), Some("Outrun Variety"));
    }

    /// Fix Original Model Errors: an Enhancements row after Lists Open on
    /// Installed, Off first, toggled by Left / Right, reset by Restore
    /// Native.
    #[test]
    fn fix_models_row_toggles_and_restores() {
        let mut s = Settings::default();
        let value = |s: &Settings| qol_rows(s).into_iter().find(|r| r.id == RowId::FixModels).unwrap().value;
        assert_eq!(value(&s), on_off(false));
        change(RowId::FixModels, &mut s, 0, true, false);
        assert!(s.enh.fix_models);
        assert_eq!(value(&s), on_off(true));
        assert_eq!(s.deviations().last().map(String::as_str), Some("model fixes"));
        s.restore_native();
        assert!(!s.enh.fix_models && s.enh.is_native());
        assert_eq!(clone_label(FIX_MODELS), Some("Fix Original Model Errors"));
    }

    /// Traffic Density: an Enhancements row after Fix Original Model
    /// Errors, Native / More / Dense, wrapping; kept by the graphics
    /// presets (gameplay), reset by Restore Native.
    #[test]
    fn traffic_density_row_steps_and_restores() {
        let mut s = Settings::default();
        let value = |s: &Settings| qol_rows(s).into_iter().find(|r| r.id == RowId::TrafficDensity).unwrap().value;
        assert_eq!(value(&s), Some(Value::Label(NATIVE)));
        change(RowId::TrafficDensity, &mut s, 0, true, false);
        assert_eq!((s.enh.traffic_density, value(&s)), (1, Some(Value::Owned("More".into()))));
        change(RowId::TrafficDensity, &mut s, 0, true, false);
        assert_eq!(s.enh.traffic_density, 2);
        crate::qol::Preset::Ultra.apply(&mut s.enh);
        assert_eq!(s.enh.traffic_density, 2, "a graphics preset keeps it");
        change(RowId::TrafficDensity, &mut s, 0, true, false);
        assert_eq!(s.enh.traffic_density, 0);
        change(RowId::TrafficDensity, &mut s, 0, false, false);
        assert_eq!(s.enh.traffic_density, 2);
        s.restore_native();
        assert!(s.enh.traffic_density == 0 && s.enh.is_native());
        assert_eq!(clone_label(TRAFFIC_DENSITY), Some("Traffic Density"));
        assert_eq!(clone_label(SMOOTH_CARS), Some("Smooth Car Shapes"));
    }

    /// The combined interface/map toggle lives on its own page and resets off.
    #[test]
    fn ui2d_rows_toggle_and_restore() {
        let mut s = Settings::default();
        // The HD UI row and its pack state on the settings page's Packs group.
        let packs = rows(Category::Settings(super::super::settings_model::UxGroup::Packs), &s, 0, false);
        assert!(
            packs.iter().any(|r| r.id == RowId::HdUi)
                && packs.iter().any(|r| r.id == RowId::HdUiPackStatus && !r.enabled)
        );
        let rows_now = |s: &Settings| rows(Category::HdUi, s, 0, false);
        assert!(rows_now(&s).iter().all(|r| r.label != 0xC1_04E0_21));
        for id in [RowId::HdUi] {
            let r = rows_now(&s).into_iter().find(|r| r.id == id).expect("row listed");
            assert!(matches!(r.value, Some(Value::Label(OFF))));
            change(id, &mut s, 0, true, false);
            let r = rows_now(&s).into_iter().find(|r| r.id == id).unwrap();
            assert!(matches!(r.value, Some(Value::Label(ON))));
        }
        assert!(s.enh.hd_ui);
        assert_eq!(clone_label(HD_UI), Some("HD UI + Maps"));
        assert_eq!(clone_label(Category::HdUi.title()), Some("HD UI"));
        s.restore_native();
        assert!(!s.enh.hd_ui);
    }

    #[test]
    fn hd_ui_missing_pack_and_restart_status_remain_informational() {
        for requested in [false, true] {
            let rows = hd_ui_rows(requested, false, "Pack missing");
            assert_eq!(
                rows.iter().map(|r| r.id).collect::<Vec<_>>(),
                [RowId::HdUi, RowId::HdUiPackStatus, RowId::HdUiActivationStatus]
            );
            assert!(rows[0].enabled, "the saved choice must remain editable without a pack");
            assert_eq!(rows[0].value, on_off(requested));
            assert_eq!(rows[1].value, Some(Value::Text("Pack missing or incomplete")));
            assert!(rows[1..].iter().all(|r| !r.enabled));
            let mut s = Settings::default();
            s.enh.hd_ui = requested;
            change(RowId::HdUi, &mut s, 0, true, false);
            assert_eq!(s.enh.hd_ui, !requested);
            for id in [RowId::HdUiPackStatus, RowId::HdUiActivationStatus, RowId::HdUiPage] {
                change(id, &mut s, 0, true, false);
            }
            assert_eq!(s.enh.hd_ui, !requested, "informational rows do not change the choice");
            let pending = hd_ui_rows(requested, true, "Restart required");
            assert_eq!(pending[1].value, Some(Value::Text("Installed")));
            assert_eq!(pending[2].value, Some(Value::Text("Restart required")));
            assert!(!pending[2].enabled);
        }
    }

    #[test]
    fn defaults_follow_the_profile_constructor() {
        let s = Settings::default();
        assert_eq!(s.menu_music, 0.7);
        assert!(s.stereo && s.rearview && s.jump_camera && !s.metric);
        assert_eq!((s.explore_map, s.race_map), (2, 2));
        assert_eq!(s.players[0].camera, 3);
        assert!(s.players[0].gauges && !s.players[0].manual);
    }

    #[test]
    fn values_cycle_like_the_item_handlers() {
        let mut s = Settings::default();
        // A new profile: Medium (0x515B15); left steps down to Easy, then
        // wraps to Hard.
        assert_eq!(s.difficulty, 1);
        change(RowId::Difficulty, &mut s, 0, false, true);
        assert_eq!(s.difficulty, 0);
        change(RowId::Difficulty, &mut s, 0, false, true);
        assert_eq!(s.difficulty, 2);
        change(RowId::Camera, &mut s, 0, true, true);
        assert_eq!(s.players[0].camera, 0);
        // Save: a new profile prompts (0x515B02); Prompt → (right) Off,
        // Off → Autosave with a profile, Prompt without.
        assert!(s.save_prompts && !s.autosave);
        change(RowId::Save, &mut s, 0, true, true);
        assert!(!s.autosave && !s.save_prompts);
        change(RowId::Save, &mut s, 0, true, true);
        assert!(s.autosave && s.save_prompts);
        let mut t = Settings { save_prompts: false, ..Settings::default() };
        change(RowId::Save, &mut t, 0, true, false);
        assert!(!t.autosave && t.save_prompts);
        // Sliders step by 0.05 within 0–1.
        change(RowId::Master, &mut s, 0, true, true);
        assert_eq!(s.master, 1.0);
        change(RowId::Master, &mut s, 0, false, true);
        assert!((s.master - 0.95).abs() < 1e-6);
        // Resolution stops at the ends.
        s.resolution = (640, 480);
        change(RowId::Resolution, &mut s, 0, false, true);
        assert_eq!(s.resolution, (640, 480));
        change(RowId::Resolution, &mut s, 0, true, true);
        assert_eq!(s.resolution, (800, 600));
    }

    #[test]
    fn the_camera_button_wraps_at_far() {
        let mut s = Settings::default();
        s.cycle_camera(0, true, false);
        assert_eq!(s.players[0].camera, 0);
        s.cycle_camera(0, false, false);
        assert_eq!(s.players[0].camera, 3);
        s.cycle_camera(0, true, true);
        assert_eq!(s.players[0].camera, 4);
    }

    /// 0x4FF0CC / 0x532569 put Super Far on for a drift race; the other
    /// set-ups (0x4FEFE0, 0x4FF05F, 0x4FF146, 0x4FF1BF, 0x53257E) turn a 4
    /// back into Far and leave 0–3 alone.
    #[test]
    fn drift_races_take_super_far_and_the_rest_give_it_back() {
        let mut s = Settings::default();
        s.players[0].camera = 1;
        s.race_camera(false);
        assert_eq!(s.players[0].camera, 1);
        s.race_camera(true);
        assert_eq!(s.players[0].camera, 4);
        s.race_camera(false);
        assert_eq!(s.players[0].camera, 3);
    }

    #[test]
    fn selection_skips_disabled_rows() {
        let mut s = Settings::default();
        let mut screen = OptionsScreen::new(Category::Player, &s, false, 0.0);
        for _ in 0..20 {
            screen.key(Key::Down, &mut s, false, true, 0.0);
        }
        // The last row (Ask for Transmission) past the disabled Feedback.
        assert_eq!(screen.selected, 11);
        assert_eq!(screen.top, 4);
    }
}

#[cfg(test)]
mod detail_tests {
    use super::estimate_detail;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn detail_follows_the_card_and_cpu() {
        // An unlisted card on a fast CPU: 0.2 + 3.2.
        assert_eq!(estimate_detail(0x10DE, 0x2684, 4000), 3);
        // A GeForce 6800 (score 5): 1.0 + 3.2.
        assert_eq!(estimate_detail(0x10DE, 0x0045, 3000), 4);
        // A GeForce FX 5600 (0x0312, score 4) is held at 3; Intel is 0.
        assert_eq!(estimate_detail(0x10DE, 0x0312, 3000), 3);
        assert_eq!(estimate_detail(0x8086, 0x1234, 3000), 0);
        assert_eq!(estimate_detail(0x1002, 0x4E48, 1000), 1);
    }
}

#[cfg(test)]
mod input_tests {
    use super::super::screens::{Jukebox, Key};
    use super::{
        CUSTOM_CAPS, Category, DEFAULT_KEYS, Kind, Pause, RowId, Settings, change, cpu_mhz, custom_presets,
        estimate_detail, rows, rows_in,
    };

    #[test]
    fn the_detail_presets_give_the_detail_switches() {
        use crate::render::postfx::Switches;
        for d in 0..=5 {
            assert_eq!(Switches::from_custom(&custom_presets(d)), Switches::from_detail(d), "detail {d}");
        }
    }

    #[test]
    fn default_keys_follow_the_games_table() {
        // DIACTION table 0x7FCAC0: action 1 (Throttle) Up then W, action
        // 12 (Pause) Esc then P.
        assert_eq!(DEFAULT_KEYS[2], ["Up", "W"]);
        assert_eq!(DEFAULT_KEYS[17], ["Esc", "P"]);
        assert_eq!(Settings::default().keys[0], ["Left".to_string(), "A".to_string()]);
    }

    #[test]
    fn rebinding_moves_a_key_off_its_old_entry() {
        // 0x5B8560: W (Throttle's secondary) bound as Steering (Left)'s
        // secondary leaves Throttle with Up alone.
        let mut s = Settings::default();
        s.bind(0, 1, "W");
        assert_eq!(s.keys[0], ["Left".to_string(), "W".to_string()]);
        assert_eq!(s.keys[2], ["Up".to_string(), String::new()]);
        // A row's own secondary bound as its primary: the primary takes
        // it and the secondary empties.
        s.bind(1, 0, "D");
        assert_eq!(s.keys[1], ["D".to_string(), String::new()]);
        // Its own primary bound as its secondary: cleared from the
        // primary, which then takes the secondary's key back.
        s.bind(3, 1, "Down");
        assert_eq!(s.keys[3], ["Down".to_string(), String::new()]);
    }

    #[test]
    fn pause_rows_follow_the_pause_builders() {
        let s = Settings::default();
        let ids = |c, p| rows_in(c, &s, 0, true, Some(p)).iter().map(|r| r.id).collect::<Vec<_>>();
        let roam = Pause { career_roam: true, ..Pause::default() };
        assert_eq!(ids(Category::Audio, roam).len(), 7);
        assert_eq!(ids(Category::Gameplay, roam)[0], RowId::Difficulty);
        assert!(!ids(Category::Gameplay, Pause::default()).contains(&RowId::Difficulty));
        assert_eq!(
            ids(Category::Player, roam),
            [
                RowId::Transmission,
                RowId::Stability,
                RowId::Camera,
                RowId::Feedback,
                RowId::Gauges,
                RowId::Position,
                RowId::LapInfo,
                RowId::SplitTime,
                RowId::Leaderboard,
                RowId::Score
            ]
        );
        let drag = Pause { race: true, drag: true, test_run: true, ..Pause::default() };
        assert_eq!(ids(Category::Player, drag), [RowId::Stability, RowId::Camera, RowId::Feedback, RowId::Gauges]);
    }

    #[test]
    fn custom_display_follows_the_detail_presets() {
        assert_eq!(custom_presets(1), {
            let mut v = [0; 22];
            v[20] = 1;
            v
        });
        let top = custom_presets(5);
        assert_eq!((top[0], top[7], top[20], top[14]), (3, 3, 2, CUSTOM_CAPS[14]));
        let mut s = Settings::default();
        change(RowId::Detail, &mut s, 0, false, false);
        assert_eq!(s.custom, custom_presets(4));
        // Road Feedback's slider runs backwards over 2–20 by 0.1.
        let before = s.players[0].road_feedback;
        change(RowId::RoadFeedback, &mut s, 0, true, false);
        assert!(s.players[0].road_feedback < before);
        for _ in 0..20 {
            change(RowId::RoadFeedback, &mut s, 0, true, false);
        }
        assert!((s.players[0].road_feedback - 2.0).abs() < 1e-4);
    }

    /// 0x4FF480 → 0x49B752: Right at World Detail 5 and
    /// Left at 1 report "not changed" and leave Customize Display alone.
    #[test]
    fn detail_end_stops_keep_customize_display() {
        let mut s = Settings::default();
        s.custom[11] = 0;
        let tuned = s.custom;
        change(RowId::Detail, &mut s, 0, true, false);
        assert_eq!((s.detail, s.custom), (5, tuned));
        s.detail = 1;
        s.custom = custom_presets(1);
        s.custom[2] = 2;
        let tuned = s.custom;
        change(RowId::Detail, &mut s, 0, false, false);
        assert_eq!((s.detail, s.custom), (1, tuned));
    }

    /// 0x5CECBE, 0x4FF550, 0x4FF480, 0x5BEA30: detail 0
    /// is stored and reloaded as 0, drawn as 1; Left there does nothing,
    /// Right gives 2 with its presets.
    #[test]
    fn detail_zero_is_kept_and_shown_as_one() {
        let mut s = Settings::default();
        s.estimated_detail(0);
        assert_eq!((s.detail, s.custom), (0, custom_presets(1)));
        let back = Settings::from_text(&s.options_text());
        assert_eq!((back.detail, back.custom), (0, s.custom));
        let row = rows(Category::Display, &s, 0, false).into_iter().find(|r| r.id == RowId::Detail).unwrap();
        assert!(matches!(row.kind, Kind::Slider { value, .. } if value == 1.0));
        let before = s.clone();
        change(RowId::Detail, &mut s, 0, false, false);
        assert!(s == before, "Left at 0: nothing");
        change(RowId::Detail, &mut s, 0, true, false);
        assert_eq!((s.detail, s.custom), (2, custom_presets(2)));
    }

    /// 0x5CECA0 → 0x5BE880: the first run's detail
    /// estimate comes with its Customize Display values, Vsync off, the
    /// display mode kept; options.txt then holds both.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn first_run_display_sets_detail_and_custom_values() {
        let mut s = Settings::default();
        s.custom[21] = 1;
        s.custom_raised[0] = Some(3);
        // An Intel card: estimate 0 whatever the CPU.
        s.first_run_display((0x8086, 0x1234));
        assert_eq!((s.detail, s.custom, s.custom_raised), (0, custom_presets(0), [None; 22]));
        assert_eq!(s.custom[21], 0);
        let d = Settings::default();
        assert_eq!(
            (s.resolution, s.fullscreen, s.aspect, s.vsync_off_mailbox),
            (d.resolution, d.fullscreen, d.aspect, d.vsync_off_mailbox)
        );
        let back = Settings::from_text(&s.options_text());
        assert_eq!((back.detail, back.custom), (s.detail, s.custom));
        // An unlisted card: the estimate for this CPU, with its presets.
        let mut t = Settings::default();
        t.first_run_display((0x1002, 0x744C));
        assert_eq!(t.detail, estimate_detail(0x1002, 0x744C, cpu_mhz()));
        assert_eq!(t.custom, custom_presets(t.detail));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn trax_states_wrap_both_ways() {
        // 0x53DEC0: left 0 → 3, right 3 → 0.
        let mut trax = [0u8; 27];
        let mut j = Jukebox::new(0.0);
        assert!(j.key(Key::Left, &mut trax, 0.0));
        assert_eq!(trax[0], 3);
        assert!(j.key(Key::Right, &mut trax, 0.0));
        assert_eq!(trax[0], 0);
        for _ in 0..5 {
            j.key(Key::Down, &mut trax, 0.0);
        }
        assert_eq!((j.selected, j.top), (5, 2));
    }
}

/// unwrap audit: options.txt missing, unreadable, cut off or full of
/// junk loads as the defaults (each bad line keeps its default), and a
/// folder that cannot be written is an error the callers ignore (the game
/// goes on with the options in memory), never a panic.
#[cfg(test)]
mod file_tests {
    use super::Settings;

    fn sandboxed(tag: &str) -> std::path::PathBuf {
        crate::frontend::profiles::sandbox::enter(
            &std::env::temp_dir().join(format!("nfsu2-options-file-{tag}-{}", std::process::id())),
        )
    }

    #[test]
    fn missing_and_junk_options_load_as_defaults() {
        std::thread::spawn(|| {
            sandboxed("junk");
            assert!(!Settings::path().exists());
            let d = Settings::default().options_text();
            assert_eq!(Settings::load_text().options_text(), d, "missing");
            let path = Settings::path();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            for junk in [
                &b""[..],
                b"\xff\xfe\x00 not utf-8 \x80",
                b"custom.\ncustom.99 3\ncustom.x 1\nkey.\nkey.1\nkey.99.0 A\nkey.1.9 A\nenh.raised.\nenh.raised.99 1\nenh.\nqr.\nqr.99 1 2 3\nqr.0 1 2\np\np9.camera 1\np0.\nresolution x\nresolution 99999999999x1\ndetail 255\ntrax 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9 9\n",
                b"resolution 1280x9\nenh.fov x\nenh.car_reflection 0\nenh.anisotropy 4294967295\nenh.render_scale -1\nenh.hdr_peak 99999\n",
            ] {
                std::fs::write(&path, junk).unwrap();
                let s = Settings::load_text();
                assert!((0..=5).contains(&s.detail), "{junk:?}");
            }
            // Cut off mid-line: the whole lines before it still apply.
            let mut t = Settings::default();
            t.resolution = (1920, 1080);
            let text = t.options_text();
            std::fs::write(&path, &text[..text.find("custom.3").unwrap() + 5]).unwrap();
            assert_eq!(Settings::load_text().resolution, (1920, 1080));
            // A size below the original's 640 × 480 is raised to it.
            for (line, want) in [("resolution 320x240\n", (640, 480)), ("resolution 1280x9\n", (1280, 480))] {
                std::fs::write(&path, line).unwrap();
                assert_eq!(Settings::load_text().resolution, want, "{line}");
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn options_write_whole_or_fail_without_a_panic() {
        std::thread::spawn(|| {
            let home = sandboxed("write");
            let mut s = Settings::default();
            s.resolution = (1600, 1200);
            s.write_file().unwrap();
            let path = Settings::path();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), s.options_text());
            assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1, "no temporary file left");
            assert_eq!(Settings::load_text().resolution, (1600, 1200));
            // A file where the folder goes: the write fails, nothing panics.
            std::fs::remove_dir_all(home.join("overbright")).unwrap();
            std::fs::write(home.join("overbright"), b"in the way").unwrap();
            assert!(s.write_file().is_err());
            assert_eq!(Settings::load_text().options_text(), Settings::default().options_text());
        })
        .join()
        .unwrap();
    }
}

#[cfg(test)]
mod overlay_row_tests {
    use super::*;

    /// docs/perf-overlay.md: the overlay's four rows after Reduced
    /// Flashing, their labels known and unique, values cycled, saved in
    /// options.txt, reset by Restore Native, and no deviation.
    #[test]
    fn perf_overlay_rows_cycle_save_and_restore() {
        let mut s = Settings::default();
        let ids = [RowId::PerfOverlay, RowId::OverlayPosition, RowId::OverlayScale, RowId::OverlayLog];
        let labels = [PERF_OVERLAY, OVERLAY_POSITION, OVERLAY_SCALE, OVERLAY_LOG];
        let r = qol_rows(&s);
        let at = r.iter().position(|r| r.id == RowId::ReducedFlashing).unwrap();
        assert_eq!(r[at + 1..at + 5].iter().map(|r| r.id).collect::<Vec<_>>(), ids);
        assert_eq!(r[at + 1..at + 5].iter().map(|r| r.label).collect::<Vec<_>>(), labels);
        assert!(labels.iter().all(|l| clone_label(*l).is_some() && (0xC1_04E0_EC..=0xC1_04E0_EF).contains(l)));
        assert_eq!(r[at + 1].value, Some(Value::Label(OFF)));
        let before = s.deviations();
        change(RowId::PerfOverlay, &mut s, 0, true, false);
        change(RowId::PerfOverlay, &mut s, 0, true, false);
        change(RowId::OverlayPosition, &mut s, 0, false, false);
        change(RowId::OverlayScale, &mut s, 0, true, false);
        change(RowId::OverlayLog, &mut s, 0, true, false);
        assert_eq!(s.overlay, crate::overlay::Settings { layout: 2, position: 3, scale: 125, csv: true, hz: 4 });
        let value = |s: &Settings, id| qol_rows(s).into_iter().find(|r| r.id == id).unwrap().value;
        assert_eq!(value(&s, RowId::PerfOverlay), Some(Value::Owned("Standard".into())));
        assert_eq!(value(&s, RowId::OverlayPosition), Some(Value::Text("Bottom Right")));
        assert_eq!(value(&s, RowId::OverlayScale), Some(Value::Owned("125%".into())));
        assert!(detail(Category::Enhancements, RowId::PerfOverlay, &s).is_some_and(|d| d.contains("F3")));
        // A tool, not a deviation from the original.
        assert_eq!(s.deviations(), before);
        let text = s.options_text();
        assert!(text.contains("overlay.layout 2\n") && text.contains("overlay.csv 1\n"));
        assert_eq!(Settings::from_text(&text).overlay, s.overlay);
        assert!(!Settings::default().options_text().contains("overlay."));
        s.restore_native();
        assert_eq!(s.overlay, crate::overlay::Settings::default());
    }
}
