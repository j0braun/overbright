use crate::*;

/// A shop's first-entry movie (0x499640 → 0x4995A0 → 0x557E00): up after
/// the drive-in on IG_PlayMovie in mode 2, skipped with Accept past its
/// 0.5 s; its end opens the shop (0x499560).
pub(crate) fn skip_shop_movie(game: &mut Game, name: &str) -> Result<()> {
    let Mode::Menu(Menu::Movie(m)) = &game.mode else { anyhow::bail!("the {name} movie on entering") };
    anyhow::ensure!(
        m.mode == 2 && m.queue == [name.to_string()] && m.shop.is_some(),
        "the {name} movie queued in mode 2: {:?}",
        m.queue
    );
    game.advance_for(20.0 / 30.0)?;
    game.menu_key(Key::Accept)?;
    // The tick the movie gives way in, then the shop's first (its INIT:
    // the hint line and the save-point snapshot); the shop's screen is
    // left at that tick (one tick).
    for _ in 0..8 {
        game.advance_ticks(1)?;
        if !matches!(game.mode, Mode::Menu(Menu::Movie(_))) {
            return game.advance_ticks(1);
        }
    }
    anyhow::bail!("the {name} movie did not end after its skip")
}

/// Play a shop scene to its end (at most 10 s).
pub(crate) fn play_shop_scene(game: &mut Game) -> Result<()> {
    for _ in 0..300 {
        if game.scenes.shop.is_none() {
            return Ok(());
        }
        game.advance_for(1.0 / 30.0)?;
    }
    anyhow::bail!("the shop scene did not end")
}

/// Yes on a question: Left first when it focuses No (`ask_focus`).
pub(crate) fn answer_yes(game: &mut Game) -> Result<()> {
    if matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1 && d.buttons.len() == 2) {
        game.menu_key(Key::Left)?;
    }
    game.menu_key(Key::Accept)
}

/// The dialog or SMS on screen draws its message and every button label
/// 1.5 s after it opened, at the game's own clock (the screens' scripts
/// start from their opening: a start time taken as absolute left the
/// message and the focused button blank, "empty dialogs" in play).
pub(crate) fn check_texts_shown(game: &Game, what: &str) -> Result<()> {
    let squash = |t: &str| t.chars().filter(|c| !c.is_whitespace() && *c != '^').collect::<String>();
    let drawn: Vec<String> = nfsu2::frontend::drawn_texts(|| {
        game.menu_quads(game.now() + 1500.0);
    })
    .into_iter()
    .map(|(_, t)| squash(&t))
    .collect();
    let want: Vec<String> = match &game.mode {
        Mode::Menu(Menu::Dialog(d, _)) => std::iter::once(d.text.clone()).chain(d.buttons.iter().cloned()).collect(),
        Mode::Menu(Menu::McMain(m)) if m.step != McStep::Name => {
            let message = match (m.step, m.replacing) {
                (McStep::Confirm, _) => game.label(0x2A9B_A6F9, "?").replace("%s", &m.name),
                (_, true) => game.label(0xD366_20FE, "?"),
                (_, false) => game.label(0x02B6_41C0, "?"),
            };
            vec![message, game.label(0xF770, "Yes"), game.label(0x061C, "No")]
        }
        Mode::Menu(Menu::McMain(m)) => vec![m.name.clone()],
        Mode::Menu(Menu::SmsViewer(v)) => {
            anyhow::ensure!(v.lines.iter().any(|l| !l.trim().is_empty()), "{what}: the SMS has a body");
            v.lines.iter().skip(v.top).take(10).filter(|l| !l.trim().is_empty()).cloned().collect()
        }
        _ => anyhow::bail!("{what}: no dialog or SMS on screen"),
    };
    for w in &want {
        let w = squash(w);
        anyhow::ensure!(!w.is_empty() && w != "?", "{what}: an empty text");
        anyhow::ensure!(
            drawn.iter().any(|d| d.contains(&w) || (w.len() > 8 && d.len() > 8 && w.contains(d.as_str()))),
            "{what}: {w:?} not drawn (drawn {drawn:?})"
        );
    }
    Ok(())
}

/// Run the career's start scene (`Game::start_career_scene`) to its end.
pub(crate) fn play_career_scene(game: &mut Game) -> Result<()> {
    let mut frames = 0;
    while game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == nfsu2::shop_scene::SceneKind::Career) {
        anyhow::ensure!(frames < 1000, "the career's start scene ends");
        game.advance_for(1.0 / 30.0)?;
        frames += 1;
    }
    Ok(())
}

/// How [`describe_mode`] names a mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeText {
    /// The flow probe's log: the menu's kind or screen (`Icons(Main)`).
    Flow,
    /// The checks' messages: a race's progress, a dialog's text, results.
    Check,
}

/// The mode for a log line or a check's message (the flows' and the
/// checks' describers in one place).
pub(crate) fn describe_mode(mode: &Mode, text: ModeText) -> String {
    match text {
        ModeText::Flow => match mode {
            Mode::Explore => "Explore".into(),
            Mode::Race(_) => "Race".into(),
            Mode::Menu(m) => match m {
                Menu::Icons(_, k) => format!("Icons({k:?})"),
                Menu::McMain(m) => format!("McMain({:?})", m.step),
                Menu::McList(_) => "McList".into(),
                Menu::McMessage(..) => "McMessage".into(),
                Menu::Dialog(d, _) => {
                    format!("Dialog({:?} {:?})", d.text.chars().take(60).collect::<String>(), d.buttons)
                }
                Menu::Movie(_) => "Movie".into(),
                Menu::SmsMailbox(_) => "SmsMailbox".into(),
                Menu::SmsViewer(_) => "SmsViewer".into(),
                Menu::Splash(_) => "Splash".into(),
                _ => "Menu(other)".into(),
            },
        },
        ModeText::Check => match mode {
            Mode::Menu(Menu::Results(_, r)) => format!("results, reward {}", r.is_some()),
            Mode::Menu(Menu::Dialog(d, _)) => format!("dialog {:?}", d.text),
            Mode::Menu(_) => "menu".into(),
            Mode::Race(r) => format!(
                "race along {} finished {:?} rivals {:?} clock {}",
                r.racers.me.along,
                r.racers.me.finished,
                r.racers.rivals.iter().map(|x| (x.progress.along, x.progress.finished)).collect::<Vec<_>>(),
                r.timing.clock
            ),
            Mode::Explore => "explore".into(),
        },
    }
}

/// A short name for the mode (the flow probe's log): [`ModeText::Flow`].
pub(crate) fn flow_mode(mode: &Mode) -> String {
    describe_mode(mode, ModeText::Flow)
}

/// A paint shot's slot entered as the player enters it (Enter on the slot
/// strip: 0x55CC60 → 0x548640, the swatches opening on the car's own
/// paint, its finish taken from the part); a slot the car cannot paint
/// (its dialog) falls back to the first swatch of the slot's finish.
pub(crate) fn shot_paint_slot(game: &mut Game, slot: usize) -> Result<Mode> {
    game.mode = game.paint_shop(slot, None, None);
    game.menu_key(Key::Accept)?;
    if matches!(&game.mode, Mode::Menu(Menu::Paint(p)) if p.selected.is_some()) {
        return Ok(std::mem::replace(&mut game.mode, Mode::Explore));
    }
    let mut m = game.paint_shop(slot, None, Some(0));
    if let Mode::Menu(Menu::Paint(p)) = &mut m {
        p.selected.get_or_insert(0);
    }
    Ok(m)
}
