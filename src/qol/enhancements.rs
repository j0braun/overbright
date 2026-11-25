use super::*;

/// The refactor plan (the maintainer's private notes): every option of
/// [`Enhancements`] is declared once, in the `options!` table below, and the
/// struct, its `Default` (the native values), the options.txt writer and
/// parser, the deviation list, the presets' keep-list and `apply_now`'s
/// rows are generated from it ([`OPTION_ROWS`]).
///
/// A row: `field: type = native default, key "enh.…" | nested, presets
/// keep | reset, write N, dev none | [N form], apply none | [N form], parse
/// form;`
/// * `key nested`: a group with its own keys and code (`extras`, `pad`).
/// * `presets keep`: the graphics presets leave the value alone
///   ([`Enhancements::preset_base`]); `reset`: a preset sets it.
/// * `write N`: the line's place in options.txt (rank N, ascending); a line
///   is written only when the value is not the default (`bool` as 0 / 1).
/// * `dev [N flag "label"]` (a bool that is on), `[N tiers ["1", "2", …]]`
///   (value 1, 2, …; the last label for every value above), `[N fmt |x| …]`
///   (the value when not the default), `[N custom |s, d, v| …]`; N is the
///   place in [`Enhancements::deviations`].
/// * `apply [N store NAME]` (a value in force: the settings store holds it,
///   `apply_now` puts every value in force in one change; NAME is its name
///   in the apply log), `[N call path]`, `[N custom |e| …]`; N is the place
///   in [`Enhancements::apply_now`] (some setters derive others: the order
///   is kept as it was).
/// * `parse bool` (`!= "0"`), `nested`, or `(|old, value| new)` (the value
///   trimmed; `old` kept for a value that does not parse unless the rule
///   says otherwise).
macro_rules! options {
    ($(
        $(#[$doc:meta])*
        $field:ident: $ty:ty = $default:expr,
        key $key:tt,
        presets $keep:ident,
        write $wr:literal,
        dev $dev:tt,
        apply $apply:tt,
        parse $parse:tt;
    )*) => {
        /// The Enhancements page's values. `Default` is native for every one.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct Enhancements {
            $(
                $(#[$doc])*
                pub $field: $ty,
            )*
        }

        impl Default for Enhancements {
            fn default() -> Enhancements {
                Enhancements { $($field: $default,)* }
            }
        }

        impl Enhancements {
            /// The graphics presets' starting point: these values with every
            /// `presets reset` row at its default (the `keep` rows kept).
            pub fn preset_base(&self) -> Enhancements {
                let d = Enhancements::default();
                Enhancements { $($field: options!(@keep $keep self.$field, d.$field),)* }
            }
        }

        /// The option table (see `options!`), in [`Enhancements`]' field order.
        pub static OPTION_ROWS: &[OptionRow] = &[$(
            OptionRow {
                field: stringify!($field),
                key: options!(@key $key),
                preset_keeps: options!(@keep $keep true, false),
                write_rank: $wr,
                dev_rank: options!(@rank $dev),
                apply_rank: options!(@rank $apply),
                write: options!(@write $field $key),
                parse: options!(@parse $field [$ty] $key $parse),
                deviation: options!(@dev $field [$ty] $dev),
                apply: options!(@apply $field [$ty] $apply),
            },
        )*];
    };

    (@keep keep $a:expr, $b:expr) => { $a };
    (@keep reset $a:expr, $b:expr) => { $b };

    (@key nested) => { None };
    (@key $k:literal) => { Some($k) };

    (@rank none) => { 0 };
    (@rank [$r:literal $($rest:tt)*]) => { $r };

    (@write $f:ident nested) => {
        |s: &Enhancements, _: &Enhancements, out: &mut String| s.$f.write(out)
    };
    (@write $f:ident $k:literal) => {
        |s: &Enhancements, d: &Enhancements, out: &mut String| {
            if s.$f != d.$f {
                out.push_str(&format!("{} {}\n", $k, OptionText::text(s.$f)));
            }
        }
    };

    (@parse $f:ident [$ty:ty] nested nested) => {
        |s: &mut Enhancements, key: &str, value: &str| s.$f.parse(key, value)
    };
    (@parse $f:ident [$ty:ty] $k:literal bool) => {
        |s: &mut Enhancements, key: &str, value: &str| key == $k && { s.$f = value != "0"; true }
    };
    (@parse $f:ident [$ty:ty] $k:literal ($rule:expr)) => {
        |s: &mut Enhancements, key: &str, value: &str| {
            let rule: fn($ty, &str) -> $ty = $rule;
            key == $k && { s.$f = rule(s.$f, value); true }
        }
    };

    (@dev $f:ident [$ty:ty] none) => {
        |_: &Enhancements, _: &Enhancements, _: &mut Vec<String>| {}
    };
    (@dev $f:ident [$ty:ty] [$r:literal flag $label:literal]) => {
        |s: &Enhancements, _: &Enhancements, v: &mut Vec<String>| {
            if s.$f {
                v.push($label.into());
            }
        }
    };
    (@dev $f:ident [$ty:ty] [$r:literal tiers $labels:expr]) => {
        |s: &Enhancements, _: &Enhancements, v: &mut Vec<String>| tier(v, s.$f, &$labels)
    };
    (@dev $f:ident [$ty:ty] [$r:literal fmt $fmt:expr]) => {
        |s: &Enhancements, d: &Enhancements, v: &mut Vec<String>| {
            let fmt: fn($ty) -> String = $fmt;
            if s.$f != d.$f {
                v.push(fmt(s.$f));
            }
        }
    };
    (@dev $f:ident [$ty:ty] [$r:literal custom $dev:expr]) => {{
        let dev: fn(&Enhancements, &Enhancements, &mut Vec<String>) = $dev;
        dev
    }};

    (@apply $f:ident [$ty:ty] none) => {
        |_: &Enhancements| {}
    };
    (@apply $f:ident [$ty:ty] [$r:literal store $name:ident]) => {
        |e: &Enhancements| record_value::<$ty>(stringify!($name), e.$f)
    };
    (@apply $f:ident [$ty:ty] [$r:literal call $call:path]) => {
        |e: &Enhancements| {
            record(stringify!($call), &e.$f);
            $call(e.$f)
        }
    };
    (@apply $f:ident [$ty:ty] [$r:literal custom $apply:expr]) => {{
        let apply: fn(&Enhancements) = $apply;
        apply
    }};
}

/// One option of [`Enhancements`] (a row of `options!`).
#[derive(Clone, Copy)]
pub struct OptionRow {
    /// The [`Enhancements`] field.
    pub field: &'static str,
    /// Its options.txt key; None for a nested group (`extras`, `pad`) that
    /// keeps its own keys.
    pub key: Option<&'static str>,
    /// The graphics presets leave it alone ([`Enhancements::preset_base`]).
    pub preset_keeps: bool,
    /// The places in options.txt, the deviation list and `apply_now`
    /// (ascending; 0 = not there).
    pub write_rank: u8,
    pub dev_rank: u8,
    pub apply_rank: u8,
    /// The line, written when the value is not the default (the 2nd
    /// argument).
    pub write: fn(&Enhancements, &Enhancements, &mut String),
    /// Reads `key value` (value trimmed) when the key is this row's; false
    /// otherwise.
    pub parse: fn(&mut Enhancements, &str, &str) -> bool,
    /// Pushes the row's deviation label(s) (the 2nd argument: the defaults).
    pub deviation: fn(&Enhancements, &Enhancements, &mut Vec<String>),
    /// Its part of putting the values in force (`apply_now`).
    pub apply: fn(&Enhancements),
}

/// Old keys a file may still hold: known and ignored (never written).
/// `enh.vector_fonts`: the old Vector Fonts row, folded into HD UI Art (a
/// file that still has it reads as before minus that row).
pub const LEGACY_KEYS: [&str; 1] = ["enh.vector_fonts"];

/// The value as options.txt writes it (`bool` as 0 / 1).
trait OptionText {
    fn text(self) -> String;
}

impl OptionText for bool {
    fn text(self) -> String {
        u8::from(self).to_string()
    }
}

macro_rules! option_text {
    ($($t:ty),*) => {$(
        impl OptionText for $t {
            fn text(self) -> String {
                self.to_string()
            }
        }
    )*};
}
option_text!(i8, u8, u16);

/// A tiered row's label: none for 0, else `labels[v - 1]`, the last one for
/// every value above.
fn tier(v: &mut Vec<String>, x: u8, labels: &[&str]) {
    if x != 0 {
        v.push(labels[usize::from(x - 1).min(labels.len() - 1)].into());
    }
}

/// A number rule: the value parsed as u32 and held to `max`.
fn min_u32(old: u8, value: &str, max: u8) -> u8 {
    value.parse::<u32>().map_or(old, |v| v.min(u32::from(max)) as u8)
}

/// The rows with a rank (`write_rank` / `dev_rank` / `apply_rank` by
/// `rank`), in ascending rank.
fn ranked(rank: fn(&OptionRow) -> u8) -> Vec<&'static OptionRow> {
    let mut v: Vec<&OptionRow> = OPTION_ROWS.iter().filter(|r| rank(r) != 0).collect();
    v.sort_by_key(|r| rank(r));
    v
}

/// The ordered rows, computed once.
struct Orders {
    write: Vec<&'static OptionRow>,
    dev: Vec<&'static OptionRow>,
    apply: Vec<&'static OptionRow>,
}

/// `apply_now`'s rows, in their `apply` order.
pub(super) fn apply_order() -> &'static [&'static OptionRow] {
    &orders().apply
}

fn orders() -> &'static Orders {
    static O: std::sync::OnceLock<Orders> = std::sync::OnceLock::new();
    O.get_or_init(|| Orders {
        write: ranked(|r| r.write_rank),
        dev: ranked(|r| r.dev_rank),
        apply: ranked(|r| r.apply_rank),
    })
}

options! {
    /// Degrees added to the player camera's field of view (the chase,
    /// hood and bumper views; scenes and replays' track cameras keep
    /// theirs). 0 = native.
    fov_offset: i8 = 0,
        key "enh.fov", presets keep, write 1,
        dev [1 fmt |x| format!("FOV {:+}", x)],
        apply [1 store FOV_OFFSET],
        parse (|old, v| v.parse::<i8>().map_or(old, |v| v.clamp(FOV_STEPS[0], FOV_STEPS[FOV_STEPS.len() - 1])));

    /// The in-game HUD's size, percent (each part kept at its corner or
    /// edge of the 640 × 480 canvas, see [`scale_hud`]). 100 = native.
    hud_scale: u8 = 100,
        key "enh.hud_scale", presets keep, write 2,
        dev [2 fmt |x| format!("HUD {}%", x)],
        apply [2 store HUD_SCALE],
        parse (|old, v| v.parse::<u8>().map_or(old, |v| v.clamp(HUD_STEPS[0], HUD_STEPS[HUD_STEPS.len() - 1])));

    /// A frame cap, frames per second. 0 = off (native: uncapped). The
    /// in-engine scenes' own 30 fps is native and not this.
    frame_cap: u16 = 0,
        key "enh.frame_cap", presets keep, write 3,
        dev [4 fmt |x| format!("cap {} fps", x)],
        apply [3 store FRAME_CAP],
        parse (|old, v| v.parse::<u16>().map_or(old, |v| if v == 0 { 0 } else { v.clamp(10, 1000) }));

    /// Profile writes flushed to the disk (fsync of the temporary file and
    /// of the folder) before and after the rename. Off = native (the write
    /// and rename alone).
    durable_saves: bool = false,
        key "enh.durable_saves", presets keep, write 4,
        dev [5 flag "durable saves"],
        apply [4 store DURABLE],
        parse bool;

    /// Rolling backups: the last N copies of a profile, taken before each
    /// save, in [`saves::backup_root`]. 0 = off.
    save_backups: u8 = 0,
        key "enh.save_backups", presets keep, write 5,
        dev [6 fmt |x| format!("backups {}", x)],
        apply [5 store BACKUPS],
        parse (|old, v| v.parse::<u8>().map_or(old, |v| v.min(BACKUP_STEPS[BACKUP_STEPS.len() - 1])));

    /// A panic writes a report to [`crash::dir`]. Off = native (the
    /// default panic message on stderr only).
    crash_reports: bool = false,
        key "enh.crash_reports", presets keep, write 6,
        dev [7 flag "crash reports"],
        apply [6 store CRASH],
        parse bool;

    /// The Graphics page (docs/formats.md "Graphics enhancements"):
    /// the 3D scene's size, percent of the screen's (100 = native; the UI
    /// stays at the screen's size). See [`scene_size`] for the caps.
    render_scale: u8 = 100,
        key "enh.render_scale", presets reset, write 7,
        dev [8 custom |s, _, v| if s.render_scale != 100 && ssaa_scale(s.post_aa).is_none() { v.push(format!("render {}%", s.render_scale)) }],
        apply [7 store RENDER_SCALE],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| v.clamp(SCALE_STEPS[0].into(), SCALE_STEPS[SCALE_STEPS.len() - 1].into()) as u8));

    /// Anti-Aliasing: 0 off (native: Customize Display's Full Screen
    /// Anti-Aliasing alone), 1 FXAA, 2 SMAA 1x, 3 / 4 / 5 MSAA 2× / 4× / 8×
    /// (the scene's sample count instead of the native option's, with
    /// alpha-to-coverage on the alpha-tested draws). 6 / 7 SSAA 2x / 4x
    /// render twice / four times the scene pixels, then downsample before
    /// the UI. IDs 8/9/10 add SSAA 8x/9x/16x. Independent overrides
    /// permit MSAA and post filters on top; defaults preserve prior presets.
    post_aa: u8 = 0,
        key "enh.post_aa", presets reset, write 8,
        dev [9 custom |s, _, v| if let Some(aa) = aa_name(s.post_aa) { v.push(aa.into()) }],
        apply [8 store POST_AA],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| v.min(u32::from(TAA)) as u8));

    /// Independent MSAA: 0 follows preset/native, 1 disables, 2/4/8/16 samples.
    aa_msaa: u8 = 0,
        key "enh.aa_msaa", presets reset, write 9,
        dev [10 fmt |x| format!("MSAA override {}", x)],
        apply [9 store AA_MSAA],
        parse (|old, v| v.parse::<u8>().map_or(old, |v| if MSAA_STEPS.contains(&v) { v } else { 0 }));

    /// Post filter: 0 follows preset, 1 off, 2 FXAA, 3 SMAA, 4 SMAA then FXAA.
    aa_filter: u8 = 0,
        key "enh.aa_filter", presets reset, write 10,
        dev [11 fmt |x| format!("post filter {}", filter_name(x))],
        apply [10 store AA_FILTER],
        parse (|old, v| v.parse::<u8>().map_or(old, |v| v.min(4)));

    /// Contrast-adaptive sharpening, percent (0 = off, native).
    sharpen: u8 = 0,
        key "enh.sharpen", presets reset, write 11,
        dev [12 fmt |x| format!("sharpen {}%", x)],
        apply [11 store SHARPEN],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| nearest(&SHARPEN_STEPS, v.min(100) as u8)));

    /// The road reflection's image (view 4): 0 native 320 × 240, 1 half
    /// the scene's size, 2 the scene's size.
    road_reflection: u8 = 0,
        key "enh.road_reflection", presets reset, write 12,
        dev [13 tiers ["road refl. half", "road refl. full"]],
        apply [12 store ROAD_REFLECTION],
        parse (|old, v| min_u32(old, v, 2));

    /// The cars' environment cube's edge (native 128).
    car_reflection: u16 = 128,
        key "enh.car_reflection", presets reset, write 13,
        dev [14 fmt |x| format!("car refl. {}", x)],
        apply [13 store CAR_REFLECTION],
        parse (|old, v| {
            // Sizes map by their power of two (512 → 1024, the tie going up).
            v.parse::<u32>().map_or(old, |v| {
                let log = v.clamp(128, 1024).next_power_of_two().ilog2() as u8;
                1 << nearest(&CAR_REFLECTION_STEPS.map(|s| s.ilog2() as u8), log)
            })
        });

    /// Detail Distance, percent (100 = native): the main view's scenery
    /// size cull (17 px, 0x5D66A4) divided by this / 100.
    detail_distance: u16 = 100,
        key "enh.detail_distance", presets reset, write 14,
        dev [15 fmt |x| format!("detail dist. {}%", x)],
        apply [14 store DETAIL_DISTANCE],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| nearest(&DETAIL_STEPS, v.clamp(100, 300) as u16)));

    /// Car Detail Distance, percent (100 = native): the car draws' screen
    /// size (0x48ACB0) times this / 100 before the LOD pick and its
    /// not-drawn test (size < 4).
    car_detail: u16 = 100,
        key "enh.car_detail", presets reset, write 15,
        dev [16 fmt |x| format!("car detail {}%", x)],
        apply [15 store CAR_DETAIL],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| nearest(&CAR_DETAIL_STEPS, v.clamp(100, 300) as u16)));

    /// Enable the installed HD HUD/map pack at startup. Changing this
    /// setting requires restart so textures and map calibration stay paired.
    hd_ui: bool = false,
        key "enh.hd_ui", presets keep, write 16,
        dev [17 flag "HD UI + Maps"],
        apply [16 store HD_UI],
        parse bool;

    /// Reload Packs When Changed (docs/modding/pack-hot-reload.md): watch the
    /// HD UI pack and the world/car texture pack folders and reload changed
    /// images live; the HD UI Art and World Texture Pack rows then apply at
    /// once. Off = native (packs read at startup / load).
    reload_packs: bool = false,
        key "enh.reload_packs", presets keep, write 17,
        dev [18 flag "pack hot reload"],
        apply [17 store RELOAD_PACKS],
        parse bool;

    /// LOD Fade: the main view's scenery size cull and the car LOD
    /// switches fade over [`LOD_FADE_SECONDS`] with a screen-door dither,
    /// with a hysteresis band. Off = native (pops).
    lod_fade: bool = false,
        key "enh.lod_fade", presets reset, write 18,
        dev [19 flag "LOD fade"],
        apply [18 store LOD_FADE],
        parse bool;

    /// Keep All Loaded: every section stays resident (as the clone's
    /// loader already holds them) and the drawn set is widened by the
    /// lists of the outlines within `world::LOOKAHEAD_RING` of the eye
    /// (`World::lookahead_sections`). Off = native (the eye's list only).
    keep_loaded: bool = false,
        key "enh.keep_loaded", presets reset, write 19,
        dev [20 flag "keep all loaded"],
        apply [19 store KEEP_LOADED],
        parse bool;

    /// Texture Filtering: 0 native (Customize Display's Texture Filtering,
    /// custom 20), else the world / car / shadow textures' anisotropy
    /// (2, 4, 8, 16) with linear filtering.
    anisotropy: u8 = 0,
        key "enh.anisotropy", presets reset, write 20,
        dev [21 fmt |x| format!("aniso {}x", x)],
        apply [20 store ANISOTROPY],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| if v < 2 { 0 } else { v.min(16).next_power_of_two().min(16) as u8 }));

    /// Reversed-Z depth: the scene's depth range reversed (near 1, far 0)
    /// with GREATEREQUAL tests. Off = native.
    reversed_z: bool = false,
        key "enh.reversed_z", presets reset, write 21,
        dev [22 flag "reversed-Z"],
        apply [21 store REVERSED_Z],
        parse bool;

    /// Shadow Quality: 0 native, 1 high, 2 ultra (the car shadow texture's
    /// filtering, see `render::display::shadow_filter`).
    shadows: u8 = 0,
        key "enh.shadows", presets reset, write 22,
        dev [23 tiers ["shadows high", "shadows ultra"]],
        apply [22 store SHADOWS],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| nearest(&SHADOW_STEPS, v.min(2) as u8)));

    /// Reflection Update Rate: 0 native (Car Reflection Update Rate's face
    /// schedule, 0x7FEEA0), 1 every face every frame.
    reflection_rate: u8 = 0,
        key "enh.reflection_rate", presets reset, write 24,
        dev [25 tiers ["refl. every frame"]],
        apply [24 store REFLECTION_RATE],
        parse (|old, v| min_u32(old, v, 1));

    /// Optional surface-aware car shading and filtered, higher-detail road reflections.
    enhanced_reflections: bool = false,
        key "enh.enhanced_reflections", presets reset, write 23,
        dev [24 flag "enhanced reflections"],
        apply [23 store ENHANCED_REFLECTIONS],
        parse bool;

    /// Ambient occlusion strength, percent (0 = off, native).
    ssao: u8 = 0,
        key "enh.ssao", presets reset, write 25,
        dev [26 fmt |x| format!("SSAO {}%", x)],
        apply [25 store SSAO],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| nearest(&SSAO_STEPS, v.min(100) as u8)));

    /// High Precision Post (docs/notes/graphics/hdr-post.md): 0 off (native, the 8-bit
    /// scene and screen passes), 1 the scene and the screen passes in
    /// half-float with the overbright glow at half size (filtered), 2 the
    /// same with the glow at the screen's size.
    hdr_post: u8 = 0,
        key "enh.hdr_post", presets reset, write 26,
        dev [27 tiers ["HP post (half glow)", "HP post (full glow)"]],
        apply [26 store HDR_POST],
        parse (|old, v| min_u32(old, v, 2));

    /// Enhanced Car Paint (clone deviation, docs/notes/graphics/car-paint-enhanced.md):
    /// clearcoat, metallic flake and pearl shift per NFSU2 paint type.
    /// Off = native car.efx.
    enhanced_paint: bool = false,
        key "enh.enhanced_paint", presets reset, write 27,
        dev [28 flag "enhanced paint"],
        apply [27 store ENHANCED_PAINT],
        parse bool;

    /// Lists Open on Installed (docs/formats.md "List openings"): the
    /// Custom Gauges list and its colour steps open on the fitted gauge /
    /// colour. Off = native (0x56C220 → 0x53C200 selects STOCK, 0x549290
    /// the first swatch).
    lists_open_installed: bool = false,
        key "enh.lists_open_installed", presets keep, write 28,
        dev [29 flag "lists open on installed"],
        apply [28 store LISTS_OPEN_INSTALLED],
        parse bool;

    /// Outrun Variety (docs/notes/parity/drag-drift-outrun-parity.md §0): a parked
    /// encounter racer that has been on the road gets a new car for its
    /// slot, drawn by the set-up's own rule (same pool, player rating,
    /// skill rows, performance match and dress). Off = native: the
    /// set-up's three cars until the next free-roam load.
    outrun_variety: bool = false,
        key "enh.outrun_variety", presets keep, write 29,
        dev [30 flag "outrun variety"],
        apply [29 store OUTRUN_VARIETY],
        parse bool;

    /// Wet Roads (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"): the road reflection with ripples,
    /// normal distortion, roughness blur, vertical stretch and puddles,
    /// driven by the rain. Off = native.
    wet_roads: bool = false,
        key "enh.wet_roads", presets reset, write 30,
        dev [31 flag "wet roads"],
        apply [30 store WET_ROADS],
        parse bool;

    /// Screen-Space Reflections (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"): 0 off (native),
    /// 1 traced at half the scene's size, 2 at its full size, 3 at full
    /// size with ray queries for what the screen misses (see
    /// docs/notes/graphics/reflections.md "Ray-traced reflections (experimental)"; SSR Ultra where the device cannot trace).
    ssr: u8 = 0,
        key "enh.ssr", presets reset, write 37,
        dev [35 tiers ["SSR half", "SSR full", "ray-traced reflections"]],
        apply [37 store SSR],
        parse (|old, v| min_u32(old, v, 3));

    /// HUD Layout (docs/playing/ultrawide.md): on screens wider than 16:9 the
    /// HUD's side elements stay in a centred 16:9 safe area (false,
    /// "Centered", the default) or go out to the screen's edges (true,
    /// "Full Width"). No effect at 16:9 and narrower.
    hud_full_width: bool = false,
        key "enh.hud_full_width", presets keep, write 38,
        dev [3 flag "HUD full width"],
        apply [67 call crate::draw::ui_frame::set_hud_full_width],
        parse bool;

    /// Dynamic Lamps (docs/notes/graphics/lamps-shadows.md): the light packs' lamps as
    /// clustered per-pixel lights added to the world and the cars. Off =
    /// native.
    dynamic_lamps: bool = false,
        key "enh.dynamic_lamps", presets reset, write 39,
        dev [36 flag "dynamic lamps"],
        apply [38 store DYNAMIC_LAMPS],
        parse bool;

    /// Lamp Shadows (docs/notes/graphics/lamps-shadows.md): 0 off (native), 1 Low (4
    /// lamps, 512² maps, one tap), 2 High (12 lamps, 1024², 3×3 taps);
    /// with the player's headlight and the cars as soft occluders.
    lamp_shadows: u8 = 0,
        key "enh.lamp_shadows", presets reset, write 40,
        dev [37 tiers ["lamp shadows low", "lamp shadows high"]],
        apply [39 store LAMP_SHADOWS],
        parse (|old, v| min_u32(old, v, 2));

    /// Contact Shadows (docs/notes/graphics/lamps-shadows.md): soft occlusion under the
    /// cars, under the native shadow texture. Off = native.
    contact_shadows: bool = false,
        key "enh.contact_shadows", presets reset, write 41,
        dev [38 flag "contact shadows"],
        apply [40 store CONTACT_SHADOWS],
        parse bool;

    /// Upscaling (docs/notes/graphics/temporal.md): 0 off (native), 1 Quality (67 %),
    /// 2 Balanced (58 %), 3 Performance (50 %): the scene drawn at that
    /// share of the screen and reconstructed to it by the temporal pass
    /// (TAA's history). Turns the temporal path on whatever the
    /// Anti-Aliasing row says (MSAA / SSAA / post filters off).
    /// 4–6 FSR 1; 7–11 FSR 2 Native AA / Quality / Balanced /
    /// Performance / Ultra Performance (docs/notes/graphics/fsr2.md), on the same
    /// temporal path.
    upscale: u8 = 0,
        key "enh.upscale", presets reset, write 42,
        dev [39 custom |s, _, v| if let Some(u) = upscale_name(s.upscale) { v.push(format!("upscaling {u}")) }],
        apply [41 store UPSCALE],
        parse (|old, v| min_u32(old, v, UPSCALE_MAX));

    /// Volumetric Fog (docs/notes/graphics/temporal.md): 0 off (native), 1 Low (light
    /// cones under the light packs' lamps), 2 High (a froxel grid lit by
    /// the lamps and the headlights, thicker in rain).
    volumetric_fog: u8 = 0,
        key "enh.volumetric_fog", presets reset, write 43,
        dev [40 tiers ["volumetric fog low", "volumetric fog high", "volumetric fog next-gen"]],
        apply [42 store VOLUMETRIC_FOG],
        parse (|old, v| min_u32(old, v, 3));

    /// Sky (docs/notes/graphics/atmosphere.md): 0 Original (the native SKYDOME), 1
    /// Next-Gen (a procedural night sky: moon, stars, drifting clouds lit
    /// by the city, in the dome's palette; `render::sky_ng`).
    sky: u8 = 0,
        key "enh.sky", presets reset, write 44,
        dev [41 tiers ["sky next-gen"]],
        apply [43 store SKY],
        parse (|old, v| min_u32(old, v, 1));

    /// Particles (docs/notes/graphics/smoke.md "Particles High and Ultra: GPU particles and detailed smoke"): 0 Original (the native
    /// quads), 1 High, 2 Ultra (the native smoke expanded into detailed
    /// GPU puffs, `render::gpu_particles`; emit rules unchanged), 3
    /// Cinematic (docs/notes/graphics/smoke.md "Particles Cinematic: the simulation and the billboard drawing": `render::smoke_cinematic`).
    particles: u8 = 0,
        key "enh.particles", presets reset, write 45,
        dev [42 tiers ["particles high", "particles ultra", "particles cinematic"]],
        apply [44 store PARTICLES],
        parse (|old, v| min_u32(old, v, 3));

    /// Sparks & Fire (docs/notes/graphics/sparks-fire.md): 0 Original (the native
    /// quads), 1 Next-Gen (`render::sparks_fire`: the native spark and
    /// flame births simulated and drawn as lit, emissive, light-casting
    /// sparks and flames with heat haze; emit rules unchanged).
    sparks_fire: u8 = 0,
        key "enh.sparks_fire", presets reset, write 46,
        dev [43 tiers ["sparks & fire next-gen"]],
        apply [45 store SPARKS_FIRE],
        parse (|old, v| min_u32(old, v, 1));

    /// Animated Water (docs/notes/graphics/water.md): off (native: the original's
    /// flip-book and scrolling water textures) or on (`render::water`:
    /// flowing, rippled, lit and reflecting fountain basins, pools and
    /// streams, moving falling-water sheets, spray and mist); 2 Realistic
    /// (docs/notes/graphics/water.md "Realistic": screen-space reflections,
    /// refraction and absorption of what lies under the water, caustics,
    /// volumetric-looking jets, wet stone around the basins;
    /// `render::water_hq`); 3 Next-Gen (docs/notes/graphics/water.md "Next-Gen":
    /// the jets breaking into simulated drops, simulated basins,
    /// volumetric mist; `render::water_ng`).
    water: u8 = 0,
        key "enh.water", presets reset, write 54,
        dev [51 tiers ["animated water", "animated water realistic", "animated water next-gen"]],
        apply [53 store WATER],
        parse (|_, v| {
            // Override: a value that is not a number reads as
            // `!= "0"` (the old on/off row), not as the old value.
            v.parse::<u32>().map_or(u8::from(v != "0"), |v| v.min(3) as u8)
        });

    /// Ambient Detail (docs/notes/graphics/ambient-detail.md, clone deviation): 0 Off
    /// (native), 1 On (low-poly street props and litter off the race line,
    /// lit by the lamps and headlights), 2 Next-Gen (also leaves and paper
    /// whirled up by passing cars and the wind, tyre dust on dry roads,
    /// steam from the road grates, moths round the lamps; visual only).
    ambient_detail: u8 = 0,
        key "enh.ambient_detail", presets reset, write 55,
        dev [52 tiers ["ambient detail", "ambient detail next-gen"]],
        apply [54 store AMBIENT_DETAIL],
        parse (|old, v| min_u32(old, v, 2));

    /// Weather (docs/notes/graphics/weather.md): Wet Car (the cars wet with the rain
    /// and road spray), Rain Detail (0 off, 1 On, 2 High: fine lit rain
    /// streaks, ground splashes, wet-air glow around lamps), Spray (wheel
    /// spray and mist behind cars on wet roads), Lens Drops (extra drops on
    /// the screen, wiped by the speed). All off = the native weather look.
    wet_car: bool = false,
        key "enh.wet_car", presets reset, write 47,
        dev [44 flag "wet car"],
        apply [46 store WET_CAR],
        parse bool;

    rain_detail: u8 = 0,
        key "enh.rain_detail", presets reset, write 48,
        dev [45 tiers ["rain detail", "rain detail high", "rain detail next-gen"]],
        apply [47 store RAIN_DETAIL],
        parse (|old, v| min_u32(old, v, RAIN_NEXT_GEN));

    spray: bool = false,
        key "enh.spray", presets reset, write 49,
        dev [46 flag "spray and splashes"],
        apply [48 store SPRAY],
        parse bool;

    lens_drops: bool = false,
        key "enh.lens_drops", presets reset, write 50,
        dev [47 flag "lens drops"],
        apply [49 store LENS_DROPS],
        parse bool;

    /// Wet Road Interaction (docs/notes/graphics/wet-road-car.md): 0 off, 1 On, 2
    /// High. Tyre tracks and wakes on the wet road, the cars' lights
    /// reflected in it, puddle splashes, rooster tails, arch spray and
    /// water sheeting off the body. Off = the native look.
    wet_interaction: u8 = 0,
        key "enh.wet_interaction", presets reset, write 51,
        dev [48 tiers ["wet road interaction", "wet road interaction high"]],
        apply [50 store WET_INTERACTION],
        parse (|old, v| min_u32(old, v, 2));

    /// Fix Original Model Errors (docs/notes/graphics/model-fixes.md): small load-time
    /// corrections of errors in the original car and rim models (markers,
    /// UVs, normals, light materials). Off = the files as shipped.
    fix_models: bool = false,
        key "enh.fix_models", presets keep, write 35,
        dev [33 flag "model fixes"],
        apply [31 store FIX_MODELS],
        parse bool;

    /// HDR Output (docs/playing/hdr.md; Graphics → Display): the swap chain in an
    /// HDR signal (PQ or scRGB) where the display offers one. Off = SDR,
    /// the native image.
    hdr_output: bool = false,
        key "enh.hdr_output", presets keep, write 31,
        dev [32 flag "HDR output"],
        apply [33 store HDR_OUTPUT],
        parse bool;

    /// HDR calibration: peak brightness and paper white in nits (0 = Auto:
    /// what the display reports, else 1000 / 203), and the SDR transfer
    /// (0 gamma 2.2, 1 sRGB, 2 gamma 2.4).
    hdr_peak: u16 = 0,
        key "enh.hdr_peak", presets keep, write 32,
        dev none,
        apply [34 store HDR_PEAK],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| if v == 0 { 0 } else { nearest(&HDR_PEAK_STEPS, v.min(10000) as u16) }));

    hdr_paper: u16 = 0,
        key "enh.hdr_paper", presets keep, write 33,
        dev none,
        apply [35 store HDR_PAPER],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| if v == 0 { 0 } else { nearest(&HDR_PAPER_STEPS, v.min(1000) as u16) }));

    hdr_gamma: u8 = 0,
        key "enh.hdr_gamma", presets keep, write 34,
        dev none,
        apply [36 store HDR_GAMMA],
        parse (|old, v| min_u32(old, v, 2));

    /// Motion Blur's clone tiers (docs/notes/graphics/motionblur-dof.md): 0 the
    /// original's speed blur (Customize Display's Motion Blur, native), 1
    /// Camera, 2 Per-Object.
    motion_blur: u8 = 0,
        key "enh.motion_blur", presets reset, write 52,
        dev [49 tiers ["motion blur camera", "motion blur per-object"]],
        apply [51 store MOTION_BLUR],
        parse (|old, v| min_u32(old, v, 2));

    /// Depth Of Field's clone tier: 0 the original's switch
    /// (native), 1 Cinematic (scenes, replays, garage, photo mode).
    depth_of_field: u8 = 0,
        key "enh.depth_of_field", presets reset, write 53,
        dev [50 tiers ["depth of field cinematic"]],
        apply [52 store DEPTH_OF_FIELD],
        parse (|old, v| min_u32(old, v, 1));

    /// Photo Mode in Pause Menu (docs/playing/photo-mode.md): the pause menu
    /// lists "Photo Mode". Off = the native pause menu.
    photo_mode: bool = false,
        key "enh.photo_mode", presets keep, write 36,
        dev [34 flag "photo mode"],
        apply [32 store PHOTO_MODE],
        parse bool;

    /// docs/notes/graphics/lighting2.md: Headlight Beams — every car's headlights
    /// as a projected beam (soft cone, brighter centre, near cut-off) lighting
    /// the road and walls; the player's with the Lamp Shadows' headlight shadow. Off =
    /// native (the player's view-9 projector only). Next-Gen car lights (docs/notes/graphics/car-lights.md):
    /// 2 Next-Gen — also the car lights in the air (volumetric cones and
    /// halos), tail / brake / underglow light on the road, lens streaks and
    /// the cars lit by each other's lights.
    headlight_beams: u8 = 0,
        key "enh.headlight_beams", presets reset, write 56,
        dev [53 tiers ["headlight beams", "car lights next-gen"]],
        apply [55 store HEADLIGHT_BEAMS],
        parse (|_, v| {
            // Override: a value that is not a number reads as
            // `!= "0"` (the old on/off row), not as the old value.
            v.parse::<u32>().map_or(u8::from(v != "0"), |v| v.min(2) as u8)
        });

    /// Light Probes — probes along the roads baked from the world's own
    /// rendering; the nearest blended into the cars' light (harmonic). Off =
    /// native (the light packs and the rig only).
    light_probes: bool = false,
        key "enh.light_probes", presets reset, write 57,
        dev [54 flag "light probes"],
        apply [56 store LIGHT_PROBES],
        parse bool;

    /// Enhanced Glass — car windows and lamp covers with the native
    /// tint as base, refraction of what is behind and a per-pixel reflection.
    /// Off = native car.efx glass.
    enhanced_glass: bool = false,
        key "enh.enhanced_glass", presets reset, write 58,
        dev [55 flag "enhanced glass"],
        apply [57 store ENHANCED_GLASS],
        parse bool;

    /// docs/notes/graphics/surface-maps.md: Surface Detail — 0 Off (native), 1
    /// Materials (material-ID roughness / specular for the world's
    /// textures), 2 Materials + Normals (with normal maps generated from
    /// the player's own textures). Feeds the headlight projector, Dynamic
    /// Lamps, Headlight Beams and Wet Roads.
    surface_detail: u8 = 0,
        key "enh.surface_detail", presets reset, write 59,
        dev [57 tiers ["surface detail materials", "surface detail materials + normals"]],
        apply [58 store SURFACE_DETAIL],
        parse (|old, v| min_u32(old, v, 2));

    /// docs/notes/graphics/modern-lighting.md: Modern Lighting — 0 Original (the
    /// baked lighting, native), 1 Low (integrated GPUs), 2 Modern, 3 Modern
    /// + RT (ray-query shadows; Modern where the device cannot trace).
    modern_lighting: u8 = 0,
        key "enh.modern_lighting", presets reset, write 60,
        dev [56 custom |s, _, v| match s.modern_lighting {
            0 => {}
            1 => v.push(format!("modern lighting low {}%", s.modern_strength)),
            2 => v.push(format!("modern lighting {}%", s.modern_strength)),
            _ => v.push(format!("modern lighting rt {}%", s.modern_strength)),
        }],
        apply [59 store MODERN_LIGHTING],
        parse (|old, v| min_u32(old, v, 3));

    /// Modern Lighting's strength, 0 (classic: the bake relit) to
    /// 100 (bold: dynamic pools, deeper darkness), in steps of 25.
    modern_strength: u8 = MODERN_STRENGTH_DEFAULT,
        key "enh.modern_strength", presets reset, write 61,
        dev none,
        apply [60 store MODERN_STRENGTH],
        parse (|old, v| v.parse::<u32>().map_or(old, |v| (v.min(100) / 25 * 25) as u8));

    /// docs/playing/modern-extras.md: Low Latency, Replay Cameras and the
    /// Accessibility rows (QoL: presets leave them alone).
    extras: crate::extras::Extras = crate::extras::Extras::default(),
        key nested, presets keep, write 62,
        dev [58 custom |s, _, v| s.extras.deviations(v)],
        apply [61 custom |e| { record("extras.apply", &e.extras); e.extras.apply() }],
        parse nested;

    /// docs/playing/controller-support.md: the Controller group (layout,
    /// dead zones, saturation, steering linearity, rumble, pause on
    /// disconnect; QoL: presets leave them alone).
    pad: crate::pad::extras::PadOptions = crate::pad::extras::PadOptions::default(),
        key nested, presets keep, write 63,
        dev [59 custom |s, _, v| s.pad.deviations(v)],
        apply none,
        parse nested;

    /// Smooth Car Shapes (docs/notes/graphics/assets-distance.md; Graphics → Cars):
    /// low-poly silhouettes rounded at load (`car_smooth`). Off = the
    /// models as shipped.
    smooth_cars: bool = false,
        key "enh.smooth_cars", presets reset, write 64,
        dev [60 flag "smooth car shapes"],
        apply [62 store SMOOTH_CARS],
        parse bool;

    /// World Detail Extreme (docs/notes/graphics/assets-distance.md): beyond Ultra's
    /// look-ahead ring, the sections the native lists would show from
    /// within [`FAR_RING`] are drawn with their lowest LOD. Off = native.
    far_draw: bool = false,
        key "enh.far_draw", presets reset, write 65,
        dev [61 flag "world detail extreme"],
        apply [63 store FAR_DRAW],
        parse bool;

    /// Traffic Density (Enhancements, gameplay): 0 Native, 1 More
    /// (1.5× the free-roam pool), 2 Dense (2×). Races keep the native pool.
    traffic_density: u8 = 0,
        key "enh.traffic_density", presets keep, write 66,
        dev [62 tiers ["traffic more", "traffic dense"]],
        apply [64 store TRAFFIC_DENSITY],
        parse (|old, v| min_u32(old, v, 2));

    /// docs/modding/texture-pack-generator.md: World Texture Pack — 0 Off
    /// (native), 1 On (the pack generated into the player's data dir; on a
    /// GPU under 8 GB the buildings' textures are held to 2×), 2 Lean (held
    /// to 2× always). Presets leave it alone (the pack is the player's own).
    texture_pack: u8 = 0,
        key "enh.texture_pack", presets keep, write 68,
        dev [63 tiers ["texture pack", "texture pack lean"]],
        apply [65 call crate::upscale_pack::set_choice],
        parse (|old, v| min_u32(old, v, 2));

    /// docs/original/loading-image.md: the least time PC_Loading stays up over
    /// the game start's load, ms (opt-in). 0 = the 200 ms rule: shown only
    /// when the start takes over 200 ms, up until it is done.
    /// No menu row yet (options.txt only); presets leave it alone.
    boot_loading_min_ms: u16 = 0,
        key "enh.boot_loading_min_ms", presets keep, write 69,
        dev none,
        apply none,
        parse (|old, v| v.parse::<u32>().map_or(old, |v| v.min(u32::from(BOOT_LOADING_MAX_MS)) as u16));

    /// L1 (docs/launcher.md): the launcher (`overbright`) starts the game
    /// at once next time, without its window (Shift, pad Back or
    /// `--launcher` still show it). Declared here so the game, which
    /// rewrites options.txt from its own model, keeps the line. Off =
    /// native (no launcher at all: the window shows). No game-side row;
    /// presets leave it alone.
    launcher_skip: bool = false,
        key "launcher.skip", presets keep, write 70,
        dev none,
        apply none,
        parse bool;

    /// docs/original/start-and-menus.md: Skip Title Screen (QoL, opt-in) — 0 Off
    /// (native: the title screen, which takes Enter only after its 4 s,
    /// 0x4C564E), 1 On (the profile step where the title stood). 2 is an
    /// older options.txt's "Movies and Title Screen": read as this row On
    /// and Skip Intro Movies On (`Settings::from_text`,
    /// [`Enhancements::set_skips`]). Presets leave it alone.
    skip_title: u8 = 0,
        key "enh.skip_title", presets keep, write 71,
        dev [64 custom |s, _, v| if s.skips_title() { v.push("skip title".into()) }],
        apply none,
        parse (|old, v| min_u32(old, v, 2));

    /// docs/original/start-and-menus.md: Skip Intro Movies (QoL, opt-in) — Off
    /// (native: the boot movies, which Enter skips), On (each boot movie
    /// passes as a missing movie file does, 0x566250). Independent of
    /// Skip Title Screen. Presets leave it alone.
    skip_movies: bool = false,
        key "enh.skip_movies", presets keep, write 72,
        dev [65 custom |s, _, v| if s.skips_movies() { v.push("skip movies".into()) }],
        apply none,
        parse bool;
}

impl Enhancements {
    /// Skip Title Screen in force (`skip_title` 1, or an older file's 2).
    pub fn skips_title(&self) -> bool {
        self.skip_title >= 1
    }

    /// Skip Intro Movies in force (`skip_movies`, or an older file's
    /// `enh.skip_title 2`, "Movies and Title Screen").
    pub fn skips_movies(&self) -> bool {
        self.skip_movies || self.skip_title >= 2
    }

    /// Both skips as two independent values (`skip_title` 0 / 1,
    /// `skip_movies`): an older file's `enh.skip_title 2` becomes 1 + On.
    pub fn set_skips(&mut self, movies: bool, title: bool) {
        self.skip_movies = movies;
        self.skip_title = u8::from(title);
    }
}

/// The longest `enh.boot_loading_min_ms` (30 s).
pub const BOOT_LOADING_MAX_MS: u16 = 30_000;

/// The values each row steps through (Left / Right, wrapping like the
/// native cycle rows).
pub const FOV_STEPS: [i8; 9] = [-10, -5, 0, 5, 10, 15, 20, 25, 30];
pub const HUD_STEPS: [u8; 11] = [50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150];
pub const CAP_STEPS: [u16; 9] = [0, 30, 60, 75, 90, 120, 144, 165, 240];
pub const BACKUP_STEPS: [u8; 5] = [0, 1, 3, 5, 10];
pub const SCALE_STEPS: [u8; 9] = [50, 67, 75, 85, 100, 125, 150, 175, 200];
pub const AA_STEPS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, TAA];
/// The Anti-Aliasing row's TAA (docs/notes/graphics/temporal.md): the temporal pass,
/// exclusive with MSAA / SSAA and the post filters.
pub const TAA: u8 = 11;
/// Upscaling's render shares: off, Quality, Balanced, Performance.
/// Off, TAAU Quality / Balanced / Performance, FSR 1 Quality /
/// Balanced / Performance (docs/playing/modern-extras.md), FSR 2 Native AA /
/// Quality / Balanced / Performance / Ultra Performance (docs/notes/graphics/fsr2.md).
pub const UPSCALE_STEPS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// The Upscaling row's last value (FSR 2 Ultra Performance).
pub const UPSCALE_MAX: u8 = 11;
/// 3 Next-Gen (docs/notes/graphics/atmosphere.md).
pub const VOLUMETRIC_STEPS: [u8; 4] = [0, 1, 2, 3];
pub const PARTICLE_STEPS: [u8; 4] = [0, 1, 2, 3];
pub const MSAA_STEPS: [u8; 6] = [0, 1, 2, 4, 8, 16];
pub const FILTER_STEPS: [u8; 5] = [0, 1, 2, 3, 4];
/// The tiers' stored values (`frontend::graphics_table`): Off / Low /
/// Medium / High etc. Each step was measured visibly different
/// (docs/notes/graphics/graphics-settings-audit.md "Tiers"); a saved value between two
/// maps to the nearest ([`nearest`]).
pub const SHARPEN_STEPS: [u8; 4] = [0, 25, 75, 100];
pub const ROAD_REFLECTION_STEPS: [u8; 3] = [0, 1, 2];
pub const CAR_REFLECTION_STEPS: [u16; 3] = [128, 256, 1024];
pub const ANISO_STEPS: [u8; 5] = [0, 2, 4, 8, 16];
/// Native and Ultra (the filtered edge; the old High, 1, was 0.13 % of
/// pixels from Ultra and loads as Ultra).
pub const SHADOW_STEPS: [u8; 2] = [0, 2];
pub const SSAO_STEPS: [u8; 4] = [0, 25, 50, 100];
/// Detail Distance: 300 % popped no less than 200 % (drive bench), so
/// 200 % is the top.
pub const DETAIL_STEPS: [u16; 3] = [100, 150, 200];
/// docs/notes/graphics/pop-in.md: World Detail Ultra / Extreme draw scenery down to
/// this radius in output pixels (a 3-pixel-wide speck), whatever the
/// Detail Distance percentage gives at the resolution.
pub const DETAIL_FULL_PX: f32 = 1.5;
/// Car Detail Distance: 150 % changed one LOD switch of ten, 200 % four.
pub const CAR_DETAIL_STEPS: [u16; 2] = [100, 200];
pub const HDR_POST_STEPS: [u8; 3] = [0, 1, 2];
pub const SSR_STEPS: [u8; 3] = [0, 1, 2];

/// The value of `list` (ascending) nearest `v`, the higher one on a tie;
/// any value above the first maps above it (an opt-in stays on).
pub fn nearest<T: Copy + Into<f64>>(list: &[T], v: T) -> T {
    let x: f64 = v.into();
    let mut best = list[0];
    for &c in list {
        if (c.into() - x).abs() <= (best.into() - x).abs() {
            best = c;
        }
    }
    if x > list[0].into() && best.into() == list[0].into() && list.len() > 1 { list[1] } else { best }
}

/// The next value of `list` after `v` toward `right`, wrapping; a value not
/// listed goes to the first listed one past it that way.
pub fn step_in<T: Copy + PartialOrd>(list: &[T], v: T, right: bool) -> T {
    if right {
        list.iter().copied().find(|&x| x > v).unwrap_or(list[0])
    } else {
        list.iter().rev().copied().find(|&x| x < v).unwrap_or(list[list.len() - 1])
    }
}

impl Enhancements {
    /// docs/developing/settings-model.md "Engine rules": these values with the
    /// stacks the conflict audit resolved, as the renderer uses them
    /// ([`Enhancements::apply`]); the settings page shows the same rules
    /// (`frontend::settings_model::RULES`). A no-op on native values.
    /// * an FSR 1 tier keeps the native MSAA: SSAA and TAA on the
    ///   Anti-Aliasing row are not drawn under it (SSAA left the scene
    ///   without MSAA, TAA turned FSR 1 into the temporal upscaler);
    /// * FSR 1 and FSR 2 sharpen with their own RCAS: no second CAS pass
    ///   (halos; p95 +3.7..+5.0 at the racing bench's frames);
    /// * Modern Lighting relights the street lamps: Dynamic Lamps off
    ///   (0 pixels differ at the look spots);
    /// * Wet Road Interaction includes Wet Roads' surface and the spray
    ///   (0 pixels differ), so both are on under it.
    pub fn effective(&self) -> Enhancements {
        let mut e = *self;
        if is_fsr(e.upscale) && matches!(e.post_aa, 6..=10 | TAA) {
            e.post_aa = 0;
        }
        if is_fsr(e.upscale) || is_fsr2(e.upscale) {
            e.sharpen = 0;
        }
        if e.modern_lighting != 0 {
            e.dynamic_lamps = false;
        }
        if e.wet_interaction != 0 {
            e.wet_roads = true;
            e.spray = true;
        }
        e
    }
}

impl Enhancements {
    pub fn is_native(&self) -> bool {
        *self == Enhancements::default()
    }

    /// The preset these graphics values are, if any (None: "Custom").
    pub fn preset(&self) -> Option<Preset> {
        PRESETS.into_iter().find(|p| {
            let mut e = *self;
            p.apply(&mut e);
            e == *self
        })
    }


    /// The deviations switched on, for the F3 overlay and crash reports.
    pub fn deviations(&self) -> Vec<String> {
        let d = Enhancements::default();
        let mut v = Vec::new();
        for r in &orders().dev {
            (r.deviation)(self, &d, &mut v);
        }
        v
    }

    /// options.txt lines for the values that are not native.
    pub fn write(&self, out: &mut String) {
        let d = Enhancements::default();
        for r in &orders().write {
            (r.write)(self, &d, out);
        }
    }

    /// An options.txt line's key and value; false when the key is not one
    /// of these. Values out of range are held to the rows' ranges.
    pub fn parse(&mut self, key: &str, value: &str) -> bool {
        let value = value.trim();
        if OPTION_ROWS.iter().filter(|r| r.key.is_some()).any(|r| (r.parse)(self, key, value)) {
            return true;
        }
        if LEGACY_KEYS.contains(&key) {
            return true;
        }
        OPTION_ROWS.iter().filter(|r| r.key.is_none()).any(|r| (r.parse)(self, key, value))
    }
}

#[cfg(test)]
mod oracle;
#[cfg(test)]
mod table_tests;
