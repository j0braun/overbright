//! Every registered module composes and validates (naga), and the world
//! modules are, text for text, what the string surgery made before the composer
//! (the oracle below; to be deleted with the string surgery).

use super::compose::{self, VariantFlag, VariantFlags};
use super::registry::{self, ShaderId, TextureBinding, WorldKey};
use wgpu::naga;

fn validate(what: &str, src: &str) {
    let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{what}: {}", e.emit_to_string(src)));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{what}: {e:?}"));
}

/// The pre-S3 world module assembly, verbatim but for its input: the
/// marker-form world.wgsl it read is [`oracle::legacy`] of today's (every
/// `#if` taking its first branch, directive lines dropped), today's taken
/// with its includes put back.
mod oracle {
    use crate::render::temporal;

    pub fn legacy(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut keep: Vec<bool> = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("#if ") {
                keep.push(keep.iter().all(|&k| k));
            } else if t == "#else" {
                *keep.last_mut().unwrap() = false;
            } else if t == "#endif" {
                keep.pop();
            } else if t.starts_with("#include ") {
            } else if keep.iter().all(|&k| k) {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    /// World.wgsl as it was before the split into files (the includes
    /// put back; byte for byte, [`super::world_text_is_the_old_file`]).
    pub fn world_wgsl() -> String {
        legacy(super::registry::text(super::ShaderId::World))
    }

    /// mod.rs's texture blocks for `TextureArrays` (the lamps' module's
    /// carries Surface Detail's map layer and material).
    const TEXTURE_ARRAY_SURFACE_WGSL: &str = "
struct TextureImmediates { scroll: Scroll, layer: u32, surface_layer: u32, surface_material: u32 };
var<immediate> imm: TextureImmediates;
@group(1) @binding(0) var tex: texture_2d_array<f32>;
@group(1) @binding(1) var samp: sampler;
fn sample_tex(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, samp, uv, imm.layer);
}
fn tex_scroll() -> Scroll {
    return imm.scroll;
}
fn surface_slot() -> vec2<u32> {
    return vec2<u32>(imm.surface_layer, imm.surface_material);
}
";

    const TEXTURE_ARRAY_WGSL: &str = "
struct TextureImmediates { scroll: Scroll, layer: u32 };
var<immediate> imm: TextureImmediates;
@group(1) @binding(0) var tex: texture_2d_array<f32>;
@group(1) @binding(1) var samp: sampler;
fn sample_tex(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, samp, uv, imm.layer);
}
fn tex_scroll() -> Scroll {
    return imm.scroll;
}
";

    fn bound_array_wgsl(snippet: &str) -> String {
        let s = snippet
            .replace(
                "@group(1) @binding(0) var tex: texture_2d_array<f32>;",
                &format!(
                    "@group(1) @binding(0) var tex: binding_array<texture_2d_array<f32>, {}>;",
                    crate::render::BOUND_ARRAYS
                ),
            )
            .replace("textureSample(tex, samp, uv, imm.layer)", "textureSample(tex[imm.array], samp, uv, imm.layer)");
        if s.contains("surface_material: u32 };") {
            s.replace("surface_material: u32 };", "surface_material: u32, array: u32 };")
        } else {
            s.replace("layer: u32 };", "layer: u32, unused0: u32, unused1: u32, array: u32 };")
        }
    }

    fn strip(text: &str, begin: &str, end: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut inside = false;
        for line in text.lines() {
            let t = line.trim_start();
            if t.starts_with(begin) {
                inside = true;
            }
            if !inside {
                out.push_str(line);
                out.push('\n');
            }
            if t.starts_with(end) {
                inside = false;
            }
        }
        out
    }

    /// modern_light.rs `source_tier`.
    fn source_tier(text: &str, modern: u8) -> String {
        match modern {
            0 => strip(text, "// BEGIN MODERN", "// END MODERN"),
            1 => strip(text, "// BEGIN MLRT", "// END MLRT"),
            _ => format!("enable wgpu_ray_query;\n{text}"),
        }
    }

    pub fn world_source_modern(lamps: bool, modern: u8) -> String {
        let source = world_wgsl();
        let mut out = String::with_capacity(source.len());
        let mut inside = false;
        for line in source.lines() {
            let t = line.trim_start();
            if t.starts_with("// BEGIN LAMPS") {
                inside = true;
            }
            if lamps || !inside {
                out.push_str(line);
                out.push('\n');
            }
            if t.starts_with("// END LAMPS") {
                inside = false;
            }
        }
        if lamps {
            out = out.replace(
                "    @builtin(position) clip: vec4<f32>,",
                "    @builtin(position) @invariant clip: vec4<f32>,",
            );
        }
        out.push_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/wet_road_world.wgsl")));
        source_tier(&out, if lamps { modern } else { 0 })
    }

    pub fn world_module_source_modern(
        lamps: bool,
        modern: u8,
        arrays: bool,
        multiview: bool,
        biased: bool,
        bound: bool,
    ) -> String {
        let source = world_source_modern(lamps, modern);
        let (rt, source) = match source.strip_prefix("enable wgpu_ray_query;\n") {
            Some(rest) => (true, rest.to_string()),
            None => (false, source),
        };
        let source = if arrays {
            let (head, rest) = source.split_once("// BEGIN TEXTURE BINDING").expect("world.wgsl's texture block");
            let (_, tail) = rest.split_once("// END TEXTURE BINDING").expect("world.wgsl's texture block");
            let block = if lamps { TEXTURE_ARRAY_SURFACE_WGSL } else { TEXTURE_ARRAY_WGSL };
            let block = if bound { bound_array_wgsl(block) } else { block.to_string() };
            format!("{head}{block}{tail}")
        } else {
            source
        };
        let source = if multiview { multiview_wgsl(&source) } else { source };
        let source = if biased { temporal::biased_source(&source) } else { source };
        let source = if bound { format!("enable wgpu_binding_array;\n{source}") } else { source };
        if rt { format!("enable wgpu_ray_query;\n{source}") } else { source }
    }

    fn multiview_wgsl(source: &str) -> String {
        let source = source.replace(
            "@group(0) @binding(0) var<uniform> frame: Frame;",
            "@group(0) @binding(0) var<uniform> frames: array<Frame, 6>;\nvar<private> frame: Frame;",
        );
        let mut out = String::with_capacity(source.len() + 1024);
        let mut entry = false;
        for line in source.lines() {
            let t = line.trim_start();
            if t.starts_with("@vertex") || t.starts_with("@fragment") {
                entry = true;
            }
            if entry
                && t.starts_with("fn ")
                && let Some(open) = line.find('(')
            {
                let close = line[open..].find(')').map(|c| open + c).filter(|&c| c > open + 1).unwrap_or(open);
                let (head, tail) = line.split_at(close);
                let sep = if close == open { "" } else { ", " };
                let head = if close == open { &line[..open + 1] } else { head };
                let tail = if close == open { &line[open + 1..] } else { tail };
                out.push_str(&format!(
                    "{head}{sep}@builtin(view_index) view_index: u32{tail}\n    frame = frames[view_index];\n"
                ));
                entry = false;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// rt_shadow.rs `source`'s blocks, from the marker-form text.
    pub fn rtvis_block(name: &str) -> String {
        let world = world_wgsl();
        let begin = format!("// BEGIN RTVIS {name}\n");
        let end = format!("// END RTVIS {name}\n");
        let (_, rest) = world.split_once(begin.as_str()).unwrap_or_else(|| panic!("world.wgsl's RTVIS {name}"));
        rest.split_once(end.as_str()).unwrap_or_else(|| panic!("world.wgsl's RTVIS {name} end")).0.to_string()
    }
}

/// Text identity: every world module the code can ask for — lamps ×
/// Modern tier × texture binding × multiview × mip bias, as
/// `world_module_source_modern`'s arguments — is byte for byte the old
/// assembly's text.
#[test]
fn world_modules_match_the_old_assembly() {
    let mut n = 0;
    for lamps in [false, true] {
        for modern in 0..=2u8 {
            for (arrays, bound) in [(false, false), (true, false), (true, true)] {
                for multiview in [false, true] {
                    for biased in [false, true] {
                        let new =
                            crate::render::world_module_source_modern(lamps, modern, arrays, multiview, biased, bound);
                        let old = oracle::world_module_source_modern(lamps, modern, arrays, multiview, biased, bound);
                        assert!(
                            new == old,
                            "lamps {lamps} modern {modern} arrays {arrays} bound {bound} multiview {multiview} biased {biased}: {}",
                            first_difference(&old, &new)
                        );
                        n += 1;
                    }
                }
            }
        }
    }
    assert_eq!(n, 72);
    // The marker-form text is a WGSL file with balanced markers.
    let legacy = oracle::world_wgsl();
    for m in ["LAMPS", "MODERN", "MLRT", "TEXTURE BINDING"] {
        assert_eq!(
            legacy.matches(&format!("// BEGIN {m}")).count(),
            legacy.matches(&format!("// END {m}")).count(),
            "{m}"
        );
    }
}

fn first_difference(a: &str, b: &str) -> String {
    let line = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(a.lines().count().min(b.lines().count()));
    format!("line {}: old {:?} new {:?}", line + 1, a.lines().nth(line), b.lines().nth(line))
}

/// rt_shadow's module takes world.wgsl's RTVIS sections as they were.
#[test]
fn rtvis_sections_match_the_old_blocks() {
    for name in ["FRAME", "LAMP", "A", "B", "C", "D"] {
        assert_eq!(registry::world_section(&format!("RTVIS {name}")), oracle::rtvis_block(name), "RTVIS {name}");
    }
    for depth_ms in [false, true] {
        validate("rt_shadow", &crate::render::rt_shadow::source(depth_ms));
    }
}

/// Every distinct world module parses and validates; the key space is
/// what `WorldShared::mode_modern` can build.
#[test]
fn every_world_key_validates() {
    let keys = WorldKey::all();
    // !lamps: 3 bindings × multiview × bias; lamps: × 3 tiers × the
    // cluster loop's two forms.
    assert_eq!(keys.len(), 12 + 72);
    for key in keys {
        let src = registry::world(key);
        assert!(!src.lines().any(|l| l.trim_start().starts_with('#')), "{key:?}: a directive left");
        validate(&format!("{key:?}"), &src);
    }
}

/// The lamps' cluster loop: the plain form (a device without subgroups:
/// DX12 through FXC, adapters without them) has no subgroup operation and
/// validates without the capability; the subgroup form needs it, in the
/// fragment stage only (wgpu's `Features::SUBGROUP` without
/// `SUBGROUP_VERTEX`). A pipeline constant could not choose between them:
/// wgpu validates the module against the device's capabilities before any
/// constant is set. The two forms differ only in `lamp_light`'s loop head:
/// the per-cluster sum (`cluster_light`) is one text.
#[test]
fn the_cluster_loop_takes_subgroups_only_where_the_device_has_them() {
    use naga::valid::Capabilities as C;
    let without = C::all() - C::SUBGROUP - C::SUBGROUP_BARRIER - C::SUBGROUP_VERTEX_STAGE;
    let fragment_only = C::all() - C::SUBGROUP_VERTEX_STAGE;
    let check = |src: &str, caps: C| {
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps).validate(&module).map(|_| ())
    };
    let mut n = 0;
    for key in WorldKey::all().into_iter().filter(|k| k.lamps && !k.subgroups) {
        let plain = registry::world(key);
        let scalar = registry::world(WorldKey { subgroups: true, ..key });
        assert!(!plain.contains("subgroup"), "{key:?}: a subgroup operation in the plain form");
        check(&plain, without).unwrap_or_else(|e| panic!("{key:?} plain: {e:?}"));
        assert_eq!(scalar.matches("subgroupBroadcastFirst(c)").count(), 1, "{key:?}");
        assert!(check(&scalar, without).is_err(), "{key:?}: the subgroup form validated without subgroups");
        check(&scalar, fragment_only).unwrap_or_else(|e| panic!("{key:?} subgroups: {e:?}"));
        let body = |s: &str| {
            s.split_once("fn cluster_light(").expect("cluster_light").1.split_once("\n}\n").unwrap().0.to_string()
        };
        assert_eq!(body(&plain), body(&scalar), "{key:?}");
        n += 1;
    }
    assert_eq!(n, 36);
    // The native modules: no cluster loop at all, whatever the device.
    for key in WorldKey::all().into_iter().filter(|k| !k.lamps) {
        assert_eq!(registry::world(WorldKey { subgroups: true, ..key }), registry::world(key));
    }
}

/// Every module's every flag combination composes: balanced `#if`s,
/// known flag names, includes found once (the include set does not depend
/// on the flags).
#[test]
fn every_source_composes() {
    for id in ShaderId::ALL {
        for bits in 0..1u32 << VariantFlag::ALL.len() {
            let flags: VariantFlags =
                VariantFlag::ALL.into_iter().enumerate().filter(|(i, _)| bits >> i & 1 != 0).map(|(_, f)| f).collect();
            registry::compose(id, flags).unwrap_or_else(|e| panic!("{id:?} {flags:?}: {e}"));
        }
    }
}

/// The lamps' sections hold everything that is not native: with LAMPS off
/// the feature flags inside them add nothing (MODERN and MLRT only ever
/// reach the lamps' module; MLRT only inside MODERN).
#[test]
fn feature_sections_nest_in_the_lamps() {
    let c = |f: &[VariantFlag]| {
        registry::compose(ShaderId::World, f.iter().copied().chain([VariantFlag::TextureBindingPlain]).collect())
            .unwrap()
    };
    use VariantFlag::*;
    let native = c(&[]);
    for f in [Modern, Mlrt] {
        assert_eq!(c(&[f]), native, "{f:?} outside LAMPS");
    }
    assert_eq!(c(&[Lamps, Mlrt]), c(&[Lamps]), "MLRT outside MODERN");
    let lamps = c(&[Lamps]);
    for s in ["lamp_light", "contact_visibility", "shadow_maps", "frame.lamps", "// BEGIN LAMPS"] {
        assert!(!native.contains(s) && lamps.contains(s), "{s}");
    }
    let modern = c(&[Lamps, Modern]);
    assert!(!lamps.contains("ml_relight") && !lamps.contains("MODERN") && modern.contains("ml_relight"));
}

/// The bound arrays' size in the shader is the renderer's.
#[test]
fn bound_array_size_matches() {
    let key = WorldKey {
        lamps: false,
        modern: 0,
        binding: TextureBinding::Bound,
        multiview: false,
        biased: false,
        subgroups: false,
    };
    assert!(
        registry::world(key)
            .contains(&format!("binding_array<texture_2d_array<f32>, {}>", crate::render::BOUND_ARRAYS))
    );
}

/// A section must stay plain WGSL; the composer rejects a directive in one.
#[test]
fn sections_are_plain() {
    let e = compose::section("w", "// BEGIN X\n#if LAMPS\n// END X\n", "X").unwrap_err();
    assert_eq!(e.line, 2);
}

/// FNV-1a 64 of `s`'s bytes.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Text identity: `id`'s source with its includes put back is byte for
/// byte the file it was cut from (that file's length and FNV-1a 64 before
/// the split; re-pinned once when only the comments changed, the parsed
/// modules identical), and where the module has no `#if` the composed
/// module is that text too (what the renderer embedded before).
fn assert_old_text(id: ShaderId, len: usize, fnv: u64) {
    let text = registry::text(id);
    assert_eq!((text.len(), fnv1a(text)), (len, fnv), "{id:?}: not the pre-split text");
    if id != ShaderId::World {
        assert_eq!(registry::compose(id, VariantFlags::NONE).unwrap(), text, "{id:?}");
    }
}

/// world.wgsl (split): the S3 oracle above reads this text.
#[test]
fn world_text_is_the_old_file() {
    // Re-pinned for one wetness (a logic change in the wet branch, `wetness.wgsl` included; the native path's
    // functions unchanged, gate-proven byte-identical): was 109_313, 0x6b9c_b239_7f8b_6696; then 112_286,
    // 0x25a3_7cd7_add9_ae3d before a comment-only rewrite of `wetness.wgsl` and `world_wet.wgsl`; then
    // 112_241, 0x0b65_3435_20cf_2975 before the de-lit texture option's `#if` blocks were removed (every
    // composed world module without them unchanged, byte for byte); then 111_658, 0xc50f_7c60_523f_c252 before
    // the light-cluster loop's subgroup form (`#if !SUBGROUPS`, and the per-cluster sum moved from
    // `lamp_light` into `cluster_light` for both forms: the same operations; the native modules unchanged); then
    // 112_837, 0xe5fb_dd21_26a5_33a1 before the cars and lamps culled per light cluster (docs/lamps-shadows.md) and the SSR mask from the main view's entries (`world_masks.wgsl`: new
    // entry points and their one binding only); then 122_409, 0xaebe_c9e7_6e96_b63a before the traced shadows'
    // steps, made in parallel from 111_658 (there: 114_001, 0x15b1_5077_1229_66a9; 116_653, 0x719f_eb19_7d8b_d114;
    // 117_249, 0xfbcb_61e4_6fd5_6c42; 117_718, 0xe375_ee4a_fa27_9b69): Modern + RT's gather moved to the top of the
    // colour functions (`ml_vis_early`; the native and the lamps' modules unchanged, Modern's by two comment lines),
    // the gather's two exact short cuts (`ml_vis_taps`: uniform blocks, no light; Modern + RT's module only), the
    // blocks' grid scale (`ml_vis_grid`; Modern + RT's module only) and the road's dry-road override (`D9_WET`; the
    // lamps' modules only); then 128_469, 0xe290_d4d5_ea1d_c825 before the lamps' module's alpha-0 entries
    // (`fs_world_clear` / `fs_nofog_clear`) and the falling rain lit at its drops' corners (`rain_lit.wgsl`): new
    // entry points inside the LAMPS blocks only, the native modules unchanged (made in parallel from 122_409:
    // 129_099, 0x5e49_6c40_8c8c_df4d there).
    assert_old_text(ShaderId::World, 135_159, 0xbb99_edb7_338b_e7e0);
}

#[test]
fn postfx_text_is_the_old_file() {
    // Re-pinned for SSAA's resolve to the screen's size (`post_scale.wgsl`'s compute resolve and its last pass,
    // docs/ssaa-screen-passes.md: new entry points only; every older entry point unchanged, the shots without SSAA
    // byte-identical): was 19_324, 0x7dcc_3c3e_3dc0_2856. Then for the glow skip (High Precision Post's two
    // prefilters each split into a function and its entry, which the flagging entries in `glow_skip.wgsl` call;
    // `native/post/post.wgsl` unchanged, the native gate byte-identical): was 25_013, 0x741f_c83a_0cec_45cc.
    assert_old_text(ShaderId::PostFx, 25_189, 0x931f_0f74_33c6_4cb7);
}

#[test]
fn weather_fx_text_is_the_old_file() {
    // Re-pinned for one wetness (Rain Next-Gen's puddles from `wetness.wgsl`): was 41_646, 0xa9d9_bf4b_5b76_2251;
    // then 43_159, 0xc4ce_c134_9d32_7eca before a comment-only rewrite of `wetness.wgsl` and `rain_ng.wgsl`;
    // then 43_130, 0x9704_be26_8974_e403 before the haze left the lamps and cones to the one volume.
    assert_old_text(ShaderId::WeatherFx, 43_263, 0x76e2_1d93_4279_1d8f);
}

#[test]
fn volume_fx_text_is_the_old_file() {
    // Re-pinned for the resolve per workgroup (`resolve.wgsl`; `volume_common.wgsl`'s note on `look6`): was
    // 47_830, 0x8619_fbfa_114f_aaaa; then 50_744, 0xa3cc_c427_5cb7_e236 before the march's override constants
    // (`march.wgsl`: the branches a renderer's look never takes left out of its program).
    assert_old_text(ShaderId::VolumeFx, 51_215, 0x6d83_3066_17fc_6c06);
}

#[test]
fn gpu_particles_text_is_the_old_file() {
    assert_old_text(ShaderId::GpuParticles, 14_333, 0x1550_8ad6_6c56_18d9);
}

#[test]
fn smoke_cinematic_text_is_the_old_file() {
    assert_old_text(ShaderId::SmokeCinematic, 21_392, 0x4815_1d6c_317c_e77e);
}

#[test]
fn fsr2_text_is_the_old_file() {
    assert_old_text(ShaderId::Fsr2, 41_038, 0x1a09_e7ac_5930_924c);
}

#[test]
fn ui_text_is_the_old_file() {
    assert_old_text(ShaderId::Ui, 6_891, 0xe2b5_2d35_6812_b3b9);
}
