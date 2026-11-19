# Menu music: which EA Trax song plays when

What the original plays from the boot to the main menu and back, and how the clone follows it.
**Traced** = read in SPEED2.EXE, **observed** = read live in the original under Wine (music
object and playlist memory, the recorded output), **inferred** = neither. The playlist
machinery itself (masks, pools, Sequential / Random) is in docs/formats.md, "Sound round 4".

## The rule

1. **Boot (traced, observed).** The music object's constructor (0x459B80) sets both masks to all
   27 songs, the front end's pool to songs 0–10 (+0x88 = mask & 0x7FF), Sequential (+0x9C = 0).
   Right after it the start-up's profile reset (0x515AA0) fills the playlist 0x83ACB8 in table
   order from the EA Trax table 0x7ECD80 (16 bytes a song; +0xC "FE" → Menu, "IG" → Racing,
   "AL" → All, else Off) and builds the masks again (0x479220 → 0x46BAA0) with the settings'
   play mode 0x83AA58 (settings block 0x83AA04 + 0x54, 1 = Random from the constructor
   0x5116E0). Observed on every cold start: FE mask 0x3F (songs 0–5), game mask 0x7FFFFC0,
   pools full, mode 1, both last positions −1 — with no profile and with one on disk (a
   profile is applied only after the title). The random state 0x7EA624 is reseeded from the
   performance counter before this (0x57ED20 → 0x57ECC0), so it differs every boot.
2. **Boot movies (traced, observed).** No song: the music state +0xC0 is 5 (held) and no
   stream plays (0x82AFD4 = −1).
3. **Title screen (traced, observed).** UG_LS_Splash's constructor 0x4A90F0 calls 0x480C30(−2)
   → 0x47AC50(1, −2). State 1 stops any stream (0x739320), sets +0xA5 = 1 (the front end may
   pick) and, because the argument is −2 and the playlist's first position is a front-end song
   (its state byte 0x83ACBC has bit 0: Menu or All), sets the front end's pool +0x88 = 1
   (0x47AED4): song 0 alone. Bit 0 is song index 0 whatever song the first position holds. The
   next frame's pick (0x47B080 → 0x46B6F0) can only take song 0, in Random (Rand_Int(1) = 0,
   one draw on 0x7EA624) and in Sequential; the pool is then empty. The state changes only on a
   change into 1, so it happens on each new title screen: after the boot movies and after the
   attract movie (observed: song 0 starts over when the title comes back).
   **Song 0 is "Riders on the Storm (Fredwreck Remix)"** (Snoop Dogg feat. The Doors; the
   caption says so, observed), event 0 → stream table entry 38, 373.6 s. It plays **from its
   first sample**: the recorded output matches the decoded stream from t = 0 (normalised
   correlation 0.985 at 5 s, 10 s and 30 s into the song, one common offset; observed).
4. **Profile (traced, observed).** Loading a profile (Continue after "Load complete",
   0x5346AA) or creating one (0x534700 → 0x515AA0) runs 0x479220 again: the pools refill from
   the masks (FE pool 0 → 0x3F) and the last positions go to −1. The song playing goes on.
5. **Song end (traced, observed).** When the stream ends (0x82AFD4 = −1) 0x47B080 picks the
   next with 0x46B6F0: Random over the songs left in the pool (Rand_Int over their count, the
   n-th by song index), Sequential from the position after the last one. After the title's
   song the pool is the profile's full one, so in Random song 0 can come again (observed once:
   Riders on the Storm, then Riders on the Storm). Without a profile load in between,
   Sequential refills and starts again at position 0 (song 0 twice; traced, not observed).
6. **Race and back (traced, observed).** A race stops the front-end song (state 7 on the load)
   and picks from the game's own pool (observed: a random Racing song, the FE pool kept). Back
   in the main menu (0x47AC50(1, 0): no −2) the front end draws from its pool where it stopped
   (observed: pool 0x3E → song 4, pool 0x2E). No song is forced there.

7. **A race's end (traced, observed).** The race's song plays on through "Winner!", the post-race
   flow's load of the ending scene, the ending scene, Event Results, UI_PostRaceReward,
   UI_SponsorPopup, the sponsorship screen and Post Race. The load is the running flow's LOADING
   (0x481210(1) → 0x4813A0 → 0x47AC50(7)), and state 7 (0x47AD0F) stops the stream only when game
   music × master² × 32767 truncates to 0 (0x47AD49..0x47AD72; the score plays then) or when an
   interactive score piece plays (+0x14 ≥ 0x1B). Observed (Quick Race, the human's finish written
   into the race record): song 24 and its stream from the race through the load (flow state 1),
   the ending scene (flow state 7) and Event Results to Post Race; the level dips and comes back
   (mix map), the song does not change. The post-race screens are in the game's audio mode
   (+0x90 ≠ 1: only the front end's load, 0x5783E0 → 0x486B30, sets 1), so the game's side.
8. **Where a song does change in a career (traced).** A race or free-roam start sets a new flow
   up: 0x60A898 → 0x481440 → state 2 (0x47ADC6) stops the song unconditionally; the next game
   song starts in the race. Entering a shop or the garage loads the front end: its end 0x5800A0
   (game state 3) calls 0x480C30(−1) → state 1, which stops the stream and lets the front end pick
   (0x47B080 with +0x90 = 1, 0x46B330 then reads the shop's score context). So a new song on
   entering a shop, on leaving it and at a race's start is the original's; a new one on the
   post-race screens is not.

## Observations (Wine, three cold starts)

| Situation | Original | Clone before | Clone after |
|---|---|---|---|
| Boot movies | none | none | none |
| Title, no profile | song 0 Riders on the Storm, from 0 s, on the title's construction (state 5 → 1) | a random front-end song (Random on the boot-seeded generator, e.g. song 3 for clock 0); song 0 only with the static seed of headless runs | song 0, from 0 s, on the title's construction |
| Title, fresh profile on disk | song 0, as above | random, as above | song 0 |
| Title, existing profile (sandbox copy) | song 0, as above; the profile's playlist (default order, Random) applied after the title | random, as above | song 0 |
| Main menu after the profile | the title's song goes on; pools refilled | the title's (random) song goes on; pools refilled only if the list changed | song 0 goes on; pools refilled |
| End of that song | Random pick over the 6 Menu songs (song 0 possible) | same rule | same rule |
| Back from a race | the front end's pool where it stopped | same rule | same rule |

The first song is fixed, not random: four cold starts (three reached the title; one hung at a
black screen, a known Wine flake) had four different random states and all played song 0.

## The clone

- `music::Playlist::title_start` (src/audio/trax/playlist.rs): 0x47AC50's −2 branch.
- `Streams::title_music` (src/audio/streams/music_manager.rs): state 1's stop, then the title
  start on the next frame, once the playlist is built from the options' playlist.
- `Game::boot_screen` (src/bin/game/flow/movies.rs): the Splash's construction calls it, as
  0x4A90F0 does — after the boot movies and after the attract movie.
- Tests: `audio::trax::tests::the_title_starts_song_0` (the rule in both modes, any seed, the
  first position's state), `the_title_plays_riders_on_the_storm_in_the_game` (the player's
  EA Trax table: song 0 is a Menu song and Riders on the Storm; the boot playlist's title pick
  for several boot seeds), `tests::scene_audio::the_title_plays_song_0_for_any_boot_seed` (the
  game from the boot movie to the title and back, offline mixer).
- The boot's list: the clone reads the profile's options (the EA Trax list and play mode among
  them) only from the profile, never from options.txt (`Settings::load`, which the boot runs
  before it loads the profile on disk), so the boot movies and the title play with the profile
  reset's list (table order, songs 0–5 Menu, Random), as the original. A list with song 0 Off
  or Racing still gives song 0 on the title.
- The profile applied (the boot's "Load complete.", the Profile Manager's Load, a new
  profile): `Streams::reset_playlist` with the profile's options, as 0x5344E0 / 0x515AA0 →
  0x479220 do: the masks follow its list, both pools refill, the last positions go back, the
  song goes on. Before, an unchanged list kept the title's emptied pool.
- Test `tests::scene_audio::a_custom_trax_list_applies_after_the_title` (song 0 Off, Racing
  and Menu; five boot seeds each).
- The race's end: `Game::begin_loading_in_flow` (the ending scene's load stops only a score
  piece), the ending scene left out of `music_context`'s scene test, and the post-race chain
  (Event Results, UI_PostRaceReward, UI_SponsorPopup, the sponsorship screen over the race) and
  the status screen on the game's side. Test `tests::scene_audio::a_race_song_plays_on_through_the_results`.
- **Inferred**: the original applies the loaded profile when "Load complete." is answered
  (the pools refilled after Continue, observed); the clone applies it as the message opens.
  Only the refill's moment differs, a few seconds while the title's song plays.
