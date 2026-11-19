# Asset format: movies (full-motion video)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

The original's movies are VP6 video and EA-XA sound interleaved in EA's chunked container. The format keeps both
bitstreams as they are (codec passthrough): the video in an IVF file, the sound as an EA stream with the original
interleave recorded, so the original file can be rebuilt byte for byte. Re-encoding would lose quality (VP6 is
lossy, and no VP6 encoder is freely available), and decoding to lossless video grows the data 7–12× (§5).

Schemas: [`movie`](schemas/movie.schema.json), [`movie-table`](schemas/movie-table.schema.json). Examples:
[`examples/movies/`](examples/movies/).

## 1. Scope

| Original | Reference install | Kind here | Engine reader today |
|---|---|---|---|
| `MOVIES/*.vp6` (chunks `MVhd`, `MV0K`, `MV0F`, `SCHl`, `SCCl`, `SCDl`, `SCEl`) | 32 files, 261,189,624 bytes | `ob.movie` | `src/movie.rs:127` (container), `crates/ob-formats/src/formats/vp6.rs:431` (VP6), `src/movie.rs:189-262` (EA-XA) |
| `GLOBAL/GLOBALB.BUN` chunks 0x39020 (the movie table: name and sound volume) | 40 chunks of 40 bytes | `ob.movie-table` | `src/movie.rs:317` |
| `MOVIES/movies.tag` | 1 file, 0 bytes | – | not read by the engine; kept whole (empty) in the residual |

Not here: the subtitle cues (`SUBTITLES/<name>`, keyed by the name the game asks for, e.g. `Scene01`): their kind,
`ob.subtitles`, is [text.md](text.md)'s (§3.7); their files sit in this unit's folder, `movies/subtitles/` (§2).
Which screen queues which movie (boot sequence, career scenes, tutorials): engine code
(docs/formats/movies-replays.md "Movies"). In-engine scenes (NIS): [animation.md](animation.md). Replays: not
assets.

## 2. Files and keys

```text
movies/table.toml                    ob.movie-table: the 40 rows of the movie table
movies/Scene01_en/movie.toml         ob.movie
movies/Scene01_en/video.ivf          VP6 frames in IVF framing
movies/Scene01_en/audio.bin          the EA sound stream and its interleave (layouts ea_stream, chunk_order)
movies/blank/movie.toml              a movie without sound: no audio.bin
movies/blank/video.ivf
movies/subtitles/Scene01.toml        ob.subtitles (text.md §3.7): the cues of one movie
```

One bundle, `movies.obz` ([layout.md](layout.md) §5). The movie table's rows come from `GLOBALB.BUN` but belong
to this unit (the volume is read when a movie starts; §5). The subtitle cues are [text.md](text.md)'s kind, stored
here beside the movies they belong to.

| Short name | Key | Example | Item |
|---|---|---|---|
| `movie` | the original file name without `.vp6`, original spelling | `movie:Scene01_en`, `movie:ealogo_du`, `movie:THX_logo` | one movie (folder `movies/<key>/`) |
| `movie-entry` | the row's name as the table holds it | `movie-entry:Scene01` | one movie-table row |

- **Names the game asks for.** The engine asks for a movie by a name (`scene01`, `ealogo`, `THX_logo_fr`) and
  resolves it to a file exactly as `movie::path` does (`src/movie.rs:277-312`): first the name the executable
  builds (`<name>.vp6` for the eight plain names, else `<name>_fr.vp6`), then the installed language's file
  (`<name>_en.vp6`; `THX_logo.vp6` for `THX_logo_fr`), compared without case; with neither, the screen ends at
  once, as the original's does for a missing file. Readers apply the same rule to the `movie` keys of the merged
  stack. The second step is a documented deviation of the clone (docs/native-deviations.md "Movie files"); the
  format does not change it.
- **Language and region variants** are separate items with their original stems: the reference install has
  `ealogo_en`, `ealogo_du`, `ealogo_dn`, `ealogo_sw` (only `_en` is ever resolved by the rule above; the others are
  kept, lossless) and `_en` versions of the scenes, tutorials and PSA. A pack that adds `Scene01_fr` is found
  before `Scene01_en` by the first step; the clash view lists it as hiding `Scene01_en`.
- `movie` keys are file-name keys ([conventions.md](conventions.md) §6.3): they resolve ASCII case-insensitively,
  and the stack MUST NOT hold two that differ only in case (`CONV.13`). `movie-entry` names follow the same rule,
  because the engine matches the table's names without case (`MOV.8`).

## 3. Source form

### 3.1 `video.ivf`

IVF as libvpx writes it (https://wiki.multimedia.cx/index.php/Duck_IVF): a 32-byte header, then per frame a
12-byte header and the frame. Little-endian.

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u8[4]` | `signature` | – | `DKIF` |
| 0x04 | 2 | `u16` | `version` | – | 0 |
| 0x06 | 2 | `u16` | `header_len` | byte | 32 |
| 0x08 | 4 | `u8[4]` | `fourcc` | – | writers: `VP61` (VP6.1, bitstream version 7 [to verify]); readers accept `VP60`, `VP61`, `VP62` and decide by the bitstream's own version field |
| 0x0C | 2 | `u16` | `width` | px | = `movie.toml` `width` |
| 0x0E | 2 | `u16` | `height` | px | = `height` |
| 0x10 | 4 | `u32` | `timebase_den` | – | = `rate` (819,175 in every original) |
| 0x14 | 4 | `u32` | `timebase_num` | – | = `scale` (32,767): one timestamp step = scale / rate s = 1/25 s |
| 0x18 | 4 | `u32` | `frame_count` | – | = `frames` |
| 0x1C | 4 | `u32` | `pad` | – | zero |

Size 32. Each frame:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `size` | byte | the frame's bytes, ≥ 1 |
| 0x04 | 8 | `u64` | `pts` | timestamp | the frame's index: 0, 1, 2, … (`MOV.3`) |
| 0x0C | `size` | `u8[size]` | `frame` | – | one VP6 frame, the original chunk's payload unchanged |

Frames are the `MV0K` / `MV0F` payloads in file order. IVF has no key-frame flag: a frame is a key frame when the
first bit of its VP6 header is 0, which is exactly when the original chunk is `MV0K` (all 29,051 frames of the
reference install, measured). The first frame MUST be a key frame. Every original frame is VP6 version 7,
profile 0, progressive (`crates/ob-formats/src/formats/vp6.rs:7-14`); a frame the engine's decoder refuses (version > 7, profile
≠ 0, interlaced) is `MOV.2`.

Why IVF: it is the smallest open framing that holds VP6 unchanged and that common tools read: FFmpeg 8.1 opens an
IVF with FourCC `VP60`/`VP61`/`VP62` and decodes the same pictures as from the original file (frame hashes equal
for every frame both paths read, measured on `ealogo_en` and `THX_logo`; FFmpeg's own EA reader stops at the
sound's `SCEl` and misses `THX_logo`'s last 23 frames, which the IVF path has; with `VP6F`, the Flash orientation,
the pictures differ). FFmpeg's IVF *muxer* refuses VP6,
so the importer writes IVF itself. Matroska (RFC 9559, https://www.rfc-editor.org/rfc/rfc9559, with
`V_MS/VFW/FOURCC`) also holds VP6 and FFmpeg writes it, but it has no codec id for EA-XA ("No wav codec tag found
for codec adpcm_ea_r3"), needs an EBML reader, and buys nothing the engine uses. IVF also carries AV1, VP8 and VP9,
the candidates for replacement movies (§9).

### 3.2 `audio.bin`

Two arrays ([conventions.md](conventions.md) §4.4):

| Layout | Element | Content |
|---|---|---|
| `ea_stream` | `u8` | the movie's sound chunks in file order, unchanged: `SCHl`, `SCCl`, every `SCDl`, `SCEl` ([audio.md](audio.md) §3.1.3). In movies the `SCHl` payload starts `GSTR` (big-endian block words; defaults 1 channel, 48 kHz, codec 10) rather than `PT` (docs/formats/movies-replays.md "Audio"). |
| `chunk_order` | `u32` | one per chunk of the stream, in stream order: how many video frames precede that chunk in the original file. Non-decreasing, each ≤ `frames`. |

`chunk_order` is what rebuilds the original interleave: it is regular (`MVhd`, `SCHl`, a frame, `SCCl`, then
frame/block pairs) in most files, but sound ends before the picture in some and after it in others, so it is
stored, not derived (114,896 bytes for all 31 movies with sound).

### 3.3 `movie.toml` (`ob.movie/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.movie/1.0` |
| `op` | enum | – | no | `merge` | layering.md §3.1 (packs): `add` or `replace` (VP6 in IVF, §4), or `merge` carrying only `disabled`. Base sets: absent. |
| `disabled` | bool | – | no | `false` | The movie counts as missing (§4). |
| `name` | string | – | yes | – | The key: the original stem (≤ 63 of `[A-Za-z0-9_]`); equals the folder name. |
| `width`, `height` | u16 | px | yes | – | MVhd +4, +6 (512 × 512; `blank` 640 × 480). |
| `frames` | u32 | – | yes | – | MVhd +8. |
| `largest_frame` | u32 | byte | yes | – | MVhd +0xC: the largest frame's size. |
| `rate` | u32 | – | yes | – | MVhd +0x10 (819,175). |
| `scale` | u32 | – | yes | – | MVhd +0x14 (32,767; the engine reads its low 16 bits). Frames per second = `rate` / `scale` in `f32`. |
| `u_0x00` | u32 | – | no | `0` | MVhd +0: **open**, zero in every original. |
| `video` | table | – | yes | – | `file` (`video.ivf`), `codec` (enum: `vp6`), optional `digest`. |
| `audio` | table | – | no | – | Absent: no sound (`blank`). Fields below. |
| `audio.stream` | array descriptor | – | yes | – | Layout `ea_stream`. |
| `audio.order` | array descriptor | – | yes | – | Layout `chunk_order`. |
| `audio.codec` | enum | – | yes | – | `ea_xa` (checked against the header). |
| `audio.channels` | u8 | – | yes | – | 2 in every original (checked). |
| `audio.rate_hz` | u32 | Hz | yes | – | 32,000 in every original (checked). |
| `audio.frames` | u32 | sample | yes | – | Samples per channel (checked; equals the blocks' sum in all 31). |
| `extras` | table | – | no | – | Conventions §9. |

The required fields apply to a movie a base set holds or a pack adds or replaces; a `merge` gives only `name` and
`disabled`.

The MVhd payload (the first chunk's, little-endian); every byte is a field of `movie.toml`:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | **open**; zero in every original |
| 0x04 | 2 | `u16` | `width` | px | |
| 0x06 | 2 | `u16` | `height` | px | |
| 0x08 | 4 | `u32` | `frames` | – | |
| 0x0C | 4 | `u32` | `largest_frame` | byte | |
| 0x10 | 4 | `u32` | `rate` | – | |
| 0x14 | 4 | `u32` | `scale` | – | |

Size 24, alignment 4.

```toml
schema = "ob.movie/1.0"
name = "ealogo_en"
width = 512
height = 512
frames = 80
largest_frame = 11552
rate = 819175
scale = 32767
video = { file = "video.ivf", codec = "vp6", digest = "xxh3-128:9a1f04c37be25d6018e4f2a9c3b70d55" }

[audio]
stream = { file = "audio.bin", offset = 0, count = 92616, layout = "ea_stream" }
order = { file = "audio.bin", offset = 92624, count = 85, layout = "chunk_order" }
codec = "ea_xa"
channels = 2
rate_hz = 32000
frames = 101431
```

### 3.4 Movie table (`ob.movie-table/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.movie-table/1.0` |
| `movie` | array<table> | – | yes | – | Rows in the original's chunk order (the engine takes the first name that matches without case). Keyed by `name`. |
| `movie.name` | string | – | yes | – | The name the game asks for (`ealogo`, `Scene01`, `THX_logo_fr`): ASCII, 1–30 characters (`MOV.6`). |
| `movie.volume` | u8 | – | yes | – | The sound's volume byte (0–127): × master × the "Speech & Movies" option (`src/bin/game/flow/movies.rs:232-239`). |
| `movie.op`, `movie.disabled` | – | – | no | – | Layering (§4). |

Each row is one 0x39020 chunk of 40 bytes: 8 bytes of 0x11 alignment fill (the chunk container's, written by the
encoder), then one `movie_record` (all 40 rows measured clean):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 31 | `u8[31]` | `name` | – | ASCII, 1–30 characters, then a NUL, then zero bytes to the end of the field (`MOV.6`). |
| 0x1F | 1 | `u8` | `volume` | – | 0–127. |

Size 32, alignment 1. The table also names movies the install does not have (`ealogo_fr`, `ealogo_gr`,
`ealogo_it`, `ealogo_sp`, `THX_logo_en`, `TeamDriftTut`, `Track11`).

```toml
schema = "ob.movie-table/1.0"

[[movie]]
name = "ealogo"
volume = 40

[[movie]]
name = "THX_logo_fr"
volume = 69

[[movie]]
name = "Scene01"
volume = 127
```

## 4. Layering

| Item | merge | add | replace | disabled | Clash key |
|---|---|---|---|---|---|
| `movie` | `disabled` only | yes, VP6 in IVF only in 1.0 | yes, VP6 in IVF only in 1.0 | yes: the movie counts as missing, so its screen ends at once (the original's path for a missing file) | `movie:<stem>` |
| `movie-entry` | `volume` | yes (appended after the base rows) | yes | yes: the name has no row (the engine's no-row volume) | `movie-entry:<name>` |

- Refused operations fail with `MRG.3`. No wildcard units.
- **VP6 in IVF from packs.** A pack's added or replacing movie is VP6 in IVF (§3.1), with its sound as an EA stream
  (§3.2) or none. This is the passthrough form [conventions.md](conventions.md) §4.6 lets a kind page accept where
  no open alternative is decided yet: it holds until an open video codec is chosen (§9), and then remains readable.
  The movie form has no other sound field before that version, so the exception covers the movie as a whole.
- Switching a movie off is a `merge` that carries only `disabled = true` (the `name` and nothing else; example
  [`examples/movies/movie-disabled.toml`](examples/movies/movie-disabled.toml)).
- A movie is replaced whole (picture, sound, interleave); a pack cannot replace only the sound in 1.0.
- With no free VP6 encoder, 1.0 replacements are practical only for disabling; open codecs come with §9.
- A community base set MAY omit every movie: the engine then skips each screen exactly as the original does
  without the file. Movies are therefore not in the required-items profile ([manifest.md](manifest.md) §5).

## 5. Import

- **Unit `movies`**: per `MOVIES/*.vp6`, `Movie::parse` (`src/movie.rs:127`) splits the chunks; the importer
  writes `video.ivf` (header from MVhd, one IVF frame per `MV0K`/`MV0F` in order), `audio.bin` (the sound chunks in
  order, then `chunk_order` at the next multiple of 16), `movie.toml`. A file with a chunk other than the seven
  tags, bytes after the last chunk, a key-bit/tag disagreement or a frame count other than MVhd's is imported
  whole into the residual instead (`kept`; none in the reference install) and the edition becomes `custom`.
- **The movie table** belongs to the unit `movies` too, although its chunks are in `GLOBALB.BUN`
  ([import.md](import.md) §4, "One file, several units"): the unit decodes the 0x39020 chunks into
  `movies/table.toml` and owns their nodes in `GLOBALB.BUN`'s residual map (`decoded`, `items =
  ["movie-entry:<name>"]`, `proof = "bytes"`); the file's map is written once, after every unit that owns one of
  its nodes has finished.
- **Codec choice** (the format's 3× rule): VP6 is lossy; the only lossless forms are raw or lossless-coded
  pictures. Measured with FFmpeg 8.1 (FFV1, level 3) against the same frames in Matroska: `ealogo_en` 7.1×,
  `THX_logo` 8.9×, `IndustEast` 11.5×; raw YUV 4:2:0 for all 29,051 frames would be 11.4 GB (≈ 52× the VP6 frames). Passthrough. The
  sound is EA-XA decoded to unrounded `f32` (`src/movie.rs:196-233`), which integer PCM cannot hold: passthrough
  ([audio.md](audio.md) §5).

## 6. Equality and verification

| Item | Level | Proof at import | Shadow |
|---|---|---|---|
| movie | Eq2 | the encoder below rebuilds the `.vp6` file; its XXH3-128 and size equal the original's | none |
| movie table | Eq2 | each record rebuilt from `name` and `volume` | none |

**Encoder** (rebuild of one `.vp6`): write the `MVhd` chunk (tag, size 32, the payload from `movie.toml`); then for
frame *i* = 0 … `frames` − 1: first every sound chunk whose `chunk_order` value is *i*, then the frame as a chunk
(`MV0K` when its first bit is 0, else `MV0F`; size = frame size + 8); last the sound chunks whose value is `frames`.
Sound chunks are copied from `ea_stream` by their own size fields. This reproduces all 32 files of the reference
install (every chunk size is a multiple of 4 and no file has trailing bytes, measured).

Gates ([parity.md](parity.md) §3), baselines recorded from the original files first:

- **Frame hashes** (CI, reference install): every frame of every movie decoded by `vp6::Decoder` and converted
  by `vp6::to_rgba`, XXH3-128 per frame; the sound decoded by `decode_block`, XXH3-128 over the `f32` bits.
  ≈ 29,000 frames, ≈ 30–60 s [guess].
- **Shots and audio**: the existing movie-screen shot (Scene01 on IG_PlayMovie) at every resolution of the
  matrix, and the boot sequence's audio capture to a WAV file (nothing played on a device).

## 7. Cooked form

| Cook unit | Model sections | Zero-copy payloads | Prepared |
|---|---|---|---|
| one movie (`movies/<stem>`), cooked when first played or read ahead | `header` (MVhd fields, audio header), `frames` (per frame: offset and size in `video.ivf`, key flag), `audio_blocks` (per `SCDl`: offset and size in `audio.bin`) | every frame and every sound block: slices of the stored bundle entries | none (decoded pictures are per-frame runtime state) |
| `movies` table | `movie_table` (rows in order) | – | none |

The read-ahead of the boot movies (`src/bin/game/loads/read_ahead.rs:25`) becomes a read-ahead of the bundle
ranges of those movies.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| MOV.1 | A `video.ivf` header that is not `DKIF`, version 0, header length 32, a FourCC outside `VP60`/`VP61`/`VP62` for codec `vp6`, or whose width, height, time base or frame count differs from `movie.toml`. |
| MOV.2 | A frame the engine's VP6 decoder refuses (version > 7, profile ≠ 0, interlaced), or a first frame that is not a key frame. |
| MOV.3 | Frame timestamps that are not 0, 1, 2, …; a frame size of 0; a frame running past the file; a largest frame other than `largest_frame`. |
| MOV.4 | An audio stream that is not one `ea_stream` with a `GSTR` or `PT` header, or whose codec, channels, rate or frame count differ from `movie.toml`. |
| MOV.5 | A `chunk_order` array whose length differs from the stream's chunk count, that decreases, or that exceeds `frames`. |
| MOV.6 | A movie-table name that is empty, longer than 30 characters, not ASCII, or a volume above 127. |
| MOV.7 | A `video.codec` or audio codec this reader does not support (values reserved for §9). |
| MOV.8 | Two `movie-entry` names that differ only in case in one layer stack (two such `movie` stems: `CONV.13`); a movie folder name that differs from `name`. |
<!-- /generated -->

## 9. Versioning and extension

- **Replacement movies in open codecs** (a minor version, once a decoder's licence has been reviewed):
  `video.codec = "av1"` in IVF (FourCC `AV01`), pictures stored top-down (the engine flips only VP6,
  docs/formats/movies-replays.md "Drawing"), and `audio.source` = a FLAC or Opus file ([audio.md](audio.md) §3.8,
  §9) instead of `stream`/`order`. Decoder candidates (licences as published; [to verify] before adoption):
  `rav1d` 1.1.0 (BSD-2-Clause, a Rust port of dav1d, no C toolchain, so also for the web [guess]); `dav1d` 0.12.0
  (MIT bindings) with libdav1d (BSD-2-Clause); for VP8/VP9 `vpx-rs` 0.2.1 (BSD-2-Clause) with libvpx (BSD-3-Clause);
  Theora via libtheora (BSD-3-Clause; no maintained Rust decoder known [to verify]). WebM/Matroska input for
  authors is converted to IVF by the CLI (`matroska-demuxer` 0.8.1, Zlib OR MIT OR Apache-2.0). All are
  GPL-3.0-compatible licences [to verify]. Size limits for replacements: width and height even, ≤ 2048 [guess].
- Extension point: `[ext.OB_*]` in `movie.toml` (e.g. a chapter list for skipping); none defined.

## 10. Open points

- MVhd +0 (`u_0x00`): zero everywhere; **open**.
- Other editions' language suffixes (`_ge`, `_fr`, …) and whether their movies differ: [to verify]; the second
  lookup step uses `_en` today.
- The original's A/V sync is not traced (docs/formats/movies-replays.md "Clone": **inferred**); the format keeps the
  interleave, so a later traced player can use it.
- VP6 FourCC for bitstream version 7: `VP61` by On2's naming [to verify]; FFmpeg maps all three to the same decoder.

## 11. Sizes and timing

| | Original | Base set |
|---|---|---|
| Video | 221,107,572 bytes of frames (+ 232,408 bytes of chunk heads) | 221,457,208 bytes of IVF (32-byte headers + 12 bytes per frame) |
| Sound | 39,848,620 bytes of chunks (31 movies, 1,144.7 s, stereo 32 kHz) | 39,848,620 + 114,896 bytes of `chunk_order` |
| `MVhd` | 1,024 bytes | in `movie.toml` (≈ 0.4 KB each) |
| **Total** | **261,189,624** (32 files, 29,051 frames, 1,162 s) | **≈ 261.43 MB** (+0.09%); no shadow |
| Movie table | 1,600 bytes (40 chunks) | ≈ 1.6 KB TOML |

Import time: copying 261 MB with per-chunk splitting, ≈ 1–2 s on an SSD [guess]; the rebuild check reads it once
more (≈ 0.5 s [guess]). Optional frame-hash gate: ≈ 30–60 s [guess] (CI only).
