use crate::*;

/// The Career Statistics key: UI_Status_Master's three pages, the career
/// page's scrolling, and back to free roam.
pub(crate) fn selftest_status(game: &mut Game) -> Result<()> {
    use nfsu2::frontend::status::{Row, VISIBLE};
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam before the status screen");
    anyhow::ensure!(game.is_action(KeyCode::KeyT, options::action::CAREER_STATS), "T opens Career Statistics");
    let entered = game.profile.as_ref().map_or(0, |p| p.races_entered);
    game.mode = game.status_screen().context("UI_Status_Master")?;
    let Mode::Menu(Menu::Status(st)) = &game.mode else { anyhow::bail!("UI_Status_Master") };
    anyhow::ensure!(
        st.stats.entered == entered && st.stats.bank == game.profile.as_ref().map_or(0, |p| p.cash),
        "bank and races entered"
    );
    anyhow::ensure!(st.stats.by_type.iter().sum::<u32>() == st.stats.wins, "wins counted by type");
    anyhow::ensure!(
        matches!(st.rows.first(), Some(Row::Header(h)) if h == "Overall Status"),
        "the career page's first row"
    );
    // The Visual Rating stars take the car's rating (0x53D880), "Total
    // Spent On Car:" the slot's spend (+0x7F0).
    anyhow::ensure!(st.screen.rating == Some(game.visual_rating()), "the stars' rating");
    let spent = game.profile.as_ref().map_or(0, |p| p.spent);
    anyhow::ensure!(
        matches!(st.rows.get(3), Some(Row::Value(_, v)) if *v == nfsu2::frontend::grouped(i64::from(spent))),
        "total spent on car"
    );
    let rows = st.rows.len();
    println!(
        "status: {} rows, wins {}, completion {:.3}, parts {:.3}, region {:.3}",
        rows, st.stats.wins, st.stats.completion, st.stats.parts, st.stats.region
    );
    for _ in 0..rows {
        game.menu_key(Key::Down)?;
    }
    let Mode::Menu(Menu::Status(st)) = &game.mode else { anyhow::bail!("still on the status screen") };
    anyhow::ensure!(st.screen.top + VISIBLE == rows, "scrolled to the last row");
    let quads = game.menu_quads(game.now());
    anyhow::ensure!(!quads.is_empty(), "the career page draws");
    for page in 1..3 {
        game.menu_key(Key::Right)?;
        let Mode::Menu(Menu::Status(st)) = &game.mode else { anyhow::bail!("page {page}") };
        anyhow::ensure!(st.screen.page == page, "page {page}");
        anyhow::ensure!(!game.menu_quads(game.now()).is_empty(), "page {page} draws");
    }
    game.menu_key(Key::Right)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Status(st)) if st.screen.page == 2),
        "no page past DVD Unlocks"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "Accept resumes free roam");
    println!("status screens: ok");
    Ok(())
}

/// The free-roam markers (docs/formats.md, "Hidden races, engage tips and
/// cash pickups"): an engage tip's XEE_GAME_TIP stands near it until its
/// SMS comes, once; a cash pickup's XEE_BANK likewise until it pays, once,
/// its flag surviving a save round trip and the HUD's bank element
/// showing it; a hidden race (event +0x7F) open but on neither map.
pub(crate) fn selftest_markers(game: &mut Game) -> Result<()> {
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && game.career_world.challenge.is_none(), "free roam, no challenge");
    let home = game.sim.drive.as_ref().context("driving")?.state.position;
    let lit = |g: &Game, name: &str| {
        let k = MARKER_EFFECTS.iter().position(|e| e.2 == name).unwrap();
        g.sim.drive.as_ref().and_then(|d| d.effects.markers[k].as_ref().map(|(at, _)| *at))
    };
    let mid = |t: &career::Trigger| {
        if t.points > 0 {
            t.polygon[..t.points as usize].iter().copied().sum::<glam::Vec2>() / t.points as f32
        } else {
            (t.min + t.max) * 0.5
        }
    };
    let go = |g: &mut Game, at: glam::Vec2| -> Result<()> {
        if let Some(d) = g.sim.drive.as_mut() {
            d.state.position = at.extend(home.z);
        }
        g.advance_for(1.0 / 60.0)
    };
    // An engage tip (kind-0x12 trigger, condition-15 message unsent).
    let t = {
        let p = game.profile.as_ref().context("a career")?;
        *game.sim.route_triggers.tips().find(|t| p.tip_pending(&game.game_data.career, t.name)).context("an unsent engage tip")?
    };
    go(game, mid(&t) + glam::Vec2::new(30.0, 0.0))?;
    anyhow::ensure!(
        lit(game, "XEE_GAME_TIP").is_some_and(|at| at.truncate() == t.position.truncate()),
        "XEE_GAME_TIP at the tip"
    );
    let count = |g: &Game, name: &str| {
        let k = MARKER_EFFECTS.iter().position(|e| e.2 == name).unwrap();
        g.sim.drive.as_ref().and_then(|d| d.effects.markers[k].as_ref().map(|(_, e)| e.len())).unwrap_or(0)
    };
    println!("XEE_GAME_TIP: {} emitters", count(game, "XEE_GAME_TIP"));
    let (cash, n) = game.profile.as_ref().map(|p| (p.cash, p.inbox.entries.len())).unwrap();
    go(game, mid(&t))?;
    go(game, mid(&t))?;
    let m = game
        .game_data.career
        .messages
        .iter()
        .find(|m| m.condition == nfsu2::frontend::sms::condition::TIP && m.value[0] == t.name)
        .context("its message")?;
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        p.inbox.contains(m.hash) && p.inbox.entries.len() == n + 1 && p.cash == cash + m.value[1],
        "the tip's SMS once and its +0x0C"
    );
    anyhow::ensure!(
        lit(game, "XEE_GAME_TIP").is_none_or(|at| at.truncate() != t.position.truncate()),
        "its effect gone"
    );
    println!("engage tip {}: once, +${}", m.name, m.value[1]);
    if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
        game.sms_close();
    }
    // A cash pickup open at this stage.
    let (t, b) = {
        let p = game.profile.as_ref().unwrap();
        game.sim.route_triggers
            .banks()
            .find_map(|t| {
                game.game_data.career
                    .banks
                    .iter()
                    .find(|b| b.trigger == t.name)
                    .filter(|b| p.bank_open(&game.game_data.career, b))
                    .map(|b| (*t, *b))
            })
            .context("an open cash pickup")?
    };
    go(game, mid(&t) + glam::Vec2::new(30.0, 0.0))?;
    anyhow::ensure!(
        lit(game, "XEE_BANK").is_some_and(|at| at.truncate() == t.position.truncate()),
        "XEE_BANK at the pickup"
    );
    println!("XEE_BANK: {} emitters", count(game, "XEE_BANK"));
    let cash = game.profile.as_ref().unwrap().cash;
    go(game, mid(&t))?;
    anyhow::ensure!(
        game.profile.as_ref().unwrap().cash == cash + b.cash as u32 && game.hud_state.hud.bank_pending() == b.cash as u32,
        "the pickup paid ${} and the HUD has it",
        b.cash
    );
    go(game, mid(&t))?;
    go(game, mid(&t) + glam::Vec2::new(30.0, 0.0))?;
    go(game, mid(&t))?;
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.cash == cash + b.cash as u32 && p.banks.contains(&b.flag), "paid once, flag {} set", b.flag);
    anyhow::ensure!(lit(game, "XEE_BANK").is_none_or(|at| at.truncate() != t.position.truncate()), "its effect gone");
    game.hud(5000.0);
    let shown = game.hud_state.hud.text_of("Cash_Bank").map(str::to_string);
    anyhow::ensure!(
        game.hud_state.hud.bank_pending() == 0
            && shown.as_deref().is_some_and(|s| s.ends_with(&nfsu2::frontend::hud::grouped_int(b.cash as i32))),
        "Cash_Bank shows it: {shown:?}"
    );
    let p = game.profile.as_ref().unwrap();
    p.save()?;
    let saved = Profile::load().context("the save")?;
    anyhow::ensure!(saved.banks.contains(&b.flag) && saved.cash == p.cash, "the flag through a save round trip");
    let mut again = saved.clone();
    anyhow::ensure!(again.collect_bank(&game.game_data.career, b.trigger).is_none(), "a loaded career does not pay it again");
    println!("cash pickup flag {}: +${} once, saved", b.flag, b.cash);
    // A hidden race: on a copy at stage 2, its open H_ events stay off the
    // minimap and the World Map while the others show.
    let backup = game.profile.clone();
    {
        let p = game.profile.as_mut().unwrap();
        p.stage = 2;
        p.intro = None;
        p.refresh_open(&game.game_data.career, &mut game.rng);
    }
    game.refresh_event_triggers();
    let open = game.profile.as_ref().unwrap().open_events(&game.game_data.career);
    let hidden: Vec<u32> = open.iter().filter(|e| e.always).map(|e| e.unlock).collect();
    let shown: Vec<u32> = open.iter().filter(|e| !e.always).map(|e| e.unlock).collect();
    anyhow::ensure!(!hidden.is_empty() && !shown.is_empty(), "stage 2 opens hidden and other races");
    let mini: Vec<u32> = game.world_entries().iter().filter_map(|e| e.3.map(|t| t.0)).collect();
    let map: Vec<u32> = game.map_icons().iter().filter_map(|i| i.target.map(|t| t.0)).collect();
    anyhow::ensure!(hidden.iter().all(|h| !mini.contains(h) && !map.contains(h)), "no hidden race on the maps");
    anyhow::ensure!(shown.iter().all(|h| mini.contains(h) && map.contains(h)), "the other open races on both");
    println!("hidden races off the maps: {} hidden, {} shown", hidden.len(), shown.len());
    game.profile = backup;
    game.refresh_event_triggers();
    if let Some(d) = game.sim.drive.as_mut() {
        d.state.position = home;
    }
    game.advance_for(1.0 / 60.0)?;
    Ok(())
}

/// SMS: the next timed message (condition 3: SMS_INSTRUCTION at 10 s of
/// free-roam driving, SMS_MINIMAP_INSTRUCTION at 40 s, …) arrives, the
/// HUD's mail flashes and then rests, the Read SMS key (Tab) opens the
/// mailbox, the message is read there, and leaving files and saves it.
pub(crate) fn selftest_sms(game: &mut Game) -> Result<()> {
    use nfsu2::frontend::sms::find;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam before the SMS test");
    anyhow::ensure!(game.is_action(KeyCode::Tab, options::action::READ_SMS), "Tab reads SMS");
    let p = game.profile.as_mut().context("a career")?;
    let (hash, due) = game
        .game_data.career
        .messages
        .iter()
        .filter(|m| m.condition == nfsu2::frontend::sms::condition::DRIVE_TIME && !p.inbox.contains(m.hash))
        .map(|m| (m.hash, m.value[0]))
        .min_by_key(|&(_, t)| t)
        .context("a timed message to come")?;
    anyhow::ensure!(p.drive_time < due as f32, "not yet due");
    p.drive_time = due as f32 - 0.1;
    let before = p.inbox.unread();
    for _ in 0..12 {
        game.advance_for(1.0 / 60.0)?;
    }
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.inbox.contains(hash) && p.inbox.unread() == before + 1, "the timed message arrived");
    // The HUD: queued, then flashing (state 2), then resting (4).
    game.hud(0.0);
    anyhow::ensure!(game.hud_state.hud.sms_state() == Some(1), "the SMS slot queued");
    // Out of the garage the car stands in its trigger: the shop's prompt
    // (priority 4) outranks the SMS slot (2), which waits queued.
    if game.career_world.marker.is_some() {
        game.hud(0.5);
        anyhow::ensure!(game.hud_state.hud.sms_state() == Some(1), "the SMS slot waits under the marker's prompt");
        game.career_world.marker = None;
    }
    game.hud(1.0);
    anyhow::ensure!(game.hud_state.hud.sms_state() == Some(2), "the mail flashing");
    game.hud(1500.0);
    game.hud(4500.0);
    anyhow::ensure!(game.hud_state.hud.sms_state() == Some(4), "the envelope resting");
    // Read SMS: one unread message opens on its own, else the Inbox.
    let mode = game.sms_open().context("the SMS screens")?;
    game.mode = mode;
    if let Mode::Menu(Menu::SmsMailbox(m)) = &game.mode {
        let p = game.profile.as_ref().unwrap();
        let row = m
            .rows
            .iter()
            .position(|&r| p.inbox.entries[r].message == hash)
            .context("the message listed in the Inbox")?;
        for _ in 0..row {
            game.menu_key(Key::Down)?;
        }
        game.menu_key(Key::Accept)?;
    }
    let Mode::Menu(Menu::SmsViewer(v)) = &game.mode else { anyhow::bail!("the message viewer") };
    let p = game.profile.as_ref().unwrap();
    let entry = p.inbox.entries[v.entry];
    let m = find(&game.game_data.career.messages, entry.message).unwrap().clone();
    anyhow::ensure!(entry.message == hash && entry.read && p.inbox.unread() == before, "the message read");
    anyhow::ensure!(v.lines.first().is_some_and(|l| !l.is_empty() && !l.ends_with("_BODY")), "its body");
    println!(
        "SMS read: {:?} from {:?}, {} lines",
        nfsu2::frontend::sms::subject_line(&game.game_data.assets, &m, Some(&entry)),
        game.game_data.assets.strings.get(m.sender),
        v.lines.len()
    );
    // Back to the mailboxes, Back again to the game: the read one filed
    // (Rachel's mailbox, category 4) and saved.
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::SmsMailbox(_))), "back to the mailbox");
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "back to free roam");
    let saved = Profile::load().context("the save")?;
    let e = saved.inbox.entries.iter().find(|e| e.message == hash).context("saved")?;
    anyhow::ensure!(
        e.read
            && e.filed
            && saved
                .inbox
                .mailbox(&game.game_data.career.messages, m.category)
                .iter()
                .any(|&i| saved.inbox.entries[i].message == hash),
        "saved read and filed in its mailbox"
    );
    if saved.inbox.unread() == 0 {
        game.hud(5000.0);
        anyhow::ensure!(game.hud_state.hud.sms_state().is_none(), "no unread: the mail goes");
    }
    println!("SMS: {} in the inbox, {} unread", saved.inbox.entries.len(), saved.inbox.unread());
    // An engage tip (condition 15, 0x5EB0F8 → 0x529C50): the car set in a
    // kind-0x12 route trigger whose message has not come; one frame later
    // it is in the inbox and its +0x0C in the bank, once.
    let tip = game.sim.route_triggers.tips().find_map(|t| {
        let m = game
            .game_data.career
            .messages
            .iter()
            .find(|m| m.condition == nfsu2::frontend::sms::condition::TIP && m.value[0] == t.name)?;
        (!game.profile.as_ref()?.inbox.contains(m.hash)).then(|| (t.min, t.max, m.hash, m.value[1]))
    });
    let (min, max, tip, pay) = tip.context("an engage-tip trigger with its message unsent")?;
    let home = game.sim.drive.as_ref().context("driving")?.state.position;
    let inside = game
        .sim.route_triggers
        .tips()
        .find(|t| t.min == min)
        .map(|t| {
            if t.points > 0 {
                t.polygon[..t.points as usize].iter().copied().sum::<glam::Vec2>() / t.points as f32
            } else {
                (t.min + t.max) * 0.5
            }
        })
        .unwrap_or((min + max) * 0.5);
    let cash = game.profile.as_ref().unwrap().cash;
    game.sim.drive.as_mut().unwrap().state.position = inside.extend(home.z);
    game.advance_for(1.0 / 60.0)?;
    game.advance_for(1.0 / 60.0)?;
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.inbox.contains(tip) && p.cash == cash + pay, "the engage tip arrived and paid {pay}");
    println!("engage tip {tip:08X}: +${pay}");
    let d = game.sim.drive.as_mut().unwrap();
    d.state.position = home;
    if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
        game.sms_close();
    }
    Ok(())
}

/// A world challenge from offer to reward (0x34A1B; 0x527980, 0x541130 →
/// 0x533BF0, 0x5016A0, 0x5150D0, 0x532B20, 0x498E70): on a copy of the
/// career moved to stage 2 with four normal wins there, STAGE_2_SHOW_1's
/// message arrives (0x533B50 → 0x5338D0), its start marker is on the
/// map; the Engage key opens UI_EngageEventDialog, Accept starts the
/// clock and offers the magazine (STAGE_2_SHOW_1_PICTURE); the clock runs
/// out, Retry puts the car back and starts it again; at the photo spot
/// the clock stops, the shoot pays and the challenge is done. The career
/// is put back afterwards.
pub(crate) fn selftest_challenge(game: &mut Game) -> Result<()> {
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && game.career_world.challenge.is_none(), "free roam, no challenge");
    let backup = game.profile.clone().context("a career")?;
    let home = game.here().context("driving")?;
    let index = game
        .game_data.career
        .mag_triggers
        .iter()
        .position(|t| t.name == nfsu2::formats::hash::bin_hash("STAGE_2_SHOW_1"))
        .context("STAGE_2_SHOW_1")?;
    let t = game.game_data.career.mag_triggers[index].clone();
    anyhow::ensure!(t.kind == 4 && t.condition == 0 && t.count == 4 && t.time == 120, "STAGE_2_SHOW_1's record: {t:?}");
    let milestone = game.game_data.career.milestones.iter().find(|m| m.hash == t.milestone).cloned().context("its milestone")?;
    {
        let data = &game.game_data.career;
        let p = game.profile.as_mut().unwrap();
        p.stage = 2;
        p.offered.remove(&milestone.hash);
        p.milestones.remove(&milestone.hash);
        p.challenges_done.clear();
        let wins: Vec<usize> = data
            .events
            .iter()
            .filter(|e| e.stage == 2 && e.category == career::Category::Normal && !e.suv)
            .take(3)
            .map(|e| e.index)
            .collect();
        p.completed.extend(wins);
        anyhow::ensure!(
            p.challenges_on_offer(data, false).iter().all(|&i| data.mag_triggers[i].stage == 2),
            "only stage 2's"
        );
        anyhow::ensure!(!p.challenges_on_offer(data, false).contains(&index), "not on offer at three wins");
        let fourth = data
            .events
            .iter()
            .find(|e| {
                e.stage == 2 && e.category == career::Category::Normal && !e.suv && !p.completed.contains(&e.index)
            })
            .context("a fourth win")?;
        p.completed.insert(fourth.index);
        // 0x533B50 on the fourth win: the message (SMS_MAG_GENERIC_*).
        p.fire_triggers(data, 0, 4);
        anyhow::ensure!(p.inbox.contains(t.message), "the challenge's message arrived");
        anyhow::ensure!(
            p.challenges_on_offer(data, false).contains(&index) && !p.offered.contains(&milestone.hash),
            "on offer, the magazine not yet"
        );
    }
    let start = game.sim.route_triggers.find(t.marker).context("the start marker's trigger")?;
    anyhow::ensure!(
        game.world_entries().iter().any(|e| e.3.is_some_and(|(h, _)| h == t.name) && e.0 == "SHOWCASEINDICATOR_"),
        "the start marker on the map"
    );
    // Into the start marker: the prompt (kind 0x12); Engage opens the dialog.
    game.sim.drive.as_mut().unwrap().state.position = ((start.min + start.max) * 0.5).extend(start.position.z);
    game.career_world.marker_trigger = None;
    game.advance_for(1.0 / 60.0)?;
    // The message is urgent: the SMS screens open on it (0x529760).
    if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
        game.sms_close();
        game.career_world.marker_trigger = None;
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(
        game.career_world.marker == Some(MarkerTarget::Challenge(index)) && game.prompt_kind(MarkerTarget::Challenge(index)) == 0x12,
        "the challenge's marker current: {:?} trigger {:?} mode menu {} at {:?}",
        game.career_world.marker,
        game.career_world.marker_trigger,
        matches!(game.mode, Mode::Menu(_)),
        start.position
    );
    anyhow::ensure!(game.engage(), "the Engage key at the challenge");
    let Mode::Menu(Menu::Challenge(d)) = &game.mode else { anyhow::bail!("UI_EngageEventDialog") };
    let texts = d.texts(&game.game_data.assets, &game.game_data.career);
    let body = texts.first().map(|t| t.1.clone()).unwrap_or_default();
    println!("challenge dialog: {texts:?}");
    anyhow::ensure!(
        body.contains(" 2:00.00") && texts.iter().any(|t| t.1.contains("Magazine Cover Opportunity")),
        "the offer's message and subject"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    let c = game.career_world.challenge.context("the challenge started")?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && c.state == 1 && c.time == 120.0, "running with 120 s: {c:?}");
    anyhow::ensure!(
        game.profile.as_ref().unwrap().offered.contains(&milestone.hash),
        "the magazine offered (0x533BF0)"
    );
    // While it runs: its goal alone on the map, the clock on the HUD.
    let entries = game.world_entries();
    anyhow::ensure!(
        entries.iter().filter(|e| e.3.is_some()).count() == 1
            && entries.iter().any(|e| e.3.is_some_and(|(h, _)| h == milestone.hash)),
        "the goal alone on the map: {:?}",
        entries.iter().map(|e| e.0).collect::<Vec<_>>()
    );
    for _ in 0..30 {
        game.advance_for(1.0 / 60.0)?;
    }
    let left = game.career_world.challenge.map_or(0.0, |c| c.time);
    anyhow::ensure!((119.0..120.0).contains(&left), "the clock runs: {left}");
    game.hud(0.0);
    anyhow::ensure!(
        game.hud_state.hud.text_of("CurrentLapTime_2").is_some_and(|s| s.contains("1:59")),
        "the HUD's challenge clock: {:?}",
        game.hud_state.hud.text_of("CurrentLapTime_2")
    );
    let drawn = nfsu2::frontend::drawn_texts(|| {
        game.hud(250.0);
    });
    anyhow::ensure!(
        drawn.iter().any(|(h, s)| *h == nfsu2::formats::hash::fe_hash("CurrentLapTime_2") && s.contains("1:59")),
        "the challenge clock emits visible glyphs after its initial fade"
    );
    // Out of time: the retry; Retry puts the car back at the start.
    game.career_world.challenge.as_mut().unwrap().time = 0.01;
    game.sim.drive.as_mut().unwrap().state.position = home.0;
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Challenge(d)) if d.kind == screens::ChallengeDialogKind::Retry)
            && game.career_world.challenge.is_some_and(|c| c.failed && c.state == 6),
        "the retry dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    let c = game.career_world.challenge.context("retried")?;
    let at = game.sim.drive.as_ref().unwrap().state.position;
    anyhow::ensure!(
        c.state == 1 && c.time == 120.0 && !c.failed && (at.truncate() - start.position.truncate()).length() < 1.0,
        "retried at the start: {c:?} at {at:?}"
    );
    // The photo spot: the clock stops (state 3); Engage takes the shot.
    let spot = game.sim.route_triggers.find(milestone.spot).context("the photo spot")?;
    game.sim.drive.as_mut().unwrap().state.position = ((spot.min + spot.max) * 0.5).extend(spot.position.z);
    game.career_world.marker_trigger = None;
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        game.career_world.challenge.is_some_and(|c| c.state == 3) && game.career_world.marker == Some(MarkerTarget::Shoot(milestone.hash)),
        "at the goal: state 3"
    );
    // The spot's first entry engages at once (0x5011C0): the milestone's
    // UI_EngageEventDialog, paid on its Continue.
    let cash = game.profile.as_ref().unwrap().cash;
    let Mode::Menu(Menu::Challenge(d)) = &game.mode else { anyhow::bail!("the milestone's dialog at once") };
    anyhow::ensure!(
        d.kind == screens::ChallengeDialogKind::Milestone { milestone: milestone.hash }
            && game.profile.as_ref().unwrap().cash == cash,
        "the milestone's dialog, nothing paid yet"
    );
    println!("milestone dialog: {:?}", d.texts(&game.game_data.assets, &game.game_data.career));
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Challenge(_))), "Back does nothing in it");
    game.menu_key(Key::Accept)?;
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.spotted.contains(&milestone.hash), "the spot's +5 bit");
    anyhow::ensure!(
        game.career_world.challenge.is_none() && p.challenges_done.contains(&t.name) && p.milestones.contains(&milestone.hash),
        "the challenge done, the cover earned"
    );
    anyhow::ensure!(p.cash >= cash + milestone.cash.max(0) as u32, "the shoot paid");
    // The HUD's cash popup (0x4AC170 → 0x4A9FF0): four updates later.
    let paid = (p.cash - cash) as i32;
    anyhow::ensure!(game.hud_state.cash_popup == Some((milestone.kind, paid, 4)), "the popup armed: {:?}", game.hud_state.cash_popup);
    let mode = std::mem::replace(&mut game.mode, Mode::Explore);
    for _ in 0..4 {
        game.hud(0.0);
    }
    game.mode = mode;
    let popup = game.hud_state.hud.text_of("Cash_Reward").map(str::to_string);
    anyhow::ensure!(
        game.hud_state.cash_popup.is_none()
            && popup.as_deref()
                == Some(&*format!("Bank Reward for Magazine: {}", nfsu2::frontend::hud::grouped_int(paid))),
        "the popup: {popup:?}"
    );
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        !p.challenges_on_offer(&game.game_data.career, false).contains(&index),
        "a done challenge is not offered again"
    );
    println!(
        "world challenge {:08X}: done, +${}, cover {:?}",
        t.name,
        p.cash - cash,
        game.game_data.assets.strings.get(nfsu2::formats::hash::bin_hash_from(milestone.cover, "_TITLE"))
    );
    // Back to the career as it was.
    game.profile = Some(backup);
    if let Some(p) = game.profile.as_ref() {
        let _ = p.save();
    }
    game.explore(Some(home))?;
    Ok(())
}

/// The unique-part reward: each visual / performance world challenge's
/// list (0x533D30); in state 4 its goal shop's front end is
/// UI_ChooseUniquePart (0x528570); Accept fits a part and ends the
/// challenge (0x561730 → 0x5419D0); Back asks, and Yes ends it with no
/// part.
pub(crate) fn selftest_unique_part(game: &mut Game) -> Result<()> {
    let backup = game.profile.clone().context("a career")?;
    let home = game.here().context("driving")?;
    let by =
        |g: &Game, name: &str| g.game_data.career.mag_triggers.iter().position(|t| t.name == nfsu2::formats::hash::bin_hash(name));
    let mut counts = Vec::new();
    for (i, t) in game.game_data.career.mag_triggers.clone().iter().enumerate().filter(|(_, t)| matches!(t.kind, 1 | 2)) {
        game.career_world.challenge = Some(WorldChallenge { index: i, time: 0.0, state: 4, failed: false, heading: 0.0 });
        let n = game.unique_reward().len();
        counts.push((
            game.game_data.assets.strings.get(t.name).map(str::to_string).unwrap_or_else(|| format!("{:08X}", t.name)),
            n,
        ));
    }
    println!("unique rewards: {counts:?}");
    for (name, slot) in [("STAGE_4_VISUAL_3", Some(0x0Cu8)), ("STAGE_3_PERF_1", None)] {
        let index = by(game, name).context("the challenge")?;
        let t = game.game_data.career.mag_triggers[index].clone();
        let shop = game.game_data.career.shops.iter().position(|s| s.hash == t.milestone).context("its goal shop")?;
        game.career_world.challenge = Some(WorldChallenge { index, time: 0.0, state: 4, failed: false, heading: 0.0 });
        game.shop.visit = Some(t.milestone);
        let items = game.unique_reward();
        anyhow::ensure!(!items.is_empty(), "{name}: a unique to choose");
        game.mode = game.shop_front_end(shop);
        let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("{name}: UI_ChooseUniquePart") };
        anyhow::ensure!(
            b.package == "UI_ChooseUniquePart" && b.categories.len() == items.len(),
            "{name}: the screen lists them"
        );
        match slot {
            Some(slot) => {
                anyhow::ensure!(
                    b.categories.iter().all(|c| c.slot == slot && c.icon == 0xBB03_4EA6),
                    "{name}: spoilers"
                );
                game.menu_key(Key::Accept)?;
                let UniqueItem::Part { hash, .. } = items[0] else { anyhow::bail!("a part") };
                let p = game.profile.as_ref().unwrap();
                anyhow::ensure!(
                    p.installed.get(&slot) == Some(&hash) && p.owned.contains(&(slot, hash)),
                    "{name}: the unique fitted"
                );
            }
            None => {
                game.menu_key(Key::Back)?;
                anyhow::ensure!(
                    matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 2),
                    "{name}: Back asks"
                );
                game.menu_key(Key::Accept)?;
            }
        }
        let p = game.profile.as_ref().unwrap();
        anyhow::ensure!(
            game.career_world.challenge.is_none() && p.challenges_done.contains(&t.name) && matches!(game.mode, Mode::Explore),
            "{name}: done, back in the city"
        );
        println!("unique part {name}: {} options, {}", items.len(), if slot.is_some() { "fitted" } else { "declined" });
    }
    game.profile = Some(backup);
    if let Some(p) = game.profile.as_ref() {
        let _ = p.save();
    }
    game.explore(Some(home))?;
    Ok(())
}

/// The world clock (0x7FB718 = sim ticks × 0.00025, 0x57A1A2, every game
/// frame) runs in free roam with traffic as without: the rain (0x6134F0)
/// and the sky cycle (0x60FAB0) read it. (The clone's once stood at 1.0
/// whenever traffic or an encounter racer was out, so it never rained.)
pub(crate) fn selftest_world_clock(game: &mut Game) -> Result<()> {
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam");
    anyhow::ensure!(game.sim.traffic.is_some(), "traffic loaded");
    let before = game.sim.drive.as_ref().context("drive")?.clock;
    for _ in 0..120 {
        game.advance_for(1.0 / 60.0)?;
    }
    let d = game.sim.drive.as_ref().context("drive")?;
    let ran = d.clock - before;
    println!(
        "world clock in free roam with traffic: {before:.2} → {:.2} s (world {:.2})",
        d.clock,
        nfsu2::weather::clock(&d.world)
    );
    anyhow::ensure!((ran - 2.0).abs() < 0.05, "the world clock runs with the game ({ran:.3} s in 2 s)");
    anyhow::ensure!((nfsu2::weather::clock(&d.world) - d.clock).abs() < 1e-3, "the world's copy follows");
    Ok(())
}

/// Career free roam's encounters and outruns (docs/formats.md, "Outrun"):
/// in stage 1 the career asks for opponents (0x513DB0) and the manager
/// places a racer on the road 250 m or more from the player (0x42A5E0); one
/// matched 20 m ahead can be challenged (0x40C030) and the key starts the
/// outrun the next frame (0x607190); more than 300 m behind the leader
/// loses (0x6076E0), and the stage's bank moves (0x541330).
pub(crate) fn selftest_outrun(game: &mut Game) -> Result<()> {
    // Wait for the manager to place a racer (one tick at a time).
    let placed = |g: &mut Game| -> Result<usize> {
        for _ in 0..60 * 45 {
            g.advance_ticks(1)?;
            if let Some(i) =
                g.sim.encounters.as_ref().filter(|e| e.state == 3).and_then(|e| e.cars.iter().position(|c| c.active))
            {
                return Ok(i);
            }
        }
        anyhow::bail!("no encounter racer placed (state {:?})", g.sim.encounters.as_ref().map(|e| (e.state, e.career)))
    };
    let i = placed(game)?;
    {
        let e = game.sim.encounters.as_ref().context("encounters")?;
        let d = game.sim.drive.as_ref().context("drive")?;
        let away = e.cars[i].state.position.truncate().distance(d.state.position.truncate());
        println!("encounter racer {} placed {away:.0} m away", e.cars[i].car.car_type.name);
        anyhow::ensure!(away >= 240.0, "placed 250 m or more from the player");
    }
    // The racer `ahead` metres in front of the player, both at `speed` on
    // the player's heading.
    let line_up = |g: &mut Game, i: usize, ahead: f32, speed: f32| {
        let d = g.sim.drive.as_mut().unwrap();
        let heading = d.state.heading();
        let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
        d.state.velocity = fwd * speed;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        d.state.publish_forward_speed();
        let at = d.state.position + fwd * ahead;
        let e = g.sim.encounters.as_mut().unwrap();
        e.cars[i].state = CarState::new(at, heading);
        e.cars[i].state.velocity = fwd * speed;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        e.cars[i].state.publish_forward_speed();
    };
    let challenge = |g: &mut Game, i: usize| -> Result<()> {
        line_up(g, i, 20.0, 20.0);
        g.advance_ticks(1)?;
        let e = g.sim.encounters.as_mut().unwrap();
        anyhow::ensure!(
            e.target == Some(i) && e.career == 2 && e.icon,
            "the matched racer can be challenged (target {:?}, career {})",
            e.target,
            e.career
        );
        e.engage();
        g.advance_ticks(1)?;
        let e = g.sim.encounters.as_ref().unwrap();
        anyhow::ensure!(
            e.state == 4 && e.outrun.as_ref().is_some_and(|o| o.opponent == i && !o.player_leads),
            "the outrun under way, the challenged car leading"
        );
        anyhow::ensure!(
            g.profile.as_ref().is_some_and(|p| p.inbox.contains(0xF833_E789)),
            "SMS_OUTRUN_INTRO on the first outrun (0x529B60)"
        );
        let _ = g.hud(0.0);
        Ok(())
    };
    challenge(game, i)?;
    let cash = game.profile.as_ref().map_or(0, |p| p.cash);
    let bank = game.game_data.career.stages.get(1).map_or(0, |s| s.outrun_bank);
    // The challenged car 400 m ahead: the player is over 300 m behind.
    line_up(game, i, 400.0, 20.0);
    game.advance_ticks(1)?;
    {
        let e = game.sim.encounters.as_ref().unwrap();
        let p = game.profile.as_ref().unwrap();
        println!("outrun lost: result {:?}, bank ${} → ${}", e.result.map(|r| (r.0, r.1)), cash, p.cash);
        anyhow::ensure!(e.outrun.is_none() && e.state == 5 && e.result.is_some_and(|r| !r.0), "the outrun lost");
        anyhow::ensure!(p.cash == cash.saturating_sub(bank.max(0) as u32), "a loss takes the stage's bank");
    }
    let _ = game.hud(0.0);
    // The manager parks the cars, goes back to NONE and tries again.
    let i = placed(game)?;
    challenge(game, i)?;
    // The player passes the challenged car on its road: within 30 m of its
    // road node, moving along it and ahead (0x5FC520 < 0) the player leads.
    let (at, dir) = {
        let e = game.sim.encounters.as_ref().unwrap();
        let (r, c, b) = e.cars[i].route.as_ref().and_then(|r| r.node()).context("the racer's road node")?;
        (e.routes.file.records[r].points[c].position, e.routes.file.direction(r, c, b))
    };
    let heading = dir.y.atan2(dir.x);
    {
        let fwd = dir.extend(0.0);
        let e = game.sim.encounters.as_mut().unwrap();
        e.cars[i].state = CarState::new(at + fwd * 2.0, heading);
        e.cars[i].state.velocity = fwd * 20.0;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        e.cars[i].state.publish_forward_speed();
        let d = game.sim.drive.as_mut().unwrap();
        d.state = CarState::new(at + fwd * 12.0, heading);
        d.state.velocity = fwd * 22.0;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        d.state.publish_forward_speed();
    }
    game.advance_ticks(1)?;
    anyhow::ensure!(
        game.sim.encounters.as_ref().unwrap().outrun.as_ref().is_some_and(|o| o.player_leads),
        "the player took the lead"
    );
    let cash = game.profile.as_ref().map_or(0, |p| p.cash);
    // The challenged car left 400 m behind: the player wins.
    line_up(game, i, -400.0, 20.0);
    game.advance_ticks(1)?;
    let e = game.sim.encounters.as_ref().unwrap();
    let p = game.profile.as_ref().unwrap();
    println!(
        "outrun won: result {:?}, bank ${} → ${}, outrun wins {:?}",
        e.result.map(|r| (r.0, r.1)),
        cash,
        p.cash,
        p.outrun_wins
    );
    anyhow::ensure!(e.result.is_some_and(|r| r.0 && r.1 == Some(i32::from(bank))), "the outrun won, its bank shown");
    anyhow::ensure!(
        p.cash == cash + bank.max(0) as u32 && p.outrun_wins.get(&1) == Some(&1),
        "a win pays the stage's bank and counts"
    );
    let _ = game.hud(0.0);
    Ok(())
}
