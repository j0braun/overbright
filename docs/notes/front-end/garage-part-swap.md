# Garage FE car: part changes built off the frame

Issue: the frame rate dropped sharply while picking different rims.

## Cause (measured)
`draw_garage` (src/bin/game.rs) rebuilt the whole FE car on every change of
the request's parts: `Car::load_upgraded` (the car's solids and texture packs
from the cache, the shared part files, the part database walk) and then a new
`CarRenderer` (every texture and the vertex/index buffers uploaded), both on
the main thread inside the frame. `game --bench rims 30` (UI_Rims_Browser's
style strip from Customize, SHOWROOM): the frame of a rim step took
**44 ms on average, 134 ms worst** (normal frames 5.5 ms); `garage: car`
5-85 ms, `garage: car renderer` 4-26 ms (NFSU2_TIMING). Every part preview
(body shop, specialties, paint, perf parts) took the same path.

## Fix (clone deviation, performance only)
A part change while a car is shown is built on a thread (`GarageCarJob`):
`Car::load_upgraded` and the renderer (`garage_car_renderer`, the same
constructor and environment cube as before) on a `Gpu::car_worker` handle (the
same device, queue and `CarShared` pipelines). The shown car stays drawn until
the job is done; then it is put in place in one step (the part-animation
channels reset as before, "a new FE car starts with everything shut") and the
old car is freed on a thread. A job finishing for a stale request is still
shown, then the latest request starts. One job runs at a time.

The first car of a garage scene, and every change in the deterministic headless
modes (`--shot`, `--selftest*`, `--flow`, `--playthrough`) or with
`NFSU2_GARAGE_SYNC=1`, are still built in the frame, so shots are unchanged.

After: step frame **5.5 ms avg** (24 ms worst, a host-load outlier; the
in-between frames' worst was 24 ms too), the swap 0.0-3 ms. The new rim is
drawn 6-10 frames (~30-50 ms wall) after the key on the loaded host; the
thread's CPU time is 12-16 ms per car. Evidence: `evidence/garage-part-swap/`.

## Native
**Not traced**: how the original's front end swaps a previewed part (whether
it loads parts asynchronously and shows the old part meanwhile) is open. The
picture of a settled preview is the same as before; only the frames in between
a key and the new part showing differ (old part still shown).

## Open
- Incremental swap (only the changed slot's draws/textures) and a cache of
  recently built cars by key would cut the latency further.
