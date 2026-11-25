# Traffic AI coverage map

Every function of `SPEED2.EXE` that decides how traffic cars appear, drive, avoid, stop, light up and
honk, and how far Overbright reproduces each one. The status words and columns are explained in
[README.md](README.md). Addresses and offsets are explained in the traffic chapter of the address map,
[formats/ai.md](../../formats/ai.md) ("Traffic"), and in [tracing/ai-lanes.md](../tracing/ai-lanes.md).

"Ours" names the file under `src/` and the function. `ai/traffic/` is `src/ai/traffic/`,
`sim/traffic/` is `src/bin/game/sim/traffic/`. Evidence words: **verified** (read in the original's
code), **observed** (read in the original under Wine), **inferred**, **open**. Harness numbers come from
the [traffic parity harness](../../traffic-harness.md) and are written as ours / original, mean
[min–max] over runs; `stand` is free roam with the player standing at the garage exit, `bot` free roam
with the scripted player driving, `qr:circuit` and `qr:drag` Quick Races at traffic level Maximum.

Last checked against the default branch with the lane check, the kept walk offsets, the race
coordinate steps and the own-lane re-plan in place; the encounter, junction and Quick Race circuit
rows measured again with the original at a fixed 1/60 s frame.

## Summary

| Status | Rows |
|---|---|
| PORTED | 80 |
| SIMPLIFIED | 10 |
| GUESSED | 8 |
| MISSING | 4 |
| NOT NEEDED | 11 |
| **Total** | **113** |

The behaviour the harness measures in free roam is within the original's spread for density,
placements, speeds, lane keeping, lane changes, Avoid Cars, jams, freezes while standing, brake
lights, horns and models. Three things stay outside. Encounter cars hit traffic far more often (8.4
contacts per 5-minute run against 1.4, with the original at a fixed 1/60 s frame). In a Quick Race
circuit, traffic spends more time in Avoid Cars (0.58 against 0.42) and jams more (0.11 against
0.016). Head-on stands at crossings while the player drives are inside the original's range, but
ours stays above its mean. Each has its row and its place in the gap list at the end.

The original's AI counts frames, so its numbers change with its frame rate (a traffic car thinks
every 2N frames). Native numbers marked "fixed 1/60" come from runs of the original at a fixed 1/60 s
frame, this game's own world step ([parity-harness.md](../../developing/parity-harness.md),
"Renderer and frame rate"). The other native numbers come from older real-time runs on wined3d at an
unknown frame rate: they are indicative only until they are measured again.

## Pool, models and skins

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x4FE9F0 | Traffic_SetupPool | cars in the pool: track byte +0xEC + 2·level + reversed; none at level 0 | `ai/traffic/pool.rs:pool_size` | **PORTED** | verified; 16 cars at track 4000, level 3 in both games | low |
| 0x412BE0 | Traffic_AssignModels | a model per pool car from the traffic types under their zone caps, random while fewer than 7 models are used | `ai/traffic/pool.rs:assign_models`, `sim/traffic/mod.rs:populate` | **PORTED** | verified, draw on the shared generator (0x412C9A); harness model shares within the spread (drag 8 vs 4 pools, free roam 8 vs 4) | low |
| 0x412840 | TrafficModels_Reset | manager reset: a skin byte per model (0x61BC30), body paint (0x6371F0) | `sim/traffic/mod.rs:load` (draws only) | **PORTED** | verified; the draws are kept in order (`bin/game/sim/rng.rs:world_build`), the results do not show: every traffic car draws `<CAR>_SKIN1` | low (draw order) |
| 0x61BC30 | CarSkin_Pick | skin byte = table[rand(count)] | `rng_sites.rs:model_slots` (draw only) | **NOT NEEDED** | verified: the remap 0x636850 maps SKIN1 to SKIN1 for all 15 traffic types, so the byte never reaches a texture; the draw is kept | none |
| 0x6371F0 | Ride_RandomPaint | ride slot 0x3F paint part per model | `ai_looks.rs:random_paint` (draw only) | **NOT NEEDED** | verified: the attribute only replaces `CARSKIN`, which no traffic geometry uses | none |
| 0x412950 | TrafficModels_Stream | every 5 s frees and loads model data by zone, not bound to any car | — | **NOT NEEDED** | verified: no placed car changes model during a track load; the clone keeps every pool model loaded | none |
| 0x412DB0, 0x420030, 0x420140, 0x412910, 0x41AC40 | — | load the chosen models; bind each car to its entry; teardown; drop users | `sim/traffic/mod.rs:load`, `load_cars` | **PORTED** | verified (model fixed per track load, pool rebuilt on the next free roam) | low |
| 0x412720 | — | save / restore of the model manager | — | **NOT NEEDED** | the clone rebuilds the pool on load; no player-visible state | none |

## Placement, keep test and race fill

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x4179F0 | Traffic_PlacerCtor | placer state, race point count (0x409900) | `ai/traffic/place.rs:fill_race` | **PORTED** | verified | low |
| 0x42F910 | Traffic_PlacementPass | timer, round robin over the slots, one car a pass (16 in drag), park or re-place | `sim/traffic/think.rs:traffic_place_pass` | **PORTED** | verified; placements 33.8 vs 39.2/min (bot), 28.2 vs 28.8/min (stand) | low |
| 0x42F800 | Traffic_PlacerStep | per try: which human the car is for, teleporter odds, then 0x42E9F0 | `ai/traffic/place.rs:placer_try_draws` | **PORTED** | verified with the draw-site list | low |
| 0x42E9F0 | Traffic_Placer | free-roam / race placement: search ahead, lane, gates, speed | `ai/traffic/place.rs:spawn`, `spawn_from`, `spawn_race`, `reach_measure` | **PORTED** | verified (asm 0x42EBBB–0x42EC19, 0x42ED4C–0x42EDA9); spawn distance p50 282 vs 281 m (drag) | low |
| — (the player's road node read by 0x42E9F0) | — | where the placer's search starts | `ai/traffic/place.rs:road_node` | **GUESSED** | inferred: the first point of the nearest segment in 2D (`ai.traffic.road_node`); spawn distances match | low |
| 0x417D80 | Traffic_PlacerStarts | the player's road and up to two roads beside it | `ai/traffic/place.rs:starts`, `spawn_from` | **SIMPLIFIED** | verified; the clone takes the route file's record order for 0x88D21C's list, and the cursor's rule after a failed try is inferred (`ai.traffic.spawn_from`) | low |
| 0x415130 | AIRoute_WalkDistance | the placers' road walk ahead (count 1, sixth argument 0) | `ai/traffic/place.rs:placer_walk`, `ai/traffic/walk.rs:road_walk` | **PORTED** | verified (decompile and asm 0x4154CA–0x415582) | low |
| 0x418110 | Traffic_SpawnPointOK | the placement gate: start line, spacing, 2D or race coordinate | `sim/traffic/place.rs:drag_gate`, `ai/traffic/place.rs:coordinate_point` | **PORTED** | verified | low |
| 0x418240 | Traffic_Spacing | min + rand(max − min) from the zone's spacing row | `ai/traffic/place.rs:spacing` | **PORTED** | verified | low |
| 0x410FC0, 0x411060 | Traffic_SpacingTest, Traffic_CoordNearCar | spacing to the other placed cars, 2D or race coordinate | `ai/traffic/place.rs:coordinate_point`, `race_coord_step` | **PORTED** | verified | low |
| 0x409840, 0x409890, 0x4098D0, 0x410DE0 | Traffic_DistToPlayers, Traffic_CoordNearPlayer, —, Traffic_NearPlayer | distance to the humans, 2D or along the race road | `ai/traffic/place.rs:race_coord_step`, `sim/traffic/think.rs:traffic_place_pass` | **PORTED** | verified; the keep limit 250 m re-observed (parks at 244–270 m) | low |
| 0x417BB0 | Traffic_NeedsPlacing | the keep test every 0.3 s: parked, or too far from the player | `sim/traffic/think.rs:traffic_place_pass`, `ai/traffic/stream.rs:ring_presence` | **PORTED** | verified | low |
| — (writer of car +0x8FC for traffic) | — | a traffic car's race coordinate, moved on in about 11.6 m steps | `ai/traffic/place.rs:race_coord_step` | **GUESSED** | observed: steps of 11.1–12.1 m (median 11.6) at 5 and 20 m/s in drag and circuit; no store found by a static search (`ai.traffic.race_coord_step`); drag placed 4.54 vs 4.65 cars after the port (was 3.64); real-time wined3d runs, not yet measured again at fixed 1/60 | med: race traffic density |
| 0x4112A0 | Traffic_JamTest | no placement while the player is slow and traffic queues | `sim/traffic/think.rs:traffic_think`, `ai/traffic/place.rs:placer_try_draws` | **PORTED** | verified; stand jam share 0.021 [0.001–0.05] vs 0.081 [0–0.45] | low |
| 0x410CA0 | Traffic_PickLane | a lane of a record: ten random picks, then the first; in drag the human's lane with odds ½ | `ai/traffic/walk.rs:pick_lane`, `ai/traffic/place.rs:drag_lane` | **PORTED** | verified | low |
| 0x42C950, 0x42C670 | Traffic_PlaceCar, Traffic_CreateOnPoint | put a car on its lane: ground, heading, mover, fresh driver, lane speed | `sim/traffic/place.rs:place_traffic`, `place_on_road` | **SIMPLIFIED** | verified; the ground placement's velocity projection on a slope is the clone's (`place_on_mover` notes) | low |
| 0x42E620, 0x417C60 | Traffic_DragPlacement, Traffic_DragFindPoint | drag placement: flag draw, point 250 m on, lane | `ai/traffic/place.rs:spawn_drag`, `drag_point` | **PORTED** | verified; drag placed 4.54 vs 4.65, within100 0.241 vs 0.261 | low |
| 0x42F7A0, 0x42E4A0, 0x42E2C0, 0x411110, 0x417C30, 0x409900 | Traffic_SetupPlace, Traffic_DragFill, Traffic_RaceFill, … | race set-up fill: drag lines the start, other races take random race points | `ai/traffic/place.rs:fill_drag`, `fill_race`, `sim/traffic/place.rs:traffic_fill_plan`, `traffic_fill_apply` | **PORTED** | verified; observed on the Bayview Bridge drag (ten cars from 452.5 to 945); which racers 0x5F0FC0 lists is inferred as the player alone (`game.traffic_fill-3`) | low |
| 0x42E0B0, 0x42E790 | Traffic_SetupTeleporter, — | teleporter placements and the "ambient" goal | — (draws kept) | **NOT NEEDED** | verified: no retail route file has teleporters (0x883DC8 = 0); the odds draw is kept in order | none |
| 0x413BB0 | TrafficGoal_TakeBranch | junction choice of a new walk, drawn on the world generator | `ai/traffic/place.rs:lane_stream`, `starts` | **SIMPLIFIED** | verified rule; the clone draws it from its own per-lane stream, not the world generator (open in [rng.md](../rng.md)) | low: same odds, other cars turn |

## Controller, think and goal

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x426580, 0x425AB0 | TrafficController_Ctor, CarDriver_Ctor | the traffic controller and the racers' driver, throttle cap 1 | `sim/traffic/place.rs:place_traffic` | **PORTED** | verified (cap 0x425B44: 1.0 for any class but 2) | low |
| 0x42A050 | TrafficCtl_Reset | reset on placement, resume of a paused car | `sim/traffic/think.rs:placed_active` | **PORTED** | verified | low |
| 0x429D90 | TrafficController_Update | stuck test, then on its frame: leave crossings, lane update, intersection, proposals, action, mover switch | `ai/traffic/think.rs:think`, `sim/traffic/think.rs:traffic_car_frame`, `traffic_lane_think` | **PORTED** | verified; schedule by the frame counter 0x7FB724 (`think_slot`); ours: rate check identical at 30/60/120/200 fps; the original's think follows its frame rate (every 2N frames: 0.4 s at 60 fps, 0.2 s at 120), so it is compared at fixed 1/60 | low |
| 0x408C40, 0x4022D0 | TrafficCtl_OnWorldLoaded, RacerCtl_OnWorldLoaded | the world under a car loaded or unloaded: mover and active flag | `sim/traffic/think.rs:traffic_stream`, `placed_active` | **PORTED** | verified; the stream rule observed | low |
| 0x4020D0, 0x402110 | — | pause / resume one car (body pause count, mode 0) | `sim/traffic/think.rs:pause_traffic`, `placed_active` | **PORTED** | verified | low |
| 0x4099B0, 0x409A30, 0x430340, 0x430310 | Traffic_PauseAll, Traffic_ResumeAll, NIS_TrafficThinkOn, NIS_ResumeTraffic | traffic before the green and during scenes | `sim/traffic/think.rs:traffic_coast`, `sim/traffic/mod.rs:load` | **GUESSED** | observed in a Quick Race drag, verified in the disassembly except the resumed car's steering: it keeps braking (`game.traffic_coast`) | low |
| 0x426C20 | TrafficGoal_Choose | Obey (600) with an intersection ahead, else Avoid Cars (300), else Drive Traffic Lane | `ai/traffic/think.rs:think` | **PORTED** | verified; the goal observed for 16 of 16 cars (kind 1, type 7) | low |
| 0x4145B0 | TrafficGoal_ProposeAvoid | Avoid Cars when 0x402ED0 (look 99999) finds an object within 5 s that 0x4140E0 keeps | `ai/traffic/think.rs:think` | **PORTED** | verified; the fast-racer exemption needs goal kind 2, never true for retail traffic (observed) | low |
| 0x4149E0, 0x41CE30 | TrafficGoal_ProposeObey, ObeyIntersections_Done | Obey while the walk has an intersection ahead | `ai/traffic/think.rs:think` | **PORTED** | verified | low |
| 0x403120, 0x401990, 0x429F56–0x42A02D | AIAction_Status, AIAction_IsDone | status by time limit / given up / done; replace by priority; mover switch on types 8, 1, 2 | `ai/traffic/think.rs:think`, `ai/traffic/walk.rs:take_avoid` | **PORTED** | verified (asm 0x429FF8–0x42A028); 26–39 % of placed samples on the MeloMvr observed | low |
| 0x4242D0 | AIActionDriveTrafficLane_Update | the lane path and target speed | `ai/traffic/plan.rs:update` | **PORTED** | verified | low |
| 0x426D70 | AmbientGoal_Choose | teleporter-placed cars only | — | **NOT NEEDED** | verified: built only for a teleporter placement, none in the retail data | none |
| — (actions Pass, JustDrive, IJustGotHit, type 0x13 seen briefly on traffic) | — | one-sample actions on native traffic, 0.03 % of samples | — | **MISSING** | observed only; the clone's traffic goal proposes Drive, Avoid and Obey; whether they draw random numbers is open | low |

## Walk and route

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x428030 | AIRoute_ExtendPoints | grow the walk to max(125 m, 4 s × speed), then lane reaches | `ai/traffic/walk.rs:extend` | **PORTED** | verified | low |
| 0x422220, 0x41EC20, 0x41EE20, 0x41CFA0 | AIRoute_Walk, AIRoute_NextPoint, AIWalk_AddTurn, AIRoute_JunctionDecision | step from point to point, junctions taken or declined | `ai/traffic/walk.rs:walk`, `walk_gated` | **PORTED** | verified; turns at junctions original 0/85, ours 3/583 (free-roam runs) | low |
| 0x40A620 | AIWalk_TrimUnanchored | drop a turn taken at the walk's end | `ai/traffic/walk.rs:extend` | **PORTED** | verified | low |
| 0x418DA0, 0x40CBC0 | AIRoute_InsertGapPoints, — | 8 m points between control points; the step between two points | `ai/traffic/walk.rs:sub_points`, `gap`, `sync_xs` | **PORTED** | verified (asm 0x418E8B–0x418E9A) | low |
| 0x41A180 | AIRoute_SmoothGapPoints | lane offsets set once per extension, from the second control point back | `ai/traffic/walk.rs:smooth`, `walk_points` | **PORTED** | verified; record 734 cp 19 reads 0.71 / 0.42 / −0.14 on both sides | low |
| 0x426810, 0x419670 | AIRouteList_BuildFromNode, AIWalk_DropBehind | build the walk from a node; drop points behind the car | `ai/traffic/walk.rs:walk_points`, `walk_count` | **PORTED** | verified | low |
| 0x418AE0, 0x411CE0 | AIWalk_FindIntersection, — | lane reaches (walk +0xB4) and the first intersection ahead | `ai/traffic/walk.rs:lane_reach` | **GUESSED** | verified except the reach on a record entered by a junction, taken from its first point (observed timing; `ai.traffic.lane_reach`) | low |
| 0x40ABB0, 0x40AB10 | TrafficWalk_AtEnd, — | park the car at the walk's end on a record with +0x0C = 1 | `ai/traffic/walk.rs:parks_at_end` | **SIMPLIFIED** | verified; walk +0xC8 has no traced writer and is taken as clear (`ai.traffic.parks_at_end`); no shipped record triggers it | none |
| 0x417750 | TrafficCtl_DeadEndAhead | stop before a dead end the car reaches soon | `ai/traffic/walk.rs:ends_ahead` | **SIMPLIFIED** | verified; the car's place on the walk is inferred for the road entry +0x18 | low |
| 0x4122C0, 0x4123C0, 0x401C40 | AIRoute_LanePointsAt, AIWalkPoint_LaneLateral, AIRoute_LaneLateral | a walk point's place on a lane and the lane's lateral | `ai/traffic/walk.rs:walk_place`, `ai/traffic/plan.rs:point` | **PORTED** | verified | low |
| 0x40A8C0, 0x40D0B0, 0x40D300 | —, — | lane speed bytes and grade per walk point | `ai/traffic/plan.rs:speed_byte`, `grade` | **PORTED** | verified | low |

## Planner, lane choice and path keeping

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x428720 | AIRoute_Update | one planner update: drop passed points, extend, road entries, lane check | `ai/traffic/plan.rs:update`, `lane_check` | **PORTED** | verified (asm 0x428A6B–0x428C45); 20 of 20 native cars switched lane 4 → 3 at x ≈ 407, ours now too | low |
| (0x428720's nearest point) | — | the nearest walk point is not kept between frames; 0x4198C0's skip of far points | `ai/traffic/plan.rs:update` | **GUESSED** | inferred (`ai.traffic.update`, `update-2`, `update-3`): the nearest never goes back, the skip only speeds up the search, 2D | low |
| 0x4053B0 | — | set the lane the lane check picked | `ai/traffic/plan.rs:lane_check` | **PORTED** | verified; lane changes 0.240 [0.078–0.424] vs 0.236 [0.188–0.277] per car-minute (stand), 0.623 vs 0.591 (bot) | low |
| 0x40F4A0 (lane check fallback, kind 1/3) | AI_RequestLane | with no lane allowed, ask the lane found at the car's place | — | **MISSING** | verified call, flag not traced; the car keeps its lane instead | low |
| 0x41D530, 0x41D160 | AIPath_Build, AIPath_Reset | build the driver's path on the walk; reset | `ai/traffic/plan.rs:plan`, `update` | **PORTED** | verified | low |
| 0x420D40 | AIAction_FollowLanePath | the lane path; keeps a traffic car's path front between thinks (+0x1D9) | `ai/traffic/plan.rs:plan` | **PORTED** | verified; +0x1D9 observed 1 for every traffic car | low |
| 0x41BFE0 | AIDriver_NeedsReplan | re-plan when the kept path is more than 10 m off (1.5 m for racers) | `ai/traffic/plan.rs:plan` | **PORTED** | verified (`REPLAN`); with 1.5 m the bot jam share drops to 0.053 (experiment, not kept) | low |
| 0x40FC80, 0x415B70, 0x415D50 | AILane_ClampOffset, AIRoute_LaneAtPoint, AIRoute_AddPathPoint | move the path toward the lane's offset at its rate; lane kept per point; add points | `ai/traffic/plan.rs:update`, `ai/traffic/walk.rs:keep_lane` | **PORTED** | verified; offsets p50 0.117 vs 0.086 m, p90 0.79 vs 0.80, wrong side 0.006 vs 0.0026 (stand) | low |
| 0x41FD50 (+0x40B1F0, 0x40B2A0, 0x40B780, 0x40B840) | AIRoute_SetLaneRoom | a walk point's room to the road edges | `ai/traffic/plan.rs:room` | **SIMPLIFIED** | verified except the room by the next point, **open** (garbled decompile), and a margin float taken as 0 (`ai.traffic.room`, `ai.traffic.LANE_MARGIN`) | low |
| 0x411DD0 | — | speed at s along the walk | `ai/traffic/plan.rs:update` | **PORTED** | verified | low |
| 0x41C380, 0x411FA0, 0x40C840 | —, —, Graph_EvalPairs | crest cap from the grade change ahead | `ai/traffic/plan.rs:crest_cap` | **PORTED** | verified; test `crest_cap_pulls_towards_10_ms` | low |
| 0x422150 | AIAction_HandPathToDriver | hand the path to the driver; belt rebuild | `sim/traffic/think.rs:traffic_lane_think`, `traffic_stream` | **PORTED** | verified | low |

## Lane tests

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x412500 | AILane_Allowed | a lane exists here and reaches the walk's end or 25 m farther | `ai/traffic/walk.rs:lane_allowed` | **PORTED** | verified | low |
| 0x40AE90, 0x40AA90, 0x40AF30 | AIRoute_LaneByte, AIRoute_LaneEnd, AIRoute_LaneReach | lane byte at a point; reach to the end; reach 25 m farther | `ai/traffic/walk.rs:lane_allowed`, `ai/traffic/plan.rs:lane_check` | **PORTED** | verified | low |
| 0x40AF30(−0.1), 0x40B110 (drag) | AIRoute_LaneReach, AIRoute_DragLaneTable | in a drag race each lane in between must pass too | — | **MISSING** | verified call (not ported, `walk.rs:lane_allowed` notes); drag lane changes 0.42 [0–1.55] vs 0.74 [0.29–1.04], lane_not_own 0.082 vs 0.068 (overlapping) | low |
| 0x40F4A0 | AI_RequestLane | the lane nearest the car's lateral at a point | `ai/traffic/avoid.rs:standing_lane` | **GUESSED** | verified except the side backward, taken as the point's travel direction (`ai.traffic.standing_lane`) | low |

## Avoidance (Avoid Cars as a traffic car)

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x6148F0 | AIPlanner_BuildRoadEntries | objects within the reach: relative velocity in each one's own walk frame, extents, lane offsets, sorted | `ai/traffic/avoid.rs:entries` | **SIMPLIFIED** | verified frames (0x6149E5, 0x614D78), 3D reach; not ported: the 15-entry cap, 0x403A40's second chance for far objects, car +0x998 (0x5EC070) (`ai.traffic.ENTRY_REACH`, `ai.traffic.entries` inferred for races) | low |
| 0x419B20, 0x4198C0, 0x419C10, 0x419BC0 | AIWalk_Locate, AIRoute_NearestNode, AIWalk_EntryLocate, — | each object's place on the walk | `ai/traffic/avoid.rs:nearest_place`, `entries` | **PORTED** | verified | low |
| 0x402ED0 | AI_FindObjectAhead | the first object in the side window ahead: gap, time, 20 m standing rule | `ai/traffic/avoid.rs:car_ahead` | **PORTED** | verified value by value; oncoming cars in a northbound window 5–7 % vs 0–8 % | low |
| 0x402CC0 | AI_BoxAhead | box test along the velocity | `ai/traffic/avoid.rs:box_ahead` | **GUESSED** | verified except the backing-up test (+0x3EC) and the box taken as the hull's (`ai.traffic.box_ahead`, `box_ahead-2`) | low |
| 0x4146C0 | AI_LaneWindow | may the car take lane l: 0x412500 and nothing in the way | `ai/traffic/avoid.rs:lane_free` | **PORTED** | verified | low |
| 0x424CB0, 0x424B40 | AIAction_TrafficAvoid_Update, — | lane changes around a closing human car, own lane included; else slow / stop | `ai/traffic/avoid.rs:avoid_update` | **PORTED** | verified (asm 0x424DFB, 0x425006); Avoid on the player 5.35/min vs 5.55, swerving 0.35 vs 0.38, stopping 0.19 vs 0.12 (bot) | low |
| 0x405410 | AI_SetLane | move to lane l and plan afresh (rate 0.151) | `ai/traffic/avoid.rs:set_lane` | **PORTED** | verified; that the argument is planner +0x30C is inferred (`ai.traffic.set_lane`) | low |
| 0x41CD10 | AIActionTrafficAvoid_Done | done once the tracked object is gone or pulls away | `ai/traffic/avoid.rs:avoid_done` | **PORTED** | verified | low |
| 0x424CB0 slow rule (0x424CB0 with 0x4140E0, ≤ 15 m) | — | 6 m/s, then 0 within 15 m of the tracked car | `ai/traffic/avoid.rs:avoid_update` | **PORTED** | verified; the head-on deadlock it allows is native (stand freezes 4/6 from 710–1362 s vs 3/9 from 213–519 s; 34.4 vs 23.4 min to a freeze) | med |
| 0x60C000, 0x60C130, 0x60BFB0, 0x60C020 | AI_RoadEntryGap, AI_FindTrackedRoadEntry, AI_CarLaneOffset, — | gap, tracked entry, lateral off a lane | `ai/traffic/avoid.rs:tracked`, `off_lane`, `ai/actions/entries.rs:gap` | **PORTED** | verified | low |
| 0x4140E0 | AI_IgnoreCrossing | drop a car crossing at the same intersection | `ai/traffic/avoid.rs:avoid_done`, `lane_free` | **PORTED** | verified | low |
| 0x404910 | AI_EntryReach | the controller's reach: max(125 m, speed × vt+0x60) | `ai/actions/entries.rs:reach` | **SIMPLIFIED** | the vt+0x60 term is **open** (`ai.actions.predicted`) | low |

## Intersections

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x413800, 0x408FB0 | — | leave every intersection slot each think; find the road's nearest car again | `ai/traffic/crossings.rs:leave` | **PORTED** | verified | low |
| 0x417890, 0x410B50 | — | register at the intersection ahead: distance and time | `ai/traffic/think.rs:think` | **PORTED** | verified | low |
| 0x412170 | Intersection_WaitLine | the wait line: 12 m before the crossing road's edge less the car's place | `ai/traffic/crossings.rs:STOP_LINE` | **SIMPLIFIED** | verified except the lane object's +0x0C, taken as 0 (its shape types 1 and 3 are **open**) | med: where crossing cars stop |
| 0x410800 | — | the other road's near edge along the direction | `ai/traffic/crossings.rs:stop_back` | **PORTED** | verified | low |
| 0x4111C0, 0x409110 | — | each placer frame a road keeps the right of way while a car is within 12 m | `ai/traffic/crossings.rs:tick` | **PORTED** | verified | low |
| 0x4109A0, 0x410A10, 0x410A50, 0x408F80 | Intersection_MayGo, — | which road may go; claim; two cars crossing; nearest car's time | `ai/traffic/crossings.rs:may_go`, `claim`, `crossing`, `nearest` | **PORTED** | verified | low |
| 0x425150 | AIAction_Intersection_Update | Obey: car ahead within 30 m → 0 within 15 m, else ≤ 6 m/s; wait for the right of way | `ai/traffic/think.rs:think`, `ai/traffic/crossings.rs` | **PORTED** | verified; test `crossings_take_turns` | low |
| — (two traffic cars meeting head-on at a junction) | — | who yields when both are in Avoid Cars at a junction outside the 38 intersections | `ai/traffic/avoid.rs:avoid_update`, `avoid_done`, `car_ahead` | **PORTED** | verified: the original has no yield, priority or timeout rule there; 0x4145B0, 0x424B40, 0x424CB0, 0x41CD10, 0x4140E0 / 0x410A50 traced value by value and equal ([formats/ai.md](../../formats/ai.md) "Two traffic cars meeting at a junction"). Observed: bot stand pairs 0.41 [0–2] vs 0.20 [0–2] per 5 min (64 vs 40 real-time runs); head-on stands 0.75 [0–4] vs 0.5 [0–3] (16 vs 12 runs, fixed 1/60). Ours' cars track the oncoming car more often per meeting (0.27 vs 0.20); that cause is **open** | med |

## Stuck test and jams

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x408DE0 | TrafficCtl_StuckTest | stop for good (+0x710) after a contact over 30 or a stall under 1 m/s | `sim/traffic/think.rs:traffic_car_frame`, `traffic_stop` | **PORTED** | verified and observed: every stop reads a record of 30.2–35.3 (ours 30.0–35.8) | low |
| 0x4023E0 | AICtl_AimAhead | aim 1 m ahead at target 0 when stopped | `sim/traffic/think.rs:traffic_stop` | **PORTED** | verified | low |
| — (release of a stopped pair or a +0x710 car) | — | none: the car stands until the placer re-places it 250 m away | `sim/traffic/think.rs:traffic_place_pass` | **PORTED** | verified: no code releases them; observed pairs standing to the end of the run | low |
| 0x408ED0, 0x42C930 | TrafficController_OnContact, — | a resolved contact takes the car off its TrafficMover | `sim/traffic/think.rs:traffic_settle`, `ai/traffic/mover.rs:step` | **PORTED** | verified | low |

## Driver and movers

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x421030, 0x41D8D0, 0x415F80 | AIDriver_Update, AIDriver_SpeedControl, AIDriver_LookAhead | the racers' driver in mode 3 (TrafficMover) or 4 (MeloMvr), look-ahead max(0.55 s × v, 5 m) | `sim/traffic/think.rs:traffic_drive`, `ai/driver/` | **PORTED** | verified; see the [racer map](racer-ai.md) | low |
| 0x408740, 0x408820, 0x5764E0 | TrafficMover_Ctor, TrafficMover_Update, — | the kinematic mover: turn about the rear axle, speed table to 55 mph | `ai/traffic/mover.rs:step`, `mover_table` | **PORTED** | verified; test `mover_turns_and_holds_its_speed` | low |
| 0x5F46B0, 0x5F4410 | — | mover switch by the world under the car: 2 TrafficMover, 3 MeloMvr, 4 BeltMover | `sim/traffic/think.rs:traffic_stream`, `ai/traffic/walk.rs:take_avoid` | **PORTED** | verified end to end | low |
| 0x5F5020, 0x5F4A00, 0x5E72F0 | — | put a TrafficMover car on the ground under its wheels | `sim/traffic/place.rs:place_on_mover` | **SIMPLIFIED** | verified; the clone's `road_below` stands in for 0x5E72F0 and the velocity projection differs on slopes | low |
| 0x59EC90, 0x5A9040 | — (BeltMover step and ctor) | cars outside the loaded world ride a belt at constant acceleration | `ai/belt.rs:step`, `build` | **PORTED** | verified; belt cars over 21 m/s are native (9.3 % of native circuit samples, rebuilt by 0x422150 / 0x42ABD0) | low |
| 0x6317A2 | World_DrawCars (skip) | a car with +0x550 clear is not drawn | `bin/game/render/frame/cars.rs:traffic_drawn` | **PORTED** | verified | low |

## Lights and horn

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x5F62C0 | Car_UpdateBrakeFlag | brake light when the driver's pedal +0x214 is over 0.05 | `drive/state.rs:brake_lights`, `drive/mod.rs:brake_light_hold` | **PORTED** | verified; lit while moving 0.036 vs 0.052 (bot, small gap from the braking itself), while standing 0.93 vs 0.86 | low |
| 0x61AEA0, 0x60D580 | Glow_UpdateParams, — | glows of drawn cars only (+0x550 set, view bit) | `bin/game/render/native/glows.rs:glow_quads` | **PORTED** | verified | low |
| 0x6154A0 | HeadlightPool_Draw | pools of drawn cars only | `bin/game/render/native/cars.rs:headlight_pools` | **PORTED** | verified; night and rain pictures not compared (no matched scene) | low |
| 0x47E6E0, 0x475B30 | — (traffic sound, horn rule) | when a traffic car honks | `audio/others/horn.rs:horn_aims`, `audio/others/others.rs:traffic_update` | **PORTED** | observed: 23 horns in 10 min, the rule predicts 21.5, 18 within 1 s; rule rate 2.6/min on both sides | low |
| 0x40D5C0 | AI_AvoidHonkAtHuman | horn draws while avoiding the human | `sim/traffic/think.rs:traffic_car_frame` | **PORTED** | verified (draw order) | low |
| — (traffic engine sound module) | — | the AI engine controller's rpm and throttle as the sound's inputs | `audio/others/traffic_ctx.rs` | **GUESSED** | inferred (`audio.traffic.module` … `module-5`) | low |

## Per race type

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| — (traffic in drift, URL, Street X) | — | none | `sim/traffic/mod.rs:load` | **PORTED** | observed: none on either side | none |
| — (outrun) | — | the free-roam rules, no outrun branch | `ai/traffic/` | **PORTED** | observed: outrun traffic matches free roam (placed 12.8 vs 10.4, Drive 0.78 vs 0.72); ours not run in an outrun | low |
| — (Quick Race circuit 4014) | — | the race placer and keep test | as above | **PORTED** | placed 8.8 vs 8.9, placements 25.7 vs 25.4/min (real-time wined3d). Fixed 1/60 (8 vs 4 runs): Avoid share 0.58 vs 0.42 [0.23–0.48], jam share 0.11 vs 0.016, stopped 0.14–0.19 vs 0.08, speed p50 8.1 vs 9.7 m/s, head-on stands 0.125 [0–1] vs 0; the placer is not the cause found, the rule that differs is **open** (gap 3) | med: race traffic dodges and queues more |
| — (Dense traffic option) | — | optional pool × 1.5 / × 2 in free roam | `ai/traffic/pool.rs:dense_pool` | **NOT NEEDED** | an opt-in extra; off by default | none |

## Encounter cars meeting traffic

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| — (encounter racers' driving near traffic) | — | how free-roam encounter cars pass traffic | racer map: AvoidAvoidable, Pass | **MISSING**, in change | observed at fixed 1/60 (16 vs 12 bot runs): encounter car–traffic contacts 8.38 [3–14] per 5 min vs 1.42 [0–4], over 30: 3.06 vs 0.58, traffic stops for good 4.38 vs 3.17. The cause is in the racers' AI (racer map, gap 1), not in traffic; a fix in change reaches 1.38 / 0.38 | med: queues behind stopped cars |

## Not needed

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x578CF0, 0x571650, 0x5716D0, 0x571780 (callers 0x4049F0, 0x409CC0, 0x40AC20, …) | — | replay snapshot serialisers of the AI objects | `bin/game/sim/replay/snapshot.rs` (own format) | **NOT NEEDED** | verified role (named sections, field writers); the clone snapshots its own state, traffic included | none |
| 0x5ECF30 | Car_SetDebugTint | debug colour of a car | — | **NOT NEEDED** | verified: a debug tint | none |
| 0x410750 | — | debug names "TrafficIntersection" / "IncomingTrafficCar" | — | **NOT NEEDED** | verified | none |
| — (render info +0x3F8 = rand(8)) | — | no reader | draw kept | **NOT NEEDED** | verified | none |

## Gaps, ranked

Each gap with what a player would notice and how to check a fix. "Harness" means
`tools/parity/trafh.py run` on both games and `trafh.py stats --by-dir BEFORE AFTER NATIVE`
([traffic-harness.md](../../traffic-harness.md)). Issue: where the fix is tracked.

| # | Gap | Rows | What a player sees | How to verify | Issue |
|---|---|---|---|---|---|
| 1 | Encounter cars hit traffic about six times as often | Encounter cars meeting traffic (MISSING, in change) | traffic cars stopped for good after a racer hits them, with queues behind | harness `bot` at fixed 1/60 with the contact census against the original: encounter–traffic contacts per 5 min 8.4 → about 1.4, over 30: 3.1 → about 0.6 | #141 (racer side) |
| 2 | Two traffic cars standing head-on at a junction more often than in the original: the original has no yield rule there and ours runs the same code; ours' cars track an oncoming car more often per meeting (cause open), and the wait line's lane-object term is taken as 0 | Intersections: head-on at a junction (PORTED), 0x412170 (SIMPLIFIED) | two cars stand nose to nose at a junction while the player drives by; inside the original's range, above its mean | harness `bot` at fixed 1/60, ≥ 16 seeds × 300 s: `headon_stops`, distinct pairs per run, tracked share per meeting, `jam_share`; trace 0x412170's +0x0C shape types | #146 |
| 3 | Quick Race circuit traffic spends more time in Avoid Cars and jams more | Quick Race circuit 4014 (PORTED); candidate: 0x6148F0 (SIMPLIFIED, gap 7) | race traffic dodging, standing and queuing more than in the original | harness `qr:circuit` 120 s at fixed 1/60: `act_avoid` 0.58 → about 0.42, `jam_share` 0.11 → about 0.02, traffic speed p50 | not tracked yet |
| 4 | A traffic car's race coordinate steps from an observation; the writer is not traced | 0x8FC writer (GUESSED) | more or fewer cars in races when the rule differs off the measured cases | trace the store; harness `qr:drag` and `qr:circuit` `placed`, `within100`, `despawn/min` | #146 follow-up / point release |
| 5 | Drag-only lane tests not ported | 0x40AF30(−0.1), 0x40B110 (MISSING) | drag traffic changes lanes slightly differently | harness `qr:drag`, 8 seeds: `lane_changes_per_car_min`, `lane_not_own` | point release |
| 6 | Lane-check fallback (lane request) not ported | 0x40F4A0 fallback (MISSING) | a car keeps a lane that ends instead of asking the lane at its place | trace the request flag; harness `stand` and `bot`: `lane_changes_per_car_min`, `off_over2` | point release |
| 7 | Road entries without the 15-entry cap and the second chance for far objects | 0x6148F0 (SIMPLIFIED) | dense scenes: a far fast car seen or missed differently | harness `qr:circuit` at Maximum: `act_avoid`, `mutual_share` | point release |
| 8 | Junction choice drawn from the clone's own stream, not the world generator | 0x413BB0 (SIMPLIFIED) | none by statistics; other cars turn at a given junction | the random-number order check in [rng.md](../rng.md); harness turn counts | later |
| 9 | Brief extra actions on native traffic | Controller: one-sample actions (MISSING) | none seen | trace whether they draw random numbers; Wine action log | later |
| 10 | Resumed traffic after a scene keeps braking (inferred) | 0x4099B0 group (GUESSED) | traffic standing a moment longer after a race start | Wine memory log of the first second after the green in a Quick Race drag | later |
