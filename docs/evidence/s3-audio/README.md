# Audio captures (2026-11-04)

Vanilla SPEED2.EXE (sha256 8f6a9ae2…2a297) in the private copy
`$OB_PRIVATE/wine/runs/<run>` (WidescreenFix off), fastgpu on :202 with
`--audio-file`; recorder `../nis-audio-patterns/record.py` + `tick-hook.bin`
(both 0x46FA30 call sites; all bytes restored, `restored: true`, 0 lost).

* `dyno.json.zlib`: the finished-career save, Garage → Performance Tuning / Dyno → DynoEnter →
  Dyno Run → Dyno Results → Dyno Shop → Quit Tuning → Quit to Garage: 278
  updates (DynoEnter only); no DynoExit.
* `lose.json.zlib`: Quick Race (Resort Loop, RX-7), `lose.py` finishes an AI
  first, then the human → EndingNis01 behind Event Results; `lose-finish.json`
  its writes; `end-events.tsv` sndwatch (5 s) during the hold.
* `career.json.zlib`: Start New Career (profile NAMEX) → CareerNis1, 4 × 403
  scripted idle updates.
* `fixture.py` appends EndingNis01 / CareerNis1 to
  `../nis-audio-patterns/native-updates.txt`.
* `waveform.md`, `wave_cmp.py`: the DynoEnter waveform comparison.
