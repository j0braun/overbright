//! Light flares from the car geometry markers.

use super::*;

/// 0x6204E0 walks the LOD A group 0 model of slots 0..0x3E (render info
/// +0x17A8, 0x20 apart).
pub const FLARE_SLOTS: usize = 0x3F;
/// Render info +0x30 is the slot 5 (BODY) LOD A solid's header +0x70
/// (0x6389A0 via 0x488EB0: model +0x0C → solid + 0x70), the translation
/// row of the header's matrix at +0x40.
pub const BODY_SLOT: usize = 5;

/// 0x620530 over each solid's markers (0x48D8A0): every head or brake
/// light marker (`crate::glow::car_flare_kind`) becomes a flare with flags
/// 4, its position the marker's translation less render info +0x30 and its
/// direction the marker matrix's z row (+0x30), +0x1C = 0.
pub fn light_flares(markers: impl Iterator<Item = (u32, [f32; 16])>, offset: Vec3) -> Vec<crate::glow::CarFlare> {
    markers
        .filter_map(|(hash, m)| {
            let kind = crate::glow::car_flare_kind(hash)?;
            Some(crate::glow::CarFlare {
                hash,
                flare: crate::formats::lights::Flare {
                    position: Vec3::new(m[12], m[13], m[14]) - offset,
                    direction: Vec3::new(m[8], m[9], m[10]),
                    kind,
                    flags: 4,
                },
            })
        })
        .collect()
}
