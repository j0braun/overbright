# Rest component evidence

`native_threshold.py` runs authentic 0x5B3672..0x5B36ED machine code and
constant 0x7A1934 in an isolated PE/Wine prefix. Synthetic support-point
inputs and continuation stubs expose the near-zero decision only; this
is not a full native collision replay. 1, 2 and 3 mm take the zero path;
4 and 10 mm continue. Executable/code hashes are in native-threshold.json.

Reproduce with LLVM mc, GNU ld i386pe and Wine:

```
NFSU2_EXE='/path/to/game/SPEED2.EXE' NFSU2_PROBE_DIR='/private/temp/dir' \
python3 $OB_PRIVATE/docs/evidence/rest-components/native_threshold.py
```

`bump-hulls.json` contains the final clone collision fixture vertices with
the zero-dt pass restored. Its strictly positive separating-axis gap
proves they are disjoint even though the native-style query returns zero.

`x2-first 64.bin` and metadata preserve the first 64 telemetry calls from
an original physics15 Wine capture (206, initial teleport). These
are recovered evidence, not a fresh the merged branch game run. The metadata gives
record offsets, stride, original hashes and instrumented scope. `clutch-rea.txt`
is the fresh Ghidra decompilation of the authentic clutch state machine.

`decode_x2.py` verifies the excerpt hash and decodes telemetry.
`check_hulls.py` independently verifies strict geometric separation.
