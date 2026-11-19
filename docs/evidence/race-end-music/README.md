# A race's song through its end (the original under Wine)

Quick Race, Circuit, first track, Peugeot 206, fresh profile, default volumes; the original on a
private display with its sound written to a file. `musmon3.py` logged the music object every
50 ms (rows when a value changes); `win.py` wrote the human's finish into the race record.

`native-music-state.tsv` columns: wall s, g_SimTicks, music state (+0xC0), song (+0x14), stream
0x82AFD4, level (+0xBC >> 8), audio mode (+0x90), pause flag (+0x13), pause mode (+0x50), game
state 0x8654A4, flow state, race status, NIS objects present (0x827E98).

- 102.16 s: song 24 starts in PRERACE (flow 3).
- 124.22 s: the post-race load (flow 1), then the ending scene (flow 7) under Event Results and
  Post Race: still song 24 on the same stream; only the level moves (45 → 38 → 45).
- 218 s: Restart → the reload (game state 5) holds the song (state 5); the reload then hung (Wine).
