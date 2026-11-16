//! The `--shot` targets: one entry per target, a name and the
//! function that sets its scene up, in files per group. `fn shot` looks
//! the screen up here ([`run`]); `game --shot-list` prints the table
//! ([`list`]) for `scripts/checks/shot_targets.py`.
pub(super) use super::OPTIONS_SHOT_AT;
pub(super) use super::env::ShotEnv;
pub(super) use crate::*;

mod career_screens;
mod dyno;
mod explore;
mod fe;
mod intro;
mod mouse;
mod options_pages;
mod race;
mod shops;

/// What a target's setup gets from the shot: the GPU and target format
/// (for the renderers a scene builds), the screen as named after the
/// prefix and suffix variants (`finished_career_`, `_dday`, `_kit`, …) were taken
/// off (a setup shared by several names tells them apart by it), and the
/// shot's environment.
#[derive(Clone, Copy)]
pub(crate) struct ShotCtx<'a> {
    pub(crate) gpu: &'a Gpu,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) screen: &'a str,
    pub(crate) env: &'a ShotEnv,
}

/// A target's setup: the mode the shot draws (the old match's arm).
pub(crate) type SceneFn = fn(&mut Game, &ShotCtx) -> Result<Mode>;

/// How a target is named.
pub(crate) enum Pattern {
    /// Exactly this screen.
    Name(&'static str),
    /// Every screen with this prefix (its setup reads the page from the
    /// rest); the checks run the pages listed.
    Pages(&'static str, &'static [&'static str]),
}

/// One `--shot` target (or one family of pages).
pub(crate) struct Target {
    pub(crate) pattern: Pattern,
    pub(crate) scene: SceneFn,
}

impl Target {
    pub(crate) const fn name(name: &'static str, scene: SceneFn) -> Self {
        Self { pattern: Pattern::Name(name), scene }
    }

    pub(crate) const fn pages(prefix: &'static str, pages: &'static [&'static str], scene: SceneFn) -> Self {
        Self { pattern: Pattern::Pages(prefix, pages), scene }
    }
}

/// The groups (one file each) and their targets.
pub(crate) const GROUPS: &[(&str, &[Target])] = &[
    ("fe", fe::TARGETS),
    ("mouse", mouse::TARGETS),
    ("options", options_pages::TARGETS),
    ("shops", shops::TARGETS),
    ("dyno", dyno::TARGETS),
    ("career", career_screens::TARGETS),
    ("race", race::TARGETS),
    ("intro", intro::TARGETS),
    ("explore", explore::TARGETS),
];

/// Targets `fn shot` handles before the setup and the table: `photo`
/// (photo mode's first frame) and `explore_rain` (`explore` in the
/// shower), with the group they are listed in.
pub(crate) const EARLY: &[(&str, &str)] = &[("photo", "explore"), ("explore_rain", "explore")];

/// Variants the checks run that are not entries of their own: a table
/// entry with a prefix or suffix `fn shot` takes off first (`finished_career_`:
/// the finished career; `_dday`: Rachel's car; `_kit`: with its
/// aftermarket parts) or an alias (`crib_parts`: the garage's parts).
pub(crate) const VARIANTS: &[(&str, &str)] = &[
    ("main_dday", "fe"),
    ("explore_kit", "explore"),
    ("crib_parts", "shops"),
    ("finished_career_shop_checks", "shops"),
    ("finished_career_specials_hydro3", "shops"),
    ("finished_career_specials_purge", "shops"),
    ("finished_career_dynorun", "dyno"),
];

/// Entries the full check set leaves out, with why.
pub(crate) const UNLISTED: &[(&str, &str)] = &[
    ("shop_checks", "needs the finished career: run as finished_career_shop_checks"),
    ("specials_hydro3", "needs the finished career: run as finished_career_specials_hydro3"),
    ("specials_purge", "needs the finished career: run as finished_career_specials_purge"),
    ("vinyl_ask", "fails on a fresh profile ('the vinyl question' not reached); open"),
];

/// The screen's setup: the first entry named exactly so, else the first
/// family of pages whose prefix it has (the old match's order: no named
/// entry shares a pages prefix after it).
fn find(screen: &str) -> Option<SceneFn> {
    let all = || GROUPS.iter().flat_map(|(_, t)| t.iter());
    all()
        .find(|t| matches!(t.pattern, Pattern::Name(n) if n == screen))
        .or_else(|| all().find(|t| matches!(t.pattern, Pattern::Pages(p, _) if screen.starts_with(p))))
        .map(|t| t.scene)
}

/// The mode the shot draws for `cx.screen`.
pub(crate) fn run(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    match find(cx.screen) {
        Some(scene) => scene(game, cx),
        None => anyhow::bail!("unknown screen {}", cx.screen),
    }
}

/// The check set: every target as `name<TAB>group`, in table order (the
/// table's names and pages, the early targets and the variants, without
/// the unlisted entries).
pub(crate) fn list() -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for &(group, targets) in GROUPS {
        for t in targets {
            match t.pattern {
                Pattern::Name(n) => out.push((n.to_string(), group)),
                Pattern::Pages(p, pages) => out.extend(pages.iter().map(|x| (format!("{p}{x}"), group))),
            }
        }
    }
    out.extend(EARLY.iter().chain(VARIANTS).map(|&(n, g)| (n.to_string(), g)));
    out.retain(|(n, _)| !UNLISTED.iter().any(|(u, _)| u == n));
    out
}

/// `game --shot-list`: [`list`], one `name<TAB>group` line each.
pub(crate) fn print_list() {
    for (name, group) in list() {
        println!("{name}\t{group}");
    }
}

/// The renderers rebuilt for the scene a setup left in `game.drive` (the
/// end of most driving setups).
pub(crate) fn finish_scene(game: &mut Game, gpu: &Gpu, format: wgpu::TextureFormat) {
    let d = game.sim.drive.as_mut().unwrap();
    game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
    game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each name is in the table once, and every early target, variant
    /// and unlisted entry names a real one.
    #[test]
    fn shot_table_names() {
        let mut names = std::collections::HashSet::new();
        for (_, targets) in GROUPS {
            for t in targets.iter() {
                if let Pattern::Name(n) = t.pattern {
                    assert!(names.insert(n), "{n} twice");
                }
            }
        }
        for (u, _) in UNLISTED {
            assert!(find(u).is_some(), "unlisted {u} is no target");
        }
        for (v, _) in VARIANTS {
            let base = v
                .strip_prefix("finished_career_")
                .or(v.strip_suffix("_dday"))
                .or(v.strip_suffix("_kit"))
                .unwrap_or("parts");
            assert!(find(base).is_some(), "variant {v}: {base} is no target");
        }
        let listed = list();
        let unique: std::collections::HashSet<_> = listed.iter().map(|(n, _)| n).collect();
        assert_eq!(unique.len(), listed.len(), "a target listed twice");
    }
}
