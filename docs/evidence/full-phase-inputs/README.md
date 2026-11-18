# Native ground and aero dependencies

This capture closes specific missing pointees for a subsequent production phase replay. It does **not** establish whole-substep equivalence or justify moving component passes yet.

## Capture and audit

The private original ran Quick Race Circuit with Peugeot 206 automatic. A controlled landing raised body Z by 0.6 m and set vertical momentum to native mass times −12 m/s; Up was pressed/released. Handling, suspension, wheel, engine and clutch values were read, not injected. `full-landing.json` retains exact writes, hook bytes, restore checks, executable/recorder hashes, pointers and payload layout. The owned session stopped after capture.

Reproduce with a private Wine copy, a display of its own and the GPU to itself. After entering that race (the scripts are kept with the maintainer's private evidence):

```
python3 capture.py OWNED_PREFIX OUTPUT_BASE --seconds 5 --drop .6 --vertical-speed -12
python3 decode.py OUTPUT_BASE
```

The retained stream has 2,580 records in 559 consecutive complete groups; 112 rest-triggered and 17 bump-triggered zero-time traversals. No records were dropped, no groups are partial, and hooks were restored. `test_decoder.py` audits the stream and rejects six rehashed semantic corruptions: wheel owner, dynamic pointer, overflow, polygon source face, unused slot and truncated group. The positive audit also checks every fixed component pointer/vtable and every dynamic pointer/count.

`recorder.s` retains the earlier boundary hooks and adds bounded copies of ground context, recent/nearby polygons, barrier entries, aero, collision header, world counter and section bias. Null/unused slots are zero padded; overflow is recorded and rejected. `test-copy.s` exercises valid, null, unused and overflow copies and register preservation in a standalone 32-bit executable; this tests recorder safety, not physics.

## Exact pointer identity and retained geometry

`59D274..59D285` follows wheel +0x20 to a **physics view at full-car +0x40**, then reads view +4. The ground pointer is therefore full-car +0x44, not full-car +4. Mover +0x58 points to that same view. An initial preflight followed the wrong base and aborted before installing hooks; the corrected recorder asserts all these identities. Older references saying simply “car +4” are ambiguous and must use the physics-view base.

The previous expanded full-car dump (0x600 bytes) already included the first 0x3C bytes of this embedded ground context at full-car +0x5C4, including selected pack, anchor and margin. It omitted recent/nearby lists and their pointees. This capture retains the full 0x334-byte context, 0x80-byte polygon records and 0x20-byte barrier entries. Recent count is bounded by the native four-slot path (`5E76F9..5E770A`); 160 nearby and 16 barrier slots are recorder bounds, not newly inferred behavior.

Pack section 736 maps to drivable section 716 with captured section bias 20. Every polygon +0x0C source-face pointer resolves exactly into the retained pack's 32-byte faces; metadata and every used vertex match the source face. Recent histories have three observed configurations; nearby faces are 99, 100, 101, 103, 104 and 105. Pack lookup arrays and geometry match before/after; only face +6's native mutable cache index is excluded from geometry comparison, with both raw snapshots preserved.

All boundaries have zero collision contacts. Own barrier entries name `PLAYER_BARRIERS_4014` and `BARRIERS_4014`, both switched off at recorded boundaries. Query-local switching still must execute in replay. Aero is mover +0x50: catch-up/draft/down-share/factor/surface-drag are 1, throttle transitions between 0 and 1. These are observed controls, not universal defaults.

## Production replay prerequisites and limits

Restore recent/nearby histories by exact source-face index, not nearest geometry. Resolve section/pack through the production world and compare native geometry before restoring history. The global outline-search order is not captured; a replay must assert its queries remain inside the retained section slack, or obtain that history before crossing a section boundary.

Native normal-pre is post-integration. It cannot seed a complete clone `substep` as though it were force-entry. The clone currently measures suspension at the start of the next substep; native measures it in normal-post, then executes rest/bump/zero before next force. A diagnostic projection must specify which history comes from normal-pre and which live state comes from normal-post/next-force. Authoritative linear/angular momenta must remain distinct from cached velocities across bumps. Engine/clutch internal histories and aero inputs must be restored explicitly rather than filled with defaults.

Next work is an actual production ground-query and force-phase replay using these dependencies. No production code or timestep behavior changes in this evidence commit; full ordering and momentum sum-order differences remain open.
