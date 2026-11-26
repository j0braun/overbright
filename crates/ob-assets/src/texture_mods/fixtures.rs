//! The texture-mod tests' fixtures (a fresh folder, a game texture, a PNG, a ZIP, a TexWizard pack made here),
//! also used by the library's tests that run the launcher and the pack loader on them.

use std::path::PathBuf;

use crate::formats::texture::{Blend, PixelFormat, Texture};
use crate::formats::tpf;

/// A fresh folder for one test.
pub fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-texture-mods-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A game texture of `w` × `h` RGBA8 texels of one colour.
pub fn texture(name: &str, w: u32, h: u32, rgba: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        hash: crate::formats::hash::bin_hash(name),
        width: w,
        height: h,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: rgba.repeat((w * h) as usize).into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Default::default(),
    }
}

/// A PNG of `w` × `h` texels with a gradient (not the original's pixels).
pub fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 16) as u8, (y * 16) as u8, 200, 255]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A plain (stored, unencrypted, unmasked) ZIP of `entries` with a
/// comment: what `tpf::read` reads like a TexMod package.
pub fn zip(entries: &[(&str, Vec<u8>)], comment: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let crc = tpf::zip_crc(data);
        let at = out.len() as u32;
        let header = |sig: &[u8], central: bool| {
            let mut h = sig.to_vec();
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes()); // version needed
            h.extend(0u16.to_le_bytes()); // flags
            h.extend(0u16.to_le_bytes()); // stored
            h.extend(0u32.to_le_bytes()); // time, date
            h.extend(crc.to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra
            if central {
                h.extend([0u8; 6]); // comment length, disk, internal attributes
                h.extend(0u32.to_le_bytes()); // external attributes
                h.extend(at.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(b"PK\x03\x04", false));
        out.extend(data);
        central.extend(header(b"PK\x01\x02", true));
    }
    let cd = out.len() as u32;
    out.extend(&central);
    out.extend(b"PK\x05\x06");
    out.extend([0u8; 4]);
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd.to_le_bytes());
    out.extend((comment.len() as u16).to_le_bytes());
    out.extend(comment.as_bytes());
    out
}

pub fn key(t: &Texture) -> u32 {
    crate::tpf_import::texmod_hash(t).unwrap()
}

/// A TexWizard pack made here: ROAD_TEST and CAR_TEST paired with the pack's
/// own textures, one pair whose original the game does not have.
pub fn texwizard_files() -> (Vec<u8>, Vec<u8>) {
    let mut road = texture("_ROAD_TEST", 8, 8, [10, 200, 30, 255]);
    road.format = PixelFormat::Bgra8;
    let mut car = texture("_CAR_TEST", 8, 8, [200, 10, 30, 255]);
    car.format = PixelFormat::Bgra8;
    let meta = br#"{
      "textures": [
        [ "ROAD_TEST", "_ROAD_TEST" ],
        [ "CAR_TEST", "_CAR_TEST" ],
        [ "NOT_A_TEXTURE_OF_THE_GAME", "_ROAD_TEST" ],
      ]
    }"#
    .to_vec();
    (meta, crate::formats::texwizard::tpk_bytes(&[road, car]))
}
