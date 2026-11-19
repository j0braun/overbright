# Model review notes (sheets in ../sheets, re-rendered from main with the carbon fix)
General: dark tiles show the _CF twin (carbon weave, CARBONFIBER material) - mid grey under the sheet env (carbon fix: matches in Body Shop).
- A3_hood_01: ok (FUSION chevrons white = chrome family/sheet env; TWISTER faint line across hood?)
- A3_hood_02: VELOCITY (STYLE23) black arc near windshield - candidate, zoom
- A3_spoiler_01..03: ok (CF twins dark carbon; stock = no spoiler)
- A3_roof_01..03: ok (CF scoops light silver: CARBONFIBRE2 -> DEFAULT in the original too, docs/notes/graphics/car-materials.md)
- A3_mirror_01..02: ok
- A3_exhaust_01: ok (low framing, tips sit under the bumper)
- A3_overview_01: ok (front wheel slightly forward = known A3 arch fix, Fix row)
- A3_skirt_01: ok
- A3_wide_01: ok (ARMAGEDDON front plate white)
- A3_rims_01..03: ok
- A3_hood_02 VELOCITY arc: crop crops/a3_velocity.png - a cowl slot cut in the hood, same on CELICA: design, not an error
- CELICA (all 17): ok. roof_01 AIRSTREAM (STYLE11, also A3): scoop tail overhangs the roof's rear edge (shared scoop on a sloped roof; authoring, minor). Velocity arc again (design).
- COROLLA (all 17): ok. AIRSTREAM overhang again (3 cars -> shared scoop design). Pop-up lights drawn raised in every view (not a part error; not judged here). Stock exhaust pipe visible under bumper: ok.
- GOLF_fb_02, rb_02, skirt_01, skirt_02: ok (SQUIRE 9 chrome bars over the plate area = known 3000GT pattern)
- GOLF_hood_01: TWISTER (STYLE07): dashed pixel cracks along a seam mid-hood (crops/golf_twister.png); the small front-right slot is in the uncapped-FPS study's TWISTER too (design). Stock hood clean (crops/golf_stockhood.png). CANDIDATE (minor, T-junction cracks) -> Wine
- GOLF hood_02, spoiler_01..03, roof_01..03, mirror_01..02, exhaust_01, overview_01, wide_01, rims_01..03: ok. Golf "badge gaps" lead: no gap at the hood/grille badge in 29 hoods; stock hood clean.
- MIATA fb_02, rb_02, skirt_01..02, hood_01: ok
- MIATA_hood_02: VELOCITY (STYLE23_HOOD_LEFT): the cowl hump's inner face renders light grey/white, paint-independent (crops/miata_velocity.png); on A3/CELICA/COROLLA/GOLF it is black. part_materials: MIATA's left half has 12 tris MIATA_SKIN1 + CLEARPLASTIC (A3: DULLPLASTIC only). CANDIDATE -> Wine (Miata + Velocity hood)
- MIATA_spoiler_01: ok
- MIATA spoiler_02..03, roof_01..03, mirror_01..02, exhaust_01, overview_01, wide_01, rims_01..03: ok. roof_01 WINDTUNNEL (STYLE09): the scoop's nose sinks into the curved hardtop leaving a detached sliver (shared scoop vs roof curvature; authoring, minor).
- RX7 (all 20): ok, except hood_02 VELOCITY: same light cowl face as MIATA (tile 13/14) -> same candidate. Brown patch on the stock nose = showroom env reflection (crops/rx7_front.png).
- RX8 (all 20): ok. fb_02 VALHALLA (KIT30) grille block light/unpainted and wide ARMAGEDDON lip white = chrome family. VELOCITY cowl black (DULLPLASTIC). Exhaust tips centred under rear diffuser ok.
- TIBURON (all 20): ok except: stock wheel (KIT00_FRONT_WHEEL_A) shows a thin dark vertical line at the front edge of the RIGHT wheels (crops/tiburon_line.png, tib_wheel_lr.png; visible in rims_01 tile 1, overview 5/11/17, skirt dark tiles). Cause (inferred): the wheel's inner cap quad (PLAINNOTHING, uv u 0.752..1.0, v 0.254..0.498) samples the alpha disc of TIBURON_TIRE; at u=1.0 bilinear WRAP pulls in column 0 (opaque) -> half-alpha column passes the alpha test. CANDIDATE -> Wine (Tiburon stock rims, both sides).
- TIBURON VELOCITY cowl black (no SKIN1 range). Hood stock dark tile ok.
- TT (all 20): ok. VELOCITY cowl black (DULLPLASTIC). VALHALLA grille block unpainted (chrome family, as RX8). Antenna at roof rear passes through nothing.
- 350Z rims_01..03, rimref_01..06: ok (stock D20 W23 vs aftermarket W26 drawn wider: sheet's width choice). MEISTER S2R (WORK_STYLE02): pinkish fleck on the lower lip in every car's tile = the lettering lead.

## Wine checks (a private Wine copy, display :219, career_end_won profile, main menu Customize -> Body Shop; raw frames kept with the maintainer's private evidence)
- PEUGOT106 JETSTREAM scoop: the stock antenna runs through the scoop in the original too -> fixed behind the Fix row (antenna left out while a scoop is fitted).
- MIATA Hoods list (and Carbon Fiber Hoods): ends at FACTOR X; the split hoods (COPPERHEAD, COBRA, VELOCITY, GATO, TRAP: part kind 5) are never offered; the clone's shop_parts drops them too (0x5449D0). The light Velocity cowl is unreachable data.
- GOLF TWISTER: the dashed seam crack shows in the original as well (authoring T-junction); not fixed (needs re-triangulation).
- SENTRA MANTIS: upper grille opening black without mesh in the original as well; not fixed (missing geometry).
- WORK MEISTER S2R on the Sentra: the red lettering fleck shows on the lip in the original as well (native).
- 3000GT SQUIRE 9 rear: plate over the three bars in the original as well (authoring).
- TIBURON stock wheels: the clone's thin dark line at the cap quad's u=1.0 edge is not seen in the original's side view; removing it with a clamped car sampler (experiment, reverted) shows the cause in the clone; the original's address mode for car.efx draws is not traced -> open.
