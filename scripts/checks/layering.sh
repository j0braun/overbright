#!/bin/bash
# layering.sh [repo]: the render layering lint (refactor plan render §0 (a) 8,
# step S3). Files under src/render/core/ and src/render/native/ (the layers
# the enhancements build on) must not
#   1. reach the enhancement or QoL layers: `crate::enh`, `render::enh`,
#      `crate::qol` / `qol::` (settings come in as values, not global reads);
#   2. hold global mutable state: a `static` of a Mutex/RwLock/OnceLock/
#      LazyLock/Cell/RefCell/Atomic*, a `static mut`, or a `thread_local!`.
# The game binary's production modules (src/bin/game.rs and src/bin/game/** outside dev/ and tests/)
# must not
#   3. name the developer harness (`dev::` paths, `mod dev;`) outside `#[cfg(feature = "dev-harness")]`
#      (or `#[cfg(all(test, feature = "dev-harness"))]`) on the item itself (the shipping
#      binary leaves the harness out; the game's paths call it through `dev_hooks`).
# The simulation's session (src/bin/game/sim/session/**: `Session::step`, docs/sim-session.md) must not
#   4. reach the game, the renderer, the audio, the window, the HUD or the front end (rule 4 below).
# Render code (src/render/**, docs/frame-settings.md) must not
#   5. add a global settings read: a `qol::` path or `RenderOptions::in_force(` (the frame's settings come in
#      as `FrameSettings` / `RenderOptions` values) on a line not in scripts/checks/layering-settings-allow.txt
#      (the reads left, matched by file and trimmed text, no line numbers; a converted read is taken off it).
# Comment lines are skipped. Prints every hit as file:line and exits 1 on any;
# prints "LAYERING: ok (N files)" otherwise. The rule set grows with the moves.
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$REPO" || exit 2
files=()
for d in src/render/core src/render/native; do
  [ -d "$d" ] && while IFS= read -r f; do files+=("$f"); done < <(find "$d" -name '*.rs' | sort)
done
fail=0
# Rule 3 first (its own file set). A line naming the harness passes when the attribute lines right above
# it (`#[...]`, skipping `#[path ...]`) include the feature's cfg.
game=()
[ -f src/bin/game.rs ] && game+=(src/bin/game.rs)
[ -d src/bin/game ] && while IFS= read -r f; do game+=("$f"); done < <(find src/bin/game -name '*.rs' -not -path 'src/bin/game/dev/*' -not -path 'src/bin/game/tests/*' | sort)
if [ ${#game[@]} -gt 0 ]; then
  hits=$(awk '
    FNR == 1 { gated = 0 }
    /^[[:space:]]*\/\// { next }
    /^[[:space:]]*#\[cfg\((all\(test, )?feature = "dev-harness"\)?\)\][[:space:]]*$/ { gated = 1; next }
    /^[[:space:]]*#\[/ { next }
    (/(^|[^A-Za-z0-9_])dev::/ || /^[[:space:]]*(pub(\([a-z]+\))? )?mod dev;/) && !gated { print FILENAME ":" FNR ": " $0 }
    { gated = 0 }' "${game[@]}")
  if [ -n "$hits" ]; then
    echo "LAYERING: dev harness named outside its feature:"
    echo "$hits" | sed 's/^/  /'
    fail=1
  fi
fi
# Rule 4: the simulation's session (src/bin/game/sim/session/**) must not reach the game, the renderer,
# the audio, the window, the HUD or the front end. It names `Game`/`Mode`/`Menu` nowhere, does not glob-import the
# crate root (`use crate::*` re-exports all of them), names no game-binary glue module (`crate::audio`,
# `crate::render`, `crate::hud_glue`, `crate::menus`, `crate::shell`, `crate::input`, `crate::garage`, `crate::ui`,
# `crate::dyno`, `crate::shops`, `crate::career_glue`, `crate::flow`, `crate::loads`, `crate::dev`,
# `crate::dev_hooks`, `crate::effects_glue`, `crate::update`), none of `Audio`, `Mixer`, `RenderState`, `Platform`,
# `Window`, `Hud`, `HudState`, `FeState`, no `wgpu::` / `winit::`, no `nfsu2::audio` and no
# `nfsu2::frontend::{hud,screens}`, and of `nfsu2::render` only the camera value `nfsu2::render::Camera`.
session=()
[ -d src/bin/game/sim/session ] && while IFS= read -r f; do session+=("$f"); done < <(find src/bin/game/sim/session -name '*.rs' | sort)
if [ ${#session[@]} -gt 0 ]; then
  hits=$( { grep -nE 'use[[:space:]]+crate::\*|(^|[^:A-Za-z0-9_])(Game|Mode|Menu|Audio|Mixer|RenderState|Platform|Window|Hud|HudState|FeState)\b|crate::(Game|Mode|Menu|Audio|Mixer|RenderState|Platform|Window|Hud|HudState|FeState|audio|render|hud_glue|menus|shell|input|garage|ui|dyno|shops|career_glue|flow|loads|dev|dev_hooks|effects_glue|update)\b|\b(wgpu|winit)::|nfsu2::(audio|frontend::(hud|screens))\b|(^|[^:A-Za-z0-9_])hud::' "${session[@]}" /dev/null
            grep -nP 'nfsu2::render::(?!Camera\b)' "${session[@]}" /dev/null; } | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' | sort -u)
  if [ -n "$hits" ]; then
    echo "LAYERING: the simulation's session reaches past its boundary:"
    echo "$hits" | sed 's/^/  /'
    fail=1
  fi
fi
# Rule 5: no new global settings read in render code.
allow=scripts/checks/layering-settings-allow.txt
if [ -d src/render ]; then
  hits=$(grep -rnE '\bqol::|RenderOptions::in_force\(' src/render --include='*.rs' /dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' \
    | awk -v allow="$allow" -F: '
        BEGIN { while ((getline l < allow) > 0) if (l !~ /^#/ && l != "") ok[l] = 1 }
        { f = $1; t = $0; sub(/^[^:]+:[0-9]+:/, "", t); gsub(/^[[:space:]]+|[[:space:]]+$/, "", t); if (!((f "\t" t) in ok)) print $0 }')
  left=$(grep -cvE '^#|^$' "$allow" 2>/dev/null || echo 0)
  if [ -n "$hits" ]; then
    echo "LAYERING: new global settings read in render code (pass the frame's FrameSettings / RenderOptions; $left reads left on the list):"
    echo "$hits" | sed 's/^/  /'
    fail=1
  fi
fi
if [ ${#files[@]} -eq 0 ]; then
  [ $fail = 0 ] || exit 1
  echo "LAYERING: ok (0 files, ${#game[@]} game files, ${#session[@]} session files)"
  exit 0
fi
check() {
  local what=$1 re=$2 hits
  hits=$(grep -nE "$re" "${files[@]}" /dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//')
  if [ -n "$hits" ]; then
    echo "LAYERING: $what:"
    echo "$hits" | sed 's/^/  /'
    fail=1
  fi
}
check "enhancement layer used" '(crate|render|super)::enh\b|\buse[[:space:]]+.*\benh::'
check "QoL settings read" '\bqol::'
check "global mutable state" '\bstatic[[:space:]]+mut\b|\bthread_local!|\bstatic[[:space:]]+[A-Z_][A-Z0-9_]*[[:space:]]*:[^=]*\b(Mutex|RwLock|OnceLock|OnceCell|LazyLock|LazyCell|Cell|RefCell|Atomic[A-Za-z0-9]*)\b'
if [ $fail -ne 0 ]; then
  exit 1
fi
echo "LAYERING: ok (${#files[@]} files, ${#game[@]} game files, ${#session[@]} session files)"
