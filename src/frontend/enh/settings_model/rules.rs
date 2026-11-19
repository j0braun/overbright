use super::*;

/// What a rule does to its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The whole row does nothing while `by` is on: disabled, "Overridden
    /// by X".
    Overridden,
    /// Some tiers do nothing or conflict while `by` is on ([`Rule::blocks`]):
    /// not offered; the current one, if blocked, reads "… (off: X)".
    Blocks,
    /// The row needs `by` on to act at all: disabled, "Needs X".
    Needs,
    /// The row acts only in part without `by`: enabled, the value reads
    /// "… (X off)" and the detail line says what is missing.
    Partial,
}

/// A dependency or exclusion between rows.
pub struct Rule {
    pub row: RowId,
    /// The row that overrides or is needed (a `RowId::Custom(n)` for the
    /// original's Customize Display row n).
    pub by: RowId,
    pub effect: Effect,
    /// Whether the rule is in force for these values.
    pub when: fn(&Settings) -> bool,
    /// The tiers `Effect::Blocks` takes out (tier indices of the row).
    pub blocks: Option<fn(usize) -> bool>,
    /// The overriding row's short name for the value text.
    pub by_name: &'static str,
    /// What the player reads (the detail line, plain words).
    pub reason: &'static str,
    /// The renderer resolves it too ([`crate::qol::Enhancements::effective`]
    /// or the pass's own code), not only the page.
    pub engine: bool,
    /// The audit's measurement (docs/developing/settings-model.md "Audit").
    pub evidence: &'static str,
}

fn temporal_upscale(s: &Settings) -> bool {
    matches!(s.enh.upscale, 1..=3)
}

/// The rules. Order matters: the first rule in force for a row is the one
/// shown.
pub static RULES: &[Rule] = &[
    Rule {
        row: RowId::PostAa,
        by: RowId::Upscale,
        effect: Effect::Overridden,
        when: temporal_upscale,
        blocks: None,
        by_name: "Upscaling (TAA)",
        reason: "Upscaling's temporal reconstruction is the anti-aliasing; this row's choice is not drawn while it is on.",
        engine: false,
        evidence: "racing bench frames 240/420: SSAA 2x under Upscaling Quality 0 pixels differ",
    },
    Rule {
        row: RowId::PostAa,
        by: RowId::Upscale,
        effect: Effect::Overridden,
        when: |s| qol::is_fsr2(s.enh.upscale),
        blocks: None,
        by_name: "FSR 2",
        reason: "FSR 2 is the anti-aliasing (it replaces SSAA, MSAA and the filters).",
        engine: false,
        evidence: "racing frames: SSAA 2x under FSR 2 Quality 0.00-0.008 % of pixels (max 28 levels)",
    },
    Rule {
        row: RowId::PostAa,
        by: RowId::Upscale,
        effect: Effect::Blocks,
        when: |s| qol::is_fsr(s.enh.upscale),
        blocks: Some(|i| matches!(i as u8, 6..=10 | qol::TAA)),
        by_name: "FSR 1",
        reason: "FSR 1 upscales one frame and keeps MSAA: SSAA would only remove the MSAA, TAA would turn it into the temporal upscaler. MSAA, FXAA and SMAA still apply.",
        engine: true,
        evidence: "racing frames: SSAA 2x under FSR 1 Balanced 6.6-7.4 % of pixels changed (MSAA lost), TAA under FSR 1 31-35 % (= Upscaling's TAA, 0.5 % from it)",
    },
    Rule {
        row: RowId::AaMsaa,
        by: RowId::PostAa,
        effect: Effect::Overridden,
        when: |s| qol::temporal_for(&s.enh),
        blocks: None,
        by_name: "TAA",
        reason: "The temporal pass (TAA, Upscaling or FSR 2) is the one anti-aliasing: no multisampling under it.",
        engine: false,
        evidence: "racing frames: MSAA 8x override under TAA 0 pixels differ",
    },
    Rule {
        row: RowId::AaFilter,
        by: RowId::PostAa,
        effect: Effect::Overridden,
        when: |s| qol::temporal_for(&s.enh),
        blocks: None,
        by_name: "TAA",
        reason: "The temporal pass (TAA, Upscaling or FSR 2) is the one anti-aliasing: no FXAA / SMAA under it.",
        engine: false,
        evidence: "qol::fxaa / smaa are off while temporal() (code)",
    },
    Rule {
        row: RowId::RenderScale,
        by: RowId::Upscale,
        effect: Effect::Overridden,
        when: |s| qol::upscale_name(s.enh.upscale).is_some(),
        blocks: None,
        by_name: "Upscaling",
        reason: "Upscaling sets the scene's resolution.",
        engine: false,
        evidence: "qol::scene_size: the upscale share first (code)",
    },
    Rule {
        row: RowId::RenderScale,
        by: RowId::PostAa,
        effect: Effect::Overridden,
        when: |s| qol::ssaa_scale(s.enh.post_aa).is_some(),
        blocks: None,
        by_name: "SSAA",
        reason: "SSAA sets the scene's resolution.",
        engine: false,
        evidence: "qol::scene_scale: the SSAA scale in place of the Render Scale (code)",
    },
    Rule {
        row: RowId::Sharpen,
        by: RowId::Upscale,
        effect: Effect::Overridden,
        when: |s| qol::is_fsr(s.enh.upscale) || qol::is_fsr2(s.enh.upscale),
        blocks: None,
        by_name: "FSR's sharpening",
        reason: "FSR sharpens with its own pass (RCAS); a second sharpening on top only adds halos.",
        engine: true,
        evidence: "racing frames: Medium over FSR 1 47-53 % of pixels, p95 +0.9/+3.7, p5 -1.6/-2.1; over FSR 2 31-35 %, p95 +1.2/+5.0 (R3 ±4)",
    },
    Rule {
        row: RowId::DynamicLamps,
        by: RowId::ModernLighting,
        effect: Effect::Overridden,
        when: |s| s.enh.modern_lighting != 0,
        blocks: None,
        by_name: "Modern Lighting",
        reason: "Modern Lighting lights the street lamps itself (round pools, shadows); Dynamic Lamps adds nothing on top.",
        engine: true,
        evidence: "4 look-direction spots + race start: Modern + Dynamic Lamps vs Modern 0 pixels differ",
    },
    Rule {
        row: RowId::ModernStrength,
        by: RowId::ModernLighting,
        effect: Effect::Needs,
        when: |s| s.enh.modern_lighting == 0,
        blocks: None,
        by_name: "Modern Lighting",
        reason: "The strength of Modern Lighting; nothing to set while it is off.",
        engine: false,
        evidence: "modern_light_bold reads it only with Modern Lighting on (code)",
    },
    Rule {
        row: RowId::LampShadows,
        by: RowId::ModernLighting,
        effect: Effect::Blocks,
        when: |s| s.enh.modern_lighting != 0,
        blocks: Some(|i| i == 2),
        by_name: "Modern Lighting",
        reason: "Modern Lighting draws the lamps' shadows itself; this row keeps only your headlights' shadow (Low).",
        engine: false,
        evidence: "Low under Modern: 0.03-4.4 % of pixels (the headlight shadow, street -1.0 mean); High vs Low under Modern 0.13-0.82 %, max 54 levels",
    },
    Rule {
        row: RowId::Spray,
        by: RowId::WetInteraction,
        effect: Effect::Overridden,
        when: |s| s.enh.wet_interaction != 0,
        blocks: None,
        by_name: "Wet Road Interaction",
        reason: "Wet Road Interaction includes the spray (denser, volumetric).",
        engine: true,
        evidence: "rain street / harbour: Wet Road Interaction On + Spray vs without 0 pixels differ",
    },
    Rule {
        row: RowId::WetRoads,
        by: RowId::WetInteraction,
        effect: Effect::Overridden,
        when: |s| s.enh.wet_interaction != 0,
        blocks: None,
        by_name: "Wet Road Interaction",
        reason: "Wet Road Interaction includes the wet road surface.",
        engine: true,
        evidence: "rain spots: 0 pixels differ (Rain Detail Next-Gen's puddle rings 0.19-0.31 %: now on under Wet Road Interaction)",
    },
    Rule {
        row: RowId::Ssr,
        by: RowId::RoadReflection,
        effect: Effect::Partial,
        when: |s| s.enh.ssr != 0 && s.custom[7] == 0,
        blocks: None,
        by_name: "road reflection off",
        reason: "Road Reflection Detail is Off: only the car paint gets screen-space reflections, the roads need the road reflection on (Low or higher).",
        engine: false,
        evidence: "SSR Ultra: 6.9-10.2 % of pixels with the road reflection, 0.16-0.63 % (cars) without; the cost stays (3-5 ms at 4K)",
    },
    Rule {
        row: RowId::Particles,
        by: RowId::Custom(10),
        effect: Effect::Needs,
        when: |s| s.custom[10] == 0,
        blocks: None,
        by_name: "Particle System",
        reason: "Uses the original's smoke emitters: needs Customize Display's Particle System on.",
        engine: false,
        evidence: "game.rs: effects.system.particles_off = custom[10] == 0 (no native particles, nothing to enhance)",
    },
    Rule {
        row: RowId::SparksFire,
        by: RowId::Custom(10),
        effect: Effect::Needs,
        when: |s| s.custom[10] == 0,
        blocks: None,
        by_name: "Particle System",
        reason: "Uses the original's spark and flame emitters: needs Customize Display's Particle System on.",
        engine: false,
        evidence: "as Particles (the native particle list feeds sparks_fire.prepare)",
    },
    Rule {
        row: RowId::LensDrops,
        by: RowId::Custom(19),
        effect: Effect::Needs,
        when: |s| s.custom[19] == 0 || s.custom[10] == 0,
        blocks: None,
        by_name: "Rain Splatter",
        reason: "The extra drops are drawn with the original's lens drops: needs Customize Display's Rain Splatter and Particle System on.",
        engine: false,
        evidence: "postfx: rain_splatter = custom[19] != 0 && custom[10] != 0 gates the lens pass the extra drops join (code)",
    },
    Rule {
        row: RowId::HdrPost,
        by: RowId::HdrOutput,
        effect: Effect::Blocks,
        when: |_| qol::hdr_output_active(),
        blocks: Some(|i| i == 0),
        by_name: "HDR Output",
        reason: "HDR Output needs the high-precision passes (at least High).",
        engine: false,
        evidence: "qol::hdr_post: max(1) under HDR Output (code)",
    },
];

/// The first rule in force for row `id`, if any.
pub fn rule_for(id: RowId, s: &Settings) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.row == id && (r.when)(s))
}

/// Whether tier `i` of row `r` is taken out by a rule in force.
pub fn blocked(r: &GraphicsRow, s: &Settings, i: usize) -> bool {
    RULES.iter().any(|x| x.row == r.id && x.effect == Effect::Blocks && (x.when)(s) && x.blocks.is_some_and(|b| b(i)))
}

/// The value text and enabled state a rule gives row `id` (whose tier
/// name is `name`), if one is in force.
pub fn shown(id: RowId, name: &str, s: &Settings) -> Option<(String, bool)> {
    let r = rule_for(id, s)?;
    Some(match r.effect {
        Effect::Overridden => (format!("Overridden by {}", r.by_name), false),
        Effect::Needs => (format!("Needs {}", r.by_name), false),
        Effect::Partial => (format!("{name} ({})", r.by_name), true),
        Effect::Blocks => {
            let row = crate::frontend::enh::graphics_table::row(id)?;
            let at = (row.get)(s);
            if !r.blocks.is_some_and(|b| b(at)) {
                return None;
            }
            (format!("{name} (off: {})", r.by_name), true)
        }
    })
}

/// The detail line's note for row `id`: the reason of the rule in force.
pub fn note(id: RowId, s: &Settings) -> Option<&'static str> {
    rule_for(id, s).map(|r| r.reason)
}
