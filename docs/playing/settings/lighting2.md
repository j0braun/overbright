# Lighting II: Headlight Beams, Light Probes, Enhanced Glass

Three rows that light the cars better, all off by default:

| Row (Advanced page group) | Values | options.txt | What you see |
|---|---|---|---|
| **Headlight Beams** (Lighting & Effects) | Off / On / Next-Gen | `enh.headlight_beams 0/1/2` | Every car gets the headlight projector that only the player's car has in the original (inferred: docs/formats/career-1.md, Car Headlight). Next-Gen adds the beams in the air: [Next-Gen car lights](car-lights.md). |
| **Light Probes** (Lighting & Effects) | Off / On | `enh.light_probes 0/1` | Cars take the light of the place they drive through, from light probes along the roads. |
| **Enhanced Glass** (Image Quality) | Off / On | `enh.enhanced_glass 0/1` | Car windows and lamp covers that bend the view behind them and reflect the surroundings. |

When the cars' lights are on stays the original's: every car has its lights on, always.

**Cost** in a night race on the reference desktop GPU: Headlight Beams about 0 at 1920 × 1080 and +0.21 ms at
3840 × 2160; Light Probes about 0 on the GPU and about 0.1 ms of CPU time while probes are made; Enhanced Glass
+0.11 / +0.33 ms. On a small integrated GPU Headlight Beams costs +4.6 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[Lighting II: how it works](../../notes/graphics/lighting2.md).
