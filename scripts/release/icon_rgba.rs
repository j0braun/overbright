// The icon as raw RGBA (scripts/release/package.sh makes the Linux PNG from it): the same drawing as the
// window icon and the Windows .ico (build_support/icon_art.rs; our own art, nothing from the game).
//   rustc -O icon_rgba.rs && ./icon_rgba 256 > icon.rgba      (n*n*4 bytes, rows top to bottom)
#[path = "../../build_support/icon_art.rs"]
mod icon_art;

use std::io::Write;

fn main() {
    let n: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(256);
    let px = icon_art::draw(n);
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        out.extend_from_slice(&p);
    }
    std::io::stdout().write_all(&out).expect("write");
}
