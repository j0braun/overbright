Scripts of the engine-bed comparison (docs/audio-levels.md §2b).
Original (PARITY_TOOLS = tools/parity, PARITY_WINE = a fastgpu tools copy, a private Wine copy and Xvfb display):
  rt_free.py SAVE OUT --copy C --display N --mode roll|idle|launch --kmh V --gear G --ratio R --secs S --fixed 120 [--at x,y,z,deg]
  drive_rt_f.py SCENARIO.toml OUT --copy C --display N   (RT_FIXED=120: the first drag at the 120 fps cadence)
  beside them: native_poll.py (audio-levels scripts), hmpoll.py COPY OUT SECS LOG, ratepoll.py COPY OUT LOG
Clone: clone_free.sh (explore shot, same save), tools/parity/parity.py clone (the drag).
Analysis: freebands.py NAT CLONE --ranges a-b (total and bands by game time), bandbins.py NAT CLONE (binned by CAR p1),
  cmpdrive.py NAT CLONE (audio rpm, throttle, speed), layercmp.py NAT CLONE FROM TO (GIN bytes, CAR p4/p14/p25),
  hitbed.py NAT CLONE (barrel hit and the 0.5 s before it).
Gears: the original's car +0x434 counts 2 = first (60 km/h: 3, ratio 7.087; 120: 4, 5.154; 200: 6, 3.184); the clone's
NFSU2_FX_GEAR counts 1 = first. The save: the stage-2 save with speech 0, game music 0, the manual gearbox (player +0x15 = 1).
