//! The game as a child process for a simulation: started headless on the
//! mod's run copy (`--map-file`, unsaved changes included), its stream read on a thread (the window never waits on
//! it), stopped with `quit` on its standard input.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError};

use glam::Vec3;

use super::format::SimMode;

/// What to simulate.
#[derive(Clone, Debug, PartialEq)]
pub enum What {
    /// The custom race with this id.
    Race(String),
    /// Free roam from this spot (heading in degrees around +z from +x).
    World(Vec3, f32),
}

impl What {
    pub fn mode(&self) -> SimMode {
        match self {
            What::Race(_) => SimMode::Race,
            What::World(..) => SimMode::World,
        }
    }
}

/// The game's command line for a simulation of the mod file `file`
/// streamed to standard output.
pub fn args(file: &Path, what: &What, seed: u64) -> Vec<String> {
    let mut a = vec!["--map-file".to_string(), file.display().to_string()];
    match what {
        What::Race(id) => a.extend(["--map-race".into(), id.clone(), "--simulate".into()]),
        What::World(p, h) => {
            a.extend(["--map-at".into(), format!("{},{},{},{}", p.x, p.y, p.z, h), "--simulate-world".into()]);
        }
    }
    a.extend(["--stream".into(), "-".into(), "--sim-seed".into(), seed.to_string()]);
    a
}

/// A running simulation.
pub struct SimChild {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    /// When `quit` was sent (the game is killed if it has not ended a
    /// couple of seconds later).
    quit_at: Option<std::time::Instant>,
    /// The run's place among the few (`link::Slot`), when Studio started it.
    slot: Option<crate::tools::map::link::Slot>,
    /// The link folder (`--studio-link`): asked to stop there too, and read
    /// for the game's health.
    link: Option<std::path::PathBuf>,
}

impl SimChild {
    /// Start `exe` with `args` for the game folder `root`. No window and
    /// no sound: the simulate mode opens neither, and the environment says
    /// so too (a game that did not know the flags stops at its usage).
    pub fn start(exe: &Path, args: &[String], root: &Path) -> Result<SimChild, String> {
        SimChild::start_in(exe, args, root, None, None)
    }

    /// [`SimChild::start`] as Studio does: in one of the few places for a
    /// run (refused when they are all taken) and linked
    /// (`--studio-link <link>`, `tools::map::link`).
    pub fn launch(exe: &Path, args: &[String], root: &Path, link: &Path, maps: &Path) -> Result<SimChild, String> {
        let slot = crate::tools::map::link::Slot::take(maps)?;
        crate::tools::map::link::clear(link);
        let mut a = args.to_vec();
        a.extend(["--studio-link".to_string(), link.display().to_string()]);
        SimChild::start_in(exe, &a, root, Some(slot), Some(link.to_path_buf()))
    }

    fn start_in(
        exe: &Path,
        args: &[String],
        root: &Path,
        slot: Option<crate::tools::map::link::Slot>,
        link: Option<std::path::PathBuf>,
    ) -> Result<SimChild, String> {
        let mut child = Command::new(exe)
            .args(args)
            .env("NFSU2_ROOT", root)
            .env("NFSU2_HEADLESS_ONLY", "1")
            .env("NFSU2_AUDIO_HOST", "none")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("The game could not start ({}): {e}", exe.display()))?;
        let out = child.stdout.take().ok_or("no stream from the game")?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("sim stream".into())
            .spawn(move || {
                for line in std::io::BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(SimChild { stdin: child.stdin.take(), child, lines: rx, quit_at: None, slot, link })
    }

    /// The lines that arrived (at most `max`, so a frame stays short) and
    /// whether the game still runs.
    pub fn take(&mut self, max: usize) -> (Vec<String>, bool) {
        if let Some(s) = &self.slot {
            s.touch();
        }
        let mut v = Vec::new();
        let mut open = true;
        while v.len() < max {
            match self.lines.try_recv() {
                Ok(l) => v.push(l),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        if let Some(t) = self.quit_at
            && t.elapsed().as_secs_f32() > 3.0
        {
            let _ = self.child.kill();
        }
        let running = open || matches!(self.child.try_wait(), Ok(None));
        if !open {
            let _ = self.child.try_wait();
        }
        (v, running && open)
    }

    /// What the game says of itself over the link (None: not linked, or
    /// nothing said yet).
    pub fn health(&self) -> Option<crate::tools::map::link::GameState> {
        crate::tools::map::link::read_state(self.link.as_ref()?)
    }

    /// Ask the game to stop (`quit`); killed if it has not a few seconds later.
    pub fn stop(&mut self) {
        if let Some(l) = self.link.as_ref().filter(|_| self.quit_at.is_none()) {
            // The link's stop too (the game answers either).
            let seq = crate::tools::map::link::read_state(l).map_or(0, |g| g.seq) + 1;
            let _ = crate::tools::map::link::write_command(
                l,
                &crate::tools::map::link::Command { seq, op: crate::tools::map::link::Op::Stop, aim: None, focus: None },
            );
        }
        if let Some(mut i) = self.stdin.take() {
            let _ = writeln!(i, "quit");
            let _ = i.flush();
        }
        self.quit_at.get_or_insert_with(std::time::Instant::now);
    }
}

impl Drop for SimChild {
    fn drop(&mut self) {
        self.stop();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_asks_for_a_headless_stream() {
        let f = Path::new("/m/my-map/.editor/run/map.obmap");
        assert_eq!(
            args(f, &What::Race("dock".into()), 7),
            [
                "--map-file",
                "/m/my-map/.editor/run/map.obmap",
                "--map-race",
                "dock",
                "--simulate",
                "--stream",
                "-",
                "--sim-seed",
                "7"
            ]
        );
        assert_eq!(
            args(f, &What::World(Vec3::new(1.5, -2.0, 3.0), 90.0), 1)[2..],
            ["--map-at", "1.5,-2,3,90", "--simulate-world", "--stream", "-", "--sim-seed", "1"]
        );
    }

    /// A stand-in game (a shell script printing a stream) read to its end.
    #[cfg(unix)]
    #[test]
    fn a_child_stream_is_read_without_waiting() {
        let s = crate::testkit::Scratch::new("sim-child");
        let exe = s.path().join("fake-game");
        std::fs::write(
            &exe,
            "#!/bin/sh\necho 'OBSIM 1 mode=race seed=1 race=r track=1 laps=1 checkpoints=2'\necho 'F 0 -1.000'\necho 'END finish'\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Tried again while the script is "busy": a test on another thread
        // may fork while it is still open for writing (ETXTBSY until that
        // child execs).
        let mut tries = 0;
        let mut c = loop {
            match SimChild::start(&exe, &[], s.path()) {
                Err(e) if e.contains("os error 26") && tries < 100 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                r => break r.unwrap(),
            }
        };
        let mut rec = super::super::record::Recording::default();
        let t = std::time::Instant::now();
        loop {
            let (lines, running) = c.take(1000);
            for l in &lines {
                rec.push_line(l);
            }
            if !running || t.elapsed().as_secs() > 10 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(rec.frames.len(), 1);
        assert_eq!(rec.end.as_deref(), Some("finish"));
    }
}
