//! Writes or checks the asset format specification's generated parts (docs/modding/asset-format/: rule tables, the
//! manifest's field tables, the schemas made from the code; ob_asset_format::docs).
//!
//! Usage: `cargo run -p ob-asset-format --example asset_format_docs -- --check|--write [DIR]`
//! (DIR defaults to docs/modding/asset-format under the current folder: run it from the repository's root). `--check` exits 1 and lists every difference.
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = match args.first().map(String::as_str) {
        Some("--write") => true,
        Some("--check") => false,
        _ => {
            eprintln!("usage: asset_format_docs --check|--write [DIR]");
            std::process::exit(2);
        }
    };
    // Run from the repository's root (scripts/checks/asset_format.sh does).
    let dir = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(ob_asset_format::docs::DIR));
    let tree = ob_asset_format::docs::load(&dir).unwrap_or_else(|e| {
        eprintln!("{}: {e}", dir.display());
        std::process::exit(2);
    });
    if write {
        let changed = ob_asset_format::docs::generate(&tree).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
        for (path, text) in &changed {
            let p = dir.join(path);
            // Written beside and renamed into place, so a reader never sees half a file.
            let tmp = p.with_extension("tmp-gen");
            std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &p)).unwrap_or_else(|e| {
                eprintln!("{}: {e}", p.display());
                std::process::exit(1);
            });
            println!("wrote {path}");
        }
        println!("ASSET FORMAT DOCS: {} file(s) written", changed.len());
    }
    let problems = ob_asset_format::docs::check(&ob_asset_format::docs::load(&dir).expect("read back"));
    for p in &problems {
        println!("FAIL {p}");
    }
    if problems.is_empty() {
        println!("ASSET FORMAT DOCS: ok (in step with the code)");
    } else {
        println!("ASSET FORMAT DOCS: {} problem(s)", problems.len());
        std::process::exit(1);
    }
}
