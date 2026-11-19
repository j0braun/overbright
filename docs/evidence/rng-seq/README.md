# Random draws: the original's sequence log against the clone

The files here are excerpts of the original's sequence log (docs/original/rng.md, "The sequence log"):
free roam's entry with the sound off and on, and a circuit and a drag race's set-up with the
sound on, each from the stage-1 save `tests/fixtures/native_saves/stage1_win_streetx.sav`.

Clone comparisons (`NFSU2_RNG_PIN`, `NFSU2_RNG_LOG`, sound off), every shared draw's site
against the original's and the world generator's fill sequence:

| run | pinned at | shared draws | world draws | result |
|---|---|---|---|---|
| circuit 4F7D2AC5 | the dialog, 12345678 | 3555 to the scene pick | 366 | identical |
| sprint 1FD60BD2 | the dialog, 12345678 | 3555 to the scene pick | 81 | identical |
| drag C7483590 | the dialog, 12345678 | 3551 to the scene pick | 287 | identical |
| free roam entry | the encounter set-up, C9390EDA | 3586 to the first wind gust | the fill | identical |
