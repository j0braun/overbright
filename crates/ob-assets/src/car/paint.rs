//! Paint jobs and the paint/material lookups of a loaded car.

use super::*;

/// A paint job: the base colour composited into the skin and the light
/// material that replaces `CARSKIN`.
#[derive(Clone, Copy, Debug)]
pub struct Paint {
    pub color: [u8; 3],
    /// The paint's GLOSS: the skin fill's alpha (0x61D7A0 packs it as the
    /// colour's top byte), which car.efx takes as the envmap and specular
    /// weight.
    pub gloss: u8,
    /// The light material hash that replaces `CARSKIN`.
    pub material: u32,
}

impl Paint {
    /// A paint part's colour and material (0x61D7A0 packs its RED, GREEN,
    /// BLUE and GLOSS attributes; attribute 0x6BA02C05 names its light
    /// material, e.g. METPAINTSILVER).
    pub fn from_part(db: &PartDb, part: &crate::formats::parts::Part) -> Paint {
        let get = |k: &str| db.attribute(part, bin_hash(k)).unwrap_or(0) as u8;
        Paint {
            color: [get("RED"), get("GREEN"), get("BLUE")],
            gloss: get("GLOSS"),
            material: db.attribute(part, 0x6BA0_2C05).unwrap_or_else(|| bin_hash("DEFAULT")),
        }
    }
}

impl Car {
    /// Paint the car: composite `<CAR>_SKIN1` (see [`composite_skin`]) and
    /// point the `CARSKIN` light material at the paint's material.
    /// Set the car up for a race mode: the stock handling, with that
    /// mode's tune and the performance set-up redone (0x5B6130).
    pub fn set_race(&mut self, race: crate::race_mode::RaceMode) {
        let mut h = self.car_type.stock.clone();
        crate::handling::setup_performance(&mut h, race);
        self.car_type.handling = h;
    }

    pub fn paint(&mut self, paint: Paint) {
        self.paint_vinyls(paint, &[]);
    }

    /// Paint the car with vinyl layers under the panel lines.
    pub fn paint_vinyls(&mut self, paint: Paint, vinyls: &[VinylLayer]) {
        let fill = [paint.color[0], paint.color[1], paint.color[2], paint.gloss];
        let part_colors = self.part_paints.map(|c| c.unwrap_or(fill));
        let skin = composite_skin(&self.textures, fill, &self.doorline, vinyls, &part_colors);
        let name = format!("{}_SKIN1", self.name);
        let skin = Texture { name, hash: skin_hash(&self.name), ..skin };
        self.textures.insert(skin.hash, skin);
        // 0x6174A4: CARSKIN and 0xA7366AE6 both take the paint's material
        // (render info +0x384), WINDSHIELD the tint's (+0x38C).
        self.light_material_remap.insert(bin_hash("CARSKIN"), paint.material);
        self.light_material_remap.insert(0xA736_6AE6, paint.material);
    }

    /// The light material actually used for a geometry reference: the remap
    /// (paint), the material itself, or `DEFAULT`.
    pub fn light_material_for(&self, hash: u32) -> Option<&LightMaterial> {
        let target = self.light_material_remap.get(&hash).copied().unwrap_or(hash);
        self.light_materials.get(&target).or_else(|| self.light_materials.get(&bin_hash("DEFAULT")))
    }

    /// The light material for a reference in one part's model: the rims'
    /// and brakes' paint where the game swaps it in (0x61797F, 0x62E05F),
    /// else [`Car::light_material_for`].
    pub fn part_light_material_for(&self, part: &CarPart, hash: u32) -> Option<&LightMaterial> {
        // The generic part draw (0x623071…0x623456): before each model it
        // remaps CARSKIN (0xD6D6080A) to [esp+0xBC] — the paint +0x384, or
        // CARBONFIBER +0x388 for the slots whose carbon flag is set (1
        // +0x3B0, 2 +0x3B4, 6 / 0x11 / 0x12 +0x3C8, 9 +0x3BC, 0xA +0x3C4:
        // the `Full` bindings of 0x634B0B…0x634E69) — and 0xA7366AE6 to
        // CARBONFIBER when +0x3B8 (KIT_CARBON) is set, else to the paint.
        if (part.carbon == Carbon::Full && hash == CARSKIN_HASH) || (part.carbon_kit && hash == 0xA736_6AE6) {
            return self.light_material_for(CARBONFIBER_MATERIAL);
        }
        let paint = if part.is_wheel {
            self.rim_material.filter(|_| matches!(hash, 0x2271_9FA9 | 0xFD10_2A92 | 0x1204_9251))
        } else if matches!(part.follows_wheel, Some((_, false))) {
            self.brake_material.filter(|_| matches!(hash, 0xD664_0DFF | 0xA318_6E2B))
        } else {
            None
        };
        // 0x48D860 at 0x630908 / 0x630CBC: the audio paint (+0x39C), the
        // body paint (+0x384) when there is none.
        let audio = match part.audio_paint {
            AudioPaint::TrunkAudio => hash == 0x5BE7_2F1C,
            AudioPaint::Component => hash == bin_hash("CARSKIN"),
            AudioPaint::None => false,
        };
        if audio {
            return match self.audio_material {
                Some(m) => self.light_material_for(m),
                None => self.light_material_for(bin_hash("CARSKIN")),
            };
        }
        if let Some((from, material)) = part.slot_paint
            && hash == from
        {
            return self.light_material_for(material);
        }
        if part.mirror_paint && hash == MIRROR_PAINT_REFERENCE {
            return match self.mirror_material {
                Some(m) => self.light_material_for(m),
                None => self.light_material_for(bin_hash("CARSKIN")),
            };
        }
        self.light_material_for(paint.unwrap_or(hash))
    }

    /// The texture drawn for a reference in one part's model: the part's
    /// carbon table (0x636850) or decal block (0x61FF50) first, else
    /// [`Car::texture_for`].
    pub fn part_texture_for(&self, part: &CarPart, hash: u32) -> Option<&Texture> {
        let carbon = match part.carbon {
            Carbon::Full => matches!(hash, 0xA736_6AE6 | 0x3C84_D757),
            Carbon::Partial => hash == 0xA736_6AE6,
            Carbon::None => false,
        };
        if carbon {
            return self.textures.get(&CARBONFIBRE);
        }
        if let Some(d) = &part.decals
            && let Some(j) = (0..8).find(|&j| dummy_decal(j) == hash)
        {
            // A target the car's packs lack draws as DEFAULTALPHA
            // (**inferred**: the game's lookup of a missing name is not
            // traced).
            return self.textures.get(&d[j]).or_else(|| self.textures.get(&bin_hash(DEFAULT_ALPHA)));
        }
        self.texture_for(hash)
    }

    /// The texture actually drawn for a geometry texture reference.
    pub fn texture_for(&self, hash: u32) -> Option<&Texture> {
        let target = self.texture_remap.get(&hash).copied().unwrap_or(hash);
        self.textures.get(&target)
    }
}
