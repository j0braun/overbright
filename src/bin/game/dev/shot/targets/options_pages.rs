//! `--shot` targets: The options menu and its pages (Graphics groups, the settings page).
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("options", options),
    Target::name("options_audio", options_audio),
    Target::name("options_display", options_display),
    Target::name("options_gameplay", options_gameplay),
    Target::name("options_player", options_player),
    Target::name("customize_display", customize_display),
    Target::name("options_enhancements", options_enhancements),
    Target::name("ui2d", options_enhancements),
    Target::pages("settings_", PAGES_SETTINGS_PAGE, settings_page),
    Target::name("settings_advanced", settings_advanced),
    Target::name("settings_main_enhanced", settings_main_enhanced),
    Target::name("options_hdui", options_hdui),
    Target::name("options_graphics", options_graphics),
    Target::name("options_graphics_custom", options_graphics_custom),
    Target::pages("options_graphics_", PAGES_OPTIONS_GRAPHICS_PAGE, options_graphics_page),
    Target::name("wheel_options", wheel_options),
    Target::name("options_texture_pack", options_texture_pack),
    Target::name("options_texture_pack_ready", options_texture_pack),
    Target::name("options_player_end", options_player_end),
];

pub(super) fn options(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Options))
}

pub(super) fn options_audio(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Audio,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_display(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Display,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_gameplay(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Gameplay,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_player(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Player,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn customize_display(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::CustomDisplay,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_enhancements(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Enhancements,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// The settings page's Advanced page.
pub(super) fn settings_advanced(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Advanced,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// The settings hub in Graphics Mode Enhanced (quality Medium; the shot's
/// own settings, not saved).
pub(super) fn settings_main_enhanced(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    nfsu2::frontend::settings_page::set_mode(&mut game.settings, true);
    nfsu2::frontend::settings_page::set_level(&mut game.settings, nfsu2::qol::Level::Medium);
    options_enhancements(game, cx)
}

/// A group page of the settings page.
pub(super) fn settings_page(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen: g, .. } = cx;
    let group = settings_group(&g["settings_".len()..])?;
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Settings(group),
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// The settings page's group pages (`settings_<page>`).
pub(super) const PAGES_SETTINGS_PAGE: &[&str] =
    &["display", "image", "lighting", "weather", "performance", "gameplay", "packs"];

/// A settings group page's group.
fn settings_group(page: &str) -> Result<nfsu2::frontend::settings_model::UxGroup> {
    use nfsu2::frontend::settings_model::UxGroup;
    Ok(match page {
        "display" => UxGroup::Display,
        "image" => UxGroup::ImageQuality,
        "lighting" => UxGroup::LightingEffects,
        "weather" => UxGroup::WeatherWater,
        "performance" => UxGroup::Performance,
        "gameplay" => UxGroup::GameplayAccessibility,
        "packs" => UxGroup::Packs,
        other => anyhow::bail!("settings group {other}"),
    })
}

pub(super) fn options_hdui(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::HdUi,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_graphics(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Graphics,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// A Graphics group page: `options_graphics_image`, `_lighting`,
/// `_reflections`, `_cars`, `_effects`, `_performance`; `_lod` is
/// Effects with LOD Fade and Performance's Keep All Loaded on and
/// `_custom` the preset row reading Custom (the shot's own
/// settings, not saved).
pub(super) fn options_graphics_custom(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    nfsu2::qol::Preset::Modern.apply(&mut game.settings.enh);
    game.settings.enh.sharpen = 25;
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Graphics,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

pub(super) fn options_graphics_page(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen: g, .. } = cx;
    let group = graphics_group(&g["options_graphics_".len()..])?;
    if g.ends_with("_lod") {
        game.settings.enh.lod_fade = true;
        game.settings.enh.keep_loaded = true;
    }
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::GraphicsGroup(group),
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// The Graphics group pages (`options_graphics_<page>`).
pub(super) const PAGES_OPTIONS_GRAPHICS_PAGE: &[&str] =
    &["image", "lighting", "reflections", "cars", "lod", "effects", "performance", "display"];

/// A Graphics group page's group.
fn graphics_group(page: &str) -> Result<options::GraphicsGroup> {
    Ok(match page {
        "image" => options::GraphicsGroup::Image,
        "lighting" => options::GraphicsGroup::Lighting,
        "reflections" => options::GraphicsGroup::Reflections,
        "cars" => options::GraphicsGroup::Cars,
        "lod" | "effects" => options::GraphicsGroup::Effects,
        "performance" => options::GraphicsGroup::Performance,
        "display" => options::GraphicsGroup::Display,
        other => anyhow::bail!("graphics group {other}"),
    })
}

pub(super) fn wheel_options(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Wheel,
        &game.settings,
        FORCE_FEEDBACK,
        OPTIONS_SHOT_AT,
    ))))
}

/// The Image group with World Texture Pack selected, greyed (no
/// pack, "Not Generated"); `_ready`: a generated pack's manifest
/// (empty) in the shot's own data folder, the row enabled.
pub(super) fn options_texture_pack(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    if screen.ends_with("_ready") {
        anyhow::ensure!(
            std::env::var_os("NFSU2_SHOT_REAL_SAVE").is_none() && std::env::var_os("NFSU2_SHOT_DATA").is_none(),
            "options_texture_pack_ready writes into the shot's own data folder only"
        );
        let dir = nfsu2::upscale_pack::world_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(nfsu2::upscale_pack::MANIFEST), "")?;
    }
    let category = options::Category::GraphicsGroup(options::GraphicsGroup::Image);
    let mut o = options::OptionsScreen::new(category, &game.settings, FORCE_FEEDBACK, OPTIONS_SHOT_AT);
    let rows = options::rows(category, &game.settings, 0, FORCE_FEEDBACK);
    let at = rows.iter().position(|r| r.id == options::RowId::TexturePack).context("World Texture Pack row")?;
    anyhow::ensure!(
        rows[at].enabled == screen.ends_with("_ready"),
        "World Texture Pack row enabled {}",
        rows[at].enabled
    );
    o.selected = at;
    o.top = at.saturating_sub(7);
    Ok(Mode::Menu(Menu::Options(o)))
}

pub(super) fn options_player_end(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mut o = options::OptionsScreen::new(options::Category::Player, &game.settings, FORCE_FEEDBACK, OPTIONS_SHOT_AT);
    let mut s = game.settings.clone();
    for _ in 0..20 {
        o.key(Key::Down, &mut s, FORCE_FEEDBACK, true, OPTIONS_SHOT_AT);
    }
    Ok(Mode::Menu(Menu::Options(o)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every listed page opens a group.
    #[test]
    fn shot_pages_have_groups() {
        assert!(PAGES_SETTINGS_PAGE.iter().all(|p| settings_group(p).is_ok()));
        assert!(PAGES_OPTIONS_GRAPHICS_PAGE.iter().all(|p| graphics_group(p).is_ok()));
    }
}
