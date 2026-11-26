use super::*;

/// A world surface's material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// Road surfaces (worldreflect draws, ROAD/ASPHALT/STREET names).
    Asphalt,
    /// Pavements, kerbs, concrete and brick walls, stone.
    Concrete,
    /// Metal: rails, fences, poles, garage doors, containers.
    Metal,
    /// Windows and glass façades.
    Glass,
    /// Painted signs, billboards, posters, road markings' own textures.
    Sign,
    /// Plants, ground, dirt: diffuse only.
    Rough,
    /// Anything else lit by the world (a mild default).
    Generic,
    /// Not touched: additive / emissive textures, glows, sky, alpha-blended
    /// decals, light pools.
    Skip,
    /// Cars ([`car_class`]; only the normal map's strength is used: the
    /// cars keep their native light materials). Tyres: sidewall lettering
    /// and tread.
    CarTyre,
    /// Grilles, vents, meshes, intakes, brake discs.
    CarMesh,
    /// Badges and emblems.
    CarBadge,
    /// Seats, dashboards, the engine bay: what shows through the windows.
    CarInterior,
}

/// What the shader takes for a class: roughness (0 mirror, 1 diffuse), F0
/// (the specular reflectance at normal incidence) and the normal map's
/// strength (0 = no map).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub rough: f32,
    pub f0: f32,
    pub normal: f32,
}

impl Class {
    pub fn params(self) -> Params {
        let p = |rough, f0, normal| Params { rough, f0, normal };
        match self {
            Class::Asphalt => p(0.62, 0.035, 1.0),
            Class::Concrete => p(0.78, 0.03, 0.8),
            Class::Metal => p(0.38, 0.45, 0.5),
            Class::Glass => p(0.06, 0.05, 0.0),
            Class::Sign => p(0.42, 0.045, 0.0),
            Class::Rough => p(0.92, 0.02, 0.0),
            Class::Generic => p(0.72, 0.03, 0.0),
            Class::Skip => p(1.0, 0.0, 0.0),
            Class::CarTyre => p(0.9, 0.03, 1.0),
            Class::CarMesh => p(0.5, 0.04, 0.8),
            Class::CarBadge => p(0.3, 0.5, 0.6),
            Class::CarInterior => p(0.8, 0.03, 0.7),
        }
    }

    /// The class packed for the draw's immediates: roughness, F0 (× 255)
    /// and normal strength (× 64) in bytes 0..2, byte 3 = 1 (a material).
    /// 0 = none (Skip).
    pub fn packed(self) -> u32 {
        if self == Class::Skip {
            return 0;
        }
        let p = self.params();
        let b = |v: f32, s: f32| ((v * s).round().clamp(0.0, 255.0)) as u32;
        b(p.rough, 255.0) | (b(p.f0, 255.0) << 8) | (b(p.normal, 64.0) << 16) | (1 << 24)
    }
}

/// What [`classify`] reads of a texture.
#[derive(Clone, Debug)]
pub struct TextureInfo {
    pub name: String,
    pub blend: Blend,
    pub alpha_test: bool,
    /// Drawn with worldreflect (a road surface, mesh flag 0x40).
    pub reflect: bool,
    /// Drawn with worldnofog / as a glow / sky (emissive).
    pub emissive: bool,
    pub width: u32,
    pub height: u32,
}

impl TextureInfo {
    pub fn of(t: &Texture, reflect: bool) -> TextureInfo {
        TextureInfo {
            name: t.name.to_ascii_uppercase(),
            blend: t.blend,
            alpha_test: t.alpha_test,
            reflect,
            emissive: false,
            width: t.width,
            height: t.height,
        }
    }
}

/// Whether `name` holds one of `words`: a `_`-separated token starting
/// with it, or (words of 6+ letters) anywhere ("A51CONCRETE2").
fn has(name: &str, words: &[&str]) -> bool {
    words.iter().any(|w| name.split(['_', '.', '&']).any(|t| t.starts_with(w)) || (w.len() >= 6 && name.contains(w)))
}

/// The material of a world texture. The art's name prefixes are EA's
/// categories (Bayview: ARC architecture 836, SGN signs 319, OBJ objects
/// 146, ORG plants 69, TRN terrain 34, ADS adverts 26, RDP road pieces 24,
/// SFX effects 17, ANM animated); the words after them refine.
pub fn classify(t: &TextureInfo) -> Class {
    let n = t.name.as_str();
    let prefix = n.split('_').next().unwrap_or("");
    // Emissive, additive and blended art: glows, flares, light pools, sky,
    // neon, shadows and decals.
    if t.emissive || matches!(t.blend, Blend::Additive | Blend::DestAlphaAdditive | Blend::DestAlpha) {
        return Class::Skip;
    }
    if matches!(prefix, "SFX" | "ANM" | "SKY")
        || has(
            n,
            &[
                "SKY",
                "SKYDOME",
                "SKYLINE",
                "GLOW",
                "FLARE",
                "HEADLIGHT",
                "NEON",
                "SHADOW",
                "LIGHT",
                "LIGHTS",
                "MOON",
                "CLOUD",
                "SMOKE",
                "WATER",
                "WATERFALL",
                "OCEAN",
                "SPARK",
                "FIRE",
                "STREAK",
                "DEFAULT",
                "PANARAMA",
                "PANORAMA",
                "PANLANDMARKS",
                "CEILINGLIGHT",
                "LIT",
            ],
        )
    {
        return Class::Skip;
    }
    if t.blend == Blend::Alpha && !t.alpha_test {
        // Alpha-blended decals (skid marks, grime, road paint overlays):
        // the surface under them carries the material.
        return Class::Skip;
    }
    if matches!(prefix, "SGN" | "ADS")
        || has(n, &["SIGN", "BILLBOARD", "POSTER", "LOGO", "BANNER", "ADVERT", "GRAFF", "MARQUEE", "STOREFRONT"])
    {
        return Class::Sign;
    }
    if prefix == "ORG"
        || prefix == "TRN"
        || has(
            n,
            &[
                "GRASS",
                "BUSH",
                "LEAF",
                "LEAVES",
                "HEDGE",
                "IVY",
                "DIRT",
                "MUD",
                "SAND",
                "GRAVEL",
                "FOLIAGE",
                "FLOWER",
                "WEED",
                "BARKMULCH",
                "TREE",
                "TREELINE",
                "HILLS",
                "RUBBLE",
                "RUBLE",
            ],
        )
    {
        return Class::Rough;
    }
    if has(n, &["WINDOW", "WINDOWS", "WIN", "WIND", "GLASS", "CASIOWIN", "THEATREWIN", "MYLAR"]) {
        return Class::Glass;
    }
    if has(
        n,
        &[
            "METAL",
            "STEEL",
            "RAIL",
            "RAILBRIDGE",
            "FENCE",
            "CHAIN",
            "CHNLINK",
            "POLE",
            "PIPE",
            "PIPES",
            "GRATE",
            "GRILL",
            "DOOR",
            "SHUTTER",
            "CONTAINER",
            "TANK",
            "TANKER",
            "VENT",
            "IRON",
            "CHROME",
            "ALUM",
            "GUARD",
            "TRUSS",
            "GIRDER",
            "CORRUG",
            "CORRGTD",
            "SIDING",
            "CRATE",
            "TRACKS",
            "HANGERWALL",
        ],
    ) {
        return Class::Metal;
    }
    if has(
        n,
        &[
            "SIDEWALK",
            "CURB",
            "KERB",
            "CONCRETE",
            "CONC",
            "CEMENT",
            "BRICK",
            "STONE",
            "ROCK",
            "CLIFF",
            "WALL",
            "PAVE",
            "TILE",
            "PLASTER",
            "STUCCO",
            "BRIDGE",
            "PILLAR",
            "PILLER",
            "COLUMN",
            "BLOCK",
            "COBBLE",
            "COBBLESTO",
            "GRANNIT",
            "FOUNDATION",
            "OVERPASS",
            "TUNWALL",
            "TUNNEL",
            "ROOF",
            "SHINGLES",
            "BARRIER",
            "COURTGD",
            "MDSURF",
        ],
    ) {
        // Road pieces drawn with worldreflect stay asphalt (the sidewalk
        // strips of the road meshes are kerb-coloured concrete).
        if t.reflect && !has(n, &["SIDEWALK", "CURB", "KERB", "COBBLE", "COBBLESTO", "COURTGD", "PAVE"]) {
            return Class::Asphalt;
        }
        return Class::Concrete;
    }
    if t.reflect
        || prefix == "RDP"
        || has(
            n,
            &[
                "ROAD",
                "ROADSTRIP",
                "ASPHALT",
                "STREET",
                "HWY",
                "HIGHWAY",
                "FREEWAY",
                "FWY",
                "TARMAC",
                "LANE",
                "LANEA",
                "LANEB",
                "LANEC",
                "INTERSECT",
                "PARKING",
                "SHOULDER",
            ],
        )
    {
        return Class::Asphalt;
    }
    if t.alpha_test {
        // Cut-outs not named above: no map.
        return Class::Rough;
    }
    Class::Generic
}

/// A world texture slot's class (name and draws only: cheap, at load).
/// `reflect`: most of its draws are worldreflect; `emissive`: it is drawn
/// only without fog (worldnofog, glows) or as the sky.
pub fn slot_class(t: &Texture, reflect: bool, emissive: bool) -> Class {
    let mut info = TextureInfo::of(t, reflect);
    info.emissive = emissive;
    classify(&info)
}

/// Maps rejected by eye in the review sheets (docs/notes/graphics/surface-maps.md
/// "Review"): façades whose painted windows the map embossed.
pub const REJECTED: &[&str] = &["ARC_WHOUSE_METAL_04", "ARC_STUCCOWHITE_A_RB"];

/// Whether a slot of `class` may get a map (before its pixels are judged).
pub fn map_candidate(t: &Texture, class: Class) -> bool {
    class.params().normal > 0.0
        && !t.alpha_test
        && t.width >= 32
        && t.height >= 32
        && !REJECTED.iter().any(|r| t.name.eq_ignore_ascii_case(r))
}

/// The car surface class of a texture `name` drawn with light material
/// `material` (both as the game names them). Skip: no map.
pub fn car_class(name: &str, material: &str) -> Class {
    let n = name.to_ascii_uppercase();
    let m = material.to_ascii_uppercase();
    let (n, m) = (n.as_str(), m.as_str());
    // The carbon weave (CARBONFIBER, the CF spoilers' DEFAULT too): native.
    // Its map was tried and rejected (docs/notes/graphics/surface-maps.md "Cars"): on the
    // glossy CARBONFIBER panels the weave's tilt turned into blotches at
    // driving distance (the chrome-flicker shimmer risk, docs/notes/graphics/car-look.md "Chrome trunk flicker and dry-road reflection strength").
    if n.starts_with("CARBONFIBRE") {
        return Class::Skip;
    }
    // Paint, rims, chrome, glass and lights: native (no relief of their
    // own, or a mirror the map would only shimmer).
    let paint = [
        "REGPAINT",
        "METPAINT",
        "PEARL",
        "CUSTOMPAINT",
        "RIMPEARL",
        "USER_RIMS",
        "MAG",
        "CHROME",
        "WINDSHIELD",
        "WINDOWMASK",
        "CLEARPLASTIC",
        "HEADLIGHT",
        "BRAKELIGHT",
        "TRAFFIC",
        "CARSKIN",
    ];
    if paint.iter().any(|p| m.starts_with(p)) {
        return Class::Skip;
    }
    match m {
        "RUBBER" if n.contains("TIRE") || n.contains("WHEEL") => Class::CarTyre,
        "RUBBER" => Class::CarInterior,
        "GRILL" => Class::CarMesh,
        _ if m.starts_with("BRAKEDISC") => Class::CarMesh,
        "INTERIOR" => Class::CarInterior,
        "LICENSEPLATE" => Class::CarBadge,
        _ if n.ends_with("_BADGING") => Class::CarBadge,
        _ if n.starts_with("GRILL") => Class::CarMesh,
        _ if n.contains("ENGINE") || n.starts_with("AUDIO") => Class::CarInterior,
        "DULLPLASTIC" if n.contains("SKIN") => Class::CarInterior,
        _ => Class::Skip,
    }
}

/// The edginess limit of a car class's region: the tyre lettering, grille
/// meshes and badges are drawn shapes that are relief, so they may be
/// edgier than the world's grain; the carbon weave and interior not.
pub fn car_max_edginess(class: Class) -> f32 {
    match class {
        Class::CarTyre | Class::CarMesh | Class::CarBadge => 24.0,
        _ => MAX_EDGINESS * 1.5,
    }
}
