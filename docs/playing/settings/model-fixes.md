# Fix Original Model Errors

A few car models in the original have small errors: exhaust tips drawn into each other, grille slats in the
body colour, wheels that sit off-centre in their arches, an antenna that runs through a roof scoop. This
setting corrects them when a car loads.

| Row (Advanced → Image Quality) | Values | options.txt |
|---|---|---|
| **Fix Original Model Errors** | Off / On | `enh.fix_models` |

**Off** (the default) leaves every model exactly as shipped. Cars already loaded keep their geometry until
they load again. The corrections are written in Overbright's own code from what your game files contain; no
mod file is used.

Fixed: the IS300's FRENZY rear bumper (exhaust tips), the GOLF's SNYPER front bumper (grille slats), the front
wheels of the MUSTANGGT and the A3 (and, after a visual review, the IMPREZAWRX, PEUGOT106, PEUGOT and
NAVIGATOR), and the PEUGOT106's roof antenna with any roof scoop. Other reported errors that could not be
confirmed in the original are listed on the notes page.

## How it works

The fixes with the data behind each, what was checked and not fixed, and the visual review:
[Fix Original Model Errors: how it works](../../notes/graphics/model-fixes.md).
