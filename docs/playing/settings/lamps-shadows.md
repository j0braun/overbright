# Dynamic Lamps, Lamp Shadows, Contact Shadows

The original lights the city with light baked into the models, and the only dynamic shadow is the car's
shadow texture. These rows add street lamps that light the cars and the world as they pass, shadows cast by
those lamps, and small contact shadows where things touch the ground.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Dynamic Lamps** | Off / On | `enh.dynamic_lamps 0/1` |
| **Lamp Shadows** | Off / Low / High | `enh.lamp_shadows 0/1/2` |
| **Contact Shadows** | Off / On | `enh.contact_shadows 0/1` |

All three are off by default. Presets: Modern sets Dynamic Lamps On, Lamp Shadows Low and Contact Shadows
On; Ultra sets Lamp Shadows High.

**Cost** at night in free roam (reference desktop GPU): Dynamic Lamps +0.03 ms, Lamp Shadows Low +0.13 ms at
1920 × 1080 and +0.25 ms at 3840 × 2160, High +0.21 / +0.46 ms, Contact Shadows about +0.02 ms. On a small
integrated GPU (2 compute units, 1080p) all three at High cost about +13.8 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[Dynamic Lamps, Lamp Shadows, Contact Shadows: how it works](../../notes/graphics/lamps-shadows.md).
