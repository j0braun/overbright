//! Enhanced Car Paint's paint type by light material (moved from car.rs).

/// The NFSU2 paint type of a car light material, for Enhanced Car Paint
/// (docs/notes/graphics/car-paint-enhanced.md). The paint parts name their light material
/// (attribute 0x6BA02C05): finish GLOSS → REGPAINT*, METAL → METPAINT*,
/// PEARL → PEARL* and CUSTOMPAINT_*; the rim paints RIMPEARL* and
/// USER_RIMS_*; stock rims MAGSILVER / MAGCHROME, trim CHROME.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaintType {
    /// Glass, plastic, rubber, lights, decals…: native car.efx only.
    Native,
    Gloss,
    Metallic,
    Pearl,
    /// The PEARL finish's CUSTOMPAINT_* (two-tone, "candy" look).
    Candy,
    Chrome,
}

impl PaintType {
    pub fn of(material: &str) -> PaintType {
        let m = material.to_ascii_uppercase();
        let m = m.as_str();
        match m {
            "CHROME" | "MAGCHROME" | "USER_RIMS_CHROME" | "USER_RIMS_SILVERGLOSS" => PaintType::Chrome,
            "MAGSILVER" | "MAGGUNMETAL" | "USER_RIMS_MET" | "USER_RIMS_GUNMETAL" | "USER_RIMS_MAGSILVER" => {
                PaintType::Metallic
            }
            "MAGSILVERGLOSS" | "USER_RIMS_GLOSS" | "USER_CALIPERS" | "TRAFFIC" => PaintType::Gloss,
            _ if m.starts_with("REGPAINT") => PaintType::Gloss,
            _ if m.starts_with("METPAINT") => PaintType::Metallic,
            _ if m.starts_with("PEARL") || m.starts_with("RIMPEARL") => PaintType::Pearl,
            _ if m.starts_with("CUSTOMPAINT") => PaintType::Candy,
            _ => PaintType::Native,
        }
    }

    /// car.wgsl's `paint`: x clearcoat (Fresnel env layer + sharp highlight),
    /// y metallic flake, z pearl shift (luminance-neutral), w per-pixel
    /// (crisp) reflection direction. All zero: native.
    pub fn params(self) -> [f32; 4] {
        match self {
            PaintType::Native => [0.0; 4],
            PaintType::Gloss => [1.0, 0.0, 0.0, 1.0],
            PaintType::Metallic => [0.8, 1.0, 0.0, 1.0],
            PaintType::Pearl => [0.8, 0.6, 1.0, 1.0],
            PaintType::Candy => [1.0, 0.35, 1.6, 1.0],
            PaintType::Chrome => [0.35, 0.0, 0.0, 1.0],
        }
    }
}
