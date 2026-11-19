//! The launcher's side of `game --prepare` (docs/developing/prepare.md): the game
//! run with that flag as a child, its progress lines read as they come.
//! The step is offered, never needed: the game starts and works without
//! it, and "Skip" stops the child (what it wrote so far stays whole).

use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

pub use crate::launch::prepare::{Line, parse_line};

/// Where a prepare run is, for the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum State {
    /// Not started (or skipped).
    #[default]
    Idle,
    /// Running: the step that has started, of how many, and its words.
    Running {
        n: u32,
        of: u32,
        what: String,
    },
    Done,
    Failed(String),
}

impl State {
    /// The bar's fill, 0..1: the steps finished so far.
    pub fn fraction(&self) -> f32 {
        match self {
            State::Running { n, of, .. } if *of > 0 => (n.saturating_sub(1)) as f32 / *of as f32,
            State::Done => 1.0,
            _ => 0.0,
        }
    }

    /// The state after `line`. A failure stays; a later line never turns
    /// it back into progress.
    pub fn after(self, line: Line) -> State {
        match (self, line) {
            (State::Failed(e), _) => State::Failed(e),
            (_, Line::Step { n, of, what }) => State::Running { n, of, what },
            (_, Line::Done) => State::Done,
            (_, Line::Failed(e)) => State::Failed(e),
        }
    }

    /// The main screen's state line for a run behind it (started by
    /// itself: `Prepare::background`).
    /// "40 % · step 2 of 5: shader programs" (and the time left).
    pub fn progress_words(&self, left: Option<std::time::Duration>) -> String {
        let pct = (self.fraction() * 100.0).round() as u32;
        let mut s = match self {
            State::Running { n, of, what } if !what.is_empty() => format!("{pct} % · step {n} of {of}: {what}"),
            State::Running { n, of, .. } => format!("{pct} % · step {n} of {of}"),
            State::Done => "100 %".into(),
            _ => format!("{pct} %"),
        };
        if let Some(l) = left {
            let secs = l.as_secs().max(1);
            s += &if secs < 60 { format!(" · about {secs} s left") } else { format!(" · about {} min left", secs.div_ceil(60)) };
        }
        s
    }

    /// The time left, from the time since the start and the part done.
    pub fn time_left(&self, since_start: std::time::Duration) -> Option<std::time::Duration> {
        let f = self.fraction();
        (matches!(self, State::Running { .. }) && f > 0.05 && f < 1.0)
            .then(|| since_start.mul_f32((1.0 - f) / f))
    }

    pub fn background_words(&self) -> String {
        match self {
            State::Running { n, of, what } if !what.is_empty() => {
                format!("Ready to play · getting ready in the background: {what} ({n} of {of})")
            }
            State::Running { n, of, .. } => format!("Ready to play · getting ready in the background ({n} of {of})"),
            _ => "Ready to play · getting ready in the background".into(),
        }
    }

    /// The line under the bar.
    pub fn words(&self) -> String {
        match self {
            State::Idle => String::new(),
            State::Running { n, of, what } if what.is_empty() => format!("Step {n} of {of}…"),
            State::Running { n, of, what } => format!("Step {n} of {of}: {what}…"),
            State::Done => "Done. The first drive will start without a loading screen.".into(),
            State::Failed(e) => format!("Preparing did not finish: {e}. The game works without it."),
        }
    }
}

/// A prepare run in progress: the child and its lines as they arrive.
pub struct Job {
    child: std::process::Child,
    lines: Receiver<Line>,
}

/// Starts `exe --prepare` (the game folder in `NFSU2_ROOT` when given).
pub fn start(exe: &Path, root: Option<&Path>) -> Result<Job, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    if let Some(r) = root {
        cmd.env("NFSU2_ROOT", r);
    }
    // Windows: no console window for the step.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd
        .arg("--prepare")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("the game could not be started ({}): {e}", exe.display()))?;
    let out = child.stdout.take().expect("piped");
    let (tx, lines) = channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(l) = parse_line(&line)
                && tx.send(l).is_err()
            {
                break;
            }
        }
    });
    Ok(Job { child, lines })
}

impl Job {
    /// The lines that arrived since the last call applied to `state`, and
    /// whether the run has ended. A child that ended without saying so
    /// (stopped, crashed) counts as failed.
    pub fn poll(&mut self, mut state: State) -> (State, bool) {
        while let Ok(l) = self.lines.try_recv() {
            state = state.after(l);
        }
        match self.child.try_wait() {
            Ok(None) => (state, false),
            Ok(Some(status)) => {
                // What the reader had not sent yet.
                while let Ok(l) = self.lines.recv_timeout(std::time::Duration::from_millis(200)) {
                    state = state.after(l);
                }
                if !matches!(state, State::Done | State::Failed(_)) {
                    let how = status.code().map_or("it was stopped".to_string(), |c| format!("it ended with code {c}"));
                    state = State::Failed(how);
                }
                (state, true)
            }
            Err(e) => (State::Failed(e.to_string()), true),
        }
    }

    /// "Skip": the child stopped and waited for. The cache files it wrote
    /// are whole (each is renamed into place); the next start of the game
    /// or of this step makes what is missing.
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_follows_the_lines() {
        let mut s = State::default();
        assert_eq!(s.fraction(), 0.0);
        for (i, what) in ["graphics device", "shader programs", "the city"].iter().enumerate() {
            s = s.after(Line::Step { n: i as u32 + 1, of: 4, what: what.to_string() });
            assert_eq!(s.fraction(), i as f32 / 4.0);
            assert!(s.words().contains(what), "{}", s.words());
        }
        s = s.after(Line::Done);
        assert_eq!((s.clone(), s.fraction()), (State::Done, 1.0));
        // A failure stays whatever comes after it.
        let f = State::default().after(Line::Failed("x".into())).after(Line::Done);
        assert_eq!(f, State::Failed("x".into()));
        assert!(f.words().contains("works without it"));
    }
}
