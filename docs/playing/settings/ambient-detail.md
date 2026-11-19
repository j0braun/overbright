# Ambient Detail

Small things that make the streets feel alive at night: litter, leaves and drain grates on the pavements,
and with Next-Gen blown leaves and paper, tyre dust, steam and moths around the lamps. It is visual only:
nothing you drive into or that changes the race.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Ambient Detail** | Off / On / Next-Gen | `enh.ambient_detail 0..2` |

- **On:** street props on the pavements and in the gutters (cans, bottles, cups, boxes, paper, leaves, oil
  stains, drain grates), never on the lanes or the racing line. They are lit by the street lamps and the cars'
  lights and fade out between 42 and 60 m.
- **Next-Gen:** everything of On, plus leaves and paper that the cars' air and the wind blow about, and the
  volumetric effects (dust, steam, moths).

No preset turns it on. **Off** draws exactly the original's streets.

**Cost** at 3840 × 2160 on the reference desktop GPU: On about +0.01 to +0.04 ms, Next-Gen +0.20 to
+0.47 ms; on a small integrated GPU Next-Gen costs about +7.8 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[Ambient Detail: how it works](../../notes/graphics/ambient-detail.md).
