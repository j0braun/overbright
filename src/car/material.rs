//! Light materials (GLOBALB 0x135200) and their shader constants.

use super::*;

/// A light material ("CHROME", "RUBBER", …): how a car surface reflects.
#[derive(Clone, Debug)]
pub struct LightMaterial {
    pub name: String,
    /// The 28 floats at record +0x30.
    pub values: [f32; 28],
}

/// The car.efx constants derived from a light material (0x5BD4B0).
#[derive(Clone, Copy, Debug, Default)]
pub struct CarMaterialConstants {
    pub diffuse_min: [f32; 4],
    pub diffuse_range: [f32; 4],
    pub specular_min: [f32; 4],
    pub specular_range: [f32; 4],
    pub envmap_min: [f32; 4],
    pub envmap_range: [f32; 4],
    pub specular_power: f32,
    pub envmap_power: f32,
}

/// Global scale applied to SpecularPower (SPEED2.EXE 0x800B94, read only
/// by 0x5BDA63: 1.1).
pub(super) const SPECULAR_POWER_SCALE: f32 = 1.1;

impl LightMaterial {
    /// Value at record offset `at` (0x30..0xA0).
    fn f(&self, at: usize) -> f32 {
        self.values[(at - 0x30) / 4]
    }

    fn rgb_scaled(&self, scale_at: usize, half: bool) -> [f32; 3] {
        let s = self.f(scale_at) * if half { 0.5 } else { 1.0 };
        [self.f(scale_at + 4) * s, self.f(scale_at + 8) * s, self.f(scale_at + 12) * s]
    }

    /// As 0x5BD4B0 computes them from a draw's flags (the mesh material
    /// record's +0x38, `solid::Material::flags`). `mesh_flags & 0x100` enables the env map;
    /// without bit 0x80 diffuse and specular are zeroed.
    pub fn constants(&self, mesh_flags: u32) -> CarMaterialConstants {
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let diffuse_min = self.rgb_scaled(0x30, false);
        let diffuse_max = self.rgb_scaled(0x40, false);
        let specular_min = self.rgb_scaled(0x5C, false);
        let specular_max = self.rgb_scaled(0x6C, false);
        let envmap_min = self.rgb_scaled(0x80, true);
        let envmap_max = self.rgb_scaled(0x90, true);
        let mut c = CarMaterialConstants {
            diffuse_min: [diffuse_min[0], diffuse_min[1], diffuse_min[2], self.f(0x50)],
            diffuse_range: {
                let d = sub(diffuse_max, diffuse_min);
                [d[0], d[1], d[2], self.f(0x54) - self.f(0x50)]
            },
            specular_min: [specular_min[0], specular_min[1], specular_min[2], 0.0],
            specular_range: {
                let d = sub(specular_max, specular_min);
                [d[0], d[1], d[2], 0.0]
            },
            envmap_min: [envmap_min[0], envmap_min[1], envmap_min[2], 0.0],
            envmap_range: {
                let d = sub(envmap_max, envmap_min);
                [d[0], d[1], d[2], 0.0]
            },
            specular_power: SPECULAR_POWER_SCALE * self.f(0x58),
            envmap_power: self.f(0x7C),
        };
        if mesh_flags & 0x100 == 0 {
            c.envmap_min = [0.0; 4];
            c.envmap_range = [0.0; 4];
        } else if mesh_flags & 0x80 == 0 {
            c.diffuse_min = [0.0; 4];
            c.diffuse_range = [0.0; 4];
            c.specular_min = [0.0; 4];
            c.specular_range = [0.0; 4];
        }
        c
    }
}

/// The glass materials' diffuse alpha (record +0x50 / +0x54, the
/// DiffuseMin / DiffuseRange w that becomes the draw's alpha), replaced
/// when the record loads (0x48E070 → 0x5BB5C0, for records whose +0x10 is
/// 3) from the globals 0x7FF060…0x7FF084, keyed by the name hash at +0x0C.
pub(super) fn glass_alpha(hash: u32) -> Option<(f32, f32)> {
    Some(match hash {
        // WINDOWMASK, WINDSHIELD, MOLDINGS: 0x7FF060 / 0x7FF070.
        0x3ED7_0C43 | 0x471A_1DCA | 0x12C9_453C => (0.3, 1.0),
        // WINDSHIELD_TINT_L1_BLACK, _L1_GREEN: 0x7FF064 / 0x7FF074.
        0x8994_6400 | 0x89F2_3A14 => (0.3, 1.0),
        // WINDSHIELD_TINT_L2_BLACK, _BLUE, _GREEN, _RED: 0x7FF068 / 0x7FF078.
        0xD68E_A0C1 | 0x8A61_6C4C | 0xD6EC_76D5 | 0x8050_CA3F => (0.4, 1.0),
        // WINDSHIELD_TINT_L3_AQUA, BLACK, BLUE, GOLD, GRAPE, GREEN, LAVENDER,
        // LIME, ORANGE, PINK, RED, YELLOW: 0x7FF06C / 0x7FF07C.
        0x8CB6_1DCD | 0x2388_DD82 | 0x8CB6_94ED | 0x8CB9_5E6B | 0x23E6_A3F4 | 0x23E6_B396 | 0x6CF4_1096
        | 0x8CBC_02EC | 0xB364_5F81 | 0x8CBE_3497 | 0x8062_E2C0 | 0xC9D2_B801 => (0.5, 1.0),
        // CLEARPLASTIC, HEADLIGHTGLASS: 0x7FF080 / 0x7FF084.
        0x33A2_6CB6 | 0xA634_8EE3 => (0.4, 0.8),
        _ => return None,
    })
}

pub fn parse_light_materials(globalb: &[u8]) -> HashMap<u32, LightMaterial> {
    bundle::chunks(globalb, 0)
        .filter(|c| c.id == LIGHT_MATERIAL && c.body.len() >= 0xA0)
        .map(|c| {
            let name = read_cstr(c.body, 0x14, 0x1C);
            let mut values: [f32; 28] = std::array::from_fn(|i| read_f32(c.body, 0x30 + i * 4));
            // 0x48E070 also scales +0x80 / +0x90 by 0x78D780, which is 1.0.
            if read_u32(c.body, 0x10) == 3
                && let Some((min, max)) = glass_alpha(read_u32(c.body, 0x0C))
            {
                values[(0x50 - 0x30) / 4] = min;
                values[(0x54 - 0x30) / 4] = max;
            }
            (bin_hash(&name), LightMaterial { name, values })
        })
        .collect()
}
