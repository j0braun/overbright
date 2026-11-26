use super::*;

fn engine() -> semver::Version {
    semver::Version::new(0, 6, 0)
}

fn toml(id: &str, extra: &str) -> String {
    format!(
        "format = 2\nkind = \"mod\"\nid = \"{id}\"\nname = \"{id} pack\"\nversion = \"1.0.0\"\noverbright = \">=0.5\"\norigin = \"own-work\"\nshare = \"local-only\"\n{extra}"
    )
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-modpack-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(dir: &Path, id: &str, extra: &str) -> PathBuf {
    let bytes =
        pack_file_bytes(&[("pack.toml".into(), toml(id, extra).into_bytes()), ("map.obmap".into(), b"x".to_vec())])
            .unwrap();
    let p = dir.join(format!("{id}-1.0.0.obpk"));
    std::fs::write(&p, bytes).unwrap();
    p
}

fn base(id: &str) -> BaseId {
    BaseId { id: id.into(), identity: "sha256:aa".into() }
}

#[test]
fn order_decides_the_stack_and_unlisted_packs_are_off() {
    let d = scratch("order");
    for id in ["a", "b", "c"] {
        put(&d, id, "");
    }
    assert!(plan(&d, None, &engine()).layers.is_empty(), "nothing listed: nothing on");
    set_enabled(&d, "a", true).unwrap();
    set_enabled(&d, "b", true).unwrap();
    let ids = |p: &Plan| p.layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
    // order.txt lists strongest first: a above b; the stack is weakest first.
    assert_eq!(ids(&plan(&d, None, &engine())), ["b", "a"]);
    let r = rows(&d, None, &engine());
    assert_eq!(
        r.iter().map(|r| (r.id.as_str(), r.on, r.new)).collect::<Vec<_>>(),
        [("a", true, false), ("b", true, false), ("c", false, true)]
    );
    move_pack(&d, "b", -1).unwrap();
    assert_eq!(ids(&plan(&d, None, &engine())), ["a", "b"]);
    set_enabled(&d, "a", false).unwrap();
    assert_eq!(ids(&plan(&d, None, &engine())), ["b"]);
    let r = rows(&d, None, &engine());
    assert_eq!(
        r.iter().map(|r| (r.id.as_str(), r.on, r.new)).collect::<Vec<_>>(),
        [("b", true, false), ("a", false, false), ("c", false, false)]
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_pack_for_another_base_is_refused_with_its_words() {
    let d = scratch("base");
    put(&d, "eu-only", "\n[game]\nbases = [\"nfsu2-pc12-eu\"]\nfingerprint = [\"sha256:aa\"]\n");
    put(&d, "wild", "\n[game]\nbases = [\"nfsu2-*\"]\n");
    put(&d, "any", "");
    for id in ["eu-only", "wild", "any"] {
        set_enabled(&d, id, true).unwrap();
    }
    let p = plan(&d, Some(&base("nfsu2-pc12-na")), &engine());
    assert_eq!(p.layers.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(), ["any", "wild"]);
    assert_eq!(p.refused.len(), 1);
    assert!(p.refused[0].1.contains("nfsu2-pc12-eu") && p.refused[0].1.contains("nfsu2-pc12-na"), "{:?}", p.refused);
    let ok = plan(&d, Some(&base("nfsu2-pc12-eu")), &engine());
    assert_eq!(ok.layers.len(), 3);
    let other = BaseId { id: "nfsu2-pc12-eu".into(), identity: "sha256:bb".into() };
    let fp = plan(&d, Some(&other), &engine());
    assert!(fp.refused[0].1.contains("fingerprint"), "{:?}", fp.refused);
    let none = plan(&d, None, &engine());
    assert!(none.refused[0].1.contains("no imported copy"), "{:?}", none.refused);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn engine_range_and_residual_are_refused() {
    let d = scratch("engine");
    let new = toml("future", "").replace(">=0.5", ">=9.0");
    std::fs::write(d.join("future-1.0.0.obpk"), pack_file_bytes(&[("pack.toml".into(), new.into_bytes())]).unwrap())
        .unwrap();
    std::fs::write(
        d.join("res-1.0.0.obpk"),
        pack_file_bytes(&[("pack.toml".into(), toml("res", "").into_bytes()), ("residual/A.bin".into(), vec![1])])
            .unwrap(),
    )
    .unwrap();
    set_enabled(&d, "future", true).unwrap();
    set_enabled(&d, "res", true).unwrap();
    let p = plan(&d, None, &engine());
    assert!(p.layers.is_empty());
    assert_eq!(p.refused.len(), 2);
    assert!(p.refused.iter().any(|(_, w)| w.contains(">=9.0")));
    assert!(p.refused.iter().any(|(_, w)| w.contains("MRG.6")));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_dependency_goes_on_and_below() {
    let d = scratch("deps");
    put(&d, "lib", "");
    put(&d, "top", "\n[dependencies]\nlib = \">=1.0\"\n");
    set_enabled(&d, "lib", false).unwrap();
    set_enabled(&d, "top", false).unwrap();
    move_pack(&d, "top", -1).unwrap_or(());
    let also = set_enabled(&d, "top", true).unwrap();
    assert_eq!(also, ["lib"]);
    let p = plan(&d, None, &engine());
    assert_eq!(p.layers.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(), ["lib", "top"]);
    assert!(p.refused.is_empty());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn import_checks_and_never_replaces() {
    let src = scratch("imp-src");
    let d = scratch("imp-dst");
    let f = put(&src, "mine", "");
    assert_eq!(import(&d, &f, &engine()).unwrap(), "mine");
    assert!(d.join("mine-1.0.0.obpk").is_file());
    assert!(!is_listed(&d, "mine"), "a new pack is off");
    assert!(import(&d, &f, &engine()).unwrap_err().contains("already installed"));
    let bad = src.join("bad.obpk");
    std::fs::write(&bad, b"not a zip").unwrap();
    assert!(import(&d, &bad, &engine()).is_err());
    let txt = src.join("x.txt");
    std::fs::write(&txt, b"x").unwrap();
    assert!(import(&d, &txt, &engine()).unwrap_err().contains("not a pack file"));
    assert_eq!(
        pack_file_bytes(&[("pack.toml".into(), b"a".to_vec())]).unwrap(),
        pack_file_bytes(&[("pack.toml".into(), b"a".to_vec())]).unwrap()
    );
    let _ = std::fs::remove_dir_all(src);
    let _ = std::fs::remove_dir_all(d);
}
