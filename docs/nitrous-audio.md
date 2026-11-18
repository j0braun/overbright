# Nitrous audio: the GB_NOS stream (2026-11-04)

Issue: the sound at high speed with nitrous did not stop after slowing down. Evidence: the maintainer's private captures.

## The stuck voice
GB_NOS. This is not a car-bank voice. It is the NIS stream's SoundFX line
**GB_NOS**: SoundFX_Select (1, 4) → `nisaudio` event 0x400E, record 13
(keys [0, 2]). The record is an **89.5 s** stereo 32 kHz stream at a steady
−6 dB RMS (the other SoundFX records last 1.7–9.5 s). The clone played it
at the NIS level (gain 0.52) and never stopped it when the shot ended, so it
went on for up to 89.5 s after the car had slowed down or stopped. The
GB_NOS request has been in the clone since about 2026-10-31. It
was not caused by the day's audio, engine-level and NIS-audio changes. It only shows with a tank
filled past 1.5 (style refills fill up to 2 tanks, 0x592650), so a fresh
tank never triggers it.

## Traced (SPEED2.EXE, CARSFX_Nitrous update 0x484F20; verified)
* State 0 → 1 (0x485014..0x4850DC): a shot is running (0x59FA40) and there
  is no purge (0x5ECDC0 = 0). **GB_NOS** is asked for when the tank
  fraction 0x5925C0 is > 1.5 (`fcomp [0x785110]` = 1.5, `test ah,0x41`),
  0x431DD0(car), [[0x8900D8]] = 4, 0x8A329C = 0, 0x88EC68 = 0 and the NIS
  stream ([0x82BA30] +8) has no pending request (+0x44 = 0). The request is
  NisAudio_Request(1, "GB_NOS" hash, 4, callback 0x45F4A0, 0) at 0x4850A1.
  Then FX_NITROUS (0x472AF0) and the 0 → 1 ramp over 400 ms.
* State 1 → 0 (0x484F57..0x485005): the shot has gone or a purge started.
  FX_NITROUS is released (0x45F2A0) and ramps to 0 over 1000 ms. Then, when
  the audio object's kind is +0x14 = 1:
  * 0x82BB29 = 0;
  * the NIS stream is **stopped at once**: 0x7227CB on [[obj +0x34] +0x18],
    with obj = [0x82BA30] +8;
  * its queue is flushed (0x482970(1));
  * a pending request (+0x44) is freed (0x575640, "Deleting Stream ID %d
    m_queuedObject due to Release Resource").

  Whatever the NIS stream plays is stopped, not only GB_NOS. Kind 1 is the
  player's car object, as BLOWNENGINE's gate reads it (formats.md); the
  clone runs `PlayerSfx` for the player only (**inferred** mapping).

## Observed under Wine
Run with vanilla SPEED2.EXE in a private Wine copy on display
:217, with `--audio-file`. Quick Race Circuit, Peugeot 206. The tank was set
through `peek` (car = [[0x8900B4] +4], size +0x454 = 8000, left +0x45C). The
sequence was gas 3 s, Left Alt about 1.7 s, then release and brake. Script:
`gbnos.py`. Levels: `rms.py`, 0.25 s windows of the capture.

| tank | 0x82BB29 during / after | capture during the shot | after the release |
|---|---|---|---|
| left 8000 (1.0) | 0 / 0 | −18 dBFS, as before the shot | −18 |
| left 16000 (2.0), run 1 | 1 / 0 | −10.7 (+7 dB) | −15.8 in the first 0.25 s, then −18.8 |
| left 16000 (2.0), run 2 | 1 / 0 | −10.2 … −11.0 | −17.5 in the first 0.25 s |

So GB_NOS plays only while an overfilled shot runs. The original stops it
within one 0.25 s window of the key's release.

## Clone
* Before the fix, with free roam, the a community save Lancer and the tank refilled to 2,
  nitrous held from 9 s, braking at 13.5 s: the GB_NOS track (2,865,475
  frames) still played at 31.5 s with the car standing (`clone-before.txt`).
  Free roam and drags with a tank at 1.0 or below: every car-bank voice
  ended as it should (nitrous loop, purge, turbo, wind, road and skids,
  checked per voice; `clone-probes.rs.txt`).
* Fix: on the shot's end edge (`PlayerSfx::nos_edge` = Some(false)),
  `car_sounds` clears the GB_NOS flag **and** stops the NIS stream
  (`Streams::stop_nis`, at once). After the fix the track is gone in the
  frame the stage reaches 0 (`clone-after.txt`).
* Test: `gb_nos_stream_ends_with_the_shot`. It runs free roam with a 4 s tank
  filled to 2, throttle, then nitrous, then release and brake. It checks
  that GB_NOS starts with the shot and stops in the same frame as the shot.
  It fails without the fix.

## Open
* The clone's queued NIS request has no counterpart: `play_nis` replaces
  the playing line, so there is nothing for 0x482970 / +0x44 to flush.
* 0x431DD0, 0x8A329C and 0x88EC68 are not modelled in the GB_NOS start
  gate. The clone uses the tank, flow 4 and an idle NIS stream.
* The stop also cuts any other NIS line playing at the shot's end, as
  natively. This was not observed under Wine: no other line was playing.
