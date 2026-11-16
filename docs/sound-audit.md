# Sound effects audit (2026-10-30, rounds 2–12 to 2026-11-01)

Every sound SPEED2.EXE plays, by the object or call site that triggers it,
against the clone. Sources: the sound-object registry (0x7ED138..0x7ED5D8,
records {name, list head, factory, id}; class = id bits 4–11), the event
name table 0x7EDD50..0x7EDE30 (`EVT_SYS/*.csi` names bound at
0x457BC1..0x458101), the 72 call sites of PlayFESound 0x458520 (and
StopFESound 0x458560), the banks under `SOUND/`, and `SDATA/sdat.viv`.
Details and addresses: docs/formats.md, "Sound effects audit" and the
Audio / Speech and music sections.

Status: **present** = the clone plays it from the traced path; **partial**
= plays, with the noted gap; **missing** = traced or listed, not played;
**open** = the trigger is not traced. "(new)" = ported in this pass.

## Front end (FE_COMMON_MB.abk ids 0–22, FE_MB.abk ids 23–93)

Every FE sound plays through its bank's event graph
(PlayCommonSample / PlayFrontEndSample) at SFXObj_FEHUD's mix-map level:
slot 0 for FE_COMMON (4235 at the default volumes), slot 1 for FE_MB
(6157), slot 6 for the turn loop (3085) — read from the original under
Wine. The rows below are about which control plays what.

| # | Sound | Trigger | Bank / id | Address | Clone |
|---|---|---|---|---|---|
| F1 | [x] Menu event sounds (up/down/left/right/select/back, triggers) | 0x50D150(event, control) → table 0x837FE0[event·27 + control] | both, by table | 0x50CF40, 0x50D150 | present (round 8: every screen the clone shows plays its handler's sounds plus FEng's sound notification (0x54D4C0 → target −5 → 0x52CDD0 → FEWidget_Handle, the screen's vtable +0xC filter; formats.md, Sound round 8): car lot / crib select / QR car select, GarageMain's swing, in-game map, status, stats, generic dialogs 0x54C060, UI_Paint, parts browser, controller page, jukebox, showcase menu, magazine view, reward / results / sponsor / dyno results / MC_List / credits / tuning graph, the splash; round 7's engage dialogs gain the notification's Accept / Back. Round 9: MC_Main and its messages (filter 0x4A33D0 by the button mode), the dyno chart (0x5103B0: silent during the run), the showcase's picture state; only the online stubs keep the control-kind rule) |
| F2 | [x] Dialog / help summon | event 1 / 2 | id 10 | 0x50D150 | present |
| F3 | [x] SMS pop-up | FOCUS on SMS_POPUP | id 13 | 0x52974B | present |
| F4 | [x] Start | START | id 6 | 0x50D150 | present (one site) |
| F5 | [x] Showcase world start / stop (Control Car, Show Off, Back) | EngageState_Run 0x529590 / _Stop 0x5295C0 (callers only in the showcase states 0x4B5A70..0x4B5E71) | table [BACK][0] (0x8380EE, id 5) | 0x5295A7, 0x5295D6 | present (new: `showcase_key` Drive / Stop) |
| F6 | [x] Help bar label / cycle / script (re-identified: the generic helpers) | FEWidget_Handle's table read, 0x51DE40(event, control) (24 callers), 0x51DE70(_, event) (column 0, 23 callers) | table | 0x51DD91, 0x51DE5F, 0x51DE8A, 0x52D697 | present (round 8: FEWidget_Handle runs on FEng's sound notification for each answered pad message; the 47 helper calls are ported with their screens; 0x52D697 is the online lobby's) |
| F7 | [x] Option rows, scroll bar, tuning curve, rating widget | table [RIGHT][0], [RIGHT][TOGGLE], [UP]/[DOWN][ctl] | table | 0x52DABB, 0x52E4DE, 0x531562, 0x5315B7, 0x53DFEC, 0x550F62, 0x550F8A | present (round 8: the jukebox's 0x53DFEC (song state, [LEFT]/[RIGHT][2]) and 0x550F62 / 0x550F8A (its reorder mode's Up / Down; the clone has no reorder mode) traced and ported; the mouse paths n/a) |
| F8 | [x] Icon scroller handler | 0x543D50: LEFT / RIGHT only off the first / last item, Up / Down ignored by horizontal strips, SELECT or WRONG on a locked item, BACK | table | 0x543FC8 | present (new; the SideScroll text list's Up / Down open) |
| F9 | [x] Shop purchase (setup purchase end) | Shop_PurchaseSetup 0x547580 ends with table [INSTALL][PAINT] | id 38 | 0x5477A9 | present (new) |
| F10 | [x] Part fitted (FE_PlayInstallSound 0x521290(part) after 0x55C240) | slot 0x10 → [INSTALL][EXHAUST] (49); slot 0x36 → id 84 then [INSTALL][0] (50); else 50 | table / id 84 | 0x5212AB; callers 0x563EE7 (custom gauges), 0x5670AC / 0x5671A9 (body shop toggle / select), 0x568E25 / 0x56901B (UI_DecalMain's handler 0x568D40, vtable 0x79BA14 slot 4), 0x56A391 (the parts list's handler 0x56A310, vtable 0x79CA60 slot 32) | present (round 8: the clone's shop screens fit through 0x521290's rule; 0x568D40 / 0x56A310 serve overlays the clone does not have — UI_CustomHUDOverlay's custom gauges are ported, the decal and neon overlays are n/a until those screens exist) |
| F11 | [x] Jukebox play mode (L/R trigger) | 0x5241F0 (round 8; 0x565DA0 is the mouse's): event +0x10C → [LTRIGGER][0] (0x8381AB), +0x110 → [RTRIGGER][0] (0x8381C6), each flips settings +0x54 then 0x524130 / 0x479220 | table | 0x565DDA, 0x565E1A, 0x524228 | present (sound4: the clone's trigger keys play these two entries, control GENERIC) |
| F12 | [x] Vinyls / rims "wrong" and browser moves | WRONG col 0, UP col 0, SELECT | table, id 5 | 0x5664F0, 0x568783, 0x56A14F, 0x56B31A, 0x56B7EF | present (round 8: 0x5664F0 WRONG on a locked parts item; 0x56A14F BuyPerformanceParts' Up / Down; 0x56B31A the jukebox's Button1 SELECT; 0x568783 / 0x56B7EF serve UI_ICEMAIN / UI_DecalMain; round 9: the rims / ICE / neon / decal handlers read key by key, `fe_rules::strip_shop`) |
| F13 | [x] Parts browser enter / leave category | direct | ids 4 / 5 | 0x55EDEC, 0x55EF9E, 0x55FCFF, 0x5605CC, 0x5614CE, 0x53A32D, 0x53A3ED, 0x55694E | present (round 8: 0x55EDEC / 0x55EF9E the parts browser's enter / leave (ids 4 / 5), 0x53A32D / 0x53A3ED the rims / carbon entries (id 4); round 9: 0x55FCFF the rims' sizes Back, 0x5605CC ICE's list Back, 0x55694E ICE's second id 4; the spinner keeps the parts browser's rule, **inferred**) |
| F14 | [x] Performance package screen | table [UP]/[SELECT]/[LEFT][PAINT_TYPE] | table | 0x55CB28, 0x55D4B7, 0x55D558, 0x55D6C1, 0x55F7AA, 0x560981 | present (round 8: these sites serve the neon / HUD overlays and UI_Paint — 0x55D4B7 / 0x55D558 are UI_Paint's mouse helper 0x55D420; UI_Paint's own 39 / 40 / 41 / 42 are ported; the neon overlay is n/a) |
| F15 | [ ] UI_VirtualKeyboard keys (handler 0x4FA5B0, vtable 0x78F8E4, factory 0x4EFD40) | Up 0, Down 1, Left 2, Right 3, Back 5, a key 46, the special keys 47 / 48, 0x5073EF13 → 8, 0xD9FEEC59 → 9 | direct | 0x4FA641..0x4FA952 | n/a (sound6: the name 0x799D54 is referenced only by its descriptor 0x7F8554 and the base ctor's compare 0x551ED4; the descriptor and the hash 0x4677789D appear nowhere in the exe: no opener) |
| F16 | [x] Transmission question answer (Auto 0x1A2826E1 / Manual 0x5F5E3886) | direct | id 86 (FE_MB 63) | 0x4EB263, 0x4EB32D | present (new) |
| F17 | [ ] Online messenger open (MenuScreen BaseNotify 0x55B5D0, Pad_Button1 while logged in, 0x8500D0 > 0xB) | direct | id 74 (FE_MB 51) | 0x55B911 | n/a (online only) |
| F18 | [x] Mouse click on a help-bar slot with mask bit 2 (0x55DE10, the FE mouse walk; bit 2 is Logoff, online only) | direct | id 75 (FE_MB 52) | 0x55DEAA | present (<work dir>: the clone's menu mouse plays 0x4B on a click on a Logoff slot, `Game::fe_mouse`; the slot only shows online, which the clone does not have, so it is unreachable in play; the other mouse paths play the keys' sounds, which they share: the help bar's messages, the slider arrows' table [type][4 / 5] **inferred** equal to the keys' 0x5650D0 ids, the tuning Tip 0x51DE70(_, 2)) |
| F19 | [x] Garage hydraulics moves (0x4A6030, parts slot 0xA1 mode 1–3: a corner rising 0x59 (mode 1, even corners) / 0x5B (mode 2, odd), falling 0x5A / 0x5D, back at rest after rising above the follow height 0x5C (corner 0) / 0x58 (corner 2)) | direct | ids 88–93 | 0x4A652E, 0x4A6541, 0x4A6614 | present (hydro: `hydraulics::Garage` → PlayFESound; the landing ids never fire in modes 1–2, whose corners stay under the follow height) |
| F20 | [x] FE car turn loop | FEGarage_UpdateCarTurn: angle +0x274 ≠ last +0x170 → play, still → StopFESound; the player's update 0x4793D0 stops it 100 ms later, level slot 6, param 3000 + 500 · mean turn rate | id 73 (FE_MB loop 50, sample looped 0..112499) | 0x4D8C15, 0x4D8C4E, 0x4793D0 | present (sound5: through PlayFrontEndSample_loop's graph) |
| F21 | [x] FEng update select | table [SELECT][0] | table | 0x5593DE, 0x559C2D | present (round 8: both sites play [SELECT][0] = id 4, which the parts browser's Accept plays on every non-locked item) |
| F22 | [ ] FEDriveOn | event name only (0x7EDDA0 bound at 0x457BF2) | FE bank | — | n/a (sound6: its slot 0x82C520 is referenced only by the binding 0x457BF7 and the destructor 0x77C750: never played) |

## HUD and in-game front end

| # | Sound | Trigger | Bank / id | Address | Clone |
|---|---|---|---|---|---|
| H1 | [x] GPS route found | HUD_MinimapGPSText: GPS state turns 3 (edge on element +0x35) | id 18 | 0x4C85A4 | present (new) |
| H2 | [x] Cash pickup | 0x4AC1B0 on a bank trigger | id 17 | 0x4AC1B0 | present |
| H3 | [ ] Countdown beeps, wrong way, race over, lap | — | — | 0x4AB5B0, 0x4AAFC0, 0x4C7F00 call no sound | n/a (no sound in the HUD code); the drift element's beep 0x4EBAC0(0x862A0519) is open |
| H5 | [x] Engage prompt appears (a marker's trigger entered: event, shop, photo shoot, world challenge; the outrun offer's encounter icon) | the prompt's slot starts flashing: 0x4DA9A0's pending pick or a takeover, 0x4C9170 from rest → 0x51DE40(FOCUS, FE_SOUND_ENGAGE_PHONE 0xD) | id 14 (FE_COMMON) | 0x4DAAB0, 0x4DAB49, 0x4C9208 | present (new, markers162: `hud::Prompts`; the SMS slot and kind 0x14 excluded, below) |
| H6 | [x] Engage prompt settles (1 s after it appeared, any kind but 0x14) | 0x4C8AC0 state 2 → 3 → 0x4AB9F0(3) → 0x51DE40(FOCUS, FE_SOUND_ENGAGE_DPAD 0xE); state 3 → 4 (0x4AB9F0(4)) plays nothing, and a marker's prompt never rests (0x4C8B80 sets 4 back to 3) | id 15 | 0x4ABA1B | present (new) |
| H7 | [x] New SMS (any message: engage tip, timed, win, Rachel's) | the SMS slot (kind 0x13) flashes: queued and picked, or from rest by 0x4C9170 → 0x51DE40(FOCUS, FE_SOUND_ENGAGE_SMS 0xF); then H6's id 15 a second later; held at state 2 while the D-Day script runs (0x502420) | id 16 | 0x4ABA01, 0x4C9204, 0x4DAAAA | present (new; under Wine: unread 6 → 7 at an engage tip, slot 4 → 2, 3 at +1.02 s, 4 at +4 s; sndwatch: ids 16 then 15) |
| H4 | [x] FX_Camera (the showcase's camera shutter) | ShowcaseCamera_Shutter 0x501250 (Take Picture, 0x4CF592) → 0x458880 sets manager +0x8 → 0x45E450 starts FX_Camera, block 0 | FX_Misc_MB event FX_Camera (slot 0x82C72C, record 0x7EDDE8) | 0x45E450, 0x501250 | present (new: `Scrapes::shutter`) |

## Player car

| # | Sound | Object | Bank | Address | Clone |
|---|---|---|---|---|---|
| C1 | [x] Engine (granular accel / decel GIN) | CARSFX_PlayerEngine + HybridMotor | `GIN_*.gin` | 0x473D30, 0x474030 | present |
| C2 | [x] Engine abk layers + rev limiter | event "CAR" | `CAR_xx_ENG_MB_EE.abk` | 0x47D920 | present |
| C3 | [x] Shift sweetener | CAR_SWTN | `SWTN_CAR_xx_MB.abk` | 0x4720D0, 0x4722B0 | present |
| C4 | [x] Decel crackle | CARSFX_SparkChatter, CAR_Sputter | SWTN banks | 0x472C10 | present |
| C5 | [x] Gear shifts | CARSFX_Shift, FX_SHIFTING_01 | `GEAR_*` | 0x471ED0 | present |
| C6 | [x] Turbo spool / blow-off | CARSFX_Turbo, FX_TURBO_01 | `TURBO_*` | 0x472480 | present |
| C7 | [x] Nitrous + purge | CARSFX_Nitrous, FX_NITROUS / FX_PURGE | `Nitrous_0x_MB` | 0x4729D0 | present |
| C8 | [x] Skids | CARSFX_Skids, FX_SKID | `SKID_*`, `SKIDS_DRIFT*` | 0x473A70 | present |
| C9 | [x] Road noise | CARSFX_RoadNoise | `ROADNOISE_00_MB` | 0x472E50 | present |
| C10 | [x] Wind, wind in weather | CARSFX_WindNoise / WindWeather | `WIND_0x_MB` | 0x47D430, 0x47D500 | present |
| C11 | [x] Bottom-out knocks | CARSFX_BottomOut | stitch whoosh / collision | 0x47ED80 | present |
| C12 | [x] Collisions | SFXObj_Collision, stitch records; the track sample 0x47A770 picks its level at volume + 60 (≤ 127) when its car is a player's under the crash camera (player +0x58 = 6) | `MB_Stich_Collision` | 0x479AD0, 0x47A770 | present (2026-11-01: the crash camera's +60, `Impact::crash_camera`) |
| C13 | [x] Scrapes | FX_Scrape | `FX_Misc_MB` | 0x46A400 | present (silent as coded) |
| C14 | [x] Drive-by whooshes (tunnel / zone entry and exit) | CARSFX_DriveBy + SFXCTL_Tunnel | whoosh stitch records 45–47, 54–56 | 0x474A20, 0x47E0D0, 0x471080, 0x483250 | present (new) |
| C15 | [x] Hydraulics | CARSFX_Hydraulics (vtable 0x78AF88, update 0x47DD40, levels 0x4615E0): one corner change a frame, rising → FX_Hydraulic kind 1, falling → kind 2, at rest / up → its voice freed; any kick (car +0x408) with no bounce voice → FX_Hydr_Bounce; volumes = the mix map's class 8 slot 0 each frame | `Hydraulics_00_MB` | 0x47DAE0, 0x47DC10, 0x47DED7, 0x4615E0 | present (hydro: `audio::hydraulics`; player car only, mask 0x5FFF) |
| C16 | [ ] Horn | — | — | — | n/a: no player horn object exists |
| C17 | [x] Traffic car stereo (bass loops) | CARSFX_TrafficStereo 0x30030 (traffic mask 0x1F): attach within 70 m (0x463E30 → 0x469B20 → 0x476580) rolls Rand_Int(3) = 0, never on TAXI / TAXI02; variant 8 + Rand_Int(2); start / stop on mix slot 0 (0x4635B0), params (0x4636E0) | `ENV_COMMON_MB` ENV_STATIC 8 / 9 (sounds 7 / 8, looped) | 0x47F4E0, 0x476580, 0x4635B0, 0x4636E0, 0x463540 | present (round 12: on the six traffic contexts, see W2) |
| C18 | [x] AI racer stereo | CARSFX_Stereo 0x200F0 (AI mask 0x802D): World_Start 0x60A690 → 0x458850 (0x60A7B3) → the managers' slot 5 0x463BB0 → each AI context's slot 4 0x469CD0 → 0x464BB0 → slot 7 0x4765F0: variant 8 + Rand_Int(2), a racer steps 0x82CB6C (session, mod 6) and plays at 0 (every sixth racer) | `ENV_COMMON_MB` ENV_STATIC 8 / 9 | 0x47F480, 0x4765F0, 0x469CD0, 0x463BB0, 0x458850 | present (new, round 12: `Stereo::racer_roll`, section 2 class 15 slot 0; the clone's racer order **inferred**) |
| C19 | [ ] FX_SPARKCHATTER | event bound (slot 0x82C5C8); no start site among the 41 of 0x7358C0; `Sparkchatter_00/01.abk` names (0x7ED198 / 0x7ED19C) unreferenced | `ENGINE/Sparkchatter_0x.abk` | — | n/a (never played; the crackle is CAR_Sputter) |

## World

| # | Sound | Object | Bank | Address | Clone |
|---|---|---|---|---|---|
| W1 | [x] AI racers' engines | CARSFX_AIEngine | `CAR_xx_ENG_MB_SPU` | 0x473CE0 | present |
| W2 | [x] Traffic engines | CARSFX_TrafficEngine, FX_TRAFFIC: started at the context's attach (0x4621C0), stopped at its release (0x475790); six contexts (0x476AF0), each given to the first traffic car within 70 m without one, in slot order, released at 70 m (0x463E30, 0x464350; no priority or stealing) | `TRAFFIC_MB` | 0x47E5B0, 0x463E30, 0x464350, 0x469B20, 0x469B90 | present (round 12: only on the six attached contexts; before, every traffic car) |
| W3 | [x] Traffic whooshes | CARSFX_TrafficWoosh (needs context +0x3C; release 0x462650) | stitch whoosh | 0x475E10 | present (round 12: attached contexts only) |
| W4 | [x] Traffic horns | CARSFX_TrafficHorn: oncoming car within 20 m, player > 15 m/s, 30 % or 5 s since the last; ENV_STATIC variants 5–7 for 1.2–4 s, 0.15 s fade; release 0x4624E0 | `ENV_COMMON_MB` | 0x47E6E0, 0x475B30, 0x4759C0, 0x462550 | present (round 12: +0x3C is the context's attach, traced; attached contexts only; mix-map field 1 open) |
| W5 | [x] Traffic bottom-out | CARSFX_TrafficBottomOut (needs the context's car; release 0x462700) | stitch | 0x47F2B0 | present (round 7: the volume source traced — car +0x28 is the mover, 2 / 3 for traffic, so 127; round 12: attached contexts only) |
| W6 | [x] Rain | CARSFX_Rain 0x474770: FX_Weather above rain 0.001, stopped with a 10 s release, level slot 0, thunder timer (rain > 0.25, every Rand_Int(20) + 25 s) → THUNDERSTRM | `FX_Rain_MB`; `nisaudio` 0x400E | 0x47DFE0, 0x4617F6, 0x4852A0 | present (the inside-view variant open; the thunder line plays only below rain 0.5, no record fits variant 2) |
| W7 | [x] Planes, trains, fountains | CSTATE_Plane / _Train / _Fountain managers (0x464410, 0x464600, 0x4647E0) on the world animations' entities (0x439F40) and the fixed fountain points; SFXObj_Plane / _Train / _Fountain | `ENV_COMMON_MB` ENV_STATIC 1 / 0 / 4, `FX_MAIN_MEM_MB` (planes in URL races) | 0x462810, 0x462B70, 0x463020, 0x4631E0, 0x463820, 0x463920 | present (sound6, `audio::enviro`; mix sections 4 / 7 / 8) |
| W8 | [x] Reverb zones | SFXObj_Reverb (preset 8 Hills at the start, 0 / 3 in the front end), SFXCTL_Tunnel's zone kinds → presets 5 / 6 / 3, the "Reverb" parameter map's region preset and offset (0x47C6D0), fades 0x4714A0 | `FXEDIT/*.fx` (12) | 0x4632C0, 0x4633F0, 0x45DB90, 0x4714A0, 0x47C6D0, 0x726DB3, 0x72655A | present (sound7: the ten module kinds' DSP ported (`audio::reverb`), the engine voices' sends and dry bytes from wet A / dry A, the graph per ≤ 256-frame chunk with the 3000-chunk idle stop; sound10: the 16-sample send / gain ramps, the voice-stop fades, the engine ABK voices' wet-B sends; the music never sends (no `.mpf` action 7 / 16); no capture of the original's output to compare) |
| W9 | [x] Ambience | SFXObj_Ambience (id 0x20: section 0 class 2; factory 0x47E410 → 0x4750D0, vtable 0x78B158; slot 7 0x475100 makes it the music object's +0xD0): the interactive score's level object | — | 0x4750D0, 0x475100 | present (round 7: SFXObj_Ambience has no update of its own — vtable 0x78B158 slots 8–13 empty; it carries the score's level and fade, ported in S7) |
| W10 | [x] Water drum splash | the props' sound object's start (vtable 0x78BB08 slot 8, 0x46AE00): WATER_DRUM (material 5), the prop knocked loose (+0x505, set once by 0x5ADFA0) and +0x50C clear → three FX_ROADNOISE_TRANS {3, 0, 0, 0, 1}; frame 0x481FD0: p1 = +0xC0 · 17000 (0x7ECF90) >> 15, p2 = 2000 / 4000 / 7000, p3 = +0xC8 | `ROADNOISE_00_MB` FX_ROADNOISE_TRANS variant 3 = sample 14 (1.99 s) | 0x46AE00, 0x481FD0, 0x5ADFA0 | present (new, round 12: `EnvSfx::splash`; +0xC0 as the impact's volume, **inferred** as for the impacts; the +0x128 bit 0x10 ramp open) |
| W11 | [ ] Sirens | `MW_SIREN_AEMS.csi` (event SIREN) and `MW_MAIN_AEMS.csi` | no bank | — | n/a (not in the csi list 0x7ED148..0x7ED15C: never loaded) |

**Free-roam markers, no sound of their own** (markers162, searched in the
code): the Engage key (0x602BE0, code 0xBF), the engage state's handler
(0x606740) and the dialog opener (0x498D00) call no FE sound; the dialog's
own keys play through F1. The marker effects (0x605790) and the effect
system they drive (0x57xxxx) call no sound code: no approach or in-range
sound. An engage tip (0x532D50 → 0x529C50) pays its cash silently; only the
SMS slot's H7 / H6 play. A cash pickup plays H2 (id 17). A photo shoot's or
world challenge's marker plays H5 / H6 like any marker, then its dialog.
Nothing plays when a prompt ends (state 6) or on leaving a trigger.

## Speech, scenes and music

| # | Sound | Trigger | Data | Address | Clone |
|---|---|---|---|---|---|
| S1 | [x] Career start scene line | NisAudio_Request kind 9 | `nisaudio` 0x4024 | 0x484AF4 | present |
| S2 | [x] Race start scene line | NisAudio_Request kind 2 from the NIS load | `nisaudio` event 0x4000 / entry 0x4023 | 0x435CE8, 0x484A60 | present (new; tuple traced, third key 1 observed under Wine for a circuit) |
| S3 | [x] Ending, dyno and shop scene lines | NIS load 0x435BE0 → 0x484510 kind 2 → NIS_Select_End {scene, track bit / 0x4000 / 0x8000, 1}, stream kind 6 | `nisaudio` 0x4024 | 0x4848F0.., 0x484AC5, 0x45A720 | present (sound5: endings and ShopEnter1 / ShopExit1; the clone has no dyno scenes; crowd187: the lines purged on the post-race / pause exits (0x481210 3 / 6 / 9) and held in the pause, formats.md "Race lines and the post-race exit") |
| S4 | [x] GB_NOS / CollisionStream / BLOWNENGINE / THUNDERSTRM streams | NisAudio_Request by name → SoundFX_Select (1, 4) / (2, 8) / (8, 0x10) / (4, variant), event 0x400E | `nisaudio` 0x400E | 0x484608..0x4846DD, 0x484F20, 0x484E82, 0x486130 | present (sound6: GB_NOS on a nitrous start above fraction 1.5, BLOWNENGINE once when the engine blows, CollisionStream when the crash camera starts, THUNDERSTRM held off while GB_NOS plays) |
| S5 | [x] Rachel's hints, in-game speech | the hint slots (0x5111F0 posters, 0x54BD40 / 0x53E0C0), front-end lines (0x5243E0), D-Day / showcase / outrun / neighbourhood lines → switches Career / Time / FE_Screen / … | `UG2_Speech` | 0x46D150.., 0x53E0C0, 0x5243E0, 0x738860 | present (ported in <work dir> / menus107 / race2; round 12 checked the wiring and the offline capture; the library's event choice and the stream arbitration **inferred**; not heard in the original without a sound device) |
| S6 | [x] EA Trax songs | music manager; playlist 0x46B6F0 / 0x479220 / 0x46BAA0 | `MusicSFx.mpf` | 0x482350, 0x46B6F0 | present (sound4: per-side pools and positions kept for the session, Sequential by playlist position, Random by Rand_Int over the songs left; in game the song's volume is the mix map's EA Trax slot 1) |
| S7 | [x] Interactive score (FE, shops, drag, drift) | 0x482180 → 0x47B260: 0x7ECC10[context +0xB4]; in game +0xB4 = the "Ambience" parameter map at the player car (0x46B330) | `MusicSFx.mpf` events 27–77 (streams 0, 11–16, 23–28, 40–42) | 0x47B260, 0x46B330 | present (sound4: the in-game fade through SFXObj_Ambience's f2 and event control 1, the level from section 0 master 4, the Volumes ramp, the shop contexts 0x16–0x1D, the score with the music off; the second event of each pair is never started by the exe's music code, see formats.md) |
| S8 | [x] Movie audio | movie player | SCHl in `.vp6` | — | present |

## Bank inventory (round 11, sfx)

Every bank the exe can load (the csi list 0x7ED148..0x7ED15C: UG2_MAIN,
UG2_FE, UG2_ENGINES2, UG2_TURBO, UG2_ENVIRO, UG2_STITCH; the bank names
0x7ED190..0x7ED22C and the engine / gear / turbo tables) with its events,
its entries and who plays them. the maintainer's probe `sound_inventory` lists the
modules, sound lists and sample lengths.

| Bank | Event | Entries | Played by | Clone |
|---|---|---|---|---|
| GLOBAL/FE_COMMON_MB | PlayCommonSample | 18 samples, ids 0–22 | PlayFESound 0x458520 (F1–F21) | present |
| FE/FE_MB | PlayFrontEndSample / _loop | 68 samples, ids 23–93 | PlayFESound; the loop id 73 (F20) | present but ids 88–93 (F19), 75 (F18); 74 and 46–48 n/a (F17, F15) |
| FE/FE_MB | FEDriveOn | — | never started (F22) | n/a |
| IG_GLOBAL/ENV_COMMON_MB | ENV_STATIC | variants 0 train, 1 plane, 4 fountain, 5–7 horn, 8–9 stereo (sounds 1–8; 2 / 3 none) | W7, W4, C17, C18 | present |
| IG_GLOBAL/FX_MAIN_MEM_MB | FX_MAIN_MEM | 4 looped layers | planes in URL races | present |
| IG_GLOBAL/FX_Misc_MB | FX_Camera, FX_Scrape | 1 + 1 | H4, C13 | present |
| IG_GLOBAL/FX_Rain_MB | FX_Weather | 2 loops | W6 | present (inside-view variant open) |
| IG_GLOBAL/MB_Stich_Collision | AEMS_StichCollision | 94 samples, 289 stitch records | C11, C12, W5, the road thump, the FE car parts | present |
| IG_GLOBAL/MB_Stitch_Whoosh | AEMS_StichWoosh | 16 samples | C14, W3 | present |
| IG_GLOBAL/ROADNOISE_00_MB | FX_ROADNOISE, FX_ROADNOISE_TRANS | 7 loops; 3 transitions | C9; W10 (variant 3) | present |
| IG_GLOBAL/TRAFFIC_MB | FX_TRAFFIC | 2 loops | W2 | present |
| IG_GLOBAL/WIND_00 / 01_MB | FX_WIND, FX_WIND_Weather | 4 / 3 loops | C10 | present |
| NOS/Hydraulics_00_MB | FX_Hydraulic, FX_Hydr_Bounce | 7 | C15 | present (hydro) |
| NOS/Nitrous_00 / 01_MB | FX_NITROUS, FX_PURGE | 6 each | C7 | present |
| SKIDS/* (4) | FX_SKID | 5 loops each | C8 | present |
| SHIFTING/GEAR_* (16) | FX_SHIFTING_01 | 2 each | C5 | present |
| TURBO/TURBO_* (15) | FX_TURBO_01 | 5 each | C6 | present |
| ENGINE/CAR_xx_ENG_MB_EE (41), GIN_* | CAR | per car | C1, C2 | present |
| ENGINE/CAR_xx_ENG_MB_SPU (41) | CAR | per car | W1 | present |
| ENGINE/SWTN_CAR_xx_MB (41) | CAR_SWTN, CAR_Sputter | per car | C3, C4 | present |
| ENGINE/Sparkchatter_00 / 01 | FX_SPARKCHATTER | 10 | never started | n/a |
| EVT_SYS/MW_MAIN_AEMS, MW_SIREN_AEMS | SIREN … | no bank | never loaded | n/a |
| SDATA nisaudio | scene lines, SoundFX streams | — | S1–S4 | present |
| SDATA UG2_Speech | Rachel's hints | — | S5 | present |
| SDATA MusicSFx | songs, score | — | S6, S7 | present |

**Event start sites** (round 11): the 41 call sites of the event start
0x7358C0 (each a call and its retry after 0x735840), by event slot:
PlayCommonSample 0x457C87 / 0x46982F, PlayFrontEndSample 0x457D37 /
0x46969D, PlayFrontEndSample_loop 0x4695EA, FX_NITROUS 0x457EE5,
FX_PURGE 0x458BE6 / 0x45F38C, FX_Hydraulic 0x457FD5, FX_Hydr_Bounce
0x47DECB, FX_Scrape 0x4580A0, FX_TURBO_01 0x45830D,
AEMS_StichCollision 0x45840B, AEMS_StichWoosh 0x4584EB, FX_Camera
0x45E522, FX_SHIFTING_01 0x45E990, FX_ROADNOISE 0x45FBAB / 0x45FC5C /
0x472F02 / 0x472FA4, FX_ROADNOISE_TRANS 0x473360 / 0x4734F7 / 0x46AEBD
(the water drum), FX_WIND 0x45FF28, FX_WIND_Weather 0x4606FF, FX_SKID
0x4609F8, CAR 0x4613DC (AI) / 0x47DA5E (player), FX_Weather 0x4617EA,
FX_TRAFFIC 0x46211F, FX_MAIN_MEM 0x4628FA / 0x462C99, ENV_STATIC 0x4629F6
/ 0x462D14 (planes) / 0x463121 (train) / 0x4639F6 (fountain) / 0x475A96
(horn) / 0x46368D (stereo), CAR_SWTN 0x472228 / 0x47240B, CAR_Sputter
0x48520A. FX_SPARKCHATTER and FEDriveOn have none. Every one is in the
table above.

## Totals

2026-11-04: 67 rows: 60 present, **0 missing**, 0 open, 7 n/a.
The rows the roadmap's C3 still listed as missing were already ported and
their rows updated: F18 the menu mouse (`Game::fe_mouse`), F19 / C15
the hydraulics (hydro), S5 Rachel's hints (round 12). Rachel's hint timers
(career +0x887C, slots +0xA8 {ticks, kind, value}, the driving / FE clocks
+0x110..+0x11C and +0x120..) are saved and loaded with the career
(`dday::Hints::to_bytes` / `from_bytes`) and match the original's own
load-and-resave byte for byte except the four dwords its loader 0x529240
zeroes (tests `clone_rewrite_matches_the_originals_resave`,
`clone_round_trip_keeps_the_originals_bytes` in tests/native_save.rs).
Engine-side change this round: the racers' engines (C-row "AI racers' CAR")
keep playing behind a race ending's post-race screens, as under Wine
(`docs/nis-audio-patterns.md`, the audio pass).

Markers (2026-11-01): 67 rows: 57 present, 3 missing, 0 open,
7 n/a. New: H5–H7, the engage prompt's queue sounds (FE ids 14, 15, 16),
which the clone lacked: it had no queue for the markers and no sounds for
the SMS slot. Traced (0x4C9170, 0x4DA9A0, 0x4C8AC0, 0x4AB9F0, 0x51DE40)
and checked in the original under Wine with the audio capture
(`fastgpu.sh --audio-file`, `sndwatch.py`: the AEMS instances started):
an engage tip started PlayCommonSample {0x10, 4235, 0x1000, 0} (id 16,
554 ms) and a second later {0xF, …} (id 15, 189 ms); the garage marker
{0xE, …} (id 14) then id 15; opening the garage's dialog id 5 (its
"ready", already ported) and Back id 5. The level parameter 4235 is
SFXObj_FEHUD's FE_COMMON slot, as the clone's. The clone's capture
(`NFSU2_AUDIO_DEBUG`, shot `sms_tip`) plays 16 then 15.

Round 12: 64 rows: 54 present, 3 missing, 0 open, 7 n/a
(before: 51 present, 4 missing, 2 open). Moved: C18 open → present (the
racers' stereo roll runs at the world's start, 0x458850 → 0x4765F0),
W10 open → present (the water drum's splash: +0x505 is the prop knocked
loose, the level from the object's impact level × 17000), S5 missing →
present (already ported; the row was stale). W2–W5 / C17: the traffic
sounds now run on the native six 70 m contexts (0x463E30 / 0x476AF0),
not on every traffic car. C15 / F19: hydraulics are a PC gameplay
feature (shop slot 0xA1, actions 19 / 22, physics 0x5A51C0); the clone
lacks it, so they stay missing (sound object traced). F18 (menu mouse)
stays missing. See formats.md, "Sound round 12".

Round 11 (sfx): 64 rows: 51 present, 4 missing, 2 open, 7 n/a (before:
59 rows, 50 present, 4 missing, 5 n/a). A bank-by-bank inventory and the
41 event start sites (above) found one sound source the audit lacked,
the car stereos: CARSFX_TrafficStereo is ported; CARSFX_Stereo on
the AI racers stays open. New open: W10 (the water drum's
splash, now named); new n/a: C19 (FX_SPARKCHATTER, never started), W11
(the Most Wanted siren csi, never loaded). Still missing: C15 / F19
hydraulics, F18 menu mouse, S5 Rachel's hints. See formats.md, "Sound
round 11".

Round 10 (mixfx): 49 present, 0 partial, 5 missing, 0 open, 5 n/a
(unchanged counts). W8's leftovers ported: a voice's gain or send that
moved ramps over the next chunk's first 16 samples (0x728B48 / 0x728957,
1/17 a step), a stopped voice's last sample fades out over 16 samples at
the next chunk (0x728FD2 / 0x72886C), the engine ABK voices send p14
(wet B) from their start blocks; the music's send commands 7 / 16 are in
no event of `MusicSFX.mpf` (nothing to port). The mixer is cheaper than
before (voices per chunk). See formats.md, "Sound round 10".

Round 7: 40 present, 9 partial, 5 missing, 0 open, 5 n/a. Moved:
W8 → present (the reverb: the FX graph's ten module kinds traced and
ported, the preset table re-read from 0x7F3778, the zones and the
"Reverb" region map). See formats.md, "Sound round 7".

Round 9: 49 present, 0 partial, 5 missing, 0 open, 5 n/a
(unchanged counts). F1: MC_Main and its messages, the dyno chart, the
showcase's picture state and the strip shops (rims, ICE, neon, decals)
traced key by key; the strip never moves from inside a list (nothing to
port); the crib's swing waits 0.3 s after the last key; the car-part
sounds of 0x61F830 play (FE ids 0x4C–0x53 in the garage, the FE_COMMON
player's collision stitches 0–5 in the showcase); Options' Player rows all
answer Left / Right; dialogs' Back mapped by wrapper. W8's leftovers (send
ramps, voice-stop declick, music sends) described for main's mixer in
formats.md, Sound round 9; the music commands' senders stay open.

Round 8: 49 present, 0 partial, 5 missing, 0 open, 5 n/a.
Moved to present: F1, F6, F7, F10, F12, F13, F14, F21 (every clone screen
plays its handler's sounds and FEng's sound notification; MC_Main, the
dyno chart and the online stubs remain on the control-kind rule, noted
in F1). Still missing: W8 reverb (ported in main), C15 / F19
hydraulics, F18 menu mouse, S5 Rachel's hints.

Round 7 (fesound): 41 present, 8 partial, 5 missing, 0 open, 5 n/a.
Moved: W5 → present (traffic bottom-out, volume 127 by the mover), W9 →
present (Ambience has no update). F1 / F6 / F7 / F10 / F12–F14 / F21
stay partial: every call site is mapped to its screen (formats.md, Sound
round 7) and the list screens, tuning sliders, engage dialogs, mailbox,
career world map and track select now play their handlers' sounds; the
car lots / selects, in-game map, status, stats, controls, jukebox, shop
browsers, showcase and the generic dialogs remain. Still missing: W8
reverb, C15 / F19 hydraulics, F18 menu mouse, S5 Rachel's hints.

Round 6: 39 present, 9 partial, 6 missing, 0 open, 5 n/a (the
59 rows). Moved: W7 → present (planes, trains, fountains), S4 → present
(GB_NOS, CollisionStream, BLOWNENGINE), F6 missing → partial (its sites
are the generic helpers, part of F1), F15 and F22 open → n/a (no opener;
never played). F12 gained the parts browser's WRONG. Still missing: W8
reverb (applies on PC; the FX graph's DSP modules not traced), W5 traffic
bottom-out (its knock volume +0x28 open), C15 / F19 hydraulics and F18
the menu mouse (the clone lacks the feature), S5 Rachel's hints (queued
separately). Still partial: F1 / F6 / F7 / F10 / F12–F14 / F21 (which
control reacts on the screens other than the icon strips and the parts
browser: 171 call sites of 0x50D150 / 0x50D180 / 0x51DE40 / 0x51DE70 to
map to the clone's screens) and W9 (SFXObj_Ambience's own update).
See formats.md, "Sound round 6".

Round 1: 59 rows: 27 present (4 of them new in this pass), 9 partial, 18 missing,
3 open, 2 n/a (no such sound in the game: H3).

Round 2: 31 present, 8 partial, 15 missing, 3 open, 2 n/a.
Moved to present: F8 (icon strips' own sound rule), F9 (setup purchase),
F16 (transmission answer), W4 (traffic horns); F1 narrowed to the
screens other than the icon strips. Not reached: S7 (table traced only),
W6–W8, C15, S3, F5–F7 / F10–F15 / F17–F19 / F21, the three open rows
(H4's flag writer not found by pattern search). Ported in this pass:
C14 (drive-by whooshes), S2 (race start scene line), H1 (GPS found), F20
(FE car turn loop). The partial rows are almost all front-end sounds the
clone plays from the key pressed rather than from the control that
reacted (F1); the missing ones by impact: S5 Rachel's hints (queued
separately), S7 the interactive score (drag / drift races and the front
end), W4 traffic horns, F5–F19 screen-specific FE sounds (menus),
W7 / W8 world objects and reverb, W6 rain, C15 hydraulics.

Round 3: 33 present, 9 partial, 11 missing, 3 open, 3 n/a.
Moved: F5 → present (the showcase's world start / stop, id 5), H4 →
present (FX_Camera is the showcase's camera shutter, not a camera-change
whoosh: its flag's only writer is 0x458880, called by 0x501250 from Take
Picture), F10 → partial (0x521290 is the part-fitted sound), S7 →
partial (the score in drag / drift races by the Ambience map), F17 →
n/a (online messenger), F15 → open (it is UI_VirtualKeyboard, which no
code opens). F18 and F19 re-identified (the menu mouse; the garage
hydraulics motion), both still missing with the clone lacking the
feature. Not reached: F6, F7, F11–F14, F21 (generic key sounds), W7 /
W8, C15, S3, F22, W9. W6 has a hook (`EnvInput::rain`) for the weather
port.

Round 4: 35 present, 8 partial, 11 missing, 2 open, 3 n/a
(the 59 rows recounted from the table). Moved: S7 → present, F11 →
present, W9 open → partial (SFXObj_Ambience is the score's level
object). S6 reworked (the playlist). Not reached: W6 rain (traced as far
as CARSFX_Rain's update 0x4747A0: start 0x461750 above 0.001, stop
0x461840, a thunder timer Rand_Int(20) + const, not ported), W7 / W8,
C15 / F19 (the clone has no hydraulics), S3, F1 / F6 / F7 / F10 / F12–F14
/ F21, F22. Open: the front end's own sounds and songs through the mix
map (SFXObj_FEHUD master 9's slots per FE id are not traced), so the
front end keeps the option levels at full scale.


Round 5: 37 present, 9 partial, 8 missing, 2 open, 3 n/a.
Moved: W6 → present (rain and its thunder), S3 → present (ending and
shop scene lines), S4 missing → partial (THUNDERSTRM). Checked under
Wine: round 4's in-game levels (EA Trax slot 1 11585, stream volume 45),
and the front end's (songs slot 0 6447 → 25, FEHUD slots 4235 / 6157 /
… / 3085), which the clone now follows (songs, score and FE sounds).
Not reached: W7 / W8 (placement not traced, no reverb DSP), C15 / F19
(no hydraulics), F1 / F6 / F7 / F10 / F12–F14 / F21 / F22 per-screen
controls.

Hydraulics (hydro, 2026-10-31): C15 and F19 → present. The feature is
ported end to end (docs/formats.md, "Hydraulics"): CARSFX_Hydraulics on
the player's car and the garage motion's FE sounds. **Inferred**: the
object always exists (the factory's pool test taken as met). Not
checked by ear: the fast GPU runs under Wine are silent.
