//! Mod packs in the game: export, import, mounting in the player's order, clashes, refusal for another base, and the
//! native default (nothing switched on: nothing mounted).

use glam::{Mat3, Vec3};
use ob_asset_format::modpack::{self, BaseId};

use super::*;
use crate::tools::map::{MapMod, PropEdit, PropRef};

/// The mounted base, the mounted packs and the cache folder are process-wide: the tests that set them take turns.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-modpacks-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn target() -> PropRef {
    PropRef { region: "L4RA".into(), section: 1407, solid: "XO_CONE_01".into(), at: Vec3::new(10.0, 20.0, 3.0), nth: 0 }
}

fn mov(name: &str, x: f32) -> MapMod {
    MapMod {
        name: name.into(),
        props: vec![PropEdit::Move { target: target(), position: Vec3::new(x, 0.0, 0.0), rotation: Mat3::IDENTITY }],
        ..Default::default()
    }
}

fn opts() -> ExportOpts<'static> {
    ExportOpts { version: semver::Version::new(1, 0, 0), licence: None, derived_ok: false, build: "test".into() }
}

fn make(maps: &Path, folder: &str, m: &MapMod, texture: bool) {
    store::save(maps, folder, m).unwrap();
    if texture {
        let t = maps.join(folder).join("textures/world");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("12345678.png"), b"png").unwrap();
    }
}

#[test]
fn a_map_mod_becomes_a_pack_the_game_mounts() {
    let _serial = serial();
    let home = scratch("home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("cache");
    let _c = crate::cache::override_dir(cache.clone());
    set_base(None);
    let maps = scratch("maps");
    make(&maps, "docks", &mov("Docks", 1.0), true);
    make(&maps, "cones", &mov("Cones", 2.0), false);
    let out = scratch("out");
    let base = BaseId { id: "nfsu2-pc12-eu".into(), identity: "sha256:aa".into() };
    // A texture with no texture list entry may start from the original's pixels: the author must say yes.
    assert!(export_map_mod(&maps, "docks", &out, Some(&base), &opts()).unwrap_err().contains("original game"));
    let yes = || ExportOpts { derived_ok: true, ..opts() };
    let e = export_map_mod(&maps, "docks", &out, Some(&base), &yes()).unwrap();
    assert_eq!(e.to, out.join("docks-1.0.0.obpk"));
    assert_eq!(
        std::fs::read(&e.to).unwrap(),
        std::fs::read(export_map_mod(&maps, "docks", &out, Some(&base), &yes()).unwrap().to).unwrap(),
        "the same export is the same bytes"
    );
    export_map_mod(&maps, "cones", &out, None, &opts()).unwrap();

    // Native default: no order.txt, nothing mounted, and no folder made.
    assert!(mount(&[]).mods.is_empty());
    assert!(!mods_dir().exists());

    // Import: new packs are off.
    assert_eq!(modpack::import(&mods_dir(), &out.join("docks-1.0.0.obpk"), &engine()).unwrap(), "docks");
    assert_eq!(modpack::import(&mods_dir(), &out.join("cones-1.0.0.obpk"), &engine()).unwrap(), "cones");
    assert!(mount(&[]).mods.is_empty(), "imported, not switched on");

    // The docks pack was made for a base set that is not in use here: refused with its words.
    modpack::set_enabled(&mods_dir(), "docks", true).unwrap();
    modpack::set_enabled(&mods_dir(), "cones", true).unwrap();
    let names = |m: Mounted| m.mods.into_iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(names(mount(&[])), ["cones"]);
    let plan = modpack::plan(&mods_dir(), None, &engine());
    assert!(plan.refused[0].1.contains("nfsu2-pc12-eu"), "{:?}", plan.refused);
    set_base(Some(base.clone()));
    // docks is first in order.txt = strongest: it comes last in the weakest-first list.
    assert_eq!(names(mount(&[])), ["cones", "docks"]);
    let dirs = texture_dirs(false);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    assert!(dirs[0].join("0x12345678.png").is_file(), "its textures come along, as image-only entries");
    assert!(texture_dirs(true).is_empty());
    assert!(root_of("docks").unwrap().join("world/_any/textures/images/0x12345678.png").is_file());
    // Both moved the same prop: the stronger one (docks) wins, and the clash says so.
    let all = mount(&[]).mods;
    let c = clashes(&all);
    assert!(
        c.iter().any(|c| c.weaker == "cones" && c.stronger == "docks" && c.field.as_deref() == Some("position")),
        "{c:?}"
    );
    // A plain map mod with the same name keeps its folder.
    assert_eq!(names(mount(&["docks".to_string()])), ["cones"]);
    modpack::move_pack(&mods_dir(), "cones", -1).unwrap();
    assert_eq!(names(mount(&[])), ["docks", "cones"]);
    modpack::set_enabled(&mods_dir(), "docks", false).unwrap();
    modpack::set_enabled(&mods_dir(), "cones", false).unwrap();
    assert!(mount(&[]).mods.is_empty(), "all off: native again");
    set_base(None);
    for d in [home, cache, maps, out] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn derived_textures_need_a_yes_and_keep_the_pack_local() {
    let _serial = serial();
    let _c = crate::cache::override_dir(scratch("derived_-cache"));
    let maps = scratch("derived");
    make(&maps, "m", &mov("M", 1.0), false);
    let out = scratch("derived-out");
    let e = export_map_mod(&maps, "m", &out, None, &ExportOpts { licence: Some("CC-BY-4.0"), ..opts() }).unwrap();
    let b = ob_asset_format::bundle::Bundle::open(&e.to, ob_asset_format::bundle::Kind::PackFile).unwrap();
    b.verify().unwrap();
    let first = b.entries().iter().min_by_key(|e| e.data_offset).unwrap();
    assert_eq!(first.path, "pack.toml", "the manifest comes first");
    let text = String::from_utf8(b.read(b.get("pack.toml").unwrap()).unwrap().into_owned()).unwrap();
    assert!(text.contains("share = \"public\"") && text.contains("licence = \"CC-BY-4.0\""), "{text}");
    assert!(export_map_mod(&maps, "Bad Name", &out, None, &opts()).is_err());
    let _ = std::fs::remove_dir_all(maps);
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn look_inside_reads_a_pack_and_its_clashes_without_switching_anything_on() {
    let _serial = serial();
    let home = scratch("in-home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("in-cache");
    let _c = crate::cache::override_dir(cache.clone());
    let maps = scratch("in-maps");
    make(&maps, "docks", &mov("Docks", 1.0), true);
    make(&maps, "cones", &mov("Cones", 2.0), false);
    let out = scratch("in-out");
    for f in ["docks", "cones"] {
        export_map_mod(&maps, f, &out, None, &ExportOpts { derived_ok: true, ..opts() }).unwrap();
        modpack::import(&mods_dir(), &out.join(format!("{f}-1.0.0.obpk")), &engine()).unwrap();
    }
    assert!(inside("nope", None).is_none());
    let alone = inside("docks", None).unwrap();
    assert_eq!((alone.moved, alone.deleted, alone.added, alone.races, alone.textures), (1, 0, 0, 0, 1));
    assert_eq!(alone.regions, ["L4RA"]);
    assert_eq!(alone.made_for, "any copy of the game");
    assert!(alone.clashes.is_empty() && alone.refused.is_none());
    assert!(alone.lines().iter().any(|l| l == "Changes 1 moved prop, 1 texture"), "{:?}", alone.lines());
    // cones is on; docks (off) is looked at: it would overrule cones on the one value both move.
    modpack::set_enabled(&mods_dir(), "cones", true).unwrap();
    let order = std::fs::read(mods_dir().join(modpack::ORDER_FILE)).unwrap();
    let i = inside("docks", None).unwrap();
    assert_eq!(i.clashes.len(), 1);
    assert_eq!(i.clashes[0].0, "cones");
    assert!(i.lines().iter().any(|l| l.starts_with("Overrule") && l.contains("cones")), "{:?}", i.lines());
    assert_eq!(std::fs::read(mods_dir().join(modpack::ORDER_FILE)).unwrap(), order, "order.txt is not touched");
    assert!(!modpack::rows(&mods_dir(), None, &engine()).iter().any(|r| r.id == "docks" && r.on));
    for d in [home, cache, maps, out] {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn full_mod() -> MapMod {
    use ob_map::tools::map::format::{CollisionEdit, CustomRace, RaceKind, Spot};
    let rot = Mat3::from_cols(Vec3::new(0.0, 1.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    let mut race = CustomRace::new("sprint", 4001, RaceKind::Sprint);
    race.name = "Docks sprint".into();
    race.checkpoints = vec![glam::Vec2::new(-1203.5, 455.25), glam::Vec2::new(-980.0, 610.75)];
    race.grid = vec![Spot { position: Vec3::new(-1214.0, 447.5, 13.5), heading: 33.5 }];
    race.start = Some(Spot { position: Vec3::new(-1210.0, 450.0, 13.5), heading: 33.5 });
    race.finish = Some(Spot { position: Vec3::new(-700.0, 641.0, 12.0), heading: 2.0 });
    let mut other = target();
    other.nth = 1;
    other.at = Vec3::new(0.1, 20.3, 3.0);
    MapMod {
        name: "Full".into(),
        author: "Me".into(),
        description: "All kinds".into(),
        props: vec![
            PropEdit::Move { target: target(), position: Vec3::new(1.1, 2.2, 3.3), rotation: rot },
            PropEdit::Delete { target: other },
            PropEdit::Add {
                id: "a1".into(),
                region: "L4RA".into(),
                solid: "XO_CONE_01".into(),
                position: Vec3::new(5.0, 6.0, 7.0),
                rotation: Mat3::IDENTITY,
            },
        ],
        races: vec![race],
        collision: vec![CollisionEdit::Remove {
            region: "L4RA".into(),
            corners: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        }],
    }
}

fn unpack(file: &Path, into: &Path) {
    use ob_asset_format::bundle::{self, Bundle};
    let b = Bundle::open(file, bundle::Kind::PackFile).unwrap();
    for e in b.entries() {
        let out = into.join(&e.path);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(out, &*b.read(e).unwrap()).unwrap();
    }
}

#[test]
fn a_pack_holds_each_kind_as_its_own_file_and_reads_back_whole() {
    let _serial = serial();
    let _c = crate::cache::override_dir(scratch("a_pack_h-cache"));
    let maps = scratch("kinds");
    let m = full_mod();
    make(&maps, "full", &m, true);
    let out = scratch("kinds-out");
    let e = export_map_mod(&maps, "full", &out, None, &ExportOpts { derived_ok: true, ..opts() }).unwrap();
    let root = scratch("kinds-root");
    unpack(&e.to, &root);
    for f in [
        "world/L4RA/placements.toml",
        "races/sprint.toml",
        "world/_any/textures/images/0x12345678.png",
        "pack.toml",
        "provenance.toml",
        "map.obmap",
    ] {
        assert!(root.join(f).is_file(), "{f}");
    }
    assert!(!root.join("textures").exists() && !root.join("looks.toml").exists());
    let pl = std::fs::read_to_string(root.join("world/L4RA/placements.toml")).unwrap();
    assert!(pl.starts_with("schema = \"ob.placements/1.0\""), "{pl}");
    let race = std::fs::read_to_string(root.join("races/sprint.toml")).unwrap();
    assert!(race.contains("schema = \"ob.race/1.0\"") && race.contains("mode = \"sprint\""), "{race}");
    // The collision edits have no kind file yet: the map file holds them and nothing else.
    let only = store::read_file(&root.join("map.obmap")).unwrap();
    assert!(only.props.is_empty() && only.races.is_empty() && only.collision.len() == 1);
    let back = content::read(&root).unwrap().unwrap();
    assert_eq!(back.props, m.props, "placements read back bit for bit");
    assert_eq!(back.races, m.races);
    assert_eq!(back.collision, m.collision);
    // Without collision edits there is no map file at all.
    let mut plain = full_mod();
    plain.collision.clear();
    make(&maps, "plain", &plain, false);
    let e = export_map_mod(&maps, "plain", &out, None, &opts()).unwrap();
    let r2 = scratch("kinds-root2");
    unpack(&e.to, &r2);
    assert!(!r2.join("map.obmap").exists());
    assert_eq!(content::read(&r2).unwrap().unwrap().props, plain.props);
    for d in [maps, out, root, r2] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn an_older_pack_with_a_map_file_still_mounts_and_an_index_key_is_refused() {
    let _serial = serial();
    let home = scratch("old-home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("old-cache");
    let _c = crate::cache::override_dir(cache.clone());
    set_base(None);
    let old = mods_dir().join("old");
    store::save(&mods_dir(), "old", &mov("Old", 4.0)).unwrap();
    std::fs::create_dir_all(old.join("textures/world")).unwrap();
    std::fs::write(old.join("textures/world/00000ABC.png"), b"png").unwrap();
    std::fs::write(
        old.join("pack.toml"),
        "format = 2\nkind = \"mod\"\nid = \"old\"\nname = \"Old\"\nversion = \"1.0.0\"\noverbright = \">=0.1\"\norigin = \"own-work\"\nshare = \"local-only\"\ncontents = [\"world\"]\naffects_play = true\n",
    )
    .unwrap();
    modpack::set_enabled(&mods_dir(), "old", true).unwrap();
    let names = |m: Mounted| m.mods.into_iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(names(mount(&[])), ["old"], "the map file form reads as before");
    assert_eq!(texture_dirs(false), [old.join("textures/world")]);
    // A placement named by its index in the base set needs the base's placement table: refused, with the reason.
    std::fs::create_dir_all(old.join("world/L4RA")).unwrap();
    std::fs::write(
        old.join("world/L4RA/placements.toml"),
        "schema = \"ob.placements/1.0\"\n[[placement]]\nkey = \"1407/0\"\nposition = [1.0, 2.0, 3.0]\n",
    )
    .unwrap();
    assert!(names(mount(&[])).is_empty(), "the pack is not used");
    let err = content::read(&old).unwrap_err();
    assert!(err.contains("1407/0") && err.contains("index"), "{err}");
    modpack::set_enabled(&mods_dir(), "old", false).unwrap();
    for d in [home, cache] {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn png(dir: &Path, rel: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, rel.as_bytes()).unwrap();
}

#[test]
fn a_legacy_map_mod_folder_and_zip_convert_with_provenance() {
    let _serial = serial();
    let _c = crate::cache::override_dir(scratch("a_legacy-cache"));
    let maps = scratch("conv-maps");
    // A folder name that is not an id, as the old editor allowed.
    let src = scratch("conv-src");
    let dir = src.join("Docks Old");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("map.obmap"), mov("Docks", 1.0).write()).unwrap();
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    let out = scratch("conv-out");
    let c = convert_legacy(&dir, &out, None, None, &opts()).unwrap();
    assert_eq!((c.kind, c.exported.id.as_str()), ("map mod", "docks-old"));
    assert_eq!(c.skipped, ["notes.txt"], "what the pack has no place for is named, not dropped silently");
    let root = scratch("conv-root");
    unpack(&c.exported.to, &root);
    let prov = std::fs::read_to_string(root.join("provenance.toml")).unwrap();
    assert!(prov.contains("converted = true") && prov.contains("converted_from = \"map mod Docks Old\""), "{prov}");
    assert_eq!(content::read(&root).unwrap().unwrap().props, mov("Docks", 1.0).props);
    // The same mod as the editor's .zip.
    store::save(&maps, "docks", &mov("Docks", 1.0)).unwrap();
    let zip = src.join("docks.zip");
    crate::tools::map::share::export(&maps, "docks", &zip, true, "test").unwrap();
    let z = convert_legacy(&zip, &out, Some("from-zip"), None, &opts()).unwrap();
    assert_eq!(z.exported.id, "from-zip");
    assert!(z.exported.to.is_file());
    // A bad id given: refused.
    assert!(convert_legacy(&dir, &out, Some("Bad Id"), None, &opts()).unwrap_err().contains("not a pack id"));
    // A folder that is neither refuses.
    let empty = src.join("nothing");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(convert_legacy(&empty, &out, None, None, &opts()).unwrap_err().contains("neither a map mod"));
    for d in [maps, src, out, root] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_legacy_texture_pack_converts_to_overrides_the_game_reads() {
    let _serial = serial();
    let home = scratch("tex-home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("tex-cache");
    let _c = crate::cache::override_dir(cache.clone());
    set_base(None);
    let src = scratch("tex-src");
    let dir = src.join("Pretty Roads");
    std::fs::write(
        {
            std::fs::create_dir_all(&dir).unwrap();
            dir.join("pack.toml")
        },
        "format = 1\nname = \"Pretty roads\"\nauthor = \"Me\"\ncompress = \"none\"\n",
    )
    .unwrap();
    png(&dir, "world/12345678.png");
    png(&dir, "world/sub/ROAD_MAIN.png");
    png(&dir, "car/0xABC.png");
    png(&dir, "any/12345678.png");
    png(&dir, "any/0000DEAD.png");
    std::fs::write(dir.join("world/readme.txt"), "x").unwrap();
    let out = scratch("tex-out");
    let c = convert_legacy(&dir, &out, Some("pretty-roads"), None, &opts()).unwrap();
    assert_eq!(c.kind, "texture pack");
    assert_eq!(c.skipped.len(), 1, "{:?}", c.skipped);
    assert!(c.skipped[0].contains("readme.txt"));
    let root = scratch("tex-root");
    unpack(&c.exported.to, &root);
    let ov = std::fs::read_to_string(root.join("global/textures/overrides.toml")).unwrap();
    assert!(ov.contains("name = \"ROAD_MAIN\"") && ov.contains("hash = \"0x00000ABC\"") && ov.contains("compress = \"none\""), "{ov}");
    // `any/12345678` has a world/ image of its own, so only cars take the any/ one.
    assert!(ov.contains("image = \"images/any/12345678.png\"\ndraw = [\"car\"]"), "{ov}");
    let pk = std::fs::read_to_string(root.join("pack.toml")).unwrap();
    assert!(pk.contains("origin = \"from-game-files\"") && pk.contains("share = \"local-only\""), "{pk}");
    // Mounted, the game reads the folders: its own domain first, then any/.
    assert_eq!(modpack::import(&mods_dir(), &c.exported.to, &engine()).unwrap(), "pretty-roads");
    modpack::set_enabled(&mods_dir(), "pretty-roads", true).unwrap();
    assert!(mount(&[]).mods.is_empty(), "no map data in it");
    let w = texture_dirs(false);
    let names: Vec<_> = w.iter().map(|d| d.strip_prefix(root_of("pretty-roads").unwrap()).unwrap().to_path_buf()).collect();
    assert_eq!(names, [PathBuf::from("global/textures/images/world"), PathBuf::from("global/textures/images/any")]);
    assert_eq!(texture_dirs(true).len(), 2);
    modpack::set_enabled(&mods_dir(), "pretty-roads", false).unwrap();
    // Two images for one texture in one folder: refused.
    png(&dir, "world/0x12345678.png");
    assert!(convert_legacy(&dir, &out, Some("pretty-roads"), None, &opts()).unwrap_err().contains("same texture"));
    for d in [home, cache, src, out, root] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn look_inside_shows_converted_provenance_and_an_old_map_obmap_pack() {
    let _serial = serial();
    let home = scratch("inc-home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("inc-cache");
    let _c = crate::cache::override_dir(cache.clone());
    let src = scratch("inc-src");
    let dir = src.join("Docks Old");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("map.obmap"), mov("Docks", 1.0).write()).unwrap();
    let out = scratch("inc-out");
    let c = convert_legacy(&dir, &out, None, None, &opts()).unwrap();
    modpack::import(&mods_dir(), &c.exported.to, &engine()).unwrap();
    let i = inside("docks-old", None).unwrap();
    assert_eq!((i.moved, i.regions.as_slice()), (1, ["L4RA".to_string()].as_slice()));
    assert_eq!(i.converted_from.as_deref(), Some("map mod Docks Old"));
    assert!(i.lines().iter().any(|l| l == "Provenance: converted from map mod Docks Old"), "{:?}", i.lines());
    // A pack folder in the old form: only map.obmap and textures/world.
    let old = mods_dir().join("old");
    store::save(&mods_dir(), "old", &mov("Old", 4.0)).unwrap();
    std::fs::create_dir_all(old.join("textures/world")).unwrap();
    std::fs::write(old.join("textures/world/00000ABC.png"), b"png").unwrap();
    std::fs::write(
        old.join("pack.toml"),
        "format = 2\nkind = \"mod\"\nid = \"old\"\nname = \"Old\"\nversion = \"1.0.0\"\noverbright = \">=0.1\"\norigin = \"own-work\"\nshare = \"local-only\"\ncontents = [\"world\"]\naffects_play = true\n",
    )
    .unwrap();
    let o = inside("old", None).unwrap();
    assert_eq!((o.moved, o.textures, o.converted_from.clone()), (1, 1, None));
    assert!(!o.lines().iter().any(|l| l.starts_with("Provenance")));
    for d in [home, cache, src, out] {
        let _ = std::fs::remove_dir_all(d);
    }
}
