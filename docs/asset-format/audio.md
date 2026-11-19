# Asset format: audio (sound banks, engine recordings, speech, music, mix maps, reverb presets)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

The original's audio is three EA codecs (EA-XA, MicroTalk, EALayer3), a granular engine-recording format and a few
small binary settings files. Every original bitstream is kept **as it is** (codec passthrough) in a documented
framing and decoded by Overbright's own decoders; the settings files become TOML. New content (mod packs, a
community base set) is written in open formats: FLAC (RFC 9639, https://www.rfc-editor.org/rfc/rfc9639) now, Ogg
Vorbis and Opus in a later minor version (§9). The one passthrough form packs may also use is an engine recording
in the original GIN format (§3.3), because the community's engine-sound tools produce GIN: the kind-page exception
that [conventions.md](conventions.md) §4.6 allows.

Schemas: [`sound-bank`](schemas/sound-bank.schema.json), [`engine-recording`](schemas/engine-recording.schema.json),
[`speech-bank`](schemas/speech-bank.schema.json), [`music-map`](schemas/music-map.schema.json),
[`mix-map`](schemas/mix-map.schema.json), [`reverb-preset`](schemas/reverb-preset.schema.json). Examples:
[`examples/audio/`](examples/audio/).

## 1. Scope

| Original | Files (reference install) | Kind here | Engine reader today |
|---|---|---|---|
| `SOUND/**/*.abk` ("ABKC" banks: AEMS event modules + a "BNKl" bank of EA-XA samples) | 176 files, 16,701,756 bytes | `ob.sound-bank` | `src/audio/banks/abk.rs:99`, `src/audio/aems/program.rs:20` |
| `SOUND/ENGINE/GIN_*.gin` and the other `*.gin` ("Gnsu20" granular recordings) | 84 files, 5,642,017 bytes | `ob.engine-recording` | `src/audio/gin.rs:33` |
| `SDATA/sdat.viv` → `speech/UG2_Speech.{csi,evt,idx,big}`, `speech/nisaudio.{csi,evt,idx,big}` | 8 entries, 133,603,712 bytes | `ob.speech-bank` | `src/audio/speech/bank.rs:340` |
| `SDATA/sdat.viv` → `pfdata/MusicSFx.{mpf,mus}` (EA Trax) | 2 entries, 125,880,248 bytes | `ob.music-map` | `src/audio/trax/mpf.rs:21`, `src/audio/streams/mod.rs:141` |
| `SOUND/MIXMAPS/*.mxb` | 4 files, 21,280 bytes | `ob.mix-map` | `src/audio/mixmap/parse.rs:75` |
| `SOUND/FXEDIT/*.fx` | 12 files, 2,824 bytes | `ob.reverb-preset` | `src/audio/reverb/graph.rs:100` |

Not on this page:

- The engine-sound records, engine choice per car, car-audio curves and every other number built into the engine
  (`crates/ob-formats/src/exe_tables/constants/car_audio.rs`): [engine-tables.md](engine-tables.md). They refer to items here by key.
- Movie soundtracks (inside `MOVIES/*.vp6`): [movies.md](movies.md). Song titles, artists and albums shown in the
  chyron: engine constants built into the engine (`crates/ob-formats/src/exe_tables/constants/frontend.rs`, [engine-tables.md](engine-tables.md)). Option volumes: native profile saves (docs/formats/saves-options.md).
- `SOUND/EVT_SYS/*.csi` (8 files, 1,180 bytes): "MOIR" switch tables no engine code reads. The importer keeps
  them whole in the residual (`fate = "kept"`, reason "not read by the engine").
- The collision-audio stitch records (chunk 0x8003B500 in front-end and in-game bundles): not yet owned by a kind
  and kept in the residual ([README.md](README.md) §3, "Not yet owned by a kind").
- The BIG4 archive itself (header, directory, 64-byte alignment fill): a `viv` container in the residual map
  ([residual.md](residual.md) §3.1); its entries become the kinds above. The original's rule that a file in the
  archive wins over a loose file of the same name (`crates/ob-formats/src/formats/viv.rs:24-30`) has no counterpart: a base set has
  one copy of each item, and packs override through layering.

## 2. Files and keys

### 2.1 Sub-units and bundles

`audio/` is split into sub-units by when the engine loads them; each is one bundle ([layout.md](layout.md) §5):

| Sub-unit | Bundle | Holds (from) | Base size (reference) |
|---|---|---|---|
| `audio/common/` | `audio-common.obz` | mix maps (`MIXMAPS`), reverb presets (`FXEDIT`) | ≈ 0.2 MB [guess: TOML] |
| `audio/frontend/` | `audio-frontend.obz` | banks of `FE/`, `GLOBAL/` | 1.38 MB |
| `audio/ingame/` | `audio-ingame.obz` | banks of `IG_GLOBAL/`, `SKIDS/`, `NOS/`, `TURBO/`, `SHIFTING/` | 4.47 MB |
| `audio/engine/` | `audio-engine.obz` | banks of `ENGINE/` (EE, SPU, SWTN), engine recordings | 16.6 MB |
| `audio/speech/` | `audio-speech.obz` | speech banks `UG2_Speech`, `nisaudio` | 133.7 MB |
| `audio/music/` | `audio-music.obz` | the music map `MusicSFx` and its streams | 125.9 MB |

Folder names are this specification's (lower case); original names inside keep their spelling.

### 2.2 Example tree

```text
audio/common/mixmaps/MAPOUTPUT.toml               ob.mix-map
audio/common/reverb/Garage.toml                   ob.reverb-preset
audio/frontend/banks/FE/FE_MB.toml                ob.sound-bank (index)
audio/frontend/banks/FE/FE_MB.bin                 the bank image (layout abk_image)
audio/ingame/banks/IG_GLOBAL/ENV_COMMON_MB.toml
audio/ingame/banks/IG_GLOBAL/ENV_COMMON_MB.bin
audio/engine/banks/ENGINE/CAR_00_ENG_MB_EE.toml
audio/engine/banks/ENGINE/CAR_00_ENG_MB_EE.bin
audio/engine/recordings/GIN_Eclipse_5Zigen.toml   ob.engine-recording
audio/engine/recordings/GIN_Eclipse_5Zigen.bin    the recording (layout gin_file)
audio/speech/UG2_Speech/bank.toml                 ob.speech-bank
audio/speech/UG2_Speech/lines.bin                 the line streams (layout ea_stream_set)
audio/music/MusicSFx/map.toml                     ob.music-map
audio/music/MusicSFx/streams.bin                  the music streams (layout ea_stream_set)
```

A mod pack adds its own payloads next to its TOML files (`audio/ingame/banks/IG_GLOBAL/sounds/rain.flac`).

### 2.3 Keys

| Short name | Key | Example | Item |
|---|---|---|---|
| `sound-bank` | `<DIR>/<STEM>`: the original folder under `SOUND/` and the file name without `.abk` | `sound-bank:IG_GLOBAL/ENV_COMMON_MB` | one bank |
| `sound` | `<bank key>/<slot>`: the sample's index in the bank's BNKl table, decimal | `sound:FE/FE_MB/12` | one sample |
| `sound-event` | `<bank key>/<EVENT>`: the event name as the bank stores it | `sound-event:FE/FE_MB/FrontEndSample` | one event (module) |
| `engine-recording` | the file name without `.gin` | `engine-recording:GIN_Eclipse_5Zigen` | one recording |
| `speech-bank` | the bank name | `speech-bank:UG2_Speech` | one bank |
| `speech-line` | `<bank>/<entry>/<record>`: positions in the bank's entry list and the entry's record list | `speech-line:UG2_Speech/3/17` | one line (stream) |
| `speech-event` | `<bank>/<id>`: the event id, `0x` + 4 upper-case hex digits | `speech-event:nisaudio/0x4023` | one event |
| `music-map` | the map name | `music-map:MusicSFx` | the EA Trax map |
| `music-stream` | `<map>/<index>`: position in the map's stream table | `music-stream:MusicSFx/38` | one stream |
| `music-event` | `<map>/<index>`: position in the event table; event *i* < 27 is song *i* | `music-event:MusicSFx/0` | one event / song |
| `mix-map` | the file name without `.mxb` | `mix-map:MAPOUTPUTDRG` | one mix map |
| `reverb-preset` | the file name without `.fx` | `reverb-preset:Simple_Tunnel` | one preset |

- Positions (slot, entry, record, stream, event) are the original order ([conventions.md](conventions.md) §6.5) and
  never change: the engine selects by them (AEMS sound lists name slots; `.idx` records are picked by index).
- **Case.** The engine finds these files without regard to case, as the original's file layer does (the engine
  tables spell `GIN_Eclipse_5Zigen.gin` differently from the install, docs/formats/audio.md "Engine choice";
  bank paths go through `game_fs`). So `sound-bank`, `engine-recording`, `mix-map` and `reverb-preset` keys are
  file-name keys ([conventions.md](conventions.md) §6.3): a reference resolves by ASCII case-insensitive
  comparison, and the layer stack MUST NOT hold two keys of one of these kinds that differ only in case
  (`CONV.13`). Keys are written in the install's spelling.
- Items a pack adds: engine recordings by a new name (`engine-recording:MyCar_V8`); nothing else can be added in
  1.0 (§4).

## 3. Source form

### 3.1 Payload framings (binary, passthrough)

Every original audio payload is stored byte for byte as the original holds it. These layouts are the documented
framings; the meaning of each structure is in docs/formats/audio.md (sections named per row).

| Layout | Element | What the array holds | Read by |
|---|---|---|---|
| `abk_image` | `u8` | one whole ABKC bank file (§3.1.1) | AEMS programs, sample decode |
| `gin_file` | `u8` | one whole Gnsu20 file (§3.1.2) | the granular player |
| `ea_stream` | `u8` | one EA stream: chunks `SCHl`, `SCCl`, `SCDl`…, optional `SCLl`, `SCEl` (§3.1.3) | stream decoders |
| `ea_stream_set` | `u8` | EA streams concatenated, each at the offset the index gives, zero bytes between | speech and music |

Why the original framing and not a new container: the engine reads the bank as one byte image addressed by file
offsets (AEMS templates hold file offsets the loader relocates, `src/audio/aems/program.rs:8-10`; sound lists sit
at offsets inside module templates, `src/audio/banks/abk.rs:188-203`), and the stream decoders consume EA's block
framing (per-block sample counts, per-channel offset tables, MicroTalk's per-frame markers, EALayer3's 0xEE
tails). A RIFF/WAVE wrapper (`WAVE_FORMAT_EXTENSIBLE` with an own SubFormat GUID,
https://learn.microsoft.com/en-us/windows/win32/api/mmreg/ns-mmreg-waveformatextensible) would have to carry the
same blocks in its `data` chunk and would still be playable by no common tool: FFmpeg 8.1 decodes the NIS bank's
EA-XA and refuses MicroTalk ("revision2=22 is not implemented") and EALayer3 ("revision2=23") (measured on the
reference install). Listening and editing go through exports (decoded WAV, `overbright-studio-cli audio export`),
not through the stored form.

#### 3.1.1 `abk_image` (one ABKC bank)

Little-endian. The header, as far as it is read or measured (176 files):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u8[4]` | `magic` | – | `ABKC` |
| 0x04 | 2 | `u16` | `u_0x04` | – | **open**; 0x0101 in every file |
| 0x06 | 2 | `u16` | `u_0x06` | – | **open**; 1, 0x300 or 0x400 |
| 0x08 | 2 | `u16` | `pad` | – | zero in every file |
| 0x0A | 2 | `u16` | `module_count` | – | 1 or 2 |
| 0x0C | 8 | `u8[8]` | `pad` | – | zero in every file |
| 0x14 | 4 | `u32` | `file_size` | byte | equals the file's size (all 176) |
| 0x18 | 4 | `u32` | `bnk_offset` | byte | the BNKl table |
| 0x1C | 4 | `u32` | `first_module` | byte | 0x5C in every file |
| 0x20 | 4 | `u32` | `u_0x20` | – | **open** |
| 0x24 | 4 | `u32` | `u_0x24` | – | **open** |
| 0x28 | 8 | `u8[8]` | `pad` | – | zero in every file |
| 0x30 | 4 | `u32` | `call_relocations` | byte | count, then offsets patched to the node table |
| 0x34 | 4 | `u32` | `pointer_relocations` | byte | count, then offsets that get the bank's base |
| 0x38 | 4 | `u32` | `events` | byte | count, then 12-byte entries {record + 4, name block, type} |
| 0x3C | 4 | `u32` | `pad` | – | zero in every file |

Size 0x40, alignment 4. Then module records (0x3C bytes + 4 × (players + sub-event slots)), their code and
templates, the BNKl table (`BNKl`, `u16` 5, `u16` sample count, `u32`, 8 zero bytes, a `u32` offset per slot,
then per slot `PT\0\0` and tagged big-endian values: 0x82 channels, 0x84 rate, 0x85 frames, 0x86 / 0x87 loop,
0x88 data offset from the BNKl, 0xA0 codec), the EA-XA sample data (15-byte blocks of 28 samples, 61-byte raw
blocks starting 0xEE), and last the relocation and event tables. All 1,689 slots of the reference install are
codec 10 (EA-XA), mono; 15 slots share or overlap another slot's data; no sample's data starts at a multiple of
16 (measured). docs/formats/audio.md: "ABKC banks", "AEMS event graphs", "BNKl", "Codec 10, EA-XA".

The image is not split: the bytes the engine reads at file offsets (code, templates, sound lists, relocation
targets) and the samples interleave, slots share data, and samples are never 16-aligned (which external array
descriptors require, [conventions.md](conventions.md) §4.4). Samples are addressed inside the image by plain
offset/size fields of the index (§3.2).

#### 3.1.2 `gin_file` (one Gnsu20 recording)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u8[8]` | `magic` | – | `Gnsu20` and two zero bytes (all 84 files) |
| 0x08 | 4 | `f32` | `min_rpm` | rpm | |
| 0x0C | 4 | `f32` | `max_rpm` | rpm | |
| 0x10 | 4 | `u32` | `rpm_steps` | – | N; 50 in every file |
| 0x14 | 4 | `u32` | `cycle_count` | – | M; 18–265 |
| 0x18 | 4 | `u32` | `frames` | sample | |
| 0x1C | 4 | `u32` | `rate_hz` | Hz | 20,000–36,000 |
| 0x20 | 4·(N+1) | `u32[N+1]` | `rpm_positions` | sample | rpm → position table |
| … | 4·(M+1) | `u32[M+1]` | `cycle_starts` | sample | |
| … | 19·⌈frames/32⌉ | – | `blocks` | – | 19-byte blocks of 32 samples, nothing after the last (all 84) |

Alignment 4. Block decoding and the player: docs/formats/audio.md "GIN".

#### 3.1.3 `ea_stream`

Chunks of `{u8[4] tag, u32 size}` (little-endian, the size counting the 8-byte head), in this order: one `SCHl`
(`PT\0\0` then tagged values as in BNKl: 0x80 version, 0x82 channels, 0x84 rate, 0x85 frames, 0xA0 codec; or, in
movie soundtracks only, `GSTR`: [movies.md](movies.md)), one `SCCl` (block count), the `SCDl` blocks (frames in
the block, one `u32` per channel: the channel's data offset, then the codec's data), an optional `SCLl` (loop), one
`SCEl`. Codec per stream: 10 EA-XA, 4 / 0x16 MicroTalk (0x16 with version ≥ 2 has per-frame markers), 0x17
EALayer3. docs/formats/audio.md "Speech and music archive"; `src/audio/codec/schl.rs:1-16`.

Measured streams of the reference install (all through `SCEl`; block frame counts sum to the header's in every
stream):

| Bank / map | Streams | Codec | Channels, rate | Bytes | Duration |
|---|---|---|---|---|---|
| `UG2_Speech` | 857 | 0x16 MicroTalk, version 2 | 1, 22,050 Hz | 56,712,060 | 6,318 s |
| `nisaudio` | 144 + 7 | 10 EA-XA | 2 / 1, 32,000 Hz | 76,639,972 | 2,223 s |
| `MusicSFx` | 43 | 0x17 EALayer3 | 2, 32,000 Hz | 125,870,600 | 7,722 s |

### 3.2 Sound bank (`ob.sound-bank/1.0`)

One file per bank: the index of what the image holds, and in mod packs the bank's overrides.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.sound-bank/1.0` |
| `bank` | string | – | yes | – | The bank key (`<DIR>/<STEM>`); the file's path MUST be `banks/<bank>.toml`. |
| `image` | array descriptor | – | base: yes; mod: no | – | The bank image, layout `abk_image`, offset 0, `count` = the original file's size. Mod packs MUST NOT have it in 1.0 (`MRG.3`). |
| `module_count` | u16 | – | if `image` | – | The image's `module_count` (checked). |
| `event` | array<table> | – | if `image` | – | The image's event table in file order (3.2.1). |
| `sound` | array<table> | – | yes | – | Base: one per BNKl slot in slot order (3.2.2). Mod: overrides, keyed by `slot` (3.2.3). |
| `extras` | table | – | no | – | Conventions §9. |

#### 3.2.1 `event` (index; base only)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | string | – | yes | – | The event name (≤ 63 characters) as the image stores it. |
| `module` | u16 | – | yes | – | The module the event starts (its position among the module records). |
| `kind` | u32 | – | yes | – | The entry's third word ("type"; **open**). |

#### 3.2.2 `sound` (index; base)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `slot` | u16 | – | yes | – | Position in the BNKl table. |
| `present` | bool | – | no | `true` | `false` for a slot whose offset is 0 or whose header is not `PT` (the engine treats it as absent); then only `slot` is given. |
| `codec` | enum | – | yes | – | `ea_xa` (tag 0xA0 = 10, also the default when absent). |
| `channels` | u8 | – | yes | – | Tag 0x82 (default 1). |
| `rate_hz` | u32 | Hz | yes | – | Tag 0x84 (default 22,050); 100–48,000 in the reference install. |
| `frames` | u32 | sample | yes | – | Tag 0x85. |
| `loop_start` | i32 | sample | no | – | Tag 0x86, when present. |
| `loop_length` | i32 | sample | no | – | Tag 0x87, when present. |
| `data_offset` | u32 | byte | yes | – | Where the sample's data starts, from the image's start (BNKl offset + tag 0x88). |
| `data_size` | u32 | byte | yes | – | Bytes the decoder reads for `frames` samples. |

The index carries no fact the image does not hold; it exists so tools, Studio and keys work without parsing the
image. A reader MUST check it against the image (`AUD.2`).

#### 3.2.3 `sound` (override; mod packs)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `slot` | u16 | – | yes | – | The slot overridden (the list's key). |
| `op` | enum | – | no | `merge` | layering.md §3.1: `replace` (new audio for the slot) or `merge` (only `loop` and `disabled`); `add` is refused in 1.0 (`MRG.3`). |
| `disabled` | bool | – | no | `false` | The slot plays silence for its original length (the voice still starts and ends as before). |
| `source` | table | – | if `replace` | – | `file` (path relative to this TOML, no `..`), `format` (`flac`), optional `digest`. |
| `loop` | bool | – | no | `false` | Whether the voice loops the whole replacement. |

Absent fields follow layering.md §3.1: under `replace` an absent `loop` is `false`, the schema default, not the
original slot's looping (tags 0x86/0x87), so a replacement for a looping slot says `loop = true`; under `merge` an
absent `loop` keeps the lower layer's value.

A replacement MUST be mono (bank voices are mono, `src/audio/banks/abk.rs:208`) and follow the FLAC profile
(§3.8). Its frames reach the voice as `f32` at 16-bit scale, as the EA-XA decoder delivers them: a 16-bit sample
`s` becomes `s as f32`, a 24-bit sample `s / 256` (exact in `f32`).

Example (base index, shortened):

```toml
schema = "ob.sound-bank/1.0"
bank = "IG_GLOBAL/ENV_COMMON_MB"
image = { file = "ENV_COMMON_MB.bin", offset = 0, count = 215768, layout = "abk_image", digest = "xxh3-128:3f0c9a51d27e4b86a1c05e7d92b4f318" }
module_count = 1

[[event]]
name = "FX_ENV_COMMON"
module = 0
kind = 0

[[sound]]
slot = 0
codec = "ea_xa"
channels = 1
rate_hz = 22050
frames = 44100
loop_start = 0
loop_length = 44100
data_offset = 0x2A44
data_size = 23625

[[sound]]
slot = 1
present = false
```

Example (mod pack override):

```toml
schema = "ob.sound-bank/1.0"
bank = "IG_GLOBAL/FX_Rain_MB"

[[sound]]
slot = 2
op = "replace"
source = { file = "sounds/rain_loop.flac", format = "flac" }
loop = true

[[sound]]
slot = 5
disabled = true
```

### 3.3 Engine recording (`ob.engine-recording/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.engine-recording/1.0` |
| `op` | enum | – | no | `merge` | layering.md §3.1. Packs: `add` (a new name) or `replace`; `merge` is refused (`MRG.3`, §4), so a pack's file always names its `op`. Base sets: absent. |
| `name` | string | – | yes | – | The key: the original file name without `.gin`, or a pack's new name (`[A-Za-z0-9_]{1,63}`). |
| `format` | enum | – | yes | – | `gin` (the original format, passthrough) or `flac` (16-bit PCM, §3.8). Packs may use either (below). |
| `data` | array descriptor | – | if `gin` | – | Layout `gin_file`, offset 0, the whole GIN file. |
| `source` | table | – | if `flac` | – | `file`, `format = "flac"`, optional `digest`. |
| `min_rpm` | f32 | rpm | yes | – | Lowest rpm of the sweep. |
| `max_rpm` | f32 | rpm | yes | – | Highest rpm; MUST be greater than `min_rpm`. |
| `rate_hz` | u32 | Hz | yes | – | Sample rate, 8,000–96,000. |
| `frames` | u32 | sample | yes | – | Samples in the recording. |
| `rpm_positions` | array<u32> | sample | yes | – | N + 1 positions for N equal rpm steps from `min_rpm` to `max_rpm` (51 in the original files); non-decreasing, each < `frames`. |
| `cycle_starts` | array<u32> | sample | yes | – | M + 1 cycle starts, M ≥ 2; strictly increasing, last ≤ `frames`. |

For `gin`, every field after `data` repeats the file's header and tables and is checked against them (`AUD.5`);
the engine reads the file. For `flac`, the fields are the recording's tables and the FLAC holds its samples; the
granular player reads them exactly as it reads a decoded GIN (`src/audio/gin.rs:86-96`: 16-bit samples).
Up to 266 table entries per file in the original set, so the tables stay inline.

**`gin` in packs.** A pack MAY add or replace a recording in the `gin` format. This is the passthrough exception a
kind page may grant under [conventions.md](conventions.md) §4.6: the community's existing engine-sound tools
produce GIN files, and a GIN file holds the recording exactly as the granular player consumes it. Every other new
audio of this page (bank sounds, speech lines, music streams) is FLAC (§3.8); a pack's `gin` file follows §3.1.2
and `AUD.5` like an imported one.

```toml
schema = "ob.engine-recording/1.0"
name = "GIN_Eclipse_5Zigen"
format = "gin"
data = { file = "GIN_Eclipse_5Zigen.bin", offset = 0, count = 70318, layout = "gin_file" }
min_rpm = 1100.0
max_rpm = 8200.5
rate_hz = 32000
frames = 113280
rpm_positions = [0, 1520, 3010, 4480]        # shortened: 51 entries in an original file
cycle_starts = [0, 1290, 2556, 3800, 5022]   # shortened
```

### 3.4 Speech bank (`ob.speech-bank/1.0`)

One folder per bank (`audio/speech/<bank>/`): `bank.toml` holds what the `.csi`, `.evt` and `.idx` hold as the
engine reads them (`src/audio/speech/bank.rs:51-335`); `lines.bin` is the `.big` (layout `ea_stream_set`).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.speech-bank/1.0` |
| `bank` | name | – | yes | – | `UG2_Speech`, `nisaudio`; equals the folder name. |
| `language` | string | – | no | – | The install's language the bank was imported from (`english`); informative. |
| `lines` | array descriptor | – | base: yes | – | Layout `ea_stream_set`, offset 0. |
| `bank_keys` | array<u8> | – | base: yes | – | The `.evt`'s two bank key bytes (+8, +9). |
| `switch` | array<table> | – | base: yes | – | `.csi` switches in file order: `name` (string ≤ 63), `id` (u32: high word from the record, low word the bank's id). |
| `event` | array<table> | – | base: yes | – | `.evt` events in table order (3.4.1). |
| `entry` | array<table> | – | base: yes | – | `.idx` entries in table order (3.4.2). |
| `line` | array<table> | – | mod: yes | – | Line overrides (3.4.3). |

#### 3.4.1 `event`

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `id` | u16 | – | yes | – | Event id. |
| `switch` | string | – | no | – | The switch-table name that fires it (switch *i* fires event *i*). |
| `lifetime` | u16 | – | yes | – | +2 (the library's queue; **inferred**). |
| `priority` | u16 | – | yes | – | +4. |
| `keys` | u8 | – | yes | – | +7. |
| `limits` | u8 | – | yes | – | +8: low nibble the channel count limit, high nibble the switch-mask count. |
| `chance` | i8 | – | yes | – | +9: the request passes when Rand(100) ≤ it. |
| `flags` | u8 | – | yes | – | +0xA. |
| `values` | u8 | – | yes | – | +0xB: request values copied from the switch's tuple. |
| `alternative` | array<table> | – | yes | – | In order: `weight` (u8), `chance` (u8), `mode` (u8, 0–3), `state` (u8), `step` (array: `entry` u16, `entry_component` u8, `entry_mode` u8, `constraint` array of {`mask` u32, `component` u8, `slot` u8}). |

#### 3.4.2 `entry`

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `id` | u16 | – | yes | – | Entry id. |
| `selector` | u16 | – | yes | – | +2: paired with the id by mode-1 steps. |
| `base` | u32 | byte | yes | – | The entry's base in `lines`. |
| `unit_bytes` | u32 | byte | yes | – | (+7 + 1) × 256. |
| `end_units` | u16 | – | yes | – | +8: the end of the last record, in units. |
| `history` | u8 | – | yes | – | +6: depth of the recently-played ring. |
| `record` | array<table> | – | yes | – | `offset_units` (u16) and `keys` (array<u8>, one per key of the entry; all records of an entry have the same count). |
| `unions` | array<u32> | – | yes | – | Per key, the union of the records' values. |
| `enabled` | bytes | – | no | – | One enable bit per record (present when +4 bit 7 is set). |
| `ring` | table | – | yes | – | `position` (u8) and `slots` (array<u8>, `history` entries; 0xFF unused). |

Line *r* of an entry is `lines[base + offset_units[r] · unit_bytes, base + next · unit_bytes)`, `next` being the next
record's offset or `end_units`; each line is one `ea_stream` (`AUD.6`).

#### 3.4.3 `line` (override; mod packs)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `entry` | u32 | – | yes | – | Entry position. |
| `record` | u32 | – | yes | – | Record position. Key: (`entry`, `record`). |
| `op` | enum | – | **yes** | – | `replace` (new audio for the line) or `merge` (only `disabled`). Required (`CONV.5` when absent): this list keeps layering.md §3.1's default (`merge`) like every other, and an override meant as a replacement must never be read as a merge because its `op` was left out. `add` is refused (`MRG.3`). |
| `disabled` | bool | – | no | `false` | The line plays silence for its original length; selection is unchanged. |
| `source` | table | – | if `replace` and not disabled | – | `file`, `format = "flac"`; mono or stereo. Decoded to `f32` in −1..1: `s / 32768` (16-bit), `s / 8388608` (24-bit), as the stream decoders deliver. |

Selection (which line is picked) never changes: it runs on the original records and keys. Silencing a line is
`op = "merge"` with `disabled = true`; `disabled = false` in a higher layer brings it back.

```toml
schema = "ob.speech-bank/1.0"
bank = "nisaudio"
language = "english"
lines = { file = "lines.bin", offset = 0, count = 76759808, layout = "ea_stream_set" }
bank_keys = [0, 0]

[[switch]]
name = "NIS_Select_End"
id = 0x48881A4B

[[event]]
id = 0x4011
switch = "NIS_Select_End"
lifetime = 0
priority = 0
keys = 0
limits = 0
chance = 100
flags = 0
values = 3

[[event.alternative]]
weight = 1
chance = 100
mode = 0
state = 0

[[event.alternative.step]]
entry = 0x4024
entry_component = 0
entry_mode = 0
constraint = [ { mask = 0x0000000F, component = 1, slot = 0 } ]

[[entry]]
id = 0x4024
selector = 0
base = 0
unit_bytes = 256
end_units = 9000
history = 0
unions = [0x0000000F]
ring = { position = 0, slots = [] }
record = [ { offset_units = 0, keys = [0] }, { offset_units = 4200, keys = [1] } ]
```

### 3.5 Music map (`ob.music-map/1.0`)

`audio/music/<map>/map.toml` holds what the engine reads from the `.mpf` (`src/audio/trax/mpf.rs:21-53`);
`streams.bin` is the `.mus` (layout `ea_stream_set`).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.music-map/1.0` |
| `map` | name | – | yes | – | `MusicSFx`; equals the folder name. |
| `streams_data` | array descriptor | – | base: yes | – | Layout `ea_stream_set`, offset 0. |
| `node` | array<table> | – | base: yes | – | In order: `kind` (u8: the type, dword +4 >> 24), `stream_id` (u16, +0: 1-based stream id for kind 4; 0 or 0xFFFF none), `next` (u16, +0x12). |
| `event` | array<table> | – | base: yes | – | In order: `start_node` (u16, record +0x1C). |
| `stream` | array<table> | – | yes | – | Base: bank 0's stream table in order: `offset` (u64 byte offset in `streams_data`, a multiple of 128), `frames` (u32, a multiple of 32). Mod: overrides keyed by `index` (below). |

Stream override (mod packs): `index` (u32, the key), `op` (**required**, as for speech lines, §3.4.3: `replace`
for new audio, `merge` for `disabled` only; `add` is refused, `MRG.3`), `disabled` (the stream counts as ended at
once: a song is skipped, the playlist goes on; the streamer-safe switch), `source` (`file`, `format = "flac"`;
stereo or mono; decoded as speech lines are; required with `replace` unless disabled).

```toml
schema = "ob.music-map/1.0"
map = "MusicSFx"
streams_data = { file = "streams.bin", offset = 0, count = 125873408, layout = "ea_stream_set" }
node = [ { kind = 1, stream_id = 0, next = 2 }, { kind = 0, stream_id = 0, next = 0 }, { kind = 4, stream_id = 39, next = 0 } ]
event = [ { start_node = 0 } ]
stream = [ { offset = 0, frames = 11955200 }, { offset = 2930688, frames = 6343680 } ]
```

### 3.6 Mix map (`ob.mix-map/1.0`)

The words the engine reads (`src/audio/mixmap/parse.rs:75-155`), raw, per section in file order. Values are kept
as the original's `u32` words; their bit fields are documented per field.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.mix-map/1.0` |
| `op` | enum | – | no | `merge` | layering.md §3.1. Packs replace a whole map: `op = "replace"`; `merge` (the default) and `add` are refused (`MRG.3`, §4), so a pack's file always says `replace`. Base sets: absent. |
| `name` | string | – | yes | – | `MAPOUTPUT`, `MAPOUTPUTDFT`, `MAPOUTPUTDRG`, `MAPOUTPUTURL`. |
| `section` | array<table> | – | yes | – | ≤ 32 sections in file order (9 in each original file). |
| `section.control` | array<table> | – | no | `[]` | Block 1: `id` (u32), `level` (u32: bit 15 clear → boost `w & 0x7FFF` mB; set → the low 16 bits as a signed mB level), `input` (u32). |
| `section.control_3d` | array<table> | – | no | `[]` | Block 2: `id` (u32; byte 3 & 0xF = the variant count), `variant` (array of 6-`u32` arrays: view selector and sources, curve types, then per direction {lo, hi} metres). |
| `section.event` | array<array<u32>> | – | no | `[]` | Block 6: 7 words each (id with the mode, level in mB, scale id, trigger id, attack, hold, release). |
| `section.sub`, `section.master` | array<table> | – | no | `[]` | Blocks 3 and 4: `id` (u32; bits 16–23 the word count, bit 28 the short form), `w1` (u32), `out` (u32; only when bit 28 is clear), `words` (array<u32>: 3D connections and inputs in file order). |
| `section.master.map` | table | – | no | – | Block 5: `head` (u32; low nibble = the entry count) and `entries` (array<u32>). |

The blocks' 12 bytes after each count, the offset tables and the section table are not read by the engine and
stay in the residual (Eq1 plus shadow, §6).

```toml
schema = "ob.mix-map/1.0"
name = "MAPOUTPUTDRG"

[[section]]
control = [ { id = 0x60000010, level = 0x000003E8, input = 0x40000020 } ]
control_3d = [ { id = 0x01000030, variant = [ [0x00001000, 0x00010000, 0, 50, 0, 200] ] } ]
event = [ [0x02000040, 600, 0x60000010, 0x40000024, 0x0000000C, 0x00000000, 0x0000003C] ]

[[section.sub]]
id = 0x00020050
w1 = 0x03E80000
out = 0x00000001
words = [0x40000020, 0x80000060]

[[section.master]]
id = 0x10010070
w1 = 0x00008000
words = [0x00020050]
map = { head = 0x00000001, entries = [0x00010000] }
```

### 3.7 Reverb preset (`ob.reverb-preset/1.0`)

Every byte of an `.fx` file (big-endian), so the file can be rebuilt (Eq2, §6). Layout: a header of 14 `u32`
(`"FX"` and a `u16` version 0x0B00; the file size; six module counts; six link counts), then per configuration
1–6 its module records (4 or 8 bytes by kind: kinds 1, 4, 5, 7 have 8) and its 12-byte link records. The engine
uses the largest configuration ≤ 2 with modules (`src/audio/reverb/graph.rs:104-105`).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.reverb-preset/1.0` |
| `op` | enum | – | no | `merge` | As for mix maps (§3.6): a pack's file says `op = "replace"`; `merge` and `add` are refused (`MRG.3`). |
| `name` | string | – | yes | – | One of the twelve preset names (`src/audio/reverb/graph.rs:9`). |
| `version` | u16 | – | yes | – | Header bytes 2–3 (0x0B00 in all files). |
| `config` | array<table> | – | yes | – | Exactly 6, in order; each `module` and `link` (arrays, may be empty). |
| `config.module` | array<table> | – | no | `[]` | `kind` (u8, 0–9: input, one-pole, low FIR, high FIR, band FIR, resonator, gain, all-pass, sum, split), `channels` (u8), `p2` (i16), `p4`, `p6` (i16; only for 8-byte kinds). Meaning by kind: docs/formats/audio.md, the section on the reverb's module graph. |
| `config.link` | array<table> | – | no | `[]` | `from` (u32), `to` (u32), `u_0x08` (u8, **open**), `input` (u8: 1 or 2), `u_0x0a` (u16, **open**). |

The encoder writes the header (the file size is the sum of the parts), then each configuration's modules and
links in order; every original file is rebuilt byte for byte this way (records fill each file exactly, measured).

```toml
schema = "ob.reverb-preset/1.0"
name = "Garage"
version = 0x0B00

[[config]]
module = [ { kind = 0, channels = 0, p2 = 0 }, { kind = 9, channels = 0x21, p2 = 0 } ]
link = [ { from = 0, to = 1, u_0x08 = 0, input = 1, u_0x0a = 0 } ]

[[config]]
module = [
  { kind = 0, channels = 0, p2 = 0 },
  { kind = 1, channels = 0, p2 = 0, p4 = 4000, p6 = 200 },
  { kind = 7, channels = 0, p2 = 180, p4 = 35, p6 = 0 },
  { kind = 9, channels = 0x21, p2 = 0 },
]
link = [ { from = 0, to = 1, u_0x08 = 0, input = 1, u_0x0a = 0 } ]   # shortened

# configurations 3–6: `module = []`, `link = []` (shortened here)
```

### 3.8 FLAC profile (new content)

New audio in packs and community base sets is FLAC as RFC 9639 defines it, within these limits (`AUD.3`):

| Property | Allowed |
|---|---|
| Container | native FLAC (`fLaC` + metadata blocks + frames); not Ogg FLAC |
| Bits per sample | 16 or 24 (engine recordings: 16) |
| Channels | 1 (bank sounds, engine recordings), 1–2 (speech lines, music) |
| Sample rate | 8,000–48,000 Hz (the mixer resamples per voice) |
| STREAMINFO | total samples known (non-zero); the signature of the unencoded audio present (non-zero): readers check it when a pack is verified |
| Size | ≤ 1 GiB per file; ≤ 2³¹ frames |
| Recommended | the streamable subset (RFC 9639 §7), blocks ≤ 4,608 samples, no tag blocks or other non-FLAC data |

Lossy formats (Ogg Vorbis, Opus) are reserved for a later minor version (§9).

## 4. Layering

| Kind | merge | add | replace | disabled | Clash key | Notes |
|---|---|---|---|---|---|---|
| `sound-bank` (the bank as a whole) | – | refused | refused | refused | – | Needs a bank compiler (AEMS modules); later. |
| `sound` (a slot) | `loop` and `disabled` only | refused | yes (FLAC) | yes: silence of the original length | `sound:<bank>/<slot>` | |
| `sound-event` | refused | refused | refused | refused | – | Events and modules are the image's. |
| `engine-recording` | refused | yes (new name; `gin` or `flac`) | yes (`gin` or `flac`) | refused | `engine-recording:<name>` | `gin` from packs: the conventions §4.6 exception (§3.3). Which car uses which recording is an engine-table override ([engine-tables.md](engine-tables.md)). |
| `speech-line` | `disabled` only | refused | yes (FLAC) | yes: silence, selection unchanged | `speech-line:<bank>/<entry>/<record>` | `op` required (§3.4.3). |
| `speech-event`, entries, switches | refused | refused | refused | refused | – | |
| `music-stream` | `disabled` only | refused | yes (FLAC) | yes: counts as ended at once | `music-stream:<map>/<index>` | `op` required (§3.5). "My music" (new songs) needs playlist data in the profile; later. |
| nodes, `music-event` | refused | refused | refused | refused | – | |
| `mix-map` | refused | refused | yes (whole map) | refused | `mix-map:<name>` | Per-field merges need stable keys inside sections; later. |
| `reverb-preset` | refused | refused | yes (whole preset) | refused | `reverb-preset:<name>` | The engine's preset list is fixed (twelve names). |

- Refused operations fail with `MRG.3`. No wildcard units.
- Lists keyed for merging: `sound` by `slot`, `line` by (`entry`, `record`), `stream` by `index`. Base index lists
  (`event`, `entry`, `node`, …) are not keyed: they are replaced only with the whole item.
- A pack's replacement keeps the original's position and selection logic: what plays changes, never when, how
  often or which slot is picked. Sounds a pack replaces sound the same with "Use mods" off as before.

## 5. Import

| Unit | Input | Output | Residual |
|---|---|---|---|
| `audio-banks` (per sub-unit) | `SOUND/<DIR>/*.abk` | image copied to `<STEM>.bin`; index from `Bank::parse` (`src/audio/banks/abk.rs:99`) | one `decoded` node per file, `proof = "bytes"` |
| `audio-engine-recordings` | `SOUND/**/*.gin` | `format = "gin"`, file copied; fields from `Gin::parse` (`src/audio/gin.rs:33`) | one node per file, `bytes` |
| `audio-speech` | `sdat.viv` entries `speech/<bank>.*` | `.big` copied to `lines.bin`; `bank.toml` from `parse_csi`, `parse_evt_switches`, `parse_idx` | `.big`: `bytes`; `.csi`, `.evt`, `.idx`: `values` (shadow) |
| `audio-music` | `pfdata/MusicSFx.{mpf,mus}` | `.mus` copied to `streams.bin`; `map.toml` from `Mpf::parse` | `.mus`: `bytes`; `.mpf`: `values` (shadow) |
| `audio-common` | `MIXMAPS/*.mxb`, `FXEDIT/*.fx`, `EVT_SYS/*.csi` | mix maps from `MixMap::parse` (raw words), presets from every byte | `.mxb`: `values` (shadow); `.fx`: `bytes`; `.csi`: `kept` |

- The archive (`sdat.viv`) is a `viv` container node; its header (BIG4, the archive size and entry count
  big-endian at +4 / +8, header size at +0xC, entries {offset, size, name}) and the zero fill that puts each entry
  at a multiple of 64 are rebuilt from the entry list.
- **Codec choice** ([conventions.md](conventions.md) §4.6: exactness first, then an open lossless form unless that
  grows the data more than 3× counting the shadow the rebuild needs; applied per kind, and the two speech rows
  below are one kind, `ob.speech-bank`, both passthrough; measured with `flac -8` on the decoded audio, rounded to
  16 bits):

| Sub-kind | Original bytes | Decoded as 16-bit PCM | FLAC | FLAC ÷ original | Decoded values integer? | Stored as |
|---|---|---|---|---|---|---|
| bank samples (EA-XA) | 15,396,586 | 56,161,920 (3.65×) | 39,480,583 | 2.56× | **no**: 91% of samples have a fraction, 208 exceed 16 bits | passthrough (image) |
| engine recordings (GIN) | 5,576,861 (blocks) | 18,782,750 (3.37×) | 13,997,220 | 2.51× (3.5× with the shadow the rebuild needs) | yes (the reader rounds and saturates) | passthrough |
| NIS speech (EA-XA) | 76,639,972 | 283,347,470 (3.70×) | ≈ 2.27× (11 of 151 streams) | ≈ 2.27× | **no** (the stream decoder is `f32`) | passthrough |
| in-game speech (MicroTalk) | 56,712,060 | 278,622,496 (4.91×) | not measured: no decoder outside Overbright | ≈ 2.5–3× [guess] | **no** (`f32` LPC synthesis) | passthrough |
| music (EALayer3) | 125,870,600 | 988,355,876 (7.85×) | not measured | ≈ 4.5–5.5× [guess] | yes (rounded to 16 bits) | passthrough (> 3×) |

  Bank samples, speech and NIS audio are decoded to unrounded `f32` (`src/audio/codec/eaxa.rs:52`,
  `src/audio/codec/schl.rs:134`, `src/audio/codec/utk.rs:229-321`); integer PCM cannot hold the values the engine
  mixes, so FLAC is not lossless for them whatever its size. Engine recordings are integer after decoding, and FLAC
  alone is 2.51×; but no encoder can give back the GIN blocks from FLAC, so the residual would keep the 5.58 MB of
  blocks as a shadow next to 14.0 MB of FLAC (3.5× in all), and the engine would change its read path. Passthrough
  keeps 5.64 MB, no shadow, and the reader unchanged; `flac` stays the form for new recordings. Music is above 3×.
  So no original audio is stored as decoded PCM, and nothing depends on carrying decoder state across block
  boundaries ([parity.md](parity.md) R.5).

## 6. Equality and verification

| Kind | Level | Proof at import | Shadow |
|---|---|---|---|
| sound bank image, GIN, speech lines, music streams | Eq2 (the payload is the original bytes) | digest of the copied range = digest of the original's range; the index checked against the image (`AUD.2`, `AUD.5`) | none |
| speech `.csi`/`.evt`/`.idx` | Eq1 | `H(Bank::load(original))` = `H(bank from bank.toml + lines)` | 8,064 bytes |
| music `.mpf` | Eq1 | `H(Mpf::parse(original))` = `H(map.toml)` | 6,840 bytes |
| mix maps | Eq1 | `H(MixMap::parse(original))` = `H(mix-map TOML)` | 21,280 bytes |
| reverb presets | Eq2 | the encoder of §3.7 rebuilds each file | none |
| `EVT_SYS/*.csi` | – | kept | 1,180 bytes (kept, not shadow) |

Encoders to add later (all fixed records with offsets): `.csi`, `.idx`, `.evt`, `.mpf`, `.mxb`; each drops its
shadow by migration ([residual.md](residual.md) §3.2). Total shadow today: 36,184 bytes.

Gates ([parity.md](parity.md) §3), recorded with the engine still reading the original files, then rerun on the
base set:

- **Payload decode hashes** (CI, reference install): XXH3-128 over the decoded output's bits of every bank slot
  (`Bank::decode`, `f32`), every GIN (`Gin::read`, `i16`), and a fixed sample of speech and music streams (all
  of them nightly: about 4.9 hours of audio, ≈ 1–2 min to decode [guess]).
- **Audio captures**: the existing `NFSU2_AUDIO_WAV` scenarios (front end, free roam, race, a scene with NIS
  speech, a song start) rendered offline to WAV files; hashes must match bit for bit. Nothing is played on a
  device.
- **Packs**: a test pack that replaces one slot, one line and one stream, and disables one of each, renders the
  expected WAV; with the pack off, the capture equals the base capture.

## 7. Cooked form

| Cook unit | Model sections | Zero-copy payloads | Copied / prepared |
|---|---|---|---|
| `audio/<sub-unit>` for banks and recordings | `banks` (per bank: key, index, events; the image as an `extref`), `recordings` (header fields, tables, `extref` or decoded PCM) | bank images, GIN files (stored bundle entries) | FLAC replacements decoded to `f32` (bank slots) or `i16` (recordings) in the model, since they must be decoded anyway |
| `audio/speech` | `speech_banks` (switches, events, entries, records as byte ranges) | `lines.bin` ranges per line | replacement lines decoded on use by the stream thread (not cooked) |
| `audio/music` | `music_map` (nodes, events, stream ranges) | `streams.bin` ranges per stream | as speech |
| `audio/common` | `mix_maps`, `reverb_presets` (the parsed structures) | – | the reverb graph's derived coefficients are runtime state, not cooked |

Prepared data: none normative (decoded AEMS op lists may be cached as prepared entries).

## 8. Errors

| Rule | Text |
|---|---|
| AUD.1 | A bank image that does not parse: no `ABKC`, `file_size` ≠ the array's count, a table offset outside the image. |
| AUD.2 | A bank index that differs from its image (module count, events, any `sound` field). |
| AUD.3 | A FLAC source outside the profile (§3.8): wrong channels for its use, bits per sample, rate, missing audio checksum or sample count, Ogg FLAC, over the limits. |
| AUD.4 | A replacement source that is missing, not FLAC, or whose digest differs. |
| AUD.5 | An engine recording whose fields disagree with its `gin` file, whose tables are not ordered as §3.3 says, or whose `min_rpm` ≥ `max_rpm`. |
| AUD.6 | A speech line range outside `lines`, records out of order, keys of uneven length, `ring` longer than `history`, or a line that is not one `ea_stream`. |
| AUD.7 | A music map with a stream range outside `streams_data` or not 128-aligned, a node `next` or event `start_node` out of range, a `stream_id` past the stream table. |
| AUD.8 | A mix map whose 3D variant count or channel word count differs from its id word, or with more than 32 sections. |
| AUD.9 | A reverb preset with a module kind above 9, `p4`/`p6` on a 4-byte kind (or missing on an 8-byte one), a link to a module that does not exist, or not exactly 6 configurations. |
| AUD.10 | An EA stream that breaks §3.1.3: no `SCHl` first, a chunk size under 8 or past the range, an unknown codec, block frame counts that do not sum to the header's. |
| AUD.11 | A `sound` override of a slot the base marks `present = false` (the engine never starts such a slot; overrides of items that do not exist at all are `MRG.1`). |

Keys differing only in case are `CONV.13` (§2.3); a line or stream override without `op` is `CONV.5`.

## 9. Versioning and extension

- 1.x may add: `format = "vorbis"` / `"opus"` sources (Ogg, https://www.rfc-editor.org/rfc/rfc3533; Opus,
  https://www.rfc-editor.org/rfc/rfc6716 and https://www.rfc-editor.org/rfc/rfc7845) once a decoder's licence is
  settled; `add` of music events with playlist fields ("My music"); per-field `merge` of mix maps; a TOML form of
  AEMS modules (the decoded op list, template and record fields: every module of the install uses the forms
  `src/audio/aems/decode.rs:129` decodes) so banks can be authored without the original's compiler, which is what
  a community base set needs for event-driven sounds.
- Decoder crates a reader may use for new content (licences as published on crates.io; each [to verify] before
  adoption): FLAC: `claxon` 0.4.3 (Apache-2.0), `symphonia-bundle-flac` 0.6.1 (MPL-2.0, which the FSF lists
  as GPL-compatible, https://www.gnu.org/licenses/license-list.html#MPL-2.0); FLAC encoding in Studio: `flacenc`
  0.5.1 (Apache-2.0); Vorbis: `lewton` 0.10.2 (MIT OR Apache-2.0) with `ogg` 0.9.2 (BSD-3-Clause), or
  `symphonia-codec-vorbis` (MPL-2.0); Opus: `opus-decoder` 0.1.1 (MIT OR Apache-2.0, pure Rust, young), or
  `magnum-opus` 0.3.2 / `audiopus` 0.2.0 (MIT/Apache-2.0, ISC) binding libopus (BSD-3-Clause). All are
  GPL-3.0-compatible licences [to verify per version]; pure-Rust ones also build for the web.
- Extension point: `[ext.OB_*]` tables on `sound`, `line` and `stream` overrides (e.g. a gain trim); none defined.

## 10. Open points

- Bank header fields `u_0x04`, `u_0x06`, `u_0x20`, `u_0x24`, the 28 bytes before the first module record, the
  event entry's third word, the module records' unnamed fields: **open**; carried in the image.
- Mix-map block heads (12 bytes after each count) and the meaning of several word bits: **open**; in the residual.
- Reverb link bytes `u_0x08`, `u_0x0a`: **open**.
- `.idx` header bytes before +0x54 and entry flags: not read; in the residual.
- Speech bank per language: the engine picks the bank name from a language table (0x7ECFA8); whether other
  editions name or lay out their banks differently is [to verify]. A base set holds the installed language only.
- EALayer3 output differs from the original's fixed-point synthesis (7 LSB rms, docs/formats/audio.md); keeping the
  bitstream lets a better decoder replace it without re-importing.
- The engine reads the GIN files and mix-map / reverb files through direct opens today
  (`src/audio/engine.rs:36-43`, `src/bin/game/audio/engine.rs:135`, `src/audio/mixmap/parse.rs:182-196`,
  `src/audio/reverb/graph.rs:200-207`), besides the archive's (`crates/ob-formats/src/formats/viv.rs:103,145`); all move behind the
  asset layer.

## 11. Sizes and timing

Reference install: `SOUND/` 284 files, 22,369,057 bytes; `SDATA/sdat.viv` 259,484,480 bytes.

| Sub-unit | Original | Base set (bundle, stored) | Shadow |
|---|---|---|---|
| common | 24,104 (+1,180 EVT_SYS) | ≈ 0.2 MB TOML [guess] | 21,280 |
| frontend | 1,383,968 | ≈ 1.39 MB | 0 |
| ingame | 4,466,004 | ≈ 4.48 MB | 0 |
| engine | 16,493,801 | ≈ 16.6 MB (images + ≈ 0.3 MB TOML) | 0 |
| speech | 133,603,712 | ≈ 133.7 MB | 8,064 |
| music | 125,880,248 | ≈ 125.9 MB | 6,840 |
| **total** | **281.85 MB** | **≈ 282.3 MB** | **36,184** |

Import time: copying ≈ 282 MB and hashing it, ≈ 1–3 s on an SSD [guess]; parsing all banks and control files
< 0.5 s [guess]. Optional decode hashes of every stream: ≈ 1–2 min [guess] (CI, not the player's import). A pack
author's FLAC encode in Studio: ≈ 1 s per minute of 44.1 kHz stereo at level 5 [guess].
