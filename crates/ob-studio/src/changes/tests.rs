//! The Changes rows on layered fixtures: yours, the original and another mod's value; reset; a step compared;
//! a map mod read into the tables.

use glam::{Mat3, Vec3};
use ob_asset_format::layering::{Entry, KindRules, Layer, Op};
use ob_map::tools::map::format::{CustomRace, MapMod, PropEdit, PropRef, RaceKind};

use super::map::{self, Game};
use super::*;

fn t(s: &str) -> toml::Table {
    s.parse().unwrap()
}

fn entry(key: &str, fields: &str) -> Entry {
    Entry { key: key.into(), op: None, disabled: None, fields: t(fields) }
}

fn layer(pack: &str, entries: Vec<Entry>) -> Layer {
    Layer { pack: pack.into(), dependencies: vec![], entries }
}

fn base() -> Vec<(String, toml::Table)> {
    vec![
        ("dome".into(), t("position = [-860.2, -297.1, 12.0]\nheading = 90.0\ntexture = \"Dome roof\"")),
        ("barrel".into(), t("position = [-805.1, -309.9, 11.8]")),
        ("cone".into(), t("position = [1.0, 2.0, 3.0]")),
    ]
}

/// city-signs (weakest) · harbour (yours) · drift-park (strongest): the mockup's stack.
fn stack() -> Vec<Layer> {
    vec![
        layer("city-signs", vec![entry("dome", "texture = \"Dome roof (city-signs)\"")]),
        layer(
            "harbour",
            vec![
                entry("dome", "position = [-842.4, -297.1, 12.0]\nheading = 102.0\ntexture = \"Dome roof, rusty\""),
                entry("barrel", "position = [-799.2, -312.0, 11.8]"),
                entry("harbour:sprint", "name = \"Casino sprint\""),
            ],
        ),
        layer("drift-park", vec![entry("barrel", "position = [-801.0, -310.5, 11.8]")]),
    ]
}

fn tables<'a>(base: &'a [(String, toml::Table)], layers: &'a [Layer]) -> Tables<'a> {
    Tables { kind: "placement", base, layers, rules: &KindRules::ALL }
}

fn value<'a>(rows: &'a [Row], key: &str, field: &str) -> &'a Value {
    rows.iter().find(|r| r.key == key).unwrap().values.iter().find(|v| v.field == field).unwrap()
}

#[test]
fn rows_tell_yours_the_original_and_another_mod_apart() {
    let (b, l) = (base(), stack());
    let rows = this_mod(&tables(&b, &l), "harbour");
    assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["dome", "barrel", "harbour:sprint"]);
    // Yours, with the original beside it.
    let pos = value(&rows, "dome", "position");
    assert_eq!(pos.from, From::Yours);
    assert_eq!(pos.original, Some(toml::Value::Array(vec![(-860.2).into(), (-297.1).into(), 12.0.into()])));
    assert!(!pos.loses());
    // Yours is above city-signs: yours wins.
    assert_eq!(value(&rows, "dome", "texture").from, From::Yours);
    // Drift park is above yours: its value is used, yours is kept beside it.
    let barrel = value(&rows, "barrel", "position");
    assert_eq!(barrel.from, From::Other("drift-park".into()));
    assert!(barrel.loses());
    assert_eq!(barrel.yours.as_ref().unwrap().as_array().unwrap()[0].as_float(), Some(-799.2));
    assert_eq!(barrel.game.as_ref().unwrap().as_array().unwrap()[0].as_float(), Some(-801.0));
    assert_eq!(rows[1].from(), From::Other("drift-park".into()));
    // An addition has no original.
    let sprint = rows.iter().find(|r| r.key == "harbour:sprint").unwrap();
    assert!(sprint.added && sprint.values[0].original.is_none());
    // The item this mod does not touch has no row.
    assert!(!rows.iter().any(|r| r.key == "cone"));
}

#[test]
fn the_filters_keep_mine_losing_or_every_value() {
    let (b, l) = (base(), stack());
    let rows = this_mod(&tables(&b, &l), "harbour");
    let mine = filter(&rows, Show::Mine);
    assert_eq!(mine.iter().map(|r| r.values.len()).sum::<usize>(), 5);
    let losing = filter(&rows, Show::Losing);
    assert_eq!(losing.len(), 1);
    assert_eq!(losing[0].key, "barrel");
    // Every value: the untouched `disabled` with the original's ring.
    let all = filter(&rows, Show::All);
    let d = value(&all, "dome", "disabled");
    assert_eq!((d.from.clone(), d.yours.clone()), (From::Original, None));
}

#[test]
fn reordering_the_game_changes_who_wins() {
    let b = base();
    let mut l = stack();
    l.swap(0, 1); // harbour below city-signs now
    let rows = this_mod(&tables(&b, &l), "harbour");
    assert_eq!(value(&rows, "dome", "texture").from, From::Other("city-signs".into()));
    // Alone in the game, every value is yours.
    let alone = vec![stack().remove(1)];
    let rows = this_mod(&tables(&b, &alone), "harbour");
    assert!(rows.iter().all(|r| !r.loses()));
}

#[test]
fn clashes_list_the_stack_in_game_order() {
    let (b, l) = (base(), stack());
    let c = clashes(&tables(&b, &l), "harbour");
    assert_eq!(c.len(), 2, "{c:?}");
    let tex = c.iter().find(|c| c.key == "dome").unwrap();
    let packs: Vec<&str> = tex.stack.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(packs, ["", "city-signs", "harbour"]);
    assert_eq!(tex.winner, From::Yours);
    let barrel = c.iter().find(|c| c.key == "barrel").unwrap();
    assert_eq!(barrel.winner, From::Other("drift-park".into()));
    assert_eq!(barrel.stack.last().unwrap().0, "drift-park");
}

#[test]
fn reset_gives_one_value_back_and_nothing_else() {
    let b = base();
    let mut l = stack();
    assert!(reset(&mut l[1], "dome", "position"));
    let rows = this_mod(&tables(&b, &l), "harbour");
    // Back to the original; the heading is still yours.
    assert!(
        rows.iter()
            .find(|r| r.key == "dome")
            .unwrap()
            .values
            .iter()
            .all(|v| v.field != "position" || v.yours.is_none())
    );
    assert_eq!(value(&rows, "dome", "position").from, From::Original);
    assert_eq!(value(&rows, "dome", "heading").from, From::Yours);
    // Reset where another mod gives a value: that mod's value stays in the game, yours is gone.
    assert!(reset(&mut l[1], "barrel", "position"));
    assert!(!l[1].entries.iter().any(|e| e.key == "barrel"), "an entry giving nothing is dropped");
    let game = layering::merge(b.clone(), &l, &KindRules::ALL);
    assert_eq!(game.get("barrel").unwrap().from_of("position"), Some("drift-park"));
    // A value the mod does not give is not reset.
    assert!(!reset(&mut l[1], "cone", "position"));
    // `disabled` given back.
    let mut hide = layer(
        "harbour",
        vec![Entry { key: "cone".into(), op: Some(Op::Merge), disabled: Some(true), fields: toml::Table::new() }],
    );
    assert!(reset(&mut hide, "cone", "disabled"));
    assert!(hide.entries.is_empty());
}

#[test]
fn compare_lists_what_one_step_changed() {
    let before = vec![("dome".to_string(), t("position = [1.0, 2.0, 3.0]\nheading = 90.0"))];
    let after = vec![
        ("dome".to_string(), t("position = [1.5, 2.0, 3.0]\nheading = 90.0")),
        ("cone".to_string(), t("position = [0.0, 0.0, 0.0]")),
    ];
    let rows = compare("placement", &before, &after);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].values.len(), 1, "the heading did not change");
    assert_eq!(rows[0].values[0].field, "position");
    assert!(rows[1].added);
}

// ---- a map mod read into the tables ----

fn target(x: f32) -> PropRef {
    PropRef { region: "L4RA".into(), section: 1407, solid: "XO_DOME".into(), at: Vec3::new(x, 0.0, 0.0), nth: 0 }
}

fn mv(x: f32, to: f32) -> PropEdit {
    PropEdit::Move { target: target(x), position: Vec3::new(to, 0.0, 0.0), rotation: Mat3::IDENTITY }
}

fn mapmod(props: Vec<PropEdit>, races: Vec<CustomRace>) -> MapMod {
    MapMod { name: "m".into(), author: String::new(), description: String::new(), props, races, collision: vec![] }
}

#[test]
fn map_mods_layer_in_their_enabled_order() {
    let harbour = mapmod(
        vec![mv(1.0, 5.0), mv(2.0, 6.0), PropEdit::Delete { target: target(3.0) }],
        vec![CustomRace::new("sprint", 1, RaceKind::Sprint)],
    );
    let drift = mapmod(vec![mv(2.0, 7.0)], vec![]);
    let g = Game::new(&[("harbour".into(), harbour.clone()), ("drift-park".into(), drift)]);
    let rows = g.this_mod("harbour");
    let k1 = map::placement_key(&target(1.0));
    let k2 = map::placement_key(&target(2.0));
    let k3 = map::placement_key(&target(3.0));
    assert_eq!(value(&rows, &k1, "position").from, From::Yours);
    assert_eq!(value(&rows, &k1, "position").original.as_ref().unwrap().as_array().unwrap()[0].as_float(), Some(1.0));
    assert_eq!(value(&rows, &k2, "position").from, From::Other("drift-park".into()));
    assert_eq!(value(&rows, &k3, "disabled").from, From::Yours);
    let race = rows.iter().find(|r| r.kind == map::RACE).unwrap();
    assert_eq!(race.key, "harbour:sprint");
    // Both moved the second: its position and heading clash.
    let c = g.clashes("harbour");
    assert_eq!(c.iter().map(|c| c.field.as_str()).collect::<Vec<_>>(), ["heading", "position"]);
    assert!(c.iter().all(|c| c.key == k2 && c.winner == From::Other("drift-park".into())));
    // The headings' originals are unknown here.
    assert!(value(&rows, &k1, "heading").original.is_none());
    assert!(!map::original_known(map::PLACEMENT, "heading"));
}

#[test]
fn map_reset_puts_a_prop_back_or_takes_the_edit_out() {
    let m = mapmod(
        vec![mv(1.0, 5.0), PropEdit::Delete { target: target(3.0) }],
        vec![CustomRace::new("sprint", 1, RaceKind::Sprint)],
    );
    let k1 = map::placement_key(&target(1.0));
    let back = map::reset(&m, "harbour", map::PLACEMENT, &k1, "position").unwrap();
    assert!(matches!(&back.props[0], PropEdit::Move { position, .. } if *position == Vec3::new(1.0, 0.0, 0.0)));
    let rows = Game::new(&[("harbour".into(), back.clone())]).this_mod("harbour");
    assert_eq!(value(&rows, &k1, "position").from, From::Original, "same as the original is the original's");
    // The heading's original is the game's: its reset takes the move out.
    let gone = map::reset(&m, "harbour", map::PLACEMENT, &k1, "heading").unwrap();
    assert_eq!(gone.props.len(), 1);
    let k3 = map::placement_key(&target(3.0));
    let shown = map::reset(&m, "harbour", map::PLACEMENT, &k3, "disabled").unwrap();
    assert!(shown.props.iter().all(|p| !matches!(p, PropEdit::Delete { .. })));
    let no_race = map::reset(&m, "harbour", map::RACE, "harbour:sprint", "name").unwrap();
    assert!(no_race.races.is_empty());
    assert!(map::reset(&m, "harbour", map::RACE, "harbour:nothing", "name").is_none());
    // Several in one step.
    let picks = vec![
        (map::PLACEMENT.to_string(), k1.clone(), "heading".to_string()),
        (map::PLACEMENT.to_string(), k1, "position".to_string()),
        (map::PLACEMENT.to_string(), k3, "disabled".to_string()),
    ];
    assert!(map::reset_all(&m, "harbour", &picks).unwrap().props.is_empty());
    // Comparing the step: what the reset changed.
    let rows = map::compare_maps(&m, &back, "harbour");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values[0].field, "position");
}

fn paint(hashes: &[(u32, &str)]) -> crate::looks::Looks {
    use crate::looks::{Domain, Entry, Origin, TextureId};
    let mut l = crate::looks::Looks::default();
    for (h, img) in hashes {
        let id = TextureId::new(Domain::World, *h);
        l.entries.insert(
            id,
            Entry {
                id,
                name: format!("TEX_{h:X}"),
                origin: Origin::Game,
                image: img.to_string(),
                width: 64,
                height: 64,
            },
        );
    }
    l
}

#[test]
fn textures_layer_in_their_enabled_order() {
    use super::looks::{GAME, IMAGE, Textures, compare_looks, reset};
    let mine = paint(&[(0x1A, "aaaa"), (0x2B, "bbbb")]);
    let other = paint(&[(0x2B, "cccc")]);
    let game = Textures::new(&[("mine".into(), mine.clone()), ("other".into(), other.clone())]);
    let rows = game.this_mod("mine");
    assert_eq!(rows.len(), 2, "one row per texture");
    let v = value(&rows, "world:0x0000001A", IMAGE);
    assert_eq!(v.from, From::Yours);
    assert_eq!(v.original, Some(toml::Value::String(GAME.into())));
    assert_eq!(v.yours, Some(toml::Value::String("aaaa".into())));
    let v = value(&rows, "world:0x0000002B", IMAGE);
    assert_eq!(v.from, From::Other("other".into()), "the stronger mod's paint is drawn");
    assert!(v.loses());
    assert_eq!(v.game, Some(toml::Value::String("cccc".into())));
    let c = game.clashes("mine");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].stack.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["", "mine", "other"]);
    // The other way round, yours wins.
    let flipped = Textures::new(&[("other".into(), other), ("mine".into(), mine.clone())]);
    assert!(flipped.this_mod("mine").iter().all(|r| !r.loses()));
    // A step that painted one more texture and repainted one.
    let after = paint(&[(0x1A, "dddd"), (0x2B, "bbbb"), (0x3C, "eeee")]);
    let steps = compare_looks(&mine, &after);
    assert_eq!(steps.len(), 2);
    assert!(steps.iter().all(|r| !r.added && r.kind == "texture"));
    let new = steps.iter().find(|r| r.key == "world:0x0000003C").unwrap();
    assert_eq!(new.values[0].original, Some(toml::Value::String(GAME.into())));
    // Reset gives a texture back to the game.
    let back = reset(&mine, &["world:0x0000001A"]).unwrap();
    assert_eq!(back.entries.len(), 1);
    assert!(reset(&mine, &["world:0x000000FF"]).is_none());
}
