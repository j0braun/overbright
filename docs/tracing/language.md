# Tracing round: installed language

How `SPEED2.EXE` decides the language and reads its files. Detail and addresses:
[docs/formats/front-end.md "Language selection"](../formats/front-end.md). Tags as in
[docs/formats.md](../formats.md).

## Traced (verified)

* The language comes from the registry string `Language` under HKLM `Software\EA Games\Need for Speed
  Underground 2` (0x5B7700), matched against 14 names ("English US", "English UK", "French", …, "Thai";
  table 0x7FCA08) to a language id in 0x7FCA00. Missing value: English (0). Unknown value: −1, which the
  game language treats as English (0x5B7B00).
* At start 0x512520 sets the game language 0x7F70D0 from it through SetLanguage 0x512360. Nothing else
  feeds it in normal play: no file is probed, the region byte 0x7FA73C is not read, the movie suffix
  (`_fr` in the reference build, 0x51C740) is a separate constant.
* The file is `LANGUAGES\<NAME>.BIN` from table 0x7F7020 (ids 0–10: English, French, German, Italian,
  Spanish, Dutch, Swedish, Danish, Korean, Chinese, Japanese); an id without a record (Thai, 11) loads
  record 0, English. The whole file is loaded (0x4FF780) with the record's font sets (Latin, Korean,
  Chinese, Japanese).
* SetLanguage also sets the metric default (any language but English), the number separators, the
  default profile name label, the speech bank call 0x468F30 (**inferred** purpose), and refreshes the
  front end.
* The system language (not the game language) decides the controls' key names and the name editor's
  starting text.
* LanguageTextures.bin is loaded with the front end for every language; Labels.bin, Largest.bin and the
  agree.* files are not named by the executable.

## Inferred

* The registry's third column (0x7FCA08 +8) looks like region codes but has no reader.
* A debug key (0x527530, gated by 0x8383C8) cycles through the installed language files.
* Under Wine the −1 seen for the controls means the prefix's `Language` value holds a name outside the
  table.

## Open

* What the installer writes into `Language` for each retail edition (not checked; no installer at
  hand). A reimplementation that has no registry should pick the language the way the result would be:
  from the installed `LANGUAGES\*.BIN` or the player's choice; English when unsure, as the executable does.
* The 0x4FF780 arguments from 0x8654A4 (game state 3 / 6) that select which font sets load.
