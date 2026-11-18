# Engine level evidence (2026-11-04)
Original: vanilla SPEED2.EXE in a private Wine copy (fastgpu :207, `--audio-file`), the finished-career save's Lancer (md5 97c1be0c…).
WAVs stay in the work dir `<work dir>/engine-work` (n-dyno2.wav, n-dyno4.wav (effects 0), n-free.wav).
* `n-dyno2.tsv.gz` (`mon.py`): DynoEnter, per ~40 ms the player CAR event's 26 params, CARSFX_PlayerEngine (HM +0x7C/80/84, +0xF8, ctx rpm/throttle, GIN bytes +0x108/+0x10C) and the reverb globals 0x82B078/0x82A998/0x82B070/0x82B880/0x82B060.
* `n-dyno3.tsv.gz` (`mon2.py`): every live AEMS instance's params through DynoEnter.
* `n-dyno5.tsv.gz` (`mon3.py`): view mode 0x82BB48, ctx +0xA8, Physics +0x1C slew, listener globals, CAR p4/p14.
* `n-free.tsv.gz` (`mon.py`): free roam idle 8 s, Up 4 s, idle.
* `players.py`: a native AEMS instance's sound players (compared with the maintainer's probe `aems_probe` for the same block: identical).
* `cmp.py NATIVE.wav CLONE.f32 [offset]`, `dbfs.py`: per-second RMS. Clone PCM: tests `dyno_enter_waveform_capture`, `free_roam_waveform_capture` (NFSU2_WAVE_SAVE, NFSU2_PATTERN_PCM).
