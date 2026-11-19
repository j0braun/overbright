use crate::*;

/// docs/notes/outrun/pace.md: a drag rival's running shot ends the frame
/// 0x40D850 fails and the next starts when it passes, so the tank goes
/// out in several shots (native f2 rival: 24000 → 20137 → 14401 → 1465).
#[test]
fn drag_rival_shot_ends_when_the_nitrous_test_fails() {
    let mut tank = nfsu2::nitrous::Tank::new(6.0, 0);
    assert!(!drag_nitrous_end(&mut tank, false), "no shot running");
    assert!(tank.start());
    tank.advance(1.0);
    assert!(!drag_nitrous_end(&mut tank, true));
    assert!(tank.running.is_some());
    assert!(drag_nitrous_end(&mut tank, false));
    assert_eq!((tank.running, tank.left), (None, 20000));
    assert!(tank.start(), "the request starts the next shot");
}

/// The game start's PC_Loading comes down only once the start's
/// work is done and `enh.boot_loading_min_ms` has run (0: at once
/// then).
#[test]
fn boot_loading_waits_for_the_start_and_the_minimum() {
    let mut b = BootLoading::new(0.0);
    assert_eq!(b.loading.package, screens::Loading::PC);
    assert!(!b.done(0), "up until the start's work is done");
    b.ready = true;
    assert!(b.done(0), "down as soon as the start is done");
    assert!(!b.done(60_000), "held for the minimum");
    b.since -= std::time::Duration::from_millis(1200);
    assert!(b.done(1000) && !b.done(5000));
}

/// The game start follows the 200 ms
/// rule. A start done under it presents no PC_Loading frame (neither
/// as the window opens nor at the first redraw); one still running
/// past it shows the screen from its first frame; the opt-in minimum
/// shows it at once.
#[test]
fn boot_loading_follows_the_200_ms_rule() {
    // A quick start: `resumed` asks before the audio device, then the
    // start is done; `redraw` asks until `done`.
    let mut quick = BootLoading::new(0.0);
    assert!(quick.frame(10.0, 0).is_none(), "under 200 ms: no frame as the window opens");
    quick.ready = true;
    assert!(quick.done(0));
    assert!(quick.frame(20.0, 0).is_none(), "done: no frame at the first redraw");
    assert!(!quick.shown, "no PC_Loading frame presented");
    // A slow start (the window and the audio device take over 200 ms).
    let mut slow = BootLoading::new(0.0);
    slow.since -= std::time::Duration::from_millis(250);
    let l = slow.frame(250.0, 0).expect("over 200 ms: up");
    assert_eq!(l.opened, 250.0, "its animation from its first frame");
    assert!(slow.shown);
    slow.ready = true;
    assert!(slow.frame(300.0, 0).is_none(), "down once the start is done");
    // The opt-in minimum: up from the window's first frame, held.
    let mut min = BootLoading::new(0.0);
    assert!(min.frame(5.0, 1500).is_some(), "the minimum shows it at once");
    min.ready = true;
    assert!(min.frame(6.0, 1500).is_some(), "held for the minimum");
    min.since -= std::time::Duration::from_millis(1600);
    assert!(min.frame(1600.0, 1500).is_none());
}

/// The world under the in-game screens (docs/notes/front-end/fe-backdrops.md): the
/// post-race Sponsorships page (UI_Sponsorship_new) shows the race's
/// world as the popup before it and the sign question after it do
/// (the clone drew it on black); the front end's screens, with no
/// drive, and the crib's signed-sponsor list stay alone.
#[test]
fn sponsor_page_over_the_world() {
    let icons = |kind: MenuKind| Mode::Menu(Menu::Icons(IconMenu::new(kind.package(), None, Vec::new(), 0.0), kind));
    assert!(world_under(&icons(MenuKind::Sponsor), true));
    assert!(!world_under(&icons(MenuKind::Sponsor), false));
    assert!(world_under(&Mode::Menu(Menu::SponsorPopup(0.0)), true));
    assert!(!world_under(&Mode::Menu(Menu::SponsorPopup(0.0)), false));
    assert!(world_under(&icons(MenuKind::PostRace), true));
    assert!(world_under(&icons(MenuKind::ExplorePause), true));
    assert!(!world_under(&icons(MenuKind::SignedSponsors), true));
    assert!(!world_under(&icons(MenuKind::Main), true));
}

#[test]
fn finish_message_excludes_drag_dropouts() {
    let me = Progress { finished: Some(30.0), ..Progress::default() };
    let mut totaled = Progress::default();
    let mut state = CarState::new(Vec3::ZERO, 0.0);
    state.totaled = true;
    totaled.drag_drop_out(&state, 5.0);
    let blown = Progress { finished: Some(10.0), dropped: Some(8), ..Progress::default() };
    let stopped = Progress { finished: Some(12.0), ended: Some(2), ..Progress::default() };
    let running = Progress::default();
    assert_eq!(me.finish_order([&totaled, &blown, &running].into_iter()), 1);
    let winner = Progress { finished: Some(25.0), ..Progress::default() };
    assert_eq!(me.finish_order([&totaled, &winner, &stopped].into_iter()), 2);
    let later = Progress { finished: Some(31.0), ..Progress::default() };
    assert_eq!(me.finish_order([&later, &stopped].into_iter()), 1);
    assert_eq!(running.finish_order([&winner].into_iter()), 0);
}

/// 0x6076E0 / 0x606CC0: a totaled car drops out with status 9 before
/// the blown-engine test, and a done racer leaves the other cars alone
/// only in drift and Street X races or after a drop-out.
#[test]
fn drag_drop_outs_and_done_racers() {
    let mut state = CarState::new(Vec3::ZERO, 0.0);
    state.totaled = true;
    let mut p = Progress::default();
    p.drag_drop_out(&state, -0.5);
    assert_eq!(p.dropped, None);
    p.drag_drop_out(&state, 3.0);
    assert_eq!((p.dropped, p.finished), (Some(9), Some(3.0)));
    assert_eq!(crate::hud_glue::views::status_label(&p), 0xB7B7_5185);
    assert!(p.leaves_traffic(RaceMode::Drag));
    let done = Progress { finished: Some(10.0), ..Progress::default() };
    assert_eq!(crate::hud_glue::views::status_label(&done), hud::FINISHED);
    assert!(!done.leaves_traffic(RaceMode::Drag));
    assert!(!done.leaves_traffic(RaceMode::FreeRoam));
    assert!(done.leaves_traffic(RaceMode::Drift));
    assert!(done.leaves_traffic(RaceMode::StreetX));
    assert!(!Progress::default().leaves_traffic(RaceMode::StreetX));
}
