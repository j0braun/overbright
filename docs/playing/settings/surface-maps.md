# Surface Detail

Gives the city's surfaces their own shine and relief under the lights: asphalt, concrete, metal, glass, signs
and façades each catch light their own way, and with Normals small bumps and grooves show. The maps are made on your
machine from your own game textures and kept in your cache folder; nothing is shipped.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Surface Detail** | Off / Materials / Materials + Normals | `enh.surface_detail 0/1/2` |

**Off** (the default) draws the original's surfaces. No preset turns it on. At Materials + Normals it also applies to the
cars, and screen-space reflections follow the mapped surface.

**Cost** at night in free roam on the reference desktop GPU: Materials about +0.05 ms, Materials + Normals
about +0.1 ms, at 1920 × 1080 and at 3840 × 2160. The maps are made once per texture.

## How it works

How the materials and normals are found, the review, the cost and what is open:
[Surface Detail: how it works](../../notes/graphics/surface-maps.md).
