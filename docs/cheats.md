# Cheat codes (SPEED2.EXE, PC)

2026-10-31 (first port) and 2026-11-04 (every code
re-checked in memory under Wine, the bonus codes' saves compared, new
profiles fixed). The full trace of the manager, table and effects is
also in `docs/formats.md` "Cheat codes"; this file is the summary of
record. Sponsor cars and vinyls are ported as traced but were not
re-examined (design note).

Wine: `$OB_PRIVATE/wine/runs/<run>`, `fastgpu.sh` on :193,
`--wined3d` (DXVK hung on the boot's "Save complete." dialog once:
game clock stopped), fresh profile, no WidescreenFix. Codes typed with
`xdotool type` after `windowfocus` (`--window` alone delivers no
WM_CHAR); memory read with `peek.py`. Evidence: the maintainer's private captures
(`typing.jsonl`: manager and table after each code; `table.json`;
`pm_create_question.png`); raw captures in `<work dir>/cheats-work/wine`.

## Input (verified)
* Only on the title screen UG_LS_Splash ("Press Enter Key"): its
  constructor 0x4A90F0 turns the manager 0x865930's input on (0x579CA0 at
  0x4A9251), its destructor off (0x579CB0 at 0x4A92E0). Memory: +0x04 = 1
  on the title, 0 once the attract movie takes over (seen: the idle
  attract cut the input mid-run; codes already typed stay active).
* WM_CHAR (0x5CCD7B) → 0x55DBE0 → 0x57B8C0: the byte goes into the
  32-byte ring (+0x14, index count & 31), the count (+0x34) steps, and
  every entry whose code is no longer than the count is compared with the
  ring's last characters, a–z without case (Wine: "NeedPerformance1"
  matched; "xxneedperformance2" matched). No time limit.
* A match runs 0x579CC0: only for an enabled entry (+0x32); the first
  time plays the FE SELECT sound (0x51DE70(_, 8); observed, <work dir>: one
  PlayCommonSample id 4 per code as its last letter was typed, the id the
  title's Enter plays too); group switch-off
  0x576060 (all groups 0); active (+0x30) = 1; kind 5 → 0x5760B0; manager
  +0x05 = 1, count = 0. A disabled entry leaves the count (Wine:
  needmybestbuy → count 13). No message on screen.
* Nothing is saved: the flags and the bonus amounts live only in memory
  until the game quits (0x575FE0 from 0x5817C4 clears only entries without
  the keep byte +0x31; all have it).

## The codes (table 0x7FB960, 20 × 0x34; memory under Wine = the clone's `CODES`)
| Code | Kind, value | Effect | Query (caller) |
|---|---|---|---|
| needperformance1 / 2 | 2, 1 / 2, 2 | performance level 1 / 2 parts in main-menu Customize | 0x579DA0 (0x529E5B, 0x52A149, 0x52A18C) |
| gimmevisual1 | 1, 2 | level-2 visual parts in Customize | 0x579D90 (0x529D8C) |
| gimmevisual2 | 1, 2 | **nothing on its own**: 0x576010 returns the flag of the *first* entry with (kind, value), gimmevisual1's (verified by trace, 0x576030–0x57604C; observed in Customize) | — |
| regmybank | 5, 2 | bonus 200 (0x865128) | 0x5760B0 |
| ordermebaby | 5, 4 | bonus 1000 (0x86512C) | 0x5760B0 |
| regmebaby | 5, 3 | RX-8 and Skyline in Quick Race (car types 0x15 / 0x1B), Contest vinyls (category 0x44) | 0x579D40 (0x529FCA), 0x579D70 (0x51537A) |
| 8 sponsor-car codes | 6, hash SPONSOR_<preset> | sponsor car in Quick Race | 0x579D80 (0x52A004) |
| 5 vinyl codes | 7, hash AD_<name> | Sponsor vinyl in shops / Customize | 0x579DB0 (0x51556B, 0x529DB7) |

needmybestbuy is disabled (+0x32 = 0 in memory; region byte 0x7FA73C =
6). Kind 5 value 0 (0x8383C8, read by 0x527530) has no code. These are
every caller of each query (call-site grep of `speed2.dis`); no other
code reads the manager or the table.

## Bonus money (verified)
* 0x5760B0 sets the amount and pays it at once (0x500F80) into the career
  in memory (0x85AD40, bank 0x861E74) when its intro has not played
  (0x86347C = 0) and the bank (0x500F90) is 0. Wine: regmybank → bank
  200; ordermebaby next → bonus 1000 set, bank stays 200.
* The career reset 0x532840 sets the bank to the sum (0x532918 →
  0x5760A0). Callers: boot 0x60384F (bonuses still 0), the loaders
  0x5344FE / 0x534777 (then the file's bank), MC_Main's 0x4F38F1 / path B
  0x534700 (the "Creating a new profile will …" Yes, state 1, tag
  0x5000000, only with a profile current).
* So a new profile's bank depends on the question (Wine, regmybank +
  ordermebaby typed on the title):
  * boot's "Would you like to create a new profile?" (none current, state
    2): no reset, the profile keeps the career in memory — **saved with
    200** (`tests/fixtures/native_saves/cheats_boot_create.sav`);
    Start New Career on it ("Start New Career" that resumes) went
    straight to the intro with 200.
  * Profile Manager → Create with NAME current → Yes: bank 1200 in
    memory at once, **saved with 1200** (`cheats_manager_create.sav`,
    profile TWO). The two files differ only in the bank, name and
    checksums.

## Clone
* `frontend::cheats`: table, ring (`Input`), queries, `ACTIVE`,
  `pay_now` (0x5760B0's rule), `creation_cash` (state 1 → `start_cash`,
  state 2 → the career in memory).
* `Game::cheat_char` (title only, from winit's key text): SELECT sound;
  pays into the current profile or, before the boot's profile step,
  into `Game::boot_cash` (the boot's career).
* MC_Main's Done (boot / Profile Manager create, Start New Career):
  `cheats::creation_cash(m.replacing, boot_cash)`. **Fixed** 2026-11-04:
  every new profile used to start with the sum (1200 where the original
  saves 200 for the boot's profile).
* Tests: `cheats::tests` (table vs memory, the Wine typing sequence with
  counts and bank, each code alone, ring and case, payment and new
  profiles), `tests/native_save.rs`
  `new_profiles_after_bonus_codes_match_the_originals` (both saves byte
  for byte; needs NFSU2_ROOT).
* Shots: `NFSU2_CHEATS=all|code,code`, `qrsponsor`, `vinyls_sponsor`.

## Checked under Wine
`$OB_PRIVATE/wine/runs/<run>`, fastgpu (DXVK) on :204 with `--audio-file`,
fresh profile; evidence in the maintainer's private captures, raw in
`<work dir>/s6-work/wine/`.
* **gimmevisual2 alone opens nothing** (observed): gimmevisual2 then
  regmybank typed on the title (entries 3 and 17 active, bank 200), boot's
  create declined, main-menu Customize → Peugeot 206 → Body Shop → Front
  Bumpers: STOCK, SNYPER, ACE, ORION (`a private Wine copy.png`).
  With gimmevisual1's entry (+0x30 of entry 2) then set in memory and the
  body shop re-entered: STOCK + SNYPER, ACE, ROHAN, ORION, RENEGADE, STRIKE,
  OVERDRIVE, SWEEPER, the five level-2 bumpers added
  (`a private Wine copy.png`). The clone's table and query give
  the same (`cheats::tests::each_code_alone`).
* **SELECT sound** (observed): `sndwatch` saw PlayCommonSample {4, 0x108B,
  0x1000, 0} (id 4 = the clone's SELECT, `fe::sound(SELECT, _)`) 38 ms after
  gimmevisual2's last key and as regmybank's last key went in, one per code
  (internal evidence: the select-sound event log, typing times beside it); the WAV's
  peak rose from −14.1 to −12.5 dBFS in those seconds under the title music.
  The clone plays it (`Game::cheat_char`).
* **A profile made later keeps the bonus bank** (observed): after declining
  the boot's create, Customize's Back with no profile current hit its save
  point, which asks "Would you like to create a new profile?" (MC state 2);
  Yes, NAME, Yes → "Save complete.", bank 200 in memory (0x861E74) and in
  the file (`tests/fixtures/native_saves/cheats_later_create.sav`,
  `tests/native_save.rs`). Clone: the Profile Manager's Create with no
  profile current gives 200 the same way (`game.rs tests::
  profile_made_after_declining_the_boot_keeps_the_bonus_bank`). The clone's
  save points without a profile go on without the create question
  (`Game::save_point`), so that path itself is not offered (**open**).

## Open
* The Profile Manager's Create with no profile current was not run under
  Wine (the save-point create was); both are MC state 2 (**inferred** the
  same).
