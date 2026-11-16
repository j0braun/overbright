# The player's engine level against the original (2026-11-04)

The audio pass found the player's engine digitally silent in DynoEnter
(`docs/evidence/s3-audio/waveform.md`). This pass measured where and why,
under Wine (vanilla SPEED2.EXE, private copy `a private Wine copy`, fastgpu on
:207 with `--audio-file`), and fixed the chain. Evidence:
`docs/evidence/engine-level/`.

Save: the finished-career save's **Lancer** (the audio pass's save, a private Wine copy's finished-career save, md5
97c1be0c…; master 0.5, engine 0.65, effects 1, tires 0.65, both music
volumes 0). Note: `player-settings/the finished-career save-20261104` (md5 4a8797d6…) is a
different the finished-career save whose current car is a **Skyline** (master 0.95, all
others 1).

## Causes (each traced, then confirmed under Wine)

1. **No reverb control without a drivetrain → kind-8 volume 0
   (verified).** SFXObj_Reverb's start (0x45D950..0x45DABC) sets
   0x82B078 = 0, 0x82A998 = 0x7F, 0x82B070 = 0, 0x82B880 = 0; SFXCTL_Tunnel's
   update (0x4714A0) then moves them to the preset's levels. The engine's
   abk block sends p13 = clamp([0x82B880] − 0x7FFF) (0x474310), which the
   CAR graph turns into its players' kind-8 "second volume"; the GIN
   voices' dry byte is [0x82A998] >> 8 (0x473ECF). The clone ran its whole
   in-game sound path (reverb, road, wind, skids, turbo, sputter) only
   when the car had a drivetrain (`car_sfx_input`), and a car placed for
   a scene and never stepped (the dyno's) has none: dry B stayed 0 → every
   CAR layer's kind 8 = 0 → gain 0, and the GIN dry byte 0x7F >> 8 = 0.
   Under Wine in DynoEnter 0x82B880 = 32767 from the first update, and
   the CAR, FX_ROADNOISE ×4, FX_WIND, FX_WIND_Weather, FX_SKID ×2,
   FX_TURBO_01 (+ blow-offs at rpm 6503 / 7388 / 5458) and CAR_Sputter
   instances are live (`n-dyno3.tsv.gz`). Fix: the sound inputs no longer
   need a drivetrain (neutral, no throttle, the idle rpm: `sound_rpm`).
   The CAR graph itself was already exact: the native instance's players
   (state, list index, params) equal the clone's for the same block
   (`players.py`, players 4 / 7 at volumes 450 / 491 on the dyno screen).
2. **Option volumes applied twice (verified).** SFXCTL_MasterVol
   (0x471970) and the player's volume fields (0x45C210) put master ×
   effects / engine into the mix map's levels (level[0] 4181 in free roam
   at master 0.5 / engine 0.65, measured from the CAR's p4 = −31491 with
   HM +0x7C = 10000; the clone gives 4182), and the library scales a voice
   by its byte alone (0x72412A: byte × 1/127 (0x78A94C) into voice +0x38,
   table [0x8B466C], 0x90 bytes a voice). The clone's mixer also
   multiplied every voice by effects × master and the GIN voices by
   engine × master (−6 / −9.8 dB with the finished-career save's volumes; 0 dB at the default
   full volumes). Under Wine with effects = 0 the engine's abk layers
   still sound (−41 dBFS on the dyno screen). Fix: with the mix map loaded
   the effects and engine buses are 1 (`stream_sounds`), and the FE
   events' compensating division is gone (`fe_event_levels`).
3. **The NIS view mode (verified).** 0x46D7E0 (0x46D961..0x46D9AD) sets
   the listener's view mode 0x82BB48 = 4 while a NIS scene's objects
   (0x827E98 / 0x827EB0 / 0x827EC8) exist and the race status
   ([0x890118]) is not 3 / 4 (or 0x82A3A4 > 0; 0 at runtime); the 3D
   controls pick their variant by it. Read under Wine: 4 through
   DynoEnter, 0 on the dyno screen. The clone took the player's camera
   setting's mode (0 here): level[0] 4434 → 5313 over the scene against
   the native ≈ 1530 → 4100 (from p4 / p14). Now 4 in a scene (a race
   start only before its cut: **inferred** mapping of the status change
   to `holds_cars`): 1481 → 5216.
4. **The dyno screen (observed).** On Performance Tuning after DynoEnter
   the car's sound objects stay alive (CAR p1 1354, p4 −31083, level[0]
   5518, the engine at −40 dBFS); the clone silenced them in menus. Now
   the tuning menus over the dyno track run them
   (`tuning_menu_over_world`): level[0] 5313, p4 −31146, −40 dBFS.
5. **The effects' throttle in a scene (traced).** The turbo reads the
   context's +0x54 (`Turbo::update`); in a scripted scene that is the
   pattern's throttle (0x46FA30), so `CarInput::throttle` takes it there.

The player engine's own outputs were already right: under Wine HM +0x7C
/ +0x80 / +0x84 and the GIN bytes (CARSFX_PlayerEngine +0x108 / +0x10C,
found at ctx − 0x140, vtable 0x78AF30) during DynoEnter are 0–6 of 127,
as the clone's — the dyno's engine is quiet in the original too.

## Levels, RMS dBFS per second (mono mean)

DynoEnter, the finished-career save's Lancer, full mix (native `n-dyno2`, second 0 = the
pattern's first update; the clone's offline mixer, one step per 1/30 s):

| second | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|
| original | −45.2 | −46.2 | −43.8 | −41.5 | −40.4 | −30.3 | −30.1 | −27.6 | −31.7 |
| clone before | −inf | −inf | −inf | −inf | −inf | −inf | −inf | −inf | −52.5 |
| clone after | −51.4 | −45.5 | −43.0 | −43.5 | −40.5 | −33.9 | −39.0 | −40.2 | −40.0 |

Seconds 6–9 of the original carry the interactive score (both music
volumes 0 → the score at the effects level: stream voices at gain
0.14 / 0.11, a 32–38 Hz bed; gone with effects 0). The clone stops the
score in a scene (`music_context_of`): fixed in L1 (`MusicContext::Dyno`,
internal notes), not engine. The audio pass's −17 dBFS
capture (same save) was ~15 dB louder than four captures here with the
same envelope (corr 0.94): a different score piece, **inferred**.

Dyno screen idle after the scene: original −40.0 (−40.8 with effects 0),
clone before silent (the score from −50), after −40.0.

Free roam, the finished-career save's Lancer at the garage exit, idle 8 s / throttle 4 s /
idle (native `n-free`, clone `free_roam_waveform_capture`):

| | idle | throttle (s 1–4) | idle after |
|---|---|---|---|
| original | −43.0 … −40.9 | −35.3 −34.3 −35.1 −32.7 (−27.3 peak) | −39.3 … −41.4 |
| clone before | −43.6 … −42.7 | −41.3 −39.3 −40.5 −38.0 (−32.5 peak) | −44.7 … −45.8 |
| clone after | −40.6 … −39.3 | −34.3 −33.7 −34.5 −31.9 (−26.5 peak) | −41.3 … −40.8 |

level[0] 4181 / 4182, p4 −31491 / −31491, GIN bytes at full throttle 2 /
2 (original / clone). The before build's throttle was ~6 dB low (cause 2);
the original's idle also holds the world's beds (score, traffic,
ENV_STATIC), so idle is not an engine-only figure.

## Not measured (open)
Garage / showcase (no CAR instance in the front end, native and clone:
silent in both), shop scenes, CareerNis1, race starts and a stock car:
not captured this pass. Fixes 1–3 apply to every NIS scene (view mode
4; the reverb and effects run for any car without a drivetrain).
The dyno run (Dyno Run's pull) is not compared. The clone's first
scene second starts the CAR event cold (the original's exists before
the scene): −51 vs −45.
