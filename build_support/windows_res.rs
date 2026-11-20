//! The Windows executable's resources (build.rs, Windows targets only): the
//! application icon, the version information and the application manifest,
//! compiled with the target's resource compiler and linked into the player's
//! programs: the game (`game.exe`) and the launcher (`overbright.exe`) from
//! build.rs, the map editor (`overbright-editor.exe`) from
//! crates/ob-studio-gui/build.rs, Studio's command-line tool
//! (`overbright-studio-cli.exe`) from crates/ob-studio/build.rs. The tool
//! binaries get none.
//!
//! The icon is drawn in code (build_support/icon_art.rs, shared with the
//! window icon, `platform::icon`): Overbright's mark, the launcher's lime
//! ring on a dark tile in the launcher's page colour; our own art.

use std::path::Path;

use crate::icon_art::draw;

/// An .ico file with 32-bit BMP entries at the usual sizes.
pub fn ico() -> Vec<u8> {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&n| bmp_entry(n, &draw(n))).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&n, img) in sizes.iter().zip(&images) {
        let dim = if n >= 256 { 0 } else { n as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

/// One icon image: BITMAPINFOHEADER (height doubled for the mask), BGRA rows
/// bottom-up, then an all-clear 1-bit AND mask (the alpha channel rules).
fn bmp_entry(n: u32, px: &[[u8; 4]]) -> Vec<u8> {
    let mask_row = n.div_ceil(32) * 4;
    let mut out = Vec::new();
    for v in [40u32, n, 2 * n] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    for v in [0u32, n * n * 4 + mask_row * n, 0, 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for y in (0..n).rev() {
        for x in 0..n {
            let [r, g, b, a] = px[(y * n + x) as usize];
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    out.resize(out.len() + (mask_row * n) as usize, 0);
    out
}

/// Escapes a string for an .rc string literal.
fn rc_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\"\"")
}

/// The application manifest of one program, `name` being its assembly
/// identity (docs/playing/platform-windows.md): Windows 10/11
/// compatibility, per-monitor DPI awareness v2 (winit asks for it at
/// run time too; the manifest makes it hold before the first window and
/// for every dialog), long paths (with the system's LongPathsEnabled
/// policy, paths past MAX_PATH work in the W-APIs std uses), and UTF-8 as
/// the process's ANSI code page (any A-API a dependency still calls gets
/// UTF-8, not the locale's code page).
pub fn manifest(name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="{name}" version="1.0.0.0" processorArchitecture="amd64"/>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and 11 -->
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
"#
    )
}

/// One program that gets the resources: its cargo bin name, its manifest
/// identity and its version resource's description.
struct Program {
    bin: &'static str,
    identity: &'static str,
    description: &'static str,
}

/// The player's programs (the archives of scripts/release/package.sh) and
/// Studio's command-line tool.
const PROGRAMS: [Program; 4] = [
    Program { bin: "game", identity: "Overbright.Game", description: "Overbright" },
    Program { bin: "overbright", identity: "Overbright.Launcher", description: "Overbright Launcher" },
    Program { bin: "overbright-editor", identity: "Overbright.MapEditor", description: "Overbright Map Editor" },
    Program {
        bin: "overbright-studio-cli",
        identity: "Overbright.StudioCli",
        description: "Overbright Studio command line",
    },
];

/// Writes the icon, the manifests and the resource scripts into `out_dir`,
/// compiles them with the target's resource compiler and links each result
/// into its program (those of `PROGRAMS` named in `bins`: the calling
/// package's). A missing resource compiler is a warning,
/// not an error: the programs then have no icon, no version information and
/// no manifest.
pub fn build(out_dir: &Path, version: &str, commit: &str, bins: &[&str]) {
    let ico = out_dir.join("overbright.ico");
    std::fs::write(&ico, self::ico()).expect("write overbright.ico");
    for p in PROGRAMS.iter().filter(|p| bins.contains(&p.bin)) {
        build_one(out_dir, &ico, p, version, commit);
    }
}

fn build_one(out_dir: &Path, ico: &Path, p: &Program, version: &str, commit: &str) {
    let manifest = out_dir.join(format!("{}.manifest", p.bin));
    std::fs::write(&manifest, self::manifest(p.identity)).expect("write the manifest");
    // FILEVERSION wants four numbers: the package version's.
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let mut nums: Vec<u16> = pkg.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    nums.resize(4, 0);
    let fv = format!("{},{},{},{}", nums[0], nums[1], nums[2], nums[3]);
    let rc = format!(
        "1 ICON \"{ico}\"\n\
         1 24 \"{manifest}\"\n\
         1 VERSIONINFO\n\
         FILEVERSION {fv}\n\
         PRODUCTVERSION {fv}\n\
         FILEOS 0x40004\n\
         FILETYPE 0x1\n\
         BEGIN\n\
         \x20 BLOCK \"StringFileInfo\"\n\
         \x20 BEGIN\n\
         \x20   BLOCK \"040904B0\"\n\
         \x20   BEGIN\n\
         \x20     VALUE \"ProductName\", \"Overbright\"\n\
         \x20     VALUE \"FileDescription\", \"{description}\"\n\
         \x20     VALUE \"ProductVersion\", \"{version}\"\n\
         \x20     VALUE \"FileVersion\", \"{version} ({commit})\"\n\
         \x20     VALUE \"OriginalFilename\", \"{bin}.exe\"\n\
         \x20     VALUE \"InternalName\", \"{bin}\"\n\
         \x20     VALUE \"LegalCopyright\", \"GPL-3.0-or-later\"\n\
         \x20   END\n\
         \x20 END\n\
         \x20 BLOCK \"VarFileInfo\"\n\
         \x20 BEGIN\n\
         \x20   VALUE \"Translation\", 0x409, 1200\n\
         \x20 END\n\
         END\n",
        ico = rc_str(&ico.to_string_lossy().replace('\\', "/")),
        manifest = rc_str(&manifest.to_string_lossy().replace('\\', "/")),
        description = rc_str(p.description),
        version = rc_str(version),
        commit = rc_str(commit),
        bin = p.bin,
    );
    let rc_path = out_dir.join(format!("{}.rc", p.bin));
    std::fs::write(&rc_path, rc).expect("write the resource script");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let (obj, status) = if msvc {
        let res = out_dir.join(format!("{}.res", p.bin));
        let rc_exe = std::env::var("RC").unwrap_or_else(|_| "rc.exe".into());
        let s = std::process::Command::new(&rc_exe).arg("/nologo").arg("/fo").arg(&res).arg(&rc_path).status();
        (res, s)
    } else {
        let o = out_dir.join(format!("{}-res.o", p.bin));
        let windres = std::env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
        let s = std::process::Command::new(&windres)
            .args(["--input-format=rc", "--output-format=coff", "-i"])
            .arg(&rc_path)
            .arg("-o")
            .arg(&o)
            .status();
        (o, s)
    };
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bin={}={}", p.bin, obj.display()),
        other => println!(
            "cargo:warning=no Windows resources (icon, version, manifest) for {}: resource compiler failed: {other:?}",
            p.bin
        ),
    }
}
