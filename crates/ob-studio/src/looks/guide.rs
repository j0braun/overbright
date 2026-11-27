//! The guide of "Repaint from blank": a faint outline of the original texture's layout, drawn over the blank
//! canvas in Studio's Looks view while you paint. It is made from the original's pixels on the fly and only ever
//! drawn: it is never written to the work file, the image store or the pack data, so a texture painted from
//! blank stays clean (and stays marked `blank`).

/// The guide's line colour (a dark grey; the canvas is white).
pub const LINE: [u8; 3] = [60, 60, 70];

/// The strongest a guide line is drawn (of 255): faint, so it guides without taking over.
pub const MAX_ALPHA: u8 = 150;

/// The outline of `rgba` (`width` × `height`, 4 bytes a pixel) as RGBA of the same size: the edges of its
/// layout in [`LINE`], their strength as the alpha (at most [`MAX_ALPHA`]), everything else transparent. Returns
/// None when the pixels do not fill the size.
pub fn outline(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return None;
    }
    // Luma, with transparent pixels counting as white (the canvas under them).
    let luma: Vec<f32> = rgba
        .chunks_exact(4)
        .map(|p| {
            let l = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
            let a = p[3] as f32 / 255.0;
            l * a + 255.0 * (1.0 - a)
        })
        .collect();
    let at = |x: isize, y: isize| luma[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let mut mag = vec![0f32; w * h];
    let mut top = 0f32;
    for y in 0..h as isize {
        for x in 0..w as isize {
            // The strongest step to a neighbour: a one-pixel line shows as itself, not only beside it.
            let c = at(x, y);
            let mut m = 0f32;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                m = m.max((at(x + dx, y + dy) - c).abs());
            }
            mag[y as usize * w + x as usize] = m;
            top = top.max(m);
        }
    }
    let mut out = vec![0u8; w * h * 4];
    // A flat texture has no layout to show: no lines. Else lines below a fifth of the strongest edge are noise.
    if top < 24.0 {
        return Some(out);
    }
    for (i, m) in mag.iter().enumerate() {
        let k = ((m / top - 0.2) / 0.8).clamp(0.0, 1.0);
        if k > 0.0 {
            let a = (k * MAX_ALPHA as f32).round() as u8;
            out[i * 4..i * 4 + 3].copy_from_slice(&LINE);
            out[i * 4 + 3] = a;
        }
    }
    Some(out)
}
