# Evidence: career stages 1–2 against the original
- `playthrough.txt`: the career bot's summary lines with the base logic and with the three fixes (default route and the `challenge` variant, seed 7), and the open-event count at STAGE_2_SHOW_1's Accept / goal.
- Owned bits in the original's save `stage2/orig/saves/01-212616.sav` (the original's career played to stage 2), slot 0, extra block +0x26C..+0x29F: only +0x286 bit 0 (spoilers, bit 80) and +0x298 = 06 (hoods, bits 1 and 2) set.
- GLOBALB.BUN chunk 0x34600 (car types) ends at file offset 0x6DE6F0 after 46 records of 0x890; the bytes there (the "record 0x2E" 0x503B10 reads for a shared-group part) are the next chunk's header `01 46 03 00 c0 1a 00 00`.
- Tests failing on the base logic (all four new rule tests): career_stages_1_2.rs:52, :78, :133, :204.
