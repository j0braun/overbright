use crate::*;

/// A key tapped as a player taps it: pressed (its key event and held),
/// one tick, released, another tick. The flows' one tap.
pub(crate) fn flow_tap(game: &mut Game, code: KeyCode) -> Result<()> {
    game.key(code, &mut || {})?;
    game.platform.keys.insert(code);
    game.advance_ticks(1)?;
    game.platform.keys.remove(&code);
    game.advance_ticks(1)
}

/// A saved career resumed as a player does: loaded, Career, Resume
/// Career, the crib's "Return to Explore Mode" (Yes), until free roam.
pub(crate) fn flow_resume_to_explore(game: &mut Game, name: String) -> Result<()> {
    let tap = flow_tap;
    game.mc_act(McAct::Load(name))?;
    game.menu_key(Key::Accept)?;
    game.mode = game.icon_menu(MenuKind::Main);
    tap(game, KeyCode::ArrowLeft)?;
    tap(game, KeyCode::Enter)?;
    tap(game, KeyCode::Enter)?;
    for _ in 0..120 {
        game.advance_for(1.0 / 60.0)?;
    }
    if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))) {
        tap(game, KeyCode::Enter)?;
        if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            tap(game, KeyCode::ArrowLeft)?;
            tap(game, KeyCode::Enter)?;
        }
        for _ in 0..600 {
            if matches!(game.mode, Mode::Explore) {
                break;
            }
            game.advance_for(1.0 / 60.0)?;
        }
    }
    // A later stage's unread messages open the inbox over free roam (as
    // the original's): Back.
    for _ in 0..4 {
        if !matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
            break;
        }
        tap(game, KeyCode::Escape)?;
        for _ in 0..60 {
            game.advance_for(1.0 / 60.0)?;
        }
    }
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam after the crib (mode {})", flow_mode(&game.mode));
    Ok(())
}
