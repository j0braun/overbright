//! The registries against the asset format's schemas, and the schema export against its kept copy (the drift
//! checks). `OB_STUDIO_BLESS=1` rewrites the kept export.

use ob_studio::registry::{self, COMMANDS, KINDS, Req, Ty};

fn spec(file: &str) -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/modding/asset-format/schemas/").to_string() + file;
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
fn each_kind_has_the_fields_of_its_file_schema() {
    for k in KINDS {
        let s = spec(k.schema_file);
        let entry = if k.entries.is_empty() { &s } else { &s["$defs"]["entry"] };
        let props = entry["properties"].as_object().unwrap();
        let mut ours: Vec<&str> = k.fields.iter().map(|f| f.key).collect();
        let mut theirs: Vec<&str> = props.keys().map(|x| x.as_str()).collect();
        ours.sort();
        theirs.sort();
        assert_eq!(ours, theirs, "{}: fields differ from {}", k.short, k.schema_file);
        let mut required: Vec<&str> = k.fields.iter().filter(|f| f.req == Req::Yes).map(|f| f.key).collect();
        let mut spec_req: Vec<&str> =
            entry["required"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap()).collect()).unwrap_or_default();
        required.sort();
        spec_req.sort();
        assert_eq!(required, spec_req, "{}: required fields", k.short);
        for f in k.fields {
            if let Ty::Enum(words) = f.ty {
                let p = &props[f.key];
                let spec_words: Vec<&str> = match (p.get("enum"), p.get("const")) {
                    (Some(e), _) => e.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect(),
                    (_, Some(c)) => vec![c.as_str().unwrap()],
                    _ => panic!("{}.{}: no enum in the schema", k.short, f.key),
                };
                assert_eq!(words, spec_words.as_slice(), "{}.{}", k.short, f.key);
            }
        }
        assert!(registry::kind(k.short).is_some());
        assert!(
            k.schema.starts_with("ob.") && s["title"].as_str().unwrap().contains(k.schema.split('/').next().unwrap())
        );
    }
}

#[test]
fn each_record_kind_has_the_records_of_its_file_schema() {
    for k in registry::RECORD_KINDS {
        let s = spec(k.schema_file);
        assert!(s["title"].as_str().unwrap().contains(k.schema.split('/').next().unwrap()));
        for r in k.records {
            let def = &s["$defs"][format!("record{}", r.tag)];
            assert_eq!(def["properties"]["record"]["const"].as_str(), Some(r.tag));
            let mut ours: Vec<&str> = r.fields.iter().map(|f| f.key).collect();
            ours.push("record");
            let mut theirs: Vec<&str> = def["properties"].as_object().unwrap().keys().map(|x| x.as_str()).collect();
            ours.sort();
            theirs.sort();
            assert_eq!(ours, theirs, "{}: {} record", k.short, r.tag);
        }
    }
}

#[test]
fn command_names_are_unique_and_stable_in_form() {
    for (i, c) in COMMANDS.iter().enumerate() {
        assert!(c.name.split('.').all(|w| !w.is_empty() && w.chars().all(|ch| ch.is_ascii_lowercase())), "{}", c.name);
        assert!(!COMMANDS[..i].iter().any(|d| d.name == c.name), "{} twice", c.name);
        assert!(registry::command(c.name).is_some());
    }
}

#[test]
fn every_map_edit_has_a_registered_command() {
    use ob_map::tools::map::ops::{Op, PropKey};
    let key = PropKey::Added("a1".into());
    let ops = [
        Op::Delete { key: key.clone() },
        Op::Reset { key: key.clone() },
        Op::DropToGround { key },
        Op::DeleteRace { race: "r".into() },
        Op::Group { label: "g".into(), ops: Vec::new() },
    ];
    for op in &ops {
        assert!(registry::command(registry::command_of(op)).is_some(), "{op:?}");
    }
    // `command_of` matches every edit (it does not build otherwise); its names are all registered.
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/registry.rs"));
    let body = &src[src.find("pub fn command_of").unwrap()..];
    let body = &body[..body.find("\n}\n").unwrap()];
    for name in body.split('"').skip(1).step_by(2) {
        assert!(registry::command(name).is_some(), "{name}");
    }
}

#[test]
fn the_schema_export_has_not_drifted() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/schema/studio.json");
    let now = registry::export().pretty();
    if std::env::var_os("OB_STUDIO_BLESS").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/schema")).unwrap();
        std::fs::write(path, &now).unwrap();
    }
    let kept = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        kept == now,
        "crates/ob-studio/{} differs from the registries; run the test with OB_STUDIO_BLESS=1 and review the diff",
        registry::EXPORT_FILE
    );
}
