#!/bin/bash
# Settings combination matrix (docs/settings-model.md "Matrix"): every Graphics row at every tier on Original and
# on "max" (every row at its top offered tier), every Customize Display value under max, every two rows at
# their top tiers (pairs), each settings-model rule's rows at the tiers that make it fire, and each Enhanced quality
# level with ray tracing and each main-page Upscaling / Anti-Aliasing choice (levels); each setting drawn
# FRAMES_EACH (default 3) frames in one bench process per scene, switched live as the Graphics page does
# (NFSU2_BENCH_MATRIX, settings_model::matrix_configs). Headless, 1280x720, NFSU2_RT=1 (the ray-traced tiers).
#
#   matrix.sh GAME OUTDIR        SCENES (default "explore race racing rain"), SPECS (default orig,max,rules,levels;
#                                PAIR_SCENES, default "explore racing", add pairs)
# Fails on a non-zero exit, any panic / wgpu validation error / RecvError in a log (the render workers' panics
# do not end the process), or a run that did not draw every setting. One line a scene.
set -u -o pipefail
. "$(dirname "$0")/../lib/common.sh"
ck_local_env
G=$(readlink -f "${1:?usage: matrix.sh GAME OUTDIR}"); OUT=${2:?usage: matrix.sh GAME OUTDIR}; mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
[ -x "$G" ] || ck_die "no binary $G"
ck_need_root
export NFSU2_HEADLESS_ONLY=1
# The game's asset cache: the caller's NFSU2_CACHE_DIR, else a folder of this run (never the player's cache).
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$OUT/cache}"
fail=0
for sc in ${SCENES:-explore race racing rain}; do
  spec=${SPECS:-orig,max,rules,levels}; case " ${PAIR_SCENES-explore racing} " in *" $sc "*) spec="$spec,pairs" ;; esac
  s=$sc; extra=(); [ "$sc" = rain ] && { s=drive; extra=(NFSU2_RAIN_CLOCK=162); }
  log=$OUT/matrix_$sc.log; t=$(date +%s)
  env "${extra[@]}" NFSU2_RT=1 XDG_DATA_HOME="$OUT/xdg-$sc" TMPDIR="$OUT" NFSU2_BENCH_SIZE=1280x720 NFSU2_BENCH_GPU=0 \
    NFSU2_BENCH_MATRIX="$spec" NFSU2_BENCH_MATRIX_FRAMES="${FRAMES_EACH:-3}" timeout 3600 "$G" --bench "$s" 10 > "$log" 2>&1 &
  pid=$!; echo "$pid matrix $sc" >> "$OUT/pids.log"; wait $pid; rc=$?
  bad=$(grep -ciE "$CK_PANIC_RE|RecvError" "$log")
  done_line=$(grep -E '^matrix: [0-9]+ of [0-9]+' "$log" | tail -1)
  read -r n m <<< "$(echo "$done_line" | sed -E 's/^matrix: ([0-9]+) of ([0-9]+).*/\1 \2/')"
  ok=1; { [ "$rc" = 0 ] && [ "$bad" = 0 ] && [ -n "${n:-}" ] && [ "$n" = "$m" ]; } || ok=0
  echo "matrix $sc ($spec): rc $rc, panic/validation $bad, ${n:-?} of ${m:-?} settings drawn, $(( $(date +%s) - t )) s"
  if [ $ok = 0 ]; then
    fail=1
    awk '/^  matrix [0-9]+\//{m=$0} tolower($0) ~ /panicked|validation error/{print "  at" m; exit}' "$log"
  fi
done
[ $fail = 0 ] && echo "MATRIX: all runs clean" || { echo "MATRIX: FAILED"; exit 1; }
