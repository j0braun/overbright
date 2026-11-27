# Asset format: strings, languages, credits, subtitles, legal texts

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.
Rule prefix: `TXT`.

Every text the original shows comes from a language's **string table**, looked up by the hash of a label
(`DEFAULT_STRING` → `0x881F8EFA`). The format keeps one unit per language, `text/<language>/`, with the strings in
TOML keyed by that hash, the label name next to it, and the text in UTF-8. The original's own code pages are kept
alongside so that every file can be rebuilt byte for byte; a text whose decoding is not proven is kept as its
bytes. Credits, the save library's texts, the licence texts and the subtitle cues are small kinds of their own.

Schemas: [`language`](schemas/language.schema.json), [`strings`](schemas/strings.schema.json),
[`credits`](schemas/credits.schema.json), [`legal-text`](schemas/legal-text.schema.json),
[`memcard-text`](schemas/memcard-text.schema.json), [`subtitles`](schemas/subtitles.schema.json). Examples:
[`examples/text/`](examples/text/).

## 1. Scope

| Original | Reference install | Kind here | Engine reader today |
|---|---|---|---|
| `LANGUAGES/<Lang>.bin` chunk 0x39000, the string table | 14 files: the 11 languages the executable names (English, French, German, Italian, Spanish, Dutch, Swedish, Danish, Korean, Chinese, Japanese), `Thailand.bin`, `Labels.bin`, `Largest.bin`; 3,925 strings each | `ob.strings`, `ob.language`, `charmap.bin` | `crates/ob-menus/src/frontend/feng/strings.rs:33`; the active language's file is read (`Assets::load_for`, `crates/ob-menus/src/frontend/assets.rs`) |
| `LANGUAGES/<Lang>.bin` chunks 0x30201 | 6 or 7 fonts per file | `ob.font` on [ui.md](ui.md) | `crates/ob-menus/src/frontend/feng/font.rs:55` |
| `LANGUAGES/Labels.bin` | the label names (§2) | `label` fields and the `labels` unit | not read |
| `LANGUAGES/Largest.bin` | per label the longest text of all languages (§2) | the `largest` unit | not read |
| `LANGUAGES/LanguageTextures.bin` | one texture pack: the glyph pages of every language's fonts (14 textures) | [textures.md](textures.md); §3.9 | `crates/ob-menus/src/frontend/assets.rs:74` |
| `CREDITS/*.TXT` | 12 files, 465,556 bytes | `ob.credits` | `crates/ob-menus/src/frontend/screens/credits.rs` (`credits_file_for`) |
| `LANGUAGES/agree.*`, `agree-ucs2.chi` | 11 files, 522,076 bytes | `ob.legal-text` | not read by the engine; no file name in the executable refers to them (string scan) |
| `memcard/LOCALE_*.loc` | 17 files, 12,308 bytes | `ob.memcard-text` | `crates/ob-menus/src/frontend/profiles/memcard_text.rs` (`locale_file`); the active language's `LOCALE_<LANG>.loc` is read, the English texts built in are the fallback |
| `SUBTITLES/*` | 22 files, 1,456 bytes | `ob.subtitles` | not read: the original starts subtitles only in regions this edition is not (docs/formats/movies-replays.md "Subtitles") |

Not on this page: the fonts themselves ([ui.md](ui.md) §3.2) and their glyph pages ([textures.md](textures.md));
the texts authored inside menu packages (`St`, [ui.md](ui.md) §3.1); the EA Trax song titles, artists and albums
of the music caption, which are built into the executable (`crates/ob-formats/src/exe_tables/constants/frontend.rs:239`,
[engine-tables.md](engine-tables.md)); references to strings from other kinds (`string:0x…`, e.g. part names in
[cars.md](cars.md)).

## 2. Files and keys

```text
text/<language>/
  language.toml               ob.language      language:<language>
  strings.toml                ob.strings       string:0xHASH   (resolved in the active language, §3.8)
  charmap.bin                 charmap_region   the original's character map (imported languages)
  fonts/<name>.toml           ob.font          fonts only this language holds (ui.md)
  textures/…                  textures.md      pictures only this language uses (§3.9)
  credits/<FILE>.toml         ob.credits       credits:<language>/<FILE>
  legal/<FILE>.toml           ob.legal-text    legal-text:<language>/<FILE>
  memcard.toml                ob.memcard-text  memcard-text:<language>
movies/subtitles/<NAME>.toml  ob.subtitles     subtitles:<NAME>
```

`<FILE>` keeps the original file name without its last extension (`credits/UK_ENGLISH.toml`,
`legal/agree.eng.toml`, `legal/agree-ucs2.chi.toml`). Subtitle cues hold no text, only label hashes, so one set
serves every language; they sit with the movies they belong to ([movies.md](movies.md) §2) and are keyed by the
name the game asks for (`Scene01` for the movie `Scene01_en.vp6` [inferred]). `<FILE>` and the subtitle `<NAME>`
stand for original files: they are looked up without regard to ASCII case, and a layer stack MUST NOT hold two of
one kind that differ only in case (conventions §6.3, `CONV.13`).

**Language keys** are `[a-z][a-z0-9_]{0,31}`: the original's file name in lower case (layout.md §2). The
importer makes these units:

| Key | String table | Credits | Legal texts | Save texts | `role` |
|---|---|---|---|---|---|
| `english` | `English.bin` | `UK_ENGLISH`, `NA_ENGLISH` | `agree.eng`, `agree.usa` | `LOCALE_ENGLISH` | `player` |
| `french`, `german`, `italian`, `spanish`, `dutch`, `swedish` | `<Lang>.bin` | `<LANG>` | `agree.fre`, `.ger`, `.ita`, `.spa`, `.dut`, `.swe` | `LOCALE_<LANG>` | `player` |
| `danish` | `Danish.bin` | `DANISH` | – | `LOCALE_DANISH` | `player` |
| `korean` | `Korean.bin` | `KOREAN` | `agree.kor` | `LOCALE_KOREAN` | `player` |
| `japanese` | `Japanese.bin` | `JAPANESE` | – | `LOCALE_JAPANESE` | `player` |
| `chinese` | `Chinese.bin` | `CHINESE` | `agree.chi`, `agree-ucs2.chi` | – | `player` |
| `thailand` | `Thailand.bin` | – | – | – | `player` |
| `brazilian_portuguese`, `czech`, `finnish`, `greek`, `norwegian`, `polish`, `portuguese` | – | – | – | `LOCALE_<LOCALE>` | `save_text` |
| `labels` | `Labels.bin` | – | – | – | `labels` |
| `largest` | `Largest.bin` | – | – | – | `layout_test` |

A `player` language holds a string table (`TXT.12`): it is what the base set's manifest lists in `languages`
([manifest.md](manifest.md) §3) and what §3.8 can make active. The seven locales that have only save-library
texts in the reference install are units of role `save_text`: they hold `language.toml` and `memcard.toml`, are
never offered to players and never active, and hold those locales' save texts.

What the two non-player tables are (measured on the reference install): **`Labels.bin`** has the same 3,925 keys
as every language and its text is the label's own name: 3,871 texts hash to their key with `bin_hash`, the other
54 with `bin_hash` of their upper-case form. It is the source of every `label` field. **`Largest.bin`** holds, for
every key, a text equal to one language's text for that key, and for 3,906 of the 3,925 keys the longest of them
(**inferred**: a layout-test table; its character map has 59 entries, more than any single language's 16–25). Neither file, nor
`Thailand.bin`, is named by a path in the executable (string scan; how the original picks its language is the
open point of §10). The `Japanese.bin`, `Chinese.bin` and `Thailand.bin` tables hold only empty texts (all 3,925)
in this edition; their fonts are complete.

| Key | Item | Notes |
|---|---|---|
| `language:<language>` | a language unit | |
| `string:0xHASH` | a string | The hash of the label. Not tied to a language: it resolves in the active language with the fallback of §3.8. |
| `credits:<language>/<FILE>` | a credits text | |
| `legal-text:<language>/<FILE>` | a licence text | |
| `memcard-text:<language>` | the save library's texts | |
| `subtitles:<NAME>` | the cues of one movie | |

## 3. Source form

### 3.1 Language manifest (`ob.language/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.language/1.0` |
| `op` | enum | – | no | `merge` | layering.md §3.1 (`add` for a language a pack brings). |
| `name` | string ≤ 63 | – | yes | – | The name players read (`English`; from the file name as `src/qol/setup.rs:128` makes it). |
| `role` | enum | – | no | `player` | `player` (a language players can choose; holds `strings.toml`, `TXT.12`), `labels` (label names), `layout_test` (longest texts) or `save_text` (only save-library texts, no string table). Only `player` languages are offered to players. |
| `original` | string | – | no | – | The original file of the string table and fonts (import provenance; absent for a pack's language). |
| `charmap` | array descriptor | – | no | absent | The original's character-map region in `charmap.bin`, layout `charmap_region` (§3.2), count 1. Absent: no `bytes` texts may decode through a map. |
| `fonts` | array<ref> | – | no | `[]` | `font:<name>` of the fonts the language holds, in the original file's order. They resolve in this unit's `fonts/` first, then in `frontend/ui/fonts/` ([ui.md](ui.md) §3.3). |
| `extras` | table | – | no | – | conventions §9 |

Example: [`examples/text/text/english/language.toml`](examples/text/text/english/language.toml).

### 3.2 String table (`ob.strings/1.0`)

```toml
schema = "ob.strings/1.0"

[[string]]
hash = "0x881F8EFA"
label = "DEFAULT_STRING"
text = "Demo default text"

[[string]]
hash = "0x078CFCEF"
label = "DEMO_SUBTITLE_01"
bytes = "hex:44656d6f81"
```

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.strings/1.0` |
| `string` | array<table> | – | no | `[]` | The strings in the original's text order (all 14 tables share one order, which is neither the hash order nor the label order: measured). |
| `string[].op` | enum | – | no | `merge` | Packs (§4). |
| `string[].hash` | hash32 | – | yes | – | `bin_hash` of the label: the item key. |
| `string[].label` | string `[A-Za-z0-9_]{1,63}` | – | no | – | The label name (from `Labels.bin` for imported tables; required for an added string, `TXT.4`). |
| `string[].text` | string ≤ 4,096 | – | no | `""` | The text as the engine shows it (§3.3). |
| `string[].bytes` | bytes | – | no | – | Instead of `text`: the stored bytes without the NUL, when the text form is not proven (§3.3). |

**The original chunk 0x39000** (docs/formats/front-end.md "Strings"; every byte accounted for on all 14 tables):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x0000 | 4 | u32 | header size | bytes | 16 (constant) |
| 0x0004 | 4 | u32 | count | – | derived: the number of strings |
| 0x0008 | 4 | u32 | table offset | bytes | 0x1814 = 16 + 6,148 (derived) |
| 0x000C | 4 | u32 | text offset | bytes | table offset + 8 × count (derived) |
| 0x0010 | 6,148 | `charmap_region` | character map | – | from `charmap.bin` |
| 0x1814 | 8 × count | {u32, u32}[count] | table | – | {hash, offset into the text}, ascending hash (derived) |
| text offset | … | char[] | texts | – | NUL-terminated, in `[[string]]` order, no gaps (derived) |
| … | 0–3 | – | pad | – | zero bytes to a multiple of 4 |

**`charmap_region`** (the layout of `charmap.bin`; size 6,148 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x0000 | 4 | u32 | `word0` | – | 256 when the region is a character map (13 tables); Korean's is 0x3D9 (**open**). |
| 0x0004 | 6,144 | u16[3072] | `entries` | – | Entry *k* < 256 for byte *k* ≥ 0x80: 0 keeps the byte, ≥ 0x80 is the code point it shows, 1–0x7F starts a double-byte pair whose second byte *d* indexes entry *v*·128 + *d* − 0x80 (0x50D3C0, `crates/ob-menus/src/frontend/feng/strings.rs:75`). |

### 3.3 Encoding rules

The engine consumes Unicode text: the parser decodes each stored text through the language's map
(`crates/ob-menus/src/frontend/feng/strings.rs:75`; the map is used only when `word0` is 256, `:45`; otherwise every byte is its
own code point). So:

1. **Text form.** The importer decodes every string as the parser does, then encodes the result back through the
   inverse of the single-byte map (a character below 0x80 is its byte; any other is the one byte ≥ 0x80 whose entry
   gives it, or whose entry is 0 and whose value is the character). When that gives the stored bytes exactly, the
   string is written as `text` (UTF-8, conventions §3.1). Proven for every string of 13 tables, including
   `Labels.bin` and `Largest.bin`: no two bytes of a map give the same character, no shipped text uses a
   double-byte lead, and no decoding hits the map's error character.
2. **Byte form.** Otherwise the string is written as `bytes`, and the engine decodes it at load exactly as the
   parser does. All 3,925 Korean strings take this form: the Korean region's `word0` is not 256, so the parser shows
   every byte as its own code point today; the Korean decoding is **open** (§10).
3. **No normalisation.** Readers and writers MUST NOT normalise Unicode (NFC, NFD,
   https://unicode.org/reports/tr15/), trim, or change line breaks in any text of this page; texts compare code
   point for code point. Every text of this page (`string[].text`, the `lines` elements, `text[].text`) is written as a
   TOML **basic string on one line, with escapes** (conventions §3.2), never as a literal or multi-line string, so
   no TOML parser can normalise its line breaks: a line break inside a text is `\n` (and `\r` where the text has
   one), a tab `\t`. Writers MUST write the controls U+0080–U+009F as `\u00XX` escapes (the English table has one
   U+0085, from a byte its map keeps), and the other control characters as TOML requires.
4. **Files in other encodings**, each decoded to UTF-8 with the original encoding recorded and the round trip
   checked at import: credits UTF-16LE with a byte-order mark (RFC 2781, https://www.rfc-editor.org/rfc/rfc2781);
   licence texts Windows-1252 (https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1252.TXT;
   **inferred** from ™ at 0x99, © at 0xA9 and the accented letters at their Windows-1252 places), UTF-8 with a
   byte-order mark (RFC 3629, https://www.rfc-editor.org/rfc/rfc3629) or UTF-16LE with one; save texts UTF-16LE.
   A file with an unpaired surrogate or a byte Windows-1252 leaves undefined (0x81, 0x8D, 0x8F, 0x90, 0x9D) is kept
   whole in the residual instead (none in the reference install).

### 3.4 Credits (`ob.credits/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.credits/1.0` |
| `op` | enum | – | no | `merge` | §4 |
| `bom` | bool | – | no | `true` | The original file starts with a UTF-16 byte-order mark. |
| `line_ending` | enum | – | no | `crlf` | `crlf` or `lf`. |
| `lines` | array<string ≤ 4,096> | – | yes | – | The lines, split at every line ending. A file that ends with a line ending has a last, empty element. These are exactly the lines the Credits screen rolls (`crates/ob-menus/src/frontend/screens/credits.rs:31`: split at LF, CR removed). |

The engine opens `CREDITS\%s%s.TXT` with a prefix and the upper-case language name (0x4B8808): in English `NA_`
for region 0 and `UK_` for a European region (`crates/ob-core/src/region.rs:20`, `crates/ob-menus/src/frontend/screens/credits.rs:20`), no prefix
in the other languages. In the format: `text/<language>/credits/<PREFIX><LANGUAGE>.toml`. A missing file gives an
empty roll, as today (`crates/ob-menus/src/frontend/screens/credits.rs:32`). Lines may hold U+00A0 (nine in the French file), kept
as is. Example: [`examples/text/text/english/credits/UK_ENGLISH.toml`](examples/text/text/english/credits/UK_ENGLISH.toml).

### 3.5 Licence texts (`ob.legal-text/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.legal-text/1.0` |
| `op` | enum | – | no | `merge` | refused for packs (§4) |
| `encoding` | enum | – | yes | – | `windows-1252`, `utf-8` or `utf-16le` |
| `bom` | bool | – | no | `false` | A byte-order mark starts the file. |
| `line_ending` | enum | – | no | `crlf` | |
| `lines` | array<string ≤ 4,096> | – | yes | – | As in credits; tabs are kept (24–25 per Western file). |

The engine does not read these files [inferred: they belong to the original's installer]; they are kept so a base
set is complete and the launcher can show the agreement in the player's language. `agree.eng` and `agree.usa` are
identical (measured) and both kept. Example:
[`examples/text/text/english/legal/agree.eng.toml`](examples/text/text/english/legal/agree.eng.toml).

### 3.6 Save library texts (`ob.memcard-text/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.memcard-text/1.0` |
| `op` | enum | – | no | `merge` | §4 |
| `locale` | string `[A-Z][A-Z_]{0,39}` | – | yes | – | `<LOCALE>` of `memcard/LOCALE_<LOCALE>.loc`. |
| `header` | bytes | – | no | `"hex:"` | The `LOCH` and `LOCI` blocks (92 bytes in every shipped file). **Open**; kept as they are. |
| `text` | array<table> | – | yes | – | The texts by index (the engine asks by index: 6 overwrite, 7 save done, … `crates/ob-menus/src/frontend/profiles/memcard_text.rs:11`). |
| `text[].text` | string ≤ 1,024 | – | yes | – | The text (UTF-16LE in the original); `%1` is the engine's placeholder. |
| `text[].extra_nul` | u8 ≤ 8 | – | no | 1 | Zero UTF-16 units after the terminating one (1 or 2 shipped). |

The `LOCL` block is derived: tag, size (to the end of the file), 0, count, one offset per text from the tag, then
the texts. The engine reads `LOCALE_ENGLISH` only and turns U+00A0 into spaces after reading
(`crates/ob-menus/src/frontend/profiles/memcard_text.rs:36`); the stored text keeps U+00A0. When the file or a text is missing the
engine uses its own built-in English texts (`:43`), as today. Example:
[`examples/text/text/english/memcard.toml`](examples/text/text/english/memcard.toml).

### 3.7 Subtitle cues (`ob.subtitles/1.0`)

The original file `SUBTITLES/<NAME>` (docs/formats/movies-replays.md, the "Subtitles" entry, **verified**; read
whole by 0x57DA20) is an array of 8-byte records, ended by a record whose time is 0xFFFF (0x57DB67). Record
`subtitle_cue`, little-endian:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `time` | **open** | When the label shows; 0xFFFF only in the end record. |
| 0x02 | 2 | `u16` | `u_0x02` | – | Not read; not zero (leftover bytes in the files), so kept for the rebuild. Not padding. |
| 0x04 | 4 | `u32` | `label_hash` | – | `bin_hash` of the label, looked up in the active language (§3.8). |

Size 8, alignment 4. The file is the cue records in order, then the end record; nothing follows it (all 22 files:
the rebuild of §5 gives every byte back).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.subtitles/1.0` |
| `op` | enum | – | no | `merge` | §4 |
| `cue` | array<table> | – | yes | – | The records before the end, in order (every shipped file's times ascend). |
| `cue[].time` | u16 < 0xFFFF | **open** | yes | – | When the label shows; the unit is not traced. |
| `cue[].label_hash` | hash32 | – | yes | – | The label of the text, looked up in the active language (§3.8); `0x001A20BA` is the empty string. |
| `cue[].u_0x02` | u16 | – | no | 0 | The unused word, kept for the rebuild. |
| `end` | table | – | yes | – | The closing record: `label_hash` (hash32) and `u_0x02` (u16, default 0). |

Example: [`examples/text/movies/subtitles/DemoScene.toml`](examples/text/movies/subtitles/DemoScene.toml).

### 3.8 Lookup and fallback (normative)

Let *L* be the active language, always one of role `player`. Which language the engine makes active is outside the
format: today it is `english` unless the player chooses another (`game --language <name>`, or `language.txt` kept by
the launcher; `qol::setup::choose_language`), and the original's rule is the registry string `Language`, a missing value being English (docs/original/tracing/language.md). The engine can show these
`player` languages: `english`, `french`, `german`, `italian`, `spanish`, `dutch`, `swedish` and `danish`
(`game_assets::language_support`; `spanish` and `italian` **observed**: the menus and text start in that language on
a ROG Ally Z1E; the others **inferred**: they share the one-byte character map of `english`, and no file of them was
tried). `korean` (§3.3 rule 2), `japanese`, `chinese` and `thailand`
(empty tables, §1) are listed as not supported; a chosen language whose file is missing is `english`. A lookup
of hash *h*:

1. If the merged table of *L* (the base set's *L* plus every pack's entries for *L*) has *h*, its entry: `text`,
   or `bytes` decoded as §3.3 says.
2. Else, if no base-set table of *L* defines *h* (a key a pack added, or *L* is a language a pack added) and the
   merged `english` table has *h*: that entry.
3. Else nothing: the engine's own behaviour for a missing label applies unchanged (a menu object draws its
   authored `St` text, [ui.md](ui.md) §3.1; `Strings::label` returns the name, `crates/ob-menus/src/frontend/feng/strings.rs:61`).

With the base set alone, step 2 never applies: every imported table has the same 3,925 keys (measured), so every
lookup is the original's. A pack's override of a key in *L* (step 1) changes only *L*. The English fallback serves
only strings packs add, and languages packs add. Fonts follow [ui.md](ui.md) §3.3; credits, save texts and licence
texts have no fallback (the engine's own rules above apply).

### 3.9 Language textures and language-only pictures

`LANGUAGES/LanguageTextures.bin` is one texture pack holding the glyph pages of every language's fonts; the original
loads it whatever the language [inferred: one path in the executable, no language in it]. The importer stores it
as one texture set of the `frontend/` unit ([textures.md](textures.md)), not split by language. The per-language
unit for pictures is `text/<language>/textures/`: texture sets a pack adds for one language (the glyph pages of
its fonts, translated menu art) and that load only with it. Imported base sets leave it empty. Texture keys there
resolve before the shared sets of the same hash while that language is active.

## 4. Layering

| Kind | `merge` | `add` | `replace` | `disabled` | Clash key |
|---|---|---|---|---|---|
| strings (per entry) | yes: `text` / `bytes` of a key the language has | yes: a new key; `label` required, `bin_hash(label)` = `hash` (`TXT.4`, CONV.15) | yes | refused (`MRG.3`): write `text = ""` | (language, hash) |
| `language` | `name` only | yes: a new language unit (role `player`) | refused (`MRG.3`) | refused (`MRG.3`) | (language, field) |
| `charmap.bin` | refused (`MRG.3`) | – | refused | – | – |
| credits | yes (fields; `lines` replaced whole) | yes: a file a language lacks | yes | refused | (language, file, field) |
| licence texts | refused (`MRG.3`) | refused | refused | refused | – |
| save texts | yes (`text` replaced whole) | yes: a new locale | yes | refused | (language, field) |
| subtitles | yes (`cue` replaced whole) | yes: cues for a movie a pack adds ([movies.md](movies.md)) | yes | refused | (name, field) |

- A pack's strings live in `text/<language>/strings.toml` of the pack; entries without `op` merge into an existing
  key. Added keys go after the base strings in layer order (layering.md §3.3), which is also their place when a
  table is written back in the original layout.
- No wildcard units: the English fallback of §3.8 serves what `text/_any/` would.
- A pack adding a language adds `text/<key>/language.toml` (`op = "add"`, `role = "player"`, no `charmap`), its
  strings as `text`, and optionally fonts and pictures. Its missing keys fall back to English.

## 5. Import

1. **Labels first**: `Labels.bin`'s table gives a name for every hash (all 3,925); each is checked against the hash
   (`bin_hash`, else `bin_hash` of the upper-case name).
2. **Each `LANGUAGES/<Lang>.bin`**: walk the chunks; 0x39000 → `strings.toml` (order kept, `label` set, text or
   bytes form per §3.3) and `charmap.bin` (the 6,148-byte region); 0x30201 → fonts ([ui.md](ui.md) §5); id-0 fill
   chunks → residual `fill` nodes. Encode the table again (§3.2) and compare with the chunk (Eq2).
3. `CREDITS/*.TXT`, `LANGUAGES/agree*`, `memcard/LOCALE_*.loc` → their kinds; each file is rebuilt from the decoded
   form and compared. `SUBTITLES/*` → `movies/subtitles/<NAME>.toml`, rebuilt and compared.
4. `language.toml` for every key of §2; the base set's manifest lists the `player` ones (`languages`,
   [manifest.md](manifest.md) §3).

A file that fails a rebuild is kept whole in the residual and reported (`TXT.11`); none fails in the reference
install.

## 6. Equality and verification

| Item | Level | Proof at import | Shadow |
|---|---|---|---|
| string table | Eq2, hence Eq1 (`Strings` map) | the table encoder rebuilds the 0x39000 chunk; a prototype rebuilt all 14 byte for byte (13 in text form, Korean in byte form) | none |
| credits | Eq2, hence Eq1 (the rolled lines) | rebuild of the file; all 12 | none |
| licence texts | Eq2 | rebuild; all 11 | none |
| save texts | Eq2, hence Eq1 (`parse_loc`) | rebuild; all 17 | none |
| subtitle cues | Eq2 | rebuild; all 22 | none |

The Eq1 check hashes the parser's `Strings` map for every imported language (not only English) from the original
file and from the base set. Gates ([parity.md](parity.md) §3): every menu, HUD and pause shot (they draw the
English strings), the Credits screen shot, the save dialogs' texts.

## 7. Cooked form

The active language's strings are a section of [ui.md](ui.md)'s cook unit `ui-<language>`: `strings`, the map hash →
`String` as the parser gives it (byte-form strings already decoded), after layering and the fallback of §3.8
(fallback entries are materialised, so the engine's lookup stays one map access). Credits, save texts, subtitle
cues and licence texts are not cooked: they are small and read from their bundle entry when a screen needs them.
Nothing here is build-specific.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| TXT.1 | A language key that breaks `[a-z][a-z0-9_]{0,31}`, or a `text/<language>/` unit without `language.toml`. |
| TXT.2 | Two entries with one `hash` in a `strings.toml`. |
| TXT.3 | An entry with both `text` and `bytes`, or `bytes` holding a zero byte. |
| TXT.4 | An added string without `label`, or with `bin_hash(label)` ≠ `hash`; an imported `label` matching neither `bin_hash(label)` nor that of its upper-case form. |
| TXT.5 | A text holding U+0000, or longer than its limit. |
| TXT.6 | A `charmap` that is not exactly one `charmap_region` (6,148 bytes). |
| TXT.7 | A `lines` element holding CR or LF; a licence text with a character its `encoding` cannot write. |
| TXT.8 | Save texts: more than 256 entries. |
| TXT.9 | A subtitle cue with time 0xFFFF (reserved for the end record). |
| TXT.10 | A pack changing a base language's `role` or `charmap`, or adding a language with `role` other than `player`. |
| TXT.11 | (import) A file whose rebuild differs from the original: kept whole in the residual and reported. |
| TXT.12 | A language of role `player` without `strings.toml` (after merging the stack), or one of role `save_text` with a `strings.toml`, `charmap` or `fonts`. |
<!-- /generated -->

Warning (Studio, never an error): a `player` language lacking keys the English table has (shown as untranslated;
the fallback covers them).

## 9. Versioning and extension

- Minor versions add optional fields: a name for `word0` and the Korean decoding once traced; a font substitution
  table per language (`ob.language/1.1`, e.g. `font_substitutes`), once the original's substitution
  ([ui.md](ui.md) §10) is traced; the subtitle time unit.
- Possible extensions (`OB_` prefix): text direction and shaping for scripts the original's fonts cannot show;
  plural forms for texts packs add. Neither changes native lookups.

## 10. Open points

- **The language entry point**: traced, see docs/original/tracing/language.md (the registry string `Language`; a
  missing value is English). Open: what the installer writes there for each retail edition; whether `Thailand.bin`,
  `Labels.bin` and `Largest.bin` are ever loaded (no path names them). Overbright keeps English unless the player
  chooses another language (§3.8).
- The Korean character map (`word0` = 0x3D9): its layout and decoding. Its strings stay in byte form.
- The Japanese, Chinese and Thai tables are empty in this edition: what the original shows with them.
- Subtitles: the time unit and the stepping (0x576470) are not traced; this edition never starts them.
- Save texts: the `LOCI` block's meaning; why some texts end with two extra zero units.
- The shared text order of all 14 tables: its source (an authoring order) is unknown; it is kept.
- Licence texts: who shows them in the original [inferred: the installer]; the Windows-1252 reading is
  **inferred**, the bytes are proven either way.

## 11. Sizes and timing

| Kind | Original (reference install) | Base set (source form) |
|---|---|---|
| string tables | 14 chunks, 2,766,340 bytes (in 14 files of 3,142,288 bytes with their fonts) | 14 `strings.toml` ≈ 6.2 MB [estimate: ≈ 0.48–0.54 MB per language with labels, ≈ 0.27 MB for an empty one] + 14 × 6,148 bytes of `charmap.bin` |
| credits | 12 files, 465,556 bytes | ≈ 0.28 MB [estimate] |
| licence texts | 11 files, 522,076 bytes | ≈ 0.6 MB [estimate] |
| save texts | 17 files, 12,308 bytes | ≈ 15 KB [guess] |
| subtitle cues | 22 files, 1,456 bytes | ≈ 8 KB [guess] |

Import of everything on this page: < 0.3 s [guess] (a Python prototype decodes and rebuilds all of it in about a
second). Loading the active language's strings from source ≈ 10–20 ms [guess]; from the cooked unit, a map read.
