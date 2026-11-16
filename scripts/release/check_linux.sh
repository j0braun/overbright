#!/bin/bash
# Checks a Linux release binary for portability: no "x86 ISA needed" note (it would refuse older
# CPUs), glibc symbol versions <= 2.35 (Ubuntu 22.04), the shared libraries the game is allowed to
# need, no RPATH/RUNPATH, and no build machine home paths in its strings.
#   check_linux.sh <elf>        exit 0 when every check passes
set -uo pipefail
bin=${1:?usage: check_linux.sh <elf>}
GLIBC_MAX=2.35
ALLOWED="libudev.so.1 libasound.so.2 libgcc_s.so.1 libm.so.6 libc.so.6 ld-linux-x86-64.so.2"
fail=0
ok() { echo "ok    $*"; }
bad() { echo "FAIL  $*"; fail=1; }

if readelf -n "$bin" | grep -qi 'ISA needed'; then bad "x86 ISA needed note: $(readelf -n "$bin" | grep -i 'ISA needed' | head -1)"
else ok "no x86 ISA needed note"; fi

max=$(readelf -V --wide "$bin" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)
if [ -n "$max" ] && [ "$(printf '%s\n%s\n' "$max" "$GLIBC_MAX" | sort -V | tail -1)" = "$GLIBC_MAX" ]; then ok "highest GLIBC_ $max (<= $GLIBC_MAX)"
else bad "highest GLIBC_ ${max:-none} (> $GLIBC_MAX)"; fi

needed=$(readelf -d "$bin" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
extra=""
for lib in $needed; do case " $ALLOWED " in *" $lib "*) ;; *) extra="$extra $lib" ;; esac; done
if [ -z "$extra" ]; then ok "NEEDED:" $needed; else bad "NEEDED outside the allow-list:$extra"; fi

if readelf -d "$bin" | grep -qE '\((RPATH|RUNPATH)\)'; then bad "RPATH/RUNPATH present"; else ok "no RPATH/RUNPATH"; fi

homes=$(strings -a "$bin" | grep -c '/home/')
if [ "$homes" = 0 ]; then ok "strings: 0 /home/"; else bad "strings: $homes /home/ (e.g. $(strings -a "$bin" | grep -m1 '/home/'))"; fi
exit $fail
