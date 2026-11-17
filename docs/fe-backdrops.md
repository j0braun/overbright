# Front-end screens over the world (2026-11-04)

Issue: the sponsor pick had a black background. The original draws the
post-race Sponsorships page (UI_Sponsorship_new) over the race's world; the clone drew it on black.

## Native behaviour
* **No per-screen gate (verified).** The frame 0x5D2860 renders each active view 1..3
  (`0x48B1E0(i)`, view +8 set → 0x48F7A0) and then Render_Frame 0x5CAAE0 (the screen passes), unless
  `g_MoviePlaying` 0x8383AC is set (0x4FE4A0). Which FE packages are on the stack does not enter.
* **The sponsor screens are packages pushed over the paused race (verified).** UI_SponsorPopup's
  Accept opens UI_Sponsorship_new; its handler 0x4C1340 signs on 0xD05FC3A3 (0x532D00) and calls
  0x4BFF80, which pushes the chain's next queued package (list +0x34/+0x38, 0x537990) or posts the
  flow event (0x5F41F0 9 / 0x15). The race world stays loaded and its view active throughout.
* **No dim (verified under Wine).** Free roam (stage4_win1.sav, :212) shot before and during the pause
  menu: every pixel outside the pause panel and the hidden HUD is identical (`pause-orig-diff-mask.png`;
  mean of the top 300x150 sky block 26.54 / 26.54). The darker look in the s4 sponsor evidence is the
  night race scene itself. None of UI_Sponsorship_new, UI_PostRaceReward, UI_PostRaceResults,
  UI_PostRace, UI_Pause, UI_SMS_Mailbox, UI_InGame_WorldMap, UI_Status_Master has a full-screen
  backdrop object (`frontend --objects`; UI_SponsorPopup's 1070x650 image is its own panel art).
* **Open:** the exact view +8 writes on entering / leaving each in-game screen were not traced; the
  rule above is from the frame function and the Wine frames.

## Clone
`world_under(mode, loaded)` in `src/bin/game.rs` is the one rule for the window (`Game::draw`) and the
shots (it replaced two copies of the same match). `MenuKind::Sponsor` with a drive loaded now draws the
world, like the popup before it and the sign question after it. No dim is applied (native). The crib's
signed-sponsor list (UI_RewardsSponsor, front end) stays as before.

## Screens checked
| Screen | Original backdrop | Clone before | Fixed | Evidence (the maintainer's private captures) |
|---|---|---|---|---|
| Sponsorships (UI_Sponsorship_new, post race) | world (s4 KENWOOD, Wine) | black | **yes** | sponsor-orig_clone-before_clone-after.jpg |
| SELECT A SPONSOR! popup | world | world | n/a | clone-reward_popup_sign_postrace.jpg; orig ../playthrough-s4-ending/s4-orig-reward-popup-sign-postrace.jpg |
| Race Reward / sign question / Post Race | world | world | n/a | same two montages |
| Event Results, race stats, tournament results / standings | world (rule; not captured this round) | world (`world_under`: Results, Stats) | n/a | — |
| Pause (free roam) | world, undimmed | world | n/a | pause-orig-free_vs_paused.jpg, pause-orig-diff-mask.png, clone-pause_sms_status.jpg |
| In-game World Map | world | world | n/a | worldmap-orig_vs_clone.jpg |
| Hidden-race dialog (SMS viewer) | world | world | n/a | ../playthrough-s4-ending/s4-hidden-dialog-orig-vs-clone.jpg |
| SMS mailbox | world (formats.md, 2026-10-29) | world | n/a | — |
| Career Statistics (UI_Status_Master) | not captured (opening it in the original was not found this round) | world | — | clone-pause_sms_status.jpg |
| DVD / magazine reward cover | black (Wine, ../magazine-covers/dvdreward_wine_before_after.png; screen context of that capture not recorded) | black | — | — |

Shots: `game --shot sponsor_world` (the post-race chain to Sponsorships, = `winchain` step 3); `winchain`
(NFSU2_CHAIN_STEP=0..5). Test: `tests::sponsor_page_over_the_world` (bin game).
