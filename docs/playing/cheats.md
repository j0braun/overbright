# Cheat codes

Overbright has the original PC game's cheat codes and handles them the same way.

## Using a code

1. Go to the title screen, the one that says "Press Enter Key".
2. Type the code. Case does not matter, and there is no time limit.
3. You hear the menu's select sound once when the code is taken. Nothing appears on screen.

Codes work only on the title screen. If the attract movie starts, typing stops being read, but codes you
already typed stay on. Nothing is saved: the codes stay on until you quit the game.

## The codes

| Code | What you get |
|---|---|
| `needperformance1` | Performance level 1 parts in the main menu's Customize |
| `needperformance2` | Performance level 2 parts in the main menu's Customize |
| `gimmevisual1` | Level 2 visual parts in Customize |
| `gimmevisual2` | Nothing on its own (see below) |
| `regmybank` | 200 extra in a new career's bank |
| `ordermebaby` | 1000 extra in a new career's bank |
| `regmebaby` | RX-8 and Skyline in Quick Race, and the Contest vinyls |
| 8 sponsor-car codes | A sponsor car in Quick Race |
| 5 vinyl codes | A sponsor vinyl in the shops and Customize |
| `needmybestbuy` | Only in the North American edition ([region.md](region.md)); off in the European one |

`gimmevisual2` opens nothing on its own, in the original too: the game looks up the first entry with the
same kind and value, and that is `gimmevisual1`'s.

### Bonus money and new profiles

The bank bonus reaches a profile in one of three ways, as in the original:

- **The start's question** "Would you like to create a new profile?" (when no profile exists): the profile
  keeps the career in memory. With `regmybank` and `ordermebaby` typed, it is saved with **200**.
- **Profile Manager → Create** while another profile is loaded, answered Yes: the bank is **1200** at once
  and saved that way.
- **A profile made later**, after you declined the start's question: it also keeps the bonus bank (200 in
  the example above).

## How it works

The rest of this page is for developers. The full trace of the manager, the table and the effects is in
[formats.md](../formats.md) "Cheat codes"; this page is the summary. Sponsor cars and vinyls are ported as
traced but were not examined again.

How it was checked: every code was checked again in memory under Wine (with `--wined3d`, a fresh profile,
no WidescreenFix), and the bonus codes' saves were compared with the original's. Codes were typed with
`xdotool type` after `windowfocus` (`--window` alone delivers no WM_CHAR); memory was read with `peek.py`.
DXVK once hung on the start's "Save complete." dialog (the game clock stopped), hence wined3d.

### Input (verified)

- Only on the title screen UG_LS_Splash: its constructor 0x4A90F0 turns the input of the manager 0x865930
  on (0x579CA0 at 0x4A9251), its destructor off (0x579CB0 at 0x4A92E0). In memory, +0x04 is 1 on the title
  and 0 once the attract movie takes over.
- WM_CHAR (0x5CCD7B) → 0x55DBE0 → 0x57B8C0: the byte goes into a 32-byte ring (+0x14, index count & 31)
  and the count (+0x34) steps. Every entry whose code is no longer than the count is compared with the
  ring's last characters, a–z without case. Under Wine "NeedPerformance1" and "xxneedperformance2" both
  matched.
- A match runs 0x579CC0, only for an enabled entry (+0x32). The first time it plays the FE SELECT sound
  (0x51DE70(_, 8)); under Wine, one PlayCommonSample id 4 per code as its last letter was typed, the same id
  the title's Enter plays. Then: group switch-off 0x576060 (all groups 0); active (+0x30) = 1; kind 5 →
  0x5760B0; manager +0x05 = 1; count = 0. A disabled entry leaves the count (Wine: needmybestbuy → count 13).
- Nothing is saved: 0x575FE0, called from 0x5817C4, clears only entries without the keep byte +0x31, and
  all entries have it.

### The table (0x7FB960, 20 entries of 0x34 bytes)

The table in memory under Wine equals Overbright's `CODES`.

| Code | Kind, value | Effect | Query (caller) |
|---|---|---|---|
| needperformance1 / 2 | 2, 1 / 2, 2 | performance level 1 / 2 parts in main-menu Customize | 0x579DA0 (0x529E5B, 0x52A149, 0x52A18C) |
| gimmevisual1 | 1, 2 | level-2 visual parts in Customize | 0x579D90 (0x529D8C) |
| gimmevisual2 | 1, 2 | nothing on its own: 0x576010 returns the flag of the first entry with (kind, value), gimmevisual1's (verified by trace, 0x576030–0x57604C; observed in Customize) | — |
| regmybank | 5, 2 | bonus 200 (0x865128) | 0x5760B0 |
| ordermebaby | 5, 4 | bonus 1000 (0x86512C) | 0x5760B0 |
| regmebaby | 5, 3 | RX-8 and Skyline in Quick Race (car types 0x15 / 0x1B), Contest vinyls (category 0x44) | 0x579D40 (0x529FCA), 0x579D70 (0x51537A) |
| 8 sponsor-car codes | 6, hash SPONSOR_<preset> | sponsor car in Quick Race | 0x579D80 (0x52A004) |
| 5 vinyl codes | 7, hash AD_<name> | Sponsor vinyl in shops / Customize | 0x579DB0 (0x51556B, 0x529DB7) |

needmybestbuy is disabled (+0x32 = 0 in memory) outside region 0; the region byte 0x7FA73C is 6
([region.md](region.md)). Kind 5 value 0 (0x8383C8, read by 0x527530) has no code. The callers listed are
every caller of each query (a call-site grep of the disassembly); no other code reads the manager or the
table.

### Bonus money (verified)

- 0x5760B0 sets the amount and pays it at once (0x500F80) into the career in memory (0x85AD40, bank
  0x861E74) when the career's intro has not played (0x86347C = 0) and the bank (0x500F90) is 0. Under Wine:
  regmybank → bank 200; ordermebaby next → bonus 1000 set, bank stays 200.
- The career reset 0x532840 sets the bank to the sum (0x532918 → 0x5760A0). Its callers: the start
  0x60384F (bonuses still 0), the loaders 0x5344FE / 0x534777 (then the file's bank), MC_Main's 0x4F38F1 /
  path B 0x534700 (the "Creating a new profile will …" Yes, state 1, tag 0x5000000, only with a profile
  loaded).
- So a new profile's bank depends on the question (Wine, regmybank and ordermebaby typed on the title):
  - The start's "Would you like to create a new profile?" (no profile, state 2): no reset, the profile keeps
    the career in memory and is saved with 200 (`tests/fixtures/native_saves/cheats_boot_create.sav`; the
    save files named on this page are in a private test set, not in the public repository). Start New
    Career on it went straight to the intro with 200.
  - Profile Manager → Create with NAME loaded → Yes: bank 1200 in memory at once, saved with 1200
    (`cheats_manager_create.sav`, profile TWO). The two files differ only in the bank, the name and the
    checksums.

### In Overbright

- `frontend::cheats`: the table, the ring (`Input`), the queries, `ACTIVE`, `pay_now` (0x5760B0's rule) and
  `creation_cash` (state 1 → `start_cash`, state 2 → the career in memory).
- `Game::cheat_char` (title only, from winit's key text): plays the SELECT sound; pays into the loaded
  profile or, before the start's profile step, into `Game::boot_cash` (the start's career).
- MC_Main's Done (create at the start or in the Profile Manager, Start New Career):
  `cheats::creation_cash(m.replacing, boot_cash)`. Earlier builds started every new profile with the sum
  (1200 where the original saves 200 for the start's profile); this is fixed.
- Tests: `cheats::tests` (the table against memory, the Wine typing sequence with counts and bank, each code
  alone, the ring and case, payment and new profiles), `tests/native_save.rs`
  `new_profiles_after_bonus_codes_match_the_originals` (both saves byte for byte; needs NFSU2_ROOT).
- Shots: `NFSU2_CHEATS=all|code,code`, `qrsponsor`, `vinyls_sponsor`.

### Checked in the original under Wine

Run with DXVK, `--audio-file` and a fresh profile.

- **gimmevisual2 alone opens nothing** (observed): gimmevisual2 then regmybank typed on the title (entries 3
  and 17 active, bank 200), the start's create declined, main-menu Customize → Peugeot 206 → Body Shop →
  Front Bumpers: STOCK, SNYPER, ACE, ORION. With gimmevisual1's entry (+0x30 of entry 2) then set in memory
  and the body shop entered again: STOCK, SNYPER, ACE, ROHAN, ORION, RENEGADE, STRIKE, OVERDRIVE, SWEEPER;
  the five level-2 bumpers were added. Overbright's table and query give the same
  (`cheats::tests::each_code_alone`).
- **SELECT sound** (observed): PlayCommonSample {4, 0x108B, 0x1000, 0} (id 4 = Overbright's SELECT,
  `fe::sound(SELECT, _)`) came 38 ms after gimmevisual2's last key and as regmybank's last key went in, one
  per code; the recording's peak rose from −14.1 to −12.5 dBFS in those seconds under the title music.
  Overbright plays it (`Game::cheat_char`).
- **A profile made later keeps the bonus bank** (observed): after the start's create was declined,
  Customize's Back with no profile hit its save point, which asks "Would you like to create a new profile?"
  (MC state 2); Yes, NAME, Yes → "Save complete.", bank 200 in memory (0x861E74) and in the file
  (`tests/fixtures/native_saves/cheats_later_create.sav`, `tests/native_save.rs`). In Overbright the
  Profile Manager's Create with no profile loaded gives 200 the same way (`game.rs tests::
  profile_made_after_declining_the_boot_keeps_the_bonus_bank`). Overbright's save points without a profile
  go on without the create question (`Game::save_point`), so that path itself is not offered (**open**).

## Open

- The Profile Manager's Create with no profile loaded was not run under Wine (the save-point create was).
  Both are MC state 2, so they are **inferred** to be the same.
