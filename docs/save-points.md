# Save points, save prompts and Autosave (2026-11-04)

Traced in SPEED2.EXE (REA / Ghidra and `$OB_PRIVATE/dis/speed2.dis`);
observed in the original under Wine (`a private Wine copy`, fastgpu, Xvfb :187,
evidence `docs/evidence/save-points/`). **Verified** = traced; **observed** =
seen under Wine; **inferred** / **open** as marked. Earlier rounds:
formats.md "Save points, records, Delete Tuned Car" and
internal notes rounds 2/3.

## Every MC_Main request (0x4A3100 call sites)

0x4A3100(caller, caller, mode, callback, …) stores the request (mode in
0x836514, low nibble = how MC_Main is shown, operation = mode & 0xF0) and
opens MC_Main.fng. Operation 0x40 / 0x50 is a save. All 25 call sites
(`grep "call   0x4a3100"`; Ghidra's caller list misses three):

| Site | Mode | Op | Who / when | Clone |
|---|---|---|---|---|
| 0x4AF0AD | 0x21961 | 0x60 | UI_StartCareer New Career: create profile | ported (boot / create flow) |
| 0x4AF103 | 0xC11 | 0x10 | UI_StartCareer: load | ported |
| 0x4AF383 | 0x151 | save | UI_CareerCrib Back "Return to Explore Mode?" Yes, changed since the crib's snapshot (0x4AF360) | ported `LotDeal::LeaveCrib` |
| 0x4AF76D | 0xC41 | save | UI_CareerCrib Save item (snapshot dropped first) | ported `Action::Save` in the crib |
| 0x4B22DD | 0x42 | save | car select Back in game mode 0x20 with the Customize latch 0x838540 | ported |
| 0x4B5A5D | 0xCB1 | 0xB0 | showcase / cover file | other job (covers) |
| 0x4B6049 | 0x2042 | save | UI_OptionsMain Back: prompts on and changed (or the prompts option changed) | ported |
| 0x4C5235, 0x4C5261 | 0x121 | 0x20 | boot splash end: create | ported |
| 0x4C580D | 0x4A51 | save | UI_ProfileManager Save | ported (fixed here) |
| 0x4C5843 | 0xC11 | 0x10 | Profile Manager Load | ported |
| 0x4C586D | 0x831 | 0x30 | Profile Manager Delete | ported |
| 0x4C589D | 0x861 | 0x60 | Profile Manager Create | ported |
| 0x4ED965, 0x4EDD97 | 0x141 | save | car lot state 6 after a car is taken, changed since the lot's snapshot | ported |
| 0x4F059D | 0x12141 | save | UI_OLEALogin (online) | not ported (online deferred) |
| 0x4F3AD4 | 0x441 | save | UI_PostRace, career, prompts on, race won: Return to Explore Mode / Quit to Main Menu | ported `post_race_save_due` |
| 0x4FCBA0 | 0x971 | 0x70 | UI_QRCarSelect | not a save |
| 0x523F59 | 0xCB1 | 0xB0 | MagazineReward_SaveCapture | ported (covers) |
| 0x552CB3 | 0x41 | save | career shops' exit to the city, changed since the shop's snapshot (0x552C90) | ported `leave_shop` |
| 0x55BF3C | 0x2042 | save | Customize Back to the Quick Race car select, changed | ported |
| 0x55E5D0 | 0x1141 | save | UI_ChooseCustomizeCategory (vtable 0x79CD6C, handler 0x55E530) Dyno Yes 0x34DC1BEC: changed since 0x552CD0's snapshot, or no profile current (0x83AA00 clear) | ported `customize_dyno_yes` |
| 0x5648EB | 0x151 | save | dyno Quit Tuning's save question Yes | ported `QuitTuningSave` |
| 0x5649E1 | 0x51 | save | dyno Save Profile Yes | ported `TuningSave` |
| 0x565C26 | 0xCC1 | 0xC0 | UI_MagazineView | other job |

No save point runs at a career stage change (no 0x4A3100 caller there):
progress made by the stage end stays in memory until the next save point.

## The state machine (verified)

* 0x4F33A0 (start): operation 0x40 → 0x4E8D30(4) with a profile, else (2)
  "create a profile?"; 0x50 → (4) too.
* 0x4E8D30: state 4 → 5 unless mode 0x4000 …; 5 → 6 with Autosave
  (0x844AD9) unless mode 0x200; 6 → 7 when mode 0x4000, or no 0x100000,
  card +0xC clear and Autosave = card +6. State 5 is "Are you sure you
  want to save^the profile named %s?" (0x4A3C10: dialog 0x2A9BA6F9, flag
  0x4000000); 6 / 7 hand the save to the memory-card library
  (0x4D9AB0 / 0x4D9B80, card +0x18 = 5 / 4), whose own dialog asks "Do
  you want to overwrite previously saved game?" (library text 6).
* Dialog results 0x4F3780: flag 0x4000000 Yes → 0x4E8D30(6), No →
  0x8867412D (MC ends, the caller goes on).
* The library's save result 0x4C63C0 (card +0x18 = 4): Autosave = 0 when
  card +0xB is clear; result 1 or 5..6 → 0x8867412D, else 0xDC12AF2E →
  0x4E8D30(2) → state 4 → 5: **the "Are you sure…" question again**.
  The library result codes themselves are not traced (they come from the
  library's thread through 0x4D9F..0x4DA0xx); the outcome per answer is
  observed below.
* MC_Main's messages (0x4FD570..0x4FD6B4): 0x8867412D → 0x4A3330, then
  (mode 0x1000 with a profile) 0x461A18EE, else 0x5347D0 and 0x4A31D0(…,
  0x8867412D). **0x5347D0 restores the profile from the snapshot**
  (0x5344E0, the payload loader, with the options re-applied) when one is
  kept, then frees it. At every save point above the snapshot is already
  gone: 0x52A4D0 (the change test) frees it before 0x4A3100, the Options
  free it right after (0x4B6010), the post-race and crib-Save sites have
  none. So a "No" never undoes the progress: it stays in memory and the
  next save point (or the Profile Manager's Save) writes it, as the clone
  does (`Profile::commit` vs `Profile::save`). The restore matters only
  for the create flow's 0x5000000 Yes (0x52A470(1)).

## Observed under Wine (internal evidence: save-points/log.txt)

Fresh profile NAME, Options Back as the save point (mode 0x2042):

1. Autosave on, the session's first autosave: "Do you want to overwrite
   previously saved game?" (No focused). **No → Autosave off** (0x844AD9
   = 0, the Save option reads Prompt) and "Are you sure you want to save
   the profile named NAME?" ([01]).
2. Yes there → the overwrite question again ([02]); No → "Are you sure…"
   again ([03]); No there → UI_Main, nothing written ([04]).
3. Autosave on again, overwrite Yes → written, no message; the next
   autosave writes unasked (as before).
4. The Profile Manager's Save with Autosave on (and the session flag
   set): "Are you sure…", Yes → the overwrite question all the same
   ([05], mode 0x4A51 has 0x4000); No → "Are you sure…" again and Autosave
   **kept** ([06]); Yes, Yes → written with **no** "Save complete." ([07]).

The post-race save point showed the same No → "Are you sure…" under Wine
before (formats.md "Screens round 3C", open (1), the finished-career save).

## Clone (src/bin/game.rs)

* `McAct::SpAsk` is the save point's "Are you sure…" (`save_point`
  uses it); the overwrite question's No is `McAct::SpNo(then, first)`:
  back to `SpAsk`; with `first` (the first autosave's question, mode
  0x100000) Autosave goes off (`settings.autosave = false`, written to
  the current profile's options) and the session flag (`autosaved`) is
  cleared. Before: the No went on (`Continue`) and kept Autosave.
* The Profile Manager's Save (`McAct::SaveAsk`): the overwrite question
  whenever the file exists, Autosave or not; its No is `McAct::SaveNo`:
  "Are you sure…" again (Autosave kept). With Autosave on the save shows
  no "Save complete." (`McAct::Save`). Before: no overwrite question with
  Autosave on, No left, the message always.
* Self-test `selftest_profiles` ("profiles: save points ok"): Prompt
  overwrite No → "Are you sure…" → No → UI_Main unsaved; first-autosave
  No → Prompt, then Yes / Yes saves with the message; the Profile
  Manager's Save under Autosave as observed.

## Customize's dyno save point

0x55E530 on 0x34DC1BEC (Dyno's "Do you want to enter the Performance Tuning
/ Dyno track?" Yes): 0x52A4D0 (compares with the snapshot 0x552CD0 took and
frees it); changed, or no profile current (0x83AA00), → 0x4A3100 mode 0x1141;
unchanged with a profile → 0x461A18EE (0x5309F0, the dyno) at once; MC's
result 0x8867412D → 0x461A18EE too (mode 0x1000). Observed under Wine
(`a private Wine copy`, NAME just made, save prompts on; evidence
internal evidence: the Customize dyno frames): Customize → Peugeot 206
(a new TUNED record) → Dyno Yes → "Are you sure you want to save the profile
named NAME?"; No → the dyno; Quit tuning, Dyno Yes again → the dyno at once.
Clone: `Game::customize_dyno_yes` compares the loaded profile with the
session's car merged into its record (`customize_merged`; the clone merges
on Back) and runs the save point on it (`AfterSave::CustomizeDyno` puts the
session's profile back); test `game.rs tests::customize_dyno_asks_to_save_only_when_changed`.

## Open

* 0x55E5D0 without a profile current: ported (L1, internal notes):
  the create question; No → "You cannot enter Performance Tuning / Dyno
  without an active profile."; Yes → name → save question → the dyno. The
  other operation-0x40 sites without a profile stay unasked (open).
* The library's result codes (0x4C63C0 / 0x4C6500) and card +6 / +8 / +9 /
  +0xB / +0xC meanings are inferred from the observed outcomes only.
* "Save complete." after a save point's Yes / Yes with Prompt comes from
  saves110's observation; with Autosave turned off by a No in the same
  flow it is assumed (not re-observed).
* 0x4F059D (online) not ported.
