# Optional graphics settings

Every setting here is off until you turn it on; with all of them off the game looks exactly like the original.
You find them under **Options → Display → Enhancements**: the main page sets the graphics mode, the quality
level, upscaling, anti-aliasing and ray tracing; **Advanced** opens one page per group below. The groups and
the rows in them follow the settings model (`src/frontend/enh/settings_model/model.rs`).

Each page names its rows and what they do, then how the feature works and what it costs. Costs on the pages
are measured on a desktop GPU unless the page says otherwise; the settings page shows the cost for your screen
size in the detail line under the rows.

## Display

| Page | Rows |
|---|---|
| [HDR Output](../hdr.md) | HDR Output, peak and paper white brightness, gamma. |
| [Screen resolutions](../resolutions.md), [ultrawide](../ultrawide.md) | Resolution and aspect ratio (the original's Display rows). |

## Image Quality

| Page | Rows |
|---|---|
| [Spatial anti-aliasing](ssaa.md) | Anti-Aliasing (SSAA 2x to 16x), MSAA, the FXAA / SMAA filter, sharpening. |
| [Temporal tier](temporal.md) | TAA in the Anti-Aliasing row, Upscaling, Volumetric Fog's temporal steps. |
| [High Precision Post](hdr-post.md) | High Precision Post: glow, streaks and the grade at higher precision. |
| [Reflections](reflections.md) | Enhanced Reflections, Car Reflection, Road Reflection, Reflection Rate, Screen-Space Reflections up to Ray Traced (experimental); Wet Roads is under Weather & Water. |
| [Enhanced Car Paint](car-paint-enhanced.md) | Enhanced Car Paint. |
| [Car special materials](car-materials.md) | Carbon fibre and chrome trim under Enhanced Car Paint. |
| [Fix Original Model Errors](model-fixes.md) | Fix Original Model Errors. |

## Lighting & Effects

| Page | Rows |
|---|---|
| [Modern Lighting](modern-lighting.md) | Modern Lighting and its strength, SSAO, Shadow Quality. |
| [Shadow softness](shadow-softness.md) | How shadows look under Modern Lighting at each Shadow Quality. |
| [Dynamic Lamps, Lamp Shadows, Contact Shadows](lamps-shadows.md) | Dynamic Lamps, Lamp Shadows, Contact Shadows. |
| [Lighting II](lighting2.md) | Headlight Beams, Light Probes, Enhanced Glass. |
| [Next-Gen car lights](car-lights.md) | Headlight Beams "Next-Gen". |
| [Neon underglow quality](neon-quality.md) | Neon underglow at Car Shadow Ultra. |
| [Surface Detail](surface-maps.md) | Surface Detail (roughness and normal maps made from your own textures). |
| [Atmosphere](atmosphere.md) | Volumetric Fog and Sky "Next-Gen". |
| [Smoke](smoke.md) | Particles: Original, High, Ultra, Cinematic (volumetric smoke). |
| [Sparks & Fire](sparks-fire.md) | Sparks & Fire, and the original's flame triggers. |
| [Motion blur and depth of field](motionblur-dof.md) | Motion Blur, Depth of Field. |
| [Ambient Detail](ambient-detail.md) | Ambient Detail: street props, debris, tyre dust, steam, moths. |

## Weather & Water

| Page | Rows |
|---|---|
| [Reflections](reflections.md) | Wet Roads. |
| [Weather package](weather.md) | Wet Car, Rain Detail, Spray & Splashes, Lens Drops. |
| [Next-Gen rain](rain-nextgen.md) | Rain Detail "Next-Gen". |
| [Wet Road Interaction](wet-road-car.md) | Wet Road Interaction: tyre tracks that dry off, and the car's effect on the wet road. |
| [Animated Water](water.md) | Animated Water for the fountains, canals and pools. |

## Performance

| Page | Rows |
|---|---|
| [Temporal tier](temporal.md) | Upscaling (the temporal upscaler) and Render Scale. |
| [FSR 2](fsr2.md) | FSR 2 temporal upscaling. |
| [Modern extras](../modern-extras.md) | Low Latency, the FSR tiers of Upscaling. |
| [Performance overlay](../perf-overlay.md) | Performance Overlay, its position, scale and log. |

## Gameplay & Accessibility, Controller, Packs

These groups are not graphics: see [Modern extras](../modern-extras.md) (replay cameras, HUD colours and sizes, reduced
flashing), [Photo mode](../photo-mode.md), [Replay cameras](../replay-cameras.md),
[Mark This Spot](../mark-spot.md), [Controller support](../controller-support.md) and, for Packs, the
[texture pack pages](../../modding/README.md).
