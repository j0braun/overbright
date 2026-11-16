/// How many world steps (physics + one collision pass each) a frame of
/// `dt` takes: one, as SPEED2.EXE runs them. Its main loop 0x5811D0 gets
/// the frame's time from 0x5EA370 / 0x601A70 into 0x865150 and calls the
/// world update 0x609CF0 once with it; the mover (Mover_UpdateFrame
/// 0x5B72E0) splits that frame into ftol(dt / 0.0044) + 1 sub-steps
/// itself, the player's input update (steering 0x416BB0, its running
/// means) runs once, and the collision step 0x5FFA60 once. Verified under
/// Wine (the maintainer's private notes): at 35–60 fps the original logs one mover
/// call per frame with dt up to 0.037 s. On the tick clock every dt is a
/// tick's, so the former `INTRO_STEP_HZ` split is gone.
pub(crate) fn world_steps(_dt: f32) -> u32 {
    1
}
