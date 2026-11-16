//! Sound round 7: the FE sound each traced screen handler plays, by
//! screen, key and state (docs/formats.md, "Sound round 7").

use nfsu2::audio::fe_rules::{self as r, MailboxState, RowClass, WorldMapState};
use nfsu2::frontend::screens::Key;

#[test]
fn screen_key_state_table() {
    let slider = |at_min, at_max| Some((RowClass::Slider { at_min, at_max }, true));
    let toggle = Some((RowClass::Toggle, true));
    let button = Some((RowClass::Button, true));
    let disabled = Some((RowClass::Toggle, false));
    let mb = |first_box, last_box, rows, row| MailboxState { first_box, last_box, rows, row };
    let wm = |event, events, first_stage, last_stage| WorldMapState { event, events, first_stage, last_stage };
    // (screen, key, state, sound id)
    let table: Vec<(&str, Key, &str, Option<u8>)> = vec![
        ("options", Key::Up, "moved or at the top", r::list_screen(Key::Up, toggle)),
        ("options", Key::Down, "any row", r::list_screen(Key::Down, toggle)),
        ("options", Key::Left, "slider moved", r::list_screen(Key::Left, slider(false, false))),
        ("options", Key::Left, "slider at min", r::list_screen(Key::Left, slider(true, false))),
        ("options", Key::Right, "slider moved", r::list_screen(Key::Right, slider(true, false))),
        ("options", Key::Right, "slider at max", r::list_screen(Key::Right, slider(false, true))),
        ("options", Key::Left, "toggle", r::list_screen(Key::Left, toggle)),
        ("options", Key::Right, "toggle", r::list_screen(Key::Right, toggle)),
        ("options", Key::Right, "button", r::list_screen(Key::Right, button)),
        ("options", Key::Right, "disabled row", r::list_screen(Key::Right, disabled)),
        ("options", Key::Accept, "button", r::list_screen(Key::Accept, button)),
        ("options", Key::Accept, "disabled row", r::list_screen(Key::Accept, disabled)),
        ("options", Key::Back, "confirmed back", r::list_screen(Key::Back, toggle)),
        ("options", Key::LTrigger, "any", r::list_screen(Key::LTrigger, toggle)),
        ("tuning", Key::Left, "any", r::tuning(Key::Left)),
        ("tuning", Key::Right, "any", r::tuning(Key::Right)),
        ("tuning", Key::Up, "any", r::tuning(Key::Up)),
        ("tuning", Key::Accept, "any", r::tuning(Key::Accept)),
        ("tuning", Key::Button1, "defaults", r::tuning(Key::Button1)),
        ("race dialog", Key::Left, "moves to Accept", r::race_dialog(Key::Left, Some(false))),
        ("race dialog", Key::Left, "already on Accept", r::race_dialog(Key::Left, Some(true))),
        ("race dialog", Key::Right, "moves to Cancel", r::race_dialog(Key::Right, Some(true))),
        ("race dialog", Key::Accept, "confirmed", r::race_dialog(Key::Accept, Some(true))),
        ("race dialog", Key::Back, "back", r::race_dialog(Key::Back, Some(true))),
        ("shop dialog", Key::Left, "already on Accept", r::shop_dialog(Key::Left, Some(true), false)),
        ("shop dialog", Key::Accept, "on Cancel", r::shop_dialog(Key::Accept, Some(false), false)),
        ("shop dialog", Key::Accept, "on Accept (enter)", r::shop_dialog(Key::Accept, Some(true), false)),
        ("shop dialog", Key::Back, "leave", r::shop_dialog(Key::Back, Some(true), false)),
        ("retry dialog", Key::Accept, "retry", r::shop_dialog(Key::Accept, Some(true), true)),
        ("retry dialog", Key::Back, "give up", r::shop_dialog(Key::Back, None, true)),
        ("event dialog", Key::Accept, "on Accept", r::event_dialog(Key::Accept, Some(true))),
        ("event dialog", Key::Accept, "on Cancel", r::event_dialog(Key::Accept, Some(false))),
        ("event dialog", Key::Right, "already on Cancel", r::event_dialog(Key::Right, Some(false))),
        ("event dialog", Key::Back, "leave", r::event_dialog(Key::Back, Some(true))),
        ("message", Key::Accept, "close", r::message_dialog(Key::Accept)),
        ("message", Key::Back, "to mailbox", r::message_dialog(Key::Back)),
        ("mailbox", Key::Right, "moved", r::mailbox(Key::Right, mb(true, false, 3, 0))),
        ("mailbox", Key::Right, "last box", r::mailbox(Key::Right, mb(false, true, 3, 0))),
        ("mailbox", Key::LTrigger, "first box", r::mailbox(Key::LTrigger, mb(true, false, 3, 0))),
        ("mailbox", Key::Up, "first row", r::mailbox(Key::Up, mb(true, false, 3, 0))),
        ("mailbox", Key::Down, "moved", r::mailbox(Key::Down, mb(true, false, 3, 0))),
        ("mailbox", Key::Down, "last row", r::mailbox(Key::Down, mb(true, false, 3, 2))),
        ("mailbox", Key::Accept, "open", r::mailbox(Key::Accept, mb(true, false, 3, 0))),
        ("mailbox", Key::Button1, "delete", r::mailbox(Key::Button1, mb(true, false, 3, 0))),
        ("mailbox", Key::Back, "leave", r::mailbox(Key::Back, mb(true, false, 3, 0))),
        ("world map", Key::Up, "first event", r::world_map(Key::Up, wm(0, 4, true, false))),
        ("world map", Key::Down, "moved", r::world_map(Key::Down, wm(0, 4, true, false))),
        ("world map", Key::Left, "first stage", r::world_map(Key::Left, wm(0, 4, true, false))),
        ("world map", Key::Right, "moved", r::world_map(Key::Right, wm(0, 4, true, false))),
        ("world map", Key::Accept, "confirmed", r::world_map(Key::Accept, wm(0, 4, true, false))),
        ("track select", Key::Right, "moved", r::track_select(Key::Right)),
        ("track select", Key::Accept, "confirmed", r::track_select(Key::Accept)),
        ("track select", Key::Back, "back", r::track_select(Key::Back)),
    ];
    let expected: [Option<u8>; 53] = [
        Some(0), Some(1), Some(8), None, Some(9), None, Some(2), Some(3), None, None, Some(4), None, Some(5), None,
        Some(9), Some(8), Some(0), None, Some(11),
        Some(2), None, Some(3), None, None,
        Some(2), Some(5), None, None, Some(5), Some(5),
        Some(4), Some(5), None, None,
        Some(5), None,
        Some(0), None, None, None, Some(1), None, None, Some(4), Some(5),
        None, Some(1), None, Some(3), Some(4),
        None, Some(4), Some(5),
    ];
    assert_eq!(table.len(), expected.len());
    for ((screen, key, state, got), want) in table.iter().zip(expected) {
        assert_eq!(*got, want, "{screen} {key:?} ({state})");
    }
    assert_eq!((r::READY_RACE, r::READY_SHOP), (15, 5));
}

/// Sound round 8: the car lots and selects, GarageMain's swing, the
/// in-game map, status, stats, the generic dialogs, UI_Paint, the parts
/// browser, the controller page, the jukebox, the showcase, the FEng
/// notification path and the screens it alone sounds.
#[test]
fn round8_screen_key_state_table() {
    use nfsu2::audio::fe_rules::{BrowserState, InGameMapState, LotState, PaintState, QuietScreen as Q};
    let lot = |first, last| LotState { first, last };
    let map = |gps, changed, focused| InGameMapState { gps, changed, focused };
    let strip = |first, last| PaintState { in_slot: false, first, last, swatches: true, finish_cycles: true };
    let slot = |swatches, finish_cycles| PaintState { in_slot: true, first: false, last: false, swatches, finish_cycles };
    let cat = |first, last, locked, empty, customize| BrowserState { in_category: false, first, last, locked, empty, customize };
    let part = |first, last| BrowserState { in_category: true, first, last, locked: false, empty: false, customize: false };
    let table: Vec<(&str, Key, &str, Option<u8>, Option<u8>)> = vec![
        ("car lot", Key::Left, "moved", r::car_lot(Key::Left, lot(false, false)), Some(2)),
        ("car lot", Key::Left, "first car", r::car_lot(Key::Left, lot(true, false)), None),
        ("car lot", Key::Right, "last car", r::car_lot(Key::Right, lot(false, true)), None),
        ("car lot", Key::Up, "any", r::car_lot(Key::Up, lot(false, false)), None),
        ("car lot", Key::Accept, "question", r::car_lot(Key::Accept, lot(false, false)), Some(4)),
        ("car lot", Key::Back, "leave question", r::car_lot(Key::Back, lot(false, false)), None),
        ("crib select", Key::Right, "moved", r::crib_select(Key::Right, lot(true, false)), Some(3)),
        ("crib select", Key::Accept, "dialog", r::crib_select(Key::Accept, lot(true, false)), None),
        ("crib select", Key::Back, "leave", r::crib_select(Key::Back, lot(true, false)), Some(5)),
        ("qr car select", Key::Left, "any", r::qr_car_select(Key::Left, true), Some(2)),
        ("qr car select", Key::Right, "not ready", r::qr_car_select(Key::Right, false), Some(3)),
        ("qr car select", Key::LTrigger, "filter", r::qr_car_select(Key::LTrigger, true), Some(0)),
        ("qr car select", Key::RTrigger, "filter", r::qr_car_select(Key::RTrigger, true), Some(1)),
        ("qr car select", Key::Accept, "ready", r::qr_car_select(Key::Accept, true), Some(4)),
        ("qr car select", Key::Accept, "not ready", r::qr_car_select(Key::Accept, false), None),
        ("qr car select", Key::Back, "not ready", r::qr_car_select(Key::Back, false), Some(5)),
        ("qr car select", Key::Up, "any", r::qr_car_select(Key::Up, true), None),
        ("garage swing", Key::Right, "last step Right", r::garage_swing(false), Some(69)),
        ("garage swing", Key::Left, "last step Left", r::garage_swing(true), Some(70)),
        ("in-game map", Key::Left, "marker changed", r::in_game_map(Key::Left, map(false, true, false)), Some(2)),
        ("in-game map", Key::Right, "no change", r::in_game_map(Key::Right, map(false, false, false)), None),
        ("in-game map", Key::Left, "gps on", r::in_game_map(Key::Left, map(true, true, false)), None),
        ("in-game map", Key::Up, "row focused", r::in_game_map(Key::Up, map(false, false, true)), Some(2)),
        ("in-game map", Key::Down, "row focused", r::in_game_map(Key::Down, map(false, false, true)), Some(3)),
        ("in-game map", Key::Down, "none focused", r::in_game_map(Key::Down, map(false, false, false)), None),
        ("in-game map", Key::LTrigger, "zoom", r::in_game_map(Key::LTrigger, map(false, false, false)), Some(2)),
        ("in-game map", Key::Button1, "gps question", r::in_game_map(Key::Button1, map(false, false, false)), Some(4)),
        ("in-game map", Key::Back, "close", r::in_game_map(Key::Back, map(false, false, false)), Some(5)),
        ("status", Key::Left, "first page", r::status(Key::Left, 0, 3), None),
        ("status", Key::Right, "moved", r::status(Key::Right, 0, 3), Some(3)),
        ("status", Key::Right, "last page", r::status(Key::Right, 2, 3), None),
        ("status", Key::Up, "any", r::status(Key::Up, 0, 3), None),
        ("status", Key::Accept, "close", r::status(Key::Accept, 0, 3), Some(4)),
        ("stats", Key::Left, "first racer", r::post_race_stats(Key::Left, 0, 4), None),
        ("stats", Key::Right, "moved", r::post_race_stats(Key::Right, 0, 4), Some(3)),
        ("stats", Key::Accept, "any", r::post_race_stats(Key::Accept, 0, 4), None),
        ("stats", Key::Back, "leave", r::post_race_stats(Key::Back, 0, 4), Some(5)),
        ("dialog", Key::Accept, "one button", r::dialog(Key::Accept, 1, true), Some(4)),
        ("dialog", Key::Left, "one button", r::dialog(Key::Left, 1, true), None),
        ("dialog", Key::Left, "two buttons", r::dialog(Key::Left, 2, true), Some(2)),
        ("dialog", Key::Right, "two buttons", r::dialog(Key::Right, 2, true), Some(3)),
        ("dialog", Key::Up, "two buttons", r::dialog(Key::Up, 2, true), None),
        ("dialog", Key::Down, "three buttons", r::dialog(Key::Down, 3, true), Some(1)),
        ("dialog", Key::Back, "back allowed", r::dialog(Key::Back, 2, true), Some(5)),
        ("dialog", Key::Back, "no back (0x558620)", r::dialog(Key::Back, 1, false), None),
        ("paint strip", Key::Left, "first slot", r::paint(Key::Left, strip(true, false)), None),
        ("paint strip", Key::Right, "moved", r::paint(Key::Right, strip(true, false)), Some(3)),
        ("paint strip", Key::Up, "any", r::paint(Key::Up, strip(true, false)), None),
        ("paint strip", Key::LTrigger, "any", r::paint(Key::LTrigger, strip(true, false)), None),
        ("paint strip", Key::Accept, "enter", r::paint(Key::Accept, strip(true, false)), Some(4)),
        ("paint swatches", Key::Up, "any", r::paint(Key::Up, slot(true, true)), Some(39)),
        ("paint swatches", Key::Down, "any", r::paint(Key::Down, slot(true, true)), Some(40)),
        ("paint swatches", Key::Left, "any", r::paint(Key::Left, slot(true, true)), Some(41)),
        ("paint swatches", Key::Right, "any", r::paint(Key::Right, slot(true, true)), Some(42)),
        ("paint swatches", Key::RTrigger, "gloss", r::paint(Key::RTrigger, slot(true, true)), Some(42)),
        ("paint swatches", Key::LTrigger, "other finish", r::paint(Key::LTrigger, slot(true, false)), None),
        ("paint swatches", Key::Accept, "no swatches", r::paint(Key::Accept, slot(false, true)), None),
        ("paint swatches", Key::Back, "any", r::paint(Key::Back, slot(true, true)), None),
        ("parts browser", Key::Left, "first category", r::parts_browser(Key::Left, cat(true, false, false, false, false)), None),
        ("parts browser", Key::Right, "moved", r::parts_browser(Key::Right, cat(true, false, false, false, false)), Some(3)),
        ("parts browser", Key::Accept, "locked", r::parts_browser(Key::Accept, cat(false, false, true, false, false)), Some(7)),
        ("parts browser", Key::Accept, "enter", r::parts_browser(Key::Accept, cat(false, false, false, false, false)), Some(4)),
        ("parts browser", Key::Back, "shop (question)", r::parts_browser(Key::Back, cat(false, false, false, false, false)), None),
        ("parts browser", Key::Back, "customize", r::parts_browser(Key::Back, cat(false, false, false, false, true)), Some(5)),
        ("parts list", Key::Up, "first part", r::parts_browser(Key::Up, part(true, false)), None),
        ("parts list", Key::Down, "moved", r::parts_browser(Key::Down, part(true, false)), Some(1)),
        ("parts list", Key::Accept, "buy question", r::parts_browser(Key::Accept, part(true, false)), Some(4)),
        ("parts list", Key::Back, "leave category", r::parts_browser(Key::Back, part(true, false)), Some(5)),
        ("controls", Key::Up, "first row", r::controls(Key::Up, 0), None),
        ("controls", Key::Up, "moved", r::controls(Key::Up, 3), Some(0)),
        ("controls", Key::Down, "any", r::controls(Key::Down, 3), Some(1)),
        ("controls", Key::Left, "any", r::controls(Key::Left, 3), Some(2)),
        ("controls", Key::Accept, "bind", r::controls(Key::Accept, 3), None),
        ("jukebox", Key::Up, "list", r::jukebox(Key::Up), None),
        ("jukebox", Key::Left, "state", r::jukebox(Key::Left), Some(2)),
        ("jukebox", Key::Right, "state", r::jukebox(Key::Right), Some(3)),
        ("jukebox", Key::RTrigger, "play mode", r::jukebox(Key::RTrigger), Some(3)),
        ("jukebox", Key::Accept, "preview", r::jukebox(Key::Accept), Some(4)),
        ("jukebox", Key::Back, "leave", r::jukebox(Key::Back), Some(5)),
        ("showcase menu", Key::Up, "top", r::showcase_menu(Key::Up, 0), None),
        ("showcase menu", Key::Down, "moved", r::showcase_menu(Key::Down, 0), Some(1)),
        ("showcase menu", Key::Down, "bottom", r::showcase_menu(Key::Down, 3), None),
        ("showcase menu", Key::Accept, "option", r::showcase_menu(Key::Accept, 2), Some(4)),
        ("magazine view", Key::Accept, "any", r::cover(Key::Accept), Some(4)),
        ("magazine view", Key::Back, "any", r::cover(Key::Back), Some(5)),
        ("magazine view", Key::LTrigger, "any", r::cover(Key::LTrigger), None),
        ("race dialog notify", Key::Accept, "on Accept", r::engage_notify(Key::Accept, Some(true), true), Some(4)),
        ("race dialog notify", Key::Accept, "none focused", r::engage_notify(Key::Accept, None, true), Some(4)),
        ("event dialog notify", Key::Accept, "none focused", r::engage_notify(Key::Accept, None, false), None),
        ("race dialog notify", Key::Back, "any", r::engage_notify(Key::Back, Some(true), true), Some(5)),
        ("race dialog notify", Key::Left, "on Cancel", r::engage_notify(Key::Left, Some(false), true), Some(2)),
        ("race dialog notify", Key::Left, "on Accept", r::engage_notify(Key::Left, Some(true), true), None),
        ("race dialog notify", Key::LTrigger, "any", r::engage_notify(Key::LTrigger, Some(true), true), None),
        ("options player notify", Key::Down, "any", r::options_player_notify(Key::Down), Some(11)),
        ("options player notify", Key::RTrigger, "not answered", r::options_player_notify(Key::RTrigger), None),
        ("reward", Key::Accept, "any", r::quiet_screen(Q::Reward, Key::Accept), Some(4)),
        ("reward", Key::Up, "filtered", r::quiet_screen(Q::Reward, Key::Up), None),
        ("results", Key::Back, "filtered", r::quiet_screen(Q::Results, Key::Back), None),
        ("results", Key::Button1, "any", r::quiet_screen(Q::Results, Key::Button1), Some(4)),
        ("mc list", Key::Down, "any", r::quiet_screen(Q::McList, Key::Down), Some(1)),
        ("mc list", Key::Left, "filtered", r::quiet_screen(Q::McList, Key::Left), None),
        ("credits", Key::Back, "any", r::quiet_screen(Q::Credits, Key::Back), Some(5)),
        ("credits", Key::Accept, "filtered", r::quiet_screen(Q::Credits, Key::Accept), None),
        ("tuning graph", Key::Button1, "any", r::quiet_screen(Q::TuneGraph, Key::Button1), Some(5)),
        ("tuning graph", Key::Accept, "filtered", r::quiet_screen(Q::TuneGraph, Key::Accept), None),
        ("splash", Key::Start, "ready", r::splash(Key::Start, true), Some(6)),
        ("splash", Key::Accept, "not ready", r::splash(Key::Accept, false), None),
    ];
    for (screen, key, state, got, want) in table {
        assert_eq!(got, want, "{screen} {key:?} ({state})");
    }
    assert_eq!(r::LOT_LEAVE_YES, 5);
}

/// Sound round 9: MC_Main and its messages, the dyno chart, the strip
/// shops (rims, ICE, neon, decals), the car-part sounds and the Options
/// Player rows (docs/formats.md, "Sound round 9").
#[test]
fn round9_screen_key_state_table() {
    use r::{McButtons as M, StripShop as S, StripState};
    let strip = |first, last, locked| StripState { in_list: false, first, last, locked, opens_list: true };
    let list = |first, last| StripState { in_list: true, first, last, locked: false, opens_list: false };
    let table: Vec<(&str, Key, &str, Option<u8>, Option<u8>)> = vec![
        ("mc main", Key::Accept, "yes / no", r::mc_main(Key::Accept, M::Two), Some(4)),
        ("mc main", Key::Left, "yes / no", r::mc_main(Key::Left, M::Two), Some(2)),
        ("mc main", Key::Right, "yes / no", r::mc_main(Key::Right, M::Two), Some(3)),
        ("mc main", Key::Back, "yes / no (ignored, still sounds)", r::mc_main(Key::Back, M::Two), Some(5)),
        ("mc main", Key::Up, "yes / no: no response", r::mc_main(Key::Up, M::Two), None),
        ("mc main", Key::Start, "yes / no", r::mc_main(Key::Start, M::Two), None),
        ("mc message", Key::Accept, "one button", r::mc_main(Key::Accept, M::One), Some(4)),
        ("mc message", Key::Back, "one button: mode 3", r::mc_main(Key::Back, M::One), None),
        ("mc message", Key::Left, "one button: mode 3", r::mc_main(Key::Left, M::One), None),
        ("mc message", Key::Down, "three buttons", r::mc_main(Key::Down, M::Three), Some(1)),
        ("mc message", Key::Left, "three buttons", r::mc_main(Key::Left, M::Three), None),
        ("mc main", Key::Accept, "buttons hidden", r::mc_main(Key::Accept, M::Hidden), None),
        ("dyno chart", Key::Accept, "running", r::dyno_chart(Key::Accept, true), None),
        ("dyno chart", Key::Accept, "done", r::dyno_chart(Key::Accept, false), Some(4)),
        ("dyno chart", Key::Back, "done: not delivered", r::dyno_chart(Key::Back, false), None),
        ("dyno chart", Key::Left, "done: filtered", r::dyno_chart(Key::Left, false), None),
        ("rims", Key::Left, "strip", r::strip_shop(S::Rims, Key::Left, strip(false, false, false)), Some(2)),
        ("rims", Key::Left, "strip, first", r::strip_shop(S::Rims, Key::Left, strip(true, false, false)), None),
        ("rims", Key::Back, "strip", r::strip_shop(S::Rims, Key::Back, strip(false, false, false)), Some(5)),
        ("rims", Key::Up, "strip", r::strip_shop(S::Rims, Key::Up, strip(false, false, false)), None),
        ("rims", Key::Left, "sizes: strip dimmed", r::strip_shop(S::Rims, Key::Left, list(false, false)), None),
        ("rims", Key::Down, "sizes", r::strip_shop(S::Rims, Key::Down, list(false, false)), Some(1)),
        ("rims", Key::Down, "sizes, last", r::strip_shop(S::Rims, Key::Down, list(false, true)), None),
        ("rims", Key::Accept, "sizes", r::strip_shop(S::Rims, Key::Accept, list(false, false)), Some(4)),
        ("rims", Key::Back, "sizes", r::strip_shop(S::Rims, Key::Back, list(false, false)), Some(5)),
        ("ice", Key::Accept, "strip, locked", r::strip_shop(S::Ice, Key::Accept, strip(false, false, true)), Some(7)),
        ("ice", Key::Accept, "strip", r::strip_shop(S::Ice, Key::Accept, strip(false, false, false)), Some(4)),
        ("ice", Key::Accept, "strip: 0x556940's own", r::strip_shop_extra(S::Ice, Key::Accept, strip(false, false, false)), Some(4)),
        ("rims", Key::Accept, "strip: no second", r::strip_shop_extra(S::Rims, Key::Accept, strip(false, false, false)), None),
        ("ice", Key::Accept, "list: the question", r::strip_shop(S::Ice, Key::Accept, list(false, false)), None),
        ("ice", Key::Back, "list", r::strip_shop(S::Ice, Key::Back, list(false, false)), Some(5)),
        ("ice", Key::Right, "list", r::strip_shop(S::Ice, Key::Right, list(false, false)), None),
        ("neon", Key::Back, "list: 0x562B20", r::strip_shop(S::Neon, Key::Back, list(false, false)), None),
        ("neon", Key::Up, "list", r::strip_shop(S::Neon, Key::Up, list(false, false)), Some(0)),
        ("decals", Key::Back, "overlay", r::strip_shop(S::Decal, Key::Back, list(false, false)), Some(5)),
        ("decals", Key::Right, "strip, last", r::strip_shop(S::Decal, Key::Right, strip(false, true, false)), None),
        ("part fe", Key::Accept, "hood open", r::part_sound_fe(0, true, false), Some(0x51)),
        ("part fe", Key::Accept, "trunk shut", r::part_sound_fe(1, false, false), Some(0x52)),
        ("part fe", Key::Accept, "door open, ANIMSTYLE 1", r::part_sound_fe(2, true, true), Some(0x4C)),
        ("part fe", Key::Accept, "door shut", r::part_sound_fe(2, false, false), Some(0x4E)),
        ("part fe", Key::Accept, "second door", r::part_sound_fe(3, true, false), None),
        ("options player notify", Key::Left, "every row answers", r::options_player_notify(Key::Left), Some(11)),
        ("showcase menu", Key::Accept, "Show Off: EngageState_Run only", r::showcase_menu(Key::Accept, 1), None),
        ("showcase menu", Key::Back, "not handled", r::showcase_menu(Key::Back, 2), None),
        ("showcase picture", Key::Accept, "save question", r::showcase_picture(Key::Accept), Some(4)),
        ("showcase picture", Key::Back, "the menu again", r::showcase_picture(Key::Back), Some(5)),
        ("showcase picture", Key::Left, "0x4B57B0 does nothing", r::showcase_picture(Key::Left), None),
    ];
    for (screen, key, state, got, want) in table {
        assert_eq!(got, want, "{screen} {key:?} ({state})");
    }
    assert_eq!([0, 1, 2, 3].map(|c| r::part_stitch(c, true)), [Some(3), Some(5), Some(1), None]);
    assert_eq!([0, 1, 2].map(|c| r::part_stitch(c, false)), [Some(2), Some(4), Some(0)]);
    assert_eq!(r::CRIB_SWING_DELAY, 0.3);
}
