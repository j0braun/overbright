//! Texture mods: a tiny TexMod package made here (one texture of its own,
//! nothing from a real mod), imported through the launcher's code path;
//! the list's order, on / off and removal; broken and foreign packages.

use std::path::{Path, PathBuf};

use super::*;
use crate::formats::texture::{Blend, PixelFormat, Texture};
use crate::formats::tpf;
use crate::tpf_import::{Domain, Source};

/// A fresh folder for one test.
pub(crate) fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-texture-mods-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A game texture of `w` × `h` RGBA8 texels of one colour.
pub(crate) fn texture(name: &str, w: u32, h: u32, rgba: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        hash: crate::formats::hash::bin_hash(name),
        width: w,
        height: h,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: rgba.repeat((w * h) as usize).into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Default::default(),
    }
}

/// A PNG of `w` × `h` texels with a gradient (not the original's pixels).
pub(crate) fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 16) as u8, (y * 16) as u8, 200, 255]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A plain (stored, unencrypted, unmasked) ZIP of `entries` with a
/// comment: what `tpf::read` reads like a TexMod package.
pub(crate) fn zip(entries: &[(&str, Vec<u8>)], comment: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let crc = tpf::zip_crc(data);
        let at = out.len() as u32;
        let header = |sig: &[u8], central: bool| {
            let mut h = sig.to_vec();
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes()); // version needed
            h.extend(0u16.to_le_bytes()); // flags
            h.extend(0u16.to_le_bytes()); // stored
            h.extend(0u32.to_le_bytes()); // time, date
            h.extend(crc.to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra
            if central {
                h.extend([0u8; 6]); // comment length, disk, internal attributes
                h.extend(0u32.to_le_bytes()); // external attributes
                h.extend(at.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(b"PK\x03\x04", false));
        out.extend(data);
        central.extend(header(b"PK\x01\x02", true));
    }
    let cd = out.len() as u32;
    out.extend(&central);
    out.extend(b"PK\x05\x06");
    out.extend([0u8; 4]);
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd.to_le_bytes());
    out.extend((comment.len() as u16).to_le_bytes());
    out.extend(comment.as_bytes());
    out
}

/// The game's textures for the tests: two world textures and a car one.
fn game() -> Vec<Source> {
    vec![
        Source {
            file: "TRACKS/L4RA.BUN".into(),
            domain: Domain::World,
            tex: texture("ROAD_TEST", 4, 4, [90, 90, 90, 255]),
        },
        Source {
            file: "TRACKS/L4RA.BUN".into(),
            domain: Domain::World,
            tex: texture("WALL_TEST", 4, 4, [30, 60, 90, 255]),
        },
        Source {
            file: "CARS/TEST/TEXTURES.BIN".into(),
            domain: Domain::Car,
            tex: texture("CAR_TEST", 4, 4, [9, 9, 9, 255]),
        },
    ]
}

fn key(t: &Texture) -> u32 {
    crate::tpf_import::texmod_hash(t).unwrap()
}

/// A package replacing ROAD_TEST and CAR_TEST, with one key the game does
/// not have.
fn package_bytes() -> Vec<u8> {
    let g = game();
    zip(
        &[
            (&format!("SPEED2.EXE_0x{:08X}.png", key(&g[0].tex)), png(8, 8)),
            (&format!("SPEED2.EXE_0x{:08X}.png", key(&g[2].tex)), png(8, 8)),
            ("SPEED2.EXE_0x12345678.png", png(8, 8)),
        ],
        "Test roads HD\nby nobody",
    )
}

#[test]
fn a_package_installs_into_its_own_folder() {
    let dir = scratch("install");
    let src = dir.join("Test Roads.tpf");
    std::fs::write(&src, package_bytes()).unwrap();
    let packages = dir.join("imported");
    let pkg = read_source(&src).unwrap();
    let r = install(&pkg, &src, &game(), &packages, &|_, _| {});
    let line = outcome_line("Test Roads.tpf", &r);
    let (info, sum) = r.unwrap();
    let info = info.expect("installed");
    assert_eq!((sum.images, sum.matched, sum.unmatched, sum.unsupported), (3, 2, 1, 0));
    assert_eq!(info.id, "test-roads");
    assert_eq!(info.name, "Test roads HD", "the package's own comment, first line");
    assert_eq!(info.source, "Test Roads.tpf");
    let road = game()[0].tex.hash;
    assert!(packages.join("test-roads/world").join(format!("{road:08X}.png")).is_file());
    assert_eq!(std::fs::read_dir(packages.join("test-roads/car")).unwrap().count(), 1);
    assert!(line.ok && line.line.contains("2 of 3 textures") && line.line.contains("1 not found"), "{}", line.line);
    // The player's package is only read; nothing else is left behind.
    assert_eq!(std::fs::read(&src).unwrap(), package_bytes());
    let names: Vec<String> =
        std::fs::read_dir(&packages).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["test-roads"], "no temporary folder left");
    // A second import of the same file gets its own id.
    let (again, _) = install(&pkg, &src, &game(), &packages, &|_, _| {}).unwrap();
    assert_eq!(again.unwrap().id, "test-roads-2");
    // Listing: folders not in the list come at the end, off.
    let list = vec![Entry { id: "test-roads-2".into(), on: true }];
    let shown = installed(&packages, &list);
    assert_eq!(
        shown.iter().map(|(e, _)| (e.id.as_str(), e.on)).collect::<Vec<_>>(),
        [("test-roads-2", true), ("test-roads", false)]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_loose_folder_imports_like_a_package() {
    let dir = scratch("folder");
    let g = game();
    let loose = dir.join("Loose pack");
    std::fs::create_dir_all(&loose).unwrap();
    std::fs::write(loose.join("road.png"), png(8, 8)).unwrap();
    std::fs::write(loose.join("texmod.def"), format!("0x{:08X}|road.png\n", key(&g[0].tex))).unwrap();
    let pkg = read_source(&loose).unwrap();
    let (info, sum) = install(&pkg, &loose, &g, &dir.join("imported"), &|_, _| {}).unwrap();
    assert_eq!((sum.matched, info.unwrap().name.as_str()), (1, "Loose pack"));
    // A folder without texmod.def is not a package.
    assert!(read_source(&dir.join("imported")).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn broken_and_foreign_packages_install_nothing() {
    let dir = scratch("broken");
    let packages = dir.join("imported");
    // Not a package at all.
    let bad = dir.join("bad.tpf");
    std::fs::write(&bad, b"this is not a TexMod package, just some bytes").unwrap();
    let r = read_source(&bad);
    assert!(r.is_err());
    // Another game's package: nothing matches, the program is named.
    let foreign = zip(&[("GTA-SA.EXE_0x0BADF00D.png", png(4, 4))], "");
    let pkg = tpf::read(&foreign).unwrap();
    let r = install(&pkg, Path::new("other.tpf"), &game(), &packages, &|_, _| {});
    let o = outcome_line("other.tpf", &r);
    assert!(!o.ok && o.line.contains("another game (GTA-SA.EXE)"), "{}", o.line);
    // A matching key whose image is broken: counted, nothing installed.
    let g = game();
    let broken = zip(&[(&format!("SPEED2.EXE_0x{:08X}.png", key(&g[0].tex)), b"not a png".to_vec())], "");
    let pkg = tpf::read(&broken).unwrap();
    let r = install(&pkg, Path::new("broken.tpf"), &g, &packages, &|_, _| {});
    let o = outcome_line("broken.tpf", &r);
    assert_eq!(r.as_ref().unwrap().1.unsupported, 1);
    assert!(!o.ok && o.id.is_none(), "{}", o.line);
    assert_eq!(std::fs::read_dir(&packages).map_or(0, |d| d.count()), 0, "no package folder, no leftover");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_list_keeps_order_and_state() {
    let text = format(&[Entry { id: "b".into(), on: true }, Entry { id: "a".into(), on: false }]);
    assert!(text.starts_with(HEADER));
    assert_eq!(parse(&text), [Entry { id: "b".into(), on: true }, Entry { id: "a".into(), on: false }]);
    // Unknown lines, bad ids, repeats and the earlier import's names are left out.
    assert_eq!(parse("on ../x\nmaybe a\non world\non ok-1\noff ok-1\n"), [Entry { id: "ok-1".into(), on: true }]);
    let dir = scratch("order");
    for id in ["first", "second", "third"] {
        std::fs::create_dir_all(dir.join(id).join("world")).unwrap();
    }
    let list = vec![
        Entry { id: "second".into(), on: true },
        Entry { id: "third".into(), on: false },
        Entry { id: "first".into(), on: true },
        Entry { id: "gone".into(), on: true },
    ];
    assert_eq!(roots_in(&list, &dir, false), [dir.join("second/world"), dir.join("first/world")], "on ones, in order");
    assert!(roots_in(&list, &dir, true).is_empty(), "no car folders");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn removing_deletes_only_the_package_folder() {
    let dir = scratch("remove");
    let packages = dir.join("imported");
    std::fs::create_dir_all(packages.join("pack/world")).unwrap();
    std::fs::write(packages.join("pack/package.txt"), "name=Pack\n").unwrap();
    std::fs::create_dir_all(packages.join("handmade/world")).unwrap();
    let mut list = vec![Entry { id: "pack".into(), on: true }, Entry { id: "handmade".into(), on: true }];
    remove(&packages, &mut list, "pack").unwrap();
    assert!(!packages.join("pack").exists());
    assert_eq!(list, [Entry { id: "handmade".into(), on: true }]);
    // A folder this module did not make is refused.
    assert!(remove(&packages, &mut list, "handmade").is_err());
    assert!(packages.join("handmade/world").is_dir());
    assert!(remove(&packages, &mut list, "../x").is_err());
    let _ = std::fs::remove_dir_all(dir);
}

/// The list in the data folder (beside game-root.txt) round-trips, and
/// the game's lookup uses the packages that are on without any variable.
#[test]
fn the_kept_list_reaches_the_game() {
    let home = scratch("kept");
    crate::platform::dirs::set_thread_data_home(home.clone());
    assert_eq!(list_file().parent(), crate::qol::setup::saved_root_file().parent());
    assert!(load().is_empty() && enabled_roots(false).is_empty(), "a fresh install: no mods");
    std::fs::create_dir_all(packages_dir().join("roads/world")).unwrap();
    std::fs::write(packages_dir().join("roads/package.txt"), "name=Roads\n").unwrap();
    save(&[Entry { id: "roads".into(), on: true }]).unwrap();
    assert_eq!(load(), [Entry { id: "roads".into(), on: true }]);
    std::thread::sleep(std::time::Duration::from_millis(550));
    assert_eq!(enabled_roots(false), [packages_dir().join("roads/world")]);
    save(&[]).unwrap();
    assert!(!list_file().exists(), "an empty list removes the file");
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn ids_and_space() {
    let dir = scratch("ids");
    assert_eq!(new_id(&dir, "NFSU2 Redux HD Textures v1.2"), "nfsu2-redux-hd-textures-v1-2");
    assert_eq!(new_id(&dir, "world"), "package-world");
    assert_eq!(new_id(&dir, "???"), "package");
    assert!(free_space(&dir.join("not/yet/there")).is_some_and(|b| b > 0));
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
