//! The few file operations the history makes, behind one trait so a test can stop any of them part way, as a
//! killed process or a power cut would ([`Faulty`]). [`Real`] is what Studio uses: every write is durable
//! before it returns (the data flushed to the disk), and a whole-file write goes through a temporary file and a
//! rename (`ob_core::platform::fs::write_atomic`), so a file is the old one or the new one, never half.

use std::io::{self, Write as _};
use std::path::Path;
use std::sync::Mutex;

/// The history's file operations.
pub trait Disk: Send + Sync {
    /// The whole file (`NotFound` when there is none).
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Add `bytes` at the end of the file (made if missing), flushed to the disk.
    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Cut the file to `len` bytes, flushed to the disk.
    fn truncate(&self, path: &Path, len: u64) -> io::Result<()>;
    /// Replace the file with `bytes` (temporary file, flush, rename).
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Remove the file (no error when there is none).
    fn remove(&self, path: &Path) -> io::Result<()>;
}

/// The real file system.
#[derive(Clone, Copy, Debug, Default)]
pub struct Real;

impl Disk for Real {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        f.write_all(bytes)?;
        f.sync_data()
    }

    fn truncate(&self, path: &Path, len: u64) -> io::Result<()> {
        let f = std::fs::OpenOptions::new().write(true).open(path)?;
        f.set_len(len)?;
        f.sync_data()
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        ob_core::platform::fs::write_atomic(path, bytes, true)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

/// How much of the stopped operation reaches the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// Nothing.
    Start,
    /// One byte.
    One,
    /// Half of it.
    Half,
    /// All but the last byte.
    AllButOne,
}

impl Cut {
    pub const ALL: [Cut; 4] = [Cut::Start, Cut::One, Cut::Half, Cut::AllButOne];

    fn of(self, len: usize) -> usize {
        match self {
            Cut::Start => 0,
            Cut::One => len.min(1),
            Cut::Half => len / 2,
            Cut::AllButOne => len.saturating_sub(1),
        }
    }
}

/// The real file system that stops at one operation, as a crash would: operation number `at` (0 = the first)
/// writes only part of its bytes ([`Cut`]) and fails, and every operation after it fails without touching
/// anything. A stopped whole-file write leaves its partial temporary file and never renames it (what
/// `write_atomic` leaves when the process dies before its rename).
#[derive(Debug)]
pub struct Faulty {
    state: Mutex<FaultState>,
}

#[derive(Debug)]
struct FaultState {
    /// Operations made so far.
    done: usize,
    at: Option<usize>,
    cut: Cut,
    crashed: bool,
}

/// The error a stopped operation gives.
pub fn crash() -> io::Error {
    io::Error::other("stopped by the fault test")
}

impl Faulty {
    /// Stop at operation `at` (None: never; counts the operations).
    pub fn new(at: Option<usize>, cut: Cut) -> Faulty {
        Faulty { state: Mutex::new(FaultState { done: 0, at, cut, crashed: false }) }
    }

    /// How many operations were made (the stopped one counted).
    pub fn ops(&self) -> usize {
        self.state.lock().map(|s| s.done).unwrap_or(0)
    }

    /// Whether it has stopped.
    pub fn crashed(&self) -> bool {
        self.state.lock().map(|s| s.crashed).unwrap_or(true)
    }

    /// None: go ahead; Some(n): stop after writing `n` of `len` bytes.
    fn step(&self, len: usize) -> io::Result<Option<usize>> {
        let mut s = self.state.lock().map_err(|_| crash())?;
        if s.crashed {
            return Err(crash());
        }
        let n = s.done;
        s.done += 1;
        if s.at == Some(n) {
            s.crashed = true;
            return Ok(Some(s.cut.of(len)));
        }
        Ok(None)
    }
}

impl Disk for Faulty {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if self.crashed() {
            return Err(crash());
        }
        Real.read(path)
    }

    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        match self.step(bytes.len())? {
            None => Real.append(path, bytes),
            Some(n) => {
                Real.append(path, &bytes[..n])?;
                Err(crash())
            }
        }
    }

    fn truncate(&self, path: &Path, len: u64) -> io::Result<()> {
        match self.step(0)? {
            None => Real.truncate(path, len),
            Some(_) => Err(crash()),
        }
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        match self.step(bytes.len())? {
            None => Real.write_atomic(path, bytes),
            Some(n) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(path.with_extension("tmp"), &bytes[..n])?;
                Err(crash())
            }
        }
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        match self.step(0)? {
            None => Real.remove(path),
            Some(_) => Err(crash()),
        }
    }
}
