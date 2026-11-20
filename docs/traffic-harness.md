# Traffic parity harness

Test tooling only: the game's behaviour is unchanged. One command runs seeded headless sessions of
this game and, given a private Wine copy of the original (the maintainer's Wine setup with fastgpu.sh,
`PARITY_WINE`), the same scenario in SPEED2.EXE. Both sides log every traffic car to one table, and one
script computes the same statistics from both and prints them side by side with their spread.

| Part | What |
|---|---|
| `src/bin/game/tests/traffic_harness.rs` | this game's side: the ignored test `tests::traffic_harness::traffic_harness` (dev-harness test binary), the route and the scripted player |
| `tools/parity/trafh_native.py` | the original's side: boot, free roam or Quick Race, memory poll, the same scripted player through keys |
| `tools/parity/trafbot.py` | the scripted player for the original (the same rules as the test's `Bot`) |
| `tools/parity/trafh.py` | `run` (both sides, then the report) and `stats` (any tables) |
| `devprobes` `traffic_walk_lanes --trafh` | the lane columns, from the route file, for both games' tables |
| `scripts/checks/traffic_harness.sh` | this game's side only, as an optional report-only check |

## Running it

Build once: `cargo test --profile gate --features dev-harness --bin game --no-run` and
`cargo build --profile gate -p devprobes --bin traffic_walk_lanes` (the scripts find both under
`$CARGO_TARGET_DIR/gate` or `/release`; `TRAFH_LANES` names the probe binary directly).
`NFSU2_ROOT` must name the game files (scripts/local.env).

```
# this game only, 3 seeds x 120 s, free roam standing (about 15 s of wall time)
tools/parity/trafh.py run --out OUT --scen stand --seeds 0-2 --seconds 120
# both games: 3 seeds here, 3 boots of the original (each about 1 min of boot plus the run)
tools/parity/trafh.py run --out OUT --scen bot --seeds 0-2 --seconds 300 --wine <your wine copy> --display 231 --runs 3
# the statistics of any tables (a folder or files; side by side by the `side=` of each header)
tools/parity/trafh.py stats OUT
tools/parity/trafh.py stats --json OUT/ours-stand-s0.tsv
# before / after a change: one column per folder and side
tools/parity/trafh.py stats --by-dir BEFORE AFTER
# the optional check (report-only; not in the fast or full sets)
scripts/checks/traffic_harness.sh OUTDIR
```

Scenarios (`--scen`):

| Scenario | This game | The original |
|---|---|---|
| `stand` | career free roam (stage 1), the player standing at `TRAFH_AT` (default 462.4, −46.2, heading −20.2°, the garage exit) | the stage-1 save's free roam (the same spot), the player standing |
| `bot` | free roam, the scripted player on the route from that spot (`OUT/route.tsv`) | the same route, the same player rules |
| `outrun` | (not yet) | an encounter car's challenge taken from free roam, the throttle held (see below) |
| `qr:circuit`, `qr:sprint`, `qr:drag` | Quick Race of that mode, its first track, traffic level 3, from the green; the player holds the throttle (`TRAFH_DRIVER=auto` the autopilot), in a drag the drag bot | the same through the menus (the mode list position is assumed for sprint and drag; check `OUT/*.nav/menu-*.png`, `--menu` overrides) |

`run` options: `--seeds` (`0-2`, `0,4,7`; 0 is the default stream, n the traffic seed knob
`NFSU2_TRAFFIC_SEED`; a seed over 0 also seeds the shared generator as a boot of its own, so
the pool's models differ as between boots of the original; `TRAFH_BOOT=0` keeps the default), `--seconds`, `--wine COPY --display N --runs N` (the original's side;
the copy must be your own: native.py refuses the shared one), `--native-only` (keeps this game's
tables, adds boots of the original; `bot` needs this game's `route.tsv` first), `--again`
(re-run boots already there), `--bin` (the test binary). The original seeds its generators from the
clock at boot, so each boot is an independent sample; this game's seeds are fixed streams.
This game's side runs under `scripts/checks/home_mask.sh` (the player's folders hidden); the
original's under fastgpu.sh (private Xvfb display, no sound, stopped by PID at the end), on DXVK
(`--wined3d` only on request: about half the original's rate). The original's side runs in real
time, so each boot is checked by the frame-rate guard (docs/developing/parity-harness.md "Renderer
and frame rate"): a boot below 100 fps or 95 % of real time ends with the table line
`# fps … valid 0`, is kept aside as `native-<scen>-r<k>.slow`, not counted, and `run` exits
non-zero; `--min-fps` / `--min-rate` change the limits. Read that page's "valid native
measurement" rule before quoting a native number.

Output in OUT: `ours-<scen>-s<seed>.tsv`, `native-<scen>-r<k>.tsv` (+ `.log`, `.nav/` screenshots),
`*.ann.tsv` (the lane columns), `route.tsv`, `report.txt`.

## The table

One header line `# trafh 1  side=ours|native  scenario=…  seed=…  level=…  driver=…  routes=…`,
then tab-separated rows, one per traffic car per sample, plus a row `P` for the player:

| Column | Meaning |
|---|---|
| `t` | game seconds from the start (free roam: the first logged tick; a race: the green) |
| `car` | the car (this game: its pool slot; the original: its car pointer) |
| `model` | its model (car record name) |
| `mode` | 0 not placed (parked or paused), 2 TrafficMover, 3 MeloMvr, 4 BeltMover; −1 the player |
| `x y z hd speed` | place, heading in degrees, speed (m/s; on the BeltMover the belt's) |
| `lane` | the game's own lane number for the car's walk (each game's numbering; for comparisons use the annotated `nlane`) |
| `action` | 7 Drive Traffic Lane, 2 Avoid Cars, 17 Obey traffic intersections; others as the original's action type |
| `target` | while avoiding: the tracked car (`P` the player, `R<k>` a racer) |
| `stopped active brake` | stopped for good, active (car +0x550), the brake-light flag |
| `ev` | `S` spawn (placed after not being placed, or moved over 40 m since its last row), `D` despawn |
| `note` | the player row: the driver and, for the bot, `state:lead` |

This game writes every second 1/60 s tick (30 a second, `TRAFH_EVERY`); the original's poll
reads about 20 samples a second of game clock. The statistics are per sample or per second, so the
rates compare.

`traffic_walk_lanes --trafh` adds `nrec nlane off wrong`: the nearest lane centre line running the
car's way within 8 m (record id, lane 2–9), the signed offset from it (right positive), and whether a
lane running the other way is nearer.

## The statistics

Per run, then per side as `mean [min-max] (runs)`; `*` marks a metric whose ranges do not overlap
(both sides with two or more runs). Placed = `mode > 0`.

| Metric | Meaning |
|---|---|
| `placed`, `within100` | placed cars per sample; share of placed rows within 100 m of the player (density) |
| `mode2/3/4` | share of placed rows per mover |
| `sp_p10/p50/p90/max`, `stopped` | speed distribution of placed cars; share under 0.5 m/s |
| `act_*`, `actions` | share of placed rows per action; `avoid_player` those tracking the player |
| `avoid_player_per_min`, `avoid_player_swerve`, `avoid_player_stop`, `avoid_player_len_p50` | Avoid Cars episodes (≥ 0.5 s) tracking the player per minute; the share that swerve (over 1.5 m off the lane or onto another lane) or stop; their median length |
| `brake_moving/standing` | brake-light share at ≥ / < 0.5 m/s |
| `spawn_per_min`, `despawn_per_min`, `spawn_d_*` | placements and parks per minute (not counting the first sample) and the placements' distance to the player |
| `long_stops`, `long_stop_max` | a placed car under 0.5 m/s for 30 s or more |
| `off_p50/p90`, `off_over1/2`, `wrong_side`, `off_unmatched` | lane offset of placed cars at ≥ 2 m/s (absolute), shares over 1 / 2 m, nearer a lane running the other way, no lane within 8 m |
| `lane_not_own` | share of those samples whose nearest lane is not the game's own lane for the car (both read as 2–5): a lane change under way |
| `lane_changes_per_car_min` | runs on one (record, lane) held ≥ 1 s; a change = the next held run on another lane of the same record within 2 s; per minute of moving car time |
| `lane_change_back_5s` | a lane change and back to the first lane within 5 s (issue #122 item 3) |
| `lane_excursions`, `lane_excursions_near_player`, `lane_excursions_per_car_min` | a short excursion: held ≥ 1 s on a lane, 0.2–5 s on another lane of the same record (not necessarily held), then back on the first and held ≥ 1 s (the near-player count: within 40 m) |
| `lane_changes_near_player_per_car_min` | the same for cars within 40 m of the player |
| `jam_share` | samples with ≥ 3 placed cars under 0.5 m/s within 30 m of each other |
| `jam_episodes`, `jam_cleared`, `jam_permanent` | jam spans ≥ 5 s; permanent = lasting ≥ 60 s to the end of the run |
| `jam_first_s`, `jam_permanent_from_s`, `jam_cleared_mean_s` | time to the first jam, to the permanent one, mean length of the cleared ones |
| `freeze_from_s` | the placer silent (no placement) for ≥ 120 s from then: the run froze |
| `mutual_share`, `headon_stops`, `headon_to_end`, `headon_max_s` | samples with a mutual Avoid pair (each the other's tracked car); such pairs both standing ≥ 5 s, those to the end, the longest |
| `player_speed_mean`, `player_km`, `bot_overtakes`, `bot_follow_share` | the player's driving |
| `follow_episodes`, `lead_lane_moved`, `lead_and_back`, `lead_avoids_player` | the bot's follow / overtake episodes per lead car, and the share in which that car's nearest lane changed, changed and came back, or it tracked the player in Avoid Cars, from the episode's start to 5 s after its end |
| `models`, `pool` | model shares over the placements, and over the pool's cars |

## The player bot

The same rules on both sides (`Bot` in the test, `trafbot.py`), binary keys, a decision every 50 ms of
game time: drive the route (a forward traffic lane's line, a point every 5 m, junctions by a fixed
seed, from the start spot) at 22 m/s, steering to a point 8–30 m ahead (6 m + 0.6 s × speed). A car
ahead within 60 m and 2.2 m of the bot's line, not oncoming, is followed (speed = its speed + 0.5 ×
(gap − 10 m)); after 4 s within 15 m the bot overtakes 3.5 m to the left of the route at the car's
speed + 6 m/s (at least 12) until 12 m past it (15 s at most), then returns to the line. Stuck (gas,
under 0.5 m/s for 3 s): 2 s of reverse with the steering mirrored. The drag bot holds the throttle,
shifts up at 1.5, 3.2, 5.3, 8.0 and 11.5 s and taps left (else right) when a car is within 50 m
ahead in its lane and the side lane is free. The original gets the keys through xdotool on its
private display, so its reactions lag this game's by a few milliseconds.

## Screenshots, horns, outruns

- `--shots N` (`run` and `trafh_native.py`): the original's bot takes a screenshot of the car it
  follows (within 15 m), at most N, 20 s apart, into `native-*.nav/follow-*.png` (tail and brake
  lights at night, in rain when it rains).
- `--audio`: the original's sound goes to `native-*.wav` (fastgpu.sh's null sink: nothing is heard)
  and its sound instances to `native-*.events.txt` (native.py's EventWatch).
  `tools/parity/trafhorn.py TABLE [EVENTS]` finds the moments the traced horn test holds in a table
  (CARSFX_TrafficHorn 0x47E6E0 / 0x475B30: an oncoming placed car within 20 m of a player over
  15 m/s), the horns that rule gives (3 s per car, a 0.3 draw or 5 s since the last horn), and with
  EVENTS the horns the original played (ENV_STATIC variants 5–7) and how many fall on such a
  moment.
- Outruns, the original: `trafh_native.py --scen outrun --save <a save with encounter cars>
  --profile <its name>` follows an encounter car out with the throttle held (a key) until the career
  offers the challenge, takes it, chases the rival (throttle held, steering at it) and logs the table with the race type in the player's note
  (`race_type7` while the outrun runs). The parity cave's outrun start (outrun_traffic_p12) died in
  every recording here (three tries, with and without the table poll beside it), so this start
  uses no cave. This game's side has no outrun session yet; `tools/parity/par2trafh.py OUT
  [native]` turns any parity record with traffic fields into a table (e.g. `parity.py clone`).
- Older native walk logs (trafwalk.py's native-walk.tsv): `tools/parity/walk2trafh.py LOG [scenario]`.

## Limits

- The original's side needs a Wine copy and runs in real time (boot about 1 min); this game's side
  runs about 12 times faster than real time.
- `stand` in the original is the save's resume spot; `TRAFH_AT` moves only this game's player.
  `--at` moves the original's.
- The lane columns need the developer probe; without it the lane metrics are left out.
- Quick Race sprint and drag in the original go through assumed menu positions (check the menu
  screenshots); only the circuit's menu path was checked.
