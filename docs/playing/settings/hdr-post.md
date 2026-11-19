# High Precision Post

Smoother glow, sky and fog. The original's end-of-frame effects (bleach bypass, light streaks, glow) run on
8-bit pictures and build the glow at half size, which bands smooth gradients and leaves blocky blooms. This
setting runs the same effects at higher precision.

| Row (Advanced → Image Quality) | Values | options.txt |
|---|---|---|
| **High Precision Post** | Off / Half-Size Glow / Full-Size Glow | `enh.hdr_post 0/1/2` |

**Off** (the default) is the original's chain exactly.

**Cost** in free roam on the reference desktop GPU: at 1920 × 1080 Half +0.28 ms, Full +0.45 ms; at
3840 × 2160 Half +2.09 ms, Full +2.79 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[High Precision Post: how it works](../../notes/graphics/hdr-post.md).
