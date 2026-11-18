//! The game as a child process for a simulation: started headless with
//! the mod saved, its stream read on a thread (the window never waits on
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

/// The game's command line for a simulation streamed to standard output.
pub fn args(folder: &str, what: &What, seed: u64) -> Vec<String> {
    let mut a = vec!["--map-mod".to_string(), folder.to_string()];
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
}

impl SimChild {
    /// Start `exe` with `args` for the game folder `root`. No window and
    /// no sound: the simulate mode opens neither, and the environment says
    /// so too (a game that did not know the flags stops at its usage).
    pub fn start(exe: &Path, args: &[String], root: &Path) -> Result<SimChild, String> {
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
        Ok(SimChild { stdin: child.stdin.take(), child, lines: rx, quit_at: None })
    }

    /// The lines that arrived (at most `max`, so a frame stays short) and
    /// whether the game still runs.
    pub fn take(&mut self, max: usize) -> (Vec<String>, bool) {
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

    /// Ask the game to stop (`quit`); killed if it has not a few seconds later.
    pub fn stop(&mut self) {
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
        assert_eq!(
            args("my-map", &What::Race("dock".into()), 7),
            ["--map-mod", "my-map", "--map-race", "dock", "--simulate", "--stream", "-", "--sim-seed", "7"]
        );
        assert_eq!(
            args("m", &What::World(Vec3::new(1.5, -2.0, 3.0), 90.0), 1),
            ["--map-mod", "m", "--map-at", "1.5,-2,3,90", "--simulate-world", "--stream", "-", "--sim-seed", "1"]
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
        let mut c = SimChild::start(&exe, &[], s.path()).unwrap();
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
