use crate::*;

/// `--flow rain`: the rain on the real play path, against the original
/// (under Wine, with the finished-career save): that finished career
/// (tests/fixtures/native_saves/career_complete.sav) resumed, the crib left,
/// then free roam idle at the crib's exit with the traffic out. The world
/// clock (0x7FB718) starts at 1.0 with the load (0x576D00) and runs with
/// the game; at the exit (camera x −64.7) the "Rain" map's row is [0.7,
/// 0.1], so 0x6134F0's |0.5·cos((clock − 1.294)°) + 0.5| ≤ 0.1 from clock
/// 144.42 to 218.17. The original: target 0 at 143.79, 0.7 (amount 0.213)
/// at 144.78; 0.7 at 217.86, 0 at 218.85.
pub(crate) fn flow_rain(game: &mut Game) -> Result<()> {
    let save = nfsu2::formats::native_save::NativeSave::parse(&crate::dev::private::require(
        crate::dev::private::FINISHED_CAREER,
        "--flow rain (career_complete.sav)",
    )?)?;
    let name = crate::dev::private::profile_name(&save);
    profiles::write(&name, &save)?;
    flow_resume_to_explore(game, name)?;
    anyhow::ensure!(game.sim.traffic.is_some(), "traffic out");
    let (mut start, mut end) = (None, None);
    let mut last = 0.0;
    let mut frames = 0;
    while end.is_none() {
        // Bound by updates as well as world time: a frozen-clock
        // regression must fail the probe instead of looping forever.
        anyhow::ensure!(frames < 300 * 60, "rain flow exceeded 300 s of updates (start {start:?})");
        frames += 1;
        game.advance_for(1.0 / 60.0)?;
        anyhow::ensure!(matches!(game.mode, Mode::Explore), "still in free roam (mode {})", flow_mode(&game.mode));
        let d = game.sim.drive.as_ref().context("drive")?;
        if d.clock - last >= 10.0 {
            last = d.clock;
            println!(
                "rain: clock {:.2} target {:.2} amount {:.3} wet {:.3} camera {:.1} {:.1}",
                d.clock, d.weather.target, d.weather.amount, d.weather.wet, d.camera.position.x, d.camera.position.y
            );
        }
        match (start, d.weather.target > 0.0) {
            (None, true) => start = Some((d.clock, d.weather.target, d.camera.position)),
            (Some(_), false) => end = Some(d.clock),
            _ => {}
        }
        anyhow::ensure!(d.clock < 300.0, "no shower by clock 300 (start {start:?})");
    }
    let (s, target, at) = start.unwrap();
    let e = end.unwrap();
    println!("rain: from clock {s:.2} to {e:.2}, target {target:.2}, camera {:.1} {:.1}", at.x, at.y);
    anyhow::ensure!(
        (at.x + 64.7).abs() < 1.0 && (at.y + 1473.9).abs() < 1.0,
        "the camera at the crib's exit, as the original's"
    );
    anyhow::ensure!((target - 0.7).abs() < 1e-3, "the map's amount 0.7 there");
    anyhow::ensure!((144.3..144.6).contains(&s), "the shower starts at clock 144.42 (the original: 143.79–144.78)");
    anyhow::ensure!((218.0..218.35).contains(&e), "the shower ends at clock 218.17 (the original: 217.86–218.85)");
    Ok(())
}
