use super::*;

/// A game folder with these files in MOVIES (empty files).
fn install(tag: &str, files: &[&str]) -> crate::testkit::Scratch {
    let s = crate::testkit::Scratch::new(tag);
    let movies = s.path().join("MOVIES");
    std::fs::create_dir_all(&movies).unwrap();
    for f in files {
        std::fs::write(movies.join(f), b"").unwrap();
    }
    s
}

/// The file name found for `name`, and how.
fn found(root: &Path, name: &str) -> (Option<String>, How) {
    let r = resolve_movie(root, name);
    (r.file_name(), r.how)
}

/// The US 1.2 English edition: no suffix at all (and movies.tag).
#[test]
fn us_names_without_suffix_resolve() {
    let s = install(
        "assets-us",
        &[
            "blank.vp6",
            "ealogo.vp6",
            "THX_logo.vp6",
            "PSA.vp6",
            "FMVOpening.vp6",
            "Scene01.vp6",
            "DriftTut.vp6",
            "movies.tag",
        ],
    );
    let r = s.path();
    assert_eq!(found(r, "ealogo"), (Some("ealogo.vp6".into()), How::Plain));
    assert_eq!(found(r, "PSA"), (Some("PSA.vp6".into()), How::Plain));
    assert_eq!(found(r, "scene01"), (Some("Scene01.vp6".into()), How::Plain));
    assert_eq!(found(r, "DriftTut"), (Some("DriftTut.vp6".into()), How::Plain));
    assert_eq!(found(r, "THX_logo_fr"), (Some("THX_logo.vp6".into()), How::Plain));
    // The names the executable builds plain stay native.
    assert_eq!(found(r, "blank"), (Some("blank.vp6".into()), How::Built));
    assert_eq!(found(r, "FMVOpening"), (Some("FMVOpening.vp6".into()), How::Built));
    assert_eq!(crate::movie::path(r, "Scene01"), Some(r.join("MOVIES/Scene01.vp6")));
}

/// The European data: `_en` files (the logo in more languages too).
#[test]
fn european_english_files_resolve() {
    let s = install(
        "assets-eu",
        &["ealogo_dn.vp6", "ealogo_du.vp6", "ealogo_en.vp6", "ealogo_sw.vp6", "Scene01_en.vp6", "PSA_en.vp6"],
    );
    let r = s.path();
    assert_eq!(found(r, "ealogo"), (Some("ealogo_en.vp6".into()), How::Language("en".into())));
    assert_eq!(found(r, "scene01"), (Some("Scene01_en.vp6".into()), How::Language("en".into())));
    assert_eq!(found(r, "PSA"), (Some("PSA_en.vp6".into()), How::Language("en".into())));
    let tried = resolve_movie(r, "DriftTut").tried;
    assert_eq!(tried, ["DriftTut_fr.vp6", "DriftTut.vp6", "DriftTut_en.vp6", "DriftTut_*.vp6"]);
    assert_eq!(found(r, "DriftTut"), (None, How::Missing));
}

/// A single-language edition with another suffix (`_ru`): the one file
/// there plays.
#[test]
fn single_other_language_resolves() {
    let s = install("assets-ru", &["ealogo_ru.vp6", "SCENE01_RU.VP6", "DriftTut_ru.vp6", "Scene010_ru.vp6"]);
    let r = s.path();
    assert_eq!(found(r, "ealogo"), (Some("ealogo_ru.vp6".into()), How::Other("ru".into())));
    assert_eq!(found(r, "scene01"), (Some("SCENE01_RU.VP6".into()), How::Other("ru".into())));
    assert_eq!(found(r, "DriftTut"), (Some("DriftTut_ru.vp6".into()), How::Other("ru".into())));
    // Scene010 is not Scene01 in another language.
    assert_eq!(found(r, "Scene05"), (None, How::Missing));
}

/// A mixed folder: the order decides (built, plain, the text language's,
/// English, then the first other by name).
#[test]
fn mixed_folder_follows_the_order() {
    let s = install(
        "assets-mixed",
        &[
            "ealogo_fr.vp6",
            "ealogo.vp6",
            "ealogo_en.vp6",
            "PSA.vp6",
            "PSA_en.vp6",
            "Scene01_sw.vp6",
            "Scene01_dn.vp6",
            "Scene05_en.vp6",
        ],
    );
    let r = s.path();
    assert_eq!(found(r, "ealogo"), (Some("ealogo_fr.vp6".into()), How::Built));
    assert_eq!(found(r, "PSA"), (Some("PSA.vp6".into()), How::Plain));
    assert_eq!(found(r, "Scene01"), (Some("Scene01_dn.vp6".into()), How::Other("dn".into())));
    assert_eq!(found(r, "Scene05"), (Some("Scene05_en.vp6".into()), How::Language("en".into())));
}

/// No MOVIES folder: every movie missing, nothing panics.
#[test]
fn no_movies_folder() {
    let s = crate::testkit::Scratch::new("assets-none");
    assert_eq!(found(s.path(), "ealogo"), (None, How::Missing));
    assert!(movies_dir(s.path()).is_none());
}

#[test]
fn stems_and_suffixes() {
    assert_eq!(movie_stem("THX_logo_fr"), "THX_logo");
    assert_eq!(movie_stem("THX_logo"), "THX_logo");
    assert_eq!(movie_stem("Scene01"), "Scene01");
    assert_eq!(suffix_of("Scene01_EN.VP6", "scene01"), Some("EN"));
    assert_eq!(suffix_of("Scene010_en.vp6", "Scene01"), None);
    assert_eq!(suffix_of("Scene01.vp6", "Scene01"), None);
    assert_eq!(suffix_of("Scene01_en.bik", "Scene01"), None);
    assert_eq!(language_suffix("english"), Some("en"));
    assert_eq!(suffix_language("GR"), Some("German"));
}

/// Every movie the game asks for, as files of an edition: `name(stem)`
/// gives each file's name (None: left out).
fn edition(tag: &str, name: impl Fn(&str) -> Option<String>, extra: &[&str]) -> crate::testkit::Scratch {
    let mut files: Vec<String> = wanted_movies().into_iter().filter_map(|n| name(movie_stem(n))).collect();
    files.extend(extra.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    install(tag, &refs)
}

/// The names the reference executable builds plain stay plain in every
/// edition (blank, FMVOpening, the flybys, THX_logo).
fn plain_name(stem: &str) -> bool {
    crate::movie::file_name(stem) == format!("{stem}.vp6")
}

/// Property: the check reports exactly what the game opens, for every
/// wanted movie, in every layout.
fn check_and_game_agree(root: &Path) {
    let r = scan(root);
    for i in r.items.iter().filter(|i| i.group != Group::Text) {
        assert_eq!(i.resolved, resolve_movie(root, &i.resolved.want), "{}", i.label);
        assert_eq!(i.resolved.file, crate::movie::path(root, &i.resolved.want), "{}", i.label);
    }
}

#[test]
fn us_edition_report() {
    let s = edition("assets-us-report", |st| Some(format!("{st}.vp6")), &["movies.tag"]);
    let r = scan(s.path());
    assert_eq!(r.edition.layout, Layout::Plain { tag: true });
    assert!(r.missing_movies().is_empty());
    assert_eq!(r.movie_language().as_deref(), Some("no language suffix"));
    let lines = r.summary();
    assert!(lines[0].1.contains("US layout"), "{lines:?}");
    let n = wanted_movies().len();
    assert!(
        lines.iter().any(|(m, t)| *m == Mark::Ok && t.starts_with(&format!("All {n} movies found ("))),
        "{lines:?}"
    );
    assert!(r.details().iter().any(|l| l == "  Scene01: Scene01.vp6 (no language suffix)"), "{:?}", r.details());
    check_and_game_agree(s.path());
}

#[test]
fn european_edition_report() {
    let s = edition(
        "assets-eu-report",
        |st| Some(if plain_name(st) { format!("{st}.vp6") } else { format!("{st}_en.vp6") }),
        &["ealogo_dn.vp6", "ealogo_du.vp6", "ealogo_sw.vp6", "movies.tag"],
    );
    let r = scan(s.path());
    assert_eq!(r.edition.layout, Layout::Multi(vec!["dn".into(), "du".into(), "en".into(), "sw".into()]));
    assert_eq!(r.movie_language().as_deref(), Some("English"));
    assert!(r.missing_movies().is_empty());
    assert!(r.summary()[0].1.contains("European multi-language"), "{:?}", r.summary());
    check_and_game_agree(s.path());
}

#[test]
fn russian_only_edition_report() {
    let s = edition(
        "assets-ru-report",
        |st| match st {
            "Scene05" | "DriftTut" => None,
            _ if plain_name(st) => Some(format!("{st}.vp6")),
            _ => Some(format!("{st}_ru.vp6")),
        },
        &[],
    );
    let r = scan(s.path());
    assert_eq!(r.edition.layout, Layout::Single("ru".into()));
    assert_eq!(r.movie_language().as_deref(), Some("Russian"));
    assert_eq!(r.missing_movies(), ["Scene05", "DriftTut"]);
    let lines = r.summary();
    assert!(
        lines.iter().any(|(m, t)| *m == Mark::Warn && t.starts_with("2 movies will not play: Scene05, DriftTut")),
        "{lines:?}"
    );
    let groups = r.groups();
    assert!(
        groups
            .iter()
            .any(|(m, t)| *m == Mark::Warn && t.starts_with("Story movies:") && t.ends_with("missing: Scene05")),
        "{groups:?}"
    );
    assert!(
        r.details()
            .iter()
            .any(|l| l == "  DriftTut: missing (tried DriftTut_fr.vp6, DriftTut.vp6, DriftTut_en.vp6, DriftTut_*.vp6)"),
        "{:?}",
        r.details()
    );
    check_and_game_agree(s.path());
}

#[test]
fn mixed_edition_report() {
    let s = edition(
        "assets-mixed-report",
        |st| Some(if st.starts_with("Scene") { format!("{st}_en.vp6") } else { format!("{st}.vp6") }),
        &[],
    );
    let r = scan(s.path());
    assert_eq!(r.edition.layout, Layout::Mixed(vec!["en".into()]));
    assert!(r.missing_movies().is_empty());
    check_and_game_agree(s.path());
}

/// No MOVIES folder: one warning, every movie missing.
#[test]
fn no_movies_report() {
    let s = crate::testkit::Scratch::new("assets-none-report");
    let r = scan(s.path());
    assert_eq!(r.edition.layout, Layout::NoMovies);
    assert!(r.summary().iter().any(|(m, t)| *m == Mark::Warn && t.starts_with("No MOVIES folder")));
    assert_eq!(r.missing_movies().len(), wanted_movies().len());
}

/// The suffix after the name format, as the reference executable holds it
/// (`.vp6\0…%sMOVIES\%s%s\0…_fr.vp6\0THX_logo\0`).
#[test]
fn exe_suffix_is_read_after_the_format() {
    let exe = b"xx.vp6\0\0\0\0%sMOVIES\\%s%s\0\0\0\0_fr.vp6\0THX_logo\0".to_vec();
    assert_eq!(exe_movie_suffix(&exe).as_deref(), Some("fr"));
    assert_eq!(exe_movie_suffix(b"%sMOVIES\\%s%s\0.vp6\0").as_deref(), None);
    assert_eq!(exe_movie_suffix(b"nothing here").as_deref(), None);
}

/// The reference install: every movie the game asks for is there, the
/// European layout, the executable's `_fr`; and every movie the career
/// names (events, shops) is on the wanted list.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn reference_install_report() {
    let root = crate::testkit::game_root();
    let r = scan(&root);
    assert!(r.missing_movies().is_empty(), "{:?}", r.details());
    assert!(matches!(r.edition.layout, Layout::Multi(_)), "{:?}", r.edition);
    assert_eq!(r.edition.exe_suffix.as_deref(), Some("fr"));
    assert!(r.items.iter().filter(|i| i.group == Group::Text).all(|i| i.resolved.file.is_some()), "{:?}", r.details());
    check_and_game_agree(&root);
    let career = crate::frontend::career::CareerData::load(&root).unwrap();
    let wanted = wanted_movies();
    let named = career
        .events
        .iter()
        .flat_map(|e| [e.movie.as_str(), e.won_movie.as_str()])
        .chain(career.shops.iter().map(|s| s.movie.as_str()))
        .filter(|n| !n.is_empty());
    for n in named {
        assert!(wanted.iter().any(|w| w.eq_ignore_ascii_case(n)), "{n} is not on the wanted list");
    }
}
