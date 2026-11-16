The rivals' launch wheelspin and their skid sound (docs/audio-levels.md §5, docs/native-fixes/ai-skids.tsv).

Original: SPEED2.EXE under Wine, private Xvfb, memory reads only.
  drag-n1      first drag (STAGE_1_DRAG_1), parity harness lockstep, 1/60 s frames, audio events polled
  drag120-n1   the same, the cave's frame time fixed at 1/120 s (fixed_fps = 120), lockstep
  rt1, rt2     the same drag at the original's own pace with every frame 1/120 s (the engine-bed method:
               drive_rt_f.py with RT_FIXED=120), WAV recorded, the cars polled by carpoll.py
  circ-n1, -n2 first circuit (STAGE_1_CIRCUIT_1), lockstep 1/60 s
Clone: tools/parity/parity.py clone on the same plans (drag120-n1's for the 1/120 s replay; the scenario's key
times doubled so that its frames land on the 1/120 s frames), with audio and NFSU2_AUDIO_STEMS_SPLIT.

Fields (tools/parity/fields.toml): xslip_* = car +0x3B4 + 8i (the exported slip), spin_* = mover +0x38.. +0x28,
clutch = mover +0x4C -> +0x14, rpm +0x440, speed +0x430, gear +0x434 (2 = first).

launch.tsv          per rival, native and clone, every 6th (60 fps) / 12th (120 fps) frame
p3-*.txt            the original's AI FX_SKID p3/p4/p5 (one-event instances, p0 = p1 = p2) by record frame;
                    the event clock is polled, so its frame is approximate (about ±0.1 s)
realtime-rt*.txt    rt1 / rt2: the rivals by time since the green, with the nearest p3 event
bands.txt           freebands.py (the engine-bed analysis): original rt1 / rt2 against the clone before (no AI
                    skids), after, and the AI skid layer alone (NFSU2_AUDIO_SOLO=FX_SKID.ai), 0.1 s windows
carpoll.py, pollview.py, p3.py, launch.py   the scripts that made them
