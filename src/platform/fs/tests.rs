//! Windows only (run under Wine by the container's test binary; the real
//! proof is NTFS on a tester's machine). Each test prints what the plain
//! `std::fs::rename` did in the same situation, so a run shows whether the
//! platform modelled the conflict at all.

use super::*;
use std::os::windows::fs::OpenOptionsExt;

struct Dir(std::path::PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(tag: &str) -> Dir {
    let d = std::env::temp_dir().join(format!("nfsu2-winfs-{tag}-{}", crate::unique_tag()));
    std::fs::create_dir_all(&d).unwrap();
    Dir(d)
}

/// FILE_SHARE_READ only: no write, no delete sharing (what a scanner or a
/// sync client holding the file looks like to a rename).
const SHARE_READ: u32 = 1;

#[test]
fn a_rename_over_a_held_file_is_retried() {
    let d = dir("held");
    let (to, from, probe) = (d.0.join("options.txt"), d.0.join("options.tmp"), d.0.join("probe.tmp"));
    std::fs::write(&to, b"old").unwrap();
    std::fs::write(&from, b"new").unwrap();
    std::fs::write(&probe, b"probe").unwrap();
    let held = std::fs::OpenOptions::new().read(true).share_mode(SHARE_READ).open(&to).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        drop(held);
    });
    let plain = std::fs::rename(&probe, &to);
    eprintln!("plain rename over a held file: {plain:?} (held: {})", plain.as_ref().is_err_and(is_held));
    let start = std::time::Instant::now();
    let r = rename_replace(&from, &to);
    let took = start.elapsed();
    release.join().unwrap();
    eprintln!("rename_replace: {r:?} after {took:?}");
    r.unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), b"new");
    if plain.is_err() {
        assert!(took >= std::time::Duration::from_millis(100), "waited for the holder ({took:?})");
    }
}

#[test]
fn a_rename_over_a_file_held_too_long_fails() {
    let d = dir("stuck");
    let (to, from) = (d.0.join("options.txt"), d.0.join("options.tmp"));
    std::fs::write(&to, b"old").unwrap();
    std::fs::write(&from, b"new").unwrap();
    let _held = std::fs::OpenOptions::new().read(true).share_mode(SHARE_READ).open(&to).unwrap();
    let start = std::time::Instant::now();
    let r = rename_replace(&from, &to);
    eprintln!("rename_replace over a file held throughout: {r:?} after {:?}", start.elapsed());
    if let Err(e) = r {
        assert!(is_held(&e), "{e}");
        assert!(start.elapsed() <= RETRY_FOR + std::time::Duration::from_millis(200));
        assert_eq!(std::fs::read(&from).unwrap(), b"new", "the new file is still there to retry");
    }
}

/// A file mapped by this process (as a [`crate::cache::CacheFile`] still
/// in use maps its file).
fn mapped(path: &Path, byte: u8) -> (std::fs::File, memmap2::Mmap) {
    std::fs::write(path, vec![byte; 4096]).unwrap();
    let file = std::fs::File::open(path).unwrap();
    // SAFETY: the test's own file, not changed in place while mapped.
    let map = unsafe { memmap2::Mmap::map(&file).unwrap() };
    (file, map)
}

#[test]
fn a_mapped_cache_file_is_replaced() {
    let d = dir("mapped");
    // What a plain rename does over a mapped file, on a file of its own.
    let (other, probe) = (d.0.join("other.nfc"), d.0.join("probe"));
    let _other = mapped(&other, 1);
    std::fs::write(&probe, b"x").unwrap();
    eprintln!("plain rename over a mapped file: {:?}", std::fs::rename(&probe, &other));
    let to = d.0.join("track-L4RA.nfc");
    let (file, map) = mapped(&to, 1);
    crate::cache::replace_file(&to, &[2u8; 4096]).unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), vec![2u8; 4096], "the new file is in place");
    assert!(map.iter().all(|&b| b == 1), "the mapping still reads the old data");
    let left: Vec<String> =
        std::fs::read_dir(&d.0).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    eprintln!("left in the folder: {left:?}");
    for n in left.iter().filter(|n| n.starts_with("track-L4RA.nfc.")) {
        // `<name>.<pid>-<hex>-<n>.<seq>.tmp`: the stale-file sweep's form.
        assert!(n.ends_with(".tmp") && n.split('.').count() == 5, "{n}");
    }
    drop(map);
    drop(file);
}

/// Once the mapping and the file are closed, the file is an ordinary one
/// again: replaced by the first rename (prints what happened, for the
/// record of what the platform does).
#[test]
fn an_unmapped_file_is_replaced() {
    let d = dir("unmapped");
    let (to, from) = (d.0.join("track-L4RA.nfc"), d.0.join("new.tmp"));
    drop(mapped(&to, 1));
    std::fs::write(&from, [2u8; 4096]).unwrap();
    let plain = std::fs::rename(&from, &to);
    eprintln!("plain rename over a file mapped before: {plain:?}");
    if plain.is_err() {
        let start = std::time::Instant::now();
        let r = rename_replace_aside(&from, &to, || d.0.join("aside.tmp"));
        eprintln!("rename_replace_aside: {r:?} after {:?}", start.elapsed());
        r.unwrap();
    }
    assert_eq!(std::fs::read(&to).unwrap(), vec![2u8; 4096]);
}
