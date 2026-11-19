# Motion blur and depth of field

Modern motion blur and a cinematic depth of field, in the same rows as the original's own effects.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Motion Blur** | Off / On (the original's) / **Camera** / **Per-Object** | `custom.11` + `enh.motion_blur 1..2` |
| **Depth Of Field** | Off / On (the original's switch) / **Cinematic** | `custom.13` + `enh.depth_of_field 1` |

- **Camera** blurs the picture by the camera's movement; **Per-Object** also blurs each car by its own
  movement. Both replace the original's speed blur.
- **Cinematic** depth of field blurs what is out of focus in scenes, replays and the garage (not behind the
  chase camera).

The first values are the original's own settings (Customize Display). The presets leave both rows alone.

**Cost** in a night race on the reference desktop GPU: Camera or Per-Object about +0.27 ms at
1920 × 1080 and +1.1 ms at 3840 × 2160; Cinematic depth of field about +0.28 / +1.30 ms where it runs. On a
small integrated GPU (1080p) the blur costs about +15 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[Motion blur and depth of field: how it works](../../notes/graphics/motionblur-dof.md).
