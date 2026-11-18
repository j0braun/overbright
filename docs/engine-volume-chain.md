I traced the full volume chain. For the player car it is almost always the two-recording blend (0x474030): one granular voice per recording, with the throttle crossfading between them. Only engine entries 9 and 38 (out of the first 40) use the single-voice path 0x473E60. Below, HM = SFXCTL_HybridMotor and ctx = the car-sound context object. "Snap" means the value is assigned directly; "rate-limited ±s" means it moves toward the target by at most s per update.

## Objects and wiring
- **Registry** (16-byte records: code, name, parent, factory; type = (code>>4)&0xFFF):
  - type 0 = Physics, 3 = Shifting, 4 = AccelTrans, 5 = Engine, 6 = HybridMotor (record 0x7ED2E4), 7 = Tunnel, 8 = 3DCarPos.
- **HM attach (0x470150):**
  - type 3 (Shifting) → +0x70
  - type 4 (AccelTrans) → +0x74
  - type 5 (Engine) → +0x6C
- **HM bind (0x45CF90, vtable +0x1C):**
  - +0x44 = ctx; +0x68 = ctx if ctx+0x14 ∈ {1,2,3}, else 0.
  - Builds a 5-point curve at +0x88 (points at +0x90) from rec84: (0,0), (r0C,0), (r0C+(r10−r0C)·r18, 1), (r10−(r10−r0C)·r14, 1), (r10,0). 0x5767A0 interpolates it linearly.
- **ctx (vtable 0x78BAA8):**
  - +0x44 = throttle, +0x48 = engine rpm.
  - +0x7C = engine index; +0x80 = rec80 = 0x7F0DE0 + idx·0x4C.
  - +0x84 = rec84 = 0x7F1AA8 + rec80.+0x14·0x34.
  - +0x78 = hybrid flag. In SetCar 0x4593B0 (at 0x4594F5), it is 1 iff rec80.+0x10 ≠ 0 and [0x850078] ≠ 4; otherwise 0 and rec84 = entry 0.
  - idx = dword0 of the 12-byte entry 0x7F2230[+0x94 + 4·+0x8C]. +0x8C = 0x459240(car+0x10); +0x94 = upgrade level 0..3 (part slots 0x18 / 0x13 / 0xF).
- **0x473BCC belongs to CARSFX_Skids** (vtable 0x78AE48). It does not apply to the player engine. In CARSFX_PlayerEngine (0x78AF30, attach 0x4614E0), +0xC0 is the HybridMotor (type 6). The other attach slots are +0xB8 = Engine (5), +0xC4 = Tunnel (7), +0xB4 = 3DCarPos (8).

## Engine record fields (0x4C bytes, table 0x7F0DE0)

| Offset | Meaning | Entry 0 value |
|---|---|---|
| +0x00 / +0x04 | EE / SPU abk name | — |
| +0x08 | id (clamped 0..60, stored in the +0xB0 block's +0x04) | — |
| +0x0C | accel GIN name | "GIN_HondaS2000_Church.gin" |
| +0x10 | hybrid flag | 1 |
| +0x14 | rec84 index | 3 |
| +0x1C / +0x20 | f, sample rpm range max / min | 9229 / 1350 |
| +0x24 / +0x28 | i, layer volumes | 18000 / 20000 |
| +0x2C | f, D0 normaliser | 150 |
| +0x30 / +0x38 | layer-A gain at t=1 / t=0 | 0.15 / 0.85 |
| +0x34 / +0x3C | layer-B gain at t=1 / t=0 | 0.85 / 0.15 |
| +0x40 / +0x44 | fixed gains A / B | 0.75 / 0.2 |
| +0x48 | i, minimum cutoff | 300 |

Other entries:
- Entry 1: 1C/20 = 8222/1500, 24/28 = 16000/23000, 30..44 = .15/.9/.8/.2/.5/.25, 48 = 1000.
- Entry 9 (h10 = 0): 8292/1450, 14500/28500, .15/.95/.8/.3/.6/.2, 48 = 500.

## Decel record fields (0x34 bytes, table 0x7F1AA8)
- +0x00 = decel GIN name (e.g. "GIN_S2000_Church_DCL.gin")
- +0x04 = i, layer-A volume when off throttle
- +0x08 = i, decel volume
- +0x0C / +0x10 = i, rpm trapezoid ends
- +0x14 / +0x18 = f, ramp fractions
- +0x1C = f, |D0| normaliser
- +0x24 / +0x2C = A gain at t2 = 1 / 0
- +0x28 / +0x30 = B gain at t2 = 1 / 0

Values for entry 3: 10000, 14500, 3692, 9182, .02, .35, 150, .15, .8, .9, .1.

## SFXCTL_Engine (HM+0x6C; vtable 0x789F70, update 0x47C010)
- **rpm** = slot 0x34 → +0x128 = src + [+0x144]·2 + [+0x1C8].
  - src is normally ctx+0x48.
  - Optionally src is rate-limited to [prev−60, prev+999].
  - +0x12C = the previous +0x128.
- **throttle %** = slot 0x38 → +0x130 = movavg(+0x90 filter; output +0x9C) + [+0x148]. The filter source:
  - Shifting.+0x78 ∉ {0,3}: reset to Shifting.+0xC0
  - else AccelTrans.+0xC8 ≠ 0: reset to AccelTrans.+0xC0
  - else: push ctx+0x44
- **+0xE0 (0x45C6C0)** = 0x7FFF, or while Shifting mode ∉ {0,3}: 0x7FFF − trunc(Shifting.+0xF8 × −32767). In both cases + trunc([+0x140]).
- **+0x118 / +0xFC** are ramps (0x459990 interpolators at +0x104 / +0xE8). Initial values 1.0 / 0.
  - Rev limiter, rpm > 9800 (0x788600) while not shifting: +0x118 → 0.15 and +0xFC → 0.85.
  - Release: +0x118 → 1.0 and +0xFC → 0 over ×50 ms.
- **+0x138** = cutoff clamp(trunc(725 + lin × (24000−725)/32767), 0, 0x7FFF). lin comes from 3DCarPos.+0x8 through 0x468160 and 0x467FF0. Exact meaning is open (distance/occlusion?).

## HM update (0x47C3D0)
1. If +0x58: +0x60 = dt, +0x5C += dt.
2. **0x45D0A0, rpm-change filter:**
   - +0xEC = ctx+0x48 − [+0xF4] (then +0xF4 = ctx+0x48); +0xF0 = Engine.+0x128 − Engine.+0x12C.
   - Filter at +0xC4: 4-tap moving average (0x579F30 N=4; 0x576980 = push, 0x576A20 = fill). D0 = +0xD0 = output.
   - Source selection:
     - [[ctx+0x40]+0xD8] set: push F0
     - else AccelTrans.C8 == 1: fill EC
     - else AccelTrans.C8 ≠ 0: push F0
     - else Shifting mode 0: push EC
     - else mode 5: fill F0
     - else Shifting.+0x7C == 2: fill 0.2·EC
     - else push EC
3. **+0xF8** = rec20 + (clamp(Engine.rpm, 1000, 10000) − 1000)·(1/9000)·(rec1C − rec20). Constants: 0x784550 = 1000, 0x784500 = 10000, 0x78A290 = 1/9000.
4. ctx+0x78 == 0 → 0x4701A0, else → 0x470540.
5. **0x45D190 post-scale:**
   - +0xB8 = trunc(Engine.FC × +0xB8)
   - +0x7C / +0x80 / +0x84 = trunc(x × Engine.+0x118)

### 0x4701A0 (single record)
- t = +0x104 = clamp(D0 / rec2C, 0, 1). D0 is signed, so only rising rpm counts.
- **Targets (A → +0x100, B → +0xFC), smoothing flag bl initially 1:**
  - [[ctx+0x40]+0xD8]: lerp.
  - Else, if 0x5ECD80(car) (car+0x28 == 1 && [[car+0x34]+0x4C]+0x48 ≠ 0: the human car is changing gear, **verified** under Wine 2026-11-11; the clone's `Drivetrain::pending`) or Shifting mode ≠ 0, switch on mode (jump table 0x470520; mode 0 / 1 fall to the default, 0x47024C):
    - 2: lerp, snap
    - 3 or 5: lerp
    - 7: lerp, snap if ctx+0x4C, else fixed
    - others (0, 1, 4, 6): fixed (so during a gear change with Shifting mode 0 the fixed gains, not the AccelTrans choice)
  - Else AccelTrans.C8:
    - 0: lerp if ctx+0x54 > 5.0, else fixed
    - 1: lerp, snap
    - other: lerp
  - lerp means A = rec38 + t(rec30 − rec38), B = rec3C + t(rec34 − rec3C); fixed means A = rec40, B = rec44.
- **Smoothing:** bl set → rate-limited ±0.2 (0x78861C) per update; else snap.
- **Cutoff +0xBC:** x = clamp(rec48 + v·(24000 − rec48)·0.01, rec48, 24000); x = min(x, Engine.+0x138); then +0xBC is rate-limited ±6000 (0x788624) toward x, or snaps.
- **Outputs:** G = int(Engine.E0) × 1/32000 (0x78AAB4). ecx at 0x4704B1 is Engine (HM+0x6C).
  - +0x7C = trunc(rec24 · +0x100 · G)
  - +0x80 = trunc(rec28 · +0xFC · G)
  - +0x84 = 0
  - +0xB8 = trunc(rec24 · G)

### 0x470540 (hybrid)
- t1 = +0x104 = clamp(|D0| / rec80.2C, 0, 1); t2 = +0x108 = clamp(|D0| / rec84.1C, 0, 1).
- **Target A/B selection** (same selector structure as 0x4701A0, jump table 0x470924). Lerps:
  - rec80-lerp: A = lerp(rec80.38 → .30, t1), B = lerp(rec80.3C → .34, t1)
  - rec84-lerp: A = lerp(rec84.2C → .24, t2), B = lerp(rec84.30 → .28, t2)
- **Cases:**
  - [[ctx+0x40]+0xD8]: rec80-lerp
  - Shifting mode ≠ 0 or 0x5ECD80 (the human car changing gear; 0x470613, then state − 2 > 5 → default 0x4706ED):
    - 2: rec80-lerp, snap
    - 3 or 5: rec80-lerp
    - 7: rec80-lerp, snap if ctx+0x4C
    - others (0, 1, 4, 6): rec84-lerp, not snapped
  - AccelTrans.C8:
    - 0: rec80-lerp if ctx+0x4C, else rec84-lerp
    - 1: rec80-lerp, snap
    - other: rec80-lerp
- Same ±0.2 smoothing as the single-record path.
- **Throttle blend:** s = curve(+0xF8) and f = v/100 + (1 − v/100)·s. Then +0x100 = f·+0x100 + (1 − f) and +0xFC = f·+0xFC.
- **Cutoff:** +0xBC = Engine.+0x138 (no throttle term).
- **Outputs:** G' = int(Engine.E0) × 1/32767 (0x78A6C8); v = Engine throttle %.
  - +0x7C = trunc((r84.04 − trunc(v·(r80.24 − r84.04)·(−0.01))) · +0x100 · G')
  - **+0x80** = trunc(v · r80.28 · G' · +0xFC · 0.01) (at 0x4708C8) — on-throttle (accel) layer
  - **+0x84** = trunc((1 − 0.01v) · r84.08 · G' · +0xFC) — off-throttle (decel) layer
  - +0xB8 = trunc(r80.24 · G')
- **+0xC0 byte** is always cleared (0x4703B5 / 0x47071A).

### Slot 6 (0x470940)
Idle-sweetener gate that moves HM+0x4 toward 0x7FFF or 0 by at most +0x3D7 / −0xC4 per call. The gate opens when speed·2.237 > 30 or |D0| ≥ 30, with +0x78 + 3.0 timing. Whether this feeds level[] is **open**.

## Player engine (0x474310)
- First sets +0x104 = HM.+0xF8 and +0xBC = 1.0.
- **Flag at 0x474336:** [[this+0x9C]+0x78] is the ctx hybrid flag. +0x9C is bound to ctx when ctx+0x14 ∈ {1,2}.
- **Voice slots** are 0x1C bytes from +0xCC, created in 0x47D740: one slot when non-hybrid, two when hybrid. Slot k has +0xCC = GIN object and +0xD0 = engine object (SetRPM target). For slot 0, +0xD8 = GIN.+0x08, **+0xDC = GIN minimum rpm (GIN.+0x04)** and +0xE0 = voice. Slot 1 is at +0xEC with its voice at +0xFC. GIN ids come from [0x82B884] + 0xDC (slot 0) or + 0xE4 (slot 1), + 4·ctx.+0x10.
- **0x473E60 (non-hybrid, voice +0xE0):**
  - **Volume:** level0 = level[0]; vol = clamp(((HM.+0x80 × level0) >> 15) >> 8, 0, 127).
  - **rpm** = max(HM.F8, +0xDC); SetRPM(rpm, 60.0).
  - **Pitch:** 0x723D9B gets trunc(min(F8 / +0xDC, 1) × +0xBC (= 1) × 4096).
  - **Cutoff:** 0x72403E gets trunc(HM.+0xBC).
  - Master and aux are as you had them.
  - The trailing +0x104 = obj.24 / 0x4669D0(obj.40 + 0.5) · ratio · 120 is a readback that the next update overwrites (consumers **open**).
- **0x474030 (hybrid):**
  - Voice +0xE0 gets HM.+0x80; voice +0xFC gets clamp(((HM.+0x84 × level0) >> 15) >> 8), stored at +0x10C.
  - Both voices get the same pitch, cutoff, master and aux. Both SetRPM calls (0x4741A3, 0x4742F5) use the same rpm, max(F8, slot-0 minimum rpm).
- **+0xB0 block (the abk EE/SPU engine, created in 0x47D920):**
  - +0x14 and +0x18 = clamp(((HM.7C × level0) >> 15) − 0x7FFF, −32767, 0)
  - +0x68 = clamp((HM.B8 × level0) >> 15, 0, 0x7FFF)
  - +0x08 = clamp(trunc(+0x104), 0, 10000)
  - +0x10 = clamp(trunc(throttle × 10.24), 0, 0x400)
  - +0x44 = clamp(−16383 − trunc(+0xBC × −16383), ±0x3FFF)
  - +0x48 = Engine.134 (0..0x400)
  - +0x58 = Engine.138 (0..25000)
  - +0x38 = clamp([0x82B880] − 0x7FFF, −0x7FFF, 0)
  - +0x3C = clamp(([0x82B070] × level0) >> 15, 0, 0x7FFF)
  - +0x40 = level slot 0x3C value (0..0x10000)
  - +0x5C = 11000 if HM.+0xC0 else 0 (effectively 0)
  - Committed with 0x735580.

## Answers
1. **The player's 0..0x7FFF volume is HM.+0x80** (plus HM.+0x84 for the second voice in hybrid mode). HM.+0x7C and +0xB8 feed only the +0xB0 abk block.
2. **The rpm passed to SetRPM is max(HM.+0xF8, GIN minimum rpm)**: the Engine rpm, clamped to 1000..10000, remapped into the record's 1C/20 range. 0x474030 differs by driving a second GIN voice (the _DCL decel recording) with HM.+0x84.
3. **Accel/decel recordings are blended by throttle in hybrid mode:** +0x80 is proportional to v%·rec80.28 (accel GIN) and +0x84 to (1 − v%)·rec84.08 (decel GIN). Both are also scaled by the smoothed +0xFC and the trapezoid factor f. In non-hybrid mode the only blend is the rpm-rise-rate t crossfade between +0x7C (abk) and +0x80 (GIN).

## Open
- What level[0] / the mix map is, and whether HM+0x4 contributes to it.
- Meanings of [0x850078] (the file holds 1988; set at runtime), [[ctx+0x40]+0xD8], ctx+0x4C, ctx+0x54, and the semantics of the Shifting and AccelTrans states.
- Engine +0x140 / +0x144 / +0x148 / +0x1C8 offsets, and the 3DCarPos source behind Engine.+0x138.
- What 0x4669D0 computes, and whether the +0xB0 abk block is audible on PC.
- Rev-limiter ramp durations: constant 0x7885EC = 450 combined with a global scale at 0x7F0064.
## Answers (ported in src/audio/engine_chain/motor.rs; details in formats.md "Engine volume chain")
- **ctx fields** (SFXCTL_Physics 0x484C50): +0x44 = throttle % slewed ±30 per update; +0x48 = (rpm − idle)/(max − idle)·9000 + 1000; **+0x4C = throttle input > 30 %**; +0x50 = gear; **+0x54 = throttle input %**.
- **[[ctx+0x40]+0xD8]**: ctx+0x40 is the Physics controller; +0xD8 is set when the game state takes control away (then 0x46FA30 scripts the revs).
- **AccelTrans +0xC8**: 0 idle, 1 tip-in (0x45CB70), 2 launch (0x45CC30), 3 launch settling, 4 lift during a transient; +0xC0 is its throttle ramp. Not ported (state 0).
- **Shifting +0x78**: the 0x482D50 blip states 1–7; 0 outside shifts. Not ported (mode 0).
- **0x5ECD80**: car +0x28 == 1 && [[car+0x34]+0x4C]+0x48 ≠ 0: the transmission's +0x48 is 1 for the 0.35–0.51 s of every gear change (read live under Wine 2026-11-11), the clone's `Drivetrain::pending`; ported as `EngineInput::shifting` (skipped while Physics +0xD8 scripts the revs, which 0x470540 / 0x4701A0 test first). The tach's shift light hidden during the shift (0x4C7090 / 0x4C784E) is the HUD's half, not ported here.
- **First update** (0x484CEE): +0xCC = old +0xC8 and +0x90 = old +0x8C, and the object is built with +0xC8 = 1 and +0x8C = 0, so the first update sees gear 1 and input 0 as the last (read live under Wine; 0x46FA30 writes +0x8C but not +0xC8). Physics reads the input block's gas +0x20C (0x484D1E), not the engine throttle.
- **Rev limiter**: 450 · v · s ms with s = clamp(gear, 1, 5) (0x7F0050 map; 0x7F0064 is its output, not a global scale).
- **The +0xB0 abk block is created on PC** (0x47D920): event "CAR" of the EE bank (CAR_xx_ENG_MB_EE.abk), now ported (`HybridMotor::abk_block`, `PlayerSfx::engine_abk`); it carries the steady layers (p4/p5 from HM +0x7C) and the rev-limiter layer (p25 from HM +0xB8). It uses no node 38/39 (only the SWTN banks do).
- Still open: level[0] (mix map), HM+0x4 (slot 6), Engine +0x140/+0x144/+0x148/+0x1C8 during transients, the 3DCarPos cutoff source.
## Engine transients (ported in src/audio/car/transients/; details in formats.md "Engine transients")
- **AccelTrans** launch (0x45CC30) / tip-in (0x45CB70) / lift (state 4) and the **Shifting** blip machine (0x482D50, states 1–7, triggers 0x46EC70 / 0x46EE60) now run; while active their ramps replace Engine +0x128 (0x46F7D0) and the throttle filter (0x45C710), the shift adds the volume bump (+0xE0, 0x45C6C0) and the rpm / volume wobble (0x45C7A0), and HM's D0 filter and gain targets follow their states.
- Engine +0x13C (0x46F9D0) rate-limits ctx rpm to −60 / +999 per update at ≤ 2500 rpm outside transients.
- Still open: Engine +0x1C8 (no writer in the class: 0), +0x134 / 0x46F580, Physics +0xD8 (0x46FA30), the mix map's level[0].
## Soundcmp (2026-11-01, measured under Wine against the recorded audio)
- **level[0]** is the mix map's (section 1, already ported): read live from the CAR event's parameter 14 (+0x3C = ([0x82B070] · level0) >> 15, [0x82B070] = 5186): 18236 idle in free roam, 6477 while Rachel speaks (the speech ducks, formats.md "Sound comparison"). The clone now gives both.
- HM +0x7C at idle with entry 27 (Rachel's 350Z) is 12497 in the original (from parameter 4 = −30297 at level0 6477) and 12500 in the clone.
- The engine entry is the driven car's (`Game::driven_perf`): 27 for Rachel's 350Z, not 25.
## Low master (verified, docs/audio-levels.md §1b)
- The accel / decel GIN bytes are ((HM +0x80 / +0x84 · level0) >> 15) >> 8, truncated (0x4740C0 / 0x474106): at full throttle (HM +0x80 ≈ 1 100, the limiter) byte 2 at master 1, 1 from 0.95, 0 from the slider's 0.55; accelerating in 1st gear (HM +0x80 7 000–27 000) 15–58 at master 1.
- The abk block's p4 = clamp(((HM +0x7C · level0) >> 15) − 0x7FFF): idle (HM +0x7C 12500) −25811 at master 1 … −31825 at 0.30, where the CAR graph has stopped all its players; p25 (the limiter layer) 280 at 0.15 stops player 0. Read live in the original's instance memory; the clone's graph gives the same players.
