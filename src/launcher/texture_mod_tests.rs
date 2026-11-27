//! The launcher's texture-mod import and package browser on the texture-mod tests' fixtures (they name the
//! launcher, so they live with it rather than with `texture_mods`).

use crate::formats::texture::PixelFormat;
use crate::texture_mods::fixtures::{key, png, scratch, texwizard_files, zip};
use crate::texture_mods::*;
use crate::tpf_import::{Domain, Source};

/// The launcher's browser lists a TexWizard pack folder and ZIPs holding one as TexWizard packs.
#[test]
fn the_browser_lists_texwizard_packs() {
    let dir = scratch("texwizard-browser");
    let (meta, bin) = texwizard_files();
    let pack = dir.join("TexturePacks/Test Roads TW");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("meta.json"), &meta).unwrap();
    std::fs::write(pack.join("textures.bin"), &bin).unwrap();
    let z = dir.join("pack.zip");
    std::fs::write(
        &z,
        crate::formats::zip::build(&[
            ("Roads/meta.json", &meta, 8),
            ("Roads/textures.bin", &bin, 8),
            ("readme.txt", b"x", 0),
        ]),
    )
    .unwrap();
    let two = dir.join("two.zip");
    std::fs::write(
        &two,
        crate::formats::zip::build(&[
            ("A/meta.json", &meta, 8),
            ("A/textures.bin", &bin, 8),
            ("B/meta.json", &meta, 0),
            ("B/textures.bin", &bin, 0),
        ]),
    )
    .unwrap();
    let broken = dir.join("Broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("meta.json"), &meta).unwrap();
    std::fs::write(broken.join("textures.bin"), b"not a chunk file").unwrap();
    let rows = crate::launcher::model::read_packages(&dir);
    let notes: Vec<(&str, &str)> = rows.iter().map(|e| (e.name.as_str(), e.note.split(',').next().unwrap())).collect();
    assert_eq!(
        notes,
        [("Broken", "TexWizard pack"), ("pack.zip", "TexWizard pack (ZIP)"), ("two.zip", "TexWizard pack (ZIP)")]
    );
    assert_eq!(crate::launcher::model::packages_here(&pack, Vec::new()), [pack.clone()]);
    let _ = std::fs::remove_dir_all(dir);
}

/// The launcher's real import path on the player's game files: a package
/// made here for one real world texture's TexMod key (its own pixels, a
/// gradient) is read from a file, the game's 10,000-odd textures are keyed,
/// the package is installed on and the game's lookup then lists it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_launcher_imports_for_the_players_game() {
    let root = crate::testkit::game_root();
    let home = scratch("players-game");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let sources = crate::tpf_import::collect(&root).unwrap();
    let road = sources.iter().find(|s| s.domain == Domain::World && s.tex.width >= 8).expect("a world texture");
    let k = key(&road.tex);
    let file = home.join("one road.tpf");
    std::fs::write(&file, zip(&[(&format!("SPEED2.EXE_0x{k:08X}.png"), png(road.tex.width, road.tex.height))], ""))
        .unwrap();
    let progress = std::sync::Mutex::new(Vec::new());
    let out = crate::launcher::model::import_paths(&[file.clone()], Some(&root), &|t| progress.lock().unwrap().push(t));
    assert_eq!(out.len(), 1);
    assert!(out[0].ok, "{}", out[0].line);
    assert!(out[0].line.contains("1 of 1 textures"), "{}", out[0].line);
    assert!(progress.lock().unwrap()[0].starts_with("Reading your game's textures"));
    let id = out[0].id.clone().unwrap();
    assert_eq!(load(), [Entry { id: id.clone(), on: true }], "added to the kept list, on");
    let world = packages_dir().join(&id).join("world");
    assert!(std::fs::read_dir(&world).unwrap().next().is_some());
    std::thread::sleep(std::time::Duration::from_millis(550));
    assert!(enabled_roots(false).contains(&world));
    // The game folder was only read: nothing written into it.
    assert!(!root.join("texture-mods.txt").exists() && !root.join("imported").exists());
    let _ = std::fs::remove_dir_all(home);
}

/// The launcher's real import path on the player's game files, for a
/// TexWizard pack made here from the game's own textures: two world
/// textures of one size, the second's pixels put in the pack under a new
/// name and paired with the first. Imported from the game folder layout
/// TexWizard uses (`TexturePacks/<pack>`), installed on, and the first
/// texture's file in the package then holds the second's pixels.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_launcher_imports_a_texwizard_pack_for_the_players_game() {
    let root = crate::testkit::game_root();
    let home = scratch("players-game-texwizard");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let sources = crate::tpf_import::collect(&root).unwrap();
    let world: Vec<&Source> = sources
        .iter()
        .filter(|s| {
            s.domain == Domain::World
                && s.tex.format == PixelFormat::Dxt1
                && s.tex.width >= 16
                && s.tex.width == s.tex.height
        })
        .collect();
    let a = world[0];
    let b = world
        .iter()
        .find(|s| {
            s.tex.hash != a.tex.hash
                && (s.tex.width, s.tex.height) == (a.tex.width, a.tex.height)
                && s.tex.decode_rgba() != a.tex.decode_rgba()
        })
        .expect("two different world textures of one size");
    let mut new = b.tex.clone();
    new.name = format!("_{}", a.tex.name);
    new.hash = crate::formats::hash::bin_hash(&new.name);
    let fake_game = home.join("game");
    let pack = fake_game.join("TexturePacks/Own Swap");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("meta.json"),
        format!("{{ \"textures\": [ [ \"0x{:08X}\", \"{}\" ] ] }}", a.tex.hash, new.name),
    )
    .unwrap();
    std::fs::write(pack.join("textures.bin"), crate::formats::texwizard::tpk_bytes(std::slice::from_ref(&new)))
        .unwrap();
    let found = crate::formats::texwizard::find_packs(&fake_game);
    assert_eq!(found, [pack.clone()]);
    let out = crate::launcher::model::import_paths(&found, Some(&root), &|_| {});
    assert_eq!(out.len(), 1);
    assert!(out[0].ok && out[0].line.contains("1 of 1 textures"), "{}", out[0].line);
    let id = out[0].id.clone().unwrap();
    assert_eq!(load(), [Entry { id: id.clone(), on: true }]);
    let file = packages_dir().join(&id).join("world").join(format!("{:08X}.dds", a.tex.hash));
    let dds = crate::upscale::parse_dds(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!((dds.format, dds.width, dds.height), (PixelFormat::Dxt1, b.tex.width, b.tex.height));
    std::thread::sleep(std::time::Duration::from_millis(550));
    assert!(enabled_roots(false).contains(&packages_dir().join(&id).join("world")));
    assert!(!root.join("texture-mods.txt").exists() && !root.join("TexturePacks").join("Own Swap").exists());
    let _ = std::fs::remove_dir_all(home);
}
