//! Oracles for the generated settings table: the hand-written `Enhancements` writer, parser, deviation
//! list, `apply_now` and preset keep-list as they were before the option
//! table (src/qol/enhancements.rs, atomics.rs, presets.rs before the table was generated),
//! verbatim but for the names and the recording of `apply_now`'s stores.
//! The table's generated code is compared against them
//! (`super::table_tests`); they go with the next step.

#![allow(clippy::all)]

use super::super::atomics::*;
use super::super::crash;
use super::super::*;

macro_rules! old_store {
    ($a:ident, $v:expr) => {{
        let v = $v;
        record(stringify!($a), &v);
    }};
}

macro_rules! old_call {
    ($name:expr, $v:expr, $call:expr) => {{
        record($name, &$v);
        $call;
    }};
}

pub(super) trait Old {
    fn old_deviations(&self) -> Vec<String>;
    fn old_write(&self, out: &mut String);
    fn old_parse(&mut self, key: &str, value: &str) -> bool;
    fn old_apply_now(&self);
}

impl Old for Enhancements {
    /// The deviations switched on, for the F3 overlay and crash reports.
    fn old_deviations(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.fov_offset != 0 {
            v.push(format!("FOV {:+}", self.fov_offset));
        }
        if self.hud_scale != 100 {
            v.push(format!("HUD {}%", self.hud_scale));
        }
        if self.hud_full_width {
            v.push("HUD full width".into());
        }
        if self.frame_cap != 0 {
            v.push(format!("cap {} fps", self.frame_cap));
        }
        if self.durable_saves {
            v.push("durable saves".into());
        }
        if self.save_backups != 0 {
            v.push(format!("backups {}", self.save_backups));
        }
        if self.crash_reports {
            v.push("crash reports".into());
        }
        if self.render_scale != 100 && ssaa_scale(self.post_aa).is_none() {
            v.push(format!("render {}%", self.render_scale));
        }
        if let Some(aa) = aa_name(self.post_aa) {
            v.push(aa.into());
        }
        if self.aa_msaa != 0 {
            v.push(format!("MSAA override {}", self.aa_msaa));
        }
        if self.aa_filter != 0 {
            v.push(format!("post filter {}", filter_name(self.aa_filter)));
        }
        if self.sharpen != 0 {
            v.push(format!("sharpen {}%", self.sharpen));
        }
        match self.road_reflection {
            0 => {}
            1 => v.push("road refl. half".into()),
            _ => v.push("road refl. full".into()),
        }
        if self.car_reflection != 128 {
            v.push(format!("car refl. {}", self.car_reflection));
        }
        if self.detail_distance != 100 {
            v.push(format!("detail dist. {}%", self.detail_distance));
        }
        if self.car_detail != 100 {
            v.push(format!("car detail {}%", self.car_detail));
        }
        if self.hd_ui {
            v.push("HD UI + Maps".into());
        }
        if self.reload_packs {
            v.push("pack hot reload".into());
        }
        if self.lod_fade {
            v.push("LOD fade".into());
        }
        if self.keep_loaded {
            v.push("keep all loaded".into());
        }
        if self.anisotropy != 0 {
            v.push(format!("aniso {}x", self.anisotropy));
        }
        if self.reversed_z {
            v.push("reversed-Z".into());
        }
        match self.shadows {
            0 => {}
            1 => v.push("shadows high".into()),
            _ => v.push("shadows ultra".into()),
        }
        if self.enhanced_reflections {
            v.push("enhanced reflections".into());
        }
        if self.reflection_rate != 0 {
            v.push("refl. every frame".into());
        }
        if self.ssao != 0 {
            v.push(format!("SSAO {}%", self.ssao));
        }
        match self.hdr_post {
            0 => {}
            1 => v.push("HP post (half glow)".into()),
            _ => v.push("HP post (full glow)".into()),
        }
        if self.enhanced_paint {
            v.push("enhanced paint".into());
        }
        if self.lists_open_installed {
            v.push("lists open on installed".into());
        }
        if self.outrun_variety {
            v.push("outrun variety".into());
        }
        if self.wet_roads {
            v.push("wet roads".into());
        }
        if self.hdr_output {
            v.push("HDR output".into());
        }
        if self.fix_models {
            v.push("model fixes".into());
        }
        if self.photo_mode {
            v.push("photo mode".into());
        }
        match self.ssr {
            0 => {}
            1 => v.push("SSR half".into()),
            2 => v.push("SSR full".into()),
            _ => v.push("ray-traced reflections".into()),
        }
        if self.dynamic_lamps {
            v.push("dynamic lamps".into());
        }
        match self.lamp_shadows {
            0 => {}
            1 => v.push("lamp shadows low".into()),
            _ => v.push("lamp shadows high".into()),
        }
        if self.contact_shadows {
            v.push("contact shadows".into());
        }
        if let Some(u) = upscale_name(self.upscale) {
            v.push(format!("upscaling {u}"));
        }
        match self.volumetric_fog {
            0 => {}
            1 => v.push("volumetric fog low".into()),
            2 => v.push("volumetric fog high".into()),
            _ => v.push("volumetric fog next-gen".into()),
        }
        if self.sky != 0 {
            v.push("sky next-gen".into());
        }
        match self.particles {
            0 => {}
            1 => v.push("particles high".into()),
            2 => v.push("particles ultra".into()),
            _ => v.push("particles cinematic".into()),
        }
        if self.sparks_fire != 0 {
            v.push("sparks & fire next-gen".into());
        }
        if self.wet_car {
            v.push("wet car".into());
        }
        match self.rain_detail {
            0 => {}
            1 => v.push("rain detail".into()),
            2 => v.push("rain detail high".into()),
            _ => v.push("rain detail next-gen".into()),
        }
        if self.spray {
            v.push("spray and splashes".into());
        }
        if self.lens_drops {
            v.push("lens drops".into());
        }
        match self.wet_interaction {
            0 => {}
            1 => v.push("wet road interaction".into()),
            _ => v.push("wet road interaction high".into()),
        }
        match self.motion_blur {
            0 => {}
            1 => v.push("motion blur camera".into()),
            _ => v.push("motion blur per-object".into()),
        }
        if self.depth_of_field != 0 {
            v.push("depth of field cinematic".into());
        }
        match self.water {
            0 => {}
            1 => v.push("animated water".into()),
            2 => v.push("animated water realistic".into()),
            _ => v.push("animated water next-gen".into()),
        }
        match self.ambient_detail {
            0 => {}
            1 => v.push("ambient detail".into()),
            _ => v.push("ambient detail next-gen".into()),
        }
        match self.headlight_beams {
            0 => {}
            1 => v.push("headlight beams".into()),
            _ => v.push("car lights next-gen".into()),
        }
        if self.light_probes {
            v.push("light probes".into());
        }
        if self.enhanced_glass {
            v.push("enhanced glass".into());
        }
        match self.modern_lighting {
            0 => {}
            1 => v.push(format!("modern lighting low {}%", self.modern_strength)),
            2 => v.push(format!("modern lighting {}%", self.modern_strength)),
            _ => v.push(format!("modern lighting rt {}%", self.modern_strength)),
        }
        match self.surface_detail {
            0 => {}
            1 => v.push("surface detail materials".into()),
            _ => v.push("surface detail materials + normals".into()),
        }
        self.extras.deviations(&mut v);
        self.pad.deviations(&mut v);
        if self.smooth_cars {
            v.push("smooth car shapes".into());
        }
        if self.far_draw {
            v.push("world detail extreme".into());
        }
        match self.traffic_density {
            0 => {}
            1 => v.push("traffic more".into()),
            _ => v.push("traffic dense".into()),
        }
        match self.texture_pack {
            0 => {}
            1 => v.push("texture pack".into()),
            _ => v.push("texture pack lean".into()),
        }
        if self.skips_title() {
            v.push("skip title".into());
        }
        if self.skips_movies() {
            v.push("skip movies".into());
        }
        v
    }

    /// options.txt lines for the values that are not native.
    fn old_write(&self, out: &mut String) {
        let d = Enhancements::default();
        if self.fov_offset != d.fov_offset {
            out.push_str(&format!("enh.fov {}\n", self.fov_offset));
        }
        if self.hud_scale != d.hud_scale {
            out.push_str(&format!("enh.hud_scale {}\n", self.hud_scale));
        }
        if self.frame_cap != d.frame_cap {
            out.push_str(&format!("enh.frame_cap {}\n", self.frame_cap));
        }
        if self.durable_saves != d.durable_saves {
            out.push_str(&format!("enh.durable_saves {}\n", u8::from(self.durable_saves)));
        }
        if self.save_backups != d.save_backups {
            out.push_str(&format!("enh.save_backups {}\n", self.save_backups));
        }
        if self.crash_reports != d.crash_reports {
            out.push_str(&format!("enh.crash_reports {}\n", u8::from(self.crash_reports)));
        }
        if self.render_scale != d.render_scale {
            out.push_str(&format!("enh.render_scale {}\n", self.render_scale));
        }
        if self.post_aa != d.post_aa {
            out.push_str(&format!("enh.post_aa {}\n", self.post_aa));
        }
        if self.aa_msaa != 0 {
            out.push_str(&format!("enh.aa_msaa {}\n", self.aa_msaa));
        }
        if self.aa_filter != 0 {
            out.push_str(&format!("enh.aa_filter {}\n", self.aa_filter));
        }
        if self.sharpen != d.sharpen {
            out.push_str(&format!("enh.sharpen {}\n", self.sharpen));
        }
        if self.road_reflection != d.road_reflection {
            out.push_str(&format!("enh.road_reflection {}\n", self.road_reflection));
        }
        if self.car_reflection != d.car_reflection {
            out.push_str(&format!("enh.car_reflection {}\n", self.car_reflection));
        }
        if self.detail_distance != d.detail_distance {
            out.push_str(&format!("enh.detail_distance {}\n", self.detail_distance));
        }
        if self.car_detail != d.car_detail {
            out.push_str(&format!("enh.car_detail {}\n", self.car_detail));
        }
        if self.hd_ui != d.hd_ui {
            out.push_str(&format!("enh.hd_ui {}\n", u8::from(self.hd_ui)));
        }
        if self.reload_packs != d.reload_packs {
            out.push_str(&format!("enh.reload_packs {}\n", u8::from(self.reload_packs)));
        }
        if self.lod_fade != d.lod_fade {
            out.push_str(&format!("enh.lod_fade {}\n", u8::from(self.lod_fade)));
        }
        if self.keep_loaded != d.keep_loaded {
            out.push_str(&format!("enh.keep_loaded {}\n", u8::from(self.keep_loaded)));
        }
        if self.anisotropy != d.anisotropy {
            out.push_str(&format!("enh.anisotropy {}\n", self.anisotropy));
        }
        if self.reversed_z != d.reversed_z {
            out.push_str(&format!("enh.reversed_z {}\n", u8::from(self.reversed_z)));
        }
        if self.shadows != d.shadows {
            out.push_str(&format!("enh.shadows {}\n", self.shadows));
        }
        if self.enhanced_reflections != d.enhanced_reflections {
            out.push_str(&format!("enh.enhanced_reflections {}\n", u8::from(self.enhanced_reflections)));
        }
        if self.reflection_rate != d.reflection_rate {
            out.push_str(&format!("enh.reflection_rate {}\n", self.reflection_rate));
        }
        if self.ssao != d.ssao {
            out.push_str(&format!("enh.ssao {}\n", self.ssao));
        }
        if self.hdr_post != d.hdr_post {
            out.push_str(&format!("enh.hdr_post {}\n", self.hdr_post));
        }
        if self.enhanced_paint != d.enhanced_paint {
            out.push_str(&format!("enh.enhanced_paint {}\n", u8::from(self.enhanced_paint)));
        }
        if self.lists_open_installed != d.lists_open_installed {
            out.push_str(&format!("enh.lists_open_installed {}\n", u8::from(self.lists_open_installed)));
        }
        if self.outrun_variety != d.outrun_variety {
            out.push_str(&format!("enh.outrun_variety {}\n", u8::from(self.outrun_variety)));
        }
        if self.wet_roads != d.wet_roads {
            out.push_str(&format!("enh.wet_roads {}\n", u8::from(self.wet_roads)));
        }
        if self.hdr_output != d.hdr_output {
            out.push_str(&format!("enh.hdr_output {}\n", u8::from(self.hdr_output)));
        }
        if self.hdr_peak != d.hdr_peak {
            out.push_str(&format!("enh.hdr_peak {}\n", self.hdr_peak));
        }
        if self.hdr_paper != d.hdr_paper {
            out.push_str(&format!("enh.hdr_paper {}\n", self.hdr_paper));
        }
        if self.hdr_gamma != d.hdr_gamma {
            out.push_str(&format!("enh.hdr_gamma {}\n", self.hdr_gamma));
        }
        if self.fix_models != d.fix_models {
            out.push_str(&format!("enh.fix_models {}\n", u8::from(self.fix_models)));
        }
        if self.photo_mode != d.photo_mode {
            out.push_str(&format!("enh.photo_mode {}\n", u8::from(self.photo_mode)));
        }
        if self.ssr != d.ssr {
            out.push_str(&format!("enh.ssr {}\n", self.ssr));
        }
        if self.hud_full_width != d.hud_full_width {
            out.push_str(&format!("enh.hud_full_width {}\n", u8::from(self.hud_full_width)));
        }
        if self.dynamic_lamps != d.dynamic_lamps {
            out.push_str(&format!("enh.dynamic_lamps {}\n", u8::from(self.dynamic_lamps)));
        }
        if self.lamp_shadows != d.lamp_shadows {
            out.push_str(&format!("enh.lamp_shadows {}\n", self.lamp_shadows));
        }
        if self.contact_shadows != d.contact_shadows {
            out.push_str(&format!("enh.contact_shadows {}\n", u8::from(self.contact_shadows)));
        }
        if self.upscale != d.upscale {
            out.push_str(&format!("enh.upscale {}\n", self.upscale));
        }
        if self.volumetric_fog != d.volumetric_fog {
            out.push_str(&format!("enh.volumetric_fog {}\n", self.volumetric_fog));
        }
        if self.sky != d.sky {
            out.push_str(&format!("enh.sky {}\n", self.sky));
        }
        if self.particles != d.particles {
            out.push_str(&format!("enh.particles {}\n", self.particles));
        }
        if self.sparks_fire != d.sparks_fire {
            out.push_str(&format!("enh.sparks_fire {}\n", self.sparks_fire));
        }
        if self.wet_car != d.wet_car {
            out.push_str(&format!("enh.wet_car {}\n", u8::from(self.wet_car)));
        }
        if self.rain_detail != d.rain_detail {
            out.push_str(&format!("enh.rain_detail {}\n", self.rain_detail));
        }
        if self.spray != d.spray {
            out.push_str(&format!("enh.spray {}\n", u8::from(self.spray)));
        }
        if self.lens_drops != d.lens_drops {
            out.push_str(&format!("enh.lens_drops {}\n", u8::from(self.lens_drops)));
        }
        if self.wet_interaction != d.wet_interaction {
            out.push_str(&format!("enh.wet_interaction {}\n", self.wet_interaction));
        }
        if self.motion_blur != d.motion_blur {
            out.push_str(&format!("enh.motion_blur {}\n", self.motion_blur));
        }
        if self.depth_of_field != d.depth_of_field {
            out.push_str(&format!("enh.depth_of_field {}\n", self.depth_of_field));
        }
        if self.water != d.water {
            out.push_str(&format!("enh.water {}\n", self.water));
        }
        if self.ambient_detail != d.ambient_detail {
            out.push_str(&format!("enh.ambient_detail {}\n", self.ambient_detail));
        }
        if self.headlight_beams != d.headlight_beams {
            out.push_str(&format!("enh.headlight_beams {}\n", self.headlight_beams));
        }
        if self.light_probes != d.light_probes {
            out.push_str(&format!("enh.light_probes {}\n", u8::from(self.light_probes)));
        }
        if self.enhanced_glass != d.enhanced_glass {
            out.push_str(&format!("enh.enhanced_glass {}\n", u8::from(self.enhanced_glass)));
        }
        if self.surface_detail != d.surface_detail {
            out.push_str(&format!("enh.surface_detail {}\n", self.surface_detail));
        }
        if self.modern_lighting != d.modern_lighting {
            out.push_str(&format!("enh.modern_lighting {}\n", self.modern_lighting));
        }
        if self.modern_strength != d.modern_strength {
            out.push_str(&format!("enh.modern_strength {}\n", self.modern_strength));
        }
        self.extras.write(out);
        self.pad.write(out);
        if self.smooth_cars != d.smooth_cars {
            out.push_str(&format!("enh.smooth_cars {}\n", u8::from(self.smooth_cars)));
        }
        if self.far_draw != d.far_draw {
            out.push_str(&format!("enh.far_draw {}\n", u8::from(self.far_draw)));
        }
        if self.traffic_density != d.traffic_density {
            out.push_str(&format!("enh.traffic_density {}\n", self.traffic_density));
        }
        if self.texture_pack != d.texture_pack {
            out.push_str(&format!("enh.texture_pack {}\n", self.texture_pack));
        }
        if self.boot_loading_min_ms != d.boot_loading_min_ms {
            out.push_str(&format!("enh.boot_loading_min_ms {}\n", self.boot_loading_min_ms));
        }
        // L1: added with the row (the oracle's copy of it).
        if self.launcher_skip != d.launcher_skip {
            out.push_str(&format!("launcher.skip {}\n", u8::from(self.launcher_skip)));
        }
        if self.skip_title != d.skip_title {
            out.push_str(&format!("enh.skip_title {}\n", self.skip_title));
        }
        if self.skip_movies != d.skip_movies {
            out.push_str(&format!("enh.skip_movies {}\n", u8::from(self.skip_movies)));
        }
    }

    /// An options.txt line's key and value; false when the key is not one
    /// of these. Values out of range are held to the rows' ranges.
    fn old_parse(&mut self, key: &str, value: &str) -> bool {
        let value = value.trim();
        match key {
            "enh.fov" => {
                self.fov_offset = value
                    .parse::<i8>()
                    .map_or(self.fov_offset, |v| v.clamp(FOV_STEPS[0], FOV_STEPS[FOV_STEPS.len() - 1]))
            }
            "enh.hud_scale" => {
                self.hud_scale = value
                    .parse::<u8>()
                    .map_or(self.hud_scale, |v| v.clamp(HUD_STEPS[0], HUD_STEPS[HUD_STEPS.len() - 1]))
            }
            "launcher.skip" => self.launcher_skip = value != "0",
            "enh.skip_title" => self.skip_title = value.parse::<u32>().map_or(self.skip_title, |v| v.min(2) as u8),
            "enh.skip_movies" => self.skip_movies = value != "0",
            "enh.boot_loading_min_ms" => {
                self.boot_loading_min_ms = value
                    .parse::<u32>()
                    .map_or(self.boot_loading_min_ms, |v| v.min(u32::from(BOOT_LOADING_MAX_MS)) as u16)
            }
            "enh.frame_cap" => {
                self.frame_cap =
                    value.parse::<u16>().map_or(self.frame_cap, |v| if v == 0 { 0 } else { v.clamp(10, 1000) })
            }
            "enh.durable_saves" => self.durable_saves = value != "0",
            "enh.save_backups" => {
                self.save_backups =
                    value.parse::<u8>().map_or(self.save_backups, |v| v.min(BACKUP_STEPS[BACKUP_STEPS.len() - 1]))
            }
            "enh.crash_reports" => self.crash_reports = value != "0",
            "enh.render_scale" => {
                self.render_scale = value.parse::<u32>().map_or(self.render_scale, |v| {
                    v.clamp(SCALE_STEPS[0].into(), SCALE_STEPS[SCALE_STEPS.len() - 1].into()) as u8
                })
            }
            "enh.post_aa" => self.post_aa = value.parse::<u32>().map_or(self.post_aa, |v| v.min(u32::from(TAA)) as u8),
            "enh.aa_msaa" => {
                self.aa_msaa = value.parse::<u8>().map_or(self.aa_msaa, |v| if MSAA_STEPS.contains(&v) { v } else { 0 })
            }
            "enh.aa_filter" => self.aa_filter = value.parse::<u8>().map_or(self.aa_filter, |v| v.min(4)),
            "enh.sharpen" => {
                self.sharpen = value.parse::<u32>().map_or(self.sharpen, |v| nearest(&SHARPEN_STEPS, v.min(100) as u8))
            }
            "enh.road_reflection" => {
                self.road_reflection = value.parse::<u32>().map_or(self.road_reflection, |v| v.min(2) as u8)
            }
            // Sizes map by their power of two (512 → 1024, the tie going up).
            "enh.car_reflection" => {
                self.car_reflection = value.parse::<u32>().map_or(self.car_reflection, |v| {
                    let log = v.clamp(128, 1024).next_power_of_two().ilog2() as u8;
                    1 << nearest(&CAR_REFLECTION_STEPS.map(|s| s.ilog2() as u8), log)
                })
            }
            "enh.detail_distance" => {
                self.detail_distance = value
                    .parse::<u32>()
                    .map_or(self.detail_distance, |v| nearest(&DETAIL_STEPS, v.clamp(100, 300) as u16))
            }
            "enh.car_detail" => {
                self.car_detail = value
                    .parse::<u32>()
                    .map_or(self.car_detail, |v| nearest(&CAR_DETAIL_STEPS, v.clamp(100, 300) as u16))
            }
            // The old Vector Fonts row, folded into HD UI Art: a file that
            // still has it reads as before minus that row (never written).
            "enh.vector_fonts" => {}
            "enh.hd_ui" => self.hd_ui = value != "0",
            "enh.reload_packs" => self.reload_packs = value != "0",
            "enh.lod_fade" => self.lod_fade = value != "0",
            "enh.keep_loaded" => self.keep_loaded = value != "0",
            "enh.anisotropy" => {
                self.anisotropy = value
                    .parse::<u32>()
                    .map_or(self.anisotropy, |v| if v < 2 { 0 } else { v.min(16).next_power_of_two().min(16) as u8 })
            }
            "enh.reversed_z" => self.reversed_z = value != "0",
            "enh.shadows" => {
                self.shadows = value.parse::<u32>().map_or(self.shadows, |v| nearest(&SHADOW_STEPS, v.min(2) as u8))
            }
            "enh.enhanced_reflections" => self.enhanced_reflections = value != "0",
            "enh.reflection_rate" => {
                self.reflection_rate = value.parse::<u32>().map_or(self.reflection_rate, |v| v.min(1) as u8)
            }
            "enh.ssao" => {
                self.ssao = value.parse::<u32>().map_or(self.ssao, |v| nearest(&SSAO_STEPS, v.min(100) as u8))
            }
            "enh.hdr_post" => self.hdr_post = value.parse::<u32>().map_or(self.hdr_post, |v| v.min(2) as u8),
            "enh.enhanced_paint" => self.enhanced_paint = value != "0",
            "enh.lists_open_installed" => self.lists_open_installed = value != "0",
            "enh.outrun_variety" => self.outrun_variety = value != "0",
            "enh.wet_roads" => self.wet_roads = value != "0",
            "enh.fix_models" => self.fix_models = value != "0",
            "enh.photo_mode" => self.photo_mode = value != "0",
            "enh.hdr_output" => self.hdr_output = value != "0",
            "enh.hdr_peak" => {
                self.hdr_peak = value
                    .parse::<u32>()
                    .map_or(self.hdr_peak, |v| if v == 0 { 0 } else { nearest(&HDR_PEAK_STEPS, v.min(10000) as u16) })
            }
            "enh.hdr_paper" => {
                self.hdr_paper = value
                    .parse::<u32>()
                    .map_or(self.hdr_paper, |v| if v == 0 { 0 } else { nearest(&HDR_PAPER_STEPS, v.min(1000) as u16) })
            }
            "enh.hdr_gamma" => self.hdr_gamma = value.parse::<u32>().map_or(self.hdr_gamma, |v| v.min(2) as u8),
            "enh.ssr" => self.ssr = value.parse::<u32>().map_or(self.ssr, |v| v.min(3) as u8),
            "enh.dynamic_lamps" => self.dynamic_lamps = value != "0",
            "enh.lamp_shadows" => {
                self.lamp_shadows = value.parse::<u32>().map_or(self.lamp_shadows, |v| v.min(2) as u8)
            }
            "enh.contact_shadows" => self.contact_shadows = value != "0",
            "enh.upscale" => {
                self.upscale = value.parse::<u32>().map_or(self.upscale, |v| v.min(u32::from(UPSCALE_MAX)) as u8)
            }
            "enh.volumetric_fog" => {
                self.volumetric_fog = value.parse::<u32>().map_or(self.volumetric_fog, |v| v.min(3) as u8)
            }
            "enh.sky" => self.sky = value.parse::<u32>().map_or(self.sky, |v| v.min(1) as u8),
            "enh.particles" => self.particles = value.parse::<u32>().map_or(self.particles, |v| v.min(3) as u8),
            "enh.ambient_detail" => {
                self.ambient_detail = value.parse::<u32>().map_or(self.ambient_detail, |v| v.min(2) as u8)
            }
            "enh.sparks_fire" => self.sparks_fire = value.parse::<u32>().map_or(self.sparks_fire, |v| v.min(1) as u8),
            "enh.water" => self.water = value.parse::<u32>().map_or(u8::from(value != "0"), |v| v.min(3) as u8),
            "enh.smooth_cars" => self.smooth_cars = value != "0",
            "enh.far_draw" => self.far_draw = value != "0",
            "enh.traffic_density" => {
                self.traffic_density = value.parse::<u32>().map_or(self.traffic_density, |v| v.min(2) as u8)
            }
            "enh.wet_car" => self.wet_car = value != "0",
            "enh.rain_detail" => {
                self.rain_detail =
                    value.parse::<u32>().map_or(self.rain_detail, |v| v.min(u32::from(RAIN_NEXT_GEN)) as u8)
            }
            "enh.spray" => self.spray = value != "0",
            "enh.lens_drops" => self.lens_drops = value != "0",
            "enh.wet_interaction" => {
                self.wet_interaction = value.parse::<u32>().map_or(self.wet_interaction, |v| v.min(2) as u8)
            }
            "enh.headlight_beams" => {
                self.headlight_beams = value.parse::<u32>().map_or(u8::from(value != "0"), |v| v.min(2) as u8)
            }
            "enh.light_probes" => self.light_probes = value != "0",
            "enh.enhanced_glass" => self.enhanced_glass = value != "0",
            "enh.surface_detail" => {
                self.surface_detail = value.parse::<u32>().map_or(self.surface_detail, |v| v.min(2) as u8)
            }
            "enh.texture_pack" => {
                self.texture_pack = value.parse::<u32>().map_or(self.texture_pack, |v| v.min(2) as u8)
            }
            "enh.modern_lighting" => {
                self.modern_lighting = value.parse::<u32>().map_or(self.modern_lighting, |v| v.min(3) as u8)
            }
            "enh.modern_strength" => {
                self.modern_strength =
                    value.parse::<u32>().map_or(self.modern_strength, |v| (v.min(100) / 25 * 25) as u8)
            }
            "enh.hud_full_width" => self.hud_full_width = value != "0",
            "enh.motion_blur" => self.motion_blur = value.parse::<u32>().map_or(self.motion_blur, |v| v.min(2) as u8),
            "enh.depth_of_field" => {
                self.depth_of_field = value.parse::<u32>().map_or(self.depth_of_field, |v| v.min(1) as u8)
            }
            _ => return self.extras.parse(key, value) || self.pad.parse(key, value),
        }
        true
    }

    /// [`Enhancements::apply`] at once.
    fn old_apply_now(&self) {
        // docs/settings-model.md: the values in force are the
        // stored ones with the settings model's engine rules applied (a
        // redundant or conflicting row resolved); options.txt keeps the
        // stored values, so turning the overriding row off restores them.
        let stored = self;
        let e = &stored.effective();
        old_store!(FOV_OFFSET, e.fov_offset as i32);
        old_store!(HUD_SCALE, e.hud_scale);
        old_store!(FRAME_CAP, e.frame_cap);
        old_store!(DURABLE, e.durable_saves);
        old_store!(BACKUPS, e.save_backups);
        old_store!(CRASH, e.crash_reports);
        old_store!(RENDER_SCALE, e.render_scale);
        old_store!(POST_AA, e.post_aa);
        old_store!(AA_MSAA, e.aa_msaa);
        old_store!(AA_FILTER, e.aa_filter);
        old_store!(SHARPEN, e.sharpen);
        old_store!(ROAD_REFLECTION, e.road_reflection);
        old_store!(CAR_REFLECTION, e.car_reflection);
        old_store!(DETAIL_DISTANCE, e.detail_distance);
        old_store!(CAR_DETAIL, e.car_detail);
        old_store!(HD_UI, e.hd_ui);
        old_store!(RELOAD_PACKS, e.reload_packs);
        old_store!(LOD_FADE, e.lod_fade);
        old_store!(KEEP_LOADED, e.keep_loaded);
        old_store!(ANISOTROPY, e.anisotropy);
        old_store!(REVERSED_Z, e.reversed_z);
        old_store!(SHADOWS, e.shadows);
        old_store!(ENHANCED_REFLECTIONS, e.enhanced_reflections);
        old_store!(REFLECTION_RATE, e.reflection_rate);
        old_store!(SSAO, e.ssao);
        old_store!(HDR_POST, e.hdr_post);
        old_store!(ENHANCED_PAINT, e.enhanced_paint);
        old_store!(LISTS_OPEN_INSTALLED, e.lists_open_installed);
        old_store!(OUTRUN_VARIETY, e.outrun_variety);
        old_store!(WET_ROADS, e.wet_roads);
        old_store!(FIX_MODELS, e.fix_models);
        old_store!(PHOTO_MODE, e.photo_mode);
        old_store!(HDR_OUTPUT, e.hdr_output);
        old_store!(HDR_PEAK, e.hdr_peak);
        old_store!(HDR_PAPER, e.hdr_paper);
        old_store!(HDR_GAMMA, e.hdr_gamma);
        old_store!(SSR, e.ssr);
        old_store!(DYNAMIC_LAMPS, e.dynamic_lamps);
        old_store!(LAMP_SHADOWS, e.lamp_shadows);
        old_store!(CONTACT_SHADOWS, e.contact_shadows);
        old_store!(UPSCALE, e.upscale);
        old_store!(VOLUMETRIC_FOG, e.volumetric_fog);
        old_store!(SKY, e.sky);
        old_store!(PARTICLES, e.particles);
        old_store!(SPARKS_FIRE, e.sparks_fire);
        old_store!(WET_CAR, e.wet_car);
        old_store!(RAIN_DETAIL, e.rain_detail);
        old_store!(SPRAY, e.spray);
        old_store!(LENS_DROPS, e.lens_drops);
        old_store!(WET_INTERACTION, e.wet_interaction);
        old_store!(MOTION_BLUR, e.motion_blur);
        old_store!(DEPTH_OF_FIELD, e.depth_of_field);
        old_store!(WATER, e.water);
        old_store!(AMBIENT_DETAIL, e.ambient_detail);
        old_store!(HEADLIGHT_BEAMS, e.headlight_beams);
        old_store!(LIGHT_PROBES, e.light_probes);
        old_store!(ENHANCED_GLASS, e.enhanced_glass);
        old_store!(SURFACE_DETAIL, e.surface_detail);
        old_store!(MODERN_LIGHTING, e.modern_lighting);
        old_store!(MODERN_STRENGTH, e.modern_strength);
        old_call!("extras.apply", e.extras, e.extras.apply());
        old_store!(SMOOTH_CARS, e.smooth_cars);
        old_store!(FAR_DRAW, e.far_draw);
        old_store!(TRAFFIC_DENSITY, e.traffic_density);
        old_call!(
            stringify!(crate::upscale_pack::set_choice),
            e.texture_pack,
            crate::upscale_pack::set_choice(e.texture_pack)
        );
        old_call!(
            stringify!(crate::render::display::set_hud_full_width),
            e.hud_full_width,
            crate::render::display::set_hud_full_width(e.hud_full_width)
        );
        if stored.crash_reports {
            old_call!("crash::keep_log", (), crash::keep_log());
        }
    }
}

/// `Preset::apply_high`'s first statement (the keep-list).
pub(super) fn old_preset_base(e: &mut Enhancements) {
    let qol = *e;
    *e = Enhancements {
        fov_offset: qol.fov_offset,
        hud_scale: qol.hud_scale,
        frame_cap: qol.frame_cap,
        durable_saves: qol.durable_saves,
        save_backups: qol.save_backups,
        crash_reports: qol.crash_reports,
        hd_ui: qol.hd_ui,
        reload_packs: qol.reload_packs,
        outrun_variety: qol.outrun_variety,
        hud_full_width: qol.hud_full_width,
        hdr_output: qol.hdr_output,
        hdr_peak: qol.hdr_peak,
        hdr_paper: qol.hdr_paper,
        hdr_gamma: qol.hdr_gamma,
        photo_mode: qol.photo_mode,
        extras: qol.extras,
        traffic_density: qol.traffic_density,
        texture_pack: qol.texture_pack,
        boot_loading_min_ms: qol.boot_loading_min_ms,
        launcher_skip: qol.launcher_skip,
        skip_title: qol.skip_title,
        skip_movies: qol.skip_movies,
        lists_open_installed: qol.lists_open_installed,
        fix_models: qol.fix_models,
        pad: qol.pad,
        ..Enhancements::default()
    };
}
