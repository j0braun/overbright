# Live NIS component list

Second owned fresh-career capture on private display :176, same original
EXE hash as ../live-precision. The entry hook at 5AB3D0 preserves flags and
all registers and copies mover128/body256/chassis64 bytes into saved padding.
It observes rather than changes the FPU control word. All 404 calls were
PC24. The final snapshot links rigid body (vtable7A20CC) then NIS chassis
(vtable7A25B8); both next/previous links close at the composite sentinel.
`decode.py` verifies all links directly from retained bytes.

Instrumentation bytes and padding were restored and read back identically;
the owned Wine session was then stopped. No original on-disk files changed.
An initial capture-script attempt had a Python unpack-size error before
navigation; its finally block restored the hook, and the corrected run
reinstalled only after verifying original bytes and zero code padding.

This proves the final observed component list, not all scene configurations
or the contents of derivative scratch storage. The chassis no-op derivative
still requires observing integration scratch/state before implementing it.
