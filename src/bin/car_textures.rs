//! `car_textures CAR PATTERN OUT_DIR` — dump a car's textures to PNG
//! (CAR `preset` loads DDAY_PLAYER_CAR with its parts).
use nfsu2::car::Car;
fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let a: Vec<String> = std::env::args().skip(1).collect();
    let root = nfsu2::game_root();
    let car = if a[0] == "preset" {
        let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN")?;
        let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").expect("preset");
        Car::load_with(&root, &preset.car, &preset.installed())?
    } else {
        Car::load(&root, &a[0])?
    };
    for t in car.textures.values().filter(|t| t.name.contains(a[1].as_str())) {
        println!("{} {}x{} {:?} test {} blend {:?}", t.name, t.width, t.height, t.format, t.alpha_test, t.blend);
        image::save_buffer(format!("{}/{}_{:08X}.png", a[2], t.name, t.hash), &t.decode_rgba(), t.width, t.height, image::ColorType::Rgba8)?;
    }
    Ok(())
}
