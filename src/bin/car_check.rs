//! `car_check CAR` — load a car the way the renderer will and report gaps;
//! `car_check preset NAME` loads a GLOBALB preset car with its parts.
use nfsu2::car::Car;

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/developing/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let name = std::env::args().nth(1).unwrap_or_else(|| "240SX".into());
    let root = nfsu2::game_root();
    let car = if name == "preset" {
        let wanted = std::env::args().nth(2).unwrap_or_else(|| "DDAY_PLAYER_CAR".into());
        let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN")?;
        let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|p| p.name == wanted).ok_or_else(|| anyhow::anyhow!("no preset {wanted}"))?;
        Car::load_with(&root, &preset.car, &preset.installed())?
    } else {
        Car::load(&root, &name)?
    };
    println!("{}: {} parts, wheels {:?}", car.name, car.parts.len(), car.car_type.wheels.map(|w| (w.position, w.radius, w.face, w.width)));
    // The exhaust markers (render +0x598 = +0x548 + 8 · 10, the marker
    // group of CARFX_NITRO's XES_NOS_FlameA): where the flames burst.
    for m in &car.exhausts {
        println!("  exhaust marker at {:?}, x {:?} y {:?} z {:?}", m.w_axis.truncate(), m.x_axis.truncate(), m.y_axis.truncate(), m.z_axis.truncate());
    }
    for p in &car.parts {
        let s = &car.solids[p.solid];
        let missing: Vec<String> = s
            .materials
            .iter()
            .filter_map(|m| s.textures.get(m.texture_slot as usize))
            .filter(|h| car.texture_for(**h).is_none())
            .map(|h| format!("{h:08X}"))
            .collect();
        let lm_missing = s.light_materials.iter().filter(|h| !car.light_materials.contains_key(h)).count();
        let tex: Vec<String> = s
            .materials
            .iter()
            .map(|m| {
                let t = s.textures.get(m.texture_slot as usize).map_or("-".into(), |h| format!("{h:08X}"));
                let l = s.light_materials.get(m.light_material as usize).map_or("-".into(), |h| format!("{h:08X}"));
                format!("{t}/{l}/{}", m.index_count)
            })
            .collect();
        println!("  {:36} textures {:?}", s.name, tex);
        println!("  {:36} bbox {:?}..{:?} missing tex {:?} missing lm {}", s.name, s.bbox_min, s.bbox_max, missing, lm_missing);
    }
    Ok(())
}
