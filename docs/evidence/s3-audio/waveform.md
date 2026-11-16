# DynoEnter engine waveform: original vs clone (audio pass item 7, 2026-11-04)

Same car and save: the finished-career save's Lancer (the Wine copy's the finished-career save, also the
clone's input), scene DynoEnter (pattern 1, 278 scripted updates).

* Original: `a private Wine copy/s1.wav` (fastgpu `--audio-file`, music volumes 0
  in this profile), cut at the recorded update ticks via `s1.sync.tsv`
  (1 s pre-roll) → `native-dynoenter.wav` (work dir, not kept here).
* Clone: test `dyno_enter_waveform_capture` (NFSU2_WAVE_SAVE = the same
  save, NFSU2_PATTERN_PCM = out; offline mixer 48 kHz, one step per 1/30 s).
  The `--shot dynoenter` path with NFSU2_AUDIO_WAV gives the same result,
  and so does main's binary (2026-11-04 17:33): **pre-existing**.
* Compare: `wave_cmp.py` (RMS per second / per 1/30 s, envelope
  cross-correlation, dominant 40–700 Hz frequency per frame).

Result — **no match**:

| second | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| original RMS dBFS | −27.3 | −17.3 | −17.6 | −25.7 | −21.5 | −24.4 | −22.9 | −16.9 | −19.6 | −14.1 |
| clone RMS dBFS | −inf | −inf | −inf | −inf | −inf | −inf | −inf | −inf | −52.5 | −49.9 |

The clone's pattern itself is right (rpm / throttle bit-exact with the
native updates, `native_scene_updates_replay_bit_for_bit`), but its engine
voices are silent: the mix map's player level 0 is ~4400–5300 during the
scene (camera 44 → 4 m from the car) and the HybridMotor's volume bytes
(`HybridMotor::voices`) stay 0–9 of 127; even at level 32767 after the
scene, idle gives bytes [0, 0] and the ABK steady layers are silent
(−50 dBFS overall). **Open** (not traced this round): the native player
engine's level 0 / volume bytes during DynoEnter (read them with peek.py
on the CAR instance, as soundcmp did in free roam) and why the clone's
chain gives near-zero bytes for this car.
