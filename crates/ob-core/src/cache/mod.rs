//! Loading support: source files mapped instead of read, and (see below)
//! the on-disk cache of parsed data.

use std::path::Path;

mod bytes;
mod dir;
mod file;
mod file_cached;
mod format;
mod keys;
mod persist;
mod prune;
pub mod read_ahead;
mod temp;
mod track;

pub use self::bytes::*;
pub use self::dir::*;
pub use self::file::*;
pub use self::file_cached::*;
pub use self::format::*;
pub use self::keys::*;
pub use self::persist::*;
use self::prune::*;
pub use self::prune::{SURFACE_MAPS, touch_file, wait_touches};
pub use self::temp::*;
pub use self::track::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// A cache file is used only while it matches its source and is whole:
    /// a changed source, a flipped byte, a cut-off file or another build's
    /// file are misses; a touched source with the same content is a hit.
    #[test]
    fn stale_and_corrupt_files_are_misses() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-unit-{}", std::process::id()));
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("A.BIN"), b"source data one").unwrap();
        let cache = dir.join("cache");
        let data: Bytes = (0..10_000u32).flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>().into();
        let sections = vec![("data".to_string(), data.clone()), ("more".to_string(), Bytes::from(&b"x"[..]))];
        write_now(&cache, &root, "unit", &["A.BIN".into()], &sections, crate::BUILD_ID).unwrap();
        let open = || CacheFile::open_in(&cache, crate::BUILD_ID, &root, "unit", &["A.BIN"]);
        let c = open().expect("a fresh file opens");
        assert!(c.bytes("data").unwrap()[..] == data[..] && !c.retimed);
        drop(c);
        let path = cache.join(file_name("unit"));
        let good = std::fs::read(&path).unwrap();
        // A byte flipped in a section.
        let mut bad = good.clone();
        let at = bad.len() - 300;
        bad[at] ^= 1;
        std::fs::write(&path, &bad).unwrap();
        assert!(open().is_none(), "a flipped byte is a miss");
        // Cut off.
        std::fs::write(&path, &good[..good.len() / 2]).unwrap();
        assert!(open().is_none(), "a truncated file is a miss");
        // The header damaged.
        let mut bad = good.clone();
        bad[40] ^= 0xFF;
        std::fs::write(&path, &bad).unwrap();
        assert!(open().is_none(), "a damaged header is a miss");
        // Another kind, another build.
        std::fs::write(&path, &good).unwrap();
        assert!(CacheFile::open_in(&cache, crate::BUILD_ID, &root, "other", &["A.BIN"]).is_none());
        let mut bad = good.clone();
        let header = CacheFile::header(&good).unwrap();
        let mut h = header.clone();
        h.build ^= 1;
        let mut w = Writer::default();
        h.put(&mut w);
        bad[28..28 + w.buf.len()].copy_from_slice(&w.buf);
        bad[20..28].copy_from_slice(&xxhash_rust::xxh3::xxh3_64(&w.buf).to_le_bytes());
        std::fs::write(&path, &bad).unwrap();
        assert!(open().is_none(), "another build's file is a miss");
        // The source rewritten with the same bytes: a hit, retimed.
        std::fs::write(&path, &good).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join("A.BIN"), b"source data one").unwrap();
        let c = open().expect("same content still opens");
        assert!(c.retimed);
        drop(c);
        // Changed content of the same size: a miss.
        std::fs::write(root.join("A.BIN"), b"source data two").unwrap();
        assert!(open().is_none(), "a changed source is a miss");
        // A missing source: a miss.
        std::fs::remove_file(root.join("A.BIN")).unwrap();
        assert!(open().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cache file that is empty, shorter than its header, garbage or a
    /// valid file cut at any point is a miss (never a panic), and the
    /// rewrite that follows a miss is a hit with the right bytes: a corrupt
    /// or short cache rebuilds itself.
    #[test]
    fn corrupt_or_short_files_rebuild() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-rebuild-{}", std::process::id()));
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("B.BIN"), b"source").unwrap();
        let cache = dir.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let path = cache.join(file_name("rebuild"));
        let open = || CacheFile::open_in(&cache, crate::BUILD_ID, &root, "rebuild", &["B.BIN"]);
        let data: Bytes = (0..3000u32).flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>().into();
        let sections = vec![("data".to_string(), data.clone())];
        write_now(&cache, &root, "rebuild", &["B.BIN".into()], &sections, crate::BUILD_ID).unwrap();
        let good = std::fs::read(&path).unwrap();
        let mut garbage: Vec<u8> = (0..good.len()).map(|i| (i * 31 + 7) as u8).collect();
        garbage[..8].copy_from_slice(MAGIC);
        let mut cases: Vec<Vec<u8>> =
            vec![Vec::new(), good[..1].to_vec(), good[..27].to_vec(), good[..28].to_vec(), garbage];
        // Every cut through the header and the start of the data, then
        // coarser steps to the end.
        cases.extend((29..good.len().min(600)).chain((600..good.len()).step_by(97)).map(|n| good[..n].to_vec()));
        for bad in cases {
            let n = bad.len();
            std::fs::write(&path, &bad).unwrap();
            assert!(open().is_none(), "{n} bytes of {} open", good.len());
            // The miss is followed by a write (what `write` does in the
            // background): the next open is a hit with the data.
            write_now(&cache, &root, "rebuild", &["B.BIN".into()], &sections, crate::BUILD_ID).unwrap();
            let c = open().expect("the rewritten file opens");
            assert!(c.bytes("data").unwrap()[..] == data[..]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// unwrap audit: a cache folder that cannot be made or written
    /// (a file in its place: read-only for any user) is a miss on open and
    /// an error on write, never a panic; the game then parses its files.
    #[test]
    fn an_unwritable_cache_folder_is_a_miss() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-blocked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("C.BIN"), b"source").unwrap();
        let cache = dir.join("cache");
        std::fs::write(&cache, b"in the way").unwrap();
        let sections = vec![("data".to_string(), Bytes::from(&b"abc"[..]))];
        assert!(write_now(&cache, &root, "blocked", &["C.BIN".into()], &sections, crate::BUILD_ID).is_err());
        assert!(CacheFile::open_in(&cache, crate::BUILD_ID, &root, "blocked", &["C.BIN"]).is_none());
        assert!(replace_file(&cache.join("pipelines"), b"data").is_err());
        // A source that is gone while caching: an error, not a panic.
        std::fs::remove_file(&cache).unwrap();
        assert!(write_now(&cache, &root, "blocked", &["MISSING.BIN".into()], &sections, crate::BUILD_ID).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A batch of replaced files: invisible until their batch is synced
    /// (every `LIMIT` files, then `commit`), each whole, an old file
    /// replaced, and no temporary file left in the folder.
    #[test]
    fn a_replace_batch_puts_whole_files_in_place() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-batch-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = |i: usize| dir.join(format!("{i:08X}.dds"));
        let data = |i: usize| vec![i as u8; 1000 + i];
        std::fs::write(name(3), b"old").unwrap();
        let n = ReplaceBatch::LIMIT + 6;
        let batch = ReplaceBatch::default();
        for i in 0..n {
            batch.add(&name(i), &data(i)).unwrap();
        }
        assert_eq!(batch.pending(), 6);
        for i in 0..ReplaceBatch::LIMIT {
            assert_eq!(std::fs::read(name(i)).unwrap(), data(i), "file {i} of the synced batch");
        }
        for i in ReplaceBatch::LIMIT..n {
            assert!(!name(i).exists(), "file {i} is visible before its batch is synced");
        }
        batch.commit().unwrap();
        assert_eq!(batch.pending(), 0);
        for i in 0..n {
            assert_eq!(std::fs::read(name(i)).unwrap(), data(i), "file {i}");
        }
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), n, "only the files themselves: {left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Decoding a damaged section fails with `Corrupt`, never panics nor
    /// allocates a bad count: every prefix and every one-byte change of an
    /// encoded value.
    #[test]
    fn damaged_sections_do_not_decode() {
        type V = (Vec<String>, Vec<Option<u64>>, Vec<Vec<f32>>);
        let v: V = (vec!["a".into(), "bcd".into()], vec![None, Some(7)], vec![vec![1.0, 2.0], vec![]]);
        let mut w = Writer::default();
        v.0.put(&mut w);
        v.1.put(&mut w);
        v.2.put(&mut w);
        let good = w.buf.clone();
        let get = |b: &[u8]| -> Option<V> {
            let mut r = Reader::new(b);
            let v = (Vec::get(&mut r).ok()?, Vec::get(&mut r).ok()?, Vec::get(&mut r).ok()?);
            r.done().then_some(v)
        };
        assert_eq!(get(&good), Some(v));
        for n in 0..good.len() {
            assert!(get(&good[..n]).is_none(), "a {n}-byte prefix decodes");
        }
        for i in 0..good.len() {
            for x in [0x01u8, 0x80, 0xFF] {
                let mut b = good.clone();
                b[i] ^= x;
                let _ = get(&b);
            }
        }
    }

    /// A write leaves only the finished file; stale temporary files of dead
    /// writers are swept, a live (locked) writer's and a young one are not.
    #[test]
    fn temporary_files_do_not_pile_up() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-tmp-unit-{}", std::process::id()));
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("A.BIN"), b"source").unwrap();
        let cache = dir.join("cache");
        write_now(&cache, &root, "unit", &["A.BIN".into()], &[("data".to_string(), Bytes::from(&b"abc"[..]))], crate::BUILD_ID).unwrap();
        let names = || {
            let mut v: Vec<String> = std::fs::read_dir(&cache)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        assert_eq!(names(), vec![file_name("unit")]);
        let old = std::time::SystemTime::now() - STALE_TMP - std::time::Duration::from_secs(60);
        let make = |n: &str, age: std::time::SystemTime| {
            let f = std::fs::OpenOptions::new().create(true).write(true).open(cache.join(n)).unwrap();
            f.set_modified(age).unwrap();
            f
        };
        assert!(
            is_tmp_name("track-X.nfc.1.0.tmp")
                && is_tmp_name("wgpu_pipeline_cache_vulkan_1002.tmp77")
                && !is_tmp_name("track-X.nfc")
                && !is_tmp_name("x.tmp")
                && !is_tmp_name("a.b.tmp")
        );
        drop(make("track-X.nfc.1.0.tmp", old));
        let held = make("track-Y.nfc.2.0.tmp", old);
        held.try_lock().unwrap();
        drop(make("track-Z.nfc.3.0.tmp", std::time::SystemTime::now()));
        sweep_tmp(&cache);
        assert_eq!(
            names(),
            vec!["track-Y.nfc.2.0.tmp".to_string(), "track-Z.nfc.3.0.tmp".to_string(), file_name("unit")]
        );
        drop(held);
        sweep_tmp(&cache);
        assert_eq!(names(), vec!["track-Z.nfc.3.0.tmp".to_string(), file_name("unit")]);
        // The names TempFile gives today (`<name>.<pid>-<hex>-<seq>.<seq>.tmp`,
        // left behind by a kill between link and rename) are swept too.
        let (f, t) = TempFile::create(&cache, "track-W.nfc").unwrap();
        let given = match &t {
            #[cfg(target_os = "linux")]
            TempFile::Anonymous(p) => p.clone(),
            TempFile::Named(p) => p.clone(),
        };
        drop(f);
        drop(t);
        let given = given.file_name().unwrap().to_str().unwrap().to_string();
        assert!(given.starts_with("track-W.nfc.") && is_tmp_name(&given), "{given}");
        assert!(
            is_tmp_name("a.nfc.12-1a2b3c-4.5.tmp")
                && !is_tmp_name("a.nfc.12-xyz-4.5.tmp")
                && !is_tmp_name("a.nfc.12-ab.5.tmp")
                && !is_tmp_name("a.nfc.-ab-4.5.tmp")
        );
        drop(make(&given, old));
        assert!(names().contains(&given));
        sweep_tmp(&cache);
        assert_eq!(names(), vec!["track-Z.nfc.3.0.tmp".to_string(), file_name("unit")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two builds never share a cache file: each build's file of a kind
    /// has the build id in its name (and header), so neither finds the
    /// other's stale and rewrites it; each still finds its own.
    #[test]
    fn two_builds_never_share_a_file() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-builds-{}", crate::unique_tag()));
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("A.BIN"), b"source").unwrap();
        let cache = dir.join("cache");
        let (a, b) = (0x1111_2222_3333_4444u64, 0x5555_6666_7777_8888u64);
        let sec = |v: &[u8]| vec![("data".to_string(), Bytes::from(v))];
        write_build(&cache, &root, "kind", &["A.BIN".into()], &sec(b"from a"), a).unwrap();
        write_build(&cache, &root, "kind", &["A.BIN".into()], &sec(b"from b"), b).unwrap();
        assert_ne!(file_name_of("kind", a), file_name_of("kind", b));
        assert_eq!(split_name(&file_name_of("kind-x", a)), Some(("kind-x", a)));
        assert_eq!(split_name("track-L4RA.nfc"), None, "a name of the builds before");
        let got = |build| {
            CacheFile::open_build(&cache, &root, "kind", &["A.BIN"], build).map(|c| c.bytes("data").unwrap().to_vec())
        };
        assert_eq!(got(a).as_deref(), Some(&b"from a"[..]));
        assert_eq!(got(b).as_deref(), Some(&b"from b"[..]));
        // A file under another build's name (copied) is still a miss: the
        // header's build is checked too.
        std::fs::copy(cache.join(file_name_of("kind", a)), cache.join(file_name_of("kind", b))).unwrap();
        assert!(got(b).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The incident this guards against: two builds' loads against one
    /// cache folder, each load of an entry missing and asking for a write
    /// (from several threads, as parallel car loads do). Each build writes
    /// its file once, a request with contents already on disk (another
    /// process of the same build finished it) writes nothing, and no
    /// temporary file is left.
    #[test]
    fn two_builds_write_each_entry_once() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-once-{}", crate::unique_tag()));
        let root = dir.join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("GEOMETRY.BIN"), vec![7u8; 100_000]).unwrap();
        let cache = dir.join("cache");
        let builds = [0xAAAA_0000_0000_0001u64, 0xBBBB_0000_0000_0002u64];
        let job = |build: u64| Job {
            dir: cache.clone(),
            root: root.clone(),
            kind: "file-solids-GEOMETRY.BIN".into(),
            sources: vec!["GEOMETRY.BIN".into()],
            sections: vec![("data".into(), Bytes::from(vec![build as u8; 300_000]))],
            build,
        };
        std::thread::scope(|s| {
            for t in 0..4 {
                let job = &job;
                s.spawn(move || {
                    for i in 0..200 {
                        submit(job(builds[(i + t) % 2]));
                    }
                });
            }
        });
        flush();
        let counts: std::collections::HashMap<_, _> = write_counts().into_iter().collect();
        for build in builds {
            let path = cache.join(file_name_of("file-solids-GEOMETRY.BIN", build));
            let c = counts[&path];
            assert_eq!(c.written, 1, "{build:x}: {c:?}");
            assert_eq!(c.failed, 0);
            assert!(
                CacheFile::open_build(&cache, &root, "file-solids-GEOMETRY.BIN", &["GEOMETRY.BIN"], build).is_some()
            );
        }
        let left: Vec<String> = std::fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp"))
            .collect();
        assert!(left.is_empty(), "temporary files left: {left:?}");
        // Written by another process (here: directly) with the same
        // contents: a request finds it there and writes nothing.
        let other = 0xCCCC_0000_0000_0003u64;
        let j = job(other);
        write_build(&cache, &root, &j.kind, &j.sources, &j.sections, other).unwrap();
        let path = cache.join(file_name_of(&j.kind, other));
        let ino = std::fs::metadata(&path).unwrap().modified().unwrap();
        submit(j);
        flush();
        let c = write_counts().into_iter().find(|(p, _)| *p == path).unwrap().1;
        assert_eq!((c.written, c.present), (0, 1), "{c:?}");
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), ino);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A temporary file whose writer's process is gone goes after
    /// [`DEAD_TMP`]; one of a live process (this one) waits for
    /// [`STALE_TMP`]; a locked one stays.
    #[test]
    fn dead_writers_temporary_files_are_swept() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-dead-{}", crate::unique_tag()));
        std::fs::create_dir_all(&dir).unwrap();
        let ago = |s: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(s);
        let make = |n: &str, t: std::time::SystemTime| {
            let f = std::fs::OpenOptions::new().create(true).write(true).open(dir.join(n)).unwrap();
            f.set_modified(t).unwrap();
            f
        };
        let me = std::process::id();
        // No process has this id (above Linux's largest).
        let gone = 4_194_305 + 7;
        drop(make(&format!("a.nfc.{gone}-1f-0.0.tmp"), ago(60)));
        drop(make(&format!("b.nfc.{gone}-1f-1.1.tmp"), ago(5)));
        drop(make(&format!("c.nfc.{me}-1f-2.2.tmp"), ago(60)));
        drop(make(&format!("0123456789abcdef.tmp{gone}-3"), ago(60)));
        let held = make(&format!("d.nfc.{gone}-1f-4.4.tmp"), ago(60));
        held.try_lock().unwrap();
        assert_eq!(tmp_pid(&format!("a.nfc.{gone}-1f-0.0.tmp")), Some(gone));
        assert_eq!(tmp_pid("0123456789abcdef.tmp42-3"), Some(42));
        assert_eq!(tmp_pid("0123456789abcdef.rg8"), None);
        sweep_tmp(&dir);
        let mut names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        let mut want = vec![format!("b.nfc.{gone}-1f-1.1.tmp"), format!("d.nfc.{gone}-1f-4.4.tmp")];
        if cfg!(target_os = "linux") {
            want.push(format!("c.nfc.{me}-1f-2.2.tmp"));
        } else {
            // Elsewhere a writer's process is never known gone: only age.
            want.push(format!("0123456789abcdef.tmp{gone}-3"));
            want.push(format!("a.nfc.{gone}-1f-0.0.tmp"));
            want.push(format!("c.nfc.{me}-1f-2.2.tmp"));
        }
        want.sort();
        assert_eq!(names, want);
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Over the size limit, other builds' files go first, then this
    /// build's, oldest first; the file just written and every file younger
    /// than [`PRUNE_GRACE`] stay; temporary files count.
    #[test]
    fn prune_order_and_grace() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-prune-{}", crate::unique_tag()));
        std::fs::create_dir_all(&dir).unwrap();
        let ago = |s: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(s);
        let make = |n: &str, t: std::time::SystemTime| {
            let f = std::fs::OpenOptions::new().create(true).write(true).open(dir.join(n)).unwrap();
            f.set_len(1 << 20).unwrap();
            f.set_modified(t).unwrap();
        };
        let own = crate::BUILD_ID.0;
        let other = own ^ 1;
        make(&file_name_of("own-old", own), ago(3600 * 5));
        make(&file_name_of("own-mid", own), ago(3600 * 2));
        make(&file_name_of("other-new", other), ago(3600));
        make("legacy.nfc", ago(1800));
        make(&file_name_of("own-young", own), ago(10));
        make(&file_name_of("other-young", other), ago(10));
        make(&file_name_of("kept", own), ago(3600 * 9));
        make("x.nfc.1-1f-0.0.tmp", ago(1));
        let names = || {
            let mut v: Vec<String> = std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        // 8 MB; to 5 MB: the other build's old file, the old-named one and
        // this build's oldest go.
        prune_to(&dir, &file_name_of("kept", own), 5 << 20);
        let mut want = vec![
            file_name_of("own-mid", own),
            file_name_of("own-young", own),
            file_name_of("other-young", other),
            file_name_of("kept", own),
            "x.nfc.1-1f-0.0.tmp".to_string(),
        ];
        want.sort();
        assert_eq!(names(), want);
        // To nothing: only what the grace and `keep` protect is left.
        prune_to(&dir, &file_name_of("kept", own), 0);
        want.retain(|n| *n != file_name_of("own-mid", own));
        assert_eq!(names(), want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Once a build has its own entry of a kind, the other builds' entries
    /// of that kind (and its old-named one) go when unused for
    /// [`SUPERSEDED_AFTER`], whatever the folder's size; a kind it has no
    /// entry of, its own entries, entries used within the hour, temporary
    /// files, the surface maps' folder and links stay. A write prunes its
    /// own kind only.
    #[test]
    fn superseded_entries_of_other_builds_go() {
        let base = std::env::temp_dir().join(format!("nfsu2-cache-superseded-{}", crate::unique_tag()));
        let dir = base.join("cache");
        let outside = base.join("outside");
        std::fs::create_dir_all(dir.join(SURFACE_MAPS)).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let ago = |s: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(s);
        let make = |p: std::path::PathBuf, t: std::time::SystemTime| {
            let f = std::fs::OpenOptions::new().create(true).write(true).open(p).unwrap();
            f.set_len(1000).unwrap();
            f.set_modified(t).unwrap();
        };
        let (own, old, older) = (0x1111_0000_0000_0001u64, 0x2222_0000_0000_0002u64, 0x3333_0000_0000_0003u64);
        let day = 24 * 3600;
        make(dir.join(file_name_of("track-L4RA", own)), ago(day));
        make(dir.join(file_name_of("track-L4RA", old)), ago(2 * 3600));
        make(dir.join(file_name_of("track-L4RA", older)), ago(day));
        make(dir.join("track-L4RA.nfc"), ago(day));
        make(dir.join(file_name_of("file-solids-GEOMETRY.BIN-00000000000000aa", own)), ago(10));
        // Used ten minutes ago by the other build: kept.
        make(dir.join(file_name_of("file-solids-GEOMETRY.BIN-00000000000000aa", old)), ago(600));
        // No entry of this build: kept (the size limit's).
        make(dir.join(file_name_of("track-L4RB", old)), ago(day));
        make(dir.join(SURFACE_MAPS).join(file_name_of("track-L4RA", old)), ago(day));
        make(dir.join(format!("{}.1-1f-0.0.tmp", file_name_of("track-L4RA", old))), ago(day));
        let target = outside.join(file_name_of("track-L4RA", older ^ 7));
        make(target.clone(), ago(day));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, dir.join(file_name_of("track-L4RA", older ^ 7))).unwrap();
        let names = || {
            let mut v: Vec<String> =
                std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            v.sort();
            v
        };
        let all = names();
        // Another build that has no entry of its own: nothing goes.
        assert_eq!(prune_superseded(&dir, 0x4444, None, SUPERSEDED_AFTER), 0);
        assert_eq!(names(), all);
        // A write of this build's other kind prunes only that kind.
        assert_eq!(prune_superseded(&dir, own, Some("track-L4RB"), SUPERSEDED_AFTER), 0);
        assert_eq!(names(), all);
        // Its track: the other builds' unused ones and the old-named one go.
        assert_eq!(prune_superseded(&dir, own, Some("track-L4RA"), SUPERSEDED_AFTER), 3000);
        let mut want: Vec<String> = all
            .iter()
            .filter(|n| {
                **n != file_name_of("track-L4RA", old)
                    && **n != file_name_of("track-L4RA", older)
                    && *n != "track-L4RA.nfc"
            })
            .cloned()
            .collect();
        assert_eq!(names(), want);
        // Every kind (the first open of a run): the car file used ten
        // minutes ago stays, until it has not been used for the hour.
        assert_eq!(prune_superseded(&dir, own, None, SUPERSEDED_AFTER), 0);
        assert_eq!(names(), want);
        assert_eq!(prune_superseded(&dir, own, None, std::time::Duration::from_secs(300)), 1000);
        want.retain(|n| *n != file_name_of("file-solids-GEOMETRY.BIN-00000000000000aa", old));
        assert_eq!(names(), want);
        // Untouched: this build's files, the other kind, the maps' folder,
        // the temporary file, the link and what it points to.
        assert!(dir.join(file_name_of("track-L4RA", own)).exists());
        assert!(dir.join(file_name_of("track-L4RB", old)).exists());
        assert!(dir.join(SURFACE_MAPS).join(file_name_of("track-L4RA", old)).exists());
        assert!(target.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The surface maps' folder counts against the size limit: its files
    /// with bytes are evicted with this build's cache files, the one used
    /// longest ago first; its empty `.none` files, its young files and its
    /// temporary files stay (the temporary ones counted).
    #[test]
    fn prune_counts_the_surface_maps() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-prune-maps-{}", crate::unique_tag()));
        let maps = dir.join(SURFACE_MAPS);
        std::fs::create_dir_all(&maps).unwrap();
        let ago = |s: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(s);
        let make = |p: std::path::PathBuf, len: u64, t: std::time::SystemTime| {
            let f = std::fs::OpenOptions::new().create(true).write(true).open(p).unwrap();
            f.set_len(len).unwrap();
            f.set_modified(t).unwrap();
        };
        let own = crate::BUILD_ID.0;
        make(dir.join(file_name_of("world", own)), 1 << 20, ago(3600));
        make(maps.join("00000000000000a1.rg8"), 1 << 20, ago(3600 * 3));
        make(maps.join("car-00000000000000a2.rg8"), 1 << 20, ago(3600 * 2));
        make(maps.join("00000000000000a3.rg8"), 1 << 20, ago(10));
        make(maps.join("00000000000000a4.none"), 0, ago(3600 * 9));
        make(maps.join("00000000000000a5.tmp1-0"), 1 << 20, ago(1));
        let left = |d: &Path| {
            let mut v: Vec<String> =
                std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            v.sort();
            v
        };
        // 5 MB in all, 4 of them in the maps' folder. Under a 5 MB limit
        // nothing goes (the maps were not counted before: 1 MB).
        prune_to(&dir, &file_name_of("none", own), 5 << 20);
        assert_eq!(left(&maps).len(), 5);
        // To 3 MB: the two oldest maps go, before the newer cache file.
        prune_to(&dir, &file_name_of("none", own), 3 << 20);
        assert_eq!(left(&maps), ["00000000000000a3.rg8", "00000000000000a4.none", "00000000000000a5.tmp1-0"]);
        assert!(dir.join(file_name_of("world", own)).exists());
        // To nothing: the cache file goes too; young, empty and temporary
        // files stay.
        prune_to(&dir, &file_name_of("none", own), 0);
        assert!(!dir.join(file_name_of("world", own)).exists());
        assert_eq!(left(&maps), ["00000000000000a3.rg8", "00000000000000a4.none", "00000000000000a5.tmp1-0"]);
        // A read map is marked as used: an hour-old one is now young.
        let old = maps.join("00000000000000b1.rg8");
        make(old.clone(), 16, ago(3600));
        touch_file(&old);
        // The mark is made on the toucher thread: waited for, as the
        // cache's flush and the game's exit wait.
        assert_eq!(wait_touches(Some(std::time::Duration::from_secs(10))), 0);
        let age = std::fs::metadata(&old).unwrap().modified().unwrap().elapsed().unwrap_or_default();
        assert!(age < PRUNE_GRACE, "{age:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gathered bytes are the pieces back to back, read lazily; pieces
    /// outside the mapping are refused.
    #[test]
    fn gathered_bytes() {
        let dir = std::env::temp_dir().join(format!("nfsu2-cache-gather-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("g");
        std::fs::write(&path, (0..=255u8).collect::<Vec<u8>>()).unwrap();
        let map = std::sync::Arc::new(unsafe { memmap2::Mmap::map(&std::fs::File::open(&path).unwrap()) }.unwrap());
        let pieces = vec![
            GatherPiece { start: 10, row: 2, stride: 5, rows: 3 },
            GatherPiece { start: 200, row: 4, stride: 4, rows: 1 },
        ];
        let b = Bytes::gathered(map.clone(), pieces).unwrap();
        assert_eq!(b.len(), 10);
        assert_eq!(&b[..], &[10, 11, 15, 16, 20, 21, 200, 201, 202, 203]);
        assert!(Bytes::gathered(map.clone(), vec![GatherPiece { start: 250, row: 4, stride: 4, rows: 2 }]).is_none());
        assert!(Bytes::gathered(map, vec![GatherPiece { start: 0, row: 8, stride: 4, rows: 2 }]).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Bad counts and short data fail to decode instead of allocating or
    /// panicking.
    #[test]
    fn bad_data_does_not_decode() {
        let mut w = Writer::default();
        w.u64(u64::MAX);
        assert!(Vec::<u32>::get(&mut Reader::new(&w.buf)).is_err());
        assert!(String::get(&mut Reader::new(&w.buf)).is_err());
        assert!(Option::<u32>::get(&mut Reader::new(&[7])).is_err());
        assert!(bool::get(&mut Reader::new(&[2])).is_err());
        assert!(<[u32; 4]>::get(&mut Reader::new(&[1, 2, 3])).is_err());
    }
}
