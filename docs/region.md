# Region

The executable keeps the game's region in the byte 0x7FA73C, read by
0x570A40 (the value) and 0x570A50 (European: 1 or 5–9). No code writes it;
the European PC 1.2 release stores 6. Overbright reads no executable, so it
takes the region from the installed data (`crates/ob-core/src/region.rs`):

* the CARS folder has CIVIC and RSX and has neither CORSA nor PEUGOT106:
  region 0 (North America);
* anything else: 6 (Europe), the value of the traced build.

**Inferred**: that a North American install's CARS folder looks like this.
It follows from the executable's own tables, which drop exactly CIVIC and
RSX for a nonzero region and exactly CORSA and PEUGOT106 for region 0; no
North American install was examined. The European install's CARS folder has
CORSA and PEUGOT106 and no CIVIC or RSX folder (checked).

The region is detected when the career data loads (`CareerData::load`).

## What follows the region (verified: read from SPEED2.EXE)

| Reader | Region 0 | Other regions |
|---|---|---|
| 0x5022C0 lot exclusions | 0x7F7C48 = 7, 6 (CORSA, PEUGOT106) | 0x7F7C50 = 5, 18 (CIVIC, RSX) |
| 0x529CD0 Quick Race free cars | 0x7F7C08 = 0, 1, 2, 5, 10, 25, 28, 22 | 0x7F7C28 = 0, 1, 2, 7, 10, 25, 28, 22 |
| 0x4FEAF0 random opponents | 0x7F6DA4 (ends 5, 18) | 0x7F6DC0 (ends 7, 6) |
| 0x57FC5E shared textures | GLOBAL/GlobalTexturesNA.bin | GLOBAL/GlobalTexturesROW.bin |
| 0x4B8808 credits (English) | NA_ENGLISH.TXT | UK_ENGLISH.TXT (European regions) |
| 0x511E60 lot make and badge (English) | every car's own | Corsa VAUXHALL / CORSA, Miata MX5, Tiburon COUPE (European regions) |
| 0x575780 needmybestbuy, graphics shop AD_BESTBUY | enabled, listed | disabled, skipped |

The SUV pool 0x7F6DDC is the same in every region. Regions other than 0 and
6 (Korea, Japan, Taiwan, China, Thailand; 0x571390) are never chosen.

Tests: `region::tests` (the installed data is 6; a synthetic US listing is
0, with CORSA or without RSX it is 6), `career::lists::region_tests` (the
tables; the lot and Quick Race lists in both regions on the installed data),
and the per-reader tests in cheats, credits, car_lot and world::parse.
