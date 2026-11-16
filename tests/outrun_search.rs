//! Independent expected outcomes from 71 original-game search calls.
use glam::{Vec2, Vec3};
use nfsu2::formats::ai_routes::RouteFile;

mod common;

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn chase_search_matches_captured_native_calls() {
    let routes = RouteFile::parse(&common::private("tests/fixtures/outrun-native/routes.bin"));
    assert_eq!(routes.records.len(), 844);
    let mut count = 0;
    for line in common::private_str("tests/fixtures/outrun-native/search.csv")
        .lines()
        .skip(1)
    {
        let v: Vec<_> = line.split(',').collect();
        let f = |i: usize| v[i].parse::<f32>().unwrap();
        let position = Vec3::new(f(2), f(3), f(4));
        let direction = Vec2::new(f(5), f(6));
        let target = Vec3::new(f(7), f(8), f(9));
        let got =
            nfsu2::ai::outrun_search::search(&routes, position, direction, target, 400.0, false);
        assert_eq!(got.is_some(), v[10] == "1", "native case: {line}");
        if let Some(got) = got {
            assert_eq!(
                routes.records[got.record].id,
                v[11].parse::<u16>().unwrap(),
                "{line}"
            );
            assert_eq!(got.point, v[12].parse::<usize>().unwrap(), "{line}");
            assert_eq!(got.forward, v[13] == "1", "{line}");
            assert!(
                (got.spent - f(14)).abs() <= 0.001,
                "native case {line}: spent {}",
                got.spent
            );
            let decisions: Vec<(usize, bool)> = v[15]
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let v = s.parse::<usize>().unwrap();
                    (v & 8191, v & 8192 != 0)
                })
                .collect();
            assert_eq!(got.decisions, decisions, "{line}");
        }
        count += 1;
    }
    assert_eq!(count, 71);
}
